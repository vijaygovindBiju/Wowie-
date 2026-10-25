//! Comprehensive integration and contract test suite for Milestone 4:
//! Pluggable LLM Provider Abstraction & Fallback Engine.
//!
//! Covers:
//! 1. Normal canned flows (Relevant, Distracted, AskContext, SuggestBreak, SuggestCorrection)
//! 2. FIFO multi-turn sequences (trajectories, queue exhaustion policies, interleaved results)
//! 3. Error injection & transient failure simulations (retries, timeouts)
//! 4. Malformed JSON recovery (truncated tokens, HTML error pages, plain text refusals)
//! 5. Disallowed action candidate recovery (shell attacks, OS tampering, casing variations)
//! 6. Error sanitization & FallbackReason categorization (zero IP or credential leaks)
//! 7. Heuristic pattern matching mode (break, distraction, research, default)
//! 8. Call history inspection and state reset
//! 9. Dynamic dispatch via `Arc<dyn CoachProvider>` (object safety)
//! 10. `OpenAiCompatibleClient` offline unit tests (config, payload construction, response parsing, error mapping, credential redaction)
//! 11. In-process loopback mock HTTP server test (100% offline network stack validation)
//! 12. 100% offline determinism stress test

use chrono::{DateTime, Utc};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use wowie_coach::{
    ActionCandidate, CoachError, CoachPrompt, CoachProvider, CoachRawResponse, CoachResponse,
    CustomEvaluator, DeterministicFallback, FallbackReason, MockBehavior, MockCoachProvider,
    OpenAiCompatibleClient, OpenAiConfig, ProviderProvenance, QueueEmptyBehavior,
    RelevanceClassification, ResponseFormatMode, TokenUsage,
};

fn sample_prompt() -> CoachPrompt {
    CoachPrompt::new(
        "You are an AI focus coach for Wowie.",
        "Goal: Build Rust microservice.\nObserved activity: cargo build in terminal.",
    )
}

fn sample_prompt_with_text(user_text: &str) -> CoachPrompt {
    CoachPrompt::new("You are an AI focus coach for Wowie.", user_text)
}

fn fixed_timestamp() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-10-24T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

// =========================================================================
// Category 1: Normal Canned Flows
// =========================================================================

#[tokio::test]
async fn test_canned_relevant_focus_flow() {
    let provider = MockCoachProvider::canned_relevant();
    let prompt = sample_prompt();

    let raw = provider.evaluate(&prompt).await.unwrap();
    assert_eq!(raw.relevance, RelevanceClassification::Relevant);
    assert!((raw.confidence - 0.95).abs() < 1e-4);
    assert_eq!(raw.uncertainty, Some(0.05));
    assert_eq!(raw.action_candidate, Some(ActionCandidate::None));

    let prov = ProviderProvenance::mock(fixed_timestamp());
    let response = raw.into_validated_response(prov).unwrap();

    assert!(response.is_relevant());
    assert!(!response.has_action());
    assert_eq!(response.effective_action(), ActionCandidate::None);
    assert_eq!(response.provenance.provider_name, "mock");
}

#[tokio::test]
async fn test_canned_possibly_unrelated_remind_flow() {
    let provider = MockCoachProvider::canned_distracted(ActionCandidate::Remind);
    let prompt = sample_prompt();

    let raw = provider.evaluate(&prompt).await.unwrap();
    assert_eq!(raw.relevance, RelevanceClassification::PossiblyUnrelated);
    assert!((raw.confidence - 0.85).abs() < 1e-4);
    assert_eq!(raw.action_candidate, Some(ActionCandidate::Remind));

    let prov = ProviderProvenance::mock(fixed_timestamp());
    let response = raw.into_validated_response(prov).unwrap();

    assert!(response.is_possibly_unrelated());
    assert!(response.has_action());
    assert_eq!(response.effective_action(), ActionCandidate::Remind);
    assert!(response.question.is_some());
}

#[tokio::test]
async fn test_canned_ambiguous_ask_context_flow() {
    let provider = MockCoachProvider::canned_distracted(ActionCandidate::AskContext);
    let prompt = sample_prompt();

    let raw = provider.evaluate(&prompt).await.unwrap();
    assert_eq!(raw.action_candidate, Some(ActionCandidate::AskContext));

    let prov = ProviderProvenance::mock(fixed_timestamp());
    let response = raw.into_validated_response(prov).unwrap();
    assert_eq!(response.effective_action(), ActionCandidate::AskContext);
}

#[tokio::test]
async fn test_canned_long_session_suggest_break_flow() {
    let provider = MockCoachProvider::canned_distracted(ActionCandidate::SuggestBreak);
    let prompt = sample_prompt();

    let raw = provider.evaluate(&prompt).await.unwrap();
    assert_eq!(raw.action_candidate, Some(ActionCandidate::SuggestBreak));

    let prov = ProviderProvenance::mock(fixed_timestamp());
    let response = raw.into_validated_response(prov).unwrap();
    assert_eq!(response.effective_action(), ActionCandidate::SuggestBreak);
}

#[tokio::test]
async fn test_canned_suggest_correction_flow() {
    let provider = MockCoachProvider::canned_distracted(ActionCandidate::SuggestCorrection);
    let prompt = sample_prompt();

    let raw = provider.evaluate(&prompt).await.unwrap();
    assert_eq!(
        raw.action_candidate,
        Some(ActionCandidate::SuggestCorrection)
    );

    let prov = ProviderProvenance::mock(fixed_timestamp());
    let response = raw.into_validated_response(prov).unwrap();
    assert_eq!(
        response.effective_action(),
        ActionCandidate::SuggestCorrection
    );
}

