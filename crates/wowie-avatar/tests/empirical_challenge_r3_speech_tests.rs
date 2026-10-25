//! Empirical Challenge and Stress Test Suite for R3 (wowie-avatar Local Speech & Audio Synthesis)
//!
//! Evaluates:
//! 1. Concurrency and async safety: rapid successive calls to `speak_async` and `speak_in_background`.
//! 2. Input edge cases: empty strings, whitespace, 1MB huge strings, Unicode/emoji, injection attempts.
//! 3. Audio pipeline robustness: behavior when audio utilities (`pw-play`, `aplay`) or TTS engines are missing.
//! 4. Non-blocking behavior and event loop starvation resistance.
//! 5. Piper streaming and draining pipeline verification.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use wowie_avatar::model::CompanionMood;
use wowie_avatar::speech::{detect_available_engine, SpeechSynthesizer, VoiceConfig, VoiceEngine};

// ============================================================================
// 1. Concurrency and Async Safety
// ============================================================================

#[tokio::test]
async fn test_challenge_concurrency_rapid_successive_speak_async() {
    let synth = Arc::new(SpeechSynthesizer::mock());

    let mut handles = Vec::new();
    for i in 0..100 {
        let synth_clone = Arc::clone(&synth);
        handles.push(tokio::spawn(async move {
            let text = format!("Concurrent speech message #{i}");
            synth_clone.speak_async(&text).await
        }));
    }

    for (i, handle) in handles.into_iter().enumerate() {
        let res = handle.await.expect("Task join failed");
        assert!(
            res.is_ok(),
            "Concurrent speech #{i} returned error: {res:?}"
        );
    }
}

#[tokio::test]
async fn test_challenge_concurrency_rapid_successive_speak_in_background() {
    let synth = SpeechSynthesizer::mock();

    let start = Instant::now();
    for i in 0..500 {
        synth.speak_in_background(format!("Background burst message #{i}"));
    }
    let elapsed = start.elapsed();

    // Spawning 500 background tasks must be instantaneous (< 30ms)
    assert!(
        elapsed < Duration::from_millis(30),
        "speak_in_background took too long: {elapsed:?}"
    );

    // Yield to let tokio runtime process spawned tasks
    tokio::time::sleep(Duration::from_millis(50)).await;
}

#[tokio::test]
async fn test_challenge_concurrency_interleaved_async_and_background() {
    let synth = Arc::new(SpeechSynthesizer::mock());

    let mut async_handles = Vec::new();
    for i in 0..50 {
        let s = Arc::clone(&synth);
        async_handles.push(tokio::spawn(async move {
            s.speak_async(&format!("Async task {i}")).await
        }));

        synth.speak_in_background(format!("Interleaved background {i}"));
    }

    for handle in async_handles {
        let res = handle.await.expect("Join async task");
        assert!(res.is_ok());
    }

    tokio::time::sleep(Duration::from_millis(30)).await;
}

#[test]
fn test_challenge_concurrency_background_outside_tokio_runtime() {
    // Calling speak_in_background from a plain thread outside a tokio runtime must not panic.
    let synth = SpeechSynthesizer::mock();
    let thread_handle = std::thread::spawn(move || {
        // tokio::runtime::Handle::try_current() will return Err
        synth.speak_in_background("Non-tokio thread speech".to_string());
        synth.speak_for_mood_in_background(CompanionMood::Focused);
    });

    let res = thread_handle.join();
    assert!(
        res.is_ok(),
        "Thread panicked calling speak_in_background outside runtime"
    );
}

#[tokio::test]
async fn test_challenge_dropped_speak_async_future_does_not_panic() {
    let synth = SpeechSynthesizer::mock();

    // Launch future and drop it immediately before it completes
    let fut = synth.speak_async("Will be dropped immediately");
    drop(fut);

    // Launch inside task and abort task
    let handle = tokio::spawn(async move { synth.speak_async("Abort test").await });
    handle.abort();
    let _ = handle.await; // May return JoinError::Cancelled, must not panic
}

// ============================================================================
// 2. Input Edge Cases & Payloads
// ============================================================================

#[tokio::test]
async fn test_challenge_edge_case_empty_and_whitespace_inputs() {
    let synth = SpeechSynthesizer::mock();

    let whitespace_cases = [
        "",
        " ",
        "   ",
        "\t",
        "\n",
        "\r\n",
        "  \t \n \r  \t  ",
        "\u{00A0}", // Non-breaking space
        "\u{2000}", // En quad
        "\u{2003}", // Em space
        "\u{200B}", // Zero-width space
        "\u{FEFF}", // Byte order mark
    ];

    for case in whitespace_cases {
        assert!(
            synth.speak_async(case).await.is_ok(),
            "Failed on whitespace input: {case:?}"
        );
        assert!(
            synth.speak(case).is_ok(),
            "Failed on sync whitespace input: {case:?}"
        );
        synth.speak_in_background(case.to_string());
    }
}

