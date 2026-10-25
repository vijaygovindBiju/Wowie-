//! Tier 2 E2E Boundary & Corner Cases Test Suite for Wowie AI Coach (`crates/wowie-coach`).
//!
//! Validates extremes, limits, corner cases, and boundary invariants completely offline:
//! - Category 1: Maximum allowed string lengths (rationale, questions, titles, errors)
//! - Category 2: Zero, negative, and non-finite float boundary validation ([0.0, 1.0], NaN, ±Infinity)
//! - Category 3: Clock extremes & time math (epoch 1970, far future, backwards regressions)
//! - Category 4: Cooldown extremes & burst calls (0s, max 7200s, rapid burst, memory limits)
//! - Category 5: Multibyte Unicode boundary cases (UTF-8 straddling, emoji, CJK, zero-width, RTL overrides)
//! - Category 6: Empty collections, empty strings, missing fields, and malformed JSON envelopes

use chrono::{Duration, TimeZone, Utc};
use wowie_coach::{
    ActionCandidate, CoachContext, CoachError, CoachRawResponse, CoachResponse, CooldownConfig,
    CooldownStatus, CooldownTracker, OpenAiCompatibleClient, OpenAiConfig, PersonalityMode,
    PromptSanitizer, ProviderProvenance, RelevanceClassification, ThrottlingReason,
    TonePreferences, UntrustedObservation, UserCorrectionSummary, MAX_COACH_NAME_LENGTH,
    MAX_CORRECTION_NOTE_LENGTH, MAX_CUSTOM_INSTRUCTIONS_LENGTH, MAX_PROVENANCE_IDENTIFIER_LENGTH,
    MAX_QUESTION_LENGTH, MAX_RATIONALE_LENGTH,
};
use wowie_core::id::SessionId;
use wowie_core::models::goal::{MAX_GOAL_TITLE_LENGTH, MAX_NEXT_ACTION_LENGTH};
use wowie_core::models::observation::{
    MAX_APP_ID_LENGTH, MAX_ORIGIN_LENGTH, MAX_WINDOW_TITLE_LENGTH,
};
use wowie_core::models::{ClassificationLabel, CorrectionScope, IdleState};

// =========================================================================
// CATEGORY 1: Maximum Allowed String Lengths (Limits & Exact Boundaries)
// =========================================================================
mod string_length_boundaries {
    use super::*;

    #[test]
    fn test_rationale_exact_boundary_1024_chars_accepted() {
        let prov = ProviderProvenance::mock(Utc::now());
        let exact_rationale = "x".repeat(MAX_RATIONALE_LENGTH);
        let resp = CoachResponse::new(
            RelevanceClassification::Relevant,
            0.9,
            None,
            exact_rationale,
            None,
            None,
            prov,
        );
        assert!(
            resp.is_ok(),
            "Rationale at exact 1024 chars must be accepted"
        );
        assert_eq!(resp.unwrap().rationale.len(), MAX_RATIONALE_LENGTH);
    }

    #[test]
    fn test_rationale_exceeding_1024_chars_rejected() {
        let prov = ProviderProvenance::mock(Utc::now());
        let oversized = "x".repeat(MAX_RATIONALE_LENGTH + 1);
        let err = CoachResponse::new(
            RelevanceClassification::Relevant,
            0.9,
            None,
            oversized,
            None,
            None,
            prov,
        )
        .expect_err("Rationale of 1025 chars must be rejected");

        assert!(matches!(
            err,
            CoachError::SchemaValidation { ref field, .. } if field == "rationale"
        ));
    }

    #[test]
    fn test_question_exact_boundary_512_chars_accepted() {
        let prov = ProviderProvenance::mock(Utc::now());
        let exact_q = "q".repeat(MAX_QUESTION_LENGTH);
        let resp = CoachResponse::new(
            RelevanceClassification::PossiblyUnrelated,
            0.7,
            None,
            "Valid rationale",
            Some(exact_q),
            Some(ActionCandidate::AskContext),
            prov,
        );
        assert!(resp.is_ok(), "Question at exact 512 chars must be accepted");
    }

    #[test]
    fn test_question_exceeding_512_chars_rejected() {
        let prov = ProviderProvenance::mock(Utc::now());
        let oversized_q = "q".repeat(MAX_QUESTION_LENGTH + 1);
        let err = CoachResponse::new(
            RelevanceClassification::PossiblyUnrelated,
            0.7,
            None,
            "Valid rationale",
            Some(oversized_q),
            Some(ActionCandidate::AskContext),
            prov,
        )
        .expect_err("Question of 513 chars must be rejected");

        assert!(matches!(
            err,
            CoachError::SchemaValidation { ref field, .. } if field == "question"
        ));
    }

    #[test]
    fn test_goal_title_exact_boundary_accepted() {
        let exact_title = "t".repeat(MAX_GOAL_TITLE_LENGTH);
        let ctx = CoachContext::new(
            exact_title,
            None,
            SessionId::new(),
            0,
            false,
            UntrustedObservation::default(),
            Vec::new(),
            TonePreferences::default(),
        );
        assert!(ctx.is_ok(), "Goal title at exact limit must be accepted");
    }

    #[test]
    fn test_goal_title_exceeding_boundary_rejected() {
        let oversized = "t".repeat(MAX_GOAL_TITLE_LENGTH + 1);
        let err = CoachContext::new(
            oversized,
            None,
            SessionId::new(),
            0,
            false,
            UntrustedObservation::default(),
            Vec::new(),
            TonePreferences::default(),
        )
        .expect_err("Goal title exceeding limit must be rejected");

        assert!(matches!(
            err,
            CoachError::SchemaValidation { ref field, .. } if field == "goal_title"
        ));
    }

