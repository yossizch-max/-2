//! Negotiation & Insurance Workspace (Phase B, milestone B7).
//!
//! This module is deliberately operational and human-controlled. It records insurer
//! claim metadata, communication/follow-up events, and monetary positions. It never
//! recommends or accepts a settlement and exposes no automatic settlement decision.
use crate::{
    db::DbState,
    error::{AppError, AppResult},
    AppState,
};
use chrono::Utc;
use rusqlite::params;
use serde_json::{json, Value};
use tauri::State;
use uuid::Uuid;

const CLAIM_STATUSES: &[&str] = &[
    "open",
    "awaiting_response",
    "negotiating",
    "settled",
    "closed",
];
const EVENT_KINDS: &[&str] = &[
    "call",
    "email",
    "letter",
    "meeting",
    "request",
    "follow_up",
    "other",
];
const POSITION_SIDES: &[&str] = &["our_side", "counterparty"];
const POSITION_KINDS: &[&str] = &["demand", "offer", "counter_offer"];

fn required_trimmed<'a>(payload: &'a Value, key: &str) -> AppResult<&'a str> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::Validation(format!("{key} required")))
}

fn optional_trimmed<'a>(payload: &'a Value, key: &str) -> Option<&'a str> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
}

fn validate_one_of(value: &str, allowed: &[&str], field: &str) -> AppResult<()> {
    if !allowed.contains(&value) {
        return Err(AppError::Validation(format!("unknown {field} \"{value}\"")));
    }
    Ok(())
}