#[tokio::test]
async fn test_challenge_edge_case_1mb_huge_payload() {
    let synth_mock = SpeechSynthesizer::mock();

    // Construct a 1MB payload (1,048,576 bytes)
    let payload_chunk = "Stay focused on your primary engineering goals! ";
    let repeat_count = (1024 * 1024 / payload_chunk.len()) + 1;
    let huge_text = payload_chunk.repeat(repeat_count);
    assert!(huge_text.len() >= 1024 * 1024);

    // 1. Mock engine handles 1MB string without issue
    let start = Instant::now();
    let res = synth_mock.speak_async(&huge_text).await;
    assert!(res.is_ok());
    assert!(start.elapsed() < Duration::from_millis(50));

    // 2. speak_in_background with 1MB string completes without blocking
    let bg_start = Instant::now();
    synth_mock.speak_in_background(huge_text.clone());
    assert!(bg_start.elapsed() < Duration::from_millis(20));

    // 3. Engines with missing binaries handle 1MB payload without panic
    let synth_piper = SpeechSynthesizer::new(VoiceConfig {
        enabled: true,
        engine: VoiceEngine::Piper,
        ..Default::default()
    });
    let res_piper = synth_piper.speak_async(&huge_text).await;
    assert!(res_piper.is_err()); // Piper binary not present in CI/offline environment

    let synth_espeak = SpeechSynthesizer::new(VoiceConfig {
        enabled: true,
        engine: VoiceEngine::EspeakNg,
        ..Default::default()
    });
    let res_espeak = synth_espeak.speak_async(&huge_text).await;
    assert!(res_espeak.is_err()); // espeak-ng not present or ARG_MAX failure, must not panic

    let synth_spd = SpeechSynthesizer::new(VoiceConfig {
        enabled: true,
        engine: VoiceEngine::SpeechDispatcher,
        ..Default::default()
    });
    let res_spd = synth_spd.speak_async(&huge_text).await;
    assert!(res_spd.is_err());
}

#[tokio::test]
async fn test_challenge_edge_case_multilingual_unicode_and_emojis() {
    let synth = SpeechSynthesizer::mock();

    let multilingual_samples = [
        "🦀 Rust and Wowie! 🚀",
        "保持专注，达成目标！",
        "集中セッションが有効です。フローを維持してください。",
        "집중 세션이 활성화되었습니다.",
        "جلسة التركيز نشطة. ابق في حالة تدفق.",
        "סשן המיקוד פעיל. שמור על ריכוז.",
        "Сеанс фокусировки активен. Оставайтесь в потоке.",
        "फोकस सत्र सक्रिय है। प्रवाह में रहें।",
        "Η συνεδρία εστίασης είναι ενεργή. Μείνετε συγκεντρωμένοι.",
        "👨‍👩‍👧‍👦 👩‍💻 🏳️‍⚧️ 🧑🏾‍🚀 ✨ 🎯 🏆 💡 🔥",
        "Control chars: \x00 \x01 \x07 \x1b[31;1mRed Alert\x1b[0m \t \r\n",
        "Bidi override: \u{202E}dlrow olleH\u{202C}",
    ];

    for sample in multilingual_samples {
        let res_async = synth.speak_async(sample).await;
        assert!(res_async.is_ok(), "Failed async on: {sample}");

        let res_sync = synth.speak(sample);
        assert!(res_sync.is_ok(), "Failed sync on: {sample}");

        synth.speak_in_background(sample.to_string());
    }

    tokio::time::sleep(Duration::from_millis(20)).await;
}

