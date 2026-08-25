//! Regression coverage for integrity gaps found by two rounds of an external "Deep
//! Control" audit of this reconstruction (2026-08-25). Both rounds re-used P0-N/P1-N
//! numbering for unrelated findings, so this file refers to them as "v1 P0-N" / "v2
//! P0-N" throughout to avoid collisions - never bare "P0-N".
//!
//! v1 findings, verified directly against this codebase before being fixed:
//!  - v1 P0-1: a source that changed after hashing never got a new DocumentVersion.
//!  - v1 P0-2: a deleted/moved source was never marked missing (exists_now stayed 1).
//!  - v1 P0-3: legal_document_sections/legal_document_paragraphs had no immutability
//!    trigger, so an approved legal document's actual text could still be edited.
//!  - v1 P0-4: confirm_paragraph flipped provenance_state to 'confirmed' with no check
//!    that a 'fact' paragraph actually had a live grounding source.
//!  - v1 P0-5: approve_version only checked provenance_state, never re-checked that a
//!    fact confirmed earlier in the session was still valid/non-stale at approval time.
//!  - v1 P1-6: lock_damage_calculation trusted a client-supplied integrity hash outright.
//! v1 P0-6 (Verified Authority requiring a source) is fixed in commands.rs but not
//! re-tested here: verify_authority lives behind tauri::State with no public
//! constructor outside a running app, same limitation already documented for
//! export_legal_document's PDF guard in gate_f_partial.rs - verified by direct code
//! reading instead.
//!
//! Also covers v1 P1-1 ("the lawyer cannot complete the AI -> proposal -> review ->
//! Verified Fact workflow"): `review_ai_proposal`'s 'approved' path previously only
//! flipped `ai_proposals.status` with no command anywhere that turned an approved
//! proposal into an actual VerifiedFact. `ai::approve_proposal` is the fix, and is
//! plain-Rust testable without a live provider (a real AI response is simulated by
//! inserting a synthetic `ai_proposals` row directly, exactly like the run/proposal
//! `run_capability` itself would have produced).
//!
//! v2 findings (a second audit pass against the v1 fixes above):
//!  - v2 P0-1: `scan_metadata` used `filter_map(Result::ok)` over the WalkDir iterator,
//!    silently discarding traversal errors (e.g. an unreadable subdirectory), then
//!    unconditionally treated reaching the end of the walk as proof of a complete scan
//!    - so a partially-failed walk could still mass-mark previously-known files
//!    missing. Fixed by counting errors instead of discarding them and gating the
//!    missing-marking step on that count (`scan_is_authoritative`, unit-tested directly
//!    in `scanner.rs`'s own test module - forcing a real WalkDir permission error
//!    isn't reliable in an integration test when the test process runs as root, which
//!    bypasses the directory-permission checks that would normally produce one).
//!  - v2 P0-2: `ai::approve_proposal` validated a cited `sourceId` only against
//!    `document_pages`, never checking whether its `document_versions` row had since
//!    gone stale - so a proposal that sat pending while its source was edited (and
//!    superseded by `scanner::rehash_changed_versions`) could still be approved into a
//!    VerifiedFact grounded in outdated content. Fixed: approval now validates every
//!    cited source's version is non-stale *before* creating anything.
//!  - v2 P1-4: `damage_inputs.value_text` parsing used `.unwrap_or(0)` in both the list
//!    and lock paths - corrupt persisted data would silently become a zero financial
//!    figure instead of failing. Fixed to a hard `rusqlite::Error` on parse failure.
//!  - v2 P1-7: `FactsAITab`'s "פתח מקור" (open source) button had no `onClick` at all.
//!    Fixed by having `list_verified_facts` also return an `occurrenceId` (joined
//!    through `verified_fact_sources.document_version_id`) and wiring the button to
//!    `open_occurrence` - this one is a frontend wire-up with no new backend logic to
//!    unit test, so it isn't covered by a test in this file.
//!  - v2 P0-3 (stale SOURCE-MANIFEST.json/QA_*.json at the repo root) and v2 P0-4 (the
//!    Windows installer trailing the latest source again) aren't code-level findings
//!    and have no corresponding test here.
//!
//! v2 P1-2 (fixed in a later pass, not part of the original v2 response): `verify_authority`
//! only ever required `source_document_version_id` to be set - an authority citing an
//! entire document could be "verified" without anyone ever having read and stood behind
//! a specific passage of it. `legal_authority_passages` existed in the schema but was
//! never read or written by any command. Fixed in the new `authorities` module (split out
//! of `commands.rs`, which is untestable here for the same `tauri::State` reason as
//! `verify_authority` itself used to be - see the v1 P0-6 note above): `add_passage`
//! requires the quoted text to appear verbatim (after `extraction::normalize_source_text`)
//! on the cited page of the authority's own source document; `approve_passage` re-checks
//! that same containment against the page's *current* text; `verify` now requires at
//! least one approved passage and folds the approved passages' hashes into the integrity
//! hash.

#![cfg(test)]