    #[test]
    fn test_next_action_exact_boundary_accepted() {
        let exact_action = "a".repeat(MAX_NEXT_ACTION_LENGTH);
        let ctx = CoachContext::new(
            "Valid Goal",
            Some(exact_action),
            SessionId::new(),
            0,
            false,
            UntrustedObservation::default(),
            Vec::new(),
            TonePreferences::default(),
        );
        assert!(ctx.is_ok(), "Next action at exact limit must be accepted");
    }

    #[test]
    fn test_next_action_exceeding_boundary_rejected() {
        let oversized = "a".repeat(MAX_NEXT_ACTION_LENGTH + 1);
        let err = CoachContext::new(
            "Valid Goal",
            Some(oversized),
            SessionId::new(),
            0,
            false,
            UntrustedObservation::default(),
            Vec::new(),
            TonePreferences::default(),
        )
        .expect_err("Next action exceeding limit must be rejected");

        assert!(matches!(
            err,
            CoachError::SchemaValidation { ref field, .. } if field == "next_action"
        ));
    }

    #[test]
    fn test_app_id_and_window_title_boundaries() {
        // App ID exact 256
        let app_exact = "a".repeat(MAX_APP_ID_LENGTH);
        let obs_app_ok = UntrustedObservation::new(Some(app_exact), None, None, None);
        assert!(obs_app_ok.is_ok());

        // App ID 257
        let app_oversized = "a".repeat(MAX_APP_ID_LENGTH + 1);
        let obs_app_err = UntrustedObservation::new(Some(app_oversized), None, None, None);
        assert!(matches!(
            obs_app_err,
            Err(CoachError::SchemaValidation { ref field, .. }) if field == "app_id"
        ));

        // Window title exact 256
        let title_exact = "w".repeat(MAX_WINDOW_TITLE_LENGTH);
        let obs_title_ok = UntrustedObservation::new(None, Some(title_exact), None, None);
        assert!(obs_title_ok.is_ok());

        // Window title 257
        let title_oversized = "w".repeat(MAX_WINDOW_TITLE_LENGTH + 1);
        let obs_title_err = UntrustedObservation::new(None, Some(title_oversized), None, None);
        assert!(matches!(
            obs_title_err,
            Err(CoachError::SchemaValidation { ref field, .. }) if field == "window_title"
        ));
    }

    #[test]
    fn test_origin_exact_boundary_and_overflow() {
        let origin_exact = "o".repeat(MAX_ORIGIN_LENGTH);
        assert!(UntrustedObservation::new(None, None, Some(origin_exact), None).is_ok());

        let origin_oversized = "o".repeat(MAX_ORIGIN_LENGTH + 1);
        let err = UntrustedObservation::new(None, None, Some(origin_oversized), None);
        assert!(matches!(
            err,
            Err(CoachError::SchemaValidation { ref field, .. }) if field == "origin"
        ));
    }

    #[test]
    fn test_correction_note_exact_boundary_and_overflow() {
        let note_exact = "n".repeat(MAX_CORRECTION_NOTE_LENGTH);
        assert!(UserCorrectionSummary::new(
            ClassificationLabel::Relevant,
            CorrectionScope::Session,
            Some("app".to_string()),
            None,
            Some(note_exact)
        )
        .is_ok());

        let note_oversized = "n".repeat(MAX_CORRECTION_NOTE_LENGTH + 1);
        let err = UserCorrectionSummary::new(
            ClassificationLabel::Relevant,
            CorrectionScope::Session,
            Some("app".to_string()),
            None,
            Some(note_oversized),
        );
        assert!(matches!(
            err,
            Err(CoachError::SchemaValidation { ref field, .. }) if field == "note"
        ));
    }

    #[test]
    fn test_coach_name_exact_boundary_and_overflow() {
        let name_exact = "c".repeat(MAX_COACH_NAME_LENGTH);
        assert!(TonePreferences::new(name_exact, PersonalityMode::Neutral).is_ok());

        let name_oversized = "c".repeat(MAX_COACH_NAME_LENGTH + 1);
        assert!(TonePreferences::new(name_oversized, PersonalityMode::Neutral).is_err());
    }

    #[test]
    fn test_custom_instructions_exact_boundary_and_overflow() {
        let mut tone = TonePreferences {
            custom_instructions: Some("i".repeat(MAX_CUSTOM_INSTRUCTIONS_LENGTH)),
            ..Default::default()
        };
        assert!(tone.validate().is_ok());

        tone.custom_instructions = Some("i".repeat(MAX_CUSTOM_INSTRUCTIONS_LENGTH + 1));
        assert!(tone.validate().is_err());
    }

    #[test]
    fn test_provenance_identifiers_length_boundary() {
        let now = Utc::now();
        let exact_id = "p".repeat(MAX_PROVENANCE_IDENTIFIER_LENGTH);
        assert!(ProviderProvenance::new(exact_id.clone(), exact_id, "1.0.0", now, None).is_ok());

        let oversized_id = "p".repeat(MAX_PROVENANCE_IDENTIFIER_LENGTH + 1);
        assert!(
            ProviderProvenance::new(oversized_id.clone(), "model", "1.0.0", now, None).is_err()
        );
        assert!(ProviderProvenance::new("provider", oversized_id, "1.0.0", now, None).is_err());
    }

    #[test]
    fn test_http_error_body_truncation_boundary_500_chars() {
        let client = OpenAiCompatibleClient::new(OpenAiConfig::ollama("llama3")).unwrap();

        // Error message of exactly 500 characters -> not truncated with "..."
        let msg_500 = "e".repeat(500);
        let err_500 = client.map_http_error(500, &msg_500);
        assert_eq!(
            err_500.to_string(),
            format!("Coach provider HTTP 500: {msg_500}")
        );

        // Error message of 501 characters -> truncated with "..."
        let msg_501 = "e".repeat(501);
        let err_501 = client.map_http_error(500, &msg_501);
        assert!(err_501.to_string().ends_with("..."));
    }

