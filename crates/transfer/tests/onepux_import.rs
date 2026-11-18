use rahasyakosh_transfer::{import_1pux, import_file, is_1pux, ImportReport};
use uuid::Uuid;

fn ids() -> impl FnMut() -> Uuid {
    let mut n = 0u128;
    move || {
        n += 1;
        Uuid::from_u128(n)
    }
}

fn run(data: &[u8]) -> ImportReport {
    import_1pux(data, &[], 100, ids()).unwrap()
}

/// Build a comprehensive in-memory .1pux fixture covering all edge cases.
fn build_fixture() -> Vec<u8> {
    use std::io::Write as _;
    use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

    let export_data = serde_json::json!({
        "accounts": [{
            "attrs": {"accountName": "test@example.com", "name": "Test"},
            "vaults": [
                {
                    "attrs": {"uuid": "v1", "name": "Personal", "type": "P"},
                    "items": [
                        // 1. Normal login with two URLs and tags
                        {
                            "uuid": "i1", "favIndex": 0, "createdAt": 1, "updatedAt": 1,
                            "state": "active", "categoryUuid": "001",
                            "details": {
                                "loginFields": [
                                    {"value": "alice@example.com", "fieldType": "T",
                                     "designation": "username"},
                                    {"value": "s3cr3t", "fieldType": "P",
                                     "designation": "password"}
                                ],
                                "notesPlain": "My GitHub account",
                                "sections": [], "passwordHistory": []
                            },
                            "overview": {
                                "title": "GitHub",
                                "url": "https://github.com",
                                "urls": [
                                    {"l": "website", "u": "https://github.com"},
                                    {"l": "api",     "u": "https://api.github.com"}
                                ],
                                "tags": ["work", "dev"]
                            }
                        },
                        // 2. Login with TOTP in a section field
                        {
                            "uuid": "i2", "state": "active", "categoryUuid": "001",
                            "details": {
                                "loginFields": [
                                    {"value": "bob", "fieldType": "T", "designation": "username"},
                                    {"value": "pw123", "fieldType": "P", "designation": "password"}
                                ],
                                "notesPlain": "",
                                "sections": [{
                                    "title": "One-Time Password", "name": "otp_section",
                                    "fields": [{
                                        "title": "one-time password", "id": "otp1",
                                        "value": {
                                            "totp": "otpauth://totp/Test:bob?secret=JBSWY3DPEHPK3PXP&period=30&digits=6"
                                        }
                                    }]
                                }],
                                "passwordHistory": []
                            },
                            "overview": {
                                "title": "Bank", "url": "https://bank.example",
                                "urls": [], "tags": []
                            }
                        },
                        // 3. Login with custom fields (string + concealed)
                        {
                            "uuid": "i3", "state": "active", "categoryUuid": "001",
                            "details": {
                                "loginFields": [
                                    {"value": "carol", "fieldType": "T", "designation": "username"},
                                    {"value": "pw456", "fieldType": "P", "designation": "password"}
                                ],
                                "notesPlain": "Line 1\nLine 2",
                                "sections": [{
                                    "title": "Details", "name": "s1",
                                    "fields": [
                                        {"title": "Recovery Code", "id": "rc",
                                         "value": {"concealed": "1234-5678"}},
                                        {"title": "Account Number", "id": "an",
                                         "value": {"string": "ACC-9876"}}
                                    ]
                                }],
                                "passwordHistory": []
                            },
                            "overview": {
                                "title": "Service", "url": "https://service.example",
                                "urls": [], "tags": []
                            }
                        },
                        // 4. Credit card — should be counted as not_logins
                        {
                            "uuid": "i4", "state": "active", "categoryUuid": "002",
                            "details": {"loginFields": [], "notesPlain": "", "sections": [],
                                        "passwordHistory": []},
                            "overview": {"title": "My Visa", "url": "", "urls": [], "tags": []}
                        },
                        // 5. Archived login — should be skipped
                        {
                            "uuid": "i5", "state": "archived", "categoryUuid": "001",
                            "details": {
                                "loginFields": [
                                    {"value": "dave", "fieldType": "T", "designation": "username"},
                                    {"value": "oldpw", "fieldType": "P", "designation": "password"}
                                ],
                                "notesPlain": "", "sections": [], "passwordHistory": []
                            },
                            "overview": {
                                "title": "Old Site", "url": "https://old.example",
                                "urls": [], "tags": []
                            }
                        },
                        // 6. Empty title — derive from URL
                        {
                            "uuid": "i6", "state": "active", "categoryUuid": "001",
                            "details": {
                                "loginFields": [
                                    {"value": "eve", "fieldType": "T", "designation": "username"},
                                    {"value": "pw789", "fieldType": "P", "designation": "password"}
                                ],
                                "notesPlain": "", "sections": [], "passwordHistory": []
                            },
                            "overview": {
                                "title": "", "url": "https://www.example.org/login",
                                "urls": [], "tags": []
                            }
                        },
                        // 7. Standalone password (category 005)
                        {
                            "uuid": "i7", "state": "active", "categoryUuid": "005",
                            "details": {
                                "loginFields": [
                                    {"value": "frank", "fieldType": "T",
                                     "designation": "username"},
                                    {"value": "standalone", "fieldType": "P",
                                     "designation": "password"}
                                ],
                                "notesPlain": "", "sections": [], "passwordHistory": []
                            },
                            "overview": {
                                "title": "Standalone Pass", "url": "",
                                "urls": [], "tags": []
                            }
                        },
                        // 8. Empty title + no URL — should be skipped
                        {
                            "uuid": "i8", "state": "active", "categoryUuid": "001",
                            "details": {"loginFields": [], "notesPlain": "", "sections": [],
                                        "passwordHistory": []},
                            "overview": {"title": "", "url": "", "urls": [], "tags": []}
                        },
                        // 9. Unicode in all fields
                        {
                            "uuid": "i9", "state": "active", "categoryUuid": "001",
                            "details": {
                                "loginFields": [
                                    {"value": "用户名", "fieldType": "T",
                                     "designation": "username"},
                                    {"value": "пароль123", "fieldType": "P",
                                     "designation": "password"}
                                ],
                                "notesPlain": "日本語ノート", "sections": [], "passwordHistory": []
                            },
                            "overview": {
                                "title": "Unicode Site", "url": "https://unicode.example",
                                "urls": [], "tags": []
                            }
                        }
                    ]
                },
                {
                    "attrs": {"uuid": "v2", "name": "Work", "type": "U"},
                    "items": [
                        // 10. Non-Personal vault — name becomes a tag
                        {
                            "uuid": "i10", "state": "active", "categoryUuid": "001",
                            "details": {
                                "loginFields": [
                                    {"value": "grace", "fieldType": "T",
                                     "designation": "username"},
                                    {"value": "workpw", "fieldType": "P",
                                     "designation": "password"}
                                ],
                                "notesPlain": "", "sections": [], "passwordHistory": []
                            },
                            "overview": {
                                "title": "Work Portal", "url": "https://work.example",
                                "urls": [], "tags": []
                            }
                        }
                    ]
                }
            ]
        }]
    });

    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(std::io::Cursor::new(&mut buf));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file("export.data", options).unwrap();
    zip.write_all(export_data.to_string().as_bytes()).unwrap();
    zip.finish().unwrap();
    buf
}

