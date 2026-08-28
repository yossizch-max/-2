//! Focused B7 correctness command facades.
//!
//! These command implementations keep the external command names stable while
//! hardening the three pre-gate correctness issues: claim-scoped snapshots,
//! matter-isolated waiting-for closure, and system-time claim `updated_at`.

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
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| AppError::Validation(format!("{field} must be a valid RFC3339 timestamp")))
}

fn is_overdue_at(value: &str, as_of: DateTime<Utc>) -> AppResult<bool> {
    Ok(parse_utc(value, "followUpAt")? < as_of)
}

fn required_str<'a>(payload: &'a Value, key: &str) -> AppResult<&'a str> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::Validation(format!("{key} is required")))
}

fn optional_trimmed<'a>(payload: &'a Value, key: &str) -> Option<&'a str> {
    payload
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
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

fn read_claim(conn: &Connection, matter_id: &str, claim_id: &str) -> AppResult<Value> {
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
         WHERE c.id = ?1 AND c.matter_id = ?2",
        params![claim_id, matter_id],
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
    .optional()?
    .ok_or_else(|| AppError::NotFound("Insurance claim not found".into()))
}

fn latest_position(
    conn: &Connection,
    matter_id: &str,
    claim_id: &str,
    our_side: bool,
) -> AppResult<Option<Value>> {
    let sql = if our_side {
        "SELECT p.id, p.side, p.kind, p.amount_cents, p.currency, p.recorded_at
         FROM negotiation_positions p
         WHERE p.matter_id = ?1
           AND p.insurance_claim_id = ?2
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
           AND p.insurance_claim_id = ?2
           AND p.side = 'counterparty'
           AND p.kind IN ('offer','counter_offer')
           AND NOT EXISTS (
             SELECT 1 FROM negotiation_position_corrections c
             WHERE c.matter_id = p.matter_id AND c.original_position_id = p.id
           )
         ORDER BY p.recorded_at DESC, p.created_at DESC, p.id DESC
         LIMIT 1"
    };
    conn.query_row(sql, params![matter_id, claim_id], |row| {
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

fn latest_interaction(conn: &Connection, matter_id: &str, claim_id: &str) -> AppResult<Option<Value>> {
    conn.query_row(
        "SELECT e.id, e.event_kind, e.happened_at, e.summary
         FROM negotiation_events e
         WHERE e.matter_id = ?1
           AND e.insurance_claim_id = ?2
           AND NOT EXISTS (
             SELECT 1 FROM negotiation_event_corrections c
             WHERE c.matter_id = e.matter_id AND c.original_event_id = e.id
           )
         ORDER BY e.happened_at DESC, e.created_at DESC, e.id DESC
         LIMIT 1",
        params![matter_id, claim_id],
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
    claim_id: &str,
    as_of: DateTime<Utc>,
) -> AppResult<Option<Value>> {
    let row: Option<(String, String, String, String, String)> = conn
        .query_row(
            "SELECT w.id, w.follow_up_at, w.party_label, w.item_label, l.event_id
             FROM negotiation_waiting_links l
             JOIN negotiation_events e ON e.id = l.event_id AND e.matter_id = l.matter_id
             JOIN waiting_for w ON w.id = l.waiting_for_id AND w.matter_id = l.matter_id
             WHERE l.matter_id = ?1
               AND e.insurance_claim_id = ?2
               AND w.status = 'open'
               AND w.follow_up_at IS NOT NULL
               AND NOT EXISTS (
                 SELECT 1 FROM negotiation_event_corrections c
                 WHERE c.matter_id = e.matter_id AND c.original_event_id = e.id
               )
             ORDER BY w.follow_up_at ASC, w.id ASC
             LIMIT 1",
            params![matter_id, claim_id],
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

fn snapshot_from_conn(
    conn: &Connection,
    matter_id: &str,
    claim_id: &str,
    as_of: DateTime<Utc>,
) -> AppResult<Value> {
    ensure_matter(conn, matter_id)?;
    ensure_claim_in_matter(conn, matter_id, claim_id)?;
    let current_claim = read_claim(conn, matter_id, claim_id)?;
    let latest_our_demand = latest_position(conn, matter_id, claim_id, true)?;
    let latest_counterparty_offer = latest_position(conn, matter_id, claim_id, false)?;
    let gap = match (&latest_our_demand, &latest_counterparty_offer) {
        (Some(demand), Some(offer)) => {
            let demand_currency = demand.get("currency").and_then(Value::as_str);
            let offer_currency = offer.get("currency").and_then(Value::as_str);
            match (demand_currency, offer_currency) {
                (Some(left), Some(right)) if left == right => {
                    let demand_amount = demand
                        .get("amountCents")
                        .and_then(Value::as_i64)
                        .ok_or_else(|| AppError::Validation("latest demand amount is malformed".into()))?;
                    let offer_amount = offer
                        .get("amountCents")
                        .and_then(Value::as_i64)
                        .ok_or_else(|| AppError::Validation("latest offer amount is malformed".into()))?;
                    Some(json!({ "amountCents": demand_amount - offer_amount, "currency": left }))
                }
                _ => None,
            }
        }
        _ => None,
    };
    let negotiation_status = current_claim.get("status").cloned().unwrap_or(Value::Null);

    Ok(json!({
        "matterId": matter_id,
        "insuranceClaimId": claim_id,
        "currentClaim": current_claim,
        "latestOurDemand": latest_our_demand,
        "latestCounterpartyOffer": latest_counterparty_offer,
        "gap": gap,
        "latestInteraction": latest_interaction(conn, matter_id, claim_id)?,
        "nextFollowUp": next_follow_up(conn, matter_id, claim_id, as_of)?,
        "negotiationStatus": negotiation_status,
    }))
}

pub(crate) fn snapshot_for_now(
    db: &DbState,
    matter_id: &str,
    claim_id: &str,
    as_of: DateTime<Utc>,
) -> AppResult<Value> {
    db.read(|conn| snapshot_from_conn(conn, matter_id, claim_id, as_of))
}

pub(crate) fn snapshot(db: &DbState, matter_id: &str, claim_id: &str) -> AppResult<Value> {
    snapshot_for_now(db, matter_id, claim_id, Utc::now())
}

pub(crate) fn close_waiting_for_item(
    db: &DbState,
    matter_id: &str,
    waiting_for_id: &str,
) -> AppResult<()> {
    db.write(|conn| {
        let changed = conn.execute(
            "UPDATE waiting_for SET status='closed',last_contact_at=?3 WHERE id=?1 AND matter_id=?2",
            params![waiting_for_id, matter_id, now_utc()],
        )?;
        if changed != 1 {
            return Err(AppError::NotFound("waiting_for item".into()));
        }
        Ok(())
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
        let system_updated_at = now_utc();
        let changed = tx.execute(
            "UPDATE insurance_claims SET status = ?3, updated_at = ?4 WHERE id = ?1 AND matter_id = ?2",
            params![claim_id, matter_id, to_status, system_updated_at],
        )?;
        if changed != 1 {
            return Err(AppError::NotFound("Insurance claim not found".into()));
        }
        let history_id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO insurance_claim_status_history
               (id, matter_id, insurance_claim_id, from_status, to_status, changed_at, note, actor_kind, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'human', ?8)",
            params![
                history_id,
                matter_id,
                claim_id,
                Some(from_status.as_str()),
                to_status,
                changed_at,
                note.as_deref(),
                now_utc()
            ],
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

#[tauri::command]
pub fn get_negotiation_snapshot(
    state: State<'_, AppState>,
    matter_id: String,
    insurance_claim_id: String,
) -> AppResult<Value> {
    snapshot(&state.db, &matter_id, &insurance_claim_id)
}

#[tauri::command]
pub fn change_insurance_claim_status(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    change_claim_status(&state.db, &payload)
}

#[tauri::command]
pub fn close_waiting_for(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id = required_str(&payload, "matterId")?;
    let waiting_for_id = required_str(&payload, "waitingForId")?;
    close_waiting_for_item(&state.db, matter_id, waiting_for_id)?;
    Ok(json!({ "ok": true }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::case_health;
    use tempfile::TempDir;

    fn temp_db() -> (TempDir, DbState) {
        let dir = tempfile::tempdir().unwrap();
        let db = DbState::open(dir.path().join("test.sqlite3")).unwrap();
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

    fn add_party(db: &DbState, matter_id: &str, name: &str) -> String {
        let party_id = Uuid::new_v4().to_string();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO matter_parties
                   (id, matter_id, role, entity_kind, display_name, created_at, updated_at)
                 VALUES (?1, ?2, 'insurer', 'organization', ?3, ?4, ?4)",
                params![party_id, matter_id, name, now_utc()],
            )?;
            Ok(())
        })
        .unwrap();
        party_id
    }

    fn add_claim_with_updated_at(
        db: &DbState,
        matter_id: &str,
        insurer_party_id: &str,
        insurer_name: &str,
        claim_number: &str,
        updated_at: &str,
    ) -> String {
        let claim_id = Uuid::new_v4().to_string();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO insurance_claims
                   (id, matter_id, insurer_name, claim_number, status, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 'open', ?5, ?5)",
                params![claim_id, matter_id, insurer_name, claim_number, updated_at],
            )?;
            conn.execute(
                "INSERT INTO insurance_claim_insurers
                   (claim_id, matter_id, insurer_party_id, insurer_name_snapshot, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
                params![claim_id, matter_id, insurer_party_id, insurer_name, updated_at],
            )?;
            conn.execute(
                "INSERT INTO insurance_claim_status_history
                   (id, matter_id, insurance_claim_id, from_status, to_status, changed_at, actor_kind, created_at)
                 VALUES (?1, ?2, ?3, NULL, 'open', ?4, 'human', ?4)",
                params![Uuid::new_v4().to_string(), matter_id, claim_id, updated_at],
            )?;
            Ok(())
        })
        .unwrap();
        claim_id
    }

    fn add_claim(
        db: &DbState,
        matter_id: &str,
        insurer_party_id: &str,
        insurer_name: &str,
        claim_number: &str,
    ) -> String {
        add_claim_with_updated_at(
            db,
            matter_id,
            insurer_party_id,
            insurer_name,
            claim_number,
            &now_utc(),
        )
    }

    fn add_position(
        db: &DbState,
        matter_id: &str,
        claim_id: &str,
        side: &str,
        kind: &str,
        amount_cents: i64,
        recorded_at: &str,
        created_at: &str,
    ) -> String {
        let position_id = Uuid::new_v4().to_string();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO negotiation_positions
                   (id, matter_id, insurance_claim_id, side, kind, amount_cents, currency, recorded_at, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'ILS', ?7, ?8)",
                params![position_id, matter_id, claim_id, side, kind, amount_cents, recorded_at, created_at],
            )?;
            Ok(())
        })
        .unwrap();
        position_id
    }

    fn add_event_with_follow_up(
        db: &DbState,
        matter_id: &str,
        claim_id: &str,
        summary: &str,
        happened_at: &str,
        follow_up_at: &str,
    ) -> (String, String) {
        let event_id = Uuid::new_v4().to_string();
        let waiting_id = Uuid::new_v4().to_string();
        db.write(|conn| {
            conn.execute(
                "INSERT INTO negotiation_events
                   (id, matter_id, insurance_claim_id, event_kind, happened_at, summary, follow_up_at, created_at)
                 VALUES (?1, ?2, ?3, 'email', ?4, ?5, ?6, ?4)",
                params![event_id, matter_id, claim_id, happened_at, summary, follow_up_at],
            )?;
            conn.execute(
                "INSERT INTO waiting_for
                   (id, matter_id, party_label, item_label, since_at, follow_up_at, status, source_ref)
                 VALUES (?1, ?2, 'Insurer', ?3, ?4, ?5, 'open', ?6)",
                params![
                    waiting_id,
                    matter_id,
                    summary,
                    happened_at,
                    follow_up_at,
                    format!("negotiation_event:{event_id}")
                ],
            )?;
            conn.execute(
                "INSERT INTO negotiation_waiting_links (event_id, matter_id, waiting_for_id, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![event_id, matter_id, waiting_id, happened_at],
            )?;
            Ok(())
        })
        .unwrap();
        (event_id, waiting_id)
    }

    #[test]
    fn snapshot_is_scoped_to_the_selected_insurance_claim() {
        let (_dir, db) = temp_db();
        let matter = add_matter(&db, "Matter A");
        let other_matter = add_matter(&db, "Matter B");
        let clal_party = add_party(&db, &matter, "Clal");
        let harel_party = add_party(&db, &matter, "Harel");
        let other_party = add_party(&db, &other_matter, "Other");
        let claim_a = add_claim(&db, &matter, &clal_party, "Clal", "A");
        let claim_b = add_claim(&db, &matter, &harel_party, "Harel", "B");
        let other_claim = add_claim(&db, &other_matter, &other_party, "Other", "X");

        add_position(
            &db,
            &matter,
            &claim_a,
            "our_side",
            "demand",
            1_200_000,
            "2026-08-27T10:00:00Z",
            "2026-08-27T10:00:00Z",
        );
        add_position(
            &db,
            &matter,
            &claim_b,
            "counterparty",
            "offer",
            500_000,
            "2026-08-28T10:00:00Z",
            "2026-08-28T10:00:00Z",
        );
        add_event_with_follow_up(
            &db,
            &matter,
            &claim_b,
            "Harel interaction",
            "2026-08-28T12:00:00Z",
            "2026-08-29T12:00:00Z",
        );

        let as_of = parse_utc("2026-08-30T00:00:00Z", "asOf").unwrap();
        let snapshot_a = snapshot_for_now(&db, &matter, &claim_a, as_of).unwrap();
        assert_eq!(snapshot_a["currentClaim"]["id"].as_str().unwrap(), claim_a);
        assert_eq!(snapshot_a["currentClaim"]["insurerDisplayName"], "Clal");
        assert_eq!(snapshot_a["latestOurDemand"]["amountCents"], 1_200_000);
        assert!(snapshot_a["latestCounterpartyOffer"].is_null());
        assert!(snapshot_a["latestInteraction"].is_null());
        assert!(snapshot_a["nextFollowUp"].is_null());
        assert!(snapshot_a["gap"].is_null());

        let snapshot_b = snapshot_for_now(&db, &matter, &claim_b, as_of).unwrap();
        assert_eq!(snapshot_b["currentClaim"]["id"].as_str().unwrap(), claim_b);
        assert_eq!(snapshot_b["currentClaim"]["insurerDisplayName"], "Harel");
        assert!(snapshot_b["latestOurDemand"].is_null());
        assert_eq!(snapshot_b["latestCounterpartyOffer"]["amountCents"], 500_000);
        assert_eq!(snapshot_b["latestInteraction"]["summary"], "Harel interaction");
        assert_eq!(snapshot_b["nextFollowUp"]["itemLabel"], "Harel interaction");
        assert!(snapshot_b["nextFollowUp"]["overdue"].as_bool().unwrap());
        assert!(snapshot_b["gap"].is_null());

        assert!(snapshot_for_now(&db, &matter, &other_claim, as_of).is_err());
    }

    #[test]
    fn close_waiting_for_requires_same_matter_and_clears_negotiation_health_signal() {
        let (_dir, db) = temp_db();
        let matter_a = add_matter(&db, "Matter A");
        let matter_b = add_matter(&db, "Matter B");
        let insurer = add_party(&db, &matter_a, "Insurer A");
        let claim = add_claim(&db, &matter_a, &insurer, "Insurer A", "A");
        let (_event_id, waiting_id) = add_event_with_follow_up(
            &db,
            &matter_a,
            &claim,
            "Overdue insurer follow-up",
            "2000-01-01T00:00:00Z",
            "2000-01-02T00:00:00Z",
        );

        let health_before = case_health::compute(&db, &matter_a).unwrap();
        assert_eq!(health_before.next_best_action.code, "follow_up_negotiation");
        let snapshot_before = snapshot_for_now(
            &db,
            &matter_a,
            &claim,
            parse_utc("2000-01-03T00:00:00Z", "asOf").unwrap(),
        )
        .unwrap();
        assert_eq!(snapshot_before["nextFollowUp"]["waitingForId"].as_str().unwrap(), waiting_id);

        assert!(close_waiting_for_item(&db, &matter_b, &waiting_id).is_err());
        let status_after_wrong_matter: String = db
            .read(|conn| {
                conn.query_row(
                    "SELECT status FROM waiting_for WHERE id = ?1 AND matter_id = ?2",
                    params![waiting_id, matter_a],
                    |row| row.get(0),
                )
                .map_err(AppError::Db)
            })
            .unwrap();
        assert_eq!(status_after_wrong_matter, "open");

        close_waiting_for_item(&db, &matter_a, &waiting_id).unwrap();
        let snapshot_after = snapshot_for_now(
            &db,
            &matter_a,
            &claim,
            parse_utc("2000-01-03T00:00:00Z", "asOf").unwrap(),
        )
        .unwrap();
        assert!(snapshot_after["nextFollowUp"].is_null());
        let health_after = case_health::compute(&db, &matter_a).unwrap();
        assert_ne!(health_after.next_best_action.code, "follow_up_negotiation");
        assert!(!health_after
            .factors
            .iter()
            .any(|factor| factor.code == "negotiation_followups_overdue"));
    }

    #[test]
    fn claim_status_history_changed_at_is_not_claim_updated_at() {
        let (_dir, db) = temp_db();
        let matter = add_matter(&db, "Matter A");
        let insurer = add_party(&db, &matter, "Insurer A");
        let created_at = "2026-08-27T10:00:00Z";
        let claim = add_claim_with_updated_at(&db, &matter, &insurer, "Insurer A", "A", created_at);

        let transition = change_claim_status(
            &db,
            &json!({
                "matterId": matter,
                "claimId": claim,
                "toStatus": "negotiating",
                "changedAt": "2000-01-01T12:00:00+02:00",
                "actorKind": "human",
                "note": "historical transition date"
            }),
        )
        .unwrap();
        assert_eq!(transition["changedAt"], "2000-01-01T10:00:00Z");

        let (claim_updated_at, history_changed_at): (String, String) = db
            .read(|conn| {
                let claim_updated_at: String = conn.query_row(
                    "SELECT updated_at FROM insurance_claims WHERE id = ?1 AND matter_id = ?2",
                    params![claim, matter],
                    |row| row.get(0),
                )?;
                let history_changed_at: String = conn.query_row(
                    "SELECT changed_at FROM insurance_claim_status_history
                     WHERE insurance_claim_id = ?1 AND matter_id = ?2 AND to_status = 'negotiating'",
                    params![claim, matter],
                    |row| row.get(0),
                )?;
                Ok((claim_updated_at, history_changed_at))
            })
            .unwrap();
        assert_eq!(history_changed_at, "2000-01-01T10:00:00Z");
        assert_ne!(claim_updated_at, history_changed_at);
        assert!(parse_utc(&claim_updated_at, "updatedAt").unwrap() > parse_utc(&history_changed_at, "changedAt").unwrap());
        assert!(parse_utc(&claim_updated_at, "updatedAt").unwrap() >= parse_utc(created_at, "createdAt").unwrap());
    }
}
