//! Phase C, milestone C2: Matter Understanding Core - the read-only Timeline and
//! Brief views. Neither view writes anything or duplicates a source record; both
//! are computed entirely over already-authoritative state:
//!   - verified ledger rows (`medical_events`/`wage_records`, `status='verified'`)
//!   - office data (`insurance_claim_status_history`, `negotiation_events`,
//!     `calendar_events`)
//!   - *approved* `understanding_event` AI proposals from `ai.rs` (never trusted as
//!     "verified" - a lawyer reviewed and accepted the item as noteworthy, which is
//!     a weaker claim than a committed ledger row, so the Timeline/Brief label it
//!     distinctly)
//! See `ai.rs` for how Matter Understanding proposals are produced and approved -
//! this module never calls an AI provider and never writes `ai_proposals`.
use crate::{db::DbState, error::AppResult};
use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TimelineItem {
    pub id: String,
    pub kind: String,
    pub business_date: String,
    pub title: String,
    pub description: Option<String>,
    pub verified: bool,
    pub inserted_at: String,
}

/// Approved `understanding_event` proposals with a known `eventDate`. An item whose
/// date the source did not support ("unknown stays unknown", per C2) has nothing to
/// sort a date-ordered timeline by, so it is excluded here rather than given a
/// fabricated date - it remains fully visible in the Matter Understanding review
/// screen either way.
fn push_understanding_events(conn: &Connection, matter_id: &str, out: &mut Vec<TimelineItem>) -> AppResult<()> {
    let mut stmt = conn.prepare(
        "SELECT p.id,p.structured_json,r.started_at
         FROM ai_proposals p
         JOIN ai_runs r ON r.id=p.ai_run_id AND r.matter_id=p.matter_id
         WHERE p.matter_id=?1 AND p.proposal_kind='understanding_event' AND p.status='approved'",
    )?;
    let rows: Vec<(String, String, String)> = stmt
        .query_map([matter_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, structured_json, started_at) in rows {
        let Ok(v) = serde_json::from_str::<Value>(&structured_json) else { continue; };
        let Some(date) = v.get("eventDate").and_then(Value::as_str) else { continue; };
        out.push(TimelineItem {
            id,
            kind: "understanding_event".to_string(),
            business_date: date.to_string(),
            title: v.get("title").and_then(Value::as_str).unwrap_or("אירוע").to_string(),
            description: v.get("description").and_then(Value::as_str).map(str::to_string),
            verified: false,
            inserted_at: started_at,
        });
    }
    Ok(())
}

fn push_medical_events(conn: &Connection, matter_id: &str, out: &mut Vec<TimelineItem>) -> AppResult<()> {
    let mut stmt = conn.prepare(
        "SELECT id,event_date,provider_name,treatment_summary,created_at
         FROM medical_events WHERE matter_id=?1 AND status='verified' AND event_date IS NOT NULL",
    )?;
    let rows: Vec<(String, String, Option<String>, String, String)> = stmt
        .query_map([matter_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, event_date, provider_name, treatment_summary, created_at) in rows {
        out.push(TimelineItem {
            id, kind: "medical_event".to_string(), business_date: event_date,
            title: provider_name.unwrap_or_else(|| "אירוע רפואי".to_string()),
            description: Some(treatment_summary), verified: true, inserted_at: created_at,
        });
    }
    Ok(())
}

fn push_wage_records(conn: &Connection, matter_id: &str, out: &mut Vec<TimelineItem>) -> AppResult<()> {
    let mut stmt = conn.prepare(
        "SELECT id,period_start,employer_name,gross_amount_cents,created_at
         FROM wage_records WHERE matter_id=?1 AND status='verified' AND period_start IS NOT NULL",
    )?;
    let rows: Vec<(String, String, Option<String>, i64, String)> = stmt
        .query_map([matter_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, period_start, employer_name, gross_amount_cents, created_at) in rows {
        out.push(TimelineItem {
            id, kind: "wage_record".to_string(), business_date: period_start,
            title: employer_name.unwrap_or_else(|| "רשומת שכר".to_string()),
            description: Some(format!("שכר ברוטו {:.2} ₪", gross_amount_cents as f64 / 100.0)),
            verified: true, inserted_at: created_at,
        });
    }
    Ok(())
}

fn push_insurance_status_history(conn: &Connection, matter_id: &str, out: &mut Vec<TimelineItem>) -> AppResult<()> {
    let mut stmt = conn.prepare(
        "SELECT h.id,h.changed_at,h.to_status,h.note,c.insurer_name
         FROM insurance_claim_status_history h
         JOIN insurance_claims c ON c.id=h.insurance_claim_id AND c.matter_id=h.matter_id
         WHERE h.matter_id=?1",
    )?;
    let rows: Vec<(String, String, String, Option<String>, String)> = stmt
        .query_map([matter_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, changed_at, to_status, note, insurer_name) in rows {
        out.push(TimelineItem {
            id, kind: "insurance_status".to_string(), business_date: changed_at.clone(),
            title: format!("{insurer_name} · {to_status}"), description: note,
            verified: true, inserted_at: changed_at,
        });
    }
    Ok(())
}

fn push_negotiation_events(conn: &Connection, matter_id: &str, out: &mut Vec<TimelineItem>) -> AppResult<()> {
    let mut stmt = conn.prepare(
        "SELECT id,happened_at,event_kind,summary FROM negotiation_events WHERE matter_id=?1",
    )?;
    let rows: Vec<(String, String, String, String)> = stmt
        .query_map([matter_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, happened_at, event_kind, summary) in rows {
        out.push(TimelineItem {
            id, kind: "negotiation_event".to_string(), business_date: happened_at.clone(),
            title: event_kind, description: Some(summary), verified: true, inserted_at: happened_at,
        });
    }
    Ok(())
}

fn push_calendar_events(conn: &Connection, matter_id: &str, out: &mut Vec<TimelineItem>) -> AppResult<()> {
    let mut stmt = conn.prepare(
        "SELECT id,starts_at,title,event_kind FROM calendar_events WHERE matter_id=?1",
    )?;
    let rows: Vec<(String, String, String, String)> = stmt
        .query_map([matter_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, starts_at, title, event_kind) in rows {
        out.push(TimelineItem {
            id, kind: "calendar_event".to_string(), business_date: starts_at.clone(),
            title, description: Some(event_kind), verified: true, inserted_at: starts_at,
        });
    }
    Ok(())
}

/// Sorted strictly by business/event date - never by `inserted_at`, which is kept
/// on each item for audit purposes only. `business_date` may be a date or a full
/// RFC3339 timestamp depending on source table; a plain lexicographic sort is still
/// correct because both formats share the same `YYYY-MM-DD` prefix ordering.
pub fn build_matter_timeline(db: &DbState, matter_id: &str) -> AppResult<Vec<TimelineItem>> {
    db.read(|conn| {
        let mut items = Vec::new();
        push_understanding_events(conn, matter_id, &mut items)?;
        push_medical_events(conn, matter_id, &mut items)?;
        push_wage_records(conn, matter_id, &mut items)?;
        push_insurance_status_history(conn, matter_id, &mut items)?;
        push_negotiation_events(conn, matter_id, &mut items)?;
        push_calendar_events(conn, matter_id, &mut items)?;
        items.sort_by(|a, b| a.business_date.cmp(&b.business_date).then_with(|| a.id.cmp(&b.id)));
        Ok(items)
    })
}

fn count(conn: &Connection, sql: &str, matter_id: &str) -> AppResult<i64> {
    Ok(conn.query_row(sql, [matter_id], |r| r.get(0))?)
}

/// A generated summary of already-approved/verified state, with any still-pending
/// AI content explicitly labeled as such (never presented as settled). Every claim,
/// amount, and contradiction section links back to its own `sourceIds` so a lawyer
/// can open the underlying document - the brief never states anything it cannot
/// trace to a real source.
pub fn build_matter_brief(db: &DbState, matter_id: &str) -> AppResult<Value> {
    db.read(|conn| {
        let parties = crate::matter_profile::list_parties(db, matter_id)?;
        let profile = crate::matter_profile::get_profile(db, matter_id)?;
        let mut timeline = Vec::new();
        push_understanding_events(conn, matter_id, &mut timeline)?;
        push_medical_events(conn, matter_id, &mut timeline)?;
        push_wage_records(conn, matter_id, &mut timeline)?;
        push_insurance_status_history(conn, matter_id, &mut timeline)?;
        push_negotiation_events(conn, matter_id, &mut timeline)?;
        push_calendar_events(conn, matter_id, &mut timeline)?;
        timeline.sort_by(|a, b| a.business_date.cmp(&b.business_date).then_with(|| a.id.cmp(&b.id)));

        let fetch_items = |kind: &str, approved_only: bool| -> AppResult<Vec<Value>> {
            let status_clause = if approved_only { "AND status='approved'" } else { "AND status IN ('pending','approved')" };
            let sql = format!(
                "SELECT id,structured_json,status FROM ai_proposals WHERE matter_id=?1 AND proposal_kind=?2 {status_clause} ORDER BY id"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows: Vec<(String, String, String)> = stmt
                .query_map(rusqlite::params![matter_id, kind], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows
                .into_iter()
                .map(|(id, structured_json, status)| {
                    let structured: Value = serde_json::from_str(&structured_json).unwrap_or(Value::Null);
                    serde_json::json!({"id": id, "status": status, "pending": status == "pending", "structured": structured})
                })
                .collect())
        };

        let claims = fetch_items("understanding_claim", false)?;
        let amounts = fetch_items("understanding_amount", false)?;
        let contradictions = fetch_items("understanding_contradiction", false)?;
        let entities = fetch_items("understanding_entity", false)?;
        let missing_info = fetch_items("understanding_question", false)?;

        let verified_fact_count = count(conn, "SELECT count(*) FROM verified_facts WHERE matter_id=?1 AND status='valid'", matter_id)?;
        let open_conflict_count = count(conn, "SELECT count(*) FROM fact_conflicts WHERE matter_id=?1 AND status='unresolved'", matter_id)?;

        Ok(serde_json::json!({
            "matterId": matter_id,
            "profile": profile,
            "parties": parties,
            "entities": entities,
            "chronology": timeline,
            "claims": claims,
            "amounts": amounts,
            "contradictions": contradictions,
            "missingInformation": missing_info,
            "verifiedFactCount": verified_fact_count,
            "openConflictCount": open_conflict_count,
        }))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rusqlite::params;
    use std::fs;
    use uuid::Uuid;

    fn new_test_db() -> (DbState, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!("tahrir-understanding-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let db = DbState::open(root.join("app.db")).unwrap();
        (db, root)
    }

    fn new_matter(db: &DbState) -> String {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO matters(id,title,matter_type,created_at,updated_at) VALUES(?1,'Matter','generic_civil',?2,?2)",
                params![id, now],
            )?;
            Ok(())
        })
        .unwrap();
        id
    }

    #[test]
    fn timeline_sorts_by_business_date_not_insertion_order() {
        let (db, root) = new_test_db();
        let matter_id = new_matter(&db);
        let now = Utc::now().to_rfc3339();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO calendar_events(id,matter_id,title,starts_at,event_kind,status,created_at)
                 VALUES(?1,?2,'דיון מאוחר','2026-06-01T00:00:00Z','hearing','active',?3)",
                params![Uuid::new_v4().to_string(), matter_id, now],
            )?;
            conn.execute(
                "INSERT INTO calendar_events(id,matter_id,title,starts_at,event_kind,status,created_at)
                 VALUES(?1,?2,'אירוע מוקדם','2026-01-01T00:00:00Z','hearing','active',?3)",
                params![Uuid::new_v4().to_string(), matter_id, now],
            )?;
            Ok(())
        })
        .unwrap();
        let timeline = build_matter_timeline(&db, &matter_id).unwrap();
        assert_eq!(timeline.len(), 2);
        assert_eq!(timeline[0].title, "אירוע מוקדם", "the earlier business date must sort first even though it was inserted second");
        assert_eq!(timeline[1].title, "דיון מאוחר");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn timeline_is_matter_isolated() {
        let (db, root) = new_test_db();
        let matter_a = new_matter(&db);
        let matter_b = new_matter(&db);
        let now = Utc::now().to_rfc3339();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO calendar_events(id,matter_id,title,starts_at,event_kind,status,created_at)
                 VALUES(?1,?2,'רק בתיק א','2026-01-01T00:00:00Z','hearing','active',?3)",
                params![Uuid::new_v4().to_string(), matter_a, now],
            )?;
            Ok(())
        })
        .unwrap();
        assert_eq!(build_matter_timeline(&db, &matter_a).unwrap().len(), 1);
        assert!(build_matter_timeline(&db, &matter_b).unwrap().is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn brief_labels_pending_ai_content_explicitly() {
        let (db, root) = new_test_db();
        let matter_id = new_matter(&db);
        let now = Utc::now().to_rfc3339();
        let run_id = Uuid::new_v4().to_string();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO ai_runs(id,matter_id,capability,status,context_manifest_sha256,client_egress_approved,started_at,finished_at)
                 VALUES(?1,?2,'extract_matter_understanding','completed','sha',0,?3,?3)",
                params![run_id, matter_id, now],
            )?;
            conn.execute(
                "INSERT INTO ai_proposals(id,ai_run_id,matter_id,proposal_kind,structured_json,source_manifest_json,status)
                 VALUES(?1,?2,?3,'understanding_claim',?4,'{}','pending')",
                params![Uuid::new_v4().to_string(), run_id, matter_id, r#"{"sourceIds":["s1"],"assertedBy":"תובע","statement":"טענה","target":null,"confidence":null}"#],
            )?;
            Ok(())
        })
        .unwrap();
        let brief = build_matter_brief(&db, &matter_id).unwrap();
        let claims = brief["claims"].as_array().unwrap();
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0]["pending"], true, "a not-yet-approved claim must be labeled pending, never presented as settled");
        let _ = fs::remove_dir_all(root);
    }
}
