//! User profiles (E13, US-PRF-01..05): the list of who has a vault in this installation.
//!
//! Every profile owns a separate vault file with its own master password, so this list holds nothing secret:
//! just each profile's ID, full name, username and the name of its vault file. It is shown before any vault is
//! unlocked, so the user can pick one. The list is stored as JSON with a version number; unknown fields from a
//! newer version are kept out of the way by `#[serde(default)]`, so attributes can be added later.
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The full name and username a first-run profile gets unless the user changes them.
pub const DEFAULT_FULL_NAME: &str = "Default user";
pub const DEFAULT_USERNAME: &str = "defaultuser";

const VERSION: u32 = 1;

/// One user's profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    /// Generated once, never changes.
    pub id: Uuid,
    pub full_name: String,
    /// Unique across profiles, ignoring case.
    pub username: String,
    /// The vault file, relative to the data folder.
    pub file: String,
}

/// Why a profile change was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProfileError {
    #[error("the full name is empty")]
    EmptyName,
    #[error("the username is empty")]
    EmptyUsername,
    #[error("another profile already uses that username")]
    UsernameTaken,
    #[error("there is no such profile")]
    Unknown,
    #[error("the last profile cannot be removed")]
    LastProfile,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ProfileList {
    #[serde(default)]
    version: u32,
    #[serde(default)]
    profiles: Vec<Profile>,
    /// The profile used last, so the app can start on it. Not a secret; may name a profile that is gone.
    #[serde(default)]
    last_used: Option<Uuid>,
}

/// A random version-4 UUID from the caller's generator.
pub fn new_profile_id(rng: &mut impl rand_core::CryptoRng) -> Uuid {
    let mut bytes = [0u8; 16];
    rng.fill_bytes(&mut bytes);
    uuid::Builder::from_random_bytes(bytes).into_uuid()
}

impl ProfileList {
    pub fn new() -> Self {
        Self {
            version: VERSION,
            profiles: Vec::new(),
            last_used: None,
        }
    }

    /// Reads a saved list; `None` if the bytes are not one (the caller decides whether that is an error).
    pub fn from_json(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }

    pub fn to_json(&self) -> Vec<u8> {
        serde_json::to_vec_pretty(self).unwrap_or_default()
    }

    pub fn all(&self) -> &[Profile] {
        &self.profiles
    }

    pub fn len(&self) -> usize {
        self.profiles.len()
    }

    pub fn is_empty(&self) -> bool {
        self.profiles.is_empty()
    }

    pub fn get(&self, id: Uuid) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    /// The profile with this username, ignoring case and surrounding spaces.
    pub fn find_by_username(&self, username: &str) -> Option<&Profile> {
        let lower = username.trim().to_lowercase();
        self.profiles
            .iter()
            .find(|p| p.username.to_lowercase() == lower)
    }

    /// Remembers which profile was used last. Returns false (and changes nothing) if there is no such profile.
    pub fn set_last_used(&mut self, id: Uuid) -> bool {
        let known = self.get(id).is_some();
        if known {
            self.last_used = Some(id);
        }
        known
    }

    /// The profile to start on: the one used last if it still exists, else the first.
    pub fn start_profile(&self) -> Option<&Profile> {
        self.last_used
            .and_then(|id| self.get(id))
            .or_else(|| self.profiles.first())
    }

    /// How to show a profile in a list: its full name, with the username added when another profile has the same
    /// full name (ignoring case), so two entries are never identical.
    pub fn display_name(&self, id: Uuid) -> String {
        let Some(p) = self.get(id) else {
            return String::new();
        };
        let lower = p.full_name.to_lowercase();
        let clash = self
            .profiles
            .iter()
            .any(|o| o.id != id && o.full_name.to_lowercase() == lower);
        if clash {
            format!("{} ({})", p.full_name, p.username)
        } else {
            p.full_name.clone()
        }
    }

