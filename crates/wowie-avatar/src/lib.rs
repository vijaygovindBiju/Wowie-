//! # wowie-avatar
//!
//! Custom 3D character system for Wowie.
//! Provides VRM 1.0, VRM 0.x, and glTF 2.0 binary asset validation,
//! untrusted file sanitization, character registry management,
//! mood/reaction mapping, and visual companion fallback rendering.

pub mod companion;
pub mod error;
pub mod model;
pub mod registry;
pub mod sample;
pub mod speech;
pub mod validator;

pub use companion::CompanionRuntime;
pub use error::AvatarError;
pub use model::{
    BoundingBox3D, CharacterId, CharacterMetadata, CharacterProfile, CompanionEvent, CompanionMood,
    Mesh3D, Model3D, ModelFormat, Vertex3D,
};
pub use registry::CharacterRegistry;
pub use sample::create_sample_vrm1_bytes;
pub use speech::{SpeechSynthesizer, VoiceConfig, VoiceEngine};
pub use validator::ModelValidator;
