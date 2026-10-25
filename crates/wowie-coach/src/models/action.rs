//! Closed allowlist of coaching action recommendations.

use crate::error::CoachError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// All valid action candidates in the closed allowlist.
pub const ALLOWED_ACTIONS: &[ActionCandidate] = &[
    ActionCandidate::None,
    ActionCandidate::Remind,
    ActionCandidate::AskContext,
    ActionCandidate::SuggestBreak,
    ActionCandidate::SuggestCorrection,
];

/// Closed allowlist of coaching action recommendations.
///
/// Under zero-OS authority and security boundaries, the AI coach can ONLY recommend
/// an action from this strictly bounded enum. Any unrecognized, external, or unauthorized
/// action is rejected during deserialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionCandidate {
    /// No proactive intervention recommended; remain silent.
    None,
    /// Gentle reminder of current goal and planned next action.
    Remind,
    /// Clarifying question asking user for context regarding current activity.
    AskContext,
    /// Suggest taking a planned break or stepping away briefly.
    SuggestBreak,
    /// Suggest submitting an activity correction (e.g. marking a tutorial on-goal).
    SuggestCorrection,
}

impl ActionCandidate {
    /// Returns the canonical snake_case string representation.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Remind => "remind",
            Self::AskContext => "ask_context",
            Self::SuggestBreak => "suggest_break",
            Self::SuggestCorrection => "suggest_correction",
        }
    }

    /// Returns `true` if the candidate represents `None` (no action).
    pub const fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Returns `true` if the candidate requires user dialogue or interaction.
    pub const fn is_interactive(&self) -> bool {
        matches!(self, Self::AskContext | Self::SuggestCorrection)
    }

    /// Returns all allowed action candidates in the closed allowlist.
    pub const fn allowed_variants() -> &'static [Self] {
        ALLOWED_ACTIONS
    }
}

impl fmt::Display for ActionCandidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for ActionCandidate {
    type Err = CoachError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "none" => Ok(Self::None),
            "remind" => Ok(Self::Remind),
            "ask_context" => Ok(Self::AskContext),
            "suggest_break" => Ok(Self::SuggestBreak),
            "suggest_correction" => Ok(Self::SuggestCorrection),
            disallowed => Err(CoachError::disallowed_action(disallowed)),
        }
    }
}
