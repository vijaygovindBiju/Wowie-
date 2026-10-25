//! Adversarial challenge test harness for wowie-core domain invariants.
//!
//! Evaluates:
//! 1. State machine transitions (all 36 state pairs & transition methods)
//! 2. Clock monotonicity & time regression rejection
//! 3. Input bounds and sanitization (titles, descriptions, durations, allowances, UUIDs)
//! 4. Consent verification enforcement across lifecycle transitions

use chrono::{DateTime, Duration, Utc};
use wowie_core::error::CoreError;
use wowie_core::id::{AmbitionId, CommitmentId, GoalId, SegmentId, SessionId};
use wowie_core::models::ambition::{
    Ambition, MAX_AMBITION_DESCRIPTION_LENGTH, MAX_AMBITION_TITLE_LENGTH,
};
use wowie_core::models::commitment::{
    Commitment, MAX_BREAK_ALLOWANCE_SECONDS, MAX_SESSION_DURATION_SECONDS,
    MIN_SESSION_DURATION_SECONDS,
};
use wowie_core::models::correction::{
    Classification, ClassificationBasis, ClassificationLabel, CorrectionScope, UserCorrection,
};
use wowie_core::models::goal::{Goal, GoalStatus, MAX_GOAL_TITLE_LENGTH, MAX_NEXT_ACTION_LENGTH};
use wowie_core::models::observation::{
    ActivitySegment, IdleState, Observation, ObservationSource, SegmentKind, MAX_APP_ID_LENGTH,
    MAX_ORIGIN_LENGTH, MAX_WINDOW_TITLE_LENGTH,
};
use wowie_core::models::review::{SessionOutcome, SessionReview, MAX_REVIEW_NOTE_LENGTH};
use wowie_core::models::session::{ConsentSnapshot, FocusSession, SessionEndReason, SessionState};
use wowie_core::state_machine::FocusSessionStateMachine;

fn valid_consent(enabled: bool) -> ConsentSnapshot {
    ConsentSnapshot {
        monitoring_enabled: enabled,
        window_title_redaction_enabled: true,
        browser_origin_enabled: false,
        granted_at: Utc::now(),
    }
}

fn create_session_in_state(state: SessionState, base_time: DateTime<Utc>) -> FocusSession {
    let mut session = FocusSession::new(CommitmentId::new(), valid_consent(true), base_time);
    match state {
        SessionState::Ready => session,
        SessionState::Active => {
            session.start(base_time + Duration::seconds(1)).unwrap();
            session
        }
        SessionState::Paused => {
            session.start(base_time + Duration::seconds(1)).unwrap();
            session.pause(base_time + Duration::seconds(10)).unwrap();
            session
        }
        SessionState::Break => {
            session.start(base_time + Duration::seconds(1)).unwrap();
            session
                .start_break(base_time + Duration::seconds(10))
                .unwrap();
            session
        }
        SessionState::Completed => {
            session.start(base_time + Duration::seconds(1)).unwrap();
            session
                .complete(
                    SessionEndReason::GoalAchieved,
                    base_time + Duration::seconds(10),
                )
                .unwrap();
            session
        }
        SessionState::Interrupted => {
            session.start(base_time + Duration::seconds(1)).unwrap();
            session
                .interrupt("OS SIGTERM".into(), base_time + Duration::seconds(10))
                .unwrap();
            session
        }
    }
}

// ---------------------------------------------------------------------------
// 1. STATE MACHINE & ILLEGAL TRANSITIONS EMPIRICAL STRESS TEST
// ---------------------------------------------------------------------------

