-- Phase B, milestone B7: Negotiation & Insurance Workspace.
-- Additive to 001-007 and idempotent on every DbState::open().
--
-- B7 records operational insurance/negotiation history. It does NOT decide whether
-- a settlement should be accepted. Offers/demands and interaction history are
-- append-only. Operational follow-up uses the existing waiting_for lifecycle; the
-- event's follow_up_at is only the immutable historical value captured when the
-- event was recorded.
--
-- insurance_claims.insurer_name is retained as a display/audit snapshot for the
-- pre-release v19 schema, but the authoritative insurer identity is the linked
-- matter_parties row in insurance_claim_insurers.

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

CREATE TABLE IF NOT EXISTS insurance_claim_insurers(
 claim_id TEXT PRIMARY KEY,
 matter_id TEXT NOT NULL,
 insurer_party_id TEXT NOT NULL,
 insurer_name_snapshot TEXT NOT NULL,
 created_at TEXT NOT NULL,
 updated_at TEXT NOT NULL,
 FOREIGN KEY(claim_id,matter_id) REFERENCES insurance_claims(id,matter_id) ON DELETE CASCADE,
 FOREIGN KEY(insurer_party_id,matter_id) REFERENCES matter_parties(id,matter_id),
 UNIQUE(claim_id,matter_id)
);
CREATE INDEX IF NOT EXISTS idx_insurance_claim_insurers_party
ON insurance_claim_insurers(matter_id,insurer_party_id);

CREATE TABLE IF NOT EXISTS insurance_claim_status_history(
 id TEXT PRIMARY KEY,
 matter_id TEXT NOT NULL,
 insurance_claim_id TEXT NOT NULL,
 from_status TEXT
   CHECK(from_status IS NULL OR from_status IN ('open','awaiting_response','negotiating','settled','closed')),
 to_status TEXT NOT NULL
   CHECK(to_status IN ('open','awaiting_response','negotiating','settled','closed')),
 changed_at TEXT NOT NULL,
 note TEXT,
 actor_kind TEXT NOT NULL DEFAULT 'human' CHECK(actor_kind='human'),
 FOREIGN KEY(insurance_claim_id,matter_id) REFERENCES insurance_claims(id,matter_id) ON DELETE CASCADE,
 UNIQUE(id,matter_id)
);
CREATE INDEX IF NOT EXISTS idx_insurance_claim_status_history
ON insurance_claim_status_history(matter_id,insurance_claim_id,changed_at,id);

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
 currency TEXT NOT NULL DEFAULT 'ILS' CHECK(currency='ILS'),
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

CREATE TABLE IF NOT EXISTS negotiation_event_corrections(
 id TEXT PRIMARY KEY,
 matter_id TEXT NOT NULL,
 original_event_id TEXT NOT NULL,
 replacement_event_id TEXT NOT NULL,
 created_at TEXT NOT NULL,
 FOREIGN KEY(original_event_id,matter_id) REFERENCES negotiation_events(id,matter_id),
 FOREIGN KEY(replacement_event_id,matter_id) REFERENCES negotiation_events(id,matter_id),
 UNIQUE(matter_id,original_event_id),
 UNIQUE(matter_id,replacement_event_id)
);

CREATE TABLE IF NOT EXISTS negotiation_position_corrections(
 id TEXT PRIMARY KEY,
 matter_id TEXT NOT NULL,
 original_position_id TEXT NOT NULL,
 replacement_position_id TEXT NOT NULL,
 created_at TEXT NOT NULL,
 FOREIGN KEY(original_position_id,matter_id) REFERENCES negotiation_positions(id,matter_id),
 FOREIGN KEY(replacement_position_id,matter_id) REFERENCES negotiation_positions(id,matter_id),
 UNIQUE(matter_id,original_position_id),
 UNIQUE(matter_id,replacement_position_id)
);

-- waiting_for predates B7 and originally had only id as its declared key. The unique
-- composite index lets the B7 link enforce same-matter isolation at the DB level.
CREATE UNIQUE INDEX IF NOT EXISTS idx_waiting_for_id_matter_unique
ON waiting_for(id,matter_id);

CREATE TABLE IF NOT EXISTS negotiation_waiting_links(
 event_id TEXT PRIMARY KEY,
 matter_id TEXT NOT NULL,
 waiting_for_id TEXT NOT NULL UNIQUE,
 created_at TEXT NOT NULL,
 FOREIGN KEY(event_id,matter_id) REFERENCES negotiation_events(id,matter_id) ON DELETE CASCADE,
 FOREIGN KEY(waiting_for_id,matter_id) REFERENCES waiting_for(id,matter_id) ON DELETE CASCADE,
 UNIQUE(event_id,matter_id)
);
CREATE INDEX IF NOT EXISTS idx_negotiation_waiting_links_matter
ON negotiation_waiting_links(matter_id,waiting_for_id);

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

CREATE TRIGGER IF NOT EXISTS trg_claim_status_history_no_update
BEFORE UPDATE ON insurance_claim_status_history
BEGIN SELECT RAISE(ABORT,'CLAIM_STATUS_HISTORY_IMMUTABLE'); END;

CREATE TRIGGER IF NOT EXISTS trg_claim_status_history_no_delete
BEFORE DELETE ON insurance_claim_status_history
WHEN (SELECT active FROM ledger_delete_guard WHERE id=1)=0
BEGIN SELECT RAISE(ABORT,'CLAIM_STATUS_HISTORY_IMMUTABLE'); END;

CREATE TRIGGER IF NOT EXISTS trg_negotiation_event_correction_no_update
BEFORE UPDATE ON negotiation_event_corrections
BEGIN SELECT RAISE(ABORT,'NEGOTIATION_HISTORY_APPEND_ONLY'); END;

CREATE TRIGGER IF NOT EXISTS trg_negotiation_event_correction_no_delete
BEFORE DELETE ON negotiation_event_corrections
WHEN (SELECT active FROM ledger_delete_guard WHERE id=1)=0
BEGIN SELECT RAISE(ABORT,'NEGOTIATION_HISTORY_APPEND_ONLY'); END;

CREATE TRIGGER IF NOT EXISTS trg_negotiation_position_correction_no_update
BEFORE UPDATE ON negotiation_position_corrections
BEGIN SELECT RAISE(ABORT,'NEGOTIATION_HISTORY_APPEND_ONLY'); END;

CREATE TRIGGER IF NOT EXISTS trg_negotiation_position_correction_no_delete
BEFORE DELETE ON negotiation_position_corrections
WHEN (SELECT active FROM ledger_delete_guard WHERE id=1)=0
BEGIN SELECT RAISE(ABORT,'NEGOTIATION_HISTORY_APPEND_ONLY'); END;

PRAGMA user_version = 19;