#[tokio::test]
async fn test_canned_custom_payload_with_telemetry() {
    let usage = TokenUsage::new(120, 45, 165);
    let raw = CoachRawResponse {
        relevance: RelevanceClassification::Relevant,
        confidence: 0.98,
        uncertainty: Some(0.02),
        rationale: "Optimized compile pass detected.".to_string(),
        question: None,
        action_candidate: Some(ActionCandidate::None),
        ..Default::default()
    }
    .with_telemetry(
        Some("raw text content".to_string()),
        Some(serde_json::json!({"id": "chat-123"})),
        Some(usage),
        Some(42),
        Some("mock-gpt-4o".to_string()),
    );

    let provider = MockCoachProvider::canned(raw);
    let result = provider.evaluate(&sample_prompt()).await.unwrap();

    assert_eq!(result.token_usage, Some(usage));
    assert_eq!(result.latency_ms, Some(42));
    assert_eq!(result.model.as_deref(), Some("mock-gpt-4o"));
    assert_eq!(result.raw_text.as_deref(), Some("raw text content"));
}

// =========================================================================
// Category 2: FIFO Multi-Turn Sequences
// =========================================================================

#[tokio::test]
async fn test_fifo_multi_turn_progression_across_session() {
    let provider = MockCoachProvider::empty_queue();

    // Turn 1: On-goal coding
    provider.push_response(CoachRawResponse {
        relevance: RelevanceClassification::Relevant,
        confidence: 0.95,
        uncertainty: Some(0.05),
        rationale: "Active coding in IDE aligns with focus goal.".to_string(),
        question: None,
        action_candidate: Some(ActionCandidate::None),
        ..Default::default()
    });

    // Turn 2: Ambiguous docs search
    provider.push_response(CoachRawResponse {
        relevance: RelevanceClassification::Unknown,
        confidence: 0.50,
        uncertainty: Some(0.50),
        rationale: "Browsing technical documentation.".to_string(),
        question: Some("Is this reference material?".to_string()),
        action_candidate: Some(ActionCandidate::AskContext),
        ..Default::default()
    });

    // Turn 3: Distracting social media
    provider.push_response(CoachRawResponse {
        relevance: RelevanceClassification::PossiblyUnrelated,
        confidence: 0.90,
        uncertainty: Some(0.10),
        rationale: "Social media browsing detected during focus block.".to_string(),
        question: Some("Need a gentle nudge back to the goal?".to_string()),
        action_candidate: Some(ActionCandidate::Remind),
        ..Default::default()
    });

    // Turn 4: Sustained session milestone
    provider.push_response(CoachRawResponse {
        relevance: RelevanceClassification::PossiblyUnrelated,
        confidence: 0.85,
        uncertainty: Some(0.15),
        rationale: "You've been focused for 90 minutes straight.".to_string(),
        question: Some("Time for a short break?".to_string()),
        action_candidate: Some(ActionCandidate::SuggestBreak),
        ..Default::default()
    });

    let p = sample_prompt();

    // Turn 1
    let t1 = provider.evaluate(&p).await.unwrap();
    assert_eq!(t1.relevance, RelevanceClassification::Relevant);
    assert_eq!(provider.call_count(), 1);

    // Turn 2
    let t2 = provider.evaluate(&p).await.unwrap();
    assert_eq!(t2.relevance, RelevanceClassification::Unknown);
    assert_eq!(t2.action_candidate, Some(ActionCandidate::AskContext));
    assert_eq!(provider.call_count(), 2);

    // Turn 3
    let t3 = provider.evaluate(&p).await.unwrap();
    assert_eq!(t3.relevance, RelevanceClassification::PossiblyUnrelated);
    assert_eq!(t3.action_candidate, Some(ActionCandidate::Remind));
    assert_eq!(provider.call_count(), 3);

    // Turn 4
    let t4 = provider.evaluate(&p).await.unwrap();
    assert_eq!(t4.action_candidate, Some(ActionCandidate::SuggestBreak));
    assert_eq!(provider.call_count(), 4);

    assert_eq!(provider.received_prompts().len(), 4);
}

#[tokio::test]
async fn test_fifo_queue_exhaustion_behavior_error() {
    let provider = MockCoachProvider::empty_queue();
    provider.push_response(CoachRawResponse {
        relevance: RelevanceClassification::Relevant,
        confidence: 0.95,
        uncertainty: None,
        rationale: "Turn 1".to_string(),
        question: None,
        action_candidate: None,
        ..Default::default()
    });
    provider.set_on_empty(QueueEmptyBehavior::Error);

    let p = sample_prompt();
    assert!(provider.evaluate(&p).await.is_ok());

    // Second evaluation on exhausted queue
    let err = provider.evaluate(&p).await.unwrap_err();
    assert!(matches!(err, CoachError::ProviderUnavailable { .. }));

    // Fallback degradation
    let fallback = DeterministicFallback::new();
    let fb_resp = fallback.evaluate(&err, fixed_timestamp());
    assert_eq!(fb_resp.relevance, RelevanceClassification::Unknown);
    assert_eq!(fb_resp.confidence, 0.0);
    assert_eq!(fb_resp.action_candidate, Some(ActionCandidate::None));
}

