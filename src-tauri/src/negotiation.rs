//! Negotiation & Insurance Workspace (Phase B, milestone B7).
//!
//! This module is deliberately operational and human-controlled. It records insurer
//! claim metadata, communication/follow-up events, and monetary positions. It never
//! recommends, accepts, rejects, or closes a settlement automatically.

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{json, Value};
use tauri::State;
use uuid::Uuid;

use crate::{
    db::DbState,
    error::{AppError, AppResult},
    AppState,
};

const CLAIM_STATUSES: &[&str] = &["open", "awaiting_response", "negotiating", "settled", "closed"];
const EVENT_KINDS: &[&str] = &["call", "email", "letter", "meeting", "request", "follow_up", "other"];
const POSITION_SIDES: &[&str] = &["our_side", "counterparty"];
const POSITION_KINDS: &[&str] = &["demand", "offer", "counter_offer"];
const B7_CURRENCY: &str = "ILS";

fn now_utc() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn normalize_datetime(value: &str, field: &str) -> AppResult<String> {
    let parsed = DateTime::parse_from_rfc3339(value.trim()).map_err(|_| {
        AppError::Validation(format!("{field} must be a valid RFC3339 timestamp"))
    })?;
    Ok(parsed
        .with_timezone(&Utc)
        .to_rfc3339_opts(SecondsFormat::Secs, true))
}

fn parse_utc(value: &str, field: &str) -> AppResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value).map(|dt| dt.with_timezone(&Utc)).map_err(|_| {
        AppError::Validation(format!("{field} must be a valid RFC3339 timestamp"))
    })
}

fn is_overdue_at(value: &str, as_of: DateTime<Utc>) -> AppResult<bool> {
    Ok(parse_utc(value, "followUpAt")? < as_of)
}

fn required_str<'a>(payload: &'a Value, key: &str) -> AppResult<&'a str> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| AppError::Validation(format!("{key} is required")))
}

fn optional_trimmed<'a>(payload: &'a Value, key: &str) -> Option<&'a str> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|v| !v.is_empty())
}

fn normalize_optional_datetime(payload: &Value, key: &str) -> AppResult<Option<String>> {
    optional_trimmed(payload, key)
        .map(|value| normalize_datetime(value, key))
        .transpose()
}

fn require_allowed(value: &str, allowed: &[&str], field: &str) -> AppResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(AppError::Validation(format!("invalid {field}")))
    }
}

fn ensure_matter(conn: &Connection, matter_id: &str) -> AppResult<()> {
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM matters WHERE id = ?1",
        params![matter_id],
        |row| row.get(0),
    )?;
    if exists == 1 {
        Ok(())
    } else {
        Err(AppError::NotFound("Matter not found".into()))
    }
}

fn validate_insurer_party(conn: &Connection, matter_id: &str, party_id: &str) -> AppResult<String> {
    let display_name: Option<String> = conn
        .query_row(
            "SELECT display_name FROM matter_parties WHERE id = ?1 AND matter_id = ?2 AND role = 'insurer'",
            params![party_id, matter_id],
            |row| row.get(0),
        )
        .optional()?;
    display_name.ok_or_else(|| {
        AppError::Validation("insurerPartyId must reference an insurer party in the same matter".into())
    })
}

fn ensure_claim_in_matter(conn: &Connection, matter_id: &str, claim_id: &str) -> AppResult<()> {
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM insurance_claims WHERE id = ?1 AND matter_id = ?2",
        params![claim_id, matter_id],
        |row| row.get(0),
    )?;
    if exists == 1 {
        Ok(())
    } else {
        Err(AppError::Validation("insuranceClaimId must belong to this matter".into()))
    }
}

fn validate_optional_claim(
    conn: &Connection,
    matter_id: &str,
    claim_id: Option<&str>,
) -> AppResult<Option<String>> {
    if let Some(claim_id) = claim_id {
        ensure_claim_in_matter(conn, matter_id, claim_id)?;
        Ok(Some(claim_id.to_string()))
    } else {
        Ok(None)
    }
}

fn validate_source_version(
    conn: &Connection,
    matter_id: &str,
    version_id: Option<&str>,
) -> AppResult<Option<String>> {
    if let Some(version_id) = version_id {
        let exists: i64 = conn.query_row(
            "SELECT COUNT(*) FROM document_versions WHERE id = ?1 AND matter_id = ?2",
            params![version_id, matter_id],
            |row| row.get(0),
        )?;
        if exists != 1 {
            return Err(AppError::Validation(
                "sourceDocumentVersionId must belong to this matter".into(),
            ));
        }
        Ok(Some(version_id.to_string()))
    } else {
        Ok(None)
    }
}

fn insurer_display_for_claim(conn: &Connection, matter_id: &str, claim_id: &str) -> AppResult<Option<String>> {
    conn.query_row(
        "SELECT p.display_name
         FROM insurance_claim_insurers i
         JOIN matter_parties p ON p.id = i.insurer_party_id AND p.matter_id = i.matter_id
         WHERE i.claim_id = ?1 AND i.matter_id = ?2",
        params![claim_id, matter_id],
        |row| row.get(0),
    )
    .optional()
    .map_err(AppError::Db)
}

fn history_id() -> String {
    Uuid::new_v4().to_string()
}

fn insert_status_history_in_tx(
    tx: &Connection,
    matter_id: &str,
    claim_id: &str,
    from_status: Option<&str>,
    to_status: &str,
    changed_at: &str,
    note: Option<&str>,
) -> AppResult<String> {
    let id = history_id();
    tx.execute(
        "INSERT INTO insurance_claim_status_history
           (id, matter_id, insurance_claim_id, from_status, to_status, changed_at, note, actor_kind, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'human', ?8)",
        params![id, matter_id, claim_id, from_status, to_status, changed_at, note, now_utc()],
    )?;
    Ok(id)
}

fn read_claim(conn: &Connection, matter_id: &str, claim_id: &str) -> AppResult<Value> {
    conn.query_row(
        "SELECT c.id,
                c.matter_id,
                i.insurer_party_id,
                p.display_name,
                i.insurer_name_snapshot,
                c.claim_number,
                c.policy_number,
                c.handler_name,
                c.handler_contact,
                c.status,
                c.notes,
                c.created_at,
                c.updated_at
         FROM insurance_claims c
         JOIN insurance_claim_insurers i ON i.claim_id = c.id AND i.matter_id = c.matter_id
         JOIN matter_parties p ON p.id = i.insurer_party_id AND p.matter_id = i.matter_id
         WHERE c.id = ?1 AND c.matter_id = ?2",
        params![claim_id, matter_id],
        |row| {
            let insurer_display_name: String = row.get(3)?;
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "matterId": row.get::<_, String>(1)?,
                "insurerPartyId": row.get::<_, String>(2)?,
                "insurerDisplayName": insurer_display_name,
                "insurerName": insurer_display_name,
                "insurerNameSnapshot": row.get::<_, String>(4)?,
                "claimNumber": row.get::<_, Option<String>>(5)?,
                "policyNumber": row.get::<_, Option<String>>(6)?,
                "handlerName": row.get::<_, Option<String>>(7)?,
                "handlerContact": row.get::<_, Option<String>>(8)?,
                "status": row.get::<_, String>(9)?,
                "notes": row.get::<_, Option<String>>(10)?,
                "createdAt": row.get::<_, String>(11)?,
                "updatedAt": row.get::<_, String>(12)?,
            }))
        },
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound("Insurance claim not found".into()))
}

