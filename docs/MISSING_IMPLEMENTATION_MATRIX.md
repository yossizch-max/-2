# Reconstruction implementation matrix

This document distinguishes reconstructed executable handlers from command contracts that remain intentionally fail-closed.

Total command contract: **69** (the original alpha.16 61-command contract plus
`create_legal_document_version`, `get_legal_document_version`, `fill_legal_document_facts`,
`add_legal_document_paragraph`, `update_legal_document_paragraph`,
`confirm_legal_document_paragraph`, `delete_legal_document_paragraph` and `choose_save_file` —
see "Gaps found and fixed during Gate F", "Legal-document authoring efficiency pass" and
`docs/RELEASE_GATES.md`'s "External audit response" section below)

Wired handlers: **69**

Fail-closed historical endpoints: **0**

All 69 commands in the Tauri contract have real handlers backed by the SQLCipher schema
(`src-tauri/migrations/001_schema_v12.sql`, 33 user tables). None return
`RECONSTRUCTED_COMMAND_NOT_YET_WIRED` any longer.

## Notable scope decisions made while wiring the remaining 36 commands

- `get_settings` / `save_settings` persist a single JSON blob in the new `app_settings` table.
- `list_matter_suggestions` / `bind_existing_matter` / `reject_matter_suggestion` are backed by a
  new `matter_suggestions` table. The scanner now records a suggestion for any top-level
  office-root folder that does not match an active `matter_folder_bindings` path, instead of
  silently dropping those files.
- `choose_folder` opens a native folder picker (`tauri-plugin-dialog`).
- `open_occurrence` / `reveal_occurrence` shell out via `tauri-plugin-opener`
  (`open_path` / `reveal_item_in_dir`) against the occurrence's indexed `path_display`.
- `test_ai_provider` performs local/offline validation only (loopback URL check for local
  providers, fixed-endpoint check for OpenAI) — it does not perform a real network call, matching
  the UI's own "בדיקה סינתטית" (synthetic test) language.
- `export_legal_document` writes a real `.txt` export and a real append-only
  `legal_export_audit` row. `docx`/`pdf` output kinds return `PdfConverterUnavailable` rather than
  faking a conversion, since no Word/LibreOffice converter is wired in this reconstruction —
  consistent with this project's "never fake success" rule.

## Also fixed while wiring these commands

The schema migration (`001_schema_v12.sql`) previously used bare `CREATE TABLE` / `CREATE INDEX` /
`CREATE TRIGGER` statements, executed unconditionally on every app launch
(`DbState::open` in `src-tauri/src/db.rs`). That would have thrown on the **second** launch against
an already-initialized database. All statements are now `IF NOT EXISTS`, and
`db::tests::migration_is_idempotent_across_repeated_app_launches` guards against a regression.

## Gaps found and fixed during Gate F testing

Writing a real, executable Gate F test (`src-tauri/src/gate_f_partial.rs`) — rather than only
reading the code — surfaced two genuine product gaps that a pure code-reading QA pass had missed:

- **No full-text search over extracted document content.** `search::search` only matched
  matter title/internal number, file names and verified-fact text; there was no way to find a
  document by words that actually appear in its extracted `document_pages` text. Fixed by adding
  a `document_pages`-backed search branch (joined through `document_versions`/`documents`, with
  the occurrence file name as a fallback title) returning a new `document_page` hit kind.
- **No way to start a new version of an approved legal document.** `legal_document_versions` rows
  become immutable once approved (by design — `trg_approved_legal_version_no_update`), but no
  command existed to create the next draft version from one, so an approved demand letter or
  claim could never be revised. Fixed by adding `legal_docs::create_new_version` (validates the
  current version is `approved`, deep-copies its sections/paragraphs/sources onto a new
  `draft` version with an incremented `version_number` and `parent_version_id`, and flips the
  parent `legal_documents` row back to `status='draft'`) and the `create_legal_document_version`
  command, wired to a "צור גרסה חדשה" (create new version) button shown on approved documents
  in `LegalDocumentsTab`.

While fixing the second gap, also fixed a related bug directly in its path: `approve_version`
updated `legal_document_versions.status` but never `legal_documents.status`, so the parent
document's status shown in the UI stayed `draft` forever even after approval. `approve_version`
now updates both in the same transaction.

## Legal-document authoring efficiency pass

Requested directly: the pleadings/claims generator (`LegalDocumentsTab`) worked but every
document started from a blank version with no way to populate or edit it beyond the API. Three
changes closed that gap:

- **Fixed section templates per document kind.** `legal_docs::create_draft` now seeds a real
  section skeleton depending on `kind` (`מכתב דרישה`/`כתב תביעה`/`כתב הגנה ותגובה` each get their
  own headings), always ending in a fixed `FACTS_SECTION_HEADING` ("עובדות מאומתות") section that
  the fact autofill below targets.
- **Auto-fill from verified facts.** `legal_docs::fill_from_verified_facts` (command
  `fill_legal_document_facts`) appends one already-`confirmed`, source-grounded paragraph per
  matter fact with `status='valid'` into the facts section — grounded via a real
  `legal_document_sources` row pointing at the `verified_fact_id`, not just copied text. It's
  idempotent: re-running only picks up facts verified since the last run, it never duplicates a
  fact already linked into the version.
- **In-app paragraph editing.** `get_legal_document_version` returns a version's full
  section/paragraph tree; `add_legal_document_paragraph` / `update_legal_document_paragraph` /
  `confirm_legal_document_paragraph` / `delete_legal_document_paragraph` let a lawyer draft, edit,
  approve-for-inclusion and remove paragraphs directly in `LegalDocumentsTab`'s new editor view,
  without leaving the app. All four (and the autofill) reject any version whose `status` isn't
  `draft` in application code. At the time this was written, the schema's immutability triggers
  guarded `legal_document_versions`/`legal_document_sources` but **not**
  `legal_document_paragraphs`/`legal_document_sections`, so the application-level draft-only check
  was the *only* thing stopping an approved version's text from being edited — an external audit
  independently caught this exact gap days later (P0-3, see `docs/RELEASE_GATES.md`'s "External
  audit response") and it's now also enforced by six new DB triggers, not application code alone.
  Editing a paragraph's text always resets its `provenance_state` to `needs_review`, so
  `approve_version`'s all-paragraphs-confirmed gate forces an explicit re-confirm before the next
  approval.

Covered for real by `gate_f_partial.rs`: template seeding, autofill (including its idempotency),
and the add/edit/confirm/delete cycle for a manually-added paragraph. The stronger integrity
properties added in response to the external audit (approved-content DB immutability, fact
grounding requirements, stale/invalidated-fact revalidation at approval, tamper-resistant damage
locking) are covered separately in `src-tauri/src/integrity_tests.rs` — see
`docs/RELEASE_GATES.md` for the full writeup.

## AI review workflow completion (P1-1 from the external audit)

The audit's P1-1 finding: "AI architecture exists, but the lawyer cannot complete the intended
AI → proposal → review → Verified Fact workflow from the UI." Checked directly and confirmed
true in two places, both now fixed:

- `review_ai_proposal`'s `'approved'` path only ever flipped `ai_proposals.status` — no command
  anywhere turned an approved proposal into an actual `VerifiedFact`. Fixed by
  `ai::approve_proposal`: it reads the proposal's `structured_json` (`subject`/`predicate`/
  `value`/`sourceIds`), re-validates every `sourceId` is a real `document_pages` row belonging to
  the same matter (rejecting the proposal otherwise, per `AppError::InvalidSourceReference` — the
  same fail-closed pattern used everywhere else in this codebase), and creates the `VerifiedFact`
  with `created_from_proposal_id` set plus one real `VerifiedFactSource` row per cited page. The
  stored quote is always read back from the actual `document_pages.display_text`, never taken
  from whatever text the model claimed — the AI proposes which source and what to extract, the
  actual quoted text is re-derived from the real document, not trusted from the response.
- The OpenAI settings card displayed `clientDataAuthorized` but had no control to change it, so
  it could only ever be `false` and `run_ai_capability` would always reject an OpenAI run with
  `AiClientEgressNotApproved`. `AISettingsPage` now has a real checkbox for it.
- `FactsAITab` gained the actual run/review UI: pick an enabled provider, run the (single,
  reconstruction-scoped) `extract_facts` capability, see the run's status and its proposals, and
  approve/reject/request-revision each one — approving calls the same `review_ai_proposal`
  command, now with the real effect described above.

Covered for real by `integrity_tests.rs::approving_an_ai_proposal_creates_a_real_grounded_verified_fact`,
which simulates what a real provider response would have produced (a synthetic `ai_proposals` row
in the same shape `ai::run_capability` itself writes) since this reconstruction has no live
provider to test against in this environment — and asserts the fact-creation-with-a-real-source
path, the double-approval rejection, and the ungrounded-`sourceId` rejection all work as described.

## Second-round audit fixes (2026-08-25)

A second external audit pass ("Deep Control v2") found four more P0s and three more P1s against
the fixes above — see `docs/RELEASE_GATES.md`'s "External audit response, round 2" section for the
full writeup: partial-scan safety wasn't actually guaranteed (a silently-discarded WalkDir error
could still trigger mass-marking files missing), a stale AI proposal could still be approved into a
fresh VerifiedFact, the shipped `SOURCE-MANIFEST.json`/`QA_*.json` described an old version of the
source (now regenerated for real via `scripts/generate-source-manifest.mjs`, and from-scratch for
the QA files), a malformed `damage_inputs.value_text` silently became 0 instead of failing, and the
Verified Facts "open source" button had no `onClick`. All fixed; see `RELEASE_GATES.md` for detail
per finding and which ones are covered by new tests.

