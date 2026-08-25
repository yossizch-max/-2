//! A real, executable subset of the Gate F (docs/RELEASE_GATES.md) manual acceptance
//! checklist, run against the actual business-logic modules (not mocks). This test
//! itself is cross-platform (it runs for real in the Windows Release Gate CI job, not
//! only in a Linux dev sandbox), so step 21/22's keyring assertions branch on whatever
//! that platform's keyring actually does rather than assuming one specific behavior -
//! see the comment at that assertion. This is still not a substitute for the full
//! 24-step Gate F: some steps fundamentally require real Tesseract/Poppler .exe
//! binaries actually executing, a live AI provider, or a human clicking through the
//! GUI. What's covered here is everything that is pure Rust logic shared by every
//! platform, exercised through `tauri::State`-free calls into the same modules the
//! Tauri commands in `commands.rs` are thin wrappers over.
//!
//! Coverage vs. docs/RELEASE_GATES.md Gate F steps:
//!  1. Create/open Matter                                    -> covered
//!  2. Scan/index files                                       -> covered (scanner::scan_metadata)
//!  3. Change source after hash, extraction refuses it         -> covered (SourceShaMismatch)
//!  7. Extract native (non-PDF) text                           -> covered (.txt path; real PDF
//!                                                                needs poppler pdftotext.exe,
//!                                                                a Windows binary)
//!  8. Search text and open source                             -> covered (search::search,
//!                                                                including full-text search
//!                                                                over extracted document_pages
//!                                                                content, not just metadata)
//! 12. Approve a fact (direct verified-fact commit)             -> covered
//! 13. Change source, prove stale propagation is at least detectable -> partially covered
//!     (re-extraction after a source change is rejected the same way as step 3; the
//!     `document_versions.stale` flag itself is model-only, never set by any command in
//!     this reconstruction - flagged as a real product gap below, not silently worked around)
//! 14. Create and lock damage calculation                      -> covered
//! 15. Create legal draft                                      -> covered
//! 16. Start a new draft version from an approved legal document -> covered
//!     (legal_docs::create_new_version: deep-copies sections/paragraphs/sources, the
//!     prior approved version stays immutable, the parent document flips back to draft)
//! 18. Approve immutable version                                -> covered
//! 19. Export (txt)                                             -> covered
//! 21. Close/reopen and verify audit                            -> covered
//! 22. Missing key recovery                                     -> covered, adaptively
//!     (see the platform-dependent keyring comment at the step 21/22 assertion)
//! 24. No client-content temp files remain                      -> covered
//!
//! Not covered, and why:
//!  4-6. OCR Hebrew/Arabic/English scanned PDF - needs the real Tesseract/Poppler
//!       Windows .exe binaries (vendored in Gate C) actually executing; this Linux
//!       container has no Windows loader.
//!  9-11. AI provider configuration/review - needs a real network call to a real
//!        local or OpenAI-compatible endpoint.
//! 17. Paragraph provenance review is a frontend/UI concern layered on the
//!     `provenance_state` column that `approve_version` already gates on (a version
//!     with any non-'confirmed' paragraph cannot be approved); not independently
//!     re-tested here.
//! 20. PDF export returning PDF_CONVERTER_UNAVAILABLE - the guard
//!     (`if output_kind!="txt" { return Err(PdfConverterUnavailable) }`) lives directly
//!     in `commands::export_legal_document`, which takes a `tauri::State` with no public
//!     constructor outside a running Tauri app, so it can't be called from a plain test.
//!     Verified by direct code reading instead (commands.rs, `export_legal_document`).
//! 23. Upgrade from a supported older DB - there is no earlier real DB to upgrade from
//!     for a fresh reconstruction; not applicable.

#![cfg(test)]

use crate::{damage, db::DbState, error::AppError, extraction, legal_docs, models::DamageInput, scanner, search};
use chrono::Utc;
use rusqlite::params;
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};
use uuid::Uuid;

struct TestDirs {
    root: PathBuf,
    office: PathBuf,
    db_path: PathBuf,
}