    /// Checks a name and username (trimmed) against the rules; `except` is the profile being edited, if any.
    fn check(
        &self,
        full_name: &str,
        username: &str,
        except: Option<Uuid>,
    ) -> Result<(String, String), ProfileError> {
        let (name, user) = (full_name.trim(), username.trim());
        if name.is_empty() {
            return Err(ProfileError::EmptyName);
        }
        if user.is_empty() {
            return Err(ProfileError::EmptyUsername);
        }
        let lower = user.to_lowercase();
        if self
            .profiles
            .iter()
            .any(|p| Some(p.id) != except && p.username.to_lowercase() == lower)
        {
            return Err(ProfileError::UsernameTaken);
        }
        Ok((name.to_string(), user.to_string()))
    }

    /// Adds a profile whose vault is `file`.
    pub fn add(
        &mut self,
        id: Uuid,
        full_name: &str,
        username: &str,
        file: &str,
    ) -> Result<&Profile, ProfileError> {
        let (full_name, username) = self.check(full_name, username, None)?;
        self.version = VERSION;
        self.profiles.push(Profile {
            id,
            full_name,
            username,
            file: file.to_string(),
        });
        Ok(self.profiles.last().expect("just pushed"))
    }

    /// Changes the name and username; the ID and vault file stay.
    pub fn edit(&mut self, id: Uuid, full_name: &str, username: &str) -> Result<(), ProfileError> {
        if self.get(id).is_none() {
            return Err(ProfileError::Unknown);
        }
        let (full_name, username) = self.check(full_name, username, Some(id))?;
        let p = self
            .profiles
            .iter_mut()
            .find(|p| p.id == id)
            .expect("checked");
        p.full_name = full_name;
        p.username = username;
        Ok(())
    }

