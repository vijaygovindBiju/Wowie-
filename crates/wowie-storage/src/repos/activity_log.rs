use super::ActivityLogRepository;
use crate::connection::DbPool;
use crate::error::StorageError;
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use wowie_core::id::{ClassificationId, CorrectionId, ObservationId, SegmentId, SessionId};
use wowie_core::models::{
    ActivitySegment, Classification, ClassificationBasis, ClassificationLabel, CorrectionScope,
    IdleState, Observation, ObservationSource, SegmentKind, UserCorrection,
};

#[derive(Serialize, Deserialize)]
struct CorrectionDetails {
    pub app_id: Option<String>,
    pub origin: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SqliteActivityLogRepository {
    pool: DbPool,
}

impl SqliteActivityLogRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    fn row_to_observation(row: &Row) -> Result<Observation, rusqlite::Error> {
        let id_str: String = row.get(0)?;
        let session_id_str: String = row.get(1)?;
        let observed_at_str: String = row.get(2)?;
        let source_str: String = row.get(3)?;
        let app_id: Option<String> = row.get(4)?;
        let window_title: Option<String> = row.get(5)?;
        let origin: Option<String> = row.get(6)?;
        let idle_state_str: String = row.get(7)?;
        let consent_scope_id: Option<String> = row.get(8)?;
        let expires_at_str: Option<String> = row.get(9)?;