impl TestDirs {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("tahrir-gatef-{}", Uuid::new_v4()));
        let office = root.join("office");
        fs::create_dir_all(&office).unwrap();
        let db_path = root.join("tahrir.db");
        Self { root, office, db_path }
    }
}

impl Drop for TestDirs {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn gate_f_partial_real_flow() {
    let dirs = TestDirs::new();
    let db = DbState::open(dirs.db_path.clone()).expect("db open (real keyring + real migration)");

    // --- Step 1: Create Matter ---
    let matter_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    db.write(|conn| {
        conn.execute(
            "INSERT INTO matters(id,title,internal_number,matter_type,status,workflow_stage,created_at,updated_at)
             VALUES(?1,'כהן נ׳ כלל - Gate F test','GF-1','personal_injury','active','intake',?2,?2)",
            params![matter_id, now],
        )?;
        Ok(())
    }).unwrap();

    let matter_folder = dirs.office.join("GF-1 matter folder");
    fs::create_dir_all(&matter_folder).unwrap();
    let path_key = matter_folder.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase();
    db.write(|conn| {
        conn.execute(
            "INSERT INTO matter_folder_bindings(id,matter_id,path_display,path_key,binding_source,active,last_seen_at)
             VALUES(?1,?2,?3,?4,'test',1,?5)",
            params![Uuid::new_v4().to_string(), matter_id, matter_folder.to_string_lossy(), path_key, now],
        )?;
        Ok(())
    }).unwrap();

    // --- Step 2: Scan/index files ---
    let original_text = "התובע אושפז בבית החולים הדסה עין כרם למשך שלושה ימים בעקבות התאונה.";
    let doc_path = matter_folder.join("medical_record.txt");
    fs::write(&doc_path, original_text).unwrap();

    let run_id = scanner::scan_metadata(&db, &dirs.office).unwrap();
    assert!(!run_id.is_empty());
    let hashed = scanner::hash_pending(&db, &matter_id).unwrap();
    assert_eq!(hashed, 1, "exactly one new file should have been hashed into a Document");

