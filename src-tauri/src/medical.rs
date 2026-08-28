//! Phase C, milestone C3: Medical Evidence Intelligence - the read-only Medical
//! Timeline, Prior-vs-Post Incident view, and Medical Brief. Mirrors
//! `understanding.rs`'s pattern exactly: pure read models over already-
//! authoritative state (approved `medical_*` proposals from `ai.rs` plus the
//! pre-existing Medical Ledger, `medical_events`), no writes, no AI calls, no
//! change to the Medical Ledger's own semantics.
use crate::{db::DbState, error::AppResult};
use rusqlite::Connection;
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MedicalTimelineItem {
    pub id: String,
    pub kind: String,
    /// `None` means the source did not support a date - the item stays fully
    /// visible (undated items are never dropped), it is simply not placed in the
    /// dated chronology. Never assigned today's date as a fallback.
    pub business_date: Option<String>,
    pub date_precision: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub verified: bool,
    pub inserted_at: String,
}

/// Which arrays actually carry a timeline-meaningful date, per the C3 schema
/// (`complaints`/`findings`/`diagnoses`/`missingEvidenceSignals`/`contradictions`
/// have no date field at all - they appear in the timeline as undated items,
/// per the "unknown-date items must remain visible" rule, not excluded).
const TIMELINE_KINDS: &[&str] = &[
    "medical_encounter", "medical_complaint", "medical_finding", "medical_diagnosis",
    "medical_test", "medical_treatment", "medical_medication", "medical_referral",
    "medical_functional_status", "medical_disability_determination",
    "medical_prior_history", "medical_opinion",
];

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

/// Maps one proposal's structured JSON to `(businessDate, datePrecision, title,
/// description)` - each medical item type stores its date under a different field
/// name (see `ai.rs`'s per-kind schema), so this is a small per-kind lookup, never
/// a fabricated date when the type genuinely has none.
fn medical_item_display(kind: &str, v: &Value) -> (Option<String>, Option<String>, String, Option<String>) {
    match kind {
        "medical_encounter" => (
            str_field(v, "eventDate"), str_field(v, "datePrecision"),
            str_field(v, "encounterType").unwrap_or_else(|| "ביקור רפואי".to_string()),
            str_field(v, "provider"),
        ),
        "medical_complaint" => (None, None, "תלונה שדווחה".to_string(), str_field(v, "complaint")),
        "medical_finding" => (None, None, "ממצא שנרשם".to_string(), str_field(v, "finding")),
        "medical_diagnosis" => (None, None, "אבחנה שנרשמה במסמך".to_string(), str_field(v, "diagnosisText")),
        "medical_test" => (
            str_field(v, "performedDate").or_else(|| str_field(v, "resultDate")).or_else(|| str_field(v, "orderedDate")), None,
            str_field(v, "testType").unwrap_or_else(|| "בדיקה".to_string()),
            str_field(v, "interpretation"),
        ),
        "medical_treatment" => (
            str_field(v, "date"), None,
            str_field(v, "treatmentType").unwrap_or_else(|| "טיפול".to_string()),
            str_field(v, "outcome"),
        ),
        "medical_medication" => (
            str_field(v, "startDate"), None,
            str_field(v, "medication").unwrap_or_else(|| "תרופה".to_string()),
            str_field(v, "dosage"),
        ),
        "medical_referral" => (
            str_field(v, "date"), None,
            str_field(v, "planType").unwrap_or_else(|| "הפניה".to_string()),
            str_field(v, "target"),
        ),
        "medical_functional_status" => (
            str_field(v, "startDate"), None,
            str_field(v, "limitation").unwrap_or_else(|| "מגבלה תפקודית".to_string()),
            str_field(v, "workCapacityStatus"),
        ),
        "medical_disability_determination" => (
            str_field(v, "startDate"), None,
            str_field(v, "determiningBody").unwrap_or_else(|| "קביעת נכות".to_string()),
            str_field(v, "disabilityType"),
        ),
        "medical_prior_history" => (
            str_field(v, "date"), None, "היסטוריה רפואית קודמת".to_string(), str_field(v, "description"),
        ),
        "medical_opinion" => (
            str_field(v, "date"), None,
            str_field(v, "opinionType").unwrap_or_else(|| "חוות דעת".to_string()),
            str_field(v, "opinionText"),
        ),
        other => (None, None, other.to_string(), None),
    }
}