        let id = ObservationId::parse_str(&id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let session_id = SessionId::parse_str(&session_id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let observed_at = DateTime::parse_from_rfc3339(&observed_at_str)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?
            .with_timezone(&Utc);

        let source = match source_str.as_str() {
            "platform_app" => ObservationSource::PlatformApp,
            "window_title" => ObservationSource::WindowTitle,
            "browser_origin" => ObservationSource::BrowserOrigin,
            "manual" => ObservationSource::Manual,
            other => {
                return Err(rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Text,
                    format!("Unknown observation source: {other}").into(),
                ))
            }
        };

        let idle_state = match idle_state_str.as_str() {
            "active" => IdleState::Active,
            "idle" => IdleState::Idle,
            "unknown" => IdleState::Unknown,
            other => {
                return Err(rusqlite::Error::FromSqlConversionFailure(
                    7,
                    rusqlite::types::Type::Text,
                    format!("Unknown idle state: {other}").into(),
                ))
            }
        };

        let expires_at = expires_at_str
            .map(|s| DateTime::parse_from_rfc3339(&s).map(|dt| dt.with_timezone(&Utc)))
            .transpose()
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    9,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

        Ok(Observation {
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

    fn row_to_segment(row: &Row) -> Result<ActivitySegment, rusqlite::Error> {
        let id_str: String = row.get(0)?;
        let session_id_str: String = row.get(1)?;
        let start_time_str: String = row.get(2)?;
        let end_time_str: String = row.get(3)?;
        let app_id: Option<String> = row.get(4)?;
        let origin: Option<String> = row.get(5)?;
        let kind_str: String = row.get(6)?;
        let confidence: f64 = row.get(7)?;
        let evidence_json: String = row.get(8)?;

        let id = SegmentId::parse_str(&id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let session_id = SessionId::parse_str(&session_id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let start_time = DateTime::parse_from_rfc3339(&start_time_str)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?
            .with_timezone(&Utc);
        let end_time = DateTime::parse_from_rfc3339(&end_time_str)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?
            .with_timezone(&Utc);

        let kind = match kind_str.as_str() {
            "active_focus" | "active_work" => SegmentKind::ActiveFocus,
            "idle" => SegmentKind::Idle,
            "switching" => SegmentKind::Switching,
            "planned_break" | "break" => SegmentKind::PlannedBreak,
            "unknown" | "ambiguous" | "distraction" => SegmentKind::Unknown,
            other => {
                return Err(rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    format!("Unknown segment kind: {other}").into(),
                ))
            }
        };

        let evidence_ids: Vec<ObservationId> =
            serde_json::from_str(&evidence_json).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    8,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

        Ok(ActivitySegment {
            id,
            session_id,
            start_time,
            end_time,
            app_id,
            origin,
            kind,
            confidence: confidence as f32,
            evidence_ids,
        })
    }

    fn row_to_classification(row: &Row) -> Result<Classification, rusqlite::Error> {
        let id_str: String = row.get(0)?;
        let segment_id_str: String = row.get(1)?;
        let label_str: String = row.get(2)?;
        let confidence: f64 = row.get(3)?;
        let basis_str: String = row.get(4)?;
        let classifier_version: String = row.get(5)?;
        let created_at_str: String = row.get(6)?;

        let id = ClassificationId::parse_str(&id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let segment_id = SegmentId::parse_str(&segment_id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?;

        let label = match label_str.as_str() {
            "relevant" => ClassificationLabel::Relevant,
            "possibly_unrelated" | "neutral" => ClassificationLabel::PossiblyUnrelated,
            "unrelated" | "distracting" => ClassificationLabel::Unrelated,
            "unknown" => ClassificationLabel::Unknown,
            other => {
                return Err(rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    format!("Unknown classification label: {other}").into(),
                ))
            }
        };

        let basis = match basis_str.as_str() {
            "deterministic_rule" => ClassificationBasis::DeterministicRule,
            "ai_suggestion" => ClassificationBasis::AiSuggestion,
            "user_correction" => ClassificationBasis::UserCorrection,
            _ => ClassificationBasis::DeterministicRule,
        };

        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?
            .with_timezone(&Utc);

        Ok(Classification {
            id,
            segment_id,
            label,
            confidence: confidence as f32,
            basis,
            classifier_version,
            created_at,
        })
    }

    fn row_to_correction(row: &Row) -> Result<UserCorrection, rusqlite::Error> {
        let id_str: String = row.get(0)?;
        let session_id_str: String = row.get(1)?;
        let target_segment_id_str: Option<String> = row.get(2)?;
        let target_app_or_origin: Option<String> = row.get(3)?;
        let scope_str: Option<String> = row.get(4)?;
        let label_str: Option<String> = row.get(5)?;
        let note: Option<String> = row.get(6)?;
        let details_json: String = row.get(7)?;
        let created_at_str: String = row.get(8)?;

        let id = CorrectionId::parse_str(&id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let session_id = SessionId::parse_str(&session_id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let target_segment_id = target_segment_id_str
            .map(|s| SegmentId::parse_str(&s))
            .transpose()
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

        let label = match label_str.as_deref().unwrap_or("relevant") {
            "relevant" => ClassificationLabel::Relevant,
            "possibly_unrelated" => ClassificationLabel::PossiblyUnrelated,
            "unrelated" => ClassificationLabel::Unrelated,
            _ => ClassificationLabel::Relevant,
        };

        let scope = match scope_str.as_deref().unwrap_or("session") {
            "current_event" | "item" => CorrectionScope::CurrentEvent,
            "session" => CorrectionScope::Session,
            "persistent_rule" => CorrectionScope::PersistentRule,
            _ => CorrectionScope::Session,
        };

        let details: Option<CorrectionDetails> = serde_json::from_str(&details_json).ok();
        let app_id = details
            .as_ref()
            .and_then(|d| d.app_id.clone())
            .or_else(|| target_app_or_origin.clone());
        let origin = details.as_ref().and_then(|d| d.origin.clone());

        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    8,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?
            .with_timezone(&Utc);

        Ok(UserCorrection {
            id,
            session_id,
            target_segment_id,
            app_id,
            origin,
            label,
            scope,
            note,
            created_at,
        })
    }
}

impl ActivityLogRepository for SqliteActivityLogRepository {
    fn record_observation(&self, observation: &Observation) -> Result<(), StorageError> {
        self.pool.with_conn(|conn| {
            conn.execute(
                "INSERT INTO activity_observations (
                    id, session_id, observed_at, source, app_id, window_title, origin,
                    idle_state, consent_scope_id, expires_at, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    observation.id.to_string(),
                    observation.session_id.to_string(),
                    observation.observed_at.to_rfc3339(),
                    observation.source.to_string(),
                    observation.app_id,
                    observation.window_title,
                    observation.origin,
                    observation.idle_state.to_string(),
                    observation.consent_scope_id,
                    observation.expires_at.map(|dt| dt.to_rfc3339()),
                    observation.observed_at.to_rfc3339(),
                ],
            )?;
            Ok(())
        })
    }

    fn record_observations_batch(&self, observations: &[Observation]) -> Result<(), StorageError> {
        self.pool.transaction(|tx| {
            let mut stmt = tx.prepare(
                "INSERT INTO activity_observations (
                    id, session_id, observed_at, source, app_id, window_title, origin,
                    idle_state, consent_scope_id, expires_at, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )?;
            for obs in observations {
                stmt.execute(params![
                    obs.id.to_string(),
                    obs.session_id.to_string(),
                    obs.observed_at.to_rfc3339(),
                    obs.source.to_string(),
                    obs.app_id,
                    obs.window_title,
                    obs.origin,
                    obs.idle_state.to_string(),
                    obs.consent_scope_id,
                    obs.expires_at.map(|dt| dt.to_rfc3339()),
                    obs.observed_at.to_rfc3339(),
                ])?;
            }
            Ok(())
        })
    }

    fn get_observation(&self, id: ObservationId) -> Result<Option<Observation>, StorageError> {
        self.pool.with_conn(|conn| {
            let res = conn
                .query_row(
                    "SELECT id, session_id, observed_at, source, app_id, window_title, origin,
                            idle_state, consent_scope_id, expires_at
                     FROM activity_observations WHERE id = ?1",
                    params![id.to_string()],
                    Self::row_to_observation,
                )
                .optional()?;
            Ok(res)
        })
    }

    fn get_session_observations(
        &self,
        session_id: SessionId,
        limit: Option<usize>,
    ) -> Result<Vec<Observation>, StorageError> {
        self.pool.with_conn(|conn| {
            let sql = if let Some(limit_val) = limit {
                format!(
                    "SELECT id, session_id, observed_at, source, app_id, window_title, origin,
                            idle_state, consent_scope_id, expires_at
                     FROM activity_observations WHERE session_id = ?1 ORDER BY observed_at ASC LIMIT {limit_val}"
                )
            } else {
                "SELECT id, session_id, observed_at, source, app_id, window_title, origin,
                        idle_state, consent_scope_id, expires_at
                 FROM activity_observations WHERE session_id = ?1 ORDER BY observed_at ASC"
                    .to_string()
            };
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params![session_id.to_string()], Self::row_to_observation)?;
            let mut result = Vec::new();
            for r in rows {
                result.push(r?);
            }
            Ok(result)
        })
    }

    fn delete_expired_observations(&self, now: DateTime<Utc>) -> Result<usize, StorageError> {
        self.pool.with_conn(|conn| {
            let deleted = conn.execute(
                "DELETE FROM activity_observations WHERE expires_at IS NOT NULL AND expires_at < ?1",
                params![now.to_rfc3339()],
            )?;
            Ok(deleted)
        })
    }

    fn record_segment(&self, segment: &ActivitySegment) -> Result<(), StorageError> {
        let evidence_json = serde_json::to_string(&segment.evidence_ids)?;
        self.pool.with_conn(|conn| {
            conn.execute(
                "INSERT INTO activity_segments (
                    id, session_id, start_time, end_time, app_id, origin, kind, confidence, evidence_refs, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    segment.id.to_string(),
                    segment.session_id.to_string(),
                    segment.start_time.to_rfc3339(),
                    segment.end_time.to_rfc3339(),
                    segment.app_id,
                    segment.origin,
                    segment.kind.to_string(),
                    f64::from(segment.confidence),
                    evidence_json,
                    segment.start_time.to_rfc3339(),
                ],
            )?;
            Ok(())
        })
    }

    fn get_segment(&self, id: SegmentId) -> Result<Option<ActivitySegment>, StorageError> {
        self.pool.with_conn(|conn| {
            let res = conn
                .query_row(
                    "SELECT id, session_id, start_time, end_time, app_id, origin, kind, confidence, evidence_refs
                     FROM activity_segments WHERE id = ?1",
                    params![id.to_string()],
                    Self::row_to_segment,
                )
                .optional()?;
            Ok(res)
        })
    }

    fn get_session_segments(
        &self,
        session_id: SessionId,
    ) -> Result<Vec<ActivitySegment>, StorageError> {
        self.pool.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, start_time, end_time, app_id, origin, kind, confidence, evidence_refs
                 FROM activity_segments WHERE session_id = ?1 ORDER BY start_time ASC",
            )?;
            let rows = stmt.query_map(params![session_id.to_string()], Self::row_to_segment)?;
            let mut result = Vec::new();
            for r in rows {
                result.push(r?);
            }
            Ok(result)
        })
    }

    fn record_classification(&self, classification: &Classification) -> Result<(), StorageError> {
        self.pool.with_conn(|conn| {
            conn.execute(
                "INSERT INTO classifications (id, segment_id, label, confidence, basis, classifier_version, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    classification.id.to_string(),
                    classification.segment_id.to_string(),
                    classification.label.to_string(),
                    f64::from(classification.confidence),
                    classification.basis.to_string(),
                    classification.classifier_version,
                    classification.created_at.to_rfc3339(),
                ],
            )?;
            Ok(())
        })
    }

