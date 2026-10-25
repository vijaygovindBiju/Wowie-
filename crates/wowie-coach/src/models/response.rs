//! Response and classification data models for the AI Coach dialogue engine.

use crate::error::CoachError;
use crate::models::action::ActionCandidate;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use wowie_core::models::ClassificationLabel;

/// Maximum allowed length for coach rationale string (matching SQLite schema limit).
pub const MAX_RATIONALE_LENGTH: usize = 1024;

/// Maximum allowed length for optional clarifying question.
pub const MAX_QUESTION_LENGTH: usize = 512;

/// Maximum allowed length for provenance identifier strings.
pub const MAX_PROVENANCE_IDENTIFIER_LENGTH: usize = 64;

/// High-level relevance classification returned by the AI Coach.
///
/// Under conservative guidance, the AI evaluates activity as `Relevant`, `PossiblyUnrelated`,
/// or `Unknown`. The AI never issues a definitive `Unrelated` label on its own; that requires
/// deterministic rules or user confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelevanceClassification {
    Relevant,
    PossiblyUnrelated,
    Unknown,
}

impl RelevanceClassification {
    /// Returns the canonical snake_case string slice.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Relevant => "relevant",
            Self::PossiblyUnrelated => "possibly_unrelated",
            Self::Unknown => "unknown",
        }
    }

    /// Converts a core `ClassificationLabel` to `RelevanceClassification` in a safe, lossy manner.
    ///
    /// `ClassificationLabel::Unrelated` is mapped to `RelevanceClassification::PossiblyUnrelated`
    /// because the AI cannot autonomously enforce an Unrelated verdict.
    pub fn from_label_lossy(label: ClassificationLabel) -> Self {
        match label {
            ClassificationLabel::Relevant => Self::Relevant,
            ClassificationLabel::PossiblyUnrelated | ClassificationLabel::Unrelated => {
                Self::PossiblyUnrelated
            }
            ClassificationLabel::Unknown => Self::Unknown,
        }
    }
}

impl fmt::Display for RelevanceClassification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl From<RelevanceClassification> for ClassificationLabel {
    fn from(relevance: RelevanceClassification) -> Self {
        match relevance {
            RelevanceClassification::Relevant => Self::Relevant,
            RelevanceClassification::PossiblyUnrelated => Self::PossiblyUnrelated,
            RelevanceClassification::Unknown => Self::Unknown,
        }
    }
}

impl TryFrom<ClassificationLabel> for RelevanceClassification {
    type Error = CoachError;

    fn try_from(label: ClassificationLabel) -> Result<Self, Self::Error> {
        match label {
            ClassificationLabel::Relevant => Ok(Self::Relevant),
            ClassificationLabel::PossiblyUnrelated => Ok(Self::PossiblyUnrelated),
            ClassificationLabel::Unknown => Ok(Self::Unknown),
            ClassificationLabel::Unrelated => Err(CoachError::schema_validation(
                "relevance",
                "ClassificationLabel::Unrelated cannot be directly converted to RelevanceClassification (use from_label_lossy if intentional)",
            )),
        }
    }
}

/// Provenance metadata tracking the generation source of a coaching response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderProvenance {
    pub provider_name: String,
    pub model_name: String,
    pub version: String,
    pub generated_at: DateTime<Utc>,
    pub latency_ms: Option<u64>,
}

impl ProviderProvenance {
    pub fn new(
        provider_name: impl Into<String>,
        model_name: impl Into<String>,
        version: impl Into<String>,
        generated_at: DateTime<Utc>,
        latency_ms: Option<u64>,
    ) -> Result<Self, CoachError> {
        let prov = Self {
            provider_name: provider_name.into().trim().to_string(),
            model_name: model_name.into().trim().to_string(),
            version: version.into().trim().to_string(),
            generated_at,
            latency_ms,
        };
        prov.validate()?;
        Ok(prov)
    }

