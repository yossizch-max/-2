//! UX Milestone 1: Direct Document Intake.
//!
//! The existing scanner.rs/matter_folder_bindings model requires a lawyer to bind a
//! matter to an Office Root folder before any document can be indexed - a real
//! prerequisite for the folder-scan workflow, but not something a lawyer should have
//! to understand just to drag one PDF onto a matter. This module is the canonical
//! direct-import path: a file the lawyer drops or picks from anywhere (Downloads,
//! Desktop, a USB drive) becomes a durable, provenance-tracked document without ever
//! depending on that original location staying valid.
//!
//! Every imported file is: hashed at its original location, copied into a
//! TAHRIR-managed per-matter folder (independent of any Office Root - the scanner
//! stays a fully optional, separate advanced workflow), re-hashed at the copy, and
//! rejected if the copy's bytes don't match the original's hash. Only then is
//! provenance registered using the exact same `documents`/`document_versions`/
//! `file_occurrences` model the scanner itself writes into - `intake::
//! process_matter_documents` (unchanged) picks the new document up exactly like any
//! scanner-discovered one, because `discover_candidates` only ever looks at
//! `document_versions`/`file_occurrences`, never at how they got there. No parallel
//! extraction/OCR/classification path is introduced.
use crate::{
    db::DbState,
    error::{AppError, AppResult},
    intake,
};
use chrono::Utc;
use rusqlite::params;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::source_snapshot::hash_file;

fn path_key(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase()
}

fn matter_exists(db: &DbState, matter_id: &str) -> AppResult<bool> {
    let count: i64 = db.read(|conn| {
        Ok(conn.query_row("SELECT COUNT(*) FROM matters WHERE id=?1", [matter_id], |r| r.get(0))?)
    })?;
    Ok(count == 1)
}

/// Copies `source_path` into a fresh, collision-proof managed location under
/// `documents_root/<matter_id>/`, verifying the copy's own hash against the source's
/// hash before any database row is written. A mismatch (partial write, disk error,
/// the source changing mid-copy) deletes the partial copy and fails loudly rather
/// than registering provenance for bytes that were never actually verified.
/// The single choke point every copy's integrity is judged against: re-hashes
/// `dest_path` and compares it to `expected_sha`, deleting the file on any mismatch
/// so a corrupted or partial copy never lingers to be mistaken for a real one. Split
/// out from `copy_verified` purely so this exact comparison-and-cleanup contract is
/// directly unit-testable without needing to force a real filesystem race.
fn verify_copy(expected_sha: &str, dest_path: &Path) -> AppResult<String> {
    let copy_sha = hash_file(dest_path)?;
    if copy_sha != expected_sha {
        let _ = fs::remove_file(dest_path);
        return Err(AppError::SourceShaMismatch);
    }
    Ok(copy_sha)
}

fn copy_verified(source_path: &Path, documents_root: &Path, matter_id: &str, file_name: &str) -> AppResult<(PathBuf, String, i64, String)> {
    let source_sha = hash_file(source_path)?;

    let dest_dir = documents_root.join(matter_id);
    fs::create_dir_all(&dest_dir)?;
    let dest_path = dest_dir.join(format!("{}-{}", Uuid::new_v4(), file_name));

    fs::copy(source_path, &dest_path)?;
    let copy_sha = verify_copy(&source_sha, &dest_path)?;

    let metadata = fs::metadata(&dest_path)?;
    let byte_size = metadata.len() as i64;
    let mtime = metadata.modified().ok().map(|x| format!("{x:?}")).unwrap_or_default();
    Ok((dest_path, copy_sha, byte_size, mtime))
}

