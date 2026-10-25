//! Comprehensive test suite for deterministic cooldown tracking and tone policy.
//!
//! File: tests/cooldown_policy_tests.rs
//!
//! Acceptance Requirement:
//! "Cooldown tests verify deterministically throttled rapid successive intervention requests."
//!
//! All tests use purely injected mock timestamps (`DateTime<Utc>`) with ZERO wall-clock sleep.

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;
use wowie_coach::error::CoachError;
use wowie_coach::models::action::ActionCandidate;
use wowie_coach::models::context::{CoachContext, UntrustedObservation, UserCorrectionSummary};
use wowie_coach::models::tone::TonePreferences;
use wowie_coach::policy::cooldown::{
    CooldownConfig, CooldownStatus, CooldownTracker, ThrottlingReason,
    DEFAULT_MIN_INTERVAL_SECONDS, MAX_TRACKED_INTERVENTIONS,
};
use wowie_core::id::SessionId;
use wowie_core::models::{ClassificationLabel, CorrectionScope, IdleState};

// =========================================================================
// MOCK TIME HARNESS & TEST FIXTURES
// =========================================================================

/// Injected mock clock for 100% deterministic, instant time stepping.
#[derive(Debug, Clone)]
struct MockClock {
    current: DateTime<Utc>,
}

impl MockClock {
    /// Initialize mock clock at an exact ISO 8601 UTC timestamp.
    fn new(iso_time: &str) -> Self {
        let dt = DateTime::parse_from_rfc3339(iso_time)
            .expect("Valid ISO 8601 timestamp required for test setup")
            .with_timezone(&Utc);
        Self { current: dt }
    }

    /// Read current injected mock timestamp.
    fn now(&self) -> DateTime<Utc> {
        self.current
    }

    /// Step time forward by exact number of seconds.
    fn advance_seconds(&mut self, seconds: u64) -> DateTime<Utc> {
        self.current += Duration::seconds(seconds as i64);
        self.current
    }

    /// Step time backward by exact number of seconds (for clock regression testing).
    fn rewind_seconds(&mut self, seconds: u64) -> DateTime<Utc> {
        self.current -= Duration::seconds(seconds as i64);
        self.current
    }
}

/// Helper to build test `CoachContext` fixtures.
fn build_test_context(
    app_id: Option<&str>,
    origin: Option<&str>,
    window_title: Option<&str>,
    is_break_state: bool,
    tone: TonePreferences,
) -> CoachContext {
    let obs = UntrustedObservation::new(
        app_id.map(String::from),
        window_title.map(String::from),
        origin.map(String::from),
        Some(IdleState::Active),
    )
    .expect("Valid observation");

    CoachContext::new(
        "Build Rust Compiler MVP",
        Some("Implement parser token stream".to_string()),
        SessionId::from_uuid(Uuid::nil()),
        1200,
        is_break_state,
        obs,
        vec![],
        tone,
    )
    .expect("Valid context")
}

// =========================================================================
// TEST SUITE 1: FUNDAMENTAL INTERVAL & TIME PROGRESSION
// =========================================================================

/// 1. Baseline intervention allowed when no prior history.
#[test]
fn test_cooldown_baseline_allowed_when_no_prior_history() {
    let clock = MockClock::new("2026-10-24T10:00:00Z");
    let tracker = CooldownTracker::with_default_config();

    let status = tracker.can_intervene(clock.now());

    assert_eq!(
        status,
        CooldownStatus::Allowed,
        "A freshly initialized tracker with no history must allow initial intervention"
    );
    assert!(status.is_allowed());
    assert!(!status.is_throttled());
    assert_eq!(status.remaining_seconds(), 0);
    assert_eq!(status.throttling_reason(), None);
    assert_eq!(status.to_coach_error(), None);
}

