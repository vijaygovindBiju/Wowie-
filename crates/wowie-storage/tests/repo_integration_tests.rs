use chrono::{Duration, Utc};
use std::sync::Arc;
use std::thread;
use wowie_core::id::*;
use wowie_core::models::*;
use wowie_storage::*;

fn create_test_db() -> (
    DbPool,
    SqliteGoalRepository,
    SqliteSessionRepository,
    SqliteActivityLogRepository,
    SqliteReviewRepository,
) {
    let pool = DbPool::open_in_memory().expect("Failed to open in-memory DB");
    let goal_repo = SqliteGoalRepository::new(pool.clone());
    let session_repo = SqliteSessionRepository::new(pool.clone());
    let activity_repo = SqliteActivityLogRepository::new(pool.clone());
    let review_repo = SqliteReviewRepository::new(pool.clone());

    (pool, goal_repo, session_repo, activity_repo, review_repo)
}

#[test]
fn test_ambition_and_goal_cascade_delete() {
    let (_pool, goal_repo, _s, _a, _r) = create_test_db();
    let now = Utc::now();

    let ambition = Ambition::new(
        "Master Rust System Programming",
        Some("Deep focus".into()),
        now,
    )
    .unwrap();
    goal_repo.create_ambition(&ambition).unwrap();

    let goal = Goal::new(
        ambition.id,
        "Build Wowie Storage Crate",
        Some("Write SQLite repos".into()),
        None,
        now,
    )
    .unwrap();
    goal_repo.create_goal(&goal).unwrap();

    // Verify both exist
    assert!(goal_repo.get_ambition(ambition.id).unwrap().is_some());
    assert!(goal_repo.get_goal(goal.id).unwrap().is_some());

    // Delete ambition
    goal_repo.delete_ambition(ambition.id).unwrap();

    // Foreign key cascade must remove goal
    assert!(goal_repo.get_ambition(ambition.id).unwrap().is_none());
    assert!(
        goal_repo.get_goal(goal.id).unwrap().is_none(),
        "Goal must be cascade deleted when ambition is removed"
    );
}