## Third-round report: UX/product/legal-domain design, tiers 1-2 (2026-08-25)

A third report, reviewing UX completeness and legal-domain maturity rather than data
integrity, found several dead UI paths and hardcoded state — see `docs/RELEASE_GATES.md`'s
"External audit response, round 3" section for the full writeup. Tier 1 (dead navigation on
Today/Action Center/Search, hardcoded `get_app_health`/`Inspector`, a literal "PIP" label, a
CSS column mismatch in the documents table) and Tier 2 (AI review proposals showing source
excerpts instead of just a count, a two-step confirm before committing a deadline) are both
fixed. Tier 3 of that same report (a deterministic deadline rules engine, a versioned Israeli
tort damages ruleset, Medical/Wage/Liability ledgers) is deliberately not attempted — it
requires a real licensed tort lawyer's active involvement to be correct, not something to
encode unilaterally from a report.

## Follow-up engineering fixes: OCR RAII cleanup and authority-passage grounding (2026-08-25)

Two items both audit rounds had explicitly logged as "deliberately not addressed" — see
`docs/RELEASE_GATES.md`'s "Follow-up fixes" section for the full writeup. Neither needed
a legal-domain judgment call, so both were picked up directly: the OCR scratch directory
now cleans up via an RAII guard (`OcrTempDir` in `extraction.rs`) instead of manual
`remove_dir_all` calls that a failed process spawn could bypass; and Verified Authority
now requires at least one *approved passage* — quoted text checked verbatim against the
authority's own source document, re-checked again at approval time — not just a source
document attached to it. `legal_authority_passages` existed in the schema since the
original import but was never wired to any command until now. A new `authorities` module
holds this logic (testable outside `tauri::State`, same pattern as `legal_docs`/`damage`),
and `AuthoritiesTab` gained a passage-management panel.

