//! Empirical Adversarial Challenge Test Suite for Milestone 5: Unified CoachEngine Facade
//!
//! Author: challenger_m5_1
//! Objectives:
//! 1. Multi-threaded massive concurrency and race condition stress testing across shared clones.
//! 2. Zero deadlocks under heavy interleaved reads/writes (evaluate, record_snooze, record_targeted_correction, reset_session).
//! 3. Zero lock poisoning crashes: resilience and automatic guard recovery when threads panic while holding inner locks.
//! 4. Clock jumps: massive forward leaps (+100 years), backward leaps (regressions), and non-monotonic microsecond jitter.
//! 5. Rapid burst throttling: 10,000 rapid requests at identical timestamps ensuring exactly 1 provider call and zero alert storm.
//! 6. Async task cancellation during provider await ensuring zero leaked lock guards.
//! 7. Fallback infallibility under chaotic provider failures.
//! 8. Strict boundary condition enforcement across snooze and target suppression caps.

use chrono::{Duration, TimeZone, Utc};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration as StdDuration;
use uuid::Uuid;
use wowie_coach::engine::{CoachEngine, CoachEngineBuilder, CoachEngineConfig};
use wowie_coach::error::CoachError;
use wowie_coach::models::action::ActionCandidate;
use wowie_coach::models::context::{CoachContext, UntrustedObservation};
use wowie_coach::models::response::{CoachRawResponse, RelevanceClassification};
use wowie_coach::models::tone::TonePreferences;
use wowie_coach::policy::cooldown::{
    CooldownConfig, CooldownStatus, ThrottlingReason, MAX_TRACKED_SUPPRESSIONS,
};
use wowie_coach::provider::{CoachProvider, MockCoachProvider};
use wowie_core::id::SessionId;
use wowie_core::models::{CorrectionScope, IdleState};

// =========================================================================
// TEST HELPERS & HARNESS
// =========================================================================

fn make_test_context(
    app: Option<&str>,
    origin: Option<&str>,
    title: Option<&str>,
    is_break: bool,
    idle: Option<IdleState>,
) -> CoachContext {
    let obs = UntrustedObservation::new(
        app.map(String::from),
        title.map(String::from),
        origin.map(String::from),
        idle.or(Some(IdleState::Active)),
    )
    .expect("Valid observation");

    CoachContext::new(
        "Adversarial Stress Goal",
        Some("Audit concurrent lock safety".to_string()),
        SessionId::from_uuid(Uuid::nil()),
        600,
        is_break,
        obs,
        vec![],
        TonePreferences::default(),
    )
    .expect("Valid context")
}

fn make_engine(provider: MockCoachProvider) -> (CoachEngine, Arc<MockCoachProvider>) {
    let mock_arc = Arc::new(provider);
    let engine = CoachEngine::with_default_config(mock_arc.clone());
    (engine, mock_arc)
}

