use super::{GoalFilter, GoalRepository};
use crate::connection::DbPool;
use crate::error::StorageError;
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension, Row};
use wowie_core::id::{AmbitionId, GoalId};
use wowie_core::models::{Ambition, Goal, GoalStatus};

#[derive(Clone, Debug)]
pub struct SqliteGoalRepository {
    pool: DbPool,
}

impl SqliteGoalRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    fn row_to_ambition(row: &Row) -> Result<Ambition, rusqlite::Error> {
        let id_str: String = row.get(0)?;
        let title: String = row.get(1)?;
        let description: Option<String> = row.get(2)?;
        let created_at_str: String = row.get(3)?;
        let updated_at_str: String = row.get(4)?;
        let archived_at_str: Option<String> = row.get(5)?;

        let id = AmbitionId::parse_str(&id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    3,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?
            .with_timezone(&Utc);
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    4,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?
            .with_timezone(&Utc);
        let archived_at = archived_at_str
            .map(|s| DateTime::parse_from_rfc3339(&s).map(|dt| dt.with_timezone(&Utc)))
            .transpose()
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

        Ok(Ambition {
            id,
            title,
            description,
            created_at,
            updated_at,
            archived_at,
        })
    }

    fn row_to_goal(row: &Row) -> Result<Goal, rusqlite::Error> {
        let id_str: String = row.get(0)?;
        let ambition_id_str: String = row.get(1)?;
        let title: String = row.get(2)?;
        let next_action: Option<String> = row.get(3)?;
        let status_str: String = row.get(4)?;
        let target_date_str: Option<String> = row.get(5)?;
        let created_at_str: String = row.get(6)?;
        let updated_at_str: String = row.get(7)?;
        let completed_at_str: Option<String> = row.get(8)?;
        let archived_at_str: Option<String> = row.get(9)?;

        let id = GoalId::parse_str(&id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let ambition_id = AmbitionId::parse_str(&ambition_id_str).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
        })?;

        let status = match status_str.as_str() {
            "active" => GoalStatus::Active,
            "paused" => GoalStatus::Paused,
            "completed" => GoalStatus::Completed,
            "archived" => GoalStatus::Archived,
            other => {
                return Err(rusqlite::Error::FromSqlConversionFailure(
                    4,
                    rusqlite::types::Type::Text,
                    format!("Unknown goal status: {other}").into(),
                ))
            }
        };

        let target_date = target_date_str
            .map(|s| DateTime::parse_from_rfc3339(&s).map(|dt| dt.with_timezone(&Utc)))
            .transpose()
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
        let created_at = DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    6,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?
            .with_timezone(&Utc);
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    7,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?
            .with_timezone(&Utc);
        let completed_at = completed_at_str
            .map(|s| DateTime::parse_from_rfc3339(&s).map(|dt| dt.with_timezone(&Utc)))
            .transpose()
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    8,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
        let archived_at = archived_at_str
            .map(|s| DateTime::parse_from_rfc3339(&s).map(|dt| dt.with_timezone(&Utc)))
            .transpose()
            .map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    9,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;

        Ok(Goal {
            id,
            ambition_id,
            title,
            next_action,
            status,
            target_date,
            created_at,
            updated_at,
            completed_at,
            archived_at,
        })
    }
}