    #[test]
    fn test_rationale_exact_whitespace_trimmed_boundary() {
        let prov = ProviderProvenance::mock(Utc::now());
        // 1024 characters with surrounding whitespace -> trimmed to exactly 1024 and accepted
        let raw_with_whitespace = format!("  {}  \n", "r".repeat(MAX_RATIONALE_LENGTH));
        let resp = CoachResponse::new(
            RelevanceClassification::Relevant,
            0.8,
            None,
            raw_with_whitespace,
            None,
            None,
            prov,
        );
        assert!(resp.is_ok());
        assert_eq!(resp.unwrap().rationale.len(), MAX_RATIONALE_LENGTH);
    }

    #[test]
    fn test_question_exact_whitespace_trimmed_boundary() {
        let prov = ProviderProvenance::mock(Utc::now());
        // 512 characters with surrounding whitespace -> trimmed to exactly 512 and accepted
        let q_with_whitespace = format!("  {} \t", "q".repeat(MAX_QUESTION_LENGTH));
        let resp = CoachResponse::new(
            RelevanceClassification::PossiblyUnrelated,
            0.7,
            None,
            "Rationale",
            Some(q_with_whitespace),
            None,
            prov,
        );
        assert!(resp.is_ok());
        assert_eq!(resp.unwrap().question.unwrap().len(), MAX_QUESTION_LENGTH);
    }

    #[test]
    fn test_provenance_empty_version_rejected() {
        let now = Utc::now();
        let err = ProviderProvenance::new("provider", "model", "", now, None);
        assert!(matches!(
            err,
            Err(CoachError::SchemaValidation { ref field, .. }) if field == "version"
        ));
    }
}

// =========================================================================
// CATEGORY 2: Zero, Negative, and Non-Finite Float Boundary Validation
// =========================================================================
mod float_range_and_non_finite_boundaries {
    use super::*;

    #[test]
    fn test_confidence_exact_zero_and_one_accepted() {
        let prov = ProviderProvenance::mock(Utc::now());
        let zero_resp = CoachResponse::new(
            RelevanceClassification::Unknown,
            0.0,
            None,
            "Zero confidence",
            None,
            None,
            prov.clone(),
        );
        assert!(zero_resp.is_ok(), "Confidence 0.0 must be accepted");

        let one_resp = CoachResponse::new(
            RelevanceClassification::Relevant,
            1.0,
            None,
            "Max confidence",
            None,
            None,
            prov,
        );
        assert!(one_resp.is_ok(), "Confidence 1.0 must be accepted");
    }

    #[test]
    fn test_confidence_negative_boundaries_rejected() {
        let prov = ProviderProvenance::mock(Utc::now());
        for bad_conf in [-1e-6_f32, -0.001, -0.5, -1.0, -100.0] {
            let err = CoachResponse::new(
                RelevanceClassification::Relevant,
                bad_conf,
                None,
                "Negative confidence",
                None,
                None,
                prov.clone(),
            );
            assert!(
                err.is_err(),
                "Confidence {bad_conf} must be rejected as negative"
            );
        }
    }

    #[test]
    fn test_confidence_above_one_boundaries_rejected() {
        let prov = ProviderProvenance::mock(Utc::now());
        for bad_conf in [1.00001_f32, 1.01, 1.5, 2.0, 999.0] {
            let err = CoachResponse::new(
                RelevanceClassification::Relevant,
                bad_conf,
                None,
                "Overbound confidence",
                None,
                None,
                prov.clone(),
            );
            assert!(
                err.is_err(),
                "Confidence {bad_conf} must be rejected as > 1.0"
            );
        }
    }

    #[test]
    fn test_confidence_non_finite_rejected() {
        let prov = ProviderProvenance::mock(Utc::now());
        assert!(CoachResponse::new(
            RelevanceClassification::Relevant,
            f32::NAN,
            None,
            "NaN",
            None,
            None,
            prov.clone()
        )
        .is_err());

        assert!(CoachResponse::new(
            RelevanceClassification::Relevant,
            f32::INFINITY,
            None,
            "Inf",
            None,
            None,
            prov.clone()
        )
        .is_err());

        assert!(CoachResponse::new(
            RelevanceClassification::Relevant,
            f32::NEG_INFINITY,
            None,
            "-Inf",
            None,
            None,
            prov
        )
        .is_err());
    }

    #[test]
    fn test_uncertainty_exact_boundaries_and_rejections() {
        let prov = ProviderProvenance::mock(Utc::now());
        // Exact 0.0 and 1.0
        assert!(CoachResponse::new(
            RelevanceClassification::Relevant,
            0.5,
            Some(0.0),
            "Uncertainty 0.0",
            None,
            None,
            prov.clone()
        )
        .is_ok());

        assert!(CoachResponse::new(
            RelevanceClassification::Relevant,
            0.5,
            Some(1.0),
            "Uncertainty 1.0",
            None,
            None,
            prov.clone()
        )
        .is_ok());

        // Invalid: negative, over 1.0, NaN, Inf
        assert!(CoachResponse::new(
            RelevanceClassification::Relevant,
            0.5,
            Some(-0.001),
            "Negative",
            None,
            None,
            prov.clone()
        )
        .is_err());

        assert!(CoachResponse::new(
            RelevanceClassification::Relevant,
            0.5,
            Some(1.001),
            "Above 1",
            None,
            None,
            prov.clone()
        )
        .is_err());

        assert!(CoachResponse::new(
            RelevanceClassification::Relevant,
            0.5,
            Some(f32::NAN),
            "NaN",
            None,
            None,
            prov.clone()
        )
        .is_err());

        assert!(CoachResponse::new(
            RelevanceClassification::Relevant,
            0.5,
            Some(f32::INFINITY),
            "Inf",
            None,
            None,
            prov
        )
        .is_err());
    }