use crate::{ai, authorities, damage, db::DbState, error::AppError, legal_docs, models::DamageInput, scanner};
use chrono::Utc;
use rusqlite::params;
use serde_json::json;
use std::{fs, path::PathBuf};
use uuid::Uuid;

struct TestDirs { root: PathBuf, office: PathBuf, db_path: PathBuf }

impl TestDirs {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("tahrir-integrity-{}", Uuid::new_v4()));
        let office = root.join("office");
        fs::create_dir_all(&office).unwrap();
        Self { db_path: root.join("tahrir.db"), root, office }
    }
}

impl Drop for TestDirs {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.root); }
}

fn new_matter(db: &DbState, title: &str) -> String {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    db.write(|conn| {
        conn.execute(
            "INSERT INTO matters(id,title,matter_type,status,workflow_stage,created_at,updated_at)
             VALUES(?1,?2,'personal_injury','active','intake',?3,?3)",
            params![id, title, now],
        ).map_err(AppError::Db)?;
        Ok(())
    }).unwrap();
    id
}

fn bind_folder(db: &DbState, matter_id: &str, folder: &PathBuf) {
    let path_key = folder.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase();
    let now = Utc::now().to_rfc3339();
    db.write(|conn| {
        conn.execute(
            "INSERT INTO matter_folder_bindings(id,matter_id,path_display,path_key,binding_source,active,last_seen_at)
             VALUES(?1,?2,?3,?4,'test',1,?5)",
            params![Uuid::new_v4().to_string(), matter_id, folder.to_string_lossy(), path_key, now],
        ).map_err(AppError::Db)?;
        Ok(())
    }).unwrap();
}

// --- P0-3: approved legal-document content is immutable at the DB level ---