fn zip_with_content(name: &str, content: &[u8]) -> Vec<u8> {
    use std::io::Write as _;
    use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};
    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(std::io::Cursor::new(&mut buf));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file(name, options).unwrap();
    zip.write_all(content).unwrap();
    zip.finish().unwrap();
    buf
}

#[test]
fn onepux_basic_import() {
    let r = run(&build_fixture());
    assert_eq!(r.detected, "1password");
    // Items 1,2,3,6,7,9,10 imported; 4+5 skipped (not_logins/archived); 8 skipped_empty
    assert_eq!(r.items.len(), 7);
    assert_eq!(r.skipped_empty, 1);
    assert_eq!(r.skipped_duplicates, 0);
}

#[test]
fn onepux_login_fields() {
    let r = run(&build_fixture());
    let gh = r.items.iter().find(|i| i.title == "GitHub").unwrap();
    assert_eq!(gh.field("username").unwrap().value, "alice@example.com");
    assert_eq!(gh.field("password").unwrap().value, "s3cr3t");
    assert!(gh.field("password").unwrap().concealed);
    assert_eq!(gh.notes, "My GitHub account");
}

#[test]
fn onepux_multiple_urls() {
    let r = run(&build_fixture());
    let gh = r.items.iter().find(|i| i.title == "GitHub").unwrap();
    // primary url "https://github.com" + extra "https://api.github.com"; no duplicate of primary
    assert_eq!(gh.urls.len(), 2);
    assert!(gh.urls.contains(&"https://github.com".to_string()));
    assert!(gh.urls.contains(&"https://api.github.com".to_string()));
}