    pub fn validate(&self) -> Result<(), CoachError> {
        let provider_name = self.provider_name.trim();
        if provider_name.is_empty() {
            return Err(CoachError::schema_validation(
                "provider_name",
                "Provider name cannot be empty",
            ));
        }
        if provider_name.chars().count() > MAX_PROVENANCE_IDENTIFIER_LENGTH {
            return Err(CoachError::schema_validation(
                "provider_name",
                format!(
                    "Provider name exceeds max length of {} characters",
                    MAX_PROVENANCE_IDENTIFIER_LENGTH
                ),
            ));
        }

        let model_name = self.model_name.trim();
        if model_name.is_empty() {
            return Err(CoachError::schema_validation(
                "model_name",
                "Model name cannot be empty",
            ));
        }
        if model_name.chars().count() > MAX_PROVENANCE_IDENTIFIER_LENGTH {
            return Err(CoachError::schema_validation(
                "model_name",
                format!(
                    "Model name exceeds max length of {} characters",
                    MAX_PROVENANCE_IDENTIFIER_LENGTH
                ),
            ));
        }

        let version = self.version.trim();
        if version.is_empty() {
            return Err(CoachError::schema_validation(
                "version",
                "Version cannot be empty",
            ));
        }

        Ok(())
    }

    /// Convenience constructor for offline unit test mocks with given timestamp.
    pub fn mock(now: DateTime<Utc>) -> Self {
        Self {
            provider_name: "mock".to_string(),
            model_name: "mock-v1".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            generated_at: now,
            latency_ms: Some(1),
        }
    }

    /// Convenience constructor for offline unit test mocks with current timestamp.
    pub fn mock_default() -> Self {
        Self::mock(Utc::now())
    }

    /// Convenience constructor for deterministic fallbacks with given timestamp.
    pub fn fallback(now: DateTime<Utc>) -> Self {
        Self {
            provider_name: "fallback".to_string(),
            model_name: "deterministic-fallback".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            generated_at: now,
            latency_ms: Some(0),
        }
    }

    /// Convenience constructor for deterministic fallbacks with custom provider name.
    pub fn fallback_with_provider(provider: impl Into<String>, now: DateTime<Utc>) -> Self {
        Self {
            provider_name: provider.into(),
            model_name: "deterministic-fallback".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            generated_at: now,
            latency_ms: Some(0),
        }
    }
}

impl Default for ProviderProvenance {
    fn default() -> Self {
        Self::mock_default()
    }
}

/// Token consumption metrics reported by an LLM provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

impl TokenUsage {
    /// Creates a new `TokenUsage` metrics record.
    pub const fn new(prompt_tokens: u32, completion_tokens: u32, total_tokens: u32) -> Self {
        Self {
            prompt_tokens,
            completion_tokens,
            total_tokens,
        }
    }
}

/// Raw LLM response payload directly parsed from model generation (before provenance attachment).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoachRawResponse {
    pub relevance: RelevanceClassification,
    pub confidence: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncertainty: Option<f32>,
    pub rationale: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub question: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_candidate: Option<ActionCandidate>,

    // Provider execution metadata and telemetry (optional, serde default)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raw_json: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_usage: Option<TokenUsage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latency_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl Default for CoachRawResponse {
    fn default() -> Self {
        Self {
            relevance: RelevanceClassification::Unknown,
            confidence: 0.0,
            uncertainty: None,
            rationale: "Default raw response".to_string(),
            question: None,
            action_candidate: None,
            raw_text: None,
            raw_json: None,
            token_usage: None,
            latency_ms: None,
            model: None,
        }
    }
}

