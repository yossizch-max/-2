//! Legal-authority passage grounding. Split out of `commands.rs` (rather than
//! inlined behind `tauri::State`) so this logic is directly testable in
//! `integrity_tests.rs`, matching the pattern already used by `legal_docs.rs` and
//! `damage.rs` - every fail-closed rule in this codebase needs a test that can
//! actually call it, not just a command a running Tauri app happens to expose.
use crate::{
    db::DbState,
    error::{AppError, AppResult},
    extraction,
};
use chrono::Utc;
use rusqlite::params;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Adds a draft (unapproved) passage to an authority, requiring the quoted text to
/// appear verbatim (after normalization) on the cited source page - a passage cannot
/// be typed freely, it must be grounded in the authority's own stored source document.
pub fn add_passage(
    db: &DbState, matter_id: &str, authority_id: &str, source_page_id: &str,
    passage_text: &str, issue_tag: Option<&str>,
) -> AppResult<String> {
    if passage_text.trim().is_empty() {
        return Err(AppError::Validation("passage text required".into()));
    }
    let id = Uuid::new_v4().to_string();
    db.write(|conn| {
        let source_version: Option<String> = conn.query_row(
            "SELECT source_document_version_id FROM legal_authorities
             WHERE id=?1 AND matter_id=?2 AND status='draft'",
            params![authority_id, matter_id], |r| r.get(0),
        ).map_err(|_| AppError::Validation("authority not editable".into()))?;
        let source_version = source_version.ok_or_else(|| AppError::Validation(
            "the authority needs a stored source document before a passage can be added to it".into()
        ))?;

        let page_normalized: String = conn.query_row(
            "SELECT normalized_text FROM document_pages
             WHERE id=?1 AND matter_id=?2 AND document_version_id=?3",
            params![source_page_id, matter_id, source_version], |r| r.get(0),
        ).map_err(|_| AppError::InvalidSourceReference)?;

        let normalized_passage = extraction::normalize_source_text(passage_text);
        if normalized_passage.is_empty() || !page_normalized.contains(&normalized_passage) {
            return Err(AppError::Validation(
                "the quoted passage was not found verbatim on the cited source page".into()
            ));
        }
        let passage_sha256 = hex::encode(Sha256::digest(normalized_passage.as_bytes()));

        conn.execute(
            "INSERT INTO legal_authority_passages(
                id,matter_id,authority_id,source_page_id,passage_text,passage_sha256,issue_tag,approved
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,0)",
            params![id, matter_id, authority_id, source_page_id, passage_text, passage_sha256, issue_tag],
        )?;
        Ok(())
    })?;
    Ok(id)
}

/// Approving a passage re-checks the verbatim-containment rule against the source
/// page's *current* text, not just what it was when the passage was drafted - the
/// same re-check-at-approval-time discipline used for legal-document fact paragraphs.
pub fn approve_passage(db: &DbState, matter_id: &str, authority_id: &str, passage_id: &str) -> AppResult<()> {
    db.write(|conn| {
        let (source_page_id, passage_text): (String, String) = conn.query_row(
            "SELECT source_page_id,passage_text FROM legal_authority_passages
             WHERE id=?1 AND matter_id=?2 AND authority_id=?3",
            params![passage_id, matter_id, authority_id], |r| Ok((r.get(0)?, r.get(1)?)),
        ).map_err(|_| AppError::Validation("passage not found".into()))?;

        let page_normalized: String = conn.query_row(
            "SELECT normalized_text FROM document_pages WHERE id=?1 AND matter_id=?2",
            params![source_page_id, matter_id], |r| r.get(0),
        ).map_err(|_| AppError::InvalidSourceReference)?;

        let normalized_passage = extraction::normalize_source_text(&passage_text);
        if normalized_passage.is_empty() || !page_normalized.contains(&normalized_passage) {
            return Err(AppError::Validation(
                "the source page no longer contains this passage verbatim; it cannot be approved".into()
            ));
        }

        let changed = conn.execute(
            "UPDATE legal_authority_passages SET approved=1 WHERE id=?1 AND matter_id=?2 AND authority_id=?3",
            params![passage_id, matter_id, authority_id],
        )?;
        if changed != 1 { return Err(AppError::Validation("passage not found".into())); }
        Ok(())
    })
}

/// Verifying an authority now requires at least one *approved* passage - a stored
/// source document alone (the pre-existing v1 P0-6 fix) is not enough, since that only
/// proves a document was attached, not that the lawyer actually read and stood behind
/// a specific quoted passage from it. Approved passage hashes are folded into the
/// integrity hash, ordered, so a later swap of an approved passage is detectable.
pub fn verify(db: &DbState, matter_id: &str, authority_id: &str) -> AppResult<String> {
    db.write(|conn| {
        let (citation, title, court, decision_date, source_version):
            (String, String, Option<String>, Option<String>, Option<String>) = conn.query_row(
            "SELECT citation,title,court,decision_date,source_document_version_id
             FROM legal_authorities WHERE id=?1 AND matter_id=?2 AND status='draft'",
            params![authority_id, matter_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        ).map_err(|_| AppError::Validation("authority not verifiable".into()))?;

        let source_version = source_version.ok_or_else(|| AppError::Validation(
            "an authority requires a stored source document before it can be verified".into()
        ))?;
        let source_sha: String = conn.query_row(
            "SELECT content_sha256 FROM document_versions WHERE id=?1 AND matter_id=?2",
            params![source_version, matter_id], |r| r.get(0),
        ).map_err(|_| AppError::InvalidSourceReference)?;

        let mut passage_stmt = conn.prepare(
            "SELECT passage_sha256 FROM legal_authority_passages
             WHERE matter_id=?1 AND authority_id=?2 AND approved=1 ORDER BY id"
        )?;
        let passage_hashes: Vec<String> = passage_stmt
            .query_map(params![matter_id, authority_id], |r| r.get(0))?
            .collect::<Result<Vec<_>, _>>()?;
        if passage_hashes.is_empty() {
            return Err(AppError::Validation(
                "an authority requires at least one approved passage before it can be verified".into()
            ));
        }

        let integrity_sha = hex::encode(Sha256::digest(format!(
            "{authority_id}:{citation}:{title}:{}:{}:{source_version}:{source_sha}:{}",
            court.unwrap_or_default(), decision_date.unwrap_or_default(), passage_hashes.join(","),
        )));
        let changed = conn.execute(
            "UPDATE legal_authorities SET status='verified',verified_at=?3,integrity_sha256=?4
             WHERE id=?1 AND matter_id=?2 AND status='draft'",
            params![authority_id, matter_id, Utc::now().to_rfc3339(), integrity_sha],
        )?;
        if changed != 1 { return Err(AppError::Validation("authority not verifiable".into())); }
        Ok(integrity_sha)
    })
}
