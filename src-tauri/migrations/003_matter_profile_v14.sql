-- Phase B, milestone B1: Matter Profile. Additive to 001/002, same discipline: every
-- statement here is re-run via execute_batch on every DbState::open() call, so
-- everything is CREATE ... IF NOT EXISTS. No existing table is altered - ALTER TABLE
-- ADD COLUMN is not idempotent in SQLite (a second run fails with "duplicate column
-- name"), which would break that re-apply-on-every-launch invariant. matters.matter_type
-- already exists and is exactly the right concept for case type; it stays untouched at
-- the schema level and is only tightened with a Rust-side allowlist (matter_profile.rs),
-- the same pattern damage.rs already uses for regime/life_state/input keys.

CREATE TABLE IF NOT EXISTS matter_profile(
 matter_id TEXT PRIMARY KEY REFERENCES matters(id) ON DELETE CASCADE,
 event_date TEXT,
 court_name TEXT,
 btl_claim_number TEXT,
 case_summary TEXT,
 updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS matter_parties(
 id TEXT PRIMARY KEY,
 matter_id TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
 role TEXT NOT NULL CHECK(role IN (
  'client','party','witness','employer','insurer',
  'medical_institution','expert','opposing_counsel','court'
 )),
 name TEXT NOT NULL,
 contact_details TEXT,
 notes TEXT,
 created_at TEXT NOT NULL,
 updated_at TEXT NOT NULL,
 UNIQUE(id,matter_id)
);
CREATE INDEX IF NOT EXISTS idx_matter_parties_matter ON matter_parties(matter_id);

PRAGMA user_version = 14;
