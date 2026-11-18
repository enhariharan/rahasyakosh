#![cfg(not(target_family = "wasm"))]

use rahasyakosh_transfer::{import_kdbx, is_kdbx, Error};
use uuid::Uuid;

fn ids() -> impl FnMut() -> Uuid {
    let mut n = 0u128;
    move || {
        n += 1;
        Uuid::from_u128(n)
    }
}

/// Builds an in-memory KDBX4 database (password: "testpass") covering all import edge cases:
///
/// Root group entries (7 items expected to be imported, 1 skipped_empty):
///   - GitHub       : normal entry with all standard fields
///   - BankPortal   : entry with TOTP (otpauth:// URI in otp field)
///   - (empty title): empty title + URL → title derived from URL ("example.org")
///   - (empty both) : empty title AND empty URL → skipped
///   - WorkVPN      : entry with custom fields ("Server", "License")
///   - SpecialChars : title/username/password with Unicode, quotes, semicolons
///
/// Work group (tag "Work"):
///   - Jira         : entry in "Work" group
///
/// Work/Dev subgroup (tag "Work/Dev"):
///   - GitLab       : entry in "Work" > "Dev" subgroup
///
/// Recycle Bin group:
///   - DeletedEntry : entry that must NOT be imported
fn build_kdbx_fixture() -> Vec<u8> {
    use keepass::{db::fields, Database, DatabaseKey};

    let mut db = Database::new();

    // Work group and its Dev subgroup (must be built before releasing root borrow)
    {
        let mut root = db.root_mut();
        let mut work = root.add_group();
        work.edit(|g| g.name = "Work".to_string());
        {
            let mut e = work.add_entry();
            e.set_unprotected(fields::TITLE, "Jira");
            e.set_unprotected(fields::USERNAME, "employee@work.com");
            e.set_protected(fields::PASSWORD, "jiraPW1!");
            e.set_unprotected(fields::URL, "https://jira.company.com");
        }
        {
            let mut dev = work.add_group();
            dev.edit(|g| g.name = "Dev".to_string());
            let mut e = dev.add_entry();
            e.set_unprotected(fields::TITLE, "GitLab");
            e.set_unprotected(fields::USERNAME, "dev1");
            e.set_protected(fields::PASSWORD, "gitlabPW!");
            e.set_unprotected(fields::URL, "https://gitlab.internal.com");
        }
    }

    // Recycle Bin group
    {
        let mut root = db.root_mut();
        let mut rb = root.add_group();
        rb.edit(|g| g.name = "Recycle Bin".to_string());
        let mut e = rb.add_entry();
        e.set_unprotected(fields::TITLE, "DeletedEntry");
        e.set_unprotected(fields::USERNAME, "deleted_user");
        e.set_protected(fields::PASSWORD, "deleted_pw");
        e.set_unprotected(fields::URL, "https://deleted.example.com");
    }

    // Root-level entries
    {
        let mut root = db.root_mut();

        // 1. Normal entry
        {
            let mut e = root.add_entry();
            e.set_unprotected(fields::TITLE, "GitHub");
            e.set_unprotected(fields::USERNAME, "alice@example.com");
            e.set_protected(fields::PASSWORD, "s3cretPW!");
            e.set_unprotected(fields::URL, "https://github.com");
            e.set_unprotected(fields::NOTES, "My primary GitHub account");
        }

        // 2. TOTP entry
        {
            let mut e = root.add_entry();
            e.set_unprotected(fields::TITLE, "BankPortal");
            e.set_unprotected(fields::USERNAME, "bob");
            e.set_protected(fields::PASSWORD, "b@nkPw99");
            e.set_unprotected(fields::URL, "https://bank.example.com");
            e.set_unprotected(
                fields::OTP,
                "otpauth://totp/BankPortal:bob?secret=JBSWY3DPEHPK3PXP&period=30&digits=6",
            );
        }

        // 3. Empty title → derive from URL
        {
            let mut e = root.add_entry();
            e.set_unprotected(fields::TITLE, "");
            e.set_unprotected(fields::USERNAME, "charlie");
            e.set_protected(fields::PASSWORD, "pw123");
            e.set_unprotected(fields::URL, "https://www.example.org/login");
        }

        // 4. Empty title AND empty URL → skipped
        {
            let mut e = root.add_entry();
            e.set_unprotected(fields::TITLE, "");
            e.set_unprotected(fields::USERNAME, "dan");
            e.set_protected(fields::PASSWORD, "pw123");
            e.set_unprotected(fields::URL, "");
        }

        // 5. Custom fields
        {
            let mut e = root.add_entry();
            e.set_unprotected(fields::TITLE, "WorkVPN");
            e.set_unprotected(fields::USERNAME, "employee1");
            e.set_protected(fields::PASSWORD, "vpnPW!");
            e.set_unprotected("Server", "vpn.company.com");
            e.set_unprotected("License", "ABC-123-XYZ");
        }

        // 6. Special characters (Unicode, quotes, semicolons)
        {
            let mut e = root.add_entry();
            e.set_unprotected(fields::TITLE, "Special \"Chars\" & Côté");
            e.set_unprotected(fields::USERNAME, "user;semi=colon");
            e.set_protected(fields::PASSWORD, "P@$$w0rd!#%&*()\u{1F511}");
            e.set_unprotected(fields::URL, "https://special.example.com");
        }
    }

    let key = DatabaseKey::new().with_password("testpass");
    let mut buf = std::io::Cursor::new(Vec::new());
    db.save(&mut buf, key).expect("failed to save kdbx fixture");
    buf.into_inner()
}