pub(crate) fn list_claims(db: &DbState, matter_id: &str) -> AppResult<Vec<Value>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id,matter_id,insurer_name,claim_number,policy_number,handler_name,
                    handler_contact,status,notes,created_at,updated_at
             FROM insurance_claims
             WHERE matter_id=?1
             ORDER BY updated_at DESC,id",
        )?;
        let rows = stmt
            .query_map([matter_id], |r| {
                Ok(json!({
                    "id": r.get::<_, String>(0)?,
                    "matterId": r.get::<_, String>(1)?,
                    "insurerName": r.get::<_, String>(2)?,
                    "claimNumber": r.get::<_, Option<String>>(3)?,
                    "policyNumber": r.get::<_, Option<String>>(4)?,
                    "handlerName": r.get::<_, Option<String>>(5)?,
                    "handlerContact": r.get::<_, Option<String>>(6)?,
                    "status": r.get::<_, String>(7)?,
                    "notes": r.get::<_, Option<String>>(8)?,
                    "createdAt": r.get::<_, String>(9)?,
                    "updatedAt": r.get::<_, String>(10)?,
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
}

pub(crate) fn save_claim(db: &DbState, payload: &Value) -> AppResult<Value> {
    let matter_id = required_trimmed(payload, "matterId")?;
    let insurer_name = required_trimmed(payload, "insurerName")?;
    let status = optional_trimmed(payload, "status").unwrap_or("open");
    validate_one_of(status, CLAIM_STATUSES, "claim status")?;
    let claim_number = optional_trimmed(payload, "claimNumber");
    let policy_number = optional_trimmed(payload, "policyNumber");
    let handler_name = optional_trimmed(payload, "handlerName");
    let handler_contact = optional_trimmed(payload, "handlerContact");
    let notes = optional_trimmed(payload, "notes");
    let now = Utc::now().to_rfc3339();

    if let Some(id) = optional_trimmed(payload, "claimId") {
        db.write(|conn| {
            let changed = conn.execute(
                "UPDATE insurance_claims SET
                    insurer_name=?3,claim_number=?4,policy_number=?5,handler_name=?6,
                    handler_contact=?7,status=?8,notes=?9,updated_at=?10
                 WHERE id=?1 AND matter_id=?2",
                params![
                    id,
                    matter_id,
                    insurer_name,
                    claim_number,
                    policy_number,
                    handler_name,
                    handler_contact,
                    status,
                    notes,
                    now
                ],
            )?;
            if changed != 1 {
                return Err(AppError::Validation(
                    "insurance claim not editable in this matter".into(),
                ));
            }
            Ok(())
        })?;
        return Ok(json!({"id": id}));
    }

    let id = Uuid::new_v4().to_string();
    db.write(|conn| {
        conn.execute(
            "INSERT INTO insurance_claims(
                id,matter_id,insurer_name,claim_number,policy_number,handler_name,
                handler_contact,status,notes,created_at,updated_at
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10)",
            params![
                id,
                matter_id,
                insurer_name,
                claim_number,
                policy_number,
                handler_name,
                handler_contact,
                status,
                notes,
                now
            ],
        )?;
        Ok(())
    })?;
    Ok(json!({"id": id}))
}

pub(crate) fn list_events(db: &DbState, matter_id: &str) -> AppResult<Vec<Value>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id,matter_id,insurance_claim_id,event_kind,happened_at,summary,
                    follow_up_at,source_document_version_id,created_at
             FROM negotiation_events
             WHERE matter_id=?1
             ORDER BY happened_at DESC,created_at DESC,id",
        )?;
        let rows = stmt
            .query_map([matter_id], |r| {
                Ok(json!({
                    "id": r.get::<_, String>(0)?,
                    "matterId": r.get::<_, String>(1)?,
                    "insuranceClaimId": r.get::<_, Option<String>>(2)?,
                    "eventKind": r.get::<_, String>(3)?,
                    "happenedAt": r.get::<_, String>(4)?,
                    "summary": r.get::<_, String>(5)?,
                    "followUpAt": r.get::<_, Option<String>>(6)?,
                    "sourceDocumentVersionId": r.get::<_, Option<String>>(7)?,
                    "createdAt": r.get::<_, String>(8)?,
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
}

pub(crate) fn add_event(db: &DbState, payload: &Value) -> AppResult<Value> {
    let matter_id = required_trimmed(payload, "matterId")?;
    let event_kind = required_trimmed(payload, "eventKind")?;
    validate_one_of(event_kind, EVENT_KINDS, "event kind")?;
    let happened_at = required_trimmed(payload, "happenedAt")?;
    let summary = required_trimmed(payload, "summary")?;
    let insurance_claim_id = optional_trimmed(payload, "insuranceClaimId");
    let follow_up_at = optional_trimmed(payload, "followUpAt");
    let source_document_version_id = optional_trimmed(payload, "sourceDocumentVersionId");
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    db.write(|conn| {
        conn.execute(
            "INSERT INTO negotiation_events(
                id,matter_id,insurance_claim_id,event_kind,happened_at,summary,
                follow_up_at,source_document_version_id,created_at
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            params![
                id,
                matter_id,
                insurance_claim_id,
                event_kind,
                happened_at,
                summary,
                follow_up_at,
                source_document_version_id,
                now
            ],
        )?;
        Ok(())
    })?;
    Ok(json!({"id": id}))
}

pub(crate) fn list_positions(db: &DbState, matter_id: &str) -> AppResult<Vec<Value>> {
    db.read(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id,matter_id,insurance_claim_id,side,kind,amount_cents,currency,
                    recorded_at,notes,source_document_version_id,created_at
             FROM negotiation_positions
             WHERE matter_id=?1
             ORDER BY recorded_at DESC,created_at DESC,id",
        )?;
        let rows = stmt
            .query_map([matter_id], |r| {
                Ok(json!({
                    "id": r.get::<_, String>(0)?,
                    "matterId": r.get::<_, String>(1)?,
                    "insuranceClaimId": r.get::<_, Option<String>>(2)?,
                    "side": r.get::<_, String>(3)?,
                    "kind": r.get::<_, String>(4)?,
                    "amountCents": r.get::<_, i64>(5)?,
                    "currency": r.get::<_, String>(6)?,
                    "recordedAt": r.get::<_, String>(7)?,
                    "notes": r.get::<_, Option<String>>(8)?,
                    "sourceDocumentVersionId": r.get::<_, Option<String>>(9)?,
                    "createdAt": r.get::<_, String>(10)?,
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })
}

pub(crate) fn add_position(db: &DbState, payload: &Value) -> AppResult<Value> {
    let matter_id = required_trimmed(payload, "matterId")?;
    let side = required_trimmed(payload, "side")?;
    validate_one_of(side, POSITION_SIDES, "position side")?;
    let kind = required_trimmed(payload, "kind")?;
    validate_one_of(kind, POSITION_KINDS, "position kind")?;
    let amount_cents = payload
        .get("amountCents")
        .and_then(Value::as_i64)
        .filter(|v| *v >= 0)
        .ok_or_else(|| AppError::Validation("amountCents must be a non-negative integer".into()))?;
    let recorded_at = required_trimmed(payload, "recordedAt")?;
    let insurance_claim_id = optional_trimmed(payload, "insuranceClaimId");
    let currency = optional_trimmed(payload, "currency").unwrap_or("ILS");
    let notes = optional_trimmed(payload, "notes");
    let source_document_version_id = optional_trimmed(payload, "sourceDocumentVersionId");
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    db.write(|conn| {
        conn.execute(
            "INSERT INTO negotiation_positions(
                id,matter_id,insurance_claim_id,side,kind,amount_cents,currency,
                recorded_at,notes,source_document_version_id,created_at
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                id,
                matter_id,
                insurance_claim_id,
                side,
                kind,
                amount_cents,
                currency,
                recorded_at,
                notes,
                source_document_version_id,
                now
            ],
        )?;
        Ok(())
    })?;
    Ok(json!({"id": id}))
}

#[tauri::command]
pub fn list_insurance_claims(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id = required_trimmed(&payload, "matterId")?;
    Ok(Value::Array(list_claims(&state.db, matter_id)?))
}

#[tauri::command]
pub fn save_insurance_claim(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    save_claim(&state.db, &payload)
}

#[tauri::command]
pub fn list_negotiation_events(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id = required_trimmed(&payload, "matterId")?;
    Ok(Value::Array(list_events(&state.db, matter_id)?))
}

#[tauri::command]
pub fn add_negotiation_event(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    add_event(&state.db, &payload)
}

#[tauri::command]
pub fn list_negotiation_positions(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id = required_trimmed(&payload, "matterId")?;
    Ok(Value::Array(list_positions(&state.db, matter_id)?))
}

#[tauri::command]
pub fn add_negotiation_position(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    add_position(&state.db, &payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn add_matter(db: &DbState, title: &str) -> String {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO matters(id,title,matter_type,status,workflow_stage,created_at,updated_at)
                 VALUES(?1,?2,'traffic_accident','active','intake',?3,?3)",
                params![id, title, now],
            )?;
            Ok(())
        })
        .unwrap();
        id
    }

    #[test]
    fn claim_event_and_position_are_matter_isolated_and_history_is_append_only() {
        let root = std::env::temp_dir().join(format!("tahrir-b7-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let db = DbState::open(root.join("app.db")).unwrap();
        let matter_a = add_matter(&db, "A");
        let matter_b = add_matter(&db, "B");

        let claim = save_claim(
            &db,
            &json!({"matterId": matter_a, "insurerName": "Insurer A", "status": "negotiating"}),
        )
        .unwrap();
        let claim_id = claim["id"].as_str().unwrap().to_string();

        let event = add_event(
            &db,
            &json!({
                "matterId": matter_a,
                "insuranceClaimId": claim_id,
                "eventKind": "call",
                "happenedAt": "2026-08-27T10:00:00+03:00",
                "summary": "handler requested wage records",
                "followUpAt": "2026-09-03T10:00:00+03:00"
            }),
        )
        .unwrap();
        let event_id = event["id"].as_str().unwrap().to_string();

        let position = add_position(
            &db,
            &json!({
                "matterId": matter_a,
                "insuranceClaimId": claim_id,
                "side": "counterparty",
                "kind": "offer",
                "amountCents": 18000000,
                "recordedAt": "2026-08-27T10:05:00+03:00"
            }),
        )
        .unwrap();
        let position_id = position["id"].as_str().unwrap().to_string();

        assert_eq!(list_claims(&db, &matter_a).unwrap().len(), 1);
        assert_eq!(list_events(&db, &matter_a).unwrap().len(), 1);
        assert_eq!(list_positions(&db, &matter_a).unwrap().len(), 1);
        assert!(list_events(&db, &matter_b).unwrap().is_empty());

        let cross_matter = add_event(
            &db,
            &json!({
                "matterId": matter_b,
                "insuranceClaimId": claim_id,
                "eventKind": "email",
                "happenedAt": "2026-08-27T11:00:00+03:00",
                "summary": "must fail"
            }),
        );
        assert!(cross_matter.is_err());

        let update_event_result = db.write(|conn| {
            conn.execute(
                "UPDATE negotiation_events SET summary='rewritten' WHERE id=?1",
                [event_id.as_str()],
            )?;
            Ok(())
        });
        assert!(update_event_result.is_err());

        let delete_event_result = db.write(|conn| {
            conn.execute(
                "DELETE FROM negotiation_events WHERE id=?1",
                [event_id.as_str()],
            )?;
            Ok(())
        });
        assert!(delete_event_result.is_err());

        let update_position_result = db.write(|conn| {
            conn.execute(
                "UPDATE negotiation_positions SET amount_cents=1 WHERE id=?1",
                [position_id.as_str()],
            )?;
            Ok(())
        });
        assert!(update_position_result.is_err());

        let delete_position_result = db.write(|conn| {
            conn.execute(
                "DELETE FROM negotiation_positions WHERE id=?1",
                [position_id.as_str()],
            )?;
            Ok(())
        });
        assert!(delete_position_result.is_err());

        assert_eq!(list_events(&db, &matter_a).unwrap().len(), 1);
        assert_eq!(list_positions(&db, &matter_a).unwrap().len(), 1);

        drop(db);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_invalid_position_and_claim_enums() {
        assert!(validate_one_of("open", CLAIM_STATUSES, "claim status").is_ok());
        assert!(validate_one_of("auto_accept", CLAIM_STATUSES, "claim status").is_err());
        assert!(validate_one_of("counterparty", POSITION_SIDES, "position side").is_ok());
        assert!(validate_one_of("accept", POSITION_KINDS, "position kind").is_err());
    }
}
