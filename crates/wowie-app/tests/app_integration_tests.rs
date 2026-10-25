use wowie_app::cli::CliRunner;
use wowie_app::config::AppSettings;
use wowie_app::coordinator::{UserInterventionChoice, WowieCoordinator};
use wowie_app::state::AppState;
use wowie_app::ui::{Screen, UiRenderer};
use wowie_core::models::{
    ClassificationLabel, CorrectionScope, SessionEndReason, SessionOutcome, SessionState,
};
use wowie_storage::{ActivityLogRepository, ReviewRepository, SessionFilter, SessionRepository};

#[tokio::test]
async fn test_e2e_focus_session_lifecycle_and_persistence() {
    let settings = AppSettings::default();
    let state = AppState::in_memory(settings).expect("In memory app state");
    let coordinator = WowieCoordinator::new(state);

    // 1. Create Goal
    let goal = coordinator
        .create_goal(
            "Master Systems Programming",
            Some("Study Rust and OS internals"),
        )
        .expect("Creates goal");
    assert_eq!(goal.title, "Master Systems Programming");

    // 2. Create Commitment
    let commitment = coordinator
        .create_commitment(goal.id, 45, 10)
        .expect("Creates commitment");
    assert_eq!(commitment.duration_seconds, 45 * 60);

    // 3. Start Session
    let session = coordinator
        .start_focus_session(commitment.id)
        .await
        .expect("Starts session");
    assert_eq!(session.state, SessionState::Active);
    assert!(session.consent.monitoring_enabled);

    // 4. Tick observation and evaluation
    let int_res = coordinator.tick().await.expect("Tick succeeds");
    assert!(
        int_res.is_none(),
        "First sample is not sustained distraction"
    );

    // 5. Pause and Resume
    coordinator.pause_session().await.expect("Pause session");
    assert_eq!(
        coordinator
            .state()
            .active_session
            .read()
            .await
            .as_ref()
            .unwrap()
            .state,
        SessionState::Paused
    );

    coordinator.resume_session().await.expect("Resume session");
    assert_eq!(
        coordinator
            .state()
            .active_session
            .read()
            .await
            .as_ref()
            .unwrap()
            .state,
        SessionState::Active
    );

    // 6. User Intervention resolution
    coordinator
        .resolve_intervention(UserInterventionChoice::Correct {
            scope: CorrectionScope::Session,
            label: ClassificationLabel::Relevant,
        })
        .await
        .expect("Resolves correction");

    // 7. Complete Session
    let review = coordinator
        .complete_session(
            SessionEndReason::GoalAchieved,
            Some("Completed chapter 4"),
            Some(5),
        )
        .await
        .expect("Completes session");

    assert_eq!(review.outcome, SessionOutcome::CompletedSuccess);
    assert_eq!(review.user_notes.as_deref(), Some("Completed chapter 4"));

    // Verify persistence in SQLite
    let persisted_sessions = coordinator
        .state()
        .session_repo
        .list_sessions(SessionFilter::default())
        .expect("Lists sessions");
    assert_eq!(persisted_sessions.len(), 1);
    assert_eq!(persisted_sessions[0].state, SessionState::Completed);

    let persisted_review = coordinator
        .state()
        .review_repo
        .get_review_by_session(session.id)
        .expect("Gets review")
        .expect("Review exists");
    assert_eq!(
        persisted_review.user_notes.as_deref(),
        Some("Completed chapter 4")
    );
}

#[tokio::test]
async fn test_ui_all_eight_screens_render_cleanly() {
    let settings = AppSettings::default();
    let state = AppState::in_memory(settings).expect("In memory app state");
    let coordinator = WowieCoordinator::new(state);

    for screen in Screen::ALL {
        let rendered = UiRenderer::render_screen(screen, &coordinator).await;
        assert!(
            !rendered.is_empty(),
            "Screen {:?} must render content",
            screen
        );
        assert!(rendered.contains("WOWIE - AI DESKTOP COMPANION"));
        assert!(rendered.contains(screen.title()));
    }
}

#[tokio::test]
async fn test_cli_runner_e2e_workflow() {
    let settings = AppSettings::default();
    let state = AppState::in_memory(settings).expect("In memory app state");
    let runner = CliRunner::from_coordinator(WowieCoordinator::new(state));

    let res = runner.run_e2e_workflow().await;
    assert!(
        res.is_ok(),
        "E2E workflow must execute cleanly without error"
    );
}

