use wowie_observer::model::WindowMetadata;
use wowie_observer::platform::gnome::GnomeAdapter;
use wowie_observer::platform::kde::KdeAdapter;

#[test]
fn test_gnome_adapter_adversarial_malformed_json() {
    // 1. Empty and whitespace-only strings must return WindowMetadata::empty() cleanly
    let empty_inputs = ["", "   ", "\t\t", "\n\r\n", "   \t\r\n   ", "{}", "[]"];
    for raw in empty_inputs {
        let res = GnomeAdapter::parse_windows_json(raw)
            .unwrap_or_else(|e| panic!("Failed on input {raw:?}: {e}"));
        assert_eq!(
            res,
            WindowMetadata::empty(),
            "Expected empty metadata for {raw:?}"
        );
    }

    // 2. Syntax-corrupted JSON must return Err(ObserverError::DeserializationFailed) and NEVER panic
    let broken_syntax = [
        "{ invalid json }",
        "{",
        "}",
        "{ \"id\": ",
        "<xml><window>firefox</window></xml>",
        "\0\0\0",
        "{\"key\": \"val\"",
        "{ 123: 456 }",
        "\"raw string literal\"",
        "42",
        "3.14159",
        "true",
        "false",
        "null",
        "{ \"1\": { \"id\": } }",
    ];

    for raw in broken_syntax {
        let res = GnomeAdapter::parse_windows_json(raw);
        assert!(
            res.is_err(),
            "Corrupted JSON {raw:?} must return Err, got Ok({res:?})"
        );
    }
}

#[test]
fn test_gnome_adapter_missing_keys_and_extreme_window_ids() {
    // 1. Window object with missing keys (defaults must populate without panic)
    let json_missing_all = r#"{
        "1": {}
    }"#;
    let res = GnomeAdapter::parse_windows_json(json_missing_all).expect("Must parse empty object");
    assert_eq!(res, WindowMetadata::empty()); // has-focus is false by default

    // 2. Window object with only has-focus: true
    let json_only_focus = r#"{
        "42": {
            "has-focus": true
        }
    }"#;
    let res = GnomeAdapter::parse_windows_json(json_only_focus).expect("Must parse focus only");
    assert_eq!(res.app_id, None);
    assert_eq!(res.window_title, None);
    assert_eq!(res.origin, None);
    assert!(!res.is_idle);

    // 3. Fallback from empty app-id to wm-class
    let json_wm_fallback = r#"{
        "99": {
            "id": 99,
            "has-focus": true,
            "title": "crates.io: Rust Package Registry - Mozilla Firefox",
            "wm-class": "firefox",
            "app-id": ""
        }
    }"#;
    let res =
        GnomeAdapter::parse_windows_json(json_wm_fallback).expect("Must parse wm-class fallback");
    assert_eq!(res.app_id.as_deref(), Some("firefox"));
    assert_eq!(res.origin.as_deref(), Some("crates.io"));

    // 4. Extreme numeric window IDs: 0, u64::MAX, huge string keys
    let json_extreme_ids = format!(
        r#"{{
            "0": {{
                "id": 0,
                "has-focus": false,
                "app-id": "zero"
            }},
            "{max_id}": {{
                "id": {max_id},
                "has-focus": true,
                "title": "YouTube - Google Chrome",
                "app-id": "google-chrome"
            }},
            "arbitrary_string_window_id_99999": {{
                "id": 12345,
                "has-focus": false,
                "app-id": "string-id"
            }}
        }}"#,
        max_id = u64::MAX
    );
    let res = GnomeAdapter::parse_windows_json(&json_extreme_ids)
        .expect("Must handle extreme u64 window IDs");
    assert_eq!(res.app_id.as_deref(), Some("google-chrome"));
    assert_eq!(res.origin.as_deref(), Some("youtube.com"));

    // 5. Incompatible ID types (negative integer, float, string) must fail cleanly without panic
    let bad_id_types = [
        r#"{"1": {"id": -1, "has-focus": true}}"#,
        r#"{"1": {"id": 12.34, "has-focus": true}}"#,
        r#"{"1": {"id": "1", "has-focus": true}}"#,
        r#"{"1": {"id": 9999999999999999999999999999999999999999999, "has-focus": true}}"#,
    ];
    for bad_json in bad_id_types {
        let res = GnomeAdapter::parse_windows_json(bad_json);
        assert!(
            res.is_err(),
            "Type violation for id {bad_json} should return Err"
        );
    }

    // 6. Array of windows instead of Map must fail cleanly
    let array_json = r#"[{"id": 1, "has-focus": true, "app-id": "firefox"}]"#;
    let res = GnomeAdapter::parse_windows_json(array_json);
    assert!(res.is_err(), "Array JSON must return Err");
}