    fn get_classification(
        &self,
        id: ClassificationId,
    ) -> Result<Option<Classification>, StorageError> {
        self.pool.with_conn(|conn| {
            let res = conn
                .query_row(
                    "SELECT id, segment_id, label, confidence, basis, classifier_version, created_at
                     FROM classifications WHERE id = ?1",
                    params![id.to_string()],
                    Self::row_to_classification,
                )
                .optional()?;
            Ok(res)
        })
    }

    fn get_segment_classifications(
        &self,
        segment_id: SegmentId,
    ) -> Result<Vec<Classification>, StorageError> {
        self.pool.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, segment_id, label, confidence, basis, classifier_version, created_at
                 FROM classifications WHERE segment_id = ?1 ORDER BY created_at ASC",
            )?;
            let rows =
                stmt.query_map(params![segment_id.to_string()], Self::row_to_classification)?;
            let mut result = Vec::new();
            for r in rows {
                result.push(r?);
            }
            Ok(result)
        })
    }

    fn record_correction(&self, correction: &UserCorrection) -> Result<(), StorageError> {
        let details = CorrectionDetails {
            app_id: correction.app_id.clone(),
            origin: correction.origin.clone(),
        };
        let details_json = serde_json::to_string(&details)?;
        let target_app_or_origin = correction
            .app_id
            .clone()
            .or_else(|| correction.origin.clone());

        self.pool.with_conn(|conn| {
            conn.execute(
                "INSERT INTO user_actions (
                    id, session_id, action_type, target_segment_id, target_app_or_origin,
                    scope, label, note, details, created_at
                 ) VALUES (?1, ?2, 'correct', ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    correction.id.to_string(),
                    correction.session_id.to_string(),
                    correction.target_segment_id.map(|id| id.to_string()),
                    target_app_or_origin,
                    correction.scope.to_string(),
                    correction.label.to_string(),
                    correction.note,
                    details_json,
                    correction.created_at.to_rfc3339(),
                ],
            )?;
            Ok(())
        })
    }

    fn get_correction(&self, id: CorrectionId) -> Result<Option<UserCorrection>, StorageError> {
        self.pool.with_conn(|conn| {
            let res = conn
                .query_row(
                    "SELECT id, session_id, target_segment_id, target_app_or_origin, scope, label, note, details, created_at
                     FROM user_actions WHERE id = ?1 AND action_type = 'correct'",
                    params![id.to_string()],
                    Self::row_to_correction,
                )
                .optional()?;
            Ok(res)
        })
    }

    fn get_session_corrections(
        &self,
        session_id: SessionId,
    ) -> Result<Vec<UserCorrection>, StorageError> {
        self.pool.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, target_segment_id, target_app_or_origin, scope, label, note, details, created_at
                 FROM user_actions WHERE session_id = ?1 AND action_type = 'correct' ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map(params![session_id.to_string()], Self::row_to_correction)?;
            let mut result = Vec::new();
            for r in rows {
                result.push(r?);
            }
            Ok(result)
        })
    }

    fn get_persistent_corrections(&self) -> Result<Vec<UserCorrection>, StorageError> {
        self.pool.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, target_segment_id, target_app_or_origin, scope, label, note, details, created_at
                 FROM user_actions WHERE action_type = 'correct' AND scope = 'persistent_rule' ORDER BY created_at ASC",
            )?;
            let rows = stmt.query_map([], Self::row_to_correction)?;
            let mut result = Vec::new();
            for r in rows {
                result.push(r?);
            }
            Ok(result)
        })
    }
}
