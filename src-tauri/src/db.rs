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
        let migration = include_str!("../migrations/001_schema_v12.sql");
        conn.execute_batch(migration)?;
        Ok(Self { path, writer: Arc::new(Mutex::new(conn)), key: Arc::new(key) })
    }

    fn open_keyed(path: &PathBuf, key: &str) -> AppResult<Connection> {
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
}

#[cfg(test)]
mod tests {
    #[test]
    fn migration_is_idempotent_across_repeated_app_launches() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let migration = include_str!("../migrations/001_schema_v12.sql");
        conn.execute_batch(migration).unwrap();
        // A real app re-runs the full schema on every launch against an
        // already-initialized database; every statement must tolerate that.
        conn.execute_batch(migration).unwrap();
        let table_count: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(table_count, 33);
    }
}
