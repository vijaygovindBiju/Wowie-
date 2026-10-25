use crate::error::CoreError;
use crate::models::session::{FocusSession, SessionEndReason, SessionState};
use chrono::{DateTime, Utc};

/// Deterministic session lifecycle validator and state machine driver.
#[derive(Debug, Default, Clone, Copy)]
pub struct FocusSessionStateMachine;

impl FocusSessionStateMachine {
    /// Asserts whether a given state transition is logically permitted.
    pub fn is_valid_transition(from: SessionState, to: SessionState) -> bool {
        match (from, to) {
            (SessionState::Ready, SessionState::Active) => true,
            (SessionState::Active, SessionState::Paused) => true,
            (SessionState::Active, SessionState::Break) => true,
            (SessionState::Active, SessionState::Completed) => true,
            (SessionState::Active, SessionState::Interrupted) => true,
            (SessionState::Paused, SessionState::Active) => true,
            (SessionState::Paused, SessionState::Completed) => true,
            (SessionState::Paused, SessionState::Interrupted) => true,
            (SessionState::Break, SessionState::Active) => true,
            (SessionState::Break, SessionState::Completed) => true,
            (SessionState::Break, SessionState::Interrupted) => true,
            (SessionState::Interrupted, SessionState::Active) => true,
            (SessionState::Interrupted, SessionState::Ready) => true,
            (SessionState::Interrupted, SessionState::Completed) => true,
            _ => false,
        }
    }

    pub fn transition_start(
        session: &mut FocusSession,
        now: DateTime<Utc>,
    ) -> Result<(), CoreError> {
        session.start(now)
    }

    pub fn transition_pause(
        session: &mut FocusSession,
        now: DateTime<Utc>,
    ) -> Result<(), CoreError> {
        session.pause(now)
    }

    pub fn transition_resume(
        session: &mut FocusSession,
        consent_check: bool,
        now: DateTime<Utc>,
    ) -> Result<(), CoreError> {
        session.resume(consent_check, now)
    }

    pub fn transition_break(
        session: &mut FocusSession,
        now: DateTime<Utc>,
    ) -> Result<(), CoreError> {
        session.start_break(now)
    }

    pub fn transition_end_break(
        session: &mut FocusSession,
        now: DateTime<Utc>,
    ) -> Result<(), CoreError> {
        session.end_break(now)
    }

    pub fn transition_complete(
        session: &mut FocusSession,
        reason: SessionEndReason,
        now: DateTime<Utc>,
    ) -> Result<(), CoreError> {
        session.complete(reason, now)
    }

    pub fn transition_interrupt(
        session: &mut FocusSession,
        details: String,
        now: DateTime<Utc>,
    ) -> Result<(), CoreError> {
        session.interrupt(details, now)
    }

