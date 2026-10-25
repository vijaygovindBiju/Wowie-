use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ObserverError {
    #[error("Platform unsupported: {0}")]
    PlatformUnsupported(String),

    #[error("Platform command failed: {0}")]
    CommandFailed(String),

    #[error("Output deserialization failed: {0}")]
    DeserializationFailed(String),

    #[error("Monitoring consent not granted or revoked")]
    ConsentNotGranted,

    #[error("Monitoring is currently paused")]
    MonitoringPaused,

    #[error("Internal observer error: {0}")]
    Internal(String),
}
