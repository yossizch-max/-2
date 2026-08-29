//! Phase C, milestone C5: Action Orchestrator / Matter Agent Core.
//!
//! This module is NOT an autonomous agent. It answers one question - "what
//! should the lawyer deal with next, why now, based on what, and what are the
//! alternatives?" - by computing a transparent, deterministic ranked list of
//! action candidates over TAHRIR's already-existing operational state. It
//! never autonomously executes a substantive action: converting a
//! recommendation into a task, marking a deadline satisfied, or changing a
//! recommendation's own display state always requires an explicit call from
//! a human-triggered command, never something this module does on its own
//! initiative while merely being read.
//!
//! It replaces two previously independent, disagreeing prioritization
//! systems: `case_health.rs`'s backend `NextBestAction` (a single top pick)
//! and the frontend's own `src/lib/actionCenter.ts` (an independent re-fetch
//! + ad-hoc sort). This module is now the single backend source of truth for
//! the *ranked list* of action candidates; `case_health.rs`'s score/factor
//! computation is untouched (it answers a different question - "how healthy
//! is this matter overall" - and its 40+ existing tests are not worth
//! destabilizing for a milestone that does not need to touch them).
//!
//! Case Signals -> Action Candidates -> Action Plan:
//! - Case Signals are read directly from existing tables at plan-computation
//!   time. Nothing here duplicates source data into an agent-memory table -
//!   existing TAHRIR state IS the memory, per the spec.
//! - Action Candidates are deterministic, computed fresh on every read. A
//!   candidate's `fingerprint` is a stable sha256 of
//!   `(matter_id, action_code, target_key)` - the same real-world condition
//!   always hashes to the same fingerprint, and a genuinely different
//!   condition (a different task, a different deadline, or the *same*
//!   deadline after it has been superseded into a new row with a new id)
//!   always hashes to a different one. Recommendation *state*
//!   (acknowledge/snooze/dismiss/convert-to-task) is the one piece of new
//!   persistent state this milestone adds (`action_recommendations`,
//!   migration 009) - it never duplicates or mutates the underlying
//!   candidate's own source data.
//! - Ranking is a fully explicit, testable lexicographic hierarchy
//!   (`rank_category` 1-10, see `RANK_*` consts below) - never a model
//!   probability or a blended score. A lower-level suggestion can never
//!   outrank a genuine committed legal obligation.
//!
//! Urgency is always computed from real operational due dates (deadline
//! due_at, task due_at, waiting_for follow_up_at) - never from a document's
//! or matter's ingestion date. A ten-year-old imported matter with an
//! evidence gap from a decade ago is not "overdue" merely because the
//! underlying evidence is old; only a real, current, unmet due date is.
//!
//! AI is entirely optional here and this module works completely with AI
//! disabled - no capability call is made anywhere in this file. A future
//! milestone may add an AI Advisor that proposes additional
//! `strategic_action_suggestion` items through the existing `ai.rs`
//! bundle-capability/no-domain-write pattern (pending lawyer review, never
//! outranking a deterministic critical obligation, never auto-creating a
//! task or deadline); this module's deterministic ranking does not depend on
//! it existing.
use crate::{
    db::DbState,
    error::{AppError, AppResult},
    requirements, workstreams, AppState,
};
use chrono::{Local, NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use tauri::State;
use uuid::Uuid;

fn required_str<'a>(payload: &'a Value, key: &str) -> AppResult<&'a str> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| AppError::Validation(format!("{key} is required")))
}