    pub fn transition_recover(
        session: &mut FocusSession,
        resume: bool,
        consent_check: bool,
        now: DateTime<Utc>,
    ) -> Result<(), CoreError> {
        if resume {
            session.recover_resume(consent_check, now)
        } else {
            session.recover_reset(now)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::CommitmentId;
    use crate::models::session::ConsentSnapshot;

    fn sample_session(now: DateTime<Utc>) -> FocusSession {
        FocusSession::new(
            CommitmentId::new(),
            ConsentSnapshot {
                monitoring_enabled: true,
                window_title_redaction_enabled: true,
                browser_origin_enabled: false,
                granted_at: now,
            },
            now,
        )
    }

    #[test]
    fn test_valid_transitions_matrix() {
        assert!(FocusSessionStateMachine::is_valid_transition(
            SessionState::Ready,
            SessionState::Active
        ));
        assert!(FocusSessionStateMachine::is_valid_transition(
            SessionState::Active,
            SessionState::Paused
        ));
        assert!(FocusSessionStateMachine::is_valid_transition(
            SessionState::Active,
            SessionState::Break
        ));
        assert!(FocusSessionStateMachine::is_valid_transition(
            SessionState::Active,
            SessionState::Completed
        ));
        assert!(FocusSessionStateMachine::is_valid_transition(
            SessionState::Active,
            SessionState::Interrupted
        ));
        assert!(FocusSessionStateMachine::is_valid_transition(
            SessionState::Paused,
            SessionState::Active
        ));
        assert!(FocusSessionStateMachine::is_valid_transition(
            SessionState::Paused,
            SessionState::Completed
        ));
        assert!(FocusSessionStateMachine::is_valid_transition(
            SessionState::Break,
            SessionState::Active
        ));
        assert!(FocusSessionStateMachine::is_valid_transition(
            SessionState::Break,
            SessionState::Completed
        ));
        assert!(FocusSessionStateMachine::is_valid_transition(
            SessionState::Paused,
            SessionState::Interrupted
        ));
        assert!(FocusSessionStateMachine::is_valid_transition(
            SessionState::Break,
            SessionState::Interrupted
        ));
        assert!(FocusSessionStateMachine::is_valid_transition(
            SessionState::Interrupted,
            SessionState::Active
        ));
        assert!(FocusSessionStateMachine::is_valid_transition(
            SessionState::Interrupted,
            SessionState::Ready
        ));
        assert!(FocusSessionStateMachine::is_valid_transition(
            SessionState::Interrupted,
            SessionState::Completed
        ));
    }

    #[test]
    fn test_invalid_transitions() {
        assert!(!FocusSessionStateMachine::is_valid_transition(
            SessionState::Ready,
            SessionState::Paused
        ));
        assert!(!FocusSessionStateMachine::is_valid_transition(
            SessionState::Ready,
            SessionState::Break
        ));
        assert!(!FocusSessionStateMachine::is_valid_transition(
            SessionState::Ready,
            SessionState::Completed
        ));
        assert!(!FocusSessionStateMachine::is_valid_transition(
            SessionState::Ready,
            SessionState::Interrupted
        ));
        assert!(!FocusSessionStateMachine::is_valid_transition(
            SessionState::Interrupted,
            SessionState::Interrupted
        ));
        assert!(!FocusSessionStateMachine::is_valid_transition(
            SessionState::Completed,
            SessionState::Active
        ));
        assert!(!FocusSessionStateMachine::is_valid_transition(
            SessionState::Completed,
            SessionState::Ready
        ));
    }

    #[test]
    fn test_state_machine_execution() {
        let now = Utc::now();
        let mut session = sample_session(now);

        FocusSessionStateMachine::transition_start(&mut session, now).unwrap();
        assert_eq!(session.state, SessionState::Active);

        let t1 = now + chrono::Duration::seconds(100);
        FocusSessionStateMachine::transition_break(&mut session, t1).unwrap();
        assert_eq!(session.state, SessionState::Break);

        let t2 = t1 + chrono::Duration::seconds(50);
        FocusSessionStateMachine::transition_end_break(&mut session, t2).unwrap();
        assert_eq!(session.state, SessionState::Active);

        let t3 = t2 + chrono::Duration::seconds(100);
        FocusSessionStateMachine::transition_complete(
            &mut session,
            SessionEndReason::GoalAchieved,
            t3,
        )
        .unwrap();
        assert_eq!(session.state, SessionState::Completed);
    }

    #[test]
    fn test_state_machine_transition_complete_and_interrupt_reject_ready() {
        let now = Utc::now();
        let mut session = sample_session(now);
        assert_eq!(session.state, SessionState::Ready);

        let t1 = now + chrono::Duration::seconds(10);
        let res_complete = FocusSessionStateMachine::transition_complete(
            &mut session,
            SessionEndReason::GoalAchieved,
            t1,
        );
        assert!(matches!(
            res_complete,
            Err(CoreError::InvalidStateTransition { from, to, .. })
                if from == "ready" && to == "completed"
        ));
        assert_eq!(session.state, SessionState::Ready);

        let res_interrupt = FocusSessionStateMachine::transition_interrupt(
            &mut session,
            "OS reboot".to_string(),
            t1,
        );
        assert!(matches!(
            res_interrupt,
            Err(CoreError::InvalidStateTransition { from, to, .. })
                if from == "ready" && to == "interrupted"
        ));
        assert_eq!(session.state, SessionState::Ready);
    }

    #[test]
    fn test_state_machine_transition_recover_reset_clears_end_reason() {
        let now = Utc::now();
        let mut session = sample_session(now);
        FocusSessionStateMachine::transition_start(&mut session, now).unwrap();

        let t1 = now + chrono::Duration::seconds(100);
        FocusSessionStateMachine::transition_interrupt(&mut session, "Power loss".to_string(), t1)
            .unwrap();
        assert_eq!(session.state, SessionState::Interrupted);
        assert!(session.end_reason.is_some());

        let t2 = t1 + chrono::Duration::seconds(10);
        FocusSessionStateMachine::transition_recover(&mut session, false, false, t2).unwrap();
        assert_eq!(session.state, SessionState::Ready);
        assert!(
            session.end_reason.is_none(),
            "transition_recover(resume=false) must clear end_reason"
        );
        assert!(
            session.started_at.is_none(),
            "transition_recover(resume=false) must clear started_at"
        );
    }

    #[test]
    fn test_state_machine_transition_interrupt_rejects_reinterruption() {
        let now = Utc::now();
        let mut session = sample_session(now);
        FocusSessionStateMachine::transition_start(&mut session, now).unwrap();

        let t1 = now + chrono::Duration::seconds(100);
        FocusSessionStateMachine::transition_interrupt(&mut session, "First crash".to_string(), t1)
            .unwrap();

        let t2 = t1 + chrono::Duration::seconds(10);
        let res = FocusSessionStateMachine::transition_interrupt(
            &mut session,
            "Second crash".to_string(),
            t2,
        );
        assert!(matches!(
            res,
            Err(CoreError::InvalidStateTransition { from, to, .. })
                if from == "interrupted" && to == "interrupted"
        ));
    }
}
