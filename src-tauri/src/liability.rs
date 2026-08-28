//! Phase C, milestone C4, Part B: Liability Evidence Intelligence - the read-only
//! Liability Brief and Liability Evidence Matrix. Mirrors `medical.rs`/`wage.rs`'s
//! pattern: pure read models over already-authoritative state (approved
//! `liability_*` proposals from `ai.rs` plus the pre-existing Liability Ledger,
//! `liability_facts`), no writes, no AI calls, no fault/negligence/credibility
//! determination anywhere in this module.
use crate::{db::DbState, error::AppResult};
use serde_json::{json, Value};

fn fetch_items(conn: &rusqlite::Connection, matter_id: &str, kind: &str) -> AppResult<Vec<Value>> {
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
}

/// A generated summary preferring approved/verified state; every not-yet-approved
/// item is labeled `pending: true`. Never determines fault, negligence, or which
/// party's account is true - only organizes what was documented.
pub fn build_liability_brief(db: &DbState, matter_id: &str) -> AppResult<Value> {
    db.read(|conn| {
        let versions = fetch_items(conn, matter_id, "liability_version_statement")?;
        let witnesses = fetch_items(conn, matter_id, "liability_witness_statement")?;
        let scene_evidence = fetch_items(conn, matter_id, "liability_scene_evidence")?;
        let police_evidence = fetch_items(conn, matter_id, "liability_police_evidence")?;
        let vehicle_damage = fetch_items(conn, matter_id, "liability_vehicle_damage")?;
        let photo_video_evidence = fetch_items(conn, matter_id, "liability_photo_video_evidence")?;
        let expert_opinions = fetch_items(conn, matter_id, "liability_expert_opinion")?;
        let admissions = fetch_items(conn, matter_id, "liability_admission")?;
        let insurer_positions = fetch_items(conn, matter_id, "liability_insurer_position")?;
        let court_findings = fetch_items(conn, matter_id, "liability_court_finding")?;
        let contradictions = fetch_items(conn, matter_id, "liability_contradiction")?;

        let pending_review_count = [
            &versions, &witnesses, &scene_evidence, &police_evidence, &vehicle_damage,
            &photo_video_evidence, &expert_opinions, &admissions, &insurer_positions,
            &court_findings, &contradictions,
        ].iter().flat_map(|v| v.iter()).filter(|item| item["pending"] == Value::Bool(true)).count() as i64;

        Ok(json!({
            "matterId": matter_id,
            "partyVersions": versions,
            "witnesses": witnesses,
            "sceneEvidence": scene_evidence,
            "policeEvidence": police_evidence,
            "vehicleDamage": vehicle_damage,
            "photoVideoEvidence": photo_video_evidence,
            "expertOpinions": expert_opinions,
            "admissions": admissions,
            "insurerPositions": insurer_positions,
            "courtFindings": court_findings,
            "contradictions": contradictions,
            "pendingLiabilityReviewCount": pending_review_count,
        }))
    })
}

fn normalized_statement(item: &Value) -> Option<String> {
    item["structured"]["statement"].as_str()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
}

fn issue_key(item: &Value) -> String {
    item["structured"]["issue"].as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .unwrap_or_default()
}

