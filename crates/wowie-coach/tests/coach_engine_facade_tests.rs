//! Comprehensive Integration Test Suite for the `CoachEngine` Facade.
//!
//! Milestone 5: Unified CoachEngine Facade Integration Tests.
//!
//! Test Suite Structure:
//! 1. Harness & Test Fixtures (`MockClock`, context builders, engine builders)
//! 2. Happy Path End-to-End Evaluation Flows
//! 3. Deterministic Cooldown Throttling & Suppression (Zero Provider Calls)
//! 4. Fallback Resilience & Error Recovery
//! 5. User Correction & Snooze State Recording Through Facade
//! 6. Concurrency, Multi-Task Parallel Execution & Thread Safety
//! 7. 100% Offline Isolation & Determinism
//! 8. Clock Regression & Edge Case Hardening

use chrono::{DateTime, Duration, Utc};
use std::sync::Arc;
use std::time::Duration as StdDuration;
use uuid::Uuid;
use wowie_coach::engine::{CoachEngine, CoachEngineConfig};
use wowie_coach::error::CoachError;
use wowie_coach::models::action::ActionCandidate;
use wowie_coach::models::context::{CoachContext, UntrustedObservation, UserCorrectionSummary};
use wowie_coach::models::response::{CoachRawResponse, RelevanceClassification};
use wowie_coach::models::tone::TonePreferences;
use wowie_coach::policy::cooldown::CooldownConfig;
use wowie_coach::provider::{CoachProvider, MockCoachProvider};
use wowie_core::id::SessionId;
use wowie_core::models::{ClassificationLabel, CorrectionScope, IdleState};

// =========================================================================
// SECTION 0: TEST FIXTURES AND HARNESS
// =========================================================================

/// Injected mock clock for 100% deterministic, instant time stepping without wall-clock sleep.
#[derive(Debug, Clone)]
struct MockClock {
    current: DateTime<Utc>,
}

impl MockClock {
    fn new(iso_time: &str) -> Self {
        let dt = DateTime::parse_from_rfc3339(iso_time)
            .expect("Valid ISO 8601 timestamp required")
            .with_timezone(&Utc);
        Self { current: dt }
    }

    fn now(&self) -> DateTime<Utc> {
        self.current
    }

    fn advance_seconds(&mut self, seconds: u64) -> DateTime<Utc> {
        self.current += Duration::seconds(seconds as i64);
        self.current
    }

    fn advance_minutes(&mut self, minutes: u64) -> DateTime<Utc> {
        self.advance_seconds(minutes * 60)
    }

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
    idle_state: Option<IdleState>,
    tone: TonePreferences,
) -> CoachContext {
    let obs = UntrustedObservation::new(
        app_id.map(String::from),
        window_title.map(String::from),
        origin.map(String::from),
        idle_state.or(Some(IdleState::Active)),
    )
    .expect("Valid observation");

    CoachContext::new(
        "Build Rust Core Microservice",
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

fn sample_on_goal_context() -> CoachContext {
    build_test_context(
        Some("dev.alacritty"),
        None,
        Some("alacritty: cargo test -p wowie-coach"),
        false,
        Some(IdleState::Active),
        TonePreferences::default(),
    )
}

fn sample_distracted_context() -> CoachContext {
    build_test_context(
        Some("org.mozilla.firefox"),
        Some("youtube.com"),
        Some("YouTube - Top 10 Best Coding Fails"),
        false,
        Some(IdleState::Active),
        TonePreferences::default(),
    )
}

fn sample_break_context() -> CoachContext {
    build_test_context(
        Some("org.mozilla.firefox"),
        Some("youtube.com"),
        Some("YouTube - Ambient Lo-Fi Chill Beats"),
        true,
        Some(IdleState::Active),
        TonePreferences::default(),
    )
}

fn sample_idle_context() -> CoachContext {
    build_test_context(
        Some("dev.alacritty"),
        None,
        Some("alacritty"),
        false,
        Some(IdleState::Idle),
        TonePreferences::default(),
    )
}

fn engine_with_mock(mock: MockCoachProvider) -> (CoachEngine, Arc<MockCoachProvider>) {
    let mock_arc = Arc::new(mock);
    let engine = CoachEngine::with_default_config(Arc::clone(&mock_arc) as Arc<dyn CoachProvider>);
    (engine, mock_arc)
}

fn engine_with_config(
    mock: MockCoachProvider,
    config: CoachEngineConfig,
) -> (CoachEngine, Arc<MockCoachProvider>) {
    let mock_arc = Arc::new(mock);
    let engine = CoachEngine::new(Arc::clone(&mock_arc) as Arc<dyn CoachProvider>, config)
        .expect("Valid engine config");
    (engine, mock_arc)
}

// =========================================================================
// SECTION 1: HAPPY PATH END-TO-END EVALUATION FLOWS
// =========================================================================

#[tokio::test]
async fn test_facade_happy_path_relevant_focus() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, mock) = engine_with_mock(MockCoachProvider::canned_relevant());
    let ctx = sample_on_goal_context();

    let response = engine.evaluate(&ctx, clock.now()).await;

    assert!(response.is_relevant());
    assert!(!response.has_action());
    assert_eq!(response.effective_action(), ActionCandidate::None);
    assert_eq!(response.provenance.provider_name, "mock_coach_provider");
    assert_eq!(mock.call_count(), 1);
    assert_eq!(engine.total_interventions(), 0);
}

