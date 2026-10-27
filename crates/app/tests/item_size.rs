//! The size limit on one item (audit finding P1-7): too big is refused when saved, never found out at unlock.
#![cfg(not(target_family = "wasm"))]

use rahasyakosh_app::{Config, Error, Vault};
use rahasyakosh_core::crypto::KdfParams;
use rahasyakosh_core::item::Item;
use rahasyakosh_core::vault::MAX_ITEM_BYTES;
use rahasyakosh_core::Error as CoreError;
use rahasyakosh_store::{MemoryStore, Uuid};
use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};

const PW: &str = "correct horse battery staple";
const KDF: KdfParams = KdfParams {
    m_kib: KdfParams::MIN_M_KIB,
    t: KdfParams::MIN_T,
    p: 1,
};

fn rng() -> ChaCha20Rng {
    ChaCha20Rng::seed_from_u64(9)
}

fn id(n: u128) -> Uuid {
    Uuid::from_u128(0x0190_0000_0000_7000_8000_0000_0000_0000 + n)
}

fn note(id: Uuid, bytes: usize) -> Item {
    let mut i = Item::login("Note", "u", "p", 1, id);
    i.notes = "a".repeat(bytes);
    i
}

fn is_too_large(e: &Error) -> bool {
    matches!(e, Error::Core(CoreError::ItemTooLarge { .. }))
}

#[test]
fn a_too_large_item_is_refused_and_the_vault_still_opens() {
    let store = MemoryStore::default();
    let mut v = Vault::create(store, PW, KDF, Config::default(), &mut rng()).unwrap();
    v.add(Item::login("Small", "u", "p", 1, id(1)), &mut rng())
        .unwrap();

    let err = v
        .add(note(id(2), MAX_ITEM_BYTES + 1), &mut rng())
        .unwrap_err();
    assert!(is_too_large(&err), "{err:?}");
    assert!(v.get(id(2)).is_none());

    // An edit that pushes an existing item over the limit is refused too, and the item is unchanged.
    let err = v
        .edit(id(1), 5, &mut rng(), |i| {
            i.notes = "a".repeat(MAX_ITEM_BYTES)
        })
        .unwrap_err();
    assert!(is_too_large(&err), "{err:?}");
    assert_eq!(v.get(id(1)).unwrap().notes, "");
}

#[test]
fn an_item_just_under_the_limit_is_saved_and_reloaded() {
    let mut v = Vault::create(
        MemoryStore::default(),
        PW,
        KDF,
        Config::default(),
        &mut rng(),
    )
    .unwrap();
    // Leave room for the rest of the item's JSON.
    v.add(note(id(1), MAX_ITEM_BYTES - 1024), &mut rng())
        .unwrap();
    assert_eq!(v.get(id(1)).unwrap().notes.len(), MAX_ITEM_BYTES - 1024);
}

#[test]
fn an_import_with_one_huge_row_adds_nothing() {
    let mut v = Vault::create(
        MemoryStore::default(),
        PW,
        KDF,
        Config::default(),
        &mut rng(),
    )
    .unwrap();
    let batch = vec![
        Item::login("Fine", "u", "p", 1, id(1)),
        note(id(2), MAX_ITEM_BYTES + 1),
    ];
    let err = v.add_many(batch, &mut rng()).unwrap_err();
    assert!(is_too_large(&err), "{err:?}");
    assert!(v.get(id(1)).is_none());
}
