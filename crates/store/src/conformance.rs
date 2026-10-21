//! A test suite that every [`Store`] backend must pass, so they all behave identically.
//!
//! A backend implements [`Fixture`] (how to make an empty store, and how to "restart" one) and calls [`run`]
//! from a `#[test]`. The data is made up: the store never looks inside a record, so no keys are needed.
use crate::{Error, Record, RecordMeta, Store, Tombstone, Uuid, VaultHeader};
use rahasyakosh_core::crypto::KdfParams;
use rahasyakosh_core::vault::SealedKey;

pub trait Fixture {
    type S: Store;
    /// A brand-new, empty store (no vault created yet).
    fn fresh(&self) -> Self::S;
    /// Close the store and open it again from wherever it keeps its data. In-memory stores return it unchanged.
    fn reopen(&self, store: Self::S) -> Self::S;
}

fn id(n: u128) -> Uuid {
    Uuid::from_u128(0x0190_0000_0000_7000_8000_0000_0000_0000 + n)
}

fn header(vault: u128, salt_byte: u8) -> VaultHeader {
    VaultHeader {
        format_version: 1,
        vault_id: id(vault),
        kdf: KdfParams::DESKTOP,
        salt: [salt_byte; 16],
        wrapped_vk: SealedKey {
            nonce: [salt_byte; 24],
            ct: vec![salt_byte; 48],
        },
    }
}

/// Binary-looking content, so a backend that mangles bytes (text encoding, NUL handling) is caught.
fn record(n: u128, rev: u64, len: usize) -> Record {
    Record {
        record_version: 1,
        item_id: id(n),
        rev,
        nonce: std::array::from_fn(|i| (i as u8).wrapping_add(n as u8)),
        ct: (0..len).map(|i| (i % 251) as u8 ^ (n as u8)).collect(),
    }
}

fn is_stale(e: &Error) -> bool {
    matches!(e, Error::StaleRevision { .. })
}

/// A store with a vault created, ready for records.
fn ready<F: Fixture>(f: &F) -> F::S {
    let mut s = f.fresh();
    s.create(&header(1, 7)).unwrap();
    s
}

/// Run every case. Panics with the case name on the first failure.
pub fn run<F: Fixture>(f: &F) {
    empty_store(f);
    create_and_header(f);
    set_header_rules(f);
    put_get_list_all(f);
    stale_and_atomic_batches(f);
    purge_and_tombstones(f);
    survives_reopen(f);
    binary_and_large_values(f);
    many_records(f);
}

fn empty_store<F: Fixture>(f: &F) {
    let s = f.fresh();
    assert!(
        matches!(s.header(), Err(Error::NotInitialised)),
        "empty: header before create"
    );
    assert!(s.get(id(1)).unwrap().is_none(), "empty: get");
    assert!(s.list().unwrap().is_empty(), "empty: list");
    assert!(s.all().unwrap().is_empty(), "empty: all");
    assert!(s.tombstones().unwrap().is_empty(), "empty: tombstones");
    let mut s = f.fresh();
    assert!(
        matches!(s.put(&record(1, 1, 10)), Err(Error::NotInitialised)),
        "empty: put before create"
    );
    assert!(
        matches!(s.purge(id(1), 1), Err(Error::NotInitialised)),
        "empty: purge before create"
    );
}

fn create_and_header<F: Fixture>(f: &F) {
    let mut s = f.fresh();
    let h = header(1, 7);
    s.create(&h).unwrap();
    assert_eq!(s.header().unwrap(), h, "create: header round trip");
    assert!(
        matches!(s.create(&header(2, 9)), Err(Error::AlreadyExists)),
        "create: twice"
    );
    assert_eq!(
        s.header().unwrap(),
        h,
        "create: failed create changes nothing"
    );
}

fn set_header_rules<F: Fixture>(f: &F) {
    let mut s = f.fresh();
    assert!(
        matches!(s.set_header(&header(1, 7)), Err(Error::NotInitialised)),
        "set_header: before create"
    );
    let mut s = ready(f);
    let changed = header(1, 99);
    s.set_header(&changed).unwrap();
    assert_eq!(s.header().unwrap(), changed, "set_header: replaced");
    assert!(
        matches!(s.set_header(&header(2, 7)), Err(Error::WrongVault)),
        "set_header: other vault id"
    );
    assert_eq!(
        s.header().unwrap(),
        changed,
        "set_header: rejected change not applied"
    );
}