#[test]
fn test_state_machine_matrix_completeness() {
    let all_states = [
        SessionState::Ready,
        SessionState::Active,
        SessionState::Paused,
        SessionState::Break,
        SessionState::Completed,
        SessionState::Interrupted,
    ];

    let mut allowed_count = 0;
    let mut forbidden_count = 0;

    for &from in &all_states {
        for &to in &all_states {
            if FocusSessionStateMachine::is_valid_transition(from, to) {
                allowed_count += 1;
            } else {
                forbidden_count += 1;
            }
        }
    }

    assert_eq!(allowed_count + forbidden_count, 36);
    // Explicitly 14 transitions allowed per state machine table:
    // (Ready, Active)
    // (Active, Paused), (Active, Break), (Active, Completed), (Active, Interrupted)
    // (Paused, Active), (Paused, Completed), (Paused, Interrupted)
    // (Break, Active), (Break, Completed), (Break, Interrupted)
    // (Interrupted, Active), (Interrupted, Ready), (Interrupted, Completed)
    assert_eq!(allowed_count, 14);
    assert_eq!(forbidden_count, 22);
}

#[test]
fn test_illegal_transitions_from_completed_terminal() {
    let base = Utc::now();
    let mut session = create_session_in_state(SessionState::Completed, base);
    let t = session.last_state_change + Duration::seconds(1);

    // Completed session cannot transition to ANY state
    assert!(session.start(t).is_err());
    assert!(session.pause(t).is_err());
    assert!(session.resume(true, t).is_err());
    assert!(session.start_break(t).is_err());
    assert!(session.end_break(t).is_err());
    assert!(matches!(
        session.complete(SessionEndReason::TimeExpired, t),
        Err(CoreError::SessionTerminated { .. })
    ));
    assert!(matches!(
        session.interrupt("interrupt".into(), t),
        Err(CoreError::SessionTerminated { .. })
    ));
    assert!(session.recover_resume(true, t).is_err());
    assert!(session.recover_reset(t).is_err());
}

#[test]
fn test_illegal_transitions_from_paused() {
    let base = Utc::now();
    let mut session = create_session_in_state(SessionState::Paused, base);
    let t = session.last_state_change + Duration::seconds(1);

    // Paused cannot start
    assert!(matches!(
        session.start(t),
        Err(CoreError::InvalidStateTransition { .. })
    ));
    // Paused cannot pause again
    assert!(matches!(
        session.pause(t),
        Err(CoreError::InvalidStateTransition { .. })
    ));
    // Paused cannot jump directly to break without resuming
    assert!(matches!(
        session.start_break(t),
        Err(CoreError::InvalidStateTransition { .. })
    ));
    // Paused cannot end break
    assert!(matches!(
        session.end_break(t),
        Err(CoreError::InvalidStateTransition { .. })
    ));
    // Paused cannot recover_resume (only from Interrupted)
    assert!(matches!(
        session.recover_resume(true, t),
        Err(CoreError::InvalidStateTransition { .. })
    ));
    // Paused cannot recover_reset (only from Interrupted)
    assert!(matches!(
        session.recover_reset(t),
        Err(CoreError::InvalidStateTransition { .. })
    ));
}

#[test]
fn test_illegal_transitions_from_break() {
    let base = Utc::now();
    let mut session = create_session_in_state(SessionState::Break, base);
    let t = session.last_state_change + Duration::seconds(1);

    // Break cannot start
    assert!(matches!(
        session.start(t),
        Err(CoreError::InvalidStateTransition { .. })
    ));
    // Break cannot pause
    assert!(matches!(
        session.pause(t),
        Err(CoreError::InvalidStateTransition { .. })
    ));
    // Break cannot resume (resume is from Paused)
    assert!(matches!(
        session.resume(true, t),
        Err(CoreError::InvalidStateTransition { .. })
    ));
    // Break cannot start another break
    assert!(matches!(
        session.start_break(t),
        Err(CoreError::InvalidStateTransition { .. })
    ));
    // Break cannot recover_resume
    assert!(matches!(
        session.recover_resume(true, t),
        Err(CoreError::InvalidStateTransition { .. })
    ));
    // Break cannot recover_reset
    assert!(matches!(
        session.recover_reset(t),
        Err(CoreError::InvalidStateTransition { .. })
    ));
}

