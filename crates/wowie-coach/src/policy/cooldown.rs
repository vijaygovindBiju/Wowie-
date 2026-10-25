//! Deterministic cooldown tracker and intervention rate limiter.
//!
//! Enforces non-bypassable pacing rules, rolling window rate limits,
//! user snooze, break state muting, and correction suppression using
//! injected timestamps with zero system clock dependencies.

use crate::error::CoachError;
use crate::models::action::ActionCandidate;
use crate::models::context::{CoachContext, UserCorrectionSummary};
use crate::models::tone::{PersonalityMode, TonePreferences};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fmt;
use wowie_core::models::CorrectionScope;

// =========================================================================
// CONSTANTS & SAFETY LIMITS
// =========================================================================

/// Absolute floor for consecutive intervention interval (prevents alert storms).
pub const MIN_SAFE_COOLDOWN_SECONDS: u32 = 60; // 1 minute

/// Maximum allowed consecutive intervention cooldown.
pub const MAX_COOLDOWN_INTERVAL_SECONDS: u32 = 7200; // 2 hours

/// Default cooldown interval between consecutive interventions (5 minutes).
pub const DEFAULT_MIN_INTERVAL_SECONDS: u32 = 300;

/// Default rolling window duration for rapid succession rate limiting (1 hour).
pub const DEFAULT_RAPID_WINDOW_SECONDS: u32 = 3600;

/// Default maximum allowed proactive interventions within the rolling window.
pub const DEFAULT_MAX_INTERVENTIONS_PER_WINDOW: u32 = 4;

/// Default suppression duration following a user classification correction (30 minutes).
pub const DEFAULT_CORRECTION_SUPPRESSION_SECONDS: u32 = 1800;

/// Default snooze duration (10 minutes).
pub const DEFAULT_SNOOZE_SECONDS: u32 = 600;

/// Maximum allowed snooze duration (2 hours).
pub const MAX_SNOOZE_SECONDS: u32 = 7200;

/// Default idle suppression duration (3 minutes).
pub const DEFAULT_IDLE_COOLDOWN_SECONDS: u32 = 180;

/// Default maximum total interventions allowed per focus session.
pub const DEFAULT_SESSION_CAP: u32 = 12;

/// Hard upper bound for tracked timestamps in memory (prevents unbounded allocation).
pub const MAX_TRACKED_INTERVENTIONS: usize = 64;

/// Hard upper bound for active correction suppressions in memory.
pub const MAX_TRACKED_SUPPRESSIONS: usize = 64;

// =========================================================================
// THROTTLING REASON & COOLDOWN STATUS
// =========================================================================

/// Explicit reason explaining why a coaching intervention is throttled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThrottlingReason {
    /// Minimum cooldown interval since last intervention has not elapsed.
    RecentIntervention,
    /// Maximum allowed interventions within the rolling window has been reached.
    RapidSuccessionRateLimit,
    /// Session-wide cumulative intervention cap reached.
    SessionCapReached,
    /// Interventions are actively snoozed by the user.
    SnoozeActive,
    /// Target application or origin is suppressed by a prior user correction.
    UserCorrectionSuppressed,
    /// User is currently in a planned break state; non-break interventions are muted.
    BreakStateActive,
    /// User is currently idle; interventions are suspended.
    IdleActive,
    /// Clock regression detected (injected timestamp earlier than prior record).
    TimeRegression,
}

impl fmt::Display for ThrottlingReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecentIntervention => write!(f, "recent_intervention"),
            Self::RapidSuccessionRateLimit => write!(f, "rapid_succession_rate_limit"),
            Self::SessionCapReached => write!(f, "session_cap_reached"),
            Self::SnoozeActive => write!(f, "snooze_active"),
            Self::UserCorrectionSuppressed => write!(f, "user_correction_suppressed"),
            Self::BreakStateActive => write!(f, "break_state_active"),
            Self::IdleActive => write!(f, "idle_active"),
            Self::TimeRegression => write!(f, "time_regression"),
        }
    }
}

