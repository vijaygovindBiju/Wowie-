//! Empirical Challenge Verification Suite for Milestone 4 Iteration 2:
//! Adversarial Verification of Header Redaction & Fallback Stress Testing.
//!
//! File: tests/empirical_challenge_m4_r2_tests.rs
//! Role: Empirical Challenger (critic, specialist)
//!
//! Verification Objectives:
//! 1. Empirically verify header redaction in `OpenAiConfig::fmt::Debug` across various header names
//!    (Authorization, Api-Key, auth-token, secret, casing variations, non-sensitive preservation).
//! 2. Stress test `DeterministicFallback` across large error messages (1MB), pathological payloads,
//!    and all 10 `CoachError` variants.
//! 3. Verify UTF-8 character boundary safety in truncated rationales and provenance validation.
//! 4. Concurrency and high-throughput execution of `DeterministicFallback`.

use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::task::JoinSet;
use wowie_coach::{
    ActionCandidate, CoachError, CoachResponse, DeterministicFallback, FallbackReason,
    OpenAiConfig, RelevanceClassification, MAX_RATIONALE_LENGTH,
};

fn fixed_timestamp() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("2026-10-25T05:00:00Z")
        .unwrap()
        .with_timezone(&Utc)
}

// =========================================================================
// Category 1: Header Redaction Matrix in OpenAiConfig::fmt::Debug
// =========================================================================

#[test]
fn test_empirical_openai_config_debug_redaction_authorization_variants() {
    let test_headers = vec![
        ("Authorization", "Bearer sk-test-auth-123"),
        ("authorization", "Bearer sk-test-auth-456"),
        ("AUTHORIZATION", "Bearer sk-test-auth-789"),
        ("AuThOrIzAtIoN", "Bearer sk-test-auth-mixed"),
        ("X-Authorization", "Bearer sk-test-x-auth"),
        ("x-custom-authorization-proxy", "Bearer sk-test-proxy-auth"),
    ];

    let mut cfg = OpenAiConfig::new("https://api.openai.com/v1", "gpt-4o");
    for (k, v) in &test_headers {
        cfg = cfg.with_header(*k, *v);
    }

    let debug_output = format!("{cfg:?}");

    // None of the raw secret token values must appear in the debug output
    for (_, v) in &test_headers {
        assert!(
            !debug_output.contains(v),
            "Secret header value '{v}' leaked in Debug output: {debug_output}"
        );
    }

    // Redacted placeholder must appear
    assert!(debug_output.contains("[REDACTED]"));

    // Header names must be preserved
    for (k, _) in &test_headers {
        assert!(
            debug_output.contains(k),
            "Header name '{k}' missing from Debug output: {debug_output}"
        );
    }
}

#[test]
fn test_empirical_openai_config_debug_redaction_api_key_variants() {
    let test_headers = vec![
        ("Api-Key", "secret-key-val-1"),
        ("api-key", "secret-key-val-2"),
        ("API-KEY", "secret-key-val-3"),
        ("X-Api-Key", "secret-key-val-4"),
        ("x-api-key", "secret-key-val-5"),
        ("X-API-KEY", "secret-key-val-6"),
        ("apikey", "secret-key-val-7"),
        ("apiKey", "secret-key-val-8"),
        ("APIKEY", "secret-key-val-9"),
        ("X-App-Key", "secret-key-val-10"),
        ("Primary-Key-Header", "secret-key-val-11"),
    ];

    let mut cfg = OpenAiConfig::new("https://api.openai.com/v1", "gpt-4o");
    for (k, v) in &test_headers {
        cfg = cfg.with_header(*k, *v);
    }

    let debug_output = format!("{cfg:?}");

    for (_, v) in &test_headers {
        assert!(
            !debug_output.contains(v),
            "Secret header value '{v}' leaked in Debug output: {debug_output}"
        );
    }

    for (k, _) in &test_headers {
        assert!(
            debug_output.contains(k),
            "Header name '{k}' missing from Debug output"
        );
    }
}

