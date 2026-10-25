use crate::error::StorageError;
use crate::migrator::{Migrator, SqliteMigrator};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub path: PathBuf,
    pub busy_timeout: Duration,
    pub wal_mode: bool,
    pub foreign_keys: bool,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            path: PathBuf::from("wowie.db"),
            busy_timeout: Duration::from_millis(5000),
            wal_mode: true,
            foreign_keys: true,
        }
    }
}

/// Thread-safe SQLite connection wrapper with configured PRAGMAs and migration support.
#[derive(Clone, Debug)]
pub struct DbPool {
    conn: Arc<Mutex<Connection>>,
}

impl DbPool {
    /// Opens an in-memory SQLite database instance configured with foreign keys and busy timeout,
    /// and initializes schema migrations.
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let mut conn = Connection::open_in_memory()?;
        Self::apply_pragmas(&conn, true, 5000)?;

        let migrator = SqliteMigrator::new();
        migrator.initialize_and_migrate(&mut conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Opens or creates an on-disk SQLite database file with WAL mode, foreign keys,
    /// busy timeout, and initializes schema migrations.
    pub fn open_file<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let mut conn = Connection::open(path)?;
        Self::apply_pragmas(&conn, false, 5000)?;

        let migrator = SqliteMigrator::new();
        migrator.initialize_and_migrate(&mut conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Creates a pool with custom configuration.
    pub fn with_config(config: &ConnectionConfig) -> Result<Self, StorageError> {
        let mut conn = Connection::open(&config.path)?;
        let timeout_ms = config.busy_timeout.as_millis();
        conn.execute_batch(&format!("PRAGMA busy_timeout = {timeout_ms};"))?;
        if config.foreign_keys {
            conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        }
        if config.wal_mode {
            conn.execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;",
            )?;
        }

        let migrator = SqliteMigrator::new();
        migrator.initialize_and_migrate(&mut conn)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    /// Wraps an existing connection without running migrations automatically.
    pub fn from_connection(conn: Connection) -> Result<Self, StorageError> {
        Self::apply_pragmas(&conn, false, 5000)?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn apply_pragmas(
        conn: &Connection,
        in_memory: bool,
        timeout_ms: u128,
    ) -> Result<(), StorageError> {
        conn.execute_batch(&format!(
            "PRAGMA foreign_keys = ON;
             PRAGMA busy_timeout = {timeout_ms};"
        ))?;
        if !in_memory {
            conn.execute_batch(
                "PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;",
            )?;
        }
        Ok(())
    }

    /// Executes a closure against the locked connection.
    pub fn with_conn<F, R>(&self, f: F) -> Result<R, StorageError>
    where
        F: FnOnce(&mut Connection) -> Result<R, StorageError>,
    {
        let mut lock = self
            .conn
            .lock()
            .map_err(|e| StorageError::LockError(e.to_string()))?;
        f(&mut lock)
    }

    /// Executes a closure within an explicit SQLite transaction, rolling back automatically on error.
    pub fn transaction<F, R>(&self, f: F) -> Result<R, StorageError>
    where
        F: FnOnce(&rusqlite::Transaction) -> Result<R, StorageError>,
    {
        let mut lock = self
            .conn
            .lock()
            .map_err(|e| StorageError::LockError(e.to_string()))?;
        let tx = lock.transaction()?;
        let result = f(&tx)?;
        tx.commit()?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_pool_open_in_memory_and_pragmas() {
        let pool = DbPool::open_in_memory().expect("Failed to open pool");

        pool.with_conn(|conn| {
            let fk_enabled: i64 = conn.query_row("PRAGMA foreign_keys", [], |r| r.get(0))?;
            assert_eq!(fk_enabled, 1);

            let busy_timeout: i64 = conn.query_row("PRAGMA busy_timeout", [], |r| r.get(0))?;
            assert_eq!(busy_timeout, 5000);

            // Migrations should be executed automatically
            let count: i64 =
                conn.query_row("SELECT count(*) FROM schema_migrations", [], |r| r.get(0))?;
            assert_eq!(count, 1);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_db_pool_transaction_commit_and_rollback() {
        let pool = DbPool::open_in_memory().unwrap();

        // Successful transaction
        pool.transaction(|tx| {
            tx.execute(
                "INSERT INTO settings (key, value, version, updated_at) VALUES ('theme', 'dark', 1, '2026-01-01T00:00:00Z')",
                [],
            )?;
            Ok(())
        }).unwrap();

        pool.with_conn(|conn| {
            let val: String =
                conn.query_row("SELECT value FROM settings WHERE key = 'theme'", [], |r| {
                    r.get(0)
                })?;
            assert_eq!(val, "dark");
            Ok(())
        })
        .unwrap();

        // Failed transaction should roll back
        let err: Result<(), StorageError> = pool.transaction(|tx| {
            tx.execute(
                "INSERT INTO settings (key, value, version, updated_at) VALUES ('language', 'en', 1, '2026-01-01T00:00:00Z')",
                [],
            )?;
            Err(StorageError::DataIntegrity("Intentionally failed".into()))
        });
        assert!(err.is_err());

        // Language setting must not exist
        pool.with_conn(|conn| {
            let count: i64 = conn.query_row(
                "SELECT count(*) FROM settings WHERE key = 'language'",
                [],
                |r| r.get(0),
            )?;
            assert_eq!(count, 0);
            Ok(())
        })
        .unwrap();
    }
}
