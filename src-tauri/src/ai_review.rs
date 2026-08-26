use crate::{
    db::DbState,
    error::{AppError, AppResult},
    AppState,
};
use rusqlite::params;
use serde_json::{json, Value};
use tauri::State;

/// Persistent matter-scoped AI review queue. Unlike `get_ai_run`, this read path
/// requires only the matter id, so pending proposals remain reachable after
/// navigation or an application restart. The query re-applies matter isolation on
/// both ai_proposals and ai_runs and source excerpts are resolved under the same
/// matter before being exposed to the reviewer.
pub(crate) fn list_for_matter(db: &DbState, matter_id: &str) -> AppResult<Vec<Value>> {
    db.read(|conn| {
        struct Row {
            id: String,
            run_id: String,
            proposal_kind: String,
            structured_json: String,
            status: String,
            reviewed_at: Option<String>,
            review_note: Option<String>,
            manifest_sha256: String,
            run_started_at: String,
        }

        let mut stmt = conn.prepare(
            "SELECT p.id,p.ai_run_id,p.proposal_kind,p.structured_json,p.status,
                    p.reviewed_at,p.review_note,r.context_manifest_sha256,r.started_at
             FROM ai_proposals p
             JOIN ai_runs r ON r.id=p.ai_run_id AND r.matter_id=p.matter_id
             WHERE p.matter_id=?1 AND r.matter_id=?1
             ORDER BY CASE WHEN p.status='pending' THEN 0 ELSE 1 END,
                      r.started_at DESC,p.id",
        )?;
        let rows = stmt
            .query_map([matter_id], |r| {
                Ok(Row {
                    id: r.get(0)?,
                    run_id: r.get(1)?,
                    proposal_kind: r.get(2)?,
                    structured_json: r.get(3)?,
                    status: r.get(4)?,
                    reviewed_at: r.get(5)?,
                    review_note: r.get(6)?,
                    manifest_sha256: r.get(7)?,
                    run_started_at: r.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut excerpt_stmt = conn.prepare(
            "SELECT p.page_number,p.display_text,
                    (SELECT o.file_name FROM file_occurrences o
                     WHERE o.document_version_id=p.document_version_id
                       AND o.matter_id=p.matter_id AND o.exists_now=1 LIMIT 1)
             FROM document_pages p
             WHERE p.id=?1 AND p.matter_id=?2",
        )?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let structured: Value = serde_json::from_str(&row.structured_json)?;
            let source_ids: Vec<String> = structured
                .get("sourceIds")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();

            let mut excerpts = Vec::with_capacity(source_ids.len());
            for source_id in &source_ids {
                if let Ok((page, text, file_name)) = excerpt_stmt.query_row(
                    params![source_id, matter_id],
                    |r| {
                        Ok((
                            r.get::<_, Option<i64>>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, Option<String>>(2)?,
                        ))
                    },
                ) {
                    let excerpt: String = text.chars().take(400).collect();
                    excerpts.push(json!({
                        "sourceId": source_id,
                        "page": page,
                        "fileName": file_name,
                        "excerpt": excerpt,
                        "truncated": text.chars().count() > 400,
                    }));
                }
            }

            out.push(json!({
                "id": row.id,
                "runId": row.run_id,
                "proposalKind": row.proposal_kind,
                "structured": structured,
                "status": row.status,
                "reviewedAt": row.reviewed_at,
                "reviewNote": row.review_note,
                "sourceManifestSha256": row.manifest_sha256,
                "runStartedAt": row.run_started_at,
                "sourceExcerpts": excerpts,
            }));
        }
        Ok(out)
    })
}

#[tauri::command]
pub fn list_ai_proposals(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id = payload
        .get("matterId")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Validation("matterId required".into()))?;
    Ok(Value::Array(list_for_matter(&state.db, matter_id)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use rusqlite::params;
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn matter_queue_is_persistent_and_strictly_matter_isolated() {
        let root = std::env::temp_dir().join(format!("tahrir-ai-review-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let db = DbState::open(root.join("app.db")).unwrap();
        let now = Utc::now().to_rfc3339();
        let matter_a = Uuid::new_v4().to_string();
        let matter_b = Uuid::new_v4().to_string();
        let run_a = Uuid::new_v4().to_string();
        let run_b = Uuid::new_v4().to_string();
        let proposal_a = Uuid::new_v4().to_string();
        let proposal_b = Uuid::new_v4().to_string();

        db.write(|conn| {
            conn.execute(
                "INSERT INTO matters(id,title,matter_type,created_at,updated_at)
                 VALUES(?1,'A','generic_civil',?3,?3),(?2,'B','generic_civil',?3,?3)",
                params![matter_a, matter_b, now],
            )?;
            conn.execute(
                "INSERT INTO ai_runs(id,matter_id,capability,status,context_manifest_sha256,
                                     client_egress_approved,started_at,finished_at)
                 VALUES(?1,?2,'extract_medical_event','completed','sha-a',0,?5,?5),
                       (?3,?4,'extract_wage_record','completed','sha-b',0,?5,?5)",
                params![run_a, matter_a, run_b, matter_b, now],
            )?;
            conn.execute(
                "INSERT INTO ai_proposals(id,ai_run_id,matter_id,proposal_kind,structured_json,
                                          source_manifest_json,status)
                 VALUES(?1,?2,?3,'extract_medical_event',?4,'{}','pending')",
                params![proposal_a, run_a, matter_a, r#"{"sourceIds":[],"eventDate":null,"providerName":null,"treatmentSummary":"A"}"#],
            )?;
            conn.execute(
                "INSERT INTO ai_proposals(id,ai_run_id,matter_id,proposal_kind,structured_json,
                                          source_manifest_json,status)
                 VALUES(?1,?2,?3,'extract_wage_record',?4,'{}','pending')",
                params![proposal_b, run_b, matter_b, r#"{"sourceIds":[],"periodStart":null,"periodEnd":null,"employerName":null,"grossAmountCents":100}"#],
            )?;
            Ok(())
        })
        .unwrap();

        // DbState::read opens a fresh keyed SQLite connection on every call, so
        // these reads prove the queue does not depend on any in-memory runId state.
        let first = list_for_matter(&db, &matter_a).unwrap();
        let second = list_for_matter(&db, &matter_a).unwrap();
        let other = list_for_matter(&db, &matter_b).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1);
        assert_eq!(first[0]["id"].as_str(), Some(proposal_a.as_str()));
        assert_eq!(second[0]["runId"].as_str(), Some(run_a.as_str()));
        assert_eq!(other.len(), 1);
        assert_eq!(other[0]["id"].as_str(), Some(proposal_b.as_str()));

        let _ = fs::remove_dir_all(root);
    }
}