    #[test]
    fn test_raw_json_float_type_mismatches_rejected() {
        // String instead of float
        let json_str_conf = r#"{
            "relevance": "relevant",
            "confidence": "0.95",
            "rationale": "Confidence is a string"
        }"#;
        assert!(CoachRawResponse::from_json(json_str_conf).is_err());

        // Boolean instead of float
        let json_bool_conf = r#"{
            "relevance": "relevant",
            "confidence": true,
            "rationale": "Confidence is a boolean"
        }"#;
        assert!(CoachRawResponse::from_json(json_bool_conf).is_err());

        // Object instead of float
        let json_obj_conf = r#"{
            "relevance": "relevant",
            "confidence": {"score": 0.95},
            "rationale": "Confidence is an object"
        }"#;
        assert!(CoachRawResponse::from_json(json_obj_conf).is_err());
    }

    #[test]
    fn test_temperature_float_bounds_in_openai_config() {
        let base = OpenAiConfig::ollama("llama3");

        // Valid boundaries: 0.0 and 2.0
        assert!(base.clone().with_temperature(0.0).is_ok());
        assert!(base.clone().with_temperature(2.0).is_ok());

        // Invalid: negative, > 2.0, NaN, Infinity
        assert!(base.clone().with_temperature(-0.01).is_err());
        assert!(base.clone().with_temperature(2.01).is_err());
        assert!(base.clone().with_temperature(f32::NAN).is_err());
        assert!(base.with_temperature(f32::INFINITY).is_err());
    }

    #[test]
    fn test_confidence_boundary_epsilon_below_zero() {
        let prov = ProviderProvenance::mock(Utc::now());
        let err = CoachResponse::new(
            RelevanceClassification::Relevant,
            -1e-7,
            None,
            "Negative epsilon",
            None,
            None,
            prov,
        );
        assert!(err.is_err(), "Confidence -1e-7 must be rejected");
    }

    #[test]
    fn test_confidence_boundary_epsilon_above_one() {
        let prov = ProviderProvenance::mock(Utc::now());
        let err = CoachResponse::new(
            RelevanceClassification::Relevant,
            1.0 + 1e-7,
            None,
            "Overbound epsilon",
            None,
            None,
            prov,
        );
        assert!(err.is_err(), "Confidence 1.0 + 1e-7 must be rejected");
    }

    #[test]
    fn test_raw_json_uncertainty_null_vs_omitted() {
        // Explicit null uncertainty
        let json_null = r#"{
            "relevance": "relevant",
            "confidence": 0.9,
            "uncertainty": null,
            "rationale": "Explicit null uncertainty"
        }"#;
        let raw_null = CoachRawResponse::from_json(json_null).unwrap();
        assert_eq!(raw_null.uncertainty, None);

        // Omitted uncertainty
        let json_omitted = r#"{
            "relevance": "relevant",
            "confidence": 0.9,
            "rationale": "Omitted uncertainty"
        }"#;
        let raw_omitted = CoachRawResponse::from_json(json_omitted).unwrap();
        assert_eq!(raw_omitted.uncertainty, None);
    }
}

// =========================================================================
// CATEGORY 3: Clock Extremes & Time Math
// =========================================================================
mod clock_extremes_and_regressions {
    use super::*;

    #[test]
    fn test_epoch_timestamp_1970_handling() {
        let tracker = CooldownTracker::with_default_config();
        let epoch = Utc.timestamp_opt(0, 0).unwrap();

        // Evaluating at Unix epoch does not underflow or panic
        let status = tracker.can_intervene(epoch);
        assert_eq!(status, CooldownStatus::Allowed);
    }

    #[test]
    fn test_far_future_timestamp_century_ahead() {
        let mut tracker = CooldownTracker::with_default_config();
        // 2126-10-25 (100 years into future)
        let future = Utc.with_ymd_and_hms(2126, 10, 25, 12, 0, 0).unwrap();

        assert_eq!(tracker.can_intervene(future), CooldownStatus::Allowed);
        tracker.record_intervention(future, ActionCandidate::Remind);

        // Immediate next timestamp throttled
        let future_plus_1 = future + Duration::seconds(1);
        let status = tracker.can_intervene(future_plus_1);
        assert!(matches!(
            status,
            CooldownStatus::Throttled {
                reason: ThrottlingReason::RecentIntervention,
                ..
            }
        ));
    }

    #[test]
    fn test_clock_regression_by_various_intervals() {
        let mut tracker = CooldownTracker::with_default_config();
        let t0 = Utc::now();
        tracker.record_intervention(t0, ActionCandidate::Remind);

        // 1 second in the past
        let t_minus_1s = t0 - Duration::seconds(1);
        assert!(matches!(
            tracker.can_intervene(t_minus_1s),
            CooldownStatus::Throttled {
                reason: ThrottlingReason::TimeRegression,
                ..
            }
        ));

        // 1 hour in the past
        let t_minus_1h = t0 - Duration::hours(1);
        assert!(matches!(
            tracker.can_intervene(t_minus_1h),
            CooldownStatus::Throttled {
                reason: ThrottlingReason::TimeRegression,
                ..
            }
        ));

        // 10 years in the past
        let t_minus_10y = t0 - Duration::days(3650);
        assert!(matches!(
            tracker.can_intervene(t_minus_10y),
            CooldownStatus::Throttled {
                reason: ThrottlingReason::TimeRegression,
                ..
            }
        ));
    }

    #[test]
    fn test_millisecond_monotonic_step_forward() {
        let mut tracker = CooldownTracker::with_default_config();
        let t0 = Utc::now();
        tracker.record_intervention(t0, ActionCandidate::Remind);

        // 1 millisecond forward
        let t_1ms = t0 + Duration::milliseconds(1);
        let status = tracker.can_intervene(t_1ms);
        assert!(matches!(
            status,
            CooldownStatus::Throttled {
                reason: ThrottlingReason::RecentIntervention,
                ..
            }
        ));
    }

