//! Phase C, milestone C1 (Smart Intake): real, filesystem-backed regression coverage
//! for `intake::process_matter_documents`, `extraction.rs`'s audit trail, and
//! `classification.rs`'s integration into the pipeline. Uses the exact same
//! matter/matter_folder_bindings/real-file fixture pattern as `gate_f_partial.rs`
//! (scan_metadata -> hash_pending -> extract), never a shortcut that bypasses the
//! real code path a live IPC call would take.
#![cfg(test)]

use crate::{db::DbState, error::AppError, extraction, intake, retrieval, scanner};
use chrono::Utc;
use rusqlite::params;
use std::{fs, path::PathBuf};
use uuid::Uuid;

struct TestDirs { root: PathBuf, office: PathBuf, db_path: PathBuf, resource_root: PathBuf }

impl TestDirs {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("tahrir-intake-{}", Uuid::new_v4()));
        let office = root.join("office");
        fs::create_dir_all(&office).unwrap();
        Self { db_path: root.join("tahrir.db"), resource_root: root.join("resources"), office, root }
    }
}

impl Drop for TestDirs {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.root); }
}

fn new_matter_with_folder(db: &DbState, dirs: &TestDirs, title: &str, folder_name: &str) -> (String, PathBuf) {
    let matter_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    db.write(|conn| conn.execute(
        "INSERT INTO matters(id,title,matter_type,status,workflow_stage,created_at,updated_at)
         VALUES(?1,?2,'personal_injury','active','intake',?3,?3)",
        params![matter_id, title, now],
    ).map_err(AppError::Db)).unwrap();

    let folder = dirs.office.join(folder_name);
    fs::create_dir_all(&folder).unwrap();
    let path_key = folder.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase();
    db.write(|conn| conn.execute(
        "INSERT INTO matter_folder_bindings(id,matter_id,path_display,path_key,binding_source,active,last_seen_at)
         VALUES(?1,?2,?3,?4,'test',1,?5)",
        params![Uuid::new_v4().to_string(), matter_id, folder.to_string_lossy(), path_key, now],
    ).map_err(AppError::Db)).unwrap();
    (matter_id, folder)
}

/// A minimal, real, valid DOCX: a zip archive containing `word/document.xml` with one
/// paragraph of real text - built with the same `zip` crate `extraction::extract_docx`
/// reads with, not a hand-rolled fake.
fn write_minimal_docx(path: &PathBuf, paragraph_text: &str) {
    let file = fs::File::create(path).unwrap();
    let mut writer = zip::ZipWriter::new(file);
    let options: zip::write::SimpleFileOptions = zip::write::SimpleFileOptions::default();
    writer.start_file("word/document.xml", options).unwrap();
    let xml = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
         <w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\">\
         <w:body><w:p><w:r><w:t>{paragraph_text}</w:t></w:r></w:p></w:body></w:document>"
    );
    std::io::Write::write_all(&mut writer, xml.as_bytes()).unwrap();
    writer.finish().unwrap();
}

fn run(db: &DbState, dirs: &TestDirs, matter_id: &str) -> serde_json::Value {
    scanner::scan_metadata(db, &dirs.office).unwrap();
    intake::process_matter_documents(db, matter_id, &dirs.resource_root).unwrap()
}

#[test]
fn process_matter_documents_extracts_hashes_classifies_and_makes_text_searchable() {
    let dirs = TestDirs::new();
    let db = DbState::open(dirs.db_path.clone()).unwrap();
    let (matter_id, folder) = new_matter_with_folder(&db, &dirs, "תיק בדיקה", "matter-a");

    fs::write(folder.join("wage_slip.txt"), "תלוש שכר לחודש 01/2026, שכר ברוטו: 12000, מעסיק: חברה בע\"מ").unwrap();
    write_minimal_docx(&folder.join("expert.docx"), "חוות דעת מומחה: נכות רפואית בשיעור 10% לאחר בדיקה רפואית");

    let result = run(&db, &dirs, &matter_id);
    assert_eq!(result["discovered"], 2);
    assert_eq!(result["hashed"], 2);
    assert_eq!(result["extracted"], 2, "both native-text formats (txt, docx) must extract without OCR");
    assert_eq!(result["ocred"], 0);
    assert_eq!(result["classified"], 2);
    assert_eq!(result["failed"], 0);
    assert_eq!(result["unsupported"], 0);

    let categories: Vec<String> = db.read(|conn| {
        let mut stmt = conn.prepare("SELECT category FROM documents WHERE matter_id=?1 ORDER BY category")?;
        let rows = stmt.query_map([&matter_id], |r| r.get(0))?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }).unwrap();
    assert_eq!(categories, vec!["expert_opinion".to_string(), "wage".to_string()]);

    // The pipeline's own persisted text must be reachable through the existing,
    // unmodified retrieval/FTS layer - no second search index.
    let manifest = retrieval::build_context_manifest(&db, &matter_id, "extract_facts", Some("מעסיק")).unwrap();
    assert!(manifest.sources.iter().any(|s| s.text.contains("מעסיק")), "the wage slip's extracted text must be findable via the existing retrieval pipeline");
}