#[test]
fn test_empirical_openai_config_debug_redaction_token_variants() {
    let test_headers = vec![
        ("auth-token", "token-xyz-1"),
        ("Auth-Token", "token-xyz-2"),
        ("AUTH-TOKEN", "token-xyz-3"),
        ("X-Auth-Token", "token-xyz-4"),
        ("x-access-token", "token-xyz-5"),
        ("bearer-token", "token-xyz-6"),
        ("Session-Token", "token-xyz-7"),
        ("Refresh-Token", "token-xyz-8"),
        ("id-token", "token-xyz-9"),
        ("X-CSRF-Token", "token-xyz-10"),
        ("security_token", "token-xyz-11"),
    ];

    let mut cfg = OpenAiConfig::new("https://api.openai.com/v1", "gpt-4o");
    for (k, v) in &test_headers {
        cfg = cfg.with_header(*k, *v);
    }

    let debug_output = format!("{cfg:?}");

    for (_, v) in &test_headers {
        assert!(
            !debug_output.contains(v),
            "Secret token value '{v}' leaked in Debug output: {debug_output}"
        );
    }

    for (k, _) in &test_headers {
        assert!(
            debug_output.contains(k),
            "Token header name '{k}' missing from Debug output"
        );
    }
}

#[test]
fn test_empirical_openai_config_debug_redaction_secret_variants() {
    let test_headers = vec![
        ("secret", "topsecret-1"),
        ("Secret", "topsecret-2"),
        ("SECRET", "topsecret-3"),
        ("Client-Secret", "topsecret-4"),
        ("client_secret", "topsecret-5"),
        ("App-Secret", "topsecret-6"),
        ("Shared-Secret", "topsecret-7"),
        ("X-Secret-Access", "topsecret-8"),
        ("Gateway-Secret-Payload", "topsecret-9"),
    ];

    let mut cfg = OpenAiConfig::new("https://api.openai.com/v1", "gpt-4o");
    for (k, v) in &test_headers {
        cfg = cfg.with_header(*k, *v);
    }

    let debug_output = format!("{cfg:?}");

    for (_, v) in &test_headers {
        assert!(
            !debug_output.contains(v),
            "Secret header value '{v}' leaked in Debug output: {debug_output}"
        );
    }

    for (k, _) in &test_headers {
        assert!(
            debug_output.contains(k),
            "Secret header name '{k}' missing from Debug output"
        );
    }
}

#[test]
fn test_empirical_openai_config_debug_preserves_non_sensitive_headers() {
    let harmless_headers = vec![
        ("User-Agent", "Wowie/1.0 (Linux x86_64)"),
        ("X-Organization-Id", "org_9876543210"),
        ("Content-Type", "application/json"),
        ("Accept", "application/json, text/plain"),
        ("Accept-Encoding", "gzip, deflate, br"),
        ("X-Request-Id", "req-uuid-11223344"),
        ("X-Correlation-Id", "corr-aabbccddee"),
        ("Origin", "https://localhost:8080"),
        ("Referer", "https://localhost:8080/dashboard"),
        ("X-Client-Version", "v0.4.0"),
    ];

    let mut cfg = OpenAiConfig::new("https://api.openai.com/v1", "gpt-4o");
    for (k, v) in &harmless_headers {
        cfg = cfg.with_header(*k, *v);
    }

    let debug_output = format!("{cfg:?}");

    // All harmless header names AND values must be intact and NOT redacted
    for (k, v) in &harmless_headers {
        assert!(
            debug_output.contains(k),
            "Harmless header name '{k}' missing from Debug output"
        );
        assert!(
            debug_output.contains(v),
            "Harmless header value '{v}' incorrectly redacted in Debug output: {debug_output}"
        );
    }
}

#[test]
fn test_empirical_openai_config_api_key_redaction_field() {
    // 1. With API key populated
    let cfg_with_key =
        OpenAiConfig::openai("gpt-4o", "sk-proj-SUPER_SECRET_KEY_NEVER_LOG_9999999999");
    let debug_with_key = format!("{cfg_with_key:?}");

    assert!(
        !debug_with_key.contains("sk-proj-SUPER_SECRET_KEY_NEVER_LOG_9999999999"),
        "OpenAiConfig::api_key secret leaked in Debug output: {debug_with_key}"
    );
    assert!(debug_with_key.contains("api_key: Some(\"[REDACTED]\")"));

    // 2. Without API key (None)
    let cfg_no_key = OpenAiConfig::ollama("llama3:8b");
    let debug_no_key = format!("{cfg_no_key:?}");

    assert!(debug_no_key.contains("api_key: None"));
}