pub(crate) fn list_claims(db: &DbState, matter_id: &str) -> AppResult<Vec<Value>> {
    db.read(|conn| {
        ensure_matter(conn, matter_id)?;
        let mut stmt = conn.prepare(
            "SELECT c.id,
                    c.matter_id,
                    i.insurer_party_id,
                    p.display_name,
                    i.insurer_name_snapshot,
                    c.claim_number,
                    c.policy_number,
                    c.handler_name,
                    c.handler_contact,
                    c.status,
                    c.notes,
                    c.created_at,
                    c.updated_at
             FROM insurance_claims c
             JOIN insurance_claim_insurers i ON i.claim_id = c.id AND i.matter_id = c.matter_id
             JOIN matter_parties p ON p.id = i.insurer_party_id AND p.matter_id = i.matter_id
             WHERE c.matter_id = ?1
             ORDER BY CASE c.status
                        WHEN 'open' THEN 0
                        WHEN 'awaiting_response' THEN 1
                        WHEN 'negotiating' THEN 2
                        WHEN 'settled' THEN 3
                        ELSE 4
                      END,
                      c.updated_at DESC,
                      c.id DESC",
        )?;
        let rows = stmt.query_map(params![matter_id], |row| {
            let insurer_display_name: String = row.get(3)?;
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "matterId": row.get::<_, String>(1)?,
                "insurerPartyId": row.get::<_, String>(2)?,
                "insurerDisplayName": insurer_display_name,
                "insurerName": insurer_display_name,
                "insurerNameSnapshot": row.get::<_, String>(4)?,
                "claimNumber": row.get::<_, Option<String>>(5)?,
                "policyNumber": row.get::<_, Option<String>>(6)?,
                "handlerName": row.get::<_, Option<String>>(7)?,
                "handlerContact": row.get::<_, Option<String>>(8)?,
                "status": row.get::<_, String>(9)?,
                "notes": row.get::<_, Option<String>>(10)?,
                "createdAt": row.get::<_, String>(11)?,
                "updatedAt": row.get::<_, String>(12)?,
            }))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::Db)
    })
}

pub(crate) fn save_claim(db: &DbState, payload: &Value) -> AppResult<Value> {
    let matter_id = required_str(payload, "matterId")?.to_string();
    let claim_id = optional_trimmed(payload, "claimId").map(str::to_string);
    let insurer_party_id = required_str(payload, "insurerPartyId")?.to_string();
    let claim_number = optional_trimmed(payload, "claimNumber").map(str::to_string);
    let policy_number = optional_trimmed(payload, "policyNumber").map(str::to_string);
    let handler_name = optional_trimmed(payload, "handlerName").map(str::to_string);
    let handler_contact = optional_trimmed(payload, "handlerContact").map(str::to_string);
    let notes = optional_trimmed(payload, "notes").map(str::to_string);
    let requested_status = optional_trimmed(payload, "status").map(str::to_string);
    if let Some(status) = requested_status.as_deref() {
        require_allowed(status, CLAIM_STATUSES, "claim status")?;
    }

    db.write(|conn| {
        ensure_matter(conn, &matter_id)?;
        let insurer_name = validate_insurer_party(conn, &matter_id, &insurer_party_id)?;
        let now = now_utc();
        let claim_id = claim_id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let existing: Option<String> = conn
            .query_row(
                "SELECT status FROM insurance_claims WHERE id = ?1 AND matter_id = ?2",
                params![claim_id, matter_id],
                |row| row.get(0),
            )
            .optional()?;

        let tx = conn.transaction()?;
        match existing.as_deref() {
            Some(current_status) => {
                if let Some(requested_status) = requested_status.as_deref() {
                    if requested_status != current_status {
                        return Err(AppError::Validation(
                            "claim status changes require the explicit human status transition command".into(),
                        ));
                    }
                }
                let changed = tx.execute(
                    "UPDATE insurance_claims
                     SET insurer_name = ?3,
                         claim_number = ?4,
                         policy_number = ?5,
                         handler_name = ?6,
                         handler_contact = ?7,
                         notes = ?8,
                         updated_at = ?9
                     WHERE id = ?1 AND matter_id = ?2",
                    params![
                        claim_id,
                        matter_id,
                        insurer_name,
                        claim_number,
                        policy_number,
                        handler_name,
                        handler_contact,
                        notes,
                        now
                    ],
                )?;
                if changed != 1 {
                    return Err(AppError::NotFound("Insurance claim not found".into()));
                }
                tx.execute(
                    "INSERT INTO insurance_claim_insurers
                       (claim_id, matter_id, insurer_party_id, insurer_name_snapshot, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?5)
                     ON CONFLICT(claim_id) DO UPDATE SET
                       insurer_party_id = excluded.insurer_party_id,
                       insurer_name_snapshot = excluded.insurer_name_snapshot,
                       updated_at = excluded.updated_at",
                    params![claim_id, matter_id, insurer_party_id, insurer_name, now],
                )?;
            }
            None => {
                if requested_status.as_deref().is_some_and(|status| status != "open") {
                    return Err(AppError::Validation(
                        "new insurance claims must start open; use explicit human status transition after creation".into(),
                    ));
                }
                tx.execute(
                    "INSERT INTO insurance_claims
                       (id, matter_id, insurer_name, claim_number, policy_number, handler_name, handler_contact, status, notes, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'open', ?8, ?9, ?9)",
                    params![
                        claim_id,
                        matter_id,
                        insurer_name,
                        claim_number,
                        policy_number,
                        handler_name,
                        handler_contact,
                        notes,
                        now
                    ],
                )?;
                tx.execute(
                    "INSERT INTO insurance_claim_insurers
                       (claim_id, matter_id, insurer_party_id, insurer_name_snapshot, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                    params![claim_id, matter_id, insurer_party_id, insurer_name, now],
                )?;
                insert_status_history_in_tx(&tx, &matter_id, &claim_id, None, "open", &now, None)?;
            }
        }
        tx.commit()?;
        read_claim(conn, &matter_id, &claim_id)
    })
}

pub(crate) fn change_claim_status(db: &DbState, payload: &Value) -> AppResult<Value> {
    let matter_id = required_str(payload, "matterId")?.to_string();
    let claim_id = required_str(payload, "claimId")?.to_string();
    let to_status = required_str(payload, "toStatus")?.to_string();
    require_allowed(&to_status, CLAIM_STATUSES, "claim status")?;
    if let Some(actor_kind) = optional_trimmed(payload, "actorKind") {
        if actor_kind != "human" {
            return Err(AppError::Validation("actorKind must be human for B7 status transitions".into()));
        }
    }
    let changed_at = normalize_optional_datetime(payload, "changedAt")?.unwrap_or_else(now_utc);
    let note = optional_trimmed(payload, "note").map(str::to_string);

    db.write(|conn| {
        ensure_matter(conn, &matter_id)?;
        let from_status: String = conn
            .query_row(
                "SELECT status FROM insurance_claims WHERE id = ?1 AND matter_id = ?2",
                params![claim_id, matter_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound("Insurance claim not found".into()))?;
        if from_status == to_status {
            return Err(AppError::Validation("claim status transition must change status".into()));
        }

        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE insurance_claims SET status = ?3, updated_at = ?4 WHERE id = ?1 AND matter_id = ?2",
            params![claim_id, matter_id, to_status, changed_at],
        )?;
        let history_id = insert_status_history_in_tx(
            &tx,
            &matter_id,
            &claim_id,
            Some(&from_status),
            &to_status,
            &changed_at,
            note.as_deref(),
        )?;
        tx.commit()?;
        Ok(json!({
            "id": claim_id,
            "matterId": matter_id,
            "fromStatus": from_status,
            "toStatus": to_status,
            "historyId": history_id,
            "changedAt": changed_at,
            "actorKind": "human"
        }))
    })
}

pub(crate) fn list_status_history(db: &DbState, matter_id: &str, claim_id: &str) -> AppResult<Vec<Value>> {
    db.read(|conn| {
        ensure_claim_in_matter(conn, matter_id, claim_id)?;
        let mut stmt = conn.prepare(
            "SELECT id, matter_id, insurance_claim_id, from_status, to_status, changed_at, note, actor_kind, created_at
             FROM insurance_claim_status_history
             WHERE matter_id = ?1 AND insurance_claim_id = ?2
             ORDER BY changed_at DESC, created_at DESC, id DESC",
        )?;
        let rows = stmt.query_map(params![matter_id, claim_id], |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "matterId": row.get::<_, String>(1)?,
                "insuranceClaimId": row.get::<_, String>(2)?,
                "fromStatus": row.get::<_, Option<String>>(3)?,
                "toStatus": row.get::<_, String>(4)?,
                "changedAt": row.get::<_, String>(5)?,
                "note": row.get::<_, Option<String>>(6)?,
                "actorKind": row.get::<_, String>(7)?,
                "createdAt": row.get::<_, String>(8)?,
            }))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::Db)
    })
}

