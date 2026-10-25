//! Context data models for the AI Coach dialogue engine.
//!
//! Encapsulates user goals, session commitment parameters, untrusted desktop observations,
//! past user classification corrections, and tone preferences.

use crate::error::CoachError;
use crate::models::tone::TonePreferences;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use wowie_core::id::{CommitmentId, CorrectionId, GoalId, ObservationId, SessionId};
use wowie_core::models::goal::{MAX_GOAL_TITLE_LENGTH, MAX_NEXT_ACTION_LENGTH};
use wowie_core::models::observation::{
    MAX_APP_ID_LENGTH, MAX_ORIGIN_LENGTH, MAX_WINDOW_TITLE_LENGTH,
};
use wowie_core::models::{
    ClassificationLabel, Commitment, CorrectionScope, FocusSession, IdleState, Observation,
    ObservationSource, SessionState, UserCorrection,
};

/// Maximum allowed length for user correction notes in Coach context.
pub const MAX_CORRECTION_NOTE_LENGTH: usize = 2048;

/// Minimum and maximum bounds for session duration (mirroring wowie-core).
pub const MIN_SESSION_DURATION_SECONDS: u32 = 300;
pub const MAX_SESSION_DURATION_SECONDS: u32 = 43200;
pub const MAX_BREAK_ALLOWANCE_SECONDS: u32 = 7200;

/// Untrusted desktop observation data passed into the Coach dialogue engine.
///
/// Fields like `window_title` and `origin` represent raw, untrusted user/browser data
/// and must never be treated as system instructions. Delimiter isolation must be applied
/// by the prompt builder before sending to any LLM provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UntrustedObservation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation_id: Option<ObservationId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<ObservationSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idle_state: Option<IdleState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_at: Option<DateTime<Utc>>,
}

impl UntrustedObservation {
    /// Create a new untrusted observation with length bounds validation.
    pub fn new(
        app_id: Option<String>,
        window_title: Option<String>,
        origin: Option<String>,
        idle_state: Option<IdleState>,
    ) -> Result<Self, CoachError> {
        let app_id = app_id.map(|s| s.trim().to_string());
        if let Some(ref a) = app_id {
            if a.chars().count() > MAX_APP_ID_LENGTH {
                return Err(CoachError::schema_validation(
                    "app_id",
                    format!(
                        "app_id exceeds maximum length of {} characters",
                        MAX_APP_ID_LENGTH
                    ),
                ));
            }
        }

        let window_title = window_title.map(|s| s.trim().to_string());
        if let Some(ref w) = window_title {
            if w.chars().count() > MAX_WINDOW_TITLE_LENGTH {
                return Err(CoachError::schema_validation(
                    "window_title",
                    format!(
                        "window_title exceeds maximum length of {} characters",
                        MAX_WINDOW_TITLE_LENGTH
                    ),
                ));
            }
        }

        let origin = origin.map(|s| s.trim().to_string());
        if let Some(ref o) = origin {
            if o.chars().count() > MAX_ORIGIN_LENGTH {
                return Err(CoachError::schema_validation(
                    "origin",
                    format!(
                        "origin exceeds maximum length of {} characters",
                        MAX_ORIGIN_LENGTH
                    ),
                ));
            }
        }

        Ok(Self {
            observation_id: None,
            app_id,
            window_title,
            origin,
            source: Some(ObservationSource::WindowTitle),
            idle_state,
            observed_at: Some(Utc::now()),
        })
    }

    /// Convenience helper to create an observation from active window title and app name.
    pub fn from_window(
        app_id: impl Into<String>,
        window_title: impl Into<String>,
    ) -> Result<Self, CoachError> {
        Self::new(
            Some(app_id.into()),
            Some(window_title.into()),
            None,
            Some(IdleState::Active),
        )
    }

    /// Check if the observation contains any activity evidence.
    pub fn is_empty(&self) -> bool {
        self.app_id.is_none() && self.window_title.is_none() && self.origin.is_none()
    }

    /// Get effective idle state, defaulting to Active if not specified.
    pub fn effective_idle_state(&self) -> IdleState {
        self.idle_state.unwrap_or(IdleState::Active)
    }
}

impl Default for UntrustedObservation {
    fn default() -> Self {
        Self {
            observation_id: None,
            app_id: None,
            window_title: None,
            origin: None,
            source: None,
            idle_state: Some(IdleState::Active),
            observed_at: None,
        }
    }
}

impl From<&Observation> for UntrustedObservation {
    fn from(obs: &Observation) -> Self {
        Self {
            observation_id: Some(obs.id),
            app_id: obs.app_id.clone(),
            window_title: obs.window_title.clone(),
            origin: obs.origin.clone(),
            source: Some(obs.source),
            idle_state: Some(obs.idle_state),
            observed_at: Some(obs.observed_at),
        }
    }
}

