use thiserror::Error;
use wowie_avatar::error::AvatarError;

#[derive(Debug, Error)]
pub enum OverlayError {
    #[error("Display server unavailable: {0}. Ensure Wayland or X11 is running.")]
    DisplayServerUnavailable(String),

    #[error("Failed to create overlay window: {0}")]
    WindowCreation(String),

    #[error("Failed to initialize graphics surface: {0}")]
    Surface(String),

    #[error("Failed to load 3D character model: {0}")]
    Model(String),

    #[error("Avatar subsystem error: {0}")]
    Avatar(#[from] AvatarError),

    #[error("Overlay runtime error: {0}")]
    Runtime(String),
}
