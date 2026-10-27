//! What the unlock screen is told before anything is typed (audit finding P1-2).
#![cfg(not(target_family = "wasm"))]

use rahasyakosh_app::{Config, Session, Settings, Vault};
use rahasyakosh_core::crypto::KdfParams;
use rahasyakosh_store::MemoryStore;
use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};

const PW: &str = "correct horse battery staple";
const KDF: KdfParams = KdfParams {
    m_kib: KdfParams::MIN_M_KIB,
    t: KdfParams::MIN_T,
    p: 1,
};

fn rng() -> ChaCha20Rng {
    ChaCha20Rng::seed_from_u64(10)
}

#[test]
fn the_unlock_screen_is_told_about_a_costly_vault_before_anything_is_typed() {
    let costly = KdfParams {
        t: KdfParams::DESKTOP.t * 2 + 1,
        ..KDF
    };
    let v = Vault::create(
        MemoryStore::new(),
        PW,
        costly,
        Config::default(),
        &mut rng(),
    )
    .unwrap();
    let s = Session::new(v.into_store(), Settings::default());
    assert_eq!(s.unlock_hints().costly_kdf, Some(costly));

    let v = Vault::create(MemoryStore::new(), PW, KDF, Config::default(), &mut rng()).unwrap();
    let s = Session::new(v.into_store(), Settings::default());
    assert_eq!(s.unlock_hints().costly_kdf, None);
}
