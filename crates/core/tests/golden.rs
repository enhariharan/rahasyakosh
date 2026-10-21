//! Compatibility contract: the committed golden vault in `spec/vectors/` must open with every future
//! version of the library. The files are embedded at compile time, so the same test runs unchanged
//! on native and WebAssembly targets. See `spec/vectors/README.md`.
use rahasyakosh_core::crypto::KdfParams;
use rahasyakosh_core::item::Item;
use rahasyakosh_core::vault::{Record, UnlockedVault, VaultHeader};
use rahasyakosh_core::Error;
use sha2::{Digest, Sha256};

const VAULT: &str = include_str!("../../../spec/vectors/golden-v1.rkv");
const EXPECTED: &str = include_str!("../../../spec/vectors/golden-v1.expected.json");
const PASSWORD: &str = "रहस्यकोश-golden-1";

#[derive(serde::Deserialize)]
struct VaultFile {
    header: VaultHeader,
    records: Vec<Record>,
}

fn sha256_hex(s: &str) -> String {
    Sha256::digest(s.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn load() -> VaultFile {
    serde_json::from_str(VAULT).expect("golden vault parses")
}

#[test]
fn golden_files_are_unmodified() {
    // If this fails you edited or regenerated a compatibility vector. Don't, unless the format version changed.
    assert_eq!(
        sha256_hex(VAULT),
        "657653e26eb87caaf5216e131c5adba60975cbdb3a6b03665440a78fcca433c7"
    );
    assert_eq!(
        sha256_hex(EXPECTED),
        "5c5d9c693a2e3005ddd01e8010986a9461bae98c947825305d33b9d7e8384a69"
    );
}

#[test]
fn header_is_as_documented() {
    let f = load();
    assert_eq!(f.header.format_version, 1);
    assert_eq!(f.header.kdf, KdfParams::DESKTOP);
    assert_eq!(f.records.len(), 5);
}

#[test]
fn golden_vault_opens_and_matches_expected_items() {
    let f = load();
    let vault = f.header.unlock(PASSWORD).expect("correct password unlocks");
    let expected: Vec<Item> = serde_json::from_str(EXPECTED).unwrap();
    let opened: Vec<Item> = f
        .records
        .iter()
        .map(|r| vault.open_record(r).expect("record opens"))
        .collect();
    assert_eq!(opened, expected);
    // Spot checks that the content is what the README says it is.
    assert!(opened.iter().any(|i| i.title == "बैंक (Bank)"));
    assert!(opened.iter().any(|i| i.deleted_at.is_some()));
    assert_eq!(
        f.records.iter().map(|r| r.rev).collect::<Vec<_>>(),
        [1, 1, 2, 1, 1]
    );
}

#[test]
fn wrong_password_is_rejected() {
    assert!(matches!(
        load().header.unlock("wrong password"),
        Err(Error::Decrypt)
    ));
}

#[test]
fn tampered_records_are_rejected() {
    let f = load();
    let vault = f.header.unlock(PASSWORD).unwrap();

    let mut flipped = f.records[0].clone();
    flipped.ct[0] ^= 1;
    assert!(vault.open_record(&flipped).is_err());

    // Swapping which item a ciphertext claims to belong to must fail (item id is bound as AAD).
    let mut swapped = f.records[0].clone();
    swapped.item_id = f.records[1].item_id;
    assert!(vault.open_record(&swapped).is_err());

    // Replaying a record under another revision must fail (revision is bound as AAD).
    let mut replay = f.records[2].clone();
    replay.rev = 1;
    assert!(vault.open_record(&replay).is_err());
}

#[test]
fn tampered_header_is_rejected() {
    let mut f = load();
    f.header.salt[0] ^= 1;
    assert!(f.header.unlock(PASSWORD).is_err());

    // Weakening the KDF parameters in the header is rejected before any key is derived.
    let mut f = load();
    f.header.kdf.m_kib = 8;
    assert!(f.header.unlock(PASSWORD).is_err());
}

#[test]
fn password_change_keeps_golden_items_readable() {
    use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};
    let f = load();
    let mut vault: UnlockedVault = f.header.unlock(PASSWORD).unwrap();
    let mut rng = ChaCha20Rng::seed_from_u64(1);
    vault
        .change_password("a different password", KdfParams::MOBILE, &mut rng)
        .unwrap();
    let again = vault.header().unlock("a different password").unwrap();
    for r in &f.records {
        assert!(again.open_record(r).is_ok());
    }
    assert!(vault.header().unlock(PASSWORD).is_err());
}
