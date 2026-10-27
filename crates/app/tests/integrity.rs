//! Rollback, silent deletion and forged tombstones (audit findings P2-1 and P2-2).
//!
//! A record proves it is genuine, not that it is the newest, or that nothing else was removed. The vault therefore
//! keeps a sealed manifest, and unlock compares what the store holds with it.
#![cfg(not(target_family = "wasm"))]

use rahasyakosh_app::{Config, Vault, Warning};
use rahasyakosh_core::crypto::KdfParams;
use rahasyakosh_core::item::Item;
use rahasyakosh_core::vault::UnlockedVault;
use rahasyakosh_store::{DirectoryStore, MemoryStore, Store, Uuid};
use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

const PW: &str = "correct horse battery staple";
const KDF: KdfParams = KdfParams {
    m_kib: KdfParams::MIN_M_KIB,
    t: KdfParams::MIN_T,
    p: 1,
};

fn rng() -> ChaCha20Rng {
    ChaCha20Rng::seed_from_u64(5)
}

fn id(n: u128) -> Uuid {
    Uuid::from_u128(0x0190_0000_0000_7000_8000_0000_0000_0000 + n)
}

fn dir(name: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let p = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "integrity-{name}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&p);
    p
}

fn open(path: &Path) -> Vault<DirectoryStore> {
    Vault::unlock(DirectoryStore::at(path), PW, Config::default()).unwrap()
}

/// A directory vault holding items 1 and 2, locked again.
fn two_items(path: &Path) {
    let mut v = Vault::create(
        DirectoryStore::at(path),
        PW,
        KDF,
        Config::default(),
        &mut rng(),
    )
    .unwrap();
    v.add(Item::login("One", "u", "p1", 1, id(1)), &mut rng())
        .unwrap();
    v.add(Item::login("Two", "u", "p2", 1, id(2)), &mut rng())
        .unwrap();
}

fn rkr(path: &Path, n: u128) -> PathBuf {
    path.join(format!("{}.rkr", id(n)))
}

fn edit_password(v: &mut Vault<DirectoryStore>, n: u128, pw: &str) {
    v.edit(id(n), 2, &mut rng(), |i| {
        i.set_field("password", pw, true);
    })
    .unwrap();
}

#[test]
fn a_clean_vault_has_no_warnings_and_counts_its_writes() {
    let p = dir("clean");
    two_items(&p);
    let v = open(&p);
    assert!(v.warnings().is_empty(), "{:?}", v.warnings());
    assert_eq!(v.len(), 2);
    assert_eq!(v.seq(), 2, "one write per add");
}

#[test]
fn an_old_record_put_back_is_noticed() {
    let p = dir("rollback");
    two_items(&p);
    let old = std::fs::read(rkr(&p, 1)).unwrap();
    let mut v = open(&p);
    edit_password(&mut v, 1, "new-password");
    drop(v);
    std::fs::write(rkr(&p, 1), old).unwrap();

    let v = open(&p);
    assert!(
        v.warnings().iter().any(|w| matches!(
            w,
            Warning::RolledBack { id: i, expected: 2, found: 1 } if *i == id(1)
        )),
        "{:?}",
        v.warnings()
    );
}

#[test]
fn a_deleted_record_file_is_noticed() {
    let p = dir("deleted");
    two_items(&p);
    std::fs::remove_file(rkr(&p, 2)).unwrap();
    let v = open(&p);
    assert_eq!(v.len(), 1);
    assert!(
        v.warnings()
            .iter()
            .any(|w| matches!(w, Warning::Missing { id: i, .. } if *i == id(2))),
        "{:?}",
        v.warnings()
    );
}

#[test]
fn a_forged_tombstone_does_not_hide_an_item_or_block_it() {
    let p = dir("forged");
    two_items(&p);
    std::fs::write(
        p.join(format!("{}.rkt", id(1))),
        format!("{{\"rev\":{}}}", u64::MAX),
    )
    .unwrap();

    let mut v = open(&p);
    assert_eq!(v.len(), 2, "the item must still be there");
    assert_eq!(v.get(id(1)).unwrap().title, "One");
    assert!(
        v.warnings()
            .iter()
            .any(|w| matches!(w, Warning::ForgedTombstone { id: i } if *i == id(1))),
        "{:?}",
        v.warnings()
    );
    // And the id is not locked out for good.
    edit_password(&mut v, 1, "still-editable");
    assert!(v.store().tombstones().unwrap().is_empty());
}

#[test]
fn a_real_purge_stays_purged_and_is_not_a_warning() {
    let p = dir("purge");
    two_items(&p);
    let mut v = open(&p);
    v.purge(id(2), &mut rng()).unwrap();
    drop(v);
    let v = open(&p);
    assert_eq!(v.len(), 1);
    assert!(v.warnings().is_empty(), "{:?}", v.warnings());
}

#[test]
fn a_purged_item_put_back_stays_hidden_and_is_reported() {
    let p = dir("resurrect");
    two_items(&p);
    let old = std::fs::read(rkr(&p, 2)).unwrap();
    let mut v = open(&p);
    v.purge(id(2), &mut rng()).unwrap();
    drop(v);
    // An attacker also removes the tombstone and puts the record back.
    std::fs::remove_file(p.join(format!("{}.rkt", id(2)))).unwrap();
    std::fs::write(rkr(&p, 2), old).unwrap();

    let v = open(&p);
    assert!(v.get(id(2)).is_none(), "a purged item must stay purged");
    assert!(
        v.warnings()
            .iter()
            .any(|w| matches!(w, Warning::Resurrected { id: i } if *i == id(2))),
        "{:?}",
        v.warnings()
    );
}

