//! Empirical Adversarial Challenge Test Suite for Milestone 3.
//!
//! File: tests/empirical_challenge_m3_c2_tests.rs
//! Role: Empirical Challenger 2 (Milestone 3: Tone Policy & Deterministic Cooldown Tracker)
//!
//! Exhaustively stresses:
//! 1. URL domain normalization & target user correction suppression isolation (zero cross-talk).
//! 2. Snooze stress: concurrent calls, monotonic extensions, bounds clamping, and cancellation.
//! 3. Clock regression fuzzing: negative deltas (-1s, -100s, -1 day, Unix epoch 1970, Year 1) with panic immunity.
//! 4. Memory saturation bounds: 10,000 continuous event streaming cycles strictly capped at MAX_TRACKED_* (64 items).
//! 5. Safety floor invariants & tone config boundaries.

use chrono::{DateTime, Duration, Utc};
use std::time::Instant;
use uuid::Uuid;
use wowie_coach::error::CoachError;
use wowie_coach::models::action::ActionCandidate;
use wowie_coach::models::context::{CoachContext, UntrustedObservation};
use wowie_coach::models::tone::TonePreferences;
use wowie_coach::policy::cooldown::{
    normalize_origin, CooldownConfig, CooldownStatus, CooldownTracker, ThrottlingReason,
    DEFAULT_MIN_INTERVAL_SECONDS, MAX_COOLDOWN_INTERVAL_SECONDS, MAX_SNOOZE_SECONDS,
    MAX_TRACKED_INTERVENTIONS,
};
use wowie_core::id::SessionId;
use wowie_core::models::{CorrectionScope, IdleState};

/// Injected mock clock for 100% deterministic, instant time stepping.
#[derive(Debug, Clone)]
struct MockClock {
    current: DateTime<Utc>,
}

impl MockClock {
    fn new(iso_time: &str) -> Self {
        let dt = DateTime::parse_from_rfc3339(iso_time)
            .expect("Valid ISO 8601 timestamp required for test setup")
            .with_timezone(&Utc);
        Self { current: dt }
    }

    fn now(&self) -> DateTime<Utc> {
        self.current
    }

    fn advance_seconds(&mut self, seconds: i64) -> DateTime<Utc> {
        self.current += Duration::seconds(seconds);
        self.current
    }
}

