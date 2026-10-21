//! Property tests: rules that must hold for *any* input, checked on many generated inputs.
//! (Known answers are in `src/crypto.rs`; the golden vault is in `tests/golden.rs`.)
//!
//! Native only: `proptest` does not build for WASI.
#![cfg(not(target_family = "wasm"))]

use proptest::prelude::*;
use rahasyakosh_core::crypto::{self, KdfParams, Sealed, NONCE_LEN};
use rahasyakosh_core::generator::{self, PasswordSpec};
use rahasyakosh_core::item::{Field, Item, ItemKind};
use rahasyakosh_core::totp::{Algorithm, Totp};
use rahasyakosh_core::vault::{Record, UnlockedVault, VaultHeader};
use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};
use std::sync::OnceLock;
use uuid::Uuid;
use zeroize::Zeroizing;

/// Cheapest parameters the library accepts when reading a header (an integration test is not `cfg(test)`).
const FAST_KDF: KdfParams = KdfParams {
    m_kib: KdfParams::MIN_M_KIB,
    t: KdfParams::MIN_T,
    p: 1,
};
const PW: &str = "property-test password";

fn rng(seed: u64) -> ChaCha20Rng {
    ChaCha20Rng::seed_from_u64(seed)
}

/// One vault shared by all cases, because deriving keys is deliberately slow.
fn vault() -> &'static UnlockedVault {
    static V: OnceLock<UnlockedVault> = OnceLock::new();
    V.get_or_init(|| UnlockedVault::create(PW, FAST_KDF, &mut rng(0)).unwrap())
}

/// Any string, including control characters, quotes, backslashes and non-BMP characters.
fn text() -> impl Strategy<Value = String> {
    "(?s).{0,40}"
}

fn key(bytes: [u8; 32]) -> crypto::Key {
    Zeroizing::new(bytes)
}

fn flip(bytes: &mut [u8], bit: usize) {
    let i = bit / 8 % bytes.len();
    bytes[i] ^= 1 << (bit % 8);
}

fn item_strategy() -> impl Strategy<Value = Item> {
    (
        any::<u128>(),
        0u8..4,
        text(),
        prop::collection::vec((text(), text(), any::<bool>()), 0..6),
        prop::collection::vec(text(), 0..4),
        prop::collection::vec(text(), 0..4),
        text(),
        any::<bool>(),
        any::<i64>(),
        any::<i64>(),
        prop::option::of(any::<i64>()),
    )
        .prop_map(
            |(id, kind, title, fields, urls, tags, notes, fav, created, updated, deleted)| {
                let kind = [
                    ItemKind::Login,
                    ItemKind::Note,
                    ItemKind::Card,
                    ItemKind::Identity,
                ][kind as usize];
                let mut i = Item::new(kind, &title, created, Uuid::from_u128(id));
                i.fields = fields
                    .into_iter()
                    .map(|(name, value, concealed)| Field {
                        name,
                        value,
                        concealed,
                    })
                    .collect();
                i.urls = urls;
                i.tags = tags;
                i.notes = notes;
                i.favourite = fav;
                i.updated_at = updated;
                i.deleted_at = deleted;
                i
            },
        )
}