#[test]
fn a_vault_older_than_this_device_last_saw_is_noticed() {
    let p = dir("highwater");
    two_items(&p);
    let v = Vault::try_unlock_checked(
        DirectoryStore::at(&p),
        PW,
        Config::default(),
        Some(9), // this device has seen the vault at write number 9
    )
    .map_err(|f| f.error)
    .unwrap();
    assert!(
        v.warnings()
            .iter()
            .any(|w| matches!(w, Warning::OlderThanSeen { seen: 9, found: 2 })),
        "{:?}",
        v.warnings()
    );
}

#[test]
fn a_removed_manifest_is_noticed_on_a_device_that_had_seen_one() {
    let p = dir("nomanifest");
    two_items(&p);
    std::fs::remove_file(rkr_nil(&p)).unwrap();
    let v = Vault::try_unlock_checked(DirectoryStore::at(&p), PW, Config::default(), Some(2))
        .map_err(|f| f.error)
        .unwrap();
    assert!(
        v.warnings()
            .iter()
            .any(|w| matches!(w, Warning::ManifestMissing)),
        "{:?}",
        v.warnings()
    );
}

fn rkr_nil(path: &Path) -> PathBuf {
    path.join(format!("{}.rkr", Uuid::nil()))
}

#[test]
fn an_old_vault_without_a_manifest_opens_quietly_and_gets_one_on_the_first_write() {
    // Built the way earlier versions did: records only, no manifest.
    let mut store = MemoryStore::new();
    let keys = UnlockedVault::create(PW, KDF, &mut rng()).unwrap();
    store.create(keys.header()).unwrap();
    let item = Item::login("Old", "u", "p", 1, id(1));
    store
        .put(&keys.seal_item(&item, 1, &mut rng()).unwrap())
        .unwrap();

    let mut v = Vault::unlock(store, PW, Config::default()).unwrap();
    assert!(v.warnings().is_empty(), "{:?}", v.warnings());
    assert_eq!(v.seq(), 0);
    v.add(Item::login("New", "u", "p", 1, id(2)), &mut rng())
        .unwrap();
    assert_eq!(v.seq(), 1);

    let store = v.into_store();
    assert!(
        store.get(Uuid::nil()).unwrap().is_some(),
        "manifest written"
    );
    let v = Vault::unlock(store, PW, Config::default()).unwrap();
    assert!(v.warnings().is_empty(), "{:?}", v.warnings());
    assert_eq!(v.len(), 2);
}

#[test]
fn the_manifest_is_never_listed_as_an_item() {
    let p = dir("hidden");
    two_items(&p);
    let v = open(&p);
    assert_eq!(v.items(rahasyakosh_app::Scope::Any).len(), 2);
    assert!(v.unreadable().is_empty());
}

#[test]
fn the_high_water_file_only_goes_up_and_is_private() {
    use rahasyakosh_app::HighWater;
    let p = dir("hwfile").join("state").join("seen.json");
    let mut h = HighWater::at(&p);
    assert_eq!(h.last_seen(id(1)), None);
    h.record(id(1), 5).unwrap();
    h.record(id(1), 3).unwrap();
    assert_eq!(HighWater::at(&p).last_seen(id(1)), Some(5));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }
    // A damaged file starts empty instead of failing.
    std::fs::write(&p, b"garbage").unwrap();
    assert_eq!(HighWater::at(&p).last_seen(id(1)), None);
}

#[test]
fn a_tracked_vault_remembers_its_writes_and_notices_a_restored_copy() {
    use rahasyakosh_app::HighWater;
    let root = dir("tracked");
    let vault_dir = root.join("vault");
    let seen = root.join("seen.json");
    two_items(&vault_dir);
    let backup = std::fs::read_dir(&vault_dir)
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            (p.clone(), std::fs::read(p).unwrap())
        })
        .collect::<Vec<_>>();

    let mut v = open(&vault_dir);
    v.track(HighWater::at(&seen));
    edit_password(&mut v, 1, "changed");
    let vault_id = v.vault_id();
    drop(v);
    assert_eq!(HighWater::at(&seen).last_seen(vault_id), Some(3));

    // The whole folder is put back to how it was before the edit: every file is genuine.
    for (p, bytes) in &backup {
        std::fs::write(p, bytes).unwrap();
    }
    let plain = open(&vault_dir);
    assert!(
        plain.warnings().is_empty(),
        "nothing inside the folder shows it"
    );
    drop(plain);
    let v = Vault::try_unlock_checked(
        DirectoryStore::at(&vault_dir),
        PW,
        Config::default(),
        HighWater::at(&seen).last_seen(vault_id),
    )
    .map_err(|f| f.error)
    .unwrap();
    assert!(
        v.warnings()
            .iter()
            .any(|w| matches!(w, Warning::OlderThanSeen { seen: 3, found: 2 })),
        "{:?}",
        v.warnings()
    );
}