    #[test]
    fn test_midnight_day_boundary_transition() {
        let mut tracker = CooldownTracker::with_default_config();
        // 23:59:50 -> intervention
        let t_night = Utc.with_ymd_and_hms(2026, 10, 25, 23, 59, 50).unwrap();
        tracker.record_intervention(t_night, ActionCandidate::Remind);

        // 00:00:10 next day (20 seconds elapsed) -> still within 300s cooldown
        let t_morning_early = Utc.with_ymd_and_hms(2026, 10, 26, 0, 0, 10).unwrap();
        let status_early = tracker.can_intervene(t_morning_early);
        assert!(matches!(
            status_early,
            CooldownStatus::Throttled {
                reason: ThrottlingReason::RecentIntervention,
                remaining_seconds: 280,
            }
        ));

        // 00:05:00 next day (310 seconds elapsed) -> allowed
        let t_morning_ok = Utc.with_ymd_and_hms(2026, 10, 26, 0, 5, 0).unwrap();
        assert_eq!(tracker.can_intervene(t_morning_ok), CooldownStatus::Allowed);
    }

    #[test]
    fn test_provenance_historic_and_future_timestamps() {
        let historic = Utc.timestamp_opt(0, 0).unwrap();
        let prov_historic = ProviderProvenance::mock(historic);
        assert_eq!(prov_historic.generated_at, historic);

        let future = Utc.with_ymd_and_hms(2099, 12, 31, 23, 59, 59).unwrap();
        let prov_future = ProviderProvenance::fallback(future);
        assert_eq!(prov_future.generated_at, future);
    }

    #[test]
    fn test_exact_same_timestamp_consecutive_rejected() {
        let mut tracker = CooldownTracker::with_default_config();
        let t0 = Utc::now();
        tracker.record_intervention(t0, ActionCandidate::Remind);

        // Consecutive call at exact t0 must be throttled by RecentIntervention
        let status = tracker.can_intervene(t0);
        assert!(matches!(
            status,
            CooldownStatus::Throttled {
                reason: ThrottlingReason::RecentIntervention,
                remaining_seconds: 300,
            }
        ));
    }

    #[test]
    fn test_timestamp_leap_year_feb_29() {
        let mut tracker = CooldownTracker::with_default_config();
        let leap_day = Utc.with_ymd_and_hms(2028, 2, 29, 12, 0, 0).unwrap();
        assert_eq!(tracker.can_intervene(leap_day), CooldownStatus::Allowed);

        tracker.record_intervention(leap_day, ActionCandidate::Remind);
        let march_1 = Utc.with_ymd_and_hms(2028, 3, 1, 12, 0, 0).unwrap();
        assert_eq!(tracker.can_intervene(march_1), CooldownStatus::Allowed);
    }

    #[test]
    fn test_cooldown_sub_second_precision() {
        let config = CooldownConfig {
            min_interval_seconds: 60,
            ..Default::default()
        };
        let mut tracker = CooldownTracker::new(config).unwrap();
        let t0 = Utc::now();
        tracker.record_intervention(t0, ActionCandidate::Remind);

        // 59.5 seconds elapsed -> 1 second remaining
        let t_59_5s = t0 + Duration::milliseconds(59500);
        let status = tracker.can_intervene(t_59_5s);
        assert_eq!(status.remaining_seconds(), 1);
    }
}

// =========================================================================
// CATEGORY 4: Cooldown Extremes, Durations, and Burst Calls
// =========================================================================
mod cooldown_and_pacing_extremes {
    use super::*;

    #[test]
    fn test_cooldown_config_validation_extremes() {
        let valid = CooldownConfig::default();

        // min_interval < 60s rejected
        let mut c1 = valid.clone();
        c1.min_interval_seconds = 59;
        assert!(c1.validate().is_err());

        // min_interval > 7200s rejected
        let mut c2 = valid.clone();
        c2.min_interval_seconds = 7201;
        assert!(c2.validate().is_err());

        // rapid_window < min_interval rejected
        let mut c3 = valid.clone();
        c3.min_interval_seconds = 300;
        c3.rapid_window_seconds = 299;
        assert!(c3.validate().is_err());

        // max_interventions_per_window == 0 rejected
        let mut c4 = valid.clone();
        c4.max_interventions_per_window = 0;
        assert!(c4.validate().is_err());

        // correction_suppression < 60s rejected
        let mut c5 = valid.clone();
        c5.correction_suppression_seconds = 59;
        assert!(c5.validate().is_err());

        // max_snooze > 7200s rejected
        let mut c6 = valid.clone();
        c6.max_snooze_seconds = 7201;
        assert!(c6.validate().is_err());

        // default_snooze > max_snooze rejected
        let mut c7 = valid.clone();
        c7.default_snooze_seconds = 1000;
        c7.max_snooze_seconds = 900;
        assert!(c7.validate().is_err());

        // session_cap == 0 rejected
        let mut c8 = valid;
        c8.session_cap = Some(0);
        assert!(c8.validate().is_err());
    }

    #[test]
    fn test_snooze_zero_defaults_and_max_clamped() {
        let mut tracker = CooldownTracker::with_default_config();
        let t0 = Utc::now();

        // 0s snooze defaults to default_snooze_seconds (600s)
        tracker.record_snooze(t0, 0).unwrap();
        assert_eq!(tracker.remaining_snooze_seconds(t0), 600);

        // 99999s snooze clamps to max_snooze_seconds (7200s)
        tracker.clear_snooze();
        tracker.record_snooze(t0, 99999).unwrap();
        assert_eq!(tracker.remaining_snooze_seconds(t0), 7200);
    }