proptest! {
    // ---- AEAD: encrypt -> decrypt is the identity; any change is rejected -------------------------

    #[test]
    fn seal_then_open_is_identity(
        k in any::<[u8; 32]>(),
        pt in prop::collection::vec(any::<u8>(), 0..2048),
        aad in prop::collection::vec(any::<u8>(), 0..64),
        seed in any::<u64>(),
    ) {
        let sealed = crypto::seal(&key(k), &pt, &aad, &mut rng(seed));
        prop_assert_eq!(sealed.ct.len(), pt.len() + 16);
        let opened = crypto::open(&key(k), &sealed, &aad).unwrap();
        prop_assert_eq!(&**opened, pt.as_slice());
    }

    #[test]
    fn any_single_bit_flip_in_ciphertext_or_nonce_is_rejected(
        k in any::<[u8; 32]>(),
        pt in prop::collection::vec(any::<u8>(), 0..256),
        aad in prop::collection::vec(any::<u8>(), 0..32),
        seed in any::<u64>(),
        bit in any::<usize>(),
        in_nonce in any::<bool>(),
    ) {
        let mut sealed = crypto::seal(&key(k), &pt, &aad, &mut rng(seed));
        if in_nonce {
            flip(&mut sealed.nonce, bit);
        } else {
            flip(&mut sealed.ct, bit);
        }
        prop_assert!(crypto::open(&key(k), &sealed, &aad).is_err());
    }

    #[test]
    fn wrong_key_wrong_aad_or_resized_ciphertext_is_rejected(
        k in any::<[u8; 32]>(),
        other in any::<[u8; 32]>(),
        pt in prop::collection::vec(any::<u8>(), 1..256),
        aad in prop::collection::vec(any::<u8>(), 0..32),
        extra in any::<u8>(),
        seed in any::<u64>(),
    ) {
        prop_assume!(k != other);
        let sealed = crypto::seal(&key(k), &pt, &aad, &mut rng(seed));
        prop_assert!(crypto::open(&key(other), &sealed, &aad).is_err());

        let mut other_aad = aad.clone();
        other_aad.push(extra);
        prop_assert!(crypto::open(&key(k), &sealed, &other_aad).is_err());

        let mut longer = sealed.clone();
        longer.ct.push(extra);
        prop_assert!(crypto::open(&key(k), &longer, &aad).is_err());
        let mut shorter = sealed;
        shorter.ct.pop();
        prop_assert!(crypto::open(&key(k), &shorter, &aad).is_err());
    }

    #[test]
    fn fresh_nonce_every_time(k in any::<[u8; 32]>(), seed in any::<u64>()) {
        let mut r = rng(seed);
        let a = crypto::seal(&key(k), b"same", b"", &mut r);
        let b = crypto::seal(&key(k), b"same", b"", &mut r);
        prop_assert_ne!(a.nonce, b.nonce);
        prop_assert_ne!(a.ct, b.ct);
    }

    #[test]
    fn hkdf_is_deterministic_and_separates_labels(
        ikm in prop::collection::vec(any::<u8>(), 1..64),
        a in prop::collection::vec(any::<u8>(), 0..32),
        b in prop::collection::vec(any::<u8>(), 0..32),
    ) {
        prop_assert_eq!(crypto::hkdf_expand(&ikm, None, &a), crypto::hkdf_expand(&ikm, None, &a));
        prop_assume!(a != b);
        prop_assert_ne!(crypto::hkdf_expand(&ikm, None, &a), crypto::hkdf_expand(&ikm, None, &b));
    }

    // ---- Items and records ---------------------------------------------------------------------

    #[test]
    fn item_survives_seal_and_open(item in item_strategy(), rev in any::<u64>(), seed in any::<u64>()) {
        let rec = vault().seal_item(&item, rev, &mut rng(seed)).unwrap();
        prop_assert_eq!(rec.item_id, item.id);
        prop_assert_eq!(rec.rev, rev);
        // Padded to 64-byte buckets, plus the 16-byte tag: lengths only reveal the bucket.
        prop_assert_eq!((rec.ct.len() - 16) % 64, 0);
        prop_assert_eq!(vault().open_record(&rec).unwrap(), item);
    }

    #[test]
    fn record_serialisation_round_trips(item in item_strategy(), rev in any::<u64>(), seed in any::<u64>()) {
        let rec = vault().seal_item(&item, rev, &mut rng(seed)).unwrap();
        let json = serde_json::to_string(&rec).unwrap();
        let back: Record = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&back, &rec);
        prop_assert_eq!(vault().open_record(&back).unwrap(), item);
    }

    #[test]
    fn any_tampering_with_a_record_is_rejected(
        item in item_strategy(),
        seed in any::<u64>(),
        bit in any::<usize>(),
        what in 0u8..5,
        other_id in any::<u128>(),
        other_rev in any::<u64>(),
    ) {
        let rec = vault().seal_item(&item, 7, &mut rng(seed)).unwrap();
        let mut bad = rec.clone();
        match what {
            0 => flip(&mut bad.ct, bit),
            1 => flip(&mut bad.nonce, bit),
            2 => {
                prop_assume!(Uuid::from_u128(other_id) != rec.item_id);
                bad.item_id = Uuid::from_u128(other_id);
            }
            3 => {
                prop_assume!(other_rev != 7);
                bad.rev = other_rev;
            }
            _ => bad.record_version = bad.record_version.wrapping_add(1),
        }
        prop_assert!(vault().open_record(&bad).is_err());
    }

    #[test]
    fn a_record_does_not_open_as_another_items_record(a in item_strategy(), b in item_strategy(), seed in any::<u64>()) {
        prop_assume!(a.id != b.id);
        let ra = vault().seal_item(&a, 1, &mut rng(seed)).unwrap();
        let mut swapped = ra.clone();
        swapped.item_id = b.id;
        prop_assert!(vault().open_record(&swapped).is_err());
    }
}

