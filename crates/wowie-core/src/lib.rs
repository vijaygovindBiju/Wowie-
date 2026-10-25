//! # wowie-core
//!
//! Core domain entities, strongly-typed identifiers, validation logic,
//! error taxonomy, and focus session state machine for Wowie.

pub mod error;
pub mod id;
pub mod models;
pub mod state_machine;

pub use error::CoreError;
pub use id::*;
pub use models::*;
pub use state_machine::FocusSessionStateMachine;
