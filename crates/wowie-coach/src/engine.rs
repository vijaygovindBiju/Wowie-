//! Unified CoachEngine facade coordinating cooldown policy, prompt synthesis,
//! provider evaluation, response validation, and deterministic fallbacks.

use crate::error::{CoachError, Result};
use crate::models::action::ActionCandidate;
use crate::models::context::{CoachContext, UserCorrectionSummary};
use crate::models::response::{CoachResponse, ProviderProvenance};
use crate::models::tone::TonePreferences;
use crate::policy::cooldown::{CooldownConfig, CooldownStatus, CooldownTracker};
use crate::policy::fallback::DeterministicFallback;
use crate::prompt::builder::PromptBuilder;
use crate::provider::trait_def::CoachProvider;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use wowie_core::models::CorrectionScope;

/// Configuration for the `CoachEngine` facade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoachEngineConfig {
    /// Pacing and rate-limiting configuration.
    pub cooldown: CooldownConfig,

    /// Whether to automatically catch provider or validation errors and return a
    /// safe deterministic fallback response instead of failing. Default: true.
    pub fallback_on_error: bool,

    /// Whether to attach JSON schema to `CoachPrompt`. Default: true.
    pub attach_json_schema: bool,

    /// Whether to include schema text directly in the system prompt. Default: true.
    pub include_schema_in_system: bool,

    /// Optional custom preamble for system prompts.
    pub system_preamble: Option<String>,
}

impl Default for CoachEngineConfig {
    fn default() -> Self {
        Self {
            cooldown: CooldownConfig::default(),
            fallback_on_error: true,
            attach_json_schema: true,
            include_schema_in_system: true,
            system_preamble: None,
        }
    }
}

impl CoachEngineConfig {
    /// Creates a new default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Validates internal configuration constraints (such as cooldown invariants).
    pub fn validate(&self) -> Result<()> {
        self.cooldown.validate()
    }

    /// Preconfigured gentle pacing preset (longer intervals, lower intervention cap).
    pub fn gentle() -> Self {
        Self {
            cooldown: CooldownConfig::gentle(),
            ..Self::default()
        }
    }

    /// Preconfigured strict pacing preset (shorter intervals, higher intervention cap).
    pub fn strict() -> Self {
        Self {
            cooldown: CooldownConfig::strict(),
            ..Self::default()
        }
    }

    /// Configures engine parameters adapted to user tone preferences.
    pub fn from_tone(tone: &TonePreferences) -> Self {
        Self {
            cooldown: CooldownConfig::from_tone(tone),
            ..Self::default()
        }
    }

    /// Sets custom cooldown pacing configuration.
    #[must_use]
    pub fn with_cooldown(mut self, cooldown: CooldownConfig) -> Self {
        self.cooldown = cooldown;
        self
    }

    /// Sets whether fallback on error is enabled.
    #[must_use]
    pub fn with_fallback_on_error(mut self, enabled: bool) -> Self {
        self.fallback_on_error = enabled;
        self
    }

    /// Sets whether JSON schema is attached to prompt payloads.
    #[must_use]
    pub fn with_attach_json_schema(mut self, enabled: bool) -> Self {
        self.attach_json_schema = enabled;
        self
    }

    /// Sets whether JSON schema instructions are included in the system prompt.
    #[must_use]
    pub fn with_include_schema_in_system(mut self, enabled: bool) -> Self {
        self.include_schema_in_system = enabled;
        self
    }

    /// Sets an optional custom preamble for the system prompt.
    #[must_use]
    pub fn with_system_preamble(mut self, preamble: impl Into<String>) -> Self {
        self.system_preamble = Some(preamble.into());
        self
    }
}

/// Builder for constructing a `CoachEngine` with custom components or configuration.
#[derive(Debug, Default)]
pub struct CoachEngineBuilder {
    provider: Option<Arc<dyn CoachProvider>>,
    config: CoachEngineConfig,
    prompt_builder: Option<PromptBuilder>,
    fallback: Option<DeterministicFallback>,
}