fn add_waiting_for_for_event(
    tx: &Connection,
    matter_id: &str,
    event_id: &str,
    claim_id: Option<&str>,
    summary: &str,
    follow_up_at: &str,
    party_label: Option<&str>,
    item_label: Option<&str>,
    created_at: &str,
) -> AppResult<String> {
    let waiting_for_id = Uuid::new_v4().to_string();
    let party_label = if let Some(label) = party_label {
        label.to_string()
    } else if let Some(claim_id) = claim_id {
        insurer_display_for_claim(tx, matter_id, claim_id)?.unwrap_or_else(|| "Negotiation".into())
    } else {
        "Negotiation".into()
    };
    let item_label = item_label
        .map(str::to_string)
        .unwrap_or_else(|| format!("Negotiation follow-up: {}", summary.chars().take(80).collect::<String>()));

    tx.execute(
        "INSERT INTO waiting_for
           (id, matter_id, party_label, item_label, since_at, follow_up_at, status, source_ref)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'open', ?7)",
        params![
            waiting_for_id,
            matter_id,
            party_label,
            item_label,
            created_at,
            follow_up_at,
            format!("negotiation_event:{event_id}")
        ],
    )?;
    tx.execute(
        "INSERT INTO negotiation_waiting_links (event_id, matter_id, waiting_for_id, created_at)
         VALUES (?1, ?2, ?3, ?4)",
        params![event_id, matter_id, waiting_for_id, created_at],
    )?;
    Ok(waiting_for_id)
}

fn insert_event_in_tx(
    tx: &Connection,
    matter_id: &str,
    insurance_claim_id: Option<&str>,
    event_kind: &str,
    happened_at: &str,
    summary: &str,
    follow_up_at: Option<&str>,
    source_document_version_id: Option<&str>,
    created_at: &str,
) -> AppResult<String> {
    validate_optional_claim(tx, matter_id, insurance_claim_id)?;
    validate_source_version(tx, matter_id, source_document_version_id)?;
    let id = Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO negotiation_events
           (id, matter_id, insurance_claim_id, event_kind, happened_at, summary, follow_up_at, source_document_version_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id,
            matter_id,
            insurance_claim_id,
            event_kind,
            happened_at,
            summary,
            follow_up_at,
            source_document_version_id,
            created_at
        ],
    )?;
    Ok(id)
}

pub(crate) fn add_event(db: &DbState, payload: &Value) -> AppResult<Value> {
    let matter_id = required_str(payload, "matterId")?.to_string();
    let insurance_claim_id = optional_trimmed(payload, "insuranceClaimId").map(str::to_string);
    let event_kind = required_str(payload, "eventKind")?.to_string();
    require_allowed(&event_kind, EVENT_KINDS, "event kind")?;
    let happened_at = normalize_datetime(required_str(payload, "happenedAt")?, "happenedAt")?;
    let summary = required_str(payload, "summary")?.to_string();
    let follow_up_at = normalize_optional_datetime(payload, "followUpAt")?;
    let source_document_version_id = optional_trimmed(payload, "sourceDocumentVersionId").map(str::to_string);
    let follow_up_party_label = optional_trimmed(payload, "followUpPartyLabel").map(str::to_string);
    let follow_up_item_label = optional_trimmed(payload, "followUpItemLabel").map(str::to_string);

    db.write(|conn| {
        ensure_matter(conn, &matter_id)?;
        let tx = conn.transaction()?;
        let created_at = now_utc();
        let id = insert_event_in_tx(
            &tx,
            &matter_id,
            insurance_claim_id.as_deref(),
            &event_kind,
            &happened_at,
            &summary,
            follow_up_at.as_deref(),
            source_document_version_id.as_deref(),
            &created_at,
        )?;
        let waiting_for_id = if let Some(follow_up_at) = follow_up_at.as_deref() {
            Some(add_waiting_for_for_event(
                &tx,
                &matter_id,
                &id,
                insurance_claim_id.as_deref(),
                &summary,
                follow_up_at,
                follow_up_party_label.as_deref(),
                follow_up_item_label.as_deref(),
                &created_at,
            )?)
        } else {
            None
        };
        tx.commit()?;
        Ok(json!({ "id": id, "matterId": matter_id, "waitingForId": waiting_for_id }))
    })
}

pub(crate) fn list_events(db: &DbState, matter_id: &str) -> AppResult<Vec<Value>> {
    db.read(|conn| {
        ensure_matter(conn, matter_id)?;
        let mut stmt = conn.prepare(
            "SELECT e.id,
                    e.matter_id,
                    e.insurance_claim_id,
                    e.event_kind,
                    e.happened_at,
                    e.summary,
                    e.follow_up_at,
                    e.source_document_version_id,
                    e.created_at,
                    ec.replacement_event_id AS corrected_by_event_id,
                    rc.original_event_id AS corrects_event_id,
                    l.waiting_for_id,
                    w.status,
                    w.follow_up_at AS operational_follow_up_at,
                    d.logical_title
             FROM negotiation_events e
             LEFT JOIN negotiation_event_corrections ec ON ec.original_event_id = e.id AND ec.matter_id = e.matter_id
             LEFT JOIN negotiation_event_corrections rc ON rc.replacement_event_id = e.id AND rc.matter_id = e.matter_id
             LEFT JOIN negotiation_waiting_links l ON l.event_id = e.id AND l.matter_id = e.matter_id
             LEFT JOIN waiting_for w ON w.id = l.waiting_for_id AND w.matter_id = l.matter_id
             LEFT JOIN document_versions dv ON dv.id = e.source_document_version_id AND dv.matter_id = e.matter_id
             LEFT JOIN documents d ON d.id = dv.document_id AND d.matter_id = e.matter_id
             WHERE e.matter_id = ?1
             ORDER BY e.happened_at DESC, e.created_at DESC, e.id DESC",
        )?;
        let rows = stmt.query_map(params![matter_id], |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "matterId": row.get::<_, String>(1)?,
                "insuranceClaimId": row.get::<_, Option<String>>(2)?,
                "eventKind": row.get::<_, String>(3)?,
                "happenedAt": row.get::<_, String>(4)?,
                "summary": row.get::<_, String>(5)?,
                "followUpAt": row.get::<_, Option<String>>(6)?,
                "sourceDocumentVersionId": row.get::<_, Option<String>>(7)?,
                "createdAt": row.get::<_, String>(8)?,
                "correctedByEventId": row.get::<_, Option<String>>(9)?,
                "correctsEventId": row.get::<_, Option<String>>(10)?,
                "waitingForId": row.get::<_, Option<String>>(11)?,
                "followUpStatus": row.get::<_, Option<String>>(12)?,
                "operationalFollowUpAt": row.get::<_, Option<String>>(13)?,
                "sourceTitle": row.get::<_, Option<String>>(14)?,
            }))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::Db)
    })
}

fn ensure_event_correctable(conn: &Connection, matter_id: &str, event_id: &str) -> AppResult<()> {
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM negotiation_events WHERE id = ?1 AND matter_id = ?2",
        params![event_id, matter_id],
        |row| row.get(0),
    )?;
    if exists != 1 {
        return Err(AppError::Validation("originalEventId must belong to this matter".into()));
    }
    let linked: i64 = conn.query_row(
        "SELECT COUNT(*) FROM negotiation_event_corrections
         WHERE matter_id = ?1 AND (original_event_id = ?2 OR replacement_event_id = ?2)",
        params![matter_id, event_id],
        |row| row.get(0),
    )?;
    if linked == 0 {
        Ok(())
    } else {
        Err(AppError::Validation("event already participates in a correction".into()))
    }
}

pub(crate) fn correct_event(db: &DbState, payload: &Value) -> AppResult<Value> {
    let matter_id = required_str(payload, "matterId")?.to_string();
    let original_event_id = required_str(payload, "originalEventId")?.to_string();
    let event_kind = required_str(payload, "eventKind")?.to_string();
    require_allowed(&event_kind, EVENT_KINDS, "event kind")?;
    let happened_at = normalize_datetime(required_str(payload, "happenedAt")?, "happenedAt")?;
    let summary = required_str(payload, "summary")?.to_string();
    let insurance_claim_id = optional_trimmed(payload, "insuranceClaimId").map(str::to_string);
    let follow_up_at = normalize_optional_datetime(payload, "followUpAt")?;
    let source_document_version_id = optional_trimmed(payload, "sourceDocumentVersionId").map(str::to_string);
    let reason = optional_trimmed(payload, "reason").map(str::to_string);
    let follow_up_party_label = optional_trimmed(payload, "followUpPartyLabel").map(str::to_string);
    let follow_up_item_label = optional_trimmed(payload, "followUpItemLabel").map(str::to_string);

    db.write(|conn| {
        ensure_matter(conn, &matter_id)?;
        let tx = conn.transaction()?;
        ensure_event_correctable(&tx, &matter_id, &original_event_id)?;
        let created_at = now_utc();
        let replacement_event_id = insert_event_in_tx(
            &tx,
            &matter_id,
            insurance_claim_id.as_deref(),
            &event_kind,
            &happened_at,
            &summary,
            follow_up_at.as_deref(),
            source_document_version_id.as_deref(),
            &created_at,
        )?;
        tx.execute(
            "UPDATE waiting_for
             SET status = 'closed'
             WHERE matter_id = ?1
               AND status = 'open'
               AND id IN (
                 SELECT waiting_for_id FROM negotiation_waiting_links
                 WHERE matter_id = ?1 AND event_id = ?2
               )",
            params![matter_id, original_event_id],
        )?;
        let waiting_for_id = if let Some(follow_up_at) = follow_up_at.as_deref() {
            Some(add_waiting_for_for_event(
                &tx,
                &matter_id,
                &replacement_event_id,
                insurance_claim_id.as_deref(),
                &summary,
                follow_up_at,
                follow_up_party_label.as_deref(),
                follow_up_item_label.as_deref(),
                &created_at,
            )?)
        } else {
            None
        };
        let correction_id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO negotiation_event_corrections
               (id, matter_id, original_event_id, replacement_event_id, reason, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![correction_id, matter_id, original_event_id, replacement_event_id, reason, created_at],
        )?;
        tx.commit()?;
        Ok(json!({
            "id": correction_id,
            "matterId": matter_id,
            "originalEventId": original_event_id,
            "replacementEventId": replacement_event_id,
            "waitingForId": waiting_for_id,
        }))
    })
}

