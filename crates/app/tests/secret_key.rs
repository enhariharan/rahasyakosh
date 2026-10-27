//! The Secret Key in the app layer (audit finding P1-3): setting it, losing it, and the device's key folder.
#![cfg(not(target_family = "wasm"))]

use rahasyakosh_app::{Config, Error, Scope, SecretKeyDir, Session, Settings, UnlockError, Vault};
use rahasyakosh_core::crypto::KdfParams;
use rahasyakosh_core::item::Item;
use rahasyakosh_store::{MemoryStore, Uuid};
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
    ChaCha20Rng::seed_from_u64(8)
}

fn temp(name: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let p = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "sk-{name}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = std::fs::remove_dir_all(&p);
    p
}

fn fast() -> Settings {
    Settings {
        kdf_memory_mib: KdfParams::MIN_M_KIB.div_ceil(1024),
        kdf_passes: KdfParams::MIN_T,
        ..Settings::default()
    }
}

fn vault_with_item() -> Vault<MemoryStore> {
    let mut v = Vault::create(MemoryStore::new(), PW, KDF, Config::default(), &mut rng()).unwrap();
    v.add(
        Item::login("Bank", "me", "pw", 1, Uuid::from_u128(1)),
        &mut rng(),
    )
    .unwrap();
    v
}

#[test]
fn adding_a_secret_key_keeps_the_items_and_needs_both_to_open() {
    let mut v = vault_with_item();
    let sk = v.set_secret_key(PW, &mut rng()).unwrap();
    assert!(v.has_secret_key());
    let store = v.into_store();

    let e = Vault::unlock(store.clone(), PW, Config::default())
        .err()
        .unwrap();
    assert!(matches!(
        e,
        Error::Core(rahasyakosh_core::Error::SecretKeyRequired)
    ));
    let v = Vault::try_unlock_full(store, PW, Some(&sk), Config::default(), None)
        .map_err(|f| f.error)
        .unwrap();
    assert_eq!(
        v.items(Scope::Live).len(),
        1,
        "no item was re-encrypted or lost"
    );
}

#[test]
fn a_wrong_master_password_cannot_add_one() {
    let mut v = vault_with_item();
    let e = v
        .set_secret_key("not the password", &mut rng())
        .err()
        .unwrap();
    assert!(matches!(e, Error::Core(rahasyakosh_core::Error::Decrypt)));
    assert!(!v.has_secret_key(), "the vault must be unchanged");
    let store = v.into_store();
    Vault::unlock(store, PW, Config::default()).expect("still opens with the password alone");
}

#[test]
fn it_can_be_removed_again_and_survives_a_password_change() {
    let mut v = vault_with_item();
    let sk = v.set_secret_key(PW, &mut rng()).unwrap();
    v.change_password("another long passphrase", KDF, &mut rng())
        .unwrap();
    let store = v.into_store();
    let mut v = Vault::try_unlock_full(
        store,
        "another long passphrase",
        Some(&sk),
        Config::default(),
        None,
    )
    .map_err(|f| f.error)
    .unwrap();
    v.clear_secret_key("another long passphrase", &mut rng())
        .unwrap();
    let store = v.into_store();
    Vault::unlock(store, "another long passphrase", Config::default()).unwrap();
}

#[test]
fn the_device_keeps_the_key_in_a_private_file_apart_from_the_vault() {
    let dir = SecretKeyDir::at(temp("dir").join("keys"));
    let mut v = vault_with_item();
    let sk = v.set_secret_key(PW, &mut rng()).unwrap();
    let id = v.vault_id();
    assert!(dir.load(id).is_none());
    dir.save(id, &sk).unwrap();
    assert_eq!(dir.load(id).unwrap(), sk);
    dir.remove(id).unwrap();
    dir.remove(id).unwrap();
    assert!(dir.load(id).is_none());
}