impl CoachEngineBuilder {
    /// Creates a new empty `CoachEngineBuilder`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Configures the AI provider.
    #[must_use]
    pub fn provider(mut self, provider: Arc<dyn CoachProvider>) -> Self {
        self.provider = Some(provider);
        self
    }

    /// Configures the AI provider (alias for `provider`).
    #[must_use]
    pub fn with_provider(self, provider: Arc<dyn CoachProvider>) -> Self {
        self.provider(provider)
    }

    /// Configures the engine configuration.
    #[must_use]
    pub fn config(mut self, config: CoachEngineConfig) -> Self {
        self.config = config;
        self
    }

    /// Configures the engine configuration (alias for `config`).
    #[must_use]
    pub fn with_config(self, config: CoachEngineConfig) -> Self {
        self.config(config)
    }

    /// Configures the cooldown policy.
    #[must_use]
    pub fn cooldown(mut self, cooldown: CooldownConfig) -> Self {
        self.config.cooldown = cooldown;
        self
    }

    /// Configures the cooldown policy (alias for `cooldown`).
    #[must_use]
    pub fn with_cooldown(self, cooldown: CooldownConfig) -> Self {
        self.cooldown(cooldown)
    }

    /// Sets whether fallback on error is enabled.
    #[must_use]
    pub fn fallback_on_error(mut self, enabled: bool) -> Self {
        self.config.fallback_on_error = enabled;
        self
    }

    /// Sets whether fallback on error is enabled (alias for `fallback_on_error`).
    #[must_use]
    pub fn with_fallback_on_error(self, enabled: bool) -> Self {
        self.fallback_on_error(enabled)
    }

    /// Sets whether JSON schema is attached to prompt payloads.
    #[must_use]
    pub fn attach_json_schema(mut self, enabled: bool) -> Self {
        self.config.attach_json_schema = enabled;
        self
    }

    /// Sets whether JSON schema is attached (alias for `attach_json_schema`).
    #[must_use]
    pub fn with_attach_json_schema(self, enabled: bool) -> Self {
        self.attach_json_schema(enabled)
    }

    /// Sets whether JSON schema instructions are included in the system prompt.
    #[must_use]
    pub fn include_schema_in_system(mut self, enabled: bool) -> Self {
        self.config.include_schema_in_system = enabled;
        self
    }

    /// Sets whether JSON schema instructions are included in the system prompt (alias for `include_schema_in_system`).
    #[must_use]
    pub fn with_include_schema_in_system(self, enabled: bool) -> Self {
        self.include_schema_in_system(enabled)
    }

    /// Sets an optional custom preamble for the system prompt.
    #[must_use]
    pub fn system_preamble(mut self, preamble: impl Into<String>) -> Self {
        self.config.system_preamble = Some(preamble.into());
        self
    }

    /// Sets an optional custom preamble (alias for `system_preamble`).
    #[must_use]
    pub fn with_system_preamble(self, preamble: impl Into<String>) -> Self {
        self.system_preamble(preamble)
    }

    /// Overrides the internal `PromptBuilder`.
    #[must_use]
    pub fn prompt_builder(mut self, prompt_builder: PromptBuilder) -> Self {
        self.prompt_builder = Some(prompt_builder);
        self
    }

    /// Overrides the internal `DeterministicFallback`.
    #[must_use]
    pub fn fallback(mut self, fallback: DeterministicFallback) -> Self {
        self.fallback = Some(fallback);
        self
    }

    /// Assembles the `CoachEngine`.
    ///
    /// # Errors
    /// Returns `CoachError::ProviderUnavailable` if no provider was provided,
    /// or `CoachError` if configuration validation fails.
    pub fn build(self) -> Result<CoachEngine> {
        let provider = self.provider.ok_or_else(|| {
            CoachError::provider_unavailable("builder", "No provider configured for CoachEngine")
        })?;
        self.config.validate()?;

        let cooldown_tracker = CooldownTracker::new(self.config.cooldown.clone())?;

        let prompt_builder = if let Some(pb) = self.prompt_builder {
            pb
        } else {
            let mut pb = PromptBuilder::new()
                .with_schema_in_system(self.config.include_schema_in_system)
                .with_attached_json_schema(self.config.attach_json_schema);
            if let Some(ref preamble) = self.config.system_preamble {
                pb = pb.with_system_preamble(preamble.clone());
            }
            pb
        };

        let fallback = self
            .fallback
            .unwrap_or_else(|| DeterministicFallback::with_provider(provider.name()));

        Ok(CoachEngine {
            provider,
            config: self.config,
            prompt_builder,
            fallback,
            cooldown_tracker: Arc::new(RwLock::new(cooldown_tracker)),
        })
    }
}