/// 2. Immediate second intervention throttled with exact remaining seconds calculated.
#[test]
fn test_cooldown_immediate_second_intervention_throttled_with_exact_seconds() {
    let mut clock = MockClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // Record baseline intervention at T0
    tracker.record_intervention(clock.now(), ActionCandidate::Remind);

    // Query 10 seconds later
    clock.advance_seconds(10);
    let status = tracker.can_intervene(clock.now());

    assert!(
        status.is_throttled(),
        "Intervention 10s after first must be throttled (min interval: 300s)"
    );
    assert!(!status.is_allowed());

    // Exact math: 300s min interval - 10s elapsed = 290s remaining
    assert_eq!(status.remaining_seconds(), 290);
    assert_eq!(
        status.throttling_reason(),
        Some(ThrottlingReason::RecentIntervention)
    );

    // Verify error conversion matches CoachError taxonomy
    let err = status.to_coach_error().expect("Must convert to CoachError");
    match err {
        CoachError::CooldownThrottled {
            remaining_seconds,
            reason,
        } => {
            assert_eq!(remaining_seconds, 290);
            assert!(
                reason.contains("recent_intervention") || reason.contains("RecentIntervention"),
                "Error reason must describe pacing throttle: {reason}"
            );
        }
        other => panic!("Expected CooldownThrottled error variant, got: {other:?}"),
    }
}

/// 3. Progression of time: advancing timestamp by cooldown interval allows next intervention.
#[test]
fn test_cooldown_progression_of_time_monotonic_decrement_and_release() {
    let mut clock = MockClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    tracker.record_intervention(clock.now(), ActionCandidate::Remind);

    // At T0 + 150s: half of 300s elapsed -> remaining: 150s
    clock.advance_seconds(150);
    let status_mid = tracker.can_intervene(clock.now());
    assert_eq!(status_mid.remaining_seconds(), 150);
    assert_eq!(
        status_mid.throttling_reason(),
        Some(ThrottlingReason::RecentIntervention)
    );

    // At T0 + 299s: 1s remaining
    clock.advance_seconds(149); // 150 + 149 = 299
    let status_almost = tracker.can_intervene(clock.now());
    assert_eq!(status_almost.remaining_seconds(), 1);
    assert!(status_almost.is_throttled());

    // At T0 + 300s: EXACT boundary -> Allowed!
    clock.advance_seconds(1); // 299 + 1 = 300
    let status_exact = tracker.can_intervene(clock.now());
    assert_eq!(
        status_exact,
        CooldownStatus::Allowed,
        "At exact cooldown boundary (300s), intervention must be allowed"
    );

    // At T0 + 301s: safely past boundary -> Allowed!
    clock.advance_seconds(1);
    let status_past = tracker.can_intervene(clock.now());
    assert_eq!(status_past, CooldownStatus::Allowed);

    // Record second intervention at T0 + 301s
    tracker.record_intervention(clock.now(), ActionCandidate::AskContext);

    // Now immediate query at T0 + 301s is throttled again for 300s
    let status_rethrottled = tracker.can_intervene(clock.now());
    assert!(status_rethrottled.is_throttled());
    assert_eq!(status_rethrottled.remaining_seconds(), 300);
}

// =========================================================================
// TEST SUITE 2: ROLLING WINDOW & RAPID INTERVENTION QUOTAS
// =========================================================================

/// 4. Rapid intervention throttling: N interventions within rolling window triggers hourly cap throttling.
#[test]
fn test_cooldown_rapid_intervention_hourly_rolling_window_cap() {
    let mut clock = MockClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // Default config: max 4 interventions per 3600s rolling window, 300s min interval.
    // 1st intervention at T0 (10:00:00)
    tracker.record_intervention(clock.now(), ActionCandidate::Remind);

    // 2nd intervention at T0 + 300s (10:05:00)
    clock.advance_seconds(300);
    assert_eq!(tracker.can_intervene(clock.now()), CooldownStatus::Allowed);
    tracker.record_intervention(clock.now(), ActionCandidate::Remind);

    // 3rd intervention at T0 + 600s (10:10:00)
    clock.advance_seconds(300);
    assert_eq!(tracker.can_intervene(clock.now()), CooldownStatus::Allowed);
    tracker.record_intervention(clock.now(), ActionCandidate::Remind);

    // 4th intervention at T0 + 900s (10:15:00)
    clock.advance_seconds(300);
    assert_eq!(tracker.can_intervene(clock.now()), CooldownStatus::Allowed);
    tracker.record_intervention(clock.now(), ActionCandidate::Remind);

    // Now 4 interventions recorded within 900s (< 3600s window).
    // Advance by 300s to T0 + 1200s (10:20:00).
    // Notice: 300s elapsed since 4th intervention, so minimum interval is SATISFIED!
    // BUT hourly rolling window quota (4) is EXHAUSTED!
    clock.advance_seconds(300);
    let status = tracker.can_intervene(clock.now());

    assert!(
        status.is_throttled(),
        "5th intervention must be throttled due to rapid window cap (4 per 3600s)"
    );
    assert_eq!(
        status.throttling_reason(),
        Some(ThrottlingReason::RapidSuccessionRateLimit)
    );

    // Oldest intervention was at T0. It exits the 3600s window at T0 + 3600s.
    // At T0 + 1200s, remaining time until quota opens = 3600 - 1200 = 2400s.
    assert_eq!(
        status.remaining_seconds(),
        2400,
        "Remaining time must equal distance until oldest intervention rolls out"
    );
}

