use std::path::PathBuf;
use wowie_avatar::model::CompanionMood;
use wowie_avatar::speech::{detect_available_engine, SpeechSynthesizer, VoiceConfig, VoiceEngine};

#[test]
fn test_voice_config_serde_defaults_and_roundtrip() {
    let default_config = VoiceConfig::default();
    assert!(!default_config.enabled);
    assert_eq!(default_config.engine, VoiceEngine::Auto);
    assert_eq!(default_config.rate, 160);
    assert_eq!(default_config.pitch, 50);
    assert_eq!(default_config.piper_model_path, None);

    let json = serde_json::to_string(&default_config).expect("Serialize VoiceConfig");
    let deserialized: VoiceConfig =
        serde_json::from_str(&json).expect("Deserialize default VoiceConfig");
    assert_eq!(deserialized, default_config);

    // Custom config with Piper engine and model path
    let custom_config = VoiceConfig {
        enabled: true,
        engine: VoiceEngine::Piper,
        rate: 180,
        pitch: 60,
        piper_model_path: Some(PathBuf::from("/usr/share/piper-voices/en_US-lessac.onnx")),
    };

    let custom_json = serde_json::to_string_pretty(&custom_config).expect("Serialize custom");
    let custom_deserialized: VoiceConfig =
        serde_json::from_str(&custom_json).expect("Deserialize custom VoiceConfig");
    assert_eq!(custom_deserialized, custom_config);
    assert_eq!(custom_deserialized.engine, VoiceEngine::Piper);
    assert!(custom_deserialized.piper_model_path.is_some());
}

#[test]
fn test_voice_engine_enum_variants_serde() {
    let engines = vec![
        (VoiceEngine::Auto, "\"auto\""),
        (VoiceEngine::Piper, "\"piper\""),
        (VoiceEngine::EspeakNg, "\"espeak_ng\""),
        (VoiceEngine::SpeechDispatcher, "\"speech_dispatcher\""),
        (VoiceEngine::Mock, "\"mock\""),
    ];

    for (engine, expected_json) in engines {
        let serialized = serde_json::to_string(&engine).expect("Serialize engine");
        assert_eq!(serialized, expected_json);

        let deserialized: VoiceEngine =
            serde_json::from_str(&serialized).expect("Deserialize engine");
        assert_eq!(deserialized, engine);
    }
}

#[test]
fn test_detect_available_engine_offline_fallback() {
    // Under CI or minimal dev environment, must not panic and must return a valid VoiceEngine variant
    let detected = detect_available_engine();
    assert!(matches!(
        detected,
        VoiceEngine::Piper
            | VoiceEngine::EspeakNg
            | VoiceEngine::SpeechDispatcher
            | VoiceEngine::Mock
    ));

    // Initializing synthesizer with Auto resolves to a concrete active engine
    let synth = SpeechSynthesizer::new(VoiceConfig {
        enabled: true,
        engine: VoiceEngine::Auto,
        ..Default::default()
    });
    assert_ne!(synth.active_engine(), &VoiceEngine::Auto);
}

#[tokio::test]
async fn test_async_speech_with_mock_engine() {
    let synth = SpeechSynthesizer::mock();
    assert!(synth.is_enabled());

    // Normal speech
    let res = synth.speak_async("Session started successfully.").await;
    assert!(res.is_ok());

    // Empty or whitespace strings should cleanly no-op
    assert!(synth.speak_async("").await.is_ok());
    assert!(synth.speak_async("   \n\t  ").await.is_ok());

    // Disabled synthesizer should no-op
    let disabled = SpeechSynthesizer::new(VoiceConfig {
        enabled: false,
        engine: VoiceEngine::Mock,
        ..Default::default()
    });
    assert!(disabled.speak_async("Should be ignored").await.is_ok());
}

#[tokio::test]
async fn test_speak_in_background_execution() {
    let synth = SpeechSynthesizer::mock();

    // Fire-and-forget in background
    synth.speak_in_background("Background audio task".to_string());
    synth.speak_in_background(String::new()); // empty ignored

    // Background mood speech
    synth.speak_for_mood_in_background(CompanionMood::Celebrating);
    synth.speak_for_mood_in_background(CompanionMood::Neutral); // no canned line, no-op

    // Yield to let tokio spawned tasks complete
    tokio::task::yield_now().await;
}

#[tokio::test]
async fn test_canned_mood_speech_async() {
    let synth = SpeechSynthesizer::mock();

    let moods = [
        (CompanionMood::Focused, true),
        (CompanionMood::Concerned, true),
        (CompanionMood::Relaxed, true),
        (CompanionMood::Snoozed, true),
        (CompanionMood::Celebrating, true),
        (CompanionMood::Neutral, false),
        (CompanionMood::Thinking, false),
    ];

    for (mood, has_canned_line) in moods {
        assert_eq!(
            SpeechSynthesizer::canned_line_for_mood(mood).is_some(),
            has_canned_line
        );
        let res = synth.speak_for_mood_async(mood).await;
        assert!(res.is_ok());
    }
}
