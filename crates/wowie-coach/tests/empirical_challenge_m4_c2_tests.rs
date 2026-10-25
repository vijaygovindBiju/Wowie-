//! Empirical Adversarial Challenge Test Suite for Milestone 4.
//!
//! File: tests/empirical_challenge_m4_c2_tests.rs
//! Role: Empirical Challenger 2 (Milestone 4: Pluggable Provider Abstraction & Fallback Engine)
//!
//! Adversarially verifies and stress-tests:
//! 1. UTF-8 char boundary panic vulnerability in `OpenAiCompatibleClient::map_http_error`.
//! 2. HTTP status error mapping (401, 403, 404, 429, 500, 502, 503, 504) to `CoachError`.
//! 3. JSON error body variations (OpenAI standard, string error, null error, HTML, plain text, empty).
//! 4. Malformed outer HTTP response bodies (truncated wire tokens, missing choices, invalid schemas).
//! 5. Malformed inner `message.content` responses (truncated JSON, plain text refusals, schema violations, disallowed actions).
//! 6. Truncated wire streams & abrupt TCP socket resets.
//! 7. Connection errors & refusals (dead ports, connection resets).
//! 8. Request timeout enforcement with in-process mock server.
//! 9. Exhaustive `DeterministicFallback` recovery matrix across all provider error variations.
//! 10. `DeterministicFallback` provider provenance boundary & serialization invariants.

use chrono::{DateTime, Utc};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use wowie_coach::{
    ActionCandidate, CoachError, CoachPrompt, CoachProvider, CoachResponse, DeterministicFallback,
    FallbackReason, OpenAiCompatibleClient, OpenAiConfig, RelevanceClassification,
};

fn sample_prompt() -> CoachPrompt {
    CoachPrompt::new(
        "You are an AI focus coach for Wowie.",
        "Goal: Build Rust microservice.\nObserved activity: cargo build in terminal.",
    )
}

fn fixed_timestamp() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-10-25T12:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

// =========================================================================
// Category 1: UTF-8 Char Boundary Vulnerability in `map_http_error`
// =========================================================================

#[test]
fn test_empirical_utf8_char_boundary_panic_vulnerability() {
    let client = OpenAiCompatibleClient::new(OpenAiConfig::ollama("llama3:8b")).unwrap();

    // The vulnerability:
    // `crates/wowie-coach/src/provider/openai.rs:373-375`:
    //     let bounded_msg = if error_msg.len() > 500 {
    //         format!("{}...", &error_msg[..500])
    //     } else {
    //         error_msg
    //     };
    //
    // `error_msg.len()` is byte length, not char count.
    // Slicing `&error_msg[..500]` assumes byte index 500 is a valid UTF-8 boundary.
    // If a multi-byte character (2, 3, or 4 bytes) straddles byte index 500, Rust PANICS!

    // Case 1: 4-byte UTF-8 emoji (🦀: 0xF0, 0x9F, 0xA6, 0x80) straddling byte 500
    // Byte offset 498: start of '🦀' (bytes 498..502)
    // Slicing at 500 attempts to slice inside '🦀' at byte 2!
    let mut bad_body_4byte = "a".repeat(498);
    bad_body_4byte.push('🦀');
    bad_body_4byte.push_str("extra content beyond 500 bytes");

    let panic_result_4byte = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = client.map_http_error(500, &bad_body_4byte);
    }));

    // Case 2: 3-byte UTF-8 character (€: 0xE2, 0x82, 0xAC) straddling byte 500
    // Byte offset 499: start of '€' (bytes 499..502)
    // Slicing at 500 attempts to slice inside '€' at byte 1!
    let mut bad_body_3byte = "a".repeat(499);
    bad_body_3byte.push('€');
    bad_body_3byte.push_str("extra content beyond 500 bytes");

    let panic_result_3byte = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = client.map_http_error(502, &bad_body_3byte);
    }));

    // Case 3: 2-byte UTF-8 character (я: 0xD1, 0x8F) straddling byte 500
    // Byte offset 499: start of 'я' (bytes 499..501)
    // Slicing at 500 attempts to slice inside 'я' at byte 1!
    let mut bad_body_2byte = "a".repeat(499);
    bad_body_2byte.push('я');
    bad_body_2byte.push_str("extra content beyond 500 bytes");

    let panic_result_2byte = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = client.map_http_error(503, &bad_body_2byte);
    }));

    // EMPIRICAL OBSERVATION & ASSERTION:
    // All 3 cases must succeed without panicking after remediation!
    assert!(
        panic_result_4byte.is_ok(),
        "Remediation verified: map_http_error did not panic on 4-byte UTF-8 boundary"
    );
    assert!(
        panic_result_3byte.is_ok(),
        "Remediation verified: map_http_error did not panic on 3-byte UTF-8 boundary"
    );
    assert!(
        panic_result_2byte.is_ok(),
        "Remediation verified: map_http_error did not panic on 2-byte UTF-8 boundary"
    );
}