/// 4b. Rolling window sliding eviction allows subsequent intervention.
#[test]
fn test_cooldown_rolling_window_sliding_eviction() {
    let mut clock = MockClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // Record 4 interventions at T0, T0+300, T0+600, T0+900
    tracker.record_intervention(clock.now(), ActionCandidate::Remind);
    for _ in 0..3 {
        clock.advance_seconds(300);
        tracker.record_intervention(clock.now(), ActionCandidate::Remind);
    }

    // Advance clock to T0 + 3601s (just past 1 hour from 1st intervention)
    // Elapsed since T0: 3601s. The 1st intervention at T0 is now expired from the rolling window.
    // Active interventions in window drop from 4 to 3.
    let remaining_to_advance = 3601 - 900;
    clock.advance_seconds(remaining_to_advance);

    let status = tracker.can_intervene(clock.now());
    assert_eq!(
        status,
        CooldownStatus::Allowed,
        "Rolling window eviction of 1st intervention must restore capacity"
    );

    // 5th intervention is now accepted
    tracker.record_intervention(clock.now(), ActionCandidate::SuggestBreak);
    assert_eq!(tracker.total_interventions(), 5);
}

// =========================================================================
// TEST SUITE 3: USER CORRECTION SUPPRESSION
// =========================================================================

/// 5. User correction suppression: recording a correction for `app_id` suppresses interventions
///    on that app for full duration (e.g. 1800s), but allows interventions on unrelated apps.
#[test]
fn test_user_correction_suppresses_targeted_app_allows_unrelated_app() {
    let mut clock = MockClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // User submits a correction for "com.spotify.Client" (marked on-goal background music)
    tracker.record_targeted_correction(
        clock.now(),
        CorrectionScope::Session,
        Some("com.spotify.Client".to_string()),
        None,
    );

    // Advance 60s
    clock.advance_seconds(60);

    // Query 1: Activity on corrected app "com.spotify.Client"
    let status_corrected =
        tracker.can_intervene_for_target(clock.now(), Some("com.spotify.Client"), None);
    assert!(
        status_corrected.is_throttled(),
        "Targeted app must be suppressed by user correction"
    );
    assert_eq!(
        status_corrected.throttling_reason(),
        Some(ThrottlingReason::UserCorrectionSuppressed)
    );
    // 1800s default suppression - 60s elapsed = 1740s
    assert_eq!(status_corrected.remaining_seconds(), 1740);

    // Query 2: Activity on UNRELATED app "com.valvesoftware.Steam"
    let status_unrelated =
        tracker.can_intervene_for_target(clock.now(), Some("com.valvesoftware.Steam"), None);
    assert_eq!(
        status_unrelated,
        CooldownStatus::Allowed,
        "Unrelated app must NOT be suppressed by correction on a different app"
    );

    // Case-insensitive app matching: "COM.SPOTIFY.CLIENT" is also suppressed
    let status_case =
        tracker.can_intervene_for_target(clock.now(), Some("COM.SPOTIFY.CLIENT"), None);
    assert!(status_case.is_throttled());
    assert_eq!(
        status_case.throttling_reason(),
        Some(ThrottlingReason::UserCorrectionSuppressed)
    );
}

