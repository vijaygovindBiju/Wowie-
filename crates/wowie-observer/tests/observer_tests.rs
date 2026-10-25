use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;
use wowie_core::id::SessionId;
use wowie_core::models::{ConsentSnapshot, IdleState, ObservationSource};
use wowie_observer::error::ObserverError;
use wowie_observer::model::{MonitoringState, ObserverConfig, WindowMetadata};
use wowie_observer::observer::ActivityObserver;
use wowie_observer::platform::detect_platform_observer;
use wowie_observer::platform::hyprland::HyprlandAdapter;
use wowie_observer::platform::mock::MockObserver;

#[tokio::test]
async fn test_observer_requires_explicit_consent() {
    let mock = Arc::new(MockObserver::with_fixed(WindowMetadata::new(
        Some("code".to_string()),
        Some("main.rs".to_string()),
        None,
        false,
    )));
    let observer = ActivityObserver::new(mock, ObserverConfig::default());
    observer.start().await.expect("Starts observer");

    let session_id = SessionId::from_uuid(Uuid::new_v4());
    let mut consent = ConsentSnapshot {
        monitoring_enabled: false, // Disabled!
        window_title_redaction_enabled: false,
        browser_origin_enabled: false,
        granted_at: Utc::now(),
    };

    let result = observer.sample(session_id, &consent, Utc::now()).await;
    assert_eq!(result.err(), Some(ObserverError::ConsentNotGranted));

    // Now enable consent
    consent.monitoring_enabled = true;
    let observation = observer
        .sample(session_id, &consent, Utc::now())
        .await
        .expect("Sampling succeeds with consent");

    assert_eq!(observation.app_id.as_deref(), Some("code"));
    assert_eq!(observation.window_title.as_deref(), Some("main.rs"));
    assert_eq!(observation.source, ObservationSource::WindowTitle);
}

#[tokio::test]
async fn test_window_title_redaction_and_origin_privacy() {
    let mock = Arc::new(MockObserver::with_fixed(WindowMetadata::new(
        Some("brave-browser".to_string()),
        Some("Confidential Project Doc - Brave".to_string()),
        Some("docs.google.com".to_string()),
        false,
    )));
    let observer = ActivityObserver::new(mock, ObserverConfig::default());
    observer.start().await.expect("Starts observer");

    let session_id = SessionId::from_uuid(Uuid::new_v4());
    let consent = ConsentSnapshot {
        monitoring_enabled: true,
        window_title_redaction_enabled: true, // Redact window titles
        browser_origin_enabled: false,        // Do NOT collect browser origins
        granted_at: Utc::now(),
    };

    let obs = observer
        .sample(session_id, &consent, Utc::now())
        .await
        .expect("Observation succeeds");

    assert_eq!(obs.app_id.as_deref(), Some("brave-browser"));
    assert_eq!(obs.window_title.as_deref(), Some("[Redacted Title]"));
    assert_eq!(obs.origin, None, "Origin must be None when not consented");
}

#[tokio::test]
async fn test_observer_pause_and_stop_controls() {
    let mock = Arc::new(MockObserver::with_fixed(WindowMetadata::new(
        Some("terminal".to_string()),
        Some("bash".to_string()),
        None,
        false,
    )));
    let observer = ActivityObserver::new(mock, ObserverConfig::default());
    let session_id = SessionId::from_uuid(Uuid::new_v4());
    let consent = ConsentSnapshot {
        monitoring_enabled: true,
        window_title_redaction_enabled: false,
        browser_origin_enabled: false,
        granted_at: Utc::now(),
    };

    // Before start: stopped
    assert_eq!(observer.current_state().await, MonitoringState::Stopped);
    assert!(observer
        .sample(session_id, &consent, Utc::now())
        .await
        .is_err());

    // Start
    observer.start().await.unwrap();
    assert_eq!(observer.current_state().await, MonitoringState::Active);
    assert!(observer
        .sample(session_id, &consent, Utc::now())
        .await
        .is_ok());

    // Pause
    observer.pause().await;
    assert_eq!(observer.current_state().await, MonitoringState::Paused);
    let pause_err = observer
        .sample(session_id, &consent, Utc::now())
        .await
        .err();
    assert_eq!(pause_err, Some(ObserverError::MonitoringPaused));

    // Resume
    observer.resume().await.unwrap();
    assert_eq!(observer.current_state().await, MonitoringState::Active);
    assert!(observer
        .sample(session_id, &consent, Utc::now())
        .await
        .is_ok());

    // Stop
    observer.stop().await;
    assert_eq!(observer.current_state().await, MonitoringState::Stopped);
    assert!(observer
        .sample(session_id, &consent, Utc::now())
        .await
        .is_err());
}

#[tokio::test]
async fn test_idle_detection_threshold() {
    let mock = Arc::new(MockObserver::with_fixed(WindowMetadata::new(
        Some("code".to_string()),
        Some("main.rs".to_string()),
        None,
        false,
    )));
    let config = ObserverConfig {
        idle_threshold_seconds: 60,
        ..Default::default()
    };
    let observer = ActivityObserver::new(mock, config);
    observer.start().await.unwrap();

    let session_id = SessionId::from_uuid(Uuid::new_v4());
    let consent = ConsentSnapshot {
        monitoring_enabled: true,
        window_title_redaction_enabled: false,
        browser_origin_enabled: false,
        granted_at: Utc::now(),
    };

    let start_time = Utc::now();
    let obs1 = observer
        .sample(session_id, &consent, start_time)
        .await
        .unwrap();
    assert_eq!(obs1.idle_state, IdleState::Active);

    // Sample at 30s: still active
    let mid_time = start_time + chrono::Duration::seconds(30);
    let obs2 = observer
        .sample(session_id, &consent, mid_time)
        .await
        .unwrap();
    assert_eq!(obs2.idle_state, IdleState::Active);

    // Sample at 65s without change: idle!
    let idle_time = start_time + chrono::Duration::seconds(65);
    let obs3 = observer
        .sample(session_id, &consent, idle_time)
        .await
        .unwrap();
    assert_eq!(obs3.idle_state, IdleState::Idle);
}