/// Evaluation verdict indicating whether an intervention is allowed or throttled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum CooldownStatus {
    /// Intervention is permitted.
    Allowed,
    /// Intervention is throttled with reason and exact remaining duration.
    Throttled {
        reason: ThrottlingReason,
        remaining_seconds: u32,
    },
}

impl CooldownStatus {
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed)
    }

    pub fn is_throttled(&self) -> bool {
        matches!(self, Self::Throttled { .. })
    }

    pub fn remaining_seconds(&self) -> u32 {
        match self {
            Self::Allowed => 0,
            Self::Throttled {
                remaining_seconds, ..
            } => *remaining_seconds,
        }
    }

    pub fn throttling_reason(&self) -> Option<ThrottlingReason> {
        match self {
            Self::Allowed => None,
            Self::Throttled { reason, .. } => Some(*reason),
        }
    }

    /// Converts a throttled status into a typed `CoachError`, or `None` if allowed.
    pub fn to_coach_error(&self) -> Option<CoachError> {
        match self {
            Self::Allowed => None,
            Self::Throttled {
                reason,
                remaining_seconds,
            } => Some(CoachError::cooldown_throttled(
                u64::from(*remaining_seconds),
                reason.to_string(),
            )),
        }
    }
}

// =========================================================================
// COOLDOWN CONFIGURATION
// =========================================================================

/// Configuration governing deterministic cooldown intervals and rate limits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CooldownConfig {
    /// Minimum duration (seconds) required between consecutive interventions.
    pub min_interval_seconds: u32,

    /// Duration (seconds) of the rolling window for rapid intervention rate limiting.
    pub rapid_window_seconds: u32,

    /// Maximum allowed interventions within the rolling window.
    pub max_interventions_per_window: u32,

    /// Duration (seconds) to suppress interventions after user enters or leaves idle.
    pub idle_cooldown_seconds: u32,

    /// Duration (seconds) to suppress interventions on items corrected by the user.
    pub correction_suppression_seconds: u32,

    /// Default duration (seconds) applied when a user requests a snooze.
    pub default_snooze_seconds: u32,

    /// Maximum allowed duration (seconds) for any user snooze request.
    pub max_snooze_seconds: u32,

    /// Whether break-related reminders are permitted during an active break state.
    pub allow_break_reminders: bool,

    /// Optional cumulative cap on total interventions for the entire session.
    pub session_cap: Option<u32>,
}

impl Default for CooldownConfig {
    fn default() -> Self {
        Self {
            min_interval_seconds: DEFAULT_MIN_INTERVAL_SECONDS,
            rapid_window_seconds: DEFAULT_RAPID_WINDOW_SECONDS,
            max_interventions_per_window: DEFAULT_MAX_INTERVENTIONS_PER_WINDOW,
            idle_cooldown_seconds: DEFAULT_IDLE_COOLDOWN_SECONDS,
            correction_suppression_seconds: DEFAULT_CORRECTION_SUPPRESSION_SECONDS,
            default_snooze_seconds: DEFAULT_SNOOZE_SECONDS,
            max_snooze_seconds: MAX_SNOOZE_SECONDS,
            allow_break_reminders: true,
            session_cap: Some(DEFAULT_SESSION_CAP),
        }
    }
}

