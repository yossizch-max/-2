//! Human-only review workflow for the existing `fact_conflicts` table.
//!
//! B6 Case Health may surface an unresolved conflict as the next operational action,
//! so the product must provide a way for a lawyer to inspect and resolve it. This
//! module deliberately does not detect conflicts, call AI, or make any substantive
//! determination. It only exposes unresolved conflicts whose two referenced facts
//! are still valid in the same matter and lets a human mark one resolved.
use crate::{
    db::DbState,
    error::{AppError, AppResult},
    AppState,
};
use rusqlite::params;
use serde_json::{json, Value};
use tauri::State;

pub(crate) fn list_for_matter(db: &DbState, matter_id: &str) -> AppResult<Vec<Value>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(
            "SELECT c.id,c.matter_id,c.created_at,
                    a.id,a.subject,a.predicate,a.value_text,
                    b.id,b.subject,b.predicate,b.value_text
             FROM fact_conflicts c
             JOIN verified_facts a
               ON a.id=c.fact_a_id AND a.matter_id=c.matter_id AND a.status='valid'
             JOIN verified_facts b
               ON b.id=c.fact_b_id AND b.matter_id=c.matter_id AND b.status='valid'
             WHERE c.matter_id=?1 AND c.status='unresolved'
             ORDER BY c.created_at,c.id",
        )?;
        let rows = stmt
            .query_map([matter_id], |r| {
                Ok(json!({
                    "id": r.get::<_, String>(0)?,
                    "matterId": r.get::<_, String>(1)?,
                    "status": "unresolved",
                    "createdAt": r.get::<_, String>(2)?,
                    "factA": {
                        "id": r.get::<_, String>(3)?,
                        "subject": r.get::<_, String>(4)?,
                        "predicate": r.get::<_, String>(5)?,
                        "value": r.get::<_, String>(6)?,
                    },
                    "factB": {
                        "id": r.get::<_, String>(7)?,
                        "subject": r.get::<_, String>(8)?,
                        "predicate": r.get::<_, String>(9)?,
                        "value": r.get::<_, String>(10)?,
                    },
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
}

pub(crate) fn resolve_for_matter(
    db: &DbState,
    matter_id: &str,
    conflict_id: &str,
    resolution_note: Option<&str>,
) -> AppResult<()> {
    let note = resolution_note
        .map(str::trim)
        .filter(|v| !v.is_empty());
    db.write(|conn| {
        let changed = conn.execute(
            "UPDATE fact_conflicts
             SET status='resolved',resolution_note=?3
             WHERE id=?1 AND matter_id=?2 AND status='unresolved'",
            params![conflict_id, matter_id, note],
        )?;
        if changed != 1 {
            return Err(AppError::Validation(
                "fact conflict not resolvable in this matter".into(),
            ));
        }
        Ok(())
    })
}

#[tauri::command]
pub fn list_fact_conflicts(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id = payload
        .get("matterId")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Validation("matterId required".into()))?;
    Ok(Value::Array(list_for_matter(&state.db, matter_id)?))
}

#[tauri::command]
pub fn resolve_fact_conflict(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id = payload
        .get("matterId")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Validation("matterId required".into()))?;
    let conflict_id = payload
        .get("conflictId")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::Validation("conflictId required".into()))?;
    let note = payload.get("resolutionNote").and_then(Value::as_str);
    resolve_for_matter(&state.db, matter_id, conflict_id, note)?;
    Ok(json!({"ok": true}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::fs;
    use uuid::Uuid;

    fn add_matter(db: &DbState, title: &str) -> String {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO matters(id,title,matter_type,status,workflow_stage,created_at,updated_at)
                 VALUES(?1,?2,'generic_civil','active','intake',?3,?3)",
                params![id, title, now],
            )?;
            Ok(())
        })
        .unwrap();
        id
    }

    fn add_fact(db: &DbState, matter_id: &str, status: &str, value: &str) -> String {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO verified_facts(id,matter_id,subject,predicate,value_text,status,verified_at)
                 VALUES(?1,?2,'subject','says',?3,?4,?5)",
                params![id, matter_id, value, status, now],
            )?;
            Ok(())
        })
        .unwrap();
        id
    }

    fn add_conflict(db: &DbState, matter_id: &str, a: &str, b: &str, status: &str) -> String {
        let id = Uuid::new_v4().to_string();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO fact_conflicts(id,matter_id,fact_a_id,fact_b_id,status,created_at)
                 VALUES(?1,?2,?3,?4,?5,'2026-01-01T00:00:00Z')",
                params![id, matter_id, a, b, status],
            )?;
            Ok(())
        })
        .unwrap();
        id
    }

    #[test]
    fn list_and_resolve_are_human_only_and_strictly_matter_isolated() {
        let root = std::env::temp_dir().join(format!("tahrir-fact-conflicts-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let db = DbState::open(root.join("app.db")).unwrap();
        let matter_a = add_matter(&db, "A");
        let matter_b = add_matter(&db, "B");
        let a1 = add_fact(&db, &matter_a, "valid", "one");
        let a2 = add_fact(&db, &matter_a, "valid", "two");
        let b1 = add_fact(&db, &matter_b, "valid", "one");
        let b2 = add_fact(&db, &matter_b, "valid", "two");
        let conflict_a = add_conflict(&db, &matter_a, &a1, &a2, "unresolved");
        let _conflict_b = add_conflict(&db, &matter_b, &b1, &b2, "unresolved");

        let rows = list_for_matter(&db, &matter_a).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"].as_str(), Some(conflict_a.as_str()));
        assert!(resolve_for_matter(&db, &matter_b, &conflict_a, None).is_err());
        resolve_for_matter(&db, &matter_a, &conflict_a, Some("lawyer reviewed sources")).unwrap();
        assert!(list_for_matter(&db, &matter_a).unwrap().is_empty());
        assert!(resolve_for_matter(&db, &matter_a, &conflict_a, None).is_err());

        drop(db);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn conflicts_with_an_invalidated_fact_are_not_presented_as_open_review_work() {
        let root = std::env::temp_dir().join(format!("tahrir-fact-conflicts-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let db = DbState::open(root.join("app.db")).unwrap();
        let matter_id = add_matter(&db, "A");
        let valid = add_fact(&db, &matter_id, "valid", "one");
        let invalid = add_fact(&db, &matter_id, "invalidated", "two");
        add_conflict(&db, &matter_id, &valid, &invalid, "unresolved");
        assert!(list_for_matter(&db, &matter_id).unwrap().is_empty());

        drop(db);
        let _ = fs::remove_dir_all(root);
    }
}
