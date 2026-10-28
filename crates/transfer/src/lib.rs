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
    /// Source flavour guessed from the file: `"chrome"`, `"firefox"`, `"bitwarden"`, `"keepass"`, `"1password"` or
    /// `"generic"`.
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

/// Which of the file's columns hold our fields, as [`import_csv`] would read them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvColumns {
    /// The header of every column, as written in the file (trimmed).
    pub headers: Vec<String>,
    /// For each of our fields (`title url username password notes totp tags`), the index into `headers`, if found.
    pub assigned: Vec<(&'static str, Option<usize>)>,
}

/// Our column names, in the order of [`CsvColumns::assigned`].
pub fn csv_field_names() -> [&'static str; 7] {
    Col::ALL.map(Col::name)
}

fn lowered(headers: &csv::StringRecord) -> Vec<String> {
    headers.iter().map(|h| h.trim().to_lowercase()).collect()
}

/// Matches headers to our columns: by alias first, then by the caller's `overrides` (one of our names, an exact header).
fn resolve_columns(
    headers: &[String],
    overrides: &[(String, String)],
) -> Result<[Option<usize>; 7]> {
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
        let slot = Col::ALL.iter().position(|c| *c == col).unwrap();
        // An empty header means "this field is not in the file", even if an alias would have matched.
        if header.trim().is_empty() {
            map[slot] = None;
            continue;
        }
        let idx = headers
            .iter()
            .position(|h| *h == header.trim().to_lowercase())
            .ok_or_else(|| Error::Mapping(format!("no column named '{header}' in the file")))?;
        map[slot] = Some(idx);
    }
    Ok(map)
}

/// Reads only the header line of a CSV and says which column would be used for each of our fields. A screen can show
/// this, let the user change it, and pass the changes back as `overrides`.
pub fn csv_columns(data: &[u8], overrides: &[(String, String)]) -> Result<CsvColumns> {
    if data.len() > MAX_IMPORT_BYTES {
        return Err(Error::Csv("file is too large".into()));
    }
    let data = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    let mut rdr = csv::ReaderBuilder::new().flexible(true).from_reader(data);
    let record = rdr.headers().map_err(|e| Error::Csv(e.to_string()))?;
    let map = resolve_columns(&lowered(record), overrides)?;
    Ok(CsvColumns {
        headers: record.iter().map(|h| h.trim().to_string()).collect(),
        assigned: Col::ALL.into_iter().map(Col::name).zip(map).collect(),
    })
}

/// Collects imported items, skipping the ones already in the vault or earlier in the file.
struct Collector {
    report: ImportReport,
    /// Passwords already seen for each (title, username, URL): one lookup per row, however many rows there are
    /// (audit finding P5-1; scanning every earlier row made a large file take quadratic time).
    seen: HashMap<LoginKey, HashSet<String>>,
    similar: usize,
    bad_totp: usize,
    not_logins: usize,
}

impl Collector {
    fn new(detected: &'static str, existing: &[Item]) -> Self {
        let mut seen: HashMap<LoginKey, HashSet<String>> = HashMap::new();
        for (login, password) in existing
            .iter()
            .filter(|i| i.deleted_at.is_none())
            .map(dedupe_key)
        {
            seen.entry(login).or_default().insert(password);
        }
        Self {
            report: ImportReport {
                detected,
                ..Default::default()
            },
            seen,
            similar: 0,
            bad_totp: 0,
            not_logins: 0,
        }
    }

    /// Adds `totp` to `item` if it is a readable secret or `otpauth://` link; otherwise counts it as unreadable.
    fn add_totp(&mut self, item: &mut Item, totp: &str) {
        if totp.is_empty() {
            return;
        }
        let ok = if totp.starts_with("otpauth://") {
            Totp::from_uri(totp).is_ok()
        } else {
            Totp::from_base32(totp).is_ok()
        };
        if ok {
            item.fields.push(Field::secret("totp", totp));
        } else {
            self.bad_totp += 1;
        }
    }

    fn push(&mut self, item: Item) {
        let (login, password) = dedupe_key(&item);
        let passwords = self.seen.entry(login).or_default();
        if passwords.contains(&password) {
            self.report.skipped_duplicates += 1;
            return;
        }
        if !passwords.is_empty() {
            self.similar += 1;
        }
        passwords.insert(password);
        self.report.items.push(item);
    }