// =========================================================================
// CHALLENGE 1: MASSIVE CONCURRENCY, INTERLEAVED METHODS & ZERO DEADLOCKS
// =========================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_adversarial_concurrency_massive_interleaved_all_methods() {
    let base_time = Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap();
    let (engine, mock) = make_engine(MockCoachProvider::canned_relevant());
    let engine = Arc::new(engine);

    // Timeout safety net: if any deadlock occurs, test fails with timeout
    let test_future = async {
        let mut handles = Vec::new();
        let num_tasks = 40;
        let iters_per_task = 25;

        // Group 1: Concurrent evaluation tasks
        for t in 0..15 {
            let eng = Arc::clone(&engine);
            handles.push(tokio::spawn(async move {
                for i in 0..iters_per_task {
                    let now = base_time + Duration::seconds(i64::from(t * 100 + i * 10));
                    let ctx = make_test_context(
                        Some(&format!("app_{t}_{i}")),
                        Some("example.org"),
                        Some("Evaluating"),
                        false,
                        Some(IdleState::Active),
                    );
                    let _ = eng.evaluate(&ctx, now).await;
                }
            }));
        }

        // Group 2: Concurrent strict evaluation tasks
        for t in 15..25 {
            let eng = Arc::clone(&engine);
            handles.push(tokio::spawn(async move {
                for i in 0..iters_per_task {
                    let now = base_time + Duration::seconds(i64::from(t * 100 + i * 10));
                    let ctx = make_test_context(
                        Some("browser"),
                        Some(&format!("origin_{t}_{i}.com")),
                        Some("Strict Eval"),
                        false,
                        Some(IdleState::Active),
                    );
                    let _ = eng.evaluate_strict(&ctx, now).await;
                }
            }));
        }

        // Group 3: Concurrent targeted corrections & batch syncs
        for t in 25..30 {
            let eng = Arc::clone(&engine);
            handles.push(tokio::spawn(async move {
                for i in 0..iters_per_task {
                    let now = base_time + Duration::seconds(i64::from(t * 100 + i * 10));
                    eng.record_targeted_correction(
                        now,
                        CorrectionScope::Session,
                        Some(format!("app_{i}")),
                        Some(format!("origin_{i}.org")),
                    );
                }
            }));
        }

        // Group 4: Concurrent snooze and snooze cancellation
        for t in 30..35 {
            let eng = Arc::clone(&engine);
            handles.push(tokio::spawn(async move {
                for i in 0..iters_per_task {
                    let now = base_time + Duration::seconds(i64::from(t * 100 + i * 10));
                    if i % 2 == 0 {
                        let _ = eng.record_snooze(now, 300);
                    } else {
                        eng.cancel_snooze();
                    }
                }
            }));
        }

        // Group 5: Concurrent state inspections & session resets
        for t in 35..num_tasks {
            let eng = Arc::clone(&engine);
            handles.push(tokio::spawn(async move {
                for i in 0..iters_per_task {
                    let now = base_time + Duration::seconds(i64::from(t * 100 + i * 10));
                    let _ = eng.cooldown_status(now);
                    let _ = eng.is_snoozed(now);
                    let _ = eng.is_break_state();
                    let _ = eng.is_idle();
                    let _ = eng.total_interventions();
                    let _ = eng.cooldown_snapshot();

                    if i % 10 == 9 {
                        eng.reset_session();
                    }
                }
            }));
        }

        // Dedicated OS thread doing aggressive synchronous writes
        let eng_os = Arc::clone(&engine);
        let os_thread = std::thread::spawn(move || {
            for i in 0..50 {
                let now = base_time + Duration::seconds(i64::from(i * 5));
                eng_os.set_break_state(i % 2 == 0, now);
                eng_os.set_idle_state(i % 3 == 0, now);
                let _ = eng_os.record_snooze(now, 60);
                std::thread::yield_now();
            }
        });

        for handle in handles {
            handle.await.expect("Task panicked or aborted");
        }
        os_thread.join().expect("OS thread panicked");
    };

    // Assert completion within 5 seconds (zero deadlock guarantee)
    tokio::time::timeout(StdDuration::from_secs(5), test_future)
        .await
        .expect("Deadlock detected during massive concurrent stress test!");

    // Verify engine is completely intact and operational afterwards
    engine.reset_session();
    let ctx = make_test_context(Some("editor"), None, Some("Done"), false, None);
    let final_resp = engine.evaluate(&ctx, base_time).await;
    assert!(final_resp.is_relevant());
    assert!(mock.call_count() > 0);
}

// =========================================================================
// CHALLENGE 2: ZERO LOCK POISONING & RECOVERY RESILIENCE
// =========================================================================

