use crate::error::CoreError;
use crate::id::{CommitmentId, SessionId};
use crate::state_machine::FocusSessionStateMachine;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionState {
    Ready,
    Active,
    Paused,
    Break,
    Completed,
    Interrupted,
}

impl SessionState {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed)
    }

    pub fn allows_observation(&self) -> bool {
        matches!(self, Self::Active)
    }
}

impl fmt::Display for SessionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready => write!(f, "ready"),
            Self::Active => write!(f, "active"),
            Self::Paused => write!(f, "paused"),
            Self::Break => write!(f, "break"),
            Self::Completed => write!(f, "completed"),
            Self::Interrupted => write!(f, "interrupted"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "details", rename_all = "snake_case")]
pub enum SessionEndReason {
    GoalAchieved,
    TimeExpired,
    UserCancelled { reason: Option<String> },
    SystemInterrupted { details: String },
}

impl fmt::Display for SessionEndReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GoalAchieved => write!(f, "goal_achieved"),
            Self::TimeExpired => write!(f, "time_expired"),
            Self::UserCancelled { reason } => {
                write!(f, "user_cancelled: {}", reason.as_deref().unwrap_or("none"))
            }
            Self::SystemInterrupted { details } => write!(f, "system_interrupted: {}", details),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsentSnapshot {
    pub monitoring_enabled: bool,
    pub window_title_redaction_enabled: bool,
    pub browser_origin_enabled: bool,
    pub granted_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FocusSession {
    pub id: SessionId,
    pub commitment_id: CommitmentId,
    pub state: SessionState,
    pub consent: ConsentSnapshot,
    pub started_at: Option<DateTime<Utc>>,
    pub ended_at: Option<DateTime<Utc>>,
    pub end_reason: Option<SessionEndReason>,
    pub accumulated_focus_seconds: u64,
    pub accumulated_break_seconds: u64,
    pub accumulated_idle_seconds: u64,
    pub last_state_change: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl FocusSession {
    pub fn new(commitment_id: CommitmentId, consent: ConsentSnapshot, now: DateTime<Utc>) -> Self {
        Self::with_id(SessionId::new(), commitment_id, consent, now)
    }

    pub fn with_id(
        id: SessionId,
        commitment_id: CommitmentId,
        consent: ConsentSnapshot,
        now: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            commitment_id,
            state: SessionState::Ready,
            consent,
            started_at: None,
            ended_at: None,
            end_reason: None,
            accumulated_focus_seconds: 0,
            accumulated_break_seconds: 0,
            accumulated_idle_seconds: 0,
            last_state_change: now,
            created_at: now,
            updated_at: now,
        }
    }

    fn check_time_progression(&self, now: DateTime<Utc>) -> Result<(), CoreError> {
        if now < self.last_state_change {
            return Err(CoreError::TimeRegression {
                current: self.last_state_change.to_rfc3339(),
                attempted: now.to_rfc3339(),
            });
        }
        Ok(())
    }

    pub fn start(&mut self, now: DateTime<Utc>) -> Result<(), CoreError> {
        self.check_time_progression(now)?;
        if self.state != SessionState::Ready {
            return Err(CoreError::InvalidStateTransition {
                from: self.state.to_string(),
                to: SessionState::Active.to_string(),
                reason: "Session can only be started from Ready state".to_string(),
            });
        }
        if !self.consent.monitoring_enabled {
            return Err(CoreError::ConsentViolation(
                "Cannot activate session without monitoring consent enabled".to_string(),
            ));
        }

        self.state = SessionState::Active;
        self.started_at = Some(now);
        self.last_state_change = now;
        self.updated_at = now;
        Ok(())
    }

    pub fn pause(&mut self, now: DateTime<Utc>) -> Result<(), CoreError> {
        self.check_time_progression(now)?;
        if self.state != SessionState::Active {
            return Err(CoreError::InvalidStateTransition {
                from: self.state.to_string(),
                to: SessionState::Paused.to_string(),
                reason: "Session can only be paused from Active state".to_string(),
            });
        }

        let delta = (now - self.last_state_change).num_seconds().max(0) as u64;
        self.accumulated_focus_seconds += delta;
        self.state = SessionState::Paused;
        self.last_state_change = now;
        self.updated_at = now;
        Ok(())
    }

    pub fn resume(&mut self, consent_confirmed: bool, now: DateTime<Utc>) -> Result<(), CoreError> {
        self.check_time_progression(now)?;
        if self.state != SessionState::Paused {
            return Err(CoreError::InvalidStateTransition {
                from: self.state.to_string(),
                to: SessionState::Active.to_string(),
                reason: "Session can only be resumed from Paused state".to_string(),
            });
        }
        if !consent_confirmed || !self.consent.monitoring_enabled {
            return Err(CoreError::ConsentViolation(
                "Explicit consent re-verification is required to resume session".to_string(),
            ));
        }

        self.state = SessionState::Active;
        self.last_state_change = now;
        self.updated_at = now;
        Ok(())
    }

    pub fn start_break(&mut self, now: DateTime<Utc>) -> Result<(), CoreError> {
        self.check_time_progression(now)?;
        if self.state != SessionState::Active {
            return Err(CoreError::InvalidStateTransition {
                from: self.state.to_string(),
                to: SessionState::Break.to_string(),
                reason: "Break can only be started from Active state".to_string(),
            });
        }

        let delta = (now - self.last_state_change).num_seconds().max(0) as u64;
        self.accumulated_focus_seconds += delta;
        self.state = SessionState::Break;
        self.last_state_change = now;
        self.updated_at = now;
        Ok(())
    }

    pub fn end_break(&mut self, now: DateTime<Utc>) -> Result<(), CoreError> {
        self.check_time_progression(now)?;
        if self.state != SessionState::Break {
            return Err(CoreError::InvalidStateTransition {
                from: self.state.to_string(),
                to: SessionState::Active.to_string(),
                reason: "Break can only be ended from Break state".to_string(),
            });
        }

        let delta = (now - self.last_state_change).num_seconds().max(0) as u64;
        self.accumulated_break_seconds += delta;
        self.state = SessionState::Active;
        self.last_state_change = now;
        self.updated_at = now;
        Ok(())
    }

    pub fn complete(
        &mut self,
        reason: SessionEndReason,
        now: DateTime<Utc>,
    ) -> Result<(), CoreError> {
        self.check_time_progression(now)?;
        if self.state.is_terminal() {
            return Err(CoreError::SessionTerminated {
                state: self.state.to_string(),
                reason: "Session has already reached a terminal state".to_string(),
            });
        }
        if !FocusSessionStateMachine::is_valid_transition(self.state, SessionState::Completed) {
            return Err(CoreError::InvalidStateTransition {
                from: self.state.to_string(),
                to: SessionState::Completed.to_string(),
                reason: format!("Cannot complete session from {} state", self.state),
            });
        }

        let delta = (now - self.last_state_change).num_seconds().max(0) as u64;
        match self.state {
            SessionState::Active => self.accumulated_focus_seconds += delta,
            SessionState::Break => self.accumulated_break_seconds += delta,
            _ => {}
        }

        self.state = SessionState::Completed;
        self.ended_at = Some(now);
        self.end_reason = Some(reason);
        self.last_state_change = now;
        self.updated_at = now;
        Ok(())
    }

    pub fn interrupt(&mut self, details: String, now: DateTime<Utc>) -> Result<(), CoreError> {
        self.check_time_progression(now)?;
        if self.state.is_terminal() {
            return Err(CoreError::SessionTerminated {
                state: self.state.to_string(),
                reason: "Cannot interrupt a terminated session".to_string(),
            });
        }
        if !FocusSessionStateMachine::is_valid_transition(self.state, SessionState::Interrupted) {
            return Err(CoreError::InvalidStateTransition {
                from: self.state.to_string(),
                to: SessionState::Interrupted.to_string(),
                reason: format!("Cannot interrupt session from {} state", self.state),
            });
        }

        let delta = (now - self.last_state_change).num_seconds().max(0) as u64;
        match self.state {
            SessionState::Active => self.accumulated_focus_seconds += delta,
            SessionState::Break => self.accumulated_break_seconds += delta,
            _ => {}
        }

        self.state = SessionState::Interrupted;
        self.end_reason = Some(SessionEndReason::SystemInterrupted { details });
        self.last_state_change = now;
        self.updated_at = now;
        Ok(())
    }

    pub fn recover_resume(
        &mut self,
        consent_confirmed: bool,
        now: DateTime<Utc>,
    ) -> Result<(), CoreError> {
        self.check_time_progression(now)?;
        if self.state != SessionState::Interrupted {
            return Err(CoreError::InvalidStateTransition {
                from: self.state.to_string(),
                to: SessionState::Active.to_string(),
                reason: "Can only recover-resume from an Interrupted state".to_string(),
            });
        }
        if !consent_confirmed || !self.consent.monitoring_enabled {
            return Err(CoreError::ConsentViolation(
                "Consent check required when resuming an interrupted session".to_string(),
            ));
        }

        self.state = SessionState::Active;
        self.end_reason = None;
        self.last_state_change = now;
        self.updated_at = now;
        Ok(())
    }

    pub fn recover_reset(&mut self, now: DateTime<Utc>) -> Result<(), CoreError> {
        self.check_time_progression(now)?;
        if self.state != SessionState::Interrupted {
            return Err(CoreError::InvalidStateTransition {
                from: self.state.to_string(),
                to: SessionState::Ready.to_string(),
                reason: "Can only reset to Ready from an Interrupted state".to_string(),
            });
        }

        self.state = SessionState::Ready;
        self.end_reason = None;
        self.started_at = None;
        self.last_state_change = now;
        self.updated_at = now;
        Ok(())
    }

    pub fn record_idle(&mut self, idle_seconds: u64, now: DateTime<Utc>) -> Result<(), CoreError> {
        self.check_time_progression(now)?;
        self.accumulated_idle_seconds += idle_seconds;
        self.updated_at = now;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn test_consent(enabled: bool) -> ConsentSnapshot {
        ConsentSnapshot {
            monitoring_enabled: enabled,
            window_title_redaction_enabled: true,
            browser_origin_enabled: false,
            granted_at: Utc::now(),
        }
    }

    #[test]
    fn test_normal_session_flow_and_accrual() {
        let t0 = Utc::now();
        let mut session = FocusSession::new(CommitmentId::new(), test_consent(true), t0);
        assert_eq!(session.state, SessionState::Ready);
        assert!(!session.state.allows_observation());

        // Start
        session.start(t0).unwrap();
        assert_eq!(session.state, SessionState::Active);
        assert!(session.state.allows_observation());

        // Focus for 10 minutes (600s), then Pause
        let t1 = t0 + Duration::seconds(600);
        session.pause(t1).unwrap();
        assert_eq!(session.state, SessionState::Paused);
        assert!(!session.state.allows_observation());
        assert_eq!(session.accumulated_focus_seconds, 600);

        // Paused for 5 minutes (300s), then Resume
        let t2 = t1 + Duration::seconds(300);
        session.resume(true, t2).unwrap();
        assert_eq!(session.state, SessionState::Active);
        assert!(session.state.allows_observation());
        assert_eq!(session.accumulated_focus_seconds, 600); // Focus time did not increase during pause

        // Focus for 15 minutes (900s), then Break
        let t3 = t2 + Duration::seconds(900);
        session.start_break(t3).unwrap();
        assert_eq!(session.state, SessionState::Break);
        assert!(!session.state.allows_observation());
        assert_eq!(session.accumulated_focus_seconds, 1500);

        // Break for 5 minutes (300s), then End Break
        let t4 = t3 + Duration::seconds(300);
        session.end_break(t4).unwrap();
        assert_eq!(session.state, SessionState::Active);
        assert_eq!(session.accumulated_break_seconds, 300);
        assert_eq!(session.accumulated_focus_seconds, 1500);

        // Focus for another 10 minutes (600s), then Complete
        let t5 = t4 + Duration::seconds(600);
        session
            .complete(SessionEndReason::GoalAchieved, t5)
            .unwrap();
        assert_eq!(session.state, SessionState::Completed);
        assert_eq!(session.accumulated_focus_seconds, 2100);
        assert_eq!(session.accumulated_break_seconds, 300);
        assert!(session.state.is_terminal());
        assert!(!session.state.allows_observation());
    }

    #[test]
    fn test_time_regression_rejected() {
        let t0 = Utc::now();
        let mut session = FocusSession::new(CommitmentId::new(), test_consent(true), t0);
        session.start(t0).unwrap();

        // Attempt transition with earlier time
        let earlier = t0 - Duration::seconds(10);
        let res = session.pause(earlier);
        assert!(matches!(res, Err(CoreError::TimeRegression { .. })));
    }

    #[test]
    fn test_consent_violation_on_start_and_resume() {
        let t0 = Utc::now();
        // Consent disabled
        let mut session = FocusSession::new(CommitmentId::new(), test_consent(false), t0);
        assert!(matches!(
            session.start(t0),
            Err(CoreError::ConsentViolation(_))
        ));

        // Valid consent, then unconfirmed resume
        let mut session2 = FocusSession::new(CommitmentId::new(), test_consent(true), t0);
        session2.start(t0).unwrap();
        let t1 = t0 + Duration::seconds(10);
        session2.pause(t1).unwrap();
        let t2 = t1 + Duration::seconds(10);
        assert!(matches!(
            session2.resume(false, t2),
            Err(CoreError::ConsentViolation(_))
        ));
    }

    #[test]
    fn test_terminal_session_cannot_transition() {
        let t0 = Utc::now();
        let mut session = FocusSession::new(CommitmentId::new(), test_consent(true), t0);
        session.start(t0).unwrap();
        let t1 = t0 + Duration::seconds(100);
        session.complete(SessionEndReason::TimeExpired, t1).unwrap();

        let t2 = t1 + Duration::seconds(10);
        assert!(matches!(
            session.start(t2),
            Err(CoreError::InvalidStateTransition { .. })
        ));
        assert!(matches!(
            session.pause(t2),
            Err(CoreError::InvalidStateTransition { .. })
        ));
        assert!(matches!(
            session.complete(SessionEndReason::GoalAchieved, t2),
            Err(CoreError::SessionTerminated { .. })
        ));
    }

    #[test]
    fn test_interruption_and_recovery() {
        let t0 = Utc::now();
        let mut session = FocusSession::new(CommitmentId::new(), test_consent(true), t0);
        session.start(t0).unwrap();

        let t1 = t0 + Duration::seconds(300);
        session.interrupt("OS suspension".into(), t1).unwrap();
        assert_eq!(session.state, SessionState::Interrupted);
        assert_eq!(session.accumulated_focus_seconds, 300);

        // Recover resume
        let t2 = t1 + Duration::seconds(60);
        session.recover_resume(true, t2).unwrap();
        assert_eq!(session.state, SessionState::Active);

        // Interrupt again and recover reset
        let t3 = t2 + Duration::seconds(200);
        session.interrupt("Crash recovery".into(), t3).unwrap();
        let t4 = t3 + Duration::seconds(10);
        session.recover_reset(t4).unwrap();
        assert_eq!(session.state, SessionState::Ready);
    }

    #[test]
    fn test_illegal_transitions_from_ready_to_completed_and_interrupted() {
        let t0 = Utc::now();
        let mut session = FocusSession::new(CommitmentId::new(), test_consent(true), t0);
        assert_eq!(session.state, SessionState::Ready);

        let t1 = t0 + Duration::seconds(10);
        // Ready -> Completed must be rejected
        let complete_res = session.complete(SessionEndReason::GoalAchieved, t1);
        assert!(matches!(
            complete_res,
            Err(CoreError::InvalidStateTransition { from, to, .. })
                if from == "ready" && to == "completed"
        ));
        assert_eq!(session.state, SessionState::Ready);
        assert!(session.started_at.is_none());
        assert!(session.ended_at.is_none());
        assert!(session.end_reason.is_none());

        // Ready -> Interrupted must be rejected
        let interrupt_res = session.interrupt("System sleep".to_string(), t1);
        assert!(matches!(
            interrupt_res,
            Err(CoreError::InvalidStateTransition { from, to, .. })
                if from == "ready" && to == "interrupted"
        ));
        assert_eq!(session.state, SessionState::Ready);
        assert!(session.end_reason.is_none());
    }

    #[test]
    fn test_illegal_transition_reinterrupt_interrupted_session() {
        let t0 = Utc::now();
        let mut session = FocusSession::new(CommitmentId::new(), test_consent(true), t0);
        session.start(t0).unwrap();

        let t1 = t0 + Duration::seconds(100);
        session
            .interrupt("First interruption".to_string(), t1)
            .unwrap();
        assert_eq!(session.state, SessionState::Interrupted);

        let t2 = t1 + Duration::seconds(10);
        // Interrupted -> Interrupted must be rejected
        let second_interrupt = session.interrupt("Second interruption".to_string(), t2);
        assert!(matches!(
            second_interrupt,
            Err(CoreError::InvalidStateTransition { from, to, .. })
                if from == "interrupted" && to == "interrupted"
        ));
        assert_eq!(session.state, SessionState::Interrupted);
    }

    #[test]
    fn test_valid_completion_from_paused_break_and_interrupted() {
        let t0 = Utc::now();

        // 1. Completion from Paused
        let mut s_paused = FocusSession::new(CommitmentId::new(), test_consent(true), t0);
        s_paused.start(t0).unwrap();
        let t1 = t0 + Duration::seconds(100);
        s_paused.pause(t1).unwrap();
        let t2 = t1 + Duration::seconds(50);
        assert!(s_paused
            .complete(SessionEndReason::UserCancelled { reason: None }, t2)
            .is_ok());
        assert_eq!(s_paused.state, SessionState::Completed);
        assert_eq!(s_paused.accumulated_focus_seconds, 100);

        // 2. Completion from Break
        let mut s_break = FocusSession::new(CommitmentId::new(), test_consent(true), t0);
        s_break.start(t0).unwrap();
        s_break.start_break(t1).unwrap();
        let t3 = t1 + Duration::seconds(60);
        assert!(s_break.complete(SessionEndReason::GoalAchieved, t3).is_ok());
        assert_eq!(s_break.state, SessionState::Completed);
        assert_eq!(s_break.accumulated_focus_seconds, 100);
        assert_eq!(s_break.accumulated_break_seconds, 60);

        // 3. Completion from Interrupted
        let mut s_interrupted = FocusSession::new(CommitmentId::new(), test_consent(true), t0);
        s_interrupted.start(t0).unwrap();
        s_interrupted
            .interrupt("OS suspension".to_string(), t1)
            .unwrap();
        let t4 = t1 + Duration::seconds(30);
        assert!(s_interrupted
            .complete(
                SessionEndReason::UserCancelled {
                    reason: Some("abandoned".into())
                },
                t4
            )
            .is_ok());
        assert_eq!(s_interrupted.state, SessionState::Completed);
    }

    #[test]
    fn test_valid_interruption_from_paused_and_break() {
        let t0 = Utc::now();

        // Interruption from Paused
        let mut s_paused = FocusSession::new(CommitmentId::new(), test_consent(true), t0);
        s_paused.start(t0).unwrap();
        let t1 = t0 + Duration::seconds(100);
        s_paused.pause(t1).unwrap();
        let t2 = t1 + Duration::seconds(50);
        assert!(s_paused
            .interrupt("Crash while paused".to_string(), t2)
            .is_ok());
        assert_eq!(s_paused.state, SessionState::Interrupted);

        // Interruption from Break
        let mut s_break = FocusSession::new(CommitmentId::new(), test_consent(true), t0);
        s_break.start(t0).unwrap();
        s_break.start_break(t1).unwrap();
        let t3 = t1 + Duration::seconds(60);
        assert!(s_break
            .interrupt("Crash during break".to_string(), t3)
            .is_ok());
        assert_eq!(s_break.state, SessionState::Interrupted);
    }

    #[test]
    fn test_recover_reset_clears_end_reason_and_started_at() {
        let t0 = Utc::now();
        let mut session = FocusSession::new(CommitmentId::new(), test_consent(true), t0);
        session.start(t0).unwrap();

        let t1 = t0 + Duration::seconds(300);
        session.interrupt("System sleep".to_string(), t1).unwrap();
        assert_eq!(session.state, SessionState::Interrupted);
        assert!(session.end_reason.is_some());
        assert_eq!(
            session.end_reason,
            Some(SessionEndReason::SystemInterrupted {
                details: "System sleep".to_string()
            })
        );
        assert!(session.started_at.is_some());

        let t2 = t1 + Duration::seconds(10);
        session.recover_reset(t2).unwrap();
        assert_eq!(session.state, SessionState::Ready);
        assert!(
            session.end_reason.is_none(),
            "recover_reset must clear stale end_reason to None"
        );
        assert!(
            session.started_at.is_none(),
            "recover_reset must clear started_at to None for Ready state"
        );
        assert_eq!(session.last_state_change, t2);
        assert_eq!(session.updated_at, t2);
    }
}