// Opening a header runs Argon2 each time, so these use fewer cases.
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn any_single_bit_flip_in_the_header_is_rejected(field in 0u8..7, bit in any::<usize>()) {
        let mut h: VaultHeader = vault().header().clone();
        match field {
            0 => h.format_version ^= 1 << (bit % 16),
            1 => {
                let mut id = *h.vault_id.as_bytes();
                flip(&mut id, bit);
                h.vault_id = Uuid::from_bytes(id);
            }
            2 => flip(&mut h.salt, bit),
            3 => flip(&mut h.wrapped_vk.nonce, bit),
            4 => flip(&mut h.wrapped_vk.ct, bit),
            // Only low bits of the cost parameters, so a flip cannot ask for gigabytes of memory.
            5 => h.kdf.t ^= 1 << (bit % 3),
            _ => h.kdf.m_kib ^= 1 << (bit % 8),
        }
        prop_assert!(h.unlock(PW).is_err());
    }

    #[test]
    fn header_serialisation_round_trips(seed in any::<u64>()) {
        let v = UnlockedVault::create(PW, FAST_KDF, &mut rng(seed)).unwrap();
        let json = serde_json::to_string(v.header()).unwrap();
        let back: VaultHeader = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&back, v.header());
        prop_assert!(back.unlock(PW).is_ok());
    }

    #[test]
    fn only_the_right_password_unlocks(
        password in "(?s).{1,24}",
        other in "(?s).{1,24}",
        seed in any::<u64>(),
    ) {
        prop_assume!(password != other);
        let v = UnlockedVault::create(&password, FAST_KDF, &mut rng(seed)).unwrap();
        prop_assert!(v.header().unlock(&password).is_ok());
        prop_assert!(v.header().unlock(&other).is_err());
    }

    #[test]
    fn changing_the_password_keeps_items_and_revokes_the_old_one(
        item in item_strategy(),
        old in "(?s).{1,24}",
        new in "(?s).{1,24}",
        seed in any::<u64>(),
    ) {
        prop_assume!(old != new);
        let mut r = rng(seed);
        let mut v = UnlockedVault::create(&old, FAST_KDF, &mut r).unwrap();
        let rec = v.seal_item(&item, 1, &mut r).unwrap();
        v.change_password(&new, FAST_KDF, &mut r).unwrap();
        prop_assert!(v.header().unlock(&old).is_err());
        let again = v.header().unlock(&new).unwrap();
        prop_assert_eq!(again.open_record(&rec).unwrap(), item);
    }
}

// ---- Generator ----------------------------------------------------------------------------------