#[tokio::test]
async fn test_adversarial_lock_poisoning_recovery_write_guard() {
    let now = Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap();
    let (engine, _) = make_engine(MockCoachProvider::canned_relevant());

    // Step 1: Intentionally poison the inner RwLock by panicking inside a write lock
    let tracker_arc = engine.cooldown_tracker();
    let poison_result = std::panic::catch_unwind(|| {
        let _guard = tracker_arc.write().unwrap();
        panic!("Deliberate adversarial poison write lock panic");
    });
    assert!(poison_result.is_err(), "Thread must panic");

    // Verify the inner lock is indeed poisoned
    assert!(
        engine.cooldown_tracker().is_poisoned(),
        "Underlying RwLock must be poisoned"
    );

    // Step 2: Now invoke CoachEngine methods across the poisoned lock
    // All methods must recover gracefully without crashing or throwing PoisonError!
    assert!(!engine.is_snoozed(now));
    assert_eq!(engine.total_interventions(), 0);

    // Write operation on poisoned lock must succeed
    engine.record_targeted_correction(
        now,
        CorrectionScope::Session,
        Some("poisoned_app".to_string()),
        None,
    );

    // Read operation must reflect the write
    let ctx_suppressed = make_test_context(Some("poisoned_app"), None, None, false, None);
    let status = engine.can_intervene_context(now, &ctx_suppressed);
    assert!(matches!(
        status,
        CooldownStatus::Throttled {
            reason: ThrottlingReason::UserCorrectionSuppressed,
            ..
        }
    ));

    // Async evaluation on poisoned lock must succeed
    let ctx_ok = make_test_context(Some("other_app"), None, None, false, None);
    let resp = engine.evaluate(&ctx_ok, now).await;
    assert!(resp.is_relevant());

    // Strict evaluation on poisoned lock must also succeed
    let strict_resp = engine.evaluate_strict(&ctx_ok, now).await;
    assert!(strict_resp.is_ok());

    // Reset session on poisoned lock must succeed
    engine.reset_session();
    assert_eq!(engine.total_interventions(), 0);
}

#[tokio::test]
async fn test_adversarial_lock_poisoning_recovery_read_guard() {
    let now = Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap();
    let (engine, _) = make_engine(MockCoachProvider::canned_relevant());

    // 1. In Rust std::sync::RwLock, a panicked reader drops its read count cleanly.
    // Verify that panicking inside a read lock does NOT prevent subsequent writes or reads.
    let tracker_arc = engine.cooldown_tracker();
    let read_panic = std::panic::catch_unwind(|| {
        let _guard = tracker_arc.read().unwrap();
        panic!("Deliberate adversarial read panic");
    });
    assert!(read_panic.is_err());

    // Subsequent write must succeed without deadlock
    engine.record_targeted_correction(
        now,
        CorrectionScope::Session,
        Some("clean_app".to_string()),
        None,
    );

    // 2. Now intentionally poison the lock with a write panic
    let write_poison = std::panic::catch_unwind(|| {
        let _guard = tracker_arc.write().unwrap();
        panic!("Deliberate adversarial writer poison");
    });
    assert!(write_poison.is_err());
    assert!(engine.cooldown_tracker().is_poisoned());

    // 3. Verify tracker_read() recovers on the poisoned lock
    let _ = engine.cooldown_status(now);
    assert!(!engine.is_snoozed(now));
    let snapshot = engine.cooldown_snapshot();
    assert_eq!(snapshot.total_interventions(), 0);

    // Verify tracker_write() also recovers on the poisoned lock
    let _ = engine.record_snooze(now, 120);
    assert!(engine.is_snoozed(now));
    engine.cancel_snooze();
    assert!(!engine.is_snoozed(now));
}

#[test]
fn test_adversarial_lock_poisoning_repeated_torture_loop() {
    let now = Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap();
    let (engine, _) = make_engine(MockCoachProvider::canned_relevant());

    // Intentionally poison the lock 20 consecutive times from 20 threads
    for i in 0..20 {
        let tracker = engine.cooldown_tracker();
        let _ = std::panic::catch_unwind(move || {
            if i % 2 == 0 {
                let _guard = tracker.write().unwrap();
                panic!("Poison loop write panic {i}");
            } else {
                let _guard = tracker.read().unwrap();
                panic!("Poison loop read panic {i}");
            }
        });

        // After every single poison, engine must remain completely healthy
        engine.record_targeted_correction(
            now,
            CorrectionScope::Session,
            Some(format!("app_{i}")),
            None,
        );
        let _ = engine.is_snoozed(now);
        let _ = engine.total_interventions();
        engine.reset_session();
    }
}

