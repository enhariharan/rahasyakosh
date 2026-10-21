//! Robustness checks for every place the programs read untrusted bytes.
//!
//! Each check takes arbitrary bytes and must return normally: no panic, no hang, no runaway memory. Where a stronger
//! property exists it is asserted too (a forged record must never open; a secret must never appear in `Debug` output;
//! parsing then writing then parsing again must give the same value).
//!
//! The same functions are driven two ways:
//! - by `cargo fuzz` (see `fuzz/` and the book chapter on fuzzing), which searches for inputs that break them, and
//! - by ordinary tests (`tests/replay.rs`), which replay the committed seed corpus and every past crash on stable Rust.
use rahasyakosh_core::crypto::KdfParams;
use rahasyakosh_core::item::Item;
use rahasyakosh_core::totp::Totp;
use rahasyakosh_core::vault::{Record, UnlockedVault, VaultHeader};
use rahasyakosh_store::conformance::{sample_header, sample_record};
use rahasyakosh_store::{DirectoryStore, MemoryStore, SqliteStore, Store, Uuid};
use rahasyakosh_transfer::{export_csv, import_csv, Bundle};
use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

/// A robustness check: takes arbitrary bytes, must return normally.
pub type Check = fn(&[u8]);

/// Every check, by the name used for its fuzz target and its corpus directory.
pub const CHECKS: &[(&str, Check)] = &[
    ("vault_header", vault_header),
    ("record", record),
    ("payload", payload),
    ("item_json", item_json),
    ("csv_import", csv_import),
    ("totp", totp),
    ("bundle", bundle),
    ("directory_store", directory_store),
    ("sqlite_store", sqlite_store),
    ("strength", strength),
];

fn rng(seed: &[u8]) -> ChaCha20Rng {
    let mut s = 0u64;
    for b in seed.iter().take(64) {
        s = s.rotate_left(5) ^ u64::from(*b);
    }
    ChaCha20Rng::seed_from_u64(s)
}

/// One vault with the cheapest key settings the library accepts, shared by all checks (deriving keys is slow on purpose).
fn vault() -> &'static UnlockedVault {
    static V: OnceLock<UnlockedVault> = OnceLock::new();
    V.get_or_init(|| {
        let kdf = KdfParams {
            m_kib: KdfParams::MIN_M_KIB,
            t: KdfParams::MIN_T,
            p: 1,
        };
        UnlockedVault::create("fuzz", kdf, &mut ChaCha20Rng::seed_from_u64(1)).unwrap()
    })
}

fn temp_path(kind: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "rk-fuzz-{kind}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ))
}

/// A header read from untrusted bytes. Unlocking one must be safe too, but only the cheap ones are tried: the library's
/// upper bounds on key-derivation cost still allow a hostile file to ask for a gigabyte and 20 passes.
pub fn vault_header(data: &[u8]) {
    let Ok(h) = serde_json::from_slice::<VaultHeader>(data) else {
        return;
    };
    let again: VaultHeader = serde_json::from_slice(&serde_json::to_vec(&h).unwrap()).unwrap();
    assert_eq!(again, h, "a header must survive a write and a read");
    let _ = h.kdf.validate();
    if h.kdf.m_kib <= KdfParams::MIN_M_KIB + 1024 && h.kdf.t <= 3 && h.kdf.p <= 2 {
        let _ = h.unlock("fuzz"); // almost certainly an error; it must not panic
    }
}

/// A record from untrusted bytes, as JSON and as raw fields. Nobody can forge one, so none may open.
pub fn record(data: &[u8]) {
    if let Ok(r) = serde_json::from_slice::<Record>(data) {
        let again: Record = serde_json::from_slice(&serde_json::to_vec(&r).unwrap()).unwrap();
        assert_eq!(again, r);
        assert!(vault().open_record(&r).is_err(), "a made-up record opened");
    }
    if data.len() >= 48 {
        let r = Record {
            record_version: 1,
            item_id: Uuid::from_slice(&data[24..40]).unwrap(),
            rev: u64::from_le_bytes(data[40..48].try_into().unwrap()),
            nonce: data[..24].try_into().unwrap(),
            ct: data[48..].to_vec(),
        };
        assert!(vault().open_record(&r).is_err(), "a forged record opened");
    }
}

/// The bytes inside a record that did authenticate: `unpad` and the item JSON must cope with anything, because a buggy or
/// newer program could have written it.
pub fn payload(data: &[u8]) {
    let id = Uuid::from_u128(0x77);
    let rec = vault().seal_raw_payload_for_fuzzing(id, 1, data, &mut rng(data));
    if let Ok(item) = vault().open_record(&rec) {
        assert_eq!(item.id, id);
        let _ = serde_json::to_vec(&item).unwrap();
    }
}