// =========================================================================
// Category 2: HTTP Status Error Mapping (401, 403, 404, 429, 500, 502, 503)
// =========================================================================

#[test]
fn test_empirical_http_status_codes_mapping() {
    let client = OpenAiCompatibleClient::new(OpenAiConfig::ollama("llama3:8b")).unwrap();

    // 1. 401 Unauthorized -> ProviderUnavailable
    let err401 = client.map_http_error(401, "Unauthorized");
    assert!(
        matches!(err401, CoachError::ProviderUnavailable { ref provider, .. } if provider == "openai_compatible")
    );
    assert!(!err401.is_retryable());
    assert!(err401.should_fallback());

    // 2. 403 Forbidden -> ProviderUnavailable
    let err403 = client.map_http_error(403, "Forbidden");
    assert!(
        matches!(err403, CoachError::ProviderUnavailable { ref provider, .. } if provider == "openai_compatible")
    );
    assert!(!err403.is_retryable());
    assert!(err403.should_fallback());

    // 3. 404 Not Found -> HttpError 404
    let err404 = client.map_http_error(404, "Model not found");
    assert!(matches!(
        err404,
        CoachError::HttpError {
            status_code: 404,
            ..
        }
    ));
    assert!(!err404.is_retryable());
    assert!(err404.should_fallback());

    // 4. 429 Too Many Requests -> HttpError 429
    let err429 = client.map_http_error(429, "Rate limit exceeded");
    assert!(matches!(
        err429,
        CoachError::HttpError {
            status_code: 429,
            ..
        }
    ));
    assert!(err429.is_retryable()); // 429 must be retryable
    assert!(err429.should_fallback());

    // 5. 500 Internal Server Error -> HttpError 500
    let err500 = client.map_http_error(500, "Internal Server Error");
    assert!(matches!(
        err500,
        CoachError::HttpError {
            status_code: 500,
            ..
        }
    ));
    assert!(err500.is_retryable()); // 500 must be retryable
    assert!(err500.should_fallback());

    // 6. 502 Bad Gateway -> HttpError 502
    let err502 = client.map_http_error(502, "Bad Gateway");
    assert!(matches!(
        err502,
        CoachError::HttpError {
            status_code: 502,
            ..
        }
    ));
    assert!(err502.is_retryable()); // 502 must be retryable
    assert!(err502.should_fallback());

    // 7. 503 Service Unavailable -> HttpError 503
    let err503 = client.map_http_error(503, "Service Unavailable");
    assert!(matches!(
        err503,
        CoachError::HttpError {
            status_code: 503,
            ..
        }
    ));
    assert!(err503.is_retryable()); // 503 must be retryable
    assert!(err503.should_fallback());

    // 8. 504 Gateway Timeout -> HttpError 504
    let err504 = client.map_http_error(504, "Gateway Timeout");
    assert!(matches!(
        err504,
        CoachError::HttpError {
            status_code: 504,
            ..
        }
    ));
    assert!(err504.is_retryable()); // 504 must be retryable
    assert!(err504.should_fallback());
}

// =========================================================================
// Category 3: JSON Error Body Variations in `map_http_error`
// =========================================================================