impl CooldownConfig {
    /// Validates all configuration parameters against safety and operational bounds.
    pub fn validate(&self) -> Result<(), CoachError> {
        if self.min_interval_seconds < MIN_SAFE_COOLDOWN_SECONDS {
            return Err(CoachError::schema_validation(
                "min_interval_seconds",
                format!("Minimum interval cannot be less than {MIN_SAFE_COOLDOWN_SECONDS}s"),
            ));
        }
        if self.min_interval_seconds > MAX_COOLDOWN_INTERVAL_SECONDS {
            return Err(CoachError::schema_validation(
                "min_interval_seconds",
                format!("Minimum interval cannot exceed {MAX_COOLDOWN_INTERVAL_SECONDS}s"),
            ));
        }
        if self.rapid_window_seconds < self.min_interval_seconds {
            return Err(CoachError::schema_validation(
                "rapid_window_seconds",
                "Rapid window duration must be at least as long as min_interval_seconds",
            ));
        }
        if self.max_interventions_per_window < 1 {
            return Err(CoachError::schema_validation(
                "max_interventions_per_window",
                "Max interventions per window must be at least 1",
            ));
        }
        if self.correction_suppression_seconds < MIN_SAFE_COOLDOWN_SECONDS {
            return Err(CoachError::schema_validation(
                "correction_suppression_seconds",
                format!("Correction suppression cannot be less than {MIN_SAFE_COOLDOWN_SECONDS}s"),
            ));
        }
        if self.default_snooze_seconds > self.max_snooze_seconds {
            return Err(CoachError::schema_validation(
                "default_snooze_seconds",
                "Default snooze cannot exceed max_snooze_seconds",
            ));
        }
        if self.max_snooze_seconds > MAX_SNOOZE_SECONDS {
            return Err(CoachError::schema_validation(
                "max_snooze_seconds",
                format!("Max snooze cannot exceed {MAX_SNOOZE_SECONDS}s"),
            ));
        }
        if let Some(cap) = self.session_cap {
            if cap < 1 {
                return Err(CoachError::schema_validation(
                    "session_cap",
                    "Session cap must be at least 1 if specified",
                ));
            }
        }
        Ok(())
    }

    /// Gentle preset: longer intervals and lower intervention frequency.
    pub fn gentle() -> Self {
        Self {
            min_interval_seconds: 600, // 10 minutes
            rapid_window_seconds: 3600,
            max_interventions_per_window: 2,
            idle_cooldown_seconds: 300,
            correction_suppression_seconds: 2400,
            default_snooze_seconds: 900,
            max_snooze_seconds: 7200,
            allow_break_reminders: true,
            session_cap: Some(6),
        }
    }

    /// Strict / pragmatic preset: tighter pacing for direct focus assistance.
    pub fn strict() -> Self {
        Self {
            min_interval_seconds: 180, // 3 minutes
            rapid_window_seconds: 3600,
            max_interventions_per_window: 6,
            idle_cooldown_seconds: 120,
            correction_suppression_seconds: 1200,
            default_snooze_seconds: 300,
            max_snooze_seconds: 3600,
            allow_break_reminders: true,
            session_cap: Some(18),
        }
    }

    /// Alias for gentle preset.
    pub fn supportive() -> Self {
        Self::gentle()
    }

    /// Alias for strict preset.
    pub fn pragmatic() -> Self {
        Self::strict()
    }

    /// Adapt configuration based on user tone preferences without violating security floors.
    pub fn from_tone(tone: &TonePreferences) -> Self {
        match tone.mode {
            PersonalityMode::Supportive | PersonalityMode::Inquisitive => Self::gentle(),
            PersonalityMode::Pragmatic => Self::strict(),
            PersonalityMode::Neutral | PersonalityMode::Playful => Self::default(),
        }
    }

    /// Alias for `from_tone`.
    pub fn from_tone_preferences(tone: &TonePreferences) -> Self {
        Self::from_tone(tone)
    }
}

// =========================================================================
// SUPPORTING DATA STRUCTURES & HELPERS
// =========================================================================

/// Normalizes an origin or URL string for matching.
pub fn normalize_origin(s: &str) -> String {
    let trimmed = s.trim().to_ascii_lowercase();
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(&trimmed);
    let host_part = without_scheme.split('/').next().unwrap_or(without_scheme);
    let host_part = host_part.split('?').next().unwrap_or(host_part);
    let host_part = host_part.split('#').next().unwrap_or(host_part);
    let host = host_part.split(':').next().unwrap_or(host_part);
    let host = host.strip_prefix("www.").unwrap_or(host);
    host.trim().to_string()
}

/// Record of a past coaching intervention.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterventionRecord {
    pub timestamp: DateTime<Utc>,
    pub action: ActionCandidate,
}