impl GoalRepository for SqliteGoalRepository {
    fn create_ambition(&self, ambition: &Ambition) -> Result<(), StorageError> {
        self.pool.with_conn(|conn| {
            conn.execute(
                "INSERT INTO ambitions (id, title, description, created_at, updated_at, archived_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    ambition.id.to_string(),
                    ambition.title,
                    ambition.description,
                    ambition.created_at.to_rfc3339(),
                    ambition.updated_at.to_rfc3339(),
                    ambition.archived_at.map(|dt| dt.to_rfc3339()),
                ],
            )?;
            Ok(())
        })
    }

    fn get_ambition(&self, id: AmbitionId) -> Result<Option<Ambition>, StorageError> {
        self.pool.with_conn(|conn| {
            let res = conn
                .query_row(
                    "SELECT id, title, description, created_at, updated_at, archived_at
                     FROM ambitions WHERE id = ?1",
                    params![id.to_string()],
                    Self::row_to_ambition,
                )
                .optional()?;
            Ok(res)
        })
    }

    fn list_ambitions(&self, include_archived: bool) -> Result<Vec<Ambition>, StorageError> {
        self.pool.with_conn(|conn| {
            let sql = if include_archived {
                "SELECT id, title, description, created_at, updated_at, archived_at
                 FROM ambitions ORDER BY created_at DESC"
            } else {
                "SELECT id, title, description, created_at, updated_at, archived_at
                 FROM ambitions WHERE archived_at IS NULL ORDER BY created_at DESC"
            };
            let mut stmt = conn.prepare(sql)?;
            let rows = stmt.query_map([], Self::row_to_ambition)?;
            let mut result = Vec::new();
            for r in rows {
                result.push(r?);
            }
            Ok(result)
        })
    }

    fn update_ambition(&self, ambition: &Ambition) -> Result<(), StorageError> {
        self.pool.with_conn(|conn| {
            let rows = conn.execute(
                "UPDATE ambitions SET title = ?2, description = ?3, updated_at = ?4, archived_at = ?5
                 WHERE id = ?1",
                params![
                    ambition.id.to_string(),
                    ambition.title,
                    ambition.description,
                    ambition.updated_at.to_rfc3339(),
                    ambition.archived_at.map(|dt| dt.to_rfc3339()),
                ],
            )?;
            if rows == 0 {
                return Err(StorageError::NotFound {
                    entity: "Ambition",
                    id: ambition.id.to_string(),
                });
            }
            Ok(())
        })
    }

    fn delete_ambition(&self, id: AmbitionId) -> Result<(), StorageError> {
        self.pool.with_conn(|conn| {
            let rows = conn.execute(
                "DELETE FROM ambitions WHERE id = ?1",
                params![id.to_string()],
            )?;
            if rows == 0 {
                return Err(StorageError::NotFound {
                    entity: "Ambition",
                    id: id.to_string(),
                });
            }
            Ok(())
        })
    }

    fn create_goal(&self, goal: &Goal) -> Result<(), StorageError> {
        self.pool.with_conn(|conn| {
            conn.execute(
                "INSERT INTO goals (id, ambition_id, title, next_action, status, target_date, created_at, updated_at, completed_at, archived_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    goal.id.to_string(),
                    goal.ambition_id.to_string(),
                    goal.title,
                    goal.next_action,
                    goal.status.to_string(),
                    goal.target_date.map(|dt| dt.to_rfc3339()),
                    goal.created_at.to_rfc3339(),
                    goal.updated_at.to_rfc3339(),
                    goal.completed_at.map(|dt| dt.to_rfc3339()),
                    goal.archived_at.map(|dt| dt.to_rfc3339()),
                ],
            )?;
            Ok(())
        })
    }

    fn get_goal(&self, id: GoalId) -> Result<Option<Goal>, StorageError> {
        self.pool.with_conn(|conn| {
            let res = conn
                .query_row(
                    "SELECT id, ambition_id, title, next_action, status, target_date, created_at, updated_at, completed_at, archived_at
                     FROM goals WHERE id = ?1",
                    params![id.to_string()],
                    Self::row_to_goal,
                )
                .optional()?;
            Ok(res)
        })
    }

    fn list_goals(&self, filter: GoalFilter) -> Result<Vec<Goal>, StorageError> {
        self.pool.with_conn(|conn| {
            let mut sql = String::from(
                "SELECT id, ambition_id, title, next_action, status, target_date, created_at, updated_at, completed_at, archived_at
                 FROM goals WHERE 1=1",
            );
            let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

            if let Some(ambition_id) = filter.ambition_id {
                sql.push_str(" AND ambition_id = ?");
                params_vec.push(Box::new(ambition_id.to_string()));
            }
            if let Some(status) = filter.status {
                sql.push_str(" AND status = ?");
                params_vec.push(Box::new(status.to_string()));
            }
            if !filter.include_archived {
                sql.push_str(" AND archived_at IS NULL");
            }
            sql.push_str(" ORDER BY created_at DESC");

            let mut stmt = conn.prepare(&sql)?;
            let rusqlite_params = rusqlite::params_from_iter(params_vec.iter().map(|b| &**b));
            let rows = stmt.query_map(rusqlite_params, Self::row_to_goal)?;
            let mut result = Vec::new();
            for r in rows {
                result.push(r?);
            }
            Ok(result)
        })
    }

    fn update_goal(&self, goal: &Goal) -> Result<(), StorageError> {
        self.pool.with_conn(|conn| {
            let rows = conn.execute(
                "UPDATE goals
                 SET title = ?2, next_action = ?3, status = ?4, target_date = ?5, updated_at = ?6, completed_at = ?7, archived_at = ?8
                 WHERE id = ?1",
                params![
                    goal.id.to_string(),
                    goal.title,
                    goal.next_action,
                    goal.status.to_string(),
                    goal.target_date.map(|dt| dt.to_rfc3339()),
                    goal.updated_at.to_rfc3339(),
                    goal.completed_at.map(|dt| dt.to_rfc3339()),
                    goal.archived_at.map(|dt| dt.to_rfc3339()),
                ],
            )?;
            if rows == 0 {
                return Err(StorageError::NotFound {
                    entity: "Goal",
                    id: goal.id.to_string(),
                });
            }
            Ok(())
        })
    }

    fn archive_goal(&self, id: GoalId, now: DateTime<Utc>) -> Result<(), StorageError> {
        self.pool.with_conn(|conn| {
            let rows = conn.execute(
                "UPDATE goals SET status = 'archived', archived_at = ?2, updated_at = ?2 WHERE id = ?1",
                params![id.to_string(), now.to_rfc3339()],
            )?;
            if rows == 0 {
                return Err(StorageError::NotFound {
                    entity: "Goal",
                    id: id.to_string(),
                });
            }
            Ok(())
        })
    }

    fn unarchive_goal(&self, id: GoalId, now: DateTime<Utc>) -> Result<(), StorageError> {
        self.pool.with_conn(|conn| {
            let rows = conn.execute(
                "UPDATE goals SET status = 'active', archived_at = NULL, updated_at = ?2 WHERE id = ?1",
                params![id.to_string(), now.to_rfc3339()],
            )?;
            if rows == 0 {
                return Err(StorageError::NotFound {
                    entity: "Goal",
                    id: id.to_string(),
                });
            }
            Ok(())
        })
    }

    fn delete_goal(&self, id: GoalId) -> Result<(), StorageError> {
        self.pool.with_conn(|conn| {
            let rows = conn.execute("DELETE FROM goals WHERE id = ?1", params![id.to_string()])?;
            if rows == 0 {
                return Err(StorageError::NotFound {
                    entity: "Goal",
                    id: id.to_string(),
                });
            }
            Ok(())
        })
    }
}
