//! SQLite backend tests. They use real files, so they run on native targets only.
#![cfg(all(feature = "sqlite", not(target_family = "wasm")))]

use rahasyakosh_store::conformance::{self, sample_header, sample_id, sample_record, Fixture};
use rahasyakosh_store::{
    Error, Migration, SqliteStore, Store, Tombstone, MIGRATIONS, SCHEMA_VERSION,
};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A fresh directory under the target dir that is removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "sqlite-{name}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        TempDir(p)
    }
    fn db(&self) -> PathBuf {
        self.0.join("vault.db")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ---- conformance -------------------------------------------------------------------------------------

struct OnDisk(std::sync::Mutex<Vec<TempDir>>);

impl Fixture for OnDisk {
    type S = SqliteStore;
    fn fresh(&self) -> SqliteStore {
        let t = TempDir::new("conf");
        let s = SqliteStore::open(t.db()).unwrap();
        self.0.lock().unwrap().push(t);
        s
    }
    fn reopen(&self, s: SqliteStore) -> SqliteStore {
        // Dropping the store first closes its connection, as a real restart would.
        let path = s.path().expect("file-backed").to_path_buf();
        drop(s);
        SqliteStore::open(path).unwrap()
    }
}

#[test]
fn sqlite_store_passes_the_conformance_suite() {
    conformance::run(&OnDisk(Default::default()));
}

struct InMemory;

impl Fixture for InMemory {
    type S = SqliteStore;
    fn fresh(&self) -> SqliteStore {
        SqliteStore::open_in_memory().unwrap()
    }
    fn reopen(&self, s: SqliteStore) -> SqliteStore {
        s // an in-memory database has nowhere to reopen from
    }
}

#[test]
fn in_memory_sqlite_passes_the_conformance_suite_too() {
    conformance::run(&InMemory);
}

// ---- file safety -------------------------------------------------------------------------------------

#[test]
fn database_and_companion_files_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let t = TempDir::new("perms");
    let mut s = SqliteStore::open(t.db()).unwrap();
    s.create(&sample_header(1, 7)).unwrap();
    s.put(&sample_record(1, 1, 64)).unwrap();
    // While the connection is open the write-ahead-log files exist.
    let mut seen = 0;
    for entry in std::fs::read_dir(&t.0).unwrap() {
        let e = entry.unwrap();
        let mode = e.metadata().unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "{:?}", e.file_name());
        seen += 1;
    }
    assert!(seen >= 1);
    assert!(t.0.join("vault.db-wal").exists(), "expected WAL mode");
}

