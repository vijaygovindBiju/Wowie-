//! Policy and deterministic rate-limiting engine for the AI Coach.

pub mod cooldown;
pub mod fallback;

pub use cooldown::{
    normalize_origin, CooldownConfig, CooldownStatus, CooldownTracker, InterventionRecord,
    TargetSuppression, ThrottlingReason, DEFAULT_CORRECTION_SUPPRESSION_SECONDS,
    DEFAULT_IDLE_COOLDOWN_SECONDS, DEFAULT_MAX_INTERVENTIONS_PER_WINDOW,
    DEFAULT_MIN_INTERVAL_SECONDS, DEFAULT_RAPID_WINDOW_SECONDS, DEFAULT_SESSION_CAP,
    DEFAULT_SNOOZE_SECONDS, MAX_COOLDOWN_INTERVAL_SECONDS, MAX_SNOOZE_SECONDS,
    MAX_TRACKED_INTERVENTIONS, MAX_TRACKED_SUPPRESSIONS, MIN_SAFE_COOLDOWN_SECONDS,
};
pub use fallback::{DeterministicFallback, FallbackReason};