#[test]
fn kdbx_is_kdbx_detection() {
    // KDBX magic: 03 D9 A2 9A
    assert!(is_kdbx(&[0x03, 0xD9, 0xA2, 0x9A, 0x00]));
    assert!(!is_kdbx(b"name,url,username,password\n"));
    assert!(!is_kdbx(b"{\"items\":[]}"));
    assert!(!is_kdbx(&[0x50, 0x4B, 0x03, 0x04])); // ZIP magic
    assert!(!is_kdbx(b""));
    assert!(!is_kdbx(&[0x03, 0xD9])); // too short
}

#[test]
fn kdbx_too_large() {
    // A single magic byte payload that is over the limit is rejected before any parsing.
    let mut big = vec![0x03u8, 0xD9, 0xA2, 0x9A];
    big.resize(50 * 1024 * 1024 + 1, 0);
    let err = import_kdbx(&big, "pw", &[], 1, ids()).unwrap_err();
    assert!(err.to_string().contains("too large"), "{err}");
}

#[test]
fn kdbx_wrong_password() {
    let data = build_kdbx_fixture();
    let err = import_kdbx(&data, "wrong!", &[], 1, ids()).unwrap_err();
    // The error should reference the database, not an internal panic.
    match err {
        Error::Kdbx(_) => {}
        other => panic!("expected Error::Kdbx, got {other}"),
    }
}

#[test]
fn kdbx_basic_import() {
    let data = build_kdbx_fixture();
    let r = import_kdbx(&data, "testpass", &[], 100, ids()).unwrap();
    assert_eq!(r.detected, "keepass");
    // 6 root entries (GitHub, BankPortal, example.org, WorkVPN, SpecialChars)
    // + 1 Work/Jira + 1 Work/Dev/GitLab = 7 (the empty-both entry is skipped)
    assert_eq!(
        r.items.len(),
        7,
        "unexpected item count: {:?}",
        r.items.iter().map(|i| &i.title).collect::<Vec<_>>()
    );
    assert_eq!(r.skipped_empty, 1, "expected 1 empty entry skipped");

    // Spot-check the GitHub entry
    let gh = r
        .items
        .iter()
        .find(|i| i.title == "GitHub")
        .expect("GitHub not found");
    assert_eq!(gh.field("username").unwrap().value, "alice@example.com");
    assert!(gh.field("password").unwrap().concealed);
    assert_eq!(gh.field("password").unwrap().value, "s3cretPW!");
    assert_eq!(gh.urls, ["https://github.com"]);
    assert_eq!(gh.notes, "My primary GitHub account");
    assert!(
        gh.tags.is_empty(),
        "root-level entry should have no group tag"
    );
}

#[test]
fn kdbx_groups_become_tags() {
    let data = build_kdbx_fixture();
    let r = import_kdbx(&data, "testpass", &[], 100, ids()).unwrap();

    let jira = r
        .items
        .iter()
        .find(|i| i.title == "Jira")
        .expect("Jira not found");
    assert_eq!(
        jira.tags,
        ["Work"],
        "Work-group entry should have tag 'Work'"
    );

    let gitlab = r
        .items
        .iter()
        .find(|i| i.title == "GitLab")
        .expect("GitLab not found");
    assert_eq!(
        gitlab.tags,
        ["Work/Dev"],
        "nested entry should have tag 'Work/Dev'"
    );
}

