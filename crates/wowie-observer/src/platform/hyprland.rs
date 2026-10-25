use super::PlatformObserver;
use crate::error::ObserverError;
use crate::model::WindowMetadata;
use async_trait::async_trait;
use serde::Deserialize;
use std::process::Command;

#[derive(Debug, Default, Deserialize)]
#[allow(dead_code)]
struct HyprlandWindowJson {
    #[serde(default)]
    class: String,
    #[serde(default)]
    title: String,
    #[serde(default, rename = "initialClass")]
    initial_class: String,
    #[serde(default, rename = "inhibitingIdle")]
    inhibiting_idle: bool,
}

#[derive(Debug, Default)]
pub struct HyprlandAdapter;

impl HyprlandAdapter {
    pub fn new() -> Self {
        Self
    }

    /// Extract common web domain from window titles of popular browsers.
    pub fn extract_origin(class: &str, title: &str) -> Option<String> {
        crate::model::extract_browser_origin(class, title)
    }
}

#[async_trait]
impl PlatformObserver for HyprlandAdapter {
    fn name(&self) -> &'static str {
        "hyprland"
    }

    fn is_available(&self) -> bool {
        if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_err()
            && !std::env::var("XDG_CURRENT_DESKTOP")
                .is_ok_and(|v| v.to_lowercase().contains("hyprland"))
        {
            return false;
        }

        // Check if hyprctl is executable
        Command::new("hyprctl")
            .arg("version")
            .output()
            .is_ok_and(|o| o.status.success())
    }

    async fn sample_active_window(&self) -> Result<WindowMetadata, ObserverError> {
        let output = tokio::process::Command::new("hyprctl")
            .args(["activewindow", "-j"])
            .output()
            .await
            .map_err(|e| ObserverError::CommandFailed(format!("Failed to execute hyprctl: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ObserverError::CommandFailed(format!(
                "hyprctl activewindow exited with code {:?}: {}",
                output.status.code(),
                stderr
            )));
        }

        let json_str = String::from_utf8_lossy(&output.stdout);
        let trimmed = json_str.trim();

        if trimmed.is_empty() || trimmed == "{}" {
            return Ok(WindowMetadata::empty());
        }

        let parsed: HyprlandWindowJson = serde_json::from_str(trimmed).map_err(|e| {
            ObserverError::DeserializationFailed(format!(
                "hyprctl output parse error: {e}. Output was: {trimmed}"
            ))
        })?;

        let class = if parsed.class.is_empty() {
            if parsed.initial_class.is_empty() {
                None
            } else {
                Some(parsed.initial_class)
            }
        } else {
            Some(parsed.class)
        };

        let title = if parsed.title.is_empty() {
            None
        } else {
            Some(parsed.title)
        };

        let origin = if let (Some(c), Some(t)) = (&class, &title) {
            Self::extract_origin(c, t)
        } else {
            None
        };

        Ok(WindowMetadata {
            app_id: class,
            window_title: title,
            origin,
            is_idle: false,
        })
    }
}
