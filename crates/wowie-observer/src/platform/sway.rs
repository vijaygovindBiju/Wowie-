use super::PlatformObserver;
use crate::error::ObserverError;
use crate::model::WindowMetadata;
use async_trait::async_trait;
use serde_json::Value;
use std::process::Command;

#[derive(Debug, Default)]
pub struct SwayAdapter;

impl SwayAdapter {
    pub fn new() -> Self {
        Self
    }

    pub fn find_focused_node(node: &Value) -> Option<(Option<String>, Option<String>)> {
        if node.get("focused").and_then(Value::as_bool) == Some(true) {
            let app_id = node
                .get("app_id")
                .and_then(Value::as_str)
                .map(ToString::to_string)
                .or_else(|| {
                    node.get("window_properties")
                        .and_then(|p| p.get("class"))
                        .and_then(Value::as_str)
                        .map(ToString::to_string)
                });

            let name = node
                .get("name")
                .and_then(Value::as_str)
                .map(ToString::to_string);

            return Some((app_id, name));
        }

        // Search nodes and floating_nodes recursively
        if let Some(nodes) = node.get("nodes").and_then(Value::as_array) {
            for child in nodes {
                if let Some(res) = Self::find_focused_node(child) {
                    return Some(res);
                }
            }
        }

        if let Some(floating) = node.get("floating_nodes").and_then(Value::as_array) {
            for child in floating {
                if let Some(res) = Self::find_focused_node(child) {
                    return Some(res);
                }
            }
        }

        None
    }
}

#[async_trait]
impl PlatformObserver for SwayAdapter {
    fn name(&self) -> &'static str {
        "sway"
    }

    fn is_available(&self) -> bool {
        if std::env::var("SWAYSOCK").is_err()
            && !std::env::var("XDG_CURRENT_DESKTOP")
                .is_ok_and(|v| v.to_lowercase().contains("sway"))
        {
            return false;
        }

        Command::new("swaymsg")
            .arg("-v")
            .output()
            .is_ok_and(|o| o.status.success())
    }

    async fn sample_active_window(&self) -> Result<WindowMetadata, ObserverError> {
        let output = tokio::process::Command::new("swaymsg")
            .args(["-t", "get_tree"])
            .output()
            .await
            .map_err(|e| ObserverError::CommandFailed(format!("Failed to execute swaymsg: {e}")))?;

        if !output.status.success() {
            return Ok(WindowMetadata::empty());
        }

        let json_str = String::from_utf8_lossy(&output.stdout);
        let tree: Value = serde_json::from_str(json_str.trim()).map_err(|e| {
            ObserverError::DeserializationFailed(format!("swaymsg tree parse error: {e}"))
        })?;

        let (app_id, window_title) = Self::find_focused_node(&tree).unwrap_or((None, None));

        let origin = if let (Some(ref c), Some(ref t)) = (&app_id, &window_title) {
            crate::model::extract_browser_origin(c, t)
        } else {
            None
        };

        Ok(WindowMetadata {
            app_id,
            window_title,
            origin,
            is_idle: false,
        })
    }
}