    #[test]
    fn test_rapid_burst_calls_at_identical_timestamps() {
        let mut tracker = CooldownTracker::with_default_config();
        let t0 = Utc::now();

        // 1st call at t0 -> Allowed
        assert_eq!(tracker.can_intervene(t0), CooldownStatus::Allowed);
        tracker.record_intervention(t0, ActionCandidate::Remind);

        // Subsequent 9 calls at exact same microsecond timestamp -> all throttled
        for i in 1..=9 {
            let status = tracker.can_intervene(t0);
            assert!(
                matches!(
                    status,
                    CooldownStatus::Throttled {
                        reason: ThrottlingReason::RecentIntervention,
                        ..
                    }
                ),
                "Burst call {i} must be throttled"
            );
        }
    }

    #[test]
    fn test_session_cap_exact_one() {
        let config = CooldownConfig {
            min_interval_seconds: 60,
            session_cap: Some(1),
            ..Default::default()
        };
        let mut tracker = CooldownTracker::new(config).unwrap();
        let t0 = Utc::now();

        // 1st intervention allowed
        assert_eq!(tracker.can_intervene(t0), CooldownStatus::Allowed);
        tracker.record_intervention(t0, ActionCandidate::Remind);

        // 2nd intervention after interval -> blocked by SessionCapReached
        let t_later = t0 + Duration::seconds(100);
        let status = tracker.can_intervene(t_later);
        assert_eq!(
            status,
            CooldownStatus::Throttled {
                reason: ThrottlingReason::SessionCapReached,
                remaining_seconds: u32::MAX,
            }
        );
    }

    #[test]
    fn test_reset_session_restores_initial_capacity() {
        let mut tracker = CooldownTracker::with_default_config();
        let t0 = Utc::now();

        tracker.record_intervention(t0, ActionCandidate::Remind);
        tracker.record_snooze(t0, 600).unwrap();
        assert!(tracker.can_intervene(t0).is_throttled());

        // Reset
        tracker.reset_session();
        assert_eq!(tracker.total_interventions(), 0);
        assert!(!tracker.is_snoozed(t0));
        assert_eq!(tracker.can_intervene(t0), CooldownStatus::Allowed);
    }

    #[test]
    fn test_memory_bounding_interventions_and_suppressions() {
        let mut tracker = CooldownTracker::with_default_config();
        let t0 = Utc::now();

        // Record 100 interventions (exceeding MAX_TRACKED_INTERVENTIONS = 64)
        for i in 0..100 {
            let ts = t0 + Duration::seconds(i * 300);
            tracker.record_intervention(ts, ActionCandidate::Remind);
        }
        assert_eq!(tracker.total_interventions(), 100);

        // Record 100 distinct suppressions (exceeding MAX_TRACKED_SUPPRESSIONS = 64)
        for i in 0..100 {
            tracker.record_targeted_correction(
                t0,
                CorrectionScope::Session,
                Some(format!("app_{i}")),
                None,
            );
        }
    }

    #[test]
    fn test_idle_cooldown_boundary() {
        let mut tracker = CooldownTracker::with_default_config();
        let t0 = Utc::now();

        tracker.set_idle_state(true, t0);
        assert!(tracker.is_idle());
        let status = tracker.can_intervene(t0);
        assert!(matches!(
            status,
            CooldownStatus::Throttled {
                reason: ThrottlingReason::IdleActive,
                ..
            }
        ));

        // When idle cleared, allowed
        tracker.set_idle_state(false, t0);
        assert!(!tracker.is_idle());
        assert_eq!(tracker.can_intervene(t0), CooldownStatus::Allowed);
    }

    #[test]
    fn test_break_state_remind_action_permitted_when_configured() {
        let config = CooldownConfig {
            allow_break_reminders: true,
            ..Default::default()
        };
        let mut tracker = CooldownTracker::new(config).unwrap();
        let t0 = Utc::now();
        tracker.set_break_state(true, t0);

        // Remind is allowed during break state if allow_break_reminders is true
        let status_remind = tracker.can_intervene_for_action(t0, ActionCandidate::Remind);
        assert_eq!(status_remind, CooldownStatus::Allowed);

        // Other actions are throttled
        let status_other = tracker.can_intervene_for_action(t0, ActionCandidate::AskContext);
        assert!(matches!(
            status_other,
            CooldownStatus::Throttled {
                reason: ThrottlingReason::BreakStateActive,
                ..
            }
        ));
    }

    #[test]
    fn test_break_state_remind_action_prohibited_when_disabled() {
        let config = CooldownConfig {
            allow_break_reminders: false,
            ..Default::default()
        };
        let mut tracker = CooldownTracker::new(config).unwrap();
        let t0 = Utc::now();
        tracker.set_break_state(true, t0);

        // When allow_break_reminders is false, even Remind is throttled
        let status_remind = tracker.can_intervene_for_action(t0, ActionCandidate::Remind);
        assert!(matches!(
            status_remind,
            CooldownStatus::Throttled {
                reason: ThrottlingReason::BreakStateActive,
                ..
            }
        ));
    }

    #[test]
    fn test_cooldown_rapid_window_pruning_boundary() {
        let config = CooldownConfig {
            min_interval_seconds: 60,
            rapid_window_seconds: 300,
            max_interventions_per_window: 1,
            ..Default::default()
        };
        let mut tracker = CooldownTracker::new(config).unwrap();
        let t0 = Utc::now();

        tracker.record_intervention(t0, ActionCandidate::Remind);

        // At t0 + 100s, throttled by RapidSuccessionRateLimit
        let t_within = t0 + Duration::seconds(100);
        assert!(matches!(
            tracker.can_intervene(t_within),
            CooldownStatus::Throttled {
                reason: ThrottlingReason::RapidSuccessionRateLimit,
                ..
            }
        ));

        // At t0 + 301s (past rapid_window_seconds 300s), allowed again
        let t_after_window = t0 + Duration::seconds(301);
        assert_eq!(
            tracker.can_intervene(t_after_window),
            CooldownStatus::Allowed
        );
    }
}