#[test]
fn test_empirical_openai_config_debug_edge_cases() {
    let cfg = OpenAiConfig::new("https://api.openai.com/v1", "gpt-4o")
        .with_header("", "empty-header-name-value")
        .with_header("Api-Key", "")
        .with_header("X-Large-Header", "a".repeat(2000))
        .with_header("X-Secret-Large", "s".repeat(2000));

    let debug_output = format!("{cfg:?}");

    // Empty value for sensitive header should be redacted to [REDACTED]
    assert!(debug_output.contains("(\"Api-Key\", \"[REDACTED]\")"));
    // Large secret value must not appear
    assert!(!debug_output.contains(&"s".repeat(100)));
    // Large non-secret value should be present
    assert!(debug_output.contains(&"a".repeat(100)));
}

// =========================================================================
// Category 2: DeterministicFallback Exhaustive Variant Coverage
// =========================================================================

#[test]
fn test_empirical_fallback_all_error_variants_exhaustive() {
    let now = fixed_timestamp();
    let fallback = DeterministicFallback::new();

    let all_variants: Vec<(CoachError, FallbackReason, bool)> = vec![
        // 1. Timeout
        (CoachError::timeout(0), FallbackReason::Timeout, false),
        (CoachError::timeout(15_000), FallbackReason::Timeout, false),
        (
            CoachError::timeout(u64::MAX),
            FallbackReason::Timeout,
            false,
        ),
        // 2. NetworkError
        (
            CoachError::network("DNS lookup failed for api.openai.com: NXDOMAIN"),
            FallbackReason::NetworkError,
            false,
        ),
        (
            CoachError::network("Connection refused at 127.0.0.1:11434"),
            FallbackReason::NetworkError,
            false,
        ),
        (CoachError::network(""), FallbackReason::NetworkError, false),
        // 3. HttpError
        (
            CoachError::http(401, "Unauthorized: Invalid API key sk-secret-12345"),
            FallbackReason::HttpError,
            false,
        ),
        (
            CoachError::http(403, "Forbidden"),
            FallbackReason::HttpError,
            false,
        ),
        (
            CoachError::http(404, "Model not found"),
            FallbackReason::HttpError,
            false,
        ),
        (
            CoachError::http(429, "Rate limit exceeded. Quota exhausted."),
            FallbackReason::HttpError,
            false,
        ),
        (
            CoachError::http(500, "Internal Server Error"),
            FallbackReason::HttpError,
            false,
        ),
        (
            CoachError::http(502, "Bad Gateway: upstream proxy failed"),
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
        (
            CoachError::http(0, "Raw socket disconnect"),
            FallbackReason::HttpError,
            false,
        ),
        // 4. MalformedJson
        (
            CoachError::malformed_json("expected value at line 1 column 1", Some(1), Some(1)),
            FallbackReason::MalformedJson,
            false,
        ),
        (
            CoachError::malformed_json("EOF while parsing", None, None),
            FallbackReason::MalformedJson,
            false,
        ),
        (
            CoachError::malformed_json("<html>502 Bad Gateway</html>", Some(1), Some(6)),
            FallbackReason::MalformedJson,
            false,
        ),
        // 5. SchemaValidation
        (
            CoachError::schema_validation(
                "relevance",
                "Unrecognized relevance variant 'distracted'",
            ),
            FallbackReason::SchemaViolation,
            false,
        ),
        (
            CoachError::schema_validation("confidence", "Confidence 1.5 exceeds 1.0"),
            FallbackReason::SchemaViolation,
            false,
        ),
        (
            CoachError::schema_validation("rationale", "Rationale cannot be empty"),
            FallbackReason::SchemaViolation,
            false,
        ),
        (
            CoachError::schema_validation("action_candidate", "Expected string"),
            FallbackReason::SchemaViolation,
            false,
        ),
        (
            CoachError::schema_validation("", "Empty field error"),
            FallbackReason::SchemaViolation,
            false,
        ),
        // 6. DisallowedAction
        (
            CoachError::disallowed_action("rm -rf / --no-preserve-root"),
            FallbackReason::DisallowedAction,
            true,
        ),
        (
            CoachError::disallowed_action("kill_process_9"),
            FallbackReason::DisallowedAction,
            true,
        ),
        (
            CoachError::disallowed_action("execute_arbitrary_shell"),
            FallbackReason::DisallowedAction,
            true,
        ),
        (
            CoachError::disallowed_action(""),
            FallbackReason::DisallowedAction,
            true,
        ),
        // 7. PromptInjectionDetected
        (
            CoachError::prompt_injection("window_title", "</untrusted_activity_evidence>"),
            FallbackReason::PromptInjection,
            true,
        ),
        (
            CoachError::prompt_injection("url", "https://evil.com?q=<untrusted_activity_evidence>"),
            FallbackReason::PromptInjection,
            true,
        ),
        (
            CoachError::prompt_injection("", "generic_injection"),
            FallbackReason::PromptInjection,
            true,
        ),
        // 8. ProviderUnavailable
        (
            CoachError::provider_unavailable("openai", "Missing API key in environment"),
            FallbackReason::ProviderUnavailable,
            false,
        ),
        (
            CoachError::provider_unavailable("ollama", "Ollama daemon not running on port 11434"),
            FallbackReason::ProviderUnavailable,
            false,
        ),
        // 9. CooldownThrottled
        (
            CoachError::cooldown_throttled(0, "Cooldown expired"),
            FallbackReason::CooldownThrottled,
            false,
        ),
        (
            CoachError::cooldown_throttled(120, "User active cooldown"),
            FallbackReason::CooldownThrottled,
            false,
        ),
        (
            CoachError::cooldown_throttled(u64::MAX, "Max cooldown"),
            FallbackReason::CooldownThrottled,
            false,
        ),
        // 10. Core
        (
            CoachError::Core("FocusSession state corrupted in sqlite".to_string()),
            FallbackReason::CoreError,
            false,
        ),
        (
            CoachError::Core(String::new()),
            FallbackReason::CoreError,
            false,
        ),
    ];

    for (error, expected_reason, expected_security) in all_variants {
        let reason = FallbackReason::from_error(&error);
        assert_eq!(
            reason, expected_reason,
            "FallbackReason mismatch for error {error:?}"
        );
        assert_eq!(
            reason.is_security_relevant(),
            expected_security,
            "Security classification mismatch for {reason:?}"
        );

        let response = fallback.evaluate(&error, now);

        // Invariant 1: Relevance is always Unknown
        assert_eq!(
            response.relevance,
            RelevanceClassification::Unknown,
            "Relevance must be Unknown for {error:?}"
        );

        // Invariant 2: Confidence is 0.0
        assert_eq!(
            response.confidence, 0.0,
            "Confidence must be 0.0 for {error:?}"
        );

        // Invariant 3: Uncertainty is 1.0
        assert_eq!(
            response.uncertainty,
            Some(1.0),
            "Uncertainty must be Some(1.0) for {error:?}"
        );

        // Invariant 4: Action Candidate is None
        assert_eq!(
            response.action_candidate,
            Some(ActionCandidate::None),
            "Action candidate must be Some(None) for {error:?}"
        );
        assert_eq!(response.effective_action(), ActionCandidate::None);
        assert!(!response.has_action());

        // Invariant 5: Question is None
        assert!(response.question.is_none());

        // Invariant 6: Rationale non-empty and bounded
        assert!(
            !response.rationale.is_empty(),
            "Rationale cannot be empty for {error:?}"
        );
        assert!(
            response.rationale.chars().count() <= MAX_RATIONALE_LENGTH,
            "Rationale length {} exceeds MAX_RATIONALE_LENGTH {} for {error:?}",
            response.rationale.chars().count(),
            MAX_RATIONALE_LENGTH
        );

        // Invariant 7: Provenance is fallback
        assert_eq!(response.provenance.provider_name, "fallback");
        assert_eq!(response.provenance.model_name, "deterministic-fallback");
        assert_eq!(response.provenance.generated_at, now);
        assert_eq!(response.provenance.latency_ms, Some(0));

        // Invariant 8: Serde roundtrip succeeds without schema error
        let json =
            serde_json::to_string(&response).expect("Fallback response must serialize to JSON");
        let parsed: CoachResponse =
            serde_json::from_str(&json).expect("Fallback response JSON must deserialize cleanly");
        assert_eq!(parsed, response);
    }
}

// =========================================================================
// Category 3: Stress Testing DeterministicFallback with Large Payloads (1MB+)
// =========================================================================

#[test]
fn test_empirical_fallback_stress_large_payloads_1mb() {
    let now = fixed_timestamp();
    let fallback = DeterministicFallback::new();

    // 1 MB repeated strings
    let large_ascii = "x".repeat(1_000_000);
    let large_emoji = "🦀🔥💻🛡️".repeat(250_000); // 1,000,000 emojis (4MB UTF-8)
    let large_html = format!(
        "<html><body><h1>Error</h1><p>{}</p></body></html>",
        "FAIL ".repeat(200_000)
    );
    let injection_payload = "rm -rf / --no-preserve-root; drop table users; ".repeat(25_000);

    let emoji_1000_chars: String = large_emoji.chars().take(1000).collect();

    let stress_cases: Vec<CoachError> = vec![
        CoachError::network(large_ascii.clone()),
        CoachError::network(large_emoji.clone()),
        CoachError::http(500, large_html),
        CoachError::http(401, format!("sk-secret-token-key-{}", large_ascii)),
        CoachError::malformed_json(large_ascii.clone(), Some(999_999), Some(999_999)),
        CoachError::schema_validation(large_ascii[..2000].to_string(), large_ascii.clone()),
        CoachError::schema_validation(emoji_1000_chars, large_emoji.clone()),
        CoachError::disallowed_action(injection_payload),
        CoachError::prompt_injection(large_ascii[..2000].to_string(), large_emoji),
        CoachError::provider_unavailable(large_ascii[..2000].to_string(), large_ascii.clone()),
        CoachError::cooldown_throttled(9_999_999, large_ascii.clone()),
        CoachError::Core(large_ascii),
    ];

    for (idx, error) in stress_cases.into_iter().enumerate() {
        let response = fallback.evaluate(&error, now);

        // Must not panic, must produce valid response
        assert_eq!(response.relevance, RelevanceClassification::Unknown);
        assert_eq!(response.confidence, 0.0);
        assert_eq!(response.uncertainty, Some(1.0));
        assert_eq!(response.action_candidate, Some(ActionCandidate::None));

        // Rationale must be strictly bounded to MAX_RATIONALE_LENGTH characters
        let char_count = response.rationale.chars().count();
        assert!(
            char_count <= MAX_RATIONALE_LENGTH,
            "Case {idx}: Rationale length {char_count} exceeds limit {MAX_RATIONALE_LENGTH}"
        );
        assert!(
            !response.rationale.is_empty(),
            "Case {idx}: Rationale cannot be empty"
        );

        // Verification of UTF-8 char boundary integrity:
        // Converting to Vec<u8> and parsing back as str must never fail
        let bytes = response.rationale.as_bytes();
        let from_utf8 = std::str::from_utf8(bytes);
        assert!(
            from_utf8.is_ok(),
            "Case {idx}: Rationale contains invalid UTF-8 bytes"
        );

        // Zero secret leakage check
        assert!(
            !response.rationale.contains("sk-secret-token-key-"),
            "Case {idx}: Leaked secret token in rationale"
        );
        assert!(
            !response.rationale.contains("drop table users"),
            "Case {idx}: Leaked SQL injection payload in rationale"
        );
        assert!(
            !response.rationale.contains("rm -rf"),
            "Case {idx}: Leaked shell command in rationale"
        );
    }
}

// =========================================================================
// Category 4: Concurrency & High Throughput Fallback Stress Harness
// =========================================================================

#[tokio::test]
async fn test_empirical_fallback_high_concurrency_stress() {
    let fallback = Arc::new(DeterministicFallback::with_provider("stress_mock_provider"));
    let mut join_set = JoinSet::new();

    // Spawn 20 tasks, each executing 50 evaluations across diverse errors
    for task_id in 0..20 {
        let fb = Arc::clone(&fallback);
        join_set.spawn(async move {
            let now = Utc::now();
            for i in 0..50 {
                let err = match (task_id + i) % 6 {
                    0 => CoachError::timeout(100 + i as u64),
                    1 => CoachError::network(format!("socket err {task_id}-{i}")),
                    2 => CoachError::http(500, format!("internal error {task_id}-{i}")),
                    3 => CoachError::malformed_json(format!("bad json {task_id}-{i}"), None, None),
                    4 => CoachError::disallowed_action(format!("hack_{task_id}_{i}")),
                    _ => CoachError::provider_unavailable("prov", "offline"),
                };

                let resp = fb.evaluate(&err, now);
                assert_eq!(resp.relevance, RelevanceClassification::Unknown);
                assert_eq!(resp.confidence, 0.0);
                assert_eq!(resp.action_candidate, Some(ActionCandidate::None));
                assert_eq!(resp.provenance.provider_name, "stress_mock_provider");
            }
        });
    }

    // Await all tasks to finish
    while let Some(res) = join_set.join_next().await {
        res.expect("Task must not panic");
    }
}

// =========================================================================
// Category 5: FallbackReason Serde & Display Invariants
// =========================================================================

#[test]
fn test_empirical_fallback_reason_serde_and_display() {
    let reasons = vec![
        (FallbackReason::Timeout, "timeout"),
        (FallbackReason::NetworkError, "network_error"),
        (FallbackReason::HttpError, "http_error"),
        (FallbackReason::MalformedJson, "malformed_json"),
        (FallbackReason::SchemaViolation, "schema_violation"),
        (FallbackReason::DisallowedAction, "disallowed_action"),
        (FallbackReason::PromptInjection, "prompt_injection"),
        (FallbackReason::ProviderUnavailable, "provider_unavailable"),
        (FallbackReason::CooldownThrottled, "cooldown_throttled"),
        (FallbackReason::CoreError, "core_error"),
    ];

    for (reason, expected_str) in reasons {
        assert_eq!(reason.as_str(), expected_str);
        assert_eq!(format!("{reason}"), expected_str);

        // Serde roundtrip:
        let json = serde_json::to_string(&reason).unwrap();
        assert_eq!(json, format!("\"{expected_str}\""));

        let deserialized: FallbackReason = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, reason);
    }
}