#[tokio::test]
async fn test_fifo_queue_exhaustion_behavior_fallback() {
    let fallback_raw = CoachRawResponse {
        relevance: RelevanceClassification::Unknown,
        confidence: 0.42,
        uncertainty: None,
        rationale: "Queue exhausted fallback".to_string(),
        question: None,
        action_candidate: Some(ActionCandidate::None),
        ..Default::default()
    };

    let provider = MockCoachProvider::empty_queue();
    provider.push_response(CoachRawResponse {
        relevance: RelevanceClassification::Relevant,
        confidence: 0.99,
        uncertainty: None,
        rationale: "Turn 1".to_string(),
        question: None,
        action_candidate: None,
        ..Default::default()
    });
    provider.set_on_empty(QueueEmptyBehavior::Fallback(fallback_raw));

    let p = sample_prompt();
    let t1 = provider.evaluate(&p).await.unwrap();
    assert_eq!(t1.confidence, 0.99);

    let t2 = provider.evaluate(&p).await.unwrap();
    assert_eq!(t2.confidence, 0.42);
    assert_eq!(t2.rationale, "Queue exhausted fallback");
}

#[tokio::test]
async fn test_fifo_queue_exhaustion_behavior_repeat_last() {
    let provider = MockCoachProvider::empty_queue();
    provider.push_response(CoachRawResponse {
        relevance: RelevanceClassification::PossiblyUnrelated,
        confidence: 0.77,
        uncertainty: None,
        rationale: "Repeated item".to_string(),
        question: None,
        action_candidate: Some(ActionCandidate::Remind),
        ..Default::default()
    });
    provider.set_on_empty(QueueEmptyBehavior::RepeatLast);

    let p = sample_prompt();
    let t1 = provider.evaluate(&p).await.unwrap();
    assert_eq!(t1.confidence, 0.77);

    let t2 = provider.evaluate(&p).await.unwrap();
    assert_eq!(t2.confidence, 0.77);

    let t3 = provider.evaluate(&p).await.unwrap();
    assert_eq!(t3.confidence, 0.77);
}

#[tokio::test]
async fn test_fifo_interleaved_success_and_errors() {
    let provider = MockCoachProvider::empty_queue();
    provider.push_response(CoachRawResponse {
        relevance: RelevanceClassification::Relevant,
        confidence: 0.9,
        uncertainty: None,
        rationale: "Success 1".to_string(),
        question: None,
        action_candidate: None,
        ..Default::default()
    });
    provider.push_error(CoachError::timeout(1000));
    provider.push_response(CoachRawResponse {
        relevance: RelevanceClassification::Relevant,
        confidence: 0.9,
        uncertainty: None,
        rationale: "Success 2".to_string(),
        question: None,
        action_candidate: None,
        ..Default::default()
    });
    provider.push_error(CoachError::http(500, "Internal Server Error"));

    let p = sample_prompt();
    assert!(provider.evaluate(&p).await.is_ok());
    assert!(matches!(
        provider.evaluate(&p).await.unwrap_err(),
        CoachError::Timeout { .. }
    ));
    assert!(provider.evaluate(&p).await.is_ok());
    assert!(matches!(
        provider.evaluate(&p).await.unwrap_err(),
        CoachError::HttpError {
            status_code: 500,
            ..
        }
    ));
}

// =========================================================================
// Category 3: Error Injection & Timeout Simulations
// =========================================================================

#[tokio::test]
async fn test_transient_failure_recovery() {
    let success = CoachRawResponse {
        relevance: RelevanceClassification::Relevant,
        confidence: 0.92,
        uncertainty: Some(0.08),
        rationale: "Recovered after transient failure".to_string(),
        question: None,
        action_candidate: Some(ActionCandidate::None),
        ..Default::default()
    };

    let provider = MockCoachProvider::transient_failure(2, CoachError::timeout(3000), success);
    let p = sample_prompt();

    // Call 1 fails
    let err1 = provider.evaluate(&p).await.unwrap_err();
    assert!(matches!(err1, CoachError::Timeout { duration_ms: 3000 }));

    // Call 2 fails
    let err2 = provider.evaluate(&p).await.unwrap_err();
    assert!(matches!(err2, CoachError::Timeout { duration_ms: 3000 }));

    // Call 3 succeeds
    let ok = provider.evaluate(&p).await.unwrap();
    assert_eq!(ok.rationale, "Recovered after transient failure");
    assert_eq!(provider.call_count(), 3);
}

#[tokio::test]
async fn test_timeout_error_conversion_to_safe_fallback() {
    let err = CoachError::timeout(3500);
    let fallback = DeterministicFallback::with_provider("openai_compatible");
    let now = fixed_timestamp();

    let resp = fallback.evaluate(&err, now);
    assert_eq!(resp.relevance, RelevanceClassification::Unknown);
    assert_eq!(resp.confidence, 0.0);
    assert_eq!(resp.uncertainty, Some(1.0));
    assert_eq!(resp.action_candidate, Some(ActionCandidate::None));
    assert_eq!(resp.effective_action(), ActionCandidate::None);
    assert!(!resp.has_action());
    assert_eq!(
        resp.rationale,
        "AI provider timed out after 3500ms. Neutral fallback applied without active intervention."
    );
    assert_eq!(resp.provenance.provider_name, "openai_compatible");
    assert_eq!(resp.provenance.model_name, "deterministic-fallback");
    assert_eq!(resp.provenance.latency_ms, Some(0));
}