    fn finish(mut self) -> ImportReport {
        if self.similar > 0 {
            self.report.warnings.push(format!(
                "{} row(s) match an existing login but have a different password; imported as new items",
                self.similar
            ));
        }
        if self.bad_totp > 0 {
            self.report.warnings.push(format!(
                "{} row(s) had an unreadable TOTP secret; imported without it",
                self.bad_totp
            ));
        }
        if self.not_logins > 0 {
            self.report.warnings.push(format!(
                "{} entry(ies) that are not logins (secure notes, cards, identities) were not imported",
                self.not_logins
            ));
        }
        self.report
    }
}

/// Splits a tags or folder cell on `;` and `,`.
fn split_tags(s: &str) -> Vec<String> {
    s.split([';', ','])
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(String::from)
        .collect()
}

/// Parse a CSV of logins.
///
/// Columns are recognised by header name (case-insensitive) using the aliases of Chrome, Firefox, Bitwarden,
/// KeePassXC, 1Password and other managers. `overrides` maps one of our column names (`title url username password
/// notes totp tags`) to an exact header in the file, for files the aliases do not cover. `existing` is used to skip
/// duplicates.
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
    let headers = lowered(rdr.headers().map_err(|e| Error::Csv(e.to_string()))?);
    let map = resolve_columns(&headers, overrides)?;
    let at = |c: Col| map[Col::ALL.iter().position(|x| *x == c).unwrap()];
    if at(Col::Password).is_none() {
        return Err(Error::Mapping(format!(
            "no password column found in headers [{}]; use --map password=<column>",
            headers.join(", ")
        )));
    }
    let has = |name: &str| headers.iter().any(|h| h == name);
    let detected = if has("httprealm") || has("formactionorigin") {
        "firefox"
    } else if has("login_uri") || has("login_username") || has("login_password") {
        "bitwarden"
    } else if has("group") && has("title") && has("url") {
        "keepass"
    } else if has("otpauth") || (has("title") && has("website")) {
        "1password"
    } else if ["name", "url", "username", "password"]
        .iter()
        .all(|h| has(h))
    {
        "chrome"
    } else {
        "generic"
    };
    // Bitwarden and 1Password write secure notes, cards and identities into the same file as logins.
    let type_col = (detected == "bitwarden" || detected == "1password")
        .then(|| headers.iter().position(|h| h == "type"))
        .flatten();

    let mut out = Collector::new(detected, existing);
    for (n, rec) in rdr.records().enumerate() {
        let rec = rec.map_err(|e| Error::Csv(format!("row {}: {e}", n + 2)))?;
        if let Some(kind) = type_col.and_then(|i| rec.get(i)) {
            let kind = kind.trim().to_lowercase();
            if !matches!(kind.as_str(), "" | "login" | "password") {
                out.not_logins += 1;
                continue;
            }
        }
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
            out.report.skipped_empty += 1;
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
        out.add_totp(&mut item, &get(Col::Totp));
        item.tags = split_tags(&get(Col::Tags));
        out.push(item);
    }
    Ok(out.finish())
}

