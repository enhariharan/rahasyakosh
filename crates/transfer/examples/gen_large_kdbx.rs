//! Generates a KDBX 4 database with 200+ entries for performance/large-import testing.
//!
//!   cargo run -p rahasyakosh-transfer --example gen_large_kdbx -- [output_path]
//!
//! Defaults to `/tmp/rk-test/large.kdbx`. Password: "testpass"

fn main() {
    #[cfg(not(target_family = "wasm"))]
    {
        use keepass::{Database, DatabaseKey, db::fields};

        let out_path = std::env::args()
            .nth(1)
            .unwrap_or_else(|| "/tmp/rk-test/large.kdbx".to_string());

        let mut db = Database::new();

        let groups = [
            "Work", "Personal", "Finance", "Social", "Shopping",
            "Travel", "Dev", "Health", "Education", "Misc",
        ];

        for (gi, group_name) in groups.iter().enumerate() {
            let mut root = db.root_mut();
            let mut g = root.add_group();
            g.edit(|grp| grp.name = group_name.to_string());
            for i in 0..22 {
                let n = gi * 22 + i + 1;
                let mut e = g.add_entry();
                e.set_unprotected(fields::TITLE,    &format!("Service {n:03}"));
                e.set_unprotected(fields::USERNAME,  &format!("user{n:03}@example.com"));
                e.set_protected(fields::PASSWORD,   &format!("Passw0rd!{n:03}"));
                e.set_unprotected(fields::URL,       &format!("https://service{n:03}.example.com"));
                e.set_unprotected(fields::NOTES,     &format!("Auto-generated entry {n}"));
            }
        }

        // A few entries in Recycle Bin — must not be imported
        {
            let mut root = db.root_mut();
            let mut rb = root.add_group();
            rb.edit(|g| g.name = "Recycle Bin".to_string());
            for i in 1..=5 {
                let mut e = rb.add_entry();
                e.set_unprotected(fields::TITLE,   &format!("Deleted {i}"));
                e.set_unprotected(fields::USERNAME, "ghost");
                e.set_protected(fields::PASSWORD,  "old");
                e.set_unprotected(fields::URL,      "https://deleted.example.com");
            }
        }

        let key = DatabaseKey::new().with_password("testpass");
        let mut buf = std::io::Cursor::new(Vec::new());
        db.save(&mut buf, key).expect("failed to save kdbx");
        let bytes = buf.into_inner();

        if let Some(parent) = std::path::Path::new(&out_path).parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(&out_path, &bytes).expect("failed to write file");
        println!("Written {} bytes to {out_path}", bytes.len());
        println!("Entries: {} logins + 5 Recycle Bin (should be skipped)", groups.len() * 22);
        println!("Password: testpass");
    }

    #[cfg(target_family = "wasm")]
    eprintln!("This example requires a non-WASM target.");
}
