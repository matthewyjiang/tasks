use clap::error::ErrorKind;
use serde::Serialize;
use thiserror::Error;

pub type CliResult<T> = Result<T, CliError>;

#[derive(Debug, Error)]
pub enum CliError {
    #[error("input error: {0}")]
    Input(String),

    #[error("local storage error: {0}")]
    LocalStorage(String),

    #[error("crypto error: {0}")]
    Crypto(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("unsupported platform capability: {0}")]
    UnsupportedPlatform(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

impl CliError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Input(_) => 1,
            Self::LocalStorage(_) => 2,
            Self::Crypto(_) => 3,
            Self::Network(_) => 4,
            Self::Conflict(_) => 5,
            Self::UnsupportedPlatform(_) => 6,
            Self::Serialization(_) => 1,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Input(_) => "input_error",
            Self::LocalStorage(_) => "local_storage_error",
            Self::Crypto(_) => "crypto_error",
            Self::Network(_) => "network_error",
            Self::Conflict(_) => "conflict",
            Self::UnsupportedPlatform(_) => "unsupported_platform",
            Self::Serialization(_) => "serialization_error",
        }
    }

    pub fn to_json_string(&self) -> String {
        let body = ErrorBody {
            error: ErrorPayload {
                code: self.code(),
                message: self.to_string(),
                details: serde_json::Value::Null,
            },
        };
        serde_json::to_string(&body).unwrap_or_else(|_| "{\"error\":{\"code\":\"serialization_error\",\"message\":\"failed to serialize error\",\"details\":null}}".to_string())
    }
}

impl From<clap::Error> for CliError {
    fn from(error: clap::Error) -> Self {
        match error.kind() {
            ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => Self::Input(error.to_string()),
            _ => Self::Input(error.to_string()),
        }
    }
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    error: ErrorPayload<'a>,
}

#[derive(Serialize)]
struct ErrorPayload<'a> {
    code: &'a str,
    message: String,
    details: serde_json::Value,
}