// =========================================================================
// CATEGORY 5: Multibyte Unicode & Escaping Boundaries
// =========================================================================
mod multibyte_unicode_and_escaping_boundaries {
    use super::*;

    #[test]
    fn test_clamp_chars_multibyte_emoji_no_split() {
        let crabs = "🦀🦀🦀🦀🦀"; // 5 crabs * 4 bytes = 20 bytes
        let clamped = PromptSanitizer::clamp_chars(crabs, 3);
        assert_eq!(clamped, "🦀🦀🦀");
        assert_eq!(clamped.len(), 12);
    }

    #[test]
    fn test_clamp_chars_cjk_characters() {
        let cjk = "专注目标与任务完成"; // 9 CJK chars * 3 bytes = 27 bytes
        let clamped = PromptSanitizer::clamp_chars(cjk, 4);
        assert_eq!(clamped, "专注目标");
        assert_eq!(clamped.chars().count(), 4);
    }

    #[test]
    fn test_window_title_with_256_emojis_accepted() {
        // 256 emoji characters = 1024 UTF-8 bytes
        let title_256_emojis = "🚀".repeat(MAX_WINDOW_TITLE_LENGTH);
        assert_eq!(title_256_emojis.chars().count(), 256);
        assert_eq!(title_256_emojis.len(), 1024);

        let obs = UntrustedObservation::new(None, Some(title_256_emojis), None, None);
        assert!(obs.is_ok(), "256 emoji characters must be accepted");

        // 257 emoji characters -> rejected
        let title_257_emojis = "🚀".repeat(MAX_WINDOW_TITLE_LENGTH + 1);
        let err = UntrustedObservation::new(None, Some(title_257_emojis), None, None);
        assert!(err.is_err(), "257 emoji characters must be rejected");
    }

    #[test]
    fn test_multibyte_http_error_truncation_no_panic() {
        let client = OpenAiCompatibleClient::new(OpenAiConfig::ollama("llama3")).unwrap();
        // 499 ASCII characters + 4-byte emoji + 10 extra characters
        let mut msg = "a".repeat(499);
        msg.push('🦀');
        msg.push_str("extra content");

        // Truncation at 500 characters must cleanly slice at codepoint boundary without panicking
        let err = client.map_http_error(500, &msg);
        assert!(err.to_string().contains("🦀..."));
    }

    #[test]
    fn test_trojan_source_bidi_isolates_stripped() {
        // CVE-2021-42574 Trojan source characters:
        // U+2066 (LRI), U+2067 (RLI), U+2068 (FSI), U+2069 (PDI)
        let attack = "function\u{2066}CheckAdmin\u{2069}() /*\u{2067} admin check \u{2069}*/";
        let cleaned = PromptSanitizer::strip_control_and_bidi(attack);
        assert_eq!(cleaned, "functionCheckAdmin() /* admin check */");
    }

    #[test]
    fn test_zero_width_and_bom_characters_stripped() {
        let hidden = "safe\u{200B}token\u{200C}leak\u{200D}payload\u{FEFF}";
        let cleaned = PromptSanitizer::strip_control_and_bidi(hidden);
        assert_eq!(cleaned, "safetokenleakpayload");
    }

    #[test]
    fn test_xml_escaping_repeated_meta_characters() {
        let meta = "<<<<<< &&&&&& >>>>>> \"\"\"\"\"\" ''''''";
        let escaped = PromptSanitizer::escape_xml(meta);
        assert_eq!(
            escaped,
            "&lt;&lt;&lt;&lt;&lt;&lt; &amp;&amp;&amp;&amp;&amp;&amp; &gt;&gt;&gt;&gt;&gt;&gt; &quot;&quot;&quot;&quot;&quot;&quot; &apos;&apos;&apos;&apos;&apos;&apos;"
        );
    }

    #[test]
    fn test_coach_name_with_multibyte_cjk_accepted() {
        let cjk_name = "专注助手";
        assert!(TonePreferences::new(cjk_name, PersonalityMode::Supportive).is_ok());
    }

    #[test]
    fn test_sanitize_text_max_len_zero() {
        let text = "Hello world";
        assert_eq!(PromptSanitizer::sanitize_text(text, 0), "");
    }

    #[test]
    fn test_sanitize_text_with_c1_controls() {
        // U+0080 through U+009F are C1 controls
        let raw = "Pre\u{0080}\u{009F}Post";
        let cleaned = PromptSanitizer::strip_control_and_bidi(raw);
        assert_eq!(cleaned, "PrePost");
    }
}

// =========================================================================
// CATEGORY 6: Empty Collections, Empty Strings, Missing Fields & Malformed JSON
// =========================================================================
mod empty_collections_and_malformed_envelopes {
    use super::*;

    #[test]
    fn test_empty_and_whitespace_goal_titles_rejected() {
        let empty_cases = ["", " ", "   ", "\t", "\n", "\r\n", "  \t\n  "];
        for empty in empty_cases {
            let err = CoachContext::new(
                empty,
                None,
                SessionId::new(),
                0,
                false,
                UntrustedObservation::default(),
                Vec::new(),
                TonePreferences::default(),
            );
            assert!(
                err.is_err(),
                "Empty/whitespace goal title '{empty:?}' must be rejected"
            );
        }
    }

    #[test]
    fn test_empty_and_whitespace_rationale_rejected() {
        let prov = ProviderProvenance::mock(Utc::now());
        let empty_cases = ["", " ", "   ", "\t", "\n", "  \t\n  "];
        for empty in empty_cases {
            let err = CoachResponse::new(
                RelevanceClassification::Relevant,
                0.9,
                None,
                empty,
                None,
                None,
                prov.clone(),
            );
            assert!(
                err.is_err(),
                "Empty/whitespace rationale '{empty:?}' must be rejected"
            );
        }
    }

