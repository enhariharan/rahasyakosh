use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    /// Wrong password or corrupted/tampered data. Deliberately not more specific.
    #[error("decryption failed: wrong password or corrupted data")]
    Decrypt,
    #[error("invalid KDF parameters: {0}")]
    KdfParams(String),
    /// The vault was made with a Secret Key and none was given.
    #[error("this vault needs its Secret Key")]
    SecretKeyRequired,
    #[error("unsupported format version {0}")]
    UnsupportedVersion(u16),
    #[error("malformed data: {0}")]
    Malformed(String),
    /// An item (with its history) is bigger than [`crate::MAX_ITEM_BYTES`]; saving it could make the vault unreadable.
    #[error("this item is too large: {size} bytes; the limit is {max} bytes")]
    ItemTooLarge { size: usize, max: usize },
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}
