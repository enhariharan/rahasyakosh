//! Performance targets from the roadmap: 10,000 items stay usable, search over 5,000 items takes under 50 ms (US-ITM-04).
//!
//! The limits asserted here are generous so a slow CI machine does not fail them; the timings are printed
//! (`cargo test -p rahasyakosh-app --test perf -- --nocapture`) so a real slowdown is visible long before a limit is hit.
//! Native only: it needs a file-backed SQLite database.
#![cfg(not(target_family = "wasm"))]

use rahasyakosh_app::{Config, Scope, Vault};
use rahasyakosh_core::crypto::KdfParams;
use rahasyakosh_core::item::Item;
use rahasyakosh_store::{SqliteStore, Store, Uuid};
use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const PW: &str = "performance test password";
const KDF: KdfParams = KdfParams {
    m_kib: KdfParams::MIN_M_KIB,
    t: KdfParams::MIN_T,
    p: 1,
};

fn timed<T>(label: &str, f: impl FnOnce() -> T) -> (T, Duration) {
    let start = Instant::now();
    let out = f();
    let took = start.elapsed();
    eprintln!("{label:<44} {:>9.1} ms", took.as_secs_f64() * 1000.0);
    (out, took)
}

/// A realistic-looking login: a title, a username, a URL, tags and a note.
fn item(n: usize) -> Item {
    let mut i = Item::login(
        &format!("Site {n:05} example"),
        &format!("user{n}@example.com"),
        &format!("password-{n}-{}", "x".repeat(n % 20)),
        1_700_000_000,
        Uuid::from_u128(0x0190_0000_0000_7000_8000_0000_0000_0000 + n as u128),
    );
    i.urls.push(format!("https://site{n}.example.com/login"));
    i.tags = vec![format!("group{}", n % 50), "all".to_string()];
    i.notes =
        format!("Account number {n}. Security answer: something fairly long to be realistic.");
    i
}

struct Db(PathBuf);

impl Db {
    fn new(name: &str) -> Self {
        let p = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("perf-{name}-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&p);
        Db(p)
    }
}

impl Drop for Db {
    fn drop(&mut self) {
        for ext in ["", "-wal", "-shm"] {
            let mut s = self.0.clone().into_os_string();
            s.push(ext);
            let _ = std::fs::remove_file(s);
        }
    }
}

fn build(db: &Db, count: usize) -> Vault<SqliteStore> {
    let mut rng = ChaCha20Rng::seed_from_u64(7);
    let store = SqliteStore::open(&db.0).unwrap();
    let mut vault = Vault::create(store, PW, KDF, Config::default(), &mut rng).unwrap();
    let items: Vec<Item> = (0..count).map(item).collect();
    let (_, took) = timed(&format!("add {count} items in one transaction"), || {
        vault.add_many(items, &mut rng).unwrap()
    });
    assert!(took < Duration::from_secs(60));
    vault
}

#[test]
fn ten_thousand_items_unlock_search_and_edit_quickly() {
    let db = Db::new("10k");
    let vault = build(&db, 10_000);
    drop(vault); // lock

    // Unlock = derive keys once (cheap parameters here) + decrypt 10,000 records + build the index.
    let (mut vault, took) = timed("unlock: decrypt + index 10,000 items", || {
        Vault::unlock(SqliteStore::open(&db.0).unwrap(), PW, Config::default()).unwrap()
    });
    assert_eq!(vault.len(), 10_000);
    assert!(
        took < Duration::from_secs(10),
        "unlocking 10k items took {took:?}"
    );

    let (hits, took) = timed("search '00420' (one match)", || vault.search("00420"));
    assert_eq!(hits.len(), 1);
    assert!(took < Duration::from_millis(250), "{took:?}");
    let (hits, _) = timed("search 'group7' (200 matches)", || vault.search("group7"));
    assert_eq!(hits.len(), 200);
    let (hits, _) = timed("search 'user99 example' (two terms)", || {
        vault.search("user99 example")
    });
    assert!(!hits.is_empty());
    let (hits, took) = timed("list everything (empty query)", || vault.search(""));
    assert_eq!(hits.len(), 10_000);
    assert!(took < Duration::from_secs(2), "{took:?}");

    // One edit must not depend on how many items there are: one record is re-sealed and written.
    let id = Uuid::from_u128(0x0190_0000_0000_7000_8000_0000_0000_0000 + 5_000);
    let mut rng = ChaCha20Rng::seed_from_u64(9);
    let (_, took) = timed("edit one item in a 10,000-item vault", || {
        vault
            .edit(id, 2_000, &mut rng, |i| i.notes = "changed".into())
            .unwrap()
    });
    assert!(took < Duration::from_secs(2), "{took:?}");
    let (_, took) = timed("trash one item", || {
        vault.trash(id, 3_000, &mut rng).unwrap()
    });
    assert!(took < Duration::from_secs(2), "{took:?}");
    assert_eq!(vault.items(Scope::Trashed).len(), 1);
    // The sealed manifest is one more record in the store.
    assert_eq!(vault.store().list().unwrap().len(), 10_001);
}

#[test]
fn search_over_five_thousand_items_meets_the_50ms_target() {
    let db = Db::new("5k");
    let vault = build(&db, 5_000);
    let queries = [
        "site04",
        "user1234",
        "group3",
        "example.com",
        "site4999 example",
        "nomatchatall",
    ];
    let mut worst = Duration::ZERO;
    for q in queries {
        let (_, took) = timed(&format!("search {q:?} over 5,000 items"), || {
            vault.search(q).len()
        });
        worst = worst.max(took);
    }
    // The target is 50 ms. Measured here it is a few milliseconds, so asserting the real target leaves a wide
    // margin for slower machines while still catching an accidental slowdown of an order of magnitude.
    assert!(
        worst < Duration::from_millis(50),
        "slowest search took {worst:?}"
    );
}

#[test]
fn listing_metadata_does_not_read_ciphertext() {
    let db = Db::new("meta");
    let vault = build(&db, 2_000);
    let (metas, took) = timed("store.list() for 2,000 records", || {
        vault.store().list().unwrap()
    });
    assert_eq!(metas.len(), 2_001, "2,000 items and the manifest");
    let (all, took_all) = timed("store.all() for 2,000 records", || {
        vault.store().all().unwrap()
    });
    assert_eq!(all.len(), 2_001);
    assert!(
        took <= took_all * 2,
        "list() should be no slower than all(): {took:?} vs {took_all:?}"
    );
}
