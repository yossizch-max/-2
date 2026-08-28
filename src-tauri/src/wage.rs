//! Phase C, milestone C4, Part A: Wage/Economic Evidence Intelligence - the
//! read-only Wage Timeline, Wage Comparison (neutral pre/post view), and Wage
//! Brief. Mirrors `medical.rs`'s pattern exactly: pure read models over already-
//! authoritative state (approved `wage_*` proposals from `ai.rs` plus the
//! pre-existing Wage Ledger, `wage_records`), no writes, no AI calls, no change to
//! the Wage Ledger's own semantics, and no wage-loss/earning-capacity calculation
//! anywhere in this module.
use crate::{db::DbState, error::AppResult};
use rusqlite::Connection;
use serde::Serialize;
use serde_json::{json, Value};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WageTimelineItem {
    pub id: String,
    pub kind: String,
    /// `None` means the source did not support a date - the item stays fully
    /// visible (undated items are never dropped), it is simply not placed in the
    /// dated chronology. Never assigned today's date as a fallback.
    pub business_date: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub verified: bool,
    pub inserted_at: String,
}

/// Which arrays carry a timeline-meaningful date (`wage_gap_signal` is a
/// review-only documentary signal, not a timeline event - same exclusion rule as
/// `medical.rs`'s `TIMELINE_KINDS`).
const TIMELINE_KINDS: &[&str] = &[
    "wage_employment", "wage_income", "wage_payslip", "wage_annual_income",
    "wage_absence", "wage_sick_leave", "wage_work_limitation",
    "wage_employment_change", "wage_benefit_payment",
];

fn str_field(v: &Value, key: &str) -> Option<String> {
    v.get(key).and_then(Value::as_str).map(str::to_string)
}

/// Maps one proposal's structured JSON to `(businessDate, title, description)` -
/// each wage item type stores its date under a different field name (see `ai.rs`'s
/// per-kind schema; a payslip's `month` is `YYYY-MM`, still a valid business-date
/// sort key by string comparison against `YYYY-MM-DD` dates).
fn wage_item_display(kind: &str, v: &Value) -> (Option<String>, String, Option<String>) {
    match kind {
        "wage_employment" => (
            str_field(v, "startDate"),
            str_field(v, "employer").unwrap_or_else(|| "העסקה".to_string()),
            str_field(v, "role"),
        ),
        "wage_income" => (
            str_field(v, "periodStart"), "הכנסה מתועדת".to_string(), str_field(v, "employerOrSource"),
        ),
        "wage_payslip" => (
            str_field(v, "month"), "תלוש שכר".to_string(), str_field(v, "components"),
        ),
        "wage_annual_income" => (
            str_field(v, "year").map(|y| format!("{y}-01-01")),
            str_field(v, "sourceType").unwrap_or_else(|| "הכנסה שנתית".to_string()),
            str_field(v, "employerOrSource"),
        ),
        "wage_absence" => (
            str_field(v, "startDate"), "היעדרות מהעבודה".to_string(), str_field(v, "statedReason"),
        ),
        "wage_sick_leave" => (
            str_field(v, "startDate"), "תעודת מחלה".to_string(), str_field(v, "issuingSource"),
        ),
        "wage_work_limitation" => (
            str_field(v, "startDate"),
            str_field(v, "limitation").unwrap_or_else(|| "מגבלת עבודה".to_string()),
            str_field(v, "workCapacityStatus"),
        ),
        "wage_employment_change" => (
            str_field(v, "date"),
            str_field(v, "changeType").unwrap_or_else(|| "שינוי תעסוקתי".to_string()),
            str_field(v, "description"),
        ),
        "wage_benefit_payment" => (
            str_field(v, "date"),
            str_field(v, "paymentType").unwrap_or_else(|| "תשלום/גמלה".to_string()),
            str_field(v, "payer"),
        ),
        other => (None, other.to_string(), None),
    }
}

fn fetch_approved_wage_timeline_items(conn: &Connection, matter_id: &str) -> AppResult<Vec<WageTimelineItem>> {
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
        let (business_date, title, description) = wage_item_display(&proposal_kind, &v);
        out.push(WageTimelineItem {
            id, kind: proposal_kind, business_date, title, description,
            verified: false, inserted_at: started_at,
        });
    }
    Ok(out)
}

fn fetch_ledger_wage_records(conn: &Connection, matter_id: &str, out: &mut Vec<WageTimelineItem>) -> AppResult<()> {
    let mut stmt = conn.prepare(
        "SELECT id,period_start,employer_name,gross_amount_cents,created_at
         FROM wage_records WHERE matter_id=?1 AND status='verified' AND period_start IS NOT NULL",
    )?;
    let rows: Vec<(String, String, Option<String>, i64, String)> = stmt
        .query_map([matter_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)))?
        .collect::<Result<Vec<_>, _>>()?;
    for (id, period_start, employer_name, gross_amount_cents, created_at) in rows {
        out.push(WageTimelineItem {
            id, kind: "wage_ledger_record".to_string(), business_date: Some(period_start),
            title: employer_name.unwrap_or_else(|| "רשומת פנקס שכר".to_string()),
            description: Some(format!("{:.2} ש\"ח ברוטו", gross_amount_cents as f64 / 100.0)),
            verified: true, inserted_at: created_at,
        });
    }
    Ok(())
}