#[test]
fn test_goal_filtering_and_archiving() {
    let (_pool, goal_repo, _s, _a, _r) = create_test_db();
    let now = Utc::now();

    let ambition = Ambition::new("Goals Ambition", None, now).unwrap();
    goal_repo.create_ambition(&ambition).unwrap();

    let g1 = Goal::new(ambition.id, "Goal 1", None, None, now).unwrap();
    let mut g2 = Goal::new(ambition.id, "Goal 2", None, None, now).unwrap();
    g2.pause(now).unwrap();

    goal_repo.create_goal(&g1).unwrap();
    goal_repo.create_goal(&g2).unwrap();

    // Filter by status Active
    let active_goals = goal_repo
        .list_goals(GoalFilter {
            status: Some(GoalStatus::Active),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(active_goals.len(), 1);
    assert_eq!(active_goals[0].id, g1.id);

    // Archive goal 1
    goal_repo.archive_goal(g1.id, now).unwrap();

    // List without archived
    let unarchived = goal_repo
        .list_goals(GoalFilter {
            include_archived: false,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(unarchived.len(), 1);
    assert_eq!(unarchived[0].id, g2.id);

    // List with archived
    let all = goal_repo
        .list_goals(GoalFilter {
            include_archived: true,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(all.len(), 2);
}

#[test]
fn test_session_lifecycle_and_review_persistence() {
    let (_pool, goal_repo, session_repo, _activity_repo, review_repo) = create_test_db();
    let now = Utc::now();

    let ambition = Ambition::new("Productivity Mastery", None, now).unwrap();
    goal_repo.create_ambition(&ambition).unwrap();
    let goal = Goal::new(
        ambition.id,
        "Execute M1 Milestone",
        Some("Pass all tests".into()),
        None,
        now,
    )
    .unwrap();
    goal_repo.create_goal(&goal).unwrap();

    let commitment = Commitment::new(goal.id, now, 1500, 300, None, now).unwrap();
    session_repo.create_commitment(&commitment).unwrap();

    let consent = ConsentSnapshot {
        monitoring_enabled: true,
        window_title_redaction_enabled: true,
        browser_origin_enabled: false,
        granted_at: now,
    };
    let mut session = FocusSession::new(commitment.id, consent, now);
    session_repo.create_session(&session).unwrap();

    // Transition: Ready -> Active
    session.start(now).unwrap();
    session_repo.update_session(&session).unwrap();

    // Query active session
    let active = session_repo
        .get_active_session()
        .unwrap()
        .expect("Active session should exist");
    assert_eq!(active.id, session.id);
    assert_eq!(active.state, SessionState::Active);

    // Transition: Active -> Completed
    let end_time = now + Duration::seconds(1500);
    session
        .complete(SessionEndReason::GoalAchieved, end_time)
        .unwrap();
    session_repo.update_session(&session).unwrap();

    // Active session should now be None
    assert!(session_repo.get_active_session().unwrap().is_none());

    // Create and save review
    let review = SessionReview::from_session(
        &session,
        SessionOutcome::CompletedSuccess,
        Some("All goals completed ahead of schedule".into()),
        Some("Proceed to M2 Observer".into()),
        end_time,
    )
    .unwrap();
    review_repo.save_review(&review).unwrap();

    let fetched_review = review_repo
        .get_review_by_session(session.id)
        .unwrap()
        .expect("Review must exist");
    assert_eq!(fetched_review.session_id, session.id);
    assert_eq!(fetched_review.actual_focus_seconds, 1500);
    assert_eq!(fetched_review.outcome, SessionOutcome::CompletedSuccess);
}

#[test]
fn test_active_session_singleton_db_constraint() {
    let (_pool, goal_repo, session_repo, _a, _r) = create_test_db();
    let now = Utc::now();

    let ambition = Ambition::new("Ambition Singleton", None, now).unwrap();
    goal_repo.create_ambition(&ambition).unwrap();
    let goal = Goal::new(ambition.id, "Goal Singleton", None, None, now).unwrap();
    goal_repo.create_goal(&goal).unwrap();

    let commitment = Commitment::new(goal.id, now, 1500, 300, None, now).unwrap();
    session_repo.create_commitment(&commitment).unwrap();

    let consent = ConsentSnapshot {
        monitoring_enabled: true,
        window_title_redaction_enabled: true,
        browser_origin_enabled: false,
        granted_at: now,
    };

    // First session: start and activate
    let mut session1 = FocusSession::new(commitment.id, consent.clone(), now);
    session1.start(now).unwrap();
    session_repo.create_session(&session1).unwrap();

    // Second session: attempting to create or start a second active session must violate the unique index
    let mut session2 = FocusSession::new(commitment.id, consent, now);
    session2.start(now).unwrap();
    let res = session_repo.create_session(&session2);
    assert!(
        res.is_err(),
        "Partial unique index must reject concurrent active session"
    );
}

#[test]
fn test_activity_observations_batch_and_expiry_deletion() {
    let (_pool, goal_repo, session_repo, activity_repo, _r) = create_test_db();
    let now = Utc::now();

    let ambition = Ambition::new("Testing", None, now).unwrap();
    goal_repo.create_ambition(&ambition).unwrap();
    let goal = Goal::new(ambition.id, "Testing", None, None, now).unwrap();
    goal_repo.create_goal(&goal).unwrap();
    let commitment = Commitment::new(goal.id, now, 1500, 300, None, now).unwrap();
    session_repo.create_commitment(&commitment).unwrap();
    let session = FocusSession::new(
        commitment.id,
        ConsentSnapshot {
            monitoring_enabled: true,
            window_title_redaction_enabled: true,
            browser_origin_enabled: false,
            granted_at: now,
        },
        now,
    );
    session_repo.create_session(&session).unwrap();

    let obs1 = Observation::new(
        session.id,
        now - Duration::hours(2),
        ObservationSource::PlatformApp,
        Some("alacritty".into()),
        Some("cargo check".into()),
        None,
        IdleState::Active,
        Some("default".into()),
        Some(now - Duration::hours(1)), // Expired relative to now, but >= observed_at
    )
    .unwrap();

    let obs2 = Observation::new(
        session.id,
        now,
        ObservationSource::PlatformApp,
        Some("firefox".into()),
        Some("docs.rs".into()),
        Some("docs.rs".into()),
        IdleState::Active,
        Some("default".into()),
        Some(now + Duration::hours(1)), // Unexpired
    )
    .unwrap();

    activity_repo
        .record_observations_batch(&[obs1, obs2])
        .unwrap();

    let list = activity_repo
        .get_session_observations(session.id, None)
        .unwrap();
    assert_eq!(list.len(), 2);

    let deleted_count = activity_repo.delete_expired_observations(now).unwrap();
    assert_eq!(deleted_count, 1);

    let list_after = activity_repo
        .get_session_observations(session.id, None)
        .unwrap();
    assert_eq!(list_after.len(), 1);
    assert_eq!(list_after[0].app_id.as_deref(), Some("firefox"));
}

#[test]
fn test_activity_segments_and_classifications() {
    let (_pool, goal_repo, session_repo, activity_repo, _r) = create_test_db();
    let now = Utc::now();

    let ambition = Ambition::new("Ambition Segments", None, now).unwrap();
    goal_repo.create_ambition(&ambition).unwrap();
    let goal = Goal::new(ambition.id, "Goal Segments", None, None, now).unwrap();
    goal_repo.create_goal(&goal).unwrap();
    let commitment = Commitment::new(goal.id, now, 1500, 300, None, now).unwrap();
    session_repo.create_commitment(&commitment).unwrap();
    let session = FocusSession::new(
        commitment.id,
        ConsentSnapshot {
            monitoring_enabled: true,
            window_title_redaction_enabled: true,
            browser_origin_enabled: false,
            granted_at: now,
        },
        now,
    );
    session_repo.create_session(&session).unwrap();

    let obs_id = ObservationId::new();
    let segment = ActivitySegment::new(
        session.id,
        now,
        now + Duration::seconds(30),
        Some("neovim".into()),
        None,
        SegmentKind::ActiveFocus,
        0.98,
        vec![obs_id],
    )
    .unwrap();
    activity_repo.record_segment(&segment).unwrap();

    let fetched_segment = activity_repo
        .get_segment(segment.id)
        .unwrap()
        .expect("Segment should exist");
    assert_eq!(fetched_segment.evidence_ids, vec![obs_id]);

    let classification = Classification::new(
        segment.id,
        ClassificationLabel::Relevant,
        0.99,
        ClassificationBasis::DeterministicRule,
        "rule-engine-v1",
        now,
    )
    .unwrap();
    activity_repo
        .record_classification(&classification)
        .unwrap();

    let classifications = activity_repo
        .get_segment_classifications(segment.id)
        .unwrap();
    assert_eq!(classifications.len(), 1);
    assert_eq!(classifications[0].id, classification.id);
    assert_eq!(classifications[0].label, ClassificationLabel::Relevant);
}

#[test]
fn test_user_corrections_and_scopes() {
    let (_pool, goal_repo, session_repo, activity_repo, _r) = create_test_db();
    let now = Utc::now();

    let ambition = Ambition::new("Ambition Corrections", None, now).unwrap();
    goal_repo.create_ambition(&ambition).unwrap();
    let goal = Goal::new(ambition.id, "Goal Corrections", None, None, now).unwrap();
    goal_repo.create_goal(&goal).unwrap();
    let commitment = Commitment::new(goal.id, now, 1500, 300, None, now).unwrap();
    session_repo.create_commitment(&commitment).unwrap();
    let session = FocusSession::new(
        commitment.id,
        ConsentSnapshot {
            monitoring_enabled: true,
            window_title_redaction_enabled: true,
            browser_origin_enabled: false,
            granted_at: now,
        },
        now,
    );
    session_repo.create_session(&session).unwrap();

    // Current event correction
    let corr1 = UserCorrection::new(
        session.id,
        None,
        Some("slack".into()),
        None,
        ClassificationLabel::PossiblyUnrelated,
        CorrectionScope::CurrentEvent,
        Some("Team coordination".into()),
        now,
    )
    .unwrap();
    activity_repo.record_correction(&corr1).unwrap();

    // Persistent rule correction
    let corr2 = UserCorrection::new(
        session.id,
        None,
        Some("spotify".into()),
        None,
        ClassificationLabel::Relevant,
        CorrectionScope::PersistentRule,
        Some("Focus audio".into()),
        now,
    )
    .unwrap();
    activity_repo.record_correction(&corr2).unwrap();

    let session_corrs = activity_repo.get_session_corrections(session.id).unwrap();
    assert_eq!(session_corrs.len(), 2);

    let persistent = activity_repo.get_persistent_corrections().unwrap();
    assert_eq!(persistent.len(), 1);
    assert_eq!(persistent[0].app_id.as_deref(), Some("spotify"));
}

#[test]
fn test_review_singleton_per_session() {
    let (_pool, goal_repo, session_repo, _a, review_repo) = create_test_db();
    let now = Utc::now();

    let ambition = Ambition::new("Review Ambition", None, now).unwrap();
    goal_repo.create_ambition(&ambition).unwrap();
    let goal = Goal::new(ambition.id, "Review Goal", None, None, now).unwrap();
    goal_repo.create_goal(&goal).unwrap();
    let commitment = Commitment::new(goal.id, now, 1500, 300, None, now).unwrap();
    session_repo.create_commitment(&commitment).unwrap();

    let mut session = FocusSession::new(
        commitment.id,
        ConsentSnapshot {
            monitoring_enabled: true,
            window_title_redaction_enabled: true,
            browser_origin_enabled: false,
            granted_at: now,
        },
        now,
    );
    session.start(now).unwrap();
    session
        .complete(
            SessionEndReason::GoalAchieved,
            now + Duration::seconds(1500),
        )
        .unwrap();
    session_repo.create_session(&session).unwrap();

    let review1 = SessionReview::from_session(
        &session,
        SessionOutcome::CompletedSuccess,
        Some("First review".into()),
        None,
        now,
    )
    .unwrap();
    review_repo.save_review(&review1).unwrap();

    // Attempting to save a second review for the same session must fail unique constraint
    let review2 = SessionReview::from_session(
        &session,
        SessionOutcome::CompletedPartial,
        Some("Duplicate review".into()),
        None,
        now,
    )
    .unwrap();
    let res = review_repo.save_review(&review2);
    assert!(
        res.is_err(),
        "Duplicate review must fail unique constraint on session_id"
    );
}

#[test]
fn test_multithreaded_concurrent_access() {
    let (pool, goal_repo, _s, _a, _r) = create_test_db();
    let now = Utc::now();

    let ambition = Ambition::new("Concurrent Ambition", None, now).unwrap();
    goal_repo.create_ambition(&ambition).unwrap();

    let goal_repo_arc = Arc::new(goal_repo);
    let ambition_id = ambition.id;

    let mut handles = Vec::new();
    for i in 0..8 {
        let repo = Arc::clone(&goal_repo_arc);
        let handle = thread::spawn(move || {
            let title = format!("Concurrent Goal {i}");
            let goal = Goal::new(ambition_id, title, None, None, Utc::now()).unwrap();
            repo.create_goal(&goal).unwrap();
            let fetched = repo.get_goal(goal.id).unwrap().expect("Must find goal");
            assert_eq!(fetched.id, goal.id);
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().expect("Thread panicked");
    }

    // Verify all 8 goals were persisted cleanly
    let goals = goal_repo_arc
        .list_goals(GoalFilter {
            ambition_id: Some(ambition_id),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(goals.len(), 8);

    // Verify pool is still completely healthy
    pool.with_conn(|conn| {
        let count: i64 = conn.query_row("SELECT count(*) FROM goals", [], |r| r.get(0))?;
        assert_eq!(count, 8);
        Ok(())
    })
    .unwrap();
}

#[test]
fn test_goal_unarchive_lifecycle_and_query_visibility_regression() {
    let (_pool, goal_repo, _s, _a, _r) = create_test_db();
    let now = Utc::now();

    let ambition = Ambition::new("Lifecycle Ambition", None, now).unwrap();
    goal_repo.create_ambition(&ambition).unwrap();

    let mut goal = Goal::new(
        ambition.id,
        "Lifecycle Goal",
        Some("Verify unarchive".into()),
        None,
        now,
    )
    .unwrap();
    goal_repo.create_goal(&goal).unwrap();

    // 1. Verify active goal is listed when include_archived is false
    let active_list = goal_repo
        .list_goals(GoalFilter {
            include_archived: false,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(active_list.len(), 1);
    assert_eq!(active_list[0].id, goal.id);

    // 2. Archive goal
    let t_archive = now + Duration::hours(1);
    goal.archive(t_archive).unwrap();
    goal_repo.update_goal(&goal).unwrap();

    // 3. Verify archived goal is hidden from non-archived listing
    let active_after_archive = goal_repo
        .list_goals(GoalFilter {
            include_archived: false,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(active_after_archive.len(), 0);

    let all_list = goal_repo
        .list_goals(GoalFilter {
            include_archived: true,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(all_list.len(), 1);
    assert_eq!(all_list[0].archived_at, Some(t_archive));

    // 4. Resume goal (unarchive via model)
    let t_resume = t_archive + Duration::hours(1);
    goal.resume(t_resume).unwrap();
    assert!(goal.archived_at.is_none());
    goal_repo.update_goal(&goal).unwrap();

    // 5. REGRESSION ASSERTION: Resumed goal MUST be returned by list_goals(include_archived: false)
    let active_after_resume = goal_repo
        .list_goals(GoalFilter {
            status: Some(GoalStatus::Active),
            include_archived: false,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        active_after_resume.len(),
        1,
        "Resumed goal must be visible in active listings"
    );
    assert_eq!(active_after_resume[0].id, goal.id);
    assert!(active_after_resume[0].archived_at.is_none());

    // 6. Direct retrieval verification
    let fetched = goal_repo
        .get_goal(goal.id)
        .unwrap()
        .expect("Goal must exist");
    assert_eq!(fetched.status, GoalStatus::Active);
    assert_eq!(fetched.archived_at, None);

    // 7. Verify direct repository archive_goal and unarchive_goal methods
    let t_archive2 = t_resume + Duration::hours(1);
    goal_repo.archive_goal(goal.id, t_archive2).unwrap();
    let hidden = goal_repo
        .list_goals(GoalFilter {
            include_archived: false,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(hidden.len(), 0);

    let t_unarchive2 = t_archive2 + Duration::hours(1);
    goal_repo.unarchive_goal(goal.id, t_unarchive2).unwrap();
    let visible = goal_repo
        .list_goals(GoalFilter {
            status: Some(GoalStatus::Active),
            include_archived: false,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].id, goal.id);
    assert!(visible[0].archived_at.is_none());
}

#[test]
fn test_session_state_machine_illegal_transition_rejection_regression() {
    let (_pool, goal_repo, session_repo, _a, _r) = create_test_db();
    let now = Utc::now();

    let ambition = Ambition::new("SM Ambition", None, now).unwrap();
    goal_repo.create_ambition(&ambition).unwrap();
    let goal = Goal::new(ambition.id, "SM Goal", None, None, now).unwrap();
    goal_repo.create_goal(&goal).unwrap();
    let commitment = Commitment::new(goal.id, now, 1500, 300, None, now).unwrap();
    session_repo.create_commitment(&commitment).unwrap();

    let consent = ConsentSnapshot {
        monitoring_enabled: true,
        window_title_redaction_enabled: true,
        browser_origin_enabled: false,
        granted_at: now,
    };
    let mut session = FocusSession::new(commitment.id, consent, now);
    session_repo.create_session(&session).unwrap();

    // 1. Attempt complete on Ready session -> must fail
    let err_complete =
        session.complete(SessionEndReason::GoalAchieved, now + Duration::seconds(10));
    assert!(matches!(
        err_complete,
        Err(wowie_core::error::CoreError::InvalidStateTransition { .. })
    ));

    // Verify DB session is still in Ready state with started_at == None
    let db_sess = session_repo.get_session(session.id).unwrap().unwrap();
    assert_eq!(db_sess.state, SessionState::Ready);
    assert!(db_sess.started_at.is_none());

    // 2. Attempt interrupt on Ready session -> must fail
    let err_interrupt = session.interrupt("early crash".into(), now + Duration::seconds(10));
    assert!(matches!(
        err_interrupt,
        Err(wowie_core::error::CoreError::InvalidStateTransition { .. })
    ));

    // 3. Legally start session
    session.start(now + Duration::seconds(10)).unwrap();
    session_repo.update_session(&session).unwrap();

    // 4. Legally interrupt session
    let t_int = now + Duration::seconds(300);
    session.interrupt("SIGINT".into(), t_int).unwrap();
    session_repo.update_session(&session).unwrap();

    // 5. Attempt second interrupt on already Interrupted session -> must fail
    let err_reinterrupt = session.interrupt("second SIGINT".into(), t_int + Duration::seconds(10));
    assert!(matches!(
        err_reinterrupt,
        Err(wowie_core::error::CoreError::InvalidStateTransition { .. })
    ));

    // 6. Recover reset session -> transitions to Ready, clears end_reason and started_at
    let t_reset = t_int + Duration::seconds(20);
    session.recover_reset(t_reset).unwrap();
    assert_eq!(session.state, SessionState::Ready);
    assert!(session.end_reason.is_none());
    assert!(session.started_at.is_none());
    session_repo.update_session(&session).unwrap();

    // Verify DB session after recover_reset has no end_reason or started_at
    let db_reset = session_repo.get_session(session.id).unwrap().unwrap();
    assert_eq!(db_reset.state, SessionState::Ready);
    assert!(db_reset.end_reason.is_none());
    assert!(db_reset.started_at.is_none());

    // 7. Legally start anew
    assert!(session.start(t_reset + Duration::seconds(1)).is_ok());
}