#[test]
fn test_illegal_transitions_from_ready() {
    let base = Utc::now();
    let mut session = create_session_in_state(SessionState::Ready, base);
    let t = session.last_state_change + Duration::seconds(1);

    // Ready cannot pause
    assert!(matches!(
        session.pause(t),
        Err(CoreError::InvalidStateTransition { .. })
    ));
    // Ready cannot resume
    assert!(matches!(
        session.resume(true, t),
        Err(CoreError::InvalidStateTransition { .. })
    ));
    // Ready cannot start break
    assert!(matches!(
        session.start_break(t),
        Err(CoreError::InvalidStateTransition { .. })
    ));
    // Ready cannot end break
    assert!(matches!(
        session.end_break(t),
        Err(CoreError::InvalidStateTransition { .. })
    ));
    // Ready cannot recover resume
    assert!(matches!(
        session.recover_resume(true, t),
        Err(CoreError::InvalidStateTransition { .. })
    ));
    // Ready cannot recover reset
    assert!(matches!(
        session.recover_reset(t),
        Err(CoreError::InvalidStateTransition { .. })
    ));
    // Ready cannot complete
    assert!(matches!(
        session.complete(SessionEndReason::GoalAchieved, t),
        Err(CoreError::InvalidStateTransition { .. })
    ));
    // Ready cannot interrupt
    assert!(matches!(
        session.interrupt("crash".into(), t),
        Err(CoreError::InvalidStateTransition { .. })
    ));
}

/// EMPIRICAL CHALLENGE: Verify state machine transition enforcement.
/// Strict invariants:
/// 1. Ready -> Completed is marked invalid and must be rejected with InvalidStateTransition.
/// 2. Ready -> Interrupted is marked invalid and must be rejected with InvalidStateTransition.
/// 3. Interrupted -> Interrupted is marked invalid and must be rejected with InvalidStateTransition.
#[test]
fn test_empirical_state_machine_divergence_flaws() {
    let base = Utc::now();
    let t = base + Duration::seconds(10);

    // 1. Ready -> Completed
    assert!(!FocusSessionStateMachine::is_valid_transition(
        SessionState::Ready,
        SessionState::Completed
    ));
    let mut session_ready1 = FocusSession::new(CommitmentId::new(), valid_consent(true), base);
    let res1 = session_ready1.complete(SessionEndReason::UserCancelled { reason: None }, t);
    assert!(
        matches!(
            res1,
            Err(CoreError::InvalidStateTransition { from, to, .. })
                if from == "ready" && to == "completed"
        ),
        "Ready -> Completed must be rejected with InvalidStateTransition"
    );
    assert_eq!(session_ready1.state, SessionState::Ready);
    assert!(session_ready1.started_at.is_none());

    // 2. Ready -> Interrupted
    assert!(!FocusSessionStateMachine::is_valid_transition(
        SessionState::Ready,
        SessionState::Interrupted
    ));
    let mut session_ready2 = FocusSession::new(CommitmentId::new(), valid_consent(true), base);
    let res2 = session_ready2.interrupt("system sleep".into(), t);
    assert!(
        matches!(
            res2,
            Err(CoreError::InvalidStateTransition { from, to, .. })
                if from == "ready" && to == "interrupted"
        ),
        "Ready -> Interrupted must be rejected with InvalidStateTransition"
    );
    assert_eq!(session_ready2.state, SessionState::Ready);

    // 3. Interrupted -> Interrupted
    assert!(!FocusSessionStateMachine::is_valid_transition(
        SessionState::Interrupted,
        SessionState::Interrupted
    ));
    let mut session_interrupted = create_session_in_state(SessionState::Interrupted, base);
    let t_later = session_interrupted.last_state_change + Duration::seconds(10);
    let res3 = session_interrupted.interrupt("second crash".into(), t_later);
    assert!(
        matches!(
            res3,
            Err(CoreError::InvalidStateTransition { from, to, .. })
                if from == "interrupted" && to == "interrupted"
        ),
        "Interrupted -> Interrupted must be rejected with InvalidStateTransition"
    );
    assert_eq!(session_interrupted.state, SessionState::Interrupted);
}

