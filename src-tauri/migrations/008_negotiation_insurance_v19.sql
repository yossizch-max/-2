-- Phase B, milestone B7: Negotiation & Insurance Workspace.
-- Additive to 001-007 and idempotent on every DbState::open().
--
-- B7 records operational insurance/negotiation history. It does NOT decide whether
-- a settlement should be accepted and it exposes no automatic settlement-approval
-- state transition. Offers/demands and interaction history are append-only records;
-- corrections are represented by a new row, preserving the audit trail. Direct
-- history deletes are blocked unless the existing guarded whole-matter cascade path
-- is active.

CREATE TABLE IF NOT EXISTS insurance_claims(
 id TEXT PRIMARY KEY,
 matter_id TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
 insurer_name TEXT NOT NULL,
 claim_number TEXT,
 policy_number TEXT,
 handler_name TEXT,
 handler_contact TEXT,
 status TEXT NOT NULL DEFAULT 'open'
   CHECK(status IN ('open','awaiting_response','negotiating','settled','closed')),
 notes TEXT,
 created_at TEXT NOT NULL,
 updated_at TEXT NOT NULL,
 UNIQUE(id,matter_id)
);
CREATE INDEX IF NOT EXISTS idx_insurance_claims_matter
ON insurance_claims(matter_id,status,updated_at);

CREATE TABLE IF NOT EXISTS negotiation_events(
 id TEXT PRIMARY KEY,
 matter_id TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
 insurance_claim_id TEXT,
 event_kind TEXT NOT NULL
   CHECK(event_kind IN ('call','email','letter','meeting','request','follow_up','other')),
 happened_at TEXT NOT NULL,
 summary TEXT NOT NULL,
 follow_up_at TEXT,
 source_document_version_id TEXT,
 created_at TEXT NOT NULL,
 FOREIGN KEY(insurance_claim_id,matter_id) REFERENCES insurance_claims(id,matter_id),
 FOREIGN KEY(source_document_version_id,matter_id) REFERENCES document_versions(id,matter_id),
 UNIQUE(id,matter_id)
);
CREATE INDEX IF NOT EXISTS idx_negotiation_events_matter
ON negotiation_events(matter_id,happened_at,created_at);

CREATE TABLE IF NOT EXISTS negotiation_positions(
 id TEXT PRIMARY KEY,
 matter_id TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
 insurance_claim_id TEXT,
 side TEXT NOT NULL CHECK(side IN ('our_side','counterparty')),
 kind TEXT NOT NULL CHECK(kind IN ('demand','offer','counter_offer')),
 amount_cents INTEGER NOT NULL CHECK(amount_cents>=0),
 currency TEXT NOT NULL DEFAULT 'ILS',
 recorded_at TEXT NOT NULL,
 notes TEXT,
 source_document_version_id TEXT,
 created_at TEXT NOT NULL,
 FOREIGN KEY(insurance_claim_id,matter_id) REFERENCES insurance_claims(id,matter_id),
 FOREIGN KEY(source_document_version_id,matter_id) REFERENCES document_versions(id,matter_id),
 UNIQUE(id,matter_id)
);
CREATE INDEX IF NOT EXISTS idx_negotiation_positions_matter
ON negotiation_positions(matter_id,recorded_at,created_at);

CREATE TRIGGER IF NOT EXISTS trg_negotiation_event_no_update
BEFORE UPDATE ON negotiation_events
BEGIN SELECT RAISE(ABORT,'NEGOTIATION_HISTORY_APPEND_ONLY'); END;

CREATE TRIGGER IF NOT EXISTS trg_negotiation_event_no_delete
BEFORE DELETE ON negotiation_events
WHEN (SELECT active FROM ledger_delete_guard WHERE id=1)=0
BEGIN SELECT RAISE(ABORT,'NEGOTIATION_HISTORY_APPEND_ONLY'); END;

CREATE TRIGGER IF NOT EXISTS trg_negotiation_position_no_update
BEFORE UPDATE ON negotiation_positions
BEGIN SELECT RAISE(ABORT,'NEGOTIATION_HISTORY_APPEND_ONLY'); END;

CREATE TRIGGER IF NOT EXISTS trg_negotiation_position_no_delete
BEFORE DELETE ON negotiation_positions
WHEN (SELECT active FROM ledger_delete_guard WHERE id=1)=0
BEGIN SELECT RAISE(ABORT,'NEGOTIATION_HISTORY_APPEND_ONLY'); END;

PRAGMA user_version = 19;