#[tokio::test]
async fn test_user_correction_propagation_and_cooldown_sync() {
    let settings = AppSettings::default();
    let state = AppState::in_memory(settings).expect("In memory app state");
    let coordinator = WowieCoordinator::new(state);

    let goal = coordinator.create_goal("Study Rust", None).unwrap();
    let commitment = coordinator.create_commitment(goal.id, 30, 5).unwrap();
    let session = coordinator
        .start_focus_session(commitment.id)
        .await
        .unwrap();

    // 1. First tick samples current active window
    let _ = coordinator.tick().await.unwrap();

    // 2. Resolve with a classification correction
    coordinator
        .resolve_intervention(UserInterventionChoice::Correct {
            scope: CorrectionScope::Session,
            label: ClassificationLabel::Relevant,
        })
        .await
        .unwrap();

    // 3. Verify stored in SQLite
    let corrections = coordinator
        .state()
        .activity_repo
        .get_session_corrections(session.id)
        .unwrap();
    assert_eq!(corrections.len(), 1);
    assert_eq!(corrections[0].label, ClassificationLabel::Relevant);

    // 4. Consecutive distractions reset to 0
    assert_eq!(
        *coordinator.state().consecutive_distractions.read().await,
        0
    );
}

#[tokio::test]
async fn test_snooze_suppresses_intervention_in_cooldown_tracker() {
    let settings = AppSettings::default();
    let state = AppState::in_memory(settings).expect("In memory app state");
    let coordinator = WowieCoordinator::new(state);

    let goal = coordinator.create_goal("Study Rust", None).unwrap();
    let commitment = coordinator.create_commitment(goal.id, 30, 5).unwrap();
    let _session = coordinator
        .start_focus_session(commitment.id)
        .await
        .unwrap();

    let now = chrono::Utc::now();
    assert!(!coordinator.state().coach_engine.is_snoozed(now));

    // Snooze for 15 minutes
    coordinator
        .resolve_intervention(UserInterventionChoice::Snooze { minutes: 15 })
        .await
        .unwrap();

    assert!(coordinator.state().coach_engine.is_snoozed(now));
    assert!(
        coordinator
            .state()
            .coach_engine
            .remaining_snooze_seconds(now)
            > 800
    );
}

#[tokio::test]
async fn test_break_and_idle_state_tracking() {
    let settings = AppSettings::default();
    let state = AppState::in_memory(settings).expect("In memory app state");
    let coordinator = WowieCoordinator::new(state);

    let goal = coordinator.create_goal("Study Rust", None).unwrap();
    let commitment = coordinator.create_commitment(goal.id, 30, 5).unwrap();
    let _session = coordinator
        .start_focus_session(commitment.id)
        .await
        .unwrap();

    assert!(!coordinator.state().coach_engine.is_break_state());
    assert!(!coordinator.state().coach_engine.is_idle());

    // Break
    coordinator.start_break().await.unwrap();
    assert!(coordinator.state().coach_engine.is_break_state());

    coordinator.end_break().await.unwrap();
    assert!(!coordinator.state().coach_engine.is_break_state());

    // Pause (idle)
    coordinator.pause_session().await.unwrap();
    assert!(coordinator.state().coach_engine.is_idle());

    coordinator.resume_session().await.unwrap();
    assert!(!coordinator.state().coach_engine.is_idle());
}

#[tokio::test]
async fn test_active_session_recovery_after_restart() {
    let temp_db =
        std::env::temp_dir().join(format!("wowie_restart_test_{}.db", uuid::Uuid::new_v4()));
    let settings = AppSettings::default();

    // 1. Process A starts session
    let state_a = AppState::new_with_db(Some(temp_db.clone()), settings.clone()).unwrap();
    let coordinator_a = WowieCoordinator::new(state_a);
    let goal = coordinator_a.create_goal("Build Kernel", None).unwrap();
    let commitment = coordinator_a.create_commitment(goal.id, 60, 10).unwrap();
    let session = coordinator_a
        .start_focus_session(commitment.id)
        .await
        .unwrap();

    // 2. Process A exits (dropped)
    drop(coordinator_a);

    // 3. Process B restarts using the same database file
    let state_b = AppState::new_with_db(Some(temp_db.clone()), settings).unwrap();
    let coordinator_b = WowieCoordinator::new(state_b);

    // Verify session was recovered on startup
    let active = coordinator_b.state().active_session.read().await.clone();
    assert!(
        active.is_some(),
        "Active session must be recovered on restart"
    );
    let recovered = active.unwrap();
    assert_eq!(recovered.id, session.id);
    assert_eq!(recovered.state, SessionState::Active);

    // Clean up
    let _ = coordinator_b
        .complete_session(
            SessionEndReason::GoalAchieved,
            Some("Recovered and finished"),
            Some(5),
        )
        .await;
    let _ = std::fs::remove_file(temp_db);
}