/// EMPIRICAL CHALLENGE: Verify Goal unarchiving clears archived_at.
/// When a Goal is archived and later resumed/activated, its archived_at field MUST be cleared.
#[test]
fn test_empirical_goal_archived_at_leak_flaw() {
    let now = Utc::now();
    let ambition_id = AmbitionId::new();
    let mut goal = Goal::new(ambition_id, "Build Core", None, None, now).unwrap();
    assert_eq!(goal.status, GoalStatus::Active);
    assert!(goal.archived_at.is_none());
    assert!(!goal.is_archived());

    // Archive the goal
    let archive_time = now + Duration::hours(1);
    goal.archive(archive_time).unwrap();
    assert_eq!(goal.status, GoalStatus::Archived);
    assert_eq!(goal.archived_at, Some(archive_time));
    assert!(goal.is_archived());

    // Resume / unarchive the goal
    let resume_time = archive_time + Duration::hours(1);
    goal.resume(resume_time).unwrap();
    assert_eq!(goal.status, GoalStatus::Active);

    // Remediated invariant: goal.archived_at is reset to None
    assert!(
        goal.archived_at.is_none(),
        "Remediated: Goal::resume() must clear archived_at to None"
    );
    assert!(!goal.is_archived());
}

// ---------------------------------------------------------------------------
// 2. CLOCK MONOTONICITY & TIME REGRESSION TESTS
// ---------------------------------------------------------------------------

#[test]
fn test_clock_monotonicity_across_all_methods() {
    let base = Utc::now();

    // 1. start() regression
    let mut s1 = FocusSession::new(CommitmentId::new(), valid_consent(true), base);
    let past = base - Duration::seconds(1);
    assert!(matches!(
        s1.start(past),
        Err(CoreError::TimeRegression { .. })
    ));

    // 2. pause() regression
    let mut s2 = create_session_in_state(SessionState::Active, base);
    let past = s2.last_state_change - Duration::seconds(1);
    assert!(matches!(
        s2.pause(past),
        Err(CoreError::TimeRegression { .. })
    ));

    // 3. resume() regression
    let mut s3 = create_session_in_state(SessionState::Paused, base);
    let past = s3.last_state_change - Duration::seconds(1);
    assert!(matches!(
        s3.resume(true, past),
        Err(CoreError::TimeRegression { .. })
    ));

    // 4. start_break() regression
    let mut s4 = create_session_in_state(SessionState::Active, base);
    let past = s4.last_state_change - Duration::seconds(1);
    assert!(matches!(
        s4.start_break(past),
        Err(CoreError::TimeRegression { .. })
    ));

    // 5. end_break() regression
    let mut s5 = create_session_in_state(SessionState::Break, base);
    let past = s5.last_state_change - Duration::seconds(1);
    assert!(matches!(
        s5.end_break(past),
        Err(CoreError::TimeRegression { .. })
    ));

    // 6. complete() regression
    let mut s6 = create_session_in_state(SessionState::Active, base);
    let past = s6.last_state_change - Duration::seconds(1);
    assert!(matches!(
        s6.complete(SessionEndReason::GoalAchieved, past),
        Err(CoreError::TimeRegression { .. })
    ));

    // 7. interrupt() regression
    let mut s7 = create_session_in_state(SessionState::Active, base);
    let past = s7.last_state_change - Duration::seconds(1);
    assert!(matches!(
        s7.interrupt("crash".into(), past),
        Err(CoreError::TimeRegression { .. })
    ));

    // 8. recover_resume() regression
    let mut s8 = create_session_in_state(SessionState::Interrupted, base);
    let past = s8.last_state_change - Duration::seconds(1);
    assert!(matches!(
        s8.recover_resume(true, past),
        Err(CoreError::TimeRegression { .. })
    ));

    // 9. recover_reset() regression
    let mut s9 = create_session_in_state(SessionState::Interrupted, base);
    let past = s9.last_state_change - Duration::seconds(1);
    assert!(matches!(
        s9.recover_reset(past),
        Err(CoreError::TimeRegression { .. })
    ));

    // 10. record_idle() regression
    let mut s10 = create_session_in_state(SessionState::Active, base);
    let past = s10.last_state_change - Duration::seconds(1);
    assert!(matches!(
        s10.record_idle(60, past),
        Err(CoreError::TimeRegression { .. })
    ));
}

