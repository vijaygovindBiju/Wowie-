use super::{SessionFilter, SessionRepository};
use crate::connection::DbPool;
use crate::error::StorageError;
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension, Row};
use wowie_core::id::{CommitmentId, GoalId, SessionId};
use wowie_core::models::{
    Commitment, ConsentSnapshot, FocusSession, SessionEndReason, SessionState,
};

#[derive(Clone, Debug)]
pub struct SqliteSessionRepository {
    pool: DbPool,
}

impl SqliteSessionRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    fn row_to_commitment(row: &Row) -> Result<Commitment, rusqlite::Error> {
        let id_str: String = row.get(0)?;
        let goal_id_str: String = row.get(1)?;
        let planned_start_str: String = row.get(2)?;
        let duration_seconds: u32 = row.get(3)?;
        let break_allowance_seconds: u32 = row.get(4)?;
        let monitoring_profile_id: Option<String> = row.get(5)?;
        let created_at_str: String = row.get(6)?;

        let id = CommitmentId::parse_str(&id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let goal_id = GoalId::parse_str(&goal_id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let planned_start = DateTime::parse_from_rfc3339(&planned_start_str)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?
            .with_timezone(&Utc);
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?
            .with_timezone(&Utc);

        Ok(Commitment {
            id,
            goal_id,
            planned_start,
            duration_seconds,
            break_allowance_seconds,
            monitoring_profile_id,
            created_at,
        })
    }

    fn row_to_session(row: &Row) -> Result<FocusSession, rusqlite::Error> {
        let id_str: String = row.get(0)?;
        let commitment_id_str: String = row.get(1)?;
        let state_str: String = row.get(2)?;
        let consent_json: String = row.get(3)?;
        let started_at_str: Option<String> = row.get(4)?;
        let ended_at_str: Option<String> = row.get(5)?;
        let end_reason_json: Option<String> = row.get(6)?;
        let accumulated_focus_seconds: i64 = row.get(7)?;
        let accumulated_break_seconds: i64 = row.get(8)?;
        let accumulated_idle_seconds: i64 = row.get(9)?;
        let last_state_change_str: String = row.get(10)?;
        let created_at_str: String = row.get(11)?;
        let updated_at_str: String = row.get(12)?;

        let id = SessionId::parse_str(&id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let commitment_id = CommitmentId::parse_str(&commitment_id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?;

        let state = match state_str.to_lowercase().as_str() {
            "ready" => SessionState::Ready,
            "active" => SessionState::Active,
            "paused" => SessionState::Paused,
            "break" => SessionState::Break,
            "completed" => SessionState::Completed,
            "interrupted" => SessionState::Interrupted,
            other => {
                return Err(rusqlite::Error::FromSqlConversionFailure(
                    2,
                    rusqlite::types::Type::Text,
                    format!("Unknown session state: {other}").into(),
                ))
            }
        };

        let consent: ConsentSnapshot = serde_json::from_str(&consent_json).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let end_reason: Option<SessionEndReason> = end_reason_json
            .map(|json| serde_json::from_str(&json))
            .transpose()
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

        let started_at = started_at_str
            .map(|s| DateTime::parse_from_rfc3339(&s).map(|dt| dt.with_timezone(&Utc)))
            .transpose()
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    4,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
        let ended_at = ended_at_str
            .map(|s| DateTime::parse_from_rfc3339(&s).map(|dt| dt.with_timezone(&Utc)))
            .transpose()
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
        let last_state_change = DateTime::parse_from_rfc3339(&last_state_change_str)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    10,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?
            .with_timezone(&Utc);
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    11,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?
            .with_timezone(&Utc);
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    12,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?
            .with_timezone(&Utc);

        Ok(FocusSession {
            id,
            commitment_id,
            state,
            consent,
            started_at,
            ended_at,
            end_reason,
            accumulated_focus_seconds: accumulated_focus_seconds.max(0) as u64,
            accumulated_break_seconds: accumulated_break_seconds.max(0) as u64,
            accumulated_idle_seconds: accumulated_idle_seconds.max(0) as u64,
            last_state_change,
            created_at,
            updated_at,
        })
    }
}

impl SessionRepository for SqliteSessionRepository {
    fn create_commitment(&self, commitment: &Commitment) -> Result<(), StorageError> {
        self.pool.with_conn(|conn| {
            conn.execute(
                "INSERT INTO commitments (id, goal_id, planned_start, duration_seconds, break_allowance_seconds, monitoring_profile_id, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    commitment.id.to_string(),
                    commitment.goal_id.to_string(),
                    commitment.planned_start.to_rfc3339(),
                    commitment.duration_seconds,
                    commitment.break_allowance_seconds,
                    commitment.monitoring_profile_id,
                    commitment.created_at.to_rfc3339(),
                ],
            )?;
            Ok(())
        })
    }

    fn get_commitment(&self, id: CommitmentId) -> Result<Option<Commitment>, StorageError> {
        self.pool.with_conn(|conn| {
            let res = conn
                .query_row(
                    "SELECT id, goal_id, planned_start, duration_seconds, break_allowance_seconds, monitoring_profile_id, created_at
                     FROM commitments WHERE id = ?1",
                    params![id.to_string()],
                    Self::row_to_commitment,
                )
                .optional()?;
            Ok(res)
        })
    }

    fn list_commitments_for_goal(&self, goal_id: GoalId) -> Result<Vec<Commitment>, StorageError> {
        self.pool.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, goal_id, planned_start, duration_seconds, break_allowance_seconds, monitoring_profile_id, created_at
                 FROM commitments WHERE goal_id = ?1 ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map(params![goal_id.to_string()], Self::row_to_commitment)?;
            let mut result = Vec::new();
            for r in rows {
                result.push(r?);
            }
            Ok(result)
        })
    }

