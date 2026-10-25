use crate::error::CoreError;
use crate::id::{CommitmentId, GoalId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub const MIN_SESSION_DURATION_SECONDS: u32 = 300; // 5 minutes
pub const MAX_SESSION_DURATION_SECONDS: u32 = 43200; // 12 hours
pub const MAX_BREAK_ALLOWANCE_SECONDS: u32 = 7200; // 2 hours

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Commitment {
    pub id: CommitmentId,
    pub goal_id: GoalId,
    pub planned_start: DateTime<Utc>,
    pub duration_seconds: u32,
    pub break_allowance_seconds: u32,
    pub monitoring_profile_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl Commitment {
    pub fn new(
        goal_id: GoalId,
        planned_start: DateTime<Utc>,
        duration_seconds: u32,
        break_allowance_seconds: u32,
        monitoring_profile_id: Option<String>,
        now: DateTime<Utc>,
    ) -> Result<Self, CoreError> {
        Self::with_id(
            CommitmentId::new(),
            goal_id,
            planned_start,
            duration_seconds,
            break_allowance_seconds,
            monitoring_profile_id,
            now,
        )
    }

    pub fn with_id(
        id: CommitmentId,
        goal_id: GoalId,
        planned_start: DateTime<Utc>,
        duration_seconds: u32,
        break_allowance_seconds: u32,
        monitoring_profile_id: Option<String>,
        now: DateTime<Utc>,
    ) -> Result<Self, CoreError> {
        if duration_seconds < MIN_SESSION_DURATION_SECONDS {
            return Err(CoreError::validation(
                "duration_seconds",
                format!(
                    "Planned duration must be at least {} seconds (5 minutes)",
                    MIN_SESSION_DURATION_SECONDS
                ),
            ));
        }
        if duration_seconds > MAX_SESSION_DURATION_SECONDS {
            return Err(CoreError::DurationLimitExceeded {
                limit: MAX_SESSION_DURATION_SECONDS,
                requested: duration_seconds,
            });
        }
        if break_allowance_seconds > MAX_BREAK_ALLOWANCE_SECONDS {
            return Err(CoreError::validation(
                "break_allowance_seconds",
                format!(
                    "Break allowance cannot exceed {} seconds (2 hours)",
                    MAX_BREAK_ALLOWANCE_SECONDS
                ),
            ));
        }
        if break_allowance_seconds >= duration_seconds {
            return Err(CoreError::validation(
                "break_allowance_seconds",
                "Break allowance must be strictly less than the total planned duration",
            ));
        }

        Ok(Self {
            id,
            goal_id,
            planned_start,
            duration_seconds,
            break_allowance_seconds,
            monitoring_profile_id,
            created_at: now,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commitment_valid() {
        let now = Utc::now();
        let goal_id = GoalId::new();
        let commitment = Commitment::new(goal_id, now, 1800, 300, None, now).unwrap();
        assert_eq!(commitment.duration_seconds, 1800);
        assert_eq!(commitment.break_allowance_seconds, 300);
    }

    #[test]
    fn test_commitment_duration_bounds() {
        let now = Utc::now();
        let goal_id = GoalId::new();

        // Too short (< 300s)
        assert!(matches!(
            Commitment::new(goal_id, now, 299, 0, None, now),
            Err(CoreError::ValidationError { field, .. }) if field == "duration_seconds"
        ));

        // Too long (> 43200s)
        assert!(matches!(
            Commitment::new(goal_id, now, 43201, 300, None, now),
            Err(CoreError::DurationLimitExceeded { .. })
        ));

        // Break allowance > 7200s
        assert!(matches!(
            Commitment::new(goal_id, now, 10000, 7201, None, now),
            Err(CoreError::ValidationError { field, .. }) if field == "break_allowance_seconds"
        ));

        // Break allowance >= duration
        assert!(matches!(
            Commitment::new(goal_id, now, 600, 600, None, now),
            Err(CoreError::ValidationError { field, .. }) if field == "break_allowance_seconds"
        ));
    }
}
