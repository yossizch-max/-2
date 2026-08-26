//! Missing Evidence Matrix (Phase B, milestone B3): a per-matter checklist of typical
//! documents a matter of a given case type tends to need, auto-seeded from the
//! matter's case type and reconciled - never destructively - when the case type later
//! changes. Split out of `commands.rs` so this logic is directly testable in
//! `integrity_tests.rs`, matching the pattern already used by `workstreams.rs`.
//!
//! These are office-workflow checklist recommendations, never phrased as statutory -
//! nothing here claims legal force. A source becoming an Approved Legal Ruleset is the
//! only way a future requirement could ever carry real legal weight.
use crate::{
    error::{AppError, AppResult},
    models::MatterRequirement,
};
use chrono::Utc;
use rusqlite::{params, Connection};
use uuid::Uuid;

pub const ALLOWED_REQUIREMENT_KEYS: &[&str] = &[
    "id_document", "police_report", "medical_records_initial", "medical_records_full_file",
    "wage_stubs", "employer_incident_report", "witness_statements", "insurance_policy",
    "btl_forms", "vehicle_photos", "expert_opinion", "contract_document", "correspondence_records",
];

pub const ALLOWED_STATUSES: &[&str] =
    &["not_applicable", "not_collected", "requested", "collected", "stale"];

pub const ALLOWED_PRIORITIES: &[&str] = &["recommended", "required_by_office_policy", "optional"];

pub fn validate_key(v: &str) -> AppResult<()> {
    if !ALLOWED_REQUIREMENT_KEYS.contains(&v) {
        return Err(AppError::Validation(format!("unknown requirement key \"{v}\"")));
    }
    Ok(())
}

pub fn validate_status(v: &str) -> AppResult<()> {
    if !ALLOWED_STATUSES.contains(&v) {
        return Err(AppError::Validation(format!("unknown requirement status \"{v}\"")));
    }
    Ok(())
}

/// (requirement_key, priority) - office-workflow recommendations, not legal
/// requirements; freely overridable per matter regardless of these defaults.
fn default_requirements(case_type: &str) -> &'static [(&'static str, &'static str)] {
    match case_type {
        "traffic_accident" => &[
            ("id_document", "required_by_office_policy"), ("police_report", "recommended"),
            ("medical_records_initial", "required_by_office_policy"), ("wage_stubs", "recommended"),
            ("insurance_policy", "required_by_office_policy"), ("btl_forms", "recommended"),
            ("vehicle_photos", "optional"),
        ],
        "work_accident" => &[
            ("id_document", "required_by_office_policy"), ("employer_incident_report", "required_by_office_policy"),
            ("medical_records_initial", "required_by_office_policy"), ("wage_stubs", "recommended"),
            ("btl_forms", "recommended"), ("witness_statements", "optional"),
        ],
        "general_negligence" => &[
            ("id_document", "required_by_office_policy"), ("witness_statements", "recommended"),
            ("expert_opinion", "optional"),
        ],
        "medical_malpractice" => &[
            ("id_document", "required_by_office_policy"), ("medical_records_initial", "required_by_office_policy"),
            ("medical_records_full_file", "required_by_office_policy"), ("expert_opinion", "recommended"),
        ],
        "civil_commercial" => &[
            ("id_document", "required_by_office_policy"), ("contract_document", "required_by_office_policy"),
            ("correspondence_records", "recommended"),
        ],
        _ /* generic_civil, other */ => &[("id_document", "required_by_office_policy")],
    }
}

fn priority_for(case_type: &str, key: &str) -> String {
    default_requirements(case_type).iter()
        .find(|(k, _)| *k == key)
        .map(|(_, p)| p.to_string())
        .unwrap_or_else(|| "not_applicable".to_string())
}