/// Active suppression for an application ID or domain origin resulting from user correction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetSuppression {
    pub app_id: Option<String>,
    pub origin: Option<String>,
    pub scope: CorrectionScope,
    pub suppressed_until: DateTime<Utc>,
}

impl TargetSuppression {
    pub fn is_global(&self) -> bool {
        self.app_id.is_none() && self.origin.is_none()
    }

    pub fn matches(&self, app: Option<&str>, orig: Option<&str>) -> bool {
        if self.is_global() {
            return true;
        }

        let mut matched = false;

        if let Some(ref target_app) = self.app_id {
            if let Some(obs_app) = app {
                if target_app.eq_ignore_ascii_case(obs_app.trim()) {
                    matched = true;
                } else {
                    return false;
                }
            } else {
                return false;
            }
        }

        if let Some(ref target_orig) = self.origin {
            if let Some(obs_orig) = orig {
                let target_norm = normalize_origin(target_orig);
                let obs_norm = normalize_origin(obs_orig);
                if obs_norm == target_norm || obs_norm.ends_with(&format!(".{target_norm}")) {
                    matched = true;
                } else {
                    return false;
                }
            } else {
                return false;
            }
        }

        matched
    }
}

// =========================================================================
// COOLDOWN TRACKER
// =========================================================================

/// Deterministic, memory-bounded session rate-limiting tracker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CooldownTracker {
    config: CooldownConfig,
    last_intervention: Option<InterventionRecord>,
    recent_interventions: VecDeque<DateTime<Utc>>,
    total_interventions: u32,
    snoozed_until: Option<DateTime<Utc>>,
    active_suppressions: VecDeque<TargetSuppression>,
    is_break_state: bool,
    is_idle: bool,
    last_idle_transition: Option<DateTime<Utc>>,
}

impl CooldownTracker {
    /// Creates a new `CooldownTracker` with validated configuration.
    pub fn new(config: CooldownConfig) -> Result<Self, CoachError> {
        config.validate()?;
        Ok(Self {
            config,
            last_intervention: None,
            recent_interventions: VecDeque::with_capacity(MAX_TRACKED_INTERVENTIONS),
            total_interventions: 0,
            snoozed_until: None,
            active_suppressions: VecDeque::with_capacity(MAX_TRACKED_SUPPRESSIONS),
            is_break_state: false,
            is_idle: false,
            last_idle_transition: None,
        })
    }

    /// Creates a new `CooldownTracker` with default configuration.
    pub fn with_default_config() -> Self {
        Self::new(CooldownConfig::default()).expect("Default config must always validate")
    }

    // ---------------------------------------------------------------------
    // EVALUATION API
    // ---------------------------------------------------------------------

    /// Evaluates whether an intervention is permitted given the current injected timestamp.
    pub fn can_intervene(&self, now: DateTime<Utc>) -> CooldownStatus {
        self.evaluate_internal(now, None, None, None)
    }

    /// Evaluates whether an intervention is permitted for a specific action candidate.
    pub fn can_intervene_for_action(
        &self,
        now: DateTime<Utc>,
        action: ActionCandidate,
    ) -> CooldownStatus {
        self.evaluate_internal(now, Some(action), None, None)
    }

    /// Evaluates whether an intervention is permitted given target application and origin.
    pub fn can_intervene_for_target(
        &self,
        now: DateTime<Utc>,
        app_id: Option<&str>,
        origin: Option<&str>,
    ) -> CooldownStatus {
        self.evaluate_internal(now, None, app_id, origin)
    }

    /// Evaluates whether an intervention is permitted for full `CoachContext`.
    pub fn can_intervene_context(&self, now: DateTime<Utc>, ctx: &CoachContext) -> CooldownStatus {
        if (ctx.is_break_state || self.is_break_state) && !self.config.allow_break_reminders {
            return CooldownStatus::Throttled {
                reason: ThrottlingReason::BreakStateActive,
                remaining_seconds: self.config.min_interval_seconds,
            };
        }

        if ctx.observation.effective_idle_state() != wowie_core::models::IdleState::Active
            || self.is_idle
        {
            return CooldownStatus::Throttled {
                reason: ThrottlingReason::IdleActive,
                remaining_seconds: self.config.idle_cooldown_seconds,
            };
        }

        let app = ctx.observation.app_id.as_deref();
        let origin = ctx.observation.origin.as_deref();
        self.evaluate_internal(now, None, app, origin)
    }

