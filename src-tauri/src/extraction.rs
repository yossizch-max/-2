use crate::{
    db::DbState,
    error::{AppError, AppResult},
    source_snapshot::VerifiedSourceSnapshot,
};
use chrono::Utc;
use quick_xml::{events::Event, Reader};
use rusqlite::params;
use sha2::{Digest, Sha256};
use std::{
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

/// Guards an OCR scratch directory: removed on drop regardless of how the
/// enclosing function exits (early `?` propagation included), so a failed
/// pdftoppm/tesseract invocation can no longer leak rasterized page images.
struct OcrTempDir {
    path: PathBuf,
}

impl OcrTempDir {
    fn create() -> AppResult<Self> {
        let path = std::env::temp_dir().join("tahrir").join(format!("ocr-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }
}

impl Drop for OcrTempDir {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.path); }
}

struct Candidate {
    matter_id: String,
    document_version_id: String,
    path: String,
    source_sha256: String,
}

struct ExtractedBlock {
    page_number: Option<i64>,
    anchor_kind: &'static str,
    block_index: i64,
    display_text: String,
    extraction_method: &'static str,
    extraction_confidence: Option<f64>,
}

/// Stable, distinguishable error codes stored in `extraction_runs.error_code` and
/// surfaced per-document in `intake::process_matter_documents`'s batch result - never
/// a generic "failed" when the real cause is known. Deliberately a closed, small set
/// matching exactly what the pipeline can actually tell apart today; anything not
/// mapped explicitly falls to "failed" honestly rather than guessing a category.
pub(crate) fn extraction_error_code(e: &AppError) -> &'static str {
    match e {
        AppError::OcrRuntimeMissing => "runtime_missing",
        AppError::UnsupportedFormat(_) => "unsupported_format",
        AppError::PdftotextFailed(_) => "pdftotext_failed",
        AppError::RasterizationFailed(_) => "rasterization_failed",
        AppError::OcrFailed(_) => "ocr_failed",
        AppError::SourceShaMismatch | AppError::SourceSnapshotChanged => "source_changed",
        AppError::Db(_) | AppError::Io(_) => "persistence_failed",
        _ => "failed",
    }
}

/// Convenience wrapper kept for the existing single-document `extract_document_text`
/// IPC command: resolves the document's current (most recent) version, then delegates
/// to `extract_document_version` - the same core the batch intake pipeline
/// (`intake::process_matter_documents`) uses, so there is exactly one extraction code
/// path and exactly one place `extraction_runs` rows get written.
pub fn extract_document(db: &DbState, document_id: &str, resource_root: &Path) -> AppResult<usize> {
    let candidate: Candidate = db.read(|conn| {
        conn.query_row(
            "SELECT v.matter_id,v.id,o.path_display,v.content_sha256
             FROM document_versions v
             JOIN file_occurrences o ON o.document_version_id=v.id
             WHERE v.document_id=?1
             ORDER BY v.created_at DESC LIMIT 1",
            [document_id],
            |r| Ok(Candidate {
                matter_id: r.get(0)?,
                document_version_id: r.get(1)?,
                path: r.get(2)?,
                source_sha256: r.get(3)?,
            }),
        ).map_err(AppError::Db)
    })?;
    extract_document_version(db, &candidate.matter_id, &candidate.document_version_id, &candidate.path, &candidate.source_sha256, resource_root)
}

/// The one real extraction code path, version-scoped so a batch caller (which already
/// has its own candidate list) never needs to re-derive it from a document_id. Writes
/// a genuine `extraction_runs` audit row for every attempt: inserted as `running`
/// before any file I/O or external process starts, updated to `completed`/`failed`
/// (with `error_code` and `finished_at`) once the attempt is fully resolved. The
/// audit row's own finishing write is best-effort - if extraction itself already
/// succeeded or failed, that real outcome is what gets returned to the caller
/// regardless of whether the audit UPDATE itself could be persisted; a `running` row
/// that group cannot be resolved is still an honest state (it says exactly what is
/// known: an attempt started and the pipeline cannot confirm how it ended) - never
/// silently rewritten to look like a success.
pub fn extract_document_version(
    db: &DbState, matter_id: &str, document_version_id: &str, path: &str, source_sha256: &str, resource_root: &Path,
) -> AppResult<usize> {
    let run_id = Uuid::new_v4().to_string();
    let started_at = Utc::now().to_rfc3339();
    db.write(|conn| conn.execute(
        "INSERT INTO extraction_runs(id,matter_id,document_version_id,source_sha256,status,started_at) VALUES(?1,?2,?3,?4,'running',?5)",
        params![run_id, matter_id, document_version_id, source_sha256, started_at],
    ).map_err(AppError::Db))?;

    let result = extract_document_version_inner(db, matter_id, document_version_id, path, resource_root);

    let finished_at = Utc::now().to_rfc3339();
    let _ = match &result {
        Ok(_) => db.write(|conn| {
            conn.execute(
                "UPDATE extraction_runs SET status='completed',finished_at=?2 WHERE id=?1",
                params![run_id, finished_at],
            )?;
            Ok(())
        }),
        // Both the audit row and the version's own extraction_state move together, in
        // one short transaction, so a failed attempt is visible two ways: the audit
        // trail (extraction_runs, with a real error_code) and the version's own
        // extraction_state='failed' - which is what list_documents/DocumentsTab.tsx
        // already reads, so a failure shows up immediately without a join. A retry is
        // just calling extract_document_version again on the same document_version_id
        // - a fresh run_id, a fresh extraction_runs row - never rewriting this one.
        Err(e) => db.write(|conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "UPDATE extraction_runs SET status='failed',error_code=?2,finished_at=?3 WHERE id=?1",
                params![run_id, extraction_error_code(e), finished_at],
            )?;
            tx.execute(
                "UPDATE document_versions SET extraction_state='failed' WHERE id=?1",
                [document_version_id],
            )?;
            tx.commit()?;
            Ok(())
        }),
    };

    result
}