#[test]
fn test_clock_monotonicity_microsecond_precision() {
    let base = Utc::now();
    let mut session = FocusSession::new(CommitmentId::new(), valid_consent(true), base);
    session.start(base).unwrap();

    // Regression by just 1 microsecond
    let regression = base - Duration::microseconds(1);
    assert!(matches!(
        session.pause(regression),
        Err(CoreError::TimeRegression { .. })
    ));

    // Zero delta (same timestamp) is permitted
    assert!(session.pause(base).is_ok());
}

#[test]
fn test_segment_chronological_ordering() {
    let now = Utc::now();
    let session_id = SessionId::new();

    // end_time precedes start_time
    let res = ActivitySegment::new(
        session_id,
        now,
        now - Duration::milliseconds(1),
        Some("vim".into()),
        None,
        SegmentKind::ActiveFocus,
        1.0,
        vec![],
    );
    assert!(matches!(
        res,
        Err(CoreError::ValidationError { field, .. }) if field == "end_time"
    ));

    // zero duration segment is allowed
    let zero_seg = ActivitySegment::new(
        session_id,
        now,
        now,
        Some("vim".into()),
        None,
        SegmentKind::ActiveFocus,
        1.0,
        vec![],
    );
    assert!(zero_seg.is_ok());
    assert_eq!(zero_seg.unwrap().duration_seconds(), 0);
}

#[test]
fn test_observation_expiration_ordering() {
    let now = Utc::now();
    let session_id = SessionId::new();

    let res = Observation::new(
        session_id,
        now,
        ObservationSource::PlatformApp,
        Some("firefox".into()),
        None,
        None,
        IdleState::Active,
        None,
        Some(now - Duration::seconds(1)),
    );
    assert!(matches!(
        res,
        Err(CoreError::ValidationError { field, .. }) if field == "expires_at"
    ));
}

// ---------------------------------------------------------------------------
// 3. BOUNDS & INPUT SANITIZATION EMPIRICAL TESTS
// ---------------------------------------------------------------------------

#[test]
fn test_ambition_bounds() {
    let now = Utc::now();

    // 1. Empty title
    assert!(matches!(
        Ambition::new("", None, now),
        Err(CoreError::ValidationError { field, .. }) if field == "title"
    ));
    assert!(matches!(
        Ambition::new("   \t\n  ", None, now),
        Err(CoreError::ValidationError { field, .. }) if field == "title"
    ));

    // 2. Exact limit (200 chars) succeeds
    let exact_title = "a".repeat(MAX_AMBITION_TITLE_LENGTH);
    assert!(Ambition::new(exact_title, None, now).is_ok());

    // 3. Oversized title (>200 chars, e.g. 201 or 257 chars)
    let over_title = "a".repeat(MAX_AMBITION_TITLE_LENGTH + 1);
    assert!(matches!(
        Ambition::new(over_title, None, now),
        Err(CoreError::ValidationError { field, .. }) if field == "title"
    ));
    let oversized_257 = "a".repeat(257);
    assert!(matches!(
        Ambition::new(oversized_257, None, now),
        Err(CoreError::ValidationError { field, .. }) if field == "title"
    ));

    // 4. Exact description limit (2000 chars) succeeds
    let exact_desc = "d".repeat(MAX_AMBITION_DESCRIPTION_LENGTH);
    assert!(Ambition::new("Valid", Some(exact_desc), now).is_ok());

    // 5. Oversized description (>2000 chars, e.g. 2001 or 4097 chars)
    let over_desc = "d".repeat(MAX_AMBITION_DESCRIPTION_LENGTH + 1);
    assert!(matches!(
        Ambition::new("Valid", Some(over_desc), now),
        Err(CoreError::ValidationError { field, .. }) if field == "description"
    ));
    let oversized_4097 = "d".repeat(4097);
    assert!(matches!(
        Ambition::new("Valid", Some(oversized_4097), now),
        Err(CoreError::ValidationError { field, .. }) if field == "description"
    ));

    // 6. Ambition update bounds
    let mut ambition = Ambition::new("Valid", None, now).unwrap();
    assert!(ambition.update(Some(String::new()), None, now).is_err());
    assert!(ambition.update(Some("x".repeat(201)), None, now).is_err());
    assert!(ambition
        .update(None, Some(Some("d".repeat(2001))), now)
        .is_err());
}

