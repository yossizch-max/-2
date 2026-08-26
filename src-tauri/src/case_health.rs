//! Phase B, milestone B6: computed Case Health + Next Best Action.
//!
//! This module is deliberately read-only. It does not persist a score, mutate a
//! task/workstream/ledger, or infer substantive law. It summarizes already-existing
//! operational state into a transparent score and one deterministic next action.
//! Legal deadlines only receive deadline weight after the existing human `commit`
//! transition; draft deadlines are surfaced merely as items still awaiting review.
use crate::{
    db::DbState,
    error::{AppError, AppResult},
    requirements, workstreams, AppState,
};
use chrono::{NaiveDate, Utc};
use rusqlite::{params, Connection};
use serde::Serialize;
use serde_json::Value;
use tauri::State;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HealthFactor {
    pub code: String,
    pub severity: String,
    pub count: i64,
    pub penalty: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NextBestAction {
    pub code: String,
    pub priority: String,
    pub target_id: Option<String>,
    pub due_at: Option<String>,
    pub label: Option<String>,
    pub secondary_label: Option<String>,
    pub requirement_key: Option<String>,
    pub workstream_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CaseHealthSnapshot {
    pub matter_id: String,
    pub score: i64,
    pub band: String,
    pub as_of: String,
    pub factors: Vec<HealthFactor>,
    pub next_best_action: NextBestAction,
}

#[derive(Debug, Clone)]
struct DeadlineSignal {
    id: String,
    action: String,
    due_at: String,
    days_until: i64,
}

#[derive(Debug, Clone)]
struct TaskSignal {
    id: String,
    title: String,
    due_at: Option<String>,
    days_until: Option<i64>,
}

#[derive(Debug, Clone)]
struct WaitingSignal {
    id: String,
    party_label: String,
    item_label: String,
    follow_up_at: Option<String>,
    days_until: Option<i64>,
}

fn storage_date(value: &str) -> Option<NaiveDate> {
    let prefix: String = value.chars().take(10).collect();
    if prefix.len() != 10 {
        return None;
    }
    NaiveDate::parse_from_str(&prefix, "%Y-%m-%d").ok()
}

fn days_until(value: &str, today: NaiveDate) -> Option<i64> {
    storage_date(value).map(|date| date.signed_duration_since(today).num_days())
}

fn push_factor(
    factors: &mut Vec<HealthFactor>,
    code: &str,
    severity: &str,
    count: i64,
    per_item: i64,
    cap: i64,
) -> i64 {
    if count <= 0 {
        return 0;
    }
    let penalty = (count.saturating_mul(per_item)).min(cap);
    factors.push(HealthFactor {
        code: code.to_string(),
        severity: severity.to_string(),
        count,
        penalty,
    });
    penalty
}

fn ledger_count(conn: &Connection, table: &str, matter_id: &str, stale_only: bool) -> AppResult<i64> {
    let stale_clause = if stale_only { "AND e.status='verified' AND e.stale=1" } else { "AND e.status='draft'" };
    let superseded_clause = if stale_only {
        format!(
            "AND NOT EXISTS (SELECT 1 FROM {table} s WHERE s.matter_id=e.matter_id \
             AND s.status='verified' AND s.supersedes_entry_id=e.id)"
        )
    } else {
        String::new()
    };
    let sql = format!(
        "SELECT COUNT(*) FROM {table} e WHERE e.matter_id=?1 {stale_clause} {superseded_clause}"
    );
    Ok(conn.query_row(&sql, [matter_id], |r| r.get(0))?)
}

pub(crate) fn compute(db: &DbState, matter_id: &str) -> AppResult<CaseHealthSnapshot> {
    let now = Utc::now();
    let today = now.date_naive();
    db.read(|conn| {
        let case_type: String = conn
            .query_row("SELECT matter_type FROM matters WHERE id=?1", [matter_id], |r| r.get(0))
            .map_err(|_| AppError::NotFound("matter".into()))?;

        let workstream_rows = workstreams::list(conn, matter_id)?;
        let requirement_rows = requirements::list(conn, matter_id, &case_type)?;

        let blocked_workstreams: Vec<String> = workstream_rows
            .iter()
            .filter(|w| w.status == "blocked")
            .map(|w| w.kind.clone())
            .collect();
        let not_started_workstreams: Vec<String> = workstream_rows
            .iter()
            .filter(|w| w.status == "not_started")
            .map(|w| w.kind.clone())
            .collect();

        let mut required_missing = Vec::new();
        let mut required_stale = Vec::new();
        let mut required_requested = Vec::new();
        let mut recommended_open = 0_i64;
        for req in &requirement_rows {
            if req.relevance != "applicable" {
                continue;
            }
            match (req.priority.as_deref(), req.status.as_str()) {
                (Some("required_by_office_policy"), "not_collected") => required_missing.push(req.requirement_key.clone()),
                (Some("required_by_office_policy"), "stale") => required_stale.push(req.requirement_key.clone()),
                (Some("required_by_office_policy"), "requested") => required_requested.push(req.requirement_key.clone()),
                (Some("recommended"), "not_collected" | "stale") => recommended_open += 1,
                _ => {}
            }
        }

        let mut committed_deadlines = Vec::new();
        let mut draft_deadline_count = 0_i64;
        {
            let mut stmt = conn.prepare(
                "SELECT id,action,due_at,state FROM legal_deadlines
                 WHERE matter_id=?1 AND state IN ('draft','committed') ORDER BY due_at,id",
            )?;
            let rows = stmt
                .query_map([matter_id], |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            for (id, action, due_at, state) in rows {
                if state == "draft" {
                    draft_deadline_count += 1;
                } else if let Some(days) = days_until(&due_at, today) {
                    committed_deadlines.push(DeadlineSignal { id, action, due_at, days_until: days });
                }
            }
        }
        let overdue_deadlines: Vec<DeadlineSignal> = committed_deadlines
            .iter()
            .filter(|d| d.days_until < 0)
            .cloned()
            .collect();
        let due_soon_deadlines: Vec<DeadlineSignal> = committed_deadlines
            .iter()
            .filter(|d| (0..=7).contains(&d.days_until))
            .cloned()
            .collect();

        let mut open_tasks = Vec::new();
        {
            let mut stmt = conn.prepare(
                "SELECT id,title,due_at FROM tasks WHERE matter_id=?1 AND status='open'
                 ORDER BY CASE WHEN due_at IS NULL THEN 1 ELSE 0 END,due_at,id",
            )?;
            open_tasks = stmt
                .query_map([matter_id], |r| {
                    let due_at: Option<String> = r.get(2)?;
                    Ok(TaskSignal {
                        id: r.get(0)?,
                        title: r.get(1)?,
                        days_until: due_at.as_deref().and_then(|v| days_until(v, today)),
                        due_at,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
        }
        let overdue_tasks: Vec<TaskSignal> = open_tasks
            .iter()
            .filter(|t| t.days_until.is_some_and(|d| d < 0))
            .cloned()
            .collect();

        let mut open_waiting = Vec::new();
        {
            let mut stmt = conn.prepare(
                "SELECT id,party_label,item_label,follow_up_at FROM waiting_for
                 WHERE matter_id=?1 AND status='open'
                 ORDER BY CASE WHEN follow_up_at IS NULL THEN 1 ELSE 0 END,follow_up_at,id",
            )?;
            open_waiting = stmt
                .query_map([matter_id], |r| {
                    let follow_up_at: Option<String> = r.get(3)?;
                    Ok(WaitingSignal {
                        id: r.get(0)?,
                        party_label: r.get(1)?,
                        item_label: r.get(2)?,
                        days_until: follow_up_at.as_deref().and_then(|v| days_until(v, today)),
                        follow_up_at,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
        }
        let overdue_waiting: Vec<WaitingSignal> = open_waiting
            .iter()
            .filter(|w| w.days_until.is_some_and(|d| d < 0))
            .cloned()
            .collect();

        let stale_verified_facts: i64 = conn.query_row(
            "SELECT COUNT(*) FROM verified_facts WHERE matter_id=?1 AND status='valid' AND stale=1",
            [matter_id],
            |r| r.get(0),
        )?;
        let stale_verified_ledgers = ledger_count(conn, "medical_events", matter_id, true)?
            + ledger_count(conn, "wage_records", matter_id, true)?
            + ledger_count(conn, "liability_facts", matter_id, true)?;
        let ledger_drafts = ledger_count(conn, "medical_events", matter_id, false)?
            + ledger_count(conn, "wage_records", matter_id, false)?
            + ledger_count(conn, "liability_facts", matter_id, false)?;
        let pending_ai_review: i64 = conn.query_row(
            "SELECT COUNT(*) FROM ai_proposals WHERE matter_id=?1 AND status='pending'",
            [matter_id],
            |r| r.get(0),
        )?;
        let documents_needing_attention: i64 = conn.query_row(
            "SELECT COUNT(*) FROM documents WHERE matter_id=?1 AND extraction_state IN ('stale','blocked')",
            [matter_id],
            |r| r.get(0),
        )?;

        let mut factors = Vec::new();
        let mut total_penalty = 0_i64;
        total_penalty += push_factor(&mut factors, "overdue_committed_deadlines", "critical", overdue_deadlines.len() as i64, 30, 60);
        total_penalty += push_factor(&mut factors, "due_soon_committed_deadlines", "high", due_soon_deadlines.len() as i64, 12, 24);
        total_penalty += push_factor(&mut factors, "overdue_tasks", "high", overdue_tasks.len() as i64, 10, 30);
        total_penalty += push_factor(&mut factors, "blocked_workstreams", "high", blocked_workstreams.len() as i64, 12, 24);
        total_penalty += push_factor(&mut factors, "required_evidence_stale", "high", required_stale.len() as i64, 10, 20);
        total_penalty += push_factor(&mut factors, "required_evidence_missing", "high", required_missing.len() as i64, 8, 24);
        total_penalty += push_factor(&mut factors, "waiting_followups_overdue", "attention", overdue_waiting.len() as i64, 6, 18);
        total_penalty += push_factor(&mut factors, "stale_verified_ledgers", "attention", stale_verified_ledgers, 8, 16);
        total_penalty += push_factor(&mut factors, "stale_verified_facts", "attention", stale_verified_facts, 6, 12);
        total_penalty += push_factor(&mut factors, "documents_needing_attention", "attention", documents_needing_attention, 4, 12);
        total_penalty += push_factor(&mut factors, "required_evidence_requested", "attention", required_requested.len() as i64, 3, 9);
        total_penalty += push_factor(&mut factors, "recommended_evidence_open", "attention", recommended_open, 2, 8);
        total_penalty += push_factor(&mut factors, "draft_deadlines_waiting_review", "attention", draft_deadline_count, 2, 6);
        total_penalty += push_factor(&mut factors, "ledger_drafts_waiting_review", "attention", ledger_drafts, 1, 5);
        total_penalty += push_factor(&mut factors, "pending_ai_review", "attention", pending_ai_review, 1, 5);

        let score = (100 - total_penalty).max(0);
        let band = if score >= 85 { "good" } else if score >= 65 { "attention" } else { "risk" };

        let action = if let Some(item) = overdue_deadlines.first() {
            NextBestAction {
                code: "resolve_overdue_deadline".into(), priority: "critical".into(),
                target_id: Some(item.id.clone()), due_at: Some(item.due_at.clone()), label: Some(item.action.clone()),
                secondary_label: None, requirement_key: None, workstream_kind: None,
            }
        } else if let Some(item) = due_soon_deadlines.first() {
            NextBestAction {
                code: "prepare_upcoming_deadline".into(), priority: "high".into(),
                target_id: Some(item.id.clone()), due_at: Some(item.due_at.clone()), label: Some(item.action.clone()),
                secondary_label: None, requirement_key: None, workstream_kind: None,
            }
        } else if let Some(item) = overdue_tasks.first() {
            NextBestAction {
                code: "complete_overdue_task".into(), priority: "high".into(),
                target_id: Some(item.id.clone()), due_at: item.due_at.clone(), label: Some(item.title.clone()),
                secondary_label: None, requirement_key: None, workstream_kind: None,
            }
        } else if let Some(kind) = blocked_workstreams.first() {
            NextBestAction {
                code: "unblock_workstream".into(), priority: "high".into(),
                target_id: None, due_at: None, label: None, secondary_label: None,
                requirement_key: None, workstream_kind: Some(kind.clone()),
            }
        } else if let Some(key) = required_stale.first() {
            NextBestAction {
                code: "refresh_required_evidence".into(), priority: "high".into(),
                target_id: None, due_at: None, label: None, secondary_label: None,
                requirement_key: Some(key.clone()), workstream_kind: None,
            }
        } else if let Some(key) = required_missing.first() {
            NextBestAction {
                code: "collect_required_evidence".into(), priority: "high".into(),
                target_id: None, due_at: None, label: None, secondary_label: None,
                requirement_key: Some(key.clone()), workstream_kind: None,
            }
        } else if let Some(item) = overdue_waiting.first() {
            NextBestAction {
                code: "follow_up_waiting".into(), priority: "high".into(),
                target_id: Some(item.id.clone()), due_at: item.follow_up_at.clone(), label: Some(item.party_label.clone()),
                secondary_label: Some(item.item_label.clone()), requirement_key: None, workstream_kind: None,
            }
        } else if stale_verified_ledgers + stale_verified_facts > 0 {
            NextBestAction {
                code: "refresh_stale_evidence".into(), priority: "high".into(),
                target_id: None, due_at: None, label: None, secondary_label: None,
                requirement_key: None, workstream_kind: None,
            }
        } else if documents_needing_attention > 0 {
            NextBestAction {
                code: "repair_document_extraction".into(), priority: "normal".into(),
                target_id: None, due_at: None, label: None, secondary_label: None,
                requirement_key: None, workstream_kind: None,
            }
        } else if pending_ai_review > 0 {
            NextBestAction {
                code: "review_ai_proposals".into(), priority: "normal".into(),
                target_id: None, due_at: None, label: None, secondary_label: None,
                requirement_key: None, workstream_kind: None,
            }
        } else if let Some(item) = open_tasks.first() {
            NextBestAction {
                code: "complete_open_task".into(), priority: "normal".into(),
                target_id: Some(item.id.clone()), due_at: item.due_at.clone(), label: Some(item.title.clone()),
                secondary_label: None, requirement_key: None, workstream_kind: None,
            }
        } else if let Some(key) = required_requested.first() {
            NextBestAction {
                code: "follow_up_required_evidence".into(), priority: "normal".into(),
                target_id: None, due_at: None, label: None, secondary_label: None,
                requirement_key: Some(key.clone()), workstream_kind: None,
            }
        } else if let Some(item) = open_waiting.first() {
            NextBestAction {
                code: "review_waiting_item".into(), priority: "normal".into(),
                target_id: Some(item.id.clone()), due_at: item.follow_up_at.clone(), label: Some(item.party_label.clone()),
                secondary_label: Some(item.item_label.clone()), requirement_key: None, workstream_kind: None,
            }
        } else if let Some(kind) = not_started_workstreams.first() {
            NextBestAction {
                code: "start_workstream".into(), priority: "normal".into(),
                target_id: None, due_at: None, label: None, secondary_label: None,
                requirement_key: None, workstream_kind: Some(kind.clone()),
            }
        } else {
            NextBestAction {
                code: "set_next_task".into(), priority: "normal".into(),
                target_id: None, due_at: None, label: None, secondary_label: None,
                requirement_key: None, workstream_kind: None,
            }
        };

        Ok(CaseHealthSnapshot {
            matter_id: matter_id.to_string(),
            score,
            band: band.to_string(),
            as_of: now.to_rfc3339(),
            factors,
            next_best_action: action,
        })
    })
}

#[tauri::command]
pub fn get_case_health(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id = payload
        .get("matterId")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Validation("matterId required".into()))?;
    Ok(serde_json::to_value(compute(&state.db, matter_id)?)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use uuid::Uuid;

    fn seeded_matter(db: &DbState, case_type: &str) -> String {
        let matter_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO matters(id,title,matter_type,status,workflow_stage,created_at,updated_at)
                 VALUES(?1,'B6 test',?2,'active','intake',?3,?3)",
                params![matter_id, case_type, now],
            )?;
            workstreams::reconcile(conn, &matter_id, case_type)?;
            requirements::reconcile(conn, &matter_id, case_type)?;
            Ok(())
        })
        .unwrap();
        matter_id
    }

    #[test]
    fn overdue_committed_deadline_is_first_and_matter_isolated() {
        let root = std::env::temp_dir().join(format!("tahrir-b6-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let db = DbState::open(root.join("app.db")).unwrap();
        let matter_a = seeded_matter(&db, "generic_civil");
        let matter_b = seeded_matter(&db, "generic_civil");
        let overdue = (Utc::now().date_naive() - chrono::Duration::days(1)).to_string();
        let future = (Utc::now().date_naive() + chrono::Duration::days(30)).to_string();
        let now = Utc::now().to_rfc3339();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO legal_deadlines(id,matter_id,action,due_at,state,trigger_source_ref,created_at)
                 VALUES(?1,?2,'A deadline',?3,'committed','test',?4)",
                params![Uuid::new_v4().to_string(), matter_a, overdue, now],
            )?;
            conn.execute(
                "INSERT INTO legal_deadlines(id,matter_id,action,due_at,state,trigger_source_ref,created_at)
                 VALUES(?1,?2,'B deadline',?3,'committed','test',?4)",
                params![Uuid::new_v4().to_string(), matter_b, future, now],
            )?;
            Ok(())
        }).unwrap();

        let snapshot = compute(&db, &matter_a).unwrap();
        assert_eq!(snapshot.next_best_action.code, "resolve_overdue_deadline");
        assert_eq!(snapshot.next_best_action.label.as_deref(), Some("A deadline"));
        assert!(snapshot.factors.iter().any(|f| f.code == "overdue_committed_deadlines" && f.count == 1));
        assert!(snapshot.score <= 70);
        drop(db);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn office_required_missing_evidence_drives_next_action_without_claiming_law() {
        let root = std::env::temp_dir().join(format!("tahrir-b6-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let db = DbState::open(root.join("app.db")).unwrap();
        let matter_id = seeded_matter(&db, "generic_civil");
        let snapshot = compute(&db, &matter_id).unwrap();
        assert_eq!(snapshot.next_best_action.code, "collect_required_evidence");
        assert_eq!(snapshot.next_best_action.requirement_key.as_deref(), Some("id_document"));
        assert!(snapshot.factors.iter().any(|f| f.code == "required_evidence_missing"));
        drop(db);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn completed_office_requirement_removes_its_penalty() {
        let root = std::env::temp_dir().join(format!("tahrir-b6-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let db = DbState::open(root.join("app.db")).unwrap();
        let matter_id = seeded_matter(&db, "generic_civil");
        db.write(|conn| requirements::update_status(conn, &matter_id, "id_document", "collected", None)).unwrap();
        let snapshot = compute(&db, &matter_id).unwrap();
        assert!(!snapshot.factors.iter().any(|f| f.code == "required_evidence_missing"));
        assert_eq!(snapshot.score, 100);
        assert_eq!(snapshot.next_best_action.code, "start_workstream");
        drop(db);
        let _ = fs::remove_dir_all(root);
    }
}