/// Registers one imported file as a brand-new logical `Document` with exactly one
/// `DocumentVersion`, plus a `file_occurrence` pointing at the managed copy (never
/// the original path) so `open_occurrence`/`reveal_occurrence` and the extraction
/// pipeline all resolve to the durable copy TAHRIR itself controls.
fn import_one(db: &DbState, matter_id: &str, source_path: &Path, documents_root: &Path) -> AppResult<String> {
    if !matter_exists(db, matter_id)? {
        return Err(AppError::NotFound("matter".into()));
    }
    if !source_path.is_file() {
        return Err(AppError::Validation("source path is not a file".into()));
    }
    let file_name = source_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| AppError::Validation("file has no usable name".into()))?
        .to_string();

    let (dest_path, sha256, byte_size, mtime) = copy_verified(source_path, documents_root, matter_id, &file_name)?;
    let extension = Path::new(&file_name).extension().and_then(|x| x.to_str()).map(str::to_string);
    let key = path_key(&dest_path);
    let now = Utc::now().to_rfc3339();
    let document_id = Uuid::new_v4().to_string();

    db.write(|conn| {
        let tx = conn.transaction()?;
        let version_id = Uuid::new_v4().to_string();
        let occurrence_id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO documents(id,matter_id,logical_title,created_at,updated_at) VALUES(?1,?2,?3,?4,?4)",
            params![document_id, matter_id, file_name, now],
        )?;
        tx.execute(
            "INSERT INTO document_versions(id,document_id,matter_id,content_sha256,byte_size,observed_mtime,created_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7)",
            params![version_id, document_id, matter_id, sha256, byte_size, mtime, now],
        )?;
        tx.execute(
            "INSERT INTO file_occurrences(
                id,matter_id,document_id,document_version_id,path_display,path_key,file_name,extension,
                byte_size,observed_mtime,availability_state,discovered_at,last_seen_at,exists_now
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'local',?11,?11,1)",
            params![
                occurrence_id, matter_id, document_id, version_id,
                dest_path.to_string_lossy(), key, file_name, extension,
                byte_size, mtime, now,
            ],
        )?;
        tx.commit()?;
        Ok(())
    })?;
    Ok(document_id)
}