## Legal rules infrastructure, Phase A (2026-08-25)

A fourth external document specified governed infrastructure for deterministic legal
rules — see `docs/RELEASE_GATES.md`'s "Legal rules infrastructure, Phase A" section for
the full writeup. This is deliberately **not** Israeli substantive law: it's the
machinery (`legal_rulesets`/`legal_ruleset_sources`/`legal_rules`/
`legal_rule_test_cases`/`legal_engine_runs`, a constrained 10-operator DSL interpreter,
a ruleset lifecycle with re-checked-at-approval-time invariants, immutable engine-run
traces, and a Settings UI) that lets a lawyer author, source, test and approve a rule
before it may drive a committed result — with zero legal content encoded by this pass.
Phase B (evidence ledgers) and Phase C (the first real lawyer-approved legal module)
remain deliberately unattempted, consistent with the position taken on the third
report's Tier 3 above.

## Legal rules infrastructure, Phase A hardening (2026-08-25)

A fifth external document audited that Phase A implementation and returned six P0
governance/integrity gaps and five P1 hardening items — see `docs/RELEASE_GATES.md`'s
"Legal rules infrastructure, Phase A hardening" section for the full per-finding
writeup. Every P0 was independently re-verified (live against SQLite for the trigger
findings, by direct code inspection for the rest) before being fixed. Closed in one
pass: superseded rulesets could still be mutated and re-approved (terminal-state
triggers now cover approved/superseded/revoked); ruleset and test-case approval didn't
actually require a reviewer identity (now required, non-empty); a citation-only source
could satisfy approval with no real source text (now requires an exact document page,
hashed from that page's own content); `engine_kind` was a trusted caller input, not
bound to the ruleset (parameter removed entirely); `effective_from`/`effective_to`
existed but were never enforced (now validated and checked against the server clock);
rule priority ties resolved non-deterministically (now `priority, rule_key` ordering
everywhere). Also: approval is now atomic (one transaction, not a check-release-
recheck sequence); the integrity hash binds effective period, explanation templates,
source kind/document/page, and reviewer identity; engine-run status can only move
forward; an empty test expectation no longer passes trivially; the DSL uses exact
integer arithmetic for comparisons/cap/floor instead of `f64`; and a legal-rule-shaped
sample value was removed from the rule-authoring form. P1-6 (linking engine runs to
`legal_deadlines`/`damage_calculations`) is deliberately deferred until that
integration itself is built, per the audit's own framing.

## Phase B, milestone B1 — Matter Profile (2026-08-25)

Three further reports (Israeli civil/tort market research, ledger-lifecycle, and
AI-pipeline deep dives) recommended TAHRIR become an AI-native Case Operating System,
laying out a Phase B roadmap (Matter Profile → Workstreams → Ledgers → AI Pipeline →
Missing Evidence Matrix → Case Health). `matters` had no client/party/insurer/court/
event-date/BTL data at all, and "workstream"/"matter pack"/"missing evidence" had zero
hits in the repo — genuinely greenfield. This pass implements only the roadmap's first
milestone, B1: a new `matter_profile` side table (event date, court, BTL claim number,
case summary) and a `matter_parties` contact list (role-constrained per the market
report's own taxonomy), plus a Rust-side allowlist tightening `matters.matter_type`. No
lock/approval lifecycle — this is plain office-management data, not an evidentiary
claim. See `docs/RELEASE_GATES.md`'s "Phase B, milestone B1" section for the full
writeup, including why `matters` itself is never `ALTER`ed. B2–B6 (Workstreams, Missing
Evidence Matrix, Medical/Wage/Liability Ledgers, AI Pipeline, Case Health) are
deliberately left as a roadmap only — each gets its own planning pass before
implementation.

A design review of this same milestone (before any client-use database depended on its
shape) prompted fixes applied in place to `003_matter_profile_v14.sql` and
`matter_profile.rs`: `event_date`/`court_name` renamed to `primary_event_date`/
`primary_court_name` (a tort matter accumulates many dates and can outgrow one
court/proceeding); `matter_parties.contact_details` (one free-text blob) replaced with
structured `display_name`/`entity_kind`/`identifier`/`phone`/`email`/`address` fields,
since a real Contacts feature is already on the roadmap; the case-type taxonomy renamed
and widened to `traffic_accident`/`work_accident`/`general_negligence`/
`medical_malpractice`/`civil_commercial`/`generic_civil`/`other` (keeping
`generic_civil`, the pre-existing `matters.matter_type` default, for backward
compatibility); and `matter_parties.role` lost its DB `CHECK` constraint in favor of
Rust-only validation, matching how most enum-shaped columns elsewhere in this schema
already work. The review also refined the roadmap for B2–B6 without changing any
code: B2 needs a `reconcile_default_workstreams` that only adds missing defaults on a
case-type change, never deletes ones in use; B3's requirement lists should read as
office-policy recommendations, not "the law requires," until a source is an Approved
Legal Ruleset; B4's ledger items should follow a `draft → verified → superseded/stale`
lifecycle where a correction supersedes rather than silently edits; B5 splits into
B5a (a focused metadata/FTS/neighbor-expansion retrieval pipeline) before B5b (AI →
ledger proposals), instead of reusing the existing wide `plan_context` unchanged.

## Phase B, milestone B2 — Workstreams + Matter Packs (2026-08-25)

Second milestone of the roadmap: a `matter_workstreams` table (medical/liability/wage/
insurance/BTL/negotiation/litigation, each `not_applicable`/`not_started`/`active`/
`blocked`/`done`), auto-seeded from a matter's case type via a new
`workstreams::reconcile` function - one idempotent, non-destructive pass that handles
seeding a brand-new matter, backfilling a pre-B2 matter with zero rows, and upgrading
only the still-`not_applicable` defaults when a matter's case type later changes,
never touching a workstream already `not_started`/`active`/`blocked`/`done`.
`create_matter` (now transactional for the first time) and `update_matter` (which now
reads the old case type before overwriting it, to detect a real change) both call it;
`list_matter_workstreams` also calls it first as read-repair. Kind/status are
Rust-only validated, matching `matter_profile.rs`'s own pattern. Matter Pack defaults
are office-workflow defaults, not legal determinations. New "מסלולי עבודה" tab in
`MatterWorkspace.tsx` rather than another `OverviewTab` panel (already dense after
B1); the edit-matter modal gained a case-type selector so a lawyer can actually trigger
a case-type change. See `docs/RELEASE_GATES.md`'s "Phase B, milestone B2" section for
the full writeup. B3–B6 remain deliberately unattempted, each still pending its own
planning pass.

## Phase B, milestone B3 — Missing Evidence Matrix (2026-08-25)

Third milestone of the roadmap: a `matter_requirements` checklist table (13 typical
document keys, each `not_applicable`/`not_collected`/`requested`/`collected`/`stale`),
built to the exact same shape and `reconcile` idiom as B2's `matter_workstreams` -
seeding, backfill-on-list, and upgrade-only-on-case-type-change, never touching a
requirement already `not_collected`/`requested`/`collected`/`stale`. A key's priority
(`recommended`/`required_by_office_policy`/`optional`) is computed at read time from a
static Rust map, never persisted, and never phrased as statutory - these are
office-workflow recommendations only, freely overridable per matter. Deliberately no
`linked_document_id` column in this pass (no consumer yet - would be a speculative,
unused abstraction) and no automatic staleness cascade (what "stale" should mean for a
checklist item isn't yet defined by real usage) - `status` stays entirely
lawyer-driven. New "ראיות חסרות" tab in `MatterWorkspace.tsx`
(`MissingEvidenceTab.tsx`), structurally identical to `WorkstreamsTab.tsx`. See
`docs/RELEASE_GATES.md`'s "Phase B, milestone B3" section for the full writeup. B4–B6
remain deliberately unattempted, each still pending its own planning pass.
