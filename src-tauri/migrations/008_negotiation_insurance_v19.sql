PRAGMA foreign_keys = ON;

-- Phase B7: Negotiation & Insurance workspace.
-- This migration is forward-only and idempotent. Insurance-claim identity is
-- anchored to matter_parties(role='insurer'); insurer_name is retained only as
-- a historical snapshot for databases created by earlier B7 branch revisions.

CREATE TABLE IF NOT EXISTS insurance_claims (
  id TEXT PRIMARY KEY,
  matter_id TEXT NOT NULL,
  insurer_name TEXT NOT NULL DEFAULT '',
  claim_number TEXT,
  policy_number TEXT,
  handler_name TEXT,
  handler_contact TEXT,
  status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','awaiting_response','negotiating','settled','closed')),
  notes TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (matter_id) REFERENCES matters(id) ON DELETE CASCADE,
  UNIQUE(id, matter_id)
);

CREATE TABLE IF NOT EXISTS insurance_claim_insurers (
  claim_id TEXT PRIMARY KEY,
  matter_id TEXT NOT NULL,
  insurer_party_id TEXT NOT NULL,
  insurer_name_snapshot TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (claim_id, matter_id) REFERENCES insurance_claims(id, matter_id) ON DELETE CASCADE,
  FOREIGN KEY (insurer_party_id, matter_id) REFERENCES matter_parties(id, matter_id) ON DELETE CASCADE,
  UNIQUE(claim_id, matter_id)
);

CREATE TABLE IF NOT EXISTS insurance_claim_status_history (
  id TEXT PRIMARY KEY,
  matter_id TEXT NOT NULL,
  insurance_claim_id TEXT NOT NULL,
  from_status TEXT CHECK (from_status IS NULL OR from_status IN ('open','awaiting_response','negotiating','settled','closed')),
  to_status TEXT NOT NULL CHECK (to_status IN ('open','awaiting_response','negotiating','settled','closed')),
  changed_at TEXT NOT NULL,
  note TEXT,
  actor_kind TEXT NOT NULL DEFAULT 'human' CHECK (actor_kind = 'human'),
  created_at TEXT NOT NULL,
  FOREIGN KEY (matter_id) REFERENCES matters(id) ON DELETE CASCADE,
  FOREIGN KEY (insurance_claim_id, matter_id) REFERENCES insurance_claims(id, matter_id) ON DELETE CASCADE,
  UNIQUE(id, matter_id)
);

CREATE TABLE IF NOT EXISTS negotiation_events (
  id TEXT PRIMARY KEY,
  matter_id TEXT NOT NULL,
  insurance_claim_id TEXT,
  event_kind TEXT NOT NULL CHECK (event_kind IN ('call','email','letter','meeting','request','follow_up','other')),
  happened_at TEXT NOT NULL,
  summary TEXT NOT NULL,
  follow_up_at TEXT,
  source_document_version_id TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY (matter_id) REFERENCES matters(id) ON DELETE CASCADE,
  FOREIGN KEY (insurance_claim_id, matter_id) REFERENCES insurance_claims(id, matter_id),
  FOREIGN KEY (source_document_version_id, matter_id) REFERENCES document_versions(id, matter_id),
  UNIQUE(id, matter_id)
);

CREATE TABLE IF NOT EXISTS negotiation_positions (
  id TEXT PRIMARY KEY,
  matter_id TEXT NOT NULL,
  insurance_claim_id TEXT,
  side TEXT NOT NULL CHECK (side IN ('our_side','counterparty')),
  kind TEXT NOT NULL CHECK (kind IN ('demand','offer','counter_offer')),
  amount_cents INTEGER NOT NULL CHECK (amount_cents >= 0),
  currency TEXT NOT NULL DEFAULT 'ILS' CHECK (currency = 'ILS'),
  recorded_at TEXT NOT NULL,
  notes TEXT,
  source_document_version_id TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY (matter_id) REFERENCES matters(id) ON DELETE CASCADE,
  FOREIGN KEY (insurance_claim_id, matter_id) REFERENCES insurance_claims(id, matter_id),
  FOREIGN KEY (source_document_version_id, matter_id) REFERENCES document_versions(id, matter_id),
  UNIQUE(id, matter_id)
);

CREATE TABLE IF NOT EXISTS negotiation_event_corrections (
  id TEXT PRIMARY KEY,
  matter_id TEXT NOT NULL,
  original_event_id TEXT NOT NULL,
  replacement_event_id TEXT NOT NULL,
  reason TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY (matter_id) REFERENCES matters(id) ON DELETE CASCADE,
  FOREIGN KEY (original_event_id, matter_id) REFERENCES negotiation_events(id, matter_id) ON DELETE CASCADE,
  FOREIGN KEY (replacement_event_id, matter_id) REFERENCES negotiation_events(id, matter_id) ON DELETE CASCADE,
  UNIQUE(original_event_id, matter_id),
  UNIQUE(replacement_event_id, matter_id),
  CHECK (original_event_id <> replacement_event_id)
);