fn put_get_list_all<F: Fixture>(f: &F) {
    let mut s = ready(f);
    // Written out of order: reads must still come back sorted by id.
    let (a, b, c) = (record(3, 1, 64), record(1, 1, 80), record(2, 5, 128));
    s.put_many(&[a.clone(), b.clone(), c.clone()]).unwrap();

    assert_eq!(s.get(id(1)).unwrap(), Some(b.clone()), "get: b");
    assert_eq!(s.get(id(2)).unwrap(), Some(c.clone()), "get: c");
    assert_eq!(s.get(id(3)).unwrap(), Some(a.clone()), "get: a");
    assert!(s.get(id(4)).unwrap().is_none(), "get: unknown id");

    assert_eq!(
        s.all().unwrap(),
        vec![b.clone(), c.clone(), a.clone()],
        "all: sorted by id"
    );
    assert_eq!(
        s.list().unwrap(),
        vec![
            RecordMeta {
                id: id(1),
                rev: 1,
                size: 80
            },
            RecordMeta {
                id: id(2),
                rev: 5,
                size: 128
            },
            RecordMeta {
                id: id(3),
                rev: 1,
                size: 64
            },
        ],
        "list: metadata only, sorted"
    );

    // A newer revision replaces the old one, and other items are untouched.
    let b2 = record(1, 2, 96);
    s.put(&b2).unwrap();
    assert_eq!(s.get(id(1)).unwrap(), Some(b2.clone()), "update: replaced");
    assert_eq!(s.all().unwrap(), vec![b2, c, a], "update: others untouched");
    assert!(s.put_many(&[]).is_ok(), "put_many: empty batch");
}

fn stale_and_atomic_batches<F: Fixture>(f: &F) {
    let mut s = ready(f);
    let orig = record(1, 3, 40);
    s.put(&orig).unwrap();

    for rev in [3, 2, 0] {
        let e = s.put(&record(1, rev, 50)).unwrap_err();
        assert!(
            is_stale(&e),
            "stale: rev {rev} should be refused, got {e:?}"
        );
    }
    assert_eq!(
        s.get(id(1)).unwrap(),
        Some(orig.clone()),
        "stale: stored value unchanged"
    );

    // One stale record rejects the whole batch: the fresh one must not be written.
    let e = s
        .put_many(&[record(2, 1, 10), record(1, 3, 10)])
        .unwrap_err();
    assert!(is_stale(&e), "batch: stale member, got {e:?}");
    assert!(
        s.get(id(2)).unwrap().is_none(),
        "batch: nothing written when one member is stale"
    );

    // The same item twice in one batch is ambiguous.
    let e = s
        .put_many(&[record(5, 1, 10), record(5, 2, 10)])
        .unwrap_err();
    assert!(
        matches!(e, Error::DuplicateInBatch),
        "batch: duplicate ids, got {e:?}"
    );
    assert!(
        s.get(id(5)).unwrap().is_none(),
        "batch: duplicate batch not written"
    );
}

