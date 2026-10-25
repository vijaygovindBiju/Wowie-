//! Empirical Adversarial Challenge Test Suite for Milestone 4 Iteration 2 (Challenger 1).
//!
//! File: tests/empirical_challenge_m4_r2_c1_tests.rs
//! Role: Empirical Challenger 1 (Milestone 4 Iteration 2: Verification of map_http_error UTF-8 char boundary truncation)
//!
//! Adversarially stress-tests:
//! 1. Arbitrary byte lengths (490..520 bytes) straddling byte 500 with 1-byte, 2-byte, 3-byte, and 4-byte UTF-8 sequences.
//! 2. Arbitrary character lengths (490..520 chars) crossing character boundary 500 across pure and mixed Unicode sequences.
//! 3. Permutations and interleavings of 1-, 2-, 3-, and 4-byte code points around boundary.
//! 4. JSON error envelope variants containing multibyte UTF-8 boundary straddling payloads.
//! 5. 10,000-iteration deterministic pseudo-random fuzzing harness across all HTTP error status codes.
//! 6. Pathological large multibyte payloads (10KB, 100KB, 1MB) ensuring fast O(500) truncation and zero panics.

use std::panic::{catch_unwind, AssertUnwindSafe};
use wowie_coach::{
    ActionCandidate, CoachError, DeterministicFallback, FallbackReason, OpenAiCompatibleClient,
    OpenAiConfig, RelevanceClassification,
};

fn create_client() -> OpenAiCompatibleClient {
    OpenAiCompatibleClient::new(OpenAiConfig::ollama("llama3:8b")).unwrap()
}

// Simple deterministic PRNG (Xorshift64) to avoid external crate dependencies
struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0xdead_beef } else { seed },
        }
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }

    fn next_range(&mut self, min: usize, max: usize) -> usize {
        assert!(max >= min);
        let span = max - min + 1;
        min + (self.next_u64() as usize % span)
    }
}

// Sample multibyte characters across all UTF-8 byte widths:
// 1-byte: ASCII
// 2-byte: Cyrillic, Greek, Latin-ext
// 3-byte: Euro symbol, CJK characters, Devanagari
// 4-byte: Emoji, SMP symbols, Math symbols
const SAMPLE_1BYTE: &[char] = &['a', 'b', 'c', '1', '9', '!', '$', 'Z'];
const SAMPLE_2BYTE: &[char] = &['я', 'é', 'Ω', 'λ', 'ñ', 'ü', 'δ', 'ж'];
const SAMPLE_3BYTE: &[char] = &['€', '中', '文', '日', '漢', '語', '₹', '௵'];
const SAMPLE_4BYTE: &[char] = &['🦀', '🔥', '🚀', '😀', '🎉', '🧠', '🛡', '⚡'];

// =========================================================================
// Category 1: Adversarial Byte Length Sweep (490..520 bytes) Straddling Byte 500
// =========================================================================

#[test]
fn test_adversarial_byte_length_sweep_straddling_byte_500() {
    let client = create_client();

    // Test combinations where multi-byte characters start at byte offsets 490..510
    for base_ascii_len in 490..=510 {
        // Test with 1-byte, 2-byte, 3-byte, and 4-byte characters
        for &mb_char in SAMPLE_1BYTE
            .iter()
            .chain(SAMPLE_2BYTE.iter())
            .chain(SAMPLE_3BYTE.iter())
            .chain(SAMPLE_4BYTE.iter())
        {
            for trailing_len in 0..=15 {
                let mut body = "a".repeat(base_ascii_len);
                body.push(mb_char);
                body.push_str(&"z".repeat(trailing_len));

                let result = catch_unwind(AssertUnwindSafe(|| client.map_http_error(500, &body)));

                assert!(
                    result.is_ok(),
                    "Panic detected in map_http_error at base_len={base_ascii_len}, char='{mb_char}', trailing_len={trailing_len}"
                );

                let err = result.unwrap();
                match err {
                    CoachError::HttpError {
                        status_code,
                        message,
                    } => {
                        assert_eq!(status_code, 500);
                        // The string must be valid UTF-8, and chars count <= 500 (+ "..." if truncated)
                        if body.chars().count() > 500 {
                            assert!(
                                message.ends_with("..."),
                                "Expected truncated message to end with '...'"
                            );
                            let prefix = message.strip_suffix("...").unwrap();
                            assert_eq!(
                                prefix.chars().count(),
                                500,
                                "Prefix before '...' must be exactly 500 characters"
                            );
                        } else {
                            assert_eq!(message, body, "Body <= 500 characters must not be altered");
                        }
                    }
                    other => panic!("Unexpected error variant: {other:?}"),
                }
            }
        }
    }
}

// =========================================================================
// Category 2: Character Length Sweep (490..520 chars) Across Pure Encodings
// =========================================================================

