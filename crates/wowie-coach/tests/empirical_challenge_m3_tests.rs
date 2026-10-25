//! Empirical Challenge Verification Suite for Milestone 3:
//! Tone Policy & Deterministic Cooldown Tracker in `crates/wowie-coach`.
//!
//! File: tests/empirical_challenge_m3_tests.rs
//!
//! This suite is an empirical stress-test harness developed by Empirical Challenger 1.
//! It rigorously verifies:
//! 1. High-frequency burst testing & exact rate limit semantics.
//! 2. Pacing boundary conditions ($t = \text{min\_interval} - 1$, $t = \text{min\_interval}$, $t = \text{min\_interval} + 1$).
//! 3. Cumulative session cap exhaustion permanence and reset semantics.
//! 4. Silence preservation: ActionCandidate::None budget neutrality.
//! 5. Multi-cycle rolling window sliding behavior over extended timelines.
//! 6. Target suppression origin normalization, subdomain matching, and anti-spoofing.
//! 7. Snooze clamping, extension, and boundary expiration.
//! 8. Break and idle state action filtering via context and direct API.
//! 9. Clock regression stress and NTP step-back immunity.
//! 10. Memory bounding under massive 50,000-event streams.
//! 11. Serde serialization/deserialization contracts and error conversion taxonomy.

use chrono::{DateTime, Duration, Utc};
use serde_json::json;
use uuid::Uuid;
use wowie_coach::error::CoachError;
use wowie_coach::models::action::ActionCandidate;
use wowie_coach::models::context::{CoachContext, UntrustedObservation, UserCorrectionSummary};
use wowie_coach::models::tone::{PersonalityMode, TonePreferences};
use wowie_coach::policy::cooldown::{
    normalize_origin, CooldownConfig, CooldownStatus, CooldownTracker, ThrottlingReason,
    DEFAULT_IDLE_COOLDOWN_SECONDS, DEFAULT_MIN_INTERVAL_SECONDS, MAX_TRACKED_INTERVENTIONS,
};
use wowie_core::id::SessionId;
use wowie_core::models::{ClassificationLabel, CorrectionScope, IdleState};

// =========================================================================
// MOCK TIME FIXTURE
// =========================================================================

#[derive(Debug, Clone)]
struct EmpiricalClock {
    current: DateTime<Utc>,
}

impl EmpiricalClock {
    fn new(rfc3339_time: &str) -> Self {
        let dt = DateTime::parse_from_rfc3339(rfc3339_time)
            .expect("Valid RFC 3339 string")
            .with_timezone(&Utc);
        Self { current: dt }
    }

    fn now(&self) -> DateTime<Utc> {
        self.current
    }

    fn advance_seconds(&mut self, s: u64) -> DateTime<Utc> {
        self.current += Duration::seconds(s as i64);
        self.current
    }

    fn rewind_seconds(&mut self, s: u64) -> DateTime<Utc> {
        self.current -= Duration::seconds(s as i64);
        self.current
    }
}

fn build_context_fixture(
    app_id: Option<&str>,
    origin: Option<&str>,
    is_break: bool,
    idle: IdleState,
) -> CoachContext {
    let obs = UntrustedObservation::new(
        app_id.map(String::from),
        Some("Test Window".to_string()),
        origin.map(String::from),
        Some(idle),
    )
    .expect("Valid observation");

    CoachContext::new(
        "Empirical Goal",
        Some("Next step".to_string()),
        SessionId::from_uuid(Uuid::nil()),
        1800,
        is_break,
        obs,
        vec![],
        TonePreferences::default(),
    )
    .expect("Valid context")
}

// =========================================================================
// 1. PACING BOUNDARY CONDITIONS
// =========================================================================

