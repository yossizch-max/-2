//! Phase B, milestone B4: Medical/Wage/Liability Ledgers. Split out of `commands.rs`
//! so this logic is directly testable in `integrity_tests.rs`, matching the pattern
//! already used by `authorities.rs`/`damage.rs`.
//!
//! A ledger entry records what a cited document *says* (a medical record, a pay
//! stub, a police report), verified by a lawyer against the actual source text -
//! never a legal conclusion TAHRIR itself asserts. `liability_facts` is named and
//! framed as a ledger of grounded facts bearing on liability, not a determination.
//!
//! Lifecycle is `draft -> verified`, correction-by-supersession, never a status
//! mutation: a verified row (and its sources) is immutable at the DB level - see
//! `006_matter_ledgers_v17.sql`'s triggers, which (unlike `legal_authority_passages`,
//! a known gap) also protect the source child tables. Because that trigger family is
//! the real integrity control, this module's `integrity_sha256` is a tamper-evidence
//! convenience over the source grounding (detects a later swap of which sources count
//! toward a hash a reviewer already saw), not the sole line of defense.
//!
//! A correction never mutates the old verified row: `supersede`-by-creating a new row
//! whose `supersedes_entry_id` points back at the old one. "Is this entry superseded"
//! is computed at read time (does some OTHER verified row point at me?), matching the
//! relevance/priority and default-workstream idiom already established in this schema.
use crate::{
    db::DbState,
    error::{AppError, AppResult},
    extraction,
    models::{LedgerSource, LiabilityFact, MedicalEvent, WageRecord},
};
use chrono::Utc;
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use uuid::Uuid;

pub const ALLOWED_LEDGER_KINDS: &[&str] = &["medical", "wage", "liability"];
pub const ALLOWED_STATUSES: &[&str] = &["draft", "verified"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LedgerKind {
    Medical,
    Wage,
    Liability,
}

impl LedgerKind {
    pub fn parse(v: &str) -> AppResult<Self> {
        match v {
            "medical" => Ok(Self::Medical),
            "wage" => Ok(Self::Wage),
            "liability" => Ok(Self::Liability),
            _ => Err(AppError::Validation(format!("unknown ledger kind \"{v}\""))),
        }
    }

    fn table(&self) -> &'static str {
        match self {
            Self::Medical => "medical_events",
            Self::Wage => "wage_records",
            Self::Liability => "liability_facts",
        }
    }

    fn source_table(&self) -> &'static str {
        match self {
            Self::Medical => "medical_event_sources",
            Self::Wage => "wage_record_sources",
            Self::Liability => "liability_fact_sources",
        }
    }
}

fn validate_supersedes(conn: &Connection, table: &str, matter_id: &str, old_entry_id: &str) -> AppResult<()> {
    let status: String = conn.query_row(
        &format!("SELECT status FROM {table} WHERE id=?1 AND matter_id=?2"),
        params![old_entry_id, matter_id], |r| r.get(0),
    ).map_err(|_| AppError::Validation("the entry being corrected was not found".into()))?;
    if status != "verified" {
        return Err(AppError::Validation(
            "only a verified ledger entry can be superseded by a correction".into()
        ));
    }
    Ok(())
}