#[test]
fn onepux_tags_preserved() {
    let r = run(&build_fixture());
    let gh = r.items.iter().find(|i| i.title == "GitHub").unwrap();
    assert_eq!(gh.tags, ["work", "dev"]);
}

#[test]
fn onepux_totp_imported() {
    let r = run(&build_fixture());
    let bank = r.items.iter().find(|i| i.title == "Bank").unwrap();
    assert!(
        bank.field("totp").is_some(),
        "Bank item should have a TOTP field"
    );
}

#[test]
fn onepux_custom_fields() {
    let r = run(&build_fixture());
    let svc = r.items.iter().find(|i| i.title == "Service").unwrap();
    let rc = svc.field("Recovery Code").unwrap();
    assert_eq!(rc.value, "1234-5678");
    assert!(rc.concealed, "concealed field should be a secret");
    let an = svc.field("Account Number").unwrap();
    assert_eq!(an.value, "ACC-9876");
    assert!(!an.concealed);
}

#[test]
fn onepux_multiline_notes() {
    let r = run(&build_fixture());
    let svc = r.items.iter().find(|i| i.title == "Service").unwrap();
    assert_eq!(svc.notes, "Line 1\nLine 2");
}

#[test]
fn onepux_non_login_categories_skipped() {
    let r = run(&build_fixture());
    assert!(
        r.items.iter().all(|i| i.title != "My Visa"),
        "credit card should not be imported"
    );
    assert!(
        r.warnings.iter().any(|w| w.contains("not logins")),
        "warning should mention skipped non-login entries"
    );
}

#[test]
fn onepux_archived_skipped() {
    let r = run(&build_fixture());
    assert!(r.items.iter().all(|i| i.title != "Old Site"));
}

#[test]
fn onepux_empty_title_derives_from_url() {
    let r = run(&build_fixture());
    // item 6 has no title but URL https://www.example.org/login → title = "example.org"
    assert!(
        r.items.iter().any(|i| i.title == "example.org"),
        "empty title should be derived from URL hostname"
    );
}

#[test]
fn onepux_empty_entry_skipped() {
    let r = run(&build_fixture());
    assert_eq!(r.skipped_empty, 1);
}

#[test]
fn onepux_standalone_password_imported() {
    let r = run(&build_fixture());
    let sp = r
        .items
        .iter()
        .find(|i| i.title == "Standalone Pass")
        .unwrap();
    assert_eq!(sp.field("username").unwrap().value, "frank");
    assert_eq!(sp.field("password").unwrap().value, "standalone");
}

#[test]
fn onepux_vault_name_as_tag() {
    let r = run(&build_fixture());
    let wp = r.items.iter().find(|i| i.title == "Work Portal").unwrap();
    assert!(
        wp.tags.contains(&"Work".to_string()),
        "non-Personal vault name should be added as a tag"
    );
}

#[test]
fn onepux_personal_vault_no_tag() {
    let r = run(&build_fixture());
    let gh = r.items.iter().find(|i| i.title == "GitHub").unwrap();
    assert!(
        !gh.tags.contains(&"Personal".to_string()),
        "\"Personal\" vault name should not appear as a tag"
    );
}

#[test]
fn onepux_unicode_preserved() {
    let r = run(&build_fixture());
    let uni = r.items.iter().find(|i| i.title == "Unicode Site").unwrap();
    assert_eq!(uni.field("username").unwrap().value, "用户名");
    assert_eq!(uni.field("password").unwrap().value, "пароль123");
    assert_eq!(uni.notes, "日本語ノート");
}

