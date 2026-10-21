//! Directory backend tests. They use real files and Unix permissions, so they run on native targets only.
#![cfg(not(target_family = "wasm"))]

use rahasyakosh_core::crypto::KdfParams;
use rahasyakosh_core::vault::SealedKey;
use rahasyakosh_store::conformance::{self, Fixture};
use rahasyakosh_store::{DirectoryStore, Error, Record, Store, Tombstone, Uuid, VaultHeader};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// A fresh directory under the target dir that is removed when dropped.
struct TempDir(PathBuf);

impl TempDir {
    fn new(name: &str) -> Self {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "dirstore-{name}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&p);
        TempDir(p)
    }
    fn store(&self) -> DirectoryStore {
        DirectoryStore::at(&self.0)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Dir(std::sync::Mutex<Vec<TempDir>>);

impl Fixture for Dir {
    type S = DirectoryStore;
    fn fresh(&self) -> DirectoryStore {
        let t = TempDir::new("conf");
        let s = t.store();
        self.0.lock().unwrap().push(t);
        s
    }
    fn reopen(&self, s: DirectoryStore) -> DirectoryStore {
        DirectoryStore::at(s.path())
    }
}

#[test]
fn directory_store_passes_the_conformance_suite() {
    conformance::run(&Dir(Default::default()));
}

fn id(n: u128) -> Uuid {
    Uuid::from_u128(0x0190_0000_0000_7000_8000_0000_0000_0000 + n)
}

fn header() -> VaultHeader {
    VaultHeader {
        format_version: 1,
        vault_id: id(1),
        kdf: KdfParams::DESKTOP,
        salt: [1; 16],
        wrapped_vk: SealedKey {
            nonce: [2; 24],
            ct: vec![3; 48],
        },
    }
}

fn record(n: u128, rev: u64) -> Record {
    Record {
        record_version: 1,
        item_id: id(n),
        rev,
        nonce: [n as u8; 24],
        ct: vec![n as u8; 64],
    }
}

fn ready(t: &TempDir) -> DirectoryStore {
    let mut s = t.store();
    s.create(&header()).unwrap();
    s
}

#[test]
fn files_have_the_documented_names_and_private_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let t = TempDir::new("layout");
    let mut s = ready(&t);
    s.put(&record(1, 1)).unwrap();
    s.purge(id(2), 1).unwrap();

    let mut names: Vec<String> = std::fs::read_dir(&t.0)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    let mut expected = vec![
        "header.json".to_string(),
        format!("{}.rkr", id(1)),
        format!("{}.rkt", id(2)),
    ];
    expected.sort();
    assert_eq!(names, expected);

    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&t.0), 0o700);
    for n in &names {
        assert_eq!(mode(&t.0.join(n)), 0o600, "{n}");
    }
    // No temporary files are left behind by a successful write.
    assert!(names.iter().all(|n| !n.ends_with(".tmp")));
}

#[test]
fn unrelated_files_are_ignored() {
    let t = TempDir::new("stray");
    let mut s = ready(&t);
    s.put(&record(1, 1)).unwrap();
    for stray in [
        "notes.txt",
        "not-a-uuid.rkr",
        ".hidden.rkr.tmp",
        &format!("{} (conflicted copy).rkr", id(1)),
        &format!("{}.rkr.bak", id(1)),
    ] {
        std::fs::write(t.0.join(stray), b"garbage").unwrap();
    }
    assert_eq!(s.all().unwrap(), vec![record(1, 1)]);
    assert_eq!(s.list().unwrap().len(), 1);
    assert!(s.tombstones().unwrap().is_empty());
    s.put(&record(2, 1)).unwrap();
    assert_eq!(s.all().unwrap().len(), 2);
}