#[tokio::test]
async fn test_facade_happy_path_distracted_remind() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, mock) = engine_with_mock(MockCoachProvider::canned_distracted(
        ActionCandidate::Remind,
    ));
    let ctx = sample_distracted_context();

    let response = engine.evaluate(&ctx, clock.now()).await;

    assert!(response.is_possibly_unrelated());
    assert!(response.has_action());
    assert_eq!(response.effective_action(), ActionCandidate::Remind);
    assert!(response.question.is_some());
    assert_eq!(mock.call_count(), 1);
    assert_eq!(engine.total_interventions(), 1);
}

#[tokio::test]
async fn test_facade_happy_path_ambiguous_ask_context() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, mock) = engine_with_mock(MockCoachProvider::canned_distracted(
        ActionCandidate::AskContext,
    ));
    let ctx = sample_distracted_context();

    let response = engine.evaluate(&ctx, clock.now()).await;

    assert_eq!(response.effective_action(), ActionCandidate::AskContext);
    assert_eq!(mock.call_count(), 1);
    assert_eq!(engine.total_interventions(), 1);
}

#[tokio::test]
async fn test_facade_happy_path_fatigue_suggest_break() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, mock) = engine_with_mock(MockCoachProvider::canned_distracted(
        ActionCandidate::SuggestBreak,
    ));
    let ctx = sample_distracted_context();

    let response = engine.evaluate(&ctx, clock.now()).await;

    assert_eq!(response.effective_action(), ActionCandidate::SuggestBreak);
    assert_eq!(mock.call_count(), 1);
    assert_eq!(engine.total_interventions(), 1);
}

#[tokio::test]
async fn test_facade_happy_path_fifo_progression() {
    let mut clock = MockClock::new("2026-10-25T10:00:00Z");
    let mock = MockCoachProvider::empty_queue();

    // Turn 1: Relevant (None)
    mock.push_response(CoachRawResponse {
        relevance: RelevanceClassification::Relevant,
        confidence: 0.95,
        uncertainty: Some(0.05),
        rationale: "Active coding in IDE".to_string(),
        action_candidate: Some(ActionCandidate::None),
        ..Default::default()
    });
    // Turn 2: Ambiguous (AskContext)
    mock.push_response(CoachRawResponse {
        relevance: RelevanceClassification::Unknown,
        confidence: 0.50,
        uncertainty: Some(0.50),
        rationale: "Browsing technical docs".to_string(),
        question: Some("Is this related to your goal?".to_string()),
        action_candidate: Some(ActionCandidate::AskContext),
        ..Default::default()
    });
    // Turn 3: Distracted (Remind)
    mock.push_response(CoachRawResponse {
        relevance: RelevanceClassification::PossiblyUnrelated,
        confidence: 0.88,
        uncertainty: Some(0.12),
        rationale: "Social media detected".to_string(),
        action_candidate: Some(ActionCandidate::Remind),
        ..Default::default()
    });
    // Turn 4: Fatigue (SuggestBreak)
    mock.push_response(CoachRawResponse {
        relevance: RelevanceClassification::PossiblyUnrelated,
        confidence: 0.82,
        uncertainty: Some(0.18),
        rationale: "Focus block duration reached".to_string(),
        action_candidate: Some(ActionCandidate::SuggestBreak),
        ..Default::default()
    });

    let (engine, mock_arc) = engine_with_mock(mock);
    let ctx = sample_on_goal_context();

    // Turn 1
    let r1 = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(r1.relevance, RelevanceClassification::Relevant);
    assert_eq!(r1.effective_action(), ActionCandidate::None);
    assert_eq!(engine.total_interventions(), 0);

    // Turn 2 (+10 min)
    clock.advance_minutes(10);
    let r2 = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(r2.effective_action(), ActionCandidate::AskContext);
    assert_eq!(engine.total_interventions(), 1);

    // Turn 3 (+10 min)
    clock.advance_minutes(10);
    let r3 = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(r3.effective_action(), ActionCandidate::Remind);
    assert_eq!(engine.total_interventions(), 2);

    // Turn 4 (+10 min)
    clock.advance_minutes(10);
    let r4 = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(r4.effective_action(), ActionCandidate::SuggestBreak);
    assert_eq!(engine.total_interventions(), 3);

    assert_eq!(mock_arc.call_count(), 4);
}

