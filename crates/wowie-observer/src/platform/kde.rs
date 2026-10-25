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

    /// Parses KWin active window properties (caption and resource class).
    pub fn parse_active_window(class: &str, title: &str) -> WindowMetadata {
        let app_id = if class.trim().is_empty() {
            None
        } else {
            Some(class.trim().to_string())
        };

        let window_title = if title.trim().is_empty() {
            None
        } else {
            Some(title.trim().to_string())
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
}
