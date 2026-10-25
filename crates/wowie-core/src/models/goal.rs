use crate::error::CoreError;
use crate::id::{AmbitionId, GoalId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const MAX_GOAL_TITLE_LENGTH: usize = 200;
pub const MAX_NEXT_ACTION_LENGTH: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GoalStatus {
    Active,
    Paused,
    Completed,
    Archived,
}

impl fmt::Display for GoalStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Paused => write!(f, "paused"),
            Self::Completed => write!(f, "completed"),
            Self::Archived => write!(f, "archived"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Goal {
    pub id: GoalId,
    pub ambition_id: AmbitionId,
    pub title: String,
    pub next_action: Option<String>,
    pub status: GoalStatus,
    pub target_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub archived_at: Option<DateTime<Utc>>,
}

impl Goal {
    pub fn new(
        ambition_id: AmbitionId,
        title: impl Into<String>,
        next_action: Option<String>,
        target_date: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
    ) -> Result<Self, CoreError> {
        Self::with_id(
            GoalId::new(),
            ambition_id,
            title,
            next_action,
            target_date,
            now,
        )
    }

    pub fn with_id(
        id: GoalId,
        ambition_id: AmbitionId,
        title: impl Into<String>,
        next_action: Option<String>,
        target_date: Option<DateTime<Utc>>,
        now: DateTime<Utc>,
    ) -> Result<Self, CoreError> {
        let title = title.into().trim().to_string();
        if title.is_empty() {
            return Err(CoreError::validation("title", "Goal title cannot be empty"));
        }
        if title.chars().count() > MAX_GOAL_TITLE_LENGTH {
            return Err(CoreError::validation(
                "title",
                format!(
                    "Goal title exceeds maximum length of {} characters",
                    MAX_GOAL_TITLE_LENGTH
                ),
            ));
        }

        let next_action = next_action.map(|a| a.trim().to_string());
        if let Some(ref a) = next_action {
            if a.chars().count() > MAX_NEXT_ACTION_LENGTH {
                return Err(CoreError::validation(
                    "next_action",
                    format!(
                        "Next action exceeds maximum length of {} characters",
                        MAX_NEXT_ACTION_LENGTH
                    ),
                ));
            }
        }

        Ok(Self {
            id,
            ambition_id,
            title,
            next_action,
            status: GoalStatus::Active,
            target_date,
            created_at: now,
            updated_at: now,
            completed_at: None,
            archived_at: None,
        })
    }

    pub fn update_next_action(
        &mut self,
        next_action: Option<String>,
        now: DateTime<Utc>,
    ) -> Result<(), CoreError> {
        let next_action = next_action.map(|a| a.trim().to_string());
        if let Some(ref a) = next_action {
            if a.chars().count() > MAX_NEXT_ACTION_LENGTH {
                return Err(CoreError::validation(
                    "next_action",
                    format!(
                        "Next action exceeds maximum length of {} characters",
                        MAX_NEXT_ACTION_LENGTH
                    ),
                ));
            }
        }
        self.next_action = next_action;
        self.updated_at = now;
        Ok(())
    }

    pub fn set_status(
        &mut self,
        new_status: GoalStatus,
        now: DateTime<Utc>,
    ) -> Result<(), CoreError> {
        self.status = new_status;
        self.updated_at = now;
        match new_status {
            GoalStatus::Active | GoalStatus::Paused => {
                self.archived_at = None;
                self.completed_at = None;
            }
            GoalStatus::Completed => {
                self.completed_at = Some(now);
                self.archived_at = None;
            }
            GoalStatus::Archived => {
                self.archived_at = Some(now);
            }
        }
        Ok(())
    }

    pub fn complete(&mut self, now: DateTime<Utc>) -> Result<(), CoreError> {
        self.set_status(GoalStatus::Completed, now)
    }

    pub fn pause(&mut self, now: DateTime<Utc>) -> Result<(), CoreError> {
        self.set_status(GoalStatus::Paused, now)
    }

    pub fn resume(&mut self, now: DateTime<Utc>) -> Result<(), CoreError> {
        self.set_status(GoalStatus::Active, now)
    }

    pub fn archive(&mut self, now: DateTime<Utc>) -> Result<(), CoreError> {
        self.set_status(GoalStatus::Archived, now)
    }

    pub fn unarchive(&mut self, now: DateTime<Utc>) -> Result<(), CoreError> {
        self.set_status(GoalStatus::Active, now)
    }

    pub fn is_archived(&self) -> bool {
        self.status == GoalStatus::Archived || self.archived_at.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_goal_creation_and_bounds() {
        let now = Utc::now();
        let ambition_id = AmbitionId::new();

        let goal = Goal::new(
            ambition_id,
            "Write code",
            Some("Open editor".into()),
            None,
            now,
        )
        .unwrap();
        assert_eq!(goal.status, GoalStatus::Active);
        assert_eq!(goal.title, "Write code");

        // Empty title
        assert!(Goal::new(ambition_id, "", None, None, now).is_err());

        // Excessive title
        let long_title = "x".repeat(MAX_GOAL_TITLE_LENGTH + 1);
        assert!(Goal::new(ambition_id, long_title, None, None, now).is_err());

        // Excessive next action
        let long_action = "a".repeat(MAX_NEXT_ACTION_LENGTH + 1);
        assert!(Goal::new(ambition_id, "Title", Some(long_action), None, now).is_err());
    }

    #[test]
    fn test_goal_status_transitions() {
        let now = Utc::now();
        let mut goal = Goal::new(AmbitionId::new(), "Goal 1", None, None, now).unwrap();
        assert_eq!(goal.status, GoalStatus::Active);

        let t1 = now + chrono::Duration::minutes(5);
        goal.pause(t1).unwrap();
        assert_eq!(goal.status, GoalStatus::Paused);

        let t2 = now + chrono::Duration::minutes(10);
        goal.resume(t2).unwrap();
        assert_eq!(goal.status, GoalStatus::Active);

        let t3 = now + chrono::Duration::minutes(15);
        goal.complete(t3).unwrap();
        assert_eq!(goal.status, GoalStatus::Completed);
        assert_eq!(goal.completed_at, Some(t3));

        let t4 = now + chrono::Duration::minutes(20);
        goal.archive(t4).unwrap();
        assert_eq!(goal.status, GoalStatus::Archived);
        assert_eq!(goal.archived_at, Some(t4));
    }

    #[test]
    fn test_goal_archive_unarchive_and_resume_cycles() {
        let now = Utc::now();
        let ambition_id = AmbitionId::new();
        let mut goal = Goal::new(ambition_id, "Test Lifecycle", None, None, now).unwrap();
        assert_eq!(goal.status, GoalStatus::Active);
        assert_eq!(goal.archived_at, None);
        assert_eq!(goal.completed_at, None);
        assert!(!goal.is_archived());

        // 1. Active -> Archived
        let t1 = now + chrono::Duration::minutes(10);
        goal.archive(t1).unwrap();
        assert_eq!(goal.status, GoalStatus::Archived);
        assert_eq!(goal.archived_at, Some(t1));
        assert!(goal.is_archived());

        // 2. Archived -> Active (via resume)
        let t2 = now + chrono::Duration::minutes(20);
        goal.resume(t2).unwrap();
        assert_eq!(goal.status, GoalStatus::Active);
        assert_eq!(goal.archived_at, None);
        assert_eq!(goal.completed_at, None);
        assert_eq!(goal.updated_at, t2);
        assert!(!goal.is_archived());

        // 3. Active -> Archived -> Active (via unarchive)
        let t3 = now + chrono::Duration::minutes(30);
        goal.archive(t3).unwrap();
        assert_eq!(goal.archived_at, Some(t3));
        let t4 = now + chrono::Duration::minutes(40);
        goal.unarchive(t4).unwrap();
        assert_eq!(goal.status, GoalStatus::Active);
        assert_eq!(goal.archived_at, None);
        assert!(!goal.is_archived());

        // 4. Active -> Archived -> Paused (via pause)
        let t5 = now + chrono::Duration::minutes(50);
        goal.archive(t5).unwrap();
        assert_eq!(goal.archived_at, Some(t5));
        let t6 = now + chrono::Duration::minutes(60);
        goal.pause(t6).unwrap();
        assert_eq!(goal.status, GoalStatus::Paused);
        assert_eq!(goal.archived_at, None);
        assert!(!goal.is_archived());

        // 5. Completed -> Reopened to Active
        let t7 = now + chrono::Duration::minutes(70);
        goal.complete(t7).unwrap();
        assert_eq!(goal.status, GoalStatus::Completed);
        assert_eq!(goal.completed_at, Some(t7));
        assert_eq!(goal.archived_at, None);

        let t8 = now + chrono::Duration::minutes(80);
        goal.resume(t8).unwrap();
        assert_eq!(goal.status, GoalStatus::Active);
        assert_eq!(goal.completed_at, None);
        assert_eq!(goal.archived_at, None);
    }
}