#[tokio::test]
async fn test_facade_happy_path_prompt_delimiter_isolation() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, mock) = engine_with_mock(MockCoachProvider::canned_relevant());
    let ctx = build_test_context(
        Some("dev.alacritty"),
        None,
        Some("Title with <script>malicious()</script> and & entities"),
        false,
        Some(IdleState::Active),
        TonePreferences::default(),
    );

    let _ = engine.evaluate(&ctx, clock.now()).await;

    let received = mock.last_prompt().expect("Prompt received");
    assert!(received
        .user_prompt
        .contains("<untrusted_activity_evidence>"));
    assert!(received
        .user_prompt
        .contains("</untrusted_activity_evidence>"));
    assert!(received.user_prompt.contains("&lt;script&gt;"));
}

#[tokio::test]
async fn test_facade_happy_path_telemetry_passthrough() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let mock = MockCoachProvider::empty_queue();
    mock.push_response(CoachRawResponse {
        relevance: RelevanceClassification::Relevant,
        confidence: 0.95,
        uncertainty: Some(0.05),
        rationale: "Active coding in IDE".to_string(),
        action_candidate: Some(ActionCandidate::None),
        token_usage: Some(wowie_coach::TokenUsage::new(100, 50, 150)),
        latency_ms: Some(42),
        model: Some("mock-gpt-4o".to_string()),
        ..Default::default()
    });
    let (engine, _) = engine_with_mock(mock);
    let ctx = sample_on_goal_context();

    let resp = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(resp.provenance.provider_name, "mock_coach_provider");
    assert_eq!(resp.provenance.model_name, "mock-gpt-4o");
    assert_eq!(resp.provenance.latency_ms, Some(42));
    assert_eq!(resp.provenance.generated_at, clock.now());
}

// =========================================================================
// SECTION 2: DETERMINISTIC COOLDOWN THROTTLING & SUPPRESSION
// =========================================================================

#[tokio::test]
async fn test_facade_cooldown_rapid_throttling_zero_provider_calls() {
    let mut clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, mock) = engine_with_mock(MockCoachProvider::canned_distracted(
        ActionCandidate::Remind,
    ));
    let ctx = sample_distracted_context();

    // Call 1 at t0: succeeds, intervention recorded
    let r1 = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(r1.effective_action(), ActionCandidate::Remind);
    assert_eq!(mock.call_count(), 1);
    assert_eq!(engine.total_interventions(), 1);

    // Call 2 at t0 + 10s: within 300s cooldown interval!
    clock.advance_seconds(10);
    let r2 = engine.evaluate(&ctx, clock.now()).await;

    // Must be throttled with ActionCandidate::None
    assert_eq!(r2.effective_action(), ActionCandidate::None);
    assert_eq!(r2.relevance, RelevanceClassification::Unknown);
    assert!(r2.rationale.contains("throttled by cooldown policy"));
    assert_eq!(r2.provenance.provider_name, "cooldown_policy");

    // CRITICAL INVARIANT: Provider call count must NOT increment!
    assert_eq!(
        mock.call_count(),
        1,
        "Throttled call must not invoke the LLM provider"
    );
    assert_eq!(engine.total_interventions(), 1);
}

#[tokio::test]
async fn test_facade_cooldown_recovery_after_interval() {
    let mut clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, mock) = engine_with_mock(MockCoachProvider::canned_distracted(
        ActionCandidate::Remind,
    ));
    let ctx = sample_distracted_context();

    // Call 1 at t0
    let _ = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(mock.call_count(), 1);

    // Call 2 at t0 + 100s (throttled)
    clock.advance_seconds(100);
    let r2 = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(r2.effective_action(), ActionCandidate::None);
    assert_eq!(mock.call_count(), 1);

    // Call 3 at t0 + 301s (past 300s default interval): permitted!
    clock.advance_seconds(201);
    let r3 = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(r3.effective_action(), ActionCandidate::Remind);
    assert_eq!(mock.call_count(), 2);
    assert_eq!(engine.total_interventions(), 2);
}

#[tokio::test]
async fn test_facade_cooldown_rolling_window_rate_limit() {
    let mut clock = MockClock::new("2026-10-25T10:00:00Z");
    let cooldown_cfg = CooldownConfig {
        min_interval_seconds: 60,
        max_interventions_per_window: 2,
        rapid_window_seconds: 3600,
        ..Default::default()
    };

    let config = CoachEngineConfig::default().with_cooldown(cooldown_cfg);
    let (engine, mock) = engine_with_config(
        MockCoachProvider::canned_distracted(ActionCandidate::Remind),
        config,
    );
    let ctx = sample_distracted_context();

    // Intervention 1 at t0
    let r1 = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(r1.effective_action(), ActionCandidate::Remind);
    assert_eq!(mock.call_count(), 1);

    // Intervention 2 at t0 + 65s (interval respected)
    clock.advance_seconds(65);
    let r2 = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(r2.effective_action(), ActionCandidate::Remind);
    assert_eq!(mock.call_count(), 2);

    // Intervention 3 at t0 + 130s: interval respected but window cap (2) exceeded!
    clock.advance_seconds(65);
    let r3 = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(r3.effective_action(), ActionCandidate::None);
    assert_eq!(mock.call_count(), 2);

    // Advance past rolling window (3600s after t0)
    clock.advance_seconds(3600);
    let r4 = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(r4.effective_action(), ActionCandidate::Remind);
    assert_eq!(mock.call_count(), 3);
}