#[test]
fn test_empirical_pacing_boundary_minus_one_exact_and_plus_one() {
    let mut clock = EmpiricalClock::new("2026-10-24T12:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // Baseline: allowed
    assert_eq!(tracker.can_intervene(clock.now()), CooldownStatus::Allowed);
    tracker.record_intervention(clock.now(), ActionCandidate::Remind);

    // Exact boundary check: min_interval = 300s
    // Case A: t = min_interval - 1 (elapsed 299s) -> Throttled, remaining = 1s
    clock.advance_seconds(299);
    let status_minus_one = tracker.can_intervene(clock.now());
    assert_eq!(
        status_minus_one,
        CooldownStatus::Throttled {
            reason: ThrottlingReason::RecentIntervention,
            remaining_seconds: 1,
        }
    );

    // Case B: t = min_interval (elapsed 300s) -> Allowed!
    clock.advance_seconds(1);
    let status_exact = tracker.can_intervene(clock.now());
    assert_eq!(status_exact, CooldownStatus::Allowed);

    // Case C: t = min_interval + 1 (elapsed 301s) -> Allowed!
    clock.advance_seconds(1);
    let status_plus_one = tracker.can_intervene(clock.now());
    assert_eq!(status_plus_one, CooldownStatus::Allowed);
}

// =========================================================================
// 2. HIGH-FREQUENCY BURST TESTING & ROLLING WINDOW RATE LIMITS
// =========================================================================

#[test]
fn test_empirical_high_frequency_burst_simulation() {
    let mut clock = EmpiricalClock::new("2026-10-24T09:00:00Z");
    // Config: 60s min interval, 600s rolling window, max 3 interventions per window
    let config = CooldownConfig {
        min_interval_seconds: 60,
        rapid_window_seconds: 600,
        max_interventions_per_window: 3,
        idle_cooldown_seconds: 60,
        correction_suppression_seconds: 600,
        default_snooze_seconds: 300,
        max_snooze_seconds: 1800,
        allow_break_reminders: true,
        session_cap: Some(10),
    };
    let mut tracker = CooldownTracker::new(config).expect("Valid config");

    let mut allowed_count = 0;
    let mut throttled_count = 0;

    // Simulate high-frequency client querying every single second for 1500 seconds
    for t in 0..1500 {
        let now = clock.now();
        let status = tracker.can_intervene(now);

        if status.is_allowed() {
            allowed_count += 1;
            tracker.record_intervention(now, ActionCandidate::Remind);
        } else {
            throttled_count += 1;
            // Verify remaining seconds is strictly positive when throttled
            assert!(
                status.remaining_seconds() >= 1,
                "Remaining seconds must be >= 1 at t={t}"
            );
        }

        clock.advance_seconds(1);
    }

    // In 1500 seconds:
    // Window 1 [0..600]: Interventions at 0, 60, 120. (Quota 3 filled at 120).
    //   Throttled from 121..599 (RecentIntervention from 121..179, RapidSuccessionRateLimit from 180..599).
    // Window 2: At 600, oldest (at 0) expires. Intervention at 600 accepted! (Count in window: [60, 120, 600] = 3).
    //   At 660, oldest (at 60) expires. Intervention at 660 accepted! (Count in window: [120, 600, 660] = 3).
    //   At 720, oldest (at 120) expires. Intervention at 720 accepted! (Count in window: [600, 660, 720] = 3).
    // Window 3: At 1200, oldest (at 600) expires. Intervention at 1200 accepted!
    //   At 1260, oldest (at 660) expires. Intervention at 1260 accepted!
    //   At 1320, oldest (at 720) expires. Intervention at 1320 accepted!
    // Total allowed interventions over 1500 seconds = 3 + 3 + 3 = 9 interventions.
    assert_eq!(
        allowed_count, 9,
        "Exactly 9 interventions permitted under burst conditions"
    );
    assert_eq!(throttled_count, 1500 - 9);
    assert_eq!(tracker.total_interventions(), 9);
}

// =========================================================================
// 3. SESSION CAP EXHAUSTION PERMANENCE & RESET
// =========================================================================

#[test]
fn test_empirical_session_cap_permanence_and_reset() {
    let mut clock = EmpiricalClock::new("2026-10-24T10:00:00Z");
    let config = CooldownConfig {
        session_cap: Some(2), // Very strict cap
        min_interval_seconds: 60,
        rapid_window_seconds: 3600,
        max_interventions_per_window: 10,
        ..Default::default()
    };
    let mut tracker = CooldownTracker::new(config).expect("Valid config");

    // 1st intervention
    assert_eq!(tracker.can_intervene(clock.now()), CooldownStatus::Allowed);
    tracker.record_intervention(clock.now(), ActionCandidate::Remind);

    // 2nd intervention after pacing
    clock.advance_seconds(60);
    assert_eq!(tracker.can_intervene(clock.now()), CooldownStatus::Allowed);
    tracker.record_intervention(clock.now(), ActionCandidate::AskContext);

    assert_eq!(tracker.total_interventions(), 2);

    // Now session cap (2) is reached.
    // Query immediately
    let status_cap = tracker.can_intervene(clock.now());
    assert_eq!(
        status_cap,
        CooldownStatus::Throttled {
            reason: ThrottlingReason::SessionCapReached,
            remaining_seconds: u32::MAX,
        }
    );

    // Advance by 1 hour (3600s) -> still capped!
    clock.advance_seconds(3600);
    assert_eq!(
        tracker.can_intervene(clock.now()),
        CooldownStatus::Throttled {
            reason: ThrottlingReason::SessionCapReached,
            remaining_seconds: u32::MAX,
        }
    );

    // Advance by 24 hours (86400s) -> still capped!
    clock.advance_seconds(86400);
    assert_eq!(
        tracker.can_intervene(clock.now()),
        CooldownStatus::Throttled {
            reason: ThrottlingReason::SessionCapReached,
            remaining_seconds: u32::MAX,
        }
    );

    // Advance by 30 days (2,592,000s) -> still permanently locked out!
    clock.advance_seconds(2_592_000);
    assert_eq!(
        tracker.can_intervene(clock.now()),
        CooldownStatus::Throttled {
            reason: ThrottlingReason::SessionCapReached,
            remaining_seconds: u32::MAX,
        }
    );

    // Explicit reset_session clears the session cap lock
    tracker.reset_session();
    assert_eq!(tracker.total_interventions(), 0);
    assert_eq!(tracker.can_intervene(clock.now()), CooldownStatus::Allowed);
}

// =========================================================================
// 4. SILENCE PRESERVATION: ActionCandidate::None BUDGET NEUTRALITY
// =========================================================================

#[test]
fn test_empirical_silence_preservation_action_none_million_cycles() {
    let mut clock = EmpiricalClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // Repeatedly record ActionCandidate::None 5,000 times (simulating silent cycles)
    for _ in 0..5000 {
        clock.advance_seconds(1);
        tracker.record_intervention(clock.now(), ActionCandidate::None);
    }

    // Zero budget consumed
    assert_eq!(tracker.total_interventions(), 0);
    assert_eq!(tracker.last_intervention(), None);
    assert_eq!(tracker.recent_interventions_count(clock.now()), 0);
    assert_eq!(tracker.can_intervene(clock.now()), CooldownStatus::Allowed);

    // Record one real proactive action
    tracker.record_intervention(clock.now(), ActionCandidate::Remind);
    assert_eq!(tracker.total_interventions(), 1);
    assert!(tracker.last_intervention().is_some());

    // Next instant query is throttled
    assert!(tracker.can_intervene(clock.now()).is_throttled());

    // Record 5,000 more ActionCandidate::None while in cooldown
    for _ in 0..5000 {
        clock.advance_seconds(1);
        tracker.record_intervention(clock.now(), ActionCandidate::None);
    }

    // Total interventions still 1!
    assert_eq!(tracker.total_interventions(), 1);

    // Since 5,000s have passed, the real intervention is now well past 300s
    assert_eq!(tracker.can_intervene(clock.now()), CooldownStatus::Allowed);
}

// =========================================================================
// 5. MULTI-CYCLE SLIDING WINDOW BURSTS OVER 10-HOUR TIMELINE
// =========================================================================

#[test]
fn test_empirical_multi_cycle_sliding_window_bursts() {
    let mut clock = EmpiricalClock::new("2026-10-24T08:00:00Z");
    // Standard config with session_cap disabled (None) to purely test rolling window sliding over 10 hours
    let config = CooldownConfig {
        session_cap: None,
        ..Default::default()
    };
    let mut tracker = CooldownTracker::new(config).expect("Valid config");

    for hour in 0..10 {
        let hour_start = clock.now();

        // 4 interventions at the start of the hour (at 0, 300, 600, 900 relative to hour start)
        for i in 0..4 {
            let status = tracker.can_intervene(clock.now());
            assert_eq!(
                status,
                CooldownStatus::Allowed,
                "Hour {hour}, intervention {i} must be allowed"
            );
            tracker.record_intervention(clock.now(), ActionCandidate::Remind);

            if i < 3 {
                clock.advance_seconds(300);
            }
        }

        // At relative t = 1200: 300s elapsed since 4th intervention, but window quota exhausted
        clock.advance_seconds(300);
        let status_throttled = tracker.can_intervene(clock.now());
        assert_eq!(
            status_throttled.throttling_reason(),
            Some(ThrottlingReason::RapidSuccessionRateLimit),
            "Hour {hour} 5th intervention must trigger RapidSuccessionRateLimit"
        );
        // Distance to 3600s from hour start: 3600 - 1200 = 2400s
        assert_eq!(status_throttled.remaining_seconds(), 2400);

        // Advance to exact end of the hour (t = 3600 from hour start)
        let advance_to_next_hour = 3600 - 1200;
        clock.advance_seconds(advance_to_next_hour);

        // Verify clock is at hour_start + 3600s
        assert_eq!(clock.now(), hour_start + Duration::seconds(3600));

        // The oldest intervention in the window (from hour_start) has now rolled out!
        let status_restored = tracker.can_intervene(clock.now());
        assert_eq!(
            status_restored,
            CooldownStatus::Allowed,
            "At exact boundary hour {hour} + 3600s, rolling capacity must restore"
        );
    }

    assert_eq!(tracker.total_interventions(), 40);
}

// =========================================================================
// 6. TARGET SUPPRESSION: NORMALIZATION, SUBDOMAINS, AND ANTI-SPOOFING
// =========================================================================

#[test]
fn test_empirical_origin_normalization_and_matching() {
    // 1. Normalization unit behavior
    assert_eq!(
        normalize_origin("https://www.youtube.com/watch?v=abc#t=10"),
        "youtube.com"
    );
    assert_eq!(
        normalize_origin("http://sub.domain.co.uk:8080/path?query=1"),
        "sub.domain.co.uk"
    );
    assert_eq!(normalize_origin("  WWW.REDDIT.COM/r/rust/  "), "reddit.com");
    assert_eq!(normalize_origin("localhost:3000"), "localhost");
    assert_eq!(normalize_origin("https://127.0.0.1:8000/api"), "127.0.0.1");

    // 2. TargetSuppression matching semantics
    let mut clock = EmpiricalClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // User corrects origin "github.com"
    tracker.record_targeted_correction(
        clock.now(),
        CorrectionScope::Session,
        None,
        Some("github.com".to_string()),
    );

    clock.advance_seconds(60);

    // Allowed subdomains must match
    assert!(tracker
        .can_intervene_for_target(clock.now(), None, Some("https://gist.github.com/"))
        .is_throttled());
    assert!(tracker
        .can_intervene_for_target(
            clock.now(),
            None,
            Some("http://api.v2.github.com:443/repos")
        )
        .is_throttled());
    assert!(tracker
        .can_intervene_for_target(clock.now(), None, Some("www.github.com"))
        .is_throttled());

    // Spoofed suffixes must NOT match (security defense)
    assert_eq!(
        tracker.can_intervene_for_target(clock.now(), None, Some("https://evilgithub.com/")),
        CooldownStatus::Allowed
    );
    assert_eq!(
        tracker.can_intervene_for_target(clock.now(), None, Some("https://my-github.com/")),
        CooldownStatus::Allowed
    );
    assert_eq!(
        tracker.can_intervene_for_target(clock.now(), None, Some("https://fake-github.com.org/")),
        CooldownStatus::Allowed
    );

    // 3. App ID case-insensitive matching
    tracker.record_targeted_correction(
        clock.now(),
        CorrectionScope::Session,
        Some("Code".to_string()), // VS Code
        None,
    );

    assert!(tracker
        .can_intervene_for_target(clock.now(), Some("code"), None)
        .is_throttled());
    assert!(tracker
        .can_intervene_for_target(clock.now(), Some("CODE"), None)
        .is_throttled());
    assert!(tracker
        .can_intervene_for_target(clock.now(), Some("  code  "), None)
        .is_throttled());
    assert_eq!(
        tracker.can_intervene_for_target(clock.now(), Some("code-insiders"), None),
        CooldownStatus::Allowed
    );

    // 4. UserCorrectionSummary integration
    let summary = UserCorrectionSummary {
        id: None,
        label: ClassificationLabel::Relevant,
        scope: CorrectionScope::Session,
        app_id: Some("Terminal".to_string()),
        origin: None,
        window_title_pattern: None,
        note: None,
        created_at: Some(clock.now()),
    };
    tracker.record_correction_summary(clock.now(), &summary);
    assert!(tracker
        .can_intervene_for_target(clock.now(), Some("terminal"), None)
        .is_throttled());
}

// =========================================================================
// 7. SNOOZE CLAMPING, EXTENSION, AND BOUNDARY EXPIRATION
// =========================================================================

#[test]
fn test_empirical_snooze_clamping_and_boundaries() {
    let mut clock = EmpiricalClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // 1. Snooze 0 defaults to default_snooze_seconds (600s)
    tracker.record_snooze(clock.now(), 0).unwrap();
    assert_eq!(tracker.remaining_snooze_seconds(clock.now()), 600);
    assert!(tracker.is_snoozed(clock.now()));

    // 2. Snooze exceeding max_snooze_seconds (7200s) is clamped
    tracker.record_snooze(clock.now(), 999_999).unwrap();
    assert_eq!(tracker.remaining_snooze_seconds(clock.now()), 7200);

    // 3. Shorter subsequent snooze does not reduce existing longer snooze
    tracker.record_snooze(clock.now(), 300).unwrap();
    assert_eq!(tracker.remaining_snooze_seconds(clock.now()), 7200);

    // 4. Exact expiration boundary:
    // At 7199s: 1 second remaining
    clock.advance_seconds(7199);
    assert_eq!(tracker.remaining_snooze_seconds(clock.now()), 1);
    assert_eq!(
        tracker.can_intervene(clock.now()),
        CooldownStatus::Throttled {
            reason: ThrottlingReason::SnoozeActive,
            remaining_seconds: 1,
        }
    );

    // At 7200s: Exactly expired!
    clock.advance_seconds(1);
    assert_eq!(tracker.remaining_snooze_seconds(clock.now()), 0);
    assert!(!tracker.is_snoozed(clock.now()));
    assert_eq!(tracker.can_intervene(clock.now()), CooldownStatus::Allowed);

    // 5. Cancellation
    tracker.record_snooze(clock.now(), 3600).unwrap();
    assert!(tracker.is_snoozed(clock.now()));
    tracker.cancel_snooze();
    assert!(!tracker.is_snoozed(clock.now()));
    assert_eq!(tracker.can_intervene(clock.now()), CooldownStatus::Allowed);
}

// =========================================================================
// 8. BREAK AND IDLE STATE ACTIONS FILTERING
// =========================================================================

#[test]
fn test_empirical_break_and_idle_state_filtering() {
    let clock = EmpiricalClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // 1. Idle state transitions
    tracker.set_idle_state(true, clock.now());
    assert!(tracker.is_idle());

    let status_idle = tracker.can_intervene(clock.now());
    assert_eq!(
        status_idle,
        CooldownStatus::Throttled {
            reason: ThrottlingReason::IdleActive,
            remaining_seconds: DEFAULT_IDLE_COOLDOWN_SECONDS,
        }
    );

    tracker.set_idle_state(false, clock.now());
    assert!(!tracker.is_idle());
    assert_eq!(tracker.can_intervene(clock.now()), CooldownStatus::Allowed);

    // 2. Break state action candidate filtering
    tracker.set_break_state(true, clock.now());
    assert!(tracker.is_break_state());

    // SuggestBreak during break is always blocked
    let status_suggest =
        tracker.can_intervene_for_action(clock.now(), ActionCandidate::SuggestBreak);
    assert_eq!(
        status_suggest.throttling_reason(),
        Some(ThrottlingReason::BreakStateActive)
    );

    // AskContext during break is blocked
    let status_ask = tracker.can_intervene_for_action(clock.now(), ActionCandidate::AskContext);
    assert_eq!(
        status_ask.throttling_reason(),
        Some(ThrottlingReason::BreakStateActive)
    );

    // Remind during break is allowed if allow_break_reminders = true (default)
    let status_remind = tracker.can_intervene_for_action(clock.now(), ActionCandidate::Remind);
    assert_eq!(status_remind, CooldownStatus::Allowed);

    // Remind during break is blocked if allow_break_reminders = false
    let strict_break_config = CooldownConfig {
        allow_break_reminders: false,
        ..Default::default()
    };
    let mut strict_tracker = CooldownTracker::new(strict_break_config).unwrap();
    strict_tracker.set_break_state(true, clock.now());

    let status_strict_remind =
        strict_tracker.can_intervene_for_action(clock.now(), ActionCandidate::Remind);
    assert_eq!(
        status_strict_remind.throttling_reason(),
        Some(ThrottlingReason::BreakStateActive)
    );

    // 3. Context-level evaluation
    let ctx_idle =
        build_context_fixture(Some("browser"), Some("youtube.com"), false, IdleState::Idle);
    let status_ctx_idle = tracker.can_intervene_context(clock.now(), &ctx_idle);
    assert_eq!(
        status_ctx_idle.throttling_reason(),
        Some(ThrottlingReason::IdleActive)
    );
}

// =========================================================================
// 9. CLOCK REGRESSION STRESS & MONOTONIC RECOVERY
// =========================================================================

#[test]
fn test_empirical_clock_regression_stress() {
    let mut clock = EmpiricalClock::new("2026-10-24T12:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    tracker.record_intervention(clock.now(), ActionCandidate::Remind);

    // Test a sweep of negative time jumps
    let regressions_seconds = [1, 5, 30, 60, 300, 3600, 86400, 1_000_000];

    for &rewind_s in &regressions_seconds {
        let mut regressed_clock = clock.clone();
        regressed_clock.rewind_seconds(rewind_s);

        let status = tracker.can_intervene(regressed_clock.now());
        assert_eq!(
            status,
            CooldownStatus::Throttled {
                reason: ThrottlingReason::TimeRegression,
                remaining_seconds: DEFAULT_MIN_INTERVAL_SECONDS,
            },
            "Rewinding by {rewind_s}s must return TimeRegression throttle"
        );
    }

    // Monotonic recovery: once clock advances >= 300s past original intervention
    clock.advance_seconds(300);
    assert_eq!(tracker.can_intervene(clock.now()), CooldownStatus::Allowed);
}

// =========================================================================
// 10. MEMORY BOUNDING UNDER MASSIVE 50,000-EVENT STREAMS
// =========================================================================

#[test]
fn test_empirical_massive_event_stream_memory_bounds() {
    let mut clock = EmpiricalClock::new("2026-10-24T00:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // Stream 50,000 rapid interventions
    for _ in 0..50_000 {
        clock.advance_seconds(1);
        tracker.record_intervention(clock.now(), ActionCandidate::Remind);
    }

    // Must be strictly capped at MAX_TRACKED_INTERVENTIONS (64)
    assert!(
        tracker.recent_interventions_count(clock.now()) <= MAX_TRACKED_INTERVENTIONS,
        "Recent interventions count must not exceed 64"
    );
    assert_eq!(tracker.total_interventions(), 50_000);

    // Stream 50,000 distinct app suppressions
    for i in 0..50_000 {
        clock.advance_seconds(1);
        tracker.record_targeted_correction(
            clock.now(),
            CorrectionScope::CurrentEvent,
            Some(format!("app_{i}")),
            None,
        );
    }

    // Tracker must remain fully responsive and deterministic
    assert!(tracker.can_intervene(clock.now()).is_throttled());
}

// =========================================================================
// 11. SERDE SERIALIZATION & ERROR TAXONOMY ROUNDTRIP
// =========================================================================

#[test]
fn test_empirical_serde_roundtrip_all_enums_and_configs() {
    // 1. ThrottlingReason variants
    let all_reasons = [
        (ThrottlingReason::RecentIntervention, "recent_intervention"),
        (
            ThrottlingReason::RapidSuccessionRateLimit,
            "rapid_succession_rate_limit",
        ),
        (ThrottlingReason::SessionCapReached, "session_cap_reached"),
        (ThrottlingReason::SnoozeActive, "snooze_active"),
        (
            ThrottlingReason::UserCorrectionSuppressed,
            "user_correction_suppressed",
        ),
        (ThrottlingReason::BreakStateActive, "break_state_active"),
        (ThrottlingReason::IdleActive, "idle_active"),
        (ThrottlingReason::TimeRegression, "time_regression"),
    ];

    for (variant, expected_tag) in all_reasons {
        let serialized = serde_json::to_string(&variant).expect("Serialize reason");
        assert_eq!(
            serialized,
            format!("\"{expected_tag}\""),
            "Reason {variant:?} must serialize to snake_case {expected_tag}"
        );
        let deserialized: ThrottlingReason =
            serde_json::from_str(&serialized).expect("Deserialize reason");
        assert_eq!(variant, deserialized);
    }

    // 2. CooldownStatus serialization
    let status_allowed = CooldownStatus::Allowed;
    let allowed_json = serde_json::to_value(&status_allowed).unwrap();
    assert_eq!(allowed_json, json!({"status": "allowed"}));

    let status_throttled = CooldownStatus::Throttled {
        reason: ThrottlingReason::RecentIntervention,
        remaining_seconds: 120,
    };
    let throttled_json = serde_json::to_value(&status_throttled).unwrap();
    assert_eq!(
        throttled_json,
        json!({
            "status": "throttled",
            "reason": "recent_intervention",
            "remaining_seconds": 120
        })
    );

    // 3. Error conversion
    let err = status_throttled.to_coach_error().unwrap();
    match err {
        CoachError::CooldownThrottled {
            remaining_seconds,
            reason,
        } => {
            assert_eq!(remaining_seconds, 120);
            assert_eq!(reason, "recent_intervention");
        }
        other => panic!("Expected CooldownThrottled error variant, got: {other:?}"),
    }

    assert_eq!(status_allowed.to_coach_error(), None);
}

// =========================================================================
// 12. TONE PROFILE COOLDOWN PRESETS AND VALIDATION BOUNDS
// =========================================================================

#[test]
fn test_empirical_tone_presets_and_safety_bounds() {
    // Verify preset relationships
    let gentle = CooldownConfig::gentle();
    let strict = CooldownConfig::strict();
    let default = CooldownConfig::default();

    assert!(gentle.min_interval_seconds > default.min_interval_seconds);
    assert!(strict.min_interval_seconds < default.min_interval_seconds);
    assert!(gentle.max_interventions_per_window < strict.max_interventions_per_window);

    // Tone preference adaptation across personality modes
    let modes = [
        (PersonalityMode::Supportive, gentle.min_interval_seconds),
        (PersonalityMode::Inquisitive, gentle.min_interval_seconds),
        (PersonalityMode::Pragmatic, strict.min_interval_seconds),
        (PersonalityMode::Neutral, default.min_interval_seconds),
        (PersonalityMode::Playful, default.min_interval_seconds),
    ];
    for (mode, expected_interval) in modes {
        let tone = TonePreferences {
            mode,
            ..Default::default()
        };
        let cfg = CooldownConfig::from_tone(&tone);
        assert_eq!(cfg.min_interval_seconds, expected_interval);
    }

    // Safety validation floors:
    // Minimum safe interval is MIN_SAFE_COOLDOWN_SECONDS (60s)
    let unsafe_config = CooldownConfig {
        min_interval_seconds: 59,
        ..Default::default()
    };
    assert!(unsafe_config.validate().is_err());

    // Rapid window cannot be smaller than min interval
    let bad_window = CooldownConfig {
        min_interval_seconds: 300,
        rapid_window_seconds: 299,
        ..Default::default()
    };
    assert!(bad_window.validate().is_err());

    // Max interventions per window must be >= 1
    let zero_window = CooldownConfig {
        max_interventions_per_window: 0,
        ..Default::default()
    };
    assert!(zero_window.validate().is_err());

    // Session cap must be >= 1 if Some
    let zero_cap = CooldownConfig {
        session_cap: Some(0),
        ..Default::default()
    };
    assert!(zero_cap.validate().is_err());
}