// =========================================================================
// CHALLENGE 3: CLOCK JUMPS, MASSIVE LEAPS, AND REGRESSIONS
// =========================================================================

#[tokio::test]
async fn test_adversarial_clock_massive_forward_jump_100_years() {
    let base_time = Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap();
    let (engine, _) = make_engine(MockCoachProvider::canned_distracted(
        ActionCandidate::Remind,
    ));
    let ctx = make_test_context(Some("distraction"), None, None, false, None);

    // First intervention at base_time
    let r1 = engine.evaluate(&ctx, base_time).await;
    assert_eq!(r1.effective_action(), ActionCandidate::Remind);
    assert_eq!(engine.total_interventions(), 1);

    // Set a snooze for 1 hour at base_time
    engine.record_snooze(base_time, 3600).unwrap();
    assert!(engine.is_snoozed(base_time));

    // Clock jumps forward 100 years (+ 36,500 days)
    let future_100y = base_time + Duration::days(36500);

    // Snooze must have cleanly expired with no integer overflow
    assert!(!engine.is_snoozed(future_100y));
    assert_eq!(engine.remaining_snooze_seconds(future_100y), 0);

    // Next intervention must be permitted with no overflow in cooldown math
    let status = engine.can_intervene(future_100y);
    assert!(status.is_allowed());

    let r2 = engine.evaluate(&ctx, future_100y).await;
    assert_eq!(r2.effective_action(), ActionCandidate::Remind);
    assert_eq!(engine.total_interventions(), 2);
}

#[tokio::test]
async fn test_adversarial_clock_regression_detection_and_fallback() {
    let base_time = Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap();
    let (engine, mock) = make_engine(MockCoachProvider::canned_distracted(
        ActionCandidate::Remind,
    ));
    let ctx = make_test_context(Some("distraction"), None, None, false, None);

    // 1. Initial intervention at T0
    let r1 = engine.evaluate(&ctx, base_time).await;
    assert_eq!(r1.effective_action(), ActionCandidate::Remind);
    assert_eq!(mock.call_count(), 1);

    // 2. Timestamp regresses by 1 second (now < last_intervention)
    let regressed_1s = base_time - Duration::seconds(1);
    let r_reg1 = engine.evaluate(&ctx, regressed_1s).await;
    assert_eq!(r_reg1.effective_action(), ActionCandidate::None);
    assert!(r_reg1.rationale.contains("time_regression"));
    assert_eq!(mock.call_count(), 1, "LLM must NOT be called on regression");

    // 3. Timestamp regresses by 10 days
    let regressed_10d = base_time - Duration::days(10);
    let r_reg2 = engine.evaluate(&ctx, regressed_10d).await;
    assert_eq!(r_reg2.effective_action(), ActionCandidate::None);
    assert!(r_reg2.rationale.contains("time_regression"));
    assert_eq!(mock.call_count(), 1);

    // 4. Timestamp regresses before UNIX epoch (1969)
    let pre_epoch = Utc.with_ymd_and_hms(1969, 12, 31, 23, 59, 59).unwrap();
    let r_pre = engine.evaluate(&ctx, pre_epoch).await;
    assert_eq!(r_pre.effective_action(), ActionCandidate::None);
    assert_eq!(mock.call_count(), 1);

    // 5. Clock recovers: advance 10 minutes past T0
    let recovered = base_time + Duration::minutes(10);
    let r_ok = engine.evaluate(&ctx, recovered).await;
    assert_eq!(r_ok.effective_action(), ActionCandidate::Remind);
    assert_eq!(mock.call_count(), 2);
}