/// Requires the parent entry to be `draft`, and the quoted text to appear verbatim
/// (after normalization) on the cited source page - same rule as
/// `authorities::add_passage`, reused via `extraction::normalize_source_text`.
pub fn add_source(
    db: &DbState, kind: LedgerKind, matter_id: &str, entry_id: &str,
    source_page_id: &str, quote_text: &str,
) -> AppResult<String> {
    if quote_text.trim().is_empty() {
        return Err(AppError::Validation("quote text required".into()));
    }
    let table = kind.table();
    let source_table = kind.source_table();
    let id = Uuid::new_v4().to_string();
    db.write(|conn| {
        let status: String = conn.query_row(
            &format!("SELECT status FROM {table} WHERE id=?1 AND matter_id=?2"),
            params![entry_id, matter_id], |r| r.get(0),
        ).map_err(|_| AppError::NotFound("ledger entry".into()))?;
        if status != "draft" {
            return Err(AppError::Validation("only a draft ledger entry can have sources added".into()));
        }

        let (document_version_id, page_normalized): (String, String) = conn.query_row(
            "SELECT document_version_id,normalized_text FROM document_pages WHERE id=?1 AND matter_id=?2",
            params![source_page_id, matter_id], |r| Ok((r.get(0)?, r.get(1)?)),
        ).map_err(|_| AppError::InvalidSourceReference)?;

        let normalized_quote = extraction::normalize_source_text(quote_text);
        if normalized_quote.is_empty() || !page_normalized.contains(&normalized_quote) {
            return Err(AppError::Validation(
                "the quoted text was not found verbatim on the cited source page".into()
            ));
        }
        let source_text_sha256 = hex::encode(Sha256::digest(normalized_quote.as_bytes()));

        conn.execute(
            &format!(
                "INSERT INTO {source_table}(
                    id,matter_id,entry_id,document_version_id,document_page_id,display_quote,source_text_sha256
                 ) VALUES(?1,?2,?3,?4,?5,?6,?7)"
            ),
            params![id, matter_id, entry_id, document_version_id, source_page_id, quote_text, source_text_sha256],
        )?;
        Ok(())
    })?;
    Ok(id)
}