fn fetch_approved_medical_timeline_items(conn: &Connection, matter_id: &str) -> AppResult<Vec<MedicalTimelineItem>> {
    let placeholders = TIMELINE_KINDS.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT p.id,p.proposal_kind,p.structured_json,r.started_at
         FROM ai_proposals p
         JOIN ai_runs r ON r.id=p.ai_run_id AND r.matter_id=p.matter_id
         WHERE p.matter_id=? AND p.status='approved' AND p.proposal_kind IN ({placeholders})"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut params: Vec<&dyn rusqlite::ToSql> = vec![&matter_id];
    for k in TIMELINE_KINDS { params.push(k); }
    let rows: Vec<(String, String, String, String)> = stmt
        .query_map(params.as_slice(), |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut out = Vec::with_capacity(rows.len());
    for (id, proposal_kind, structured_json, started_at) in rows {
        let Ok(v) = serde_json::from_str::<Value>(&structured_json) else { continue; };
        let (business_date, date_precision, title, description) = medical_item_display(&proposal_kind, &v);
        out.push(MedicalTimelineItem {
            id, kind: proposal_kind, business_date, date_precision, title, description,
            verified: false, inserted_at: started_at,
        });
    }
    Ok(out)
}

fn fetch_ledger_medical_events(conn: &Connection, matter_id: &str, out: &mut Vec<MedicalTimelineItem>) -> AppResult<()> {
    let mut stmt = conn.prepare(
        "SELECT id,event_date,provider_name,treatment_summary,created_at
         FROM medical_events WHERE matter_id=?1 AND status='verified' AND event_date IS NOT NULL",
    )?;
    let rows: Vec<(String, String, Option<String>, String, String)> = stmt
        .query_map([matter_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, event_date, provider_name, treatment_summary, created_at) in rows {
        out.push(MedicalTimelineItem {
            id, kind: "medical_ledger_event".to_string(), business_date: Some(event_date), date_precision: None,
            title: provider_name.unwrap_or_else(|| "רשומת פנקס רפואי".to_string()),
            description: Some(treatment_summary), verified: true, inserted_at: created_at,
        });
    }
    Ok(())
}

/// Dated items sorted by business date (then `id` as a deterministic tie-break for
/// same/unknown dates); undated items are appended after, in their own stable
/// (`id`-sorted) block - never dropped, never given a fabricated date.
pub fn build_medical_timeline(db: &DbState, matter_id: &str) -> AppResult<Vec<MedicalTimelineItem>> {
    db.read(|conn| {
        let mut items = fetch_approved_medical_timeline_items(conn, matter_id)?;
        fetch_ledger_medical_events(conn, matter_id, &mut items)?;
        items.sort_by(|a, b| match (&a.business_date, &b.business_date) {
            (Some(x), Some(y)) => x.cmp(y).then_with(|| a.id.cmp(&b.id)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.id.cmp(&b.id),
        });
        Ok(items)
    })
}

/// A neutral comparison only - never a causation engine. Buckets approved medical
/// items as documented strictly before vs. on/after the matter's own recorded
/// incident date (`matter_profile.primary_event_date`); anything whose own date is
/// unknown, or whose comparison is impossible because the matter has no recorded
/// incident date, lands in `undated` rather than being guessed into either bucket.
/// `filter` (optional) narrows to items whose title/description contains it
/// case-insensitively - a simple, honest substring match, not a body-region taxonomy.
pub fn build_prior_vs_post_incident(db: &DbState, matter_id: &str, filter: Option<&str>) -> AppResult<Value> {
    db.read(|conn| {
        let incident_date: Option<String> = conn.query_row(
            "SELECT primary_event_date FROM matter_profile WHERE matter_id=?1", [matter_id], |r| r.get(0),
        ).ok().flatten();

        let items = fetch_approved_medical_timeline_items(conn, matter_id)?;
        let filter_lower = filter.map(str::to_lowercase);
        let matches = |item: &MedicalTimelineItem| -> bool {
            match &filter_lower {
                None => true,
                Some(f) => item.title.to_lowercase().contains(f.as_str())
                    || item.description.as_deref().map(|d| d.to_lowercase().contains(f.as_str())).unwrap_or(false),
            }
        };

        let mut before = Vec::new();
        let mut after = Vec::new();
        let mut undated = Vec::new();
        for item in items.into_iter().filter(matches) {
            match (&item.business_date, &incident_date) {
                (Some(d), Some(inc)) if d < inc => before.push(item),
                (Some(_), Some(_)) => after.push(item),
                _ => undated.push(item),
            }
        }
        Ok(json!({
            "incidentDate": incident_date,
            "documentedBefore": before,
            "documentedAfter": after,
            "undated": undated,
        }))
    })
}

/// A generated summary preferring approved/verified state; every not-yet-approved
/// item is labeled `pending: true` so it can never be presented as settled medical
/// evidence. Every section links back to `sourceIds` already carried on each item's
/// `structured` payload.
pub fn build_medical_brief(db: &DbState, matter_id: &str) -> AppResult<Value> {
    db.read(|conn| {
        let mut timeline = fetch_approved_medical_timeline_items(conn, matter_id)?;
        fetch_ledger_medical_events(conn, matter_id, &mut timeline)?;
        timeline.sort_by(|a, b| match (&a.business_date, &b.business_date) {
            (Some(x), Some(y)) => x.cmp(y).then_with(|| a.id.cmp(&b.id)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.id.cmp(&b.id),
        });

        let fetch_items = |kind: &str| -> AppResult<Vec<Value>> {
            let mut stmt = conn.prepare(
                "SELECT id,structured_json,status FROM ai_proposals \
                 WHERE matter_id=?1 AND proposal_kind=?2 AND status IN ('pending','approved') ORDER BY id"
            )?;
            let rows: Vec<(String, String, String)> = stmt
                .query_map(rusqlite::params![matter_id, kind], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(rows.into_iter().map(|(id, structured_json, status)| {
                let structured: Value = serde_json::from_str(&structured_json).unwrap_or(Value::Null);
                json!({"id": id, "status": status, "pending": status == "pending", "structured": structured})
            }).collect())
        };

        let encounters = fetch_items("medical_encounter")?;
        let complaints = fetch_items("medical_complaint")?;
        let findings = fetch_items("medical_finding")?;
        let diagnoses = fetch_items("medical_diagnosis")?;
        let tests = fetch_items("medical_test")?;
        let treatments = fetch_items("medical_treatment")?;
        let functional_statuses = fetch_items("medical_functional_status")?;
        let disability_determinations = fetch_items("medical_disability_determination")?;
        let prior_history = fetch_items("medical_prior_history")?;
        let opinions = fetch_items("medical_opinion")?;
        let gap_signals = fetch_items("medical_gap_signal")?;
        let missing_evidence_signals = fetch_items("medical_missing_evidence_signal")?;
        let contradictions = fetch_items("medical_contradiction")?;

        let pending_review_count = [
            &encounters, &complaints, &findings, &diagnoses, &tests, &treatments,
            &functional_statuses, &disability_determinations, &prior_history, &opinions,
            &gap_signals, &missing_evidence_signals, &contradictions,
        ].iter().flat_map(|v| v.iter()).filter(|item| item["pending"] == Value::Bool(true)).count() as i64;

        Ok(json!({
            "matterId": matter_id,
            "mainTreatmentHistory": encounters,
            "keyComplaints": complaints,
            "objectiveFindings": findings,
            "diagnoses": diagnoses,
            "testsImaging": tests,
            "treatments": treatments,
            "functionalWorkLimitations": functional_statuses,
            "disabilityDeterminations": disability_determinations,
            "priorDocumentedHistory": prior_history,
            "medicalOpinions": opinions,
            "candidateGaps": gap_signals,
            "missingEvidenceSignals": missing_evidence_signals,
            "contradictions": contradictions,
            "chronology": timeline,
            "pendingMedicalReviewCount": pending_review_count,
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
        let root = std::env::temp_dir().join(format!("tahrir-medical-{}", Uuid::new_v4()));
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
        }).unwrap();
        id
    }

    fn insert_approved_medical_proposal(db: &DbState, matter_id: &str, kind: &str, structured: Value) -> String {
        let proposal_id = Uuid::new_v4().to_string();
        let run_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO ai_runs(id,matter_id,capability,status,context_manifest_sha256,client_egress_approved,started_at,finished_at)
                 VALUES(?1,?2,'extract_medical_evidence','completed','sha',0,?3,?3)",
                params![run_id, matter_id, now],
            )?;
            conn.execute(
                "INSERT INTO ai_proposals(id,ai_run_id,matter_id,proposal_kind,structured_json,source_manifest_json,status)
                 VALUES(?1,?2,?3,?4,?5,'{}','approved')",
                params![proposal_id, run_id, matter_id, kind, serde_json::to_string(&structured).unwrap()],
            )?;
            Ok(())
        }).unwrap();
        proposal_id
    }

    #[test]
    fn timeline_places_dated_items_before_undated_and_sorts_deterministically() {
        let (db, root) = new_test_db();
        let matter_id = new_matter(&db);
        insert_approved_medical_proposal(&db, &matter_id, "medical_encounter", json!({
            "sourceIds":["s1"],"encounterType":"clinic_visit","provider":null,"institution":null,"specialty":null,
            "eventDate":"2024-05-01","datePrecision":"exact","documentDate":null,"confidence":null
        }));
        insert_approved_medical_proposal(&db, &matter_id, "medical_diagnosis", json!({
            "sourceIds":["s1"],"diagnosisText":"אבחנה ללא תאריך","code":null,"certainty":"confirmed","provider":null,"confidence":null
        }));
        let timeline = build_medical_timeline(&db, &matter_id).unwrap();
        assert_eq!(timeline.len(), 2);
        assert_eq!(timeline[0].business_date.as_deref(), Some("2024-05-01"), "the dated item must sort first");
        assert!(timeline[1].business_date.is_none(), "an item type with no date field must remain visible, undated, never given today's date");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn timeline_is_matter_isolated() {
        let (db, root) = new_test_db();
        let matter_a = new_matter(&db);
        let matter_b = new_matter(&db);
        insert_approved_medical_proposal(&db, &matter_a, "medical_treatment", json!({
            "sourceIds":["s1"],"treatmentType":"פיזיותרפיה","date":"2024-01-01","provider":null,"frequency":null,"outcome":null,"confidence":null
        }));
        assert_eq!(build_medical_timeline(&db, &matter_a).unwrap().len(), 1);
        assert!(build_medical_timeline(&db, &matter_b).unwrap().is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prior_vs_post_incident_is_neutral_and_never_asserts_causation() {
        let (db, root) = new_test_db();
        let matter_id = new_matter(&db);
        db.write(|conn| {
            conn.execute(
                "INSERT INTO matter_profile(matter_id,primary_event_date,updated_at) VALUES(?1,'2024-06-01',?2)",
                params![matter_id, Utc::now().to_rfc3339()],
            )?;
            Ok(())
        }).unwrap();
        insert_approved_medical_proposal(&db, &matter_id, "medical_treatment", json!({
            "sourceIds":["s1"],"treatmentType":"טיפול לפני האירוע","date":"2024-01-01","provider":null,"frequency":null,"outcome":null,"confidence":null
        }));
        insert_approved_medical_proposal(&db, &matter_id, "medical_treatment", json!({
            "sourceIds":["s1"],"treatmentType":"טיפול אחרי האירוע","date":"2024-07-01","provider":null,"frequency":null,"outcome":null,"confidence":null
        }));
        let view = build_prior_vs_post_incident(&db, &matter_id, None).unwrap();
        assert_eq!(view["documentedBefore"].as_array().unwrap().len(), 1);
        assert_eq!(view["documentedAfter"].as_array().unwrap().len(), 1);
        let serialized = view.to_string();
        assert!(!serialized.contains("caused"), "the view must never assert causation");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn brief_labels_pending_medical_content_explicitly() {
        let (db, root) = new_test_db();
        let matter_id = new_matter(&db);
        let proposal_id = Uuid::new_v4().to_string();
        let run_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO ai_runs(id,matter_id,capability,status,context_manifest_sha256,client_egress_approved,started_at,finished_at)
                 VALUES(?1,?2,'extract_medical_evidence','completed','sha',0,?3,?3)",
                params![run_id, matter_id, now],
            )?;
            conn.execute(
                "INSERT INTO ai_proposals(id,ai_run_id,matter_id,proposal_kind,structured_json,source_manifest_json,status)
                 VALUES(?1,?2,?3,'medical_diagnosis',?4,'{}','pending')",
                params![proposal_id, run_id, matter_id, r#"{"sourceIds":["s1"],"diagnosisText":"suspected fracture","code":null,"certainty":"suspected","provider":null,"confidence":null}"#],
            )?;
            Ok(())
        }).unwrap();
        let brief = build_medical_brief(&db, &matter_id).unwrap();
        let diagnoses = brief["diagnoses"].as_array().unwrap();
        assert_eq!(diagnoses.len(), 1);
        assert_eq!(diagnoses[0]["pending"], true, "a not-yet-approved diagnosis must be labeled pending, never presented as settled");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn repeated_timeline_reads_never_duplicate_an_existing_ledger_event() {
        let (db, root) = new_test_db();
        let matter_id = new_matter(&db);
        let now = Utc::now().to_rfc3339();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO medical_events(id,matter_id,event_date,provider_name,treatment_summary,status,verified_at,created_at,updated_at)
                 VALUES(?1,?2,'2024-01-01','בית חולים','טיפול','verified',?3,?3,?3)",
                params![Uuid::new_v4().to_string(), matter_id, now],
            )?;
            Ok(())
        }).unwrap();
        let first = build_medical_timeline(&db, &matter_id).unwrap();
        let second = build_medical_timeline(&db, &matter_id).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1, "the timeline is a read model over the existing Medical Ledger row - repeated reads must never duplicate it");
        assert_eq!(first[0].id, second[0].id);
        let _ = fs::remove_dir_all(root);
    }
}
