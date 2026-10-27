//! Import and export formats (US-IMP-01, US-IMP-03). Pure functions: bytes in, items out. No file,
//! terminal or clock access; the caller supplies the time and a fresh-id generator.
//!
//! Exported plaintext holds every secret in the clear. Callers must warn and re-authenticate first.
mod bundle;

pub use bundle::{Bundle, MAX_BUNDLE_BYTES};

use rahasyakosh_core::item::{Field, Item, ITEM_SCHEMA};
use rahasyakosh_core::totp::Totp;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use zeroize::Zeroizing;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("not a valid CSV file: {0}")]
    Csv(String),
    #[error("{0}")]
    Mapping(String),
    #[error("export failed: {0}")]
    Export(String),
    #[error("not a valid vault bundle: {0}")]
    Bundle(String),
}
pub type Result<T> = std::result::Result<T, Error>;

/// Largest CSV file we will parse.
pub const MAX_IMPORT_BYTES: usize = 50 * 1024 * 1024;

/// Columns of the CSV that [`export_csv`] writes (and [`import_csv`] reads back).
pub const EXPORT_COLUMNS: [&str; 7] = [
    "title", "url", "username", "password", "notes", "totp", "tags",
];

/// What an importer needs to know about a CSV file's columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Col {
    Title,
    Url,
    Username,
    Password,
    Notes,
    Totp,
    Tags,
}

impl Col {
    const ALL: [Col; 7] = [
        Col::Title,
        Col::Url,
        Col::Username,
        Col::Password,
        Col::Notes,
        Col::Totp,
        Col::Tags,
    ];

    fn name(self) -> &'static str {
        match self {
            Col::Title => "title",
            Col::Url => "url",
            Col::Username => "username",
            Col::Password => "password",
            Col::Notes => "notes",
            Col::Totp => "totp",
            Col::Tags => "tags",
        }
    }

    fn from_name(s: &str) -> Option<Col> {
        Col::ALL.into_iter().find(|c| c.name() == s)
    }

    /// Header names (lowercase) used by Chrome, Firefox, Bitwarden, 1Password and others.
    fn aliases(self) -> &'static [&'static str] {
        match self {
            Col::Title => &["title", "name", "account", "item name"],
            Col::Url => &[
                "url",
                "website",
                "web site",
                "login_uri",
                "login url",
                "origin",
            ],
            Col::Username => &[
                "username",
                "user",
                "login",
                "login_username",
                "user name",
                "email",
            ],
            Col::Password => &["password", "pass", "login_password"],
            Col::Notes => &["notes", "note", "extra", "comments"],
            Col::Totp => &["totp", "otp", "otpauth", "login_totp"],
            Col::Tags => &["tags", "folder", "grouping", "group"],
        }
    }
}

/// Result of [`import_csv`]. Nothing has been stored yet; the caller decides what to do with `items`.
#[derive(Debug, Default)]
pub struct ImportReport {
    /// Source flavour guessed from the headers: `"chrome"`, `"firefox"` or `"generic"`.
    pub detected: &'static str,
    /// New items, ready to be sealed into the vault.
    pub items: Vec<Item>,
    /// Rows identical (title, username, URL and password) to an existing item or an earlier row.
    pub skipped_duplicates: usize,
    /// Rows with neither a title nor a URL, or entirely empty.
    pub skipped_empty: usize,
    /// Human-readable notes. Never contain passwords.
    pub warnings: Vec<String>,
}

