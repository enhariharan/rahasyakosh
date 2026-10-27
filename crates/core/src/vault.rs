//! Vault header, key wrapping and per-item records. See docs/04-security-and-crypto.md.
use crate::crypto::{self, KdfParams, Key, Sealed, NONCE_LEN, SALT_LEN};
use crate::item::Item;
use crate::secret_key::SecretKey;
use crate::{Error, Result};
use rand_core::CryptoRng;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use unicode_normalization::UnicodeNormalization;
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

/// How the master password was turned into bytes before key derivation (audit finding P1-1).
///
/// The same visible text can be different bytes (`é` as one character or as `e` plus an accent), depending on the
/// keyboard and platform. NFKC is one agreed form, so the same password always gives the same key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PwNorm {
    #[serde(rename = "nfkc")]
    Nfkc,
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
    /// `None` in vaults made before the password was normalised: those were keyed from the raw bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pw_norm: Option<PwNorm>,
    /// The vault key is also locked with a Secret Key (see [`SecretKey`]). Authenticated like every field.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub secret_key: bool,
}

impl VaultHeader {
    fn aad(&self) -> Vec<u8> {
        let mut a = header_aad(self.format_version, &self.vault_id, &self.kdf, &self.salt);
        // Appended only when set, so the AAD of an older header is byte for byte what it always was.
        if let Some(PwNorm::Nfkc) = self.pw_norm {
            a.extend_from_slice(b"pw:nfkc");
        }
        if self.secret_key {
            a.extend_from_slice(b"sk:1");
        }
        a
    }

    /// Derive keys from `password` and unwrap the vault key.
    pub fn unlock(&self, password: &str) -> Result<UnlockedVault> {
        self.unlock_with(password, None)
    }

    /// Like [`unlock`](Self::unlock), for a vault that may need its Secret Key. A vault made with one fails with
    /// [`Error::SecretKeyRequired`] when none is given; a Secret Key given to a vault without one is ignored.
    pub fn unlock_with(
        &self,
        password: &str,
        secret_key: Option<&SecretKey>,
    ) -> Result<UnlockedVault> {
        if self.format_version != FORMAT_VERSION {
            return Err(Error::UnsupportedVersion(self.format_version));
        }
        let sk = match (self.secret_key, secret_key) {
            (true, None) => return Err(Error::SecretKeyRequired),
            (true, Some(k)) => Some(k),
            (false, _) => None,
        };
        check_kdf(&self.kdf)?;
        let mut last = Error::Decrypt;
        for candidate in self.password_forms(password) {
            let kek = derive_kek(&candidate, &self.salt, &self.kdf, sk)?;
            match crypto::open(&kek, &(&self.wrapped_vk).into(), &self.aad()) {
                Ok(vk_bytes) => {
                    let vk: [u8; 32] =
                        vk_bytes.as_slice().try_into().map_err(|_| Error::Decrypt)?;
                    return Ok(UnlockedVault {
                        header: self.clone(),
                        vk: Zeroizing::new(vk),
                        secret_key: sk.cloned(),
                    });
                }
                Err(e) => last = e,
            }
        }
        Err(last)
    }

