-- Phase B, milestone B4: Medical/Wage/Liability Ledgers. Additive to 001-005, same
-- discipline: every statement here is re-run via execute_batch on every
-- DbState::open() call, so everything is CREATE ... IF NOT EXISTS.
--
-- Three parallel ledgers (medical_events/wage_records/liability_facts), each with its
-- own source-grounding child table copying legal_authorities/legal_authority_passages'
-- shape (composite FOREIGN KEY(entry_id,matter_id) REFERENCES <table>(id,matter_id),
-- the codebase's standard cross-matter-leak guard). A ledger entry records what a
-- cited document SAYS, verified by a lawyer against the actual source text - never a
-- legal conclusion TAHRIR itself asserts. liability_facts is named/framed as a ledger
-- of grounded facts bearing on liability, not a determination.
--
-- Lifecycle is draft -> verified, correction-by-supersession (never a status
-- mutation): a verified row's content is made immutable at the DB level via an
-- UPDATE-blocking trigger, same rigor as damage_calculations/damage_inputs (stricter
-- than legal_authority_passages, which has no trigger at all on its child table - a
-- gap deliberately not repeated here for the source tables' INSERT/UPDATE). A
-- correction to a verified entry INSERTs a brand-new row whose supersedes_entry_id
-- points back at the old one; the old row's content is never touched. "Is this entry
-- superseded" is computed at read time (does any other verified row in this table
-- have supersedes_entry_id = my id?), not persisted - matching the relevance/priority
-- and default-workstream idiom already established in this schema.
--
-- Deliberately NO delete-blocking trigger (unlike damage_calculations): SQLite fires a
-- child row's own BEFORE DELETE trigger even when the row is being removed by an
-- ON DELETE CASCADE from its parent FK (verified empirically), so a no-delete trigger
-- here would make deleting a matter with any verified ledger entry raise ABORT instead
-- of cascading - breaking a real, exposed feature (delete_matter) for no real benefit,
-- since this app never exposes a "delete a ledger entry" command in the first place.
-- The UPDATE-blocking triggers below are what actually enforces "a correction is never
-- a silent edit"; they are unaffected by this, since UPDATE is never part of a cascade.
--
-- The parent UPDATE-blocking trigger carves out `stale`: scanner.rs's staleness
-- cascade must still be able to flip a verified entry's `stale` flag to 1 when its
-- cited document changes underneath it, without opening the door to any other field
-- changing on a verified row (mirroring legal_rulesets' one explicit carved-out
-- approved->superseded transition, which similarly permits only specific columns to
-- change once a row is terminal).
--
-- `stale` is a separate boolean, cascaded from scanner.rs::rehash_changed_versions the
-- same way verified_facts.stale already is, when a cited document_version changes
-- underneath a verified entry.

CREATE TABLE IF NOT EXISTS medical_events(
 id TEXT PRIMARY KEY,
 matter_id TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
 event_date TEXT,
 provider_name TEXT,
 treatment_summary TEXT NOT NULL,
 status TEXT NOT NULL DEFAULT 'draft',
 stale INTEGER NOT NULL DEFAULT 0,
 supersedes_entry_id TEXT,
 integrity_sha256 TEXT,
 verified_at TEXT,
 created_at TEXT NOT NULL,
 updated_at TEXT NOT NULL,
 FOREIGN KEY(supersedes_entry_id,matter_id) REFERENCES medical_events(id,matter_id),
 UNIQUE(id,matter_id)
);
CREATE TABLE IF NOT EXISTS medical_event_sources(
 id TEXT PRIMARY KEY,
 matter_id TEXT NOT NULL,
 entry_id TEXT NOT NULL,
 document_version_id TEXT NOT NULL,
 document_page_id TEXT NOT NULL,
 display_quote TEXT NOT NULL,
 source_text_sha256 TEXT NOT NULL,
 FOREIGN KEY(entry_id,matter_id) REFERENCES medical_events(id,matter_id) ON DELETE CASCADE,
 FOREIGN KEY(document_version_id,matter_id) REFERENCES document_versions(id,matter_id),
 FOREIGN KEY(document_page_id) REFERENCES document_pages(id)
);
CREATE INDEX IF NOT EXISTS idx_medical_events_matter ON medical_events(matter_id,status);

CREATE TABLE IF NOT EXISTS wage_records(
 id TEXT PRIMARY KEY,
 matter_id TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
 period_start TEXT,
 period_end TEXT,
 employer_name TEXT,
 gross_amount_cents INTEGER NOT NULL DEFAULT 0 CHECK(gross_amount_cents>=0),
 status TEXT NOT NULL DEFAULT 'draft',
 stale INTEGER NOT NULL DEFAULT 0,
 supersedes_entry_id TEXT,
 integrity_sha256 TEXT,
 verified_at TEXT,
 created_at TEXT NOT NULL,
 updated_at TEXT NOT NULL,
 FOREIGN KEY(supersedes_entry_id,matter_id) REFERENCES wage_records(id,matter_id),
 UNIQUE(id,matter_id)
);
CREATE TABLE IF NOT EXISTS wage_record_sources(
 id TEXT PRIMARY KEY,
 matter_id TEXT NOT NULL,
 entry_id TEXT NOT NULL,
 document_version_id TEXT NOT NULL,
 document_page_id TEXT NOT NULL,
 display_quote TEXT NOT NULL,
 source_text_sha256 TEXT NOT NULL,
 FOREIGN KEY(entry_id,matter_id) REFERENCES wage_records(id,matter_id) ON DELETE CASCADE,
 FOREIGN KEY(document_version_id,matter_id) REFERENCES document_versions(id,matter_id),
 FOREIGN KEY(document_page_id) REFERENCES document_pages(id)
);
CREATE INDEX IF NOT EXISTS idx_wage_records_matter ON wage_records(matter_id,status);

CREATE TABLE IF NOT EXISTS liability_facts(
 id TEXT PRIMARY KEY,
 matter_id TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
 claim_basis TEXT,
 liable_party_name TEXT,
 description TEXT NOT NULL,
 status TEXT NOT NULL DEFAULT 'draft',
 stale INTEGER NOT NULL DEFAULT 0,
 supersedes_entry_id TEXT,
 integrity_sha256 TEXT,
 verified_at TEXT,
 created_at TEXT NOT NULL,
 updated_at TEXT NOT NULL,
 FOREIGN KEY(supersedes_entry_id,matter_id) REFERENCES liability_facts(id,matter_id),
 UNIQUE(id,matter_id)
);
CREATE TABLE IF NOT EXISTS liability_fact_sources(
 id TEXT PRIMARY KEY,
 matter_id TEXT NOT NULL,
 entry_id TEXT NOT NULL,
 document_version_id TEXT NOT NULL,
 document_page_id TEXT NOT NULL,
 display_quote TEXT NOT NULL,
 source_text_sha256 TEXT NOT NULL,
 FOREIGN KEY(entry_id,matter_id) REFERENCES liability_facts(id,matter_id) ON DELETE CASCADE,
 FOREIGN KEY(document_version_id,matter_id) REFERENCES document_versions(id,matter_id),
 FOREIGN KEY(document_page_id) REFERENCES document_pages(id)
);
CREATE INDEX IF NOT EXISTS idx_liability_facts_matter ON liability_facts(matter_id,status);

CREATE TRIGGER IF NOT EXISTS trg_verified_medical_event_no_update BEFORE UPDATE ON medical_events
WHEN OLD.status='verified' AND (
 NEW.status IS NOT OLD.status OR NEW.event_date IS NOT OLD.event_date OR
 NEW.provider_name IS NOT OLD.provider_name OR NEW.treatment_summary IS NOT OLD.treatment_summary OR
 NEW.supersedes_entry_id IS NOT OLD.supersedes_entry_id OR NEW.integrity_sha256 IS NOT OLD.integrity_sha256 OR
 NEW.verified_at IS NOT OLD.verified_at OR NEW.updated_at IS NOT OLD.updated_at
) BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_verified_medical_source_no_insert BEFORE INSERT ON medical_event_sources
WHEN EXISTS(SELECT 1 FROM medical_events e WHERE e.id=NEW.entry_id AND e.status='verified')
BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_verified_medical_source_no_update BEFORE UPDATE ON medical_event_sources
WHEN EXISTS(SELECT 1 FROM medical_events e WHERE e.id=OLD.entry_id AND e.status='verified')
BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;

CREATE TRIGGER IF NOT EXISTS trg_verified_wage_record_no_update BEFORE UPDATE ON wage_records
WHEN OLD.status='verified' AND (
 NEW.status IS NOT OLD.status OR NEW.period_start IS NOT OLD.period_start OR
 NEW.period_end IS NOT OLD.period_end OR NEW.employer_name IS NOT OLD.employer_name OR
 NEW.gross_amount_cents IS NOT OLD.gross_amount_cents OR
 NEW.supersedes_entry_id IS NOT OLD.supersedes_entry_id OR NEW.integrity_sha256 IS NOT OLD.integrity_sha256 OR
 NEW.verified_at IS NOT OLD.verified_at OR NEW.updated_at IS NOT OLD.updated_at
) BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_verified_wage_source_no_insert BEFORE INSERT ON wage_record_sources
WHEN EXISTS(SELECT 1 FROM wage_records e WHERE e.id=NEW.entry_id AND e.status='verified')
BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_verified_wage_source_no_update BEFORE UPDATE ON wage_record_sources
WHEN EXISTS(SELECT 1 FROM wage_records e WHERE e.id=OLD.entry_id AND e.status='verified')
BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;

CREATE TRIGGER IF NOT EXISTS trg_verified_liability_fact_no_update BEFORE UPDATE ON liability_facts
WHEN OLD.status='verified' AND (
 NEW.status IS NOT OLD.status OR NEW.claim_basis IS NOT OLD.claim_basis OR
 NEW.liable_party_name IS NOT OLD.liable_party_name OR NEW.description IS NOT OLD.description OR
 NEW.supersedes_entry_id IS NOT OLD.supersedes_entry_id OR NEW.integrity_sha256 IS NOT OLD.integrity_sha256 OR
 NEW.verified_at IS NOT OLD.verified_at OR NEW.updated_at IS NOT OLD.updated_at
) BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_verified_liability_source_no_insert BEFORE INSERT ON liability_fact_sources
WHEN EXISTS(SELECT 1 FROM liability_facts e WHERE e.id=NEW.entry_id AND e.status='verified')
BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_verified_liability_source_no_update BEFORE UPDATE ON liability_fact_sources
WHEN EXISTS(SELECT 1 FROM liability_facts e WHERE e.id=OLD.entry_id AND e.status='verified')
BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;

PRAGMA user_version = 17;