#[tokio::test]
async fn test_simulated_delay_execution() {
    let provider = MockCoachProvider::canned_relevant().with_delay(Duration::from_millis(50));
    let start = std::time::Instant::now();
    let res = provider.evaluate(&sample_prompt()).await;
    let elapsed = start.elapsed();

    assert!(res.is_ok());
    assert!(
        elapsed >= Duration::from_millis(40),
        "Simulated delay was {elapsed:?}, expected >= 40ms"
    );
}

// =========================================================================
// Category 4: Malformed JSON Recovery
// =========================================================================

#[tokio::test]
async fn test_malformed_json_truncated_tokens_recovery() {
    let truncated_wire = r#"{"relevance": "relevant", "confid"#;
    let err = CoachRawResponse::from_json(truncated_wire).unwrap_err();
    assert!(matches!(err, CoachError::MalformedJson { .. }));

    let fallback = DeterministicFallback::new();
    let resp = fallback.evaluate(&err, fixed_timestamp());
    assert_eq!(resp.relevance, RelevanceClassification::Unknown);
    assert_eq!(resp.confidence, 0.0);
    assert_eq!(resp.action_candidate, Some(ActionCandidate::None));
    assert_eq!(
        resp.rationale,
        "AI provider returned an unparseable response payload. Neutral fallback applied without active intervention."
    );
}

#[tokio::test]
async fn test_malformed_json_html_error_page_recovery() {
    let html =
        "<html><head><title>502 Bad Gateway</title></head><body>502 Bad Gateway</body></html>";
    let err = CoachRawResponse::from_json(html).unwrap_err();
    assert!(matches!(err, CoachError::MalformedJson { .. }));

    let fallback = DeterministicFallback::new();
    let resp = fallback.evaluate(&err, fixed_timestamp());
    assert!(!resp.rationale.contains("<html>"));
    assert!(!resp.rationale.contains("<head>"));
    assert!(!resp.rationale.contains("<body>"));
}

#[tokio::test]
async fn test_malformed_json_plain_text_refusal_recovery() {
    let refusal = "I apologize, but as an AI language model, I cannot evaluate desktop activity.";
    let err = CoachRawResponse::from_json(refusal).unwrap_err();
    assert!(matches!(err, CoachError::MalformedJson { .. }));

    let fallback = DeterministicFallback::new();
    let resp = fallback.evaluate(&err, fixed_timestamp());
    assert_eq!(resp.relevance, RelevanceClassification::Unknown);
    assert_eq!(resp.effective_action(), ActionCandidate::None);
}

// =========================================================================
// Category 5: Disallowed Action Candidate Recovery
// =========================================================================

#[tokio::test]
async fn test_disallowed_action_shell_command_recovery() {
    let malicious = r#"{
        "relevance": "possibly_unrelated",
        "confidence": 0.9,
        "rationale": "Attack payload test",
        "action_candidate": "rm -rf / --no-preserve-root"
    }"#;

    let err = CoachRawResponse::from_json(malicious).unwrap_err();
    assert!(matches!(err, CoachError::DisallowedAction { .. }));

    let fallback = DeterministicFallback::new();
    let resp = fallback.evaluate(&err, fixed_timestamp());

    assert_eq!(resp.relevance, RelevanceClassification::Unknown);
    assert_eq!(resp.action_candidate, Some(ActionCandidate::None));
    assert_eq!(resp.effective_action(), ActionCandidate::None);

    // Ensure zero command leakage in rationale
    assert!(!resp.rationale.contains("rm -rf"));
    assert!(!resp.rationale.contains("--no-preserve-root"));
}

#[tokio::test]
async fn test_disallowed_action_os_tampering_recovery() {
    let attacks = [
        "kill_process",
        "shutdown",
        "reboot",
        "block_network",
        "exec_bash",
        "download_binary",
    ];

    let fallback = DeterministicFallback::new();

    for attack in attacks {
        let json = format!(
            r#"{{"relevance": "relevant", "confidence": 0.8, "rationale": "Test", "action_candidate": "{attack}"}}"#
        );
        let err = CoachRawResponse::from_json(&json).unwrap_err();
        assert!(matches!(err, CoachError::DisallowedAction { .. }));

        let resp = fallback.evaluate(&err, fixed_timestamp());
        assert_eq!(resp.effective_action(), ActionCandidate::None);
        assert!(!resp.rationale.contains(attack));
    }
}

#[tokio::test]
async fn test_disallowed_action_casing_and_typos() {
    let typos = ["None", "NONE", "REMIND", "Ask_Context", "suggest-break", ""];

    for typo in typos {
        let json = format!(
            r#"{{"relevance": "relevant", "confidence": 0.8, "rationale": "Test", "action_candidate": "{typo}"}}"#
        );
        let err = CoachRawResponse::from_json(&json).unwrap_err();
        assert!(matches!(err, CoachError::DisallowedAction { .. }));
    }
}

// =========================================================================
// Category 6: Error Sanitization, Network, & Fallback Classification
// =========================================================================

#[tokio::test]
async fn test_network_dns_failure_zero_ip_leak() {
    let sensitive_err = CoachError::network(
        "DNS lookup failed for internal-prod-llm.corp.local:9000 (10.100.5.23: Connection refused)",
    );

    let fallback = DeterministicFallback::new();
    let resp = fallback.evaluate(&sensitive_err, fixed_timestamp());

    assert_eq!(
        resp.rationale,
        "Network communication with AI provider failed. Neutral fallback applied without active intervention."
    );
    assert!(!resp.rationale.contains("internal-prod-llm"));
    assert!(!resp.rationale.contains("corp.local"));
    assert!(!resp.rationale.contains("9000"));
    assert!(!resp.rationale.contains("10.100.5.23"));
}

