use crate::error::ObserverError;
use crate::model::{MonitoringState, ObserverConfig, WindowMetadata};
use crate::platform::PlatformObserver;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::RwLock;
use wowie_core::id::SessionId;
use wowie_core::models::{ConsentSnapshot, IdleState, Observation, ObservationSource};

#[derive(Debug, Clone)]
pub struct ActivityObserver {
    platform: Arc<dyn PlatformObserver>,
    state: Arc<RwLock<MonitoringState>>,
    config: ObserverConfig,
    last_metadata: Arc<RwLock<Option<WindowMetadata>>>,
    last_change_time: Arc<RwLock<DateTime<Utc>>>,
}

impl ActivityObserver {
    pub fn new(platform: Arc<dyn PlatformObserver>, config: ObserverConfig) -> Self {
        let initial_state = if platform.is_available() {
            MonitoringState::Stopped
        } else {
            MonitoringState::Unavailable
        };

        Self {
            platform,
            state: Arc::new(RwLock::new(initial_state)),
            config,
            last_metadata: Arc::new(RwLock::new(None)),
            last_change_time: Arc::new(RwLock::new(Utc::now())),
        }
    }

    pub fn platform_name(&self) -> &'static str {
        self.platform.name()
    }

    pub async fn current_state(&self) -> MonitoringState {
        *self.state.read().await
    }

    pub async fn start(&self) -> Result<(), ObserverError> {
        let mut state = self.state.write().await;
        if *state == MonitoringState::Unavailable {
            return Err(ObserverError::PlatformUnsupported(
                "Observer platform is unavailable".to_string(),
            ));
        }
        *state = MonitoringState::Active;
        Ok(())
    }

    pub async fn pause(&self) {
        let mut state = self.state.write().await;
        if *state != MonitoringState::Unavailable {
            *state = MonitoringState::Paused;
        }
    }

    pub async fn resume(&self) -> Result<(), ObserverError> {
        let mut state = self.state.write().await;
        if *state == MonitoringState::Unavailable {
            return Err(ObserverError::PlatformUnsupported(
                "Observer platform is unavailable".to_string(),
            ));
        }
        *state = MonitoringState::Active;
        Ok(())
    }

    pub async fn stop(&self) {
        let mut state = self.state.write().await;
        if *state != MonitoringState::Unavailable {
            *state = MonitoringState::Stopped;
        }
    }

    /// Samples desktop activity strictly according to the session consent snapshot.
    pub async fn sample(
        &self,
        session_id: SessionId,
        consent: &ConsentSnapshot,
        now: DateTime<Utc>,
    ) -> Result<Observation, ObserverError> {
        // Strict privacy check 1: Monitoring must be enabled in consent snapshot
        if !consent.monitoring_enabled {
            return Err(ObserverError::ConsentNotGranted);
        }

        // Check observer runtime state
        let current_state = *self.state.read().await;
        if current_state == MonitoringState::Paused {
            return Err(ObserverError::MonitoringPaused);
        }
        if current_state != MonitoringState::Active {
            return Err(ObserverError::Internal(format!(
                "Monitoring is not active (state: {:?})",
                current_state
            )));
        }

        // Sample raw window metadata from platform
        let raw = self.platform.sample_active_window().await?;

        // Determine idle state based on metadata changes
        let mut last_meta = self.last_metadata.write().await;
        let mut last_change = self.last_change_time.write().await;

        let has_changed = match &*last_meta {
            Some(prev) => prev.app_id != raw.app_id || prev.window_title != raw.window_title,
            None => true,
        };

        if has_changed {
            *last_meta = Some(raw.clone());
            *last_change = now;
        }

        let elapsed_idle = (now.signed_duration_since(*last_change))
            .num_seconds()
            .max(0) as u64;
        let idle_state = if raw.is_idle || elapsed_idle >= self.config.idle_threshold_seconds {
            IdleState::Idle
        } else {
            IdleState::Active
        };

        // Redaction & privacy controls
        let app_id = raw.app_id;

        let window_title = if consent.window_title_redaction_enabled {
            raw.window_title.map(|_| "[Redacted Title]".to_string())
        } else {
            raw.window_title
        };

        let origin = if consent.browser_origin_enabled {
            raw.origin
        } else {
            None
        };

        let source = if origin.is_some() {
            ObservationSource::BrowserOrigin
        } else if window_title.is_some() {
            ObservationSource::WindowTitle
        } else {
            ObservationSource::PlatformApp
        };

        Observation::new(
            session_id,
            now,
            source,
            app_id,
            window_title,
            origin,
            idle_state,
            None,
            None,
        )
        .map_err(|e| ObserverError::Internal(e.to_string()))
    }
}