/// Parse a Bitwarden JSON export (not the "encrypted" kind). Only logins are read; secure notes, cards and identities
/// are counted in a warning. Folders become tags.
pub fn import_bitwarden_json(
    data: &[u8],
    existing: &[Item],
    now: i64,
    mut new_id: impl FnMut() -> Uuid,
) -> Result<ImportReport> {
    if data.len() > MAX_IMPORT_BYTES {
        return Err(Error::Bundle("file is too large".into()));
    }
    let root: serde_json::Value =
        serde_json::from_slice(data).map_err(|e| Error::Bundle(e.to_string()))?;
    if root.get("encrypted").and_then(|v| v.as_bool()) == Some(true) {
        return Err(Error::Mapping(
            "this Bitwarden export is encrypted; export it again as an unencrypted .json file"
                .into(),
        ));
    }
    let list = |key: &str| root.get(key).and_then(|v| v.as_array());
    let entries = list("items").ok_or_else(|| {
        Error::Bundle(
            "this JSON file has no \"items\" list, so it is not a Bitwarden export".into(),
        )
    })?;
    let folders: HashMap<&str, &str> = list("folders")
        .into_iter()
        .flatten()
        .filter_map(|f| Some((f.get("id")?.as_str()?, f.get("name")?.as_str()?)))
        .collect();
    let text = |v: &serde_json::Value, key: &str| -> String {
        v.get(key)
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string()
    };

    let mut out = Collector::new("bitwarden", existing);
    for entry in entries {
        // Type 1 is a login; 2 is a secure note, 3 a card and 4 an identity.
        let login = entry
            .get("login")
            .filter(|_| entry.get("type").and_then(|t| t.as_u64()).unwrap_or(1) == 1);
        let Some(login) = login else {
            out.not_logins += 1;
            continue;
        };
        let uris: Vec<String> = login
            .get("uris")
            .and_then(|u| u.as_array())
            .into_iter()
            .flatten()
            .map(|u| text(u, "uri").trim().to_string())
            .filter(|u| !u.is_empty())
            .collect();
        let mut title = text(entry, "name").trim().to_string();
        if title.is_empty() && uris.is_empty() {
            out.report.skipped_empty += 1;
            continue;
        }
        if title.is_empty() {
            title = host_of(&uris[0]);
        }
        let mut item = Item::login(
            &title,
            &text(login, "username"),
            &text(login, "password"),
            now,
            new_id(),
        );
        item.urls = uris;
        item.notes = text(entry, "notes");
        out.add_totp(&mut item, text(login, "totp").trim());
        if let Some(folder) = entry
            .get("folderId")
            .and_then(|f| f.as_str())
            .and_then(|id| folders.get(id))
        {
            item.tags = split_tags(folder);
        }
        out.push(item);
    }
    Ok(out.finish())
}

/// Does this file look like JSON (so [`import_file`] will read it as a Bitwarden export, not a CSV)?
pub fn is_json(data: &[u8]) -> bool {
    let body = data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data);
    body.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{')
}

