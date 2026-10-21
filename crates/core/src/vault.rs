//! Vault header, key wrapping and per-item records. See docs/04-security-and-crypto.md.
use crate::crypto::{self, KdfParams, Key, Sealed, NONCE_LEN, SALT_LEN};
use crate::item::Item;
use crate::{Error, Result};
use rand_core::CryptoRng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zeroize::Zeroizing;

pub const MAGIC: &[u8; 4] = b"RKV1";
pub const FORMAT_VERSION: u16 = 1;
pub const RECORD_VERSION: u16 = 1;
const PAD_BUCKET: usize = 64;

mod b64 {
    use base64::{engine::general_purpose::STANDARD_NO_PAD as E, Engine};
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(v: &impl AsRef<[u8]>, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&E.encode(v))
    }
    pub fn deserialize<'de, D: Deserializer<'de>, T: TryFrom<Vec<u8>>>(
        d: D,
    ) -> Result<T, D::Error> {
        let s = String::deserialize(d)?;
        let v = E.decode(s).map_err(serde::de::Error::custom)?;
        T::try_from(v).map_err(|_| serde::de::Error::custom("wrong length"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SealedKey {
    #[serde(with = "b64")]
    pub nonce: [u8; NONCE_LEN],
    #[serde(with = "b64")]
    pub ct: Vec<u8>,
}

impl From<Sealed> for SealedKey {
    fn from(s: Sealed) -> Self {
        Self {
            nonce: s.nonce,
            ct: s.ct,
        }
    }
}
impl From<&SealedKey> for Sealed {
    fn from(s: &SealedKey) -> Self {
        Self {
            nonce: s.nonce,
            ct: s.ct.clone(),
        }
    }
}

/// Plaintext vault header. Every field is authenticated as AAD of `wrapped_vk`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultHeader {
    pub format_version: u16,
    pub vault_id: Uuid,
    pub kdf: KdfParams,
    #[serde(with = "b64")]
    pub salt: [u8; SALT_LEN],
    pub wrapped_vk: SealedKey,
}

impl VaultHeader {
    fn aad(&self) -> Vec<u8> {
        header_aad(self.format_version, &self.vault_id, &self.kdf, &self.salt)
    }

    /// Derive keys from `password` and unwrap the vault key.
    pub fn unlock(&self, password: &str) -> Result<UnlockedVault> {
        if self.format_version != FORMAT_VERSION {
            return Err(Error::UnsupportedVersion(self.format_version));
        }
        check_kdf(&self.kdf)?;
        let kek = derive_kek(password, &self.salt, &self.kdf)?;
        let vk_bytes = crypto::open(&kek, &(&self.wrapped_vk).into(), &self.aad())?;
        let vk: [u8; 32] = vk_bytes.as_slice().try_into().map_err(|_| Error::Decrypt)?;
        Ok(UnlockedVault {
            header: self.clone(),
            vk: Zeroizing::new(vk),
        })
    }
}

fn check_kdf(p: &KdfParams) -> Result<()> {
    // Unit tests use deliberately cheap parameters.
    if cfg!(test) {
        return Ok(());
    }
    p.validate()
}

fn header_aad(ver: u16, id: &Uuid, kdf: &KdfParams, salt: &[u8; SALT_LEN]) -> Vec<u8> {
    let mut a = Vec::with_capacity(64);
    a.extend_from_slice(MAGIC);
    a.extend_from_slice(&ver.to_le_bytes());
    a.extend_from_slice(id.as_bytes());
    a.extend_from_slice(&kdf.m_kib.to_le_bytes());
    a.extend_from_slice(&kdf.t.to_le_bytes());
    a.extend_from_slice(&kdf.p.to_le_bytes());
    a.extend_from_slice(salt);
    a
}

fn derive_kek(password: &str, salt: &[u8], kdf: &KdfParams) -> Result<Key> {
    let mk = crypto::derive_master_key(password.as_bytes(), salt, kdf)?;
    Ok(crypto::hkdf_expand(&*mk, None, b"rk/kek/v1"))
}

/// One encrypted item as stored/synced. Only id and rev are plaintext.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub record_version: u16,
    pub item_id: Uuid,
    /// Monotonic revision (later: hybrid logical clock for sync).
    pub rev: u64,
    #[serde(with = "b64")]
    pub nonce: [u8; NONCE_LEN],
    #[serde(with = "b64")]
    pub ct: Vec<u8>,
}

/// An unlocked vault holding the vault key. Key is zeroized on drop.
pub struct UnlockedVault {
    header: VaultHeader,
    vk: Key,
}

impl UnlockedVault {
    pub fn create(password: &str, kdf: KdfParams, rng: &mut impl CryptoRng) -> Result<Self> {
        check_kdf(&kdf)?;
        let salt = crypto::random_bytes::<SALT_LEN>(rng);
        let vault_id = Uuid::from_bytes(crypto::random_bytes::<16>(rng));
        let vk = crypto::random_key(rng);
        let mut header = VaultHeader {
            format_version: FORMAT_VERSION,
            vault_id,
            kdf,
            salt,
            wrapped_vk: SealedKey {
                nonce: [0; NONCE_LEN],
                ct: vec![],
            },
        };
        header.wrapped_vk = Self::wrap(password, &header, &vk, rng)?;
        Ok(Self { header, vk })
    }

    fn wrap(
        password: &str,
        h: &VaultHeader,
        vk: &Key,
        rng: &mut impl CryptoRng,
    ) -> Result<SealedKey> {
        let kek = derive_kek(password, &h.salt, &h.kdf)?;
        Ok(crypto::seal(&kek, vk.as_slice(), &h.aad(), rng).into())
    }

    pub fn header(&self) -> &VaultHeader {
        &self.header
    }

    /// Re-wrap the vault key under a new password (fresh salt). Items are untouched.
    pub fn change_password(
        &mut self,
        new_password: &str,
        kdf: KdfParams,
        rng: &mut impl CryptoRng,
    ) -> Result<()> {
        check_kdf(&kdf)?;
        let mut h = self.header.clone();
        h.kdf = kdf;
        h.salt = crypto::random_bytes::<SALT_LEN>(rng);
        h.wrapped_vk = Self::wrap(new_password, &h, &self.vk, rng)?;
        self.header = h;
        Ok(())
    }

    fn item_key(&self, id: &Uuid) -> Key {
        let mut info = b"rk/item/v1".to_vec();
        info.extend_from_slice(id.as_bytes());
        crypto::hkdf_expand(self.vk.as_slice(), None, &info)
    }

    fn record_aad(&self, id: &Uuid, rev: u64, rv: u16) -> Vec<u8> {
        let mut a = Vec::with_capacity(48);
        a.extend_from_slice(MAGIC);
        a.extend_from_slice(self.header.vault_id.as_bytes());
        a.extend_from_slice(id.as_bytes());
        a.extend_from_slice(&rev.to_le_bytes());
        a.extend_from_slice(&rv.to_le_bytes());
        a
    }

    pub fn seal_item(&self, item: &Item, rev: u64, rng: &mut impl CryptoRng) -> Result<Record> {
        let json =
            Zeroizing::new(serde_json::to_vec(item).map_err(|e| Error::Malformed(e.to_string()))?);
        let padded = pad(&json);
        let s = crypto::seal(
            &self.item_key(&item.id),
            &padded,
            &self.record_aad(&item.id, rev, RECORD_VERSION),
            rng,
        );
        Ok(Record {
            record_version: RECORD_VERSION,
            item_id: item.id,
            rev,
            nonce: s.nonce,
            ct: s.ct,
        })
    }

    /// Seal `padded` byte-for-byte as a record's payload, bypassing the item model and the padding step.
    ///
    /// **Only for fuzzing and tests.** Random bytes can never pass authentication, so this is the only way to reach the
    /// code that parses a decrypted payload (`unpad` and the item JSON) with hostile input, which is what a buggy or
    /// newer program could produce. It is hidden from the documentation and never used by the programs.
    #[doc(hidden)]
    pub fn seal_raw_payload_for_fuzzing(
        &self,
        id: Uuid,
        rev: u64,
        padded: &[u8],
        rng: &mut impl CryptoRng,
    ) -> Record {
        let s = crypto::seal(
            &self.item_key(&id),
            padded,
            &self.record_aad(&id, rev, RECORD_VERSION),
            rng,
        );
        Record {
            record_version: RECORD_VERSION,
            item_id: id,
            rev,
            nonce: s.nonce,
            ct: s.ct,
        }
    }

    pub fn open_record(&self, r: &Record) -> Result<Item> {
        if r.record_version != RECORD_VERSION {
            return Err(Error::UnsupportedVersion(r.record_version));
        }
        let pt = crypto::open(
            &self.item_key(&r.item_id),
            &Sealed {
                nonce: r.nonce,
                ct: r.ct.clone(),
            },
            &self.record_aad(&r.item_id, r.rev, r.record_version),
        )?;
        let item: Item =
            serde_json::from_slice(unpad(&pt)?).map_err(|e| Error::Malformed(e.to_string()))?;
        if item.id != r.item_id {
            return Err(Error::Decrypt);
        }
        Ok(item)
    }
}

/// `u32 LE length ‖ data ‖ zero padding` up to a multiple of `PAD_BUCKET`.
fn pad(data: &[u8]) -> Zeroizing<Vec<u8>> {
    let total = (4 + data.len()).div_ceil(PAD_BUCKET) * PAD_BUCKET;
    let mut v = Zeroizing::new(Vec::with_capacity(total));
    v.extend_from_slice(&(data.len() as u32).to_le_bytes());
    v.extend_from_slice(data);
    v.resize(total, 0);
    v
}

fn unpad(p: &[u8]) -> Result<&[u8]> {
    let len = u32::from_le_bytes(p.get(..4).ok_or(Error::Decrypt)?.try_into().unwrap()) as usize;
    // `4 + len` must not overflow on 32-bit targets (WebAssembly), where a length near 4 GiB would wrap or panic.
    let end = len.checked_add(4).ok_or(Error::Decrypt)?;
    p.get(4..end).ok_or(Error::Decrypt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item::Item;
    use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};

    fn new_id(r: &mut ChaCha20Rng) -> Uuid {
        Uuid::from_bytes(crypto::random_bytes::<16>(r))
    }

    fn rng() -> ChaCha20Rng {
        ChaCha20Rng::seed_from_u64(7)
    }

    #[test]
    fn create_unlock_roundtrip() {
        let mut r = rng();
        let v = UnlockedVault::create("hunter2", KdfParams::INSECURE_TEST, &mut r).unwrap();
        let item = Item::login("GitHub", "alice", "s3cret", 1, new_id(&mut r));
        let rec = v.seal_item(&item, 1, &mut r).unwrap();

        let json = serde_json::to_string(v.header()).unwrap();
        let h: VaultHeader = serde_json::from_str(&json).unwrap();
        let v2 = h.unlock("hunter2").unwrap();
        assert_eq!(v2.open_record(&rec).unwrap(), item);
    }

    #[test]
    fn wrong_password_fails() {
        let v = UnlockedVault::create("a", KdfParams::INSECURE_TEST, &mut rng()).unwrap();
        assert!(matches!(v.header().unlock("b"), Err(Error::Decrypt)));
    }

    #[test]
    fn header_tamper_detected() {
        let v = UnlockedVault::create("a", KdfParams::INSECURE_TEST, &mut rng()).unwrap();
        let mut h = v.header().clone();
        h.kdf.t += 1;
        assert!(h.unlock("a").is_err());
    }

    #[test]
    fn record_tamper_and_swap_detected() {
        let mut r = rng();
        let v = UnlockedVault::create("a", KdfParams::INSECURE_TEST, &mut r).unwrap();
        let a = Item::login("a", "u", "p", 1, new_id(&mut r));
        let b = Item::login("b", "u", "p", 1, new_id(&mut r));
        let ra = v.seal_item(&a, 1, &mut r).unwrap();
        let rb = v.seal_item(&b, 1, &mut r).unwrap();

        let mut bad = ra.clone();
        bad.ct[0] ^= 1;
        assert!(v.open_record(&bad).is_err());
        let mut bad = ra.clone();
        bad.rev = 2; // replayed under a different revision
        assert!(v.open_record(&bad).is_err());
        let mut swapped = ra.clone();
        swapped.ct = rb.ct;
        swapped.nonce = rb.nonce;
        assert!(v.open_record(&swapped).is_err());
    }

    #[test]
    fn change_password_keeps_items() {
        let mut r = rng();
        let mut v = UnlockedVault::create("old", KdfParams::INSECURE_TEST, &mut r).unwrap();
        let item = Item::login("x", "u", "p", 1, new_id(&mut r));
        let rec = v.seal_item(&item, 1, &mut r).unwrap();
        v.change_password("new", KdfParams::INSECURE_TEST, &mut r)
            .unwrap();
        assert!(v.header().unlock("old").is_err());
        let v2 = v.header().unlock("new").unwrap();
        assert_eq!(v2.open_record(&rec).unwrap(), item);
    }

    #[test]
    fn padding_hides_small_length_differences() {
        let mut r = rng();
        let v = UnlockedVault::create("a", KdfParams::INSECURE_TEST, &mut r).unwrap();
        let id = new_id(&mut r);
        let r1 = v
            .seal_item(&Item::login("t", "u", "p", 1, id), 1, &mut r)
            .unwrap();
        let r2 = v
            .seal_item(&Item::login("t", "u", "pp", 1, id), 1, &mut r)
            .unwrap();
        assert_eq!(r1.ct.len(), r2.ct.len());
    }
}