/// Dated items sorted by business date (then `id` as a deterministic tie-break);
/// undated items appended after, in their own stable (`id`-sorted) block - never
/// dropped, never given a fabricated date.
pub fn build_wage_timeline(db: &DbState, matter_id: &str) -> AppResult<Vec<WageTimelineItem>> {
    db.read(|conn| {
        let mut items = fetch_approved_wage_timeline_items(conn, matter_id)?;
        fetch_ledger_wage_records(conn, matter_id, &mut items)?;
        items.sort_by(|a, b| match (&a.business_date, &b.business_date) {
            (Some(x), Some(y)) => x.cmp(y).then_with(|| a.id.cmp(&b.id)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.id.cmp(&b.id),
        });
        Ok(items)
    })
}

/// A neutral comparison only - never a wage-loss calculator. Buckets approved wage
/// items as documented strictly before vs. on/after the matter's own recorded
/// incident date (`matter_profile.primary_event_date`); anything whose own date is
/// unknown, or whose comparison is impossible because the matter has no recorded
/// incident date, lands in `undated` rather than being guessed into either bucket.
pub fn build_wage_comparison(db: &DbState, matter_id: &str, filter: Option<&str>) -> AppResult<Value> {
    db.read(|conn| {
        let incident_date: Option<String> = conn.query_row(
            "SELECT primary_event_date FROM matter_profile WHERE matter_id=?1", [matter_id], |r| r.get(0),
        ).ok().flatten();

        let items = fetch_approved_wage_timeline_items(conn, matter_id)?;
        let filter_lower = filter.map(str::to_lowercase);
        let matches = |item: &WageTimelineItem| -> bool {
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
/// item is labeled `pending: true` so it can never be presented as settled wage
/// evidence. Never computes actual wage loss - only organizes what was documented.
pub fn build_wage_brief(db: &DbState, matter_id: &str) -> AppResult<Value> {
    db.read(|conn| {
        let mut timeline = fetch_approved_wage_timeline_items(conn, matter_id)?;
        fetch_ledger_wage_records(conn, matter_id, &mut timeline)?;
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

        let employment = fetch_items("wage_employment")?;
        let income = fetch_items("wage_income")?;
        let payslips = fetch_items("wage_payslip")?;
        let annual_income = fetch_items("wage_annual_income")?;
        let absences = fetch_items("wage_absence")?;
        let sick_leave = fetch_items("wage_sick_leave")?;
        let work_limitations = fetch_items("wage_work_limitation")?;
        let employment_changes = fetch_items("wage_employment_change")?;
        let benefit_payments = fetch_items("wage_benefit_payment")?;
        let gap_signals = fetch_items("wage_gap_signal")?;

        let pending_review_count = [
            &employment, &income, &payslips, &annual_income, &absences, &sick_leave,
            &work_limitations, &employment_changes, &benefit_payments, &gap_signals,
        ].iter().flat_map(|v| v.iter()).filter(|item| item["pending"] == Value::Bool(true)).count() as i64;

        Ok(json!({
            "matterId": matter_id,
            "employment": employment,
            "income": income,
            "payslips": payslips,
            "annualIncome": annual_income,
            "absences": absences,
            "sickLeave": sick_leave,
            "workLimitations": work_limitations,
            "employmentChanges": employment_changes,
            "benefitPayments": benefit_payments,
            "missingEvidenceSignals": gap_signals,
            "chronology": timeline,
            "pendingWageReviewCount": pending_review_count,
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
        let root = std::env::temp_dir().join(format!("tahrir-wage-{}", Uuid::new_v4()));
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

    fn insert_approved_wage_proposal(db: &DbState, matter_id: &str, kind: &str, structured: Value) -> String {
        let proposal_id = Uuid::new_v4().to_string();
        let run_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO ai_runs(id,matter_id,capability,status,context_manifest_sha256,client_egress_approved,started_at,finished_at)
                 VALUES(?1,?2,'extract_wage_evidence','completed','sha',0,?3,?3)",
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
    fn timeline_sorts_by_wage_business_period_and_keeps_undated_items_visible() {
        let (db, root) = new_test_db();
        let matter_id = new_matter(&db);
        insert_approved_wage_proposal(&db, &matter_id, "wage_payslip", json!({
            "sourceIds":["s1"],"month":"2024-05","grossAmountCents":1000000,"netAmountCents":800000,"components":null,"confidence":null
        }));
        insert_approved_wage_proposal(&db, &matter_id, "wage_income", json!({
            "sourceIds":["s1"],"amountCents":500000,"amountBasis":"gross","incomeType":"self_employed",
            "employerOrSource":null,"periodStart":null,"periodEnd":null,"currency":"ILS","confidence":null
        }));
        let timeline = build_wage_timeline(&db, &matter_id).unwrap();
        assert_eq!(timeline.len(), 2);
        assert_eq!(timeline[0].business_date.as_deref(), Some("2024-05"));
        assert!(timeline[1].business_date.is_none(), "an income record with no stated period must remain visible, undated");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn timeline_is_matter_isolated() {
        let (db, root) = new_test_db();
        let matter_a = new_matter(&db);
        let matter_b = new_matter(&db);
        insert_approved_wage_proposal(&db, &matter_a, "wage_employment", json!({
            "sourceIds":["s1"],"employer":"חברה בע\"מ","role":null,"employmentStatus":"employee",
            "startDate":"2020-01-01","endDate":null,"confidence":null
        }));
        assert_eq!(build_wage_timeline(&db, &matter_a).unwrap().len(), 1);
        assert!(build_wage_timeline(&db, &matter_b).unwrap().is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn wage_comparison_is_neutral_and_never_computes_a_loss_figure() {
        let (db, root) = new_test_db();
        let matter_id = new_matter(&db);
        db.write(|conn| {
            conn.execute(
                "INSERT INTO matter_profile(matter_id,primary_event_date,updated_at) VALUES(?1,'2024-06-01',?2)",
                params![matter_id, Utc::now().to_rfc3339()],
            )?;
            Ok(())
        }).unwrap();
        insert_approved_wage_proposal(&db, &matter_id, "wage_payslip", json!({
            "sourceIds":["s1"],"month":"2024-01","grossAmountCents":1200000,"netAmountCents":null,"components":null,"confidence":null
        }));
        insert_approved_wage_proposal(&db, &matter_id, "wage_payslip", json!({
            "sourceIds":["s1"],"month":"2024-07","grossAmountCents":600000,"netAmountCents":null,"components":null,"confidence":null
        }));
        let view = build_wage_comparison(&db, &matter_id, None).unwrap();
        assert_eq!(view["documentedBefore"].as_array().unwrap().len(), 1);
        assert_eq!(view["documentedAfter"].as_array().unwrap().len(), 1);
        let serialized = view.to_string();
        assert!(!serialized.contains("caused") && !serialized.contains("lossCents"), "the comparison must stay neutral and never compute a loss figure");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn brief_labels_pending_wage_content_explicitly() {
        let (db, root) = new_test_db();
        let matter_id = new_matter(&db);
        let proposal_id = Uuid::new_v4().to_string();
        let run_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO ai_runs(id,matter_id,capability,status,context_manifest_sha256,client_egress_approved,started_at,finished_at)
                 VALUES(?1,?2,'extract_wage_evidence','completed','sha',0,?3,?3)",
                params![run_id, matter_id, now],
            )?;
            conn.execute(
                "INSERT INTO ai_proposals(id,ai_run_id,matter_id,proposal_kind,structured_json,source_manifest_json,status)
                 VALUES(?1,?2,?3,'wage_annual_income',?4,'{}','pending')",
                params![proposal_id, run_id, matter_id, r#"{"sourceIds":["s1"],"sourceType":"form_106","year":"2023","amountCents":10000000,"employerOrSource":null,"confidence":null}"#],
            )?;
            Ok(())
        }).unwrap();
        let brief = build_wage_brief(&db, &matter_id).unwrap();
        let annual = brief["annualIncome"].as_array().unwrap();
        assert_eq!(annual.len(), 1);
        assert_eq!(annual[0]["pending"], true, "a not-yet-approved annual income item must be labeled pending, never presented as settled");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn repeated_timeline_reads_never_duplicate_an_existing_ledger_record() {
        let (db, root) = new_test_db();
        let matter_id = new_matter(&db);
        let now = Utc::now().to_rfc3339();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO wage_records(id,matter_id,period_start,employer_name,gross_amount_cents,status,verified_at,created_at,updated_at)
                 VALUES(?1,?2,'2024-01-01','מעסיק',1000000,'verified',?3,?3,?3)",
                params![Uuid::new_v4().to_string(), matter_id, now],
            )?;
            Ok(())
        }).unwrap();
        let first = build_wage_timeline(&db, &matter_id).unwrap();
        let second = build_wage_timeline(&db, &matter_id).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1, "the timeline is a read model over the existing Wage Ledger row - repeated reads must never duplicate it");
        assert_eq!(first[0].id, second[0].id);
        let _ = fs::remove_dir_all(root);
    }
}
