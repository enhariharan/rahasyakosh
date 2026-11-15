//! Generates a comprehensive KDBX 4 test fixture covering all KeePass import edge cases.
//!
//! Run with:
//!   cargo run -p rahasyakosh-transfer --example gen_kdbx_fixture -- [output_path]
//!
//! Defaults to `crates/transfer/tests/fixtures/sample.kdbx`.
//! Password: "testpass"

fn main() {
    #[cfg(not(target_family = "wasm"))]
    {
        use keepass::{Database, DatabaseKey, db::fields};

        let out_path = std::env::args()
            .nth(1)
            .unwrap_or_else(|| "crates/transfer/tests/fixtures/sample.kdbx".to_string());

        let mut db = Database::new();

        // Work > Dev subgroup
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
                e.set_unprotected(fields::NOTES, "Work issue tracker");
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

        // Personal group
        {
            let mut root = db.root_mut();
            let mut personal = root.add_group();
            personal.edit(|g| g.name = "Personal".to_string());
            let mut e = personal.add_entry();
            e.set_unprotected(fields::TITLE, "Netflix");
            e.set_unprotected(fields::USERNAME, "me@email.com");
            e.set_protected(fields::PASSWORD, "netfl!xPW");
            e.set_unprotected(fields::URL, "https://www.netflix.com");
        }

        // Recycle Bin (should be ignored on import)
        {
            let mut root = db.root_mut();
            let mut rb = root.add_group();
            rb.edit(|g| g.name = "Recycle Bin".to_string());
            let mut e = rb.add_entry();
            e.set_unprotected(fields::TITLE, "OldAccount");
            e.set_unprotected(fields::USERNAME, "old_user");
            e.set_protected(fields::PASSWORD, "old_pw");
            e.set_unprotected(fields::URL, "https://old.example.com");
        }

        // Root-level entries
        {
            let mut root = db.root_mut();

            // Normal entry with all fields
            {
                let mut e = root.add_entry();
                e.set_unprotected(fields::TITLE, "GitHub");
                e.set_unprotected(fields::USERNAME, "alice@example.com");
                e.set_protected(fields::PASSWORD, "s3cretPW!");
                e.set_unprotected(fields::URL, "https://github.com");
                e.set_unprotected(fields::NOTES, "My primary GitHub account\nSecond line");
            }

            // Entry with TOTP
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

            // Empty title → derives from URL
            {
                let mut e = root.add_entry();
                e.set_unprotected(fields::TITLE, "");
                e.set_unprotected(fields::USERNAME, "charlie");
                e.set_protected(fields::PASSWORD, "pw123");
                e.set_unprotected(fields::URL, "https://www.example.org/login");
            }

            // Empty title + empty URL → skipped on import
            {
                let mut e = root.add_entry();
                e.set_unprotected(fields::TITLE, "");
                e.set_unprotected(fields::USERNAME, "dan");
                e.set_protected(fields::PASSWORD, "pw123");
                e.set_unprotected(fields::URL, "");
            }

            // Custom fields
            {
                let mut e = root.add_entry();
                e.set_unprotected(fields::TITLE, "WorkVPN");
                e.set_unprotected(fields::USERNAME, "employee1");
                e.set_protected(fields::PASSWORD, "vpnPW!");
                e.set_unprotected(fields::URL, "https://vpn.company.com");
                e.set_unprotected("Server", "vpn.company.com");
                e.set_unprotected("License", "ABC-123-XYZ");
                e.set_unprotected("Department", "Engineering");
            }

            // Special characters (Unicode, quotes, semicolons)
            {
                let mut e = root.add_entry();
                e.set_unprotected(fields::TITLE, "Special \"Chars\" & Côté 日本語");
                e.set_unprotected(fields::USERNAME, "user;semi=colon");
                e.set_protected(fields::PASSWORD, "P@$$w0rd!#%&*()\u{1F511}");
                e.set_unprotected(fields::URL, "https://special.example.com");
            }

            // Long password (256 chars)
            {
                let mut e = root.add_entry();
                e.set_unprotected(fields::TITLE, "LongPW");
                e.set_unprotected(fields::USERNAME, "user");
                e.set_protected(fields::PASSWORD, "X".repeat(256));
                e.set_unprotected(fields::URL, "https://longpw.example.com");
            }
        }

        let key = DatabaseKey::new().with_password("testpass");
        let mut buf = std::io::Cursor::new(Vec::new());
        db.save(&mut buf, key).expect("failed to save kdbx");
        let bytes = buf.into_inner();

        if let Some(parent) = std::path::Path::new(&out_path).parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(&out_path, &bytes).expect("failed to write fixture file");
        println!("Written {} bytes to {out_path}", bytes.len());
        println!("Password: testpass");
    }

    #[cfg(target_family = "wasm")]
    eprintln!("This example requires a non-WASM target.");
}