pub fn list_entry_sources(db: &DbState, kind: LedgerKind, matter_id: &str, entry_id: &str) -> AppResult<Vec<LedgerSource>> {
    let source_table = kind.source_table();
    db.read(|conn| {
        let mut stmt = conn.prepare(&format!(
            "SELECT id,matter_id,entry_id,document_version_id,document_page_id,display_quote,source_text_sha256
             FROM {source_table} WHERE matter_id=?1 AND entry_id=?2 ORDER BY id"
        ))?;
        let rows = stmt.query_map(params![matter_id, entry_id], |r| Ok(LedgerSource {
            id: r.get(0)?, matter_id: r.get(1)?, entry_id: r.get(2)?, document_version_id: r.get(3)?,
            document_page_id: r.get(4)?, display_quote: r.get(5)?, source_text_sha256: r.get(6)?,
        }))?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
}

/// Requires the entry to be `draft` with at least one source (fail closed - an
/// unsupported entry can never verify), and re-checks verbatim containment fresh
/// against each source's *current* page text right before flipping status - the same
/// recheck-at-terminal-transition discipline used by `authorities::verify`,
/// `legal_rules::approve_ruleset`, and `ai::approve_proposal`.
pub fn verify_entry(db: &DbState, kind: LedgerKind, matter_id: &str, entry_id: &str) -> AppResult<String> {
    let table = kind.table();
    let source_table = kind.source_table();
    db.write(|conn| {
        let status: String = conn.query_row(
            &format!("SELECT status FROM {table} WHERE id=?1 AND matter_id=?2"),
            params![entry_id, matter_id], |r| r.get(0),
        ).map_err(|_| AppError::Validation("ledger entry not verifiable".into()))?;
        if status != "draft" {
            return Err(AppError::Validation("ledger entry not verifiable".into()));
        }

        let mut stmt = conn.prepare(&format!(
            "SELECT document_page_id,display_quote,source_text_sha256
             FROM {source_table} WHERE matter_id=?1 AND entry_id=?2 ORDER BY id"
        ))?;
        let sources: Vec<(String, String, String)> = stmt
            .query_map(params![matter_id, entry_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        if sources.is_empty() {
            return Err(AppError::Validation(
                "a ledger entry requires at least one source before it can be verified".into()
            ));
        }

        let mut hashes = Vec::with_capacity(sources.len());
        for (page_id, quote, hash) in &sources {
            let page_normalized: String = conn.query_row(
                "SELECT normalized_text FROM document_pages WHERE id=?1 AND matter_id=?2",
                params![page_id, matter_id], |r| r.get(0),
            ).map_err(|_| AppError::InvalidSourceReference)?;
            let normalized_quote = extraction::normalize_source_text(quote);
            if normalized_quote.is_empty() || !page_normalized.contains(&normalized_quote) {
                return Err(AppError::Validation(
                    "a cited source no longer contains its quote verbatim; this entry cannot be verified".into()
                ));
            }
            hashes.push(hash.clone());
        }

        let integrity_sha = hex::encode(Sha256::digest(format!("{entry_id}:{table}:{}", hashes.join(","))));
        let changed = conn.execute(
            &format!(
                "UPDATE {table} SET status='verified',verified_at=?3,integrity_sha256=?4
                 WHERE id=?1 AND matter_id=?2 AND status='draft'"
            ),
            params![entry_id, matter_id, Utc::now().to_rfc3339(), integrity_sha],
        )?;
        if changed != 1 { return Err(AppError::Validation("ledger entry not verifiable".into())); }
        Ok(integrity_sha)
    })
}

/// True if some other verified row in this table supersedes `entry_id` - computed at
/// read time, never persisted on the row itself (the old row is never touched).
fn superseded_ids(conn: &Connection, table: &str, matter_id: &str) -> AppResult<HashSet<String>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT supersedes_entry_id FROM {table}
         WHERE matter_id=?1 AND status='verified' AND supersedes_entry_id IS NOT NULL"
    ))?;
    let ids = stmt.query_map([matter_id], |r| r.get(0))?.collect::<Result<HashSet<_>, _>>()?;
    Ok(ids)
}

pub fn create_medical_event(
    db: &DbState, matter_id: &str, event_date: Option<&str>, provider_name: Option<&str>,
    treatment_summary: &str, supersedes_entry_id: Option<&str>,
) -> AppResult<String> {
    if treatment_summary.trim().is_empty() {
        return Err(AppError::Validation("treatment summary required".into()));
    }
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    db.write(|conn| {
        if let Some(old_id) = supersedes_entry_id {
            validate_supersedes(conn, "medical_events", matter_id, old_id)?;
        }
        conn.execute(
            "INSERT INTO medical_events(
                id,matter_id,event_date,provider_name,treatment_summary,supersedes_entry_id,created_at,updated_at
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?7)",
            params![id, matter_id, event_date, provider_name, treatment_summary, supersedes_entry_id, now],
        )?;
        Ok(())
    })?;
    Ok(id)
}

pub fn update_draft_medical_event(
    db: &DbState, matter_id: &str, entry_id: &str,
    event_date: Option<&str>, provider_name: Option<&str>, treatment_summary: &str,
) -> AppResult<()> {
    if treatment_summary.trim().is_empty() {
        return Err(AppError::Validation("treatment summary required".into()));
    }
    db.write(|conn| {
        let now = Utc::now().to_rfc3339();
        let changed = conn.execute(
            "UPDATE medical_events SET event_date=?3,provider_name=?4,treatment_summary=?5,updated_at=?6
             WHERE id=?1 AND matter_id=?2 AND status='draft'",
            params![entry_id, matter_id, event_date, provider_name, treatment_summary, now],
        )?;
        if changed != 1 { return Err(AppError::Validation("only a draft ledger entry can be edited".into())); }
        Ok(())
    })
}