#[test]
fn test_empirical_http_error_json_body_parsing() {
    let client = OpenAiCompatibleClient::new(OpenAiConfig::ollama("llama3:8b")).unwrap();

    // 1. Standard OpenAI error structure: `{"error": {"message": "...", "type": "..."}}`
    let body_standard = r#"{
        "error": {
            "message": "You exceeded your current quota, please check your plan and billing details.",
            "type": "insufficient_quota",
            "param": null,
            "code": "insufficient_quota"
        }
    }"#;
    let err = client.map_http_error(429, body_standard);
    if let CoachError::HttpError { message, .. } = err {
        assert!(message.contains("You exceeded your current quota"));
        assert!(!message.contains("insufficient_quota")); // extracted message only
    } else {
        panic!("Expected HttpError");
    }

    // 2. Direct string error: `{"error": "Direct string error message"}`
    let body_string = r#"{"error": "Custom gateway rejection"}"#;
    let err = client.map_http_error(500, body_string);
    if let CoachError::HttpError { message, .. } = err {
        assert_eq!(message, "Custom gateway rejection");
    } else {
        panic!("Expected HttpError");
    }

    // 3. Null error object: `{"error": null}`
    let body_null = r#"{"error": null}"#;
    let err = client.map_http_error(500, body_null);
    if let CoachError::HttpError { message, .. } = err {
        assert_eq!(message, body_null);
    } else {
        panic!("Expected HttpError");
    }

    // 4. HTML error body (Nginx/Cloudflare 502 Bad Gateway)
    let body_html = "<html><body><h1>502 Bad Gateway</h1></body></html>";
    let err = client.map_http_error(502, body_html);
    if let CoachError::HttpError { message, .. } = err {
        assert_eq!(message, body_html);
    } else {
        panic!("Expected HttpError");
    }

    // 5. Completely empty response body
    let err = client.map_http_error(503, "");
    if let CoachError::HttpError { message, .. } = err {
        assert_eq!(message, "");
    } else {
        panic!("Expected HttpError");
    }
}

// =========================================================================
// Category 4: Malformed Outer HTTP Response Bodies
// =========================================================================

