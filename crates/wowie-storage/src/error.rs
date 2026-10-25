use thiserror::Error;
use wowie_core::error::CoreError;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Serialization / JSON error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Domain core validation error: {0}")]
    Core(#[from] CoreError),

    #[error("Database migration error: {0}")]
    Migration(String),

    #[error(
        "Migration checksum mismatch for version {version}: expected '{expected}', found '{found}'"
    )]
    MigrationChecksumMismatch {
        version: i64,
        expected: String,
        found: String,
    },

    #[error("Entity not found: {entity} with ID {id}")]
    NotFound { entity: &'static str, id: String },

    #[error("Connection lock error: {0}")]
    LockError(String),

    #[error("Data integrity error: {0}")]
    DataIntegrity(String),
}