#[test]
fn test_goal_bounds() {
    let now = Utc::now();
    let ambition_id = AmbitionId::new();

    // 1. Empty title
    assert!(matches!(
        Goal::new(ambition_id, "", None, None, now),
        Err(CoreError::ValidationError { field, .. }) if field == "title"
    ));
    assert!(matches!(
        Goal::new(ambition_id, "   \n\t", None, None, now),
        Err(CoreError::ValidationError { field, .. }) if field == "title"
    ));

    // 2. Exact limit (200 chars) succeeds
    let exact_title = "g".repeat(MAX_GOAL_TITLE_LENGTH);
    assert!(Goal::new(ambition_id, exact_title, None, None, now).is_ok());

    // 3. Oversized title (>200 chars, e.g. 257)
    let over_title = "g".repeat(MAX_GOAL_TITLE_LENGTH + 1);
    assert!(matches!(
        Goal::new(ambition_id, over_title, None, None, now),
        Err(CoreError::ValidationError { field, .. }) if field == "title"
    ));
    let oversized_257 = "g".repeat(257);
    assert!(matches!(
        Goal::new(ambition_id, oversized_257, None, None, now),
        Err(CoreError::ValidationError { field, .. }) if field == "title"
    ));

    // 4. Next action bounds (limit 500 chars)
    let exact_action = "x".repeat(MAX_NEXT_ACTION_LENGTH);
    assert!(Goal::new(ambition_id, "Goal", Some(exact_action), None, now).is_ok());
    let over_action = "x".repeat(MAX_NEXT_ACTION_LENGTH + 1);
    assert!(matches!(
        Goal::new(ambition_id, "Goal", Some(over_action), None, now),
        Err(CoreError::ValidationError { field, .. }) if field == "next_action"
    ));

    // 5. Update next action bounds
    let mut goal = Goal::new(ambition_id, "Goal", None, None, now).unwrap();
    assert!(goal.update_next_action(Some("x".repeat(501)), now).is_err());
    assert!(goal.update_next_action(Some("x".repeat(500)), now).is_ok());
}

#[test]
fn test_commitment_bounds_and_duration_limits() {
    let now = Utc::now();
    let goal_id = GoalId::new();

    // Under minimum (< 300s)
    assert!(matches!(
        Commitment::new(goal_id, now, 0, 0, None, now),
        Err(CoreError::ValidationError { field, .. }) if field == "duration_seconds"
    ));
    assert!(matches!(
        Commitment::new(goal_id, now, 299, 0, None, now),
        Err(CoreError::ValidationError { field, .. }) if field == "duration_seconds"
    ));

    // Exact minimum (300s)
    assert!(Commitment::new(goal_id, now, MIN_SESSION_DURATION_SECONDS, 0, None, now).is_ok());

    // Exact maximum (43,200s / 12h)
    assert!(Commitment::new(goal_id, now, MAX_SESSION_DURATION_SECONDS, 300, None, now).is_ok());

    // Over maximum (> 43,200s)
    assert!(matches!(
        Commitment::new(goal_id, now, MAX_SESSION_DURATION_SECONDS + 1, 300, None, now),
        Err(CoreError::DurationLimitExceeded { limit: 43200, requested }) if requested == 43201
    ));
    assert!(matches!(
        Commitment::new(goal_id, now, 86400, 300, None, now),
        Err(CoreError::DurationLimitExceeded {
            limit: 43200,
            requested: 86400
        })
    ));
    assert!(matches!(
        Commitment::new(goal_id, now, u32::MAX, 300, None, now),
        Err(CoreError::DurationLimitExceeded { limit: 43200, requested }) if requested == u32::MAX
    ));

    // Break allowance exceeds 7200s
    assert!(matches!(
        Commitment::new(goal_id, now, 20000, MAX_BREAK_ALLOWANCE_SECONDS + 1, None, now),
        Err(CoreError::ValidationError { field, .. }) if field == "break_allowance_seconds"
    ));

    // Break allowance equals or exceeds duration
    assert!(matches!(
        Commitment::new(goal_id, now, 600, 600, None, now),
        Err(CoreError::ValidationError { field, .. }) if field == "break_allowance_seconds"
    ));
    assert!(matches!(
        Commitment::new(goal_id, now, 600, 601, None, now),
        Err(CoreError::ValidationError { field, .. }) if field == "break_allowance_seconds"
    ));
}

