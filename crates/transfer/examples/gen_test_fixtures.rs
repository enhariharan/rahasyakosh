//! Generates sample import files for manual testing. Run with:
//!
//!   cargo run -p rahasyakosh-transfer --example gen_test_fixtures -- <output-dir>
//!
//! If no output directory is given, files are written to `crates/transfer/tests/fixtures/`.

use std::{
    io::Write as _,
    path::PathBuf,
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

fn main() {
    let out_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
        });
    std::fs::create_dir_all(&out_dir).expect("create output directory");

    let path = out_dir.join("sample.1pux");
    let data = build_sample_1pux();
    std::fs::write(&path, &data).expect("write sample.1pux");
    println!("wrote {} ({} bytes)", path.display(), data.len());
    println!();
    println!("Import test file with:");
    println!("  rk import {}", path.display());
    println!("or through Settings → Import in the desktop app.");
    println!();
    println!("Expected: 10 logins imported (7 from Personal vault, 1 from Work vault,");
    println!("          2 items skipped: 1 credit card, 1 archived login, 1 empty-title).");
}

fn build_sample_1pux() -> Vec<u8> {
    let export_data = serde_json::json!({
        "accounts": [{
            "attrs": {
                "accountName": "sample@example.com",
                "name": "Sample Account",
                "domain": "my.1password.com"
            },
            "vaults": [
                {
                    "attrs": {"uuid": "vault-personal", "name": "Personal", "type": "P"},
                    "items": [
                        {
                            "uuid": "item-github",
                            "favIndex": 1,
                            "createdAt": 1700000000,
                            "updatedAt": 1700000001,
                            "state": "active",
                            "categoryUuid": "001",
                            "details": {
                                "loginFields": [
                                    {"value": "alice@example.com", "fieldType": "T",
                                     "designation": "username", "id": "username",
                                     "name": "username"},
                                    {"value": "Str0ng!P@ssw0rd", "fieldType": "P",
                                     "designation": "password", "id": "password",
                                     "name": "password"}
                                ],
                                "notesPlain": "Work GitHub account.\nUse SSH key for repos.",
                                "sections": [
                                    {
                                        "title": "Security",
                                        "name": "security_section",
                                        "fields": [
                                            {
                                                "title": "Recovery code",
                                                "id": "rc1",
                                                "value": {"concealed": "abcd-efgh-ijkl-mnop"}
                                            },
                                            {
                                                "title": "one-time password",
                                                "id": "otp1",
                                                "value": {
                                                    "totp": "otpauth://totp/GitHub:alice%40example.com?secret=JBSWY3DPEHPK3PXP&issuer=GitHub&period=30&digits=6"
                                                }
                                            }
                                        ]
                                    }
                                ],
                                "passwordHistory": [
                                    {"value": "0ldPassw0rd!", "time": 1699000000}
                                ]
                            },
                            "overview": {
                                "title": "GitHub",
                                "subtitle": "alice@example.com",
                                "url": "https://github.com",
                                "urls": [
                                    {"l": "website", "u": "https://github.com"},
                                    {"l": "gist",    "u": "https://gist.github.com"}
                                ],
                                "tags": ["work", "dev"]
                            }
                        },
                        {
                            "uuid": "item-bank",
                            "createdAt": 1700000002,
                            "updatedAt": 1700000002,
                            "state": "active",
                            "categoryUuid": "001",
                            "details": {
                                "loginFields": [
                                    {"value": "alice123", "fieldType": "T",
                                     "designation": "username"},
                                    {"value": "BankP@ss2024!", "fieldType": "P",
                                     "designation": "password"}
                                ],
                                "notesPlain": "",
                                "sections": [
                                    {
                                        "title": "Details",
                                        "name": "details",
                                        "fields": [
                                            {"title": "Account number", "id": "acctno",
                                             "value": {"string": "1234-5678-9012"}},
                                            {"title": "Sort code", "id": "sort",
                                             "value": {"string": "12-34-56"}},
                                            {"title": "PIN", "id": "pin",
                                             "value": {"concealed": "9876"}}
                                        ]
                                    }
                                ],
                                "passwordHistory": []
                            },
                            "overview": {
                                "title": "My Bank",
                                "url": "https://online.mybank.example",
                                "urls": [{"l": "website", "u": "https://online.mybank.example"}],
                                "tags": ["finance"]
                            }
                        },
                        {
                            "uuid": "item-email",
                            "createdAt": 1700000003,
                            "updatedAt": 1700000003,
                            "state": "active",
                            "categoryUuid": "001",
                            "details": {
                                "loginFields": [
                                    {"value": "alice@gmail.com", "fieldType": "T",
                                     "designation": "username"},
                                    {"value": "Gm@ilSecret99", "fieldType": "P",
                                     "designation": "password"}
                                ],
                                "notesPlain": "",
                                "sections": [],
                                "passwordHistory": []
                            },
                            "overview": {
                                "title": "Gmail",
                                "url": "https://mail.google.com",
                                "urls": [{"l": "website", "u": "https://mail.google.com"}],
                                "tags": []
                            }
                        },
                        {
                            "uuid": "item-unicode",
                            "createdAt": 1700000004,
                            "updatedAt": 1700000004,
                            "state": "active",
                            "categoryUuid": "001",
                            "details": {
                                "loginFields": [
                                    {"value": "用户名@example.com", "fieldType": "T",
                                     "designation": "username"},
                                    {"value": "密码123!パスワード", "fieldType": "P",
                                     "designation": "password"}
                                ],
                                "notesPlain": "日本語のメモ\n한국어 메모",
                                "sections": [],
                                "passwordHistory": []
                            },
                            "overview": {
                                "title": "国際サービス",
                                "url": "https://international.example",
                                "urls": [{"l": "website", "u": "https://international.example"}],
                                "tags": []
                            }
                        },
                        {
                            "uuid": "item-special-chars",
                            "createdAt": 1700000005,
                            "updatedAt": 1700000005,
                            "state": "active",
                            "categoryUuid": "001",
                            "details": {
                                "loginFields": [
                                    {"value": "user \"quoted\" <angle>", "fieldType": "T",
                                     "designation": "username"},
                                    {"value": "P@$$w0rd!#%&*()", "fieldType": "P",
                                     "designation": "password"}
                                ],
                                "notesPlain": "Line 1\nLine 2\nLine 3",
                                "sections": [],
                                "passwordHistory": []
                            },
                            "overview": {
                                "title": "Special \"Chars\" & <Tags>",
                                "url": "https://special.example",
                                "urls": [{"l": "website", "u": "https://special.example"}],
                                "tags": ["test"]
                            }
                        },
                        {
                            "uuid": "item-no-title",
                            "createdAt": 1700000006,
                            "updatedAt": 1700000006,
                            "state": "active",
                            "categoryUuid": "001",
                            "details": {
                                "loginFields": [
                                    {"value": "user@notitle.example", "fieldType": "T",
                                     "designation": "username"},
                                    {"value": "NoTitlePw!", "fieldType": "P",
                                     "designation": "password"}
                                ],
                                "notesPlain": "",
                                "sections": [],
                                "passwordHistory": []
                            },
                            "overview": {
                                "title": "",
                                "url": "https://www.notitle.example/login",
                                "urls": [{"l": "website",
                                          "u": "https://www.notitle.example/login"}],
                                "tags": []
                            }
                        },
                        {
                            "uuid": "item-standalone-pw",
                            "createdAt": 1700000007,
                            "updatedAt": 1700000007,
                            "state": "active",
                            "categoryUuid": "005",
                            "details": {
                                "loginFields": [
                                    {"value": "", "fieldType": "T", "designation": "username"},
                                    {"value": "StandaloneSecret!", "fieldType": "P",
                                     "designation": "password"}
                                ],
                                "notesPlain": "A standalone password item",
                                "sections": [],
                                "passwordHistory": []
                            },
                            "overview": {
                                "title": "Wi-Fi Password",
                                "url": "",
                                "urls": [],
                                "tags": ["home"]
                            }
                        },
                        // Credit card — should be skipped (non-login category)
                        {
                            "uuid": "item-creditcard",
                            "createdAt": 1700000008,
                            "updatedAt": 1700000008,
                            "state": "active",
                            "categoryUuid": "002",
                            "details": {"loginFields": [], "notesPlain": "", "sections": [],
                                        "passwordHistory": []},
                            "overview": {"title": "Visa Credit", "url": "", "urls": [], "tags": []}
                        },
                        // Archived login — should be skipped
                        {
                            "uuid": "item-archived",
                            "createdAt": 1700000009,
                            "updatedAt": 1700000010,
                            "state": "archived",
                            "categoryUuid": "001",
                            "details": {
                                "loginFields": [
                                    {"value": "olduser", "fieldType": "T",
                                     "designation": "username"},
                                    {"value": "0ldP@ss!", "fieldType": "P",
                                     "designation": "password"}
                                ],
                                "notesPlain": "",
                                "sections": [],
                                "passwordHistory": []
                            },
                            "overview": {
                                "title": "Archived Service",
                                "url": "https://archived.example",
                                "urls": [],
                                "tags": []
                            }
                        }
                    ]
                },
                {
                    "attrs": {"uuid": "vault-work", "name": "Work", "type": "U"},
                    "items": [
                        {
                            "uuid": "item-jira",
                            "createdAt": 1700000020,
                            "updatedAt": 1700000020,
                            "state": "active",
                            "categoryUuid": "001",
                            "details": {
                                "loginFields": [
                                    {"value": "alice@corp.example", "fieldType": "T",
                                     "designation": "username"},
                                    {"value": "Corp@Pass2024", "fieldType": "P",
                                     "designation": "password"}
                                ],
                                "notesPlain": "",
                                "sections": [],
                                "passwordHistory": []
                            },
                            "overview": {
                                "title": "Jira",
                                "url": "https://corp.atlassian.net",
                                "urls": [{"l": "website", "u": "https://corp.atlassian.net"}],
                                "tags": ["corp"]
                            }
                        }
                    ]
                }
            ]
        }]
    });

    build_zip("export.data", export_data.to_string().as_bytes())
}

fn build_zip(filename: &str, content: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut zip = ZipWriter::new(std::io::Cursor::new(&mut buf));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file(filename, options).unwrap();
    zip.write_all(content).unwrap();
    zip.finish().unwrap();
    buf
}