CREATE TABLE IF NOT EXISTS negotiation_position_corrections (
  id TEXT PRIMARY KEY,
  matter_id TEXT NOT NULL,
  original_position_id TEXT NOT NULL,
  replacement_position_id TEXT NOT NULL,
  reason TEXT,
  created_at TEXT NOT NULL,
  FOREIGN KEY (matter_id) REFERENCES matters(id) ON DELETE CASCADE,
  FOREIGN KEY (original_position_id, matter_id) REFERENCES negotiation_positions(id, matter_id) ON DELETE CASCADE,
  FOREIGN KEY (replacement_position_id, matter_id) REFERENCES negotiation_positions(id, matter_id) ON DELETE CASCADE,
  UNIQUE(original_position_id, matter_id),
  UNIQUE(replacement_position_id, matter_id),
  CHECK (original_position_id <> replacement_position_id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_waiting_for_id_matter_unique ON waiting_for(id, matter_id);

CREATE TABLE IF NOT EXISTS negotiation_waiting_links (
  event_id TEXT PRIMARY KEY,
  matter_id TEXT NOT NULL,
  waiting_for_id TEXT NOT NULL UNIQUE,
  created_at TEXT NOT NULL,
  FOREIGN KEY (event_id, matter_id) REFERENCES negotiation_events(id, matter_id) ON DELETE CASCADE,
  FOREIGN KEY (waiting_for_id, matter_id) REFERENCES waiting_for(id, matter_id) ON DELETE CASCADE,
  UNIQUE(event_id, matter_id)
);

CREATE INDEX IF NOT EXISTS idx_insurance_claims_matter ON insurance_claims(matter_id, status, updated_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_insurance_claim_insurers_party ON insurance_claim_insurers(matter_id, insurer_party_id);
CREATE INDEX IF NOT EXISTS idx_insurance_claim_status_history ON insurance_claim_status_history(matter_id, insurance_claim_id, changed_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_negotiation_events_matter ON negotiation_events(matter_id, happened_at DESC, created_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_negotiation_events_claim ON negotiation_events(matter_id, insurance_claim_id, happened_at DESC);
CREATE INDEX IF NOT EXISTS idx_negotiation_events_source ON negotiation_events(matter_id, source_document_version_id);
CREATE INDEX IF NOT EXISTS idx_negotiation_positions_matter ON negotiation_positions(matter_id, recorded_at DESC, created_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_negotiation_positions_claim ON negotiation_positions(matter_id, insurance_claim_id, recorded_at DESC);
CREATE INDEX IF NOT EXISTS idx_negotiation_positions_source ON negotiation_positions(matter_id, source_document_version_id);
CREATE INDEX IF NOT EXISTS idx_negotiation_waiting_links_matter ON negotiation_waiting_links(matter_id, waiting_for_id);
CREATE INDEX IF NOT EXISTS idx_negotiation_event_corrections_matter ON negotiation_event_corrections(matter_id, original_event_id, replacement_event_id);
CREATE INDEX IF NOT EXISTS idx_negotiation_position_corrections_matter ON negotiation_position_corrections(matter_id, original_position_id, replacement_position_id);

CREATE TRIGGER IF NOT EXISTS trg_insurance_claim_insurer_role_insert
BEFORE INSERT ON insurance_claim_insurers
WHEN NOT EXISTS (
  SELECT 1 FROM matter_parties p
  WHERE p.id = NEW.insurer_party_id AND p.matter_id = NEW.matter_id AND p.role = 'insurer'
)
BEGIN
  SELECT RAISE(ABORT, 'INSURER_PARTY_REQUIRED');
END;

CREATE TRIGGER IF NOT EXISTS trg_insurance_claim_insurer_role_update
BEFORE UPDATE OF insurer_party_id, matter_id ON insurance_claim_insurers
WHEN NOT EXISTS (
  SELECT 1 FROM matter_parties p
  WHERE p.id = NEW.insurer_party_id AND p.matter_id = NEW.matter_id AND p.role = 'insurer'
)
BEGIN
  SELECT RAISE(ABORT, 'INSURER_PARTY_REQUIRED');
END;

CREATE TRIGGER IF NOT EXISTS trg_insurance_claim_insurer_party_role_guard
BEFORE UPDATE OF role, matter_id ON matter_parties
WHEN EXISTS (
  SELECT 1 FROM insurance_claim_insurers i
  WHERE i.insurer_party_id = OLD.id AND i.matter_id = OLD.matter_id
) AND (NEW.role <> 'insurer' OR NEW.matter_id <> OLD.matter_id)
BEGIN
  SELECT RAISE(ABORT, 'INSURER_PARTY_REQUIRED');
END;

CREATE TRIGGER IF NOT EXISTS trg_insurance_claim_insurer_party_delete_guard
BEFORE DELETE ON matter_parties
WHEN EXISTS (
  SELECT 1 FROM insurance_claim_insurers i
  WHERE i.insurer_party_id = OLD.id AND i.matter_id = OLD.matter_id
) AND (SELECT active FROM ledger_delete_guard WHERE id = 1) = 0
BEGIN
  SELECT RAISE(ABORT, 'INSURER_PARTY_IN_USE');
END;

CREATE TRIGGER IF NOT EXISTS trg_negotiation_events_no_update
BEFORE UPDATE ON negotiation_events
BEGIN
  SELECT RAISE(ABORT, 'NEGOTIATION_EVENT_APPEND_ONLY');
END;

CREATE TRIGGER IF NOT EXISTS trg_negotiation_events_no_delete
BEFORE DELETE ON negotiation_events
WHEN (SELECT active FROM ledger_delete_guard WHERE id = 1) = 0
BEGIN
  SELECT RAISE(ABORT, 'NEGOTIATION_EVENT_APPEND_ONLY');
END;

CREATE TRIGGER IF NOT EXISTS trg_negotiation_positions_no_update
BEFORE UPDATE ON negotiation_positions
BEGIN
  SELECT RAISE(ABORT, 'NEGOTIATION_POSITION_APPEND_ONLY');
END;

CREATE TRIGGER IF NOT EXISTS trg_negotiation_positions_no_delete
BEFORE DELETE ON negotiation_positions
WHEN (SELECT active FROM ledger_delete_guard WHERE id = 1) = 0
BEGIN
  SELECT RAISE(ABORT, 'NEGOTIATION_POSITION_APPEND_ONLY');
END;

CREATE TRIGGER IF NOT EXISTS trg_insurance_claim_status_history_no_update
BEFORE UPDATE ON insurance_claim_status_history
BEGIN
  SELECT RAISE(ABORT, 'INSURANCE_CLAIM_STATUS_HISTORY_IMMUTABLE');
END;

CREATE TRIGGER IF NOT EXISTS trg_insurance_claim_status_history_no_delete
BEFORE DELETE ON insurance_claim_status_history
WHEN (SELECT active FROM ledger_delete_guard WHERE id = 1) = 0
BEGIN
  SELECT RAISE(ABORT, 'INSURANCE_CLAIM_STATUS_HISTORY_IMMUTABLE');
END;

CREATE TRIGGER IF NOT EXISTS trg_negotiation_event_corrections_no_chain
BEFORE INSERT ON negotiation_event_corrections
WHEN EXISTS (
  SELECT 1 FROM negotiation_event_corrections c
  WHERE c.matter_id = NEW.matter_id
    AND (c.replacement_event_id = NEW.original_event_id OR c.original_event_id = NEW.replacement_event_id)
)
BEGIN
  SELECT RAISE(ABORT, 'NEGOTIATION_EVENT_CORRECTION_CHAIN_FORBIDDEN');
END;

CREATE TRIGGER IF NOT EXISTS trg_negotiation_event_corrections_no_update
BEFORE UPDATE ON negotiation_event_corrections
BEGIN
  SELECT RAISE(ABORT, 'NEGOTIATION_EVENT_CORRECTION_IMMUTABLE');
END;

CREATE TRIGGER IF NOT EXISTS trg_negotiation_event_corrections_no_delete
BEFORE DELETE ON negotiation_event_corrections
WHEN (SELECT active FROM ledger_delete_guard WHERE id = 1) = 0
BEGIN
  SELECT RAISE(ABORT, 'NEGOTIATION_EVENT_CORRECTION_IMMUTABLE');
END;

CREATE TRIGGER IF NOT EXISTS trg_negotiation_position_corrections_no_chain
BEFORE INSERT ON negotiation_position_corrections
WHEN EXISTS (
  SELECT 1 FROM negotiation_position_corrections c
  WHERE c.matter_id = NEW.matter_id
    AND (c.replacement_position_id = NEW.original_position_id OR c.original_position_id = NEW.replacement_position_id)
)
BEGIN
  SELECT RAISE(ABORT, 'NEGOTIATION_POSITION_CORRECTION_CHAIN_FORBIDDEN');
END;

CREATE TRIGGER IF NOT EXISTS trg_negotiation_position_corrections_no_update
BEFORE UPDATE ON negotiation_position_corrections
BEGIN
  SELECT RAISE(ABORT, 'NEGOTIATION_POSITION_CORRECTION_IMMUTABLE');
END;

CREATE TRIGGER IF NOT EXISTS trg_negotiation_position_corrections_no_delete
BEFORE DELETE ON negotiation_position_corrections
WHEN (SELECT active FROM ledger_delete_guard WHERE id = 1) = 0
BEGIN
  SELECT RAISE(ABORT, 'NEGOTIATION_POSITION_CORRECTION_IMMUTABLE');
END;

PRAGMA user_version = 19;