pub fn list_medical_events(db: &DbState, matter_id: &str) -> AppResult<Vec<MedicalEvent>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id,matter_id,event_date,provider_name,treatment_summary,status,stale,
                    supersedes_entry_id,integrity_sha256,verified_at,created_at,updated_at
             FROM medical_events WHERE matter_id=?1 ORDER BY created_at"
        )?;
        let rows: Vec<MedicalEvent> = stmt.query_map([matter_id], |r| Ok(MedicalEvent {
            id: r.get(0)?, matter_id: r.get(1)?, event_date: r.get(2)?, provider_name: r.get(3)?,
            treatment_summary: r.get(4)?, status: r.get(5)?, stale: r.get::<_, i64>(6)? != 0,
            supersedes_entry_id: r.get(7)?, integrity_sha256: r.get(8)?, verified_at: r.get(9)?,
            created_at: r.get(10)?, updated_at: r.get(11)?, superseded: false,
        }))?.collect::<Result<Vec<_>, _>>()?;
        let superseded = superseded_ids(conn, "medical_events", matter_id)?;
        Ok(rows.into_iter().map(|mut e| { e.superseded = superseded.contains(&e.id); e }).collect())
    })
}

pub fn create_wage_record(
    db: &DbState, matter_id: &str, period_start: Option<&str>, period_end: Option<&str>,
    employer_name: Option<&str>, gross_amount_cents: i64, supersedes_entry_id: Option<&str>,
) -> AppResult<String> {
    if gross_amount_cents < 0 {
        return Err(AppError::Validation("gross amount cannot be negative".into()));
    }
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    db.write(|conn| {
        if let Some(old_id) = supersedes_entry_id {
            validate_supersedes(conn, "wage_records", matter_id, old_id)?;
        }
        conn.execute(
            "INSERT INTO wage_records(
                id,matter_id,period_start,period_end,employer_name,gross_amount_cents,
                supersedes_entry_id,created_at,updated_at
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?8)",
            params![id, matter_id, period_start, period_end, employer_name, gross_amount_cents,
                supersedes_entry_id, now],
        )?;
        Ok(())
    })?;
    Ok(id)
}

pub fn update_draft_wage_record(
    db: &DbState, matter_id: &str, entry_id: &str, period_start: Option<&str>, period_end: Option<&str>,
    employer_name: Option<&str>, gross_amount_cents: i64,
) -> AppResult<()> {
    if gross_amount_cents < 0 {
        return Err(AppError::Validation("gross amount cannot be negative".into()));
    }
    db.write(|conn| {
        let now = Utc::now().to_rfc3339();
        let changed = conn.execute(
            "UPDATE wage_records SET period_start=?3,period_end=?4,employer_name=?5,
                gross_amount_cents=?6,updated_at=?7
             WHERE id=?1 AND matter_id=?2 AND status='draft'",
            params![entry_id, matter_id, period_start, period_end, employer_name, gross_amount_cents, now],
        )?;
        if changed != 1 { return Err(AppError::Validation("only a draft ledger entry can be edited".into())); }
        Ok(())
    })
}

pub fn list_wage_records(db: &DbState, matter_id: &str) -> AppResult<Vec<WageRecord>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id,matter_id,period_start,period_end,employer_name,gross_amount_cents,status,stale,
                    supersedes_entry_id,integrity_sha256,verified_at,created_at,updated_at
             FROM wage_records WHERE matter_id=?1 ORDER BY created_at"
        )?;
        let rows: Vec<WageRecord> = stmt.query_map([matter_id], |r| Ok(WageRecord {
            id: r.get(0)?, matter_id: r.get(1)?, period_start: r.get(2)?, period_end: r.get(3)?,
            employer_name: r.get(4)?, gross_amount_cents: r.get(5)?, status: r.get(6)?,
            stale: r.get::<_, i64>(7)? != 0, supersedes_entry_id: r.get(8)?, integrity_sha256: r.get(9)?,
            verified_at: r.get(10)?, created_at: r.get(11)?, updated_at: r.get(12)?, superseded: false,
        }))?.collect::<Result<Vec<_>, _>>()?;
        let superseded = superseded_ids(conn, "wage_records", matter_id)?;
        Ok(rows.into_iter().map(|mut e| { e.superseded = superseded.contains(&e.id); e }).collect())
    })
}