#[tokio::test]
async fn test_adversarial_clock_microsecond_jitter_alternation() {
    let base_time = Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap();
    let (engine, mock) = make_engine(MockCoachProvider::canned_distracted(
        ActionCandidate::Remind,
    ));
    let ctx = make_test_context(Some("distraction"), None, None, false, None);

    // Trigger initial intervention
    let _ = engine.evaluate(&ctx, base_time).await;
    assert_eq!(mock.call_count(), 1);

    // Rapid alternating non-monotonic timestamps (jitter)
    let jitter_sequence = [
        base_time + Duration::milliseconds(1),
        base_time - Duration::milliseconds(1), // regression
        base_time + Duration::milliseconds(2),
        base_time, // regression
        base_time + Duration::milliseconds(3),
        base_time - Duration::milliseconds(5), // regression
    ];

    for ts in jitter_sequence {
        let resp = engine.evaluate(&ctx, ts).await;
        // Every single jittered call must either be throttled by RecentIntervention or TimeRegression
        assert_eq!(resp.effective_action(), ActionCandidate::None);
    }

    // Zero additional provider calls
    assert_eq!(mock.call_count(), 1);
}

// =========================================================================
// CHALLENGE 4: RAPID BURST OF 10,000 REQUESTS AT IDENTICAL TIMESTAMP
// =========================================================================

#[tokio::test]
async fn test_adversarial_rapid_burst_10000_identical_timestamps() {
    let now = Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap();
    let (engine, mock) = make_engine(MockCoachProvider::canned_distracted(
        ActionCandidate::Remind,
    ));
    let ctx = make_test_context(Some("rapid_app"), None, None, false, None);

    // First call: succeeds
    let first = engine.evaluate(&ctx, now).await;
    assert_eq!(first.effective_action(), ActionCandidate::Remind);
    assert_eq!(mock.call_count(), 1);

    // Rapid flood of 9,999 calls at the EXACT same timestamp
    for _ in 0..9_999 {
        let throttled = engine.evaluate(&ctx, now).await;
        assert_eq!(throttled.effective_action(), ActionCandidate::None);
    }

    // Under 10,000 total calls, exactly 1 reached the provider
    assert_eq!(mock.call_count(), 1);
    assert_eq!(engine.total_interventions(), 1);
}

// =========================================================================
// CHALLENGE 5: ASYNC TASK CANCELLATION DURING PROVIDER AWAIT
// =========================================================================

#[tokio::test]
async fn test_adversarial_task_cancellation_during_provider_delay_no_lock_leak() {
    let now = Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap();
    // Provider with a noticeable delay
    let mock = MockCoachProvider::canned_relevant().with_delay(StdDuration::from_millis(100));
    let (engine, _) = make_engine(mock);
    let engine = Arc::new(engine);

    let mut handles = Vec::new();

    // Spawn 20 tasks that start evaluating
    for i in 0..20 {
        let eng = Arc::clone(&engine);
        let ctx = make_test_context(Some(&format!("cancel_app_{i}")), None, None, false, None);
        handles.push(tokio::spawn(async move { eng.evaluate(&ctx, now).await }));
    }

    // Give tasks 10ms to enter `provider.evaluate().await` (Phase 3)
    tokio::time::sleep(StdDuration::from_millis(10)).await;

    // Ruthlessly abort all 20 tasks while they are awaiting provider
    for handle in handles {
        handle.abort();
    }

    // Ensure the runtime yields and drops aborted tasks
    tokio::time::sleep(StdDuration::from_millis(50)).await;

    // Immediately launch new tasks to ensure locks are NOT poisoned, deadlocked, or held
    for i in 0..5 {
        let ctx = make_test_context(Some(&format!("fresh_app_{i}")), None, None, false, None);
        let resp = engine.evaluate(&ctx, now).await;
        assert!(resp.is_relevant());
    }

    // State mutations must work cleanly
    engine.record_targeted_correction(now, CorrectionScope::Session, Some("fresh".into()), None);
    assert_eq!(engine.total_interventions(), 0); // canned_relevant has None action
}

// =========================================================================
// CHALLENGE 6: CHAOTIC PROVIDER FAILURE MATRIX & INFALLIBILITY
// =========================================================================

#[derive(Debug)]
struct ChaosProvider {
    counter: AtomicUsize,
}

