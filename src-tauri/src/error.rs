use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("audio capture failed: {0}")]
    Audio(String),
    #[error("recording state error: {0}")]
    State(String),
    #[error("secure storage failed: {0}")]
    Crypto(String),
    #[error("library storage failed: {0}")]
    Storage(String),
    #[error("recording not found")]
    NotFound,
    #[error("permission is required: {0}")]
    Permission(String),
    #[error("operation failed: {0}")]
    Other(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Storage(value.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(value: std::io::Error) -> Self {
        Self::Storage(value.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
