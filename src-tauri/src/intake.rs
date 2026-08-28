//! Phase C, milestone C1: Smart Intake. A single matter-scoped orchestrator - scan/
//! hash, extract/OCR, classify, leave content immediately visible to FTS/retrieval -
//! built entirely out of the existing `scanner.rs`/`extraction.rs`/`classification.rs`
//! primitives. No parallel document/OCR/retrieval system: this module does not touch
//! `document_pages`, `document_versions`, or `extraction_runs` directly - it only
//! calls the functions that already own those writes, so there is exactly one place
//! each of those tables is written, regardless of whether a document is processed one
//! at a time (`extract_document_text`) or as part of a batch (this module).
//!
//! Never creates a VerifiedFact, a ledger entry, a legal deadline, or any liability/
//! damage conclusion - see `classification.rs`'s own doc comment for that boundary.
//! Runs with zero AI provider configured and zero network calls.
use crate::{classification, db::DbState, error::{AppError, AppResult}, extraction, scanner};
use chrono::Utc;
use rusqlite::params;
use serde_json::{json, Value};
use std::path::Path;

const SUPPORTED_EXTENSIONS: &[&str] = &["pdf", "docx", "txt"];

struct Candidate {
    document_id: String,
    document_version_id: String,
    matter_id: String,
    file_name: String,
    path: String,
    source_sha256: String,
    extraction_state: String,
    category_source: Option<String>,
}

/// Discovers every current (non-stale) document version in the matter that still
/// needs processing. "Current" is whatever `file_occurrences.document_version_id`
/// points at right now - the same join `extraction.rs`'s own single-document lookup
/// uses - so a version `scanner::rehash_changed_versions` has already superseded and
/// marked stale is never picked up here, matching the FTS/retrieval staleness
/// contract this pipeline must not weaken.
fn discover_candidates(db: &DbState, matter_id: &str) -> AppResult<Vec<Candidate>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(
            "SELECT d.id, v.id, v.matter_id, coalesce(o.file_name,d.logical_title,''), o.path_display,
                    v.content_sha256, v.extraction_state, d.category_source
             FROM documents d
             JOIN document_versions v ON v.document_id=d.id AND v.matter_id=d.matter_id
             JOIN file_occurrences o ON o.document_version_id=v.id AND o.matter_id=d.matter_id
             WHERE d.matter_id=?1 AND v.stale=0"
        )?;
        let rows = stmt.query_map([matter_id], |r| Ok(Candidate {
            document_id: r.get(0)?, document_version_id: r.get(1)?, matter_id: r.get(2)?,
            file_name: r.get(3)?, path: r.get(4)?, source_sha256: r.get(5)?,
            extraction_state: r.get(6)?, category_source: r.get(7)?,
        }))?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
}

fn extracted_text_for_classification(db: &DbState, document_version_id: &str) -> AppResult<String> {
    db.read(|conn| conn.query_row(
        "SELECT coalesce(group_concat(normalized_text,' '),'') FROM document_pages WHERE document_version_id=?1",
        [document_version_id], |r| r.get(0),
    ).map_err(AppError::Db))
}

fn was_ocr(db: &DbState, document_version_id: &str) -> AppResult<bool> {
    let count: i64 = db.read(|conn| conn.query_row(
        "SELECT count(*) FROM document_pages WHERE document_version_id=?1 AND extraction_method='tesseract'",
        [document_version_id], |r| r.get(0),
    ).map_err(AppError::Db))?;
    Ok(count > 0)
}

