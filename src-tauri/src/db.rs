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
        conn.execute_batch(include_str!("../migrations/003_matter_profile_v14.sql"))?;
        conn.execute_batch(include_str!("../migrations/004_matter_workstreams_v15.sql"))?;
        conn.execute_batch(include_str!("../migrations/005_matter_requirements_v16.sql"))?;
        conn.execute_batch(include_str!("../migrations/006_matter_ledgers_v17.sql"))?;
        conn.execute_batch(include_str!("../migrations/007_retrieval_context_v18.sql"))?;
        conn.execute_batch(include_str!("../migrations/008_negotiation_insurance_v19.sql"))?;
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
    const MIGRATION_003: &str = include_str!("../migrations/003_matter_profile_v14.sql");
    const MIGRATION_004: &str = include_str!("../migrations/004_matter_workstreams_v15.sql");
    const MIGRATION_005: &str = include_str!("../migrations/005_matter_requirements_v16.sql");
    const MIGRATION_006: &str = include_str!("../migrations/006_matter_ledgers_v17.sql");
    const MIGRATION_007: &str = include_str!("../migrations/007_retrieval_context_v18.sql");
    const MIGRATION_008: &str = include_str!("../migrations/008_negotiation_insurance_v19.sql");

    /// Phase B, milestone B5a step 0: FTS5 is not a Cargo feature on rusqlite - the
    /// bundled SQLCipher build this project already links (`bundled-sqlcipher-
    /// vendored-openssl`) either has SQLITE_ENABLE_FTS5 compiled in or it doesn't,
    /// and the only way to know is to ask the actual library at runtime. This test
    /// runs on every `cargo test --locked`, including the real Windows CI runner -
    /// not just once locally - so a toolchain regression here is caught immediately,
    /// before retrieval.rs's migration/module are ever written on top of it.
    #[test]
    fn fts5_is_available_in_this_sqlite_build() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let enabled: i64 = conn.query_row(
            "SELECT sqlite_compileoption_used('ENABLE_FTS5')", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(enabled, 1, "this SQLite build must have FTS5 compiled in for B5a's retrieval pipeline to work");
        conn.execute_batch(
            "CREATE VIRTUAL TABLE t USING fts5(x);
             INSERT INTO t(x) VALUES('hello world');
             SELECT * FROM t WHERE t MATCH 'hello';"
        ).unwrap();
    }

    #[test]
    fn migration_is_idempotent_across_repeated_app_launches() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATION_001).unwrap();
        conn.execute_batch(MIGRATION_002).unwrap();
        conn.execute_batch(MIGRATION_003).unwrap();
        conn.execute_batch(MIGRATION_004).unwrap();
        conn.execute_batch(MIGRATION_005).unwrap();
        conn.execute_batch(MIGRATION_006).unwrap();
        conn.execute_batch(MIGRATION_007).unwrap();
        conn.execute_batch(MIGRATION_008).unwrap();
        // A real app re-runs the full schema on every launch against an
        // already-initialized database; every statement must tolerate that.
        conn.execute_batch(MIGRATION_001).unwrap();
        conn.execute_batch(MIGRATION_002).unwrap();
        conn.execute_batch(MIGRATION_003).unwrap();
        conn.execute_batch(MIGRATION_004).unwrap();
        conn.execute_batch(MIGRATION_005).unwrap();
        conn.execute_batch(MIGRATION_006).unwrap();
        conn.execute_batch(MIGRATION_007).unwrap();
        conn.execute_batch(MIGRATION_008).unwrap();
        // Counted excluding document_pages_fts and its own FTS5-internal shadow
        // tables (_data/_idx/_docsize/_config/_content) - their exact number is an
        // FTS5 implementation detail, not something to hardcode a guess for here;
        // see fts5_migration_creates_index_backfills_and_stays_in_sync below for
        // the FTS5-specific assertions instead.
        let table_count: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE 'document_pages_fts%'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(table_count, 57, "the 57 real application tables must be unaffected by FTS5's own shadow tables");
        let user_version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(user_version, 19);
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

    #[test]
    fn a_v13_database_upgrades_cleanly_to_v14_without_touching_matters() {
        // simulates an install that already has 001+002 applied (with real matter data)
        // before the app is upgraded to a build that also ships 003 - matter_profile.rs's
        // whole design point is to never ALTER the existing matters table, so this proves
        // the upgrade path leaves pre-existing matter rows completely untouched.
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATION_001).unwrap();
        conn.execute_batch(MIGRATION_002).unwrap();
        let matter_id = "m1";
        conn.execute(
            "INSERT INTO matters(id,title,matter_type,status,workflow_stage,created_at,updated_at)
             VALUES(?1,'existing matter','generic_civil','active','intake','x','x')",
            [matter_id],
        ).unwrap();
        conn.execute_batch(MIGRATION_003).unwrap();
        let profile_table_exists: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='matter_profile'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(profile_table_exists, 1);
        let matter_survived: String = conn.query_row(
            "SELECT title FROM matters WHERE id=?1", [matter_id], |r| r.get(0),
        ).unwrap();
        assert_eq!(matter_survived, "existing matter");
    }

    #[test]
    fn a_v14_database_upgrades_cleanly_to_v15_without_touching_matters() {
        // simulates an install that already has 001-003 applied (with real matter data)
        // before the app is upgraded to a build that also ships 004 - workstreams.rs's
        // seeding is done in commands.rs, not in the migration itself, so this migration
        // alone must leave pre-existing matter rows completely untouched.
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATION_001).unwrap();
        conn.execute_batch(MIGRATION_002).unwrap();
        conn.execute_batch(MIGRATION_003).unwrap();
        let matter_id = "m1";
        conn.execute(
            "INSERT INTO matters(id,title,matter_type,status,workflow_stage,created_at,updated_at)
             VALUES(?1,'existing matter','generic_civil','active','intake','x','x')",
            [matter_id],
        ).unwrap();
        conn.execute_batch(MIGRATION_004).unwrap();
        let workstreams_table_exists: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='matter_workstreams'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(workstreams_table_exists, 1);
        let matter_survived: String = conn.query_row(
            "SELECT title FROM matters WHERE id=?1", [matter_id], |r| r.get(0),
        ).unwrap();
        assert_eq!(matter_survived, "existing matter");
    }

    #[test]
    fn a_v15_database_upgrades_cleanly_to_v16_without_touching_matters() {
        // simulates an install that already has 001-004 applied (with real matter data)
        // before the app is upgraded to a build that also ships 005 - requirements.rs's
        // seeding is done in commands.rs, not in the migration itself, so this migration
        // alone must leave pre-existing matter rows completely untouched.
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATION_001).unwrap();
        conn.execute_batch(MIGRATION_002).unwrap();
        conn.execute_batch(MIGRATION_003).unwrap();
        conn.execute_batch(MIGRATION_004).unwrap();
        let matter_id = "m1";
        conn.execute(
            "INSERT INTO matters(id,title,matter_type,status,workflow_stage,created_at,updated_at)
             VALUES(?1,'existing matter','generic_civil','active','intake','x','x')",
            [matter_id],
        ).unwrap();
        conn.execute_batch(MIGRATION_005).unwrap();
        let requirements_table_exists: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='matter_requirements'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(requirements_table_exists, 1);
        let matter_survived: String = conn.query_row(
            "SELECT title FROM matters WHERE id=?1", [matter_id], |r| r.get(0),
        ).unwrap();
        assert_eq!(matter_survived, "existing matter");
    }

    #[test]
    fn a_v16_database_upgrades_cleanly_to_v17_without_touching_matters() {
        // simulates an install that already has 001-005 applied (with real matter data)
        // before the app is upgraded to a build that also ships 006 - the ledger
        // tables' seeding (there is none - ledger entries are always created
        // explicitly by a lawyer, never auto-seeded like workstreams/requirements) is
        // irrelevant here; this migration alone must leave pre-existing matter rows
        // completely untouched.
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATION_001).unwrap();
        conn.execute_batch(MIGRATION_002).unwrap();
        conn.execute_batch(MIGRATION_003).unwrap();
        conn.execute_batch(MIGRATION_004).unwrap();
        conn.execute_batch(MIGRATION_005).unwrap();
        let matter_id = "m1";
        conn.execute(
            "INSERT INTO matters(id,title,matter_type,status,workflow_stage,created_at,updated_at)
             VALUES(?1,'existing matter','generic_civil','active','intake','x','x')",
            [matter_id],
        ).unwrap();
        conn.execute_batch(MIGRATION_006).unwrap();
        let ledger_tables_exist: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN
             ('medical_events','medical_event_sources','wage_records','wage_record_sources',
              'liability_facts','liability_fact_sources')",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(ledger_tables_exist, 6);
        let matter_survived: String = conn.query_row(
            "SELECT title FROM matters WHERE id=?1", [matter_id], |r| r.get(0),
        ).unwrap();
        assert_eq!(matter_survived, "existing matter");
    }

    #[test]
    fn a_v17_database_upgrades_cleanly_to_v18_and_backfills_pre_existing_pages() {
        // simulates an install that already has 001-006 applied, WITH a real
        // document_pages row already indexed on disk, before the app is upgraded to
        // a build that also ships 007 - the whole point of the backfill INSERT in
        // 007_retrieval_context_v18.sql is that a page written before this migration
        // ever ran must still be searchable afterward, not just pages written from
        // this point onward (which the triggers alone would cover).
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATION_001).unwrap();
        conn.execute_batch(MIGRATION_002).unwrap();
        conn.execute_batch(MIGRATION_003).unwrap();
        conn.execute_batch(MIGRATION_004).unwrap();
        conn.execute_batch(MIGRATION_005).unwrap();
        conn.execute_batch(MIGRATION_006).unwrap();
        let matter_id = "m1";
        conn.execute(
            "INSERT INTO matters(id,title,matter_type,status,workflow_stage,created_at,updated_at)
             VALUES(?1,'existing matter','generic_civil','active','intake','x','x')",
            [matter_id],
        ).unwrap();
        conn.execute(
            "INSERT INTO documents(id,matter_id,logical_title,created_at,updated_at)
             VALUES('doc1',?1,'x','x','x')", [matter_id],
        ).unwrap();
        conn.execute(
            "INSERT INTO document_versions(id,document_id,matter_id,content_sha256,stale,created_at)
             VALUES('v1','doc1',?1,'x',0,'x')", [matter_id],
        ).unwrap();
        conn.execute(
            "INSERT INTO document_pages(
                id,matter_id,document_version_id,page_number,anchor_kind,block_index,
                display_text,normalized_text,text_sha256,extraction_method,created_at
             ) VALUES('p1',?1,'v1',1,'page',0,'x','גלגל מפוצץ ברכב','x','native_text','x')",
            [matter_id],
        ).unwrap();

        conn.execute_batch(MIGRATION_007).unwrap();

        let fts_table_exists: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='document_pages_fts'",
            [], |r| r.get(0),
        ).unwrap();
        assert_eq!(fts_table_exists, 1, "document_pages_fts must exist after migration 007");
        for trigger in ["trg_document_pages_fts_insert", "trg_document_pages_fts_update", "trg_document_pages_fts_delete"] {
            let exists: i64 = conn.query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='trigger' AND name=?1", [trigger], |r| r.get(0),
            ).unwrap();
            assert_eq!(exists, 1, "{trigger} must exist after migration 007");
        }
        let backfilled: String = conn.query_row(
            "SELECT page_id FROM document_pages_fts WHERE document_pages_fts MATCH 'גלגל'", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(backfilled, "p1", "a page written before migration 007 ran must be found via FTS after the backfill");

        // re-running the migration must be a true no-op on the already-backfilled row
        conn.execute_batch(MIGRATION_007).unwrap();
        let row_count: i64 = conn.query_row(
            "SELECT count(*) FROM document_pages_fts WHERE page_id='p1'", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(row_count, 1, "the backfill INSERT must be idempotent, never duplicating an already-indexed page");

        let matter_survived: String = conn.query_row(
            "SELECT title FROM matters WHERE id=?1", [matter_id], |r| r.get(0),
        ).unwrap();
        assert_eq!(matter_survived, "existing matter");
    }

    #[test]
    fn a_v18_database_upgrades_cleanly_to_v19_and_preserves_existing_matter_data() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATION_001).unwrap();
        conn.execute_batch(MIGRATION_002).unwrap();
        conn.execute_batch(MIGRATION_003).unwrap();
        conn.execute_batch(MIGRATION_004).unwrap();
        conn.execute_batch(MIGRATION_005).unwrap();
        conn.execute_batch(MIGRATION_006).unwrap();
        conn.execute_batch(MIGRATION_007).unwrap();
        conn.execute(
            "INSERT INTO matters(id,title,matter_type,status,workflow_stage,created_at,updated_at)
             VALUES('m1','existing matter','traffic_accident','active','intake','x','x')",
            [],
        ).unwrap();

        conn.execute_batch(MIGRATION_008).unwrap();

        let b7_tables: i64 = conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name IN
             ('insurance_claims','insurance_claim_insurers','insurance_claim_status_history',
              'negotiation_events','negotiation_positions','negotiation_event_corrections',
              'negotiation_position_corrections','negotiation_waiting_links')",
            [],
            |r| r.get(0),
        ).unwrap();
        assert_eq!(b7_tables, 8);
        let title: String = conn.query_row(
            "SELECT title FROM matters WHERE id='m1'", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(title, "existing matter");
        let user_version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(user_version, 19);

        conn.execute_batch(MIGRATION_008).unwrap();
        let user_version_after_rerun: i64 =
            conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(user_version_after_rerun, 19);
    }

    #[test]
    fn document_pages_fts_stays_in_sync_via_triggers_for_new_insert_update_and_delete() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(MIGRATION_001).unwrap();
        conn.execute_batch(MIGRATION_002).unwrap();
        conn.execute_batch(MIGRATION_003).unwrap();
        conn.execute_batch(MIGRATION_004).unwrap();
        conn.execute_batch(MIGRATION_005).unwrap();
        conn.execute_batch(MIGRATION_006).unwrap();
        conn.execute_batch(MIGRATION_007).unwrap();
        let matter_id = "m1";
        conn.execute(
            "INSERT INTO matters(id,title,matter_type,status,workflow_stage,created_at,updated_at)
             VALUES(?1,'m','generic_civil','active','intake','x','x')", [matter_id],
        ).unwrap();
        conn.execute(
            "INSERT INTO documents(id,matter_id,logical_title,created_at,updated_at)
             VALUES('doc1',?1,'x','x','x')", [matter_id],
        ).unwrap();
        conn.execute(
            "INSERT INTO document_versions(id,document_id,matter_id,content_sha256,stale,created_at)
             VALUES('v1','doc1',?1,'x',0,'x')", [matter_id],
        ).unwrap();

        // INSERT sync
        conn.execute(
            "INSERT INTO document_pages(
                id,matter_id,document_version_id,page_number,anchor_kind,block_index,
                display_text,normalized_text,text_sha256,extraction_method,created_at
             ) VALUES('p1',?1,'v1',1,'page',0,'x','שבר בפרק כף היד','x','native_text','x')",
            [matter_id],
        ).unwrap();
        let found: i64 = conn.query_row(
            "SELECT count(*) FROM document_pages_fts WHERE document_pages_fts MATCH 'שבר'", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(found, 1, "a newly-inserted page must be indexed immediately via the AFTER INSERT trigger");

        // UPDATE sync
        conn.execute("UPDATE document_pages SET normalized_text='טקסט שונה לחלוטין' WHERE id='p1'", []).unwrap();
        let old_gone: i64 = conn.query_row(
            "SELECT count(*) FROM document_pages_fts WHERE document_pages_fts MATCH 'שבר'", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(old_gone, 0, "the AFTER UPDATE trigger must refresh the indexed text, not leave the old text still matchable");
        let new_found: i64 = conn.query_row(
            "SELECT count(*) FROM document_pages_fts WHERE document_pages_fts MATCH 'שונה'", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(new_found, 1, "the AFTER UPDATE trigger must make the new text matchable");

        // DELETE sync
        conn.execute("DELETE FROM document_pages WHERE id='p1'", []).unwrap();
        let deleted: i64 = conn.query_row(
            "SELECT count(*) FROM document_pages_fts WHERE page_id='p1'", [], |r| r.get(0),
        ).unwrap();
        assert_eq!(deleted, 0, "the AFTER DELETE trigger must remove the FTS row when its source page is deleted");
    }
}
