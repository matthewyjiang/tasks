use thiserror::Error;
use uuid::Uuid;

pub type CoreResult<T> = Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error(transparent)]
    Crypto(#[from] CryptoError),

    #[error(transparent)]
    Database(#[from] DbError),

    #[error(transparent)]
    Sync(#[from] SyncError),

    #[error(transparent)]
    Platform(#[from] PlatformError),

    #[error(transparent)]
    Settings(#[from] SettingsError),

    #[error("serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

impl From<rusqlite::Error> for CoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(DbError::Sqlite(error))
    }
}

impl From<std::io::Error> for CoreError {
    fn from(error: std::io::Error) -> Self {
        Self::Settings(SettingsError::Io(error))
    }
}

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("decrypt failed")]
    DecryptFailed,

    #[error("bad key length: expected 32 bytes, got {0}")]
    BadKeyLength(usize),

    #[error("deserialization failed: {0}")]
    DeserFailed(serde_json::Error),

    #[error("key agreement failed")]
    KeyAgreementFailed,
}

#[derive(Debug, Error)]
pub enum DbError {
    #[error("task not found: {0}")]
    TaskNotFound(Uuid),

    #[error("invalid task status: {0}")]
    InvalidTaskStatus(String),

    #[error("invalid row data: {0}")]
    InvalidRowData(String),

    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("key not found: {0}")]
    KeyNotFound(String),

    #[error("platform operation failed: {0}")]
    OperationFailed(String),
}

#[derive(Debug, Error)]
pub enum SettingsError {
    #[error("settings I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("unsupported vault settings schema version: {0}")]
    UnsupportedVaultSchemaVersion(i32),

    #[error("unexpected vault settings id: {0}")]
    UnexpectedVaultSettingsId(String),
}

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("network unavailable")]
    NetworkUnavailable,

    #[error("auth expired")]
    AuthExpired,

    #[error("blob conflict: {0}")]
    BlobConflict(Uuid),

    #[error("server error {status}: {body}")]
    ServerError { status: u16, body: String },
}