/// Parse a CSV of logins.
///
/// Columns are recognised by header name (case-insensitive) using the aliases of Chrome, Firefox and other
/// managers. `overrides` maps one of our column names (`title url username password notes totp tags`) to
/// an exact header in the file, for files the aliases do not cover. `existing` is used to skip duplicates.
pub fn import_csv(
    data: &[u8],
    overrides: &[(String, String)],
    existing: &[Item],
    now: i64,
    mut new_id: impl FnMut() -> Uuid,
) -> Result<ImportReport> {
    if data.len() > MAX_IMPORT_BYTES {
        return Err(Error::Csv("file is too large".into()));
    }
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    let mut rdr = csv::ReaderBuilder::new().flexible(true).from_reader(data);
    let headers: Vec<String> = rdr
        .headers()
        .map_err(|e| Error::Csv(e.to_string()))?
        .iter()
        .map(|h| h.trim().to_lowercase())
        .collect();

    let mut map: [Option<usize>; 7] = [None; 7];
    for (i, col) in Col::ALL.into_iter().enumerate() {
        map[i] = headers
            .iter()
            .position(|h| col.aliases().contains(&h.as_str()));
    }
    for (field, header) in overrides {
        let col = Col::from_name(field).ok_or_else(|| {
            Error::Mapping(format!(
                "unknown field '{field}'; choose from: {}",
                Col::ALL.map(Col::name).join(", ")
            ))
        })?;
        let idx = headers
            .iter()
            .position(|h| *h == header.trim().to_lowercase())
            .ok_or_else(|| Error::Mapping(format!("no column named '{header}' in the file")))?;
        map[Col::ALL.iter().position(|c| *c == col).unwrap()] = Some(idx);
    }
    let at = |c: Col| map[Col::ALL.iter().position(|x| *x == c).unwrap()];
    if at(Col::Password).is_none() {
        return Err(Error::Mapping(format!(
            "no password column found in headers [{}]; use --map password=<column>",
            headers.join(", ")
        )));
    }

    let detected = if headers
        .iter()
        .any(|h| h == "httprealm" || h == "formactionorigin")
    {
        "firefox"
    } else if ["name", "url", "username", "password"]
        .iter()
        .all(|h| headers.iter().any(|x| x == h))
    {
        "chrome"
    } else {
        "generic"
    };

    let mut report = ImportReport {
        detected,
        ..Default::default()
    };
    // Passwords already seen for each (title, username, URL): one lookup per row, however many rows there are
    // (audit finding P5-1; scanning every earlier row made a large file take quadratic time).
    let mut seen: HashMap<LoginKey, HashSet<String>> = HashMap::new();
    for (login, password) in existing
        .iter()
        .filter(|i| i.deleted_at.is_none())
        .map(dedupe_key)
    {
        seen.entry(login).or_default().insert(password);
    }
    let mut similar = 0usize;
    let mut bad_totp = 0usize;

    for (n, rec) in rdr.records().enumerate() {
        let rec = rec.map_err(|e| Error::Csv(format!("row {}: {e}", n + 2)))?;
        // Titles, URLs, TOTP secrets and tags are tidied. Username, password and notes are kept exactly as
        // they are in the file: altering a secret on import (even stray spaces) would be a silent bug.
        let raw = |c: Col| -> String {
            at(c)
                .and_then(|i| rec.get(i))
                .map(String::from)
                .unwrap_or_default()
        };
        let get = |c: Col| -> String { raw(c).trim().to_string() };
        let (url, mut title) = (get(Col::Url), get(Col::Title));
        if title.is_empty() && url.is_empty() {
            report.skipped_empty += 1;
            continue;
        }
        if title.is_empty() {
            title = host_of(&url);
        }
        let mut item = Item::login(
            &title,
            &raw(Col::Username),
            &raw(Col::Password),
            now,
            new_id(),
        );
        if !url.is_empty() {
            item.urls.push(url);
        }
        item.notes = raw(Col::Notes);
        let totp = get(Col::Totp);
        if !totp.is_empty() {
            let ok = if totp.starts_with("otpauth://") {
                Totp::from_uri(&totp).is_ok()
            } else {
                Totp::from_base32(&totp).is_ok()
            };
            if ok {
                item.fields.push(Field::secret("totp", &totp));
            } else {
                bad_totp += 1;
            }
        }
        item.tags = get(Col::Tags)
            .split([';', ','])
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(String::from)
            .collect();

        let (login, password) = dedupe_key(&item);
        let passwords = seen.entry(login).or_default();
        if passwords.contains(&password) {
            report.skipped_duplicates += 1;
            continue;
        }
        if !passwords.is_empty() {
            similar += 1;
        }
        passwords.insert(password);
        report.items.push(item);
    }

    if similar > 0 {
        report.warnings.push(format!(
            "{similar} row(s) match an existing login but have a different password; imported as new items"
        ));
    }
    if bad_totp > 0 {
        report.warnings.push(format!(
            "{bad_totp} row(s) had an unreadable TOTP secret; imported without it"
        ));
    }
    Ok(report)
}

/// Which login a row is about: title, username, first URL.
type LoginKey = (String, String, String);

/// Identity of a login for duplicate detection: the [`LoginKey`] and the password.
fn dedupe_key(i: &Item) -> (LoginKey, String) {
    let value = |n: &str| i.field(n).map(|f| f.value.clone()).unwrap_or_default();
    (
        (
            i.title.trim().to_lowercase(),
            value("username").to_lowercase(),
            i.urls
                .first()
                .map(|u| u.trim().trim_end_matches('/').to_lowercase())
                .unwrap_or_default(),
        ),
        value("password"),
    )
}

