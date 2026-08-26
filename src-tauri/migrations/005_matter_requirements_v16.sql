-- Phase B, milestone B3: Missing Evidence Matrix. Additive to 001-004, same
-- discipline: every statement here is re-run via execute_batch on every
-- DbState::open() call, so everything is CREATE ... IF NOT EXISTS.
--
-- No DB CHECK on requirement_key/status: validated in
-- src-tauri/src/requirements.rs only, matching matter_workstreams. These are
-- office-workflow checklist recommendations, never a statutory requirement -
-- nothing here claims legal force.
--
-- UNIQUE(matter_id,requirement_key) makes seeding/reconciling idempotent via
-- INSERT ... ON CONFLICT(matter_id,requirement_key) DO NOTHING/DO UPDATE.

CREATE TABLE IF NOT EXISTS matter_requirements(
 id TEXT PRIMARY KEY,
 matter_id TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
 requirement_key TEXT NOT NULL,
 status TEXT NOT NULL DEFAULT 'not_applicable',
 notes TEXT,
 created_at TEXT NOT NULL,
 updated_at TEXT NOT NULL,
 UNIQUE(matter_id,requirement_key)
);
CREATE INDEX IF NOT EXISTS idx_matter_requirements_matter ON matter_requirements(matter_id);

PRAGMA user_version = 16;