// =========================================================================
// Category 6: OpenAiCompatibleClient Debug Wrapper Redaction
// =========================================================================

#[test]
fn test_empirical_openai_client_debug_wrapper_redaction() {
    let cfg = OpenAiConfig::openai("gpt-4o-mini", "sk-live-super-secret-key-12345")
        .with_header("Authorization", "Bearer custom-bearer-token")
        .with_header("X-Api-Key", "api-secret-key-abcdef")
        .with_header("X-Secret-Token", "secret-token-val")
        .with_header("User-Agent", "WowieCoach/1.0");

    let client = wowie_coach::OpenAiCompatibleClient::new(cfg).unwrap();
    let client_debug = format!("{client:?}");

    // All secrets must be redacted even when formatted through OpenAiCompatibleClient
    assert!(!client_debug.contains("sk-live-super-secret-key-12345"));
    assert!(!client_debug.contains("custom-bearer-token"));
    assert!(!client_debug.contains("api-secret-key-abcdef"));
    assert!(!client_debug.contains("secret-token-val"));

    // Redaction placeholder must be present
    assert!(client_debug.contains("[REDACTED]"));

    // Harmless telemetry preserved
    assert!(client_debug.contains("WowieCoach/1.0"));
    assert!(client_debug.contains("User-Agent"));
}

