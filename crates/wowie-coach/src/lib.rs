//! # wowie-coach
//!
//! AI Coach dialogue prompt engine for Wowie focus sessions.
//!
//! Analyzes active goals, commitments, and observed desktop activity
//! to produce structured, tone-aware coaching advice without direct
//! OS execution authority.

#![forbid(unsafe_code)]
#![warn(missing_debug_implementations)]

pub mod engine;
pub mod error;
pub mod models;
pub mod policy;
pub mod prompt;
pub mod provider;

pub use engine::*;
pub use error::{CoachError, Result};
pub use models::response::TokenUsage;
pub use models::*;
pub use policy::*;
pub use prompt::{CoachPrompt, PromptBuilder, PromptSanitizer};
pub use provider::*;