#[test]
fn test_hyprland_origin_extraction_heuristics() {
    assert_eq!(
        HyprlandAdapter::extract_origin("brave-browser", "(176) Rust Tutorial - YouTube - Brave"),
        Some("youtube.com".to_string())
    );
    assert_eq!(
        HyprlandAdapter::extract_origin(
            "firefox",
            "wowie/crates/wowie-coach at main · GitHub - Mozilla Firefox"
        ),
        Some("github.com".to_string())
    );
    assert_eq!(
        HyprlandAdapter::extract_origin(
            "google-chrome",
            "Reddit - Dive into anything - Google Chrome"
        ),
        Some("reddit.com".to_string())
    );
    assert_eq!(
        HyprlandAdapter::extract_origin("code", "Rust Tutorial - YouTube in VSCode"),
        None,
        "Non-browser application must not extract origin"
    );
}

#[test]
fn test_extract_browser_origin_comprehensive() {
    use wowie_observer::model::extract_browser_origin;

    // Direct services
    assert_eq!(
        extract_browser_origin("firefox", "Twitch Streamer - Twitch - Firefox"),
        Some("twitch.tv".to_string())
    );
    assert_eq!(
        extract_browser_origin("chromium", "Stranger Things - Netflix - Chromium"),
        Some("netflix.com".to_string())
    );
    assert_eq!(
        extract_browser_origin("brave-browser", "Coursera | Online Courses - Brave"),
        Some("coursera.org".to_string())
    );

    // Embedded domains in title
    assert_eq!(
        extract_browser_origin(
            "firefox",
            "tokio - crates.io: Rust Package Registry - Mozilla Firefox"
        ),
        Some("crates.io".to_string())
    );
    assert_eq!(
        extract_browser_origin("zen", "https://news.ycombinator.com/item?id=123 - Zen"),
        Some("news.ycombinator.com".to_string())
    );
    assert_eq!(
        extract_browser_origin("floorp", "archlinux.org - Package Search - Floorp"),
        Some("archlinux.org".to_string())
    );

    // Non-browser ignores domain
    assert_eq!(
        extract_browser_origin("alacritty", "crates.io in bash terminal"),
        None
    );
}

#[test]
fn test_sway_focused_node_parsing() {
    use serde_json::json;
    use wowie_observer::platform::sway::SwayAdapter;

    let tree = json!({
        "id": 1,
        "name": "root",
        "nodes": [
            {
                "id": 2,
                "name": "workspace 1",
                "nodes": [
                    {
                        "id": 3,
                        "app_id": "firefox",
                        "name": "Rust Documentation - docs.rs - Mozilla Firefox",
                        "focused": true
                    }
                ]
            }
        ]
    });

    let (app_id, name) = SwayAdapter::find_focused_node(&tree).expect("Finds focused node");
    assert_eq!(app_id.as_deref(), Some("firefox"));
    assert_eq!(
        name.as_deref(),
        Some("Rust Documentation - docs.rs - Mozilla Firefox")
    );
}

#[tokio::test]
async fn test_platform_detector_hyprland_on_arch() {
    let detector = detect_platform_observer();
    // On this Arch Linux machine running Hyprland, it should detect hyprland!
    if std::env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase()
        .contains("hyprland")
    {
        assert_eq!(detector.name(), "hyprland");
        assert!(detector.is_available());
        let sample_res = detector.sample_active_window().await;
        assert!(sample_res.is_ok(), "Sampling on Hyprland should succeed");
    }
}

#[tokio::test]
async fn test_observer_integrates_native_messaging_origin_cache() {
    let mock = Arc::new(MockObserver::with_fixed(WindowMetadata::new(
        Some("brave-browser".to_string()),
        Some("Untitled Page".to_string()), // Title heuristic will yield None
        None,
        false,
    )));
    let observer = ActivityObserver::new(mock, ObserverConfig::default());
    observer.start().await.expect("Starts observer");

    // Write to native messaging cache
    let cache_path = wowie_observer::native_messaging::default_origin_cache_path();
    wowie_observer::native_messaging::write_cached_origin(
        &cache_path,
        "https://crates.io",
        true,
        Some("crates.io"),
    )
    .expect("Writes cache");

    let session_id = SessionId::from_uuid(Uuid::new_v4());
    let consent = ConsentSnapshot {
        monitoring_enabled: true,
        window_title_redaction_enabled: false,
        browser_origin_enabled: true,
        granted_at: Utc::now(),
    };

    let observation = observer
        .sample(session_id, &consent, Utc::now())
        .await
        .expect("Sampling succeeds");

    assert_eq!(observation.origin.as_deref(), Some("https://crates.io"));
    assert_eq!(observation.source, ObservationSource::BrowserOrigin);
}