#[tokio::test]
async fn test_http_status_codes_fallback() {
    let fallback = DeterministicFallback::new();

    let codes = [400, 401, 403, 404, 429, 500, 502, 503, 504];
    for code in codes {
        let err = CoachError::http(code, format!("Error {code} with sensitive auth header"));
        let resp = fallback.evaluate(&err, fixed_timestamp());

        assert_eq!(resp.relevance, RelevanceClassification::Unknown);
        assert_eq!(resp.confidence, 0.0);
        assert_eq!(resp.action_candidate, Some(ActionCandidate::None));
        assert!(resp.rationale.contains(&format!("HTTP status {code}")));
        assert!(!resp.rationale.contains("auth header"));
    }
}

#[test]
fn test_fallback_reason_classification() {
    let cases = [
        (CoachError::timeout(100), FallbackReason::Timeout, false),
        (
            CoachError::network("broken"),
            FallbackReason::NetworkError,
            false,
        ),
        (
            CoachError::http(500, "err"),
            FallbackReason::HttpError,
            false,
        ),
        (
            CoachError::malformed_json("bad", None, None),
            FallbackReason::MalformedJson,
            false,
        ),
        (
            CoachError::schema_validation("f", "r"),
            FallbackReason::SchemaViolation,
            false,
        ),
        (
            CoachError::disallowed_action("hack"),
            FallbackReason::DisallowedAction,
            true,
        ),
        (
            CoachError::prompt_injection("input", "tag"),
            FallbackReason::PromptInjection,
            true,
        ),
        (
            CoachError::provider_unavailable("openai", "down"),
            FallbackReason::ProviderUnavailable,
            false,
        ),
        (
            CoachError::cooldown_throttled(60, "wait"),
            FallbackReason::CooldownThrottled,
            false,
        ),
    ];

    for (err, expected_reason, expected_security) in cases {
        let reason = FallbackReason::from_error(&err);
        assert_eq!(reason, expected_reason);
        assert_eq!(reason.is_security_relevant(), expected_security);
        assert_eq!(reason.to_string(), reason.as_str());
    }
}

// =========================================================================
// Category 7: Heuristic Matching Mode
// =========================================================================

#[tokio::test]
async fn test_heuristic_break_state() {
    let provider = MockCoachProvider::heuristic();
    let prompt = sample_prompt_with_text(
        "Goal: Build Rust microservice.\nBreak active: true\nObserved: browsing youtube.com",
    );

    let raw = provider.evaluate(&prompt).await.unwrap();
    assert_eq!(raw.relevance, RelevanceClassification::Relevant);
    assert_eq!(raw.confidence, 0.90);
    assert_eq!(raw.action_candidate, Some(ActionCandidate::None));
    assert!(raw.rationale.contains("scheduled focus break"));
}

#[tokio::test]
async fn test_heuristic_distraction_keywords() {
    let provider = MockCoachProvider::heuristic();
    let distractions = [
        "youtube.com",
        "reddit.com",
        "twitter.com",
        "netflix.com",
        "steam",
    ];

    for d in distractions {
        let prompt = sample_prompt_with_text(&format!(
            "Goal: Focus on Rust coding.\nObserved activity: Watching video on {d}"
        ));
        let raw = provider.evaluate(&prompt).await.unwrap();

        assert_eq!(raw.relevance, RelevanceClassification::PossiblyUnrelated);
        assert_eq!(raw.action_candidate, Some(ActionCandidate::Remind));
        assert!(raw.rationale.contains(d));
    }
}

#[tokio::test]
async fn test_heuristic_ambiguous_keywords() {
    let provider = MockCoachProvider::heuristic();
    let ambiguous = [
        "tutorial",
        "stackoverflow.com",
        "wikipedia.org",
        "docs",
        "guide",
    ];

    for a in ambiguous {
        let prompt = sample_prompt_with_text(&format!(
            "Goal: Focus on Rust coding.\nObserved activity: Reading {a} for error fix"
        ));
        let raw = provider.evaluate(&prompt).await.unwrap();

        assert_eq!(raw.relevance, RelevanceClassification::PossiblyUnrelated);
        assert_eq!(raw.action_candidate, Some(ActionCandidate::AskContext));
        assert!(raw.rationale.contains(a));
    }
}

#[tokio::test]
async fn test_heuristic_default_productive() {
    let provider = MockCoachProvider::heuristic();
    let prompt = sample_prompt_with_text(
        "Goal: Focus on Rust coding.\nObserved activity: cargo test -p wowie-core",
    );

    let raw = provider.evaluate(&prompt).await.unwrap();
    assert_eq!(raw.relevance, RelevanceClassification::Relevant);
    assert_eq!(raw.confidence, 0.92);
    assert_eq!(raw.action_candidate, Some(ActionCandidate::None));
}

// =========================================================================
// Category 8: Call History Inspection & Reset
// =========================================================================

#[tokio::test]
async fn test_call_history_inspection_and_reset() {
    let provider = MockCoachProvider::canned_relevant();
    assert_eq!(provider.call_count(), 0);
    assert!(provider.received_prompts().is_empty());
    assert!(provider.last_prompt().is_none());

    let p1 = sample_prompt_with_text("User prompt 1");
    let p2 = sample_prompt_with_text("User prompt 2");
    let p3 = sample_prompt_with_text("User prompt 3");

    let _ = provider.evaluate(&p1).await;
    let _ = provider.evaluate(&p2).await;
    let _ = provider.evaluate(&p3).await;

    assert_eq!(provider.call_count(), 3);
    assert_eq!(provider.received_prompts().len(), 3);
    assert_eq!(provider.last_prompt().unwrap().user_prompt, "User prompt 3");

    provider.reset_history();
    assert_eq!(provider.call_count(), 0);
    assert!(provider.received_prompts().is_empty());
    assert!(provider.last_prompt().is_none());
}

