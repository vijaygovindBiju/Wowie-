//! # wowie-storage
//!
//! SQLite persistence, connection management, schema migrations,
//! and repository implementations for the Wowie companion.

pub mod connection;
pub mod error;
pub mod migrator;
pub mod repos;

pub use connection::{ConnectionConfig, DbPool};
pub use error::StorageError;
pub use migrator::{Migration, MigrationRecord, Migrator, SqliteMigrator, V1_SCHEMA_SQL};
pub use repos::{
    activity_log::SqliteActivityLogRepository, goal::SqliteGoalRepository,
    review::SqliteReviewRepository, session::SqliteSessionRepository, ActivityLogRepository,
    GoalFilter, GoalRepository, ReviewRepository, SessionFilter, SessionRepository,
};
