//! Asynchronous LLM provider trait for the Wowie AI Coach dialogue engine.

use crate::error::CoachError;
use crate::models::response::CoachRawResponse;
use crate::prompt::CoachPrompt;

/// Asynchronous LLM provider abstraction for generating coaching assessments.
///
/// Implemented by offline test fixtures (`MockCoachProvider`) and HTTP clients (`OpenAiCompatibleClient`).
/// Object-safe via `#[async_trait::async_trait]` to support dynamic dispatch (`Arc<dyn CoachProvider>`).
#[async_trait::async_trait]
pub trait CoachProvider: Send + Sync + std::fmt::Debug {
    /// Canonical static identifier for this provider (e.g., "mock", "openai_compatible", "ollama").
    fn name(&self) -> &'static str;

    /// Evaluates the coach prompt and produces a raw coaching response payload.
    ///
    /// # Errors
    /// Returns `CoachError` on request timeout, transport disconnection, HTTP errors,
    /// malformed JSON, schema bounds violations, or unauthorized action candidates.
    async fn evaluate(&self, prompt: &CoachPrompt) -> Result<CoachRawResponse, CoachError>;

    /// Model identifier configured on this provider.
    fn model_name(&self) -> &str {
        static DEFAULT: &str = "default";
        DEFAULT
    }

    /// Health check or availability probe. Default implementation returns `true`.
    async fn is_available(&self) -> bool {
        true
    }
}