// =========================================================================
// Category 7: Multi-Byte UTF-8 Char Boundary Safety in Rationale Truncation
// =========================================================================

#[test]
fn test_empirical_multibyte_utf8_rationale_clamping_no_panic() {
    let now = fixed_timestamp();
    let fallback = DeterministicFallback::new();

    // Test 1: 4-byte emojis causing rationale to exceed MAX_RATIONALE_LENGTH (1024)
    let crab_field = "🦀".repeat(1500);
    let err_4byte = CoachError::schema_validation(crab_field, "bad field");
    let resp_4byte = fallback.evaluate(&err_4byte, now);

    assert_eq!(resp_4byte.rationale.chars().count(), MAX_RATIONALE_LENGTH);
    assert!(std::str::from_utf8(resp_4byte.rationale.as_bytes()).is_ok());

    // Test 2: 3-byte Euro symbols (€)
    let euro_field = "€".repeat(1500);
    let err_3byte = CoachError::schema_validation(euro_field, "bad field");
    let resp_3byte = fallback.evaluate(&err_3byte, now);

    assert_eq!(resp_3byte.rationale.chars().count(), MAX_RATIONALE_LENGTH);
    assert!(std::str::from_utf8(resp_3byte.rationale.as_bytes()).is_ok());

    // Test 3: 2-byte Cyrillic chars (я)
    let cyrillic_field = "я".repeat(1500);
    let err_2byte = CoachError::schema_validation(cyrillic_field, "bad field");
    let resp_2byte = fallback.evaluate(&err_2byte, now);

    assert_eq!(resp_2byte.rationale.chars().count(), MAX_RATIONALE_LENGTH);
    assert!(std::str::from_utf8(resp_2byte.rationale.as_bytes()).is_ok());

    // Test 4: Mixed multi-byte string
    let mixed_field = "a€🦀я".repeat(500);
    let err_mixed = CoachError::schema_validation(mixed_field, "bad field");
    let resp_mixed = fallback.evaluate(&err_mixed, now);

    assert_eq!(resp_mixed.rationale.chars().count(), MAX_RATIONALE_LENGTH);
    assert!(std::str::from_utf8(resp_mixed.rationale.as_bytes()).is_ok());
}