fn validate_amount(payload: &Value) -> AppResult<i64> {
    let amount = payload
        .get("amountCents")
        .and_then(Value::as_i64)
        .ok_or_else(|| AppError::Validation("amountCents must be an integer cent amount".into()))?;
    if amount < 0 {
        return Err(AppError::Validation("amountCents cannot be negative".into()));
    }
    Ok(amount)
}

fn validate_currency(payload: &Value) -> AppResult<String> {
    let currency = optional_trimmed(payload, "currency").unwrap_or(B7_CURRENCY);
    if currency == B7_CURRENCY {
        Ok(B7_CURRENCY.into())
    } else {
        Err(AppError::Validation("B7 MVP supports ILS currency only".into()))
    }
}

fn insert_position_in_tx(
    tx: &Connection,
    matter_id: &str,
    insurance_claim_id: Option<&str>,
    side: &str,
    kind: &str,
    amount_cents: i64,
    currency: &str,
    recorded_at: &str,
    notes: Option<&str>,
    source_document_version_id: Option<&str>,
    created_at: &str,
) -> AppResult<String> {
    validate_optional_claim(tx, matter_id, insurance_claim_id)?;
    validate_source_version(tx, matter_id, source_document_version_id)?;
    let id = Uuid::new_v4().to_string();
    tx.execute(
        "INSERT INTO negotiation_positions
           (id, matter_id, insurance_claim_id, side, kind, amount_cents, currency, recorded_at, notes, source_document_version_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
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
            created_at
        ],
    )?;
    Ok(id)
}

pub(crate) fn add_position(db: &DbState, payload: &Value) -> AppResult<Value> {
    let matter_id = required_str(payload, "matterId")?.to_string();
    let insurance_claim_id = optional_trimmed(payload, "insuranceClaimId").map(str::to_string);
    let side = required_str(payload, "side")?.to_string();
    let kind = required_str(payload, "kind")?.to_string();
    require_allowed(&side, POSITION_SIDES, "position side")?;
    require_allowed(&kind, POSITION_KINDS, "position kind")?;
    let amount_cents = validate_amount(payload)?;
    let currency = validate_currency(payload)?;
    let recorded_at = normalize_datetime(required_str(payload, "recordedAt")?, "recordedAt")?;
    let notes = optional_trimmed(payload, "notes").map(str::to_string);
    let source_document_version_id = optional_trimmed(payload, "sourceDocumentVersionId").map(str::to_string);

    db.write(|conn| {
        ensure_matter(conn, &matter_id)?;
        let tx = conn.transaction()?;
        let created_at = now_utc();
        let id = insert_position_in_tx(
            &tx,
            &matter_id,
            insurance_claim_id.as_deref(),
            &side,
            &kind,
            amount_cents,
            &currency,
            &recorded_at,
            notes.as_deref(),
            source_document_version_id.as_deref(),
            &created_at,
        )?;
        tx.commit()?;
        Ok(json!({ "id": id, "matterId": matter_id }))
    })
}

pub(crate) fn list_positions(db: &DbState, matter_id: &str) -> AppResult<Vec<Value>> {
    db.read(|conn| {
        ensure_matter(conn, matter_id)?;
        let mut stmt = conn.prepare(
            "SELECT p.id,
                    p.matter_id,
                    p.insurance_claim_id,
                    p.side,
                    p.kind,
                    p.amount_cents,
                    p.currency,
                    p.recorded_at,
                    p.notes,
                    p.source_document_version_id,
                    p.created_at,
                    pc.replacement_position_id AS corrected_by_position_id,
                    rc.original_position_id AS corrects_position_id,
                    d.logical_title
             FROM negotiation_positions p
             LEFT JOIN negotiation_position_corrections pc ON pc.original_position_id = p.id AND pc.matter_id = p.matter_id
             LEFT JOIN negotiation_position_corrections rc ON rc.replacement_position_id = p.id AND rc.matter_id = p.matter_id
             LEFT JOIN document_versions dv ON dv.id = p.source_document_version_id AND dv.matter_id = p.matter_id
             LEFT JOIN documents d ON d.id = dv.document_id AND d.matter_id = p.matter_id
             WHERE p.matter_id = ?1
             ORDER BY p.recorded_at DESC, p.created_at DESC, p.id DESC",
        )?;
        let rows = stmt.query_map(params![matter_id], |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "matterId": row.get::<_, String>(1)?,
                "insuranceClaimId": row.get::<_, Option<String>>(2)?,
                "side": row.get::<_, String>(3)?,
                "kind": row.get::<_, String>(4)?,
                "amountCents": row.get::<_, i64>(5)?,
                "currency": row.get::<_, String>(6)?,
                "recordedAt": row.get::<_, String>(7)?,
                "notes": row.get::<_, Option<String>>(8)?,
                "sourceDocumentVersionId": row.get::<_, Option<String>>(9)?,
                "createdAt": row.get::<_, String>(10)?,
                "correctedByPositionId": row.get::<_, Option<String>>(11)?,
                "correctsPositionId": row.get::<_, Option<String>>(12)?,
                "sourceTitle": row.get::<_, Option<String>>(13)?,
            }))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(AppError::Db)
    })
}

fn ensure_position_correctable(conn: &Connection, matter_id: &str, position_id: &str) -> AppResult<()> {
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM negotiation_positions WHERE id = ?1 AND matter_id = ?2",
        params![position_id, matter_id],
        |row| row.get(0),
    )?;
    if exists != 1 {
        return Err(AppError::Validation("originalPositionId must belong to this matter".into()));
    }
    let linked: i64 = conn.query_row(
        "SELECT COUNT(*) FROM negotiation_position_corrections
         WHERE matter_id = ?1 AND (original_position_id = ?2 OR replacement_position_id = ?2)",
        params![matter_id, position_id],
        |row| row.get(0),
    )?;
    if linked == 0 {
        Ok(())
    } else {
        Err(AppError::Validation("position already participates in a correction".into()))
    }
}

pub(crate) fn correct_position(db: &DbState, payload: &Value) -> AppResult<Value> {
    let matter_id = required_str(payload, "matterId")?.to_string();
    let original_position_id = required_str(payload, "originalPositionId")?.to_string();
    let insurance_claim_id = optional_trimmed(payload, "insuranceClaimId").map(str::to_string);
    let side = required_str(payload, "side")?.to_string();
    let kind = required_str(payload, "kind")?.to_string();
    require_allowed(&side, POSITION_SIDES, "position side")?;
    require_allowed(&kind, POSITION_KINDS, "position kind")?;
    let amount_cents = validate_amount(payload)?;
    let currency = validate_currency(payload)?;
    let recorded_at = normalize_datetime(required_str(payload, "recordedAt")?, "recordedAt")?;
    let notes = optional_trimmed(payload, "notes").map(str::to_string);
    let source_document_version_id = optional_trimmed(payload, "sourceDocumentVersionId").map(str::to_string);
    let reason = optional_trimmed(payload, "reason").map(str::to_string);

    db.write(|conn| {
        ensure_matter(conn, &matter_id)?;
        let tx = conn.transaction()?;
        ensure_position_correctable(&tx, &matter_id, &original_position_id)?;
        let created_at = now_utc();
        let replacement_position_id = insert_position_in_tx(
            &tx,
            &matter_id,
            insurance_claim_id.as_deref(),
            &side,
            &kind,
            amount_cents,
            &currency,
            &recorded_at,
            notes.as_deref(),
            source_document_version_id.as_deref(),
            &created_at,
        )?;
        let correction_id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO negotiation_position_corrections
               (id, matter_id, original_position_id, replacement_position_id, reason, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![correction_id, matter_id, original_position_id, replacement_position_id, reason, created_at],
        )?;
        tx.commit()?;
        Ok(json!({
            "id": correction_id,
            "matterId": matter_id,
            "originalPositionId": original_position_id,
            "replacementPositionId": replacement_position_id,
        }))
    })
}