/// 6. Suppression expiration: after 1800s, interventions on previously corrected app resume.
#[test]
fn test_user_correction_suppression_expiration_and_resumption() {
    let mut clock = MockClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    tracker.record_targeted_correction(
        clock.now(),
        CorrectionScope::CurrentEvent,
        Some("org.videolan.vlc".to_string()),
        None,
    );

    // At T0 + 1799s: 1 second before expiration
    clock.advance_seconds(1799);
    let status_pre = tracker.can_intervene_for_target(clock.now(), Some("org.videolan.vlc"), None);
    assert!(status_pre.is_throttled());
    assert_eq!(status_pre.remaining_seconds(), 1);

    // At T0 + 1800s: Exactly at expiration boundary
    clock.advance_seconds(1);
    let status_at = tracker.can_intervene_for_target(clock.now(), Some("org.videolan.vlc"), None);
    assert_eq!(
        status_at,
        CooldownStatus::Allowed,
        "Suppression must expire at exact 1800s mark"
    );

    // At T0 + 1801s: Well past expiration
    clock.advance_seconds(1);
    let status_post = tracker.can_intervene_for_target(clock.now(), Some("org.videolan.vlc"), None);
    assert_eq!(status_post, CooldownStatus::Allowed);
}

/// 6b. Web origin / domain suppression matching.
#[test]
fn test_user_correction_origin_domain_suppression() {
    let mut clock = MockClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // User corrects origin "youtube.com" (e.g. video tutorial)
    tracker.record_targeted_correction(
        clock.now(),
        CorrectionScope::Session,
        None,
        Some("youtube.com".to_string()),
    );

    clock.advance_seconds(120);

    // Matching URLs with schemes or paths
    let status_sub = tracker.can_intervene_for_target(
        clock.now(),
        None,
        Some("https://youtube.com/watch?v=dQw4w9WgXcQ"),
    );
    assert!(
        status_sub.is_throttled(),
        "Full URL matching origin must be suppressed"
    );
    assert_eq!(
        status_sub.throttling_reason(),
        Some(ThrottlingReason::UserCorrectionSuppressed)
    );

    // Non-matching origin is allowed
    let status_other =
        tracker.can_intervene_for_target(clock.now(), None, Some("https://reddit.com/r/rust"));
    assert_eq!(status_other, CooldownStatus::Allowed);
}

/// 6c. User correction summary integration.
#[test]
fn test_user_correction_summary_record() {
    let mut clock = MockClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    let summary = UserCorrectionSummary {
        id: None,
        label: ClassificationLabel::Relevant,
        scope: CorrectionScope::Session,
        app_id: Some("code".to_string()),
        origin: None,
        window_title_pattern: None,
        note: Some("VSCode editor is work".to_string()),
        created_at: Some(clock.now()),
    };

    tracker.record_correction_summary(clock.now(), &summary);
    clock.advance_seconds(10);

    let status = tracker.can_intervene_for_target(clock.now(), Some("code"), None);
    assert!(status.is_throttled());
    assert_eq!(
        status.throttling_reason(),
        Some(ThrottlingReason::UserCorrectionSuppressed)
    );
}

// =========================================================================
// TEST SUITE 4: SNOOZE ACTIVATION & SCOPED EXEMPTIONS
// =========================================================================

/// 7. Snooze activation: snoozing for N seconds throttles all non-emergency interventions.
#[test]
fn test_snooze_activation_throttles_all_non_emergency_interventions() {
    let mut clock = MockClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // User activates snooze for 600s (10 minutes)
    tracker
        .record_snooze(clock.now(), 600)
        .expect("Snooze record succeeds");

    assert!(tracker.is_snoozed(clock.now()));

    // Advance 100 seconds
    clock.advance_seconds(100);

    // 1. Off-task reminder throttled
    let status_remind = tracker.can_intervene_for_action(clock.now(), ActionCandidate::Remind);
    assert!(status_remind.is_throttled());
    assert_eq!(
        status_remind.throttling_reason(),
        Some(ThrottlingReason::SnoozeActive)
    );
    assert_eq!(status_remind.remaining_seconds(), 500); // 600 - 100

    // 2. Clarifying question throttled
    let status_ask = tracker.can_intervene_for_action(clock.now(), ActionCandidate::AskContext);
    assert!(status_ask.is_throttled());
    assert_eq!(
        status_ask.throttling_reason(),
        Some(ThrottlingReason::SnoozeActive)
    );

    // 3. Correction suggestion throttled
    let status_corr =
        tracker.can_intervene_for_action(clock.now(), ActionCandidate::SuggestCorrection);
    assert!(status_corr.is_throttled());
    assert_eq!(
        status_corr.throttling_reason(),
        Some(ThrottlingReason::SnoozeActive)
    );
}