fn extract_document_version_inner(
    db: &DbState, matter_id: &str, document_version_id: &str, path: &str, resource_root: &Path,
) -> AppResult<usize> {
    let source_sha256: String = db.read(|conn| conn.query_row(
        "SELECT content_sha256 FROM document_versions WHERE id=?1", [document_version_id], |r| r.get(0),
    ).map_err(AppError::Db))?;
    let snapshot = VerifiedSourceSnapshot::create(Path::new(path), &source_sha256)?;
    let extension = Path::new(path)
        .extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();

    let blocks = match extension.as_str() {
        "pdf" => extract_pdf(snapshot.path(), resource_root)?,
        "docx" => extract_docx(snapshot.path())?,
        "txt" => vec![ExtractedBlock {
            page_number: None, anchor_kind: "document", block_index: 0,
            display_text: std::fs::read_to_string(snapshot.path())?,
            extraction_method: "native_text", extraction_confidence: None,
        }],
        other => return Err(AppError::UnsupportedFormat(other.to_string())),
    };

    snapshot.verify_unchanged()?;

    db.write(|conn| {
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM document_pages WHERE document_version_id=?1", [document_version_id])?;
        for block in &blocks {
            let normalized = normalize_source_text(&block.display_text);
            let text_sha = hex::encode(Sha256::digest(normalized.as_bytes()));
            tx.execute(
                "INSERT INTO document_pages(
                    id,matter_id,document_version_id,page_number,anchor_kind,block_index,
                    display_text,normalized_text,text_sha256,extraction_method,extraction_confidence,created_at
                 ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
                params![
                    Uuid::new_v4().to_string(), matter_id, document_version_id,
                    block.page_number, block.anchor_kind, block.block_index, block.display_text,
                    normalized, text_sha, block.extraction_method, block.extraction_confidence, Utc::now().to_rfc3339()
                ],
            )?;
        }
        tx.execute(
            "UPDATE document_versions
             SET extraction_state='complete', extractor_version='alpha16.1-reconstructed'
             WHERE id=?1",
            [document_version_id],
        )?;
        tx.commit()?;
        Ok(())
    })?;

    Ok(blocks.len())
}

fn extract_pdf(path: &Path, resource_root: &Path) -> AppResult<Vec<ExtractedBlock>> {
    let pdftotext = resource_root.join("ocr").join("vendor").join("poppler").join("pdftotext.exe");
    if !pdftotext.exists() { return Err(AppError::OcrRuntimeMissing); }

    let output = Command::new(&pdftotext)
        .args(["-layout", "-enc", "UTF-8"])
        .arg(path).arg("-").output()
        .map_err(|e| AppError::PdftotextFailed(e.to_string()))?;

    if output.status.success() {
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        if text.trim().chars().count() > 40 {
            return Ok(text.split('\x0c').enumerate()
                .filter(|(_, page)| !page.trim().is_empty())
                .map(|(index, page)| ExtractedBlock {
                    page_number: Some((index + 1) as i64),
                    anchor_kind: "page",
                    block_index: 0,
                    display_text: page.to_string(),
                    extraction_method: "pdftotext",
                    extraction_confidence: None,
                }).collect());
        }
    }

    extract_scanned_pdf(path, resource_root)
}