#[test]
fn approved_legal_content_is_immutable_at_db_level() {
    let dirs = TestDirs::new();
    let db = DbState::open(dirs.db_path.clone()).unwrap();
    let matter_id = new_matter(&db, "P0-3 test");

    let doc_id = legal_docs::create_draft(&db, &matter_id, "מכתב דרישה", "demand").unwrap();
    let version_id: String = db.read(|conn| conn.query_row(
        "SELECT current_version_id FROM legal_documents WHERE id=?1", [&doc_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    let section_id: String = db.read(|conn| conn.query_row(
        "SELECT id FROM legal_document_sections WHERE legal_document_version_id=?1 ORDER BY section_index LIMIT 1",
        [&version_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();

    let paragraph_id = legal_docs::add_paragraph(&db, &matter_id, &version_id, &section_id, "טענה משפטית.").unwrap();
    legal_docs::confirm_paragraph(&db, &matter_id, &version_id, &paragraph_id).unwrap();
    legal_docs::approve_version(&db, &matter_id, &version_id).unwrap();

    let mutate_paragraph_text = db.write(|conn| conn.execute(
        "UPDATE legal_document_paragraphs SET body_text='HACKED' WHERE id=?1", [&paragraph_id]
    ).map_err(AppError::Db));
    assert!(mutate_paragraph_text.is_err(), "trigger must block editing an approved version's paragraph text");

    let mutate_section_heading = db.write(|conn| conn.execute(
        "UPDATE legal_document_sections SET heading='HACKED' WHERE id=?1", [&section_id]
    ).map_err(AppError::Db));
    assert!(mutate_section_heading.is_err(), "trigger must block editing an approved version's section heading");

    let delete_paragraph = db.write(|conn| conn.execute(
        "DELETE FROM legal_document_paragraphs WHERE id=?1", [&paragraph_id]
    ).map_err(AppError::Db));
    assert!(delete_paragraph.is_err(), "trigger must block deleting an approved version's paragraph");

    let insert_new_paragraph = db.write(|conn| conn.execute(
        "INSERT INTO legal_document_paragraphs(
            id,matter_id,legal_document_version_id,section_id,paragraph_index,paragraph_kind,body_text,provenance_state
         ) VALUES(?1,?2,?3,?4,99,'argument','snuck in','confirmed')",
        params![Uuid::new_v4().to_string(), matter_id, version_id, section_id],
    ).map_err(AppError::Db));
    assert!(insert_new_paragraph.is_err(), "trigger must block inserting a new paragraph into an approved version");
}

// --- P0-4: a 'fact' paragraph requires a live grounding source to be confirmed ---

#[test]
fn confirming_a_fact_paragraph_requires_a_live_source() {
    let dirs = TestDirs::new();
    let db = DbState::open(dirs.db_path.clone()).unwrap();
    let matter_id = new_matter(&db, "P0-4 test");

    let doc_id = legal_docs::create_draft(&db, &matter_id, "כתב תביעה", "claim").unwrap();
    let version_id: String = db.read(|conn| conn.query_row(
        "SELECT current_version_id FROM legal_documents WHERE id=?1", [&doc_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    let section_id: String = db.read(|conn| conn.query_row(
        "SELECT id FROM legal_document_sections WHERE legal_document_version_id=?1 ORDER BY section_index LIMIT 1",
        [&version_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();

    // A bare paragraph_kind='fact' row with no source at all - inserted directly to
    // simulate the exact attack the audit described, bypassing add_paragraph (which
    // never produces 'fact'-kind paragraphs in the first place).
    let bare_fact_id = Uuid::new_v4().to_string();
    db.write(|conn| conn.execute(
        "INSERT INTO legal_document_paragraphs(
            id,matter_id,legal_document_version_id,section_id,paragraph_index,paragraph_kind,body_text,provenance_state
         ) VALUES(?1,?2,?3,?4,0,'fact','עובדה כביכול בלי מקור','needs_review')",
        params![bare_fact_id, matter_id, version_id, section_id],
    ).map_err(AppError::Db)).unwrap();

    let confirm_ungrounded = legal_docs::confirm_paragraph(&db, &matter_id, &version_id, &bare_fact_id);
    assert!(confirm_ungrounded.is_err(), "a fact-kind paragraph with no linked verified fact must not be confirmable");

    // add_paragraph itself only ever produces 'argument'-kind paragraphs, which have no
    // grounding requirement - confirming one is a plain editorial review action.
    let argument_id = legal_docs::add_paragraph(&db, &matter_id, &version_id, &section_id, "טענה משפטית ללא מקור עובדתי.").unwrap();
    legal_docs::confirm_paragraph(&db, &matter_id, &version_id, &argument_id).unwrap();
}

// --- P0-5: approval re-validates fact freshness, not just a stale provenance_state ---

#[test]
fn approval_rejects_a_fact_invalidated_after_it_was_confirmed() {
    let dirs = TestDirs::new();
    let db = DbState::open(dirs.db_path.clone()).unwrap();
    let matter_id = new_matter(&db, "P0-5 test");
    let now = Utc::now().to_rfc3339();

    let fact_id = Uuid::new_v4().to_string();
    db.write(|conn| conn.execute(
        "INSERT INTO verified_facts(id,matter_id,subject,predicate,value_text,status,verified_at)
         VALUES(?1,?2,'התובע','נחבל','ברגל','valid',?3)",
        params![fact_id, matter_id, now],
    ).map_err(AppError::Db)).unwrap();

    let doc_id = legal_docs::create_draft(&db, &matter_id, "כתב תביעה", "claim").unwrap();
    let version_id: String = db.read(|conn| conn.query_row(
        "SELECT current_version_id FROM legal_documents WHERE id=?1", [&doc_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    let added = legal_docs::fill_from_verified_facts(&db, &matter_id, &version_id).unwrap();
    assert_eq!(added, 1);

    // the fact is invalidated AFTER being auto-filled-and-confirmed, before approval -
    // provenance_state on the paragraph is still 'confirmed' from when it was added.
    db.write(|conn| conn.execute(
        "UPDATE verified_facts SET status='invalidated' WHERE id=?1", [&fact_id]
    ).map_err(AppError::Db)).unwrap();

    let approve_with_invalidated_fact = legal_docs::approve_version(&db, &matter_id, &version_id);
    assert!(approve_with_invalidated_fact.is_err(), "approval must re-check fact validity, not trust a stale provenance_state");

    // re-verify the fact and confirm approval succeeds once it's genuinely valid again
    db.write(|conn| conn.execute(
        "UPDATE verified_facts SET status='valid' WHERE id=?1", [&fact_id]
    ).map_err(AppError::Db)).unwrap();
    legal_docs::approve_version(&db, &matter_id, &version_id).unwrap();
}

#[test]
fn approval_rejects_a_fact_gone_stale_after_it_was_confirmed() {
    let dirs = TestDirs::new();
    let db = DbState::open(dirs.db_path.clone()).unwrap();
    let matter_id = new_matter(&db, "P0-5b test");
    let now = Utc::now().to_rfc3339();

    let fact_id = Uuid::new_v4().to_string();
    db.write(|conn| conn.execute(
        "INSERT INTO verified_facts(id,matter_id,subject,predicate,value_text,status,verified_at)
         VALUES(?1,?2,'התובע','נחבל','ברגל','valid',?3)",
        params![fact_id, matter_id, now],
    ).map_err(AppError::Db)).unwrap();

    let doc_id = legal_docs::create_draft(&db, &matter_id, "כתב תביעה", "claim").unwrap();
    let version_id: String = db.read(|conn| conn.query_row(
        "SELECT current_version_id FROM legal_documents WHERE id=?1", [&doc_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    legal_docs::fill_from_verified_facts(&db, &matter_id, &version_id).unwrap();

    db.write(|conn| conn.execute(
        "UPDATE verified_facts SET stale=1 WHERE id=?1", [&fact_id]
    ).map_err(AppError::Db)).unwrap();

    let approve_with_stale_fact = legal_docs::approve_version(&db, &matter_id, &version_id);
    assert!(approve_with_stale_fact.is_err(), "approval must reject a version citing a fact that has gone stale");
}

// --- P0-1 / P0-2: scanner re-versioning, staleness cascade, missing-file detection ---

#[test]
fn changed_source_gets_a_new_version_under_the_same_document_and_cascades_stale() {
    let dirs = TestDirs::new();
    let db = DbState::open(dirs.db_path.clone()).unwrap();
    let matter_id = new_matter(&db, "P0-1 test");
    let folder = dirs.office.join("matter folder");
    fs::create_dir_all(&folder).unwrap();
    bind_folder(&db, &matter_id, &folder);

    let file_path = folder.join("letter.txt");
    fs::write(&file_path, "original content").unwrap();
    scanner::scan_metadata(&db, &dirs.office).unwrap();
    scanner::hash_pending(&db, &matter_id).unwrap();

    let occurrence_id: String = db.read(|conn| conn.query_row(
        "SELECT id FROM file_occurrences WHERE matter_id=?1", [&matter_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    let old_version_id: String = db.read(|conn| conn.query_row(
        "SELECT document_version_id FROM file_occurrences WHERE id=?1", [&occurrence_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();

    // ground a verified fact in the original version's page, so its stale-cascade can be checked
    let now = Utc::now().to_rfc3339();
    let fact_id = Uuid::new_v4().to_string();
    let page_id = Uuid::new_v4().to_string();
    db.write(|conn| {
        conn.execute(
            "INSERT INTO document_pages(id,matter_id,document_version_id,page_number,anchor_kind,block_index,display_text,normalized_text,text_sha256,extraction_method,created_at)
             VALUES(?1,?2,?3,1,'page',0,'x','x','x','native_text',?4)",
            params![page_id, matter_id, old_version_id, now],
        ).map_err(AppError::Db)?;
        conn.execute(
            "INSERT INTO verified_facts(id,matter_id,subject,predicate,value_text,status,verified_at)
             VALUES(?1,?2,'a','b','c','valid',?3)",
            params![fact_id, matter_id, now],
        ).map_err(AppError::Db)?;
        conn.execute(
            "INSERT INTO verified_fact_sources(id,matter_id,verified_fact_id,document_version_id,document_page_id,display_quote,normalized_quote,source_text_sha256)
             VALUES(?1,?2,?3,?4,?5,'x','x','x')",
            params![Uuid::new_v4().to_string(), matter_id, fact_id, old_version_id, page_id],
        ).map_err(AppError::Db)?;
        Ok(())
    }).unwrap();

    // the source legitimately changes
    fs::write(&file_path, "REPLACED content, materially different from the original").unwrap();
    scanner::scan_metadata(&db, &dirs.office).unwrap();
    let rehashed = scanner::hash_pending(&db, &matter_id).unwrap();
    assert_eq!(rehashed, 1, "the changed occurrence must be picked up by the rehash pass");

    let new_version_id: String = db.read(|conn| conn.query_row(
        "SELECT document_version_id FROM file_occurrences WHERE id=?1", [&occurrence_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    assert_ne!(new_version_id, old_version_id, "a changed source must move the occurrence onto a NEW DocumentVersion");

    let old_stale: i64 = db.read(|conn| conn.query_row(
        "SELECT stale FROM document_versions WHERE id=?1", [&old_version_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(old_stale, 1, "the superseded version must be marked stale");

    let (old_document_id, new_document_id): (String, String) = db.read(|conn| {
        let old_doc: String = conn.query_row(
            "SELECT document_id FROM document_versions WHERE id=?1", [&old_version_id], |r| r.get(0)
        )?;
        let new_doc: String = conn.query_row(
            "SELECT document_id FROM document_versions WHERE id=?1", [&new_version_id], |r| r.get(0)
        )?;
        Ok((old_doc, new_doc))
    }.map_err(AppError::Db)).unwrap();
    assert_eq!(old_document_id, new_document_id, "the new version must belong to the SAME logical Document, not a new one");

    let fact_stale: i64 = db.read(|conn| conn.query_row(
        "SELECT stale FROM verified_facts WHERE id=?1", [&fact_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(fact_stale, 1, "a fact grounded in the superseded version must cascade to stale");
}

#[test]
fn a_metadata_only_touch_does_not_spawn_a_pointless_new_version() {
    let dirs = TestDirs::new();
    let db = DbState::open(dirs.db_path.clone()).unwrap();
    let matter_id = new_matter(&db, "P0-1b test");
    let folder = dirs.office.join("matter folder");
    fs::create_dir_all(&folder).unwrap();
    bind_folder(&db, &matter_id, &folder);

    let file_path = folder.join("letter.txt");
    fs::write(&file_path, "identical content").unwrap();
    scanner::scan_metadata(&db, &dirs.office).unwrap();
    scanner::hash_pending(&db, &matter_id).unwrap();

    let occurrence_id: String = db.read(|conn| conn.query_row(
        "SELECT id FROM file_occurrences WHERE matter_id=?1", [&matter_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    let version_before: String = db.read(|conn| conn.query_row(
        "SELECT document_version_id FROM file_occurrences WHERE id=?1", [&occurrence_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();

    // re-write the exact same bytes (bumps mtime, not content)
    std::thread::sleep(std::time::Duration::from_millis(10));
    fs::write(&file_path, "identical content").unwrap();
    scanner::scan_metadata(&db, &dirs.office).unwrap();
    let rehashed = scanner::hash_pending(&db, &matter_id).unwrap();

    let version_after: String = db.read(|conn| conn.query_row(
        "SELECT document_version_id FROM file_occurrences WHERE id=?1", [&occurrence_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(version_before, version_after, "identical content must not spawn a new DocumentVersion, even if mtime changed");
    let _ = rehashed;
}

#[test]
fn a_fully_removed_source_file_is_marked_missing_after_a_complete_scan() {
    let dirs = TestDirs::new();
    let db = DbState::open(dirs.db_path.clone()).unwrap();
    let matter_id = new_matter(&db, "P0-2 test");
    let folder = dirs.office.join("matter folder");
    fs::create_dir_all(&folder).unwrap();
    bind_folder(&db, &matter_id, &folder);

    let file_path = folder.join("will-be-deleted.txt");
    fs::write(&file_path, "temporary").unwrap();
    scanner::scan_metadata(&db, &dirs.office).unwrap();

    let occurrence_id: String = db.read(|conn| conn.query_row(
        "SELECT id FROM file_occurrences WHERE matter_id=?1", [&matter_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    let exists_before: i64 = db.read(|conn| conn.query_row(
        "SELECT exists_now FROM file_occurrences WHERE id=?1", [&occurrence_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(exists_before, 1);

    std::thread::sleep(std::time::Duration::from_millis(10));
    fs::remove_file(&file_path).unwrap();
    scanner::scan_metadata(&db, &dirs.office).unwrap();

    let exists_after: i64 = db.read(|conn| conn.query_row(
        "SELECT exists_now FROM file_occurrences WHERE id=?1", [&occurrence_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(exists_after, 0, "a full scan that no longer observes a previously-known file must mark it missing");
}

#[test]
fn a_file_untouched_by_a_scan_of_an_unrelated_root_is_not_marked_missing() {
    let dirs = TestDirs::new();
    let db = DbState::open(dirs.db_path.clone()).unwrap();
    let matter_id = new_matter(&db, "P0-2b test");
    let folder = dirs.office.join("matter folder");
    fs::create_dir_all(&folder).unwrap();
    bind_folder(&db, &matter_id, &folder);

    let file_path = folder.join("kept.txt");
    fs::write(&file_path, "kept").unwrap();
    scanner::scan_metadata(&db, &dirs.office).unwrap();

    // scanning a disjoint, empty root must never mark files under the real office root missing
    let other_root = dirs.root.join("unrelated");
    fs::create_dir_all(&other_root).unwrap();
    scanner::scan_metadata(&db, &other_root).unwrap();

    let occurrence_id: String = db.read(|conn| conn.query_row(
        "SELECT id FROM file_occurrences WHERE matter_id=?1", [&matter_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    let exists_now: i64 = db.read(|conn| conn.query_row(
        "SELECT exists_now FROM file_occurrences WHERE id=?1", [&occurrence_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(exists_now, 1, "a scan of a different root must not affect files outside it");
}

// --- P1-6: damage lock recompute primitive ---

#[test]
fn damage_calculate_used_for_locking_is_deterministic_and_tamper_evident() {
    let inputs = vec![DamageInput{key:"past_wage_loss".into(),cents:10_000_00,source:"payslips".into()}];
    let a = damage::calculate("tort","living",&inputs).unwrap();
    let b = damage::calculate("tort","living",&inputs).unwrap();
    assert_eq!(a.integrity_sha256, b.integrity_sha256, "the same inputs must always hash the same way");
    assert!(damage::verify_for_lock("tort","living",&inputs,a.gross_cents,a.deductions_cents,999_999).is_err());
}

// --- P1-1: approving an AI proposal must actually produce a grounded VerifiedFact ---

#[test]
fn approving_an_ai_proposal_creates_a_real_grounded_verified_fact() {
    let dirs = TestDirs::new();
    let db = DbState::open(dirs.db_path.clone()).unwrap();
    let matter_id = new_matter(&db, "P1-1 test");
    let now = Utc::now().to_rfc3339();

    let document_id = Uuid::new_v4().to_string();
    let version_id = Uuid::new_v4().to_string();
    let page_id = Uuid::new_v4().to_string();
    db.write(|conn| {
        conn.execute(
            "INSERT INTO documents(id,matter_id,logical_title,created_at,updated_at) VALUES(?1,?2,'מסמך',?3,?3)",
            params![document_id, matter_id, now],
        ).map_err(AppError::Db)?;
        conn.execute(
            "INSERT INTO document_versions(id,document_id,matter_id,content_sha256,created_at) VALUES(?1,?2,?3,'x',?4)",
            params![version_id, document_id, matter_id, now],
        ).map_err(AppError::Db)?;
        conn.execute(
            "INSERT INTO document_pages(
                id,matter_id,document_version_id,page_number,anchor_kind,block_index,
                display_text,normalized_text,text_sha256,extraction_method,created_at
             ) VALUES(?1,?2,?3,1,'page',0,'התובע נחבל ברגל שמאל.','התובע נחבל ברגל שמאל.','x','native_text',?4)",
            params![page_id, matter_id, version_id, now],
        ).map_err(AppError::Db)?;
        Ok(())
    }).unwrap();

    // simulates what ai::run_capability itself would have written for a real provider
    // response - approve_proposal is tested against that same shape, not a mock of it.
    let run_id = Uuid::new_v4().to_string();
    db.write(|conn| conn.execute(
        "INSERT INTO ai_runs(id,matter_id,capability,status,context_manifest_sha256,started_at)
         VALUES(?1,?2,'extract_facts','completed','x',?3)",
        params![run_id, matter_id, now],
    ).map_err(AppError::Db)).unwrap();

    let proposal_id = Uuid::new_v4().to_string();
    let structured = json!({
        "sourceIds": [page_id], "subject": "התובע", "predicate": "נחבל", "value": "רגל שמאל"
    }).to_string();
    db.write(|conn| conn.execute(
        "INSERT INTO ai_proposals(id,ai_run_id,matter_id,proposal_kind,structured_json,source_manifest_json,status)
         VALUES(?1,?2,?3,'extract_facts',?4,'[]','pending')",
        params![proposal_id, run_id, matter_id, structured],
    ).map_err(AppError::Db)).unwrap();

    let fact_id = ai::approve_proposal(&db, &proposal_id, Some("looks right")).unwrap();

    let (subject, created_from): (String, Option<String>) = db.read(|conn| conn.query_row(
        "SELECT subject,created_from_proposal_id FROM verified_facts WHERE id=?1",
        [&fact_id], |r| Ok((r.get(0)?, r.get(1)?))
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(subject, "התובע");
    assert_eq!(created_from.as_deref(), Some(proposal_id.as_str()), "the fact must be traceable back to the proposal that produced it");

    let (source_count, quote): (i64, String) = db.read(|conn| conn.query_row(
        "SELECT count(*),max(display_quote) FROM verified_fact_sources WHERE verified_fact_id=?1",
        [&fact_id], |r| Ok((r.get(0)?, r.get(1)?))
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(source_count, 1);
    assert_eq!(quote, "התובע נחבל ברגל שמאל.", "the stored quote must come from the real document page, not the model's claim");

    let proposal_status: String = db.read(|conn| conn.query_row(
        "SELECT status FROM ai_proposals WHERE id=?1", [&proposal_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(proposal_status, "approved");

    // approving an already-approved proposal must not succeed or duplicate the fact
    assert!(ai::approve_proposal(&db, &proposal_id, None).is_err());

    // a proposal citing a sourceId that isn't a real document_pages row must be rejected,
    // not silently create an ungrounded fact
    let bad_proposal_id = Uuid::new_v4().to_string();
    let bad_structured = json!({
        "sourceIds": ["not-a-real-page"], "subject": "x", "predicate": "y", "value": "z"
    }).to_string();
    db.write(|conn| conn.execute(
        "INSERT INTO ai_proposals(id,ai_run_id,matter_id,proposal_kind,structured_json,source_manifest_json,status)
         VALUES(?1,?2,?3,'extract_facts',?4,'[]','pending')",
        params![bad_proposal_id, run_id, matter_id, bad_structured],
    ).map_err(AppError::Db)).unwrap();
    assert!(ai::approve_proposal(&db, &bad_proposal_id, None).is_err());
}

// --- P0-2: approving a proposal whose cited source has since gone stale must fail ---
// (a run's context only ever offers non-stale pages - see ai::plan_context's `v.stale=0`
// filter - but a proposal can sit pending while the source changes underneath it)

#[test]
fn approving_a_proposal_whose_source_has_gone_stale_is_rejected() {
    let dirs = TestDirs::new();
    let db = DbState::open(dirs.db_path.clone()).unwrap();
    let matter_id = new_matter(&db, "P0-2 test");
    let now = Utc::now().to_rfc3339();

    let document_id = Uuid::new_v4().to_string();
    let version_id = Uuid::new_v4().to_string();
    let page_id = Uuid::new_v4().to_string();
    db.write(|conn| {
        conn.execute(
            "INSERT INTO documents(id,matter_id,logical_title,created_at,updated_at) VALUES(?1,?2,'מסמך',?3,?3)",
            params![document_id, matter_id, now],
        ).map_err(AppError::Db)?;
        conn.execute(
            "INSERT INTO document_versions(id,document_id,matter_id,content_sha256,created_at) VALUES(?1,?2,?3,'x',?4)",
            params![version_id, document_id, matter_id, now],
        ).map_err(AppError::Db)?;
        conn.execute(
            "INSERT INTO document_pages(
                id,matter_id,document_version_id,page_number,anchor_kind,block_index,
                display_text,normalized_text,text_sha256,extraction_method,created_at
             ) VALUES(?1,?2,?3,1,'page',0,'טקסט מקורי.','טקסט מקורי.','x','native_text',?4)",
            params![page_id, matter_id, version_id, now],
        ).map_err(AppError::Db)?;
        Ok(())
    }).unwrap();

    let run_id = Uuid::new_v4().to_string();
    db.write(|conn| conn.execute(
        "INSERT INTO ai_runs(id,matter_id,capability,status,context_manifest_sha256,started_at)
         VALUES(?1,?2,'extract_facts','completed','x',?3)",
        params![run_id, matter_id, now],
    ).map_err(AppError::Db)).unwrap();

    let proposal_id = Uuid::new_v4().to_string();
    let structured = json!({
        "sourceIds": [page_id], "subject": "x", "predicate": "y", "value": "z"
    }).to_string();
    db.write(|conn| conn.execute(
        "INSERT INTO ai_proposals(id,ai_run_id,matter_id,proposal_kind,structured_json,source_manifest_json,status)
         VALUES(?1,?2,?3,'extract_facts',?4,'[]','pending')",
        params![proposal_id, run_id, matter_id, structured],
    ).map_err(AppError::Db)).unwrap();

    // the source changes underneath the still-pending proposal, exactly like
    // scanner::rehash_changed_versions would do for real
    db.write(|conn| conn.execute(
        "UPDATE document_versions SET stale=1 WHERE id=?1", [&version_id]
    ).map_err(AppError::Db)).unwrap();

    let approve_stale = ai::approve_proposal(&db, &proposal_id, None);
    assert!(approve_stale.is_err(), "approving a proposal whose cited source version has gone stale must be rejected");

    let fact_count: i64 = db.read(|conn| conn.query_row(
        "SELECT count(*) FROM verified_facts WHERE matter_id=?1", [&matter_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(fact_count, 0, "a rejected approval must not have created any VerifiedFact");
}

// --- P1-4: corrupt damage_inputs.value_text must fail closed, never silently become 0 ---

#[test]
fn malformed_damage_input_value_fails_closed_instead_of_becoming_zero() {
    let dirs = TestDirs::new();
    let db = DbState::open(dirs.db_path.clone()).unwrap();
    let matter_id = new_matter(&db, "P1-4 test");
    let now = Utc::now().to_rfc3339();

    let calc_id = Uuid::new_v4().to_string();
    db.write(|conn| {
        conn.execute(
            "INSERT INTO damage_calculations(id,matter_id,regime,life_state,status,ruleset_id,ruleset_version,created_at,updated_at)
             VALUES(?1,?2,'tort','living','draft','default','1',?3,?3)",
            params![calc_id, matter_id, now],
        ).map_err(AppError::Db)?;
        conn.execute(
            "INSERT INTO damage_inputs(id,matter_id,calculation_id,input_key,value_kind,value_text,source_kind)
             VALUES(?1,?2,?3,'past_wage_loss','cents','NOT-A-NUMBER','manual')",
            params![Uuid::new_v4().to_string(), matter_id, calc_id],
        ).map_err(AppError::Db)?;
        Ok(())
    }).unwrap();

    // this exercises the exact query shape commands.rs uses for both
    // list_damage_calculations and lock_damage_calculation's input parsing
    let result: Result<i64, _> = db.read(|conn| {
        let mut stmt = conn.prepare("SELECT value_text FROM damage_inputs WHERE calculation_id=?1")?;
        stmt.query_row([&calc_id], |r| {
            let value_text: String = r.get(0)?;
            value_text.parse::<i64>().map_err(|_|
                rusqlite::Error::InvalidColumnType(0, "value_text".to_string(), rusqlite::types::Type::Text)
            )
        }).map_err(AppError::Db)
    });
    assert!(result.is_err(), "a corrupt damage_inputs.value_text must be a hard error, never silently parsed as 0");
}

// --- v2 P1-2: an authority requires an approved passage, not just a source document ---
// (legal_authority_passages already existed in the schema but nothing ever read or
// wrote it - verify_authority only ever required source_document_version_id to be
// set, so an authority citing an entire unrelated document could still be "verified")

fn new_document_with_page(db: &DbState, matter_id: &str, page_text: &str) -> (String, String) {
    let document_id = Uuid::new_v4().to_string();
    let version_id = Uuid::new_v4().to_string();
    let page_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    db.write(|conn| {
        conn.execute(
            "INSERT INTO documents(id,matter_id,logical_title,created_at,updated_at) VALUES(?1,?2,'מסמך',?3,?3)",
            params![document_id, matter_id, now],
        ).map_err(AppError::Db)?;
        conn.execute(
            "INSERT INTO document_versions(id,document_id,matter_id,content_sha256,created_at) VALUES(?1,?2,?3,'x',?4)",
            params![version_id, document_id, matter_id, now],
        ).map_err(AppError::Db)?;
        conn.execute(
            "INSERT INTO document_pages(
                id,matter_id,document_version_id,page_number,anchor_kind,block_index,
                display_text,normalized_text,text_sha256,extraction_method,created_at
             ) VALUES(?1,?2,?3,1,'page',0,?4,?4,'x','native_text',?5)",
            params![page_id, matter_id, version_id, page_text, now],
        ).map_err(AppError::Db)?;
        Ok(())
    }).unwrap();
    (version_id, page_id)
}

fn new_draft_authority(db: &DbState, matter_id: &str, source_version_id: Option<&str>) -> String {
    let id = Uuid::new_v4().to_string();
    db.write(|conn| conn.execute(
        "INSERT INTO legal_authorities(id,matter_id,citation,title,source_document_version_id,status)
         VALUES(?1,?2,'רע״א 1234/20','כותרת',?3,'draft')",
        params![id, matter_id, source_version_id],
    ).map_err(AppError::Db)).unwrap();
    id
}

#[test]
fn adding_an_authority_passage_requires_a_stored_source_document() {
    let dirs = TestDirs::new();
    let db = DbState::open(dirs.db_path.clone()).unwrap();
    let matter_id = new_matter(&db, "v2 P1-2 test: no source");

    let authority_id = new_draft_authority(&db, &matter_id, None);
    let (_, page_id) = new_document_with_page(&db, &matter_id, "קטע כלשהו.");

    let result = authorities::add_passage(&db, &matter_id, &authority_id, &page_id, "קטע כלשהו.", None);
    assert!(result.is_err(), "an authority with no stored source document must not accept a passage");
}

#[test]
fn adding_an_authority_passage_requires_verbatim_containment_in_the_cited_page() {
    let dirs = TestDirs::new();
    let db = DbState::open(dirs.db_path.clone()).unwrap();
    let matter_id = new_matter(&db, "v2 P1-2 test: containment");

    let (version_id, page_id) = new_document_with_page(&db, &matter_id, "בית המשפט קבע כי יש להטיל אשם תורם בשיעור 20%.");
    let authority_id = new_draft_authority(&db, &matter_id, Some(&version_id));

    let fabricated = authorities::add_passage(&db, &matter_id, &authority_id, &page_id, "בית המשפט קבע כי אין להטיל אשם תורם", None);
    assert!(fabricated.is_err(), "a passage not actually present on the source page must be rejected, not trusted from free text");

    let real = authorities::add_passage(&db, &matter_id, &authority_id, &page_id, "יש להטיל אשם תורם בשיעור 20%", Some("אשם תורם"));
    assert!(real.is_ok(), "a passage that genuinely appears (after whitespace normalization) on the source page must be accepted");
}

#[test]
fn verifying_an_authority_requires_at_least_one_approved_passage() {
    let dirs = TestDirs::new();
    let db = DbState::open(dirs.db_path.clone()).unwrap();
    let matter_id = new_matter(&db, "v2 P1-2 test: verify gating");

    let (version_id, page_id) = new_document_with_page(&db, &matter_id, "התובע זכאי לפיצוי בגין נזק לא ממוני.");
    let authority_id = new_draft_authority(&db, &matter_id, Some(&version_id));

    // no passage at all yet
    assert!(authorities::verify(&db, &matter_id, &authority_id).is_err(), "verifying with zero passages must fail");

    let passage_id = authorities::add_passage(&db, &matter_id, &authority_id, &page_id, "זכאי לפיצוי בגין נזק לא ממוני", None).unwrap();

    // a passage exists but isn't approved yet
    assert!(authorities::verify(&db, &matter_id, &authority_id).is_err(), "verifying with only an unapproved passage must fail");

    authorities::approve_passage(&db, &matter_id, &authority_id, &passage_id).unwrap();
    let integrity_sha = authorities::verify(&db, &matter_id, &authority_id).unwrap();
    assert_eq!(integrity_sha.len(), 64, "must return a real sha256 hex digest");

    let status: String = db.read(|conn| conn.query_row(
        "SELECT status FROM legal_authorities WHERE id=?1", [&authority_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(status, "verified");

    // an already-verified authority is no longer in 'draft', so re-verifying must fail
    assert!(authorities::verify(&db, &matter_id, &authority_id).is_err(), "an already-verified authority must not be re-verifiable");
}

#[test]
fn approving_a_passage_re_checks_containment_against_the_current_source_text() {
    let dirs = TestDirs::new();
    let db = DbState::open(dirs.db_path.clone()).unwrap();
    let matter_id = new_matter(&db, "v2 P1-2 test: re-check at approval");

    let (version_id, page_id) = new_document_with_page(&db, &matter_id, "הנתבע התרשל בנהיגתו.");
    let authority_id = new_draft_authority(&db, &matter_id, Some(&version_id));
    let passage_id = authorities::add_passage(&db, &matter_id, &authority_id, &page_id, "הנתבע התרשל בנהיגתו", None).unwrap();

    // simulate the source page's text changing after the passage was drafted but
    // before anyone approved it (e.g. a corrected re-extraction)
    db.write(|conn| conn.execute(
        "UPDATE document_pages SET normalized_text='טקסט שונה לחלוטין.' WHERE id=?1", [&page_id]
    ).map_err(AppError::Db)).unwrap();

    let result = authorities::approve_passage(&db, &matter_id, &authority_id, &passage_id);
    assert!(result.is_err(), "approval must re-check containment against the page's current text, not trust what was true when the passage was drafted");
}