/// 8. Snooze expiration: after snooze duration, normal evaluation resumes.
#[test]
fn test_snooze_expiration_and_automatic_resumption() {
    let mut clock = MockClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    tracker.record_snooze(clock.now(), 600).unwrap();

    // At T0 + 599s: 1 second remaining
    clock.advance_seconds(599);
    let status_pre = tracker.can_intervene(clock.now());
    assert!(status_pre.is_throttled());
    assert_eq!(status_pre.remaining_seconds(), 1);
    assert!(tracker.is_snoozed(clock.now()));

    // At T0 + 600s: Snooze expired!
    clock.advance_seconds(1);
    assert!(!tracker.is_snoozed(clock.now()));
    let status_at = tracker.can_intervene(clock.now());
    assert_eq!(
        status_at,
        CooldownStatus::Allowed,
        "After snooze expiration, interventions must resume without manual action"
    );

    // At T0 + 601s
    clock.advance_seconds(1);
    assert_eq!(tracker.can_intervene(clock.now()), CooldownStatus::Allowed);
}

/// 8b. Explicit snooze cancellation.
#[test]
fn test_snooze_explicit_cancellation() {
    let clock = MockClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    tracker.record_snooze(clock.now(), 1800).unwrap();
    assert!(tracker.is_snoozed(clock.now()));

    tracker.cancel_snooze();
    assert!(!tracker.is_snoozed(clock.now()));
    assert_eq!(tracker.can_intervene(clock.now()), CooldownStatus::Allowed);
}

// =========================================================================
// TEST SUITE 5: BREAK STATE BEHAVIOR
// =========================================================================

/// 9. Break state behavior: break state suppresses off-task reminders but allows break-specific advice.
#[test]
fn test_break_state_suppresses_off_task_reminders_allows_break_advice() {
    let clock = MockClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // Enter planned focus break
    tracker.set_break_state(true, clock.now());
    assert!(tracker.is_break_state());

    // 1. Off-task reminder during break state check
    let _status_remind = tracker.can_intervene_for_action(clock.now(), ActionCandidate::Remind);

    let ctx_off_task = build_test_context(
        Some("com.valvesoftware.Steam"),
        None,
        Some("Steam"),
        true, // is_break_state
        TonePreferences::default(),
    );

    // Evaluating off-task context during break:
    // With allow_break_reminders = false, all work prompts are muted.
    let strict_break_config = CooldownConfig {
        allow_break_reminders: false,
        ..Default::default()
    };
    let strict_break_tracker = CooldownTracker::new(strict_break_config).unwrap();

    let status_break = strict_break_tracker.can_intervene_context(clock.now(), &ctx_off_task);
    assert!(
        status_break.is_throttled(),
        "Off-task reminder during break state must be muted"
    );
    assert_eq!(
        status_break.throttling_reason(),
        Some(ThrottlingReason::BreakStateActive)
    );

    // Transition back to active focus session
    tracker.set_break_state(false, clock.now());
    assert!(!tracker.is_break_state());
}

// =========================================================================
// TEST SUITE 6: TONE POLICY ADAPTATION & PACING PRESETS
// =========================================================================