#[test]
fn durability_settings_are_applied() {
    let t = TempDir::new("pragmas");
    let s = SqliteStore::open(t.db()).unwrap();
    assert_eq!(s.pragma("journal_mode").unwrap(), "wal");
    assert_eq!(s.pragma("secure_delete").unwrap(), "1");
    assert_eq!(s.pragma("synchronous").unwrap(), "2", "FULL");
    assert_eq!(s.pragma("foreign_keys").unwrap(), "1");
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn purged_ciphertext_is_wiped_from_the_file() {
    let t = TempDir::new("secure-delete");
    let marker = vec![0xAB; 512];
    let mut rec = sample_record(1, 1, 0);
    rec.ct = marker.clone();
    {
        let mut s = SqliteStore::open(t.db()).unwrap();
        s.create(&sample_header(1, 7)).unwrap();
        s.put(&rec).unwrap();
    } // closing the last connection folds the write-ahead log into the main file
    assert!(
        contains(&std::fs::read(t.db()).unwrap(), &marker),
        "sanity: the ciphertext is in the file before the purge"
    );
    {
        let mut s = SqliteStore::open(t.db()).unwrap();
        s.purge(sample_id(1), 2).unwrap();
        assert!(s.get(sample_id(1)).unwrap().is_none());
    }
    let after = std::fs::read(t.db()).unwrap();
    assert!(
        !contains(&after, &marker),
        "purged ciphertext must not linger in free pages"
    );
    assert!(!contains(&after, &marker[..64]));
}

#[test]
fn garbage_file_is_reported_as_corrupt() {
    let t = TempDir::new("garbage");
    std::fs::write(t.db(), vec![0x5A; 8192]).unwrap();
    assert!(matches!(SqliteStore::open(t.db()), Err(Error::Corrupt(_))));
}

#[test]
fn damaged_header_is_reported_as_corrupt() {
    let t = TempDir::new("bad-header");
    {
        let mut s = SqliteStore::open(t.db()).unwrap();
        s.create(&sample_header(1, 7)).unwrap();
    }
    Connection::open(t.db())
        .unwrap()
        .execute("UPDATE meta SET value = x'7b2262' WHERE key = 'header'", [])
        .unwrap();
    let s = SqliteStore::open(t.db()).unwrap();
    assert!(matches!(s.header(), Err(Error::Corrupt(_))));
}

// ---- revisions beyond SQLite's signed integers ----------------------------------------------------------

#[test]
fn full_u64_revisions_round_trip_and_order_correctly() {
    let mut s = SqliteStore::open_in_memory().unwrap();
    s.create(&sample_header(1, 7)).unwrap();
    s.put(&sample_record(1, 5, 10)).unwrap();
    s.put(&sample_record(1, u64::MAX - 1, 10)).unwrap();
    assert_eq!(s.get(sample_id(1)).unwrap().unwrap().rev, u64::MAX - 1);
    s.put(&sample_record(1, u64::MAX, 10)).unwrap();
    assert_eq!(s.list().unwrap()[0].rev, u64::MAX);
    // A small revision is older than a huge one, even though the huge one is negative as an i64.
    assert!(matches!(
        s.put(&sample_record(1, 6, 10)),
        Err(Error::StaleRevision { .. })
    ));
    s.purge(sample_id(2), u64::MAX).unwrap();
    assert_eq!(
        s.tombstones().unwrap(),
        vec![Tombstone {
            id: sample_id(2),
            rev: u64::MAX
        }]
    );
}

// ---- migrations --------------------------------------------------------------------------------------

#[test]
fn a_new_database_is_at_the_current_schema_version() {
    let t = TempDir::new("version");
    let s = SqliteStore::open(t.db()).unwrap();
    assert_eq!(s.schema_version().unwrap(), SCHEMA_VERSION);
    assert_eq!(SCHEMA_VERSION as usize, MIGRATIONS.len());
}

#[test]
fn reopening_does_not_rerun_migrations_or_lose_data() {
    let t = TempDir::new("reopen-migrate");
    {
        let mut s = SqliteStore::open(t.db()).unwrap();
        s.create(&sample_header(1, 7)).unwrap();
        s.put(&sample_record(1, 1, 64)).unwrap();
    }
    for _ in 0..3 {
        let s = SqliteStore::open(t.db()).unwrap();
        assert_eq!(s.schema_version().unwrap(), SCHEMA_VERSION);
        assert_eq!(s.all().unwrap(), vec![sample_record(1, 1, 64)]);
    }
}

#[test]
fn a_database_from_a_newer_version_is_refused_and_left_untouched() {
    let t = TempDir::new("too-new");
    {
        let mut s = SqliteStore::open(t.db()).unwrap();
        s.create(&sample_header(1, 7)).unwrap();
    }
    Connection::open(t.db())
        .unwrap()
        .pragma_update(None, "user_version", 99)
        .unwrap();
    let before = std::fs::read(t.db()).unwrap();
    match SqliteStore::open(t.db()) {
        Err(Error::TooNew { found, supported }) => {
            assert_eq!((found, supported), (99, SCHEMA_VERSION));
        }
        Err(e) => panic!("wrong error: {e:?}"),
        Ok(_) => panic!("a newer database must be refused"),
    }
    let v: u32 = Connection::open(t.db())
        .unwrap()
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(v, 99);
    assert_eq!(std::fs::read(t.db()).unwrap().len(), before.len());
}

fn add_label_table(tx: &rusqlite::Transaction) -> rusqlite::Result<()> {
    tx.execute_batch(
        "CREATE TABLE labels (name TEXT PRIMARY KEY); INSERT INTO labels VALUES ('migrated');",
    )
}

fn fail_half_way(tx: &rusqlite::Transaction) -> rusqlite::Result<()> {
    tx.execute_batch("CREATE TABLE half_done (x INTEGER);")?;
    tx.execute_batch("THIS IS NOT SQL")
}

#[test]
fn a_new_migration_runs_once_and_keeps_existing_data() {
    let t = TempDir::new("migrate-up");
    {
        let mut s = SqliteStore::open(t.db()).unwrap();
        s.create(&sample_header(1, 7)).unwrap();
        s.put(&sample_record(1, 1, 64)).unwrap();
    }
    let steps: Vec<Migration> = MIGRATIONS
        .iter()
        .copied()
        .chain([add_label_table as Migration])
        .collect();
    let s = SqliteStore::open_with_migrations(t.db(), &steps).unwrap();
    assert_eq!(s.schema_version().unwrap(), SCHEMA_VERSION + 1);
    assert_eq!(
        s.all().unwrap(),
        vec![sample_record(1, 1, 64)],
        "data survives the migration"
    );
    let label: String = Connection::open(t.db())
        .unwrap()
        .query_row("SELECT name FROM labels", [], |r| r.get(0))
        .unwrap();
    assert_eq!(label, "migrated");
    drop(s);
    // Opening again with the same list does not run it a second time (it would fail: the table exists).
    SqliteStore::open_with_migrations(t.db(), &steps).unwrap();
    // The released program, which only knows the original list, now refuses the upgraded file.
    assert!(matches!(
        SqliteStore::open(t.db()),
        Err(Error::TooNew { .. })
    ));
}

#[test]
fn a_failing_migration_rolls_back_completely() {
    let t = TempDir::new("migrate-fail");
    {
        let mut s = SqliteStore::open(t.db()).unwrap();
        s.create(&sample_header(1, 7)).unwrap();
        s.put(&sample_record(1, 1, 64)).unwrap();
    }
    let steps: Vec<Migration> = MIGRATIONS
        .iter()
        .copied()
        .chain([fail_half_way as Migration])
        .collect();
    assert!(SqliteStore::open_with_migrations(t.db(), &steps).is_err());

    // Nothing from the failed step is left, and the version did not move.
    let conn = Connection::open(t.db()).unwrap();
    let version: u32 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
    let leftover: i64 = conn
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name = 'half_done'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(leftover, 0);
    drop(conn);
    let s = SqliteStore::open(t.db()).unwrap();
    assert_eq!(s.all().unwrap(), vec![sample_record(1, 1, 64)]);
}

/// Every released schema version has a fixture database in `spec/vectors/`. Each one must still open and upgrade.
#[test]
fn every_schema_fixture_upgrades_to_the_current_version() {
    let vectors = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/vectors");
    let fixtures: Vec<_> = std::fs::read_dir(&vectors)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("schema-v") && n.ends_with(".sqlite"))
        })
        .collect();
    assert!(
        fixtures.len() >= SCHEMA_VERSION as usize,
        "there must be a fixture for each of the {SCHEMA_VERSION} schema version(s)"
    );
    for fixture in fixtures {
        let t = TempDir::new("fixture");
        // Work on a copy: opening a database may create journal files next to it.
        std::fs::copy(&fixture, t.db()).unwrap();
        let s = SqliteStore::open(t.db()).unwrap_or_else(|e| panic!("{}: {e}", fixture.display()));
        assert_eq!(
            s.schema_version().unwrap(),
            SCHEMA_VERSION,
            "{}",
            fixture.display()
        );
        assert_eq!(s.header().unwrap(), sample_header(1, 7));
        assert_eq!(
            s.all().unwrap(),
            vec![
                sample_record(1, 1, 64),
                sample_record(2, 3, 128),
                sample_record(3, 2, 0)
            ],
            "{}",
            fixture.display()
        );
        assert_eq!(
            s.tombstones().unwrap(),
            vec![Tombstone {
                id: sample_id(4),
                rev: 5
            }]
        );
    }
}