#[test]
fn a_session_uses_the_device_key_or_one_the_user_types() {
    let keys = SecretKeyDir::at(temp("session").join("keys"));
    let mut s = Session::create(MemoryStore::new(), PW, fast(), 0, &mut rng())
        .unwrap()
        .with_secret_keys(keys.clone());
    let sk = s.vault(0).unwrap().set_secret_key(PW, &mut rng()).unwrap();
    let id = s.vault(0).unwrap().vault_id();
    s.lock();

    // No key on the device and none typed: refused, and that is not a wrong password.
    let e = s.unlock(PW, 1, &mut rng()).unwrap_err();
    assert!(
        matches!(
            e,
            UnlockError::Failed(Error::Core(rahasyakosh_core::Error::SecretKeyRequired))
        ),
        "{e:?}"
    );
    // A typed key works.
    s.provide_secret_key(Some(sk.clone()));
    s.unlock(PW, 2, &mut rng()).unwrap();
    s.lock();
    s.provide_secret_key(None);

    // The device's own copy works without typing anything.
    keys.save(id, &sk).unwrap();
    s.unlock(PW, 3, &mut rng()).unwrap();
}

#[cfg(unix)]
#[test]
fn the_key_file_and_its_folder_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let root = temp("modes").join("keys");
    let dir = SecretKeyDir::at(&root);
    let mut v = vault_with_item();
    let sk = v.set_secret_key(PW, &mut rng()).unwrap();
    dir.save(v.vault_id(), &sk).unwrap();
    let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(&root), 0o700);
    let file = std::fs::read_dir(&root)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(mode(&file), 0o600);
}

#[test]
fn a_key_that_worked_is_kept_on_the_device_so_the_next_unlock_needs_only_the_password() {
    let keys = SecretKeyDir::at(temp("typed").join("keys"));
    let mut s = Session::create(MemoryStore::new(), PW, fast(), 0, &mut rng())
        .unwrap()
        .with_secret_keys(keys.clone());
    let sk = s.vault(0).unwrap().set_secret_key(PW, &mut rng()).unwrap();
    let id = s.vault(0).unwrap().vault_id();
    s.lock();
    assert!(s.unlock_hints().needs_secret_key);

    // A wrong password with the right key saves nothing.
    s.provide_secret_key(Some(sk.clone()));
    assert!(s.unlock("wrong", 1, &mut rng()).is_err());
    assert!(keys.load(id).is_none());

    s.provide_secret_key(Some(sk.clone()));
    s.unlock(PW, 100, &mut rng()).unwrap();
    assert_eq!(keys.load(id).unwrap(), sk);
    s.lock();
    assert!(!s.unlock_hints().needs_secret_key);
    s.unlock(PW, 200, &mut rng()).unwrap();
}

#[test]
fn a_session_can_add_a_secret_key_and_keeps_it_on_the_device() {
    let keys = SecretKeyDir::at(temp("add").join("keys"));
    let mut s = Session::create(MemoryStore::new(), PW, fast(), 0, &mut rng())
        .unwrap()
        .with_secret_keys(keys.clone());
    let id = s.vault(0).unwrap().vault_id();

    // The master password is checked first, and nothing is kept when it is wrong.
    assert!(s.add_secret_key("not the password", 1, &mut rng()).is_err());
    assert!(keys.load(id).is_none());

    let (text, kept) = s.add_secret_key(PW, 2, &mut rng()).unwrap();
    assert!(kept);
    assert_eq!(keys.load(id).unwrap().to_text().as_str(), text.as_str());

    // This device opens it with the password alone; a device without the key needs the text.
    s.lock();
    assert!(!s.unlock_hints().needs_secret_key);
    s.unlock(PW, 100, &mut rng()).unwrap();
    s.lock();
    keys.remove(id).unwrap();
    assert!(s.unlock_hints().needs_secret_key);
    s.provide_secret_key(Some(
        rahasyakosh_core::secret_key::SecretKey::parse(&text).unwrap(),
    ));
    s.unlock(PW, 200, &mut rng()).unwrap();
}

#[test]
fn without_a_key_folder_the_key_is_not_kept_and_the_caller_is_told() {
    let mut s = Session::create(MemoryStore::new(), PW, fast(), 0, &mut rng()).unwrap();
    let (_, kept) = s.add_secret_key(PW, 1, &mut rng()).unwrap();
    assert!(!kept);
}