#[test]
fn test_adversarial_char_length_sweep_pure_encodings() {
    let client = create_client();

    for char_count in 490..=520 {
        // 1. Pure 1-byte ASCII
        let ascii_str: String = "x".repeat(char_count);
        let res_ascii = catch_unwind(AssertUnwindSafe(|| client.map_http_error(502, &ascii_str)));
        assert!(res_ascii.is_ok());
        verify_truncation_oracle(&ascii_str, res_ascii.unwrap());

        // 2. Pure 2-byte Cyrillic
        let cyrillic_str: String = "я".repeat(char_count);
        let res_cyrillic = catch_unwind(AssertUnwindSafe(|| {
            client.map_http_error(503, &cyrillic_str)
        }));
        assert!(res_cyrillic.is_ok());
        verify_truncation_oracle(&cyrillic_str, res_cyrillic.unwrap());

        // 3. Pure 3-byte CJK
        let cjk_str: String = "中".repeat(char_count);
        let res_cjk = catch_unwind(AssertUnwindSafe(|| client.map_http_error(504, &cjk_str)));
        assert!(res_cjk.is_ok());
        verify_truncation_oracle(&cjk_str, res_cjk.unwrap());

        // 4. Pure 4-byte Emoji
        let emoji_str: String = "🦀".repeat(char_count);
        let res_emoji = catch_unwind(AssertUnwindSafe(|| client.map_http_error(500, &emoji_str)));
        assert!(res_emoji.is_ok());
        verify_truncation_oracle(&emoji_str, res_emoji.unwrap());
    }
}

// =========================================================================
// Category 3: Interleaved Multibyte Permutations Straddling Boundary
// =========================================================================

#[test]
fn test_adversarial_interleaved_multibyte_permutations() {
    let client = create_client();

    // A 4-character token with 1+2+3+4 = 10 bytes: 'a' (1), 'я' (2), '€' (3), '🦀' (4)
    let pattern = ['a', 'я', '€', '🦀'];

    for repeat_count in 120..=130 {
        let mut text = String::new();
        for _ in 0..repeat_count {
            for &c in &pattern {
                text.push(c);
            }
        }

        // Test with different phase offsets (0, 1, 2, 3)
        for offset in 0..4 {
            let offset_text: String = text.chars().skip(offset).collect();
            let res = catch_unwind(AssertUnwindSafe(|| {
                client.map_http_error(429, &offset_text)
            }));

            assert!(res.is_ok());
            verify_truncation_oracle(&offset_text, res.unwrap());
        }
    }
}

// =========================================================================
// Category 4: JSON Error Envelope Boundary Variations
// =========================================================================

#[test]
fn test_adversarial_json_envelope_boundary_variations() {
    let client = create_client();

    // 1. JSON error object with message field straddling 500 chars with 4-byte emoji
    for len in 495..=505 {
        let msg = format!("{}{}{}", "x".repeat(len), "🦀🔥🚀", "y".repeat(10));
        let json_body = serde_json::json!({
            "error": {
                "message": msg,
                "type": "invalid_request_error",
                "code": "context_length_exceeded"
            }
        })
        .to_string();

        let res = catch_unwind(AssertUnwindSafe(|| client.map_http_error(400, &json_body)));
        assert!(res.is_ok());
        verify_truncation_oracle(&msg, res.unwrap());
    }

    // 2. JSON error as a plain string: {"error": "..."}
    for len in 495..=505 {
        let msg = format!("{}{}", "z".repeat(len), "€€€€");
        let json_body = serde_json::json!({
            "error": msg
        })
        .to_string();

        let res = catch_unwind(AssertUnwindSafe(|| client.map_http_error(404, &json_body)));
        assert!(res.is_ok());
        let err = res.unwrap();
        match err {
            CoachError::HttpError {
                status_code,
                message,
            } => {
                assert_eq!(status_code, 404);
                assert!(message.contains("not found: "));
            }
            other => panic!("Unexpected error: {other:?}"),
        }
    }

    // 3. JSON error object without message or string: falls back to raw body
    let raw_obj = serde_json::json!({
        "error": {
            "code": 500,
            "details": "я".repeat(300)
        }
    })
    .to_string();
    let res_raw = catch_unwind(AssertUnwindSafe(|| client.map_http_error(500, &raw_obj)));
    assert!(res_raw.is_ok());
    verify_truncation_oracle(&raw_obj, res_raw.unwrap());
}

// =========================================================================
// Category 5: 10,000 Iteration Deterministic Pseudo-Random Fuzzing Harness
// =========================================================================