    /// The forms of `password` to try, in order. A vault with `pw_norm` set uses only the normalised form. An older
    /// vault was keyed from whatever bytes the user's keyboard produced, so it tries the typed bytes first, then
    /// the two standard forms (composed and decomposed). Text that is the same in all of them costs one attempt.
    fn password_forms(&self, password: &str) -> Vec<Zeroizing<String>> {
        let nfkc = Zeroizing::new(password.nfkc().collect::<String>());
        if self.pw_norm.is_some() {
            return vec![nfkc];
        }
        let mut forms = vec![Zeroizing::new(password.to_owned())];
        let nfkd = Zeroizing::new(password.nfkd().collect::<String>());
        for f in [nfkc, nfkd] {
            if !forms.iter().any(|g| **g == *f) {
                forms.push(f);
            }
        }
        forms
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

fn derive_kek(
    password: &str,
    salt: &[u8],
    kdf: &KdfParams,
    secret_key: Option<&SecretKey>,
) -> Result<Key> {
    let mk = crypto::derive_master_key(password.as_bytes(), salt, kdf)?;
    Ok(match secret_key {
        None => crypto::hkdf_expand(&*mk, None, b"rk/kek/v1"),
        // The Secret Key is the HKDF salt: without it the master key alone gives nothing useful.
        Some(sk) => crypto::hkdf_expand(&*mk, Some(sk.as_bytes()), b"rk/kek/v2"),
    })
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

/// The item id under which the sealed [`Manifest`] is stored. No item can have it (item ids are random UUIDs).
pub const MANIFEST_ID: Uuid = Uuid::nil();

/// A signed-off list of what the vault should contain, sealed under the vault key (audit findings P2-1 and P2-2).
/// A record by itself proves only that it is genuine, not that it is the latest, or that nothing else was removed.
/// The manifest says which revision of each item is the newest and which items were deleted for good, so a
/// swapped-in old record, a removed record or a forged tombstone can be noticed. Its own `rev` in the store is `seq`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// Counts the writes ever made to the vault. Only goes up.
    pub seq: u64,
    /// Item id to the newest revision written.
    pub live: BTreeMap<Uuid, u64>,
    /// Item id to the revision of its tombstone, for items deleted for good.
    pub gone: BTreeMap<Uuid, u64>,
}

/// An unlocked vault holding the vault key. Key is zeroized on drop.
pub struct UnlockedVault {
    header: VaultHeader,
    vk: Key,
    /// Kept so that a password change can lock the vault key with it again.
    secret_key: Option<SecretKey>,
}

impl UnlockedVault {
    pub fn create(password: &str, kdf: KdfParams, rng: &mut impl CryptoRng) -> Result<Self> {
        Self::create_with(password, kdf, Some(PwNorm::Nfkc), rng)
    }

    /// `norm = None` makes a header in the format before password normalisation. For tests of that format only.
    #[doc(hidden)]
    pub fn create_with(
        password: &str,
        kdf: KdfParams,
        norm: Option<PwNorm>,
        rng: &mut impl CryptoRng,
    ) -> Result<Self> {
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
            pw_norm: norm,
            secret_key: false,
        };
        header.wrapped_vk = Self::wrap(password, &header, &vk, None, rng)?;
        Ok(Self {
            header,
            vk,
            secret_key: None,
        })
    }

    /// Make a vault that is also locked with a new Secret Key. Show the key to the user once; it is not stored in the
    /// vault and cannot be recovered from it.
    pub fn create_with_secret_key(
        password: &str,
        kdf: KdfParams,
        rng: &mut impl CryptoRng,
    ) -> Result<(Self, SecretKey)> {
        let mut v = Self::create(password, kdf, rng)?;
        let sk = v.set_secret_key(password, rng)?;
        Ok((v, sk))
    }

    /// Lock the vault key with a new Secret Key as well as the master password (replacing an earlier one). Items are
    /// untouched. `password` must be the current master password, which the caller has just checked.
    pub fn set_secret_key(
        &mut self,
        password: &str,
        rng: &mut impl CryptoRng,
    ) -> Result<SecretKey> {
        let sk = SecretKey::generate(rng);
        let mut h = self.header.clone();
        h.secret_key = true;
        h.wrapped_vk = Self::wrap(password, &h, &self.vk, Some(&sk), rng)?;
        self.header = h;
        self.secret_key = Some(sk.clone());
        Ok(sk)
    }

    /// Go back to the master password alone.
    pub fn clear_secret_key(&mut self, password: &str, rng: &mut impl CryptoRng) -> Result<()> {
        let mut h = self.header.clone();
        h.secret_key = false;
        h.wrapped_vk = Self::wrap(password, &h, &self.vk, None, rng)?;
        self.header = h;
        self.secret_key = None;
        Ok(())
    }

    /// The Secret Key this vault was opened with, if it has one.
    pub fn secret_key(&self) -> Option<&SecretKey> {
        self.secret_key.as_ref()
    }

    fn wrap(
        password: &str,
        h: &VaultHeader,
        vk: &Key,
        secret_key: Option<&SecretKey>,
        rng: &mut impl CryptoRng,
    ) -> Result<SealedKey> {
        // The form of the password is the header's: `pw_norm` set means normalised, unset means the typed bytes.
        let form = h.password_forms(password).swap_remove(0);
        let kek = derive_kek(&form, &h.salt, &h.kdf, secret_key)?;
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
        // A password change moves an older vault to the normalised form.
        h.pw_norm = Some(PwNorm::Nfkc);
        h.salt = crypto::random_bytes::<SALT_LEN>(rng);
        h.wrapped_vk = Self::wrap(new_password, &h, &self.vk, self.secret_key.as_ref(), rng)?;
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

    fn manifest_key(&self) -> Key {
        crypto::hkdf_expand(self.vk.as_slice(), None, b"rk/manifest/v1")
    }

    /// Seal the manifest as a record with id [`MANIFEST_ID`] and revision `m.seq`.
    pub fn seal_manifest(&self, m: &Manifest, rng: &mut impl CryptoRng) -> Result<Record> {
        let json =
            Zeroizing::new(serde_json::to_vec(m).map_err(|e| Error::Malformed(e.to_string()))?);
        let s = crypto::seal(
            &self.manifest_key(),
            &pad(&json),
            &self.record_aad(&MANIFEST_ID, m.seq, RECORD_VERSION),
            rng,
        );
        Ok(Record {
            record_version: RECORD_VERSION,
            item_id: MANIFEST_ID,
            rev: m.seq,
            nonce: s.nonce,
            ct: s.ct,
        })
    }

    pub fn open_manifest(&self, r: &Record) -> Result<Manifest> {
        if r.record_version != RECORD_VERSION {
            return Err(Error::UnsupportedVersion(r.record_version));
        }
        if r.item_id != MANIFEST_ID {
            return Err(Error::Decrypt);
        }
        let pt = crypto::open(
            &self.manifest_key(),
            &Sealed {
                nonce: r.nonce,
                ct: r.ct.clone(),
            },
            &self.record_aad(&MANIFEST_ID, r.rev, r.record_version),
        )?;
        let m: Manifest =
            serde_json::from_slice(unpad(&pt)?).map_err(|e| Error::Malformed(e.to_string()))?;
        if m.seq != r.rev {
            return Err(Error::Decrypt);
        }
        Ok(m)
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
