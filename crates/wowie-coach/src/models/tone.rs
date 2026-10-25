//! Tone, personality mode, and presentation preferences for the AI Coach.
//!
//! Controls wording, phrasing, and conversational style.
//! Does NOT grant authority, alter cooldowns, or bypass deterministic rules.

use crate::error::CoachError;
use serde::{Deserialize, Serialize};
use std::fmt;

pub const DEFAULT_COACH_NAME: &str = "Wowie";
pub const MAX_COACH_NAME_LENGTH: usize = 64;
pub const MAX_CUSTOM_INSTRUCTIONS_LENGTH: usize = 512;

/// Discrete level for tone dimensions (directness, warmth, humor).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum ToneLevel {
    /// Minimal intensity / gentle / reserved.
    Low,
    /// Balanced / moderate intensity (standard default).
    #[default]
    Medium,
    /// High intensity / pronounced / assertive.
    High,
}

impl ToneLevel {
    /// Convert normalized float score [0.0, 1.0] to discrete ToneLevel.
    pub fn from_score(score: f32) -> Self {
        if score < 0.33 {
            Self::Low
        } else if score <= 0.67 {
            Self::Medium
        } else {
            Self::High
        }
    }

    /// Return representative float score [0.0, 1.0].
    pub fn as_score(&self) -> f32 {
        match self {
            Self::Low => 0.2,
            Self::Medium => 0.5,
            Self::High => 0.8,
        }
    }
}

impl fmt::Display for ToneLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
        }
    }
}

/// High-level personality style / persona for the AI Coach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PersonalityMode {
    /// Objective, nonjudgmental, neutral companion (system default).
    #[default]
    Neutral,
    /// Empathetic, encouraging, reinforcing small wins.
    Supportive,
    /// Action-oriented, minimal fluff, focused on next concrete step.
    Pragmatic,
    /// Gentle question-asker, prompts self-reflection to resolve blocks.
    Inquisitive,
    /// Lighthearted, cheerful, playful accountability.
    Playful,
}

impl fmt::Display for PersonalityMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Neutral => write!(f, "neutral"),
            Self::Supportive => write!(f, "supportive"),
            Self::Pragmatic => write!(f, "pragmatic"),
            Self::Inquisitive => write!(f, "inquisitive"),
            Self::Playful => write!(f, "playful"),
        }
    }
}

/// User tone and personality preferences for the AI Coach.
///
/// Controls conversational phrasing without altering safety rules,
/// cooldowns, or deterministic policy constraints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TonePreferences {
    /// Name used by the AI coach in self-reference.
    #[serde(default = "default_coach_name")]
    pub coach_name: String,

    /// Persona or mode of interaction.
    #[serde(default, alias = "persona")]
    pub mode: PersonalityMode,

    /// Directness of recommendations: Low (gentle/suggestive), Medium (clear/balanced), High (direct/concise).
    #[serde(default)]
    pub directness: ToneLevel,

    /// Warmth of dialogue: Low (cool/objective), Medium (friendly/respectful), High (warm/empathetic).
    #[serde(default)]
    pub warmth: ToneLevel,

    /// Humor and playfulness: Low (serious/no jokes), Medium (light wit), High (playful/quirky).
    #[serde(default)]
    pub humor: ToneLevel,

    /// Optional bounded custom instructions for wording style.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_instructions: Option<String>,
}

fn default_coach_name() -> String {
    DEFAULT_COACH_NAME.to_string()
}

impl Default for TonePreferences {
    fn default() -> Self {
        Self {
            coach_name: default_coach_name(),
            mode: PersonalityMode::Neutral,
            directness: ToneLevel::Medium,
            warmth: ToneLevel::Medium,
            humor: ToneLevel::Low,
            custom_instructions: None,
        }
    }
}

impl TonePreferences {
    pub fn new(coach_name: impl Into<String>, mode: PersonalityMode) -> Result<Self, CoachError> {
        let prefs = Self {
            coach_name: coach_name.into(),
            mode,
            ..Default::default()
        };
        prefs.validate()?;
        Ok(prefs)
    }

    pub fn neutral() -> Self {
        Self {
            coach_name: default_coach_name(),
            mode: PersonalityMode::Neutral,
            directness: ToneLevel::Medium,
            warmth: ToneLevel::Low,
            humor: ToneLevel::Low,
            custom_instructions: None,
        }
    }

    pub fn supportive() -> Self {
        Self {
            coach_name: default_coach_name(),
            mode: PersonalityMode::Supportive,
            directness: ToneLevel::Low,
            warmth: ToneLevel::High,
            humor: ToneLevel::Medium,
            custom_instructions: None,
        }
    }

    pub fn pragmatic() -> Self {
        Self {
            coach_name: default_coach_name(),
            mode: PersonalityMode::Pragmatic,
            directness: ToneLevel::High,
            warmth: ToneLevel::Low,
            humor: ToneLevel::Low,
            custom_instructions: None,
        }
    }

    pub fn validate(&self) -> Result<(), CoachError> {
        let trimmed_name = self.coach_name.trim();
        if trimmed_name.is_empty() {
            return Err(CoachError::schema_validation(
                "coach_name",
                "Coach name cannot be empty or whitespace-only",
            ));
        }
        if self.coach_name.chars().count() > MAX_COACH_NAME_LENGTH {
            return Err(CoachError::schema_validation(
                "coach_name",
                format!("Coach name exceeds maximum length of {MAX_COACH_NAME_LENGTH} characters"),
            ));
        }
        if self.coach_name.contains('<') || self.coach_name.contains('>') {
            return Err(CoachError::schema_validation(
                "coach_name",
                "Coach name cannot contain '<' or '>' characters",
            ));
        }
        if let Some(ref custom) = self.custom_instructions {
            if custom.chars().count() > MAX_CUSTOM_INSTRUCTIONS_LENGTH {
                return Err(CoachError::schema_validation(
                    "custom_instructions",
                    format!(
                        "Custom instructions exceed maximum length of {MAX_CUSTOM_INSTRUCTIONS_LENGTH} characters"
                    ),
                ));
            }
            if custom.contains("<untrusted_activity_evidence>")
                || custom.contains("</untrusted_activity_evidence>")
            {
                return Err(CoachError::prompt_injection(
                    "custom_instructions",
                    "Delimiter tag sequence detected in custom instructions",
                ));
            }
        }
        Ok(())
    }

    /// Convenient accessor alias for persona/mode.
    pub fn persona(&self) -> PersonalityMode {
        self.mode
    }
}