fn active_claim(conn: &Connection, matter_id: &str) -> AppResult<Option<Value>> {
    conn.query_row(
        "SELECT c.id,
                i.insurer_party_id,
                p.display_name,
                c.claim_number,
                c.policy_number,
                c.handler_name,
                c.handler_contact,
                c.status
         FROM insurance_claims c
         JOIN insurance_claim_insurers i ON i.claim_id = c.id AND i.matter_id = c.matter_id
         JOIN matter_parties p ON p.id = i.insurer_party_id AND p.matter_id = i.matter_id
         WHERE c.matter_id = ?1
         ORDER BY CASE c.status
                    WHEN 'open' THEN 0
                    WHEN 'awaiting_response' THEN 0
                    WHEN 'negotiating' THEN 0
                    WHEN 'settled' THEN 1
                    ELSE 2
                  END,
                  c.updated_at DESC,
                  c.id DESC
         LIMIT 1",
        params![matter_id],
        |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "insurerPartyId": row.get::<_, String>(1)?,
                "insurerDisplayName": row.get::<_, String>(2)?,
                "claimNumber": row.get::<_, Option<String>>(3)?,
                "policyNumber": row.get::<_, Option<String>>(4)?,
                "handlerName": row.get::<_, Option<String>>(5)?,
                "handlerContact": row.get::<_, Option<String>>(6)?,
                "status": row.get::<_, String>(7)?,
            }))
        },
    )
    .optional()
    .map_err(AppError::Db)
}

fn latest_position(conn: &Connection, matter_id: &str, our_side: bool) -> AppResult<Option<Value>> {
    let sql = if our_side {
        "SELECT p.id, p.side, p.kind, p.amount_cents, p.currency, p.recorded_at
         FROM negotiation_positions p
         WHERE p.matter_id = ?1
           AND p.side = 'our_side'
           AND p.kind IN ('demand','counter_offer')
           AND NOT EXISTS (
             SELECT 1 FROM negotiation_position_corrections c
             WHERE c.matter_id = p.matter_id AND c.original_position_id = p.id
           )
         ORDER BY p.recorded_at DESC, p.created_at DESC, p.id DESC
         LIMIT 1"
    } else {
        "SELECT p.id, p.side, p.kind, p.amount_cents, p.currency, p.recorded_at
         FROM negotiation_positions p
         WHERE p.matter_id = ?1
           AND p.side = 'counterparty'
           AND p.kind IN ('offer','counter_offer')
           AND NOT EXISTS (
             SELECT 1 FROM negotiation_position_corrections c
             WHERE c.matter_id = p.matter_id AND c.original_position_id = p.id
           )
         ORDER BY p.recorded_at DESC, p.created_at DESC, p.id DESC
         LIMIT 1"
    };
    conn.query_row(sql, params![matter_id], |row| {
        Ok(json!({
            "id": row.get::<_, String>(0)?,
            "side": row.get::<_, String>(1)?,
            "kind": row.get::<_, String>(2)?,
            "amountCents": row.get::<_, i64>(3)?,
            "currency": row.get::<_, String>(4)?,
            "recordedAt": row.get::<_, String>(5)?,
        }))
    })
    .optional()
    .map_err(AppError::Db)
}

fn latest_interaction(conn: &Connection, matter_id: &str) -> AppResult<Option<Value>> {
    conn.query_row(
        "SELECT e.id, e.event_kind, e.happened_at, e.summary
         FROM negotiation_events e
         WHERE e.matter_id = ?1
           AND NOT EXISTS (
             SELECT 1 FROM negotiation_event_corrections c
             WHERE c.matter_id = e.matter_id AND c.original_event_id = e.id
           )
         ORDER BY e.happened_at DESC, e.created_at DESC, e.id DESC
         LIMIT 1",
        params![matter_id],
        |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "eventKind": row.get::<_, String>(1)?,
                "happenedAt": row.get::<_, String>(2)?,
                "summary": row.get::<_, String>(3)?,
            }))
        },
    )
    .optional()
    .map_err(AppError::Db)
}