impl From<Observation> for UntrustedObservation {
    fn from(obs: Observation) -> Self {
        Self {
            observation_id: Some(obs.id),
            app_id: obs.app_id,
            window_title: obs.window_title,
            origin: obs.origin,
            source: Some(obs.source),
            idle_state: Some(obs.idle_state),
            observed_at: Some(obs.observed_at),
        }
    }
}

/// Snapshot of the active focus session and commitment parameters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionCommitmentContext {
    pub session_id: SessionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commitment_id: Option<CommitmentId>,
    pub planned_duration_seconds: u32,
    pub elapsed_seconds: u32,
    pub is_break_state: bool,
    pub break_allowance_seconds: u32,
    pub break_taken_seconds: u32,
    #[serde(default = "default_session_state")]
    pub session_state: SessionState,
}

fn default_session_state() -> SessionState {
    SessionState::Active
}

impl SessionCommitmentContext {
    /// Create a fully-specified SessionCommitmentContext with domain invariant checks.
    pub fn new(
        session_id: SessionId,
        commitment_id: Option<CommitmentId>,
        planned_duration_seconds: u32,
        elapsed_seconds: u32,
        is_break_state: bool,
        break_allowance_seconds: u32,
        break_taken_seconds: u32,
        session_state: SessionState,
    ) -> Result<Self, CoachError> {
        if planned_duration_seconds < MIN_SESSION_DURATION_SECONDS {
            return Err(CoachError::schema_validation(
                "planned_duration_seconds",
                format!(
                    "Planned duration must be at least {} seconds",
                    MIN_SESSION_DURATION_SECONDS
                ),
            ));
        }
        if planned_duration_seconds > MAX_SESSION_DURATION_SECONDS {
            return Err(CoachError::schema_validation(
                "planned_duration_seconds",
                format!(
                    "Planned duration cannot exceed {} seconds",
                    MAX_SESSION_DURATION_SECONDS
                ),
            ));
        }
        if break_allowance_seconds > MAX_BREAK_ALLOWANCE_SECONDS {
            return Err(CoachError::schema_validation(
                "break_allowance_seconds",
                format!(
                    "Break allowance cannot exceed {} seconds",
                    MAX_BREAK_ALLOWANCE_SECONDS
                ),
            ));
        }

        Ok(Self {
            session_id,
            commitment_id,
            planned_duration_seconds,
            elapsed_seconds,
            is_break_state,
            break_allowance_seconds,
            break_taken_seconds,
            session_state,
        })
    }

    /// Convenience helper for quick creation during testing or minimal telemetry.
    pub fn simple(session_id: SessionId, elapsed_seconds: u32, is_break_state: bool) -> Self {
        Self {
            session_id,
            commitment_id: None,
            planned_duration_seconds: 1500, // 25 min default
            elapsed_seconds,
            is_break_state,
            break_allowance_seconds: 300, // 5 min default
            break_taken_seconds: 0,
            session_state: if is_break_state {
                SessionState::Break
            } else {
                SessionState::Active
            },
        }
    }

    /// Construct from wowie-core FocusSession and Commitment models.
    pub fn from_session_and_commitment(session: &FocusSession, commitment: &Commitment) -> Self {
        Self {
            session_id: session.id,
            commitment_id: Some(commitment.id),
            planned_duration_seconds: commitment.duration_seconds,
            elapsed_seconds: session.accumulated_focus_seconds as u32,
            break_allowance_seconds: commitment.break_allowance_seconds,
            break_taken_seconds: session.accumulated_break_seconds as u32,
            is_break_state: matches!(session.state, SessionState::Break),
            session_state: session.state,
        }
    }
}

/// Summary of a past user classification correction relevant to the current session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserCorrectionSummary {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<CorrectionId>,
    pub label: ClassificationLabel,
    pub scope: CorrectionScope,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_title_pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
}

impl UserCorrectionSummary {
    pub fn new(
        label: ClassificationLabel,
        scope: CorrectionScope,
        app_id: Option<String>,
        origin: Option<String>,
        note: Option<String>,
    ) -> Result<Self, CoachError> {
        if app_id.is_none() && origin.is_none() {
            return Err(CoachError::schema_validation(
                "target",
                "User correction summary must specify at least one of app_id or origin",
            ));
        }
        if let Some(ref n) = note {
            if n.chars().count() > MAX_CORRECTION_NOTE_LENGTH {
                return Err(CoachError::schema_validation(
                    "note",
                    format!(
                        "Correction note exceeds maximum length of {} characters",
                        MAX_CORRECTION_NOTE_LENGTH
                    ),
                ));
            }
        }

        Ok(Self {
            id: None,
            label,
            scope,
            app_id,
            origin,
            window_title_pattern: None,
            note,
            created_at: Some(Utc::now()),
        })
    }
}