#[tokio::test]
async fn test_challenge_edge_case_shell_injection_and_metacharacters() {
    let canary_file = std::env::temp_dir().join(format!(
        "wowie_speech_canary_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));

    // Ensure canary does not exist
    let _ = fs::remove_file(&canary_file);
    assert!(!canary_file.exists());

    let injection_payloads = [
        format!("; touch {};", canary_file.display()),
        format!("&& touch {} &&", canary_file.display()),
        format!("| touch {}", canary_file.display()),
        format!("`touch {}`", canary_file.display()),
        format!("$(touch {})", canary_file.display()),
        format!("' ; touch {} ; '", canary_file.display()),
        format!("\" ; touch {} ; \"", canary_file.display()),
        format!("\n touch {} \n", canary_file.display()),
    ];

    let synth_mock = SpeechSynthesizer::mock();
    for payload in &injection_payloads {
        let _ = synth_mock.speak_async(payload).await;
        let _ = synth_mock.speak(payload);
        synth_mock.speak_in_background(payload.clone());
        assert!(
            !canary_file.exists(),
            "Security vulnerability: shell command was executed via payload: {payload}"
        );
    }

    // Also test with explicit external engines to verify Command::arg avoids shell execution
    let synth_piper = SpeechSynthesizer::new(VoiceConfig {
        enabled: true,
        engine: VoiceEngine::Piper,
        ..Default::default()
    });
    let synth_espeak = SpeechSynthesizer::new(VoiceConfig {
        enabled: true,
        engine: VoiceEngine::EspeakNg,
        ..Default::default()
    });

    for payload in &injection_payloads {
        let _ = synth_piper.speak_async(payload).await;
        let _ = synth_espeak.speak_async(payload).await;
        assert!(
            !canary_file.exists(),
            "Security vulnerability: shell command executed by piper or espeak: {payload}"
        );
    }
}

// ============================================================================
// 3. Audio Pipeline Robustness & Missing Binaries
// ============================================================================

#[tokio::test]
async fn test_challenge_missing_tts_engines_error_handling() {
    let piper_synth = SpeechSynthesizer::new(VoiceConfig {
        enabled: true,
        engine: VoiceEngine::Piper,
        ..Default::default()
    });
    let espeak_synth = SpeechSynthesizer::new(VoiceConfig {
        enabled: true,
        engine: VoiceEngine::EspeakNg,
        ..Default::default()
    });
    let spd_synth = SpeechSynthesizer::new(VoiceConfig {
        enabled: true,
        engine: VoiceEngine::SpeechDispatcher,
        ..Default::default()
    });

    // When binaries do not exist, calling speak_async must return ExecutionFailed error and NOT panic
    let res_piper = piper_synth.speak_async("Hello piper").await;
    assert!(
        res_piper.is_err(),
        "Expected Piper to fail when binary is missing"
    );
    let err_str = res_piper.unwrap_err().to_string();
    assert!(
        err_str.contains("Piper") || err_str.contains("error"),
        "Unexpected error: {err_str}"
    );

    let res_espeak = espeak_synth.speak_async("Hello espeak").await;
    assert!(
        res_espeak.is_err(),
        "Expected espeak-ng to fail when binary is missing"
    );

    let res_spd = spd_synth.speak_async("Hello spd").await;
    assert!(
        res_spd.is_err(),
        "Expected spd-say to fail when binary is missing"
    );

    // Synchronous speak must also return Result without panicking
    assert!(piper_synth.speak("Sync piper test").is_err());
    assert!(espeak_synth.speak("Sync espeak test").is_err());
    assert!(spd_synth.speak("Sync spd test").is_err());

    // Background speech must catch errors silently and not panic or crash
    piper_synth.speak_in_background("Background piper test".to_string());
    espeak_synth.speak_in_background("Background espeak test".to_string());
    spd_synth.speak_in_background("Background spd test".to_string());

    tokio::time::sleep(Duration::from_millis(30)).await;
}

#[test]
fn test_challenge_detect_engine_robustness_offline() {
    let detected = detect_available_engine();
    assert!(matches!(
        detected,
        VoiceEngine::Piper
            | VoiceEngine::EspeakNg
            | VoiceEngine::SpeechDispatcher
            | VoiceEngine::Mock
    ));

    // Auto resolution must never remain VoiceEngine::Auto
    let synth = SpeechSynthesizer::new(VoiceConfig {
        enabled: true,
        engine: VoiceEngine::Auto,
        ..Default::default()
    });
    assert_ne!(synth.active_engine(), &VoiceEngine::Auto);
}

#[tokio::test]
async fn test_challenge_disabled_synthesizer_bypasses_all_engines() {
    let disabled_piper = SpeechSynthesizer::new(VoiceConfig {
        enabled: false,
        engine: VoiceEngine::Piper,
        ..Default::default()
    });
    let disabled_espeak = SpeechSynthesizer::new(VoiceConfig {
        enabled: false,
        engine: VoiceEngine::EspeakNg,
        ..Default::default()
    });

    // When disabled, speak_async must return Ok(()) without attempting to spawn any processes
    assert!(disabled_piper
        .speak_async("Should be skipped")
        .await
        .is_ok());
    assert!(disabled_piper.speak("Should be skipped").is_ok());
    disabled_piper.speak_in_background("Should be skipped".to_string());

    assert!(disabled_espeak
        .speak_async("Should be skipped")
        .await
        .is_ok());
    assert!(disabled_espeak.speak("Should be skipped").is_ok());
    disabled_espeak.speak_in_background("Should be skipped".to_string());
}

#[tokio::test]
async fn test_challenge_canned_mood_speech_exhaustive_handling() {
    let synth = SpeechSynthesizer::mock();

    let all_moods = [
        CompanionMood::Focused,
        CompanionMood::Concerned,
        CompanionMood::Relaxed,
        CompanionMood::Snoozed,
        CompanionMood::Celebrating,
        CompanionMood::Neutral,
        CompanionMood::Thinking,
    ];

    for mood in all_moods {
        let res_async = synth.speak_for_mood_async(mood).await;
        assert!(res_async.is_ok(), "speak_for_mood_async failed on {mood:?}");

        let res_sync = synth.speak_for_mood(mood);
        assert!(res_sync.is_ok(), "speak_for_mood failed on {mood:?}");

        synth.speak_for_mood_in_background(mood);
    }

    tokio::time::sleep(Duration::from_millis(20)).await;
}

// ============================================================================
// 4. Non-Blocking Behavior and Event Loop Starvation Resistance
// ============================================================================

#[tokio::test]
async fn test_challenge_speak_in_background_is_strictly_non_blocking() {
    let synth = SpeechSynthesizer::mock();

    let count = 200;
    let start = Instant::now();
    for i in 0..count {
        synth.speak_in_background(format!("Non-blocking assertion #{i}"));
    }
    let elapsed = start.elapsed();

    // 200 invocations must take < 15ms total (< 0.075ms per call)
    assert!(
        elapsed < Duration::from_millis(15),
        "speak_in_background is blocking caller thread! Took {elapsed:?} for {count} calls"
    );
}

#[test]
fn test_challenge_async_speech_does_not_starve_single_thread_runtime() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to build current_thread runtime");

    runtime.block_on(async {
        let counter = Arc::new(AtomicUsize::new(0));
        let done = Arc::new(AtomicBool::new(false));

        // Background ticker task on the single thread
        let counter_clone = Arc::clone(&counter);
        let done_clone = Arc::clone(&done);
        tokio::spawn(async move {
            while !done_clone.load(Ordering::Relaxed) {
                counter_clone.fetch_add(1, Ordering::Relaxed);
                tokio::task::yield_now().await;
            }
        });

        let synth = SpeechSynthesizer::mock();
        for _ in 0..20 {
            synth.speak_async("Testing cooperative multitasking").await.expect("Speech ok");
            tokio::task::yield_now().await;
        }

        done.store(true, Ordering::Relaxed);
        let ticks = counter.load(Ordering::Relaxed);
        assert!(
            ticks > 0,
            "Cooperative multitasking starved! Background task did not get scheduled (ticks = {ticks})"
        );
    });
}

// ============================================================================
// 5. Piper Pipeline Streaming & Draining Empirical Verification
// ============================================================================

#[tokio::test]
async fn test_challenge_piper_draining_when_player_absent_simulated() {
    // Test the pipeline behavior when piper produces large audio data (>64KB pipe buffer)
    // and no audio playback sink is available, verifying async_drain prevents deadlocks.
    let temp_sandbox = std::env::temp_dir().join(format!(
        "wowie_piper_drain_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&temp_sandbox).expect("Create sandbox dir");

    // Create a mock 'piper' executable script in sandbox
    let mock_piper_script = temp_sandbox.join("piper");
    let script_content = r"#!/bin/sh
# Consume stdin
/usr/bin/cat > /dev/null
# Output 128KB of dummy raw PCM audio data to stdout
/usr/bin/head -c 131072 /dev/zero
exit 0
";
    fs::write(&mock_piper_script, script_content).expect("Write mock piper");
    let mut perms = fs::metadata(&mock_piper_script)
        .expect("Metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&mock_piper_script, perms).expect("Set executable permissions");

    // Run isolated subprocess with PATH pointing ONLY to temp_sandbox
    // This ensures pw-play and aplay are ABSENT, forcing Piper to use async_drain.
    let output = std::process::Command::new(std::env::current_exe().expect("current exe"))
        .arg("--nocapture")
        .arg("test_isolated_piper_drain_target")
        .env("PATH", &temp_sandbox)
        .env("WOWIE_TEST_PIPER_DRAIN_RUNNER", "1")
        .output();

    // Clean up sandbox
    let _ = fs::remove_dir_all(&temp_sandbox);

    let out = output.expect("Execute child drain test");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    println!("Child drain stdout:\n{stdout}");
    println!("Child drain stderr:\n{stderr}");

    assert!(
        out.status.success(),
        "Isolated Piper drain runner failed with status: {:?}\nStdout: {stdout}\nStderr: {stderr}",
        out.status.code()
    );
}

// Internal target executed by the isolated child process above
#[tokio::test]
async fn test_isolated_piper_drain_target() {
    if std::env::var("WOWIE_TEST_PIPER_DRAIN_RUNNER").is_err() {
        return; // Only run when explicitly invoked in isolated child process
    }

    let config = VoiceConfig {
        enabled: true,
        engine: VoiceEngine::Piper,
        rate: 160,
        pitch: 50,
        piper_model_path: Some(PathBuf::from("/mock/model.onnx")),
    };
    let synth = SpeechSynthesizer::new(config);

    // Speak text through mock piper with 128KB raw PCM output and no audio player in PATH
    let start = Instant::now();
    let res = synth
        .speak_async("Session started! Stay focused and productive.")
        .await;
    let elapsed = start.elapsed();

    assert!(res.is_ok(), "Piper drain failed: {res:?}");
    assert!(
        elapsed < Duration::from_secs(5),
        "Piper async_drain took too long (possible pipe deadlock): {elapsed:?}"
    );
}

#[tokio::test]
async fn test_challenge_piper_piping_to_player_simulated() {
    // Test the pipeline behavior when piper produces audio data AND mock pw-play receives it
    let temp_sandbox = std::env::temp_dir().join(format!(
        "wowie_piper_play_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&temp_sandbox).expect("Create sandbox dir");

    let received_audio_file = temp_sandbox.join("received.pcm");

    // Mock piper script
    let mock_piper_script = temp_sandbox.join("piper");
    let piper_script_content = r"#!/bin/sh
/usr/bin/cat > /dev/null
/usr/bin/head -c 65536 /dev/zero
exit 0
";
    fs::write(&mock_piper_script, piper_script_content).expect("Write mock piper");
    let mut perms = fs::metadata(&mock_piper_script)
        .expect("Metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&mock_piper_script, perms).expect("Set exec permissions");

    // Mock pw-play script
    let mock_player_script = temp_sandbox.join("pw-play");
    let player_script_content = format!(
        r#"#!/bin/sh
if [ "$1" = "--version" ]; then
    echo "pw-play mock 1.0"
    exit 0
fi
/usr/bin/cat > "{}"
exit 0
"#,
        received_audio_file.display()
    );
    fs::write(&mock_player_script, player_script_content).expect("Write mock pw-play");
    let mut pperms = fs::metadata(&mock_player_script)
        .expect("Metadata")
        .permissions();
    pperms.set_mode(0o755);
    fs::set_permissions(&mock_player_script, pperms).expect("Set exec permissions");

    // Run isolated subprocess with PATH pointing to temp_sandbox
    let output = std::process::Command::new(std::env::current_exe().expect("current exe"))
        .arg("--nocapture")
        .arg("test_isolated_piper_player_target")
        .env("PATH", &temp_sandbox)
        .env("WOWIE_TEST_PIPER_PLAYER_RUNNER", "1")
        .output();

    let out = output.expect("Failed to execute child test runner");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    println!("Child player stdout:\n{stdout}");
    println!("Child player stderr:\n{stderr}");

    assert!(
        out.status.success(),
        "Isolated Piper player runner failed with status: {:?}\nStdout: {stdout}\nStderr: {stderr}",
        out.status.code()
    );

    let file_exists = received_audio_file.exists();
    let file_size = if file_exists {
        fs::metadata(&received_audio_file).map_or(0, |m| m.len())
    } else {
        0
    };

    // Clean up sandbox
    let _ = fs::remove_dir_all(&temp_sandbox);

    assert!(file_exists, "Player did not receive audio output file");
    assert_eq!(
        file_size, 65536,
        "Player did not receive full piped 64KB audio data"
    );
}

// Internal target executed by isolated player test
#[tokio::test]
async fn test_isolated_piper_player_target() {
    if std::env::var("WOWIE_TEST_PIPER_PLAYER_RUNNER").is_err() {
        return;
    }

    let config = VoiceConfig {
        enabled: true,
        engine: VoiceEngine::Piper,
        rate: 160,
        pitch: 50,
        piper_model_path: None,
    };
    let synth = SpeechSynthesizer::new(config);

    let res = synth
        .speak_async("Session completed! Congratulations!")
        .await;
    assert!(res.is_ok(), "Piper play piping failed: {res:?}");
}