/// Item JSON: stable under write and read, safe to operate on, and its `Debug` output never shows a concealed value.
pub fn item_json(data: &[u8]) {
    let Ok(item) = serde_json::from_slice::<Item>(data) else {
        return;
    };
    let again: Item = serde_json::from_slice(&serde_json::to_vec(&item).unwrap()).unwrap();
    assert_eq!(again, item);

    let _ = item.matches("a");
    let _ = item.field("password");
    let mut work = item.clone();
    work.push_history(1, 3);
    let _ = work.revert_to(0, 2, 3);
    work.set_field("x", "y", true);
    work.remove_field("x");

    // `Debug` escapes quotes and backslashes, so compare escaped forms: a leaked secret would appear escaped, and a
    // harmless coincidence can only be with the escaped form of something that is shown anyway.
    let esc = |s: &str| {
        let d = format!("{s:?}");
        d[1..d.len() - 1].to_string()
    };
    let shown = format!("{item:?}{:?}", item.history);
    let id_text = item.id.to_string();
    let mut visible: Vec<String> = vec![esc(&item.title), esc(&id_text)];
    visible.extend(item.fields.iter().map(|f| esc(&f.name)));
    visible.extend(
        item.fields
            .iter()
            .filter(|f| !f.concealed)
            .map(|f| esc(&f.value)),
    );
    // The fixed words of the `Debug` format itself.
    let words = [
        "Item", "Field", "REDACTED", "name", "value", "fields", "kind", "title", "id", "Login",
        "Note",
    ];
    for f in item
        .fields
        .iter()
        .filter(|f| f.concealed && f.value.len() >= 8)
    {
        let secret = esc(&f.value);
        let explained = visible.iter().any(|v| v.contains(&secret))
            || words.iter().any(|w| w.contains(&secret));
        if !explained {
            assert!(
                !shown.contains(&secret),
                "a concealed value appeared in Debug output: field {:?} ({} bytes)",
                f.name,
                f.value.len()
            );
        }
    }
}

/// CSV import: any bytes, with and without a column mapping. What comes back must itself export and re-import.
pub fn csv_import(data: &[u8]) {
    let mut n = 0u128;
    let mut ids = || {
        n += 1;
        Uuid::from_u128(n)
    };
    let _ = import_csv(data, &[], &[], 0, &mut ids);
    // Also exercise explicit column mapping, using the file's own first header names.
    let first_line: &[u8] = data.split(|b| *b == b'\n').next().unwrap_or(&[]);
    let names: Vec<String> = String::from_utf8_lossy(first_line)
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .take(4)
        .collect();
    let fields = ["password", "title", "username", "url"];
    let map: Vec<(String, String)> = fields
        .iter()
        .zip(&names)
        .map(|(f, n)| (f.to_string(), n.clone()))
        .collect();
    if let Ok(report) = import_csv(data, &map, &[], 0, &mut ids) {
        assert!(
            report.items.iter().all(|i| !i.title.is_empty()),
            "an imported item has no title"
        );
        if let Ok(csv) = export_csv(&report.items) {
            import_csv(csv.as_bytes(), &[], &[], 0, &mut ids)
                .expect("our own export must be readable");
        }
    }
}

/// TOTP secrets and `otpauth://` links, then computing codes from whatever parsed.
pub fn totp(data: &[u8]) {
    let s = String::from_utf8_lossy(data);
    let time = data
        .get(..8)
        .map_or(0, |b| u64::from_le_bytes(b.try_into().unwrap()));
    for t in [Totp::from_base32(&s), Totp::from_uri(&s)]
        .into_iter()
        .flatten()
    {
        let code = t.code_at(time);
        assert_eq!(code.len(), t.digits as usize);
        assert!(t.seconds_remaining(time) >= 1);
    }
}

/// The vault bundle (JSON file) read from anywhere.
pub fn bundle(data: &[u8]) {
    let Ok(b) = Bundle::from_slice(data) else {
        return;
    };
    let again = Bundle::from_slice(&b.to_vec().unwrap()).unwrap();
    assert_eq!(again, b);
    let mut store = MemoryStore::new();
    if store.create(&b.header).is_ok() {
        let _ = store.put_many(&b.records);
        let _ = (store.list(), store.all(), store.tombstones());
    }
}

/// A directory store whose files hold arbitrary bytes: every operation returns a value or an error.
pub fn directory_store(data: &[u8]) {
    let dir = temp_path("dir");
    let id = Uuid::from_u128(1);
    {
        let mut s = DirectoryStore::at(&dir);
        s.create(&sample_header(1, 7)).unwrap();
        let (sel, rest) = data.split_first().map_or((0, &[][..]), |(a, b)| (*a, b));
        let half = rest.len() / 2;
        let put = |name: String, bytes: &[u8]| {
            let _ = std::fs::write(dir.join(name), bytes);
        };
        match sel % 4 {
            0 => put("header.json".into(), rest),
            1 => put(format!("{id}.rkr"), rest),
            2 => put(format!("{id}.rkt"), rest),
            _ => {
                put(format!("{id}.rkr"), &rest[..half]);
                put(format!("{id}.rkt"), &rest[half..]);
            }
        }
        let _ = (s.header(), s.get(id), s.list(), s.all(), s.tombstones());
        let _ = s.put(&sample_record(1, 1_000, 64));
        let _ = s.purge(id, 2_000);
        let _ = (s.all(), s.tombstones());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// A SQLite file with arbitrary contents (or a damaged real one): opening and reading must never panic.
pub fn sqlite_store(data: &[u8]) {
    let path = temp_path("sqlite");
    if std::fs::write(&path, data).is_ok() {
        if let Ok(mut s) = SqliteStore::open(&path) {
            let id = Uuid::from_u128(1);
            let _ = (s.header(), s.get(id), s.list(), s.all(), s.tombstones());
            let _ = s.put(&sample_record(1, 1_000, 64));
            let _ = s.purge(id, 2_000);
        }
    }
    for ext in ["", "-wal", "-shm", "-journal"] {
        let mut p = path.clone().into_os_string();
        p.push(ext);
        let _ = std::fs::remove_file(p);
    }
}

/// The strength estimator on arbitrary text.
pub fn strength(data: &[u8]) {
    let s = String::from_utf8_lossy(data);
    let short: String = s.chars().take(200).collect();
    let r = rahasyakosh_strength::estimate(&short, &["alice", "github"]);
    assert!(r.score <= 4);
}