/// Primary facade for the Wowie AI Coach dialogue and intervention engine.
///
/// Thread-safe (`Send + Sync`) and cloneable (`Arc`-backed shared state).
#[derive(Debug, Clone)]
pub struct CoachEngine {
    provider: Arc<dyn CoachProvider>,
    config: CoachEngineConfig,
    prompt_builder: PromptBuilder,
    fallback: DeterministicFallback,
    cooldown_tracker: Arc<RwLock<CooldownTracker>>,
}

impl CoachEngine {
    /// Creates a new `CoachEngineBuilder`.
    pub fn builder() -> CoachEngineBuilder {
        CoachEngineBuilder::new()
    }

    /// Creates a new `CoachEngine` with the given provider and configuration.
    ///
    /// # Errors
    /// Returns `CoachError` if configuration validation fails.
    pub fn new(provider: Arc<dyn CoachProvider>, config: CoachEngineConfig) -> Result<Self> {
        CoachEngineBuilder::new()
            .with_provider(provider)
            .with_config(config)
            .build()
    }

    /// Creates a new `CoachEngine` with default configuration.
    pub fn with_default_config(provider: Arc<dyn CoachProvider>) -> Self {
        Self::new(provider, CoachEngineConfig::default())
            .expect("Default configuration must always validate")
    }

    /// Primary evaluation pipeline with automatic fallback and cooldown handling.
    ///
    /// This method is infallible and will never panic. If the cooldown policy throttles
    /// the request, a safe fallback response is returned immediately without calling
    /// the underlying LLM provider. If the provider fails or returns invalid data,
    /// a safe deterministic fallback response is generated.
    pub async fn evaluate(&self, context: &CoachContext, now: DateTime<Utc>) -> CoachResponse {
        // Phase 1: Check cooldown policy gate. Lock is dropped before proceeding.
        let status = {
            let tracker = self.tracker_read();
            tracker.can_intervene_context(now, context)
        };

        if let Some(err) = status.to_coach_error() {
            let mut response = self
                .fallback
                .evaluate_with_provider(&err, "cooldown_policy", now);
            if let CoachError::CooldownThrottled {
                remaining_seconds,
                ref reason,
            } = err
            {
                response.rationale = format!(
                    "Coaching intervention throttled by cooldown policy ({remaining_seconds}s remaining; {reason})."
                );
            }
            return response;
        }

        // Phase 2: Build prompt payload.
        let prompt = match self.prompt_builder.build(context) {
            Ok(p) => p,
            Err(err) => {
                return self
                    .fallback
                    .evaluate_with_provider(&err, self.provider.name(), now);
            }
        };

        // Phase 3: Evaluate via provider with ZERO locks held across .await point.
        let raw_response = match self.provider.evaluate(&prompt).await {
            Ok(r) => r,
            Err(err) => {
                return self
                    .fallback
                    .evaluate_with_provider(&err, self.provider.name(), now);
            }
        };

        // Phase 4: Validate response and assemble provenance.
        let latency_ms = raw_response.latency_ms;
        let model_name = raw_response
            .model
            .clone()
            .unwrap_or_else(|| self.provider.model_name().to_string());
        let provenance = match ProviderProvenance::new(
            self.provider.name(),
            model_name,
            env!("CARGO_PKG_VERSION"),
            now,
            latency_ms,
        ) {
            Ok(p) => p,
            Err(err) => {
                return self
                    .fallback
                    .evaluate_with_provider(&err, self.provider.name(), now);
            }
        };

        let response = match raw_response.into_validated_response(provenance) {
            Ok(resp) => resp,
            Err(err) => {
                return self
                    .fallback
                    .evaluate_with_provider(&err, self.provider.name(), now);
            }
        };

        // Phase 5: If proactive intervention, record into CooldownTracker.
        if let Some(action) = response.action_candidate {
            if !action.is_none() {
                let mut tracker = self.tracker_write();
                tracker.record_intervention(now, action);
            }
        }

        response
    }

