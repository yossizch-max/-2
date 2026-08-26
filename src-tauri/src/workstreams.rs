//! Workstreams + Matter Packs (Phase B, milestone B2): per-matter parallel tracks
//! (medical/liability/wage/insurance/BTL/negotiation/litigation), each with a status,
//! auto-seeded from the matter's case type and reconciled - never destructively - when
//! the case type later changes. Split out of `commands.rs` so this logic is directly
//! testable in `integrity_tests.rs`, matching the pattern already used by
//! `matter_profile.rs`/`authorities.rs`/`damage.rs`.
//!
//! Matter Pack defaults below are office-workflow defaults, not legal determinations -
//! a lawyer can flip any workstream's status regardless of these, and no Israeli
//! substantive law is encoded here.
use crate::{
    error::{AppError, AppResult},
    models::Workstream,
};
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

/// Ordered as the report itself orders workstreams (not alphabetically) - this is also
/// the order `list_matter_workstreams` returns them in.
pub const ALLOWED_KINDS: &[&str] =
    &["medical", "liability", "wage", "insurance", "btl", "negotiation", "litigation"];

pub const ALLOWED_STATUSES: &[&str] =
    &["not_applicable", "not_started", "active", "blocked", "done"];

pub fn validate_kind(v: &str) -> AppResult<()> {
    if !ALLOWED_KINDS.contains(&v) {
        return Err(AppError::Validation(format!("unknown workstream kind \"{v}\"")));
    }
    Ok(())
}

pub fn validate_status(v: &str) -> AppResult<()> {
    if !ALLOWED_STATUSES.contains(&v) {
        return Err(AppError::Validation(format!("unknown workstream status \"{v}\"")));
    }
    Ok(())
}

fn default_active_kinds(case_type: &str) -> &'static [&'static str] {
    match case_type {
        "traffic_accident" => &["medical", "wage", "insurance", "btl", "negotiation", "litigation"],
        "work_accident" => &["medical", "wage", "liability", "btl", "negotiation", "litigation"],
        "general_negligence" => &["liability", "insurance", "negotiation", "litigation"],
        "medical_malpractice" => &["medical", "liability", "insurance", "negotiation", "litigation"],
        "civil_commercial" => &["negotiation", "litigation"],
        _ /* generic_civil, other */ => &["negotiation", "litigation"],
    }
}

/// Idempotent, non-destructive. Handles three cases with one pass:
/// - a brand-new matter (nothing exists yet -> full seed);
/// - a pre-B2 matter with zero workstream rows (full backfill, same as above);
/// - an existing matter whose case type just changed (only the second step below can
///   fire, since every kind already has a row).
///
/// Never touches a row already at `not_started`/`active`/`blocked`/`done` - those are
/// either already visible to the lawyer or already in use. Takes `&Connection` (not
/// `&DbState`) so it composes inside the same transaction as the caller's own INSERT/
/// UPDATE on `matters`.
pub fn reconcile(conn: &Connection, matter_id: &str, case_type: &str) -> AppResult<()> {
    let defaults = default_active_kinds(case_type);
    let now = Utc::now().to_rfc3339();
    for kind in ALLOWED_KINDS {
        let status = if defaults.contains(kind) { "not_started" } else { "not_applicable" };
        conn.execute(
            "INSERT INTO matter_workstreams(id,matter_id,kind,status,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,?5)
             ON CONFLICT(matter_id,kind) DO NOTHING",
            params![Uuid::new_v4().to_string(), matter_id, kind, status, now],
        )?;
    }
    for kind in defaults {
        conn.execute(
            "UPDATE matter_workstreams SET status='not_started',updated_at=?3
             WHERE matter_id=?1 AND kind=?2 AND status='not_applicable'",
            params![matter_id, kind, now],
        )?;
    }
    Ok(())
}

pub fn list(conn: &Connection, matter_id: &str) -> AppResult<Vec<Workstream>> {
    let mut stmt = conn.prepare(
        "SELECT id,matter_id,kind,status,notes,created_at,updated_at
         FROM matter_workstreams WHERE matter_id=?1"
    )?;
    let mut rows: Vec<Workstream> = stmt.query_map([matter_id], |r| Ok(Workstream {
        id: r.get(0)?, matter_id: r.get(1)?, kind: r.get(2)?, status: r.get(3)?,
        notes: r.get(4)?, created_at: r.get(5)?, updated_at: r.get(6)?,
    }))?.collect::<Result<Vec<_>, _>>()?;
    rows.sort_by_key(|w| ALLOWED_KINDS.iter().position(|k| *k == w.kind).unwrap_or(usize::MAX));
    Ok(rows)
}

pub fn update_status(
    conn: &Connection, matter_id: &str, kind: &str, status: &str, notes: Option<&str>,
) -> AppResult<()> {
    validate_kind(kind)?;
    validate_status(status)?;
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO matter_workstreams(id,matter_id,kind,status,notes,created_at,updated_at)
         VALUES(?1,?2,?3,?4,?5,?6,?6)
         ON CONFLICT(matter_id,kind) DO UPDATE SET
            status=excluded.status, notes=coalesce(?5,notes), updated_at=excluded.updated_at",
        params![Uuid::new_v4().to_string(), matter_id, kind, status, notes, now],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_an_unknown_kind() {
        assert!(validate_kind("medical").is_ok());
        assert!(validate_kind("made_up_kind").is_err());
    }

    #[test]
    fn rejects_an_unknown_status() {
        assert!(validate_status("active").is_ok());
        assert!(validate_status("made_up_status").is_err());
    }

    #[test]
    fn default_active_kinds_covers_every_case_type() {
        assert!(default_active_kinds("traffic_accident").contains(&"btl"));
        assert!(default_active_kinds("work_accident").contains(&"liability"));
        assert!(default_active_kinds("general_negligence").contains(&"liability"));
        assert!(default_active_kinds("medical_malpractice").contains(&"medical"));
        assert_eq!(default_active_kinds("civil_commercial"), &["negotiation", "litigation"]);
        assert_eq!(default_active_kinds("generic_civil"), &["negotiation", "litigation"]);
        assert_eq!(default_active_kinds("other"), &["negotiation", "litigation"]);
    }
}
