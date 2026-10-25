use chrono::{Duration, Utc};
use rusqlite::OptionalExtension;
use std::fs;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Barrier,
};
use std::thread;
use std::time::Instant;
use uuid::Uuid;
use wowie_core::id::*;
use wowie_core::models::*;
use wowie_storage::*;

struct TempDbGuard {
    path: PathBuf,
}

impl TempDbGuard {
    fn new(prefix: &str) -> Self {
        let unique = Uuid::new_v4();
        let path = std::env::temp_dir().join(format!("wowie_adv_{prefix}_{unique}.db"));
        Self { path }
    }

    fn path(&self) -> &PathBuf {
        &self.path
    }
}

impl Drop for TempDbGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
        let _ = fs::remove_file(format!("{}-wal", self.path.display()));
        let _ = fs::remove_file(format!("{}-shm", self.path.display()));
    }
}

fn create_in_memory_suite() -> (
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

// =========================================================================
// CHALLENGE 1: ACTIVE SESSION SINGLETON CONSTRAINT
// =========================================================================

#[test]
fn challenge_1_active_session_singleton_state_combinations() {
    let (_pool, goal_repo, session_repo, _a, _r) = create_in_memory_suite();
    let now = Utc::now();

    let ambition = Ambition::new("Singleton Ambition", None, now).unwrap();
    goal_repo.create_ambition(&ambition).unwrap();
    let goal = Goal::new(ambition.id, "Singleton Goal", None, None, now).unwrap();
    goal_repo.create_goal(&goal).unwrap();

    let commitment = Commitment::new(goal.id, now, 1500, 300, None, now).unwrap();
    session_repo.create_commitment(&commitment).unwrap();

    let consent = ConsentSnapshot {
        monitoring_enabled: true,
        window_title_redaction_enabled: true,
        browser_origin_enabled: false,
        granted_at: now,
    };

    // Sub-test 1.1: Active + Active -> must reject second
    {
        let mut s1 = FocusSession::new(commitment.id, consent.clone(), now);
        s1.start(now).unwrap(); // State: Active
        session_repo.create_session(&s1).unwrap();

        let mut s2 = FocusSession::new(commitment.id, consent.clone(), now);
        s2.start(now).unwrap(); // State: Active
        let res = session_repo.create_session(&s2);
        assert!(
            res.is_err(),
            "Concurrent Active + Active sessions must be rejected by unique index"
        );

        // Retire s1 to Completed so next tests can run
        s1.complete(
            SessionEndReason::GoalAchieved,
            now + Duration::seconds(1500),
        )
        .unwrap();
        session_repo.update_session(&s1).unwrap();
    }

    // Sub-test 1.2: Active + Paused -> must reject second
    {
        let mut s_active = FocusSession::new(commitment.id, consent.clone(), now);
        s_active.start(now).unwrap();
        session_repo.create_session(&s_active).unwrap();

        let mut s_paused = FocusSession::new(commitment.id, consent.clone(), now);
        s_paused.start(now).unwrap();
        s_paused.pause(now + Duration::seconds(10)).unwrap(); // State: Paused
        let res = session_repo.create_session(&s_paused);
        assert!(
            res.is_err(),
            "Concurrent Active + Paused sessions must be rejected by unique index"
        );

        s_active
            .complete(
                SessionEndReason::GoalAchieved,
                now + Duration::seconds(1500),
            )
            .unwrap();
        session_repo.update_session(&s_active).unwrap();
    }

    // Sub-test 1.3: Paused + Paused -> must reject second
    {
        let mut s_p1 = FocusSession::new(commitment.id, consent.clone(), now);
        s_p1.start(now).unwrap();
        s_p1.pause(now + Duration::seconds(10)).unwrap();
        session_repo.create_session(&s_p1).unwrap();

        let mut s_p2 = FocusSession::new(commitment.id, consent.clone(), now);
        s_p2.start(now).unwrap();
        s_p2.pause(now + Duration::seconds(10)).unwrap();
        let res = session_repo.create_session(&s_p2);
        assert!(
            res.is_err(),
            "Concurrent Paused + Paused sessions must be rejected by unique index"
        );

        s_p1.complete(
            SessionEndReason::GoalAchieved,
            now + Duration::seconds(1500),
        )
        .unwrap();
        session_repo.update_session(&s_p1).unwrap();
    }

    // Sub-test 1.4: Active + Break -> must reject second
    {
        let mut s_active = FocusSession::new(commitment.id, consent.clone(), now);
        s_active.start(now).unwrap();
        session_repo.create_session(&s_active).unwrap();

        let mut s_break = FocusSession::new(commitment.id, consent.clone(), now);
        s_break.start(now).unwrap();
        s_break.start_break(now + Duration::seconds(10)).unwrap(); // State: Break
        let res = session_repo.create_session(&s_break);
        assert!(
            res.is_err(),
            "Concurrent Active + Break sessions must be rejected by unique index"
        );

        s_active
            .complete(
                SessionEndReason::GoalAchieved,
                now + Duration::seconds(1500),
            )
            .unwrap();
        session_repo.update_session(&s_active).unwrap();
    }

    // Sub-test 1.5: Break + Paused -> must reject second
    {
        let mut s_break = FocusSession::new(commitment.id, consent.clone(), now);
        s_break.start(now).unwrap();
        s_break.start_break(now + Duration::seconds(10)).unwrap();
        session_repo.create_session(&s_break).unwrap();

        let mut s_paused = FocusSession::new(commitment.id, consent, now);
        s_paused.start(now).unwrap();
        s_paused.pause(now + Duration::seconds(10)).unwrap();
        let res = session_repo.create_session(&s_paused);
        assert!(
            res.is_err(),
            "Concurrent Break + Paused sessions must be rejected by unique index"
        );

        s_break
            .complete(
                SessionEndReason::GoalAchieved,
                now + Duration::seconds(1500),
            )
            .unwrap();
        session_repo.update_session(&s_break).unwrap();
    }
}

#[test]
fn challenge_1_active_session_singleton_update_evasion() {
    let (_pool, goal_repo, session_repo, _a, _r) = create_in_memory_suite();
    let now = Utc::now();

    let ambition = Ambition::new("Update Evasion Ambition", None, now).unwrap();
    goal_repo.create_ambition(&ambition).unwrap();
    let goal = Goal::new(ambition.id, "Update Evasion Goal", None, None, now).unwrap();
    goal_repo.create_goal(&goal).unwrap();
    let commitment = Commitment::new(goal.id, now, 1500, 300, None, now).unwrap();
    session_repo.create_commitment(&commitment).unwrap();

    let consent = ConsentSnapshot {
        monitoring_enabled: true,
        window_title_redaction_enabled: true,
        browser_origin_enabled: false,
        granted_at: now,
    };

    // Create session 1: Started and Active
    let mut s1 = FocusSession::new(commitment.id, consent.clone(), now);
    s1.start(now).unwrap();
    session_repo.create_session(&s1).unwrap();

    // Create session 2: In Ready state (allowed!)
    let s2 = FocusSession::new(commitment.id, consent, now);
    assert_eq!(s2.state, SessionState::Ready);
    session_repo.create_session(&s2).unwrap();

    // Attempt to update session 2 state to Active via update_session_state
    let res_active = session_repo.update_session_state(
        s2.id,
        SessionState::Active,
        None,
        None,
        now + Duration::seconds(10),
    );
    assert!(
        res_active.is_err(),
        "Updating second session to Active via update_session_state must be rejected"
    );

    // Attempt to update session 2 state to Paused
    let res_paused = session_repo.update_session_state(
        s2.id,
        SessionState::Paused,
        None,
        None,
        now + Duration::seconds(10),
    );
    assert!(
        res_paused.is_err(),
        "Updating second session to Paused via update_session_state must be rejected"
    );

    // Attempt to update session 2 state to Break
    let res_break = session_repo.update_session_state(
        s2.id,
        SessionState::Break,
        None,
        None,
        now + Duration::seconds(10),
    );
    assert!(
        res_break.is_err(),
        "Updating second session to Break via update_session_state must be rejected"
    );

    // Verify session 2 is still in Ready state in DB
    let s2_fetched = session_repo.get_session(s2.id).unwrap().unwrap();
    assert_eq!(s2_fetched.state, SessionState::Ready);

    // Now retire s1 to Interrupted
    session_repo
        .update_session_state(
            s1.id,
            SessionState::Interrupted,
            Some(&SessionEndReason::SystemInterrupted {
                details: "test".into(),
            }),
            Some(now + Duration::seconds(100)),
            now + Duration::seconds(100),
        )
        .unwrap();

    // Now updating session 2 to Active must SUCCEED
    session_repo
        .update_session_state(
            s2.id,
            SessionState::Active,
            None,
            None,
            now + Duration::seconds(110),
        )
        .unwrap();

    let s2_active = session_repo.get_active_session().unwrap().unwrap();
    assert_eq!(s2_active.id, s2.id);
    assert_eq!(s2_active.state, SessionState::Active);
}

#[test]
fn challenge_1_active_session_singleton_direct_sql_and_casing() {
    let (pool, goal_repo, session_repo, _a, _r) = create_in_memory_suite();
    let now = Utc::now();

    let ambition = Ambition::new("Direct SQL Ambition", None, now).unwrap();
    goal_repo.create_ambition(&ambition).unwrap();
    let goal = Goal::new(ambition.id, "Direct SQL Goal", None, None, now).unwrap();
    goal_repo.create_goal(&goal).unwrap();
    let commitment = Commitment::new(goal.id, now, 1500, 300, None, now).unwrap();
    session_repo.create_commitment(&commitment).unwrap();

    let c_id = commitment.id.to_string();
    let now_str = now.to_rfc3339();

    // Insert first session in lowercase 'active'
    let s1_id = SessionId::new().to_string();
    pool.with_conn(|conn| {
        conn.execute(
            "INSERT INTO focus_sessions (id, commitment_id, state, consent_snapshot, last_state_change, created_at, updated_at)
             VALUES (?1, ?2, 'active', '{}', ?3, ?3, ?3)",
            rusqlite::params![s1_id, c_id, now_str],
        )?;
        Ok(())
    }).unwrap();

    // Attempt to insert second session in uppercase 'Active'
    let s2_id = SessionId::new().to_string();
    let res = pool.with_conn(|conn| {
        conn.execute(
            "INSERT INTO focus_sessions (id, commitment_id, state, consent_snapshot, last_state_change, created_at, updated_at)
             VALUES (?1, ?2, 'Active', '{}', ?3, ?3, ?3)",
            rusqlite::params![s2_id, c_id, now_str],
        )?;
        Ok(())
    });
    assert!(
        res.is_err(),
        "Direct SQL: 'Active' must conflict with existing 'active' session"
    );

    // Attempt to insert third session in 'paused'
    let s3_id = SessionId::new().to_string();
    let res = pool.with_conn(|conn| {
        conn.execute(
            "INSERT INTO focus_sessions (id, commitment_id, state, consent_snapshot, last_state_change, created_at, updated_at)
             VALUES (?1, ?2, 'paused', '{}', ?3, ?3, ?3)",
            rusqlite::params![s3_id, c_id, now_str],
        )?;
        Ok(())
    });
    assert!(
        res.is_err(),
        "Direct SQL: 'paused' must conflict with existing 'active' session"
    );

    // Test raw SQL UPDATE evasion: insert a 'ready' session and update it to 'Active'
    let s_ready_id = SessionId::new().to_string();
    pool.with_conn(|conn| {
        conn.execute(
            "INSERT INTO focus_sessions (id, commitment_id, state, consent_snapshot, last_state_change, created_at, updated_at)
             VALUES (?1, ?2, 'ready', '{}', ?3, ?3, ?3)",
            rusqlite::params![s_ready_id, c_id, now_str],
        )?;
        Ok(())
    }).unwrap();

    let update_res = pool.with_conn(|conn| {
        conn.execute(
            "UPDATE focus_sessions SET state = 'Active' WHERE id = ?1",
            rusqlite::params![s_ready_id],
        )?;
        Ok(())
    });
    assert!(
        update_res.is_err(),
        "Direct raw SQL UPDATE to 'Active' while another is active must be rejected by unique constraint"
    );

    // Verify non-active states allow multiple insertions
    for i in 0..5 {
        let sid = SessionId::new().to_string();
        let state = if i % 2 == 0 { "completed" } else { "ready" };
        pool.with_conn(|conn| {
            conn.execute(
                "INSERT INTO focus_sessions (id, commitment_id, state, consent_snapshot, last_state_change, created_at, updated_at)
                 VALUES (?1, ?2, ?3, '{}', ?4, ?4, ?4)",
                rusqlite::params![sid, c_id, state, now_str],
            )?;
            Ok(())
        }).unwrap();
    }
}

#[test]
fn challenge_1_active_session_singleton_concurrent_race() {
    let (pool, goal_repo, session_repo, _a, _r) = create_in_memory_suite();
    let now = Utc::now();

    let ambition = Ambition::new("Race Ambition", None, now).unwrap();
    goal_repo.create_ambition(&ambition).unwrap();
    let goal = Goal::new(ambition.id, "Race Goal", None, None, now).unwrap();
    goal_repo.create_goal(&goal).unwrap();
    let commitment = Commitment::new(goal.id, now, 1500, 300, None, now).unwrap();
    session_repo.create_commitment(&commitment).unwrap();

    let num_threads = 16;
    let barrier = Arc::new(Barrier::new(num_threads));
    let success_count = Arc::new(AtomicUsize::new(0));
    let failure_count = Arc::new(AtomicUsize::new(0));
    let session_repo_arc = Arc::new(session_repo);

    let mut handles = Vec::new();
    for _ in 0..num_threads {
        let b = Arc::clone(&barrier);
        let repo = Arc::clone(&session_repo_arc);
        let succ = Arc::clone(&success_count);
        let fail = Arc::clone(&failure_count);
        let c_id = commitment.id;

        let handle = thread::spawn(move || {
            let consent = ConsentSnapshot {
                monitoring_enabled: true,
                window_title_redaction_enabled: true,
                browser_origin_enabled: false,
                granted_at: Utc::now(),
            };
            let mut session = FocusSession::new(c_id, consent, Utc::now());
            session.start(Utc::now()).unwrap();

            // Synchronize all threads right before insertion
            b.wait();

            match repo.create_session(&session) {
                Ok(()) => {
                    succ.fetch_add(1, Ordering::SeqCst);
                }
                Err(_) => {
                    fail.fetch_add(1, Ordering::SeqCst);
                }
            }
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().expect("Thread panicked");
    }

    assert_eq!(
        success_count.load(Ordering::SeqCst),
        1,
        "Exactly ONE concurrent thread must succeed in creating an active session"
    );
    assert_eq!(
        failure_count.load(Ordering::SeqCst),
        num_threads - 1,
        "All other concurrent threads must fail due to unique constraint"
    );

    // Verify active session count in DB is exactly 1
    pool.with_conn(|conn| {
        let count: i64 = conn.query_row(
            "SELECT count(*) FROM focus_sessions WHERE state IN ('Active', 'active', 'Paused', 'paused', 'Break', 'break')",
            [],
            |r| r.get(0),
        )?;
        assert_eq!(count, 1);
        Ok(())
    }).unwrap();
}

// =========================================================================
// CHALLENGE 2: CASCADING DELETES AND ZERO ORPHAN ROWS
// =========================================================================

#[test]
fn challenge_2_cascade_deletes_full_hierarchy() {
    let (pool, goal_repo, session_repo, activity_repo, review_repo) = create_in_memory_suite();
    let now = Utc::now();

    // 1. Create Ambition
    let ambition = Ambition::new("Full Cascade Ambition", Some("Root".into()), now).unwrap();
    goal_repo.create_ambition(&ambition).unwrap();

    // 2. Create 2 Goals
    let goal1 = Goal::new(ambition.id, "Goal 1", Some("Action 1".into()), None, now).unwrap();
    let goal2 = Goal::new(ambition.id, "Goal 2", Some("Action 2".into()), None, now).unwrap();
    goal_repo.create_goal(&goal1).unwrap();
    goal_repo.create_goal(&goal2).unwrap();

    // Add User Rules to Goal 1
    pool.with_conn(|conn| {
        conn.execute(
            "INSERT INTO user_rules (id, pattern_type, pattern_value, relevance, goal_id, is_enabled, created_at, updated_at)
             VALUES (?1, 'app_id', 'code', 'relevant', ?2, 1, ?3, ?3)",
            rusqlite::params![Uuid::new_v4().to_string(), goal1.id.to_string(), now.to_rfc3339()],
        )?;
        Ok(())
    }).unwrap();

    // 3. Create Commitments under Goal 1 and Goal 2
    let c1 = Commitment::new(goal1.id, now, 1500, 300, None, now).unwrap();
    let c2 = Commitment::new(goal2.id, now, 1500, 300, None, now).unwrap();
    session_repo.create_commitment(&c1).unwrap();
    session_repo.create_commitment(&c2).unwrap();

    let consent = ConsentSnapshot {
        monitoring_enabled: true,
        window_title_redaction_enabled: true,
        browser_origin_enabled: false,
        granted_at: now,
    };

    // 4. Create Sessions under C1 and C2 (in Completed state so multiple can exist)
    let mut s1 = FocusSession::new(c1.id, consent.clone(), now);
    s1.start(now).unwrap();
    s1.complete(
        SessionEndReason::GoalAchieved,
        now + Duration::seconds(1500),
    )
    .unwrap();
    session_repo.create_session(&s1).unwrap();

    let mut s2 = FocusSession::new(c2.id, consent, now);
    s2.start(now).unwrap();
    s2.complete(
        SessionEndReason::GoalAchieved,
        now + Duration::seconds(1500),
    )
    .unwrap();
    session_repo.create_session(&s2).unwrap();

    // 5. Populate child records for s1:
    // 5a. Observations
    let obs1 = Observation::new(
        s1.id,
        now,
        ObservationSource::PlatformApp,
        Some("nvim".into()),
        None,
        None,
        IdleState::Active,
        None,
        None,
    )
    .unwrap();
    let obs2 = Observation::new(
        s1.id,
        now + Duration::seconds(10),
        ObservationSource::WindowTitle,
        Some("firefox".into()),
        Some("Docs".into()),
        None,
        IdleState::Active,
        None,
        None,
    )
    .unwrap();
    activity_repo.record_observation(&obs1).unwrap();
    activity_repo.record_observation(&obs2).unwrap();

    // 5b. Segments
    let segment = ActivitySegment::new(
        s1.id,
        now,
        now + Duration::seconds(60),
        Some("nvim".into()),
        None,
        SegmentKind::ActiveFocus,
        0.95,
        vec![obs1.id],
    )
    .unwrap();
    activity_repo.record_segment(&segment).unwrap();

    // 5c. Classifications
    let classification = Classification::new(
        segment.id,
        ClassificationLabel::Relevant,
        0.99,
        ClassificationBasis::DeterministicRule,
        "v1",
        now,
    )
    .unwrap();
    activity_repo
        .record_classification(&classification)
        .unwrap();

    // 5d. User Actions
    let correction = UserCorrection::new(
        s1.id,
        Some(segment.id),
        Some("nvim".into()),
        None,
        ClassificationLabel::Relevant,
        CorrectionScope::Session,
        Some("Note".into()),
        now,
    )
    .unwrap();
    activity_repo.record_correction(&correction).unwrap();

    // 5e. Reviews
    let review = SessionReview::from_session(
        &s1,
        SessionOutcome::CompletedSuccess,
        Some("Review note".into()),
        None,
        now + Duration::seconds(1500),
    )
    .unwrap();
    review_repo.save_review(&review).unwrap();

    // 5f. Event history (with ON DELETE SET NULL)
    pool.with_conn(|conn| {
        conn.execute(
            "INSERT INTO event_history (id, session_id, event_type, entity_type, entity_id, created_at)
             VALUES (?1, ?2, 'session_completed', 'focus_session', ?2, ?3)",
            rusqlite::params![Uuid::new_v4().to_string(), s1.id.to_string(), now.to_rfc3339()],
        )?;
        Ok(())
    }).unwrap();

    // Assert initial counts
    pool.with_conn(|conn| {
        let fk_violations: i64 = conn
            .query_row("PRAGMA foreign_key_check", [], |_r| Ok(1))
            .unwrap_or(0);
        assert_eq!(fk_violations, 0, "No FK violations initially");

        let ambitions_cnt: i64 =
            conn.query_row("SELECT count(*) FROM ambitions", [], |r| r.get(0))?;
        let goals_cnt: i64 = conn.query_row("SELECT count(*) FROM goals", [], |r| r.get(0))?;
        let commitments_cnt: i64 =
            conn.query_row("SELECT count(*) FROM commitments", [], |r| r.get(0))?;
        let sessions_cnt: i64 =
            conn.query_row("SELECT count(*) FROM focus_sessions", [], |r| r.get(0))?;
        let obs_cnt: i64 =
            conn.query_row("SELECT count(*) FROM activity_observations", [], |r| {
                r.get(0)
            })?;
        let segments_cnt: i64 =
            conn.query_row("SELECT count(*) FROM activity_segments", [], |r| r.get(0))?;
        let classifications_cnt: i64 =
            conn.query_row("SELECT count(*) FROM classifications", [], |r| r.get(0))?;
        let actions_cnt: i64 =
            conn.query_row("SELECT count(*) FROM user_actions", [], |r| r.get(0))?;
        let reviews_cnt: i64 =
            conn.query_row("SELECT count(*) FROM session_reviews", [], |r| r.get(0))?;
        let rules_cnt: i64 = conn.query_row("SELECT count(*) FROM user_rules", [], |r| r.get(0))?;

        assert_eq!(ambitions_cnt, 1);
        assert_eq!(goals_cnt, 2);
        assert_eq!(commitments_cnt, 2);
        assert_eq!(sessions_cnt, 2);
        assert_eq!(obs_cnt, 2);
        assert_eq!(segments_cnt, 1);
        assert_eq!(classifications_cnt, 1);
        assert_eq!(actions_cnt, 1);
        assert_eq!(reviews_cnt, 1);
        assert_eq!(rules_cnt, 1);
        Ok(())
    })
    .unwrap();

    // --- Sub-test 2.1: Delete session 1 directly via SQL ---
    pool.with_conn(|conn| {
        let rows = conn.execute(
            "DELETE FROM focus_sessions WHERE id = ?1",
            rusqlite::params![s1.id.to_string()],
        )?;
        assert_eq!(rows, 1);

        // Verify child tables of session 1 are cascaded to 0
        let obs_cnt: i64 = conn.query_row(
            "SELECT count(*) FROM activity_observations WHERE session_id = ?1",
            rusqlite::params![s1.id.to_string()],
            |r| r.get(0),
        )?;
        let seg_cnt: i64 = conn.query_row(
            "SELECT count(*) FROM activity_segments WHERE session_id = ?1",
            rusqlite::params![s1.id.to_string()],
            |r| r.get(0),
        )?;
        let cls_cnt: i64 =
            conn.query_row("SELECT count(*) FROM classifications", [], |r| r.get(0))?;
        let act_cnt: i64 = conn.query_row(
            "SELECT count(*) FROM user_actions WHERE session_id = ?1",
            rusqlite::params![s1.id.to_string()],
            |r| r.get(0),
        )?;
        let rev_cnt: i64 = conn.query_row(
            "SELECT count(*) FROM session_reviews WHERE session_id = ?1",
            rusqlite::params![s1.id.to_string()],
            |r| r.get(0),
        )?;

        assert_eq!(obs_cnt, 0, "All observations for s1 must be deleted");
        assert_eq!(seg_cnt, 0, "All segments for s1 must be deleted");
        assert_eq!(
            cls_cnt, 0,
            "All classifications for s1 segments must be deleted"
        );
        assert_eq!(act_cnt, 0, "All user_actions for s1 must be deleted");
        assert_eq!(rev_cnt, 0, "Session review for s1 must be deleted");

        // Verify event_history session_id was set to NULL
        let event_session: Option<String> =
            conn.query_row("SELECT session_id FROM event_history LIMIT 1", [], |r| {
                r.get(0)
            })?;
        assert!(
            event_session.is_none(),
            "event_history.session_id must be SET NULL"
        );

        // Verify foreign key integrity holds
        let fk_check: Vec<String> = conn
            .prepare("PRAGMA foreign_key_check")?
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        assert!(
            fk_check.is_empty(),
            "Zero FK violations after session delete: {:?}",
            fk_check
        );

        Ok(())
    })
    .unwrap();

    // --- Sub-test 2.2: Delete Goal 1 via repo ---
    goal_repo.delete_goal(goal1.id).unwrap();

    pool.with_conn(|conn| {
        let g1_commitments: i64 = conn.query_row(
            "SELECT count(*) FROM commitments WHERE goal_id = ?1",
            rusqlite::params![goal1.id.to_string()],
            |r| r.get(0),
        )?;
        let g1_rules: i64 = conn.query_row(
            "SELECT count(*) FROM user_rules WHERE goal_id = ?1",
            rusqlite::params![goal1.id.to_string()],
            |r| r.get(0),
        )?;
        assert_eq!(g1_commitments, 0, "Commitments under goal1 must be deleted");
        assert_eq!(g1_rules, 0, "User rules under goal1 must be deleted");

        // Sibling Goal 2 and its commitment/session must still exist
        let g2_cnt: i64 = conn.query_row(
            "SELECT count(*) FROM goals WHERE id = ?1",
            rusqlite::params![goal2.id.to_string()],
            |r| r.get(0),
        )?;
        let c2_cnt: i64 = conn.query_row(
            "SELECT count(*) FROM commitments WHERE id = ?1",
            rusqlite::params![c2.id.to_string()],
            |r| r.get(0),
        )?;
        let s2_cnt: i64 = conn.query_row(
            "SELECT count(*) FROM focus_sessions WHERE id = ?1",
            rusqlite::params![s2.id.to_string()],
            |r| r.get(0),
        )?;
        assert_eq!(g2_cnt, 1);
        assert_eq!(c2_cnt, 1);
        assert_eq!(s2_cnt, 1);
        Ok(())
    })
    .unwrap();

    // --- Sub-test 2.3: Delete Ambition (Root Cascade) ---
    goal_repo.delete_ambition(ambition.id).unwrap();

    pool.with_conn(|conn| {
        let tables = [
            "ambitions",
            "goals",
            "commitments",
            "focus_sessions",
            "activity_observations",
            "activity_segments",
            "classifications",
            "user_actions",
            "session_reviews",
            "user_rules",
        ];

        for table in tables {
            let count: i64 =
                conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))?;
            assert_eq!(
                count, 0,
                "Table {table} must have 0 rows after deleting root ambition"
            );
        }

        // Foreign keys must be 100% clean
        let fk_check: Vec<String> = conn
            .prepare("PRAGMA foreign_key_check")?
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        assert!(
            fk_check.is_empty(),
            "Zero FK violations after ambition cascade delete: {:?}",
            fk_check
        );

        Ok(())
    })
    .unwrap();
}

#[test]
fn challenge_2_cascade_deletes_segment_and_commitment_levels() {
    let (pool, goal_repo, session_repo, activity_repo, _r) = create_in_memory_suite();
    let now = Utc::now();

    let ambition = Ambition::new("Levels Ambition", None, now).unwrap();
    goal_repo.create_ambition(&ambition).unwrap();
    let goal = Goal::new(ambition.id, "Levels Goal", None, None, now).unwrap();
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

    // Create Segment
    let segment = ActivitySegment::new(
        session.id,
        now,
        now + Duration::seconds(30),
        Some("terminal".into()),
        None,
        SegmentKind::ActiveFocus,
        0.9,
        vec![],
    )
    .unwrap();
    activity_repo.record_segment(&segment).unwrap();

    // Create 2 Classifications for this segment
    let c1 = Classification::new(
        segment.id,
        ClassificationLabel::Relevant,
        0.9,
        ClassificationBasis::DeterministicRule,
        "v1",
        now,
    )
    .unwrap();
    let c2 = Classification::new(
        segment.id,
        ClassificationLabel::PossiblyUnrelated,
        0.7,
        ClassificationBasis::DeterministicRule,
        "v1",
        now,
    )
    .unwrap();
    activity_repo.record_classification(&c1).unwrap();
    activity_repo.record_classification(&c2).unwrap();

    // Create User Action targeting this segment
    let action = UserCorrection::new(
        session.id,
        Some(segment.id),
        Some("terminal".into()),
        None,
        ClassificationLabel::Relevant,
        CorrectionScope::Session,
        None,
        now,
    )
    .unwrap();
    activity_repo.record_correction(&action).unwrap();

    // Assert all exist
    assert_eq!(
        activity_repo
            .get_segment_classifications(segment.id)
            .unwrap()
            .len(),
        2
    );
    assert!(activity_repo.get_correction(action.id).unwrap().is_some());

    // 1. Delete Activity Segment directly via SQL
    pool.with_conn(|conn| {
        conn.execute(
            "DELETE FROM activity_segments WHERE id = ?1",
            rusqlite::params![segment.id.to_string()],
        )?;
        Ok(())
    })
    .unwrap();

    // Assert Classifications were CASCADE deleted
    assert_eq!(
        activity_repo
            .get_segment_classifications(segment.id)
            .unwrap()
            .len(),
        0
    );

    // Assert User Action row is NOT deleted, but its target_segment_id is SET NULL
    let fetched_action = activity_repo.get_correction(action.id).unwrap().unwrap();
    assert!(
        fetched_action.target_segment_id.is_none(),
        "User correction target_segment_id must be SET NULL when segment is deleted"
    );

    // 2. Delete Commitment directly via SQL
    pool.with_conn(|conn| {
        conn.execute(
            "DELETE FROM commitments WHERE id = ?1",
            rusqlite::params![commitment.id.to_string()],
        )?;
        Ok(())
    })
    .unwrap();

    // Assert Focus Session and all child records are CASCADE deleted
    assert!(session_repo.get_session(session.id).unwrap().is_none());
    assert!(activity_repo.get_correction(action.id).unwrap().is_none());

    // Verify foreign key integrity holds
    pool.with_conn(|conn| {
        let fk_check: Vec<String> = conn
            .prepare("PRAGMA foreign_key_check")?
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        assert!(fk_check.is_empty(), "Zero FK violations: {:?}", fk_check);
        Ok(())
    })
    .unwrap();
}

#[test]
fn challenge_2_cascade_deletes_on_disk_pool() {
    let guard = TempDbGuard::new("cascade_disk");
    let pool = DbPool::open_file(guard.path()).expect("Failed to open on-disk DB");
    let goal_repo = SqliteGoalRepository::new(pool.clone());
    let session_repo = SqliteSessionRepository::new(pool.clone());
    let now = Utc::now();

    let ambition = Ambition::new("Disk Ambition", None, now).unwrap();
    goal_repo.create_ambition(&ambition).unwrap();
    let goal = Goal::new(ambition.id, "Disk Goal", None, None, now).unwrap();
    goal_repo.create_goal(&goal).unwrap();
    let commitment = Commitment::new(goal.id, now, 1500, 300, None, now).unwrap();
    session_repo.create_commitment(&commitment).unwrap();

    // Verify foreign keys pragma is active on on-disk pool
    pool.with_conn(|conn| {
        let fk: i64 = conn.query_row("PRAGMA foreign_keys", [], |r| r.get(0))?;
        assert_eq!(fk, 1, "Foreign keys PRAGMA must be 1 on disk connection");
        Ok(())
    })
    .unwrap();

    // Delete ambition -> verify goal and commitment are deleted
    goal_repo.delete_ambition(ambition.id).unwrap();
    assert!(goal_repo.get_ambition(ambition.id).unwrap().is_none());
    assert!(goal_repo.get_goal(goal.id).unwrap().is_none());
    assert!(session_repo
        .get_commitment(commitment.id)
        .unwrap()
        .is_none());

    // Open a second connection to the same file and verify clean state
    let pool2 = DbPool::open_file(guard.path()).expect("Failed to open second connection");
    pool2
        .with_conn(|conn| {
            let c_count: i64 =
                conn.query_row("SELECT count(*) FROM commitments", [], |r| r.get(0))?;
            assert_eq!(c_count, 0);
            let fk_check: Vec<String> = conn
                .prepare("PRAGMA foreign_key_check")?
                .query_map([], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            assert!(fk_check.is_empty());
            Ok(())
        })
        .unwrap();
}

// =========================================================================
// CHALLENGE 3: CHECKSUM TAMPERING DETECTION IN MIGRATOR
// =========================================================================

#[test]
fn challenge_3_checksum_tampering_variations_and_abort() {
    let guard = TempDbGuard::new("tamper_test");

    // Step 1: Initialize cleanly on disk
    {
        let pool = DbPool::open_file(guard.path()).unwrap();
        pool.with_conn(|conn| {
            let count: i64 =
                conn.query_row("SELECT count(*) FROM schema_migrations", [], |r| r.get(0))?;
            assert_eq!(count, 1);
            Ok(())
        })
        .unwrap();
    }

    // Step 2: Corrupt the recorded checksum directly in the database file
    {
        let raw_conn = rusqlite::Connection::open(guard.path()).unwrap();
        raw_conn.execute(
            "UPDATE schema_migrations SET checksum = 'corrupted_sha256_hash_value' WHERE version = 1",
            [],
        ).unwrap();
    }

    // Step 3: Attempt to open pool or run migrator -> MUST be rejected!
    let open_res = DbPool::open_file(guard.path());
    assert!(
        open_res.is_err(),
        "DbPool::open_file must reject tampered checksum"
    );
    match open_res.unwrap_err() {
        StorageError::MigrationChecksumMismatch {
            version,
            expected,
            found,
        } => {
            assert_eq!(version, 1);
            assert_eq!(found, "corrupted_sha256_hash_value");
            assert_eq!(
                expected,
                SqliteMigrator::compute_checksum(crate::migrator::V1_SCHEMA_SQL)
            );
        }
        other => panic!("Expected MigrationChecksumMismatch, got: {:?}", other),
    }

    // Step 4: Empty checksum tampering
    {
        let raw_conn = rusqlite::Connection::open(guard.path()).unwrap();
        raw_conn
            .execute(
                "UPDATE schema_migrations SET checksum = '' WHERE version = 1",
                [],
            )
            .unwrap();
    }

    let migrator = SqliteMigrator::new();
    let mut raw_conn = rusqlite::Connection::open(guard.path()).unwrap();
    let res = migrator.initialize_and_migrate(&mut raw_conn);
    assert!(matches!(
        res,
        Err(StorageError::MigrationChecksumMismatch { version: 1, .. })
    ));

    // Step 5: Restore correct checksum and verify migration succeeds idempotently
    let original_checksum = SqliteMigrator::compute_checksum(crate::migrator::V1_SCHEMA_SQL);
    raw_conn
        .execute(
            "UPDATE schema_migrations SET checksum = ?1 WHERE version = 1",
            rusqlite::params![original_checksum],
        )
        .unwrap();

    let applied = migrator.initialize_and_migrate(&mut raw_conn).unwrap();
    assert_eq!(
        applied, 0,
        "Idempotent re-run on valid checksum must apply 0 migrations"
    );
}

#[test]
fn challenge_3_tamper_detection_transaction_rollback() {
    let mut conn = rusqlite::Connection::open_in_memory().unwrap();
    let migrator = SqliteMigrator::new();
    migrator.initialize_and_migrate(&mut conn).unwrap();

    // Insert a canary setting before tampering
    conn.execute(
        "INSERT INTO settings (key, value, version, updated_at) VALUES ('canary', 'initial', 1, '2026-01-01T00:00:00Z')",
        [],
    ).unwrap();

    // Tamper with checksum
    conn.execute(
        "UPDATE schema_migrations SET checksum = 'tampered' WHERE version = 1",
        [],
    )
    .unwrap();

    // Next migration attempt fails
    let res = migrator.initialize_and_migrate(&mut conn);
    assert!(res.is_err());

    // Canary setting must remain intact and database must not be corrupted
    let val: String = conn
        .query_row("SELECT value FROM settings WHERE key = 'canary'", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(val, "initial");
}

// =========================================================================
// CHALLENGE 4: MULTI-THREADED POOL CONTENTION & STRESS HARNESS
// =========================================================================

#[test]
fn challenge_4_multithreaded_shared_pool_high_contention() {
    let (pool, goal_repo, session_repo, activity_repo, _r) = create_in_memory_suite();
    let now = Utc::now();

    let ambition = Ambition::new("High Contention Ambition", None, now).unwrap();
    goal_repo.create_ambition(&ambition).unwrap();

    let num_threads = 32;
    let iterations_per_thread = 50;
    let barrier = Arc::new(Barrier::new(num_threads));

    let pool_arc = Arc::new(pool);
    let goal_repo_arc = Arc::new(goal_repo);
    let session_repo_arc = Arc::new(session_repo);
    let activity_repo_arc = Arc::new(activity_repo);

    let error_counter = Arc::new(AtomicUsize::new(0));
    let mut handles = Vec::new();

    let start_time = Instant::now();

    for thread_idx in 0..num_threads {
        let b = Arc::clone(&barrier);
        let gr = Arc::clone(&goal_repo_arc);
        let sr = Arc::clone(&session_repo_arc);
        let ar = Arc::clone(&activity_repo_arc);
        let pr = Arc::clone(&pool_arc);
        let errs = Arc::clone(&error_counter);
        let amb_id = ambition.id;

        let handle = thread::spawn(move || {
            b.wait();

            for iter in 0..iterations_per_thread {
                let t_now = Utc::now();

                // 1. Write goal
                let g_title = format!("Thread {thread_idx} Goal {iter}");
                let Ok(goal) = Goal::new(amb_id, g_title, None, None, t_now) else {
                    errs.fetch_add(1, Ordering::SeqCst);
                    continue;
                };
                if gr.create_goal(&goal).is_err() {
                    errs.fetch_add(1, Ordering::SeqCst);
                }

                // 2. Read back goal
                match gr.get_goal(goal.id) {
                    Ok(Some(f)) if f.id == goal.id => {}
                    _ => {
                        errs.fetch_add(1, Ordering::SeqCst);
                    }
                }

                // 3. Write commitment & completed session
                let Ok(comm) = Commitment::new(goal.id, t_now, 1500, 300, None, t_now) else {
                    errs.fetch_add(1, Ordering::SeqCst);
                    continue;
                };
                if sr.create_commitment(&comm).is_err() {
                    errs.fetch_add(1, Ordering::SeqCst);
                }

                let consent = ConsentSnapshot {
                    monitoring_enabled: true,
                    window_title_redaction_enabled: true,
                    browser_origin_enabled: false,
                    granted_at: t_now,
                };
                let mut sess = FocusSession::new(comm.id, consent, t_now);
                let _ = sess.start(t_now);
                let _ = sess.complete(
                    SessionEndReason::GoalAchieved,
                    t_now + Duration::seconds(1500),
                );
                if sr.create_session(&sess).is_err() {
                    errs.fetch_add(1, Ordering::SeqCst);
                }

                // 4. Batch observation write
                let Ok(obs) = Observation::new(
                    sess.id,
                    t_now,
                    ObservationSource::PlatformApp,
                    Some("alacritty".into()),
                    None,
                    None,
                    IdleState::Active,
                    None,
                    None,
                ) else {
                    errs.fetch_add(1, Ordering::SeqCst);
                    continue;
                };
                if ar.record_observation(&obs).is_err() {
                    errs.fetch_add(1, Ordering::SeqCst);
                }

                // 5. Query active session (should be None since all are completed)
                match sr.get_active_session() {
                    Ok(opt) => {
                        if opt.is_some() {
                            errs.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                    Err(_) => {
                        errs.fetch_add(1, Ordering::SeqCst);
                    }
                }

                // 6. Direct SQL read via with_conn
                let count_res = pr.with_conn(|conn| {
                    let c: i64 = conn.query_row("SELECT count(*) FROM goals", [], |r| r.get(0))?;
                    Ok(c)
                });
                if count_res.is_err() {
                    errs.fetch_add(1, Ordering::SeqCst);
                }
            }
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().expect("Worker thread panicked");
    }

    let elapsed = start_time.elapsed();
    let total_errors = error_counter.load(Ordering::SeqCst);
    assert_eq!(
        total_errors, 0,
        "High-contention shared pool produced {total_errors} errors in {:?}",
        elapsed
    );

    // Verify row counts match exactly: num_threads * iterations_per_thread
    let expected_count = (num_threads * iterations_per_thread) as i64;
    pool_arc
        .with_conn(|conn| {
            let goals_count: i64 =
                conn.query_row("SELECT count(*) FROM goals", [], |r| r.get(0))?;
            assert_eq!(goals_count, expected_count);

            let sessions_count: i64 =
                conn.query_row("SELECT count(*) FROM focus_sessions", [], |r| r.get(0))?;
            assert_eq!(sessions_count, expected_count);

            let obs_count: i64 =
                conn.query_row("SELECT count(*) FROM activity_observations", [], |r| {
                    r.get(0)
                })?;
            assert_eq!(obs_count, expected_count);

            Ok(())
        })
        .unwrap();
}

#[test]
fn challenge_4_multi_connection_on_disk_wal_contention() {
    let guard = TempDbGuard::new("wal_contention");
    let db_path = guard.path().clone();

    // Initialize the file cleanly first
    {
        let _init_pool = DbPool::open_file(&db_path).expect("Failed to initialize DB file");
    }

    let num_writer_threads = 6;
    let num_reader_threads = 6;
    let total_threads = num_writer_threads + num_reader_threads;
    let iterations = 40;

    let barrier = Arc::new(Barrier::new(total_threads));
    let error_counter = Arc::new(AtomicUsize::new(0));
    let stop_signal = Arc::new(AtomicBool::new(false));

    let mut handles = Vec::new();

    // 1. Spawning Writer Threads - each opening its OWN independent DbPool
    for w_idx in 0..num_writer_threads {
        let b = Arc::clone(&barrier);
        let path = db_path.clone();
        let errs = Arc::clone(&error_counter);

        let handle = thread::spawn(move || {
            let pool = match DbPool::open_file(&path) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Writer {w_idx} failed to open DB: {e:?}");
                    errs.fetch_add(1, Ordering::SeqCst);
                    return;
                }
            };

            b.wait();

            for iter in 0..iterations {
                let now = Utc::now();
                let ambition_id = AmbitionId::new();
                let title = format!("Ambition W{w_idx} I{iter}");

                // Execute write via transaction
                let tx_res = pool.transaction(|tx| {
                    tx.execute(
                        "INSERT INTO ambitions (id, title, description, created_at, updated_at) VALUES (?1, ?2, 'desc', ?3, ?3)",
                        rusqlite::params![ambition_id.to_string(), title, now.to_rfc3339()],
                    )?;
                    let goal_id = GoalId::new();
                    tx.execute(
                        "INSERT INTO goals (id, ambition_id, title, status, created_at, updated_at) VALUES (?1, ?2, 'G', 'active', ?3, ?3)",
                        rusqlite::params![goal_id.to_string(), ambition_id.to_string(), now.to_rfc3339()],
                    )?;
                    Ok(())
                });

                if let Err(e) = tx_res {
                    eprintln!("Writer {w_idx} failed transaction at iter {iter}: {e:?}");
                    errs.fetch_add(1, Ordering::SeqCst);
                }
            }
        });
        handles.push(handle);
    }

    // 2. Spawning Reader Threads - each opening its OWN independent DbPool
    for r_idx in 0..num_reader_threads {
        let b = Arc::clone(&barrier);
        let path = db_path.clone();
        let errs = Arc::clone(&error_counter);
        let stop = Arc::clone(&stop_signal);

        let handle = thread::spawn(move || {
            let pool = match DbPool::open_file(&path) {
                Ok(p) => p,
                Err(e) => {
                    eprintln!("Reader {r_idx} failed to open DB: {e:?}");
                    errs.fetch_add(1, Ordering::SeqCst);
                    return;
                }
            };

            b.wait();

            while !stop.load(Ordering::SeqCst) {
                let read_res = pool.with_conn(|conn| {
                    let _count: i64 =
                        conn.query_row("SELECT count(*) FROM goals", [], |r| r.get(0))?;
                    let _active: Option<String> = conn.query_row(
                        "SELECT id FROM focus_sessions WHERE state IN ('Active', 'active') LIMIT 1",
                        [],
                        |r| r.get(0),
                    ).optional()?;
                    Ok(())
                });

                if let Err(e) = read_res {
                    eprintln!("Reader {r_idx} read error: {e:?}");
                    errs.fetch_add(1, Ordering::SeqCst);
                }

                // Yield to allow writers to interleave
                thread::yield_now();
            }
        });
        handles.push(handle);
    }

    // Wait for writers to complete
    let mut writer_handles = handles;
    let reader_handles = writer_handles.split_off(num_writer_threads);

    for wh in writer_handles {
        wh.join().expect("Writer thread panicked");
    }

    // Stop readers
    stop_signal.store(true, Ordering::SeqCst);
    for rh in reader_handles {
        rh.join().expect("Reader thread panicked");
    }

    let total_errs = error_counter.load(Ordering::SeqCst);
    assert_eq!(
        total_errs, 0,
        "WAL multi-connection contention produced {total_errs} errors"
    );

    // Verify all rows written
    let final_pool = DbPool::open_file(&db_path).unwrap();
    final_pool
        .with_conn(|conn| {
            let expected = (num_writer_threads * iterations) as i64;
            let count: i64 = conn.query_row("SELECT count(*) FROM ambitions", [], |r| r.get(0))?;
            assert_eq!(count, expected);
            let g_count: i64 = conn.query_row("SELECT count(*) FROM goals", [], |r| r.get(0))?;
            assert_eq!(g_count, expected);

            let fk_check: Vec<String> = conn
                .prepare("PRAGMA foreign_key_check")?
                .query_map([], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            assert!(
                fk_check.is_empty(),
                "Foreign key violations after concurrent WAL writes"
            );
            Ok(())
        })
        .unwrap();
}

#[test]
fn challenge_4_busy_timeout_lock_wait_success() {
    let guard = TempDbGuard::new("busy_wait");
    let path = guard.path().clone();

    let pool1 = DbPool::open_file(&path).unwrap();
    let pool2 = DbPool::open_file(&path).unwrap();

    let now = Utc::now().to_rfc3339();
    let now_clone = now.clone();
    let ambition_id = AmbitionId::new().to_string();

    let lock_acquired = Arc::new(AtomicBool::new(false));
    let lock_acquired_clone = Arc::clone(&lock_acquired);

    // Thread 1 holds an exclusive transaction lock for 150ms
    let h1 = thread::spawn(move || {
        pool1.transaction(|tx| {
            tx.execute(
                "INSERT INTO ambitions (id, title, description, created_at, updated_at) VALUES (?1, 'T1', 'desc', ?2, ?2)",
                rusqlite::params![ambition_id, now_clone],
            )?;
            lock_acquired_clone.store(true, Ordering::SeqCst);
            thread::sleep(std::time::Duration::from_millis(150));
            Ok(())
        }).unwrap();
    });

    // Wait until Thread 1 has acquired the lock
    while !lock_acquired.load(Ordering::SeqCst) {
        thread::yield_now();
    }

    // Thread 2 attempts to write while Thread 1 holds the lock.
    // Thanks to PRAGMA busy_timeout = 5000ms, Thread 2 must NOT fail with SQLITE_BUSY,
    // but rather wait for Thread 1 to commit and then succeed.
    let ambition_id_2 = AmbitionId::new().to_string();
    let res = pool2.transaction(|tx| {
        tx.execute(
            "INSERT INTO ambitions (id, title, description, created_at, updated_at) VALUES (?1, 'T2', 'desc', ?2, ?2)",
            rusqlite::params![ambition_id_2, now],
        )?;
        Ok(())
    });

    assert!(
        res.is_ok(),
        "Thread 2 must succeed via busy_timeout without SQLITE_BUSY"
    );
    h1.join().unwrap();

    // Verify both rows exist
    pool2
        .with_conn(|conn| {
            let count: i64 = conn.query_row("SELECT count(*) FROM ambitions", [], |r| r.get(0))?;
            assert_eq!(count, 2);
            Ok(())
        })
        .unwrap();
}

#[test]
fn challenge_4_concurrent_transaction_rollback_and_pool_health() {
    let pool = DbPool::open_in_memory().unwrap();
    let num_threads = 16;
    let barrier = Arc::new(Barrier::new(num_threads));
    let pool_arc = Arc::new(pool);

    let mut handles = Vec::new();
    for thread_idx in 0..num_threads {
        let p = Arc::clone(&pool_arc);
        let b = Arc::clone(&barrier);

        let handle = thread::spawn(move || {
            b.wait();

            for iter in 0..20 {
                let key = format!("t{thread_idx}_i{iter}");
                if iter % 2 == 0 {
                    // Intentionally failing transaction
                    let _: Result<(), StorageError> = p.transaction(|tx| {
                        tx.execute(
                            "INSERT INTO settings (key, value, version, updated_at) VALUES (?1, 'fail', 1, '2026-01-01T00:00:00Z')",
                            rusqlite::params![key],
                        )?;
                        Err(StorageError::DataIntegrity("Intended abort".into()))
                    });
                } else {
                    // Succeeding transaction
                    let res = p.transaction(|tx| {
                        tx.execute(
                            "INSERT INTO settings (key, value, version, updated_at) VALUES (?1, 'success', 1, '2026-01-01T00:00:00Z')",
                            rusqlite::params![key],
                        )?;
                        Ok(())
                    });
                    assert!(
                        res.is_ok(),
                        "Succeeding transaction must not be affected by failing peers"
                    );
                }
            }
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().unwrap();
    }

    // Verify exactly (num_threads * 10) settings exist, all with value 'success'
    pool_arc
        .with_conn(|conn| {
            let count: i64 = conn.query_row("SELECT count(*) FROM settings", [], |r| r.get(0))?;
            assert_eq!(count, (num_threads * 10) as i64);

            let bad_count: i64 = conn.query_row(
                "SELECT count(*) FROM settings WHERE value != 'success'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(bad_count, 0, "No failed transaction rows must exist");
            Ok(())
        })
        .unwrap();
}

#[test]
fn challenge_4_high_churn_connection_opening() {
    let guard = TempDbGuard::new("churn");
    let path = guard.path().clone();

    // Initialize cleanly first
    {
        let _init = DbPool::open_file(&path).unwrap();
    }

    let num_threads = 16;
    let barrier = Arc::new(Barrier::new(num_threads));
    let mut handles = Vec::new();

    for t_idx in 0..num_threads {
        let b = Arc::clone(&barrier);
        let p = path.clone();

        let handle = thread::spawn(move || {
            b.wait();
            for i in 0..15 {
                let pool = DbPool::open_file(&p).unwrap();
                let now = Utc::now().to_rfc3339();
                let id = Uuid::new_v4().to_string();
                pool.with_conn(|conn| {
                    conn.execute(
                        "INSERT INTO ambitions (id, title, description, created_at, updated_at) VALUES (?1, ?2, 'churn', ?3, ?3)",
                        rusqlite::params![id, format!("Churn {t_idx} {i}"), now],
                    )?;
                    Ok(())
                }).unwrap();
            }
        });
        handles.push(handle);
    }

    for h in handles {
        h.join().unwrap();
    }

    let final_pool = DbPool::open_file(&path).unwrap();
    final_pool
        .with_conn(|conn| {
            let count: i64 = conn.query_row("SELECT count(*) FROM ambitions", [], |r| r.get(0))?;
            assert_eq!(count, (num_threads * 15) as i64);
            Ok(())
        })
        .unwrap();
}