#[tokio::test]
async fn test_facade_cooldown_session_cap_enforced() {
    let mut clock = MockClock::new("2026-10-25T10:00:00Z");
    let cooldown_cfg = CooldownConfig {
        min_interval_seconds: 60,
        session_cap: Some(2),
        ..Default::default()
    };

    let config = CoachEngineConfig::default().with_cooldown(cooldown_cfg);
    let (engine, mock) = engine_with_config(
        MockCoachProvider::canned_distracted(ActionCandidate::Remind),
        config,
    );
    let ctx = sample_distracted_context();

    // Call 1 (allowed)
    let _ = engine.evaluate(&ctx, clock.now()).await;
    // Call 2 at +10 min (allowed)
    clock.advance_minutes(10);
    let _ = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(mock.call_count(), 2);
    assert_eq!(engine.total_interventions(), 2);

    // Call 3 at +1 hour (session cap reached!)
    clock.advance_minutes(60);
    let r3 = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(r3.effective_action(), ActionCandidate::None);
    assert!(r3.rationale.contains("session_cap_reached"));
    assert_eq!(mock.call_count(), 2);
}

#[tokio::test]
async fn test_facade_cooldown_break_state_suppression() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let cooldown_cfg = CooldownConfig {
        allow_break_reminders: false,
        ..Default::default()
    };

    let config = CoachEngineConfig::default().with_cooldown(cooldown_cfg);
    let (engine, mock) = engine_with_config(
        MockCoachProvider::canned_distracted(ActionCandidate::Remind),
        config,
    );
    let ctx = sample_break_context();

    let resp = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(resp.effective_action(), ActionCandidate::None);
    assert_eq!(mock.call_count(), 0);
}

#[tokio::test]
async fn test_facade_cooldown_idle_state_suppression() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, mock) = engine_with_mock(MockCoachProvider::canned_distracted(
        ActionCandidate::Remind,
    ));
    let ctx = sample_idle_context();

    let resp = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(resp.effective_action(), ActionCandidate::None);
    assert_eq!(mock.call_count(), 0);
}

#[tokio::test]
async fn test_facade_cooldown_non_action_no_rate_limit_consumption() {
    let mut clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, mock) = engine_with_mock(MockCoachProvider::canned_relevant());
    let ctx = sample_on_goal_context();

    for _ in 0..5 {
        clock.advance_seconds(5);
        let resp = engine.evaluate(&ctx, clock.now()).await;
        assert_eq!(resp.effective_action(), ActionCandidate::None);
    }

    assert_eq!(mock.call_count(), 5);
    assert_eq!(engine.total_interventions(), 0);
}

#[tokio::test]
async fn test_facade_cooldown_snapshot_inspection() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, _) = engine_with_mock(MockCoachProvider::canned_distracted(
        ActionCandidate::Remind,
    ));
    let ctx = sample_distracted_context();

    let _ = engine.evaluate(&ctx, clock.now()).await;

    let snapshot = engine.cooldown_snapshot();
    assert_eq!(snapshot.total_interventions(), 1);
    assert_eq!(engine.total_interventions(), 1);
    assert!(engine.can_intervene(clock.now()).is_throttled());
    assert!(engine
        .can_intervene_context(clock.now(), &ctx)
        .is_throttled());
    assert!(engine.cooldown_status(clock.now()).is_throttled());
}

// =========================================================================
// SECTION 3: FALLBACK RESILIENCE & ERROR RECOVERY
// =========================================================================

#[tokio::test]
async fn test_facade_fallback_on_provider_timeout() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, _) = engine_with_mock(MockCoachProvider::canned_error(CoachError::timeout(3500)));
    let ctx = sample_on_goal_context();

    let resp = engine.evaluate(&ctx, clock.now()).await;

    assert_eq!(resp.relevance, RelevanceClassification::Unknown);
    assert_eq!(resp.confidence, 0.0);
    assert_eq!(resp.uncertainty, Some(1.0));
    assert_eq!(resp.effective_action(), ActionCandidate::None);
    assert!(resp.rationale.contains("timed out after 3500ms"));
    assert_eq!(resp.provenance.provider_name, "mock_coach_provider");
}

#[tokio::test]
async fn test_facade_fallback_on_malformed_json_truncated() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, _) = engine_with_mock(MockCoachProvider::canned_error(
        CoachError::malformed_json("unexpected EOF", Some(1), Some(10)),
    ));
    let ctx = sample_on_goal_context();

    let resp = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(resp.relevance, RelevanceClassification::Unknown);
    assert_eq!(resp.effective_action(), ActionCandidate::None);
    assert!(resp.rationale.contains("unparseable response payload"));
}

