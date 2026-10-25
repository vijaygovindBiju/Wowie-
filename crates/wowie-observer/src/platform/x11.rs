use super::PlatformObserver;
use crate::error::ObserverError;
use crate::model::WindowMetadata;
use async_trait::async_trait;
use std::process::Command;

#[derive(Debug, Default)]
pub struct X11Adapter;

impl X11Adapter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl PlatformObserver for X11Adapter {
    fn name(&self) -> &'static str {
        "x11"
    }

    fn is_available(&self) -> bool {
        if std::env::var("DISPLAY").is_err() {
            return false;
        }

        // Check if xdotool is present
        Command::new("xdotool")
            .arg("version")
            .output()
            .is_ok_and(|o| o.status.success())
    }

    async fn sample_active_window(&self) -> Result<WindowMetadata, ObserverError> {
        let id_output = tokio::process::Command::new("xdotool")
            .args(["getactivewindow"])
            .output()
            .await
            .map_err(|e| ObserverError::CommandFailed(format!("Failed to execute xdotool: {e}")))?;

        if !id_output.status.success() {
            return Ok(WindowMetadata::empty());
        }

        let window_id = String::from_utf8_lossy(&id_output.stdout)
            .trim()
            .to_string();
        if window_id.is_empty() {
            return Ok(WindowMetadata::empty());
        }

        let class_output = tokio::process::Command::new("xdotool")
            .args(["getwindowclassname", &window_id])
            .output()
            .await;

        let class = match class_output {
            Ok(ref out) if out.status.success() => {
                let c = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if c.is_empty() {
                    None
                } else {
                    Some(c)
                }
            }
            _ => None,
        };

        let title_output = tokio::process::Command::new("xdotool")
            .args(["getwindowname", &window_id])
            .output()
            .await;

        let window_title = match title_output {
            Ok(ref out) if out.status.success() => {
                let t = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if t.is_empty() {
                    None
                } else {
                    Some(t)
                }
            }
            _ => None,
        };

        let origin = if let (Some(c), Some(t)) = (&class, &window_title) {
            crate::model::extract_browser_origin(c, t)
        } else {
            None
        };

        Ok(WindowMetadata {
            app_id: class.or_else(|| Some("x11_window".to_string())),
            window_title,
            origin,
            is_idle: false,
        })
    }
}

#[derive(Debug, Default)]
pub struct UnavailablePlatformAdapter;

impl UnavailablePlatformAdapter {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl PlatformObserver for UnavailablePlatformAdapter {
    fn name(&self) -> &'static str {
        "unavailable"
    }

    fn is_available(&self) -> bool {
        false
    }

    async fn sample_active_window(&self) -> Result<WindowMetadata, ObserverError> {
        Err(ObserverError::PlatformUnsupported(
            "No supported desktop compositor or window inspector found on this system".to_string(),
        ))
    }
}
