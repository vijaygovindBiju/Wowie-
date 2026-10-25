use super::PlatformObserver;
use crate::error::ObserverError;
use crate::model::WindowMetadata;
use async_trait::async_trait;
use std::process::Command;

#[derive(Debug, Default)]
pub struct KdeAdapter;

impl KdeAdapter {
    pub fn new() -> Self {
        Self
    }

    /// Infers browser application class from window title suffix.
    pub fn infer_browser_class(title: &str) -> Option<&'static str> {
        let trimmed = title.trim();
        if trimmed.ends_with("- Mozilla Firefox")
            || trimmed.ends_with("— Mozilla Firefox")
            || trimmed.ends_with("– Mozilla Firefox")
            || trimmed.ends_with("- Firefox")
        {
            Some("firefox")
        } else if trimmed.ends_with("- Google Chrome")
            || trimmed.ends_with("— Google Chrome")
            || trimmed.ends_with("– Google Chrome")
        {
            Some("google-chrome")
        } else if trimmed.ends_with("- Chromium")
            || trimmed.ends_with("— Chromium")
            || trimmed.ends_with("– Chromium")
        {
            Some("chromium")
        } else if trimmed.ends_with("- Brave")
            || trimmed.ends_with("— Brave")
            || trimmed.ends_with("- Brave Browser")
        {
            Some("brave")
        } else if trimmed.ends_with("- Microsoft Edge")
            || trimmed.ends_with("— Microsoft Edge")
            || trimmed.ends_with("– Microsoft Edge")
        {
            Some("microsoft-edge")
        } else if trimmed.ends_with("- Vivaldi") || trimmed.ends_with("— Vivaldi") {
            Some("vivaldi")
        } else if trimmed.ends_with("- Opera") || trimmed.ends_with("— Opera") {
            Some("opera")
        } else if trimmed.ends_with("- Zen Browser") || trimmed.ends_with("— Zen Browser") {
            Some("zen")
        } else if trimmed.ends_with("- Floorp") || trimmed.ends_with("— Floorp") {
            Some("floorp")
        } else if trimmed.ends_with("- LibreWolf") || trimmed.ends_with("— LibreWolf") {
            Some("librewolf")
        } else {
            None
        }
    }

    /// Parses KWin active window properties (caption and resource class).
    pub fn parse_active_window(class: &str, title: &str) -> WindowMetadata {
        let class_trimmed = class.trim();
        let title_trimmed = title.trim();

        let window_title = if title_trimmed.is_empty() {
            None
        } else {
            Some(title_trimmed.to_string())
        };

        let app_id = if let Some(inferred) =
            window_title.as_deref().and_then(Self::infer_browser_class)
        {
            if class_trimmed.is_empty() || class_trimmed == "kwin" || class_trimmed == "kwin-app" {
                Some(inferred.to_string())
            } else {
                Some(class_trimmed.to_string())
            }
        } else if !class_trimmed.is_empty() {
            Some(class_trimmed.to_string())
        } else {
            None
        };

        let origin = if let (Some(c), Some(t)) = (&app_id, &window_title) {
            crate::model::extract_browser_origin(c, t)
        } else {
            None
        };

        WindowMetadata {
            app_id,
            window_title,
            origin,
            is_idle: false,
        }
    }
}

#[async_trait]
impl PlatformObserver for KdeAdapter {
    fn name(&self) -> &'static str {
        "kde"
    }

    fn is_available(&self) -> bool {
        if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
            let lower = desktop.to_lowercase();
            if lower.contains("kde") || lower.contains("plasma") {
                return true;
            }
        }
        if let Ok(session) = std::env::var("DESKTOP_SESSION") {
            let lower = session.to_lowercase();
            if lower.contains("plasma") || lower.contains("kde") {
                return true;
            }
        }

        Command::new("qdbus")
            .arg("--version")
            .output()
            .is_ok_and(|o| o.status.success())
    }

    async fn sample_active_window(&self) -> Result<WindowMetadata, ObserverError> {
        // Query KWin active window via qdbus or kdotool
        let output = tokio::process::Command::new("qdbus")
            .args(["org.kde.KWin", "/KWin", "org.kde.KWin.activeWindow"])
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => {
                let text = String::from_utf8_lossy(&out.stdout);
                let title = text.trim();
                Ok(Self::parse_active_window("kwin", title))
            }
            _ => {
                // Fallback: check kdotool if available
                let kdotool_out = tokio::process::Command::new("kdotool")
                    .args(["getactivewindow", "getwindowname"])
                    .output()
                    .await;

                if let Ok(k_out) = kdotool_out {
                    if k_out.status.success() {
                        let title = String::from_utf8_lossy(&k_out.stdout);
                        return Ok(Self::parse_active_window("kwin-app", title.trim()));
                    }
                }

                Ok(WindowMetadata::empty())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kde_parse_active_window() {
        let meta = KdeAdapter::parse_active_window(
            "google-chrome",
            "YouTube - Broadcast Yourself - Google Chrome",
        );
        assert_eq!(meta.app_id.as_deref(), Some("google-chrome"));
        assert_eq!(
            meta.window_title.as_deref(),
            Some("YouTube - Broadcast Yourself - Google Chrome")
        );
        assert_eq!(meta.origin.as_deref(), Some("youtube.com"));
    }

    #[test]
    fn test_kde_empty_strings() {
        let meta = KdeAdapter::parse_active_window("", "");
        assert_eq!(meta, WindowMetadata::empty());
    }

    #[test]
    fn test_kde_caption_only_infers_browser_and_origin() {
        let meta_firefox = KdeAdapter::parse_active_window(
            "kwin",
            "crates.io: Rust Package Registry - Mozilla Firefox",
        );
        assert_eq!(meta_firefox.app_id.as_deref(), Some("firefox"));
        assert_eq!(meta_firefox.origin.as_deref(), Some("crates.io"));

        let meta_chrome = KdeAdapter::parse_active_window(
            "kwin-app",
            "YouTube - Broadcast Yourself - Google Chrome",
        );
        assert_eq!(meta_chrome.app_id.as_deref(), Some("google-chrome"));
        assert_eq!(meta_chrome.origin.as_deref(), Some("youtube.com"));

        let meta_chromium = KdeAdapter::parse_active_window(
            "kwin",
            "docs.rs - Rust Package Documentation - Chromium",
        );
        assert_eq!(meta_chromium.app_id.as_deref(), Some("chromium"));
        assert_eq!(meta_chromium.origin.as_deref(), Some("docs.rs"));

        let meta_term = KdeAdapter::parse_active_window("kwin", "bash — Konsole");
        assert_eq!(meta_term.app_id.as_deref(), Some("kwin"));
        assert_eq!(meta_term.origin, None);
    }
}
