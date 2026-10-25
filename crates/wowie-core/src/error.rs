use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Error taxonomy for domain operations and invariants in `wowie-core`.
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CoreError {
    #[error("Validation failed for field '{field}': {message}")]
    ValidationError { field: String, message: String },

    #[error("Invalid session state transition from '{from}' to '{to}': {reason}")]
    InvalidStateTransition {
        from: String,
        to: String,
        reason: String,
    },

    #[error("Time regression detected: attempted timestamp '{attempted}' is earlier than current state timestamp '{current}'")]
    TimeRegression { current: String, attempted: String },

    #[error("Invalid identifier: {0}")]
    InvalidId(String),

    #[error("Consent violation: {0}")]
    ConsentViolation(String),

    #[error("Duration limit exceeded: requested {requested}s exceeds limit {limit}s")]
    DurationLimitExceeded { limit: u32, requested: u32 },

    #[error("Session is already in terminal state '{state}': {reason}")]
    SessionTerminated { state: String, reason: String },
}

impl CoreError {
    pub fn validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ValidationError {
            field: field.into(),
            message: message.into(),
        }
    }
}
