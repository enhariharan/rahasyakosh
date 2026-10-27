//! The Secret Key (audit finding P1-3): a second, random secret that the vault file does not contain.
use rahasyakosh_core::crypto::KdfParams;
use rahasyakosh_core::secret_key::SecretKey;
use rahasyakosh_core::vault::UnlockedVault;
use rahasyakosh_core::Error;
use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};

const KDF: KdfParams = KdfParams {
    m_kib: KdfParams::MIN_M_KIB,
    t: KdfParams::MIN_T,
    p: 1,
};
const PW: &str = "a weak master password";

fn rng() -> ChaCha20Rng {
    ChaCha20Rng::seed_from_u64(31)
}

#[test]
fn the_vault_file_alone_is_not_enough() {
    let (v, sk) = UnlockedVault::create_with_secret_key(PW, KDF, &mut rng()).unwrap();
    let h = v.header().clone();
    assert!(h.secret_key);
    assert!(matches!(h.unlock(PW), Err(Error::SecretKeyRequired)));
    // The right master password with the wrong Secret Key is just a failed decryption.
    let other = SecretKey::generate(&mut ChaCha20Rng::seed_from_u64(32));
    assert!(matches!(
        h.unlock_with(PW, Some(&other)),
        Err(Error::Decrypt)
    ));
    assert!(matches!(
        h.unlock_with("not it", Some(&sk)),
        Err(Error::Decrypt)
    ));
    h.unlock_with(PW, Some(&sk)).expect("both together open it");
}

#[test]
fn the_secret_key_is_not_in_the_header() {
    let (v, sk) = UnlockedVault::create_with_secret_key(PW, KDF, &mut rng()).unwrap();
    let json = serde_json::to_string(v.header()).unwrap();
    let text = sk.to_text();
    let body: String = text.chars().filter(|c| *c != '-').skip(3).collect();
    assert!(!json.contains(&*body));
    assert!(json.contains("\"secret_key\":true"));
}

#[test]
fn a_vault_without_one_is_unchanged_and_ignores_a_key_given_anyway() {
    let v = UnlockedVault::create(PW, KDF, &mut rng()).unwrap();
    assert!(!serde_json::to_string(v.header())
        .unwrap()
        .contains("secret_key"));
    let sk = SecretKey::generate(&mut rng());
    v.header().unlock_with(PW, Some(&sk)).unwrap();
}

#[test]
fn flag_is_authenticated() {
    let (v, sk) = UnlockedVault::create_with_secret_key(PW, KDF, &mut rng()).unwrap();
    let mut h = v.header().clone();
    h.secret_key = false;
    assert!(h.unlock_with(PW, Some(&sk)).is_err());
}

#[test]
fn it_can_be_added_to_an_existing_vault_and_removed_again() {
    let mut v = UnlockedVault::create(PW, KDF, &mut rng()).unwrap();
    let sk = v.set_secret_key(PW, &mut rng()).unwrap();
    assert!(matches!(
        v.header().unlock(PW),
        Err(Error::SecretKeyRequired)
    ));
    v.header().unlock_with(PW, Some(&sk)).unwrap();
    v.clear_secret_key(PW, &mut rng()).unwrap();
    v.header().unlock(PW).unwrap();
}

#[test]
fn changing_the_password_keeps_the_secret_key() {
    let (mut v, sk) = UnlockedVault::create_with_secret_key(PW, KDF, &mut rng()).unwrap();
    let h = v.header().clone();
    let mut opened = h.unlock_with(PW, Some(&sk)).unwrap();
    opened
        .change_password("another password", KDF, &mut rng())
        .unwrap();
    let h2 = opened.header().clone();
    assert!(h2.secret_key);
    h2.unlock_with("another password", Some(&sk)).unwrap();
    assert!(h2.unlock_with(PW, Some(&sk)).is_err());
    let _ = &mut v;
}

#[test]
fn the_text_form_round_trips_and_catches_typing_mistakes() {
    let sk = SecretKey::generate(&mut rng());
    let text = sk.to_text();
    assert!(text.starts_with("RK1-"));
    assert_eq!(text.len(), 3 + 8 * 5, "RK1 and eight groups of four");
    assert_eq!(SecretKey::parse(&text).unwrap(), sk);
    // Case, spaces and a missing prefix do not matter.
    let loose = text.to_lowercase().replace('-', " ");
    assert_eq!(SecretKey::parse(&loose).unwrap(), sk);
    assert_eq!(SecretKey::parse(&text[4..]).unwrap(), sk);
    // One wrong character is caught by the checksum.
    let mut chars: Vec<char> = text.chars().collect();
    let i = chars.len() - 1;
    chars[i] = if chars[i] == 'A' { 'B' } else { 'A' };
    let bad: String = chars.into_iter().collect();
    assert!(SecretKey::parse(&bad).is_err());
    assert!(SecretKey::parse("RK1-ABCD").is_err());
    assert!(SecretKey::parse("").is_err());
    assert_eq!(format!("{sk:?}"), "SecretKey(<redacted>)");
}

/// Golden vector: the text format must not change, or printed recovery sheets stop working.
#[test]
fn a_known_secret_key_text_stays_valid() {
    const TEXT: &str = "RK1-TI3U-IUCF-MBRZ-5RTQ-W6QX-2SJL-E4IN-TCKT";
    let sk = SecretKey::generate(&mut ChaCha20Rng::seed_from_u64(1));
    assert_eq!(sk.to_text().as_str(), TEXT);
    assert_eq!(SecretKey::parse(TEXT).unwrap(), sk);
}