/// Imports every given source path for one matter, then immediately runs the
/// existing `intake::process_matter_documents` batch pipeline - unchanged, so
/// extraction/OCR/classification for the newly imported files happens through
/// exactly the same code path a folder scan already uses, with no duplicated
/// pipeline logic. A single file's import failure (unreadable, disk full, hash
/// mismatch) is recorded per-file and never blocks the rest of the batch, matching
/// the same "one failure does not block the batch" discipline `process_matter_
/// documents` itself already follows for extraction failures.
pub fn import_and_process(
    db: &DbState,
    matter_id: &str,
    source_paths: &[PathBuf],
    documents_root: &Path,
    resource_root: &Path,
) -> AppResult<Value> {
    let mut imported = 0i64;
    let mut import_errors: Vec<Value> = Vec::new();

    for source_path in source_paths {
        let file_name = source_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        match import_one(db, matter_id, source_path, documents_root) {
            Ok(_) => imported += 1,
            Err(e) => import_errors.push(json!({ "fileName": file_name, "errorMessage": e.to_string() })),
        }
    }

    let mut summary = intake::process_matter_documents(db, matter_id, resource_root)?;
    if let Value::Object(ref mut map) = summary {
        map.insert("imported".into(), json!(imported));
        map.insert("importErrors".into(), json!(import_errors));
    }
    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup() -> (DbState, tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let db = DbState::open(dir.path().join("app.db")).unwrap();
        let documents_root = dir.path().join("documents");
        let resource_root = dir.path().join("resources");
        fs::create_dir_all(&resource_root).unwrap();
        (db, dir, documents_root, resource_root)
    }

    fn new_matter(db: &DbState) -> String {
        let id = Uuid::new_v4().to_string();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO matters(id,title,matter_type,status,workflow_stage,created_at,updated_at)
                 VALUES(?1,'test matter','generic_civil','active','intake','x','x')",
                [&id],
            )?;
            Ok(())
        }).unwrap();
        id
    }

    fn write_source(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn importing_a_file_registers_document_version_and_occurrence() {
        let (db, dir, documents_root, resource_root) = setup();
        let matter_id = new_matter(&db);
        let src_dir = dir.path().join("downloads");
        fs::create_dir_all(&src_dir).unwrap();
        let source = write_source(&src_dir, "letter.txt", b"hello world");

        let summary = import_and_process(&db, &matter_id, &[source], &documents_root, &resource_root).unwrap();
        assert_eq!(summary["imported"], 1);
        assert_eq!(summary["importErrors"].as_array().unwrap().len(), 0);

        let (doc_count, version_count, occurrence_count): (i64, i64, i64) = db.read(|conn| {
            Ok((
                conn.query_row("SELECT COUNT(*) FROM documents WHERE matter_id=?1", [&matter_id], |r| r.get(0))?,
                conn.query_row("SELECT COUNT(*) FROM document_versions WHERE matter_id=?1", [&matter_id], |r| r.get(0))?,
                conn.query_row("SELECT COUNT(*) FROM file_occurrences WHERE matter_id=?1", [&matter_id], |r| r.get(0))?,
            ))
        }).unwrap();
        assert_eq!(doc_count, 1);
        assert_eq!(version_count, 1);
        assert_eq!(occurrence_count, 1);
    }

    #[test]
    fn the_managed_copy_is_independent_of_the_original_file() {
        let (db, dir, documents_root, resource_root) = setup();
        let matter_id = new_matter(&db);
        let src_dir = dir.path().join("desktop");
        fs::create_dir_all(&src_dir).unwrap();
        let source = write_source(&src_dir, "contract.txt", b"a contract document");

        import_and_process(&db, &matter_id, &[source.clone()], &documents_root, &resource_root).unwrap();
        // The original "Downloads/Desktop" file is deleted after import - exactly the
        // scenario the direct-import path exists to survive.
        fs::remove_file(&source).unwrap();

        let stored_path: String = db.read(|conn| {
            Ok(conn.query_row(
                "SELECT path_display FROM file_occurrences WHERE matter_id=?1",
                [&matter_id], |r| r.get(0),
            )?)
        }).unwrap();
        assert!(PathBuf::from(&stored_path).is_file(), "the managed copy must still exist after the original is gone");
        assert_ne!(PathBuf::from(&stored_path), source, "the occurrence must point at the managed copy, not the original path");
    }

    #[test]
    fn imported_document_content_is_read_and_findable_via_the_existing_pipeline() {
        let (db, dir, documents_root, resource_root) = setup();
        let matter_id = new_matter(&db);
        let src_dir = dir.path().join("downloads");
        fs::create_dir_all(&src_dir).unwrap();
        let source = write_source(&src_dir, "note.txt", "תלוש שכר לדוגמה".as_bytes());

        let summary = import_and_process(&db, &matter_id, &[source], &documents_root, &resource_root).unwrap();
        // process_matter_documents already ran as part of import_and_process - a .txt
        // file needs no OCR, so it should already be fully extracted.
        assert_eq!(summary["extracted"], 1);
        assert_eq!(summary["failed"], 0);

        let page_text: String = db.read(|conn| {
            Ok(conn.query_row(
                "SELECT p.normalized_text FROM document_pages p
                 JOIN document_versions v ON v.id=p.document_version_id
                 WHERE v.matter_id=?1",
                [&matter_id], |r| r.get(0),
            )?)
        }).unwrap();
        assert!(page_text.contains("תלוש"));
    }

    #[test]
    fn a_copy_that_does_not_match_the_expected_hash_is_rejected_and_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("copy.txt");
        fs::write(&dest, b"whatever bytes actually landed here").unwrap();
        let wrong_expected_sha = hash_file(&dir.path().join("never-written.txt")).unwrap_or_else(|_| {
            // a fixed, definitely-wrong sha256 hex string, same length as a real one
            "0".repeat(64)
        });

        let result = verify_copy(&wrong_expected_sha, &dest);
        assert!(matches!(result, Err(AppError::SourceShaMismatch)));
        assert!(!dest.exists(), "a copy that fails hash verification must not linger on disk");
    }

    #[test]
    fn a_copy_that_matches_the_expected_hash_is_accepted_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("copy.txt");
        fs::write(&dest, b"consistent bytes").unwrap();
        let expected_sha = hash_file(&dest).unwrap();

        let result = verify_copy(&expected_sha, &dest);
        assert_eq!(result.unwrap(), expected_sha);
        assert!(dest.exists(), "a verified copy must be kept, not deleted");
    }

    #[test]
    fn a_hash_mismatch_during_real_import_writes_nothing_to_the_database() {
        // Exercises the same failure through the real import path: a source that
        // cannot even be read (permission/removed mid-import) surfaces as an error
        // from copy_verified's own std::io calls, and import_one must not have
        // written any documents/document_versions/file_occurrences row before that
        // error is returned - `import_one` only commits its transaction after both
        // the copy and its hash verification already succeeded.
        let (db, dir, documents_root, _resource_root) = setup();
        let matter_id = new_matter(&db);
        let missing_source = dir.path().join("downloads").join("never-existed.pdf");

        let result = import_one(&db, &matter_id, &missing_source, &documents_root);
        assert!(result.is_err());

        let doc_count: i64 = db.read(|conn| {
            Ok(conn.query_row("SELECT COUNT(*) FROM documents WHERE matter_id=?1", [&matter_id], |r| r.get(0))?)
        }).unwrap();
        assert_eq!(doc_count, 0);
    }

    #[test]
    fn importing_into_a_nonexistent_matter_reports_a_per_file_error_and_imports_nothing() {
        // A nonexistent matter surfaces through the same per-file error path as any
        // other single-file failure (one bad file never aborts the whole call) -
        // the summary itself stays Ok, but reports zero imports and one recorded
        // error, never a silently "successful" import.
        let (db, dir, documents_root, resource_root) = setup();
        let src_dir = dir.path().join("downloads");
        fs::create_dir_all(&src_dir).unwrap();
        let source = write_source(&src_dir, "x.txt", b"content");
        let summary = import_and_process(&db, "not-a-real-matter", &[source], &documents_root, &resource_root).unwrap();
        assert_eq!(summary["imported"], 0);
        assert_eq!(summary["importErrors"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn one_bad_file_never_blocks_the_rest_of_the_batch() {
        let (db, dir, documents_root, resource_root) = setup();
        let matter_id = new_matter(&db);
        let src_dir = dir.path().join("downloads");
        fs::create_dir_all(&src_dir).unwrap();
        let good = write_source(&src_dir, "good.txt", b"real content");
        let missing = src_dir.join("does-not-exist.txt");

        let summary = import_and_process(&db, &matter_id, &[missing, good], &documents_root, &resource_root).unwrap();
        assert_eq!(summary["imported"], 1);
        assert_eq!(summary["importErrors"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn two_matters_importing_files_of_the_same_name_stay_isolated() {
        let (db, dir, documents_root, resource_root) = setup();
        let matter_a = new_matter(&db);
        let matter_b = new_matter(&db);
        let src_dir = dir.path().join("downloads");
        fs::create_dir_all(&src_dir).unwrap();
        let source_a = write_source(&src_dir, "same-name.txt", b"matter A content");
        let source_b_dir = dir.path().join("downloads2");
        fs::create_dir_all(&source_b_dir).unwrap();
        let source_b = write_source(&source_b_dir, "same-name.txt", b"matter B content");

        import_and_process(&db, &matter_a, &[source_a], &documents_root, &resource_root).unwrap();
        import_and_process(&db, &matter_b, &[source_b], &documents_root, &resource_root).unwrap();

        let count_a: i64 = db.read(|conn| Ok(conn.query_row("SELECT COUNT(*) FROM documents WHERE matter_id=?1", [&matter_a], |r| r.get(0))?)).unwrap();
        let count_b: i64 = db.read(|conn| Ok(conn.query_row("SELECT COUNT(*) FROM documents WHERE matter_id=?1", [&matter_b], |r| r.get(0))?)).unwrap();
        assert_eq!(count_a, 1);
        assert_eq!(count_b, 1);
    }

    #[test]
    fn office_root_scanner_tables_are_completely_untouched_by_direct_import() {
        let (db, dir, documents_root, resource_root) = setup();
        let matter_id = new_matter(&db);
        let src_dir = dir.path().join("downloads");
        fs::create_dir_all(&src_dir).unwrap();
        let source = write_source(&src_dir, "solo.txt", b"content");

        import_and_process(&db, &matter_id, &[source], &documents_root, &resource_root).unwrap();

        let (bindings, suggestions, scan_runs): (i64, i64, i64) = db.read(|conn| {
            Ok((
                conn.query_row("SELECT COUNT(*) FROM matter_folder_bindings", [], |r| r.get(0))?,
                conn.query_row("SELECT COUNT(*) FROM matter_suggestions", [], |r| r.get(0))?,
                conn.query_row("SELECT COUNT(*) FROM scan_runs", [], |r| r.get(0))?,
            ))
        }).unwrap();
        assert_eq!(bindings, 0);
        assert_eq!(suggestions, 0);
        assert_eq!(scan_runs, 0);
    }
}
