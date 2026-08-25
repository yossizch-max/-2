//! Matter Profile (Phase B, milestone B1): case-type taxonomy, event/court/BTL fields,
//! and party contacts. Split out of `commands.rs` so this logic is directly testable in
//! `integrity_tests.rs`, matching the pattern already used by `authorities.rs`/`damage.rs`.
//!
//! This is plain office-management data - editable like `update_matter` today, with no
//! lock/approval lifecycle and no DB immutability triggers. None of it is an evidentiary
//! claim, so it doesn't need the source-grounding machinery `authorities.rs`/
//! `verified_facts` use.
use crate::{
    db::DbState,
    error::{AppError, AppResult},
    models::{MatterParty, MatterProfile},
};
use chrono::{NaiveDate, Utc};
use rusqlite::params;
use uuid::Uuid;

pub const ALLOWED_CASE_TYPES: &[&str] = &[
    "motor_vehicle_accident",   // תאונת דרכים
    "workplace_accident",       // תאונת עבודה
    "general_negligence",       // רשלנות כללית
    "medical_negligence",       // רשלנות רפואית
    "civil_commercial_dispute", // סכסוך אזרחי/מסחרי
    "generic_civil",            // ברירת מחדל/אחר
];

pub const ALLOWED_PARTY_ROLES: &[&str] = &[
    "client", "party", "witness", "employer", "insurer",
    "medical_institution", "expert", "opposing_counsel", "court",
];

pub fn validate_case_type(v: &str) -> AppResult<()> {
    if !ALLOWED_CASE_TYPES.contains(&v) {
        return Err(AppError::Validation(format!("unknown case type \"{v}\"")));
    }
    Ok(())
}

pub fn validate_party_role(v: &str) -> AppResult<()> {
    if !ALLOWED_PARTY_ROLES.contains(&v) {
        return Err(AppError::Validation(format!("unknown party role \"{v}\"")));
    }
    Ok(())
}

fn validate_event_date(v: Option<&str>) -> AppResult<()> {
    if let Some(s) = v {
        NaiveDate::parse_from_str(s, "%Y-%m-%d")
            .map_err(|_| AppError::Validation(format!("eventDate must be an ISO date (YYYY-MM-DD), got \"{s}\"")))?;
    }
    Ok(())
}

pub fn save_profile(
    db: &DbState, matter_id: &str, event_date: Option<&str>, court_name: Option<&str>,
    btl_claim_number: Option<&str>, case_summary: Option<&str>,
) -> AppResult<()> {
    validate_event_date(event_date)?;
    let now = Utc::now().to_rfc3339();
    db.write(|conn| {
        conn.execute(
            "INSERT INTO matter_profile(matter_id,event_date,court_name,btl_claim_number,case_summary,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6)
             ON CONFLICT(matter_id) DO UPDATE SET
                event_date=excluded.event_date, court_name=excluded.court_name,
                btl_claim_number=excluded.btl_claim_number, case_summary=excluded.case_summary,
                updated_at=excluded.updated_at",
            params![matter_id, event_date, court_name, btl_claim_number, case_summary, now],
        )?;
        Ok(())
    })
}

/// A matter can exist with no profile row yet (it's only created once a lawyer saves
/// one) - this returns an empty-but-well-formed profile in that case rather than an error.
pub fn get_profile(db: &DbState, matter_id: &str) -> AppResult<MatterProfile> {
    db.read(|conn| {
        conn.query_row(
            "SELECT event_date,court_name,btl_claim_number,case_summary,updated_at
             FROM matter_profile WHERE matter_id=?1",
            [matter_id],
            |r| Ok(MatterProfile {
                matter_id: matter_id.to_string(),
                event_date: r.get(0)?, court_name: r.get(1)?,
                btl_claim_number: r.get(2)?, case_summary: r.get(3)?, updated_at: r.get(4)?,
            }),
        ).or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(MatterProfile {
                matter_id: matter_id.to_string(), event_date: None, court_name: None,
                btl_claim_number: None, case_summary: None, updated_at: String::new(),
            }),
            other => Err(AppError::Db(other)),
        })
    })
}

pub fn list_parties(db: &DbState, matter_id: &str) -> AppResult<Vec<MatterParty>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id,matter_id,role,name,contact_details,notes,created_at,updated_at
             FROM matter_parties WHERE matter_id=?1 ORDER BY role,name"
        )?;
        let rows = stmt.query_map([matter_id], |r| Ok(MatterParty {
            id: r.get(0)?, matter_id: r.get(1)?, role: r.get(2)?, name: r.get(3)?,
            contact_details: r.get(4)?, notes: r.get(5)?, created_at: r.get(6)?, updated_at: r.get(7)?,
        }))?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
}

pub fn add_party(
    db: &DbState, matter_id: &str, role: &str, name: &str,
    contact_details: Option<&str>, notes: Option<&str>,
) -> AppResult<String> {
    validate_party_role(role)?;
    if name.trim().is_empty() {
        return Err(AppError::Validation("party name required".into()));
    }
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    db.write(|conn| {
        conn.execute(
            "INSERT INTO matter_parties(id,matter_id,role,name,contact_details,notes,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?7)",
            params![id, matter_id, role, name, contact_details, notes, now],
        )?;
        Ok(())
    })?;
    Ok(id)
}

pub fn update_party(
    db: &DbState, party_id: &str, matter_id: &str, role: Option<&str>, name: Option<&str>,
    contact_details: Option<&str>, notes: Option<&str>,
) -> AppResult<()> {
    if let Some(r) = role { validate_party_role(r)?; }
    db.write(|conn| {
        let changed = conn.execute(
            "UPDATE matter_parties SET
                role=coalesce(?3,role), name=coalesce(?4,name),
                contact_details=coalesce(?5,contact_details), notes=coalesce(?6,notes),
                updated_at=?7
             WHERE id=?1 AND matter_id=?2",
            params![party_id, matter_id, role, name, contact_details, notes, Utc::now().to_rfc3339()],
        )?;
        if changed != 1 { return Err(AppError::NotFound("matter party".into())); }
        Ok(())
    })
}

pub fn delete_party(db: &DbState, party_id: &str, matter_id: &str) -> AppResult<()> {
    db.write(|conn| {
        let changed = conn.execute(
            "DELETE FROM matter_parties WHERE id=?1 AND matter_id=?2",
            params![party_id, matter_id],
        )?;
        if changed != 1 { return Err(AppError::NotFound("matter party".into())); }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_an_unknown_case_type() {
        assert!(validate_case_type("motor_vehicle_accident").is_ok());
        assert!(validate_case_type("made_up_type").is_err());
    }

    #[test]
    fn rejects_an_unknown_party_role() {
        assert!(validate_party_role("insurer").is_ok());
        assert!(validate_party_role("made_up_role").is_err());
    }

    #[test]
    fn rejects_a_malformed_event_date() {
        assert!(validate_event_date(None).is_ok());
        assert!(validate_event_date(Some("2026-03-12")).is_ok());
        assert!(validate_event_date(Some("12/03/2026")).is_err());
    }
}