// =========================================================================
// Category 9: Dynamic Dispatch & Arc<dyn CoachProvider>
// =========================================================================

#[tokio::test]
async fn test_object_safety_and_dynamic_dispatch() {
    let mock = MockCoachProvider::canned_relevant()
        .with_name("custom_mock")
        .with_model_name("mock-engine-v2");

    let provider: Arc<dyn CoachProvider> = Arc::new(mock);

    assert_eq!(provider.name(), "custom_mock");
    assert_eq!(provider.model_name(), "mock-engine-v2");
    assert!(provider.is_available().await);

    let raw = provider.evaluate(&sample_prompt()).await.unwrap();
    assert_eq!(raw.relevance, RelevanceClassification::Relevant);
}

// =========================================================================
// Category 10: OpenAiCompatibleClient Offline Unit & Mock Server Tests
// =========================================================================

#[test]
fn test_openai_config_validation_and_presets() {
    // 1. Ollama preset
    let ollama_cfg = OpenAiConfig::ollama("llama3.2:3b");
    assert_eq!(
        ollama_cfg.endpoint_url,
        "http://localhost:11434/v1/chat/completions"
    );
    assert_eq!(ollama_cfg.model_name, "llama3.2:3b");
    assert!(ollama_cfg.api_key.is_none());
    assert!(ollama_cfg.validate().is_ok());

    // 2. OpenAI preset
    let openai_cfg = OpenAiConfig::openai("gpt-4o-mini", "sk-secret-token-9999");
    assert_eq!(
        openai_cfg.endpoint_url,
        "https://api.openai.com/v1/chat/completions"
    );
    assert_eq!(openai_cfg.api_key.as_deref(), Some("sk-secret-token-9999"));
    assert!(openai_cfg.validate().is_ok());

    // 3. Credential redaction in Debug
    let debug_str = format!("{openai_cfg:?}");
    assert!(debug_str.contains("[REDACTED]"));
    assert!(!debug_str.contains("sk-secret-token-9999"));

    // 4. Validation errors
    let mut bad_cfg = OpenAiConfig::new("", "model");
    assert!(bad_cfg.validate().is_err());

    bad_cfg.endpoint_url = "ftp://invalid.com".to_string();
    assert!(bad_cfg.validate().is_err());

    bad_cfg.endpoint_url = "http://valid.com".to_string();
    bad_cfg.model_name = String::new();
    assert!(bad_cfg.validate().is_err());

    bad_cfg.model_name = "model".to_string();
    bad_cfg.timeout = Duration::ZERO;
    assert!(bad_cfg.validate().is_err());
}

#[test]
fn test_openai_config_debug_redaction_custom_headers() {
    let cfg = OpenAiConfig::openai("gpt-4o-mini", "sk-secret-root-key")
        .with_header("Authorization", "Bearer sk-auth-custom-token")
        .with_header("X-Api-Key", "custom-api-key-999")
        .with_header("X-Service-Token", "super-secret-token")
        .with_header("X-Secret-Access", "access-token-abc")
        .with_header("User-Agent", "Wowie/1.0")
        .with_header("X-Organization-Id", "org-non-sensitive");

    let debug_str = format!("{cfg:?}");

    // Sensitive values must be redacted
    assert!(!debug_str.contains("sk-secret-root-key"));
    assert!(!debug_str.contains("sk-auth-custom-token"));
    assert!(!debug_str.contains("custom-api-key-999"));
    assert!(!debug_str.contains("super-secret-token"));
    assert!(!debug_str.contains("access-token-abc"));

    // Redacted placeholders should appear
    assert!(debug_str.contains("[REDACTED]"));

    // Non-sensitive values and header names should be preserved
    assert!(debug_str.contains("User-Agent"));
    assert!(debug_str.contains("Wowie/1.0"));
    assert!(debug_str.contains("X-Organization-Id"));
    assert!(debug_str.contains("org-non-sensitive"));
    assert!(debug_str.contains("Authorization"));
}

#[test]
fn test_openai_payload_construction() {
    let config = OpenAiConfig::openai("gpt-4o-mini", "test-key")
        .with_temperature(0.3)
        .unwrap()
        .with_max_tokens(512)
        .with_response_format(ResponseFormatMode::JsonObject);

    let client = OpenAiCompatibleClient::new(config).unwrap();
    let prompt = sample_prompt();

    let payload = client.build_request_payload(&prompt);
    assert_eq!(payload["model"], "gpt-4o-mini");
    assert!((payload["temperature"].as_f64().unwrap() - 0.3).abs() < 1e-4);
    assert_eq!(payload["max_tokens"], 512);
    assert_eq!(payload["response_format"]["type"], "json_object");

    let messages = payload["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["role"], "system");
    assert_eq!(messages[1]["role"], "user");
}

#[test]
fn test_openai_parse_completion_response_success() {
    let client = OpenAiCompatibleClient::new(OpenAiConfig::ollama("llama3:8b")).unwrap();

    let wire_body = r#"{
        "id": "chatcmpl-abc123xyz",
        "object": "chat.completion",
        "created": 1729771200,
        "model": "llama3:8b-instruct",
        "choices": [
            {
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "{\"relevance\":\"relevant\",\"confidence\":0.96,\"uncertainty\":0.04,\"rationale\":\"User is editing code actively.\",\"action_candidate\":\"none\"}"
                },
                "finish_reason": "stop"
            }
        ],
        "usage": {
            "prompt_tokens": 150,
            "completion_tokens": 30,
            "total_tokens": 180
        }
    }"#;

    let raw = client.parse_completion_response(wire_body).unwrap();
    assert_eq!(raw.relevance, RelevanceClassification::Relevant);
    assert!((raw.confidence - 0.96).abs() < 1e-4);
    assert_eq!(raw.action_candidate, Some(ActionCandidate::None));
    assert_eq!(raw.token_usage, Some(TokenUsage::new(150, 30, 180)));
    assert_eq!(raw.model.as_deref(), Some("llama3:8b-instruct"));
    assert!(raw.raw_text.is_some());
    assert!(raw.raw_json.is_some());
}

