use crate::error::{AppError, AppResult};
use crate::security::load_or_create_db_key;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct DbState {
    pub path: PathBuf,
    writer: Arc<Mutex<Connection>>,
    key: Arc<String>,
}

impl DbState {
    pub fn open(path: PathBuf) -> AppResult<Self> {
        let exists = path.exists();
        if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
        let key = load_or_create_db_key(exists)?;
        let conn = Self::open_keyed(&path, &key)?;
        conn.execute_batch(include_str!("../migrations/001_schema_v12.sql"))?;
        conn.execute_batch(include_str!("../migrations/002_legal_rules_infrastructure_v13.sql"))?;
        Ok(Self { path, writer: Arc::new(Mutex::new(conn)), key: Arc::new(key) })
    }

    pub(crate) fn open_keyed(path: &PathBuf, key: &str) -> AppResult<Connection> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "key", format!("x'{key}'"))?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.pragma_update(None, "secure_delete", "ON")?;
        let _: String = conn.query_row("PRAGMA cipher_version", [], |row| row.get(0))
            .map_err(|_| AppError::RecoveryRequired)?;
        Ok(conn)
    }

    pub fn write<T>(&self, f: impl FnOnce(&mut Connection) -> AppResult<T>) -> AppResult<T> {
        let mut guard = self.writer.lock()
            .map_err(|_| AppError::Validation("database writer mutex poisoned".into()))?;
        f(&mut guard)
    }

    pub fn read<T>(&self, f: impl FnOnce(&Connection) -> AppResult<T>) -> AppResult<T> {
        let conn = Self::open_keyed(&self.path, &self.key)?;
        f(&conn)
    }

    /// Test-only: the raw DB encryption key, so a test can reopen a raw connection
    /// to the same file directly (bypassing the OS keyring lookup) to verify data
    /// persistence independently of whether this environment's keyring backend
    /// itself persists across process/entry instances.
    #[cfg(test)]
    pub(crate) fn test_key(&self) -> &str { &self.key }
}

#[cfg(test)]
mod tests {
    const MIGRATION_001: &str = include_str!("../migrations/001_schema_v12.sql");
    const MIGRATION_002: &str = include_str!("../migrations/002_legal_rules_infrastructure_v13.sql");

    #[test]
    fn migration_is_idempotent_across_repeated_app_launches() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATION_001).unwrap();
        conn.execute_batch(MIGRATION_002).unwrap();
        // A real app re-runs the full schema on every launch against an
        // already-initialized database; every statement must tolerate that.
        conn.execute_batch(MIGRATION_001).unwrap();
        conn.execute_batch(MIGRATION_002).unwrap();
        let table_count: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(table_count, 38);
        let user_version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(user_version, 13);
    }

    #[test]
    fn a_v12_only_database_upgrades_cleanly_to_v13() {
        // simulates a real pre-existing install: only 001 has ever run, then the app
        // is upgraded to a build that also ships 002 - DbState::open runs both in
        // order on every launch, so this must succeed against that starting state too.
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATION_001).unwrap();
        let matter_id = "m1";
        conn.execute(
            "INSERT INTO matters(id,title,matter_type,status,workflow_stage,created_at,updated_at)
             VALUES(?1,'existing matter','personal_injury','active','intake','x','x')",
            [matter_id],
        ).unwrap();
        conn.execute_batch(MIGRATION_002).unwrap();
        let ruleset_table_exists: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='legal_rulesets'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(ruleset_table_exists, 1);
        let matter_survived: String = conn.query_row(
            "SELECT title FROM matters WHERE id=?1", [matter_id], |r| r.get(0),
        ).unwrap();
        assert_eq!(matter_survived, "existing matter");
    }
}
