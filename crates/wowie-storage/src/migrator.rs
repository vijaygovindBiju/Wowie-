use crate::error::StorageError;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

pub const V1_SCHEMA_SQL: &str = include_str!("../migrations/V1__initial_schema.sql");

#[derive(Debug, Clone)]
pub struct Migration {
    pub version: i64,
    pub name: &'static str,
    pub sql: &'static str,
}

pub const MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "V1__initial_schema",
    sql: V1_SCHEMA_SQL,
}];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationRecord {
    pub version: i64,
    pub name: String,
    pub applied_at: String,
    pub checksum: String,
}

pub trait Migrator: Send + Sync {
    fn initialize_and_migrate(&self, conn: &mut Connection) -> Result<u32, StorageError>;
    fn current_version(&self, conn: &Connection) -> Result<Option<i64>, StorageError>;
    fn migration_history(&self, conn: &Connection) -> Result<Vec<MigrationRecord>, StorageError>;
}

#[derive(Default, Debug, Clone, Copy)]
pub struct SqliteMigrator;

impl SqliteMigrator {
    pub fn new() -> Self {
        Self
    }

    pub fn compute_checksum(sql: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(sql.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

impl Migrator for SqliteMigrator {
    fn initialize_and_migrate(&self, conn: &mut Connection) -> Result<u32, StorageError> {
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = 5000;",
        )?;

        let tx = conn.transaction()?;

        // Ensure tracking table exists
        tx.execute(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                name TEXT NOT NULL,
                applied_at TEXT NOT NULL,
                checksum TEXT NOT NULL
            );",
            [],
        )?;

        // Read currently applied migrations
        let mut applied: HashMap<i64, String> = HashMap::new();
        {
            let mut stmt =
                tx.prepare("SELECT version, checksum FROM schema_migrations ORDER BY version ASC")?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let version: i64 = row.get(0)?;
                let checksum: String = row.get(1)?;
                applied.insert(version, checksum);
            }
        }

        let mut applied_count = 0u32;

        for migration in MIGRATIONS {
            let expected_checksum = Self::compute_checksum(migration.sql);

            if let Some(recorded_checksum) = applied.get(&migration.version) {
                if *recorded_checksum != expected_checksum {
                    return Err(StorageError::MigrationChecksumMismatch {
                        version: migration.version,
                        expected: expected_checksum,
                        found: recorded_checksum.clone(),
                    });
                }
            } else {
                tx.execute_batch(migration.sql)?;

                let applied_at = chrono::Utc::now().to_rfc3339();
                tx.execute(
                    "INSERT INTO schema_migrations (version, name, applied_at, checksum) VALUES (?1, ?2, ?3, ?4)",
                    params![migration.version, migration.name, applied_at, expected_checksum],
                )?;
                applied_count += 1;
            }
        }

        tx.commit()?;
        Ok(applied_count)
    }

    fn current_version(&self, conn: &Connection) -> Result<Option<i64>, StorageError> {
        let table_exists: bool = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations'",
            [],
            |r| r.get::<_, i64>(0).map(|c| c > 0),
        )?;

        if !table_exists {
            return Ok(None);
        }

        let max_ver: Option<i64> =
            conn.query_row("SELECT max(version) FROM schema_migrations", [], |r| {
                r.get(0)
            })?;

        Ok(max_ver)
    }

    fn migration_history(&self, conn: &Connection) -> Result<Vec<MigrationRecord>, StorageError> {
        let table_exists: bool = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations'",
            [],
            |r| r.get::<_, i64>(0).map(|c| c > 0),
        )?;

        if !table_exists {
            return Ok(Vec::new());
        }

        let mut stmt = conn.prepare(
            "SELECT version, name, applied_at, checksum FROM schema_migrations ORDER BY version ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(MigrationRecord {
                version: row.get(0)?,
                name: row.get(1)?,
                applied_at: row.get(2)?,
                checksum: row.get(3)?,
            })
        })?;

        let mut records = Vec::new();
        for r in rows {
            records.push(r?);
        }
        Ok(records)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migrator_schema_initialization() {
        let mut conn = Connection::open_in_memory().expect("Failed to open connection");
        let migrator = SqliteMigrator::new();

        assert_eq!(migrator.current_version(&conn).unwrap(), None);

        let applied = migrator.initialize_and_migrate(&mut conn).unwrap();
        assert_eq!(applied, 1);
        assert_eq!(migrator.current_version(&conn).unwrap(), Some(1));

        let history = migrator.migration_history(&conn).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].version, 1);
        assert_eq!(history[0].name, "V1__initial_schema");
        assert_eq!(
            history[0].checksum,
            SqliteMigrator::compute_checksum(V1_SCHEMA_SQL)
        );

        // Check total tables
        let total_tables: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(total_tables, 13);
    }

    #[test]
    fn test_migrator_idempotency() {
        let mut conn = Connection::open_in_memory().expect("Failed to open connection");
        let migrator = SqliteMigrator::new();

        let applied_first = migrator.initialize_and_migrate(&mut conn).unwrap();
        assert_eq!(applied_first, 1);

        // Re-run migration
        let applied_second = migrator.initialize_and_migrate(&mut conn).unwrap();
        assert_eq!(applied_second, 0);
        assert_eq!(migrator.current_version(&conn).unwrap(), Some(1));
    }

    #[test]
    fn test_migrator_checksum_tamper_detection() {
        let mut conn = Connection::open_in_memory().expect("Failed to open connection");
        let migrator = SqliteMigrator::new();
        migrator.initialize_and_migrate(&mut conn).unwrap();

        // Tamper with checksum in schema_migrations
        conn.execute(
            "UPDATE schema_migrations SET checksum = 'tampered-hash' WHERE version = 1",
            [],
        )
        .unwrap();

        let res = migrator.initialize_and_migrate(&mut conn);
        assert!(matches!(
            res,
            Err(StorageError::MigrationChecksumMismatch { version: 1, .. })
        ));
    }
}