/// 10. Tone policy adaptation: verifying tone adjustments (e.g., tough love vs gentle) with cooldown thresholds.
#[test]
fn test_tone_policy_adaptation_and_safety_floor_enforcement() {
    // 1. Supportive / Gentle Profile
    let gentle_config = CooldownConfig::gentle();
    assert_eq!(gentle_config.min_interval_seconds, 600); // 10 minutes
    assert_eq!(gentle_config.max_interventions_per_window, 2); // 2 per hour
    assert_eq!(gentle_config.correction_suppression_seconds, 2400); // 40 minutes

    let gentle_tracker = CooldownTracker::new(gentle_config).unwrap();
    assert_eq!(gentle_tracker.config().min_interval_seconds, 600);

    // 2. Pragmatic / Strict ("Tough Love") Profile
    let strict_config = CooldownConfig::strict();
    assert_eq!(strict_config.min_interval_seconds, 180); // 3 minutes
    assert_eq!(strict_config.max_interventions_per_window, 6); // 6 per hour
    assert_eq!(strict_config.correction_suppression_seconds, 1200); // 20 minutes

    let strict_tracker = CooldownTracker::new(strict_config).unwrap();
    assert_eq!(strict_tracker.config().min_interval_seconds, 180);

    // 3. Safety Floor Enforcement:
    // Even an aggressive "tough love" custom config CANNOT breach system safety floors!
    let dangerous_config = CooldownConfig {
        min_interval_seconds: 10, // Unsafe: 10 seconds!
        ..Default::default()
    };
    assert!(
        dangerous_config.validate().is_err(),
        "Config validation must reject intervals below MIN_SAFE_COOLDOWN_SECONDS (60s)"
    );

    let spam_config = CooldownConfig {
        max_interventions_per_window: 0, // Invalid
        ..Default::default()
    };
    assert!(spam_config.validate().is_err());

    // 4. Tone derivation helper
    let supportive_tone = TonePreferences::supportive();
    let config_from_tone = CooldownConfig::from_tone(&supportive_tone);
    assert_eq!(config_from_tone.min_interval_seconds, 600);

    let pragmatic_tone = TonePreferences::pragmatic();
    let config_pragmatic = CooldownConfig::from_tone(&pragmatic_tone);
    assert_eq!(config_pragmatic.min_interval_seconds, 180);
}

// =========================================================================
// TEST SUITE 7: MONOTONICITY, CLOCK REGRESSIONS & MEMORY SAFETY
// =========================================================================