    fn create_session(&self, session: &FocusSession) -> Result<(), StorageError> {
        let consent_json = serde_json::to_string(&session.consent)?;
        let end_reason_json = session
            .end_reason
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;

        self.pool.with_conn(|conn| {
            conn.execute(
                "INSERT INTO focus_sessions (
                    id, commitment_id, state, consent_snapshot, started_at, ended_at, end_reason,
                    accumulated_focus_seconds, accumulated_break_seconds, accumulated_idle_seconds,
                    last_state_change, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    session.id.to_string(),
                    session.commitment_id.to_string(),
                    session.state.to_string(),
                    consent_json,
                    session.started_at.map(|dt| dt.to_rfc3339()),
                    session.ended_at.map(|dt| dt.to_rfc3339()),
                    end_reason_json,
                    session.accumulated_focus_seconds as i64,
                    session.accumulated_break_seconds as i64,
                    session.accumulated_idle_seconds as i64,
                    session.last_state_change.to_rfc3339(),
                    session.created_at.to_rfc3339(),
                    session.updated_at.to_rfc3339(),
                ],
            )?;
            Ok(())
        })
    }

    fn get_session(&self, id: SessionId) -> Result<Option<FocusSession>, StorageError> {
        self.pool.with_conn(|conn| {
            let res = conn
                .query_row(
                    "SELECT id, commitment_id, state, consent_snapshot, started_at, ended_at, end_reason,
                            accumulated_focus_seconds, accumulated_break_seconds, accumulated_idle_seconds,
                            last_state_change, created_at, updated_at
                     FROM focus_sessions WHERE id = ?1",
                    params![id.to_string()],
                    Self::row_to_session,
                )
                .optional()?;
            Ok(res)
        })
    }

    fn get_active_session(&self) -> Result<Option<FocusSession>, StorageError> {
        self.pool.with_conn(|conn| {
            let res = conn
                .query_row(
                    "SELECT id, commitment_id, state, consent_snapshot, started_at, ended_at, end_reason,
                            accumulated_focus_seconds, accumulated_break_seconds, accumulated_idle_seconds,
                            last_state_change, created_at, updated_at
                     FROM focus_sessions
                     WHERE state IN ('Active', 'Paused', 'Break', 'active', 'paused', 'break')
                     ORDER BY started_at DESC LIMIT 1",
                    [],
                    Self::row_to_session,
                )
                .optional()?;
            Ok(res)
        })
    }

    fn list_sessions(&self, filter: SessionFilter) -> Result<Vec<FocusSession>, StorageError> {
        self.pool.with_conn(|conn| {
            let mut sql = String::from(
                "SELECT id, commitment_id, state, consent_snapshot, started_at, ended_at, end_reason,
                        accumulated_focus_seconds, accumulated_break_seconds, accumulated_idle_seconds,
                        last_state_change, created_at, updated_at
                 FROM focus_sessions WHERE 1=1",
            );
            let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

            if let Some(commitment_id) = filter.commitment_id {
                sql.push_str(" AND commitment_id = ?");
                params_vec.push(Box::new(commitment_id.to_string()));
            }
            if let Some(state) = filter.state {
                sql.push_str(" AND (state = ? OR state = ?)");
                params_vec.push(Box::new(state.to_string()));
                params_vec.push(Box::new(state.to_string().to_lowercase()));
            }
            sql.push_str(" ORDER BY created_at DESC");

            if let Some(limit) = filter.limit {
                sql.push_str(" LIMIT ?");
                params_vec.push(Box::new(limit as i64));
            }

            let mut stmt = conn.prepare(&sql)?;
            let rusqlite_params = rusqlite::params_from_iter(params_vec.iter().map(|b| &**b));
            let rows = stmt.query_map(rusqlite_params, Self::row_to_session)?;
            let mut result = Vec::new();
            for r in rows {
                result.push(r?);
            }
            Ok(result)
        })
    }

    fn update_session(&self, session: &FocusSession) -> Result<(), StorageError> {
        let consent_json = serde_json::to_string(&session.consent)?;
        let end_reason_json = session
            .end_reason
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;

        self.pool.with_conn(|conn| {
            let rows = conn.execute(
                "UPDATE focus_sessions
                 SET state = ?2, consent_snapshot = ?3, started_at = ?4, ended_at = ?5, end_reason = ?6,
                     accumulated_focus_seconds = ?7, accumulated_break_seconds = ?8, accumulated_idle_seconds = ?9,
                     last_state_change = ?10, updated_at = ?11
                 WHERE id = ?1",
                params![
                    session.id.to_string(),
                    session.state.to_string(),
                    consent_json,
                    session.started_at.map(|dt| dt.to_rfc3339()),
                    session.ended_at.map(|dt| dt.to_rfc3339()),
                    end_reason_json,
                    session.accumulated_focus_seconds as i64,
                    session.accumulated_break_seconds as i64,
                    session.accumulated_idle_seconds as i64,
                    session.last_state_change.to_rfc3339(),
                    session.updated_at.to_rfc3339(),
                ],
            )?;
            if rows == 0 {
                return Err(StorageError::NotFound {
                    entity: "FocusSession",
                    id: session.id.to_string(),
                });
            }
            Ok(())
        })
    }

    fn update_session_state(
        &self,
        id: SessionId,
        state: SessionState,
        end_reason: Option<&SessionEndReason>,
        ended_at: Option<DateTime<Utc>>,
        last_state_change: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        let end_reason_json = end_reason.map(serde_json::to_string).transpose()?;
        self.pool.with_conn(|conn| {
            let rows = conn.execute(
                "UPDATE focus_sessions
                 SET state = ?2, end_reason = ?3, ended_at = ?4, last_state_change = ?5, updated_at = ?5
                 WHERE id = ?1",
                params![
                    id.to_string(),
                    state.to_string(),
                    end_reason_json,
                    ended_at.map(|dt| dt.to_rfc3339()),
                    last_state_change.to_rfc3339(),
                ],
            )?;
            if rows == 0 {
                return Err(StorageError::NotFound {
                    entity: "FocusSession",
                    id: id.to_string(),
                });
            }
            Ok(())
        })
    }

    fn update_session_durations(
        &self,
        id: SessionId,
        focus_seconds: u64,
        break_seconds: u64,
        idle_seconds: u64,
        now: DateTime<Utc>,
    ) -> Result<(), StorageError> {
        self.pool.with_conn(|conn| {
            let rows = conn.execute(
                "UPDATE focus_sessions
                 SET accumulated_focus_seconds = ?2, accumulated_break_seconds = ?3, accumulated_idle_seconds = ?4, updated_at = ?5
                 WHERE id = ?1",
                params![
                    id.to_string(),
                    focus_seconds as i64,
                    break_seconds as i64,
                    idle_seconds as i64,
                    now.to_rfc3339(),
                ],
            )?;
            if rows == 0 {
                return Err(StorageError::NotFound {
                    entity: "FocusSession",
                    id: id.to_string(),
                });
            }
            Ok(())
        })
    }
}
