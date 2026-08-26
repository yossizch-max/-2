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
-- gap deliberately not repeated here for the source tables' INSERT/UPDATE/DELETE). A
-- correction to a verified entry INSERTs a brand-new row whose supersedes_entry_id
-- points back at the old one; the old row's content is never touched. "Is this entry
-- superseded" is computed at read time (does any other verified row in this table
-- have supersedes_entry_id = my id?), not persisted - matching the relevance/priority
-- and default-workstream idiom already established in this schema. A partial unique
-- index (matter_id, supersedes_entry_id) WHERE status='verified' guarantees an old
-- entry can only ever have ONE verified successor - concurrent draft corrections may
-- exist, but only the first one to verify wins; a second verify attempt on a
-- different draft correction of the same entry is rejected (checked in Rust first,
-- with this index as the DB-level backstop against a race or direct SQL bypass).
--
-- `stale` is a separate boolean, cascaded from scanner.rs::rehash_changed_versions the
-- same way verified_facts.stale already is, when a cited document_version changes
-- underneath a verified entry. The parent UPDATE-blocking trigger carves out `stale`
-- for exactly that reason - but only 0->1: once a verified entry has gone stale, SQL
-- cannot silently reset it back to 0 (that would let a row claim "still trustworthy"
-- without ever being re-verified against fresh source text); the only way to move a
-- stale, verified entry forward again is a fresh supersession, never a bit flip.
--
-- Verified rows and their sources are also protected against DELETE, not just UPDATE -
-- but a whole-matter deletion (matters.id ON DELETE CASCADE) must still be able to
-- cascade through them. SQLite fires a child row's own BEFORE DELETE trigger even when
-- the row is being removed by an ON DELETE CASCADE from its parent FK (verified
-- empirically), so a plain unconditional no-delete trigger would make deleting a
-- matter with any verified ledger entry raise ABORT instead of cascading. The
-- ledger_delete_guard control table (below) is the deliberate escape hatch: any code
-- path that intentionally deletes a whole matter must flip it on for the duration of
-- that single statement (see ledger::with_cascade_delete_guard in ledger.rs) - any
-- other DELETE against a verified row or its sources is rejected.

CREATE TABLE IF NOT EXISTS ledger_delete_guard(
 id INTEGER PRIMARY KEY CHECK(id=1),
 active INTEGER NOT NULL DEFAULT 0
);
INSERT OR IGNORE INTO ledger_delete_guard(id,active) VALUES(1,0);

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
CREATE UNIQUE INDEX IF NOT EXISTS idx_medical_events_one_verified_successor
ON medical_events(matter_id,supersedes_entry_id) WHERE status='verified' AND supersedes_entry_id IS NOT NULL;

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
CREATE UNIQUE INDEX IF NOT EXISTS idx_wage_records_one_verified_successor
ON wage_records(matter_id,supersedes_entry_id) WHERE status='verified' AND supersedes_entry_id IS NOT NULL;

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
CREATE UNIQUE INDEX IF NOT EXISTS idx_liability_facts_one_verified_successor
ON liability_facts(matter_id,supersedes_entry_id) WHERE status='verified' AND supersedes_entry_id IS NOT NULL;

CREATE TRIGGER IF NOT EXISTS trg_verified_medical_event_no_update BEFORE UPDATE ON medical_events
WHEN OLD.status='verified' AND (
 NEW.status IS NOT OLD.status OR NEW.event_date IS NOT OLD.event_date OR
 NEW.provider_name IS NOT OLD.provider_name OR NEW.treatment_summary IS NOT OLD.treatment_summary OR
 NEW.supersedes_entry_id IS NOT OLD.supersedes_entry_id OR NEW.integrity_sha256 IS NOT OLD.integrity_sha256 OR
 NEW.verified_at IS NOT OLD.verified_at OR NEW.updated_at IS NOT OLD.updated_at OR
 (OLD.stale=1 AND NEW.stale=0)
) BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_verified_medical_event_no_delete BEFORE DELETE ON medical_events
WHEN OLD.status='verified' AND (SELECT active FROM ledger_delete_guard WHERE id=1)=0
BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_verified_medical_source_no_insert BEFORE INSERT ON medical_event_sources
WHEN EXISTS(SELECT 1 FROM medical_events e WHERE e.id=NEW.entry_id AND e.status='verified')
BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_verified_medical_source_no_update BEFORE UPDATE ON medical_event_sources
WHEN EXISTS(SELECT 1 FROM medical_events e WHERE e.id=OLD.entry_id AND e.status='verified')
BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_verified_medical_source_no_delete BEFORE DELETE ON medical_event_sources
WHEN EXISTS(SELECT 1 FROM medical_events e WHERE e.id=OLD.entry_id AND e.status='verified')
 AND (SELECT active FROM ledger_delete_guard WHERE id=1)=0
BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;

CREATE TRIGGER IF NOT EXISTS trg_verified_wage_record_no_update BEFORE UPDATE ON wage_records
WHEN OLD.status='verified' AND (
 NEW.status IS NOT OLD.status OR NEW.period_start IS NOT OLD.period_start OR
 NEW.period_end IS NOT OLD.period_end OR NEW.employer_name IS NOT OLD.employer_name OR
 NEW.gross_amount_cents IS NOT OLD.gross_amount_cents OR
 NEW.supersedes_entry_id IS NOT OLD.supersedes_entry_id OR NEW.integrity_sha256 IS NOT OLD.integrity_sha256 OR
 NEW.verified_at IS NOT OLD.verified_at OR NEW.updated_at IS NOT OLD.updated_at OR
 (OLD.stale=1 AND NEW.stale=0)
) BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_verified_wage_record_no_delete BEFORE DELETE ON wage_records
WHEN OLD.status='verified' AND (SELECT active FROM ledger_delete_guard WHERE id=1)=0
BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_verified_wage_source_no_insert BEFORE INSERT ON wage_record_sources
WHEN EXISTS(SELECT 1 FROM wage_records e WHERE e.id=NEW.entry_id AND e.status='verified')
BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_verified_wage_source_no_update BEFORE UPDATE ON wage_record_sources
WHEN EXISTS(SELECT 1 FROM wage_records e WHERE e.id=OLD.entry_id AND e.status='verified')
BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_verified_wage_source_no_delete BEFORE DELETE ON wage_record_sources
WHEN EXISTS(SELECT 1 FROM wage_records e WHERE e.id=OLD.entry_id AND e.status='verified')
 AND (SELECT active FROM ledger_delete_guard WHERE id=1)=0
BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;

CREATE TRIGGER IF NOT EXISTS trg_verified_liability_fact_no_update BEFORE UPDATE ON liability_facts
WHEN OLD.status='verified' AND (
 NEW.status IS NOT OLD.status OR NEW.claim_basis IS NOT OLD.claim_basis OR
 NEW.liable_party_name IS NOT OLD.liable_party_name OR NEW.description IS NOT OLD.description OR
 NEW.supersedes_entry_id IS NOT OLD.supersedes_entry_id OR NEW.integrity_sha256 IS NOT OLD.integrity_sha256 OR
 NEW.verified_at IS NOT OLD.verified_at OR NEW.updated_at IS NOT OLD.updated_at OR
 (OLD.stale=1 AND NEW.stale=0)
) BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_verified_liability_fact_no_delete BEFORE DELETE ON liability_facts
WHEN OLD.status='verified' AND (SELECT active FROM ledger_delete_guard WHERE id=1)=0
BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_verified_liability_source_no_insert BEFORE INSERT ON liability_fact_sources
WHEN EXISTS(SELECT 1 FROM liability_facts e WHERE e.id=NEW.entry_id AND e.status='verified')
BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_verified_liability_source_no_update BEFORE UPDATE ON liability_fact_sources
WHEN EXISTS(SELECT 1 FROM liability_facts e WHERE e.id=OLD.entry_id AND e.status='verified')
BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_verified_liability_source_no_delete BEFORE DELETE ON liability_fact_sources
WHEN EXISTS(SELECT 1 FROM liability_facts e WHERE e.id=OLD.entry_id AND e.status='verified')
 AND (SELECT active FROM ledger_delete_guard WHERE id=1)=0
BEGIN SELECT RAISE(ABORT,'VERIFIED_LEDGER_ENTRY_IMMUTABLE'); END;

PRAGMA user_version = 17;