#[test]
fn test_observation_and_window_title_bounds() {
    let now = Utc::now();
    let session_id = SessionId::new();

    // Exact 256 chars for window title and app id
    let exact_app = "a".repeat(MAX_APP_ID_LENGTH);
    let exact_title = "t".repeat(MAX_WINDOW_TITLE_LENGTH);
    let exact_origin = "o".repeat(MAX_ORIGIN_LENGTH);
    assert!(Observation::new(
        session_id,
        now,
        ObservationSource::WindowTitle,
        Some(exact_app),
        Some(exact_title),
        Some(exact_origin),
        IdleState::Active,
        None,
        None,
    )
    .is_ok());

    // Window title > 256 chars
    let over_title = "t".repeat(MAX_WINDOW_TITLE_LENGTH + 1);
    assert!(matches!(
        Observation::new(
            session_id,
            now,
            ObservationSource::WindowTitle,
            None,
            Some(over_title),
            None,
            IdleState::Active,
            None,
            None,
        ),
        Err(CoreError::ValidationError { field, .. }) if field == "window_title"
    ));

    // App ID > 256 chars
    let over_app = "a".repeat(MAX_APP_ID_LENGTH + 1);
    assert!(matches!(
        Observation::new(
            session_id,
            now,
            ObservationSource::PlatformApp,
            Some(over_app),
            None,
            None,
            IdleState::Active,
            None,
            None,
        ),
        Err(CoreError::ValidationError { field, .. }) if field == "app_id"
    ));

    // Origin > 256 chars
    let over_origin = "o".repeat(MAX_ORIGIN_LENGTH + 1);
    assert!(matches!(
        Observation::new(
            session_id,
            now,
            ObservationSource::BrowserOrigin,
            None,
            None,
            Some(over_origin),
            IdleState::Active,
            None,
            None,
        ),
        Err(CoreError::ValidationError { field, .. }) if field == "origin"
    ));
}

#[test]
fn test_classification_and_correction_bounds() {
    let now = Utc::now();
    let segment_id = SegmentId::new();
    let session_id = SessionId::new();

    // Classification confidence out of [0.0, 1.0]
    assert!(Classification::new(
        segment_id,
        ClassificationLabel::Relevant,
        -0.01,
        ClassificationBasis::DeterministicRule,
        "v1",
        now
    )
    .is_err());
    assert!(Classification::new(
        segment_id,
        ClassificationLabel::Relevant,
        1.01,
        ClassificationBasis::DeterministicRule,
        "v1",
        now
    )
    .is_err());
    assert!(Classification::new(
        segment_id,
        ClassificationLabel::Relevant,
        f32::NAN,
        ClassificationBasis::DeterministicRule,
        "v1",
        now
    )
    .is_err());

    // Classification empty version
    assert!(Classification::new(
        segment_id,
        ClassificationLabel::Relevant,
        0.5,
        ClassificationBasis::DeterministicRule,
        "   ",
        now
    )
    .is_err());

    // User correction with no target
    assert!(matches!(
        UserCorrection::new(
            session_id,
            None,
            None,
            None,
            ClassificationLabel::Relevant,
            CorrectionScope::Session,
            None,
            now,
        ),
        Err(CoreError::ValidationError { field, .. }) if field == "target"
    ));

    // User correction with Unknown label
    assert!(matches!(
        UserCorrection::new(
            session_id,
            Some(segment_id),
            None,
            None,
            ClassificationLabel::Unknown,
            CorrectionScope::Session,
            None,
            now,
        ),
        Err(CoreError::ValidationError { field, .. }) if field == "label"
    ));
}