#[test]
fn test_openai_parse_completion_response_missing_fields_or_empty_choices() {
    let client = OpenAiCompatibleClient::new(OpenAiConfig::ollama("llama3:8b")).unwrap();

    // 1. Missing choices
    let res1 = client.parse_completion_response(r#"{"id": "123"}"#);
    assert!(matches!(
        res1.unwrap_err(),
        CoachError::SchemaValidation { .. }
    ));

    // 2. Empty choices array
    let res2 = client.parse_completion_response(r#"{"choices": []}"#);
    assert!(matches!(
        res2.unwrap_err(),
        CoachError::SchemaValidation { .. }
    ));

    // 3. Missing message
    let res3 = client.parse_completion_response(r#"{"choices": [{}]}"#);
    assert!(matches!(
        res3.unwrap_err(),
        CoachError::SchemaValidation { .. }
    ));
}

#[test]
fn test_openai_error_mapping() {
    let client = OpenAiCompatibleClient::new(OpenAiConfig::ollama("llama3:8b")).unwrap();

    // 401
    let err401 =
        client.map_http_error(401, r#"{"error": {"message": "Invalid API key provided"}}"#);
    assert!(matches!(err401, CoachError::ProviderUnavailable { .. }));

    // 404
    let err404 = client.map_http_error(404, "model not found");
    assert!(matches!(
        err404,
        CoachError::HttpError {
            status_code: 404,
            ..
        }
    ));

    // 429
    let err429 = client.map_http_error(
        429,
        r#"{"error": {"message": "Rate limit reached for requests per minute"}}"#,
    );
    assert!(matches!(
        err429,
        CoachError::HttpError {
            status_code: 429,
            ..
        }
    ));

    // 500
    let err500 = client.map_http_error(500, "Internal Server Error");
    assert!(matches!(
        err500,
        CoachError::HttpError {
            status_code: 500,
            ..
        }
    ));
}

#[test]
fn test_openai_error_mapping_utf8_char_boundary_truncation() {
    let client = OpenAiCompatibleClient::new(OpenAiConfig::ollama("llama3:8b")).unwrap();

    // 1. Exactly 500 ASCII characters - must NOT be truncated
    let exact_500 = "x".repeat(500);
    let err = client.map_http_error(500, &exact_500);
    match err {
        CoachError::HttpError { message, .. } => {
            assert_eq!(message.len(), 500);
            assert!(!message.ends_with("..."));
        }
        other => panic!("Expected HttpError, got {other:?}"),
    }

    // 2. 501 ASCII characters - must truncate to 500 chars + "..."
    let over_500 = "x".repeat(501);
    let err = client.map_http_error(500, &over_500);
    match err {
        CoachError::HttpError { message, .. } => {
            assert_eq!(message, format!("{}...", "x".repeat(500)));
        }
        other => panic!("Expected HttpError, got {other:?}"),
    }

    // 3. Multi-byte UTF-8: 4-byte emoji (🦀) straddling byte 500
    let mut bad_body_4byte = "a".repeat(498);
    bad_body_4byte.push('🦀'); // bytes 498..502
    bad_body_4byte.push_str("extra content beyond 500 bytes");

    let err = client.map_http_error(500, &bad_body_4byte);
    match err {
        CoachError::HttpError { message, .. } => {
            assert!(message.ends_with("..."));
            assert!(message.contains('🦀'));
            // Must contain exactly 500 characters before the trailing "..."
            let base = message.strip_suffix("...").unwrap();
            assert_eq!(base.chars().count(), 500);
        }
        other => panic!("Expected HttpError, got {other:?}"),
    }

    // 4. Multi-byte UTF-8: 3-byte symbol (€) straddling byte 500
    let mut bad_body_3byte = "a".repeat(499);
    bad_body_3byte.push('€'); // bytes 499..502
    bad_body_3byte.push_str("extra content beyond 500 bytes");

    let err = client.map_http_error(502, &bad_body_3byte);
    match err {
        CoachError::HttpError { message, .. } => {
            assert!(message.ends_with("..."));
            assert!(message.contains('€'));
            let base = message.strip_suffix("...").unwrap();
            assert_eq!(base.chars().count(), 500);
        }
        other => panic!("Expected HttpError, got {other:?}"),
    }

    // 5. Multi-byte UTF-8: 2-byte symbol (я) straddling byte 500
    let mut bad_body_2byte = "a".repeat(499);
    bad_body_2byte.push('я'); // bytes 499..501
    bad_body_2byte.push_str("trailing");

    let err = client.map_http_error(503, &bad_body_2byte);
    match err {
        CoachError::HttpError { message, .. } => {
            assert!(message.ends_with("..."));
            assert!(message.contains('я'));
            let base = message.strip_suffix("...").unwrap();
            assert_eq!(base.chars().count(), 500);
        }
        other => panic!("Expected HttpError, got {other:?}"),
    }

    // 6. Multi-byte UTF-8 inside JSON error envelope
    let json_err_body = format!(
        r#"{{"error": {{"message": "{}🦀extra content"}}}}"#,
        "b".repeat(498)
    );
    let err = client.map_http_error(400, &json_err_body);
    match err {
        CoachError::HttpError { message, .. } => {
            assert!(message.ends_with("..."));
            assert!(message.contains('🦀'));
            let base = message.strip_suffix("...").unwrap();
            assert_eq!(base.chars().count(), 500);
        }
        other => panic!("Expected HttpError, got {other:?}"),
    }

    // 7. Huge payload - ensures fast O(500) early-exit truncation
    let huge_payload = "🔥".repeat(25_000); // 100,000 bytes
    let err = client.map_http_error(503, &huge_payload);
    match err {
        CoachError::HttpError { message, .. } => {
            assert!(message.ends_with("..."));
            let base = message.strip_suffix("...").unwrap();
            assert_eq!(base.chars().count(), 500);
        }
        other => panic!("Expected HttpError, got {other:?}"),
    }
}

#[tokio::test]
async fn test_openai_client_in_process_loopback_evaluation() {
    // 1. Bind in-process mock server on 127.0.0.1:0 (ephemeral port)
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();
    let endpoint_url = format!("http://{local_addr}/v1/chat/completions");

    // 2. Spawn simple mock HTTP responder
    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 4096];
            let _ = socket.read(&mut buf).await;

            let response_body = serde_json::json!({
                "id": "chatcmpl-mock-loopback-001",
                "object": "chat.completion",
                "created": 1_729_771_200,
                "model": "loopback-mock-model",
                "choices": [
                    {
                        "index": 0,
                        "message": {
                            "role": "assistant",
                            "content": "{\"relevance\":\"relevant\",\"confidence\":0.97,\"uncertainty\":0.03,\"rationale\":\"Loopback mock assessment aligned.\",\"action_candidate\":\"none\"}"
                        },
                        "finish_reason": "stop"
                    }
                ],
                "usage": {
                    "prompt_tokens": 100,
                    "completion_tokens": 25,
                    "total_tokens": 125
                }
            }).to_string();

            let http_response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );

            let _ = socket.write_all(http_response.as_bytes()).await;
            let _ = socket.flush().await;
        }
    });

    // 3. Configure client pointing to in-process loopback endpoint
    let config = OpenAiConfig::new(endpoint_url, "loopback-mock-model")
        .with_api_key("sk-test-loopback")
        .with_timeout(Duration::from_secs(3));

    let client = OpenAiCompatibleClient::new(config).unwrap();
    let prompt = sample_prompt();

    // 4. Execute evaluation completely offline
    let raw = client.evaluate(&prompt).await.unwrap();

    assert_eq!(raw.relevance, RelevanceClassification::Relevant);
    assert!((raw.confidence - 0.97).abs() < 1e-4);
    assert_eq!(raw.uncertainty, Some(0.03));
    assert_eq!(raw.action_candidate, Some(ActionCandidate::None));
    assert_eq!(raw.token_usage, Some(TokenUsage::new(100, 25, 125)));
    assert_eq!(raw.model.as_deref(), Some("loopback-mock-model"));
    assert!(raw.latency_ms.is_some());
}

