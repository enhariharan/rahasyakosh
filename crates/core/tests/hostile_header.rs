//! A header can be written by anyone who can write the vault file, and `unlock` reads its key-derivation cost
//! before anything is authenticated. These tests (audit finding P1-2) check that an absurd cost is refused at once
//! instead of being run. An integration test is compiled without `cfg(test)`, so the real validation is on.
use rahasyakosh_core::crypto::KdfParams;
use rahasyakosh_core::vault::UnlockedVault;
use rahasyakosh_core::Error;
use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};
use std::time::{Duration, Instant};

const FAST_KDF: KdfParams = KdfParams {
    m_kib: KdfParams::MIN_M_KIB,
    t: KdfParams::MIN_T,
    p: 1,
};

fn vault() -> UnlockedVault {
    UnlockedVault::create("pw", FAST_KDF, &mut ChaCha20Rng::seed_from_u64(1)).unwrap()
}

#[test]
fn ceiling_is_what_the_app_can_itself_produce() {
    // The desktop settings clamp to these same constants, so every vault the app makes opens.
    const { assert!(KdfParams::MAX_M_KIB <= 256 * 1024) };
    const { assert!(KdfParams::MAX_T <= 10) };
    assert!(KdfParams {
        m_kib: KdfParams::MAX_M_KIB,
        t: KdfParams::MAX_T,
        p: KdfParams::MAX_P
    }
    .validate()
    .is_ok());
}

#[test]
fn a_header_asking_for_one_gib_is_refused_without_running_argon2() {
    let mut h = vault().header().clone();
    h.kdf = KdfParams {
        m_kib: 1024 * 1024,
        t: 20,
        p: 1,
    };
    let start = Instant::now();
    let res = h.unlock("pw");
    assert!(matches!(res, Err(Error::KdfParams(_))), "should be refused");
    assert!(
        start.elapsed() < Duration::from_millis(200),
        "refusal took {:?}: the derivation must not run",
        start.elapsed()
    );
}

#[test]
fn each_limit_is_enforced_on_its_own() {
    let base = vault().header().clone();
    for kdf in [
        KdfParams {
            m_kib: KdfParams::MAX_M_KIB + 1,
            ..FAST_KDF
        },
        KdfParams {
            t: KdfParams::MAX_T + 1,
            ..FAST_KDF
        },
        KdfParams {
            p: KdfParams::MAX_P + 1,
            ..FAST_KDF
        },
        KdfParams { p: 0, ..FAST_KDF },
    ] {
        let mut h = base.clone();
        h.kdf = kdf;
        assert!(
            matches!(h.unlock("pw"), Err(Error::KdfParams(_))),
            "{kdf:?} must be refused"
        );
    }
}