#[test]
fn kdbx_recycle_bin_skipped() {
    let data = build_kdbx_fixture();
    let r = import_kdbx(&data, "testpass", &[], 100, ids()).unwrap();
    assert!(
        r.items.iter().all(|i| i.title != "DeletedEntry"),
        "Recycle Bin entry should not be imported"
    );
}

#[test]
fn kdbx_totp_imported() {
    let data = build_kdbx_fixture();
    let r = import_kdbx(&data, "testpass", &[], 100, ids()).unwrap();
    let bank = r
        .items
        .iter()
        .find(|i| i.title == "BankPortal")
        .expect("BankPortal not found");
    let totp = bank.field("totp").expect("totp field should be present");
    assert!(totp.concealed);
    assert!(
        totp.value.contains("JBSWY3DPEHPK3PXP") || totp.value.starts_with("otpauth://"),
        "totp value should contain secret or be URI: {}",
        totp.value
    );
}

#[test]
fn kdbx_custom_fields() {
    let data = build_kdbx_fixture();
    let r = import_kdbx(&data, "testpass", &[], 100, ids()).unwrap();
    let vpn = r
        .items
        .iter()
        .find(|i| i.title == "WorkVPN")
        .expect("WorkVPN not found");
    let server = vpn
        .field("Server")
        .expect("Server custom field should be present");
    assert_eq!(server.value, "vpn.company.com");
    assert!(!server.concealed, "Server field should not be concealed");
    let license = vpn
        .field("License")
        .expect("License field should be present");
    assert_eq!(license.value, "ABC-123-XYZ");
}

#[test]
fn kdbx_empty_title_derived_from_url() {
    let data = build_kdbx_fixture();
    let r = import_kdbx(&data, "testpass", &[], 100, ids()).unwrap();
    // Entry with empty title + URL "https://www.example.org/login" → title "example.org"
    assert!(
        r.items.iter().any(|i| i.title == "example.org"),
        "empty-title entry should derive title from URL"
    );
}

#[test]
fn kdbx_empty_entries_skipped() {
    let data = build_kdbx_fixture();
    let r = import_kdbx(&data, "testpass", &[], 100, ids()).unwrap();
    assert_eq!(r.skipped_empty, 1);
}

#[test]
fn kdbx_special_characters() {
    let data = build_kdbx_fixture();
    let r = import_kdbx(&data, "testpass", &[], 100, ids()).unwrap();
    let sc = r
        .items
        .iter()
        .find(|i| i.title.contains("Chars"))
        .expect("SpecialChars entry not found");
    assert!(
        sc.title.contains("Côté"),
        "Unicode in title should be preserved"
    );
    assert_eq!(sc.field("username").unwrap().value, "user;semi=colon");
    assert!(sc.field("password").unwrap().value.contains('\u{1F511}'));
}

#[test]
fn kdbx_duplicate_detection() {
    let data = build_kdbx_fixture();
    // First import
    let first = import_kdbx(&data, "testpass", &[], 100, ids()).unwrap();
    assert_eq!(first.skipped_duplicates, 0);
    let n = first.items.len();
    // Second import against the first run's items → all duplicates
    let second = import_kdbx(&data, "testpass", &first.items, 200, ids()).unwrap();
    assert_eq!(second.items.len(), 0, "second import should add nothing");
    assert_eq!(second.skipped_duplicates, n as usize);
}

#[test]
fn kdbx_notes_multiline() {
    // Quickly verify that Notes content is preserved exactly (a common whitespace trap).
    use keepass::{db::fields, Database, DatabaseKey};
    let mut db = Database::new();
    {
        let mut root = db.root_mut();
        let mut e = root.add_entry();
        e.set_unprotected(fields::TITLE, "NoteEntry");
        e.set_protected(fields::PASSWORD, "pw");
        e.set_unprotected(fields::NOTES, "line one\nline two\n  indented");
    }
    let key = DatabaseKey::new().with_password("note_test");
    let mut buf = std::io::Cursor::new(Vec::new());
    db.save(&mut buf, key).unwrap();
    let r = import_kdbx(&buf.into_inner(), "note_test", &[], 1, ids()).unwrap();
    assert_eq!(r.items[0].notes, "line one\nline two\n  indented");
}