#[test]
fn matter_isolation_holds_after_intake() {
    let dirs = TestDirs::new();
    let db = DbState::open(dirs.db_path.clone()).unwrap();
    let (matter_a, folder_a) = new_matter_with_folder(&db, &dirs, "תיק א", "matter-iso-a");
    let (matter_b, folder_b) = new_matter_with_folder(&db, &dirs, "תיק ב", "matter-iso-b");
    fs::write(folder_a.join("doc.txt"), "מונח ייחודי לבדיקת בידוד תיקים").unwrap();
    fs::write(folder_b.join("doc.txt"), "מונח ייחודי לבדיקת בידוד תיקים").unwrap();

    run(&db, &dirs, &matter_a);
    run(&db, &dirs, &matter_b);

    let manifest_a = retrieval::build_context_manifest(&db, &matter_a, "extract_facts", Some("בידוד")).unwrap();
    let manifest_b = retrieval::build_context_manifest(&db, &matter_b, "extract_facts", Some("בידוד")).unwrap();
    assert_eq!(manifest_a.sources.len(), 1);
    assert_eq!(manifest_b.sources.len(), 1);
    assert_ne!(manifest_a.sources[0].source_id, manifest_b.sources[0].source_id, "identical text in two matters must never resolve to the same source");
}

#[test]
fn unsupported_file_types_do_not_abort_the_batch() {
    let dirs = TestDirs::new();
    let db = DbState::open(dirs.db_path.clone()).unwrap();
    let (matter_id, folder) = new_matter_with_folder(&db, &dirs, "תיק בדיקה", "matter-unsupported");
    fs::write(folder.join("good.txt"), "בית משפט השלום נתן החלטה בעניין הבקשה").unwrap();
    fs::write(folder.join("bad.xlsx"), "not really an xlsx, just bytes").unwrap();

    let result = run(&db, &dirs, &matter_id);
    assert_eq!(result["discovered"], 2);
    assert_eq!(result["extracted"], 1, "the unsupported file must never block the supported one from completing");
    assert_eq!(result["unsupported"], 1);
    let documents = result["documents"].as_array().unwrap();
    let unsupported_entry = documents.iter().find(|d| d["fileName"] == "bad.xlsx").unwrap();
    assert_eq!(unsupported_entry["outcome"], "unsupported");
    assert_eq!(unsupported_entry["errorCode"], "unsupported_format");
    let good_entry = documents.iter().find(|d| d["fileName"] == "good.txt").unwrap();
    assert_eq!(good_entry["outcome"], "extracted");
    assert_eq!(good_entry["category"], "court");
}