fn extract_scanned_pdf(path: &Path, resource_root: &Path) -> AppResult<Vec<ExtractedBlock>> {
    let poppler = resource_root.join("ocr").join("vendor").join("poppler");
    let tesseract_root = resource_root.join("ocr").join("vendor").join("tesseract");
    let pdftoppm = poppler.join("pdftoppm.exe");
    let tesseract = tesseract_root.join("tesseract.exe");
    let tessdata = resource_root.join("ocr").join("tessdata");
    if !pdftoppm.exists() || !tesseract.exists()
        || !tessdata.join("heb.traineddata").exists()
        || !tessdata.join("ara.traineddata").exists()
        || !tessdata.join("eng.traineddata").exists() {
        return Err(AppError::OcrRuntimeMissing);
    }

    let temp = OcrTempDir::create()?;
    let prefix = temp.path.join("page");

    let raster = Command::new(pdftoppm)
        .args(["-png", "-r", "220"])
        .arg(path).arg(&prefix).output()
        .map_err(|e| AppError::RasterizationFailed(e.to_string()))?;
    if !raster.status.success() {
        return Err(AppError::RasterizationFailed(String::from_utf8_lossy(&raster.stderr).to_string()));
    }

    let mut images: Vec<PathBuf> = std::fs::read_dir(&temp.path)?
        .filter_map(Result::ok).map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("png"))
        .collect();
    images.sort();

    let mut blocks = Vec::new();
    for (index, image) in images.iter().enumerate() {
        let output = Command::new(&tesseract)
            .arg(image).arg("stdout")
            .args(["-l", "heb+ara+eng", "--tessdata-dir"])
            .arg(&tessdata)
            .output()
            .map_err(|e| AppError::OcrFailed(format!("page {}: {e}", index + 1)))?;
        if !output.status.success() {
            return Err(AppError::OcrFailed(format!("page {}: {}", index + 1, String::from_utf8_lossy(&output.stderr))));
        }
        blocks.push(ExtractedBlock {
            page_number: Some((index + 1) as i64),
            anchor_kind: "page",
            block_index: 0,
            display_text: String::from_utf8_lossy(&output.stdout).to_string(),
            extraction_method: "tesseract",
            // Confidence: investigated (see docs/RELEASE_GATES.md's C1 writeup) whether
            // `tesseract --tsv`/`hocr` mean-confidence output could populate this
            // reliably. It cannot, without a real rewrite: this call reads stdout as
            // plain UTF-8 text, and a TSV/hOCR run is a *different* output mode with
            // its own per-word/per-line parsing surface, not a number obtainable from
            // the same invocation. Left NULL rather than inventing one - a fabricated
            // confidence value would be worse than an honestly-absent one.
            extraction_confidence: None,
        });
    }
    Ok(blocks)
}

fn extract_docx(path: &Path) -> AppResult<Vec<ExtractedBlock>> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| AppError::Validation(format!("DOCX zip: {e}")))?;
    let mut xml = String::new();
    archive.by_name("word/document.xml")
        .map_err(|e| AppError::Validation(format!("DOCX document.xml: {e}")))?
        .read_to_string(&mut xml)?;

    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(false);
    let mut text = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Text(t)) => {
                let decoded=t.decode().map_err(|e|AppError::Validation(e.to_string()))?;
                let unescaped=quick_xml::escape::unescape(&decoded).map_err(|e|AppError::Validation(e.to_string()))?;
                text.push_str(&unescaped);
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"w:p" => text.push('\n'),
            Ok(Event::Eof) => break,
            Err(e) => return Err(AppError::Validation(format!("DOCX XML: {e}"))),
            _ => {}
        }
    }

    Ok(vec![ExtractedBlock {
        page_number: None,
        anchor_kind: "document",
        block_index: 0,
        display_text: text,
        extraction_method: "docx_xml",
        extraction_confidence: None,
    }])
}

pub fn normalize_source_text(input: &str) -> String {
    input.nfc()
        .filter(|c| !matches!(*c,
            '\u{200E}' | '\u{200F}' |
            '\u{202A}'..='\u{202E}' |
            '\u{2066}'..='\u{2069}'
        ))
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ocr_temp_dir_is_removed_on_early_return_through_the_question_mark_operator() {
        fn fails_partway(marker: &mut Option<PathBuf>) -> AppResult<()> {
            let temp = OcrTempDir::create()?;
            *marker = Some(temp.path.clone());
            assert!(temp.path.exists());
            Err(AppError::Validation("simulated pdftoppm/tesseract failure".into()))?;
            unreachable!()
        }

        let mut marker = None;
        let result = fails_partway(&mut marker);
        assert!(result.is_err());
        let path = marker.expect("temp dir was created before the simulated failure");
        assert!(!path.exists(), "OcrTempDir must clean up even when the function exits via `?`");
    }
}