proptest! {
    #[test]
    fn uniform_stays_in_range(n in 1usize..1_000_000, seed in any::<u64>()) {
        let mut r = rng(seed);
        for _ in 0..16 {
            prop_assert!(generator::uniform(&mut r, n) < n);
        }
    }

    #[test]
    fn passwords_match_their_spec(
        length in 4usize..128,
        lower in any::<bool>(),
        upper in any::<bool>(),
        digits in any::<bool>(),
        symbols in any::<bool>(),
        avoid_ambiguous in any::<bool>(),
        seed in any::<u64>(),
    ) {
        let spec = PasswordSpec { length, lower, upper, digits, symbols, avoid_ambiguous };
        let classes = [lower, upper, digits, symbols].iter().filter(|c| **c).count();
        let result = generator::generate_password(&spec, &mut rng(seed));
        if classes == 0 {
            prop_assert!(result.is_err());
        } else {
            let p = result.unwrap();
            prop_assert_eq!(p.chars().count(), length);
            let ambiguous = |c: char| "O0Il1|".contains(c);
            let is_symbol = |c: char| !c.is_ascii_alphanumeric();
            if lower { prop_assert!(p.chars().any(|c| c.is_ascii_lowercase() && !(avoid_ambiguous && ambiguous(c)))); }
            if upper { prop_assert!(p.chars().any(|c| c.is_ascii_uppercase() && !(avoid_ambiguous && ambiguous(c)))); }
            if digits { prop_assert!(p.chars().any(|c| c.is_ascii_digit() && !(avoid_ambiguous && ambiguous(c)))); }
            if symbols { prop_assert!(p.chars().any(is_symbol)); }
            if !lower { prop_assert!(!p.chars().any(|c| c.is_ascii_lowercase())); }
            if !upper { prop_assert!(!p.chars().any(|c| c.is_ascii_uppercase())); }
            if !digits { prop_assert!(!p.chars().any(|c| c.is_ascii_digit())); }
            if !symbols { prop_assert!(!p.chars().any(is_symbol)); }
            if avoid_ambiguous { prop_assert!(!p.chars().any(ambiguous)); }
        }
    }

    #[test]
    fn passphrases_have_the_requested_shape(
        words in 3usize..=32,
        sep in "[-. _]{0,2}",
        capitalize in any::<bool>(),
        seed in any::<u64>(),
    ) {
        let list = rahasyakosh_core::wordlist::eff_large();
        let p = generator::generate_passphrase(&list, words, &sep, capitalize, &mut rng(seed)).unwrap();
        // Some EFF words contain '-', so count by rebuilding rather than splitting when sep == "-".
        let lower = p.to_lowercase();
        let mut rest = lower.as_str();
        for i in 0..words {
            let w = list.iter().find(|w| rest.starts_with(**w)).copied();
            prop_assert!(w.is_some(), "word {i} not in list: {rest}");
            rest = &rest[w.unwrap().len()..];
            if i + 1 < words { rest = rest.strip_prefix(sep.as_str()).unwrap_or(rest); }
        }
        prop_assert!(rest.is_empty());
        if capitalize { prop_assert!(p.chars().next().unwrap().is_uppercase() || !p.chars().next().unwrap().is_alphabetic()); }
    }

    #[test]
    fn passphrase_rejects_bad_word_counts(words in prop_oneof![0usize..3, 33usize..100], seed in any::<u64>()) {
        let list = rahasyakosh_core::wordlist::eff_large();
        prop_assert!(generator::generate_passphrase(&list, words, "-", false, &mut rng(seed)).is_err());
    }
}

// ---- TOTP ---------------------------------------------------------------------------------------

proptest! {
    #[test]
    fn totp_codes_have_the_right_shape_and_are_stable_within_a_period(
        secret in prop::collection::vec(any::<u8>(), 1..64),
        algo in 0u8..3,
        digits in 6u32..=8,
        period in prop_oneof![Just(30u64), Just(60u64)],
        step in 0u64..1_000_000,
        a in 0u64..30,
        b in 0u64..30,
    ) {
        let algorithm = [Algorithm::Sha1, Algorithm::Sha256, Algorithm::Sha512][algo as usize];
        let t = Totp::new(secret, algorithm, digits, period).unwrap();
        let base = step * period;
        let code = t.code_at(base + a);
        prop_assert_eq!(code.len(), digits as usize);
        prop_assert!(code.chars().all(|c| c.is_ascii_digit()));
        prop_assert_eq!(&code, &t.code_at(base + b));
        prop_assert_eq!(t.seconds_remaining(base + a), period - a);
    }

    #[test]
    fn totp_rejects_bad_parameters(digits in prop_oneof![0u32..6, 9u32..20], secret in prop::collection::vec(any::<u8>(), 1..8)) {
        prop_assert!(Totp::new(secret, Algorithm::Sha1, digits, 30).is_err());
    }

    #[test]
    fn totp_parsers_never_panic(s in "(?s).{0,80}") {
        let _ = Totp::from_base32(&s);
        let _ = Totp::from_uri(&s);
        let _ = Totp::from_uri(&format!("otpauth://totp/x?secret={s}"));
    }
}

#[test]
fn nonce_length_is_what_the_format_documents() {
    assert_eq!(NONCE_LEN, 24);
    let _ = Sealed {
        nonce: [0; NONCE_LEN],
        ct: vec![],
    };
}