#[tokio::test]
async fn test_facade_fallback_on_html_502_error_page() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, _) = engine_with_mock(MockCoachProvider::canned_error(CoachError::http(
        502,
        "<html><body>502 Bad Gateway</body></html>",
    )));
    let ctx = sample_on_goal_context();

    let resp = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(resp.effective_action(), ActionCandidate::None);
    assert!(resp.rationale.contains("HTTP status 502"));
    assert!(!resp.rationale.contains("<html>"));
}

#[tokio::test]
async fn test_facade_fallback_on_schema_violation_confidence() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, _) = engine_with_mock(MockCoachProvider::canned_error(
        CoachError::schema_validation("confidence", "Confidence 1.8 exceeds 1.0"),
    ));
    let ctx = sample_on_goal_context();

    let resp = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(resp.relevance, RelevanceClassification::Unknown);
    assert_eq!(resp.effective_action(), ActionCandidate::None);
    assert!(resp
        .rationale
        .contains("schema validation for field 'confidence'"));
}

#[tokio::test]
async fn test_facade_fallback_on_disallowed_action_injection() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, _) = engine_with_mock(MockCoachProvider::canned_error(
        CoachError::disallowed_action("rm -rf / --no-preserve-root"),
    ));
    let ctx = sample_on_goal_context();

    let resp = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(resp.effective_action(), ActionCandidate::None);
    assert!(!resp.rationale.contains("rm -rf"));
    assert!(resp.rationale.contains("unauthorized action"));
}

#[tokio::test]
async fn test_facade_fallback_on_network_dns_failure_ip_sanitized() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, _) = engine_with_mock(MockCoachProvider::canned_error(CoachError::network(
        "DNS failed for internal-llm-node.corp.internal (10.10.2.14: Connection refused)",
    )));
    let ctx = sample_on_goal_context();

    let resp = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(resp.effective_action(), ActionCandidate::None);
    assert!(!resp.rationale.contains("10.10.2.14"));
    assert!(!resp.rationale.contains("internal-llm-node"));
}

#[tokio::test]
async fn test_facade_fallback_on_empty_queue_exhaustion() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let mock = MockCoachProvider::empty_queue();
    let (engine, _) = engine_with_mock(mock);
    let ctx = sample_on_goal_context();

    let resp = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(resp.relevance, RelevanceClassification::Unknown);
    assert_eq!(resp.effective_action(), ActionCandidate::None);
    assert!(resp.rationale.contains("AI provider"));
}

#[tokio::test]
async fn test_facade_evaluate_without_fallback_propagates_error() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, _) = engine_with_mock(MockCoachProvider::canned_error(CoachError::timeout(2500)));
    let ctx = sample_on_goal_context();

    let res = engine.evaluate_without_fallback(&ctx, clock.now()).await;
    assert!(res.is_err());
    assert!(matches!(
        res.unwrap_err(),
        CoachError::Timeout { duration_ms: 2500 }
    ));
}

#[tokio::test]
async fn test_facade_evaluate_strict_matches_evaluate_without_fallback() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, _) = engine_with_mock(MockCoachProvider::canned_error(CoachError::timeout(1234)));
    let ctx = sample_on_goal_context();

    let res = engine.evaluate_strict(&ctx, clock.now()).await;
    assert!(res.is_err());
    assert!(matches!(
        res.unwrap_err(),
        CoachError::Timeout { duration_ms: 1234 }
    ));
}

// =========================================================================
// SECTION 4: USER CORRECTION AND SNOOZE RECORDING THROUGH FACADE
// =========================================================================

#[tokio::test]
async fn test_facade_record_targeted_correction_app_id() {
    let mut clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, mock) = engine_with_mock(MockCoachProvider::canned_distracted(
        ActionCandidate::Remind,
    ));

    // Record user correction suppressing "com.stremio.app"
    engine.record_targeted_correction(
        clock.now(),
        CorrectionScope::Session,
        Some("com.stremio.app".to_string()),
        None,
    );

    // Context with suppressed app: throttled!
    clock.advance_seconds(60);
    let suppressed_ctx = build_test_context(
        Some("com.stremio.app"),
        None,
        Some("Stremio"),
        false,
        Some(IdleState::Active),
        TonePreferences::default(),
    );
    let r1 = engine.evaluate(&suppressed_ctx, clock.now()).await;
    assert_eq!(r1.effective_action(), ActionCandidate::None);
    assert_eq!(mock.call_count(), 0);

    // Context with non-suppressed app: allowed!
    let allowed_ctx = build_test_context(
        Some("dev.alacritty"),
        None,
        Some("Terminal"),
        false,
        Some(IdleState::Active),
        TonePreferences::default(),
    );
    let r2 = engine.evaluate(&allowed_ctx, clock.now()).await;
    assert_eq!(r2.effective_action(), ActionCandidate::Remind);
    assert_eq!(mock.call_count(), 1);

    // Advance past correction suppression (1800s default)
    clock.advance_seconds(1801);
    let r3 = engine.evaluate(&suppressed_ctx, clock.now()).await;
    assert_eq!(r3.effective_action(), ActionCandidate::Remind);
    assert_eq!(mock.call_count(), 2);
}