#[async_trait::async_trait]
impl CoachProvider for ChaosProvider {
    fn name(&self) -> &'static str {
        "chaos_provider"
    }

    fn model_name(&self) -> &'static str {
        "chaos_model_v1"
    }

    async fn evaluate(
        &self,
        _prompt: &wowie_coach::prompt::builder::CoachPrompt,
    ) -> Result<CoachRawResponse, CoachError> {
        let idx = self.counter.fetch_add(1, Ordering::SeqCst);
        match idx % 6 {
            0 => Err(CoachError::timeout(5000)),
            1 => Err(CoachError::network(
                "HTTP 502 Bad Gateway: Cloudflare edge error",
            )),
            2 => Err(CoachError::malformed_json(
                "Unexpected EOF at line 1 column 4",
                Some(1),
                Some(4),
            )),
            3 => Err(CoachError::disallowed_action("rm -rf / --no-preserve-root")),
            4 => Err(CoachError::schema_validation(
                "confidence",
                "Confidence 2.5 exceeds maximum bound 1.0",
            )),
            _ => CoachRawResponse::from_json(
                r#"{"relevance":"relevant","confidence":0.95,"uncertainty":0.05,"rationale":"Chaos passed","action_candidate":"none"}"#,
            ),
        }
    }
}

#[tokio::test]
async fn test_adversarial_evaluate_infallibility_under_chaos_provider() {
    let now = Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap();
    let chaos = Arc::new(ChaosProvider {
        counter: AtomicUsize::new(0),
    });
    let engine = CoachEngine::with_default_config(chaos);

    let ctx = make_test_context(Some("chaos_app"), None, None, false, None);

    // Call evaluate 30 times across all failure modes
    for i in 0..30 {
        let resp = engine.evaluate(&ctx, now).await;
        // Infallible: MUST NEVER PANIC and ALWAYS return valid CoachResponse
        assert!(resp.confidence >= 0.0 && resp.confidence <= 1.0);
        assert!(!resp.rationale.is_empty());

        if i % 6 == 5 {
            assert!(resp.is_relevant());
        } else {
            // Degraded to safe deterministic fallback preserving origin provider name
            assert_eq!(resp.relevance, RelevanceClassification::Unknown);
            assert_eq!(resp.effective_action(), ActionCandidate::None);
        }
        assert_eq!(resp.provenance.provider_name, "chaos_provider");
    }
}

#[tokio::test]
async fn test_adversarial_evaluate_strict_propagates_exact_chaos_errors() {
    let now = Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap();
    let chaos = Arc::new(ChaosProvider {
        counter: AtomicUsize::new(0),
    });
    let engine = CoachEngine::with_default_config(chaos);
    let ctx = make_test_context(Some("chaos_app"), None, None, false, None);

    // Strict pipeline must propagate exact errors without fallback absorption
    let e0 = engine.evaluate_strict(&ctx, now).await.unwrap_err();
    assert!(matches!(e0, CoachError::Timeout { .. }));

    let e1 = engine.evaluate_strict(&ctx, now).await.unwrap_err();
    assert!(matches!(e1, CoachError::NetworkError { .. }));

    let e2 = engine.evaluate_strict(&ctx, now).await.unwrap_err();
    assert!(matches!(e2, CoachError::MalformedJson { .. }));

    let e3 = engine.evaluate_strict(&ctx, now).await.unwrap_err();
    assert!(matches!(e3, CoachError::DisallowedAction { .. }));

    let e4 = engine.evaluate_strict(&ctx, now).await.unwrap_err();
    assert!(matches!(e4, CoachError::SchemaValidation { .. }));

    let ok5 = engine.evaluate_strict(&ctx, now).await;
    assert!(ok5.is_ok());
}

// =========================================================================
// CHALLENGE 7: SNOOZE AND TARGET SUPPRESSION RING BUFFER BOUNDARIES
// =========================================================================