#[test]
fn test_gnome_adapter_unicode_and_pathological_titles() {
    // 1. Right-to-Left Override and BiDi controls
    let bidi_title = "\u{202E}txt.evilexe\u{202C} - Google Chrome";
    let bidi_json = format!(
        r#"{{
            "1": {{
                "id": 1,
                "title": "{bidi_title}",
                "app-id": "google-chrome",
                "has-focus": true
            }}
        }}"#
    );
    let res = GnomeAdapter::parse_windows_json(&bidi_json).expect("BiDi title parse");
    assert_eq!(res.app_id.as_deref(), Some("google-chrome"));
    assert_eq!(res.window_title.as_deref(), Some(bidi_title));

    // 2. Complex graphemes and emojis
    let emoji_title = "👨‍👩‍👧‍👦 Family & 🦀 Rust 🚀 - Mozilla Firefox";
    let emoji_json = format!(
        r#"{{
            "2": {{
                "id": 2,
                "title": "{emoji_title}",
                "app-id": "firefox",
                "has-focus": true
            }}
        }}"#
    );
    let res = GnomeAdapter::parse_windows_json(&emoji_json).expect("Emoji title parse");
    assert_eq!(res.app_id.as_deref(), Some("firefox"));
    assert_eq!(res.window_title.as_deref(), Some(emoji_title));

    // 3. Multi-lingual non-ASCII (CJK, Cyrillic, Arabic, Greek, Hindi)
    let cjk_title = "ドキュメント - Google Chrome";
    let cjk_json = format!(
        r#"{{
            "3": {{
                "id": 3,
                "title": "{cjk_title}",
                "app-id": "google-chrome",
                "has-focus": true
            }}
        }}"#
    );
    let res = GnomeAdapter::parse_windows_json(&cjk_json).expect("CJK title parse");
    assert_eq!(res.app_id.as_deref(), Some("google-chrome"));

    // 4. HTML script tags and injection patterns
    let html_title = "<script>alert('xss')</script> - Chromium";
    let html_json = format!(
        r#"{{
            "4": {{
                "id": 4,
                "title": "{html_title}",
                "app-id": "chromium",
                "has-focus": true
            }}
        }}"#
    );
    let res = GnomeAdapter::parse_windows_json(&html_json).expect("HTML title parse");
    assert_eq!(res.app_id.as_deref(), Some("chromium"));

    // 5. Massive 500KB title string
    let huge_title = format!("{} - Mozilla Firefox", "A".repeat(500_000));
    let huge_json = format!(
        r#"{{
            "5": {{
                "id": 5,
                "title": "{huge_title}",
                "app-id": "firefox",
                "has-focus": true
            }}
        }}"#
    );
    let res = GnomeAdapter::parse_windows_json(&huge_json).expect("500KB title parse");
    assert_eq!(res.app_id.as_deref(), Some("firefox"));
    assert_eq!(res.window_title.as_ref().map(|s| s.len()), Some(500_018));
}

#[test]
fn test_kde_adapter_spoofed_browser_suffixes() {
    // 1. Valid browser suffixes must infer correctly
    let valid_pairs = [
        ("Dashboard - Mozilla Firefox", Some("firefox")),
        ("Dashboard — Mozilla Firefox", Some("firefox")),
        ("Dashboard – Mozilla Firefox", Some("firefox")),
        ("Dashboard - Firefox", Some("firefox")),
        ("Video - Google Chrome", Some("google-chrome")),
        ("Video — Google Chrome", Some("google-chrome")),
        ("Video – Google Chrome", Some("google-chrome")),
        ("Repo - Chromium", Some("chromium")),
        ("Repo — Chromium", Some("chromium")),
        ("Repo – Chromium", Some("chromium")),
        ("Search - Brave", Some("brave")),
        ("Search — Brave", Some("brave")),
        ("Search - Brave Browser", Some("brave")),
        ("Docs - Microsoft Edge", Some("microsoft-edge")),
        ("Docs — Microsoft Edge", Some("microsoft-edge")),
        ("Docs – Microsoft Edge", Some("microsoft-edge")),
        ("Site - Vivaldi", Some("vivaldi")),
        ("Site — Vivaldi", Some("vivaldi")),
        ("Music - Opera", Some("opera")),
        ("Music — Opera", Some("opera")),
        ("Home - Zen Browser", Some("zen")),
        ("Home — Zen Browser", Some("zen")),
        ("Test - Floorp", Some("floorp")),
        ("Test — Floorp", Some("floorp")),
        ("Wiki - LibreWolf", Some("librewolf")),
        ("Wiki — LibreWolf", Some("librewolf")),
        // Suffix with trailing whitespace should still infer due to trim()
        ("Padded - Mozilla Firefox   ", Some("firefox")),
    ];

    for (title, expected) in valid_pairs {
        let inferred = KdeAdapter::infer_browser_class(title);
        assert_eq!(
            inferred, expected,
            "Failed inference for valid title {title}"
        );
    }

    // 2. Spoofed or deceptive suffixes must NOT infer browser classes
    let spoofed_titles = [
        "- Fake-Mozilla Firefox-Evil",
        "Document - Mozilla Firefox.exe",
        "Document - Mozilla Firefox (Private Browsing)",
        "Document - Google Chrome Dev",
        "Document - Chromium-Helper",
        "Brave Browser Helper Service",
        "Fake - Microsoft Edge Canary",
        "Fake - Vivaldi Browser Extension",
        "Fake - Opera Mini",
        "Mozilla Firefox", // missing dash prefix
        "Google Chrome",   // missing dash prefix
        "Chromium",        // missing dash prefix
        "",
        "   ",
        "\t\r\n",
        "Not a browser title",
        "main.rs — rust-analyzer",
        "bash - Konsole",
        "nvim - Alacritty",
    ];

    for title in spoofed_titles {
        let inferred = KdeAdapter::infer_browser_class(title);
        assert_eq!(
            inferred, None,
            "Spoofed or invalid title {title:?} should NOT infer browser, got {inferred:?}"
        );
    }
}

