pub mod hyprland;
pub mod mock;
pub mod sway;
pub mod x11;

use crate::error::ObserverError;
use crate::model::WindowMetadata;
use async_trait::async_trait;

#[async_trait]
pub trait PlatformObserver: Send + Sync + std::fmt::Debug {
    fn name(&self) -> &'static str;
    fn is_available(&self) -> bool;
    async fn sample_active_window(&self) -> Result<WindowMetadata, ObserverError>;
}

/// Detects the current desktop platform and returns the appropriate observer adapter.
pub fn detect_platform_observer() -> Box<dyn PlatformObserver> {
    let hyprland = hyprland::HyprlandAdapter::new();
    if hyprland.is_available() {
        return Box::new(hyprland);
    }

    let sway = sway::SwayAdapter::new();
    if sway.is_available() {
        return Box::new(sway);
    }

    let x11 = x11::X11Adapter::new();
    if x11.is_available() {
        return Box::new(x11);
    }

    Box::new(x11::UnavailablePlatformAdapter::new())
}