/// Manual classification always wins (section 7 of the C1 spec): a document the
/// lawyer has already categorized (`documents.category_source='manual'`) is never
/// touched here - the classifier only ever populates documents it hasn't been
/// overridden on. `classify_document_manual` (commands.rs, pre-existing) is the only
/// other writer of `documents.category*`, and it always sets `category_source=
/// 'manual'`, so this simple equality check is sufficient - no timestamp race is
/// possible since this pipeline runs synchronously per document, one command
/// invocation at a time.
fn classify_and_persist(db: &DbState, candidate: &Candidate) -> AppResult<Option<String>> {
    if candidate.category_source.as_deref() == Some("manual") {
        return Ok(None);
    }
    let text = extracted_text_for_classification(db, &candidate.document_version_id)?;
    let result = classification::classify(&candidate.file_name, &text);
    db.write(|conn| conn.execute(
        "UPDATE documents SET category=?2,category_source='auto',category_confidence=?3,updated_at=?4 WHERE id=?1",
        params![candidate.document_id, result.category, result.confidence, Utc::now().to_rfc3339()],
    ).map_err(AppError::Db))?;
    Ok(Some(result.category))
}

/// The one Smart Intake entry point (`process_matter_documents`, wired as a single
/// `#[tauri::command]` in `commands.rs`). Discovers unprocessed current versions,
/// extracts/OCRs each in turn via `extraction::extract_document_version` (which is
/// itself the exact same core `extract_document_text` uses, and which already keeps
/// the slow work - pdftotext/pdftoppm/tesseract - entirely outside any `db.write`
/// closure, only acquiring the writer for the brief final persistence step), then
/// classifies. A single document's failure is caught and recorded per-document; it
/// never aborts the loop, matching the "one failure does not block the batch"
/// requirement.
pub fn process_matter_documents(db: &DbState, matter_id: &str, resource_root: &Path) -> AppResult<Value> {
    let hashed = scanner::hash_pending(db, matter_id)?;
    let candidates = discover_candidates(db, matter_id)?;
    let discovered = candidates.len();

    let mut already_complete = 0i64;
    let mut extracted = 0i64;
    let mut ocred = 0i64;
    let mut classified = 0i64;
    let mut failed = 0i64;
    let mut unsupported = 0i64;
    let mut documents = Vec::with_capacity(candidates.len());

    for candidate in &candidates {
        if candidate.extraction_state == "complete" {
            already_complete += 1;
            let category = classify_and_persist(db, candidate)?;
            if category.is_some() { classified += 1; }
            documents.push(json!({
                "documentId": candidate.document_id, "fileName": candidate.file_name,
                "outcome": "already_complete", "category": category,
                "errorCode": Value::Null, "errorMessage": Value::Null,
            }));
            continue;
        }

        let extension = Path::new(&candidate.path)
            .extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
        if !SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
            unsupported += 1;
            documents.push(json!({
                "documentId": candidate.document_id, "fileName": candidate.file_name,
                "outcome": "unsupported", "category": Value::Null,
                "errorCode": "unsupported_format", "errorMessage": Value::Null,
            }));
            continue;
        }

        match extraction::extract_document_version(
            db, &candidate.matter_id, &candidate.document_version_id,
            &candidate.path, &candidate.source_sha256, resource_root,
        ) {
            Ok(_) => {
                let used_ocr = was_ocr(db, &candidate.document_version_id)?;
                if used_ocr { ocred += 1; } else { extracted += 1; }
                let category = classify_and_persist(db, candidate)?;
                if category.is_some() { classified += 1; }
                documents.push(json!({
                    "documentId": candidate.document_id, "fileName": candidate.file_name,
                    "outcome": if used_ocr { "ocred" } else { "extracted" }, "category": category,
                    "errorCode": Value::Null, "errorMessage": Value::Null,
                }));
            }
            Err(e) => {
                failed += 1;
                documents.push(json!({
                    "documentId": candidate.document_id, "fileName": candidate.file_name,
                    "outcome": "failed", "category": Value::Null,
                    "errorCode": extraction::extraction_error_code(&e), "errorMessage": e.to_string(),
                }));
            }
        }
    }

    Ok(json!({
        "discovered": discovered, "hashed": hashed, "alreadyComplete": already_complete,
        "extracted": extracted, "ocred": ocred, "classified": classified,
        "failed": failed, "unsupported": unsupported, "documents": documents,
    }))
}