/// Reads an import file of either kind: a Bitwarden JSON export if it starts with `{`, else a CSV.
/// `overrides` only matters for CSV.
pub fn import_file(
    data: &[u8],
    overrides: &[(String, String)],
    existing: &[Item],
    now: i64,
    new_id: impl FnMut() -> Uuid,
) -> Result<ImportReport> {
    if is_json(data) {
        import_bitwarden_json(
            data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data),
            existing,
            now,
            new_id,
        )
    } else {
        import_csv(data, overrides, existing, now, new_id)
    }
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
    fn bitwarden_keepass_and_1password_csv_are_recognised() {
        let bw = run(
            "folder,favorite,type,name,notes,fields,reprompt,login_uri,login_username,login_password,login_totp\n\
             Work,,login,GitHub,,,0,https://github.com,alice,pw1,JBSWY3DPEHPK3PXP\n\
             ,,note,A secure note,secret text,,0,,,,\n",
            &[],
        );
        assert_eq!(bw.detected, "bitwarden");
        assert_eq!(bw.items.len(), 1, "the note is not a login");
        assert_eq!(bw.items[0].tags, ["Work"]);
        assert!(bw.items[0].field("totp").is_some());
        assert!(bw.warnings.iter().any(|w| w.contains("not logins")));

        let kp = run(
            "\"Group\",\"Title\",\"Username\",\"Password\",\"URL\",\"Notes\",\"TOTP\"\n\
             \"Root/Web\",\"Bank\",\"me\",\"pw2\",\"https://bank.example\",\"n\",\"\"\n",
            &[],
        );
        assert_eq!(kp.detected, "keepass");
        assert_eq!(kp.items[0].title, "Bank");
        assert_eq!(kp.items[0].tags, ["Root/Web"]);

        let op = run(
            "Title,Website,Username,Password,OTPAuth,Favorite,Archived,Tags,Notes\n\
             Mail,https://mail.example,bob,pw3,,0,0,home,hi\n",
            &[],
        );
        assert_eq!(op.detected, "1password");
        assert_eq!(op.items[0].urls, ["https://mail.example"]);
        assert_eq!(op.items[0].notes, "hi");
    }

    #[test]
    fn csv_columns_lists_headers_and_applies_overrides() {
        let csv = "Site,Who,Secret,Misc\nx,y,z,w\n";
        let c = csv_columns(csv.as_bytes(), &[]).unwrap();
        assert_eq!(c.headers, ["Site", "Who", "Secret", "Misc"]);
        assert!(c.assigned.iter().all(|(_, i)| i.is_none()));
        let over = [
            ("title".to_string(), "Site".to_string()),
            ("username".to_string(), "who".to_string()),
            ("password".to_string(), "Secret".to_string()),
        ];
        let c = csv_columns(csv.as_bytes(), &over).unwrap();
        let find = |n: &str| c.assigned.iter().find(|(f, _)| *f == n).unwrap().1;
        assert_eq!(
            (find("title"), find("username"), find("password")),
            (Some(0), Some(1), Some(2))
        );
        assert_eq!(find("notes"), None);
        // An empty header clears a field an alias would have found.
        let cleared = csv_columns(
            b"name,url,username,password\nA,,b,c\n",
            &[("url".to_string(), String::new())],
        )
        .unwrap();
        assert_eq!(cleared.assigned[1], ("url", None));
        assert!(is_json(b"  {\"items\":[]}") && !is_json(b"a,b\n"));
        assert!(csv_columns(csv.as_bytes(), &[("title".into(), "nope".into())]).is_err());
        // The same overrides let import_csv read a file it could not read before.
        assert!(import_csv(csv.as_bytes(), &[], &[], 1, ids()).is_err());
        let r = import_csv(csv.as_bytes(), &over, &[], 1, ids()).unwrap();
        assert_eq!(r.items[0].field("password").unwrap().value, "z");
    }

    const BITWARDEN_JSON: &str = r#"{"encrypted":false,
        "folders":[{"id":"f1","name":"Work"}],
        "items":[
          {"type":1,"name":"GitHub","notes":"n","folderId":"f1",
           "login":{"username":"alice","password":"pw1","totp":"JBSWY3DPEHPK3PXP",
                    "uris":[{"uri":"https://github.com"},{"uri":"https://gist.github.com"}]}},
          {"type":1,"name":"","login":{"username":"u","password":"p","uris":[{"uri":"https://www.example.org/x"}]}},
          {"type":2,"name":"Note","notes":"text","secureNote":{"type":0}},
          {"type":3,"name":"Card","card":{"number":"4111"}},
          {"type":1,"name":"","login":{"username":"","password":"","uris":[]}}
        ]}"#;

    #[test]
    fn bitwarden_json_reads_logins_folders_and_skips_the_rest() {
        let r = import_bitwarden_json(BITWARDEN_JSON.as_bytes(), &[], 5, ids()).unwrap();
        assert_eq!(r.detected, "bitwarden");
        assert_eq!(r.items.len(), 2);
        assert_eq!(r.skipped_empty, 1);
        let gh = &r.items[0];
        assert_eq!(gh.title, "GitHub");
        assert_eq!(gh.urls.len(), 2);
        assert_eq!(gh.tags, ["Work"]);
        assert_eq!(gh.notes, "n");
        assert!(gh.field("totp").is_some());
        assert_eq!(r.items[1].title, "example.org");
        assert!(r.warnings.iter().any(|w| w.starts_with("2 entry(ies)")));
        // A second read against what was just imported adds nothing.
        let again = import_bitwarden_json(BITWARDEN_JSON.as_bytes(), &r.items, 6, ids()).unwrap();
        assert_eq!((again.items.len(), again.skipped_duplicates), (0, 2));
    }

    #[test]
    fn bitwarden_json_errors_are_explained_and_import_file_picks_the_reader() {
        let enc = br#"{"encrypted":true,"data":"x"}"#;
        assert!(import_bitwarden_json(enc, &[], 1, ids())
            .unwrap_err()
            .to_string()
            .contains("encrypted"));
        assert!(import_bitwarden_json(b"{}", &[], 1, ids()).is_err());
        assert!(import_bitwarden_json(b"[1,2]", &[], 1, ids()).is_err());
        let json = import_file(BITWARDEN_JSON.as_bytes(), &[], &[], 1, ids()).unwrap();
        assert_eq!(json.items.len(), 2);
        let with_bom = [b"\xEF\xBB\xBF".as_slice(), BITWARDEN_JSON.as_bytes()].concat();
        assert_eq!(
            import_file(&with_bom, &[], &[], 1, ids())
                .unwrap()
                .items
                .len(),
            2
        );
        let csv = import_file(b"name,url,username,password\nA,,b,c\n", &[], &[], 1, ids()).unwrap();
        assert_eq!(csv.detected, "chrome");
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
