PRAGMA foreign_keys = ON;

-- Phase C, milestone C5: Action Orchestrator / Matter Agent Core.
--
-- Two genuinely new pieces of persistent domain state, per the C5 spec - this
-- is the first milestone since 001-008 where a migration is actually
-- justified (C2/C3/C4 stored everything as free-TEXT ai_proposals rows and
-- needed nothing new):
--
-- (1) Deadline satisfaction lifecycle. `legal_deadlines` today only ever
--     moves draft -> committed (`commit_deadline`) or draft|committed ->
--     superseded (`supersede_deadline`, which transactionally creates a
--     fresh draft replacement). There is no "the lawyer actually handled
--     this" transition anywhere in production code - a `state` column with
--     no DB-level CHECK (validated in Rust only, same as today), so this
--     migration adds no CHECK either; commands.rs enforces that `satisfied`
--     is reachable only from `committed`. `due_at`, `committed_at`,
--     `trigger_source_ref`, `rule_id`, `calculation_snapshot_json` etc. are
--     all left completely untouched - satisfying a deadline is a pure
--     additive audit stamp (satisfied_at + optional satisfaction_note), never
--     a rewrite of the deadline's own legal/audit history. A satisfied
--     deadline still shows up in every existing list_deadlines query exactly
--     as before; only the new action_engine excludes it from active/overdue
--     candidate generation.
--
-- (2) Recommendation lifecycle. Action candidates are computed, not stored -
--     the underlying TAHRIR state (deadlines/tasks/workstreams/requirements/
--     waiting_for/ledgers/ai_proposals) IS the memory, per the spec's
--     explicit instruction not to duplicate it into an agent database. The
--     only new persistent fact is a human's *response* to a candidate
--     (acknowledge/snooze/dismiss/convert-to-task), keyed by a deterministic
--     fingerprint of the underlying actionable state so the same real-world
--     condition always maps to the same row, and a genuinely new condition
--     (different fingerprint) is never hidden by a stale response to an old
--     one.

-- Same discipline established in 003_matter_profile_v14.sql: every statement
-- here is re-run via execute_batch on every DbState::open() call, and
-- `ALTER TABLE ADD COLUMN` is not idempotent in SQLite (a second run fails
-- with "duplicate column name"). So the satisfaction audit stamp is a new
-- side table, one row per satisfied deadline, rather than new columns on
-- legal_deadlines - `legal_deadlines` itself is not altered at all.
-- `legal_deadlines.state` moving to 'satisfied' needs no schema change: the
-- column has never had a DB-level CHECK constraint (validated in Rust only,
-- same as every other state transition on this table today).
CREATE TABLE IF NOT EXISTS legal_deadline_satisfaction (
  deadline_id TEXT PRIMARY KEY,
  matter_id TEXT NOT NULL,
  satisfied_at TEXT NOT NULL,
  satisfaction_note TEXT,
  FOREIGN KEY (deadline_id) REFERENCES legal_deadlines(id),
  FOREIGN KEY (matter_id) REFERENCES matters(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_legal_deadline_satisfaction_matter
  ON legal_deadline_satisfaction(matter_id);

CREATE TABLE IF NOT EXISTS action_recommendations (
  id TEXT PRIMARY KEY,
  matter_id TEXT NOT NULL,
  fingerprint TEXT NOT NULL,
  action_code TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'active'
    CHECK (state IN ('active','acknowledged','snoozed','dismissed','converted_to_task')),
  snoozed_until TEXT,
  dismissed_at TEXT,
  acknowledged_at TEXT,
  converted_task_id TEXT,
  converted_at TEXT,
  first_seen_at TEXT NOT NULL,
  last_seen_at TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (matter_id) REFERENCES matters(id) ON DELETE CASCADE,
  FOREIGN KEY (converted_task_id) REFERENCES tasks(id),
  UNIQUE(matter_id, fingerprint)
);

CREATE INDEX IF NOT EXISTS idx_action_recommendations_matter
  ON action_recommendations(matter_id, state, updated_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_action_recommendations_fingerprint
  ON action_recommendations(matter_id, fingerprint);

-- Recommendation-state rows are a human decision trail (who dismissed/
-- snoozed what, and when) - append-only in spirit like the B7 negotiation
-- ledgers: a state row's *effect* (state/snoozed_until/dismissed_at/
-- acknowledged_at/converted_task_id/converted_at) may only be updated by the
-- same explicit human actions this migration models, never silently
-- rewritten. Unlike the B7 ledgers this table intentionally allows UPDATE
-- (state transitions are the whole point, e.g. snoozed -> active once the
-- snooze expires and the fingerprint resurfaces, or active -> dismissed),
-- so no append-only/no-update trigger is added here; what IS guarded is
-- deletion, matching every other matter-scoped table's delete-guard
-- discipline used throughout this schema.
CREATE TRIGGER IF NOT EXISTS trg_action_recommendations_no_delete
BEFORE DELETE ON action_recommendations
WHEN (SELECT active FROM ledger_delete_guard WHERE id = 1) = 0
BEGIN
  SELECT RAISE(ABORT, 'ACTION_RECOMMENDATION_APPEND_ONLY');
END;

PRAGMA user_version = 20;