#[test]
fn test_empirical_malformed_outer_http_response_bodies() {
    let client = OpenAiCompatibleClient::new(OpenAiConfig::ollama("llama3:8b")).unwrap();

    // 1. Truncated wire JSON tokens
    let res = client.parse_completion_response(r#"{"id": "chat-1", "choices": [{"message": {"c"#);
    assert!(matches!(res.unwrap_err(), CoachError::MalformedJson { .. }));

    // 2. Missing 'choices' field
    let res = client.parse_completion_response(r#"{"id": "chat-1", "model": "test"}"#);
    assert!(matches!(
        res.unwrap_err(),
        CoachError::SchemaValidation { field, .. } if field == "choices"
    ));

    // 3. Empty 'choices' array
    let res = client.parse_completion_response(r#"{"id": "chat-1", "choices": []}"#);
    assert!(matches!(
        res.unwrap_err(),
        CoachError::SchemaValidation { field, .. } if field == "choices"
    ));

    // 4. 'choices' is an object or number instead of an array
    let res = client.parse_completion_response(r#"{"id": "chat-1", "choices": 42}"#);
    assert!(matches!(
        res.unwrap_err(),
        CoachError::SchemaValidation { field, .. } if field == "choices"
    ));

    // 5. First choice missing 'message'
    let res = client.parse_completion_response(r#"{"choices": [{"index": 0}]}"#);
    assert!(matches!(
        res.unwrap_err(),
        CoachError::SchemaValidation { field, .. } if field == "message"
    ));

    // 6. Choice message missing 'content'
    let res =
        client.parse_completion_response(r#"{"choices": [{"message": {"role": "assistant"}}]}"#);
    assert!(matches!(
        res.unwrap_err(),
        CoachError::SchemaValidation { field, .. } if field == "content"
    ));

    // 7. Choice message content is null (OpenAI tool calls mode)
    let res = client.parse_completion_response(
        r#"{"choices": [{"message": {"role": "assistant", "content": null}}]}"#,
    );
    assert!(matches!(
        res.unwrap_err(),
        CoachError::SchemaValidation { field, .. } if field == "content"
    ));
}

// =========================================================================
// Category 5: Malformed Inner `message.content` Payloads
// =========================================================================

#[test]
fn test_empirical_malformed_inner_content_payloads() {
    let client = OpenAiCompatibleClient::new(OpenAiConfig::ollama("llama3:8b")).unwrap();

    let build_envelope = |inner_content: &str| -> String {
        serde_json::json!({
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": inner_content
                }
            }]
        })
        .to_string()
    };

    // 1. Truncated JSON inside content
    let env = build_envelope(r#"{"relevance": "relevant", "confidence": 0."#);
    let err = client.parse_completion_response(&env).unwrap_err();
    assert!(matches!(err, CoachError::MalformedJson { .. }));

    // 2. Plain text refusal
    let env = build_envelope("I cannot evaluate this activity due to safety policies.");
    let err = client.parse_completion_response(&env).unwrap_err();
    assert!(matches!(err, CoachError::MalformedJson { .. }));

    // 3. Missing required field 'relevance'
    let env = build_envelope(r#"{"confidence": 0.9, "rationale": "Coding active"}"#);
    let err = client.parse_completion_response(&env).unwrap_err();
    assert!(matches!(
        err,
        CoachError::SchemaValidation { field, .. } if field == "relevance"
    ));

    // 4. Missing required field 'confidence'
    let env = build_envelope(r#"{"relevance": "relevant", "rationale": "Coding active"}"#);
    let err = client.parse_completion_response(&env).unwrap_err();
    assert!(matches!(
        err,
        CoachError::SchemaValidation { field, .. } if field == "confidence"
    ));

    // 5. Confidence out of bounds (1.5)
    let env = build_envelope(
        r#"{"relevance": "relevant", "confidence": 1.5, "rationale": "Coding active"}"#,
    );
    let err = client.parse_completion_response(&env).unwrap_err();
    assert!(matches!(
        err,
        CoachError::SchemaValidation { field, .. } if field == "confidence"
    ));

    // 6. Missing required field 'rationale'
    let env = build_envelope(r#"{"relevance": "relevant", "confidence": 0.8}"#);
    let err = client.parse_completion_response(&env).unwrap_err();
    assert!(matches!(
        err,
        CoachError::SchemaValidation { field, .. } if field == "rationale"
    ));

    // 7. Empty rationale
    let env = build_envelope(r#"{"relevance": "relevant", "confidence": 0.8, "rationale": "   "}"#);
    let err = client.parse_completion_response(&env).unwrap_err();
    assert!(matches!(
        err,
        CoachError::SchemaValidation { field, .. } if field == "rationale"
    ));

    // 8. Disallowed action candidate attack
    let env = build_envelope(
        r#"{"relevance": "possibly_unrelated", "confidence": 0.9, "rationale": "Attack", "action_candidate": "kill_process"}"#,
    );
    let err = client.parse_completion_response(&env).unwrap_err();
    assert!(matches!(err, CoachError::DisallowedAction { .. }));
}

// =========================================================================
// Category 6: Truncated Wire Streams & Abrupt Socket Termination
// =========================================================================

#[tokio::test]
async fn test_empirical_truncated_wire_body_abrupt_disconnect() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 1024];
            let _ = socket.read(&mut buf).await;

            // Send HTTP headers stating Content-Length: 1000, but only send 40 bytes then drop socket!
            let partial_resp = "HTTP/1.1 200 OK\r\nContent-Length: 1000\r\nContent-Type: application/json\r\n\r\n{\"choices\": [{\"message\": {\"content\": ";
            let _ = socket.write_all(partial_resp.as_bytes()).await;
            let _ = socket.flush().await;
            // Dropping `socket` sends TCP FIN, truncating response body
        }
    });

    let config = OpenAiConfig::new(
        format!("http://{addr}/v1/chat/completions"),
        "test-truncated-model",
    )
    .with_timeout(Duration::from_secs(2));

    let client = OpenAiCompatibleClient::new(config).unwrap();
    let res = client.evaluate(&sample_prompt()).await;

    // Must return an error, never panic!
    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(matches!(err, CoachError::NetworkError { .. }));
    assert!(err.should_fallback());
}

// =========================================================================
// Category 7: Connection Refusal & Aborted Sockets
// =========================================================================

#[tokio::test]
async fn test_empirical_connection_refusal_dead_port() {
    // Bind and immediately drop listener to find an unused closed port
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let closed_addr = listener.local_addr().unwrap();
    drop(listener);

    let config = OpenAiConfig::new(
        format!("http://{closed_addr}/v1/chat/completions"),
        "test-refused-model",
    )
    .with_timeout(Duration::from_secs(1));

    let client = OpenAiCompatibleClient::new(config).unwrap();
    let res = client.evaluate(&sample_prompt()).await;

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(matches!(err, CoachError::NetworkError { .. }));
    if let CoachError::NetworkError { message } = &err {
        assert!(message.contains("Failed to connect"));
    }
    assert!(err.should_fallback());
}

// =========================================================================
// Category 8: Request Timeout Handling
// =========================================================================

#[tokio::test]
async fn test_empirical_timeout_handling() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        if let Ok((mut socket, _)) = listener.accept().await {
            let mut buf = [0u8; 1024];
            let _ = socket.read(&mut buf).await;
            // Sleep longer than the client's timeout (200ms vs 50ms)
            tokio::time::sleep(Duration::from_millis(200)).await;
            let _ = socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
                .await;
        }
    });

    let config = OpenAiConfig::new(
        format!("http://{addr}/v1/chat/completions"),
        "test-timeout-model",
    )
    .with_timeout(Duration::from_millis(50));

    let client = OpenAiCompatibleClient::new(config).unwrap();
    let start = std::time::Instant::now();
    let res = client.evaluate(&sample_prompt()).await;
    let elapsed = start.elapsed();

    assert!(res.is_err());
    let err = res.unwrap_err();
    assert!(
        matches!(err, CoachError::Timeout { duration_ms: 50 }),
        "Expected timeout of 50ms, got: {err:?}"
    );
    assert!(err.is_retryable());
    assert!(err.should_fallback());
    assert!(elapsed < Duration::from_millis(180));
}

