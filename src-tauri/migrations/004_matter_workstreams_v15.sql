-- Phase B, milestone B2: Workstreams + Matter Packs. Additive to 001-003, same
-- discipline: every statement here is re-run via execute_batch on every
-- DbState::open() call, so everything is CREATE ... IF NOT EXISTS.
--
-- No DB CHECK on kind/status: validated in src-tauri/src/workstreams.rs only,
-- matching how matter_profile.rs already validates case_type/party role/entity_kind
-- (and how matters.status/documents.category have never had a CHECK either) - lets
-- the workstream kind/status taxonomy evolve without a schema migration.
--
-- UNIQUE(matter_id,kind) makes seeding/reconciling idempotent via
-- INSERT ... ON CONFLICT(matter_id,kind) DO NOTHING/DO UPDATE.

CREATE TABLE IF NOT EXISTS matter_workstreams(
 id TEXT PRIMARY KEY,
 matter_id TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
 kind TEXT NOT NULL,
 status TEXT NOT NULL DEFAULT 'not_started',
 notes TEXT,
 created_at TEXT NOT NULL,
 updated_at TEXT NOT NULL,
 UNIQUE(matter_id,kind)
);
CREATE INDEX IF NOT EXISTS idx_matter_workstreams_matter ON matter_workstreams(matter_id);

PRAGMA user_version = 15;