#[test]
fn test_fuzzing_10000_multibyte_combinations_zero_panic() {
    let client = create_client();
    let mut rng = XorShift64::new(0x1234_5678_9abc_def0);

    let status_codes = [400, 401, 403, 404, 429, 500, 502, 503, 504, 520, 599];

    for iteration in 0..10_000 {
        let code = status_codes[rng.next_range(0, status_codes.len() - 1)];

        // Generate length: bias towards boundary 480..520, with some very small and very large
        let len = match rng.next_range(0, 10) {
            0 => rng.next_range(0, 10),        // Tiny: 0..10 chars
            1..=7 => rng.next_range(480, 520), // Boundary focus: 480..520 chars
            8 => rng.next_range(100, 400),     // Medium: 100..400 chars
            _ => rng.next_range(600, 1500),    // Large: 600..1500 chars
        };

        let mut payload = String::with_capacity(len * 4);
        for _ in 0..len {
            let width = rng.next_range(1, 4);
            let ch = match width {
                1 => SAMPLE_1BYTE[rng.next_range(0, SAMPLE_1BYTE.len() - 1)],
                2 => SAMPLE_2BYTE[rng.next_range(0, SAMPLE_2BYTE.len() - 1)],
                3 => SAMPLE_3BYTE[rng.next_range(0, SAMPLE_3BYTE.len() - 1)],
                _ => SAMPLE_4BYTE[rng.next_range(0, SAMPLE_4BYTE.len() - 1)],
            };
            payload.push(ch);
        }

        // Test half raw, half wrapped in JSON
        let body = if rng.next_range(0, 1) == 0 {
            payload.clone()
        } else {
            serde_json::json!({ "error": { "message": payload } }).to_string()
        };

        let res = catch_unwind(AssertUnwindSafe(|| client.map_http_error(code, &body)));

        assert!(
            res.is_ok(),
            "Panic detected on iteration {iteration} with status {code}, char count {len}"
        );

        let err = res.unwrap();
        // Check variant consistency
        match code {
            401 | 403 => {
                assert!(
                    matches!(err, CoachError::ProviderUnavailable { ref provider, .. } if provider == "openai_compatible"),
                    "Iteration {iteration}: Expected ProviderUnavailable for {code}, got {err:?}"
                );
            }
            _ => {
                assert!(
                    matches!(err, CoachError::HttpError { status_code, .. } if status_code == code),
                    "Iteration {iteration}: Expected HttpError({code}), got {err:?}"
                );
            }
        }
    }
}

// =========================================================================
// Category 6: Pathological Large Multibyte Payloads (10KB, 100KB, 1MB)
// =========================================================================

#[test]
fn test_multibyte_large_payloads_up_to_1mb_zero_panic() {
    let client = create_client();

    let sizes = [
        ("10KB_emoji", "🦀".repeat(2_500)),
        ("100KB_cjk", "中".repeat(33_333)),
        ("1MB_mixed", "aя€🦀".repeat(100_000)),
    ];

    for (desc, payload) in sizes {
        let res = catch_unwind(AssertUnwindSafe(|| client.map_http_error(500, &payload)));
        assert!(res.is_ok(), "Panic on large payload {desc}");

        let err = res.unwrap();
        match err {
            CoachError::HttpError {
                status_code,
                message,
            } => {
                assert_eq!(status_code, 500);
                assert!(message.ends_with("..."));
                let base = message.strip_suffix("...").unwrap();
                assert_eq!(
                    base.chars().count(),
                    500,
                    "Base message before '...' must have exactly 500 characters for {desc}"
                );
            }
            other => panic!("Unexpected error: {other:?}"),
        }
    }
}

// =========================================================================
// Category 7: Deterministic Fallback Pipeline Integration
// =========================================================================

#[test]
fn test_fallback_integration_with_multibyte_truncated_errors() {
    let client = create_client();

    let mut payload = "a".repeat(498);
    payload.push('🦀');
    payload.push('€');
    payload.push_str(&"extra".repeat(20));

    let err = client.map_http_error(500, &payload);

    let fallback = DeterministicFallback::with_provider("openai_compatible");
    let now = chrono::Utc::now();
    let response = fallback.evaluate(&err, now);
    let reason = FallbackReason::from_error(&err);

    assert_eq!(response.relevance, RelevanceClassification::Unknown);
    assert_eq!(response.action_candidate, Some(ActionCandidate::None));
    assert_eq!(reason, FallbackReason::HttpError);
    assert!(
        response.rationale.contains("500"),
        "Fallback rationale must reference HTTP 500"
    );
}

// =========================================================================
// Helper Oracle
// =========================================================================

fn verify_truncation_oracle(expected_full_msg: &str, err: CoachError) {
    let msg = match err {
        CoachError::HttpError { message, .. } => message,
        CoachError::ProviderUnavailable { reason, .. } => reason,
        other => panic!("Unexpected CoachError variant: {other:?}"),
    };

    // Strip status-specific prefixes to isolate bounded_msg
    let bounded = if let Some(stripped) = msg.strip_prefix("Rate limit exceeded: ") {
        stripped
    } else if let Some(pos) = msg.find("not found: ") {
        &msg[pos + "not found: ".len()..]
    } else if let Some(pos) = msg.find("): ") {
        &msg[pos + "): ".len()..]
    } else {
        &msg
    };

    let original_char_count = expected_full_msg.chars().count();

    if original_char_count <= 500 {
        assert_eq!(
            bounded, expected_full_msg,
            "Full message of {original_char_count} chars should match exactly"
        );
    } else {
        assert!(
            bounded.ends_with("..."),
            "Truncated message must end with '...'"
        );
        let base = bounded.strip_suffix("...").unwrap();
        assert_eq!(
            base.chars().count(),
            500,
            "Base message before '...' must have exactly 500 characters"
        );
    }
}
