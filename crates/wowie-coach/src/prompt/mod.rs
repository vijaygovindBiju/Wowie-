//! Prompt engineering, delimiter isolation, and sanitization for Wowie AI Coach.
//!
//! Provides defense-in-depth against prompt injection through XML delimiter isolation,
//! entity escaping, control character stripping, and structured meta-instructions.

pub mod builder;
pub mod sanitizer;

pub use builder::{CoachPrompt, PromptBuilder};
pub use sanitizer::PromptSanitizer;