pub fn create_liability_fact(
    db: &DbState, matter_id: &str, claim_basis: Option<&str>, liable_party_name: Option<&str>,
    description: &str, supersedes_entry_id: Option<&str>,
) -> AppResult<String> {
    if description.trim().is_empty() {
        return Err(AppError::Validation("description required".into()));
    }
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    db.write(|conn| {
        if let Some(old_id) = supersedes_entry_id {
            validate_supersedes(conn, "liability_facts", matter_id, old_id)?;
        }
        conn.execute(
            "INSERT INTO liability_facts(
                id,matter_id,claim_basis,liable_party_name,description,supersedes_entry_id,created_at,updated_at
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?7)",
            params![id, matter_id, claim_basis, liable_party_name, description, supersedes_entry_id, now],
        )?;
        Ok(())
    })?;
    Ok(id)
}

pub fn update_draft_liability_fact(
    db: &DbState, matter_id: &str, entry_id: &str,
    claim_basis: Option<&str>, liable_party_name: Option<&str>, description: &str,
) -> AppResult<()> {
    if description.trim().is_empty() {
        return Err(AppError::Validation("description required".into()));
    }
    db.write(|conn| {
        let now = Utc::now().to_rfc3339();
        let changed = conn.execute(
            "UPDATE liability_facts SET claim_basis=?3,liable_party_name=?4,description=?5,updated_at=?6
             WHERE id=?1 AND matter_id=?2 AND status='draft'",
            params![entry_id, matter_id, claim_basis, liable_party_name, description, now],
        )?;
        if changed != 1 { return Err(AppError::Validation("only a draft ledger entry can be edited".into())); }
        Ok(())
    })
}

pub fn list_liability_facts(db: &DbState, matter_id: &str) -> AppResult<Vec<LiabilityFact>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id,matter_id,claim_basis,liable_party_name,description,status,stale,
                    supersedes_entry_id,integrity_sha256,verified_at,created_at,updated_at
             FROM liability_facts WHERE matter_id=?1 ORDER BY created_at"
        )?;
        let rows: Vec<LiabilityFact> = stmt.query_map([matter_id], |r| Ok(LiabilityFact {
            id: r.get(0)?, matter_id: r.get(1)?, claim_basis: r.get(2)?, liable_party_name: r.get(3)?,
            description: r.get(4)?, status: r.get(5)?, stale: r.get::<_, i64>(6)? != 0,
            supersedes_entry_id: r.get(7)?, integrity_sha256: r.get(8)?, verified_at: r.get(9)?,
            created_at: r.get(10)?, updated_at: r.get(11)?, superseded: false,
        }))?.collect::<Result<Vec<_>, _>>()?;
        let superseded = superseded_ids(conn, "liability_facts", matter_id)?;
        Ok(rows.into_iter().map(|mut e| { e.superseded = superseded.contains(&e.id); e }).collect())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ledger_kind_parses_known_kinds_and_rejects_unknown_ones() {
        assert!(matches!(LedgerKind::parse("medical"), Ok(LedgerKind::Medical)));
        assert!(matches!(LedgerKind::parse("wage"), Ok(LedgerKind::Wage)));
        assert!(matches!(LedgerKind::parse("liability"), Ok(LedgerKind::Liability)));
        assert!(LedgerKind::parse("made_up").is_err());
    }

    #[test]
    fn ledger_kind_maps_to_the_right_table_names() {
        assert_eq!(LedgerKind::Medical.table(), "medical_events");
        assert_eq!(LedgerKind::Medical.source_table(), "medical_event_sources");
        assert_eq!(LedgerKind::Wage.table(), "wage_records");
        assert_eq!(LedgerKind::Wage.source_table(), "wage_record_sources");
        assert_eq!(LedgerKind::Liability.table(), "liability_facts");
        assert_eq!(LedgerKind::Liability.source_table(), "liability_fact_sources");
    }
}
