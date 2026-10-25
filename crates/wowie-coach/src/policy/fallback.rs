//! Deterministic fallback engine for graceful degradation upon AI provider failures.
//!
//! Converts provider timeouts, network failures, HTTP errors, malformed JSON,
//! schema violations, and disallowed action candidates into safe, deterministic
//! `CoachResponse`s without leaking sensitive internal details.

use crate::error::CoachError;
use crate::models::action::ActionCandidate;
use crate::models::response::{
    CoachResponse, ProviderProvenance, RelevanceClassification, MAX_RATIONALE_LENGTH,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Categorized reason code for a deterministic fallback activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FallbackReason {
    Timeout,
    NetworkError,
    HttpError,
    MalformedJson,
    SchemaViolation,
    DisallowedAction,
    PromptInjection,
    ProviderUnavailable,
    CooldownThrottled,
    CoreError,
}

impl FallbackReason {
    /// Maps a strongly-typed `CoachError` to its corresponding `FallbackReason`.
    pub fn from_error(error: &CoachError) -> Self {
        match error {
            CoachError::Timeout { .. } => Self::Timeout,
            CoachError::NetworkError { .. } => Self::NetworkError,
            CoachError::HttpError { .. } => Self::HttpError,
            CoachError::MalformedJson { .. } => Self::MalformedJson,
            CoachError::SchemaValidation { .. } => Self::SchemaViolation,
            CoachError::DisallowedAction { .. } => Self::DisallowedAction,
            CoachError::PromptInjectionDetected { .. } => Self::PromptInjection,
            CoachError::ProviderUnavailable { .. } => Self::ProviderUnavailable,
            CoachError::CooldownThrottled { .. } => Self::CooldownThrottled,
            CoachError::Core(_) => Self::CoreError,
        }
    }

    /// Returns the static snake_case string representation.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Timeout => "timeout",
            Self::NetworkError => "network_error",
            Self::HttpError => "http_error",
            Self::MalformedJson => "malformed_json",
            Self::SchemaViolation => "schema_violation",
            Self::DisallowedAction => "disallowed_action",
            Self::PromptInjection => "prompt_injection",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::CooldownThrottled => "cooldown_throttled",
            Self::CoreError => "core_error",
        }
    }

    /// Indicates whether the fallback was triggered by a potential security anomaly.
    pub const fn is_security_relevant(&self) -> bool {
        matches!(self, Self::PromptInjection | Self::DisallowedAction)
    }
}

impl std::fmt::Display for FallbackReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Deterministic fallback engine for producing safe, sanitized, advisory `CoachResponse`s.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeterministicFallback {
    default_provider_name: String,
}

impl Default for DeterministicFallback {
    fn default() -> Self {
        Self::new()
    }
}

impl DeterministicFallback {
    /// Creates a new `DeterministicFallback` engine with default provider name "fallback".
    pub fn new() -> Self {
        Self {
            default_provider_name: "fallback".to_string(),
        }
    }

    /// Creates a fallback engine associated with a specific provider identifier.
    pub fn with_provider(provider_name: impl Into<String>) -> Self {
        Self {
            default_provider_name: provider_name.into(),
        }
    }

    /// Produces a safe fallback response for the given error and timestamp using default provider name.
    pub fn evaluate(&self, error: &CoachError, now: DateTime<Utc>) -> CoachResponse {
        Self::fallback_for_error(error, &self.default_provider_name, now)
    }

    /// Produces a safe fallback response for the given error and timestamp with custom provider name.
    pub fn evaluate_with_provider(
        &self,
        error: &CoachError,
        provider_name: &str,
        now: DateTime<Utc>,
    ) -> CoachResponse {
        Self::fallback_for_error(error, provider_name, now)
    }

    /// Static generator producing an immutable, sanitized, safe fallback `CoachResponse`.
    pub fn fallback_for_error(
        error: &CoachError,
        provider_name: &str,
        now: DateTime<Utc>,
    ) -> CoachResponse {
        let rationale = Self::sanitize_error_rationale(error);
        let provenance = ProviderProvenance::fallback_with_provider(provider_name, now);

        CoachResponse {
            relevance: RelevanceClassification::Unknown,
            confidence: 0.0,
            uncertainty: Some(1.0),
            rationale,
            question: None,
            action_candidate: Some(ActionCandidate::None),
            provenance,
        }
    }

    /// Sanitizes an error to produce a user-safe rationale that never leaks sensitive data.
    ///
    /// Specifically:
    /// - Discards internal URLs, hostnames, IP addresses, credentials, and error stack traces.
    /// - Drops unauthorized action names or shell injection attempts.
    /// - Strips prompt injection attack patterns.
    /// - Bounds length to `MAX_RATIONALE_LENGTH`.
    pub fn sanitize_error_rationale(error: &CoachError) -> String {
        let text = match error {
            CoachError::Timeout { duration_ms } => {
                format!("AI provider timed out after {duration_ms}ms. Neutral fallback applied without active intervention.")
            }
            CoachError::NetworkError { .. } => {
                "Network communication with AI provider failed. Neutral fallback applied without active intervention.".to_string()
            }
            CoachError::HttpError { status_code, .. } => {
                format!("AI provider returned HTTP status {status_code}. Neutral fallback applied without active intervention.")
            }
            CoachError::MalformedJson { .. } => {
                "AI provider returned an unparseable response payload. Neutral fallback applied without active intervention.".to_string()
            }
            CoachError::SchemaValidation { field, .. } => {
                format!("AI provider response failed schema validation for field '{field}'. Neutral fallback applied without active intervention.")
            }
            CoachError::DisallowedAction { .. } => {
                "AI provider suggested an unauthorized action outside the allowed allowlist. Neutral fallback applied with action candidate None.".to_string()
            }
            CoachError::PromptInjectionDetected { input_source, .. } => {
                format!("Potential prompt injection detected in {input_source}. Operation suppressed for safety.")
            }
            CoachError::ProviderUnavailable { provider, .. } => {
                format!("AI provider '{provider}' is currently unavailable. Neutral fallback applied without active intervention.")
            }
            CoachError::CooldownThrottled { remaining_seconds, .. } => {
                format!("Coaching intervention throttled by cooldown policy ({remaining_seconds}s remaining).")
            }
            CoachError::Core(_) => {
                "Internal domain error occurred. Neutral fallback applied without active intervention.".to_string()
            }
        };

        if text.chars().count() > MAX_RATIONALE_LENGTH {
            text.chars().take(MAX_RATIONALE_LENGTH).collect()
        } else {
            text
        }
    }
}
