//! The Secret Key: a second secret that is mixed into the vault's key (audit finding P1-3).
//!
//! A master password is chosen by a person and can be weak. The Secret Key is 128 random bits that the program
//! makes, so it cannot be guessed. It is **not** stored in the vault. It lives on the user's own devices and on a
//! printed recovery sheet. Someone who copies only the vault file therefore cannot even start guessing master
//! passwords: they would first have to guess the Secret Key too.
use crate::crypto;
use crate::{Error, Result};
use rand_core::CryptoRng;
use sha2::{Digest, Sha256};
use std::fmt;
use zeroize::Zeroizing;

const KEY_BYTES: usize = 16;
const CHECK_BYTES: usize = 4;
const PREFIX: &str = "RK1";
const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// 128 random bits. Wiped when dropped. `Debug` never shows it.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretKey(Zeroizing<[u8; KEY_BYTES]>);

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretKey(<redacted>)")
    }
}

fn check(key: &[u8; KEY_BYTES]) -> [u8; CHECK_BYTES] {
    let mut h = Sha256::new();
    h.update(b"rk/secret-key/check/v1");
    h.update(key);
    let d = h.finalize();
    [d[0], d[1], d[2], d[3]]
}

impl SecretKey {
    pub fn generate(rng: &mut impl CryptoRng) -> Self {
        Self(Zeroizing::new(crypto::random_bytes::<KEY_BYTES>(rng)))
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &*self.0
    }

    /// The text to print or type: `RK1-` and eight groups of four letters and digits. The last 32 bits are a
    /// checksum, so a typing mistake is caught at once instead of looking like a wrong password.
    pub fn to_text(&self) -> Zeroizing<String> {
        let mut raw = Zeroizing::new(Vec::with_capacity(KEY_BYTES + CHECK_BYTES));
        raw.extend_from_slice(&*self.0);
        raw.extend_from_slice(&check(&self.0));
        let mut out = Zeroizing::new(String::from(PREFIX));
        // 20 bytes = 160 bits = exactly 32 base32 characters.
        let (mut buf, mut bits, mut n) = (0u32, 0u32, 0usize);
        for b in raw.iter() {
            buf = (buf << 8) | u32::from(*b);
            bits += 8;
            while bits >= 5 {
                bits -= 5;
                if n % 4 == 0 {
                    out.push('-');
                }
                out.push(ALPHABET[((buf >> bits) & 31) as usize] as char);
                n += 1;
            }
            buf &= (1 << bits) - 1;
        }
        out
    }

    /// Read the text form. Case, spaces and dashes do not matter; the `RK1` prefix is optional.
    pub fn parse(text: &str) -> Result<Self> {
        let t: String = text
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '-')
            .collect::<String>()
            .to_ascii_uppercase();
        let t = Zeroizing::new(t);
        let body = t.strip_prefix(PREFIX).unwrap_or(&t);
        if body.len() != 32 {
            return Err(Error::InvalidArgument(
                "a Secret Key has 32 letters and digits after RK1".into(),
            ));
        }
        let raw = Zeroizing::new(crate::totp::base32_decode(body)?);
        if raw.len() != KEY_BYTES + CHECK_BYTES {
            return Err(Error::InvalidArgument("not a Secret Key".into()));
        }
        let key: [u8; KEY_BYTES] = raw[..KEY_BYTES].try_into().expect("length checked");
        if raw[KEY_BYTES..] != check(&key) {
            return Err(Error::InvalidArgument(
                "the Secret Key has a typing mistake; check it against your recovery sheet".into(),
            ));
        }
        Ok(Self(Zeroizing::new(key)))
    }
}