#[test]
fn test_review_note_bounds_and_validity() {
    let now = Utc::now();
    let session = create_session_in_state(SessionState::Completed, now);

    // Note > 2000 chars
    let over_note = "n".repeat(MAX_REVIEW_NOTE_LENGTH + 1);
    assert!(matches!(
        SessionReview::from_session(
            &session,
            SessionOutcome::CompletedSuccess,
            Some(over_note),
            None,
            now,
        ),
        Err(CoreError::ValidationError { field, .. }) if field == "user_notes"
    ));

    // Note <= 2000 chars succeeds
    let exact_note = "n".repeat(MAX_REVIEW_NOTE_LENGTH);
    assert!(SessionReview::from_session(
        &session,
        SessionOutcome::CompletedSuccess,
        Some(exact_note),
        None,
        now,
    )
    .is_ok());

    // Non-terminal session cannot produce review
    let active_session = create_session_in_state(SessionState::Active, now);
    assert!(matches!(
        SessionReview::from_session(
            &active_session,
            SessionOutcome::CompletedSuccess,
            None,
            None,
            now,
        ),
        Err(CoreError::ValidationError { field, .. }) if field == "session_state"
    ));
}

// ---------------------------------------------------------------------------
// 4. CONSENT VERIFICATION INVARIANTS EMPIRICAL TESTS
// ---------------------------------------------------------------------------

#[test]
fn test_consent_strictly_enforced_on_start() {
    let now = Utc::now();
    // Consent disabled
    let mut session = FocusSession::new(CommitmentId::new(), valid_consent(false), now);
    let res = session.start(now);
    assert!(matches!(res, Err(CoreError::ConsentViolation(_))));
}

#[test]
fn test_consent_strictly_enforced_on_resume() {
    let now = Utc::now();
    let mut session = FocusSession::new(CommitmentId::new(), valid_consent(true), now);
    session.start(now).unwrap();

    let t1 = now + Duration::seconds(10);
    session.pause(t1).unwrap();

    let t2 = t1 + Duration::seconds(10);
    // 1. Resume with consent_confirmed = false
    assert!(matches!(
        session.resume(false, t2),
        Err(CoreError::ConsentViolation(_))
    ));

    // 2. Resume with consent snapshot monitoring_enabled revoked/false
    session.consent.monitoring_enabled = false;
    assert!(matches!(
        session.resume(true, t2),
        Err(CoreError::ConsentViolation(_))
    ));
}

#[test]
fn test_consent_strictly_enforced_on_recover_resume() {
    let now = Utc::now();
    let mut session = FocusSession::new(CommitmentId::new(), valid_consent(true), now);
    session.start(now).unwrap();

    let t1 = now + Duration::seconds(10);
    session.interrupt("crash".into(), t1).unwrap();

    let t2 = t1 + Duration::seconds(10);
    // 1. Recover resume with consent_confirmed = false
    assert!(matches!(
        session.recover_resume(false, t2),
        Err(CoreError::ConsentViolation(_))
    ));

    // 2. Recover resume with monitoring_enabled = false
    session.consent.monitoring_enabled = false;
    assert!(matches!(
        session.recover_resume(true, t2),
        Err(CoreError::ConsentViolation(_))
    ));
}
