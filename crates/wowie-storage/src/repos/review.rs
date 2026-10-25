use super::ReviewRepository;
use crate::connection::DbPool;
use crate::error::StorageError;
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension, Row};
use wowie_core::id::{ReviewId, SessionId};
use wowie_core::models::{SessionOutcome, SessionReview};

#[derive(Clone, Debug)]
pub struct SqliteReviewRepository {
    pool: DbPool,
}

impl SqliteReviewRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    fn row_to_review(row: &Row) -> Result<SessionReview, rusqlite::Error> {
        let id_str: String = row.get(0)?;
        let session_id_str: String = row.get(1)?;
        let actual_focus_seconds: i64 = row.get(2)?;
        let actual_break_seconds: i64 = row.get(3)?;
        let actual_idle_seconds: i64 = row.get(4)?;
        let outcome_str: String = row.get(5)?;
        let user_notes: Option<String> = row.get(6)?;
        let next_action_revision: Option<String> = row.get(7)?;
        let created_at_str: String = row.get(8)?;

        let id = ReviewId::parse_str(&id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let session_id = SessionId::parse_str(&session_id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?;

        let outcome = match outcome_str.as_str() {
            "completed_success" | "completed_as_planned" => SessionOutcome::CompletedSuccess,
            "completed_partial" | "partially_completed" => SessionOutcome::CompletedPartial,
            "cancelled" => SessionOutcome::Cancelled,
            "abandoned" | "interrupted" => SessionOutcome::Abandoned,
            other => {
                return Err(rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    format!("Unknown session outcome: {other}").into(),
                ))
            }
        };

        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    8,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?
            .with_timezone(&Utc);

        Ok(SessionReview {
            id,
            session_id,
            actual_focus_seconds: actual_focus_seconds.max(0) as u64,
            actual_break_seconds: actual_break_seconds.max(0) as u64,
            actual_idle_seconds: actual_idle_seconds.max(0) as u64,
            outcome,
            user_notes,
            next_action_revision,
            created_at,
        })
    }
}

impl ReviewRepository for SqliteReviewRepository {
    fn save_review(&self, review: &SessionReview) -> Result<(), StorageError> {
        let total_duration =
            review.actual_focus_seconds + review.actual_break_seconds + review.actual_idle_seconds;

        self.pool.with_conn(|conn| {
            conn.execute(
                "INSERT INTO session_reviews (
                    id, session_id, total_duration_seconds, active_focus_seconds, break_seconds, idle_seconds,
                    interventions_count, corrections_count, outcome, user_note, next_plan, summary_payload, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0, ?7, ?8, ?9, '{}', ?10)",
                params![
                    review.id.to_string(),
                    review.session_id.to_string(),
                    total_duration as i64,
                    review.actual_focus_seconds as i64,
                    review.actual_break_seconds as i64,
                    review.actual_idle_seconds as i64,
                    review.outcome.to_string(),
                    review.user_notes,
                    review.next_action_revision,
                    review.created_at.to_rfc3339(),
                ],
            )?;
            Ok(())
        })
    }

    fn get_review(&self, id: ReviewId) -> Result<Option<SessionReview>, StorageError> {
        self.pool.with_conn(|conn| {
            let res = conn
                .query_row(
                    "SELECT id, session_id, active_focus_seconds, break_seconds, idle_seconds,
                            outcome, user_note, next_plan, created_at
                     FROM session_reviews WHERE id = ?1",
                    params![id.to_string()],
                    Self::row_to_review,
                )
                .optional()?;
            Ok(res)
        })
    }

    fn get_review_by_session(
        &self,
        session_id: SessionId,
    ) -> Result<Option<SessionReview>, StorageError> {
        self.pool.with_conn(|conn| {
            let res = conn
                .query_row(
                    "SELECT id, session_id, active_focus_seconds, break_seconds, idle_seconds,
                            outcome, user_note, next_plan, created_at
                     FROM session_reviews WHERE session_id = ?1",
                    params![session_id.to_string()],
                    Self::row_to_review,
                )
                .optional()?;
            Ok(res)
        })
    }

    fn list_reviews(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<SessionReview>, StorageError> {
        self.pool.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, session_id, active_focus_seconds, break_seconds, idle_seconds,
                        outcome, user_note, next_plan, created_at
                 FROM session_reviews ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
            )?;
            let rows = stmt.query_map(params![limit as i64, offset as i64], Self::row_to_review)?;
            let mut result = Vec::new();
            for r in rows {
                result.push(r?);
            }
            Ok(result)
        })
    }

    fn delete_review(&self, id: ReviewId) -> Result<(), StorageError> {
        self.pool.with_conn(|conn| {
            let rows = conn.execute(
                "DELETE FROM session_reviews WHERE id = ?1",
                params![id.to_string()],
            )?;
            if rows == 0 {
                return Err(StorageError::NotFound {
                    entity: "SessionReview",
                    id: id.to_string(),
                });
            }
            Ok(())
        })
    }
}
