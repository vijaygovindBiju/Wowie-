//! # wowie-app
//!
//! Linux-first desktop application and orchestrator for Wowie.
//! Integrates goals, focus sessions, live desktop activity observation,
//! contextual AI coaching, custom 3D avatars, and reversible distraction enforcement.

pub mod cli;
pub mod config;
pub mod coordinator;
pub mod error;
pub mod state;
pub mod ui;

pub use cli::CliRunner;
pub use config::AppSettings;
pub use coordinator::{UserInterventionChoice, WowieCoordinator};
pub use error::AppError;
pub use state::AppState;
pub use ui::{Screen, UiRenderer};
