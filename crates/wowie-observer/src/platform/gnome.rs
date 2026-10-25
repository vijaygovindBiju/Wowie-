use super::PlatformObserver;
use crate::error::ObserverError;
use crate::model::WindowMetadata;
use async_trait::async_trait;
use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Default, Deserialize)]
pub struct GnomeWindowEntry {
    #[serde(default)]
    pub id: u64,
    #[serde(default)]
    pub title: String,
    #[serde(default, rename = "wm-class")]
    pub wm_class: String,
    #[serde(default, rename = "app-id")]
    pub app_id: String,
    #[serde(default, rename = "has-focus")]
    pub has_focus: bool,
}

#[derive(Debug, Default)]
pub struct GnomeAdapter;

impl GnomeAdapter {
    pub fn new() -> Self {
        Self
    }

    /// Parses JSON representation of GNOME Shell Introspect window metadata.
    pub fn parse_windows_json(raw: &str) -> Result<WindowMetadata, ObserverError> {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed == "{}" || trimmed == "[]" {
            return Ok(WindowMetadata::empty());
        }

        // GNOME Shell Introspect returns a dictionary of window ID -> window struct
        let windows: std::collections::HashMap<String, GnomeWindowEntry> =
            serde_json::from_str(trimmed).map_err(|e| {
                ObserverError::DeserializationFailed(format!("GNOME Shell window parse error: {e}"))
            })?;

        // Find the window that currently has desktop focus
        if let Some((_, active)) = windows.into_iter().find(|(_, w)| w.has_focus) {
            let app_id = if !active.app_id.is_empty() {
                Some(active.app_id)
            } else if !active.wm_class.is_empty() {
                Some(active.wm_class)
            } else {
                None
            };

            let title = if active.title.is_empty() {
                None
            } else {
                Some(active.title)
            };

            let origin = if let (Some(c), Some(t)) = (&app_id, &title) {
                crate::model::extract_browser_origin(c, t)
            } else {
                None
            };

            Ok(WindowMetadata {
                app_id,
                window_title: title,
                origin,
                is_idle: false,
            })
        } else {
            Ok(WindowMetadata::empty())
        }
    }
}

#[async_trait]
impl PlatformObserver for GnomeAdapter {
    fn name(&self) -> &'static str {
        "gnome"
    }

    fn is_available(&self) -> bool {
        if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
            if desktop.to_lowercase().contains("gnome") {
                return true;
            }
        }
        if let Ok(session) = std::env::var("DESKTOP_SESSION") {
            if session.to_lowercase().contains("gnome") {
                return true;
            }
        }

        Command::new("gdbus")
            .arg("help")
            .output()
            .is_ok_and(|o| o.status.success())
    }

    async fn sample_active_window(&self) -> Result<WindowMetadata, ObserverError> {
        // Query GNOME Shell Introspect D-Bus method
        let output = tokio::process::Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest",
                "org.gnome.Shell",
                "--object-path",
                "/org/gnome/Shell/Introspect",
                "--method",
                "org.gnome.Shell.Introspect.GetWindows",
            ])
            .output()
            .await
            .map_err(|e| ObserverError::CommandFailed(format!("Failed to execute gdbus: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ObserverError::CommandFailed(format!(
                "gdbus GetWindows failed: {stderr}"
            )));
        }

        let raw_output = String::from_utf8_lossy(&output.stdout);
        // gdbus wraps response in tuple notation: `(<{ ... }>,)`
        let trimmed = raw_output.trim();
        let json_slice = if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
            &trimmed[start..=end]
        } else {
            return Ok(WindowMetadata::empty());
        };

        Self::parse_windows_json(json_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gnome_parse_focused_window() {
        let json_data = r#"{
            "123": {
                "id": 123,
                "title": "crates.io: Rust Package Registry - Mozilla Firefox",
                "wm-class": "firefox",
                "app-id": "org.mozilla.firefox",
                "has-focus": true
            },
            "456": {
                "id": 456,
                "title": "Terminal",
                "wm-class": "alacritty",
                "app-id": "alacritty",
                "has-focus": false
            }
        }"#;

        let meta = GnomeAdapter::parse_windows_json(json_data).expect("Parses successfully");
        assert_eq!(meta.app_id.as_deref(), Some("org.mozilla.firefox"));
        assert_eq!(
            meta.window_title.as_deref(),
            Some("crates.io: Rust Package Registry - Mozilla Firefox")
        );
        assert_eq!(meta.origin.as_deref(), Some("crates.io"));
    }

    #[test]
    fn test_gnome_empty_window_list() {
        let meta = GnomeAdapter::parse_windows_json("{}").expect("Handles empty JSON");
        assert_eq!(meta, WindowMetadata::empty());
    }
}