fn next_follow_up(
    conn: &Connection,
    matter_id: &str,
    as_of: DateTime<Utc>,
) -> AppResult<Option<Value>> {
    let row: Option<(String, String, String, String, String)> = conn
        .query_row(
            "SELECT w.id, w.follow_up_at, w.party_label, w.item_label, l.event_id
             FROM negotiation_waiting_links l
             JOIN waiting_for w ON w.id = l.waiting_for_id AND w.matter_id = l.matter_id
             WHERE l.matter_id = ?1 AND w.status = 'open' AND w.follow_up_at IS NOT NULL
             ORDER BY w.follow_up_at ASC, w.id ASC
             LIMIT 1",
            params![matter_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()?;
    row.map(|(id, follow_up_at, party_label, item_label, event_id)| {
        Ok(json!({
            "waitingForId": id,
            "eventId": event_id,
            "followUpAt": follow_up_at,
            "overdue": is_overdue_at(&follow_up_at, as_of)?,
            "partyLabel": party_label,
            "itemLabel": item_label,
        }))
    })
    .transpose()
}

fn snapshot_from_conn(conn: &Connection, matter_id: &str, as_of: DateTime<Utc>) -> AppResult<Value> {
    ensure_matter(conn, matter_id)?;
    let current_claim = active_claim(conn, matter_id)?;
    let latest_our_demand = latest_position(conn, matter_id, true)?;
    let latest_counterparty_offer = latest_position(conn, matter_id, false)?;
    let gap = match (&latest_our_demand, &latest_counterparty_offer) {
        (Some(demand), Some(offer)) => {
            let demand_currency = demand.get("currency").and_then(Value::as_str);
            let offer_currency = offer.get("currency").and_then(Value::as_str);
            match (demand_currency, offer_currency) {
                (Some(left), Some(right)) if left == right => {
                    let demand_amount = demand.get("amountCents").and_then(Value::as_i64).ok_or_else(|| {
                        AppError::Validation("latest demand amount is malformed".into())
                    })?;
                    let offer_amount = offer.get("amountCents").and_then(Value::as_i64).ok_or_else(|| {
                        AppError::Validation("latest offer amount is malformed".into())
                    })?;
                    Some(json!({ "amountCents": demand_amount - offer_amount, "currency": left }))
                }
                _ => None,
            }
        }
        _ => None,
    };
    let negotiation_status = current_claim
        .as_ref()
        .and_then(|claim| claim.get("status"))
        .cloned()
        .unwrap_or(Value::Null);

    Ok(json!({
        "matterId": matter_id,
        "currentClaim": current_claim,
        "latestOurDemand": latest_our_demand,
        "latestCounterpartyOffer": latest_counterparty_offer,
        "gap": gap,
        "latestInteraction": latest_interaction(conn, matter_id)?,
        "nextFollowUp": next_follow_up(conn, matter_id, as_of)?,
        "negotiationStatus": negotiation_status,
    }))
}

fn snapshot_for_now(db: &DbState, matter_id: &str, as_of: DateTime<Utc>) -> AppResult<Value> {
    db.read(|conn| snapshot_from_conn(conn, matter_id, as_of))
}

pub(crate) fn snapshot(db: &DbState, matter_id: &str) -> AppResult<Value> {
    snapshot_for_now(db, matter_id, Utc::now())
}

#[tauri::command]
pub fn list_insurance_claims(state: State<'_, AppState>, matter_id: String) -> AppResult<Vec<Value>> {
    list_claims(&state.db, &matter_id)
}

#[tauri::command]
pub fn save_insurance_claim(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    save_claim(&state.db, &payload)
}

#[tauri::command]
pub fn change_insurance_claim_status(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    change_claim_status(&state.db, &payload)
}

#[tauri::command]
pub fn list_insurance_claim_status_history(
    state: State<'_, AppState>,
    matter_id: String,
    claim_id: String,
) -> AppResult<Vec<Value>> {
    list_status_history(&state.db, &matter_id, &claim_id)
}

#[tauri::command]
pub fn list_negotiation_events(state: State<'_, AppState>, matter_id: String) -> AppResult<Vec<Value>> {
    list_events(&state.db, &matter_id)
}

#[tauri::command]
pub fn add_negotiation_event(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    add_event(&state.db, &payload)
}

#[tauri::command]
pub fn correct_negotiation_event(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    correct_event(&state.db, &payload)
}

#[tauri::command]
pub fn list_negotiation_positions(state: State<'_, AppState>, matter_id: String) -> AppResult<Vec<Value>> {
    list_positions(&state.db, &matter_id)
}

#[tauri::command]
pub fn add_negotiation_position(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    add_position(&state.db, &payload)
}

#[tauri::command]
pub fn correct_negotiation_position(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    correct_position(&state.db, &payload)
}

#[tauri::command]
pub fn get_negotiation_snapshot(state: State<'_, AppState>, matter_id: String) -> AppResult<Value> {
    snapshot(&state.db, &matter_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger;
    use tempfile::TempDir;

    fn temp_db() -> (TempDir, DbState) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.sqlite3");
        let db = DbState::open(db_path).unwrap();
        (dir, db)
    }

    fn add_matter(db: &DbState, title: &str) -> String {
        let matter_id = Uuid::new_v4().to_string();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO matters(id,title,matter_type,status,workflow_stage,created_at,updated_at)
                 VALUES(?1,?2,'generic_civil','active','intake',?3,?3)",
                params![matter_id, title, now_utc()],
            )?;
            Ok(())
        })
        .unwrap();
        matter_id
    }

    fn add_party(db: &DbState, matter_id: &str, role: &str, display_name: &str) -> String {
        let id = Uuid::new_v4().to_string();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO matter_parties
                   (id, matter_id, role, entity_kind, display_name, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'organization', ?4, ?5, ?5)",
                params![id, matter_id, role, display_name, now_utc()],
            )?;
            Ok(())
        })
        .unwrap();
        id
    }

    fn add_source_version(db: &DbState, matter_id: &str, title: &str) -> String {
        let document_id = Uuid::new_v4().to_string();
        let version_id = Uuid::new_v4().to_string();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO documents(id, matter_id, logical_title, category, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 'correspondence', ?4, ?4)",
                params![document_id, matter_id, title, now_utc()],
            )?;
            conn.execute(
                "INSERT INTO document_versions
                   (id, document_id, matter_id, content_sha256, extraction_state, stale, created_at)
                 VALUES (?1, ?2, ?3, ?4, 'complete', 0, ?5)",
                params![version_id, document_id, matter_id, Uuid::new_v4().to_string(), now_utc()],
            )?;
            Ok(())
        })
        .unwrap();
        version_id
    }

    fn create_claim(db: &DbState, matter_id: &str, insurer_party_id: &str) -> String {
        save_claim(
            db,
            &json!({
                "matterId": matter_id,
                "insurerPartyId": insurer_party_id,
                "claimNumber": "CLM-1",
                "handlerName": "Handler"
            }),
        )
        .unwrap()
        .get("id")
        .and_then(Value::as_str)
        .unwrap()
        .to_string()
    }

    fn count_rows(db: &DbState, table: &str, matter_id: &str) -> i64 {
        db.read(|conn| {
            let sql = format!("SELECT COUNT(*) FROM {table} WHERE matter_id = ?1");
            conn.query_row(&sql, params![matter_id], |row| row.get(0)).map_err(AppError::Db)
        })
        .unwrap()
    }

    #[test]
    fn claims_require_canonical_insurer_party_and_record_status_history() {
        let (_dir, db) = temp_db();
        let matter = add_matter(&db, "Matter A");
        let other_matter = add_matter(&db, "Matter B");
        let insurer = add_party(&db, &matter, "insurer", "Insurer Ltd");
        let non_insurer = add_party(&db, &matter, "opposing_party", "Driver");
        let cross_matter_insurer = add_party(&db, &other_matter, "insurer", "Other Insurer");

        let claim = save_claim(
            &db,
            &json!({
                "matterId": matter,
                "insurerPartyId": insurer,
                "claimNumber": "123",
                "policyNumber": "P-7"
            }),
        )
        .unwrap();
        let claim_id = claim["id"].as_str().unwrap().to_string();
        assert_eq!(claim["insurerPartyId"], insurer);
        assert_eq!(claim["insurerDisplayName"], "Insurer Ltd");
        assert_eq!(claim["insurerNameSnapshot"], "Insurer Ltd");
        assert_eq!(claim["status"], "open");

        let history = list_status_history(&db, &matter, &claim_id).unwrap();
        assert_eq!(history.len(), 1);
        assert!(history[0]["fromStatus"].is_null());
        assert_eq!(history[0]["toStatus"], "open");
        assert_eq!(history[0]["actorKind"], "human");

        assert!(save_claim(
            &db,
            &json!({ "matterId": matter, "insurerPartyId": non_insurer })
        )
        .is_err());
        assert!(save_claim(
            &db,
            &json!({ "matterId": matter, "insurerPartyId": cross_matter_insurer })
        )
        .is_err());
        assert!(save_claim(
            &db,
            &json!({ "matterId": matter, "insurerPartyId": insurer, "status": "settled" })
        )
        .is_err());
    }

    #[test]
    fn explicit_status_transition_is_human_audited_and_offers_do_not_settle() {
        let (_dir, db) = temp_db();
        let matter = add_matter(&db, "Matter A");
        let insurer = add_party(&db, &matter, "insurer", "Insurer Ltd");
        let claim_id = create_claim(&db, &matter, &insurer);

        add_position(
            &db,
            &json!({
                "matterId": matter,
                "insuranceClaimId": claim_id,
                "side": "counterparty",
                "kind": "offer",
                "amountCents": 2500000,
                "currency": "ILS",
                "recordedAt": "2026-08-27T10:00:00+03:00"
            }),
        )
        .unwrap();
        let claim = list_claims(&db, &matter).unwrap().remove(0);
        assert_eq!(claim["status"], "open");

        assert!(save_claim(
            &db,
            &json!({
                "matterId": matter,
                "claimId": claim_id,
                "insurerPartyId": insurer,
                "status": "closed"
            })
        )
        .is_err());
        assert!(change_claim_status(
            &db,
            &json!({
                "matterId": matter,
                "claimId": claim_id,
                "toStatus": "automatic"
            })
        )
        .is_err());
        assert!(change_claim_status(
            &db,
            &json!({
                "matterId": matter,
                "claimId": claim_id,
                "toStatus": "settled",
                "actorKind": "system"
            })
        )
        .is_err());

        let transition = change_claim_status(
            &db,
            &json!({
                "matterId": matter,
                "claimId": claim_id,
                "toStatus": "settled",
                "changedAt": "2026-08-27T15:30:00+03:00",
                "note": "Lawyer confirmed settlement state"
            }),
        )
        .unwrap();
        assert_eq!(transition["actorKind"], "human");
        assert_eq!(transition["changedAt"], "2026-08-27T12:30:00Z");
        let history = list_status_history(&db, &matter, &claim_id).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0]["fromStatus"], "open");
        assert_eq!(history[0]["toStatus"], "settled");
    }

    #[test]
    fn events_create_waiting_for_lifecycle_and_validate_sources_and_timestamps() {
        let (_dir, db) = temp_db();
        let matter = add_matter(&db, "Matter A");
        let other_matter = add_matter(&db, "Matter B");
        let insurer = add_party(&db, &matter, "insurer", "Insurer Ltd");
        let claim_id = create_claim(&db, &matter, &insurer);
        let source = add_source_version(&db, &matter, "Demand letter");
        let cross_source = add_source_version(&db, &other_matter, "Other matter letter");

        let event = add_event(
            &db,
            &json!({
                "matterId": matter,
                "insuranceClaimId": claim_id,
                "eventKind": "email",
                "happenedAt": "2026-08-27T10:00:00+03:00",
                "summary": "Insurer requested updated records",
                "followUpAt": "2026-08-30T09:00:00+03:00",
                "sourceDocumentVersionId": source
            }),
        )
        .unwrap();
        let waiting_id = event["waitingForId"].as_str().unwrap().to_string();
        let events = list_events(&db, &matter).unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["happenedAt"], "2026-08-27T07:00:00Z");
        assert_eq!(events[0]["followUpAt"], "2026-08-30T06:00:00Z");
        assert_eq!(events[0]["operationalFollowUpAt"], "2026-08-30T06:00:00Z");
        assert_eq!(events[0]["sourceDocumentVersionId"], source);
        assert_eq!(events[0]["sourceTitle"], "Demand letter");

        let waiting_row: (String, String) = db
            .read(|conn| {
                conn.query_row(
                    "SELECT status, source_ref FROM waiting_for WHERE id = ?1 AND matter_id = ?2",
                    params![waiting_id, matter],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(AppError::Db)
            })
            .unwrap();
        assert_eq!(waiting_row.0, "open");
        assert!(waiting_row.1.starts_with("negotiation_event:"));

        db.write(|conn| {
            conn.execute(
                "UPDATE waiting_for SET status = 'closed' WHERE id = ?1 AND matter_id = ?2",
                params![waiting_id, matter],
            )?;
            Ok(())
        })
        .unwrap();
        let snapshot = snapshot_for_now(&db, &matter, parse_utc("2026-09-01T00:00:00Z", "asOf").unwrap()).unwrap();
        assert!(snapshot["nextFollowUp"].is_null());

        assert!(add_event(
            &db,
            &json!({
                "matterId": matter,
                "eventKind": "call",
                "happenedAt": "not-a-date",
                "summary": "Bad timestamp"
            })
        )
        .is_err());
        assert!(add_event(
            &db,
            &json!({
                "matterId": matter,
                "eventKind": "email",
                "happenedAt": "2026-08-27T10:00:00Z",
                "summary": "Cross source",
                "sourceDocumentVersionId": cross_source
            })
        )
        .is_err());
        assert!(add_event(
            &db,
            &json!({
                "matterId": matter,
                "eventKind": "invalid",
                "happenedAt": "2026-08-27T10:00:00Z",
                "summary": "Invalid kind"
            })
        )
        .is_err());
    }

    #[test]
    fn positions_validate_money_currency_sources_and_gap_deterministically() {
        let (_dir, db) = temp_db();
        let matter = add_matter(&db, "Matter A");
        let other_matter = add_matter(&db, "Matter B");
        let insurer = add_party(&db, &matter, "insurer", "Insurer Ltd");
        let other_insurer = add_party(&db, &other_matter, "insurer", "Other Insurer");
        let claim_id = create_claim(&db, &matter, &insurer);
        let other_claim_id = create_claim(&db, &other_matter, &other_insurer);
        let source = add_source_version(&db, &matter, "Offer PDF");
        let cross_source = add_source_version(&db, &other_matter, "Other offer PDF");

        add_position(
            &db,
            &json!({
                "matterId": matter,
                "insuranceClaimId": claim_id,
                "side": "our_side",
                "kind": "demand",
                "amountCents": 12345,
                "currency": "ILS",
                "recordedAt": "2026-08-26T09:00:00+03:00",
                "sourceDocumentVersionId": source
            }),
        )
        .unwrap();
        let positions = list_positions(&db, &matter).unwrap();
        assert_eq!(positions[0]["amountCents"], 12345);
        assert_eq!(positions[0]["currency"], "ILS");
        assert_eq!(positions[0]["recordedAt"], "2026-08-26T06:00:00Z");

        assert!(add_position(
            &db,
            &json!({
                "matterId": matter,
                "side": "our_side",
                "kind": "demand",
                "amountCents": -1,
                "currency": "ILS",
                "recordedAt": "2026-08-27T10:00:00Z"
            })
        )
        .is_err());
        assert!(add_position(
            &db,
            &json!({
                "matterId": matter,
                "side": "our_side",
                "kind": "demand",
                "amountCents": 10,
                "currency": "USD",
                "recordedAt": "2026-08-27T10:00:00Z"
            })
        )
        .is_err());
        assert!(add_position(
            &db,
            &json!({
                "matterId": matter,
                "insuranceClaimId": other_claim_id,
                "side": "our_side",
                "kind": "demand",
                "amountCents": 10,
                "currency": "ILS",
                "recordedAt": "2026-08-27T10:00:00Z"
            })
        )
        .is_err());
        assert!(add_position(
            &db,
            &json!({
                "matterId": matter,
                "insuranceClaimId": claim_id,
                "side": "counterparty",
                "kind": "offer",
                "amountCents": 10,
                "currency": "ILS",
                "recordedAt": "2026-08-27T10:00:00Z",
                "sourceDocumentVersionId": cross_source
            })
        )
        .is_err());

        add_position(
            &db,
            &json!({
                "matterId": matter,
                "insuranceClaimId": claim_id,
                "side": "our_side",
                "kind": "demand",
                "amountCents": 1000000,
                "currency": "ILS",
                "recordedAt": "2026-08-28T10:00:00Z"
            }),
        )
        .unwrap();
        add_position(
            &db,
            &json!({
                "matterId": matter,
                "insuranceClaimId": claim_id,
                "side": "counterparty",
                "kind": "offer",
                "amountCents": 750000,
                "currency": "ILS",
                "recordedAt": "2026-08-28T11:00:00Z"
            }),
        )
        .unwrap();
        let snapshot = snapshot_for_now(&db, &matter, parse_utc("2026-08-29T00:00:00Z", "asOf").unwrap()).unwrap();
        assert_eq!(snapshot["latestOurDemand"]["amountCents"], 1000000);
        assert_eq!(snapshot["latestCounterpartyOffer"]["amountCents"], 750000);
        assert_eq!(snapshot["gap"]["amountCents"], 250000);
        assert_eq!(snapshot["gap"]["currency"], "ILS");
    }

    #[test]
    fn corrections_keep_originals_and_drive_effective_snapshot() {
        let (_dir, db) = temp_db();
        let matter = add_matter(&db, "Matter A");
        let other_matter = add_matter(&db, "Matter B");
        let insurer = add_party(&db, &matter, "insurer", "Insurer Ltd");
        let claim_id = create_claim(&db, &matter, &insurer);
        let original_event = add_event(
            &db,
            &json!({
                "matterId": matter,
                "insuranceClaimId": claim_id,
                "eventKind": "email",
                "happenedAt": "2026-08-27T08:00:00Z",
                "summary": "Original summary",
                "followUpAt": "2026-08-28T08:00:00Z"
            }),
        )
        .unwrap();
        let original_event_id = original_event["id"].as_str().unwrap().to_string();
        let original_waiting_id = original_event["waitingForId"].as_str().unwrap().to_string();
        let correction = correct_event(
            &db,
            &json!({
                "matterId": matter,
                "originalEventId": original_event_id,
                "insuranceClaimId": claim_id,
                "eventKind": "email",
                "happenedAt": "2026-08-27T09:00:00Z",
                "summary": "Corrected summary",
                "reason": "Wrong time"
            }),
        )
        .unwrap();
        let replacement_event_id = correction["replacementEventId"].as_str().unwrap().to_string();
        assert!(correct_event(
            &db,
            &json!({
                "matterId": matter,
                "originalEventId": original_event_id,
                "eventKind": "email",
                "happenedAt": "2026-08-27T10:00:00Z",
                "summary": "Duplicate correction"
            })
        )
        .is_err());
        assert!(correct_event(
            &db,
            &json!({
                "matterId": other_matter,
                "originalEventId": replacement_event_id,
                "eventKind": "email",
                "happenedAt": "2026-08-27T10:00:00Z",
                "summary": "Cross matter"
            })
        )
        .is_err());
        let events = list_events(&db, &matter).unwrap();
        assert_eq!(events.len(), 2);
        assert!(events.iter().any(|event| event["correctedByEventId"] == replacement_event_id));
        assert!(events.iter().any(|event| event["correctsEventId"] == original_event_id));
        let original_waiting_status: String = db
            .read(|conn| {
                conn.query_row(
                    "SELECT status FROM waiting_for WHERE id = ?1 AND matter_id = ?2",
                    params![original_waiting_id, matter],
                    |row| row.get(0),
                )
                .map_err(AppError::Db)
            })
            .unwrap();
        assert_eq!(original_waiting_status, "closed");

        let original_position = add_position(
            &db,
            &json!({
                "matterId": matter,
                "insuranceClaimId": claim_id,
                "side": "our_side",
                "kind": "demand",
                "amountCents": 2000000,
                "currency": "ILS",
                "recordedAt": "2026-08-30T10:00:00Z"
            }),
        )
        .unwrap();
        let original_position_id = original_position["id"].as_str().unwrap().to_string();
        let position_correction = correct_position(
            &db,
            &json!({
                "matterId": matter,
                "originalPositionId": original_position_id,
                "insuranceClaimId": claim_id,
                "side": "our_side",
                "kind": "demand",
                "amountCents": 700000,
                "currency": "ILS",
                "recordedAt": "2026-08-29T10:00:00Z",
                "reason": "Corrected amount"
            }),
        )
        .unwrap();
        let replacement_position_id = position_correction["replacementPositionId"].as_str().unwrap().to_string();
        assert!(correct_position(
            &db,
            &json!({
                "matterId": matter,
                "originalPositionId": original_position_id,
                "side": "our_side",
                "kind": "demand",
                "amountCents": 100,
                "currency": "ILS",
                "recordedAt": "2026-08-29T12:00:00Z"
            })
        )
        .is_err());
        let positions = list_positions(&db, &matter).unwrap();
        assert!(positions.iter().any(|position| position["correctedByPositionId"] == replacement_position_id));
        assert!(positions.iter().any(|position| position["correctsPositionId"] == original_position_id));
        let snapshot = snapshot_for_now(&db, &matter, parse_utc("2026-09-01T00:00:00Z", "asOf").unwrap()).unwrap();
        assert_eq!(snapshot["latestOurDemand"]["id"], replacement_position_id);
        assert_eq!(snapshot["latestOurDemand"]["amountCents"], 700000);
    }

    #[test]
    fn append_only_history_and_guarded_whole_matter_delete() {
        let (_dir, db) = temp_db();
        let matter = add_matter(&db, "Matter A");
        let insurer = add_party(&db, &matter, "insurer", "Insurer Ltd");
        let claim_id = create_claim(&db, &matter, &insurer);
        change_claim_status(
            &db,
            &json!({
                "matterId": matter,
                "claimId": claim_id,
                "toStatus": "negotiating",
                "changedAt": "2026-08-27T10:00:00Z"
            }),
        )
        .unwrap();
        let event = add_event(
            &db,
            &json!({
                "matterId": matter,
                "insuranceClaimId": claim_id,
                "eventKind": "call",
                "happenedAt": "2026-08-27T10:00:00Z",
                "summary": "Call",
                "followUpAt": "2026-08-28T10:00:00Z"
            }),
        )
        .unwrap();
        let event_id = event["id"].as_str().unwrap().to_string();
        let event_correction = correct_event(
            &db,
            &json!({
                "matterId": matter,
                "originalEventId": event_id,
                "insuranceClaimId": claim_id,
                "eventKind": "call",
                "happenedAt": "2026-08-27T11:00:00Z",
                "summary": "Corrected call"
            }),
        )
        .unwrap();
        let event_correction_id = event_correction["id"].as_str().unwrap().to_string();
        let position = add_position(
            &db,
            &json!({
                "matterId": matter,
                "insuranceClaimId": claim_id,
                "side": "counterparty",
                "kind": "offer",
                "amountCents": 400000,
                "currency": "ILS",
                "recordedAt": "2026-08-27T10:00:00Z"
            }),
        )
        .unwrap();
        let position_id = position["id"].as_str().unwrap().to_string();
        let position_correction = correct_position(
            &db,
            &json!({
                "matterId": matter,
                "originalPositionId": position_id,
                "insuranceClaimId": claim_id,
                "side": "counterparty",
                "kind": "offer",
                "amountCents": 450000,
                "currency": "ILS",
                "recordedAt": "2026-08-27T11:00:00Z"
            }),
        )
        .unwrap();
        let position_correction_id = position_correction["id"].as_str().unwrap().to_string();
        let history = list_status_history(&db, &matter, &claim_id).unwrap();
        let history_id = history[0]["id"].as_str().unwrap().to_string();

        db.write(|conn| {
            assert!(conn.execute("UPDATE negotiation_events SET summary = 'x' WHERE id = ?1", params![event_id]).is_err());
            assert!(conn.execute("DELETE FROM negotiation_events WHERE id = ?1", params![event_id]).is_err());
            assert!(conn.execute("UPDATE negotiation_positions SET notes = 'x' WHERE id = ?1", params![position_id]).is_err());
            assert!(conn.execute("DELETE FROM negotiation_positions WHERE id = ?1", params![position_id]).is_err());
            assert!(conn.execute("UPDATE insurance_claim_status_history SET note = 'x' WHERE id = ?1", params![history_id]).is_err());
            assert!(conn.execute("DELETE FROM insurance_claim_status_history WHERE id = ?1", params![history_id]).is_err());
            assert!(conn.execute("UPDATE negotiation_event_corrections SET reason = 'x' WHERE id = ?1", params![event_correction_id]).is_err());
            assert!(conn.execute("DELETE FROM negotiation_event_corrections WHERE id = ?1", params![event_correction_id]).is_err());
            assert!(conn.execute("UPDATE negotiation_position_corrections SET reason = 'x' WHERE id = ?1", params![position_correction_id]).is_err());
            assert!(conn.execute("DELETE FROM negotiation_position_corrections WHERE id = ?1", params![position_correction_id]).is_err());
            Ok(())
        })
        .unwrap();

        db.write(|conn| {
            ledger::with_cascade_delete_guard(conn, |conn| {
                conn.execute("DELETE FROM matters WHERE id = ?1", params![matter])?;
                Ok(())
            })
        })
        .unwrap();

        assert_eq!(count_rows(&db, "insurance_claims", &matter), 0);
        assert_eq!(count_rows(&db, "insurance_claim_insurers", &matter), 0);
        assert_eq!(count_rows(&db, "insurance_claim_status_history", &matter), 0);
        assert_eq!(count_rows(&db, "negotiation_events", &matter), 0);
        assert_eq!(count_rows(&db, "negotiation_positions", &matter), 0);
        assert_eq!(count_rows(&db, "negotiation_event_corrections", &matter), 0);
        assert_eq!(count_rows(&db, "negotiation_position_corrections", &matter), 0);
        assert_eq!(count_rows(&db, "negotiation_waiting_links", &matter), 0);
        assert_eq!(count_rows(&db, "waiting_for", &matter), 0);
        let guard_active: i64 = db
            .read(|conn| {
                conn.query_row("SELECT active FROM ledger_delete_guard WHERE id = 1", [], |row| row.get(0))
                    .map_err(AppError::Db)
            })
            .unwrap();
        assert_eq!(guard_active, 0);
    }

    #[test]
    fn v19_schema_exposes_expected_b7_tables_indexes_and_triggers() {
        let (_dir, db) = temp_db();
        db.read(|conn| {
            for table in [
                "insurance_claims",
                "insurance_claim_insurers",
                "insurance_claim_status_history",
                "negotiation_events",
                "negotiation_positions",
                "negotiation_event_corrections",
                "negotiation_position_corrections",
                "negotiation_waiting_links",
            ] {
                let count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    params![table],
                    |row| row.get(0),
                )?;
                assert_eq!(count, 1, "missing B7 table {table}");
            }
            for index in [
                "idx_waiting_for_id_matter_unique",
                "idx_insurance_claim_insurers_party",
                "idx_insurance_claim_status_history",
                "idx_negotiation_waiting_links_matter",
                "idx_negotiation_event_corrections_matter",
                "idx_negotiation_position_corrections_matter",
            ] {
                let count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                    params![index],
                    |row| row.get(0),
                )?;
                assert_eq!(count, 1, "missing B7 index {index}");
            }
            for trigger in [
                "trg_insurance_claim_insurer_role_insert",
                "trg_insurance_claim_insurer_role_update",
                "trg_insurance_claim_insurer_party_role_guard",
                "trg_negotiation_events_no_update",
                "trg_negotiation_events_no_delete",
                "trg_negotiation_positions_no_update",
                "trg_negotiation_positions_no_delete",
                "trg_insurance_claim_status_history_no_update",
                "trg_insurance_claim_status_history_no_delete",
                "trg_negotiation_event_corrections_no_chain",
                "trg_negotiation_event_corrections_no_update",
                "trg_negotiation_event_corrections_no_delete",
                "trg_negotiation_position_corrections_no_chain",
                "trg_negotiation_position_corrections_no_update",
                "trg_negotiation_position_corrections_no_delete",
            ] {
                let count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name = ?1",
                    params![trigger],
                    |row| row.get(0),
                )?;
                assert_eq!(count, 1, "missing B7 trigger {trigger}");
            }
            let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
            assert_eq!(user_version, 19);
            Ok(())
        })
        .unwrap();
    }
}
