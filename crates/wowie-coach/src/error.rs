//! Strongly-typed error taxonomy for the Wowie AI Coach crate.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Result alias for coach operations.
pub type Result<T> = std::result::Result<T, CoachError>;

/// Error taxonomy for domain operations, network transport, schema validation,
/// prompt sanitization, action authorization, and cooldown rate limits.
#[derive(Error, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "details", rename_all = "snake_case")]
pub enum CoachError {
    /// Provider request timed out before receiving a response.
    #[error("Coach provider request timed out after {duration_ms}ms")]
    Timeout { duration_ms: u64 },

    /// Low-level network or transport failure (DNS resolution, connection refused, connection reset).
    #[error("Network error communicating with coach provider: {message}")]
    NetworkError { message: String },

    /// Coach provider returned a non-2xx HTTP status code.
    #[error("Coach provider HTTP {status_code}: {message}")]
    HttpError { status_code: u16, message: String },

    /// Response payload from provider is not syntactically valid JSON.
    #[error("Malformed JSON in coach provider response: {error}")]
    MalformedJson {
        error: String,
        line: Option<usize>,
        column: Option<usize>,
    },

    /// JSON is valid, but violates schema constraints (missing field, bounds violation, bad length).
    #[error("Schema validation failed for field '{field}': {reason}")]
    SchemaValidation { field: String, reason: String },

    /// Model suggested an action candidate that is not in the closed allowlist.
    #[error("Disallowed action candidate '{attempted_action}': not in authorized allowlist")]
    DisallowedAction { attempted_action: String },

    /// Delimiter escape attempt or prompt injection signature detected.
    #[error("Prompt injection detected in '{input_source}': {pattern}")]
    PromptInjectionDetected {
        #[serde(rename = "source")]
        input_source: String,
        pattern: String,
    },

    /// Coaching intervention throttled by active cooldown, user snooze, or rate limit.
    #[error(
        "Coaching intervention throttled: cooldown active for {remaining_seconds}s ({reason})"
    )]
    CooldownThrottled {
        remaining_seconds: u64,
        reason: String,
    },

    /// Provider is offline, unconfigured, missing credentials, or resource-bounded.
    #[error("Coach provider '{provider}' is unavailable: {reason}")]
    ProviderUnavailable { provider: String, reason: String },

    /// Underlying core domain error.
    #[error("Core domain error: {0}")]
    Core(String),
}

impl CoachError {
    pub fn timeout(duration_ms: u64) -> Self {
        Self::Timeout { duration_ms }
    }

    pub fn network(message: impl Into<String>) -> Self {
        Self::NetworkError {
            message: message.into(),
        }
    }

    pub fn http(status_code: u16, message: impl Into<String>) -> Self {
        Self::HttpError {
            status_code,
            message: message.into(),
        }
    }

    pub fn malformed_json(
        error: impl Into<String>,
        line: Option<usize>,
        column: Option<usize>,
    ) -> Self {
        Self::MalformedJson {
            error: error.into(),
            line,
            column,
        }
    }

    pub fn schema_validation(field: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::SchemaValidation {
            field: field.into(),
            reason: reason.into(),
        }
    }

    pub fn disallowed_action(attempted_action: impl Into<String>) -> Self {
        Self::DisallowedAction {
            attempted_action: attempted_action.into(),
        }
    }

    pub fn prompt_injection(source: impl Into<String>, pattern: impl Into<String>) -> Self {
        Self::PromptInjectionDetected {
            input_source: source.into(),
            pattern: pattern.into(),
        }
    }

    pub fn cooldown_throttled(remaining_seconds: u64, reason: impl Into<String>) -> Self {
        Self::CooldownThrottled {
            remaining_seconds,
            reason: reason.into(),
        }
    }

    pub fn provider_unavailable(provider: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::ProviderUnavailable {
            provider: provider.into(),
            reason: reason.into(),
        }
    }

    /// Whether this error represents an adversarial prompt injection or authorization violation.
    pub fn is_security_relevant(&self) -> bool {
        matches!(
            self,
            Self::PromptInjectionDetected { .. } | Self::DisallowedAction { .. }
        )
    }

    /// Whether the error indicates an ephemeral condition that could be retried.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Timeout { .. }
                | Self::NetworkError { .. }
                | Self::HttpError {
                    status_code: 429 | 500..=504,
                    ..
                }
        )
    }

    /// Whether the operation should silently fall back to a safe deterministic default.
    pub fn should_fallback(&self) -> bool {
        !matches!(self, Self::CooldownThrottled { .. })
    }
}

impl From<wowie_core::CoreError> for CoachError {
    fn from(err: wowie_core::CoreError) -> Self {
        Self::Core(err.to_string())
    }
}