    let document_id: String = db.read(|conn| conn.query_row(
        "SELECT id FROM documents WHERE matter_id=?1", [&matter_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();

    // --- Step 3: change source after hash, extraction must refuse it (fail closed) ---
    fs::write(&doc_path, "TAMPERED CONTENT - must never be silently accepted").unwrap();
    let resource_root = dirs.root.join("resources"); // unused by the .txt extraction path
    let tampered_result = extraction::extract_document(&db, &document_id, &resource_root);
    assert!(
        matches!(tampered_result, Err(AppError::SourceShaMismatch)),
        "extraction must fail closed on a source that changed after hashing, got {:?}", tampered_result
    );
    // no document_pages should have been written for the rejected attempt
    let pages_after_reject: i64 = db.read(|conn| conn.query_row(
        "SELECT count(*) FROM document_pages WHERE document_version_id IN
         (SELECT id FROM document_versions WHERE document_id=?1)",
        [&document_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(pages_after_reject, 0, "a rejected extraction must not create any document_pages");

    // restore original bytes and confirm extraction now succeeds
    fs::write(&doc_path, original_text).unwrap();

    // --- Step 7 (native, non-PDF proxy): extract text ---
    let block_count = extraction::extract_document(&db, &document_id, &resource_root).unwrap();
    assert_eq!(block_count, 1);

    let (page_id, version_id, text_sha): (String, String, String) = db.read(|conn| conn.query_row(
        "SELECT p.id,p.document_version_id,p.text_sha256 FROM document_pages p
         JOIN document_versions v ON v.id=p.document_version_id WHERE v.document_id=?1",
        [&document_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    ).map_err(AppError::Db)).unwrap();

    // --- Step 8: search text and locate the source ---
    let matter_hits = search::search(&db, "GF-1").unwrap();
    assert!(matter_hits.iter().any(|h| h.kind == "matter"), "matter should be findable by internal_number");
    let file_hits = search::search(&db, "medical_record").unwrap();
    assert!(file_hits.iter().any(|h| h.kind == "file"), "file should be findable by file name");
    // full-text search over the extracted document_pages content itself (not just
    // filename/matter/fact metadata) - "התאונה" only occurs in the extracted page text.
    let page_hits = search::search(&db, "התאונה").unwrap();
    assert!(
        page_hits.iter().any(|h| h.kind == "document_page" && h.id == document_id),
        "extracted page text should be full-text searchable and resolve back to its document"
    );

    // --- Step 12 (direct commit half): verify a fact grounded in the extracted page ---
    let fact_id = Uuid::new_v4().to_string();
    let source_id = Uuid::new_v4().to_string();
    db.write(|conn| {
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO verified_facts(id,matter_id,subject,predicate,value_text,status,verified_at)
             VALUES(?1,?2,'התובע','אושפז','הדסה עין כרם, 3 ימים','valid',?3)",
            params![fact_id, matter_id, now],
        )?;
        tx.execute(
            "INSERT INTO verified_fact_sources(
                id,matter_id,verified_fact_id,document_version_id,document_page_id,
                display_quote,normalized_quote,source_text_sha256
             ) VALUES(?1,?2,?3,?4,?5,?6,?6,?7)",
            params![source_id, matter_id, fact_id, version_id, page_id, original_text, text_sha],
        )?;
        tx.commit()?;
        Ok(())
    }).unwrap();

    let fact_hits = search::search(&db, "הדסה").unwrap();
    assert!(fact_hits.iter().any(|h| h.kind == "verified_fact"), "verified fact should be findable by its content");

    // --- Step 14: create and lock a damage calculation ---
    let inputs = vec![
        DamageInput { key: "past_wage_loss".into(), cents: 32_400_00, source: "payslips".into() },
        DamageInput { key: "pain_suffering".into(), cents: 8_000_00, source: "lawyer_input".into() },
    ];
    let calc = damage::calculate("tort", "living", &inputs).unwrap();
    assert_eq!(calc.net_cents, 32_400_00 + 8_000_00);

    let calc_id = Uuid::new_v4().to_string();
    db.write(|conn| {
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO damage_calculations(
                id,matter_id,regime,life_state,status,gross_cents,deductions_cents,net_cents,
                ruleset_id,ruleset_version,created_at,updated_at
             ) VALUES(?1,?2,'tort','living','draft',?3,?4,?5,'default','1',?6,?6)",
            params![calc_id, matter_id, calc.gross_cents, calc.deductions_cents, calc.net_cents, now],
        )?;
        for i in &inputs {
            tx.execute(
                "INSERT INTO damage_inputs(id,matter_id,calculation_id,input_key,value_kind,value_text,source_kind)
                 VALUES(?1,?2,?3,?4,'cents',?5,?6)",
                params![Uuid::new_v4().to_string(), matter_id, calc_id, i.key, i.cents.to_string(), i.source],
            )?;
        }
        tx.commit()?;
        Ok(())
    }).unwrap();

    db.write(|conn| {
        let changed = conn.execute(
            "UPDATE damage_calculations SET status='locked',integrity_sha256=?2,locked_at=?3,updated_at=?3
             WHERE id=?1 AND status='draft'",
            params![calc_id, calc.integrity_sha256, Utc::now().to_rfc3339()],
        )?;
        assert_eq!(changed, 1);
        Ok(())
    }).unwrap();

    // a locked calculation must reject further mutation (schema trigger, not app code)
    let mutate_locked = db.write(|conn| {
        conn.execute("UPDATE damage_calculations SET status='draft' WHERE id=?1", [&calc_id])
            .map_err(AppError::Db)
    });
    assert!(mutate_locked.is_err(), "trg_locked_calc_no_update must block mutating a locked calculation");

    // --- Step 15: create legal draft ---
    let legal_doc_id = legal_docs::create_draft(&db, &matter_id, "מכתב דרישה", "demand").unwrap();
    let legal_version_id: String = db.read(|conn| conn.query_row(
        "SELECT current_version_id FROM legal_documents WHERE id=?1", [&legal_doc_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();

    // create_draft seeds a fixed template of sections (per document kind), ending in
    // the FACTS_SECTION_HEADING section - confirm the template landed for real.
    let seeded_section_count: i64 = db.read(|conn| conn.query_row(
        "SELECT count(*) FROM legal_document_sections WHERE legal_document_version_id=?1",
        [&legal_version_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    assert!(seeded_section_count >= 2, "create_draft should seed a real section template, not an empty document");

    // Auto-fill the draft from the matter's verified facts (the "מילוי אוטומטי מעובדות
    // מאומתות" feature) rather than hand-inserting a paragraph - this exercises the
    // real fill_from_verified_facts path end to end, including its idempotency.
    let added_first_pass = legal_docs::fill_from_verified_facts(&db, &matter_id, &legal_version_id).unwrap();
    assert_eq!(added_first_pass, 1, "the one verified fact created above should be auto-filled in");
    let added_second_pass = legal_docs::fill_from_verified_facts(&db, &matter_id, &legal_version_id).unwrap();
    assert_eq!(added_second_pass, 0, "re-running the fill must not duplicate an already-linked fact");

    let (facts_section_id, paragraph_id): (String, String) = db.read(|conn| conn.query_row(
        "SELECT s.id,p.id FROM legal_document_sections s
         JOIN legal_document_paragraphs p ON p.section_id=s.id
         WHERE s.legal_document_version_id=?1 AND s.heading=?2",
        params![legal_version_id, legal_docs::FACTS_SECTION_HEADING], |r| Ok((r.get(0)?, r.get(1)?))
    ).map_err(AppError::Db)).unwrap();
    let auto_filled_body: String = db.read(|conn| conn.query_row(
        "SELECT body_text FROM legal_document_paragraphs WHERE id=?1", [&paragraph_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    assert!(auto_filled_body.contains("הדסה"), "auto-filled paragraph should carry the fact's actual content");
    assert_eq!(
        db.read(|conn| conn.query_row(
            "SELECT provenance_state FROM legal_document_paragraphs WHERE id=?1", [&paragraph_id], |r| r.get::<_, String>(0)
        ).map_err(AppError::Db)).unwrap(),
        "confirmed",
        "a fact-grounded auto-filled paragraph should already be confirmed, not pending review"
    );

    // Manual paragraph editing: add a free-text paragraph, edit it (which must drop it
    // back to needs_review), confirm it, then delete it - the real add/update/confirm/
    // delete-paragraph commands' underlying logic, not just the auto-fill path.
    let manual_paragraph_id = legal_docs::add_paragraph(
        &db, &matter_id, &legal_version_id, &facts_section_id, "טיוטה ראשונית לפסקה."
    ).unwrap();
    assert_eq!(
        db.read(|conn| conn.query_row(
            "SELECT provenance_state FROM legal_document_paragraphs WHERE id=?1", [&manual_paragraph_id], |r| r.get::<_, String>(0)
        ).map_err(AppError::Db)).unwrap(),
        "needs_review",
        "a freshly added manual paragraph must not be pre-confirmed"
    );
    legal_docs::update_paragraph(&db, &matter_id, &legal_version_id, &manual_paragraph_id, "נוסח מתוקן של הפסקה.").unwrap();
    legal_docs::confirm_paragraph(&db, &matter_id, &legal_version_id, &manual_paragraph_id).unwrap();
    assert_eq!(
        db.read(|conn| conn.query_row(
            "SELECT body_text,provenance_state FROM legal_document_paragraphs WHERE id=?1",
            [&manual_paragraph_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        ).map_err(AppError::Db)).unwrap(),
        ("נוסח מתוקן של הפסקה.".to_string(), "confirmed".to_string())
    );
    legal_docs::delete_paragraph(&db, &matter_id, &legal_version_id, &manual_paragraph_id).unwrap();
    let manual_paragraph_gone: i64 = db.read(|conn| conn.query_row(
        "SELECT count(*) FROM legal_document_paragraphs WHERE id=?1", [&manual_paragraph_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(manual_paragraph_gone, 0);

    // --- Step 18: approve immutable version ---
    let approval_sha = legal_docs::approve_version(&db, &matter_id, &legal_version_id).unwrap();
    assert_eq!(approval_sha.len(), 64, "approval hash should be a hex sha256");

    // approved version must reject further mutation (schema trigger)
    let mutate_approved = db.write(|conn| {
        conn.execute("UPDATE legal_document_versions SET status='draft' WHERE id=?1", [&legal_version_id])
            .map_err(AppError::Db)
    });
    assert!(mutate_approved.is_err(), "trg_approved_legal_version_no_update must block mutating an approved version");

    let status_after_approval: String = db.read(|conn| conn.query_row(
        "SELECT status FROM legal_documents WHERE id=?1", [&legal_doc_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(status_after_approval, "approved", "approving the current version should mark the parent document approved");

    // --- Step 16: start a new draft version from the approved one ---
    // an already-draft document must refuse a second "new version" (nothing approved yet)
    let second_draft_id = legal_docs::create_draft(&db, &matter_id, "טיוטה נוספת", "claim").unwrap();
    let reject_non_approved = legal_docs::create_new_version(&db, &matter_id, &second_draft_id);
    assert!(reject_non_approved.is_err(), "create_new_version must refuse a document whose current version is not approved");

    let new_version_id = legal_docs::create_new_version(&db, &matter_id, &legal_doc_id).unwrap();
    assert_ne!(new_version_id, legal_version_id);

    let (new_status, new_version_number, parent_version_id): (String, i64, Option<String>) = db.read(|conn| conn.query_row(
        "SELECT status,version_number,parent_version_id FROM legal_document_versions WHERE id=?1",
        [&new_version_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(new_status, "draft");
    assert_eq!(new_version_number, 2);
    assert_eq!(parent_version_id.as_deref(), Some(legal_version_id.as_str()));

    let (doc_current_version, doc_status): (String, String) = db.read(|conn| conn.query_row(
        "SELECT current_version_id,status FROM legal_documents WHERE id=?1", [&legal_doc_id], |r| Ok((r.get(0)?, r.get(1)?))
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(doc_current_version, new_version_id, "the document should now point at the new draft version");
    assert_eq!(doc_status, "draft", "starting a new version should flip the parent document back to draft");

    // the paragraph, its confirmed provenance state and its source grounding must all
    // have been deep-copied onto the new version, not merely referenced
    let (copied_paragraph_count, copied_body, copied_provenance): (i64, String, String) = db.read(|conn| conn.query_row(
        "SELECT count(*),max(body_text),max(provenance_state) FROM legal_document_paragraphs
         WHERE legal_document_version_id=?1",
        [&new_version_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(copied_paragraph_count, 1);
    assert!(copied_body.contains("הדסה"));
    assert_eq!(copied_provenance, "confirmed");
    let copied_source_count: i64 = db.read(|conn| conn.query_row(
        "SELECT count(*) FROM legal_document_sources WHERE legal_document_version_id=?1",
        [&new_version_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(copied_source_count, 1, "the paragraph's source grounding must be copied onto the new version too");

    // the original approved version and its paragraph must be untouched
    let original_paragraph_count: i64 = db.read(|conn| conn.query_row(
        "SELECT count(*) FROM legal_document_paragraphs WHERE legal_document_version_id=?1",
        [&legal_version_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(original_paragraph_count, 1, "the prior approved version's paragraphs must be untouched by the copy");
    let original_status_unchanged: String = db.read(|conn| conn.query_row(
        "SELECT status FROM legal_document_versions WHERE id=?1", [&legal_version_id], |r| r.get(0)
    ).map_err(AppError::Db)).unwrap();
    assert_eq!(original_status_unchanged, "approved", "the prior approved version must remain approved and immutable");

    // --- Step 19: export (txt) -- replicates commands::export_legal_document's txt path ---
    let content: String = db.read(|conn| {
        let mut stmt = conn.prepare(
            "SELECT p.body_text FROM legal_document_paragraphs p
             JOIN legal_document_sections s ON s.id=p.section_id
             WHERE p.legal_document_version_id=?1
             ORDER BY s.section_index,p.paragraph_index"
        )?;
        let paragraphs = stmt.query_map([&legal_version_id], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(paragraphs.join("\n\n"))
    }).unwrap();
    assert!(content.contains("הדסה"));

    let export_path = dirs.root.join("export.txt");
    fs::write(&export_path, &content).unwrap();
    let output_sha256 = hex::encode(Sha256::digest(content.as_bytes()));
    let export_id = Uuid::new_v4().to_string();
    db.write(|conn| {
        conn.execute(
            "INSERT INTO legal_export_audit(
                id,matter_id,legal_document_version_id,output_kind,output_path,output_sha256,exported_at,converter_kind
             ) VALUES(?1,?2,?3,'txt',?4,?5,?6,'native_text')",
            params![export_id, matter_id, legal_version_id, export_path.to_string_lossy(), output_sha256, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }).unwrap();
    assert_eq!(fs::read_to_string(&export_path).unwrap(), content);

    // export audit is append-only (schema trigger)
    let mutate_audit = db.write(|conn| {
        conn.execute("UPDATE legal_export_audit SET output_kind='pdf' WHERE id=?1", [&export_id])
            .map_err(AppError::Db)
    });
    assert!(mutate_audit.is_err(), "trg_export_audit_no_update must block mutating export audit rows");

    // --- Step 21: close/reopen and verify audit persisted ---
    // This step has two genuinely separate concerns, tested separately:
    //  (a) does the OS keyring persist the DB encryption key across app restarts?
    //  (b) does the actual matter/export/damage data persist correctly once reopened?
    // (a) is genuinely environment-dependent, not just a CI quirk to work around: a
    // Linux dev sandbox's keyring backend may not persist entries across separate
    // `keyring::Entry` instances (confirmed directly in one such sandbox: set-then-get
    // in a fresh Entry failed with "No matching entry found in secure storage"), while
    // the real target - Windows' OS Credential Manager - does persist. Both outcomes
    // are valid and are handled correctly here: if the key can't be retrieved,
    // `DbState::open` must fail closed with `RecoveryRequired` rather than proceeding
    // with no key; if it CAN be retrieved (as on real Windows), the reopened DbState
    // must actually work and see the real data, not just "not error."
    let key = db.test_key().to_string();
    match DbState::open(dirs.db_path.clone()) {
        Err(AppError::RecoveryRequired) => {}
        Ok(reopened) => {
            let title: String = reopened.read(|conn| conn.query_row(
                "SELECT title FROM matters WHERE id=?1", [&matter_id], |r| r.get(0)
            ).map_err(AppError::Db)).unwrap();
            assert_eq!(title, "כהן נ׳ כלל - Gate F test", "a keyring that persists the key must reopen onto the real data");
        }
        Err(other) => panic!("reopening an existing DB must either succeed or fail closed with RecoveryRequired, got {other:?}"),
    }
    drop(db);

    // (b) data persistence itself, independent of the keyring: reopen the same
    // encrypted file directly with the key we already had (standing in for "the OS
    // keyring successfully returned the same key it was given at creation", which is
    // the real-world Windows behavior this sandbox can't reproduce).
    let conn2 = DbState::open_keyed(&dirs.db_path, &key).expect("raw reopen with the real key must succeed");
    let title: String = conn2.query_row("SELECT title FROM matters WHERE id=?1", [&matter_id], |r| r.get(0)).unwrap();
    let export_count: i64 = conn2.query_row(
        "SELECT count(*) FROM legal_export_audit WHERE matter_id=?1", [&matter_id], |r| r.get(0)
    ).unwrap();
    let calc_status: String = conn2.query_row(
        "SELECT status FROM damage_calculations WHERE id=?1", [&calc_id], |r| r.get(0)
    ).unwrap();
    assert_eq!(title, "כהן נ׳ כלל - Gate F test");
    assert_eq!(export_count, 1);
    assert_eq!(calc_status, "locked");

    // --- Step 24: confirm no client-content temp files remain ---
    let snapshot_dir = std::env::temp_dir().join("tahrir").join("source-snapshots");
    if snapshot_dir.exists() {
        let leftover: Vec<_> = fs::read_dir(&snapshot_dir).unwrap()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .collect();
        assert!(leftover.is_empty(), "VerifiedSourceSnapshot must clean up on Drop, found leftovers: {leftover:?}");
    }
}