    /// Removes a profile and returns it (so the caller can delete its vault file).
    pub fn remove(&mut self, id: Uuid) -> Result<Profile, ProfileError> {
        let at = self
            .profiles
            .iter()
            .position(|p| p.id == id)
            .ok_or(ProfileError::Unknown)?;
        if self.profiles.len() == 1 {
            return Err(ProfileError::LastProfile);
        }
        let gone = self.profiles.remove(at);
        if self.last_used == Some(gone.id) {
            self.last_used = None;
        }
        Ok(gone)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_core::SeedableRng;

    fn list() -> ProfileList {
        let mut l = ProfileList::new();
        l.add(
            Uuid::from_u128(1),
            DEFAULT_FULL_NAME,
            DEFAULT_USERNAME,
            "vault.db",
        )
        .unwrap();
        l
    }

    #[test]
    fn usernames_are_unique_ignoring_case_and_names_are_required() {
        let mut l = list();
        assert_eq!(
            l.add(Uuid::from_u128(2), "Other", " DefaultUser ", "b.db")
                .unwrap_err(),
            ProfileError::UsernameTaken
        );
        assert_eq!(
            l.add(Uuid::from_u128(2), " ", "x", "b.db").unwrap_err(),
            ProfileError::EmptyName
        );
        assert_eq!(
            l.add(Uuid::from_u128(2), "X", "  ", "b.db").unwrap_err(),
            ProfileError::EmptyUsername
        );
        assert_eq!(l.len(), 1);
        l.add(Uuid::from_u128(2), "Asha Rao", "asha", "b.db")
            .unwrap();
        assert_eq!(l.len(), 2);
    }

    #[test]
    fn editing_keeps_the_id_and_may_keep_the_same_username() {
        let mut l = list();
        l.add(Uuid::from_u128(2), "Asha Rao", "asha", "b.db")
            .unwrap();
        l.edit(Uuid::from_u128(1), "Me", "DEFAULTUSER").unwrap();
        assert_eq!(l.get(Uuid::from_u128(1)).unwrap().full_name, "Me");
        assert_eq!(
            l.edit(Uuid::from_u128(1), "Me", "Asha").unwrap_err(),
            ProfileError::UsernameTaken
        );
        assert_eq!(
            l.edit(Uuid::from_u128(9), "A", "b").unwrap_err(),
            ProfileError::Unknown
        );
    }

    #[test]
    fn the_last_profile_cannot_be_removed() {
        let mut l = list();
        assert_eq!(
            l.remove(Uuid::from_u128(1)).unwrap_err(),
            ProfileError::LastProfile
        );
        l.add(Uuid::from_u128(2), "Asha Rao", "asha", "b.db")
            .unwrap();
        assert_eq!(l.remove(Uuid::from_u128(2)).unwrap().file, "b.db");
        assert_eq!(l.len(), 1);
    }

    #[test]
    fn the_list_round_trips_and_ignores_fields_it_does_not_know() {
        let l = list();
        assert_eq!(ProfileList::from_json(&l.to_json()), Some(l.clone()));
        let newer = br#"{"version":2,"profiles":[{"id":"00000000-0000-0000-0000-000000000001",
            "full_name":"A","username":"a","file":"v.db","colour":"red"}],"theme":1}"#;
        assert_eq!(ProfileList::from_json(newer).unwrap().len(), 1);
        assert!(ProfileList::from_json(b"not json").is_none());
    }

    #[test]
    fn the_start_profile_is_the_one_used_last_while_it_exists() {
        let mut l = list();
        l.add(Uuid::from_u128(2), "Asha Rao", "asha", "b.db")
            .unwrap();
        assert_eq!(l.start_profile().unwrap().id, Uuid::from_u128(1));
        assert!(l.set_last_used(Uuid::from_u128(2)));
        assert!(!l.set_last_used(Uuid::from_u128(9)), "unknown profile");
        assert_eq!(l.start_profile().unwrap().id, Uuid::from_u128(2));
        // It survives a save and a load.
        let l2 = ProfileList::from_json(&l.to_json()).unwrap();
        assert_eq!(l2.start_profile().unwrap().id, Uuid::from_u128(2));
        // Removing it falls back to the first profile.
        l.remove(Uuid::from_u128(2)).unwrap();
        assert_eq!(l.start_profile().unwrap().id, Uuid::from_u128(1));
        // A list saved before this field existed starts on the first profile.
        let old = br#"{"version":1,"profiles":[{"id":"00000000-0000-0000-0000-000000000001",
            "full_name":"A","username":"a","file":"v.db"}]}"#;
        assert!(ProfileList::from_json(old)
            .unwrap()
            .start_profile()
            .is_some());
        assert!(ProfileList::new().start_profile().is_none());
    }

    #[test]
    fn same_full_names_show_the_username_and_others_do_not() {
        let mut l = list();
        l.add(Uuid::from_u128(2), "Asha Rao", "asha", "b.db")
            .unwrap();
        l.add(Uuid::from_u128(3), "ASHA RAO", "asha2", "c.db")
            .unwrap();
        assert_eq!(l.display_name(Uuid::from_u128(1)), "Default user");
        assert_eq!(l.display_name(Uuid::from_u128(2)), "Asha Rao (asha)");
        assert_eq!(l.display_name(Uuid::from_u128(3)), "ASHA RAO (asha2)");
        assert_eq!(l.display_name(Uuid::from_u128(9)), "");
    }

    #[test]
    fn a_username_is_found_ignoring_case() {
        let l = list();
        assert!(l.find_by_username(" DefaultUser ").is_some());
        assert!(l.find_by_username("nobody").is_none());
    }

    #[test]
    fn generated_ids_are_random_version_4() {
        let mut rng = rand_chacha::ChaCha20Rng::seed_from_u64(7);
        let (a, b) = (new_profile_id(&mut rng), new_profile_id(&mut rng));
        assert_ne!(a, b);
        assert_eq!(a.get_version_num(), 4);
    }
}
