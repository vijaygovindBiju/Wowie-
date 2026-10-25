use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AvatarError {
    #[error("Model file exceeds maximum permitted size of {limit_bytes} bytes (got {actual_bytes} bytes)")]
    FileTooLarge { actual_bytes: u64, limit_bytes: u64 },

    #[error("Invalid 3D model container magic header: expected 'glTF' (0x46546C67), found {0:?}")]
    InvalidMagic([u8; 4]),

    #[error("Malformed glTF/GLB chunk structure: {0}")]
    MalformedGlb(String),

    #[error("Security policy violation: {0}")]
    SecurityViolation(String),

    #[error("Unsupported model format or missing humanoid rig: {0}")]
    UnsupportedFormat(String),

    #[error("Character not found: {0}")]
    NotFound(String),

    #[error("I/O error during avatar operation: {0}")]
    Io(String),

    #[error("JSON deserialization error: {0}")]
    Serialization(String),
}
