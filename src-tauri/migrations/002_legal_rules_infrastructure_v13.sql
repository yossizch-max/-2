-- Legal rules infrastructure (Phase A). Governs HOW a deterministic legal rule may be
-- authored, sourced, tested, approved and executed - it deliberately contains NO
-- Israeli substantive law. A ruleset only becomes usable for a committed/locked legal
-- result once it is 'approved', which itself requires at least one verified source and
-- at least one approved, passing test case (enforced in src-tauri/src/legal_rules.rs,
-- not just here - see that module for the actual approval gate). This file is additive
-- to 001_schema_v12.sql and re-applied every launch like it, so every statement here
-- must also tolerate running against an already-initialized database.

CREATE TABLE IF NOT EXISTS legal_rulesets(
 id TEXT PRIMARY KEY, engine_kind TEXT NOT NULL, jurisdiction TEXT NOT NULL,
 title TEXT NOT NULL, version TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'draft',
 effective_from TEXT, effective_to TEXT, description TEXT,
 created_at TEXT NOT NULL, created_by TEXT,
 submitted_for_review_at TEXT, approved_at TEXT, approved_by TEXT,
 superseded_by TEXT REFERENCES legal_rulesets(id),
 integrity_sha256 TEXT,
 CHECK(status IN ('draft','under_review','approved','superseded','revoked')),
 UNIQUE(engine_kind,jurisdiction,version)
);

CREATE TABLE IF NOT EXISTS legal_ruleset_sources(
 id TEXT PRIMARY KEY, ruleset_id TEXT NOT NULL REFERENCES legal_rulesets(id) ON DELETE CASCADE,
 source_kind TEXT NOT NULL, citation TEXT NOT NULL, pinpoint TEXT,
 document_version_id TEXT, document_page_id TEXT,
 source_sha256 TEXT NOT NULL, verified_at TEXT, verified_by TEXT,
 created_at TEXT NOT NULL,
 CHECK(source_kind IN ('legislation','regulation','judgment','official_guidance','internal_legal_memo'))
);

CREATE TABLE IF NOT EXISTS legal_rules(
 id TEXT PRIMARY KEY, ruleset_id TEXT NOT NULL REFERENCES legal_rulesets(id) ON DELETE CASCADE,
 rule_key TEXT NOT NULL, rule_type TEXT NOT NULL, priority INTEGER NOT NULL DEFAULT 0,
 conditions_json TEXT NOT NULL, operation_json TEXT NOT NULL, explanation_template TEXT,
 source_id TEXT REFERENCES legal_ruleset_sources(id),
 created_at TEXT NOT NULL,
 UNIQUE(ruleset_id,rule_key)
);

CREATE TABLE IF NOT EXISTS legal_rule_test_cases(
 id TEXT PRIMARY KEY, ruleset_id TEXT NOT NULL REFERENCES legal_rulesets(id) ON DELETE CASCADE,
 name TEXT NOT NULL, input_json TEXT NOT NULL, expected_output_json TEXT NOT NULL,
 review_status TEXT NOT NULL DEFAULT 'draft', reviewed_by TEXT, reviewed_at TEXT,
 created_at TEXT NOT NULL,
 CHECK(review_status IN ('draft','approved','rejected'))
);

CREATE TABLE IF NOT EXISTS legal_engine_runs(
 id TEXT PRIMARY KEY, matter_id TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
 engine_kind TEXT NOT NULL, ruleset_id TEXT NOT NULL REFERENCES legal_rulesets(id),
 ruleset_version TEXT NOT NULL,
 input_snapshot_json TEXT NOT NULL, result_json TEXT NOT NULL, trace_json TEXT NOT NULL,
 ruleset_integrity_sha256 TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'proposed',
 created_at TEXT NOT NULL, reviewed_at TEXT, review_note TEXT,
 CHECK(status IN ('proposed','reviewed','committed','locked'))
);

CREATE INDEX IF NOT EXISTS idx_rulesets_engine ON legal_rulesets(engine_kind,jurisdiction,status);
CREATE INDEX IF NOT EXISTS idx_ruleset_sources_ruleset ON legal_ruleset_sources(ruleset_id);
CREATE INDEX IF NOT EXISTS idx_rules_ruleset ON legal_rules(ruleset_id,priority);
CREATE INDEX IF NOT EXISTS idx_rule_test_cases_ruleset ON legal_rule_test_cases(ruleset_id);
CREATE INDEX IF NOT EXISTS idx_engine_runs_matter ON legal_engine_runs(matter_id,engine_kind,created_at);

