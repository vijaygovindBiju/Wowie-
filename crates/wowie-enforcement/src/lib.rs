//! # wowie-enforcement
//!
//! User-controlled distraction enforcement subsystem for Wowie on Linux.
//! Provides deterministic policy validation, essential tool protections,
//! single-use explicit confirmation tokens, reversible temporary blocking adapters,
//! and predictable crash recovery / session cleanup.

pub mod adapter;
pub mod error;
pub mod manager;
pub mod model;
pub mod policy;

pub use adapter::{
    linux::LinuxEnforcementAdapter, mock::MockEnforcementAdapter, EnforcementAdapter,
};
pub use error::EnforcementError;
pub use manager::EnforcementManager;
pub use model::{ActiveBlock, EnforcementProposal, EnforcementRule, EnforcementTarget};
pub use policy::EnforcementPolicyEngine;