    // ---------------------------------------------------------------------
    // STATE MUTATION API
    // ---------------------------------------------------------------------

    /// Records a successful proactive coaching intervention at `now`.
    ///
    /// Note: `ActionCandidate::None` is a no-op and does not consume rate limits.
    pub fn record_intervention(&mut self, now: DateTime<Utc>, action: ActionCandidate) {
        if action.is_none() {
            return;
        }

        self.last_intervention = Some(InterventionRecord {
            timestamp: now,
            action,
        });
        self.total_interventions = self.total_interventions.saturating_add(1);

        self.prune_rolling_window(now);
        if self.recent_interventions.len() >= MAX_TRACKED_INTERVENTIONS {
            self.recent_interventions.pop_front();
        }
        self.recent_interventions.push_back(now);
    }

    /// Records a user classification correction, suppressing interventions for the target.
    pub fn record_user_correction(&mut self, now: DateTime<Utc>, scope: CorrectionScope) {
        self.record_targeted_correction(now, scope, None, None);
    }

    /// Records a user classification correction with target metadata.
    pub fn record_targeted_correction(
        &mut self,
        now: DateTime<Utc>,
        scope: CorrectionScope,
        app_id: Option<String>,
        origin: Option<String>,
    ) {
        let duration = Duration::seconds(i64::from(self.config.correction_suppression_seconds));
        let suppressed_until = now + duration;

        let clean_app = app_id.map(|s| s.trim().to_string());
        let clean_origin = origin.map(|s| normalize_origin(&s));

        self.prune_suppressions(now);

        // Update existing if target matches
        for supp in &mut self.active_suppressions {
            let app_matches = match (&supp.app_id, &clean_app) {
                (Some(a1), Some(a2)) => a1.eq_ignore_ascii_case(a2),
                (None, None) => true,
                _ => false,
            };
            let origin_matches = match (&supp.origin, &clean_origin) {
                (Some(o1), Some(o2)) => o1.eq_ignore_ascii_case(o2),
                (None, None) => true,
                _ => false,
            };

            if app_matches && origin_matches {
                supp.suppressed_until = supp.suppressed_until.max(suppressed_until);
                supp.scope = scope;
                return;
            }
        }

        if self.active_suppressions.len() >= MAX_TRACKED_SUPPRESSIONS {
            self.active_suppressions.pop_front();
        }

        self.active_suppressions.push_back(TargetSuppression {
            app_id: clean_app,
            origin: clean_origin,
            scope,
            suppressed_until,
        });
    }

    /// Records a correction from a `UserCorrectionSummary`.
    pub fn record_correction_summary(
        &mut self,
        now: DateTime<Utc>,
        summary: &UserCorrectionSummary,
    ) {
        self.record_targeted_correction(
            now,
            summary.scope,
            summary.app_id.clone(),
            summary.origin.clone(),
        );
    }

    /// Records a user snooze request for `duration_seconds`.
    pub fn record_snooze(
        &mut self,
        now: DateTime<Utc>,
        duration_seconds: u32,
    ) -> Result<(), CoachError> {
        let duration = if duration_seconds == 0 {
            self.config.default_snooze_seconds
        } else {
            duration_seconds.min(self.config.max_snooze_seconds)
        };

        let snooze_end = now + Duration::seconds(i64::from(duration));
        self.snoozed_until = Some(match self.snoozed_until {
            Some(existing) if existing > snooze_end => existing,
            _ => snooze_end,
        });
        Ok(())
    }

    /// Cancels any active snooze.
    pub fn cancel_snooze(&mut self) {
        self.snoozed_until = None;
    }

    /// Alias for `cancel_snooze`.
    pub fn clear_snooze(&mut self) {
        self.cancel_snooze();
    }