// =========================================================================
// Category 11: 100% Offline Determinism
// =========================================================================

#[tokio::test]
async fn test_offline_determinism_identical_runs_zero_sockets() {
    let provider = MockCoachProvider::canned_relevant();
    let prompt = sample_prompt();
    let now = fixed_timestamp();

    let base_raw = provider.evaluate(&prompt).await.unwrap();
    let base_resp = base_raw
        .into_validated_response(ProviderProvenance::mock(now))
        .unwrap();

    for _ in 0..200 {
        let raw = provider.evaluate(&prompt).await.unwrap();
        let resp = raw
            .into_validated_response(ProviderProvenance::mock(now))
            .unwrap();

        assert_eq!(resp, base_resp);
    }

    assert_eq!(provider.call_count(), 201);
}

#[test]
fn test_mock_behavior_debug_and_custom() {
    let custom: CustomEvaluator = Box::new(|prompt| {
        if prompt.user_prompt.contains("fail") {
            Err(CoachError::network("custom error"))
        } else {
            Ok(CoachRawResponse {
                relevance: RelevanceClassification::Relevant,
                confidence: 0.90,
                uncertainty: None,
                rationale: "Custom match".to_string(),
                question: None,
                action_candidate: Some(ActionCandidate::None),
                ..Default::default()
            })
        }
    });

    let behavior = MockBehavior::Custom(custom);
    let debug_str = format!("{behavior:?}");
    assert!(debug_str.contains("Custom(<closure>)"));

    let canned = MockBehavior::Canned(Ok(CoachRawResponse::default()));
    assert!(format!("{canned:?}").contains("Canned"));

    let heuristic = MockBehavior::Heuristic;
    assert_eq!(format!("{heuristic:?}"), "Heuristic");

    let resp: CoachResponse = CoachResponse::fallback(fixed_timestamp(), "reason");
    assert_eq!(resp.relevance, RelevanceClassification::Unknown);
}