#[tokio::test]
async fn test_facade_record_targeted_correction_origin() {
    let mut clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, mock) = engine_with_mock(MockCoachProvider::canned_distracted(
        ActionCandidate::Remind,
    ));

    engine.record_targeted_correction(
        clock.now(),
        CorrectionScope::PersistentRule,
        None,
        Some("docs.rs".to_string()),
    );

    clock.advance_seconds(30);
    let ctx_docs = build_test_context(
        Some("org.mozilla.firefox"),
        Some("docs.rs"),
        Some("Tokio Documentation"),
        false,
        Some(IdleState::Active),
        TonePreferences::default(),
    );
    let resp = engine.evaluate(&ctx_docs, clock.now()).await;
    assert_eq!(resp.effective_action(), ActionCandidate::None);
    assert_eq!(mock.call_count(), 0);
}

#[tokio::test]
async fn test_facade_record_correction_summary_struct() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, mock) = engine_with_mock(MockCoachProvider::canned_distracted(
        ActionCandidate::Remind,
    ));

    let summary = UserCorrectionSummary::new(
        ClassificationLabel::Relevant,
        CorrectionScope::Session,
        Some("org.videolan.vlc".to_string()),
        None,
        Some("Tutorial video".to_string()),
    )
    .expect("Valid correction summary");
    engine.record_correction_summary(clock.now(), &summary);

    let ctx = build_test_context(
        Some("org.videolan.vlc"),
        None,
        Some("VLC"),
        false,
        Some(IdleState::Active),
        TonePreferences::default(),
    );
    let resp = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(resp.effective_action(), ActionCandidate::None);
    assert_eq!(mock.call_count(), 0);
}

#[tokio::test]
async fn test_facade_sync_corrections_batch() {
    let mut clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, mock) = engine_with_mock(MockCoachProvider::canned_distracted(
        ActionCandidate::Remind,
    ));

    let batch = vec![
        UserCorrectionSummary::new(
            ClassificationLabel::Relevant,
            CorrectionScope::Session,
            Some("app.batch.one".to_string()),
            None,
            None,
        )
        .expect("Valid summary 1"),
        UserCorrectionSummary::new(
            ClassificationLabel::Relevant,
            CorrectionScope::Session,
            None,
            Some("batch-docs.com".to_string()),
            None,
        )
        .expect("Valid summary 2"),
    ];
    engine.sync_corrections(clock.now(), &batch);

    clock.advance_seconds(10);
    let ctx_app = build_test_context(
        Some("app.batch.one"),
        None,
        Some("Batch App"),
        false,
        Some(IdleState::Active),
        TonePreferences::default(),
    );
    let r1 = engine.evaluate(&ctx_app, clock.now()).await;
    assert_eq!(r1.effective_action(), ActionCandidate::None);

    let ctx_origin = build_test_context(
        Some("browser"),
        Some("batch-docs.com"),
        Some("Batch Docs"),
        false,
        Some(IdleState::Active),
        TonePreferences::default(),
    );
    let r2 = engine.evaluate(&ctx_origin, clock.now()).await;
    assert_eq!(r2.effective_action(), ActionCandidate::None);

    assert_eq!(mock.call_count(), 0);
}

#[tokio::test]
async fn test_facade_record_snooze_and_expiration() {
    let mut clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, mock) = engine_with_mock(MockCoachProvider::canned_distracted(
        ActionCandidate::Remind,
    ));
    let ctx = sample_distracted_context();

    // Snooze for 600 seconds
    engine
        .record_snooze(clock.now(), 600)
        .expect("Valid snooze");
    assert!(engine.is_snoozed(clock.now()));

    // During snooze (at +300s): throttled!
    clock.advance_seconds(300);
    assert_eq!(engine.remaining_snooze_seconds(clock.now()), 300);
    let r1 = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(r1.effective_action(), ActionCandidate::None);
    assert_eq!(mock.call_count(), 0);

    // After snooze (at +601s): permitted!
    clock.advance_seconds(301);
    assert!(!engine.is_snoozed(clock.now()));
    let r2 = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(r2.effective_action(), ActionCandidate::Remind);
    assert_eq!(mock.call_count(), 1);
}

#[tokio::test]
async fn test_facade_cancel_snooze_immediate_restoration() {
    let mut clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, mock) = engine_with_mock(MockCoachProvider::canned_distracted(
        ActionCandidate::Remind,
    ));
    let ctx = sample_distracted_context();

    engine.record_snooze(clock.now(), 600).unwrap();
    clock.advance_seconds(30);
    assert!(engine.is_snoozed(clock.now()));

    // Cancel snooze
    engine.cancel_snooze();
    assert!(!engine.is_snoozed(clock.now()));

    // Immediately permitted
    let resp = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(resp.effective_action(), ActionCandidate::Remind);
    assert_eq!(mock.call_count(), 1);
}