    /// Updates active break state.
    pub fn set_break_state(&mut self, is_break: bool, _now: DateTime<Utc>) {
        self.is_break_state = is_break;
    }

    /// Updates active idle state.
    pub fn set_idle_state(&mut self, is_idle: bool, now: DateTime<Utc>) {
        if self.is_idle != is_idle {
            self.is_idle = is_idle;
            self.last_idle_transition = Some(now);
        }
    }

    /// Resets dynamic session state while preserving configuration.
    pub fn reset_session(&mut self) {
        self.last_intervention = None;
        self.recent_interventions.clear();
        self.total_interventions = 0;
        self.snoozed_until = None;
        self.active_suppressions.clear();
        self.is_break_state = false;
        self.is_idle = false;
        self.last_idle_transition = None;
    }

    // ---------------------------------------------------------------------
    // ACCESSORS
    // ---------------------------------------------------------------------

    pub fn config(&self) -> &CooldownConfig {
        &self.config
    }

    pub fn last_intervention(&self) -> Option<&InterventionRecord> {
        self.last_intervention.as_ref()
    }

    pub fn total_interventions(&self) -> u32 {
        self.total_interventions
    }

    pub fn recent_interventions_count(&self, now: DateTime<Utc>) -> usize {
        let cutoff = now - Duration::seconds(i64::from(self.config.rapid_window_seconds));
        self.recent_interventions
            .iter()
            .filter(|&&ts| ts > cutoff)
            .count()
    }

    pub fn is_snoozed(&self, now: DateTime<Utc>) -> bool {
        self.snoozed_until.is_some_and(|until| now < until)
    }

    pub fn remaining_snooze_seconds(&self, now: DateTime<Utc>) -> u32 {
        self.snoozed_until.map_or(0, |until| {
            if now < until {
                u32::try_from((until - now).num_seconds().max(1)).unwrap_or(u32::MAX)
            } else {
                0
            }
        })
    }

    pub fn is_break_state(&self) -> bool {
        self.is_break_state
    }

    pub fn is_idle(&self) -> bool {
        self.is_idle
    }

    // ---------------------------------------------------------------------
    // INTERNAL EVALUATION ENGINE
    // ---------------------------------------------------------------------