fn purge_and_tombstones<F: Fixture>(f: &F) {
    let mut s = ready(f);
    s.put_many(&[record(1, 2, 10), record(2, 1, 10)]).unwrap();

    // Purge must be newer than the stored record.
    let e = s.purge(id(1), 2).unwrap_err();
    assert!(is_stale(&e), "purge: not newer than the record, got {e:?}");
    assert!(
        s.get(id(1)).unwrap().is_some(),
        "purge: refused purge keeps the record"
    );

    s.purge(id(1), 3).unwrap();
    assert!(s.get(id(1)).unwrap().is_none(), "purge: record gone");
    assert_eq!(s.list().unwrap().len(), 1, "purge: list excludes it");
    assert_eq!(s.all().unwrap().len(), 1, "purge: all excludes it");
    assert_eq!(
        s.tombstones().unwrap(),
        vec![Tombstone { id: id(1), rev: 3 }],
        "purge: tombstone"
    );

    // Purging again is harmless and never lowers the tombstone.
    s.purge(id(1), 3).unwrap();
    s.purge(id(1), 2).unwrap();
    assert_eq!(
        s.tombstones().unwrap(),
        vec![Tombstone { id: id(1), rev: 3 }],
        "purge: idempotent"
    );
    s.purge(id(1), 6).unwrap();
    assert_eq!(
        s.tombstones().unwrap(),
        vec![Tombstone { id: id(1), rev: 6 }],
        "purge: tombstone rises"
    );

    // Purging something never seen still records a tombstone (a synced device may delete what we lack).
    s.purge(id(9), 4).unwrap();
    assert_eq!(
        s.tombstones().unwrap(),
        vec![
            Tombstone { id: id(1), rev: 6 },
            Tombstone { id: id(9), rev: 4 }
        ],
        "purge: unknown item, sorted"
    );

    // A tombstone blocks old writes; a newer one brings the item back and clears it.
    let e = s.put(&record(1, 6, 10)).unwrap_err();
    assert!(
        is_stale(&e),
        "tombstone: write at the tombstone's rev, got {e:?}"
    );
    let e = s.put(&record(1, 4, 10)).unwrap_err();
    assert!(
        is_stale(&e),
        "tombstone: write below the tombstone's rev, got {e:?}"
    );
    s.put(&record(1, 7, 10)).unwrap();
    assert!(s.get(id(1)).unwrap().is_some(), "tombstone: resurrected");
    assert_eq!(
        s.tombstones().unwrap(),
        vec![Tombstone { id: id(9), rev: 4 }],
        "tombstone: cleared by newer write"
    );
}

fn survives_reopen<F: Fixture>(f: &F) {
    let mut s = ready(f);
    let h = header(1, 42);
    s.set_header(&h).unwrap();
    s.put_many(&[record(1, 1, 33), record(2, 4, 200)]).unwrap();
    s.purge(id(3), 2).unwrap();
    s.purge(id(2), 5).unwrap();

    let s = f.reopen(s);
    assert_eq!(s.header().unwrap(), h, "reopen: header");
    assert_eq!(s.all().unwrap(), vec![record(1, 1, 33)], "reopen: records");
    assert_eq!(
        s.tombstones().unwrap(),
        vec![
            Tombstone { id: id(2), rev: 5 },
            Tombstone { id: id(3), rev: 2 }
        ],
        "reopen: tombstones"
    );
    let mut s = s;
    let e = s.put(&record(1, 1, 5)).unwrap_err();
    assert!(
        is_stale(&e),
        "reopen: revision rules still apply, got {e:?}"
    );
    assert!(
        matches!(s.create(&h), Err(Error::AlreadyExists)),
        "reopen: create again"
    );
}

fn binary_and_large_values<F: Fixture>(f: &F) {
    let mut s = ready(f);
    // All 256 byte values, including NUL and bytes that are invalid UTF-8.
    let mut every_byte = record(1, 1, 0);
    every_byte.ct = (0..=255u8).collect();
    every_byte.nonce = [0; 24];
    let big = record(2, 1, 1024 * 1024);
    let empty = record(3, 1, 0);
    s.put_many(&[every_byte.clone(), big.clone(), empty.clone()])
        .unwrap();
    let s = f.reopen(s);
    assert_eq!(
        s.get(id(1)).unwrap(),
        Some(every_byte),
        "binary: all byte values"
    );
    assert_eq!(s.get(id(2)).unwrap(), Some(big), "binary: 1 MiB value");
    assert_eq!(
        s.get(id(3)).unwrap(),
        Some(empty),
        "binary: empty ciphertext"
    );
}

fn many_records<F: Fixture>(f: &F) {
    let mut s = ready(f);
    let batch: Vec<Record> = (0..300)
        .map(|n| record(n, 1 + (n as u64 % 4), 64 + (n as usize % 5) * 64))
        .collect();
    s.put_many(&batch).unwrap();
    let s = f.reopen(s);
    assert_eq!(s.list().unwrap().len(), 300, "many: count");
    assert_eq!(s.all().unwrap(), batch, "many: contents and order");
    let ids: Vec<Uuid> = s.list().unwrap().iter().map(|m| m.id).collect();
    assert!(
        ids.windows(2).all(|w| w[0] < w[1]),
        "many: strictly sorted by id"
    );
}