#[test]
fn test_adversarial_snooze_extreme_bounds() {
    let now = Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap();
    let (engine, _) = make_engine(MockCoachProvider::canned_relevant());

    // 0 seconds snooze -> defaults to 600s
    engine.record_snooze(now, 0).unwrap();
    assert_eq!(engine.remaining_snooze_seconds(now), 600);

    // u32::MAX snooze -> clamped to max_snooze_seconds (7200)
    engine.record_snooze(now, u32::MAX).unwrap();
    assert_eq!(engine.remaining_snooze_seconds(now), 7200);

    // Overlapping snooze: earlier end does not shrink longer active snooze
    engine.record_snooze(now, 100).unwrap();
    assert_eq!(engine.remaining_snooze_seconds(now), 7200);

    // Cancel snooze
    engine.cancel_snooze();
    assert_eq!(engine.remaining_snooze_seconds(now), 0);
}

#[test]
fn test_adversarial_target_suppressions_bounded_ring_buffer() {
    let now = Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap();
    let (engine, _) = make_engine(MockCoachProvider::canned_relevant());

    // Insert 128 unique targeted suppressions (cap is MAX_TRACKED_SUPPRESSIONS = 64)
    for i in 0..128 {
        engine.record_targeted_correction(
            now,
            CorrectionScope::Session,
            Some(format!("app_{i}")),
            None,
        );
    }

    // Inspect snapshot: suppressions count must NOT exceed 64
    let snapshot = engine.cooldown_snapshot();
    let snapshot_json = serde_json::to_string(&snapshot).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&snapshot_json).unwrap();
    let active_suppressions = parsed["active_suppressions"].as_array().unwrap();
    assert_eq!(active_suppressions.len(), MAX_TRACKED_SUPPRESSIONS);

    // Oldest entries (app_0 .. app_63) must have been evicted in FIFO order
    let ctx_evicted = make_test_context(Some("app_0"), None, None, false, None);
    assert!(engine.can_intervene_context(now, &ctx_evicted).is_allowed());

    // Most recent entries (app_64 .. app_127) must be actively suppressed
    let ctx_retained = make_test_context(Some("app_127"), None, None, false, None);
    assert!(matches!(
        engine.can_intervene_context(now, &ctx_retained),
        CooldownStatus::Throttled {
            reason: ThrottlingReason::UserCorrectionSuppressed,
            ..
        }
    ));
}

// =========================================================================
// CHALLENGE 8: BUILDER & CONFIGURATION FUZZING
// =========================================================================

#[test]
fn test_adversarial_builder_missing_provider_rejected() {
    let result = CoachEngineBuilder::new().build();
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(matches!(err, CoachError::ProviderUnavailable { .. }));
}

#[test]
fn test_adversarial_builder_invalid_config_invariants() {
    let mock = Arc::new(MockCoachProvider::canned_relevant());

    // Invalid min_interval (< 60s)
    let bad_cfg_1 = CoachEngineConfig::default().with_cooldown(CooldownConfig {
        min_interval_seconds: 30,
        ..CooldownConfig::default()
    });
    assert!(CoachEngine::new(mock.clone(), bad_cfg_1).is_err());

    // Invalid rapid_window (< min_interval)
    let bad_cfg_2 = CoachEngineConfig::default().with_cooldown(CooldownConfig {
        min_interval_seconds: 500,
        rapid_window_seconds: 300,
        ..CooldownConfig::default()
    });
    assert!(CoachEngine::new(mock.clone(), bad_cfg_2).is_err());

    // Invalid max_interventions_per_window (0)
    let bad_cfg_3 = CoachEngineConfig::default().with_cooldown(CooldownConfig {
        max_interventions_per_window: 0,
        ..CooldownConfig::default()
    });
    assert!(CoachEngine::new(mock.clone(), bad_cfg_3).is_err());

    // Invalid max_snooze (> 7200)
    let bad_cfg_4 = CoachEngineConfig::default().with_cooldown(CooldownConfig {
        max_snooze_seconds: 10000,
        ..CooldownConfig::default()
    });
    assert!(CoachEngine::new(mock.clone(), bad_cfg_4).is_err());

    // Invalid session_cap (0)
    let bad_cfg_5 = CoachEngineConfig::default().with_cooldown(CooldownConfig {
        session_cap: Some(0),
        ..CooldownConfig::default()
    });
    assert!(CoachEngine::new(mock, bad_cfg_5).is_err());
}