#[tokio::test]
async fn test_facade_reset_session_restores_clean_slate() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, _mock) = engine_with_mock(MockCoachProvider::canned_distracted(
        ActionCandidate::Remind,
    ));
    let ctx = sample_distracted_context();

    // Populate state
    let _ = engine.evaluate(&ctx, clock.now()).await;
    engine.record_snooze(clock.now(), 600).unwrap();
    engine.record_targeted_correction(
        clock.now(),
        CorrectionScope::Session,
        Some("test".into()),
        None,
    );

    assert_eq!(engine.total_interventions(), 1);
    assert!(engine.is_snoozed(clock.now()));

    // Reset session
    engine.reset_session();

    assert_eq!(engine.total_interventions(), 0);
    assert!(!engine.is_snoozed(clock.now()));
    assert!(engine.can_intervene(clock.now()).is_allowed());
}

// =========================================================================
// SECTION 5: CONCURRENCY, MULTI-TASK PARALLEL EXECUTION & THREAD SAFETY
// =========================================================================

#[test]
fn test_facade_send_sync_thread_safety_compile_time() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    assert_send::<CoachEngine>();
    assert_sync::<CoachEngine>();
    assert_send::<CoachEngineConfig>();
    assert_sync::<CoachEngineConfig>();
}

#[tokio::test]
async fn test_facade_concurrency_multi_task_parallel() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, mock) = engine_with_mock(MockCoachProvider::canned_relevant());
    let engine_arc = Arc::new(engine);

    let mut handles = Vec::new();
    for i in 0..20 {
        let eng = Arc::clone(&engine_arc);
        let now = clock.now();
        let handle = tokio::spawn(async move {
            let ctx = build_test_context(
                Some(&format!("app_{i}")),
                None,
                Some(&format!("window_{i}")),
                false,
                Some(IdleState::Active),
                TonePreferences::default(),
            );
            eng.evaluate(&ctx, now).await
        });
        handles.push(handle);
    }

    for handle in handles {
        let resp = handle.await.expect("Task must join without panic");
        assert!(resp.is_relevant());
    }

    assert_eq!(mock.call_count(), 20);
}

#[tokio::test]
async fn test_facade_concurrency_interleaved_readers_writers() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, _) = engine_with_mock(MockCoachProvider::canned_relevant());
    let engine_arc = Arc::new(engine);

    let mut handles = Vec::new();

    // 10 readers
    for _ in 0..10 {
        let eng = Arc::clone(&engine_arc);
        let now = clock.now();
        handles.push(tokio::spawn(async move {
            let ctx = sample_on_goal_context();
            eng.evaluate(&ctx, now).await;
        }));
    }

    // 5 writers
    for i in 0..5 {
        let eng = Arc::clone(&engine_arc);
        let now = clock.now();
        handles.push(tokio::spawn(async move {
            eng.record_targeted_correction(
                now,
                CorrectionScope::Session,
                Some(format!("app_{i}")),
                None,
            );
            let _ = eng.record_snooze(now, 60);
        }));
    }

    for handle in handles {
        assert!(handle.await.is_ok());
    }
}

#[tokio::test]
async fn test_facade_concurrency_lock_dropped_across_provider_delay() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let mock = MockCoachProvider::canned_relevant().with_delay(StdDuration::from_millis(50));
    let (engine, _) = engine_with_mock(mock);
    let engine_arc = Arc::new(engine);

    let start = std::time::Instant::now();
    let mut handles = Vec::new();

    for _ in 0..5 {
        let eng = Arc::clone(&engine_arc);
        let now = clock.now();
        handles.push(tokio::spawn(async move {
            let ctx = sample_on_goal_context();
            eng.evaluate(&ctx, now).await
        }));
    }

    for handle in handles {
        assert!(handle.await.is_ok());
    }

    let elapsed = start.elapsed();
    // 5 tasks with 50ms delay executing concurrently should take < 200ms, not 250ms serialized
    assert!(
        elapsed < StdDuration::from_millis(200),
        "Tasks took {elapsed:?}, expected concurrent execution under 200ms"
    );
}

#[tokio::test]
async fn test_facade_clone_shares_identical_cooldown_state() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine1, _) = engine_with_mock(MockCoachProvider::canned_distracted(
        ActionCandidate::Remind,
    ));
    let engine2 = engine1.clone();

    // Mutate state through engine1
    engine1.record_snooze(clock.now(), 600).unwrap();

    // Check shared state through engine2
    assert!(engine2.is_snoozed(clock.now()));
    assert_eq!(engine2.remaining_snooze_seconds(clock.now()), 600);

    // Cancel through engine2
    engine2.cancel_snooze();
    assert!(!engine1.is_snoozed(clock.now()));
}

// =========================================================================
// SECTION 6: 100% OFFLINE ISOLATION & DETERMINISM
// =========================================================================

#[tokio::test]
async fn test_facade_offline_isolation_zero_sockets() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, mock) = engine_with_mock(MockCoachProvider::canned_relevant());
    let ctx = sample_on_goal_context();

    let resp = engine.evaluate(&ctx, clock.now()).await;
    assert!(resp.is_relevant());
    assert_eq!(mock.call_count(), 1);
}

