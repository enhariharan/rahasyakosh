//! RFC 6238 TOTP (HOTP per RFC 4226).
use crate::{Error, Result};
use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;
use sha2::{Sha256, Sha512};
use zeroize::{Zeroize, ZeroizeOnDrop};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    Sha1,
    Sha256,
    Sha512,
}

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct Totp {
    secret: Vec<u8>,
    #[zeroize(skip)]
    pub algorithm: Algorithm,
    #[zeroize(skip)]
    pub digits: u32,
    #[zeroize(skip)]
    pub period: u64,
}

impl Totp {
    pub fn new(secret: Vec<u8>, algorithm: Algorithm, digits: u32, period: u64) -> Result<Self> {
        if !(6..=8).contains(&digits) || period == 0 || secret.is_empty() {
            return Err(Error::InvalidArgument("bad TOTP parameters".into()));
        }
        Ok(Self { secret, algorithm, digits, period })
    }

    /// Standard defaults: SHA-1, 6 digits, 30 s, secret given as base32.
    pub fn from_base32(secret: &str) -> Result<Self> {
        Self::new(base32_decode(secret)?, Algorithm::Sha1, 6, 30)
    }

    /// Parse an `otpauth://totp/...?secret=...&algorithm=...&digits=...&period=...` URI.
    pub fn from_uri(uri: &str) -> Result<Self> {
        let rest = uri
            .strip_prefix("otpauth://totp/")
            .ok_or_else(|| Error::InvalidArgument("not an otpauth://totp URI".into()))?;
        let query = rest.split_once('?').map(|x| x.1).unwrap_or("");
        let (mut secret, mut algo, mut digits, mut period) = (None, Algorithm::Sha1, 6, 30);
        for kv in query.split('&') {
            let Some((k, v)) = kv.split_once('=') else { continue };
            match k {
                "secret" => secret = Some(base32_decode(v)?),
                "algorithm" => {
                    algo = match v.to_ascii_uppercase().as_str() {
                        "SHA1" => Algorithm::Sha1,
                        "SHA256" => Algorithm::Sha256,
                        "SHA512" => Algorithm::Sha512,
                        _ => return Err(Error::InvalidArgument("unknown algorithm".into())),
                    }
                }
                "digits" => digits = v.parse().map_err(|_| Error::InvalidArgument("digits".into()))?,
                "period" => period = v.parse().map_err(|_| Error::InvalidArgument("period".into()))?,
                _ => {}
            }
        }
        Self::new(secret.ok_or_else(|| Error::InvalidArgument("missing secret".into()))?, algo, digits, period)
    }

    pub fn code_at(&self, unix_time: u64) -> String {
        let counter = (unix_time / self.period).to_be_bytes();
        let mac = match self.algorithm {
            Algorithm::Sha1 => hmac_bytes::<Hmac<Sha1>>(&self.secret, &counter),
            Algorithm::Sha256 => hmac_bytes::<Hmac<Sha256>>(&self.secret, &counter),
            Algorithm::Sha512 => hmac_bytes::<Hmac<Sha512>>(&self.secret, &counter),
        };
        let off = (mac[mac.len() - 1] & 0x0f) as usize;
        let bin = u32::from_be_bytes([mac[off] & 0x7f, mac[off + 1], mac[off + 2], mac[off + 3]]);
        let code = bin % 10u32.pow(self.digits);
        format!("{code:0width$}", width = self.digits as usize)
    }

    pub fn seconds_remaining(&self, unix_time: u64) -> u64 {
        self.period - unix_time % self.period
    }
}

fn hmac_bytes<M: Mac + KeyInit>(key: &[u8], msg: &[u8]) -> Vec<u8> {
    let mut m = <M as KeyInit>::new_from_slice(key).expect("HMAC accepts any key length");
    m.update(msg);
    m.finalize().into_bytes().to_vec()
}

/// RFC 4648 base32, case-insensitive, ignores spaces, dashes and padding.
pub fn base32_decode(s: &str) -> Result<Vec<u8>> {
    let (mut buf, mut bits, mut out) = (0u32, 0u32, Vec::new());
    for c in s.chars().filter(|c| !matches!(c, ' ' | '-' | '=')) {
        let v = match c.to_ascii_uppercase() {
            c @ 'A'..='Z' => c as u32 - 'A' as u32,
            c @ '2'..='7' => c as u32 - '2' as u32 + 26,
            _ => return Err(Error::InvalidArgument("invalid base32".into())),
        };
        buf = (buf << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 6238 Appendix B, 8 digits. Seeds are ASCII "12345678901234567890" (+ repeats for SHA-256/512).
    #[test]
    fn rfc6238_vectors() {
        let s1 = b"12345678901234567890".to_vec();
        let s256 = b"12345678901234567890123456789012".to_vec();
        let s512 = b"1234567890123456789012345678901234567890123456789012345678901234".to_vec();
        let cases: [(u64, &str, &str, &str); 6] = [
            (59, "94287082", "46119246", "90693936"),
            (1111111109, "07081804", "68084774", "25091201"),
            (1111111111, "14050471", "67062674", "99943326"),
            (1234567890, "89005924", "91819424", "93441116"),
            (2000000000, "69279037", "90698825", "38618901"),
            (20000000000, "65353130", "77737706", "47863826"),
        ];
        for (t, a, b, c) in cases {
            assert_eq!(Totp::new(s1.clone(), Algorithm::Sha1, 8, 30).unwrap().code_at(t), a);
            assert_eq!(Totp::new(s256.clone(), Algorithm::Sha256, 8, 30).unwrap().code_at(t), b);
            assert_eq!(Totp::new(s512.clone(), Algorithm::Sha512, 8, 30).unwrap().code_at(t), c);
        }
    }

    #[test]
    fn base32_and_uri() {
        assert_eq!(base32_decode("MZXW6YTBOI======").unwrap(), b"foobar");
        let t = Totp::from_uri("otpauth://totp/Ex:alice?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&digits=8").unwrap();
        assert_eq!(t.code_at(59), "94287082");
        assert!(base32_decode("1!").is_err());
    }
}