impl CoachRawResponse {
    /// Parses a JSON string (with optional markdown code fences) into a validated `CoachRawResponse`,
    /// discarding any unauthorized or extraneous fields.
    pub fn from_json(raw: &str) -> Result<Self, CoachError> {
        let cleaned = strip_markdown_code_fences(raw);
        let value: serde_json::Value = serde_json::from_str(cleaned).map_err(|e| {
            CoachError::malformed_json(e.to_string(), Some(e.line()), Some(e.column()))
        })?;

        let obj = value.as_object().ok_or_else(|| {
            CoachError::schema_validation("root", "Expected top-level JSON object")
        })?;

        // 1. Relevance
        let relevance_val = obj.get("relevance").ok_or_else(|| {
            CoachError::schema_validation("relevance", "Missing required field 'relevance'")
        })?;
        let relevance_str = relevance_val.as_str().ok_or_else(|| {
            CoachError::schema_validation("relevance", "Field 'relevance' must be a string")
        })?;
        let relevance = match relevance_str {
            "relevant" => RelevanceClassification::Relevant,
            "possibly_unrelated" => RelevanceClassification::PossiblyUnrelated,
            "unknown" => RelevanceClassification::Unknown,
            other => {
                return Err(CoachError::schema_validation(
                    "relevance",
                    format!("Unrecognized relevance variant: '{other}'"),
                ));
            }
        };

        // 2. Confidence
        let confidence_val = obj.get("confidence").ok_or_else(|| {
            CoachError::schema_validation("confidence", "Missing required field 'confidence'")
        })?;
        let confidence = confidence_val.as_f64().ok_or_else(|| {
            CoachError::schema_validation("confidence", "Field 'confidence' must be a number")
        })? as f32;
        if !confidence.is_finite() || !(0.0..=1.0).contains(&confidence) {
            return Err(CoachError::schema_validation(
                "confidence",
                format!("Confidence score {confidence} must be in range [0.0, 1.0]"),
            ));
        }

        // 3. Uncertainty
        let uncertainty = match obj.get("uncertainty") {
            None | Some(serde_json::Value::Null) => None,
            Some(v) => {
                let u = v.as_f64().ok_or_else(|| {
                    CoachError::schema_validation(
                        "uncertainty",
                        "Field 'uncertainty' must be a number",
                    )
                })? as f32;
                if !u.is_finite() || !(0.0..=1.0).contains(&u) {
                    return Err(CoachError::schema_validation(
                        "uncertainty",
                        format!("Uncertainty score {u} must be in range [0.0, 1.0]"),
                    ));
                }
                Some(u)
            }
        };

        // 4. Rationale
        let rationale_val = obj.get("rationale").ok_or_else(|| {
            CoachError::schema_validation("rationale", "Missing required field 'rationale'")
        })?;
        let rationale_str = rationale_val.as_str().ok_or_else(|| {
            CoachError::schema_validation("rationale", "Field 'rationale' must be a string")
        })?;
        let trimmed_rationale = rationale_str.trim();
        if trimmed_rationale.is_empty() {
            return Err(CoachError::schema_validation(
                "rationale",
                "Rationale cannot be empty or whitespace-only",
            ));
        }
        if trimmed_rationale.chars().count() > MAX_RATIONALE_LENGTH {
            return Err(CoachError::schema_validation(
                "rationale",
                format!(
                    "Rationale length {} exceeds maximum allowed limit of {} characters",
                    trimmed_rationale.chars().count(),
                    MAX_RATIONALE_LENGTH
                ),
            ));
        }

        // 5. Question
        let question = match obj.get("question") {
            None | Some(serde_json::Value::Null) => None,
            Some(v) => {
                let q_str = v.as_str().ok_or_else(|| {
                    CoachError::schema_validation("question", "Field 'question' must be a string")
                })?;
                let q_trimmed = q_str.trim();
                if q_trimmed.is_empty() {
                    return Err(CoachError::schema_validation(
                        "question",
                        "Question, if present, cannot be empty or whitespace",
                    ));
                }
                if q_trimmed.chars().count() > MAX_QUESTION_LENGTH {
                    return Err(CoachError::schema_validation(
                        "question",
                        format!(
                            "Question length {} exceeds maximum allowed limit of {} characters",
                            q_trimmed.chars().count(),
                            MAX_QUESTION_LENGTH
                        ),
                    ));
                }
                Some(q_trimmed.to_string())
            }
        };

        // 6. Action candidate (closed allowlist)
        let action_candidate = match obj.get("action_candidate") {
            None | Some(serde_json::Value::Null) => None,
            Some(v) => {
                let act_str = v.as_str().ok_or_else(|| {
                    CoachError::schema_validation(
                        "action_candidate",
                        "Field 'action_candidate' must be a string",
                    )
                })?;
                match act_str {
                    "none" => Some(ActionCandidate::None),
                    "remind" => Some(ActionCandidate::Remind),
                    "ask_context" => Some(ActionCandidate::AskContext),
                    "suggest_break" => Some(ActionCandidate::SuggestBreak),
                    "suggest_correction" => Some(ActionCandidate::SuggestCorrection),
                    disallowed => {
                        return Err(CoachError::disallowed_action(disallowed));
                    }
                }
            }
        };

        // All unauthorized or unknown extra fields are discarded safely without error.

        Ok(Self {
            relevance,
            confidence,
            uncertainty,
            rationale: trimmed_rationale.to_string(),
            question,
            action_candidate,
            raw_text: None,
            raw_json: None,
            token_usage: None,
            latency_ms: None,
            model: None,
        })
    }

