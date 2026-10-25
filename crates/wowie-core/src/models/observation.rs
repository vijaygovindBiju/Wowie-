use crate::error::CoreError;
use crate::id::{ObservationId, SegmentId, SessionId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const MAX_APP_ID_LENGTH: usize = 256;
pub const MAX_WINDOW_TITLE_LENGTH: usize = 256;
pub const MAX_ORIGIN_LENGTH: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationSource {
    PlatformApp,
    WindowTitle,
    BrowserOrigin,
    Manual,
}

impl fmt::Display for ObservationSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PlatformApp => write!(f, "platform_app"),
            Self::WindowTitle => write!(f, "window_title"),
            Self::BrowserOrigin => write!(f, "browser_origin"),
            Self::Manual => write!(f, "manual"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdleState {
    Active,
    Idle,
    Unknown,
}

impl fmt::Display for IdleState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Idle => write!(f, "idle"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observation {
    pub id: ObservationId,
    pub session_id: SessionId,
    pub observed_at: DateTime<Utc>,
    pub source: ObservationSource,
    pub app_id: Option<String>,
    pub window_title: Option<String>,
    pub origin: Option<String>,
    pub idle_state: IdleState,
    pub consent_scope_id: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl Observation {
    pub fn new(
        session_id: SessionId,
        observed_at: DateTime<Utc>,
        source: ObservationSource,
        app_id: Option<String>,
        window_title: Option<String>,
        origin: Option<String>,
        idle_state: IdleState,
        consent_scope_id: Option<String>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<Self, CoreError> {
        Self::with_id(
            ObservationId::new(),
            session_id,
            observed_at,
            source,
            app_id,
            window_title,
            origin,
            idle_state,
            consent_scope_id,
            expires_at,
        )
    }

    pub fn with_id(
        id: ObservationId,
        session_id: SessionId,
        observed_at: DateTime<Utc>,
        source: ObservationSource,
        app_id: Option<String>,
        window_title: Option<String>,
        origin: Option<String>,
        idle_state: IdleState,
        consent_scope_id: Option<String>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<Self, CoreError> {
        let app_id = app_id.map(|a| a.trim().to_string());
        if let Some(ref a) = app_id {
            if a.chars().count() > MAX_APP_ID_LENGTH {
                return Err(CoreError::validation(
                    "app_id",
                    format!("App ID exceeds maximum of {} characters", MAX_APP_ID_LENGTH),
                ));
            }
        }

        let window_title = window_title.map(|t| t.trim().to_string());
        if let Some(ref t) = window_title {
            if t.chars().count() > MAX_WINDOW_TITLE_LENGTH {
                return Err(CoreError::validation(
                    "window_title",
                    format!(
                        "Window title exceeds maximum of {} characters",
                        MAX_WINDOW_TITLE_LENGTH
                    ),
                ));
            }
        }

        let origin = origin.map(|o| o.trim().to_string());
        if let Some(ref o) = origin {
            if o.chars().count() > MAX_ORIGIN_LENGTH {
                return Err(CoreError::validation(
                    "origin",
                    format!("Origin exceeds maximum of {} characters", MAX_ORIGIN_LENGTH),
                ));
            }
        }

        if let Some(exp) = expires_at {
            if exp < observed_at {
                return Err(CoreError::validation(
                    "expires_at",
                    "Expiration time cannot precede observation time",
                ));
            }
        }

        Ok(Self {
            id,
            session_id,
            observed_at,
            source,
            app_id,
            window_title,
            origin,
            idle_state,
            consent_scope_id,
            expires_at,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SegmentKind {
    ActiveFocus,
    Idle,
    Switching,
    PlannedBreak,
    Unknown,
}

impl fmt::Display for SegmentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ActiveFocus => write!(f, "active_focus"),
            Self::Idle => write!(f, "idle"),
            Self::Switching => write!(f, "switching"),
            Self::PlannedBreak => write!(f, "planned_break"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivitySegment {
    pub id: SegmentId,
    pub session_id: SessionId,
    pub start_time: DateTime<Utc>,
    pub end_time: DateTime<Utc>,
    pub app_id: Option<String>,
    pub origin: Option<String>,
    pub kind: SegmentKind,
    pub confidence: f32,
    pub evidence_ids: Vec<ObservationId>,
}

impl ActivitySegment {
    pub fn new(
        session_id: SessionId,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        app_id: Option<String>,
        origin: Option<String>,
        kind: SegmentKind,
        confidence: f32,
        evidence_ids: Vec<ObservationId>,
    ) -> Result<Self, CoreError> {
        Self::with_id(
            SegmentId::new(),
            session_id,
            start_time,
            end_time,
            app_id,
            origin,
            kind,
            confidence,
            evidence_ids,
        )
    }

    pub fn with_id(
        id: SegmentId,
        session_id: SessionId,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        app_id: Option<String>,
        origin: Option<String>,
        kind: SegmentKind,
        confidence: f32,
        evidence_ids: Vec<ObservationId>,
    ) -> Result<Self, CoreError> {
        if end_time < start_time {
            return Err(CoreError::validation(
                "end_time",
                "Segment end_time cannot precede start_time",
            ));
        }
        if !(0.0..=1.0).contains(&confidence) {
            return Err(CoreError::validation(
                "confidence",
                "Confidence must be between 0.0 and 1.0",
            ));
        }

        Ok(Self {
            id,
            session_id,
            start_time,
            end_time,
            app_id,
            origin,
            kind,
            confidence,
            evidence_ids,
        })
    }

    pub fn duration_seconds(&self) -> u64 {
        (self.end_time - self.start_time).num_seconds().max(0) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_observation_bounds() {
        let now = Utc::now();
        let session_id = SessionId::new();

        // Valid observation
        let obs = Observation::new(
            session_id,
            now,
            ObservationSource::PlatformApp,
            Some("code".into()),
            Some("main.rs - VS Code".into()),
            None,
            IdleState::Active,
            Some("profile-1".into()),
            Some(now + Duration::hours(24)),
        )
        .unwrap();
        assert_eq!(obs.app_id.as_deref(), Some("code"));

        // App id too long
        let long_app = "x".repeat(MAX_APP_ID_LENGTH + 1);
        assert!(Observation::new(
            session_id,
            now,
            ObservationSource::PlatformApp,
            Some(long_app),
            None,
            None,
            IdleState::Active,
            None,
            None,
        )
        .is_err());

        // Expiration preceding observation time
        assert!(Observation::new(
            session_id,
            now,
            ObservationSource::PlatformApp,
            Some("code".into()),
            None,
            None,
            IdleState::Active,
            None,
            Some(now - Duration::seconds(1)),
        )
        .is_err());
    }

    #[test]
    fn test_segment_bounds() {
        let now = Utc::now();
        let session_id = SessionId::new();

        // End before start
        assert!(ActivitySegment::new(
            session_id,
            now,
            now - Duration::seconds(10),
            Some("code".into()),
            None,
            SegmentKind::ActiveFocus,
            0.9,
            vec![],
        )
        .is_err());

        // Confidence out of range
        assert!(ActivitySegment::new(
            session_id,
            now,
            now + Duration::seconds(10),
            Some("code".into()),
            None,
            SegmentKind::ActiveFocus,
            1.5,
            vec![],
        )
        .is_err());

        // Valid segment
        let seg = ActivitySegment::new(
            session_id,
            now,
            now + Duration::seconds(100),
            Some("code".into()),
            None,
            SegmentKind::ActiveFocus,
            0.85,
            vec![ObservationId::new()],
        )
        .unwrap();
        assert_eq!(seg.duration_seconds(), 100);
    }
}