#[test]
fn onepux_duplicate_detection() {
    let data = build_fixture();
    let first = import_1pux(&data, &[], 1, ids()).unwrap();
    let second = import_1pux(&data, &first.items, 2, ids()).unwrap();
    assert_eq!(second.items.len(), 0);
    assert_eq!(second.skipped_duplicates, first.items.len());
}

#[test]
fn onepux_missing_export_data() {
    let zip = zip_with_content("other.json", b"{}");
    let err = import_1pux(&zip, &[], 1, ids()).unwrap_err();
    assert!(
        err.to_string().contains("missing export.data"),
        "got: {err}"
    );
}

#[test]
fn onepux_bad_json() {
    let zip = zip_with_content("export.data", b"not json at all");
    let err = import_1pux(&zip, &[], 1, ids()).unwrap_err();
    assert!(err.to_string().contains("not valid JSON"), "got: {err}");
}

#[test]
fn onepux_no_accounts_key() {
    let zip = zip_with_content("export.data", b"{\"foo\":1}");
    let err = import_1pux(&zip, &[], 1, ids()).unwrap_err();
    assert!(err.to_string().contains("accounts"), "got: {err}");
}

#[test]
fn onepux_not_a_zip() {
    let err = import_1pux(b"this is not a zip file", &[], 1, ids()).unwrap_err();
    assert!(err.to_string().contains(".1pux archive"), "got: {err}");
}

#[test]
fn onepux_too_large() {
    // Build a vec just over the limit — don't actually fill it, just fake the size check by
    // using a slice that is too large via a custom approach. We use a ZST trick: allocate
    // the metadata only and check the error path.
    let big = vec![0u8; rahasyakosh_transfer::MAX_IMPORT_BYTES + 1];
    let err = import_1pux(&big, &[], 1, ids()).unwrap_err();
    assert!(err.to_string().contains("too large"), "got: {err}");
}

#[test]
fn onepux_is_1pux_detection() {
    let zip = build_fixture();
    assert!(is_1pux(&zip));
    assert!(!is_1pux(b"title,url,username,password\n"));
    assert!(!is_1pux(b"{\"accounts\":[]}"));
    assert!(!is_1pux(b""));
    assert!(is_1pux(b"\x50\x4B")); // 2-byte PK prefix is enough to signal a ZIP
    assert!(!is_1pux(b"\x50")); // single byte is not enough
}

#[test]
fn onepux_import_file_dispatch() {
    let zip = build_fixture();
    let r = import_file(&zip, &[], &[], 1, ids()).unwrap();
    assert_eq!(r.detected, "1password");
    assert_eq!(r.items.len(), 7);
}

#[test]
fn onepux_url_deduplication() {
    // The GitHub item has the same URL in overview.url and in overview.urls[0].
    // We should only store it once.
    let r = run(&build_fixture());
    let gh = r.items.iter().find(|i| i.title == "GitHub").unwrap();
    let count = gh
        .urls
        .iter()
        .filter(|u| *u == "https://github.com")
        .count();
    assert_eq!(count, 1, "primary URL should not be duplicated");
}

#[test]
fn onepux_secure_note_and_identity_skipped() {
    use std::io::Write as _;
    use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

    let data = serde_json::json!({
        "accounts": [{"attrs": {}, "vaults": [{"attrs": {"name": "Personal"}, "items": [
            {"uuid": "n1", "state": "active", "categoryUuid": "003",
             "details": {"loginFields": [], "notesPlain": "secret", "sections": [],
                         "passwordHistory": []},
             "overview": {"title": "My Note", "url": "", "urls": [], "tags": []}},
            {"uuid": "n2", "state": "active", "categoryUuid": "004",
             "details": {"loginFields": [], "notesPlain": "", "sections": [],
                         "passwordHistory": []},
             "overview": {"title": "My Identity", "url": "", "urls": [], "tags": []}}
        ]}]}]
    });
    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(std::io::Cursor::new(&mut buf));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file("export.data", options).unwrap();
    zip.write_all(data.to_string().as_bytes()).unwrap();
    zip.finish().unwrap();

    let r = import_1pux(&buf, &[], 1, ids()).unwrap();
    assert_eq!(r.items.len(), 0);
    assert!(r.warnings.iter().any(|w| w.contains("not logins")));
}
