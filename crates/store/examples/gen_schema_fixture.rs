//! Writes the committed fixture database for the CURRENT schema version into `spec/vectors/`.
//!
//!     cargo run -p rahasyakosh-store --features sqlite --example gen_schema_fixture -- spec/vectors
//!
//! Run it once when a schema version is released, then keep the file forever: the migration test opens every
//! fixture and checks that it still upgrades to the current schema. Never regenerate an old version's fixture.
use rahasyakosh_store::conformance::{sample_header, sample_id, sample_record};
use rahasyakosh_store::{SqliteStore, Store, SCHEMA_VERSION};

fn main() {
    let dir = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .expect("usage: gen_schema_fixture <dir>"),
    );
    let path = dir.join(format!("schema-v{SCHEMA_VERSION}.sqlite"));
    assert!(
        !path.exists(),
        "{} already exists; fixtures are never regenerated",
        path.display()
    );
    {
        let mut s = SqliteStore::open(&path).unwrap();
        s.create(&sample_header(1, 7)).unwrap();
        s.put_many(&[
            sample_record(1, 1, 64),
            sample_record(2, 3, 128),
            sample_record(3, 2, 0),
        ])
        .unwrap();
        s.purge(sample_id(4), 5).unwrap();
    }
    // Leave a single self-contained file rather than a write-ahead-log database.
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.pragma_update_and_check(None, "journal_mode", "DELETE", |r| r.get::<_, String>(0))
        .unwrap();
    println!("wrote {}", path.display());
}
