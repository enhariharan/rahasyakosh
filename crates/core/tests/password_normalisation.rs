//! The same visible master password must open the vault however it was typed (audit finding P1-1), and vaults made
//! before normalisation existed must still open. An integration test is compiled without `cfg(test)`, so the real
//! key-derivation checks run.
use rahasyakosh_core::crypto::KdfParams;
use rahasyakosh_core::vault::{PwNorm, UnlockedVault, VaultHeader};
use rahasyakosh_core::Error;
use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};

const KDF: KdfParams = KdfParams {
    m_kib: KdfParams::MIN_M_KIB,
    t: KdfParams::MIN_T,
    p: 1,
};

/// "café ऩ" with the accent as one character (U+00E9) and the Devanagari letter NNNA as one character (U+0929).
const COMPOSED: &str = "caf\u{e9} \u{929}";
/// The same text with `e` + combining acute (U+0301) and NA (U+0928) + nukta (U+093C).
const DECOMPOSED: &str = "cafe\u{301} \u{928}\u{93c}";

fn rng() -> ChaCha20Rng {
    ChaCha20Rng::seed_from_u64(2024)
}

#[test]
fn the_two_spellings_are_different_bytes() {
    assert_ne!(COMPOSED.as_bytes(), DECOMPOSED.as_bytes());
}

#[test]
fn a_new_vault_opens_with_either_spelling() {
    let v = UnlockedVault::create(COMPOSED, KDF, &mut rng()).unwrap();
    assert_eq!(v.header().pw_norm, Some(PwNorm::Nfkc));
    let h = v.header().clone();
    h.unlock(COMPOSED).expect("as created");
    h.unlock(DECOMPOSED).expect("typed another way");
    let e = h.unlock("cafe").err().expect("a different password");
    assert!(matches!(e, Error::Decrypt));
}

#[test]
fn compatibility_forms_are_folded_too() {
    // NFKC also folds "compatibility" look-alikes, such as a full-width letter or a ligature.
    let v = UnlockedVault::create("\u{ff21}bc\u{fb01}", KDF, &mut rng()).unwrap();
    v.header()
        .unlock("Abcfi")
        .expect("full-width A and the fi ligature");
}

#[test]
fn the_flag_is_part_of_what_is_authenticated() {
    let h = UnlockedVault::create(COMPOSED, KDF, &mut rng())
        .unwrap()
        .header()
        .clone();
    let mut stripped = h.clone();
    stripped.pw_norm = None;
    // Removing the flag changes the authenticated header, and the key would be derived differently.
    assert!(stripped.unlock(COMPOSED).is_err());
}

/// Written by the program before normalisation existed (password "café ऩ" with the decomposed Devanagari letter),
/// frozen here as a golden vector. It must keep opening for as long as the format is supported.
const OLD_HEADER: &str = r#"{"format_version":1,"vault_id":"66810fc8-75e6-ccb5-5342-6a8328b96f2d","kdf":{"m_kib":19456,"t":2,"p":1},"salt":"BoKK20+PEuThTidcgez01Q","wrapped_vk":{"nonce":"mKVkgn0+1L5ZvnjXs183jPH1vLqowSjT","ct":"d6ksJTViLGSAngtlfGxhCtH6Vd5Y2R7wXvjsiW7xkckq0oDdYToFdEDK+mfTnDAM"}}"#;

#[test]
fn a_vault_from_before_normalisation_still_opens() {
    let h: VaultHeader = serde_json::from_str(OLD_HEADER).unwrap();
    assert_eq!(h.pw_norm, None);
    // The bytes it was made with.
    h.unlock("caf\u{e9} \u{928}\u{93c}")
        .expect("typed as it was made");
    assert!(h.unlock("cafe").is_err());
}

#[test]
fn an_old_vault_made_from_one_standard_form_opens_with_the_other() {
    // Older vaults were keyed from the typed bytes. If those were composed, or decomposed, either spelling now works.
    let composed = UnlockedVault::create_with(COMPOSED, KDF, None, &mut rng()).unwrap();
    composed
        .header()
        .unlock(DECOMPOSED)
        .expect("made composed, typed decomposed");
    let decomposed = UnlockedVault::create_with(DECOMPOSED, KDF, None, &mut rng()).unwrap();
    decomposed
        .header()
        .unlock(COMPOSED)
        .expect("made decomposed, typed composed");
    // Known limit: a password made from a mixture of the two (like the golden vector) opens only with exactly
    // those bytes.
}

#[test]
fn an_old_vault_moves_to_the_new_form_when_the_password_is_changed() {
    let h: VaultHeader = serde_json::from_str(OLD_HEADER).unwrap();
    let mut v = h.unlock("caf\u{e9} \u{928}\u{93c}").unwrap();
    v.change_password(COMPOSED, KDF, &mut rng()).unwrap();
    assert_eq!(v.header().pw_norm, Some(PwNorm::Nfkc));
    let h = v.header().clone();
    h.unlock(COMPOSED).unwrap();
    h.unlock(DECOMPOSED).unwrap();
}

#[test]
fn a_plain_ascii_password_costs_one_attempt_in_an_old_vault() {
    // Not timed: the forms to try are the same text, so they are not repeated. This checks the result only.
    let v = UnlockedVault::create_with("hunter2 hunter2", KDF, None, &mut rng()).unwrap();
    assert_eq!(v.header().pw_norm, None);
    v.header().unlock("hunter2 hunter2").unwrap();
    assert!(v.header().unlock("hunter3 hunter3").is_err());
}

#[test]
fn the_header_json_has_the_flag_only_when_set() {
    let new = UnlockedVault::create("pw pw pw pw", KDF, &mut rng()).unwrap();
    assert!(serde_json::to_string(new.header())
        .unwrap()
        .contains("\"pw_norm\":\"nfkc\""));
    let old = UnlockedVault::create_with("pw pw pw pw", KDF, None, &mut rng()).unwrap();
    assert!(!serde_json::to_string(old.header())
        .unwrap()
        .contains("pw_norm"));
}