    fn evaluate_internal(
        &self,
        now: DateTime<Utc>,
        action: Option<ActionCandidate>,
        app_id: Option<&str>,
        origin: Option<&str>,
    ) -> CooldownStatus {
        // 1. Clock Regression Check
        if let Some(ref last) = self.last_intervention {
            if now < last.timestamp {
                return CooldownStatus::Throttled {
                    reason: ThrottlingReason::TimeRegression,
                    remaining_seconds: self.config.min_interval_seconds,
                };
            }
        }

        // 2. Snooze Check
        if let Some(snooze_end) = self.snoozed_until {
            if now < snooze_end {
                let rem = u32::try_from((snooze_end - now).num_seconds().max(1)).unwrap_or(1);
                return CooldownStatus::Throttled {
                    reason: ThrottlingReason::SnoozeActive,
                    remaining_seconds: rem,
                };
            }
        }

        // 3. Break State Check
        if self.is_break_state {
            let is_allowed_break_action = match action {
                Some(ActionCandidate::SuggestBreak) => false,
                Some(ActionCandidate::Remind) if self.config.allow_break_reminders => true,
                _ => false,
            };
            if !is_allowed_break_action {
                return CooldownStatus::Throttled {
                    reason: ThrottlingReason::BreakStateActive,
                    remaining_seconds: self.config.min_interval_seconds,
                };
            }
        }

        // 4. Idle State Check
        if self.is_idle {
            return CooldownStatus::Throttled {
                reason: ThrottlingReason::IdleActive,
                remaining_seconds: self.config.idle_cooldown_seconds,
            };
        }

        // 5. Target User Correction Suppression Check
        for supp in &self.active_suppressions {
            if now < supp.suppressed_until
                && (supp.is_global() || app_id.is_some() || origin.is_some())
                && supp.matches(app_id, origin)
            {
                let rem =
                    u32::try_from((supp.suppressed_until - now).num_seconds().max(1)).unwrap_or(1);
                return CooldownStatus::Throttled {
                    reason: ThrottlingReason::UserCorrectionSuppressed,
                    remaining_seconds: rem,
                };
            }
        }

        // 6. Cumulative Session Cap Check
        if let Some(cap) = self.config.session_cap {
            if self.total_interventions >= cap {
                return CooldownStatus::Throttled {
                    reason: ThrottlingReason::SessionCapReached,
                    remaining_seconds: u32::MAX,
                };
            }
        }

        // 7. Minimum Interval Check (Consecutive Pacing)
        if let Some(ref last) = self.last_intervention {
            let elapsed = (now - last.timestamp).num_seconds();
            if elapsed < i64::from(self.config.min_interval_seconds) {
                let rem =
                    u32::try_from((i64::from(self.config.min_interval_seconds) - elapsed).max(1))
                        .unwrap_or(1);
                return CooldownStatus::Throttled {
                    reason: ThrottlingReason::RecentIntervention,
                    remaining_seconds: rem,
                };
            }
        }

        // 8. Rolling Window Rapid Succession Check
        let window_duration = Duration::seconds(i64::from(self.config.rapid_window_seconds));
        let cutoff = now - window_duration;
        let active_count = self
            .recent_interventions
            .iter()
            .filter(|&&ts| ts > cutoff)
            .count();

        if active_count >= self.config.max_interventions_per_window as usize {
            // Oldest entry in current window determines remaining throttle duration
            if let Some(&oldest_in_window) =
                self.recent_interventions.iter().find(|&&ts| ts > cutoff)
            {
                let expires_at = oldest_in_window + window_duration;
                let rem = u32::try_from((expires_at - now).num_seconds().max(1)).unwrap_or(1);
                return CooldownStatus::Throttled {
                    reason: ThrottlingReason::RapidSuccessionRateLimit,
                    remaining_seconds: rem,
                };
            }
        }

        CooldownStatus::Allowed
    }

    fn prune_rolling_window(&mut self, now: DateTime<Utc>) {
        let cutoff = now - Duration::seconds(i64::from(self.config.rapid_window_seconds));
        self.recent_interventions.retain(|&ts| ts > cutoff);
    }

    fn prune_suppressions(&mut self, now: DateTime<Utc>) {
        self.active_suppressions
            .retain(|supp| supp.suppressed_until > now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remaining_snooze_seconds_sub_second_boundary() {
        let mut tracker = CooldownTracker::new(CooldownConfig::default()).unwrap();
        let now = Utc::now();

        // 1. Un-snoozed tracker returns 0
        assert_eq!(tracker.remaining_snooze_seconds(now), 0);
        assert!(!tracker.is_snoozed(now));

        // 2. Active snooze for 60 seconds
        tracker.record_snooze(now, 60).unwrap();
        assert!(tracker.is_snoozed(now));
        assert_eq!(tracker.remaining_snooze_seconds(now), 60);

        // 3. Sub-second remaining duration (59s + 999ms, only 1ms remaining before expiration)
        let sub_second_left = now + Duration::seconds(59) + Duration::milliseconds(999);
        assert!(tracker.is_snoozed(sub_second_left));
        // Must return at least 1 second when now < until, matching can_intervene
        assert_eq!(tracker.remaining_snooze_seconds(sub_second_left), 1);
        let status = tracker.can_intervene(sub_second_left);
        assert_eq!(status.remaining_seconds(), 1);

        // 4. Exact expiration timestamp (now == until)
        let exact_expiration = now + Duration::seconds(60);
        assert!(!tracker.is_snoozed(exact_expiration));
        assert_eq!(tracker.remaining_snooze_seconds(exact_expiration), 0);

        // 5. Past expiration timestamp (now > until)
        let past_expiration = now + Duration::seconds(61);
        assert!(!tracker.is_snoozed(past_expiration));
        assert_eq!(tracker.remaining_snooze_seconds(past_expiration), 0);
    }
}