#[tokio::test]
async fn test_facade_offline_determinism_identical_runs() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let ctx = sample_distracted_context();

    let (engine1, _) = engine_with_mock(MockCoachProvider::canned_distracted(
        ActionCandidate::Remind,
    ));
    let (engine2, _) = engine_with_mock(MockCoachProvider::canned_distracted(
        ActionCandidate::Remind,
    ));

    let r1 = engine1.evaluate(&ctx, clock.now()).await;
    let r2 = engine2.evaluate(&ctx, clock.now()).await;

    assert_eq!(r1.relevance, r2.relevance);
    assert_eq!(r1.confidence, r2.confidence);
    assert_eq!(r1.uncertainty, r2.uncertainty);
    assert_eq!(r1.effective_action(), r2.effective_action());
    assert_eq!(r1.rationale, r2.rationale);
    assert_eq!(r1.question, r2.question);
}

// =========================================================================
// SECTION 7: CLOCK REGRESSION & EDGE CASE HARDENING
// =========================================================================

#[tokio::test]
async fn test_facade_clock_regression_detection() {
    let mut clock = MockClock::new("2026-10-25T12:00:00Z");
    let (engine, mock) = engine_with_mock(MockCoachProvider::canned_distracted(
        ActionCandidate::Remind,
    ));
    let ctx = sample_distracted_context();

    // First call at 12:00
    let _ = engine.evaluate(&ctx, clock.now()).await;
    assert_eq!(mock.call_count(), 1);

    // Backward jump to 11:30 (clock regression)
    clock.rewind_seconds(1800);
    let r_regress = engine.evaluate(&ctx, clock.now()).await;

    // Must be safely throttled without underflow or panic
    assert_eq!(r_regress.effective_action(), ActionCandidate::None);
    assert!(r_regress.rationale.contains("time_regression"));
    assert_eq!(mock.call_count(), 1);
}

#[tokio::test]
async fn test_facade_extreme_context_boundaries() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, _) = engine_with_mock(MockCoachProvider::canned_relevant());

    let long_title = "a".repeat(200);
    let long_window = "w".repeat(256);
    let obs = UntrustedObservation::new(
        Some("com.extreme.app".to_string()),
        Some(long_window),
        None,
        Some(IdleState::Active),
    )
    .unwrap();

    let ctx = CoachContext::new(
        long_title,
        Some("Next action".to_string()),
        SessionId::from_uuid(Uuid::nil()),
        43200, // Max session duration
        false,
        obs,
        vec![],
        TonePreferences::default(),
    )
    .unwrap();

    let resp = engine.evaluate(&ctx, clock.now()).await;
    assert!(resp.is_relevant());
}

#[tokio::test]
async fn test_facade_tone_preferences_propagation() {
    let clock = MockClock::new("2026-10-25T10:00:00Z");
    let (engine, mock) = engine_with_mock(MockCoachProvider::canned_relevant());

    let tone = TonePreferences::pragmatic();
    let ctx = build_test_context(
        Some("dev.alacritty"),
        None,
        Some("Terminal"),
        false,
        Some(IdleState::Active),
        tone,
    );

    let _ = engine.evaluate(&ctx, clock.now()).await;
    let prompt = mock.last_prompt().unwrap();
    assert!(prompt.system_prompt.contains("direct"));
}

#[test]
fn test_facade_config_validation_and_presets() {
    let default_cfg = CoachEngineConfig::default();
    assert!(default_cfg.validate().is_ok());

    let gentle_cfg = CoachEngineConfig::gentle();
    assert!(gentle_cfg.validate().is_ok());
    assert_eq!(gentle_cfg.cooldown.min_interval_seconds, 600);

    let strict_cfg = CoachEngineConfig::strict();
    assert!(strict_cfg.validate().is_ok());
    assert_eq!(strict_cfg.cooldown.min_interval_seconds, 180);

    let invalid_cooldown = CooldownConfig {
        min_interval_seconds: 10, // Below MIN_SAFE_COOLDOWN_SECONDS (60)
        ..Default::default()
    };
    let invalid_cfg = CoachEngineConfig::default().with_cooldown(invalid_cooldown);
    assert!(invalid_cfg.validate().is_err());
}

#[test]
fn test_facade_builder_customization() {
    let mock = Arc::new(MockCoachProvider::canned_relevant());
    let engine = CoachEngine::builder()
        .with_provider(mock)
        .with_cooldown(CooldownConfig::gentle())
        .with_fallback_on_error(true)
        .with_system_preamble("Custom preamble")
        .build()
        .expect("Builder should succeed");

    assert_eq!(engine.config().cooldown.min_interval_seconds, 600);
    assert_eq!(
        engine.config().system_preamble.as_deref(),
        Some("Custom preamble")
    );
}

#[test]
fn test_facade_builder_missing_provider_errors() {
    let result = CoachEngine::builder().build();
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        CoachError::ProviderUnavailable { .. }
    ));
}
