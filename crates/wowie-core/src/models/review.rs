use crate::error::CoreError;
use crate::id::{ReviewId, SessionId};
use crate::models::session::{FocusSession, SessionState};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const MAX_REVIEW_NOTE_LENGTH: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionOutcome {
    CompletedSuccess,
    CompletedPartial,
    Cancelled,
    Abandoned,
}

impl fmt::Display for SessionOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CompletedSuccess => write!(f, "completed_success"),
            Self::CompletedPartial => write!(f, "completed_partial"),
            Self::Cancelled => write!(f, "cancelled"),
            Self::Abandoned => write!(f, "abandoned"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionReview {
    pub id: ReviewId,
    pub session_id: SessionId,
    pub actual_focus_seconds: u64,
    pub actual_break_seconds: u64,
    pub actual_idle_seconds: u64,
    pub outcome: SessionOutcome,
    pub user_notes: Option<String>,
    pub next_action_revision: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl SessionReview {
    pub fn from_session(
        session: &FocusSession,
        outcome: SessionOutcome,
        user_notes: Option<String>,
        next_action_revision: Option<String>,
        now: DateTime<Utc>,
    ) -> Result<Self, CoreError> {
        Self::with_id(
            ReviewId::new(),
            session,
            outcome,
            user_notes,
            next_action_revision,
            now,
        )
    }

    pub fn with_id(
        id: ReviewId,
        session: &FocusSession,
        outcome: SessionOutcome,
        user_notes: Option<String>,
        next_action_revision: Option<String>,
        now: DateTime<Utc>,
    ) -> Result<Self, CoreError> {
        if session.state != SessionState::Completed && session.state != SessionState::Interrupted {
            return Err(CoreError::validation(
                "session_state",
                "Session review can only be generated for Completed or Interrupted sessions",
            ));
        }

        let user_notes = user_notes.map(|n| n.trim().to_string());
        if let Some(ref n) = user_notes {
            if n.chars().count() > MAX_REVIEW_NOTE_LENGTH {
                return Err(CoreError::validation(
                    "user_notes",
                    format!(
                        "Review note exceeds maximum limit of {} characters",
                        MAX_REVIEW_NOTE_LENGTH
                    ),
                ));
            }
        }

        Ok(Self {
            id,
            session_id: session.id,
            actual_focus_seconds: session.accumulated_focus_seconds,
            actual_break_seconds: session.accumulated_break_seconds,
            actual_idle_seconds: session.accumulated_idle_seconds,
            outcome,
            user_notes,
            next_action_revision,
            created_at: now,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::CommitmentId;
    use crate::models::session::{ConsentSnapshot, SessionEndReason};

    fn dummy_consent() -> ConsentSnapshot {
        ConsentSnapshot {
            monitoring_enabled: true,
            window_title_redaction_enabled: true,
            browser_origin_enabled: false,
            granted_at: Utc::now(),
        }
    }

    #[test]
    fn test_session_review_creation() {
        let now = Utc::now();
        let mut session = FocusSession::new(CommitmentId::new(), dummy_consent(), now);

        // Active session should fail review generation
        session.start(now).unwrap();
        assert!(SessionReview::from_session(
            &session,
            SessionOutcome::CompletedSuccess,
            None,
            None,
            now,
        )
        .is_err());

        // Complete session
        let end_time = now + chrono::Duration::seconds(1200);
        session
            .complete(SessionEndReason::GoalAchieved, end_time)
            .unwrap();

        // Valid review
        let review = SessionReview::from_session(
            &session,
            SessionOutcome::CompletedSuccess,
            Some("Solid deep work session".into()),
            Some("Continue with part 2".into()),
            end_time,
        )
        .unwrap();

        assert_eq!(review.session_id, session.id);
        assert_eq!(review.actual_focus_seconds, 1200);
        assert_eq!(review.outcome, SessionOutcome::CompletedSuccess);
    }

    #[test]
    fn test_session_review_note_bounds() {
        let now = Utc::now();
        let mut session = FocusSession::new(CommitmentId::new(), dummy_consent(), now);
        session.start(now).unwrap();
        session
            .complete(SessionEndReason::TimeExpired, now)
            .unwrap();

        let long_note = "n".repeat(MAX_REVIEW_NOTE_LENGTH + 1);
        assert!(SessionReview::from_session(
            &session,
            SessionOutcome::CompletedPartial,
            Some(long_note),
            None,
            now,
        )
        .is_err());
    }
}