    /// Attaches provider execution telemetry to a parsed response.
    #[must_use]
    pub fn with_telemetry(
        mut self,
        raw_text: Option<String>,
        raw_json: Option<serde_json::Value>,
        token_usage: Option<TokenUsage>,
        latency_ms: Option<u64>,
        model: Option<String>,
    ) -> Self {
        self.raw_text = raw_text;
        self.raw_json = raw_json;
        self.token_usage = token_usage;
        self.latency_ms = latency_ms;
        self.model = model;
        self
    }

    /// Validates all fields against contract bounds.
    pub fn validate(&self) -> Result<(), CoachError> {
        if !self.confidence.is_finite() || !(0.0..=1.0).contains(&self.confidence) {
            return Err(CoachError::schema_validation(
                "confidence",
                format!(
                    "Confidence {} must be a finite float in [0.0, 1.0]",
                    self.confidence
                ),
            ));
        }

        if let Some(uncertainty) = self.uncertainty {
            if !uncertainty.is_finite() || !(0.0..=1.0).contains(&uncertainty) {
                return Err(CoachError::schema_validation(
                    "uncertainty",
                    format!(
                        "Uncertainty {} must be a finite float in [0.0, 1.0]",
                        uncertainty
                    ),
                ));
            }
        }

        let rationale_trimmed = self.rationale.trim();
        if rationale_trimmed.is_empty() {
            return Err(CoachError::schema_validation(
                "rationale",
                "Rationale cannot be empty or whitespace-only",
            ));
        }
        if rationale_trimmed.chars().count() > MAX_RATIONALE_LENGTH {
            return Err(CoachError::schema_validation(
                "rationale",
                format!(
                    "Rationale length {} exceeds maximum allowed limit of {} characters",
                    rationale_trimmed.chars().count(),
                    MAX_RATIONALE_LENGTH
                ),
            ));
        }

        if let Some(ref q) = self.question {
            let q_trimmed = q.trim();
            if q_trimmed.is_empty() {
                return Err(CoachError::schema_validation(
                    "question",
                    "Question, if present, cannot be empty or whitespace",
                ));
            }
            if q_trimmed.chars().count() > MAX_QUESTION_LENGTH {
                return Err(CoachError::schema_validation(
                    "question",
                    format!(
                        "Question length {} exceeds maximum allowed limit of {} characters",
                        q_trimmed.chars().count(),
                        MAX_QUESTION_LENGTH
                    ),
                ));
            }
        }

        Ok(())
    }

    /// Attaches provenance to produce a complete `CoachResponse`.
    pub fn into_validated_response(
        self,
        provenance: ProviderProvenance,
    ) -> Result<CoachResponse, CoachError> {
        CoachResponse::from_raw(self, provenance)
    }
}

/// Fully validated coaching response contract with provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "CoachResponseUnchecked")]
pub struct CoachResponse {
    pub relevance: RelevanceClassification,
    pub confidence: f32,
    pub uncertainty: Option<f32>,
    pub rationale: String,
    pub question: Option<String>,
    pub action_candidate: Option<ActionCandidate>,
    pub provenance: ProviderProvenance,
}

impl CoachResponse {
    /// Constructs and validates a new `CoachResponse`.
    pub fn new(
        relevance: RelevanceClassification,
        confidence: f32,
        uncertainty: Option<f32>,
        rationale: impl Into<String>,
        question: Option<String>,
        action_candidate: Option<ActionCandidate>,
        provenance: ProviderProvenance,
    ) -> Result<Self, CoachError> {
        let raw = CoachRawResponse {
            relevance,
            confidence,
            uncertainty,
            rationale: rationale.into(),
            question,
            action_candidate,
            ..Default::default()
        };
        raw.validate()?;
        provenance.validate()?;

        let rationale = raw.rationale.trim().to_string();
        let question = raw.question.map(|q| q.trim().to_string());

        Ok(Self {
            relevance,
            confidence: raw.confidence,
            uncertainty: raw.uncertainty,
            rationale,
            question,
            action_candidate: raw.action_candidate,
            provenance,
        })
    }

