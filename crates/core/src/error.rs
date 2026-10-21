use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    /// Wrong password or corrupted/tampered data. Deliberately not more specific.
    #[error("decryption failed: wrong password or corrupted data")]
    Decrypt,
    #[error("invalid KDF parameters: {0}")]
    KdfParams(String),
    #[error("unsupported format version {0}")]
    UnsupportedVersion(u16),
    #[error("malformed data: {0}")]
    Malformed(String),
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}