#[test]
fn corrupt_files_are_reported_not_panicked_on() {
    let t = TempDir::new("corrupt");
    let mut s = ready(&t);
    s.put_many(&[record(1, 1), record(2, 1)]).unwrap();

    // Garbage in a record file.
    std::fs::write(t.0.join(format!("{}.rkr", id(1))), b"{ not json").unwrap();
    assert!(matches!(s.get(id(1)), Err(Error::Corrupt(_))));
    assert!(matches!(s.all(), Err(Error::Corrupt(_))));
    assert!(matches!(s.list(), Err(Error::Corrupt(_))));
    assert!(
        matches!(s.put(&record(1, 2)), Err(Error::Corrupt(_))),
        "a corrupt record is not silently overwritten"
    );
    // The healthy item is still readable on its own.
    assert_eq!(s.get(id(2)).unwrap(), Some(record(2, 1)));

    // A valid record filed under the wrong id.
    let good = serde_json::to_vec(&record(2, 1)).unwrap();
    std::fs::write(t.0.join(format!("{}.rkr", id(1))), good).unwrap();
    assert!(matches!(s.get(id(1)), Err(Error::Corrupt(_))));

    // Truncated / garbage header and tombstone.
    std::fs::write(t.0.join("header.json"), b"{\"format_ver").unwrap();
    assert!(matches!(s.header(), Err(Error::Corrupt(_))));
    std::fs::write(t.0.join(format!("{}.rkt", id(3))), b"nope").unwrap();
    assert!(matches!(s.tombstones(), Err(Error::Corrupt(_))));
    assert!(matches!(s.get(id(3)), Ok(None)));
}

#[test]
fn oversized_files_are_refused() {
    let t = TempDir::new("huge");
    let s = ready(&t);
    let f = std::fs::File::create(t.0.join(format!("{}.rkr", id(1)))).unwrap();
    f.set_len(65 * 1024 * 1024).unwrap();
    assert!(matches!(s.get(id(1)), Err(Error::Corrupt(_))));
}

#[test]
fn a_record_left_behind_by_a_crashed_purge_reads_as_purged() {
    // `purge` writes the tombstone first, then removes the record. Simulate dying in between.
    let t = TempDir::new("crash-purge");
    let mut s = ready(&t);
    s.put(&record(1, 2)).unwrap();
    std::fs::write(t.0.join(format!("{}.rkt", id(1))), br#"{"rev":3}"#).unwrap();

    assert!(s.get(id(1)).unwrap().is_none());
    assert!(s.all().unwrap().is_empty());
    assert!(s.list().unwrap().is_empty());
    assert_eq!(
        s.tombstones().unwrap(),
        vec![Tombstone { id: id(1), rev: 3 }]
    );
    // Finishing the purge is harmless, and a genuinely newer write revives the item.
    s.purge(id(1), 3).unwrap();
    s.put(&record(1, 4)).unwrap();
    assert!(s.get(id(1)).unwrap().is_some());
}

#[test]
fn a_tombstone_left_behind_by_a_crashed_write_is_ignored() {
    // `put` writes the record, then removes the tombstone. Simulate dying in between.
    let t = TempDir::new("crash-put");
    let mut s = ready(&t);
    s.purge(id(1), 3).unwrap();
    std::fs::write(
        t.0.join(format!("{}.rkr", id(1))),
        serde_json::to_vec(&record(1, 4)).unwrap(),
    )
    .unwrap();

    assert_eq!(s.get(id(1)).unwrap(), Some(record(1, 4)));
    assert_eq!(s.all().unwrap(), vec![record(1, 4)]);
    assert!(
        s.tombstones().unwrap().is_empty(),
        "the superseded tombstone is not reported"
    );
}

#[test]
fn two_handles_on_one_directory_see_each_others_writes() {
    // This is what a folder-sync tool effectively does.
    let t = TempDir::new("two");
    let mut a = ready(&t);
    let b = DirectoryStore::at(&t.0);
    a.put(&record(1, 1)).unwrap();
    assert_eq!(b.get(id(1)).unwrap(), Some(record(1, 1)));
    a.set_header(&VaultHeader {
        salt: [9; 16],
        ..header()
    })
    .unwrap();
    assert_eq!(b.header().unwrap().salt, [9; 16]);
}