    #[test]
    fn test_empty_and_whitespace_question_rejected() {
        let prov = ProviderProvenance::mock(Utc::now());
        let empty_cases = ["", " ", "   ", "\t", "\n"];
        for empty in empty_cases {
            let err = CoachResponse::new(
                RelevanceClassification::PossiblyUnrelated,
                0.8,
                None,
                "Valid rationale",
                Some(empty.to_string()),
                None,
                prov.clone(),
            );
            assert!(
                err.is_err(),
                "Empty/whitespace question '{empty:?}' must be rejected"
            );
        }
    }

    #[test]
    fn test_empty_untrusted_observation_semantics() {
        let default_obs = UntrustedObservation::default();
        assert!(default_obs.is_empty());
        assert_eq!(default_obs.effective_idle_state(), IdleState::Active);

        let wrapped = PromptSanitizer::wrap_untrusted_observation(&default_obs);
        assert!(wrapped.contains("<status>none</status>"));
        assert!(wrapped.contains("<idle_state>active</idle_state>"));
    }

    #[test]
    fn test_empty_user_corrections_list_semantics() {
        let wrapped = PromptSanitizer::wrap_user_corrections(&[]);
        assert!(wrapped.contains("<user_corrections>"));
        assert!(wrapped.contains("<status>none</status>"));
        assert!(wrapped.contains("</user_corrections>"));
    }

    #[test]
    fn test_malformed_outer_json_strings_rejected() {
        // Completely empty string
        assert!(CoachRawResponse::from_json("").is_err());

        // Whitespace only
        assert!(CoachRawResponse::from_json("   \n\t  ").is_err());

        // Truncated / unclosed JSON object
        assert!(CoachRawResponse::from_json(r#"{"relevance": "relevant""#).is_err());

        // Trailing garbage / syntax error
        assert!(CoachRawResponse::from_json(r#"{"relevance": "relevant",,,,}"#).is_err());
    }

    #[test]
    fn test_outer_json_primitive_or_array_rejected() {
        // JSON boolean
        assert!(CoachRawResponse::from_json("true").is_err());

        // JSON number
        assert!(CoachRawResponse::from_json("42").is_err());

        // JSON string
        assert!(CoachRawResponse::from_json(r#""relevant""#).is_err());

        // JSON array
        assert!(CoachRawResponse::from_json(r#"["relevant", 0.95]"#).is_err());
    }

    #[test]
    fn test_missing_required_fields_rejected() {
        // Missing "relevance"
        let missing_rel = r#"{
            "confidence": 0.95,
            "rationale": "Missing relevance field"
        }"#;
        assert!(matches!(
            CoachRawResponse::from_json(missing_rel),
            Err(CoachError::SchemaValidation { ref field, .. }) if field == "relevance"
        ));

        // Missing "confidence"
        let missing_conf = r#"{
            "relevance": "relevant",
            "rationale": "Missing confidence field"
        }"#;
        assert!(matches!(
            CoachRawResponse::from_json(missing_conf),
            Err(CoachError::SchemaValidation { ref field, .. }) if field == "confidence"
        ));

        // Missing "rationale"
        let missing_rat = r#"{
            "relevance": "relevant",
            "confidence": 0.95
        }"#;
        assert!(matches!(
            CoachRawResponse::from_json(missing_rat),
            Err(CoachError::SchemaValidation { ref field, .. }) if field == "rationale"
        ));
    }

    #[test]
    fn test_null_for_required_fields_rejected() {
        let null_rel = r#"{
            "relevance": null,
            "confidence": 0.9,
            "rationale": "Null relevance"
        }"#;
        assert!(CoachRawResponse::from_json(null_rel).is_err());

        let null_conf = r#"{
            "relevance": "relevant",
            "confidence": null,
            "rationale": "Null confidence"
        }"#;
        assert!(CoachRawResponse::from_json(null_conf).is_err());

        let null_rat = r#"{
            "relevance": "relevant",
            "confidence": 0.9,
            "rationale": null
        }"#;
        assert!(CoachRawResponse::from_json(null_rat).is_err());
    }

    #[test]
    fn test_unrecognized_relevance_variant_rejected() {
        let bad_rel = r#"{
            "relevance": "partially_productive",
            "confidence": 0.8,
            "rationale": "Invalid enum variant"
        }"#;
        let err = CoachRawResponse::from_json(bad_rel).unwrap_err();
        assert!(matches!(
            err,
            CoachError::SchemaValidation { ref field, .. } if field == "relevance"
        ));
    }

    #[test]
    fn test_extra_unauthorized_json_fields_discarded_safely() {
        let extra_fields_json = r#"{
            "relevance": "relevant",
            "confidence": 0.95,
            "rationale": "Legitimate rationale",
            "unauthorized_field": "sudo rm -rf /",
            "shell_exec": true,
            "os_command": "killall cargo"
        }"#;

        let raw = CoachRawResponse::from_json(extra_fields_json)
            .expect("Extra unauthorized fields must be discarded safely without error");
        assert_eq!(raw.relevance, RelevanceClassification::Relevant);
        assert_eq!(raw.confidence, 0.95);
        assert_eq!(raw.rationale, "Legitimate rationale");
    }

    #[test]
    fn test_json_array_root_error_type() {
        let err = CoachRawResponse::from_json("[]").expect_err("JSON array must be rejected");
        assert!(matches!(
            err,
            CoachError::SchemaValidation { ref field, .. } if field == "root"
        ));
    }

    #[test]
    fn test_raw_response_from_json_non_string_question() {
        let json_num_q = r#"{
            "relevance": "relevant",
            "confidence": 0.9,
            "rationale": "Valid rationale",
            "question": 12345
        }"#;
        let err = CoachRawResponse::from_json(json_num_q)
            .expect_err("Non-string question must be rejected");
        assert!(matches!(
            err,
            CoachError::SchemaValidation { ref field, .. } if field == "question"
        ));
    }
}