impl From<&UserCorrection> for UserCorrectionSummary {
    fn from(c: &UserCorrection) -> Self {
        Self {
            id: Some(c.id),
            label: c.label,
            scope: c.scope,
            app_id: c.app_id.clone(),
            origin: c.origin.clone(),
            window_title_pattern: None,
            note: c.note.clone(),
            created_at: Some(c.created_at),
        }
    }
}

impl From<UserCorrection> for UserCorrectionSummary {
    fn from(c: UserCorrection) -> Self {
        Self {
            id: Some(c.id),
            label: c.label,
            scope: c.scope,
            app_id: c.app_id,
            origin: c.origin,
            window_title_pattern: None,
            note: c.note,
            created_at: Some(c.created_at),
        }
    }
}

/// Complete input payload supplied to the AI Coach engine for dialogue generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoachContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal_id: Option<GoalId>,
    pub goal_title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_action: Option<String>,
    pub session_id: SessionId,
    pub elapsed_seconds: u32,
    pub is_break_state: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_commitment: Option<SessionCommitmentContext>,
    pub observation: UntrustedObservation,
    #[serde(default)]
    pub user_corrections: Vec<UserCorrectionSummary>,
    #[serde(default)]
    pub tone: TonePreferences,
}

impl CoachContext {
    /// Create a validated CoachContext with standard session parameters.
    pub fn new(
        goal_title: impl Into<String>,
        next_action: Option<String>,
        session_id: SessionId,
        elapsed_seconds: u32,
        is_break_state: bool,
        observation: UntrustedObservation,
        user_corrections: Vec<UserCorrectionSummary>,
        tone: TonePreferences,
    ) -> Result<Self, CoachError> {
        let goal_title = goal_title.into().trim().to_string();
        if goal_title.is_empty() {
            return Err(CoachError::schema_validation(
                "goal_title",
                "Goal title cannot be empty",
            ));
        }
        if goal_title.chars().count() > MAX_GOAL_TITLE_LENGTH {
            return Err(CoachError::schema_validation(
                "goal_title",
                format!(
                    "Goal title exceeds maximum length of {} characters",
                    MAX_GOAL_TITLE_LENGTH
                ),
            ));
        }

        let next_action = next_action.map(|a| a.trim().to_string());
        if let Some(ref a) = next_action {
            if a.chars().count() > MAX_NEXT_ACTION_LENGTH {
                return Err(CoachError::schema_validation(
                    "next_action",
                    format!(
                        "Next action exceeds maximum length of {} characters",
                        MAX_NEXT_ACTION_LENGTH
                    ),
                ));
            }
        }

        Ok(Self {
            goal_id: None,
            goal_title,
            next_action,
            session_id,
            elapsed_seconds,
            is_break_state,
            session_commitment: None,
            observation,
            user_corrections,
            tone,
        })
    }

    /// Construct CoachContext attaching a full SessionCommitmentContext.
    pub fn with_session_commitment(
        goal_title: impl Into<String>,
        next_action: Option<String>,
        session: SessionCommitmentContext,
        observation: UntrustedObservation,
        user_corrections: Vec<UserCorrectionSummary>,
        tone: TonePreferences,
    ) -> Result<Self, CoachError> {
        let mut ctx = Self::new(
            goal_title,
            next_action,
            session.session_id,
            session.elapsed_seconds,
            session.is_break_state,
            observation,
            user_corrections,
            tone,
        )?;
        ctx.session_commitment = Some(session);
        Ok(ctx)
    }

    /// Construct CoachContext directly from wowie-core entities.
    pub fn from_core_entities(
        goal: &wowie_core::models::Goal,
        session: &wowie_core::models::FocusSession,
        commitment: &wowie_core::models::Commitment,
        observation: UntrustedObservation,
        user_corrections: Vec<UserCorrectionSummary>,
        tone: TonePreferences,
    ) -> Result<Self, CoachError> {
        let commitment_ctx =
            SessionCommitmentContext::from_session_and_commitment(session, commitment);
        let mut ctx = Self::with_session_commitment(
            &goal.title,
            goal.next_action.clone(),
            commitment_ctx,
            observation,
            user_corrections,
            tone,
        )?;
        ctx.goal_id = Some(goal.id);
        Ok(ctx)
    }

    /// Retrieve full session commitment context, falling back to minimal telemetry if not explicitly supplied.
    pub fn session_commitment_context(&self) -> SessionCommitmentContext {
        self.session_commitment.clone().unwrap_or_else(|| {
            SessionCommitmentContext::simple(
                self.session_id,
                self.elapsed_seconds,
                self.is_break_state,
            )
        })
    }
}
