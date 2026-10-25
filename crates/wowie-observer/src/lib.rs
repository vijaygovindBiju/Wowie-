//! # wowie-observer
//!
//! Activity observation subsystem for Wowie on Linux desktops.
//! Provides platform abstraction with native Hyprland support, X11 fallback,
//! strict consent checks, window title redaction, and idle state detection.

pub mod error;
pub mod model;
pub mod observer;
pub mod platform;

pub use error::ObserverError;
pub use model::{MonitoringState, ObserverConfig, WindowMetadata};
pub use observer::ActivityObserver;
pub use platform::{detect_platform_observer, PlatformObserver};