// =========================================================================
// Category 9: Exhaustive `DeterministicFallback` Recovery Matrix
// =========================================================================

#[test]
fn test_empirical_deterministic_fallback_comprehensive_matrix() {
    let fallback = DeterministicFallback::with_provider("openai_compatible");
    let now = fixed_timestamp();

    let test_cases: Vec<(CoachError, FallbackReason, bool)> = vec![
        // 1. Timeouts
        (CoachError::timeout(100), FallbackReason::Timeout, false),
        (CoachError::timeout(15000), FallbackReason::Timeout, false),
        (
            CoachError::timeout(u64::MAX),
            FallbackReason::Timeout,
            false,
        ),
        // 2. Network errors
        (
            CoachError::network("Connection refused"),
            FallbackReason::NetworkError,
            false,
        ),
        (
            CoachError::network(
                "DNS resolution failure for internal-api.corp.local:9000 (10.0.1.5)",
            ),
            FallbackReason::NetworkError,
            false,
        ),
        (
            CoachError::network("a".repeat(2000)),
            FallbackReason::NetworkError,
            false,
        ),
        // 3. HTTP status errors
        (
            CoachError::http(400, "Bad Request"),
            FallbackReason::HttpError,
            false,
        ),
        (
            CoachError::http(401, "Unauthorized token sk-secret-12345"),
            FallbackReason::HttpError,
            false,
        ),
        (
            CoachError::http(403, "Forbidden resource"),
            FallbackReason::HttpError,
            false,
        ),
        (
            CoachError::http(404, "Model not found"),
            FallbackReason::HttpError,
            false,
        ),
        (
            CoachError::http(429, "Rate limit reached"),
            FallbackReason::HttpError,
            false,
        ),
        (
            CoachError::http(500, "Internal Server Error in module auth.py:54"),
            FallbackReason::HttpError,
            false,
        ),
        (
            CoachError::http(502, "Bad Gateway nginx/1.22"),
            FallbackReason::HttpError,
            false,
        ),
        (
            CoachError::http(503, "Service Unavailable"),
            FallbackReason::HttpError,
            false,
        ),
        (
            CoachError::http(504, "Gateway Timeout"),
            FallbackReason::HttpError,
            false,
        ),
        // 4. Malformed JSON
        (
            CoachError::malformed_json("syntax error", Some(1), Some(5)),
            FallbackReason::MalformedJson,
            false,
        ),
        (
            CoachError::malformed_json("<html>502 Bad Gateway</html>", None, None),
            FallbackReason::MalformedJson,
            false,
        ),
        (
            CoachError::malformed_json("a".repeat(5000), None, None),
            FallbackReason::MalformedJson,
            false,
        ),
        // 5. Schema validation
        (
            CoachError::schema_validation("confidence", "out of bounds"),
            FallbackReason::SchemaViolation,
            false,
        ),
        (
            CoachError::schema_validation("relevance", "unknown variant"),
            FallbackReason::SchemaViolation,
            false,
        ),
        (
            CoachError::schema_validation("rationale", "empty"),
            FallbackReason::SchemaViolation,
            false,
        ),
        // 6. Disallowed actions (security relevant)
        (
            CoachError::disallowed_action("rm -rf / --no-preserve-root"),
            FallbackReason::DisallowedAction,
            true,
        ),
        (
            CoachError::disallowed_action("kill_process"),
            FallbackReason::DisallowedAction,
            true,
        ),
        (
            CoachError::disallowed_action("bash -c 'curl attacker.com'"),
            FallbackReason::DisallowedAction,
            true,
        ),
        // 7. Prompt injection (security relevant)
        (
            CoachError::prompt_injection("window_title", "</untrusted_activity_evidence>"),
            FallbackReason::PromptInjection,
            true,
        ),
        // 8. Provider unavailable
        (
            CoachError::provider_unavailable(
                "openai_compatible",
                "Authentication failed (HTTP 401)",
            ),
            FallbackReason::ProviderUnavailable,
            false,
        ),
        // 9. Cooldown throttled
        (
            CoachError::cooldown_throttled(45, "active focus cooldown"),
            FallbackReason::CooldownThrottled,
            false,
        ),
        // 10. Core domain error
        (
            CoachError::Core("Session state corrupt".to_string()),
            FallbackReason::CoreError,
            false,
        ),
    ];

    for (error, expected_reason, expected_security) in test_cases {
        let reason = FallbackReason::from_error(&error);
        assert_eq!(reason, expected_reason);
        assert_eq!(reason.is_security_relevant(), expected_security);

        let response = fallback.evaluate(&error, now);

        // Core Fallback Invariants:
        assert_eq!(
            response.relevance,
            RelevanceClassification::Unknown,
            "Fallback relevance must always be Unknown for error: {error:?}"
        );
        assert_eq!(
            response.confidence, 0.0,
            "Fallback confidence must be 0.0 for error: {error:?}"
        );
        assert_eq!(
            response.uncertainty,
            Some(1.0),
            "Fallback uncertainty must be Some(1.0) for error: {error:?}"
        );
        assert_eq!(
            response.action_candidate,
            Some(ActionCandidate::None),
            "Fallback action_candidate must be Some(None) for error: {error:?}"
        );
        assert_eq!(
            response.effective_action(),
            ActionCandidate::None,
            "Effective action must be None"
        );
        assert!(
            !response.has_action(),
            "Fallback response must never have an active action"
        );
        assert!(response.question.is_none(), "Question must be None");

        // Rationale safety invariants:
        assert!(!response.rationale.is_empty(), "Rationale cannot be empty");
        assert!(
            response.rationale.chars().count() <= 1000,
            "Rationale length cannot exceed MAX_RATIONALE_LENGTH"
        );

        // Leakage protection invariants:
        assert!(
            !response.rationale.contains("sk-secret-12345"),
            "Rationale must not leak API key"
        );
        assert!(
            !response.rationale.contains("10.0.1.5"),
            "Rationale must not leak IP address"
        );
        assert!(
            !response.rationale.contains("internal-api.corp.local"),
            "Rationale must not leak internal hostname"
        );
        assert!(
            !response.rationale.contains("rm -rf"),
            "Rationale must not leak malicious command"
        );
        assert!(
            !response.rationale.contains("auth.py"),
            "Rationale must not leak source file or stack trace"
        );

        // Provenance invariants:
        assert_eq!(response.provenance.provider_name, "openai_compatible");
        assert_eq!(response.provenance.model_name, "deterministic-fallback");
        assert_eq!(response.provenance.generated_at, now);
        assert_eq!(response.provenance.latency_ms, Some(0));

        // Serialization round-trip test:
        let serialized = serde_json::to_string(&response).expect("Must serialize to JSON");
        let deserialized: CoachResponse =
            serde_json::from_str(&serialized).expect("Must deserialize from JSON");
        assert_eq!(deserialized, response);
    }
}

// =========================================================================
// Category 10: Fallback Provider Provenance Boundary Checks
// =========================================================================

#[test]
fn test_empirical_fallback_provenance_round_trip() {
    let fallback_default = DeterministicFallback::new();
    let now = fixed_timestamp();

    let resp = fallback_default.evaluate(&CoachError::timeout(500), now);
    assert_eq!(resp.provenance.provider_name, "fallback");
    assert_eq!(resp.provenance.model_name, "deterministic-fallback");

    let custom_fb = DeterministicFallback::with_provider("custom_ollama_engine");
    let resp_custom = custom_fb.evaluate(&CoachError::timeout(500), now);
    assert_eq!(resp_custom.provenance.provider_name, "custom_ollama_engine");

    let json = serde_json::to_string(&resp_custom).unwrap();
    let back: CoachResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(back.provenance.provider_name, "custom_ollama_engine");
}
