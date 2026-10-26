//! # wowie-overlay
//!
//! Real 3D desktop companion overlay for Arch Linux with Hyprland and Wayland.
//! Renders user-imported VRM 1.0 and glTF 2.0 3D characters in a transparent,
//! floating desktop overlay with dynamic mood animations, Z-buffer depth testing,
//! and interactive show/hide/quit controls.

pub mod backend;
pub mod error;
pub mod math;
pub mod renderer;
pub mod roam;
pub mod state;
pub mod window;

pub use backend::{
    is_gpu_acceleration_available, query_gpu_device_info, select_rendering_backend,
    GpuAccelerationPipeline, GpuDeviceInfo, GraphicsBackend,
};
pub use error::OverlayError;
pub use math::{Mat4, Vec3, Vec4};
pub use renderer::{Camera3D, RenderStats, Renderer3D};
pub use roam::RoamController;
pub use state::{OverlayBridge, OverlayConfig, OverlayControlEvent};
pub use window::DesktopOverlay;