// ---- more than one handle ------------------------------------------------------------------------------

#[test]
fn a_second_handle_cannot_overwrite_with_a_stale_revision() {
    let t = TempDir::new("two-handles");
    let mut a = SqliteStore::open(t.db()).unwrap();
    a.create(&sample_header(1, 7)).unwrap();
    a.put(&sample_record(1, 1, 10)).unwrap();
    let mut b = SqliteStore::open(t.db()).unwrap();

    // Both "read" revision 1 and prepare revision 2. The first writer wins; the second is told it lost.
    a.put(&sample_record(1, 2, 20)).unwrap();
    let e = b.put(&sample_record(1, 2, 30)).unwrap_err();
    assert!(
        matches!(
            e,
            Error::StaleRevision {
                stored: 2,
                offered: 2,
                ..
            }
        ),
        "{e:?}"
    );
    assert_eq!(b.get(sample_id(1)).unwrap().unwrap().ct.len(), 20);
}

#[test]
fn concurrent_writers_do_not_lose_updates() {
    let t = TempDir::new("threads");
    {
        let mut s = SqliteStore::open(t.db()).unwrap();
        s.create(&sample_header(1, 7)).unwrap();
    }
    let path = t.db();
    let threads: Vec<_> = (0..4u128)
        .map(|w| {
            let path = path.clone();
            std::thread::spawn(move || {
                let mut s = SqliteStore::open(path).unwrap();
                for i in 0..25u128 {
                    s.put(&sample_record(w * 100 + i, 1, 32)).unwrap();
                }
            })
        })
        .collect();
    for th in threads {
        th.join().unwrap();
    }
    let s = SqliteStore::open(&path).unwrap();
    assert_eq!(s.list().unwrap().len(), 100);
}