/// Idempotent, non-destructive - identical shape to `workstreams::reconcile`. Handles
/// a brand-new matter, a pre-B3 matter with zero requirement rows, and a case-type
/// change on an existing matter with one pass: never touches a row already at
/// `not_collected`/`requested`/`collected`/`stale`.
pub fn reconcile(conn: &Connection, matter_id: &str, case_type: &str) -> AppResult<()> {
    let defaults = default_requirements(case_type);
    let now = Utc::now().to_rfc3339();
    for key in ALLOWED_REQUIREMENT_KEYS {
        let status = if defaults.iter().any(|(k, _)| k == key) { "not_collected" } else { "not_applicable" };
        conn.execute(
            "INSERT INTO matter_requirements(id,matter_id,requirement_key,status,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,?5)
             ON CONFLICT(matter_id,requirement_key) DO NOTHING",
            params![Uuid::new_v4().to_string(), matter_id, key, status, now],
        )?;
    }
    for (key, _) in defaults {
        conn.execute(
            "UPDATE matter_requirements SET status='not_collected',updated_at=?3
             WHERE matter_id=?1 AND requirement_key=?2 AND status='not_applicable'",
            params![matter_id, key, now],
        )?;
    }
    Ok(())
}

pub fn list(conn: &Connection, matter_id: &str, case_type: &str) -> AppResult<Vec<MatterRequirement>> {
    let mut stmt = conn.prepare(
        "SELECT id,matter_id,requirement_key,status,notes,created_at,updated_at
         FROM matter_requirements WHERE matter_id=?1"
    )?;
    let mut rows: Vec<MatterRequirement> = stmt.query_map([matter_id], |r| {
        let requirement_key: String = r.get(2)?;
        let priority = priority_for(case_type, &requirement_key);
        Ok(MatterRequirement {
            id: r.get(0)?, matter_id: r.get(1)?, requirement_key, status: r.get(3)?,
            priority, notes: r.get(4)?, created_at: r.get(5)?, updated_at: r.get(6)?,
        })
    })?.collect::<Result<Vec<_>, _>>()?;
    rows.sort_by_key(|req| ALLOWED_REQUIREMENT_KEYS.iter().position(|k| *k == req.requirement_key).unwrap_or(usize::MAX));
    Ok(rows)
}

pub fn update_status(
    conn: &Connection, matter_id: &str, key: &str, status: &str, notes: Option<&str>,
) -> AppResult<()> {
    validate_key(key)?;
    validate_status(status)?;
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO matter_requirements(id,matter_id,requirement_key,status,notes,created_at,updated_at)
         VALUES(?1,?2,?3,?4,?5,?6,?6)
         ON CONFLICT(matter_id,requirement_key) DO UPDATE SET
            status=excluded.status, notes=coalesce(?5,notes), updated_at=excluded.updated_at",
        params![Uuid::new_v4().to_string(), matter_id, key, status, notes, now],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_an_unknown_key() {
        assert!(validate_key("id_document").is_ok());
        assert!(validate_key("made_up_key").is_err());
    }

    #[test]
    fn rejects_an_unknown_status() {
        assert!(validate_status("collected").is_ok());
        assert!(validate_status("made_up_status").is_err());
    }

    #[test]
    fn default_requirements_covers_every_case_type() {
        assert!(default_requirements("traffic_accident").iter().any(|(k, _)| *k == "police_report"));
        assert!(default_requirements("work_accident").iter().any(|(k, _)| *k == "employer_incident_report"));
        assert!(default_requirements("general_negligence").iter().any(|(k, _)| *k == "witness_statements"));
        assert!(default_requirements("medical_malpractice").iter().any(|(k, _)| *k == "medical_records_full_file"));
        assert!(default_requirements("civil_commercial").iter().any(|(k, _)| *k == "contract_document"));
        assert_eq!(default_requirements("generic_civil"), &[("id_document", "required_by_office_policy")]);
        assert_eq!(default_requirements("other"), &[("id_document", "required_by_office_policy")]);
    }

    #[test]
    fn priority_for_a_key_outside_the_pack_is_not_applicable() {
        assert_eq!(priority_for("civil_commercial", "vehicle_photos"), "not_applicable");
        assert_eq!(priority_for("traffic_accident", "vehicle_photos"), "optional");
    }

    #[test]
    fn every_default_requirements_priority_is_an_allowed_priority() {
        for case_type in ["traffic_accident", "work_accident", "general_negligence",
            "medical_malpractice", "civil_commercial", "generic_civil", "other"] {
            for (_, priority) in default_requirements(case_type) {
                assert!(ALLOWED_PRIORITIES.contains(priority), "unexpected priority \"{priority}\" for {case_type}");
            }
        }
    }
}