    /// Strict evaluation pipeline propagating typed errors without fallback absorption.
    ///
    /// # Errors
    /// Returns `CoachError::CooldownThrottled` if throttled by policy,
    /// or errors from prompt construction, provider execution, or schema validation.
    pub async fn evaluate_strict(
        &self,
        context: &CoachContext,
        now: DateTime<Utc>,
    ) -> Result<CoachResponse> {
        // Phase 1: Check cooldown policy gate. Lock is dropped before proceeding.
        let status = {
            let tracker = self.tracker_read();
            tracker.can_intervene_context(now, context)
        };

        if let Some(err) = status.to_coach_error() {
            return Err(err);
        }

        // Phase 2: Build prompt payload.
        let prompt = self.prompt_builder.build(context)?;

        // Phase 3: Evaluate via provider with ZERO locks held across .await point.
        let raw_response = self.provider.evaluate(&prompt).await?;

        // Phase 4: Validate response and assemble provenance.
        let latency_ms = raw_response.latency_ms;
        let model_name = raw_response
            .model
            .clone()
            .unwrap_or_else(|| self.provider.model_name().to_string());
        let provenance = ProviderProvenance::new(
            self.provider.name(),
            model_name,
            env!("CARGO_PKG_VERSION"),
            now,
            latency_ms,
        )?;

        let response = raw_response.into_validated_response(provenance)?;

        // Phase 5: If proactive intervention, record into CooldownTracker.
        if let Some(action) = response.action_candidate {
            if !action.is_none() {
                let mut tracker = self.tracker_write();
                tracker.record_intervention(now, action);
            }
        }

        Ok(response)
    }

    /// Alias for `evaluate_strict` for diagnostic workflows that propagate errors instead of falling back.
    pub async fn evaluate_without_fallback(
        &self,
        context: &CoachContext,
        now: DateTime<Utc>,
    ) -> Result<CoachResponse> {
        self.evaluate_strict(context, now).await
    }

    // ---------------------------------------------------------------------
    // STATE MUTATION API
    // ---------------------------------------------------------------------

    /// Records an intervention directly into the cooldown tracker.
    pub fn record_intervention(&self, now: DateTime<Utc>, action: ActionCandidate) {
        self.tracker_write().record_intervention(now, action);
    }

    /// Records a user classification correction, suppressing interventions for the target.
    pub fn record_user_correction(&self, now: DateTime<Utc>, scope: CorrectionScope) {
        self.tracker_write().record_user_correction(now, scope);
    }

    /// Records a user classification correction with target metadata.
    pub fn record_targeted_correction(
        &self,
        now: DateTime<Utc>,
        scope: CorrectionScope,
        app_id: Option<String>,
        origin: Option<String>,
    ) {
        self.tracker_write()
            .record_targeted_correction(now, scope, app_id, origin);
    }

    /// Records a correction from a `UserCorrectionSummary`.
    pub fn record_correction_summary(&self, now: DateTime<Utc>, summary: &UserCorrectionSummary) {
        self.tracker_write().record_correction_summary(now, summary);
    }

    /// Batch-syncs a list of past user corrections into the tracker.
    pub fn sync_corrections(&self, now: DateTime<Utc>, corrections: &[UserCorrectionSummary]) {
        let mut tracker = self.tracker_write();
        for corr in corrections {
            tracker.record_correction_summary(now, corr);
        }
    }

    /// Records a user snooze request for `duration_seconds`.
    ///
    /// # Errors
    /// Returns `CoachError` if duration constraints are violated.
    pub fn record_snooze(&self, now: DateTime<Utc>, duration_seconds: u32) -> Result<()> {
        self.tracker_write().record_snooze(now, duration_seconds)
    }

