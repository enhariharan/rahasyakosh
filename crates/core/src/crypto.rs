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
    seal_with_nonce(key, random_bytes::<NONCE_LEN>(rng), plaintext, aad)
}

/// Encrypt with a caller-chosen nonce. Reusing a nonce under one key breaks the cipher, so this is
/// private; it exists so known-answer tests can pin the exact output.
fn seal_with_nonce(key: &Key, nonce: [u8; NONCE_LEN], plaintext: &[u8], aad: &[u8]) -> Sealed {
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

/// Known-answer tests: published vectors and cross-checks against independent implementations.
/// If one of these fails, a dependency upgrade or a code change has altered the output of a primitive.
#[cfg(test)]
mod kat {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        let s: String = s.split_whitespace().collect();
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    fn to_hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    // RFC 5869 Appendix A. Our HKDF always outputs 32 bytes, which is a prefix of the RFC's 42-byte OKM.
    #[test]
    fn hkdf_rfc5869_case_1() {
        let okm = hkdf_expand(
            &[0x0b; 22],
            Some(&hex("000102030405060708090a0b0c")),
            &hex("f0f1f2f3f4f5f6f7f8f9"),
        );
        assert_eq!(
            to_hex(&*okm),
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf"
        );
    }

    #[test]
    fn hkdf_rfc5869_case_3_empty_salt_and_info() {
        let okm = hkdf_expand(&[0x0b; 22], None, b"");
        assert_eq!(
            to_hex(&*okm),
            "8da4e775a563c18f715f802a063c5a31b8a11f5c5ee1879ec3454e5f3c738d2d"
        );
    }

    // draft-irtf-cfrg-xchacha-03, Appendix A.3.1.
    const XC_KEY: &str = "808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9f";
    const XC_NONCE: &str = "404142434445464748494a4b4c4d4e4f5051525354555657";
    const XC_AAD: &str = "50515253c0c1c2c3c4c5c6c7";
    const XC_PLAINTEXT: &str = "4c616469657320616e642047656e746c656d656e206f662074686520636c6173
        73206f66202739393a204966204920636f756c64206f6666657220796f75206f
        6e6c79206f6e652074697020666f7220746865206675747572652c2073756e73
        637265656e20776f756c642062652069742e";
    // Ciphertext followed by the 16-byte Poly1305 tag.
    const XC_SEALED: &str = "bd6d179d3e83d43b9576579493c0e939572a1700252bfaccbed2902c21396cbb
        731c7f1b0b4aa6440bf3a82f4eda7e39ae64c6708c54c216cb96b72e1213b452
        2f8c9ba40db5d945b11b69b982c1bb9e3f3fac2bc369488f76b2383565d3fff9
        21f9664c97637da9768812f615c68b13b52e
        c0875924c1c7987947deafd8780acf49";

    fn xc_key() -> Key {
        Zeroizing::new(hex(XC_KEY).try_into().unwrap())
    }

    #[test]
    fn xchacha20poly1305_draft_vector_seal() {
        let nonce: [u8; NONCE_LEN] = hex(XC_NONCE).try_into().unwrap();
        let sealed = seal_with_nonce(&xc_key(), nonce, &hex(XC_PLAINTEXT), &hex(XC_AAD));
        assert_eq!(sealed.nonce, nonce);
        assert_eq!(to_hex(&sealed.ct), to_hex(&hex(XC_SEALED)));
    }

    #[test]
    fn xchacha20poly1305_draft_vector_open() {
        let sealed = Sealed {
            nonce: hex(XC_NONCE).try_into().unwrap(),
            ct: hex(XC_SEALED),
        };
        let pt = open(&xc_key(), &sealed, &hex(XC_AAD)).unwrap();
        assert_eq!(&**pt, hex(XC_PLAINTEXT).as_slice());
        // Any change to the associated data or a ciphertext bit must be rejected.
        assert!(open(&xc_key(), &sealed, b"other").is_err());
        let mut bad = sealed.clone();
        bad.ct[0] ^= 1;
        assert!(open(&xc_key(), &bad, &hex(XC_AAD)).is_err());
    }

    // RFC 9106 section 5.3 uses a secret and associated data, which our wrapper does not expose, so this
    // checks the `argon2` crate itself (algorithm id, version 0x13, block layout) at that exact vector.
    #[test]
    fn argon2id_rfc9106_vector() {
        use argon2::{AssociatedData, ParamsBuilder};
        let params = ParamsBuilder::new()
            .m_cost(32)
            .t_cost(3)
            .p_cost(4)
            .output_len(32)
            .data(AssociatedData::new(&[4u8; 12]).unwrap())
            .build()
            .unwrap();
        let a = Argon2::new_with_secret(&[3u8; 8], Algorithm::Argon2id, Version::V0x13, params)
            .unwrap();
        let mut out = [0u8; 32];
        a.hash_password_into(&[1u8; 32], &[2u8; 16], &mut out)
            .unwrap();
        assert_eq!(
            to_hex(&out),
            "0d640df58d78766c08c037a34a8b53c9d01ef0452d75b65eb52520e96b01e659"
        );
    }

    // The next three were computed independently with OpenSSL 3.5 (`openssl kdf ... ARGON2ID`) and
    // Python `cryptography` 46, which agreed. They pin our parameter wiring: m, t, p and the 32-byte output.
    fn kdf(m_kib: u32, t: u32, p: u32, pw: &str, salt: &[u8]) -> String {
        to_hex(&*derive_master_key(pw.as_bytes(), salt, &KdfParams { m_kib, t, p }).unwrap())
    }

    #[test]
    fn argon2id_wrapper_small_params() {
        assert_eq!(
            kdf(32, 3, 4, "password", b"somesaltsomesalt"),
            "f25048ec48311a804ea9edd74e08c30765aa0f8d47c2a5b5a6097243cdf2e5ce"
        );
    }

    #[test]
    fn argon2id_wrapper_desktop_params() {
        let salt: Vec<u8> = (0u8..16).collect();
        assert_eq!(
            KdfParams::DESKTOP,
            KdfParams {
                m_kib: 65536,
                t: 3,
                p: 1
            }
        );
        assert_eq!(
            kdf(65536, 3, 1, "correct horse battery staple", &salt),
            "0d1a3c6523c8f06e4e0af9c515aa5b5448cfebd6838f2d52c3d8b6ef8ddc3c2e"
        );
    }

    #[test]
    fn argon2id_wrapper_minimum_params() {
        assert_eq!(KdfParams::MIN_M_KIB, 19456);
        assert_eq!(
            kdf(19456, 2, 2, "hunter2", b"0123456789abcdef"),
            "6fe14723e95014833794091a2f6963bec43b32af5b36bae0e1c79af22e4593f6"
        );
    }
}