#[test]
fn already_complete_documents_are_not_reextracted_but_are_still_reported() {
    let dirs = TestDirs::new();
    let db = DbState::open(dirs.db_path.clone()).unwrap();
    let (matter_id, folder) = new_matter_with_folder(&db, &dirs, "תיק בדיקה", "matter-already-complete");
    fs::write(folder.join("doc.txt"), "תוכן כללי לבדיקת אי-חילוץ חוזר").unwrap();

    let first = run(&db, &dirs, &matter_id);
    assert_eq!(first["extracted"], 1);
    assert_eq!(first["alreadyComplete"], 0);

    let version_id: String = db.read(|conn| conn.query_row(
        "SELECT id FROM document_versions WHERE matter_id=?1", [&matter_id], |r| r.get(0),
    ).map_err(AppError::Db)).unwrap();
    let runs_after_first: i64 = db.read(|conn| conn.query_row(
        "SELECT count(*) FROM extraction_runs WHERE document_version_id=?1", [&version_id], |r| r.get(0),
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(runs_after_first, 1);

    let second = run(&db, &dirs, &matter_id);
    assert_eq!(second["extracted"], 0, "an unchanged, already-complete version must not be re-extracted");
    assert_eq!(second["alreadyComplete"], 1);
    let runs_after_second: i64 = db.read(|conn| conn.query_row(
        "SELECT count(*) FROM extraction_runs WHERE document_version_id=?1", [&version_id], |r| r.get(0),
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(runs_after_second, 1, "no new extraction_runs row should be created for a document that was skipped, not re-attempted");
}

#[test]
fn manual_category_is_never_overwritten_by_the_pipeline() {
    let dirs = TestDirs::new();
    let db = DbState::open(dirs.db_path.clone()).unwrap();
    let (matter_id, folder) = new_matter_with_folder(&db, &dirs, "תיק בדיקה", "matter-manual-category");
    fs::write(folder.join("doc.txt"), "תלוש שכר שכר ברוטו מעסיק").unwrap();

    run(&db, &dirs, &matter_id);
    let document_id: String = db.read(|conn| conn.query_row(
        "SELECT id FROM documents WHERE matter_id=?1", [&matter_id], |r| r.get(0),
    ).map_err(AppError::Db)).unwrap();
    let auto_category: String = db.read(|conn| conn.query_row(
        "SELECT category FROM documents WHERE id=?1", [&document_id], |r| r.get(0),
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(auto_category, "wage");

    // Lawyer manually overrides the category (mirrors commands::classify_document_manual).
    db.write(|conn| conn.execute(
        "UPDATE documents SET category='court',category_source='manual',category_confidence=1.0,updated_at=?2 WHERE id=?1",
        params![document_id, Utc::now().to_rfc3339()],
    ).map_err(AppError::Db)).unwrap();

    // Re-running intake (e.g. after adding another file) must never touch this document's category again.
    fs::write(folder.join("second.txt"), "עוד מסמך כלשהו").unwrap();
    run(&db, &dirs, &matter_id);

    let (category_after, source_after): (String, Option<String>) = db.read(|conn| conn.query_row(
        "SELECT category,category_source FROM documents WHERE id=?1", [&document_id], |r| Ok((r.get(0)?, r.get(1)?)),
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(category_after, "court", "a manually-set category must never be silently overwritten by the automatic classifier");
    assert_eq!(source_after.as_deref(), Some("manual"));
}

#[test]
fn extraction_runs_records_a_real_audit_trail_and_retry_creates_a_new_row() {
    let dirs = TestDirs::new();
    let db = DbState::open(dirs.db_path.clone()).unwrap();
    let (matter_id, folder) = new_matter_with_folder(&db, &dirs, "תיק בדיקה", "matter-audit");
    let doc_path = folder.join("doc.txt");
    fs::write(&doc_path, "תוכן מקורי").unwrap();

    scanner::scan_metadata(&db, &dirs.office).unwrap();
    scanner::hash_pending(&db, &matter_id).unwrap();
    let document_id: String = db.read(|conn| conn.query_row(
        "SELECT id FROM documents WHERE matter_id=?1", [&matter_id], |r| r.get(0),
    ).map_err(AppError::Db)).unwrap();
    let version_id: String = db.read(|conn| conn.query_row(
        "SELECT id FROM document_versions WHERE matter_id=?1", [&matter_id], |r| r.get(0),
    ).map_err(AppError::Db)).unwrap();

    // First attempt: real success.
    extraction::extract_document(&db, &document_id, &dirs.resource_root).unwrap();
    let (first_run_id, first_status, first_finished): (String, String, Option<String>) = db.read(|conn| conn.query_row(
        "SELECT id,status,finished_at FROM extraction_runs WHERE document_version_id=?1 ORDER BY started_at",
        [&version_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(first_status, "completed");
    assert!(first_finished.is_some());
    let version_state: String = db.read(|conn| conn.query_row(
        "SELECT extraction_state FROM document_versions WHERE id=?1", [&version_id], |r| r.get(0),
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(version_state, "complete");

    // Tamper the file after hashing (without rehashing) so a re-extraction attempt on
    // the SAME version fails closed with a source mismatch - this simulates a retry
    // hitting a real, distinguishable failure.
    fs::write(&doc_path, "תוכן שהשתנה - לא אמור להתקבל").unwrap();
    let retry_result = extraction::extract_document(&db, &document_id, &dirs.resource_root);
    assert!(retry_result.is_err());

    let runs: Vec<(String, String, Option<String>)> = db.read(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id,status,error_code FROM extraction_runs WHERE document_version_id=?1 ORDER BY started_at"
        )?;
        let rows = stmt.query_map([&version_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }).unwrap();
    assert_eq!(runs.len(), 2, "a retry must create a second extraction_runs row, never rewrite the first");
    assert_eq!(runs[0].0, first_run_id);
    assert_eq!(runs[0].1, "completed", "the original successful run's row must never be rewritten by a later failed retry");
    assert_eq!(runs[1].1, "failed");
    assert_eq!(runs[1].2.as_deref(), Some("source_changed"));

    // The version's own extraction_state must honestly reflect the failed retry, and
    // must never claim "complete" for content that was actually rejected.
    let version_state_after_failed_retry: String = db.read(|conn| conn.query_row(
        "SELECT extraction_state FROM document_versions WHERE id=?1", [&version_id], |r| r.get(0),
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(version_state_after_failed_retry, "failed");

    // The original successfully-extracted pages must still be intact (never partially
    // overwritten by the rejected retry attempt).
    let page_count: i64 = db.read(|conn| conn.query_row(
        "SELECT count(*) FROM document_pages WHERE document_version_id=?1", [&version_id], |r| r.get(0),
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(page_count, 1, "a rejected retry must not touch the previously-committed pages");

    // Restore the original bytes and retry again for real - this must succeed and add
    // a third audit row, still never touching the first two.
    fs::write(&doc_path, "תוכן מקורי").unwrap();
    extraction::extract_document(&db, &document_id, &dirs.resource_root).unwrap();
    let final_run_count: i64 = db.read(|conn| conn.query_row(
        "SELECT count(*) FROM extraction_runs WHERE document_version_id=?1", [&version_id], |r| r.get(0),
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(final_run_count, 3);
    let final_state: String = db.read(|conn| conn.query_row(
        "SELECT extraction_state FROM document_versions WHERE id=?1", [&version_id], |r| r.get(0),
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(final_state, "complete");
}

#[test]
fn a_stale_document_version_is_never_picked_up_by_intake_or_returned_by_retrieval() {
    let dirs = TestDirs::new();
    let db = DbState::open(dirs.db_path.clone()).unwrap();
    let (matter_id, folder) = new_matter_with_folder(&db, &dirs, "תיק בדיקה", "matter-stale");
    let doc_path = folder.join("doc.txt");
    // Deliberately zero shared words between the two contents (retrieval.rs's
    // tokenizer splits on any non-alphanumeric character, including underscore, so an
    // underscore-joined "unique" identifier is not actually one token - see B5a's own
    // tokenization lessons) - each sentence uses entirely distinct vocabulary.
    fs::write(&doc_path, "ראשונית").unwrap();

    run(&db, &dirs, &matter_id);
    let old_version_id: String = db.read(|conn| conn.query_row(
        "SELECT id FROM document_versions WHERE matter_id=?1", [&matter_id], |r| r.get(0),
    ).map_err(AppError::Db)).unwrap();

    // Change the file's content for real (triggers scanner::rehash_changed_versions'
    // new-version-and-stale-the-old-one path) and re-run intake.
    fs::write(&doc_path, "מעודכנת").unwrap();
    let second = run(&db, &dirs, &matter_id);
    assert_eq!(second["hashed"], 1, "the changed file must be rehashed into a new version");

    let old_stale: i64 = db.read(|conn| conn.query_row(
        "SELECT stale FROM document_versions WHERE id=?1", [&old_version_id], |r| r.get(0),
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(old_stale, 1);

    // intake must never have re-extracted the now-stale old version.
    let old_version_runs: i64 = db.read(|conn| conn.query_row(
        "SELECT count(*) FROM extraction_runs WHERE document_version_id=?1", [&old_version_id], |r| r.get(0),
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(old_version_runs, 1, "the stale version must have exactly its original extraction_runs row - never reprocessed");

    // retrieval must only ever surface the current version's content, never the stale one's.
    let manifest_old_term = retrieval::build_context_manifest(&db, &matter_id, "extract_facts", Some("ראשונית")).unwrap();
    assert!(manifest_old_term.sources.is_empty(), "a stale version's text must never be returned by retrieval");
    let manifest_new_term = retrieval::build_context_manifest(&db, &matter_id, "extract_facts", Some("מעודכנת")).unwrap();
    assert_eq!(manifest_new_term.sources.len(), 1);
}
