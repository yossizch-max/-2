-- Phase B, milestone B1: Matter Profile. Additive to 001/002, same discipline: every
-- statement here is re-run via execute_batch on every DbState::open() call, so
-- everything is CREATE ... IF NOT EXISTS. No existing table is altered - ALTER TABLE
-- ADD COLUMN is not idempotent in SQLite (a second run fails with "duplicate column
-- name"), which would break that re-apply-on-every-launch invariant. matters.matter_type
-- already exists and is exactly the right concept for case type; it stays untouched at
-- the schema level and is only tightened with a Rust-side allowlist (matter_profile.rs),
-- the same pattern damage.rs already uses for regime/life_state/input keys.
--
-- Reworked before any real Windows CI run confirmed this shape (no client-use database
-- depends on the earlier column names, so editing this file in place - not adding a new
-- migration - mirrors the precedent already set by the Phase A hardening pass, which
-- rewrote 002 in place for the same reason): a review of the B1 plan flagged three
-- issues fixed here.
--  - event_date/court_name renamed to primary_event_date/primary_court_name: a tort
--    matter will accumulate many dates (accident, discovery of harm, hospitalization,
--    notice, service, filing...) and often more than one proceeding/court - the
--    unqualified names would have implied "the" single date/court. A future
--    matter_events/Chronology feature and multi-proceeding tracking are the real home
--    for the rest; this stays the one primary/display value for now.
--  - btl_claim_number stays as a convenience/display field only, not a BTL/insurer
--    model - a matter can in practice have several BTL claims or insurer claim
--    numbers. A proper matter_external_references (kind/value/label) table is deferred
--    until that's actually needed.
--  - matter_parties.role has no DB CHECK: validated in matter_profile.rs only, matching
--    how most enum-shaped columns in this codebase already work (matters.status,
--    documents.category have no CHECK either) - lets the role/entity_kind taxonomy
--    evolve without a schema migration. contact_details (a single free-text blob) is
--    replaced with structured contact fields, since a real Contacts feature is already
--    on the Phase B roadmap and a blob would just need to be parsed back apart later.

CREATE TABLE IF NOT EXISTS matter_profile(
 matter_id TEXT PRIMARY KEY REFERENCES matters(id) ON DELETE CASCADE,
 primary_event_date TEXT,
 primary_court_name TEXT,
 btl_claim_number TEXT,
 case_summary TEXT,
 updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS matter_parties(
 id TEXT PRIMARY KEY,
 matter_id TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
 role TEXT NOT NULL,
 display_name TEXT NOT NULL,
 entity_kind TEXT NOT NULL DEFAULT 'unknown',
 identifier TEXT,
 phone TEXT,
 email TEXT,
 address TEXT,
 notes TEXT,
 created_at TEXT NOT NULL,
 updated_at TEXT NOT NULL,
 UNIQUE(id,matter_id)
);
CREATE INDEX IF NOT EXISTS idx_matter_parties_matter ON matter_parties(matter_id);

PRAGMA user_version = 14;