/// Best-effort display name from a URL: `https://www.example.com/login` -> `example.com`.
fn host_of(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |x| x.1);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    let host = host.rsplit('@').next().unwrap_or(host);
    let host = host.strip_prefix("www.").unwrap_or(host);
    if host.is_empty() {
        "(untitled)".into()
    } else {
        host.into()
    }
}

/// Export live (not trashed) items as CSV with the columns in [`EXPORT_COLUMNS`].
///
/// CSV is lossy: custom fields and card or identity details are not included. Use [`export_json`] for a
/// complete export. Cells are written verbatim, so a spreadsheet may treat a value starting with `=`, `+`,
/// `-` or `@` as a formula; open exports in a text editor, not a spreadsheet.
pub fn export_csv(items: &[Item]) -> Result<Zeroizing<String>> {
    let mut w = csv::Writer::from_writer(Vec::new());
    w.write_record(EXPORT_COLUMNS)
        .map_err(|e| Error::Export(e.to_string()))?;
    for i in items.iter().filter(|i| i.deleted_at.is_none()) {
        let value = |n: &str| i.field(n).map(|f| f.value.as_str()).unwrap_or("");
        w.write_record([
            i.title.as_str(),
            i.urls.first().map(String::as_str).unwrap_or(""),
            value("username"),
            value("password"),
            i.notes.as_str(),
            value("totp"),
            &i.tags.join(";"),
        ])
        .map_err(|e| Error::Export(e.to_string()))?;
    }
    let bytes = w.into_inner().map_err(|e| Error::Export(e.to_string()))?;
    let bytes = Zeroizing::new(bytes);
    Ok(Zeroizing::new(
        String::from_utf8(bytes.to_vec()).map_err(|e| Error::Export(e.to_string()))?,
    ))
}

/// Export live items as JSON, including all fields.
pub fn export_json(items: &[Item]) -> Result<Zeroizing<String>> {
    #[derive(serde::Serialize)]
    struct Doc<'a> {
        format: &'static str,
        version: u16,
        items: Vec<&'a Item>,
    }
    let doc = Doc {
        format: "rahasyakosh-export",
        version: ITEM_SCHEMA,
        items: items.iter().filter(|i| i.deleted_at.is_none()).collect(),
    };
    serde_json::to_string_pretty(&doc)
        .map(Zeroizing::new)
        .map_err(|e| Error::Export(e.to_string()))
}