#[test]
fn test_kde_adapter_pathological_titles_and_empty_captions() {
    // 1. Empty and whitespace-only captions and classes
    let meta_empty = KdeAdapter::parse_active_window("", "");
    assert_eq!(meta_empty, WindowMetadata::empty());

    let meta_ws = KdeAdapter::parse_active_window("   \t  ", "   \r\n  ");
    assert_eq!(meta_ws, WindowMetadata::empty());

    // 2. Empty class with valid browser title
    let meta_inferred_class =
        KdeAdapter::parse_active_window("", "crates.io: Rust Registry - Mozilla Firefox");
    assert_eq!(meta_inferred_class.app_id.as_deref(), Some("firefox"));
    assert_eq!(meta_inferred_class.origin.as_deref(), Some("crates.io"));

    // 3. Generic "kwin" and "kwin-app" classes get overridden when browser inferred
    let meta_kwin =
        KdeAdapter::parse_active_window("kwin", "YouTube - Broadcast Yourself - Google Chrome");
    assert_eq!(meta_kwin.app_id.as_deref(), Some("google-chrome"));
    assert_eq!(meta_kwin.origin.as_deref(), Some("youtube.com"));

    let meta_kwin_app =
        KdeAdapter::parse_active_window("kwin-app", "Rust Questions - stackoverflow - Chromium");
    assert_eq!(meta_kwin_app.app_id.as_deref(), Some("chromium"));
    assert_eq!(meta_kwin_app.origin.as_deref(), Some("stackoverflow.com"));

    // 4. Non-browser application class IMMUNITY:
    // Even if title contains browser suffix, specific app class must NOT be overwritten!
    let meta_terminal = KdeAdapter::parse_active_window("alacritty", "editing - Mozilla Firefox");
    assert_eq!(
        meta_terminal.app_id.as_deref(),
        Some("alacritty"),
        "Specific class alacritty must NOT be overridden by inferred browser"
    );
    assert_eq!(
        meta_terminal.origin, None,
        "Terminal must not produce browser origin"
    );

    let meta_code = KdeAdapter::parse_active_window("code", "YouTube - Google Chrome");
    assert_eq!(meta_code.app_id.as_deref(), Some("code"));
    assert_eq!(meta_code.origin, None);

    // 5. Embedded newlines, tabs, and carriage returns in caption
    let meta_newlines = KdeAdapter::parse_active_window(
        "kwin",
        "First Line\nSecond Line\r\ncrates.io - Mozilla Firefox",
    );
    assert_eq!(meta_newlines.app_id.as_deref(), Some("firefox"));
    assert_eq!(meta_newlines.origin.as_deref(), Some("crates.io"));

    // 6. Injection payloads in caption
    let injection_titles = [
        "'; DROP TABLE windows; -- - Google Chrome",
        "$(rm -rf /) `whoami` | id - Chromium",
        "<script>alert(1)</script> - Brave",
        "{{7*7}} - Mozilla Firefox",
    ];
    for title in injection_titles {
        let meta = KdeAdapter::parse_active_window("kwin", title);
        assert!(meta.app_id.is_some(), "Must parse without panic");
        assert_eq!(meta.window_title.as_deref(), Some(title));
    }

    // 7. Massive 500KB caption
    let massive_caption = format!("{} - Mozilla Firefox", "Z".repeat(500_000));
    let meta_massive = KdeAdapter::parse_active_window("kwin", &massive_caption);
    assert_eq!(meta_massive.app_id.as_deref(), Some("firefox"));
    assert_eq!(
        meta_massive.window_title.as_ref().map(|s| s.len()),
        Some(500_018)
    );
}