-- An approved ruleset (and everything that composes it) is immutable at the DB level,
-- mirroring the existing legal_document_* / legal_authorities immutability pattern.
-- Superseding a ruleset creates a NEW row and points the old one's superseded_by at it
-- (still an UPDATE on the old, already-approved row) - so that one specific transition
-- is allowed even while approved, everything else is not.
CREATE TRIGGER IF NOT EXISTS trg_approved_ruleset_no_update BEFORE UPDATE ON legal_rulesets
WHEN OLD.status='approved' AND NOT (
 NEW.status='superseded' AND NEW.superseded_by IS NOT NULL AND
 NEW.id IS OLD.id AND NEW.engine_kind IS OLD.engine_kind AND NEW.jurisdiction IS OLD.jurisdiction AND
 NEW.title IS OLD.title AND NEW.version IS OLD.version AND NEW.integrity_sha256 IS OLD.integrity_sha256
)
BEGIN SELECT RAISE(ABORT,'APPROVED_RULESET_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_approved_ruleset_no_delete BEFORE DELETE ON legal_rulesets
WHEN OLD.status IN ('approved','superseded') BEGIN SELECT RAISE(ABORT,'APPROVED_RULESET_IMMUTABLE'); END;

CREATE TRIGGER IF NOT EXISTS trg_approved_ruleset_sources_no_insert BEFORE INSERT ON legal_ruleset_sources
WHEN EXISTS(SELECT 1 FROM legal_rulesets r WHERE r.id=NEW.ruleset_id AND r.status='approved')
BEGIN SELECT RAISE(ABORT,'APPROVED_RULESET_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_approved_ruleset_sources_no_update BEFORE UPDATE ON legal_ruleset_sources
WHEN EXISTS(SELECT 1 FROM legal_rulesets r WHERE r.id=OLD.ruleset_id AND r.status='approved')
BEGIN SELECT RAISE(ABORT,'APPROVED_RULESET_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_approved_ruleset_sources_no_delete BEFORE DELETE ON legal_ruleset_sources
WHEN EXISTS(SELECT 1 FROM legal_rulesets r WHERE r.id=OLD.ruleset_id AND r.status='approved')
BEGIN SELECT RAISE(ABORT,'APPROVED_RULESET_IMMUTABLE'); END;

CREATE TRIGGER IF NOT EXISTS trg_approved_rules_no_insert BEFORE INSERT ON legal_rules
WHEN EXISTS(SELECT 1 FROM legal_rulesets r WHERE r.id=NEW.ruleset_id AND r.status='approved')
BEGIN SELECT RAISE(ABORT,'APPROVED_RULESET_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_approved_rules_no_update BEFORE UPDATE ON legal_rules
WHEN EXISTS(SELECT 1 FROM legal_rulesets r WHERE r.id=OLD.ruleset_id AND r.status='approved')
BEGIN SELECT RAISE(ABORT,'APPROVED_RULESET_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_approved_rules_no_delete BEFORE DELETE ON legal_rules
WHEN EXISTS(SELECT 1 FROM legal_rulesets r WHERE r.id=OLD.ruleset_id AND r.status='approved')
BEGIN SELECT RAISE(ABORT,'APPROVED_RULESET_IMMUTABLE'); END;

CREATE TRIGGER IF NOT EXISTS trg_approved_rule_test_cases_no_insert BEFORE INSERT ON legal_rule_test_cases
WHEN EXISTS(SELECT 1 FROM legal_rulesets r WHERE r.id=NEW.ruleset_id AND r.status='approved')
BEGIN SELECT RAISE(ABORT,'APPROVED_RULESET_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_approved_rule_test_cases_no_update BEFORE UPDATE ON legal_rule_test_cases
WHEN EXISTS(SELECT 1 FROM legal_rulesets r WHERE r.id=OLD.ruleset_id AND r.status='approved')
BEGIN SELECT RAISE(ABORT,'APPROVED_RULESET_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_approved_rule_test_cases_no_delete BEFORE DELETE ON legal_rule_test_cases
WHEN EXISTS(SELECT 1 FROM legal_rulesets r WHERE r.id=OLD.ruleset_id AND r.status='approved')
BEGIN SELECT RAISE(ABORT,'APPROVED_RULESET_IMMUTABLE'); END;

-- legal_engine_runs is an immutable calculation trace once written: its snapshot,
-- result, trace and the ruleset hash it ran against may never change. status may still
-- progress forward (proposed -> reviewed -> committed -> locked) plus reviewed_at/
-- review_note, so only those columns are allowed to differ from OLD.
CREATE TRIGGER IF NOT EXISTS trg_engine_run_no_update BEFORE UPDATE ON legal_engine_runs
WHEN NEW.input_snapshot_json IS NOT OLD.input_snapshot_json
 OR NEW.result_json IS NOT OLD.result_json
 OR NEW.trace_json IS NOT OLD.trace_json
 OR NEW.ruleset_integrity_sha256 IS NOT OLD.ruleset_integrity_sha256
 OR NEW.ruleset_id IS NOT OLD.ruleset_id
 OR NEW.ruleset_version IS NOT OLD.ruleset_version
 OR NEW.matter_id IS NOT OLD.matter_id
 OR NEW.engine_kind IS NOT OLD.engine_kind
BEGIN SELECT RAISE(ABORT,'ENGINE_RUN_SNAPSHOT_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_engine_run_no_delete BEFORE DELETE ON legal_engine_runs
BEGIN SELECT RAISE(ABORT,'ENGINE_RUN_SNAPSHOT_IMMUTABLE'); END;

PRAGMA user_version = 13;