/// Export live (not trashed) items as a plain text file a person can read: one block per item, sorted by title, with
/// every field written out.
///
/// The text starts with a warning, because the file holds every password unprotected. Values that run over several
/// lines (notes) are indented so a block stays easy to scan.
pub fn export_txt(items: &[Item]) -> Zeroizing<String> {
    fn put(out: &mut String, label: &str, value: &str) {
        let mut lines = value.lines();
        out.push_str(label);
        out.push_str(": ");
        out.push_str(lines.next().unwrap_or(""));
        for line in lines {
            out.push_str("\n    ");
            out.push_str(line);
        }
        out.push('\n');
    }
    let mut live: Vec<&Item> = items.iter().filter(|i| i.deleted_at.is_none()).collect();
    live.sort_by_cached_key(|i| i.title.to_lowercase());
    let mut out = String::from(
        "RAHASYAKOSH EXPORT (PLAIN TEXT)\n\
         Anyone who can read this file can read every password in it.\n\
         Keep it somewhere safe and delete it when you no longer need it.\n",
    );
    out.push_str(&format!("\n{} login(s)\n", live.len()));
    for i in live {
        out.push_str("\n------------------------------------------------------------\n");
        put(&mut out, "Title", &i.title);
        let named = |n: &str| i.field(n).map(|f| f.value.as_str());
        put(&mut out, "Username", named("username").unwrap_or(""));
        put(&mut out, "Password", named("password").unwrap_or(""));
        for url in &i.urls {
            put(&mut out, "Website", url);
        }
        if let Some(totp) = named("totp") {
            put(&mut out, "Two-factor secret", totp);
        }
        for f in i
            .fields
            .iter()
            .filter(|f| !matches!(f.name.as_str(), "username" | "password" | "totp"))
        {
            put(&mut out, &f.name, &f.value);
        }
        if !i.tags.is_empty() {
            put(&mut out, "Tags", &i.tags.join(", "));
        }
        if !i.notes.is_empty() {
            put(&mut out, "Notes", &i.notes);
        }
    }
    Zeroizing::new(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> impl FnMut() -> Uuid {
        let mut n = 0u128;
        move || {
            n += 1;
            Uuid::from_u128(n)
        }
    }

    fn run(csv: &str, existing: &[Item]) -> ImportReport {
        import_csv(csv.as_bytes(), &[], existing, 100, ids()).unwrap()
    }

    #[test]
    fn chrome_format() {
        let r = run(
            "name,url,username,password,note\nGitHub,https://github.com/login,alice,s3cret,work\n",
            &[],
        );
        assert_eq!(r.detected, "chrome");
        let i = &r.items[0];
        assert_eq!(i.title, "GitHub");
        assert_eq!(i.field("username").unwrap().value, "alice");
        assert!(i.field("password").unwrap().concealed);
        assert_eq!(i.urls, ["https://github.com/login"]);
        assert_eq!(i.notes, "work");
    }

    #[test]
    fn firefox_format_derives_title_from_url() {
        let r = run(
            "url,username,password,httpRealm,formActionOrigin,guid,timeCreated\nhttps://www.example.com/a,bob,pw,,https://www.example.com,{x},1\n",
            &[],
        );
        assert_eq!(r.detected, "firefox");
        assert_eq!(r.items[0].title, "example.com");
    }

    #[test]
    fn generic_format_with_tags_and_totp() {
        let r = run(
            "title,url,username,password,notes,totp,tags\nBank,bank.example,me,pw,\"line1\nline2\",JBSWY3DPEHPK3PXP,fin; home\n",
            &[],
        );
        let i = &r.items[0];
        assert_eq!(r.detected, "generic");
        assert_eq!(i.notes, "line1\nline2");
        assert_eq!(i.field("totp").unwrap().value, "JBSWY3DPEHPK3PXP");
        assert_eq!(i.tags, ["fin", "home"]);
    }

    #[test]
    fn secrets_are_not_trimmed() {
        // Found by a property test: surrounding whitespace in a username or password is part of the value.
        let r = run("name,username,password\nA,\" u \",\"\tp w \"\n", &[]);
        assert_eq!(r.items[0].field("username").unwrap().value, " u ");
        assert_eq!(r.items[0].field("password").unwrap().value, "\tp w ");
    }

    #[test]
    fn bom_and_case_insensitive_headers() {
        let r = run("\u{feff}Name,URL,Username,Password\nA,a.test,u,p\n", &[]);
        assert_eq!(r.items.len(), 1);
    }

    #[test]
    fn explicit_mapping() {
        let csv = "site,who,secret\nA,u,p\n";
        assert!(import_csv(csv.as_bytes(), &[], &[], 1, ids()).is_err());
        let map = vec![
            ("title".to_string(), "site".to_string()),
            ("username".to_string(), "who".to_string()),
            ("password".to_string(), "secret".to_string()),
        ];
        let r = import_csv(csv.as_bytes(), &map, &[], 1, ids()).unwrap();
        assert_eq!(r.items[0].title, "A");
        assert_eq!(r.items[0].field("password").unwrap().value, "p");
    }

    #[test]
    fn bad_mapping_is_rejected() {
        let csv = "name,password\nA,p\n";
        let bad_field = vec![("nope".to_string(), "name".to_string())];
        assert!(import_csv(csv.as_bytes(), &bad_field, &[], 1, ids()).is_err());
        let bad_col = vec![("title".to_string(), "missing".to_string())];
        assert!(import_csv(csv.as_bytes(), &bad_col, &[], 1, ids()).is_err());
    }

    #[test]
    fn duplicates_in_file_and_vault_are_skipped() {
        let existing = [Item::login("GitHub", "alice", "pw", 1, Uuid::from_u128(99))];
        let r = run(
            "name,url,username,password\nGitHub,,alice,pw\nSite,,bob,x\nSite,,bob,x\n",
            &existing,
        );
        assert_eq!(r.skipped_duplicates, 2);
        assert_eq!(r.items.len(), 1);
        assert_eq!(r.items[0].title, "Site");
    }

    #[test]
    fn same_login_different_password_is_kept_with_warning() {
        let existing = [Item::login(
            "GitHub",
            "alice",
            "old",
            1,
            Uuid::from_u128(99),
        )];
        let r = run("name,username,password\nGitHub,alice,NEWPW123\n", &existing);
        assert_eq!(r.items.len(), 1);
        assert_eq!(r.warnings.len(), 1);
        assert!(!r.warnings[0].contains("NEWPW123"));
    }

    #[test]
    fn trashed_items_do_not_count_as_duplicates() {
        let mut gone = Item::login("GitHub", "alice", "pw", 1, Uuid::from_u128(99));
        gone.deleted_at = Some(2);
        let r = run("name,username,password\nGitHub,alice,pw\n", &[gone]);
        assert_eq!(r.items.len(), 1);
    }

    #[test]
    fn empty_rows_and_bad_totp() {
        let r = run(
            "name,url,username,password,totp\n,,,,\nA,,u,p,not base32!\n",
            &[],
        );
        assert_eq!(r.skipped_empty, 1);
        assert_eq!(r.items.len(), 1);
        assert!(r.items[0].field("totp").is_none());
        assert_eq!(r.warnings.len(), 1);
    }

    #[test]
    fn warnings_never_contain_secrets() {
        let r = run("name,username,password,totp\nA,u,SECRETPW,bad totp!\n", &[]);
        assert!(r.warnings.iter().all(|w| !w.contains("SECRETPW")));
    }

    #[test]
    fn csv_export_round_trips() {
        let mut a = Item::login("A, \"quoted\"", "u", "p;w", 1, Uuid::from_u128(1));
        a.urls.push("https://a.test".into());
        a.notes = "multi\nline".into();
        a.tags = vec!["x".into(), "y".into()];
        a.fields.push(Field::secret("totp", "JBSWY3DPEHPK3PXP"));
        let mut trashed = Item::login("Gone", "u", "p", 1, Uuid::from_u128(2));
        trashed.deleted_at = Some(5);

        let out = export_csv(&[a.clone(), trashed]).unwrap();
        assert!(!out.contains("Gone"));
        let r = import_csv(out.as_bytes(), &[], &[], 9, ids()).unwrap();
        assert_eq!(r.items.len(), 1);
        let b = &r.items[0];
        assert_eq!(b.title, a.title);
        assert_eq!(b.urls, a.urls);
        assert_eq!(b.notes, a.notes);
        assert_eq!(b.tags, a.tags);
        assert_eq!(b.field("password").unwrap().value, "p;w");
        assert_eq!(b.field("totp").unwrap().value, "JBSWY3DPEHPK3PXP");
    }

    #[test]
    fn json_export_is_complete_and_skips_trash() {
        let mut a = Item::login("A", "u", "pw", 1, Uuid::from_u128(1));
        a.fields.push(Field::text("custom", "v"));
        let mut gone = Item::login("Gone", "u", "p", 1, Uuid::from_u128(2));
        gone.deleted_at = Some(5);
        let out = export_json(&[a, gone]).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["format"], "rahasyakosh-export");
        assert_eq!(v["items"].as_array().unwrap().len(), 1);
        assert_eq!(v["items"][0]["fields"][2]["name"], "custom");
        assert_eq!(v["items"][0]["fields"][1]["value"], "pw");
    }

    #[test]
    fn host_extraction() {
        assert_eq!(host_of("https://www.example.com/login?x=1"), "example.com");
        assert_eq!(host_of("http://user@host.test:8080/p"), "host.test:8080");
        assert_eq!(host_of(""), "(untitled)");
    }

    #[test]
    fn txt_export_lists_live_logins_sorted_with_every_field() {
        let mut b = Item::login("beta", "bob", "pw-b", 1, Uuid::from_u128(2));
        b.urls.push("https://beta.example".into());
        b.tags.push("work".into());
        b.notes = "line one\nline two".into();
        b.set_field("totp", "JBSWY3DPEHPK3PXP", true);
        b.set_field("pin", "4321", true);
        let a = Item::login("Alpha", "al", "pw-a", 2, Uuid::from_u128(1));
        let mut gone = Item::login("Trashed", "t", "pw-t", 3, Uuid::from_u128(3));
        gone.deleted_at = Some(10);
        let text = export_txt(&[b, gone, a]);
        assert!(text.starts_with("RAHASYAKOSH EXPORT (PLAIN TEXT)\n"));
        assert!(text.contains("2 login(s)"));
        assert!(!text.contains("Trashed") && !text.contains("pw-t"));
        assert!(
            text.find("Title: Alpha").unwrap() < text.find("Title: beta").unwrap(),
            "sorted by title, ignoring case"
        );
        for needle in [
            "Username: bob",
            "Password: pw-b",
            "Website: https://beta.example",
            "Two-factor secret: JBSWY3DPEHPK3PXP",
            "pin: 4321",
            "Tags: work",
            "Notes: line one\n    line two",
        ] {
            assert!(text.contains(needle), "missing {needle:?} in\n{}", *text);
        }
    }

    #[test]
    fn txt_export_of_nothing_still_has_the_warning() {
        let text = export_txt(&[]);
        assert!(text.contains("Anyone who can read this file"));
        assert!(text.contains("0 login(s)"));
    }
}