    /// Cancels any active snooze.
    pub fn cancel_snooze(&self) {
        self.tracker_write().cancel_snooze();
    }

    /// Updates active break state.
    pub fn set_break_state(&self, is_break: bool, now: DateTime<Utc>) {
        self.tracker_write().set_break_state(is_break, now);
    }

    /// Updates active idle state.
    pub fn set_idle_state(&self, is_idle: bool, now: DateTime<Utc>) {
        self.tracker_write().set_idle_state(is_idle, now);
    }

    /// Resets dynamic session state while preserving configuration.
    pub fn reset_session(&self) {
        self.tracker_write().reset_session();
    }

    // ---------------------------------------------------------------------
    // ACCESSORS & INTROSPECTION
    // ---------------------------------------------------------------------

    /// Checks cooldown status for the current injected timestamp.
    pub fn cooldown_status(&self, now: DateTime<Utc>) -> CooldownStatus {
        self.can_intervene(now)
    }

    /// Checks whether an intervention is permitted given the current injected timestamp.
    pub fn can_intervene(&self, now: DateTime<Utc>) -> CooldownStatus {
        self.tracker_read().can_intervene(now)
    }

    /// Evaluates whether an intervention is permitted for full `CoachContext`.
    pub fn can_intervene_context(
        &self,
        now: DateTime<Utc>,
        context: &CoachContext,
    ) -> CooldownStatus {
        self.tracker_read().can_intervene_context(now, context)
    }

    /// Returns whether interventions are currently snoozed.
    pub fn is_snoozed(&self, now: DateTime<Utc>) -> bool {
        self.tracker_read().is_snoozed(now)
    }

    /// Returns remaining snooze duration in seconds, or 0 if not snoozed.
    pub fn remaining_snooze_seconds(&self, now: DateTime<Utc>) -> u32 {
        self.tracker_read().remaining_snooze_seconds(now)
    }

    /// Returns whether the tracker is currently in a break state.
    pub fn is_break_state(&self) -> bool {
        self.tracker_read().is_break_state()
    }

    /// Returns whether the tracker is currently in an idle state.
    pub fn is_idle(&self) -> bool {
        self.tracker_read().is_idle()
    }

    /// Returns total interventions recorded in this session.
    pub fn total_interventions(&self) -> u32 {
        self.tracker_read().total_interventions()
    }

    /// Returns count of recent interventions within the rapid rolling window.
    pub fn recent_interventions_count(&self, now: DateTime<Utc>) -> usize {
        self.tracker_read().recent_interventions_count(now)
    }

    /// Clones and returns an in-memory snapshot of the current `CooldownTracker`.
    pub fn cooldown_snapshot(&self) -> CooldownTracker {
        self.tracker_read().clone()
    }

    /// Returns reference to the configured LLM provider.
    pub fn provider(&self) -> &Arc<dyn CoachProvider> {
        &self.provider
    }

    /// Returns reference to engine configuration.
    pub fn config(&self) -> &CoachEngineConfig {
        &self.config
    }

    /// Returns reference to internal prompt builder.
    pub fn prompt_builder(&self) -> &PromptBuilder {
        &self.prompt_builder
    }

    /// Returns reference to internal deterministic fallback generator.
    pub fn fallback(&self) -> &DeterministicFallback {
        &self.fallback
    }

    /// Returns clone of the shared `Arc<RwLock<CooldownTracker>>`.
    pub fn cooldown_tracker(&self) -> Arc<RwLock<CooldownTracker>> {
        Arc::clone(&self.cooldown_tracker)
    }

    // ---------------------------------------------------------------------
    // INTERNAL LOCK HELPERS (POISON-RESISTANT)
    // ---------------------------------------------------------------------

    fn tracker_read(&self) -> std::sync::RwLockReadGuard<'_, CooldownTracker> {
        self.cooldown_tracker
            .read()
            .unwrap_or_else(|p| p.into_inner())
    }

    fn tracker_write(&self) -> std::sync::RwLockWriteGuard<'_, CooldownTracker> {
        self.cooldown_tracker
            .write()
            .unwrap_or_else(|p| p.into_inner())
    }
}