/// A neutral matrix grouping version statements, witness statements, and scene
/// evidence by a shared `issue` label (a short free-text tag the model or a lawyer
/// attaches to an item, e.g. "traffic light color" - used only to group related
/// items, never to assign truth). `unresolvedConflict` is a purely textual signal:
/// true iff two or more distinct (trimmed, case-normalized) statement texts share
/// the same issue - TAHRIR never decides which one is correct, it only surfaces
/// that they differ. Items with no `issue` are grouped together under an empty
/// issue key ("unassigned"), never dropped.
pub fn build_liability_matrix(db: &DbState, matter_id: &str) -> AppResult<Value> {
    db.read(|conn| {
        let versions = fetch_items(conn, matter_id, "liability_version_statement")?;
        let witnesses = fetch_items(conn, matter_id, "liability_witness_statement")?;
        let scene_evidence = fetch_items(conn, matter_id, "liability_scene_evidence")?;

        let mut issues: Vec<String> = Vec::new();
        let mut push_issue = |k: &str, issues: &mut Vec<String>| {
            if !issues.iter().any(|existing| existing == k) { issues.push(k.to_string()); }
        };
        for item in versions.iter().chain(witnesses.iter()).chain(scene_evidence.iter()) {
            push_issue(&issue_key(item), &mut issues);
        }
        issues.sort();

        let mut rows = Vec::with_capacity(issues.len());
        for issue in &issues {
            let row_versions: Vec<&Value> = versions.iter().filter(|i| &issue_key(i) == issue).collect();
            let row_witnesses: Vec<&Value> = witnesses.iter().filter(|i| &issue_key(i) == issue).collect();
            let row_scene: Vec<&Value> = scene_evidence.iter().filter(|i| &issue_key(i) == issue).collect();

            let mut distinct_statements: Vec<String> = Vec::new();
            for item in row_versions.iter().chain(row_witnesses.iter()) {
                if let Some(s) = normalized_statement(item) {
                    if !distinct_statements.contains(&s) { distinct_statements.push(s); }
                }
            }

            rows.push(json!({
                "issue": if issue.is_empty() { Value::Null } else { Value::String(issue.clone()) },
                "versions": row_versions,
                "witnesses": row_witnesses,
                "objectiveEvidence": row_scene,
                "unresolvedConflict": distinct_statements.len() > 1,
            }));
        }

        Ok(json!({ "matterId": matter_id, "rows": rows }))
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
        let root = std::env::temp_dir().join(format!("tahrir-liability-{}", Uuid::new_v4()));
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

    fn insert_proposal(db: &DbState, matter_id: &str, kind: &str, status: &str, structured: Value) -> String {
        let proposal_id = Uuid::new_v4().to_string();
        let run_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO ai_runs(id,matter_id,capability,status,context_manifest_sha256,client_egress_approved,started_at,finished_at)
                 VALUES(?1,?2,'extract_liability_evidence','completed','sha',0,?3,?3)",
                params![run_id, matter_id, now],
            )?;
            conn.execute(
                "INSERT INTO ai_proposals(id,ai_run_id,matter_id,proposal_kind,structured_json,source_manifest_json,status)
                 VALUES(?1,?2,?3,?4,?5,'{}',?6)",
                params![proposal_id, run_id, matter_id, kind, serde_json::to_string(&structured).unwrap(), status],
            )?;
            Ok(())
        }).unwrap();
        proposal_id
    }

    #[test]
    fn brief_labels_pending_liability_content_explicitly() {
        let (db, root) = new_test_db();
        let matter_id = new_matter(&db);
        insert_proposal(&db, &matter_id, "liability_witness_statement", "pending", json!({
            "sourceIds":["s1"],"witness":"עד","statement":"הרכב עצר באור אדום","issue":null,"date":null,"confidence":null
        }));
        let brief = build_liability_brief(&db, &matter_id).unwrap();
        let witnesses = brief["witnesses"].as_array().unwrap();
        assert_eq!(witnesses.len(), 1);
        assert_eq!(witnesses[0]["pending"], true, "a not-yet-approved witness statement must be labeled pending, never presented as settled");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn matrix_is_matter_isolated() {
        let (db, root) = new_test_db();
        let matter_a = new_matter(&db);
        let matter_b = new_matter(&db);
        insert_proposal(&db, &matter_a, "liability_version_statement", "approved", json!({
            "sourceIds":["s1"],"assertedBy":"תובע","statement":"האור היה ירוק","issue":"צבע הרמזור",
            "eventDate":null,"datePrecision":null,"confidence":null
        }));
        let matrix_a = build_liability_matrix(&db, &matter_a).unwrap();
        let matrix_b = build_liability_matrix(&db, &matter_b).unwrap();
        assert_eq!(matrix_a["rows"].as_array().unwrap().len(), 1);
        assert!(matrix_b["rows"].as_array().unwrap().is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn matrix_flags_a_textual_conflict_without_choosing_a_winner() {
        let (db, root) = new_test_db();
        let matter_id = new_matter(&db);
        insert_proposal(&db, &matter_id, "liability_version_statement", "approved", json!({
            "sourceIds":["s1"],"assertedBy":"תובע","statement":"האור היה ירוק","issue":"צבע הרמזור",
            "eventDate":null,"datePrecision":null,"confidence":null
        }));
        insert_proposal(&db, &matter_id, "liability_version_statement", "approved", json!({
            "sourceIds":["s2"],"assertedBy":"נתבע","statement":"האור היה אדום","issue":"צבע הרמזור",
            "eventDate":null,"datePrecision":null,"confidence":null
        }));
        let matrix = build_liability_matrix(&db, &matter_id).unwrap();
        let rows = matrix["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["unresolvedConflict"], true);
        let serialized = matrix.to_string();
        assert!(!serialized.contains("\"winner\"") && !serialized.contains("\"faultPercentage\""), "the matrix must never output a winner or a fault score");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn matrix_groups_agreeing_statements_without_flagging_a_conflict() {
        let (db, root) = new_test_db();
        let matter_id = new_matter(&db);
        insert_proposal(&db, &matter_id, "liability_version_statement", "approved", json!({
            "sourceIds":["s1"],"assertedBy":"תובע","statement":"האור היה ירוק","issue":"צבע הרמזור",
            "eventDate":null,"datePrecision":null,"confidence":null
        }));
        insert_proposal(&db, &matter_id, "liability_witness_statement", "approved", json!({
            "sourceIds":["s2"],"witness":"עד","statement":"האור היה ירוק","issue":"צבע הרמזור",
            "date":null,"confidence":null
        }));
        let matrix = build_liability_matrix(&db, &matter_id).unwrap();
        let rows = matrix["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["unresolvedConflict"], false);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn items_without_an_issue_are_grouped_but_never_dropped() {
        let (db, root) = new_test_db();
        let matter_id = new_matter(&db);
        insert_proposal(&db, &matter_id, "liability_witness_statement", "approved", json!({
            "sourceIds":["s1"],"witness":"עד","statement":"ראיתי את התאונה","issue":null,"date":null,"confidence":null
        }));
        let matrix = build_liability_matrix(&db, &matter_id).unwrap();
        let rows = matrix["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0]["issue"].is_null(), "an item with no issue label must land in an unassigned row, not be dropped");
        assert_eq!(rows[0]["witnesses"].as_array().unwrap().len(), 1);
        let _ = fs::remove_dir_all(root);
    }
}