// Lexicographic rank hierarchy - lower number always outranks higher, no
// exceptions, no blending with anything else. A lower-ranked AI suggestion
// (there are none in this deterministic module, but the invariant is stated
// here because a future AI Advisor must never violate it) can never place at
// a lower `rank_category` number than a genuine committed legal deadline.
const RANK_OVERDUE_DEADLINE: i64 = 1;
const RANK_IMMINENT_DEADLINE: i64 = 2;
const RANK_OVERDUE_TASK: i64 = 3;
const RANK_BLOCKING_CONFLICT: i64 = 4;
const RANK_EVIDENCE_BLOCKING: i64 = 5;
const RANK_OVERDUE_FOLLOWUP: i64 = 6;
const RANK_INTEGRITY_ISSUE: i64 = 7;
const RANK_PENDING_REVIEW: i64 = 8;
const RANK_OPEN_TASK: i64 = 9;
const RANK_BACKLOG: i64 = 10;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ActionCandidate {
    pub action_code: String,
    pub matter_id: String,
    pub matter_title: String,
    pub title: String,
    pub reason: String,
    pub source_type: String,
    pub source_id: Option<String>,
    pub target_id: Option<String>,
    pub workstream_kind: Option<String>,
    pub requirement_key: Option<String>,
    pub due_at: Option<String>,
    pub urgency: String,
    pub blocking: bool,
    pub rank_category: i64,
    pub human_action_options: Vec<String>,
    pub fingerprint: String,
    pub recommendation_state: String,
    pub snoozed_until: Option<String>,
    #[serde(skip)]
    order_key: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UrgencyCount {
    pub urgency: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionPlan {
    pub matter_id: String,
    pub matter_title: String,
    pub as_of: String,
    pub primary_action: Option<ActionCandidate>,
    pub alternatives: Vec<ActionCandidate>,
    pub candidates: Vec<ActionCandidate>,
    pub blockers: Vec<String>,
    pub counts_by_urgency: Vec<UrgencyCount>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionCenterEntry {
    pub matter_id: String,
    pub matter_title: String,
    pub plan: ActionPlan,
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

fn fingerprint_of(matter_id: &str, action_code: &str, target_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(matter_id.as_bytes());
    hasher.update(b"|");
    hasher.update(action_code.as_bytes());
    hasher.update(b"|");
    hasher.update(target_key.as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(Debug, Clone, Default)]
struct RecState {
    state: String,
    snoozed_until: Option<String>,
}

fn load_recommendation_states(conn: &Connection, matter_id: &str) -> AppResult<HashMap<String, RecState>> {
    let mut stmt = conn.prepare(
        "SELECT fingerprint,state,snoozed_until FROM action_recommendations WHERE matter_id=?1",
    )?;
    let rows = stmt
        .query_map([matter_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                RecState { state: r.get(1)?, snoozed_until: r.get(2)? },
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows.into_iter().collect())
}

#[allow(clippy::too_many_arguments)]
fn candidate(
    matter_id: &str,
    matter_title: &str,
    action_code: &str,
    title: String,
    reason: String,
    source_type: &str,
    source_id: Option<String>,
    target_id: Option<String>,
    workstream_kind: Option<String>,
    requirement_key: Option<String>,
    due_at: Option<String>,
    urgency: &str,
    blocking: bool,
    rank_category: i64,
    human_action_options: &[&str],
    order_key: String,
    rec_states: &HashMap<String, RecState>,
) -> ActionCandidate {
    let target_key = target_id
        .clone()
        .or_else(|| requirement_key.clone())
        .or_else(|| workstream_kind.clone())
        .unwrap_or_else(|| "none".to_string());
    let fingerprint = fingerprint_of(matter_id, action_code, &target_key);
    let rec = rec_states.get(&fingerprint).cloned().unwrap_or_default();
    let recommendation_state = if rec.state.is_empty() { "active".to_string() } else { rec.state };
    ActionCandidate {
        action_code: action_code.to_string(),
        matter_id: matter_id.to_string(),
        matter_title: matter_title.to_string(),
        title,
        reason,
        source_type: source_type.to_string(),
        source_id,
        target_id,
        workstream_kind,
        requirement_key,
        due_at,
        urgency: urgency.to_string(),
        blocking,
        rank_category,
        human_action_options: human_action_options.iter().map(|s| s.to_string()).collect(),
        fingerprint,
        recommendation_state,
        snoozed_until: rec.snoozed_until,
        order_key,
    }
}

fn ledger_gap_count(conn: &Connection, table: &str, matter_id: &str, stale_only: bool) -> AppResult<i64> {
    let clause = if stale_only { "AND e.status='verified' AND e.stale=1" } else { "AND e.status='draft'" };
    let sql = format!("SELECT COUNT(*) FROM {table} e WHERE e.matter_id=?1 {clause}");
    Ok(conn.query_row(&sql, [matter_id], |r| r.get(0))?)
}

fn approved_signal_count(conn: &Connection, matter_id: &str, kind: &str) -> AppResult<i64> {
    Ok(conn.query_row(
        "SELECT COUNT(*) FROM ai_proposals WHERE matter_id=?1 AND proposal_kind=?2 AND status='approved'",
        params![matter_id, kind],
        |r| r.get(0),
    )?)
}

/// The deterministic core: gathers every Case Signal for one matter as of
/// `today`/`as_of`, turns each into an `ActionCandidate`, and returns them in
/// full rank order (category ascending, then a real business/audit timestamp
/// - never a UUID's own lexical order - then id as a final stable
/// tie-breaker). Public so both `compute_plan` (single matter) and
/// `compute_action_center` (all active matters) share one candidate list.
pub fn compute_candidates_for_date(
    conn: &Connection,
    matter_id: &str,
    matter_title: &str,
    case_type: &str,
    today: NaiveDate,
) -> AppResult<Vec<ActionCandidate>> {
    let rec_states = load_recommendation_states(conn, matter_id)?;
    let mut out: Vec<ActionCandidate> = Vec::new();

    // 1/2: committed legal deadlines. A deadline whose state is no longer
    // 'committed' (superseded, or satisfied via satisfy_deadline) is
    // excluded here automatically - satisfaction never deletes or mutates
    // the row, it just moves it out of this active-candidate query.
    {
        let mut stmt = conn.prepare(
            "SELECT id,action,due_at FROM legal_deadlines
             WHERE matter_id=?1 AND state='committed' ORDER BY due_at,id",
        )?;
        let rows = stmt
            .query_map([matter_id], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (id, action, due_at) in rows {
            let Some(days) = days_until(&due_at, today) else { continue };
            if days < 0 {
                out.push(candidate(
                    matter_id, matter_title, "resolve_overdue_deadline", action.clone(),
                    format!("מועד מחייב חלף ({due_at}) וטרם סומן כטופל"), "legal_deadline",
                    Some(id.clone()), Some(id.clone()), None, None, Some(due_at.clone()),
                    "overdue", true, RANK_OVERDUE_DEADLINE,
                    &["mark_satisfied", "supersede", "snooze_display"],
                    format!("{due_at}|{id}"), &rec_states,
                ));
            } else if days <= 14 {
                out.push(candidate(
                    matter_id, matter_title, "prepare_upcoming_deadline", action.clone(),
                    format!("מועד מחייב מתקרב ({due_at})"), "legal_deadline",
                    Some(id.clone()), Some(id.clone()), None, None, Some(due_at.clone()),
                    "due_soon", true, RANK_IMMINENT_DEADLINE,
                    &["mark_satisfied", "supersede", "snooze_display"],
                    format!("{due_at}|{id}"), &rec_states,
                ));
            }
        }
    }

    // 3/9: tasks.
    {
        let mut stmt = conn.prepare(
            "SELECT id,title,due_at FROM tasks WHERE matter_id=?1 AND status='open'
             ORDER BY CASE WHEN due_at IS NULL THEN 1 ELSE 0 END,due_at,id",
        )?;
        let rows = stmt
            .query_map([matter_id], |r| {
                let due_at: Option<String> = r.get(2)?;
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, due_at))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (id, title, due_at) in rows {
            let days = due_at.as_deref().and_then(|v| days_until(v, today));
            if days.is_some_and(|d| d < 0) {
                let due = due_at.clone().unwrap();
                out.push(candidate(
                    matter_id, matter_title, "complete_overdue_task", title.clone(),
                    format!("משימה באיחור ({due})"), "task", Some(id.clone()), Some(id.clone()),
                    None, None, Some(due.clone()), "overdue", false, RANK_OVERDUE_TASK,
                    &["complete", "snooze", "create_task"], format!("{due}|{id}"), &rec_states,
                ));
            } else {
                let order = due_at.clone().unwrap_or_else(|| "9999-99-99".to_string());
                out.push(candidate(
                    matter_id, matter_title, "complete_open_task", title.clone(),
                    "משימה פתוחה".to_string(), "task", Some(id.clone()), Some(id.clone()),
                    None, None, due_at, "normal", false, RANK_OPEN_TASK,
                    &["complete", "snooze", "dismiss"], format!("{order}|{id}"), &rec_states,
                ));
            }
        }
    }

    // 4: blocking conflicts - blocked workstreams, unresolved verified-fact
    // conflicts, and approved C3/C4 contradiction signals.
    {
        let rows = workstreams::list(conn, matter_id)?;
        for w in rows.iter().filter(|w| w.status == "blocked") {
            out.push(candidate(
                matter_id, matter_title, "unblock_workstream", format!("מסלול חסום: {}", w.kind),
                "מסלול עבודה מסומן כחסום".to_string(), "workstream", None, None,
                Some(w.kind.clone()), None, None, "blocking", true, RANK_BLOCKING_CONFLICT,
                &["update_status"], format!("blocked|{}", w.kind), &rec_states,
            ));
        }
    }
    {
        let mut stmt = conn.prepare(
            "SELECT c.id,c.created_at FROM fact_conflicts c
             JOIN verified_facts a ON a.id=c.fact_a_id AND a.matter_id=c.matter_id AND a.status='valid'
             JOIN verified_facts b ON b.id=c.fact_b_id AND b.matter_id=c.matter_id AND b.status='valid'
             WHERE c.matter_id=?1 AND c.status='unresolved' ORDER BY c.created_at,c.id",
        )?;
        let rows = stmt
            .query_map([matter_id], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        for (id, created_at) in rows {
            out.push(candidate(
                matter_id, matter_title, "review_fact_conflict", "עובדות סותרות".to_string(),
                "שתי עובדות מאומתות סותרות זו את זו וטרם נפתרו".to_string(), "fact_conflict",
                Some(id.clone()), Some(id.clone()), None, None, None, "blocking", true,
                RANK_BLOCKING_CONFLICT, &["resolve"], format!("{created_at}|{id}"), &rec_states,
            ));
        }
    }
    for (kind, label) in [
        ("medical_contradiction", "סתירה בראיות רפואיות"),
        ("liability_contradiction", "סתירה בראיות אחריות"),
    ] {
        let count = approved_signal_count(conn, matter_id, kind)?;
        if count > 0 {
            out.push(candidate(
                matter_id, matter_title, "review_approved_contradiction", label.to_string(),
                format!("{count} סתירות מאושרות עדיין ממתינות להכרעת עו\"ד"), "ai_signal", None,
                None, None, None, None, "blocking", true, RANK_BLOCKING_CONFLICT,
                &["review"], format!("z|{kind}"), &rec_states,
            ));
        }
    }

    // 5: required/stale evidence blocking progress, including approved C3/C4
    // gap/missing-evidence signals (these are consumed as already-structured
    // states, never independently reinterpreted here).
    {
        let requirement_rows = requirements::list(conn, matter_id, case_type)?;
        for req in &requirement_rows {
            if req.relevance != "applicable" || req.priority.as_deref() != Some("required_by_office_policy") {
                continue;
            }
            let (code, label) = match req.status.as_str() {
                "stale" => ("refresh_required_evidence", "יש לרענן ראיה נדרשת"),
                "not_collected" => ("collect_required_evidence", "חסרה ראיה נדרשת"),
                _ => continue,
            };
            out.push(candidate(
                matter_id, matter_title, code, format!("ראיה נדרשת: {}", req.requirement_key),
                label.to_string(), "requirement", None, None, None,
                Some(req.requirement_key.clone()), None, "blocking", true, RANK_EVIDENCE_BLOCKING,
                &["update_status"], format!("{code}|{}", req.requirement_key), &rec_states,
            ));
        }
    }
    for (kind, label) in [
        ("medical_gap_signal", "פער תיעוד רפואי מאושר"),
        ("medical_missing_evidence_signal", "ראיה רפואית חסרה (מאושר)"),
        ("wage_gap_signal", "פער תיעוד שכר/הכנסה מאושר"),
    ] {
        let count = approved_signal_count(conn, matter_id, kind)?;
        if count > 0 {
            out.push(candidate(
                matter_id, matter_title, "review_approved_evidence_gap", label.to_string(),
                format!("{count} פערי ראיות מאושרים לא תועדו במקורות שנקלטו עד כה"), "ai_signal",
                None, None, None, None, None, "blocking", true, RANK_EVIDENCE_BLOCKING,
                &["review"], format!("z|{kind}"), &rec_states,
            ));
        }
    }

    // 6: overdue negotiation/waiting follow-ups.
    {
        let mut stmt = conn.prepare(
            "SELECT w.id,w.party_label,w.item_label,w.follow_up_at,
                    CASE WHEN l.waiting_for_id IS NULL THEN 0 ELSE 1 END AS is_negotiation
             FROM waiting_for w
             LEFT JOIN negotiation_waiting_links l ON l.waiting_for_id=w.id AND l.matter_id=w.matter_id
             WHERE w.matter_id=?1 AND w.status='open'
             ORDER BY CASE WHEN w.follow_up_at IS NULL THEN 1 ELSE 0 END,w.follow_up_at,w.id",
        )?;
        let rows = stmt
            .query_map([matter_id], |r| {
                let follow_up_at: Option<String> = r.get(3)?;
                Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?, follow_up_at, r.get::<_, i64>(4)? == 1))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (id, party, item, follow_up_at, is_negotiation) in rows {
            let days = follow_up_at.as_deref().and_then(|v| days_until(v, today));
            let title = format!("{party} · {item}");
            if days.is_some_and(|d| d < 0) {
                let due = follow_up_at.clone().unwrap();
                let code = if is_negotiation { "follow_up_negotiation" } else { "follow_up_waiting" };
                out.push(candidate(
                    matter_id, matter_title, code, title, format!("מעקב באיחור ({due})"), "waiting_for",
                    Some(id.clone()), Some(id.clone()), None, None, Some(due.clone()), "overdue", false,
                    RANK_OVERDUE_FOLLOWUP, &["close", "snooze", "create_task"], format!("{due}|{id}"), &rec_states,
                ));
            } else {
                let order = follow_up_at.clone().unwrap_or_else(|| "9999-99-99".to_string());
                out.push(candidate(
                    matter_id, matter_title, "review_waiting_item", title, "ממתין לצד שלישי".to_string(),
                    "waiting_for", Some(id.clone()), Some(id.clone()), None, None, follow_up_at, "normal",
                    false, RANK_BACKLOG, &["close", "snooze", "dismiss"], format!("{order}|{id}"), &rec_states,
                ));
            }
        }
    }

    // 7: extraction/integrity issues + stale verified evidence.
    let stale_facts: i64 = conn.query_row(
        "SELECT COUNT(*) FROM verified_facts WHERE matter_id=?1 AND status='valid' AND stale=1 AND superseded_by IS NULL",
        [matter_id], |r| r.get(0),
    )?;
    let stale_ledgers = ledger_gap_count(conn, "medical_events", matter_id, true)?
        + ledger_gap_count(conn, "wage_records", matter_id, true)?
        + ledger_gap_count(conn, "liability_facts", matter_id, true)?;
    if stale_facts + stale_ledgers > 0 {
        out.push(candidate(
            matter_id, matter_title, "refresh_stale_evidence", "ראיות מאומתות שהתיישנו".to_string(),
            format!("{} רשומות מאומתות סומנו כמיושנות עקב גרסת מקור חדשה", stale_facts + stale_ledgers),
            "integrity", None, None, None, None, None, "attention", false, RANK_INTEGRITY_ISSUE,
            &["review"], "z|stale_evidence".to_string(), &rec_states,
        ));
    }
    let extraction_issues: i64 = conn.query_row(
        "SELECT COUNT(*) FROM documents d WHERE d.matter_id=?1
           AND (SELECT v.extraction_state FROM document_versions v
                WHERE v.document_id=d.id AND v.matter_id=d.matter_id
                ORDER BY v.created_at DESC,v.id DESC LIMIT 1) IN ('stale','blocked','failed')",
        [matter_id], |r| r.get(0),
    )?;
    if extraction_issues > 0 {
        out.push(candidate(
            matter_id, matter_title, "repair_document_extraction", "כשל בחילוץ מסמכים".to_string(),
            format!("{extraction_issues} מסמכים דורשים טיפול בחילוץ הטקסט"), "integrity", None, None,
            None, None, None, "attention", false, RANK_INTEGRITY_ISSUE, &["review"],
            "z|extraction_issues".to_string(), &rec_states,
        ));
    }

    // 8: pending lawyer review - pending AI proposals, draft deadlines, draft
    // ledger entries, and approved liability issues awaiting a decision.
    let pending_ai: i64 = conn.query_row(
        "SELECT COUNT(*) FROM ai_proposals WHERE matter_id=?1 AND status='pending'", [matter_id], |r| r.get(0),
    )?;
    if pending_ai > 0 {
        out.push(candidate(
            matter_id, matter_title, "review_ai_proposals", "הצעות AI ממתינות לאישור".to_string(),
            format!("{pending_ai} הצעות טרם נבדקו"), "ai_review", None, None, None, None, None,
            "normal", false, RANK_PENDING_REVIEW, &["review"], "z|pending_ai".to_string(), &rec_states,
        ));
    }
    let draft_deadlines: i64 = conn.query_row(
        "SELECT COUNT(*) FROM legal_deadlines WHERE matter_id=?1 AND state='draft'", [matter_id], |r| r.get(0),
    )?;
    if draft_deadlines > 0 {
        out.push(candidate(
            matter_id, matter_title, "review_draft_deadlines", "מועדים בטיוטה".to_string(),
            format!("{draft_deadlines} מועדים טרם אושרו כמחייבים"), "legal_deadline", None, None,
            None, None, None, "normal", false, RANK_PENDING_REVIEW, &["review"],
            "z|draft_deadlines".to_string(), &rec_states,
        ));
    }
    let ledger_drafts = ledger_gap_count(conn, "medical_events", matter_id, false)?
        + ledger_gap_count(conn, "wage_records", matter_id, false)?
        + ledger_gap_count(conn, "liability_facts", matter_id, false)?;
    if ledger_drafts > 0 {
        out.push(candidate(
            matter_id, matter_title, "review_ledger_drafts", "רשומות בפנקסים בטיוטה".to_string(),
            format!("{ledger_drafts} רשומות טרם אומתו"), "ledger", None, None, None, None, None,
            "normal", false, RANK_PENDING_REVIEW, &["review"], "z|ledger_drafts".to_string(), &rec_states,
        ));
    }
    let liability_issues = approved_signal_count(conn, matter_id, "liability_issue")?;
    if liability_issues > 0 {
        out.push(candidate(
            matter_id, matter_title, "review_liability_issue", "סוגיית אחריות לבירור".to_string(),
            format!("{liability_issues} סוגיות אחריות מאושרות ממתינות להחלטת עו\"ד"), "ai_signal",
            None, None, None, None, None, "normal", false, RANK_PENDING_REVIEW, &["review"],
            "z|liability_issue".to_string(), &rec_states,
        ));
    }

    // 10: backlog - recommended (non-mandatory) requirements and not-started
    // workstreams. Requested-but-not-yet-collected required evidence is also
    // backlog (it is already in motion, not blocking).
    {
        let requirement_rows = requirements::list(conn, matter_id, case_type)?;
        for req in &requirement_rows {
            if req.relevance != "applicable" {
                continue;
            }
            match (req.priority.as_deref(), req.status.as_str()) {
                (Some("required_by_office_policy"), "requested") => {
                    out.push(candidate(
                        matter_id, matter_title, "follow_up_required_evidence",
                        format!("מעקב אחר ראיה שהוזמנה: {}", req.requirement_key),
                        "ראיה נדרשת הוזמנה, טרם התקבלה".to_string(), "requirement", None, None,
                        None, Some(req.requirement_key.clone()), None, "normal", false, RANK_BACKLOG,
                        &["update_status"], format!("z|{}", req.requirement_key), &rec_states,
                    ));
                }
                (Some("recommended"), "not_collected" | "stale") => {
                    out.push(candidate(
                        matter_id, matter_title, "collect_recommended_evidence",
                        format!("ראיה מומלצת: {}", req.requirement_key), "לא חובה, אך מומלצת".to_string(),
                        "requirement", None, None, None, Some(req.requirement_key.clone()), None,
                        "backlog", false, RANK_BACKLOG, &["update_status"],
                        format!("zz|{}", req.requirement_key), &rec_states,
                    ));
                }
                _ => {}
            }
        }
        let workstream_rows = workstreams::list(conn, matter_id)?;
        for w in workstream_rows.iter().filter(|w| w.status == "not_started") {
            out.push(candidate(
                matter_id, matter_title, "start_workstream", format!("להתחיל מסלול: {}", w.kind),
                "מסלול עבודה רלוונטי טרם החל".to_string(), "workstream", None, None,
                Some(w.kind.clone()), None, None, "backlog", false, RANK_BACKLOG,
                &["update_status"], format!("zz|ws|{}", w.kind), &rec_states,
            ));
        }
    }

    out.sort_by(|a, b| (a.rank_category, &a.order_key).cmp(&(b.rank_category, &b.order_key)));
    Ok(out)
}

fn compute_plan_for_date(
    db: &DbState,
    matter_id: &str,
    today: NaiveDate,
    as_of: &str,
) -> AppResult<ActionPlan> {
    db.read(|conn| {
        let (matter_title, case_type): (String, String) = conn
            .query_row("SELECT title,matter_type FROM matters WHERE id=?1", [matter_id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .map_err(|_| AppError::NotFound("matter".into()))?;
        let candidates = compute_candidates_for_date(conn, matter_id, &matter_title, &case_type, today)?;

        let selectable: Vec<&ActionCandidate> = candidates
            .iter()
            .filter(|c| {
                c.recommendation_state != "dismissed"
                    && !(c.recommendation_state == "snoozed"
                        && c.snoozed_until.as_deref().is_some_and(|s| s > as_of))
            })
            .collect();
        let primary_action = selectable.first().cloned().cloned();
        let alternatives: Vec<ActionCandidate> =
            selectable.iter().skip(1).take(4).map(|c| (*c).clone()).collect();

        let blocked_workstreams = workstreams::list(conn, matter_id)?
            .into_iter()
            .filter(|w| w.status == "blocked")
            .map(|w| w.kind)
            .collect::<Vec<_>>();

        let mut counts: HashMap<String, i64> = HashMap::new();
        for c in &candidates {
            *counts.entry(c.urgency.clone()).or_insert(0) += 1;
        }
        let mut counts_by_urgency: Vec<UrgencyCount> =
            counts.into_iter().map(|(urgency, count)| UrgencyCount { urgency, count }).collect();
        counts_by_urgency.sort_by(|a, b| a.urgency.cmp(&b.urgency));

        Ok(ActionPlan {
            matter_id: matter_id.to_string(),
            matter_title,
            as_of: as_of.to_string(),
            primary_action,
            alternatives,
            candidates,
            blockers: blocked_workstreams,
            counts_by_urgency,
        })
    })
}

pub fn compute_plan(db: &DbState, matter_id: &str) -> AppResult<ActionPlan> {
    let as_of = Utc::now().to_rfc3339();
    // Local calendar date, matching case_health.rs's own choice, so both
    // surfaces classify the same day boundary identically.
    compute_plan_for_date(db, matter_id, Local::now().date_naive(), &as_of)
}

fn compute_action_center_for_date(
    db: &DbState,
    today: NaiveDate,
    as_of: &str,
) -> AppResult<Vec<ActionCenterEntry>> {
    let matter_ids: Vec<String> = db.read(|conn| {
        let mut stmt = conn.prepare("SELECT id FROM matters WHERE status='active' ORDER BY id")?;
        let rows = stmt.query_map([], |r| r.get(0))?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })?;
    let mut entries = Vec::new();
    for matter_id in matter_ids {
        let plan = compute_plan_for_date(db, &matter_id, today, as_of)?;
        entries.push(ActionCenterEntry { matter_id: matter_id.clone(), matter_title: plan.matter_title.clone(), plan });
    }
    // Global ordering: rank_category of each matter's own primary action,
    // then its due date (real business timestamp, never a UUID), then
    // matter_id as a final stable tie-break - never insertion order.
    entries.sort_by(|a, b| {
        let ra = a.plan.primary_action.as_ref().map(|p| p.rank_category).unwrap_or(i64::MAX);
        let rb = b.plan.primary_action.as_ref().map(|p| p.rank_category).unwrap_or(i64::MAX);
        let da = a.plan.primary_action.as_ref().and_then(|p| p.due_at.clone()).unwrap_or_else(|| "9999-99-99".into());
        let db_ = b.plan.primary_action.as_ref().and_then(|p| p.due_at.clone()).unwrap_or_else(|| "9999-99-99".into());
        (ra, da, &a.matter_id).cmp(&(rb, db_, &b.matter_id))
    });
    Ok(entries)
}

pub fn compute_action_center(db: &DbState) -> AppResult<Vec<ActionCenterEntry>> {
    let as_of = Utc::now().to_rfc3339();
    compute_action_center_for_date(db, Local::now().date_naive(), &as_of)
}

const CRITICAL_DEADLINE_CODES: &[&str] = &["resolve_overdue_deadline", "prepare_upcoming_deadline"];
const REC_STATES: &[&str] = &["active", "acknowledged", "snoozed", "dismissed", "converted_to_task"];

/// Explicit human action only - never called from anywhere except the
/// `set_action_recommendation_state` command. Rejects an unknown fingerprint
/// (it must belong to a candidate this engine can currently compute for the
/// matter) and rejects a plain "dismissed" transition for a committed legal
/// deadline's own candidate - per the spec, a committed legal obligation can
/// never be permanently hidden by a generic dismiss; only marking it
/// satisfied, superseding it, or a display-only snooze are allowed.
pub fn set_recommendation_state(
    db: &DbState,
    matter_id: &str,
    fingerprint: &str,
    state: &str,
    snoozed_until: Option<&str>,
    note: Option<&str>,
) -> AppResult<()> {
    if !REC_STATES.contains(&state) {
        return Err(AppError::Validation(format!("unknown recommendation state \"{state}\"")));
    }
    if state == "snoozed" && snoozed_until.is_none() {
        return Err(AppError::Validation("snoozedUntil is required when snoozing".into()));
    }
    let today = Local::now().date_naive();
    let plan_candidates = db.read(|conn| {
        let (matter_title, case_type): (String, String) = conn
            .query_row("SELECT title,matter_type FROM matters WHERE id=?1", [matter_id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .map_err(|_| AppError::NotFound("matter".into()))?;
        compute_candidates_for_date(conn, matter_id, &matter_title, &case_type, today)
    })?;
    let found = plan_candidates
        .iter()
        .find(|c| c.fingerprint == fingerprint)
        .ok_or_else(|| AppError::Validation("fingerprint does not match a current action candidate for this matter".into()))?;
    if state == "dismissed" && CRITICAL_DEADLINE_CODES.contains(&found.action_code.as_str()) {
        return Err(AppError::Validation(
            "a committed legal deadline cannot be dismissed - mark it satisfied or supersede it instead".into(),
        ));
    }

    let now = Utc::now().to_rfc3339();
    let id = Uuid::new_v4().to_string();
    db.write(|conn| {
        conn.execute(
            "INSERT INTO action_recommendations(
                id,matter_id,fingerprint,action_code,state,snoozed_until,dismissed_at,acknowledged_at,
                first_seen_at,last_seen_at,created_at,updated_at)
             VALUES(?1,?2,?3,?4,?5,?6,
                CASE WHEN ?5='dismissed' THEN ?7 ELSE NULL END,
                CASE WHEN ?5='acknowledged' THEN ?7 ELSE NULL END,
                ?7,?7,?7,?7)
             ON CONFLICT(matter_id,fingerprint) DO UPDATE SET
                state=excluded.state, snoozed_until=excluded.snoozed_until,
                dismissed_at=CASE WHEN excluded.state='dismissed' THEN ?7 ELSE action_recommendations.dismissed_at END,
                acknowledged_at=CASE WHEN excluded.state='acknowledged' THEN ?7 ELSE action_recommendations.acknowledged_at END,
                last_seen_at=?7, updated_at=?7",
            params![id, matter_id, fingerprint, found.action_code, state, snoozed_until, now],
        )?;
        let _ = note; // reserved for a future audit column; not persisted separately from state today
        Ok(())
    })
}

/// Explicit human action only. Creates exactly one real `tasks` row from a
/// recommendation and records the backlink via `converted_task_id` so a
/// second click against the same fingerprint cannot create a duplicate task.
pub fn convert_to_task(db: &DbState, matter_id: &str, fingerprint: &str, title: Option<&str>) -> AppResult<String> {
    let today = Local::now().date_naive();
    let (matter_title_for_lookup, candidate_title, existing_task) = db.read(|conn| {
        let (matter_title, case_type): (String, String) = conn
            .query_row("SELECT title,matter_type FROM matters WHERE id=?1", [matter_id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .map_err(|_| AppError::NotFound("matter".into()))?;
        let candidates = compute_candidates_for_date(conn, matter_id, &matter_title, &case_type, today)?;
        let found = candidates
            .iter()
            .find(|c| c.fingerprint == fingerprint)
            .ok_or_else(|| AppError::Validation("fingerprint does not match a current action candidate for this matter".into()))?;
        let existing: Option<String> = conn
            .query_row(
                "SELECT converted_task_id FROM action_recommendations WHERE matter_id=?1 AND fingerprint=?2",
                params![matter_id, fingerprint], |r| r.get(0),
            )
            .optional()?
            .flatten();
        Ok((matter_title, found.title.clone(), existing))
    })?;
    let _ = matter_title_for_lookup;
    if let Some(task_id) = existing_task {
        return Ok(task_id);
    }

    let task_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    let task_title = title.map(str::to_string).unwrap_or(candidate_title);
    db.write(|conn| {
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO tasks(id,matter_id,title,status,risk_class,source_ref,created_at,updated_at)
             VALUES(?1,?2,?3,'open','normal','action_recommendation',?4,?4)",
            params![task_id, matter_id, task_title, now],
        )?;
        tx.execute(
            "INSERT INTO action_recommendations(
                id,matter_id,fingerprint,action_code,state,converted_task_id,converted_at,
                first_seen_at,last_seen_at,created_at,updated_at)
             VALUES(?1,?2,?3,'convert_action_to_task','converted_to_task',?4,?5,?5,?5,?5,?5)
             ON CONFLICT(matter_id,fingerprint) DO UPDATE SET
                state='converted_to_task', converted_task_id=excluded.converted_task_id,
                converted_at=?5, last_seen_at=?5, updated_at=?5",
            params![Uuid::new_v4().to_string(), matter_id, fingerprint, task_id, now],
        )?;
        tx.commit()?;
        Ok(())
    })?;
    Ok(task_id)
}

/// Explicit human action only. The only way `legal_deadlines.state` may ever
/// become 'satisfied' - reachable only from 'committed', never from 'draft'
/// (a draft was never a binding obligation to begin with) or 'superseded'
/// (its replacement row is the live one now). `due_at`/`committed_at`/
/// `trigger_source_ref`/etc. are left completely untouched.
pub fn satisfy_deadline(db: &DbState, deadline_id: &str, note: Option<&str>) -> AppResult<()> {
    let now = Utc::now().to_rfc3339();
    db.write(|conn| {
        let tx = conn.transaction()?;
        let changed = tx.execute(
            "UPDATE legal_deadlines SET state='satisfied' WHERE id=?1 AND state='committed'",
            params![deadline_id],
        )?;
        if changed != 1 {
            return Err(AppError::Validation("deadline not satisfiable (must be committed)".into()));
        }
        tx.execute(
            "INSERT INTO legal_deadline_satisfaction(deadline_id,matter_id,satisfied_at,satisfaction_note)
             SELECT id,matter_id,?2,?3 FROM legal_deadlines WHERE id=?1",
            params![deadline_id, now, note],
        )?;
        tx.commit()?;
        Ok(())
    })
}

#[tauri::command]
pub fn get_matter_action_plan(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id = required_str(&payload, "matterId")?;
    Ok(serde_json::to_value(compute_plan(&state.db, matter_id)?)?)
}

#[tauri::command]
pub fn get_action_center(state: State<'_, AppState>) -> AppResult<Value> {
    Ok(serde_json::to_value(compute_action_center(&state.db)?)?)
}

#[tauri::command]
pub fn set_action_recommendation_state(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id = required_str(&payload, "matterId")?;
    let fingerprint = required_str(&payload, "fingerprint")?;
    let rec_state = required_str(&payload, "state")?;
    let snoozed_until = payload.get("snoozedUntil").and_then(Value::as_str);
    let note = payload.get("note").and_then(Value::as_str);
    set_recommendation_state(&state.db, matter_id, fingerprint, rec_state, snoozed_until, note)?;
    Ok(json!({ "ok": true }))
}

#[tauri::command]
pub fn convert_action_to_task(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id = required_str(&payload, "matterId")?;
    let fingerprint = required_str(&payload, "fingerprint")?;
    let title = payload.get("title").and_then(Value::as_str);
    let task_id = convert_to_task(&state.db, matter_id, fingerprint, title)?;
    Ok(json!({ "taskId": task_id }))
}

#[tauri::command]
pub fn mark_deadline_satisfied(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let deadline_id = required_str(&payload, "deadlineId")?;
    let note = payload.get("note").and_then(Value::as_str);
    satisfy_deadline(&state.db, deadline_id, note)?;
    Ok(json!({ "ok": true }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (DbState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db = DbState::open(dir.path().join("t.db")).unwrap();
        (db, dir)
    }

    fn new_matter(db: &DbState, case_type: &str) -> String {
        let id = Uuid::new_v4().to_string();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO matters(id,title,matter_type,status,workflow_stage,created_at,updated_at)
                 VALUES(?1,?2,?3,'active','intake','x','x')",
                params![id, format!("matter {id}"), case_type],
            )?;
            Ok(())
        })
        .unwrap();
        id
    }

    fn insert_committed_deadline(db: &DbState, matter_id: &str, due_at: &str) -> String {
        let id = Uuid::new_v4().to_string();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO legal_deadlines(id,matter_id,action,due_at,state,trigger_source_ref,committed_at,created_at)
                 VALUES(?1,?2,'file response',?3,'committed','manual','2026-01-01','2026-01-01')",
                params![id, matter_id, due_at],
            )?;
            Ok(())
        })
        .unwrap();
        id
    }

    fn insert_task(db: &DbState, matter_id: &str, title: &str, due_at: Option<&str>) -> String {
        let id = Uuid::new_v4().to_string();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO tasks(id,matter_id,title,status,risk_class,created_at,updated_at)
                 VALUES(?1,?2,?3,'open','normal','x','x')",
                params![id, matter_id, title],
            )?;
            if let Some(d) = due_at {
                conn.execute("UPDATE tasks SET due_at=?2 WHERE id=?1", params![id, d])?;
            }
            Ok(())
        })
        .unwrap();
        id
    }

    // 1-9: deadline satisfaction lifecycle.
    #[test]
    fn overdue_committed_deadline_is_top_ranked() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        insert_committed_deadline(&db, &m, "2020-01-01");
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        assert_eq!(plan.primary_action.unwrap().action_code, "resolve_overdue_deadline");
    }

    #[test]
    fn mark_deadline_satisfied_requires_committed_state() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        let id = Uuid::new_v4().to_string();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO legal_deadlines(id,matter_id,action,due_at,state,trigger_source_ref,created_at)
                 VALUES(?1,?2,'x','2020-01-01','draft','manual','x')",
                params![id, m],
            )?;
            Ok(())
        }).unwrap();
        assert!(satisfy_deadline(&db, &id, None).is_err());
    }

    #[test]
    fn satisfying_a_deadline_preserves_due_at_and_committed_at() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        let id = insert_committed_deadline(&db, &m, "2020-01-01");
        satisfy_deadline(&db, &id, Some("handled by phone")).unwrap();
        let (due_at, committed_at, state): (String, String, String) = db.read(|conn| {
            Ok(conn.query_row(
                "SELECT due_at,committed_at,state FROM legal_deadlines WHERE id=?1",
                [&id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )?)
        }).unwrap();
        let note: Option<String> = db.read(|conn| {
            Ok(conn.query_row(
                "SELECT satisfaction_note FROM legal_deadline_satisfaction WHERE deadline_id=?1",
                [&id], |r| r.get(0),
            )?)
        }).unwrap();
        assert_eq!(due_at, "2020-01-01");
        assert_eq!(committed_at, "2026-01-01");
        assert_eq!(state, "satisfied");
        assert_eq!(note.as_deref(), Some("handled by phone"));
    }

    #[test]
    fn satisfied_deadline_stops_generating_overdue_candidates() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        let id = insert_committed_deadline(&db, &m, "2020-01-01");
        satisfy_deadline(&db, &id, None).unwrap();
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        assert!(plan.candidates.iter().all(|c| c.action_code != "resolve_overdue_deadline"));
    }

    #[test]
    fn satisfying_an_already_satisfied_deadline_fails() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        let id = insert_committed_deadline(&db, &m, "2020-01-01");
        satisfy_deadline(&db, &id, None).unwrap();
        assert!(satisfy_deadline(&db, &id, None).is_err());
    }

    #[test]
    fn satisfying_a_superseded_deadline_fails() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        let old_id = insert_committed_deadline(&db, &m, "2020-01-01");
        let new_id = Uuid::new_v4().to_string();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO legal_deadlines(id,matter_id,action,due_at,state,trigger_source_ref,created_at)
                 VALUES(?1,?2,'x','2027-01-01','draft','manual','x')",
                params![new_id, m],
            )?;
            conn.execute(
                "UPDATE legal_deadlines SET state='superseded',superseded_by=?2 WHERE id=?1",
                params![old_id, new_id],
            )?;
            Ok(())
        }).unwrap();
        assert!(satisfy_deadline(&db, &old_id, None).is_err());
    }

    #[test]
    fn draft_deadline_cannot_be_marked_satisfied_directly() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        let id = Uuid::new_v4().to_string();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO legal_deadlines(id,matter_id,action,due_at,state,trigger_source_ref,created_at)
                 VALUES(?1,?2,'x','2020-01-01','draft','manual','x')",
                params![id, m],
            )?;
            Ok(())
        }).unwrap();
        assert!(satisfy_deadline(&db, &id, None).is_err());
    }

    #[test]
    fn satisfied_deadline_still_appears_in_list_deadlines() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        let id = insert_committed_deadline(&db, &m, "2020-01-01");
        satisfy_deadline(&db, &id, None).unwrap();
        let count: i64 = db.read(|conn| {
            Ok(conn.query_row("SELECT COUNT(*) FROM legal_deadlines WHERE matter_id=?1", [&m], |r| r.get(0))?)
        }).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn imminent_deadline_within_14_days_ranks_below_overdue_but_above_tasks() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        insert_committed_deadline(&db, &m, "2026-01-10");
        insert_task(&db, &m, "overdue task", Some("2020-01-01"));
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        assert_eq!(plan.primary_action.unwrap().action_code, "prepare_upcoming_deadline");
    }

    #[test]
    fn deadline_more_than_14_days_out_does_not_generate_a_candidate() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        insert_committed_deadline(&db, &m, "2026-06-01");
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        assert!(plan.candidates.iter().all(|c| c.source_type != "legal_deadline" || c.action_code == "review_draft_deadlines"));
    }

    // 10-19: general ranking hierarchy.
    #[test]
    fn overdue_task_outranks_open_task() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        insert_task(&db, &m, "open task", None);
        insert_task(&db, &m, "overdue task", Some("2020-01-01"));
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        assert_eq!(plan.primary_action.unwrap().action_code, "complete_overdue_task");
    }

    #[test]
    fn blocked_workstream_outranks_overdue_negotiation_followup() {
        let (db, _d) = setup();
        let m = new_matter(&db, "traffic_accident");
        db.write(|conn| workstreams::update_status(conn, &m, "medical", "blocked", None)).unwrap();
        db.write(|conn| {
            let wid = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO waiting_for(id,matter_id,party_label,item_label,since_at,follow_up_at,status,source_ref)
                 VALUES(?1,?2,'insurer','claim file','2020-01-01','2020-01-01','open','manual')",
                params![wid, m],
            )?;
            Ok(())
        }).unwrap();
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        assert_eq!(plan.primary_action.unwrap().action_code, "unblock_workstream");
    }

    #[test]
    fn approved_medical_contradiction_ranks_as_a_blocking_conflict() {
        let (db, _d) = setup();
        let m = new_matter(&db, "traffic_accident");
        db.write(|conn| {
            let run_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO ai_runs(id,matter_id,capability,status,context_manifest_sha256,started_at)
                 VALUES(?1,?2,'extract_medical_evidence','completed','x','x')",
                params![run_id, m],
            )?;
            conn.execute(
                "INSERT INTO ai_proposals(id,ai_run_id,matter_id,proposal_kind,structured_json,source_manifest_json,status)
                 VALUES(?1,?2,?3,'medical_contradiction','{}','{}','approved')",
                params![Uuid::new_v4().to_string(), run_id, m],
            )?;
            Ok(())
        }).unwrap();
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let found = plan.candidates.iter().find(|c| c.action_code == "review_approved_contradiction").unwrap();
        assert_eq!(found.rank_category, RANK_BLOCKING_CONFLICT);
    }

    #[test]
    fn pending_ai_review_never_outranks_a_committed_overdue_deadline() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        insert_committed_deadline(&db, &m, "2020-01-01");
        db.write(|conn| {
            let run_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO ai_runs(id,matter_id,capability,status,context_manifest_sha256,started_at)
                 VALUES(?1,?2,'extract_facts','completed','x','x')",
                params![run_id, m],
            )?;
            conn.execute(
                "INSERT INTO ai_proposals(id,ai_run_id,matter_id,proposal_kind,structured_json,source_manifest_json,status)
                 VALUES(?1,?2,?3,'understanding_date','{}','{}','pending')",
                params![Uuid::new_v4().to_string(), run_id, m],
            )?;
            Ok(())
        }).unwrap();
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        assert_eq!(plan.primary_action.unwrap().action_code, "resolve_overdue_deadline");
    }

    #[test]
    fn required_missing_evidence_outranks_overdue_waiting_followup() {
        let (db, _d) = setup();
        let m = new_matter(&db, "traffic_accident");
        db.write(|conn| {
            let wid = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO waiting_for(id,matter_id,party_label,item_label,since_at,follow_up_at,status,source_ref)
                 VALUES(?1,?2,'insurer','claim file','2020-01-01','2020-01-01','open','manual')",
                params![wid, m],
            )?;
            Ok(())
        }).unwrap();
        db.write(|conn| requirements::update_status(conn, &m, "id_document", "not_collected", None)).unwrap();
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        assert_eq!(plan.primary_action.unwrap().action_code, "collect_required_evidence");
    }

    #[test]
    fn backlog_recommended_evidence_never_ranks_above_open_task() {
        let (db, _d) = setup();
        let m = new_matter(&db, "traffic_accident");
        insert_task(&db, &m, "an open task", None);
        db.write(|conn| requirements::update_status(conn, &m, "police_report", "not_collected", None)).unwrap();
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        assert_eq!(plan.primary_action.unwrap().action_code, "complete_open_task");
        assert!(plan.candidates.iter().any(|c| c.action_code == "collect_recommended_evidence"));
    }

    #[test]
    fn no_candidates_yields_no_primary_action() {
        let (db, _d) = setup();
        let m = new_matter(&db, "civil_commercial");
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        assert!(plan.primary_action.is_none());
        assert!(plan.candidates.is_empty());
    }

    #[test]
    fn extraction_failure_ranks_below_evidence_gaps_and_above_pending_review() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        db.write(|conn| {
            let doc_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO documents(id,matter_id,logical_title,category,created_at,updated_at)
                 VALUES(?1,?2,'x.pdf','general','x','x')",
                params![doc_id, m],
            )?;
            let ver_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO document_versions(id,matter_id,document_id,content_sha256,stale,extraction_state,created_at)
                 VALUES(?1,?2,?3,'x',0,'failed','x')",
                params![ver_id, m, doc_id],
            )?;
            Ok(())
        }).unwrap();
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let found = plan.candidates.iter().find(|c| c.action_code == "repair_document_extraction").unwrap();
        assert_eq!(found.rank_category, RANK_INTEGRITY_ISSUE);
        assert!(found.rank_category > RANK_EVIDENCE_BLOCKING);
        assert!(found.rank_category < RANK_PENDING_REVIEW);
    }

    // 20-23: plan/tie-break determinism.
    #[test]
    fn two_computations_of_the_same_state_produce_identical_candidate_order() {
        let (db, _d) = setup();
        let m = new_matter(&db, "traffic_accident");
        insert_committed_deadline(&db, &m, "2020-01-01");
        insert_task(&db, &m, "a", Some("2020-01-05"));
        insert_task(&db, &m, "b", Some("2020-01-05"));
        let plan1 = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let plan2 = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let codes1: Vec<_> = plan1.candidates.iter().map(|c| c.fingerprint.clone()).collect();
        let codes2: Vec<_> = plan2.candidates.iter().map(|c| c.fingerprint.clone()).collect();
        assert_eq!(codes1, codes2);
    }

    #[test]
    fn same_due_date_tasks_break_ties_by_id_not_insertion_order_alone() {
        let (db, _d) = setup();
        let m = new_matter(&db, "traffic_accident");
        insert_task(&db, &m, "a", Some("2020-01-05"));
        insert_task(&db, &m, "b", Some("2020-01-05"));
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let ids: Vec<_> = plan.candidates.iter().filter(|c| c.source_type == "task").map(|c| c.target_id.clone().unwrap()).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted);
    }

    #[test]
    fn alternatives_never_include_the_primary_action() {
        let (db, _d) = setup();
        let m = new_matter(&db, "traffic_accident");
        for i in 0..6 { insert_task(&db, &m, &format!("t{i}"), Some("2020-01-01")); }
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let primary_fp = plan.primary_action.unwrap().fingerprint;
        assert!(plan.alternatives.iter().all(|a| a.fingerprint != primary_fp));
        assert!(plan.alternatives.len() <= 4);
    }

    #[test]
    fn cross_matter_data_never_leaks_into_another_matters_plan() {
        let (db, _d) = setup();
        let m1 = new_matter(&db, "generic_civil");
        let m2 = new_matter(&db, "generic_civil");
        insert_committed_deadline(&db, &m1, "2020-01-01");
        let plan2 = compute_plan_for_date(&db, &m2, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        assert!(plan2.candidates.is_empty());
    }

    // 24-25: fingerprint stability/regeneration.
    #[test]
    fn same_deadline_always_produces_the_same_fingerprint() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        insert_committed_deadline(&db, &m, "2020-01-01");
        let p1 = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let p2 = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 2).unwrap(), "2026-01-02T00:00:00Z").unwrap();
        assert_eq!(p1.primary_action.unwrap().fingerprint, p2.primary_action.unwrap().fingerprint);
    }

    #[test]
    fn superseding_a_deadline_produces_a_new_fingerprint_not_hidden_by_old_dismissal() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        let old_id = insert_committed_deadline(&db, &m, "2020-01-01");
        let today = NaiveDate::from_ymd_opt(2026, 1, 1).unwrap();
        let plan = compute_plan_for_date(&db, &m, today, "2026-01-01T00:00:00Z").unwrap();
        let old_fp = plan.primary_action.unwrap().fingerprint;
        let new_id = Uuid::new_v4().to_string();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO legal_deadlines(id,matter_id,action,due_at,state,trigger_source_ref,created_at)
                 VALUES(?1,?2,'x','2020-02-01','committed','manual','x')",
                params![new_id, m],
            )?;
            conn.execute("UPDATE legal_deadlines SET state='superseded',superseded_by=?2 WHERE id=?1", params![old_id, new_id])?;
            Ok(())
        }).unwrap();
        let plan2 = compute_plan_for_date(&db, &m, today, "2026-01-01T00:00:00Z").unwrap();
        let new_fp = plan2.primary_action.unwrap().fingerprint;
        assert_ne!(old_fp, new_fp);
    }

    // 26-29: dismissal/snooze semantics.
    #[test]
    fn dismissing_a_committed_deadline_recommendation_is_rejected() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        insert_committed_deadline(&db, &m, "2020-01-01");
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let fp = plan.primary_action.unwrap().fingerprint;
        assert!(set_recommendation_state(&db, &m, &fp, "dismissed", None, None).is_err());
    }

    #[test]
    fn snoozing_a_committed_deadline_is_allowed_but_never_mutates_the_deadline_itself() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        let id = insert_committed_deadline(&db, &m, "2020-01-01");
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let fp = plan.primary_action.unwrap().fingerprint;
        set_recommendation_state(&db, &m, &fp, "snoozed", Some("2026-01-05"), None).unwrap();
        let state: String = db.read(|conn| Ok(conn.query_row("SELECT state FROM legal_deadlines WHERE id=?1", [&id], |r| r.get(0))?)).unwrap();
        assert_eq!(state, "committed");
    }

    #[test]
    fn dismissing_an_open_task_recommendation_suppresses_it_from_primary_action() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        insert_task(&db, &m, "only task", None);
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let fp = plan.primary_action.unwrap().fingerprint;
        set_recommendation_state(&db, &m, &fp, "dismissed", None, None).unwrap();
        let plan2 = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        assert!(plan2.primary_action.is_none());
        assert_eq!(plan2.candidates.len(), 1);
        assert_eq!(plan2.candidates[0].recommendation_state, "dismissed");
    }

    #[test]
    fn a_new_different_condition_is_never_hidden_by_an_old_dismissal() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        let t1 = insert_task(&db, &m, "task one", None);
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let fp = plan.primary_action.unwrap().fingerprint;
        set_recommendation_state(&db, &m, &fp, "dismissed", None, None).unwrap();
        let _ = insert_task(&db, &m, "task two", None);
        let plan2 = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        assert!(plan2.primary_action.is_some());
        assert_ne!(plan2.primary_action.as_ref().unwrap().fingerprint, fp);
        let _ = t1;
    }

    #[test]
    fn snoozed_item_resurfaces_once_the_snooze_expires() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        insert_task(&db, &m, "only task", None);
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let fp = plan.primary_action.unwrap().fingerprint;
        set_recommendation_state(&db, &m, &fp, "snoozed", Some("2026-01-05T00:00:00Z"), None).unwrap();
        let while_snoozed = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 2).unwrap(), "2026-01-02T00:00:00Z").unwrap();
        assert!(while_snoozed.primary_action.is_none());
        let after_snooze = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(), "2026-01-10T00:00:00Z").unwrap();
        assert!(after_snooze.primary_action.is_some());
    }

    // 30-32: task conversion.
    #[test]
    fn converting_a_recommendation_creates_exactly_one_real_task() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        db.write(|conn| workstreams::update_status(conn, &m, "medical", "blocked", None)).unwrap();
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let fp = plan.primary_action.unwrap().fingerprint;
        let task_id = convert_to_task(&db, &m, &fp, None).unwrap();
        let count: i64 = db.read(|conn| Ok(conn.query_row("SELECT COUNT(*) FROM tasks WHERE id=?1", [&task_id], |r| r.get(0))?)).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn repeated_conversion_clicks_never_create_a_duplicate_task() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        db.write(|conn| workstreams::update_status(conn, &m, "medical", "blocked", None)).unwrap();
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let fp = plan.primary_action.unwrap().fingerprint;
        let t1 = convert_to_task(&db, &m, &fp, None).unwrap();
        let t2 = convert_to_task(&db, &m, &fp, None).unwrap();
        assert_eq!(t1, t2);
        let count: i64 = db.read(|conn| Ok(conn.query_row("SELECT COUNT(*) FROM tasks WHERE matter_id=?1", [&m], |r| r.get(0))?)).unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn task_completion_does_not_automatically_change_recommendation_state() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        db.write(|conn| workstreams::update_status(conn, &m, "medical", "blocked", None)).unwrap();
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let fp = plan.primary_action.unwrap().fingerprint;
        let task_id = convert_to_task(&db, &m, &fp, None).unwrap();
        db.write(|conn| { conn.execute("UPDATE tasks SET status='done' WHERE id=?1", [&task_id])?; Ok(()) }).unwrap();
        let state: String = db.read(|conn| {
            Ok(conn.query_row("SELECT state FROM action_recommendations WHERE matter_id=?1 AND fingerprint=?2", params![m, fp], |r| r.get(0))?)
        }).unwrap();
        assert_eq!(state, "converted_to_task");
    }

    // 33-37: cross-surface ranking consistency.
    #[test]
    fn action_center_orders_matters_by_the_same_rank_category_as_their_own_plan() {
        let (db, _d) = setup();
        let m1 = new_matter(&db, "generic_civil");
        let m2 = new_matter(&db, "generic_civil");
        insert_task(&db, &m1, "open", None);
        insert_committed_deadline(&db, &m2, "2020-01-01");
        let entries = compute_action_center_for_date(&db, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        assert_eq!(entries[0].matter_id, m2);
    }

    #[test]
    fn inactive_matters_are_excluded_from_the_action_center() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        insert_committed_deadline(&db, &m, "2020-01-01");
        db.write(|conn| { conn.execute("UPDATE matters SET status='closed' WHERE id=?1", [&m])?; Ok(()) }).unwrap();
        let entries = compute_action_center_for_date(&db, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn matter_isolation_holds_inside_the_action_center_too() {
        let (db, _d) = setup();
        let m1 = new_matter(&db, "generic_civil");
        let m2 = new_matter(&db, "generic_civil");
        insert_committed_deadline(&db, &m1, "2020-01-01");
        let entries = compute_action_center_for_date(&db, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let e2 = entries.iter().find(|e| e.matter_id == m2).unwrap();
        assert!(e2.plan.candidates.is_empty());
    }

    #[test]
    fn a_matters_case_health_and_action_plan_never_disagree_on_the_presence_of_an_overdue_deadline() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        insert_committed_deadline(&db, &m, "2020-01-01");
        let health = crate::case_health::compute(&db, &m).unwrap();
        let plan = compute_plan(&db, &m).unwrap();
        assert_eq!(health.next_best_action.code, "resolve_overdue_deadline");
        assert_eq!(plan.primary_action.unwrap().action_code, "resolve_overdue_deadline");
    }

    #[test]
    fn action_center_ordering_is_deterministic_across_two_computations() {
        let (db, _d) = setup();
        for _ in 0..3 { new_matter(&db, "generic_civil"); }
        let a = compute_action_center_for_date(&db, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let b = compute_action_center_for_date(&db, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let ids_a: Vec<_> = a.iter().map(|e| e.matter_id.clone()).collect();
        let ids_b: Vec<_> = b.iter().map(|e| e.matter_id.clone()).collect();
        assert_eq!(ids_a, ids_b);
    }

    // 38-42: C3/C4/historical-date integration.
    #[test]
    fn approved_wage_gap_signal_produces_an_evidence_blocking_candidate() {
        let (db, _d) = setup();
        let m = new_matter(&db, "work_accident");
        db.write(|conn| {
            let run_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO ai_runs(id,matter_id,capability,status,context_manifest_sha256,started_at)
                 VALUES(?1,?2,'extract_wage_economic_evidence','completed','x','x')",
                params![run_id, m],
            )?;
            conn.execute(
                "INSERT INTO ai_proposals(id,ai_run_id,matter_id,proposal_kind,structured_json,source_manifest_json,status)
                 VALUES(?1,?2,?3,'wage_gap_signal','{}','{}','approved')",
                params![Uuid::new_v4().to_string(), run_id, m],
            )?;
            Ok(())
        }).unwrap();
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let found = plan.candidates.iter().find(|c| c.action_code == "review_approved_evidence_gap").unwrap();
        assert_eq!(found.rank_category, RANK_EVIDENCE_BLOCKING);
    }

    #[test]
    fn pending_ai_proposal_never_counted_toward_approved_signal_candidates() {
        let (db, _d) = setup();
        let m = new_matter(&db, "work_accident");
        db.write(|conn| {
            let run_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO ai_runs(id,matter_id,capability,status,context_manifest_sha256,started_at)
                 VALUES(?1,?2,'extract_wage_economic_evidence','completed','x','x')",
                params![run_id, m],
            )?;
            conn.execute(
                "INSERT INTO ai_proposals(id,ai_run_id,matter_id,proposal_kind,structured_json,source_manifest_json,status)
                 VALUES(?1,?2,?3,'wage_gap_signal','{}','{}','pending')",
                params![Uuid::new_v4().to_string(), run_id, m],
            )?;
            Ok(())
        }).unwrap();
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        assert!(plan.candidates.iter().all(|c| c.action_code != "review_approved_evidence_gap"));
        assert!(plan.candidates.iter().any(|c| c.action_code == "review_ai_proposals"));
    }

    #[test]
    fn a_decade_old_document_alone_never_generates_an_overdue_candidate() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        db.write(|conn| {
            conn.execute(
                "INSERT INTO documents(id,matter_id,logical_title,category,created_at,updated_at)
                 VALUES('doc1',?1,'x.pdf','general','2016-01-01','2016-01-01')",
                [&m],
            )?;
            conn.execute(
                "INSERT INTO document_versions(id,matter_id,document_id,content_sha256,stale,extraction_state,created_at)
                 VALUES('v1',?1,'doc1','x',0,'complete','2016-01-01')",
                [&m],
            )?;
            Ok(())
        }).unwrap();
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        assert!(plan.candidates.is_empty());
    }

    #[test]
    fn historical_matter_with_only_a_stale_required_document_still_ranks_it_as_evidence_blocking() {
        let (db, _d) = setup();
        let m = new_matter(&db, "traffic_accident");
        db.write(|conn| requirements::update_status(conn, &m, "id_document", "stale", None)).unwrap();
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let found = plan.candidates.iter().find(|c| c.action_code == "refresh_required_evidence").unwrap();
        assert_eq!(found.rank_category, RANK_EVIDENCE_BLOCKING);
    }

    #[test]
    fn liability_issue_and_regime_review_never_reinterprets_liability_evidence_itself() {
        let (db, _d) = setup();
        let m = new_matter(&db, "traffic_accident");
        db.write(|conn| {
            let run_id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO ai_runs(id,matter_id,capability,status,context_manifest_sha256,started_at)
                 VALUES(?1,?2,'extract_liability_evidence','completed','x','x')",
                params![run_id, m],
            )?;
            conn.execute(
                "INSERT INTO ai_proposals(id,ai_run_id,matter_id,proposal_kind,structured_json,source_manifest_json,status)
                 VALUES(?1,?2,?3,'liability_issue','{\"issueType\":\"eligibility_question\",\"description\":\"x\"}','{}','approved')",
                params![Uuid::new_v4().to_string(), run_id, m],
            )?;
            Ok(())
        }).unwrap();
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let found = plan.candidates.iter().find(|c| c.action_code == "review_liability_issue").unwrap();
        assert!(!found.reason.contains("אשם"));
    }

    // 43-44: close/reopen persistence. Windows-gated: reopening a second real
    // `DbState::open()` against the same encrypted DB file depends on the OS
    // keyring backend actually persisting the key between opens, which does
    // not hold in this Linux dev/CI environment - same established pattern
    // as every other reopen test in this codebase since RC0.
    #[cfg(target_os = "windows")]
    #[test]
    fn recommendation_state_survives_a_fresh_db_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        let db = DbState::open(path.clone()).unwrap();
        let m = new_matter(&db, "generic_civil");
        insert_task(&db, &m, "only task", None);
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let fp = plan.primary_action.unwrap().fingerprint;
        set_recommendation_state(&db, &m, &fp, "acknowledged", None, None).unwrap();
        drop(db);
        let db2 = DbState::open(path).unwrap();
        let plan2 = compute_plan_for_date(&db2, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        assert_eq!(plan2.candidates[0].recommendation_state, "acknowledged");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn deadline_satisfaction_survives_a_fresh_db_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.db");
        let db = DbState::open(path.clone()).unwrap();
        let m = new_matter(&db, "generic_civil");
        let id = insert_committed_deadline(&db, &m, "2020-01-01");
        satisfy_deadline(&db, &id, Some("done")).unwrap();
        drop(db);
        let db2 = DbState::open(path).unwrap();
        let state: String = db2.read(|conn| {
            Ok(conn.query_row("SELECT state FROM legal_deadlines WHERE id=?1", [&id], |r| r.get(0))?)
        }).unwrap();
        let note: Option<String> = db2.read(|conn| {
            Ok(conn.query_row("SELECT satisfaction_note FROM legal_deadline_satisfaction WHERE deadline_id=?1", [&id], |r| r.get(0))?)
        }).unwrap();
        assert_eq!(state, "satisfied");
        assert_eq!(note.as_deref(), Some("done"));
    }

    // 45-50: integrity rejection.
    #[test]
    fn malformed_recommendation_state_is_rejected() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        insert_task(&db, &m, "t", None);
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let fp = plan.primary_action.unwrap().fingerprint;
        assert!(set_recommendation_state(&db, &m, &fp, "made_up_state", None, None).is_err());
    }

    #[test]
    fn cross_matter_recommendation_mutation_is_rejected() {
        let (db, _d) = setup();
        let m1 = new_matter(&db, "generic_civil");
        let m2 = new_matter(&db, "generic_civil");
        insert_task(&db, &m1, "t", None);
        let plan = compute_plan_for_date(&db, &m1, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let fp = plan.primary_action.unwrap().fingerprint;
        assert!(set_recommendation_state(&db, &m2, &fp, "dismissed", None, None).is_err());
    }

    #[test]
    fn invalid_fingerprint_is_rejected() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        assert!(set_recommendation_state(&db, &m, "not-a-real-fingerprint", "dismissed", None, None).is_err());
    }

    #[test]
    fn critical_recommendation_cannot_be_permanently_hidden_by_a_generic_dismiss() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        insert_committed_deadline(&db, &m, "2020-01-01");
        let plan = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let fp = plan.primary_action.unwrap().fingerprint;
        let err = set_recommendation_state(&db, &m, &fp, "dismissed", None, None);
        assert!(err.is_err());
        let plan_after = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        assert!(plan_after.primary_action.is_some());
    }

    #[test]
    fn no_domain_mutation_happens_merely_from_computing_a_plan() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        insert_committed_deadline(&db, &m, "2020-01-01");
        let _ = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let _ = compute_plan_for_date(&db, &m, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), "2026-01-01T00:00:00Z").unwrap();
        let count: i64 = db.read(|conn| Ok(conn.query_row("SELECT COUNT(*) FROM action_recommendations WHERE matter_id=?1", [&m], |r| r.get(0))?)).unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn no_task_or_deadline_is_ever_auto_created_by_reading_the_action_center() {
        let (db, _d) = setup();
        let m = new_matter(&db, "generic_civil");
        insert_committed_deadline(&db, &m, "2020-01-01");
        let before: i64 = db.read(|conn| Ok(conn.query_row("SELECT COUNT(*) FROM tasks WHERE matter_id=?1", [&m], |r| r.get(0))?)).unwrap();
        let _ = compute_action_center(&db).unwrap();
        let after: i64 = db.read(|conn| Ok(conn.query_row("SELECT COUNT(*) FROM tasks WHERE matter_id=?1", [&m], |r| r.get(0))?)).unwrap();
        assert_eq!(before, after);
    }

}
