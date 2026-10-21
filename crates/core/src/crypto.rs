//! Thin, misuse-resistant wrappers over RustCrypto primitives.
use crate::{Error, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use rand_core::CryptoRng;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::Zeroizing;

pub const KEY_LEN: usize = 32;
pub const NONCE_LEN: usize = 24;
pub const SALT_LEN: usize = 16;

/// 256-bit symmetric key, wiped on drop.
pub type Key = Zeroizing<[u8; KEY_LEN]>;

/// Argon2id parameters (stored in the vault header).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    pub m_kib: u32,
    pub t: u32,
    pub p: u32,
}

impl KdfParams {
    /// Lower bounds enforced when *reading* a header, to resist downgrade.
    pub const MIN_M_KIB: u32 = 19 * 1024;
    pub const MIN_T: u32 = 2;
    /// Upper bounds to avoid DoS from a hostile header.
    pub const MAX_M_KIB: u32 = 1024 * 1024;
    pub const MAX_T: u32 = 20;
    pub const MAX_P: u32 = 16;

    pub const DESKTOP: Self = Self {
        m_kib: 64 * 1024,
        t: 3,
        p: 1,
    };
    pub const MOBILE: Self = Self {
        m_kib: 32 * 1024,
        t: 3,
        p: 1,
    };
    /// Cheap parameters for unit tests only; rejected by `validate`.
    pub const INSECURE_TEST: Self = Self {
        m_kib: 8,
        t: 1,
        p: 1,
    };

    pub fn validate(&self) -> Result<()> {
        if self.m_kib < Self::MIN_M_KIB || self.t < Self::MIN_T {
            return Err(Error::KdfParams("below minimum strength".into()));
        }
        if self.m_kib > Self::MAX_M_KIB
            || self.t > Self::MAX_T
            || self.p == 0
            || self.p > Self::MAX_P
        {
            return Err(Error::KdfParams("above maximum".into()));
        }
        Ok(())
    }
}

pub fn random_bytes<const N: usize>(rng: &mut impl CryptoRng) -> [u8; N] {
    let mut b = [0u8; N];
    rng.fill_bytes(&mut b);
    b
}

pub fn random_key(rng: &mut impl CryptoRng) -> Key {
    Zeroizing::new(random_bytes::<KEY_LEN>(rng))
}

/// Argon2id: password (+ salt) -> master key.
pub fn derive_master_key(password: &[u8], salt: &[u8], params: &KdfParams) -> Result<Key> {
    let p = Params::new(params.m_kib, params.t, params.p, Some(KEY_LEN))
        .map_err(|e| Error::KdfParams(e.to_string()))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, p);
    let mut out = Zeroizing::new([0u8; KEY_LEN]);
    argon
        .hash_password_into(password, salt, &mut *out)
        .map_err(|e| Error::KdfParams(e.to_string()))?;
    Ok(out)
}

/// HKDF-SHA256 with domain-separating `info`.
pub fn hkdf_expand(ikm: &[u8], salt: Option<&[u8]>, info: &[u8]) -> Key {
    let hk = Hkdf::<Sha256>::new(salt, ikm);
    let mut out = Zeroizing::new([0u8; KEY_LEN]);
    hk.expand(info, &mut *out)
        .expect("32 bytes is a valid HKDF-SHA256 length");
    out
}

/// A sealed blob: random nonce + ciphertext (incl. 16-byte tag).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sealed {
    pub nonce: [u8; NONCE_LEN],
    pub ct: Vec<u8>,
}

pub fn seal(key: &Key, plaintext: &[u8], aad: &[u8], rng: &mut impl CryptoRng) -> Sealed {
    let nonce = random_bytes::<NONCE_LEN>(rng);
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_slice()).expect("32-byte key");
    let ct = cipher
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .expect("XChaCha20-Poly1305 encryption cannot fail for in-memory buffers");
    Sealed { nonce, ct }
}

pub fn open(key: &Key, sealed: &Sealed, aad: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    let cipher = XChaCha20Poly1305::new_from_slice(key.as_slice()).expect("32-byte key");
    cipher
        .decrypt(
            &XNonce::from(sealed.nonce),
            Payload {
                msg: &sealed.ct,
                aad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| Error::Decrypt)
}