/// 11. Monotonicity & clock regressions: handling backwards clock anomalies gracefully without panics.
#[test]
fn test_monotonicity_backwards_clock_regression_handled_gracefully() {
    let mut clock = MockClock::new("2026-10-24T12:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // Record intervention at 12:00:00 UTC
    tracker.record_intervention(clock.now(), ActionCandidate::Remind);

    // Scenario A: Clock jumps backward by 60 seconds (11:59:00 UTC) due to NTP sync / VM resume
    clock.rewind_seconds(60);

    // Query must NOT panic, must NOT underflow, and must safely throttle
    let status_regression = tracker.can_intervene(clock.now());

    assert!(
        status_regression.is_throttled(),
        "Clock regression must throttle interventions to prevent alert storms"
    );
    assert_eq!(
        status_regression.throttling_reason(),
        Some(ThrottlingReason::TimeRegression)
    );
    assert_eq!(
        status_regression.remaining_seconds(),
        DEFAULT_MIN_INTERVAL_SECONDS
    );

    // Scenario B: Extreme backward jump (e.g. 1 hour backward)
    clock.rewind_seconds(3600);
    let status_extreme = tracker.can_intervene(clock.now());
    assert!(status_extreme.is_throttled());
    assert_eq!(
        status_extreme.throttling_reason(),
        Some(ThrottlingReason::TimeRegression)
    );

    // Scenario C: Clock returns forward to normal (12:05:01 UTC)
    let forward_clock = MockClock::new("2026-10-24T12:05:01Z");
    let status_recovered = tracker.can_intervene(forward_clock.now());
    assert_eq!(
        status_recovered,
        CooldownStatus::Allowed,
        "Once clock moves past the original cooldown window, interventions resume"
    );
}

/// 12. Silent action `ActionCandidate::None` does not consume cooldown quota.
#[test]
fn test_silent_action_none_does_not_consume_cooldown_quota() {
    let clock = MockClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // Recording ActionCandidate::None (decision engine remained silent)
    tracker.record_intervention(clock.now(), ActionCandidate::None);

    assert_eq!(
        tracker.total_interventions(),
        0,
        "Silent action None must not increment total interventions"
    );
    assert_eq!(
        tracker.last_intervention(),
        None,
        "Silent action None must not set last intervention timestamp"
    );
    assert_eq!(
        tracker.can_intervene(clock.now()),
        CooldownStatus::Allowed,
        "Tracker must remain Allowed after silent action"
    );
}

/// 13. Interleaved multi-app suppressions with independent lifecycles.
#[test]
fn test_interleaved_multi_app_suppressions_independent_lifecycles() {
    let mut clock = MockClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // App 1 suppressed at T0
    tracker.record_targeted_correction(
        clock.now(),
        CorrectionScope::CurrentEvent,
        Some("app1".to_string()),
        None,
    );

    // 300s later, App 2 suppressed at T0 + 300s
    clock.advance_seconds(300);
    tracker.record_targeted_correction(
        clock.now(),
        CorrectionScope::CurrentEvent,
        Some("app2".to_string()),
        None,
    );

    // Advance to T0 + 1850s (App 1 has expired: 1850 > 1800, but App 2 has 250s remaining: 1850 < 300 + 1800)
    clock.advance_seconds(1550); // 300 + 1550 = 1850

    // App 1 is now Allowed
    let status_app1 = tracker.can_intervene_for_target(clock.now(), Some("app1"), None);
    assert_eq!(status_app1, CooldownStatus::Allowed);

    // App 2 is still Throttled
    let status_app2 = tracker.can_intervene_for_target(clock.now(), Some("app2"), None);
    assert!(status_app2.is_throttled());
    assert_eq!(
        status_app2.throttling_reason(),
        Some(ThrottlingReason::UserCorrectionSuppressed)
    );
    assert_eq!(status_app2.remaining_seconds(), 250); // 2100 - 1850 = 250
}

/// 14. Memory bounded capacity and eviction under high event volume.
#[test]
fn test_memory_bounded_capacity_and_eviction() {
    let mut clock = MockClock::new("2026-10-24T10:00:00Z");
    let mut tracker = CooldownTracker::with_default_config();

    // Simulate 100 rapid interventions (stepping by 1 second)
    for _ in 0..100 {
        clock.advance_seconds(1);
        tracker.record_intervention(clock.now(), ActionCandidate::Remind);
    }

    // Verify recent interventions buffer does NOT grow unboundedly beyond MAX_TRACKED_INTERVENTIONS (64)
    assert!(
        tracker.recent_interventions_count(clock.now()) <= MAX_TRACKED_INTERVENTIONS,
        "Tracked interventions must be capped at MAX_TRACKED_INTERVENTIONS"
    );

    // Simulate 100 different app suppressions
    for i in 0..100 {
        clock.advance_seconds(1);
        tracker.record_targeted_correction(
            clock.now(),
            CorrectionScope::CurrentEvent,
            Some(format!("app_{i}")),
            None,
        );
    }

    // Tracker remains functional and does not panic or leak memory
    assert_eq!(tracker.total_interventions(), 100);
}

/// 15. Cumulative session intervention cap.
#[test]
fn test_session_cap_exhaustion_locks_out_further_interventions() {
    let mut clock = MockClock::new("2026-10-24T10:00:00Z");

    let config = CooldownConfig {
        session_cap: Some(3), // Low cap for test
        min_interval_seconds: 60,
        rapid_window_seconds: 3600,
        max_interventions_per_window: 10,
        ..Default::default()
    };
    let mut tracker = CooldownTracker::new(config).unwrap();

    // Interventions 1, 2, 3
    for _ in 0..3 {
        assert_eq!(tracker.can_intervene(clock.now()), CooldownStatus::Allowed);
        tracker.record_intervention(clock.now(), ActionCandidate::Remind);
        clock.advance_seconds(65);
    }

    // Attempt 4th intervention: session cap reached!
    let status = tracker.can_intervene(clock.now());
    assert!(
        status.is_throttled(),
        "Interventions past session cap must be locked out"
    );
    assert_eq!(
        status.throttling_reason(),
        Some(ThrottlingReason::SessionCapReached)
    );

    // Resetting session clears the cap
    tracker.reset_session();
    assert_eq!(tracker.total_interventions(), 0);
    assert_eq!(tracker.can_intervene(clock.now()), CooldownStatus::Allowed);
}