// =========================================================================
// CHALLENGE 9: CLONE SYNCHRONIZATION & RAPID STATE TOGGLING
// =========================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_adversarial_clones_cross_thread_cooldown_synchronization() {
    let now = Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap();
    let (engine_orig, mock) = make_engine(MockCoachProvider::canned_distracted(
        ActionCandidate::Remind,
    ));

    // Clone across 4 threads
    let c1 = engine_orig.clone();
    let c2 = engine_orig.clone();
    let c3 = engine_orig.clone();
    let c4 = engine_orig.clone();

    // c1 records intervention
    let ctx = make_test_context(Some("distracted_app"), None, None, false, None);
    let r1 = c1.evaluate(&ctx, now).await;
    assert_eq!(r1.effective_action(), ActionCandidate::Remind);
    assert_eq!(mock.call_count(), 1);

    // c2, c3, c4 immediately observe throttled status and intervention count == 1
    assert_eq!(c2.total_interventions(), 1);
    assert_eq!(c3.total_interventions(), 1);
    assert_eq!(c4.total_interventions(), 1);

    let r2 = c2.evaluate(&ctx, now).await;
    assert_eq!(r2.effective_action(), ActionCandidate::None);
    assert_eq!(
        mock.call_count(),
        1,
        "Clones must share identical rate limits"
    );

    // c3 records snooze
    c3.record_snooze(now, 500).unwrap();
    assert!(c1.is_snoozed(now));
    assert!(c4.is_snoozed(now));

    // c4 cancels snooze
    c4.cancel_snooze();
    assert!(!c1.is_snoozed(now));
    assert!(!c2.is_snoozed(now));
}

#[tokio::test]
async fn test_adversarial_break_and_idle_state_toggling_under_evaluation() {
    let now = Utc.with_ymd_and_hms(2026, 10, 25, 12, 0, 0).unwrap();
    let (engine, mock) = make_engine(MockCoachProvider::canned_distracted(
        ActionCandidate::Remind,
    ));
    let ctx = make_test_context(Some("distracted_app"), None, None, false, None);

    // Toggle break state on
    engine.set_break_state(true, now);
    assert!(engine.is_break_state());

    // Evaluated: throttled by BreakStateActive (canned remind is not allowed in break state by default without allow_break_reminders)
    let resp_break = engine.evaluate(&ctx, now).await;
    assert_eq!(resp_break.effective_action(), ActionCandidate::None);
    assert_eq!(mock.call_count(), 0);

    // Toggle break state off
    engine.set_break_state(false, now);
    assert!(!engine.is_break_state());

    // Toggle idle state on
    engine.set_idle_state(true, now);
    assert!(engine.is_idle());

    // Evaluated: throttled by IdleActive
    let resp_idle = engine.evaluate(&ctx, now).await;
    assert_eq!(resp_idle.effective_action(), ActionCandidate::None);
    assert_eq!(mock.call_count(), 0);

    // Toggle idle off
    engine.set_idle_state(false, now);
    assert!(!engine.is_idle());

    // Evaluated: permitted
    let resp_ok = engine.evaluate(&ctx, now).await;
    assert_eq!(resp_ok.effective_action(), ActionCandidate::Remind);
    assert_eq!(mock.call_count(), 1);
}

#[test]
fn test_adversarial_tone_adapted_presets_invariants() {
    let tone_supportive = TonePreferences::supportive();
    let cfg_gentle = CoachEngineConfig::from_tone(&tone_supportive);
    assert!(cfg_gentle.validate().is_ok());
    assert!(cfg_gentle.cooldown.min_interval_seconds >= 600);

    let tone_pragmatic = TonePreferences::pragmatic();
    let cfg_strict = CoachEngineConfig::from_tone(&tone_pragmatic);
    assert!(cfg_strict.validate().is_ok());
    assert!(cfg_strict.cooldown.min_interval_seconds <= 180);
}
