//! What `Vault::unlock` does when the stored data has been damaged or planted by someone else
//! (audit finding P2-3): one bad record must not lock the user out of everything else.
#![cfg(not(target_family = "wasm"))]

use rahasyakosh_app::{Config, Error, Vault};
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

fn temp(name: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let p = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "hostile-{name}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&p);
    p
}

fn vault_with_two_items<S: Store>(store: S) -> Vault<S> {
    let mut v = Vault::create(store, PW, KDF, Config::default(), &mut rng()).unwrap();
    v.add(Item::login("One", "u", "p1", 1, id(1)), &mut rng())
        .unwrap();
    v.add(Item::login("Two", "u", "p2", 1, id(2)), &mut rng())
        .unwrap();
    v
}

#[test]
fn a_garbage_record_file_is_reported_and_the_rest_opens() {
    let dir = temp("garbage");
    let v = vault_with_two_items(DirectoryStore::at(dir.join("vault")));
    drop(v.into_store());
    let planted = id(77);
    std::fs::write(dir.join("vault").join(format!("{planted}.rkr")), b"{}").unwrap();

    let v = Vault::unlock(DirectoryStore::at(dir.join("vault")), PW, Config::default())
        .expect("one bad file must not block the vault");
    assert_eq!(v.len(), 2);
    assert_eq!(v.get(id(1)).unwrap().title, "One");
    assert_eq!(v.get(id(2)).unwrap().title, "Two");
    let skipped = v.unreadable();
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].id, Some(planted));
    assert!(!skipped[0].reason.is_empty());
}

#[test]
fn a_record_that_does_not_open_is_reported_and_the_rest_opens() {
    // A record sealed under another vault's key is well-formed but can never open here.
    let mut store = vault_with_two_items(MemoryStore::new()).into_store();
    // (A different seed: the same seed would make the same vault key.)
    let mut other = ChaCha20Rng::seed_from_u64(99);
    let stranger = UnlockedVault::create("someone else", KDF, &mut other).unwrap();
    let foreign = stranger
        .seal_item(&Item::login("Planted", "u", "p", 1, id(9)), 1, &mut other)
        .unwrap();
    store.put(&foreign).unwrap();

    let v = Vault::unlock(store, PW, Config::default()).unwrap();
    assert_eq!(v.len(), 2);
    assert_eq!(v.unreadable().len(), 1);
    assert_eq!(v.unreadable()[0].id, Some(id(9)));
}

#[test]
fn a_clean_vault_reports_nothing() {
    let v = vault_with_two_items(MemoryStore::new());
    let v = Vault::unlock(v.into_store(), PW, Config::default()).unwrap();
    assert!(v.unreadable().is_empty());
}

#[test]
fn a_wrong_password_still_fails_closed_and_shows_nothing() {
    let dir = temp("wrongpw");
    let v = vault_with_two_items(DirectoryStore::at(dir.join("vault")));
    drop(v.into_store());
    std::fs::write(dir.join("vault").join(format!("{}.rkr", id(77))), b"{}").unwrap();

    let r = Vault::unlock(
        DirectoryStore::at(dir.join("vault")),
        "wrong password",
        Config::default(),
    );
    assert!(matches!(
        r,
        Err(Error::Core(rahasyakosh_core::Error::Decrypt))
    ));
}