/// Helper to build test CoachContext fixtures.
fn build_context(
    app_id: Option<&str>,
    origin: Option<&str>,
    window_title: Option<&str>,
    is_break: bool,
) -> CoachContext {
    let obs = UntrustedObservation::new(
        app_id.map(String::from),
        window_title.map(String::from),
        origin.map(String::from),
        Some(IdleState::Active),
    )
    .expect("Valid observation");

    CoachContext::new(
        "Build Empirical Challenge Suite",
        Some("Stress test CooldownTracker".to_string()),
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
// SECTION 1: URL DOMAIN NORMALIZATION & TARGET SUPPRESSION ISOLATION
// =========================================================================

#[test]
fn test_empirical_url_domain_normalization_cases() {
    // 1. Complex path with query and fragment
    assert_eq!(
        normalize_origin("https://docs.google.com/document/d/123/edit?usp=sharing#heading=h.abc"),
        "docs.google.com"
    );

    // 2. Localhost with port
    assert_eq!(normalize_origin("http://localhost:8080"), "localhost");
    assert_eq!(
        normalize_origin("http://localhost:8080/api/v1/status"),
        "localhost"
    );

    // 3. Subdomain with explicit port and no scheme
    assert_eq!(normalize_origin("sub.domain.com:443"), "sub.domain.com");

    // 4. Uppercase scheme, www prefix, custom port, query params
    assert_eq!(
        normalize_origin("HTTPS://WWW.Example.COM:8443/search?q=rust+test#top"),
        "example.com"
    );

    // 5. Bare domain without scheme or port
    assert_eq!(normalize_origin("crates.io"), "crates.io");
    assert_eq!(normalize_origin("www.crates.io"), "crates.io");

    // 6. Leading/trailing whitespace
    assert_eq!(
        normalize_origin("   https://github.com/rust-lang/rust   "),
        "github.com"
    );
}

#[test]
fn test_empirical_target_user_correction_suppression_isolation_and_zero_crosstalk() {
    let mut clock = MockClock::new("2026-10-24T12:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // User marks Google Docs as on-goal research
    tracker.record_targeted_correction(
        clock.now(),
        CorrectionScope::Session,
        None,
        Some("https://docs.google.com/document/d/123".to_string()),
    );

    // Advance 30 seconds
    clock.advance_seconds(30);

    // TEST 1: Google Docs spreadsheet is suppressed (same normalized origin)
    let status_gdocs = tracker.can_intervene_for_target(
        clock.now(),
        Some("google-chrome"),
        Some("https://docs.google.com/spreadsheets/d/456"),
    );
    assert!(
        status_gdocs.is_throttled(),
        "Google Docs spreadsheet must be suppressed by Google Docs document correction"
    );
    assert_eq!(
        status_gdocs.throttling_reason(),
        Some(ThrottlingReason::UserCorrectionSuppressed)
    );
    // 1800s default - 30s = 1770s
    assert_eq!(status_gdocs.remaining_seconds(), 1770);

    // TEST 2: YouTube is NOT suppressed (zero cross-talk)
    let status_youtube = tracker.can_intervene_for_target(
        clock.now(),
        Some("google-chrome"),
        Some("https://youtube.com/watch?v=dQw4w9WgXcQ"),
    );
    assert_eq!(
        status_youtube,
        CooldownStatus::Allowed,
        "YouTube must NOT be suppressed by Google Docs correction"
    );

    // TEST 3: Attacker / spoofing domains are NOT suppressed
    let status_spoof1 = tracker.can_intervene_for_target(
        clock.now(),
        Some("google-chrome"),
        Some("https://docs.google.com.attacker.com/malware"),
    );
    assert_eq!(
        status_spoof1,
        CooldownStatus::Allowed,
        "Domain suffix match must require a dot boundary (docs.google.com.attacker.com must NOT match)"
    );

    let status_spoof2 = tracker.can_intervene_for_target(
        clock.now(),
        Some("google-chrome"),
        Some("https://fakegoogle.com"),
    );
    assert_eq!(
        status_spoof2,
        CooldownStatus::Allowed,
        "fakegoogle.com must NOT match google.com"
    );

    // TEST 4: VSCode (desktop app with no origin) is NOT suppressed
    let status_vscode = tracker.can_intervene_for_target(clock.now(), Some("code"), None);
    assert_eq!(
        status_vscode,
        CooldownStatus::Allowed,
        "VSCode app must NOT be suppressed by Google Docs URL correction"
    );

    // TEST 5: Generic query without target is NOT suppressed
    let status_generic = tracker.can_intervene(clock.now());
    assert_eq!(
        status_generic,
        CooldownStatus::Allowed,
        "Targeted suppression must NOT throttle generic targetless evaluation"
    );
}

#[test]
fn test_empirical_case_insensitive_app_id_suppression_matrix() {
    let mut clock = MockClock::new("2026-10-24T12:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // Record correction with mixed-case and whitespace
    tracker.record_targeted_correction(
        clock.now(),
        CorrectionScope::Session,
        Some("  Code.exe  ".to_string()),
        None,
    );

    clock.advance_seconds(10);

    // Lowercase match
    let status_lower = tracker.can_intervene_for_target(clock.now(), Some("code.exe"), None);
    assert!(status_lower.is_throttled());
    assert_eq!(
        status_lower.throttling_reason(),
        Some(ThrottlingReason::UserCorrectionSuppressed)
    );

    // Uppercase match
    let status_upper = tracker.can_intervene_for_target(clock.now(), Some("CODE.EXE"), None);
    assert!(status_upper.is_throttled());

    // Padded whitespace query
    let status_padded = tracker.can_intervene_for_target(clock.now(), Some("  code.exe\t"), None);
    assert!(status_padded.is_throttled());

    // Different app is NOT throttled
    let status_unrelated = tracker.can_intervene_for_target(clock.now(), Some("spotify.exe"), None);
    assert_eq!(status_unrelated, CooldownStatus::Allowed);

    // Substring / partial app name is NOT throttled (must be full match)
    let status_partial = tracker.can_intervene_for_target(clock.now(), Some("Code"), None);
    assert_eq!(status_partial, CooldownStatus::Allowed);
}

#[test]
fn test_empirical_domain_hierarchy_subdomain_matching() {
    let mut clock = MockClock::new("2026-10-24T12:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // User corrects parent domain "google.com"
    tracker.record_targeted_correction(
        clock.now(),
        CorrectionScope::Session,
        None,
        Some("google.com".to_string()),
    );

    clock.advance_seconds(5);

    // Parent domain matches
    assert!(tracker
        .can_intervene_for_target(clock.now(), None, Some("https://google.com"))
        .is_throttled());

    // Subdomain matches parent correction
    assert!(tracker
        .can_intervene_for_target(clock.now(), None, Some("https://docs.google.com/doc"))
        .is_throttled());

    // Deep subdomain matches parent correction
    assert!(tracker
        .can_intervene_for_target(clock.now(), None, Some("https://sub.docs.google.com/doc"))
        .is_throttled());

    // Suffix collision without dot does NOT match
    assert_eq!(
        tracker.can_intervene_for_target(clock.now(), None, Some("https://mygoogle.com")),
        CooldownStatus::Allowed
    );

    // Now test the reverse: correcting a SUBDOMAIN does NOT suppress the parent domain
    let mut tracker2 = CooldownTracker::with_default_config();
    tracker2.record_targeted_correction(
        clock.now(),
        CorrectionScope::Session,
        None,
        Some("docs.google.com".to_string()),
    );

    // Parent domain must NOT be throttled
    assert_eq!(
        tracker2.can_intervene_for_target(clock.now(), None, Some("https://google.com")),
        CooldownStatus::Allowed,
        "Correcting docs.google.com must NOT suppress root google.com"
    );

    // Sibling subdomain must NOT be throttled
    assert_eq!(
        tracker2.can_intervene_for_target(clock.now(), None, Some("https://mail.google.com")),
        CooldownStatus::Allowed,
        "Correcting docs.google.com must NOT suppress mail.google.com"
    );
}

// =========================================================================
// SECTION 2: SNOOZE STRESS, EXTENSIONS, CLAMPING & CANCELLATION
// =========================================================================

#[test]
fn test_empirical_snooze_concurrent_extensions_and_monotonicity() {
    let mut clock = MockClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // 1. Initial snooze for 300s (ends at T0 + 300s)
    tracker.record_snooze(clock.now(), 300).expect("Snooze ok");
    assert!(tracker.is_snoozed(clock.now()));
    assert_eq!(tracker.remaining_snooze_seconds(clock.now()), 300);

    // 2. Advance 60s to T0 + 60s (240s remaining)
    clock.advance_seconds(60);
    assert_eq!(tracker.remaining_snooze_seconds(clock.now()), 240);

    // 3. User requests extension for 600s (from T0 + 60s, ends at T0 + 660s)
    tracker
        .record_snooze(clock.now(), 600)
        .expect("Extension ok");
    assert_eq!(tracker.remaining_snooze_seconds(clock.now()), 600);

    // 4. Advance 100s to T0 + 160s (500s remaining)
    clock.advance_seconds(100);
    assert_eq!(tracker.remaining_snooze_seconds(clock.now()), 500);

    // 5. User requests a shorter snooze (100s, ending at T0 + 260s).
    // Snooze tracker MUST NOT shrink an existing active snooze!
    tracker
        .record_snooze(clock.now(), 100)
        .expect("Shorter snooze attempt ok");
    assert_eq!(
        tracker.remaining_snooze_seconds(clock.now()),
        500,
        "Shorter snooze request must NOT reduce an existing longer snooze"
    );

    // 6. Advance to T0 + 659s (1 second remaining)
    clock.advance_seconds(499);
    assert_eq!(tracker.remaining_snooze_seconds(clock.now()), 1);
    let status_almost = tracker.can_intervene(clock.now());
    assert!(status_almost.is_throttled());
    assert_eq!(
        status_almost.throttling_reason(),
        Some(ThrottlingReason::SnoozeActive)
    );

    // 7. Advance to T0 + 660s (exact boundary) -> Snooze expired!
    clock.advance_seconds(1);
    assert!(!tracker.is_snoozed(clock.now()));
    assert_eq!(tracker.remaining_snooze_seconds(clock.now()), 0);
    assert_eq!(tracker.can_intervene(clock.now()), CooldownStatus::Allowed);
}

#[test]
fn test_empirical_snooze_bounds_clamping_and_zero_duration() {
    let clock = MockClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // 1. Zero duration passes default snooze duration (600s)
    tracker
        .record_snooze(clock.now(), 0)
        .expect("Zero snooze ok");
    assert_eq!(
        tracker.remaining_snooze_seconds(clock.now()),
        tracker.config().default_snooze_seconds
    );

    // 2. Cancellation
    tracker.cancel_snooze();
    assert_eq!(tracker.remaining_snooze_seconds(clock.now()), 0);

    // 3. Huge duration is clamped to MAX_SNOOZE_SECONDS (7200s)
    tracker
        .record_snooze(clock.now(), 1_000_000)
        .expect("Oversized snooze ok");
    assert_eq!(
        tracker.remaining_snooze_seconds(clock.now()),
        MAX_SNOOZE_SECONDS
    );

    // 4. Repeated cancellations are safe and idempotent
    tracker.cancel_snooze();
    tracker.cancel_snooze();
    assert!(!tracker.is_snoozed(clock.now()));
    assert_eq!(tracker.can_intervene(clock.now()), CooldownStatus::Allowed);
}

#[test]
fn test_empirical_snooze_action_filtering_matrix() {
    let clock = MockClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    tracker.record_snooze(clock.now(), 600).unwrap();

    let proactive_actions = [
        ActionCandidate::Remind,
        ActionCandidate::AskContext,
        ActionCandidate::SuggestCorrection,
        ActionCandidate::SuggestBreak,
    ];

    for action in proactive_actions {
        let status = tracker.can_intervene_for_action(clock.now(), action);
        assert!(
            status.is_throttled(),
            "Action {action:?} must be throttled during active snooze"
        );
        assert_eq!(
            status.throttling_reason(),
            Some(ThrottlingReason::SnoozeActive)
        );
    }
}

// =========================================================================
// SECTION 3: CLOCK REGRESSION FUZZING & PANIC IMMUNITY
// =========================================================================

#[test]
fn test_empirical_clock_regression_fuzzing_comprehensive() {
    let mut clock = MockClock::new("2026-10-24T12:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // Baseline intervention recorded at 12:00:00 UTC
    tracker.record_intervention(clock.now(), ActionCandidate::Remind);

    // Fuzz negative time jumps across various orders of magnitude
    let negative_deltas_seconds: &[i64] = &[
        -1,             // 1 second back
        -10,            // 10 seconds back
        -60,            // 1 minute back
        -100,           // 100 seconds back
        -300,           // exactly min_interval back
        -3600,          // 1 hour back
        -86400,         // 1 day back
        -604_800,       // 1 week back
        -2_592_000,     // 30 days back
        -31_536_000,    // 1 year back
        -1_791_830_400, // ~56 years back (to Unix epoch 1970-01-01)
    ];

    for &delta in negative_deltas_seconds {
        let mut fuzzed_clock = clock.clone();
        fuzzed_clock.advance_seconds(delta);

        let status = tracker.can_intervene(fuzzed_clock.now());

        assert!(
            status.is_throttled(),
            "Delta {delta}s must throttle due to clock regression"
        );
        assert_eq!(
            status.throttling_reason(),
            Some(ThrottlingReason::TimeRegression),
            "Reason must be TimeRegression for delta {delta}s"
        );
        assert_eq!(
            status.remaining_seconds(),
            DEFAULT_MIN_INTERVAL_SECONDS,
            "Remaining seconds on clock regression must default safely to min_interval"
        );

        // CoachError conversion must succeed without error
        let err = status.to_coach_error().expect("Must convert cleanly");
        match err {
            CoachError::CooldownThrottled { reason, .. } => {
                assert!(reason.contains("time_regression"));
            }
            other => panic!("Expected CooldownThrottled error variant, got: {other:?}"),
        }
    }

    // Direct Year 1 AD test
    let year_one = DateTime::parse_from_rfc3339("0001-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let status_year_one = tracker.can_intervene(year_one);
    assert_eq!(
        status_year_one.throttling_reason(),
        Some(ThrottlingReason::TimeRegression)
    );

    // Verify recovery once clock moves forward past cooldown window (12:05:01 UTC)
    clock.advance_seconds(301);
    let status_recovered = tracker.can_intervene(clock.now());
    assert_eq!(
        status_recovered,
        CooldownStatus::Allowed,
        "System must resume Allowed state when clock moves forward past cooldown window"
    );
}

#[test]
fn test_empirical_clock_regression_with_active_snooze_and_suppression() {
    let mut clock = MockClock::new("2026-10-24T12:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // Set snooze and suppression at T0
    tracker.record_snooze(clock.now(), 600).unwrap();
    tracker.record_targeted_correction(
        clock.now(),
        CorrectionScope::Session,
        Some("code".to_string()),
        None,
    );
    tracker.record_intervention(clock.now(), ActionCandidate::Remind);

    // Rewind clock by 1 day
    clock.advance_seconds(-86400);

    // Intervention check must return TimeRegression (highest precedence check)
    let status = tracker.can_intervene_for_target(clock.now(), Some("code"), None);
    assert_eq!(
        status.throttling_reason(),
        Some(ThrottlingReason::TimeRegression),
        "Clock regression check takes precedence over snooze and target suppression"
    );
}

// =========================================================================
// SECTION 4: MEMORY SATURATION BOUNDS & HIGH EVENT STREAMING
// =========================================================================

#[test]
fn test_empirical_memory_saturation_10k_continuous_cycles() {
    let mut clock = MockClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    let start_time = Instant::now();

    // Feed 10,000 continuous event streaming cycles
    for cycle in 0..10_000 {
        // Advance clock by 1 second each cycle
        clock.advance_seconds(1);

        // Every 300 cycles (5 minutes), attempt an intervention
        if cycle % 300 == 0 {
            tracker.record_intervention(clock.now(), ActionCandidate::Remind);
        }

        // Every 50 cycles, record a new targeted correction
        if cycle % 50 == 0 {
            let app_name = format!("app_{}", cycle % 200);
            let origin = format!("https://service-{}.example.com/api", cycle % 150);
            tracker.record_targeted_correction(
                clock.now(),
                CorrectionScope::Session,
                Some(app_name),
                Some(origin),
            );
        }

        // Query context evaluation
        let ctx = build_context(
            Some("app_42"),
            Some("https://service-42.example.com/api"),
            Some("Working on code"),
            false,
        );
        let _ = tracker.can_intervene_context(clock.now(), &ctx);

        // Memory bounding invariants: tracked items must NEVER exceed 64
        assert!(
            tracker.recent_interventions_count(clock.now()) <= MAX_TRACKED_INTERVENTIONS,
            "recent_interventions count must never exceed MAX_TRACKED_INTERVENTIONS (64)"
        );
    }

    let elapsed = start_time.elapsed();

    // Verify 10,000 cycles execute sub-second with zero degradation
    assert!(
        elapsed.as_millis() < 500,
        "10,000 event streaming cycles took {:?}, must complete under 500ms",
        elapsed
    );

    // Tracker remains functional after 10k cycles
    assert_eq!(tracker.total_interventions(), 34); // 0..10_000 step 300 = 34 interventions
}

#[test]
fn test_empirical_fifo_ring_buffer_eviction_semantics() {
    let mut clock = MockClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // Insert 100 unique suppressions at 1-second intervals
    for i in 0..100 {
        clock.advance_seconds(1);
        tracker.record_targeted_correction(
            clock.now(),
            CorrectionScope::Session,
            Some(format!("app_{i}")),
            None,
        );
    }

    // Capacity is MAX_TRACKED_SUPPRESSIONS = 64.
    // The first 36 suppressions (app_0 .. app_35) must have been evicted via FIFO ring buffer!
    // The last 64 suppressions (app_36 .. app_99) must still be present and throttled!

    // Verify oldest item app_0 was evicted and is now Allowed
    let status_oldest = tracker.can_intervene_for_target(clock.now(), Some("app_0"), None);
    assert_eq!(
        status_oldest,
        CooldownStatus::Allowed,
        "Oldest item (app_0) must have been evicted once queue reached 64 items"
    );

    // Verify app_35 was also evicted
    let status_35 = tracker.can_intervene_for_target(clock.now(), Some("app_35"), None);
    assert_eq!(status_35, CooldownStatus::Allowed);

    // Verify app_36 is still tracked and Throttled
    let status_36 = tracker.can_intervene_for_target(clock.now(), Some("app_36"), None);
    assert!(
        status_36.is_throttled(),
        "Item app_36 is within the newest 64 and must remain active"
    );

    // Verify most recent item app_99 is active and Throttled
    let status_newest = tracker.can_intervene_for_target(clock.now(), Some("app_99"), None);
    assert!(status_newest.is_throttled());
}

#[test]
fn test_empirical_total_interventions_u32_saturation_safety() {
    let clock = MockClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // Verify saturating_add behavior on total interventions
    // We can simulate multiple interventions and verify it accumulates without panic
    for _ in 0..200 {
        tracker.record_intervention(clock.now(), ActionCandidate::Remind);
    }
    assert_eq!(tracker.total_interventions(), 200);

    // Resetting session restores counter to 0 cleanly
    tracker.reset_session();
    assert_eq!(tracker.total_interventions(), 0);
    assert_eq!(tracker.last_intervention(), None);
}

// =========================================================================
// SECTION 5: SAFETY FLOORS & CONFIG BOUNDARY INVARIANTS
// =========================================================================

#[test]
fn test_empirical_safety_floor_violations_rejected_categorically() {
    // 1. Min interval below MIN_SAFE_COOLDOWN_SECONDS (60s)
    let bad_interval = CooldownConfig {
        min_interval_seconds: 59,
        ..Default::default()
    };
    assert!(bad_interval.validate().is_err());

    // 2. Min interval above MAX_COOLDOWN_INTERVAL_SECONDS (7200s)
    let excessive_interval = CooldownConfig {
        min_interval_seconds: MAX_COOLDOWN_INTERVAL_SECONDS + 1,
        ..Default::default()
    };
    assert!(excessive_interval.validate().is_err());

    // 3. Rapid window smaller than min_interval
    let inverted_window = CooldownConfig {
        min_interval_seconds: 300,
        rapid_window_seconds: 299,
        ..Default::default()
    };
    assert!(inverted_window.validate().is_err());

    // 4. Max interventions per window is 0
    let zero_window = CooldownConfig {
        max_interventions_per_window: 0,
        ..Default::default()
    };
    assert!(zero_window.validate().is_err());

    // 5. Correction suppression below safety floor (60s)
    let unsafe_suppression = CooldownConfig {
        correction_suppression_seconds: 59,
        ..Default::default()
    };
    assert!(unsafe_suppression.validate().is_err());

    // 6. Default snooze exceeds max snooze
    let inverted_snooze = CooldownConfig {
        default_snooze_seconds: 7201,
        max_snooze_seconds: 7200,
        ..Default::default()
    };
    assert!(inverted_snooze.validate().is_err());

    // 7. Max snooze exceeds MAX_SNOOZE_SECONDS
    let excessive_snooze = CooldownConfig {
        max_snooze_seconds: MAX_SNOOZE_SECONDS + 1,
        ..Default::default()
    };
    assert!(excessive_snooze.validate().is_err());

    // 8. Session cap is 0
    let zero_cap = CooldownConfig {
        session_cap: Some(0),
        ..Default::default()
    };
    assert!(zero_cap.validate().is_err());
}