    /// Assembles a `CoachResponse` from a validated raw response and provenance.
    pub fn from_raw(
        raw: CoachRawResponse,
        provenance: ProviderProvenance,
    ) -> Result<Self, CoachError> {
        raw.validate()?;
        provenance.validate()?;

        let rationale = raw.rationale.trim().to_string();
        let question = raw.question.map(|q| q.trim().to_string());

        Ok(Self {
            relevance: raw.relevance,
            confidence: raw.confidence,
            uncertainty: raw.uncertainty,
            rationale,
            question,
            action_candidate: raw.action_candidate,
            provenance,
        })
    }

    /// Parses raw JSON from an LLM provider and validates all invariants, attaching provenance.
    pub fn parse_and_validate(
        raw_json: &str,
        provenance: ProviderProvenance,
    ) -> Result<Self, CoachError> {
        let raw = CoachRawResponse::from_json(raw_json)?;
        Self::from_raw(raw, provenance)
    }

    /// Generates a safe deterministic fallback response when provider calls fail or time out.
    pub fn fallback(now: DateTime<Utc>, reason: impl Into<String>) -> Self {
        Self {
            relevance: RelevanceClassification::Unknown,
            confidence: 0.0,
            uncertainty: Some(1.0),
            rationale: reason.into(),
            question: None,
            action_candidate: Some(ActionCandidate::None),
            provenance: ProviderProvenance::fallback(now),
        }
    }

    /// Safe deterministic fallback triggered by a `CoachError` and provider identifier.
    pub fn safe_fallback(error: &CoachError, provider_name: &str) -> Self {
        let now = Utc::now();
        let mut resp = Self::fallback(
            now,
            format!("Safe deterministic fallback triggered: {error}"),
        );
        resp.provenance.provider_name = provider_name.to_string();
        resp
    }

    /// Returns the effective action candidate, resolving `None` or omitted to `ActionCandidate::None`.
    pub fn effective_action(&self) -> ActionCandidate {
        self.action_candidate.unwrap_or(ActionCandidate::None)
    }

    /// Returns `true` if an active action candidate (other than `None`) is recommended.
    pub fn has_action(&self) -> bool {
        matches!(self.action_candidate, Some(action) if !action.is_none())
    }

    /// Returns `true` if relevance is `Relevant`.
    pub fn is_relevant(&self) -> bool {
        matches!(self.relevance, RelevanceClassification::Relevant)
    }

    /// Returns `true` if relevance is `PossiblyUnrelated`.
    pub fn is_possibly_unrelated(&self) -> bool {
        matches!(self.relevance, RelevanceClassification::PossiblyUnrelated)
    }

    /// Returns `true` if relevance is `Unknown`.
    pub fn is_unknown(&self) -> bool {
        matches!(self.relevance, RelevanceClassification::Unknown)
    }
}

/// Helper struct for validating `CoachResponse` during direct JSON deserialization.
#[derive(Deserialize)]
struct CoachResponseUnchecked {
    pub relevance: RelevanceClassification,
    pub confidence: f32,
    #[serde(default)]
    pub uncertainty: Option<f32>,
    pub rationale: String,
    #[serde(default)]
    pub question: Option<String>,
    #[serde(default)]
    pub action_candidate: Option<ActionCandidate>,
    pub provenance: ProviderProvenance,
}

impl TryFrom<CoachResponseUnchecked> for CoachResponse {
    type Error = CoachError;

    fn try_from(raw: CoachResponseUnchecked) -> Result<Self, Self::Error> {
        Self::new(
            raw.relevance,
            raw.confidence,
            raw.uncertainty,
            raw.rationale,
            raw.question,
            raw.action_candidate,
            raw.provenance,
        )
    }
}

/// Utility function to strip markdown code fences from LLM responses.
fn strip_markdown_code_fences(input: &str) -> &str {
    let mut s = input.trim();
    if s.starts_with("```") {
        if let Some(newline_idx) = s.find('\n') {
            s = &s[newline_idx + 1..];
        } else {
            return "";
        }
    }
    if s.ends_with("```") {
        if let Some(fence_idx) = s.rfind("```") {
            s = &s[..fence_idx];
        }
    }
    s.trim()
}
