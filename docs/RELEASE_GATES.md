# Release Gates

Current reconstruction verdict:
**Developer source only. No client use.**

Status as of this reconstruction pass: **A and B verified. C: a real, verified OCR
runtime (Tesseract/Poppler/tessdata) now builds into the Windows installer and was
confirmed present in the release bundle output — only the actual OCR smoke test against
real documents remains, which needs a human running the packaged app. D's automated
checks verified, one real WCAG AA failure found and fixed. E: a real Windows build now
succeeds in CI and produces an unsigned installer with OCR included (see Gate E for the
artifact link) — signing, release manifest and rollback package are still outstanding.
F: a real automated test (`cargo test gate_f_partial`) now covers every step of the
24-step manual checklist that is platform-independent business logic (matter/scan/hash,
fail-closed source-tamper rejection, full-text search, fact verification, damage lock,
legal-document authoring — template sections, auto-fill from verified facts, manual
paragraph add/edit/confirm/delete, approval, starting a new version from an approved
one — export+audit, DB reopen) — genuinely executed, not mocked. Two rounds of an
independent audit (2026-08-25, see the two "External audit response" sections below)
then found and this project fixed eleven P0-severity integrity gaps a passing test
suite hadn't caught — approved-document immutability, source-lifecycle/staleness
(including partial-scan safety and stale-source AI approvals), fact-grounding
requirements, a client-trusted damage hash, and stale/self-contradictory package
provenance metadata — with regression tests for everything code-level. A third,
different-kind report (UX/product/legal-domain maturity, not code integrity) then found
several dead UI paths and hardcoded state; tiers 1 and 2 of that report are fixed (see
the round-3 section below), tier 3 (Israeli tort-law domain modules) is deliberately out
of scope pending a real lawyer's involvement. Two engineering-only items both audit
rounds had logged as "deliberately not addressed" — the OCR temp-directory RAII gap and
Verified Authority not requiring an approved passage — were then also fixed (see the
"Follow-up fixes" section below), confirmed on real Windows CI at run #12, commit
`d0cdb3e` (2026-08-25). A fourth document then specified governed infrastructure for
deterministic legal rules; Phase A (schema, DSL interpreter, ruleset lifecycle, engine
runs, Settings UI — deliberately zero Israeli substantive law) was then confirmed on
real Windows CI at run #13, commit `64b7066`. A fifth document then audited that Phase A
implementation and returned six P0 governance/integrity gaps (superseded-ruleset
mutability, missing reviewer identity, citation-only sources satisfying approval,
caller-controlled engine_kind, unenforced effective dates, non-deterministic priority
ties) plus five P1s — all fixed in one hardening pass (see "Legal rules infrastructure,
Phase A hardening" below), confirmed on real Windows CI at run #14, commit `d7b3ae6`.
Three further reports (market/product research plus ledger-lifecycle and AI-pipeline
deep dives) then laid out a Phase B roadmap; its first milestone, B1 (Matter Profile —
case type, event/court/BTL fields, party contacts), was confirmed on real Windows CI at
run #15, commit `16a31b3` — then a design review of that same milestone (before any
client ever depended on its shape) prompted renaming two fields for room to grow
(`primary_event_date`/`primary_court_name`), replacing a single contact-details blob
with structured party fields, and widening/relabeling the case-type taxonomy (see
"Phase B, milestone B1" below for the full writeup), confirmed on real Windows CI at
run #16, commit `6e89cd0`. B2 (Workstreams + Matter Packs — per-matter status tracks
auto-seeded from case type, reconciled non-destructively on change) was confirmed on
real Windows CI at run #17, commit `0be99a9` (see "Phase B, milestone B2" below); B3
(Missing Evidence Matrix — the same reconcile idiom applied to a document checklist)
was confirmed at run #18, commit `94f7c63`, then a user code review of B3 found and
fixed a genuine semantic gap (`priority` could contradict a lawyer-collected item's
status — split into `relevance`/`priority`, no migration needed), confirmed at run #19,
commit `958f372` (see "Phase B, milestone B3" below for the full writeup). B4
(Medical/Wage/Liability Ledgers — evidence-grounded, lawyer-verified records with a
`draft → verified` lifecycle and correction-by-supersession, copying
`legal_authorities`' verbatim-containment-checked source-grounding pattern) was
confirmed at run #20, commit `7bdb60a`, then a user code review of B4 found and fixed 5
integrity gaps (verified evidence deletable via SQL, `stale` resettable, verify not
checking source-version staleness, multiple verified successors possible, a weak
integrity hash), confirmed at run #21, commit `55616b9` (see "Phase B, milestone B4"
below for the full writeup). **B1 through the B4 hardening-fix are all confirmed on
real Windows CI.** B5a (Focused AI Retrieval — replaces `ai.rs::plan_context`'s old flat,
unranked, recency-only, capability-blind 80-row query with a real matter-scoped, FTS5-
backed retrieval pipeline: deterministic ranking, page-level neighbor expansion,
deterministic windowing of oversized sources instead of dropping them, and an auditable,
canonically-hashed `ContextManifest`) is complete locally (127/127 tests) but **has not
yet had its own Windows CI run** — see "Phase B, milestone B5a" below for the full
writeup. What's left needs a human on a real Windows machine with a fresh
installer: real OCR, real AI provider calls, and
DOCX export, which doesn't
exist in this reconstruction yet.**

**This is still not a client-ready release.** An unsigned installer from a
reconstruction that hasn't passed Gates C or F must not be used for real client work.

## External audit response, round 1 — "Deep Control" report, 2026-08-25

An independent audit of this source (`TAHRIR_CANONICAL_DEEP_CONTROL_20260825.md`)
returned a **NO-GO**, backed by seven P0-severity findings. Each was independently
re-verified directly against this codebase before being fixed — none were taken on
faith. All seven are now closed:

- **v1 P0-1, source version lifecycle incomplete** — `scanner::hash_pending` only ever
  hashed occurrences with `document_version_id IS NULL`, so a file edited/replaced
  after being hashed could never enter the source graph as a new version; nothing ever
  set `document_versions.stale`. Fixed: `scanner::rehash_changed_versions` detects a
  changed occurrence (its current metadata no longer matches its DocumentVersion's
  recorded metadata — refreshed on every scan by `flush_batch`), re-hashes it, and if
  the content actually differs, creates a new DocumentVersion under the **same**
  logical Document, marks the old version `stale=1`, and cascades `stale=1` onto any
  `verified_facts` grounded in the old version's pages. A metadata-only touch
  (identical content, bumped mtime) does not spawn a version.
- **v1 P0-2, deleted/moved sources never marked missing** — nothing ever set
  `exists_now=0`. Fixed: a full, uninterrupted `scan_metadata` run now marks
  previously-known occurrences under its root that weren't re-observed this run as
  `exists_now=0`. A scan that errors out mid-walk returns before this step and never
  mass-marks anything missing.
- **v1 P0-3, approved legal-document content not immutable at the DB level** — a direct
  `UPDATE legal_document_paragraphs`/`legal_document_sections` succeeded even after the
  parent version was approved; only `legal_document_versions` and
  `legal_document_sources` had triggers. Fixed: six new triggers
  (insert/update/delete × sections/paragraphs) in `001_schema_v12.sql`, mirroring the
  existing `legal_document_sources` pattern.
- **v1 P0-4, paragraph "confirmed" required no provenance** — `confirm_paragraph` flipped
  any paragraph to `confirmed` with no check that a source existed. Fixed:
  `add_paragraph` now only ever produces `paragraph_kind='argument'` (lawyer-authored
  legal text — confirming is editorial review, no source needed); `paragraph_kind='fact'`
  paragraphs are only ever created by `fill_from_verified_facts`, which grounds them
  atomically, and `confirm_paragraph` now refuses to confirm a `'fact'` paragraph
  without a currently-valid, non-stale `verified_fact` source.
- **v1 P0-5, invalid/stale facts could remain in an approvable draft** — `approve_version`
  only checked `provenance_state`, never re-checked that a linked fact was still valid.
  Fixed: `approve_version` now re-validates every `'fact'` paragraph's grounding at
  approval time, not just at confirm time — a fact invalidated or gone stale after
  being confirmed blocks approval.
- **v1 P0-6, an authority could be "verified" without a source** — `verify_authority` never
  required `source_document_version_id`. Fixed: verification now requires it, and binds
  the source's `content_sha256` into the integrity hash. `AuthoritiesTab` gained a
  source-document picker so this is reachable from the UI.
- **v1 P0-7, installer stale relative to source** — `main` was 5 commits ahead of the
  commit (`e92a148`) Gate C's installer was built from. Closed by rerunning the Windows
  Release Gate against the current commit — see the updated Gate C/E entries below.
  This took three attempts on real CI, each a genuine bug caught by actually running it
  rather than assuming: run #7 failed at `contract:check`
  (`scripts/check-command-contract.mjs` had a hardcoded `=== 61` check left over from
  before this session added any commands - fixed to just check `missing`/`extra` are
  empty); run #8 got past that and failed a real `cargo test` assertion in
  `gate_f_partial.rs`, which had hard-asserted the DB-reopen-without-a-retrievable-key
  path always fails closed - true in the Linux dev sandbox this was written in, but
  false (and *should* be false) on real Windows, where the OS Credential Manager
  correctly persists the key; fixed to accept either valid outcome and actually verify
  whichever one occurs. Run #9 succeeded end-to-end.

Also closed while responding to the same report (flagged there as P1, not P0, but easy
to close alongside these and directly relevant to "don't trust unverified state"):
- **Damage lock trusted a client-supplied integrity hash.** `lock_damage_calculation`
  now re-derives it from the persisted `damage_inputs` via the new
  `damage::verify_for_lock`, and refuses to lock if that doesn't match the calculation's
  stored totals.
- **`approval_sha256` only hashed paragraph bodies**, not section headings/order,
  paragraph kind/provenance, or source references — a reordered section or silently
  detached source wouldn't change the hash. `legal_docs::canonical_content` now builds a
  structural fingerprint of the whole version (plus a linked, locked damage
  calculation's own hash, when one is cited), and `export_legal_document` recomputes and
  verifies it before writing output — defense in depth behind the new triggers, not a
  replacement for them.
- **The UI had no way to actually approve or export a legal document** — the commands
  existed, nothing called them. `LegalDocumentsTab`'s editor now has "אשר מסמך" (approve)
  and "ייצוא כטקסט" (export) actions, the latter via a new `choose_save_file` command.

Real regression coverage for all of this lives in `src-tauri/src/integrity_tests.rs`
(10 new tests, all passing) — see its module doc comment for exactly which finding each
test covers. `src-tauri/src/gate_f_partial.rs`'s existing coverage is unaffected (still
passing) since these are additive constraints, not behavior changes to the paths it
already exercises.

**Deliberately not addressed** (P1/P2 in the report, out of scope for this pass): DOCX/PDF
export still don't exist; the damage engine is still an additive prototype, not a
versioned legal ruleset engine; there's no deterministic legal-deadline rules engine.
None of these were claimed fixed. (The OCR temp-directory RAII gap noted here was
subsequently fixed — see the follow-up fixes section below.)

**P1-1 (AI run/review/verify-fact UI not wired end-to-end) was subsequently fixed** in a
follow-up pass — see `docs/MISSING_IMPLEMENTATION_MATRIX.md`'s "AI review workflow
completion" section: `review_ai_proposal`'s approval path now actually creates a
source-grounded `VerifiedFact` (it previously only flipped a status flag), the OpenAI
client-data-authorization checkbox that was missing from the settings UI now exists, and
`FactsAITab` has a real run/review flow. Covered by a new regression test; not covered
against a live provider, since none is reachable from this environment.

## External audit response, round 2 — "Deep Control v2" report, 2026-08-25

A second-round audit of the round-1-fixed source (`TAHRIR_DEEP_CONTROL_V2_20260825.md`)
confirmed the round-1 fixes hold (directly re-attempted the approved-document mutation
attack and it's still blocked; reconfirmed 69/69 commands, 33 tables, 24 triggers,
`integrity_check`/`foreign_key_check` clean) but found four new P0s and three more P1s.
All re-verified against the actual code before being fixed - the round-2 report reused
the same P0-1..P0-4 numbering as round 1 for unrelated findings, so these are
disambiguated as "v2 P0-N" everywhere below and in the code comments themselves.

- **v2 P0-1, partial-scan safety wasn't actually guaranteed.** `scanner::scan_metadata`
  iterated with `WalkDir::new(root).into_iter().filter_map(Result::ok)`, silently
  discarding any traversal error (an unreadable subdirectory, a race with deletion),
  then unconditionally treated reaching the end of the walk as proof the scan was
  complete - so an unreadable subtree could still trigger the missing-file mass-update
  for everything else. Fixed: every traversal and per-entry metadata error is now
  counted (not discarded), and the missing-marking step only runs when that count is
  zero; `scan_runs.error_count`/`.partial` (columns that already existed in the schema
  but were never written to) now actually record this.
- **v2 P0-2, a stale AI proposal could still be approved into a fresh VerifiedFact.**
  `ai::plan_context` correctly excludes stale DocumentVersions when a run starts, but
  `ai::approve_proposal` validated a cited `sourceId` only against `document_pages`,
  never checking whether its version had since gone stale while the proposal sat
  pending. Fixed: approval now joins `document_versions` and requires `stale=0` for
  every cited source, failing the whole approval closed (no partial fact creation) if
  any source has changed since the run.
- **v2 P0-3, the shipped SOURCE-MANIFEST.json/QA_*.json were describing a different,
  older version of the source than the one they shipped with** (61 commands/31
  tables/18 triggers, a `tsconfig.qa.json` that doesn't exist, hash/size mismatches for
  37 files) - these were one-time hand snapshots from the original package import that
  were never regenerated as the source changed underneath them. Fixed at the root
  cause, not just patched once: `scripts/generate-source-manifest.mjs` now computes
  `SOURCE-MANIFEST.json` for real from the actual git-tracked file tree (path/size/
  sha256 for all 86 files, plus the commit it was generated against), and
  `QA_SCHEMA.json`/`QA_RECONSTRUCTION_REPORT.json`/`QA_TYPESCRIPT.json` were
  regenerated from checks actually run against the current source, not re-typed by
  hand. Whoever packages a release build should re-run the manifest script rather than
  editing these files directly - that's exactly how they went stale the first time.
- **v2 P0-4, the Windows installer was behind the latest source again** - expected: any
  further code change (including this round's own fixes) makes the previous CI run
  stale by definition. Not independently re-fixed; needs another Windows Release Gate
  run against the commit these fixes land in before Gate C/E can be called current
  again.

Also fixed (v2 P1, not P0, but cheap to close alongside these):
- **v2 P1-4, malformed `damage_inputs.value_text` silently became 0** via
  `.parse().unwrap_or(0)` in both the list and lock paths - a corrupted persisted
  financial figure would be treated as zero instead of raising an error. Fixed to a
  hard, propagated parse error in both places.
- **v2 P1-7, `FactsAITab`'s "פתח מקור" (open source) button had no `onClick`** - a
  leftover from before this pass's AI-review-UI work, carried over unchanged.
  `list_verified_facts` now also returns an `occurrenceId` (joined through
  `verified_fact_sources.document_version_id`), and the button calls `open_occurrence`.

**Deliberately not addressed** (v2 P1/P2, out of scope for this pass): v2 P1-3 (damage
engine still an additive prototype, not a versioned rules engine); v2 P1-5 (no
deterministic deadline engine); v2 P1-6 (DOCX/PDF export still absent). None of these
were claimed fixed. (v2 P1-2 — Verified Authority requiring an approved passage — and
the round-1 OCR RAII cleanup gap were both subsequently fixed; see the follow-up fixes
section below.)

Real regression coverage lives in `src-tauri/src/integrity_tests.rs` (4 new tests) and
`src-tauri/src/scanner.rs`'s own test module (1 new unit test for the partial-scan
gating logic - a genuine WalkDir permission error isn't reliably forceable when tests
run as root, which is documented directly on that test rather than worked around
silently). v2 P1-7 is a frontend-only wire-up with no new backend logic, so it has no
dedicated test.

## External audit response, round 3 — UX/product/legal-domain design report, 2026-08-25

A third report (`TAHRIR_DEEP_DESIGN_LEGAL_REPORT_20260825.md`) reviewed the product from
a different angle than rounds 1-2 — UX completeness and legal-domain maturity, not
code-level data integrity. Its concrete, spot-checkable claims (dead `onClick` handlers,
hardcoded health status, a literal "PIP" label, a CSS column mismatch) were each
independently re-verified against the code before being fixed. Prioritized into three
tiers; tiers 1 and 2 are closed as of commit `721cc03`:

**Tier 1 — dead UI and fake state:**
- `TodayPage`/`ActionCenterPage`/`SearchPage` rows rendered as buttons but had no
  `onClick` — clicking a row did nothing. Wired all three through the `matterId`
  navigation state already used by `MattersPage`.
- `TodayPage` grouped items by a fixed kind-order list, not by date. Now buckets by real
  `dueAt` (overdue/today/tomorrow/next 7 days), falling back to kind-based buckets
  (review/waiting/resume) for undated items, with a summary line.
- `Inspector` and `commands::get_app_health` were fully hardcoded — always reported
  "SQLCipher"/"מקומי"/"כבוי" regardless of actual state. `get_app_health` now runs a real
  `SELECT 1` DB probe, checks whether `office_root` is bound, checks whether the
  Tesseract/Poppler binaries exist on disk under `resource_root`, and checks
  `ai_provider_profiles` for an `enabled=1` row; `Inspector` renders whatever comes back.
- `DamageTab` labelled the PIP regime option literally "PIP" (English acronym in a
  Hebrew-first UI) — relabeled to "פלת\"ד" (the internal `"pip"` enum value is unchanged).
- `DocumentsTab` renders 5 columns but the shared `.tr` grid CSS was hardcoded to 4
  columns, silently clipping the "פעולות" (actions) column. Scoped a 5-column override
  to `.documents-table` rather than changing the shared `.tr` rule other tables
  (`DamageTab`, `SettingsPage`) still rely on.

**Tier 2 — bounded legal-UX pass:**
- The AI review UI (`FactsAITab`) showed only a source count per proposal, with no way
  to read what the AI actually saw before approving or rejecting. `get_ai_run` now
  resolves each proposal's `sourceIds` to the underlying `document_pages` text
  (truncated to 400 characters, with file name and page number) and the UI renders it
  inline above the approve/reject/needs-revision actions.
- Committing a deadline (`commit_deadline`) was a single irreversible click with no
  preflight. `TasksCalendarTab` now requires an explicit two-step confirm — the first
  click reveals the date/source being locked and requires a second, separate click
  before `commit_deadline` actually fires.

**Deliberately not addressed in this pass:** Tier 3 of the same report (a deterministic
deadline-computation rules engine per Israeli civil procedure, a versioned Israeli tort
damages ruleset, and Medical/Wage/Liability domain ledgers) requires a real licensed
tort lawyer's active involvement to be correct — this is not something to encode
unilaterally from a report, and building it without that validation would be worse than
leaving the gap explicit.

Verified: `cargo check/test --locked` (19/19), `npm run build`, `contract:check` (69/69,
no drift), `qa:static` (all checks pass). Confirmed on real Windows CI at
[run #11](https://github.com/yossizch-max/-2/actions/runs/32854305830), commit
`721cc03`, 2026-08-25 — see the updated Gate C/E entries below.

## Follow-up fixes: OCR temp-dir RAII cleanup and authority-passage grounding (2026-08-25)

Two items that both audit rounds had explicitly logged as "deliberately not addressed"
were picked up in a later pass, since neither needs a lawyer's judgment call to fix —
both are ordinary engineering gaps:

- **Round-1 OCR temp-directory RAII gap.** `extraction::extract_scanned_pdf` created a
  scratch directory for rasterized page images and cleaned it up with manual
  `std::fs::remove_dir_all` calls at each known failure branch — but several operations
  in between (`Command::output()?`, `std::fs::read_dir(&temp)?`) used the `?` operator,
  so a failure at any of *those* points (e.g. the OS failing to spawn `pdftoppm.exe` or
  `tesseract.exe`, not just those processes returning a non-zero exit code) would
  propagate immediately and leak the directory. Fixed with an `OcrTempDir` RAII guard
  (mirroring `VerifiedSourceSnapshot`'s existing `Drop`-based cleanup pattern in
  `source_snapshot.rs`): the directory is removed on drop regardless of which path the
  function exits through. Covered by a real unit test in `extraction.rs` that forces an
  early return through `?` and asserts the directory no longer exists afterward.
- **v2 P1-2, Verified Authority didn't require an approved passage.** `verify_authority`
  only ever required `source_document_version_id` to be set — an authority could be
  "verified" by attaching an entire unrelated document, without anyone having read or
  stood behind any specific passage of it. `legal_authority_passages` already existed in
  the schema (`source_page_id`, `passage_text`, `passage_sha256`, `approved`) but no
  command ever read or wrote it. Fixed by extracting `verify_authority`'s logic (which
  can't be tested directly — see the v1 P0-6 note above about `tauri::State`) into a new
  `authorities` module: `add_passage` requires the quoted text to appear verbatim, after
  `extraction::normalize_source_text`, on the cited page of the authority's own source
  document — not typed freely; `approve_passage` re-checks that same containment against
  the page's *current* text at approval time, not just when it was drafted;
  `verify` now requires at least one approved passage and folds the approved passages'
  hashes into the integrity hash. `AuthoritiesTab` gained a passage-management panel:
  pick a source page, read its text, quote a passage, approve it, then verify.

Both covered by real regression tests: `extraction::tests::ocr_temp_dir_is_removed_on_early_return_through_the_question_mark_operator`,
and four new tests in `integrity_tests.rs` (`adding_an_authority_passage_requires_a_stored_source_document`,
`adding_an_authority_passage_requires_verbatim_containment_in_the_cited_page`,
`verifying_an_authority_requires_at_least_one_approved_passage`,
`approving_a_passage_re_checks_containment_against_the_current_source_text`) — 24/24
backend tests pass. `npm run build`, `contract:check` (72/72, no drift — three new
commands: `list_authority_passages`, `add_authority_passage`, `approve_authority_passage`),
and `qa:static` all pass. Confirmed on real Windows CI at
[run #12](https://github.com/yossizch-max/-2/actions/runs/32866131285), commit
`d0cdb3e`, 2026-08-25 — all 24 tests pass there too, not just in this sandbox — see the
updated Gate C/E entries below.

## Legal rules infrastructure, Phase A (2026-08-25)

A fourth external document (`TAHRIR_LEGAL_RULES_INFRASTRUCTURE_SPEC_20260825.md`) specified
governed infrastructure for deterministic legal rules — explicitly **not** Israeli
substantive law itself, only the machinery for a lawyer to author, source, test and
approve a rule before it may drive a committed legal result. Reviewed before
implementation (see the conversation for the review pass); Phase A (infrastructure
only — "No Israeli rule content yet," per the spec) is now built:

- **Schema** (`002_legal_rules_infrastructure_v13.sql`, additive to `001`, `PRAGMA
  user_version=13`): `legal_rulesets` (draft → under_review → approved → superseded/
  revoked; firm-wide, not matter-scoped — a Ruleset is a governed asset used across
  every matter), `legal_ruleset_sources`, `legal_rules`, `legal_rule_test_cases`,
  `legal_engine_runs` (an immutable, matter-bound calculation trace). Triggers mirror
  the existing `legal_document_*` immutability pattern: an approved ruleset (and
  everything that composes it) cannot be mutated or deleted, except the one specific
  transition supersession needs; a committed engine run's snapshot/result/trace can
  never change.
- **The DSL** (`src-tauri/src/legal_rules.rs`): `conditions_json` is an implicit-AND
  list of `{field,op,value}` checks against a flat JSON context; `operation_json` is an
  ordered sequence of steps over named registers, restricted to 10 fixed safe operators
  (`compare`, `add_days`, `subtract_days`, `add_amount`, `subtract_amount`,
  `multiply_decimal` — fixed-point, no floating-point drift, half-up rounding — `cap`,
  `floor`, `choose`, `require_input`). No code evaluation, no SQL, no filesystem, no
  network access — an unknown operator is a hard error, not silently ignored.
- **Ruleset lifecycle**: create/add-source/add-rule/add-test-case all only work on a
  `draft` ruleset. A source bound to a real in-app document is verified immediately
  (its SHA256 is read from `document_versions`, never trusted from the caller); a
  citation-only source is only verified if the caller names a `verified_by` — a
  lawyer's deliberate act, not a default. `approve_ruleset` re-checks everything at
  approval time, not just at draft time: at least one currently verified, non-stale
  source; every rule cites one; at least one test case exists and *all* of them are
  reviewer-approved *and* pass a fresh, non-cached run of the DSL right now. Only then
  is an integrity SHA256 computed over the full structural content and the ruleset
  locked. Supersession never deletes — the old ruleset is retained forever, just
  marked `superseded`, and the replacement must itself already be an approved ruleset
  for the same engine/jurisdiction.
- **Engine runs**: `preview_legal_engine_run` computes a result without persisting
  anything; `commit_legal_engine_run` never trusts a previewed result — it recomputes
  from scratch server-side against the ruleset as it exists right now (re-checking the
  matched rule's specific source for staleness one more time), then persists an
  immutable trace row. No rule matching fails closed with `NoApprovedRuleForContext`
  (surfaced as the fixed `NO_APPROVED_RULE_FOR_CONTEXT` error code the spec asked for),
  never silently produces a result.
- **Settings UI**: `SettingsPage` gained a "כללים משפטיים" card linking to a new
  `LegalRulesPage` — a ruleset list (engine/title/version/status/source count/test
  coverage) plus a per-ruleset editor (sources, rules, test cases, run-tests, submit/
  approve, and an engine-run preview panel for approved rulesets). No "activate" button
  exists for a draft; every action's real gating comes from the backend, the UI just
  surfaces whatever error it returns.

**Beyond the spec's literal section 8 command list**, `review_legal_rule_test_case`
was added — the spec's own approval invariant ("required test cases are approved")
has no command anywhere in section 8 to actually mark a test case approved, so this is
a necessary completion of the spec's own stated requirement, not scope creep.

**Deliberately not attempted**: Phase B (Medical/Wage/Liability evidence ledgers) and
Phase C (the first real lawyer-approved legal module — a deadline rules engine, a
damages ruleset). Also not wired: `commit_legal_engine_run` integration into
`TasksCalendarTab`/`DamageTab` (the spec's "Integration with current TAHRIR" section)
— the command exists and is tested, but no UI calls it yet from a matter workspace.

15 new backend tests (9 mapped to the spec's own section-12 list by number, plus the
DSL's 11 pure-function unit tests) — 44/44 total. `cargo check/test --locked`,
`npm run build`, `contract:check` (86/86, no drift), `qa:static` all pass.
`scripts/static-qa.mjs`'s table-count check was split into one assertion per migration
file (`thirtyThreeTablesInBaseSchema`/`fiveTablesInLegalRulesInfra`) rather than one
combined total — the same class of silent-staleness risk as the `contract:check`
`=== 61` bug from earlier in this project, caught proactively this time instead of by
a CI failure. Confirmed on real Windows CI at
[run #13](https://github.com/yossizch-max/-2/actions/runs/32877320049), commit
`64b7066`, 2026-08-25 — see the updated Gate C/E entries below.

## Legal rules infrastructure, Phase A hardening (2026-08-25)

An independent audit of Phase A (`TAHRIR_LEGAL_RULES_PHASE_A_DEEP_CONTROL_20260825.md`)
returned a **strong foundation, not yet safe to connect to real substantive rules** -
six P0 governance/integrity gaps and five P1 hardening items. Every P0 was
independently re-verified before being fixed - the trigger-level findings against
actual SQLite behavior (not just read), the code-level findings by direct inspection.
All six P0s and five of six P1s are now closed in one focused hardening pass, per the
audit's own recommended order; P1-6 remains deliberately deferred (explained below).

- **P0-1, superseded rulesets were mutable and re-approvable.** The original trigger
  only guarded `OLD.status='approved'`, so once a ruleset became `superseded` its
  title, sources, rules and test cases could all still be edited directly, and its
  status could be flipped straight back to `'approved'` - verified live against real
  SQLite before fixing. Fixed: `approved`/`superseded`/`revoked` are now all terminal
  states; the *only* permitted mutation is the exact `approved -> superseded`
  transition, changing solely `status`/`superseded_by` (every other column checked
  byte-for-byte unchanged via null-safe `IS` comparisons) - and only from `approved`,
  never from an already-terminal state. Old trigger names dropped explicitly so an
  already-migrated database doesn't keep the buggy ones alongside the new ones.
- **P0-2, "lawyer approved" identity wasn't actually required.** `approved_by`/
  `reviewed_by` were optional, and the UI never collected them - a ruleset could reach
  `approved` with a null approver. Fixed: both are now required, non-empty strings,
  rejected at the Rust layer before touching the database; `LegalRulesPage` gained a
  reviewer-name field gating the approve/test-case-approve actions.
- **P0-3, a citation-only source could satisfy approval with no real source text.**
  `add_source`'s citation-only path hashed the citation *text itself*, not any actual
  legal source, and still counted as "verified" once `verified_by` was set - so a
  Ruleset could be approved with zero stored source text. Fixed: a document-backed
  source must now name an exact page (not just a document version), hashed from that
  page's own `text_sha256`; the approval gate only counts sources bound to a real,
  currently non-stale page. Citation-only sources can still be added for reference but
  can never satisfy approval, regardless of `verified_by`. `LegalRulesPage` gained a
  real matter/document/page picker for this (it previously exposed no document picker
  at all, matching the audit's UI finding).
- **P0-4, `engine_kind` was caller-controlled, not bound to the ruleset.**
  `commit_engine_run` accepted `engine_kind` as a parameter with no check it matched
  the ruleset's own - a deadline ruleset could be committed as a `damage` run. Fixed:
  the parameter is gone entirely; `engine_kind` is always read from `legal_rulesets`
  and stored as-is.
- **P0-5, `effective_from`/`effective_to` existed in the schema but were never
  enforced.** Fixed: validated (real ISO dates, `from <= to`) at create/update time;
  checked against the server clock at preview/commit time, rejecting a ruleset outside
  its own declared period; the applicability date is recorded in the engine-run trace.
- **P0-6, rule priority ties resolved non-deterministically.** `ORDER BY priority ASC`
  alone gives SQLite no guaranteed tie-break order. Fixed: `ORDER BY priority ASC,
  rule_key ASC` everywhere rules are matched (engine run and test runner alike) -
  verified deterministic across five repeated runs in a dedicated test, not just once.

Also fixed (P1, five of six):
- **P1-1, approval wasn't atomic.** The original flow checked invariants, released the
  writer lock, ran the test suite through a separate connection, then reacquired the
  lock and only rechecked `status` - a window a concurrent command could exploit.
  Fixed: everything now runs inside one `db.write` call on one connection
  (`run_tests_against_conn`, shared with the public `run_tests`), holding the writer
  lock for the whole operation.
- **P1-2, the integrity hash omitted legally meaningful fields.** `canonical_content`
  now binds effective period, each rule's `explanation_template`, each source's
  `source_kind`/`document_version_id`/`document_page_id`, each test case's
  `reviewed_by`, and the approver's identity - not just bodies and keys.
- **P1-3, engine-run status could move backward.** A direct SQL test confirmed
  `locked -> proposed` succeeded before the fix. Fixed: a trigger now ranks
  `proposed(1) < reviewed(2) < committed(3) < locked(4)` and blocks any update where
  the new rank is lower than the old.
- **P1-4, an empty test expectation was trivially meaningless.**
  `expected_output_json: {}` passed regardless of whether a rule matched. Fixed: a
  no-match expectation must now be explicit (`"expectNoMatch": true`); a test whose
  rule matches must assert at least one real outcome, or it fails as meaningless
  rather than trivially passing.
- **P1-5, the DSL used floating point in comparison/cap/floor.** For a money engine,
  `f64` risks precision loss on large cent values. Fixed: `compare`/`cap`/`floor` now
  use exact `i64` arithmetic whenever both operands are integers, falling back to
  `f64` only for genuinely non-integer numbers - covered by a test using values beyond
  `f64`'s exact-integer range (2^53).

Also fixed the audit's UI finding: the rule-authoring form's `operation` textarea had
a prefilled `add_days ... days:30` sample value that "visually resembles a legal rule"
even as a placeholder. Replaced with an empty value and a genuinely generic
placeholder attribute (`days:N`).

**Deliberately deferred: P1-6**, linking `legal_engine_runs` to `legal_deadlines`/
`damage_calculations` via an `engine_run_id` FK - the audit itself frames this as
"before wiring Phase C," and that integration (the spec's own "Integration with
current TAHRIR" section) hasn't been started yet; adding the FK now would be
speculative against an interface that doesn't exist. The broader UI polish the audit
also noted (structured form controls instead of raw JSON, an "advanced" toggle) is
explicitly framed as prep for a lawyer authoring real rules, i.e. Phase C work, not
required for Phase A's own correctness.

13 new backend tests (one per P0/P1 item with dedicated DB or DSL coverage) - 58/58
total. `cargo check/test --locked`, `npm run build`, `contract:check` (86/86, no
drift), `qa:static` all pass.

**Confirmed on real Windows CI**: [run #14](https://github.com/yossizch-max/-2/actions/runs/32885155747),
commit `d7b3ae6`, 2026-08-25 - `cargo test --locked` passed 58/58 on the real
`windows-2025` runner.

## Phase B, milestone B1 — Matter Profile (2026-08-25)

Three further reports (`TAHRIR_ISRAEL_CIVIL_TORT_MARKET_PRODUCT_RESEARCH_20260825.md`,
`TAHRIR_LEDGER_LIFECYCLE_DEEP_REPORT_20260825.md`,
`TAHRIR_AI_PIPELINE_AUTOPILOT_DEEP_REPORT_20260825.md`) recommended TAHRIR become an
AI-native Case Operating System for Israeli tort/civil practice, laying out a Phase B
roadmap (Matter Profile → Workstreams → Ledgers → AI Pipeline → Missing Evidence Matrix
→ Case Health). Codebase exploration confirmed this is genuinely greenfield - `matters`
had no client/party/insurer/court/event-date/BTL data at all. This pass implements only
the first milestone, B1 (Matter Profile); B2-B6 are deliberately left as a roadmap, each
to get its own planning pass before implementation.

- **New migration `003_matter_profile_v14.sql`**: a `matter_profile` 1:1 side table
  (event date, court name, BTL claim number, case summary) and a `matter_parties`
  contact list (role constrained to client/party/witness/employer/insurer/
  medical_institution/expert/opposing_counsel/court, matching the market report's own
  taxonomy). `matters` itself is deliberately never `ALTER`ed - every migration to date
  is additive-only and re-run in full on every `DbState::open()` call, and
  `ALTER TABLE ... ADD COLUMN` isn't idempotent in SQLite (a second run fails with
  "duplicate column name"), which would break that invariant. `matters.matter_type`
  already had the right concept for case type; it's tightened with a Rust-side
  allowlist (`matter_profile::validate_case_type`, the same pattern `damage.rs` already
  uses for `regime`/`life_state`/input keys) rather than a redundant new column.
- Plain office-management data, not an evidentiary claim: no lock/approval lifecycle,
  no DB immutability triggers - editable like `update_matter` already was.
- 6 new commands (`get_matter_profile`, `save_matter_profile`, `list_matter_parties`,
  `add_matter_party`, `update_matter_party`, `delete_matter_party`), a case-type
  `<select>` on matter creation, and a "פרופיל תיק"/"צדדים" panel pair on
  `OverviewTab.tsx`.

7 new backend tests (upsert idempotency, malformed-date rejection, party-role
validation, cascade delete of both new tables with their matter, plus 3 pure unit
tests for the allowlists) - 65/65 total. `cargo check/test --locked`, `npm run build`,
`contract:check` (92/92, no drift), `qa:static` (now also checking
`twoTablesInMatterProfile`) all pass. Migration re-verified idempotent (applied 3x: 40
tables, 27 indexes, 37 triggers, `user_version=14`, integrity ok, `fk_check` clean).

**Review fixes (2026-08-25), applied before any Windows CI run confirmed the original
shape** - `003_matter_profile_v14.sql` was edited in place (not superseded by a new
migration file, mirroring the precedent set by the Phase A hardening pass rewriting
002 in place):
- `event_date`/`court_name` renamed to `primary_event_date`/`primary_court_name` - a
  tort matter accumulates many dates and can outgrow a single court/proceeding; the
  unqualified names implied there could only ever be one of each. A future
  `matter_events`/Chronology feature and multi-proceeding tracking are the real home
  for the rest.
- `btl_claim_number` stays as a convenience/display field only, explicitly not a BTL/
  insurer model - a matter can have several BTL or insurer claim numbers in practice. A
  proper `matter_external_references` (kind/value/label) table is deferred to the
  roadmap's B7 (Negotiation + Insurance/BTL).
- `matter_parties.contact_details` (one free-text blob) replaced with structured
  `display_name`/`entity_kind`/`identifier`/`phone`/`email`/`address`, since a real
  Contacts feature is already on the roadmap and a blob would just get parsed back
  apart later. `role` lost its DB `CHECK` constraint - validated in `matter_profile.rs`
  only now, matching how most enum-shaped columns elsewhere in this schema already work
  (`matters.status`, `documents.category`), so the role/entity_kind taxonomy can evolve
  without a schema migration.
- Case-type taxonomy renamed/expanded to `traffic_accident`/`work_accident`/
  `general_negligence`/`medical_malpractice`/`civil_commercial`/`generic_civil`/`other`
  - `generic_civil` (the pre-existing `matters.matter_type` default) is deliberately
  kept in the allowlist for backward compatibility, with `other` as a separate
  catch-all.

Also captured for when B2+ actually get planned (not applied here): B2 needs a
`reconcile_default_workstreams(matter_id, new_case_type)` that only adds missing
default workstreams when a matter's case type changes, never deletes or overwrites
ones already in use; B3's per-case-type requirement lists should read as
recommended/office-policy/optional (`Matter Pack Requirements`), never phrased as
statutory "the law requires," until a source is an Approved Legal Ruleset; B4's ledger
items should follow a `draft → verified → superseded/stale` lifecycle where `verified`
is immutable and a correction creates a supersession, not a silent edit; B5 should
split into B5a (a focused metadata/FTS/neighbor-expansion retrieval pipeline replacing
`plan_context`'s current up-to-80-pages-unfiltered approach) before B5b (AI → ledger
proposals), so the three new ledgers aren't fed unfiltered context.

7 new backend tests updated in place for the renamed/restructured fields; 66/66 total
after the review fixes (one added for entity-kind validation). Re-verified: `cargo
test --locked`, `npm run build`, `contract:check` (92/92), `qa:static`, and the
migration's idempotency (still 40 tables/27 indexes/37 triggers/`user_version=14`).

The original B1 commit (`16a31b3`, before this review pass) was confirmed on real
Windows CI at [run #15](https://github.com/yossizch-max/-2/actions/runs/32892388064) -
65/65 tests passed on the real runner. The review-fix commit (`6e89cd0`) was then
confirmed at [run #16](https://github.com/yossizch-max/-2/actions/runs/32901003830) -
66/66 tests passed on the real runner. B1 is now fully CI-confirmed on Gate C/E.

## Phase B, milestone B2 — Workstreams + Matter Packs (2026-08-25)

Second milestone of the Phase B roadmap locked in by the B1 design review: per-matter
Workstream tracks (medical/liability/wage/insurance/BTL/negotiation/litigation), each
with a status (`not_applicable`/`not_started`/`active`/`blocked`/`done`), auto-seeded
from the matter's case type and reconciled - never destructively - when the case type
later changes. Planned via `EnterPlanMode` (an Explore pass first confirmed
`create_matter` had no existing "seed child rows on parent creation" precedent to
reuse, `update_matter` overwrote `matter_type` blind with no prior read of the old
value, and `OverviewTab.tsx` was already dense enough after B1 that a dedicated tab -
not another panel - was the right call for this milestone's UI).

- **New migration `004_matter_workstreams_v15.sql`**: one `matter_workstreams` table
  (`matter_id`, `kind`, `status`, `notes`), `UNIQUE(matter_id,kind)`. No DB `CHECK` on
  `kind`/`status` - validated in `src-tauri/src/workstreams.rs` only, the same pattern
  `matter_profile.rs` already uses.
- **`workstreams::reconcile(conn, matter_id, case_type)`** is one function serving all
  three cases with a single idempotent, non-destructive pass: `INSERT ... ON
  CONFLICT(matter_id,kind) DO NOTHING` seeds any of the 7 kinds not yet present
  (`not_started` if the case type's default pack includes it, else
  `not_applicable`), then a second pass flips an existing `not_applicable` row to
  `not_started` only if the *new* case type's defaults now include it - a workstream
  already at `not_started`/`active`/`blocked`/`done` is never touched. This one
  function handles a brand-new matter (full seed), a case-type change on an existing
  matter (only the second pass can fire), and a pre-B2 matter with zero workstream
  rows (full backfill) identically.
- **`create_matter`** is now wrapped in `conn.transaction()` (it had none before) and
  calls `reconcile` right after its `INSERT`, so a matter is never left without its
  workstreams. **`update_matter`** now `SELECT`s the old `matter_type` before its
  `UPDATE` (it previously overwrote blind via `coalesce`) and calls `reconcile` only
  when the value actually changed. **`list_matter_workstreams`** calls `reconcile`
  first (read-repair) before listing - the one `list_*` command that legitimately
  needs a write, documented as such - so a pre-B2 matter gets backfilled transparently
  on first view with no migration/backfill script needed.
- Matter Pack defaults (which kinds are "on" per case type) are office-workflow
  defaults, not legal determinations, and stay freely overridable per matter.
- New "מסלולי עבודה" tab in `MatterWorkspace.tsx` (`WorkstreamsTab.tsx`) rather than
  another `OverviewTab` panel; the existing edit-matter modal gained a case-type
  `<select>` so a lawyer can actually trigger the reconcile-on-change path (it
  previously had no way to change a matter's case type at all after creation).

9 new backend tests (seeding, reconcile-never-touches-an-active-workstream on a
case-type change, backfill-via-list on a pre-B2 matter, kind/status validation,
cascade delete, plus 3 pure unit tests for the allowlists/defaults) - 75/75 total.
`cargo check/test --locked`, `npm run build`, `contract:check` (94/94, no drift),
`qa:static` (now also checking `oneTableInMatterWorkstreams`) all pass. Migration
re-verified idempotent via direct sqlite3 (applied 3x): 41 tables, 28 indexes, 37
triggers, `user_version=15`, integrity ok, `fk_check` clean.

**Confirmed on real Windows CI at [run #17](https://github.com/yossizch-max/-2/actions/runs/32926614574), commit `0be99a9`** - 75/75 tests passed on the real runner.

## Phase B, milestone B3 — Missing Evidence Matrix (2026-08-25)

Third milestone of the Phase B roadmap: a per-matter checklist of typical documents a
matter of a given case type tends to need (13 keys: id_document, police_report,
medical_records_initial, medical_records_full_file, wage_stubs,
employer_incident_report, witness_statements, insurance_policy, btl_forms,
vehicle_photos, expert_opinion, contract_document, correspondence_records), each with a
status (`not_applicable`/`not_collected`/`requested`/`collected`/`stale`) a lawyer
tracks over the matter's life. Built to the exact same shape and `reconcile` idiom as
B2's `matter_workstreams` - the pattern worked well and needed no changes, just a
different key set.

- **New migration `005_matter_requirements_v16.sql`**: one `matter_requirements` table
  (`matter_id`, `requirement_key`, `status`, `notes`), `UNIQUE(matter_id,
  requirement_key)`, no DB `CHECK` (Rust-only validation, same rationale as
  `matter_workstreams`).
- **`requirements::reconcile`** is the same idempotent, non-destructive one-function
  design as `workstreams::reconcile`: seeds any of the 13 keys not yet present
  (`not_collected` if the matter's case type's default pack includes it, else
  `not_applicable`), then upgrades an existing `not_applicable` row to `not_collected`
  only if the *new* case type's defaults now include it - never touches a row already
  at `not_collected`/`requested`/`collected`/`stale`. Called from the same three sites
  as `workstreams::reconcile`: `create_matter`, `update_matter` on a real case-type
  change, and as read-repair in the new `list_matter_requirements`.
- A key's **priority** (`recommended`/`required_by_office_policy`/`optional`) is
  computed at read time from the matter's *current* case type via a static Rust map -
  never persisted, and never phrased as statutory ("the law requires"); these are
  office-workflow recommendations only, freely overridable per matter. Only an
  Approved Legal Ruleset could ever give a future requirement real legal weight, and
  that's out of scope here.
- **Deliberately no `linked_document_id` column in this pass** - wiring a requirement
  to a specific document (an "open source" button, a staleness cascade off
  `scanner.rs`) is a real feature but has no consumer yet; adding the column now would
  be a speculative, unused abstraction, cheap to add later (e.g. alongside B4) once
  something actually needs it. `status` stays entirely lawyer-driven for the same
  reason - what `stale` should mean for a checklist item (time-based? tied to a
  specific document version?) isn't yet defined by real usage.
- New "ראיות חסרות" tab in `MatterWorkspace.tsx` (`MissingEvidenceTab.tsx`),
  structurally identical to `WorkstreamsTab.tsx`.

11 new backend tests (seeding, reconcile-never-touches-collected on a case-type
change, backfill-via-list on a pre-B3 matter, key/status validation, cascade delete,
plus unit tests including a priority-lookup check) - 86/86 total at the original shape.
`cargo check/test --locked`, `npm run build`, `contract:check` (96/96, no drift),
`qa:static` (now also checking `oneTableInMatterRequirements`) all pass. Migration
re-verified idempotent via direct sqlite3 (applied 3x): 42 tables, 29 indexes, 37
triggers, `user_version=16`, integrity ok, `fk_check` clean.

**Review fix (2026-08-26), applied before any Windows CI run confirmed the original
shape** - a user code review of the just-shipped B3 milestone found a genuine
semantic gap: `priority` was computed only from the matter's current case-type Pack,
so a requirement the lawyer had manually collected outside the Pack (or one whose Pack
membership changed after collection) could surface as `status: collected` next to
`priority: not_applicable` - self-contradictory, even though the underlying
`reconcile`/status-preservation logic was correct and had never actually touched the
row wrongly. No migration needed - pure read-model logic, no schema/persistence
change, per the review's own framing. Fixed in `requirements.rs`/`models.rs` by
splitting the overloaded value into two fields, both still computed at read time and
never persisted:
- **`relevance`** (`applicable`/`not_applicable`) - whether the key currently belongs
  to the matter's Pack, or the lawyer has otherwise acted on it.
- **`priority`** (`Option<String>`, `required_by_office_policy`/`recommended`/
  `optional`) - only meaningful when relevant; `None` when not.

A key in the current Pack keeps the Pack's priority regardless of status. A key
outside the Pack still at `status: not_applicable` is `relevance: not_applicable`/
`priority: None`. A key outside the Pack that the lawyer has moved to
`not_collected`/`requested`/`collected`/`stale` becomes `relevance: applicable`/
`priority: optional` - a lawyer's own action makes the item relevant in practice
instead of displaying a contradictory state. The frontend (`MissingEvidenceTab.tsx`,
`types.ts`) was updated to show "לא רלוונטי" only when `relevance` is
`not_applicable`, the priority label otherwise. `linked_document_id` and manual-only
`stale` remain deliberately deferred, reaffirmed by the same review as still correct.

3 new tests (2 unit tests on the relevance/priority split, 1 integration regression
test reproducing the exact reported scenario) plus updated assertions on the existing
seeding test - 89/89 total. `cargo check/test --locked`, `npm run build`,
`contract:check` (96/96, no drift), `qa:static` all pass. Migration re-verified
idempotent (still 42 tables/29 indexes/37 triggers/`user_version=16` - unchanged).

**Both confirmed on real Windows CI**: the original B3 commit at
[run #18](https://github.com/yossizch-max/-2/actions/runs/32929039159), commit
`94f7c63` (86/86 tests), and this review-fix commit at
[run #19](https://github.com/yossizch-max/-2/actions/runs/32930093339), commit
`958f372` (89/89 tests). B4-B6 remain deliberately unattempted, each still pending its
own planning pass.

## Phase B, milestone B4 — Medical/Wage/Liability Ledgers (2026-08-26)

Fourth milestone of the Phase B roadmap: three parallel per-matter ledgers -
`medical_events`, `wage_records`, `liability_facts` - TAHRIR's first *evidence-
grounded, lawyer-verified* structured records, as opposed to B1-B3's plain office-
workflow bookkeeping. A ledger entry records what a cited document *says* (a medical
record, a pay stub, a police report), verified by a lawyer against the actual source
text - never a legal conclusion TAHRIR itself asserts. `liability_facts` is deliberately
named/framed as a ledger of grounded facts bearing on liability, not a determination -
consistent with the standing rule that no Israeli substantive law is encoded without a
real lawyer's validation.

- **New migration `006_matter_ledgers_v17.sql`**: three parent tables plus their own
  source-grounding child tables (`medical_event_sources`/`wage_record_sources`/
  `liability_fact_sources`), copying `legal_authorities`/`legal_authority_passages`'
  shape exactly - composite `FOREIGN KEY(entry_id,matter_id) REFERENCES
  <table>(id,matter_id)` against a parent `UNIQUE(id,matter_id)`, this schema's standard
  cross-matter-leak guard. No DB `CHECK` on free-text fields; `gross_amount_cents` does
  get a `CHECK(>=0)`, matching `damage_calculations`' own cents-field convention.
- **Lifecycle is `draft → verified`, correction-by-supersession, never a status
  mutation** - unlike `legal_authorities`' terminal-only `verified`. `verify_entry`
  requires at least one source (fails closed) and re-checks verbatim containment fresh
  against each source's *current* page text right before flipping status - the same
  recheck-at-terminal-transition discipline used by `authorities::verify`,
  `legal_rules::approve_ruleset`, and `ai::approve_proposal`. A correction never mutates
  the old verified row: it `INSERT`s a brand-new row whose `supersedes_entry_id` points
  back at the old one (a composite self-FK, structurally blocking cross-matter
  supersession the same way `legal_document_versions.damage_calculation_id`'s composite
  FK does). "Is this entry superseded" is computed at read time (does some *other*
  **verified** row point at me?) - matching B3's relevance/priority and B2's
  default-workstream idiom of deriving state at read time rather than persisting it. A
  pending, unverified draft correction deliberately does **not** yet mark the old entry
  superseded - only a verified correction actually replaces it.
- **DB-level immutability once verified**, copying `damage_calculations`' rigor (not
  `legal_authority_passages`' weaker one): a `BEFORE UPDATE` trigger blocks any field
  change on a verified row, with **one deliberate carve-out** - `stale` - so
  `scanner.rs`'s staleness cascade (extended here alongside its pre-existing
  `verified_facts` line) can still flip it when a cited document changes underneath a
  verified entry. Matching `BEFORE INSERT/UPDATE` triggers on the source child tables
  close a gap `legal_authority_passages` itself still has (no such trigger at all).
- **`DELETE` is blocked too, via a guarded escape hatch** - a verified entry and its
  sources are also protected against direct deletion, not just mutation. SQLite fires a
  child row's own `BEFORE DELETE` trigger even when the row is removed via an
  `ON DELETE CASCADE` from its parent FK (verified empirically), so a plain
  unconditional no-delete trigger would make deleting a matter with any verified ledger
  entry raise `ABORT` instead of cascading. The fix is a `ledger_delete_guard` control
  table: the delete-blocking triggers only fire when it's inactive, and
  `ledger::with_cascade_delete_guard` is the one deliberate way to flip it on for the
  duration of a whole-matter deletion. Any other `DELETE` against a verified row - a
  direct one, not wrapped in the guard - stays rejected.
- **One shared Rust engine** (`ledger.rs`, a `LedgerKind` enum dispatching table names -
  hardcoded, never user-controlled) implements `add_source`/`verify_entry`/
  `list_entry_sources`/supersession-validation generically across all three kinds;
  each kind gets its own small typed `create_*`/`update_draft_*`/`list_*` function and
  `models.rs` struct, avoiding tripling the trickiest logic while keeping each ledger's
  domain fields flat/typed (not a JSON blob), matching this schema's dominant idiom.
- New **"פנקסים" (Ledgers)** tab in `MatterWorkspace.tsx` (`LedgersTab.tsx`): three
  sections, each with a creation/correction form, a source-attachment control reusing
  the document/page-picker pattern from `AuthoritiesTab.tsx`, and verify/correct
  actions.
- **No AI-proposal integration in this pass** - deliberately deferred to B5b per the
  already-agreed roadmap, which will write into these same tables once it lands.

14 new backend tests (draft create/edit, containment rejection, verify-requires-a-
source, re-check-at-verify-time, verified-immutability with the `stale` carve-out,
source-immutability, supersession lifecycle including the pending-draft-correction
case, cross-matter-supersession-blocked, cascade-delete-even-when-verified, plus 2 pure
unit tests on `LedgerKind`) plus one existing scanner test extended to also assert the
new stale-cascade - 103/103 total at the original shape.

**Hardening fix (2026-08-26), applied before any Windows CI run confirmed the original
shape** - a user code review of the just-shipped B4 milestone found 5 real integrity
gaps (3 P0, 2 P1), all fixed in place in `006_matter_ledgers_v17.sql`/`ledger.rs`
before B5 starts feeding the AI into these ledgers:
1. **Verified evidence could be deleted directly via SQL** - the source/entry
   immutability triggers blocked `UPDATE`/`INSERT` but not `DELETE`, so a verified
   row's grounding could silently vanish. Fixed with the `ledger_delete_guard`/
   `with_cascade_delete_guard` mechanism described above, so only a deliberate
   whole-matter cascade can remove a verified row - any other delete is rejected.
2. **`stale` could be reset from 1 back to 0** on a verified row by a plain `UPDATE`,
   letting it silently reclaim "still trustworthy" without ever being re-verified.
   Fixed: the carve-out now only permits 0→1; 1→0 is blocked like every other field.
3. **`verify_entry` never checked `document_versions.stale`** - only the cited page's
   text, so a draft could be verified against a source whose underlying document had
   already changed. Fixed by joining `document_versions` and rejecting a stale source
   at verify time, mirroring `ai::approve_proposal`'s own re-check of the same flag.
4. **Two independent draft corrections of the same verified entry could both become
   verified successors**, since `validate_supersedes` only checked the *old* entry's
   status, never whether it already had a successor. Fixed with a Rust pre-check (for
   a clean error message) plus a `UNIQUE(matter_id,supersedes_entry_id) WHERE
   status='verified'` partial index as the DB-level backstop against a race or direct
   SQL bypass - multiple draft corrections may still coexist, only one can verify.
5. **`integrity_sha256` hashed only `entry_id` and the source hashes**, not the
   entry's own domain fields, so it wasn't a real snapshot of what the lawyer
   verified. Fixed with a generic per-row column snapshot (`ValueRef`-based, so the
   shared engine needs no per-kind field knowledge) folded into the hash alongside
   each source's page id, document-version id, and hash.

6 new/updated tests (unguarded-delete-rejected, guarded-cascade-still-succeeds,
stale-cannot-reset-to-0, verify-rejects-a-stale-source-version,
only-one-verified-successor, hash-reflects-domain-fields) - 106/106 total. `cargo
check/test --locked`, `npm run build`, `contract:check` (106/106, no drift),
`qa:static` (now checking `sevenTablesInMatterLedgers`, since `ledger_delete_guard` is
a 7th table in this migration) all pass. Migration re-verified idempotent via direct
sqlite3 (applied 3x): 49 tables, 35 indexes, 52 triggers, `user_version=17`
(unchanged), integrity ok, `fk_check` clean.

**Both confirmed on real Windows CI**: the original B4 commit at
[run #20](https://github.com/yossizch-max/-2/actions/runs/32932125926), commit
`7bdb60a` (103/103 tests), and this hardening-fix commit at
[run #21](https://github.com/yossizch-max/-2/actions/runs/32933104625), commit
`55616b9` (106/106 tests). B4 is fully CI-confirmed on Gate C/E.

## Phase B, milestone B5a — Focused AI Retrieval (2026-08-26)

Fifth milestone of the Phase B roadmap: a **pure retrieval-infrastructure pass** - no
ledger writes, no embeddings, local-first - that replaces `ai.rs::plan_context`'s old
behavior (a flat query for the 80 most recent non-stale pages, `capability` accepted
but never used to filter anything, no ranking or neighbor context at all) with a real,
auditable retrieval pipeline, before B5b ever writes an AI-sourced proposal into the B4
ledgers.

- **New migration `007_retrieval_context_v18.sql`** adds a local FTS5 index
  (`document_pages_fts`) over `document_pages.normalized_text` - no new application
  table (`rusqlite` 0.37 has no `fts5` Cargo feature; the bundled SQLCipher build this
  project already uses is compiled with `SQLITE_ENABLE_FTS5` on, confirmed at runtime,
  not assumed, by a permanent `fts5_is_available_in_this_sqlite_build` test that runs on
  every `cargo test --locked` including real Windows CI). Kept in sync via 3 triggers on
  `document_pages` (insert / update-of-`normalized_text` / delete) with **zero changes
  to `extraction.rs`** - the delete trigger alone correctly handles cascaded deletes too,
  since SQLite fires a child row's own triggers even for an `ON DELETE CASCADE`
  (verified empirically during the B4 hardening pass), so no guard table is needed here
  the way `ledger_delete_guard` is needed for B4's tables. One idempotent, incremental
  backfill statement covers every `document_pages` row that predates the migration.
  `unicode61 remove_diacritics 2` is documented honestly as targeting Latin-script
  diacritics only - real Unicode-aware word-boundary tokenization works today, but there
  is no Hebrew stemmer and no guaranteed nikud normalization, stated plainly rather than
  oversold.
- **New `src-tauri/src/retrieval.rs` module**, `build_context_manifest(db, matter_id,
  capability, query) -> ContextManifest`:
  - **Safe FTS5 query compilation** - free text is never handed raw to `MATCH`.
    `compile_fts_query` tokenizes on non-alphanumeric boundaries (Unicode-aware, so
    Hebrew/Arabic text passes through with no special-casing), phrase-quotes each term
    (embedded quotes escaped by doubling) and OR-joins them, so FTS5 query-syntax
    operators a lawyer might accidentally type (quotes, `AND`/`OR`/`NOT`, parens,
    `NEAR`, `*`, `^`) can never be interpreted as query syntax - parameter binding alone
    stops SQL injection but not FTS5 syntax errors.
  - **`matter_id` and `document_versions.stale=0` are re-applied against the live
    `document_pages`/`document_versions` tables at every stage**, including neighbor
    expansion - the FTS index is never trusted as authoritative for filtering, only as a
    candidate-search accelerator.
  - **Explicit deterministic sort tuple, never a blended score**: `bm25` ascending
    (lower = more relevant), category-boosted as a tie-break only (can never outrank a
    real text match), version recency, `page_number`, `block_index`, `source_id` as a
    final determinism tie-break.
  - **Page-level neighbor expansion**: for each of the top-10 ranked
    `anchor_kind='page'` candidates, adjacent `page_number±1` rows are pulled live
    (re-checked against both hard filters) and tagged `includedVia:"neighbor"` plus
    `neighborOfSourceId` recording *which* anchor pulled it in - not just that it was a
    neighbor. A page reachable both as a genuine match and as another match's neighbor
    is recorded once, as `"match"` - guaranteed by building the ordered candidate list
    as all ranked matches first, then all neighbors, never interleaved.
  - **Oversized sources are windowed, never silently dropped**: a source at or under
    8,000 chars is always sent whole (`textMode:"full"`). A larger single-row
    `anchor_kind='document'` source (a large DOCX/TXT) is deterministically windowed to
    4,000 chars, centered on the first char-index match of any query term (or
    start-of-text with no query) - same input always produces the same
    `windowStart`/`windowEnd`/`windowSha256` - while the manifest still carries the
    source's real, unchanged `sourceId`/`textSha256` for its full original text.
    `approve_proposal`'s existing re-validation still checks the live full source, never
    the window.
  - **Budget enforcement is char-based**, not row-count-based: sources are walked in
    rank order, each included if its already-decided full/window text still fits the
    remaining budget, skipped (never truncated) if not, continuing to try smaller
    sources after it.
  - **`ContextManifest` carries a canonical, non-circular `manifest_sha256`** - hashed
    over a payload type with no hash field of its own, then the result is attached to
    the public struct, so a struct can never include its own hash inside the bytes being
    hashed. `retrieval_version` is a fixed literal (`"b5a-v1"`) with no timestamp
    anywhere in the hashed payload, so identical inputs on the same DB always produce a
    byte-identical manifest and hash, every run.
  - **`capability_profile` is a mechanism, not content** - this milestone deliberately
    only populates `extract_facts` (today's one real capability, `default_query: None`,
    since general fact-extraction has no natural keyword focus - with no explicit query
    it honestly degrades to the old recency-ordered candidate list). No placeholder
    entries for `extract_medical_event`/`extract_wage_record`/`extract_liability_fact` -
    their query/category profiles are a B5b decision, not B5a's to make.
- **`ai.rs` changes**: `plan_context` is now a thin wrapper over
  `retrieval::build_context_manifest`. Both `plan_ai_context` (preview) and
  `run_ai_capability` (the real run) gained an optional `query` payload field, both
  threading it to the *same* underlying call, so preview and the real run can never
  diverge onto different candidate sets. `ai_runs.context_manifest_sha256` now reuses
  the manifest's own canonical hash directly instead of computing a second, redundant
  hash of the serialized context blob. `ai_proposals.source_manifest_json` now stores
  the **entire `ContextManifest`** (what was allowed and sent), not just its `sources`
  array - `structured_json.sourceIds` already records what the model *cited*, a
  genuinely different thing that must not be conflated with what it was *given*.
- **Frontend**: `FactsAITab.tsx` gained one optional free-text query input ("מיקוד
  לחיפוש") wired through to `run_ai_capability` - the smallest real change that lets a
  lawyer actually exercise the new ranking today.

**Two review passes shaped this design before it shipped.** A first planning pass was
sent back with 8 real defects plus 2 scope adjustments before any code was written: a
mistaken belief that `rusqlite` needed an `"fts5"` Cargo feature (corrected to a runtime
probe test instead); a missing post-upgrade backfill of pre-existing pages (would have
made every existing matter look empty to retrieval right after the upgrade); a
naive `table_count += 1` migration assertion (FTS5's own shadow tables would have broken
it); unsafe raw-text-to-`MATCH` query construction; an overclaimed Hebrew-nikud-handling
claim about `remove_diacritics 2`; a flat "never include a partial source" policy that
would have silently dropped exactly the case that matters most (a large DOCX); a
circular manifest-hash bug; a thinner audit trail than the review wanted
(`neighborOfSourceId`, storing the full manifest not just `sources`); preview/run
divergence risk; and B5b capability-profile content leaking into a B5a-scoped milestone.
Every point above already reflects the fix, not the original draft.

10 new integration tests (cross-matter isolation, stale exclusion even when it would
rank first, relevance-beats-recency, neighbor expansion with correct
`neighborOfSourceId`, oversized-document windowing that is never skipped plus
deterministic across runs, full-pipeline determinism, char-based budget enforcement,
empty/no-match behavior, every manifest source resolves to a real live page,
preview/run parity) plus 8 new unit tests in `retrieval.rs` (ranking tie-break rules,
FTS query compilation incl. Hebrew/Arabic/quotes/parens/embedded-quote-escaping,
deterministic windowing) - 127/127 total. `cargo check/test --locked`, `npm run build`,
`contract:check` (106/106, no drift - only new optional payload fields, no new
commands), `qa:static` (now also checking migration 007's virtual table/sync-triggers/
idempotent-backfill and that `retrieval.rs` never `MATCH`es a raw, uncompiled query) all
pass. Migration re-verified idempotent via direct sqlite3 (applied 3x): 49 real
application tables + 6 FTS5 shadow tables, 35 indexes, 55 triggers, `user_version=18`,
integrity ok, `fk_check` clean; insert/update/direct-delete/cascade-delete-via-matter
sync and post-migration backfill of pre-existing rows were all verified against a live
in-memory DB, not merely asserted.

**This commit has not yet had its own Windows CI run.** B5b (AI Autopilot → Ledger
Proposals) and B6 (Case Health) remain deliberately unattempted, each still pending its
own planning pass.

## Gate A, source integrity — verified by code review
- source snapshot created before extraction — `extraction.rs::extract_document` calls
  `VerifiedSourceSnapshot::create` before any parsing. ✅
- snapshot SHA equals indexed DocumentVersion SHA — `VerifiedSourceSnapshot::create`
  rejects a mismatch with `SourceShaMismatch` before a snapshot file is even kept. ✅
- extraction reads snapshot only — `extract_pdf`/`extract_docx` operate on
  `snapshot.path()`, never the original file path. ✅
- snapshot reverified before persistence — `snapshot.verify_unchanged()` runs
  immediately before the `db.write` transaction. ✅
- source mismatch creates zero `document_pages` — the mismatch errors propagate before
  the transaction that inserts pages ever opens. ✅
- provider refusal prose is never persisted — `ai.rs::extract_output_text` maps a
  refusal to `AppError::AiProviderRefusal` (a fixed error code), never storing the
  provider's free-form text. ✅

This is static code review, not a live run against real scanned/OCR'd documents —
that live check belongs to Gate F.

## Gate B, reproducible dependencies — verified, passing
- `package-lock.json` and `src-tauri/Cargo.lock` are committed and reviewed. ✅
- `npm ci` — clean install, 0 vulnerabilities from `npm audit --audit-level=high`. ✅
- `cargo check --locked` — passes. ✅
- `cargo test --locked -- --test-threads=1` — 58/58 tests pass (grown from the original
  4 as Gates F and the external-audit response added real coverage). ✅
- Build does not modify either lockfile — verified by hashing both files before and
  after `npm ci` + `cargo check --locked` + `cargo test --locked` + `npm run build`;
  identical. ✅

## Gate C, Windows OCR runtime — succeeded end-to-end (run #6, reconfirmed on current source at run #16)

`config/ocr-runtime.json` now pins a real, verified manifest (not the old fail-closed
placeholder): Tesseract 5.4.0.20240606 (UB-Mannheim, Apache-2.0), Poppler 24.08.0-0
(oschwartz10612/poppler-windows, GPL — invoked as a subprocess, never linked),
heb/ara/eng.traineddata (official tesseract-ocr/tessdata, Apache-2.0). Every URL was
actually downloaded and its SHA256 verified against the pinned value before use — not
guessed. `scripts/vendor-ocr-runtime.ps1` downloads, verifies, and stages all of it into
`src-tauri/resources/ocr/`.

**Incident 1 (2026-08-24, run #4):** the first real attempt ran the Tesseract installer
with `/VERYSILENT`, assuming it was Inno Setup. It hung for **5.5 hours** (19:41–01:18
UTC) before GitHub cancelled it — almost certainly a GUI/UAC prompt with no one on a
headless runner to answer it. Fixed by never executing the installer at all, plus adding
explicit `timeout-minutes` to every step (and a 90-minute job-level cap) so a future hang
fails fast instead of silently burning hours of runner time.

**Incident 2 (2026-08-25, run #5):** the fix above used `innoextract`, which failed
immediately with `Not a supported Inno Setup installer!` — the "Inno Setup" assumption
itself was wrong. `strings`/`file` on the downloaded installer show it is actually an
**NSIS (Nullsoft)** installer (`Nullsoft.NSIS.exehead`, "Nullsoft Install System
v3.08-3"). Re-fixed to extract with `7z x` (7-Zip, pre-installed on GitHub's Windows
runners, reads NSIS archives directly without executing them) instead — verified locally
first (`config/ocr-runtime.json`'s `installerKind` is now `"nsis"`): `7z x` on the
downloaded installer produces 139 files including `tesseract.exe` and all 57 of its
DLLs directly at the extraction root, and the poppler zip's internal
`poppler-24.08.0/Library/bin/` directory was confirmed (via `unzip -l`) to contain
`pdftotext.exe` and `pdftoppm.exe` exactly where the script's directory search expects
them — checked ahead of the next CI run instead of guessed again.

**Confirmed working (2026-08-25, run #6):** `7z`-based extraction, the whole vendoring
step, and the full `desktop:build` all succeeded on a real `windows-2025` runner
([run #6](https://github.com/yossizch-max/-2/actions/runs/32813326367), commit
`e92a148`). The "Report where OCR runtime files landed" step's log confirms all 6 files
landed exactly where the app expects them at runtime:
```
D:\a\-2\-2\src-tauri\target\release\resources\ocr\tessdata\ara.traineddata
D:\a\-2\-2\src-tauri\target\release\resources\ocr\tessdata\eng.traineddata
D:\a\-2\-2\src-tauri\target\release\resources\ocr\tessdata\heb.traineddata
D:\a\-2\-2\src-tauri\target\release\resources\ocr\vendor\poppler\pdftoppm.exe
D:\a\-2\-2\src-tauri\target\release\resources\ocr\vendor\poppler\pdftotext.exe
D:\a\-2\-2\src-tauri\target\release\resources\ocr\vendor\tesseract\tesseract.exe
```

**Reconfirmed on the current source (2026-08-25, run #9):** after the external-audit
response landed (v1 P0-1 through v1 P0-7 above), Gate C/E needed a fresh Windows run to prove
the *current* commit still builds, not just the one from three days' worth of commits
ago — see P0-7 above for the two real bugs run #7 and run #8 caught along the way (a
stale hardcoded command count, and a keyring test assertion that was actually wrong on
real Windows). [Run #9](https://github.com/yossizch-max/-2/actions/runs/32835457436),
commit `fb1ec86`, succeeded end-to-end: `cargo check`/`cargo test` (15/15) both passed
on the real runner, OCR vendoring and verification succeeded, and the same six files
landed in the same places again:
```
D:\a\-2\-2\src-tauri\target\release\resources\ocr\tessdata\ara.traineddata
D:\a\-2\-2\src-tauri\target\release\resources\ocr\tessdata\eng.traineddata
D:\a\-2\-2\src-tauri\target\release\resources\ocr\tessdata\heb.traineddata
D:\a\-2\-2\src-tauri\target\release\resources\ocr\vendor\poppler\pdftoppm.exe
D:\a\-2\-2\src-tauri\target\release\resources\ocr\vendor\poppler\pdftotext.exe
D:\a\-2\-2\src-tauri\target\release\resources\ocr\vendor\tesseract\tesseract.exe
```
Gate C's "verify final Tauri bundle contains all required runtime files" is satisfied —
the resulting installer artifact (`tahrir-windows-installer-unsigned`, run #9) is
61,398,227 bytes (~61.4MB, identical to run #6's size), up from 5.4MB before OCR
vendoring existed, consistent with the OCR runtime actually being embedded and
unchanged by the audit-response commits.

**Reconfirmed again after round-2 and round-3 fixes (2026-08-25, run #10 then run #11):**
run #10 (commit `7a3ec11`, the round-2 audit fixes) succeeded end-to-end; run #11
(commit `721cc03`, the round-3 Tier 1/2 UX fixes) then also succeeded end-to-end —
every step green including `cargo check --locked`, `cargo test --locked` (19/19, up
from 15 as round-2's regression tests were added), OCR vendoring, "Verify OCR runtime
files are present", and `npm run desktop:build`. Neither round touched OCR vendoring or
extraction code, so this reconfirms the runtime is still correctly embedded rather than
re-testing new logic. The `tahrir-windows-installer-unsigned` artifact from run #11 is
61,417,485 bytes (~61.4MB) — the small delta from run #9's 61,398,227 bytes is expected
(application code size changed; the OCR payload itself did not) and is not a regression.

**Reconfirmed again after the OCR RAII cleanup fix (2026-08-25, run #12):** this round
*did* touch `extraction.rs` (the `OcrTempDir` guard around `extract_scanned_pdf`'s
scratch directory), so this run is the real test of that change, not just a rebuild.
[Run #12](https://github.com/yossizch-max/-2/actions/runs/32866131285), commit
`d0cdb3e`, succeeded end-to-end: `cargo test --locked` (24/24, including the new
`ocr_temp_dir_is_removed_on_early_return_through_the_question_mark_operator` test and
the four new authority-passage tests) passed on the real runner, OCR vendoring and
verification both succeeded, and the installer artifact
(`tahrir-windows-installer-unsigned`) is 61,431,884 bytes — consistent with the OCR
payload being unchanged (only the cleanup logic around it changed).

**Reconfirmed again after the legal rules infrastructure Phase A landed (2026-08-25,
run #13):** this round touched schema (a new migration), a large new Rust module, new
commands and a new UI page, but nothing in the OCR vendoring/extraction path.
[Run #13](https://github.com/yossizch-max/-2/actions/runs/32877320049), commit
`64b7066`, succeeded end-to-end: `cargo test --locked` (44/44, including all 15 new
legal-rules tests) passed on the real runner, OCR vendoring and verification both
succeeded, and the installer artifact is 61,506,165 bytes — the small delta from
run #12 is expected (application code size grew; the OCR payload itself did not).

Still remaining:
1. Hebrew/Arabic/English OCR smoke tests against real scanned documents — needs the
   packaged app actually running on a real machine, which this session cannot do.
2. A human should sanity-check the distribution choices above (UB-Mannheim and
   oschwartz10612 are widely-used, reputable community builds referenced by the
   tesseract-ocr project itself, but this session picked them unilaterally under
   "continue with OCR" — flag here if a different distribution is preferred).

## Gate D, product consistency
- `aria-current` on active navigation — ✅ (`scripts/static-qa.mjs::ariaCurrent`)
- command palette `role="dialog"` + `aria-modal="true"` — ✅ (`commandPaletteDialog`)
- focus trap — ✅ (`commandPaletteFocusTrap`)
- focus restoration — ✅ (`CommandPalette.tsx` restores focus to `openerRef.current`
  on close)
- visible `:focus-visible` — ✅ (`focusVisible`, `tokens.css`)
- WCAG AA small text — **failed, then fixed.** `--muted` (used for all `<small>`/
  `.quiet` text, including this pass's new loading/error states) measured 4.36:1
  against `--canvas` in light mode — below the 4.5:1 AA minimum for normal text.
  Darkened to `#5c6670` (5.27:1 against canvas, 5.85:1 against surface; dark mode was
  already passing at 8.13:1). Verified by computing WCAG relative-luminance contrast
  directly, not by inspection.
- no stale text claiming legal-document engine is blocked — checked, none found. ✅

## Gate E, real Windows build — partially closed
`.github/workflows/windows-release-gate.yml` ran successfully end-to-end on a real
`windows-2025` GitHub Actions runner, most recently reconfirmed on the current source:
[run #16](https://github.com/yossizch-max/-2/actions/runs/32901003830), commit
`6e89cd0`, 2026-08-25 (rounds 2/3, the follow-up fixes, Phase A, its hardening pass, and
the original B1 shape were separately confirmed at
[run #10](https://github.com/yossizch-max/-2/actions/runs/32851140829) commit `7a3ec11`,
[run #11](https://github.com/yossizch-max/-2/actions/runs/32854305830) commit `721cc03`,
[run #12](https://github.com/yossizch-max/-2/actions/runs/32866131285) commit `d0cdb3e`,
[run #13](https://github.com/yossizch-max/-2/actions/runs/32877320049) commit `64b7066`,
[run #14](https://github.com/yossizch-max/-2/actions/runs/32885155747) commit `d7b3ae6`,
and [run #15](https://github.com/yossizch-max/-2/actions/runs/32892388064) commit
`16a31b3`, all same day). Build took ~33 minutes total (no cross-run cache: everything,
including SQLCipher/OpenSSL, compiles from source every run).

- Node/npm versions recorded (in the run log) ✅
- rustc/cargo versions recorded (in the run log) ✅
- frontend release build ✅
- Rust locked compile (`cargo check --locked`) ✅
- Rust locked tests (`cargo test --locked`, 66/66 as of run #16, up from 65 at run #15 as
  the B1 design-review-fix test updates were added) ✅
- NSIS bundle ✅ — produced and uploaded as the `tahrir-windows-installer-unsigned`
  Actions artifact. First produced without OCR at 5.4MB (run #3); as of
  [run #6](https://github.com/yossizch-max/-2/actions/runs/32813326367) it includes the
  full OCR runtime (see Gate C) at 61.4MB, and [run #16]
  (https://github.com/yossizch-max/-2/actions/runs/32901003830) reconfirms the artifact
  on the current source. Each run's artifact expires 14 days after that run.
- Windows code signing — ❌ not done. Needs a human with a real code-signing
  certificate; nothing in this repo can substitute for that.
- release SHA256 / release manifest / rollback package — ❌ not done. These are
  release-process steps for whoever owns distribution, not something a CI run
  produces on its own.

Two real bugs were found and fixed getting this far (neither was catchable from
Linux `cargo check`, which is why they only surfaced here):
1. `src-tauri/icons/icon.ico` was missing — `tauri-build`'s build script requires it
   on the Windows target specifically to generate the `.exe`'s resource file. Added a
   real multi-resolution ICO.
2. The workflow ran `npm run desktop:build` but never uploaded the resulting
   installer anywhere — a successful build produced a file inside the ephemeral
   runner that was discarded when the job ended. Added `actions/upload-artifact`.

Earlier note, kept for history: this session's GitHub integration initially got
`403 Resource not accessible by integration` trying to dispatch the workflow. That
resolved after the repo owner updated the GitHub App's permissions **and** the
workflow file was merged to `main` (GitHub only discovers `workflow_dispatch`
workflows that exist on the default branch).

## Phase C, milestone C1 — Document Intelligence / Smart Intake Pipeline (2026-08-28)

First post-RC0 product milestone: a lawyer adds documents to a matter, clicks one
action, and TAHRIR scans, hashes, extracts/OCRs, classifies, indexes, reports
failures, and leaves every result auditable. Built entirely on the existing
`scanner.rs`/`extraction.rs`/`document_pages`/`document_versions`/`extraction_runs`/
FTS5/`retrieval.rs` - **no parallel document, OCR, or retrieval system**, and **no
new migration** (009 was not needed - `documents.category*`,
`document_pages.extraction_*`, and `extraction_runs` already had every column this
milestone needed, unused by any prior milestone).

- **`extraction.rs` hardening**: the single-document `extract_document` (used by the
  pre-existing `extract_document_text` command) is now a thin wrapper over a new
  version-scoped `extract_document_version` core, which both it and the new batch
  pipeline share - one extraction code path, one place `document_pages`/
  `extraction_runs` are ever written. `extraction_runs` is now a genuine audit trail:
  every attempt inserts a `running` row before any file I/O or external process
  starts, and is updated to `completed`/`failed` (with a real `error_code` and
  `finished_at`) once resolved - never rewritten by a later retry, which always gets
  its own fresh row. A failed attempt also sets `document_versions.extraction_state=
  'failed'` (a new value, no schema change needed - the column was always free text)
  so a failure is visible directly through the existing `list_documents` query, and
  `case_health.rs`'s "documents needing attention" signal (previously only
  `stale`/`blocked`) was extended to include it. Seven previously-generic
  `AppError::Validation` sites were split into distinguishable typed errors
  (`UnsupportedFormat`/`PdftotextFailed`/`RasterizationFailed`/`OcrFailed`, alongside
  the pre-existing `OcrRuntimeMissing`/`SourceShaMismatch`/`SourceSnapshotChanged`),
  mapped to seven stable `error_code` strings - never a generic "failed" when the real
  cause is known. `document_pages.extraction_confidence` was investigated for
  Tesseract pages: the current invocation reads plain stdout text, not a `--tsv`/hOCR
  run with its own per-word confidence field, so populating it reliably would need a
  real rewrite of the OCR call shape - left `NULL` with that reasoning documented in
  the code, rather than fabricating a number. Native/DOCX pages were never assigned
  one either.
- **New `classification.rs`**: deterministic local document classification - a fixed-
  order keyword rule table (medical/wage/court/expert_opinion/correspondence, the
  exact term lists from the milestone spec) over filename + extracted text, returning
  `category`/`confidence`/`reason`/`signals`/`classifier_version`. Pure organization
  aid, explicitly never a source of a VerifiedFact, ledger entry, deadline, or
  liability/damage conclusion - that boundary is stated in the module's own doc
  comment. 10 unit tests, including an explicit identical-input-identical-output
  determinism test.
- **New `intake.rs`**: `process_matter_documents`, the one matter-scoped Smart Intake
  orchestrator (`scanner::hash_pending` -> discover current non-stale, non-complete
  document versions -> `extraction::extract_document_version` -> `classification::
  classify`, skipping any document whose `category_source='manual'`). A single
  document's extraction failure is caught per-document and never aborts the batch;
  an already-`complete` version is never re-extracted (though it is still re-checked
  for classification, which is cheap and idempotent, so a document extracted before
  this milestone existed still gets a category). Returns a structured summary
  (`discovered`/`hashed`/`alreadyComplete`/`extracted`/`ocred`/`classified`/`failed`/
  `unsupported` plus a per-document outcome/error array) - no fake progress. Holds the
  shared writer-mutex (`DbState::write`) only for short final persistence statements;
  `DbState::read` opens an independent connection with no lock at all; the actual
  pdftotext/pdftoppm/tesseract process execution and file hashing already happened
  entirely outside any `db.write` closure in the pre-existing `scanner.rs`/
  `extraction.rs` code this milestone reuses.
- **`DocumentsTab.tsx` upgrade**: the old separate "scan" + per-document "extract"
  buttons are replaced by one "סרוק ועבד מסמכים" action with a post-run summary line;
  each row now shows category with an "אוטומטי"/"ידני" source badge, page count,
  extraction method (native vs. OCR), and - for a failed document - a real,
  human-readable error label (`error_code` -> Hebrew string map) plus a "נסה שוב"
  retry button (calls the existing `extract_document_text`, which now goes through
  the same audited core, creating a fresh `extraction_runs` row rather than rewriting
  the failed one). A new expandable per-document detail panel (`get_document_pages` +
  the new `list_extraction_runs` command) shows every extracted page/block's text,
  method, and hash, plus the full extraction-attempt history - reading directly from
  `document_pages`, never duplicating source-of-truth text into persistent frontend
  state.
- **Windows CI**: a new smoke-test step runs immediately after the existing "OCR
  runtime files are present" check - generates a synthetic PNG (Hebrew text plus the
  fixed ASCII token `TAHRIR1234`, no client data) via `System.Drawing` and invokes the
  real vendored `tesseract.exe` against it with the exact same argument shape
  `extraction.rs` uses, asserting a zero exit code, non-empty output, and that
  `TAHRIR1234` is actually recovered. This is new: previously CI only proved the OCR
  binaries were *present*, never that they successfully *execute* text recognition.
  Deliberately does not assert Hebrew glyph accuracy, to avoid a flaky gate over
  something this milestone isn't trying to benchmark.

17 new backend tests (10 classification + 7 real-filesystem intake/audit/retrieval
integration tests in a new `intake_tests.rs`, using the same real matter/
matter_folder_bindings/on-disk-file fixture pattern `gate_f_partial.rs` established) -
covering batch extraction of a real `.txt` and a real, `zip`-crate-constructed `.docx`
fixture, matter isolation, an unsupported extension not blocking the rest of the
batch, an already-complete version never being re-extracted, a manually-set category
never being overwritten by a later intake run, a real retry sequence proving a second
`extraction_runs` row is created without touching the first (including that a
tampered-then-restored file's page content is never partially overwritten by the
rejected attempt), and that a version `scanner.rs` has marked stale is neither
re-extracted by intake nor ever returned by `retrieval::build_context_manifest`.

## Phase C, milestone C2 — Matter Understanding Core (2026-08-28)

First provider-agnostic case-understanding layer built entirely on the existing AI
architecture (`ai.rs`, `retrieval.rs`/`ContextManifest`, `ai_runs`, `ai_proposals`,
`ai_review.rs`, `verified_facts`) - no second AI pipeline, no new "agent memory"
database, and no model-generated conclusion is ever stored as trusted state. **No
migration 009** - see the reasoning below; migrations 001-008 are untouched.

**New capability `extract_matter_understanding`**: a single bundle capability whose
provider output is one JSON object with up to 7 arrays (`entities`, `events`,
`claims`, `amounts`, `dates`, `contradictions`, `suggestedQuestions`). Each array's
items are validated against their own schema and split into their own `ai_proposals`
row with its own `proposal_kind` (`understanding_entity`, `understanding_event`,
`understanding_claim`, `understanding_amount`, `understanding_date`,
`understanding_contradiction`, `understanding_question`) - all sharing one
`ai_run_id`. This is the one place a single run produces several distinct proposal
kinds, which required two small, fully backward-compatible fixes to the pre-existing
B5b machinery: `persist_completed_run`/`canonicalize_provider_output` now return
`(proposal_kind, json)` pairs instead of assuming `proposal_kind == capability`, and
`approve_proposal`'s stored-manifest capability check now reads `ai_runs.capability`
(the run's real capability) instead of the row's own `proposal_kind` - for every
pre-existing kind the two strings were always identical, so this is a bug fix with
zero behavior change for `extract_facts`/`extract_medical_event`/`extract_wage_record`/
`extract_liability_fact`, exercised by all their existing regression tests.

**Every item requires real sourceIds**, validated against the run's own
`ContextManifest` exactly like every existing capability - `parse_source_ids`/
`validate_source_ids` are reused unchanged. `confidence` is optional, bounded to
`[0,1]` when present, and is documented everywhere as model certainty only, never
legal certainty. Controlled vocabularies (`entityType`, `eventType`, `amountType`,
`dateType`) are Rust-only validated lists, matching this codebase's established
idiom (no DB `CHECK`s on free-text proposal JSON).

**Approving a Matter Understanding item writes no domain row.**
`ai_proposals.status='approved'` *is* the durable, audited, item-level "reviewed and
accepted" state:
- A **claim** never becomes a `verified_facts` row - it stays an assertion
  ("plaintiff says X" is never rewritten as "X").
- An **amount** never feeds `damage_calculations`/the Damage Engine.
- An **entity** never automatically writes `matter_parties` - linking an entity to
  an existing party or creating a new one is a separate, explicit lawyer action
  outside this generic approval path (not yet wired to a dedicated UI action in this
  milestone; `matter_profile::add_party`/`update_party` already exist for it).
  `matter_profile::ALLOWED_PARTY_ROLES` gained `government_body` to cover the
  entity taxonomy's court/government-body category.
- A **contradiction** is a review item only - it is deliberately *not* stored in the
  existing `fact_conflicts` table, because that table's `fact_a_id`/`fact_b_id`
  columns are hard foreign keys into `verified_facts`; a Matter Understanding
  contradiction is between two *pre-verification* candidate items, and forcing it
  through `fact_conflicts` would require fabricating `verified_facts` rows for
  unverified assertions, corrupting that table's meaning. `fact_conflicts` remains
  exactly what it was: post-verification conflict review between two real
  `verified_facts`.
- Item-level approval falls directly out of `ai_proposals` already being one row per
  item with its own `status` - approving one event from a bundle run leaves every
  sibling claim/amount/entity from the same run untouched at `pending`, and
  rejecting one never affects another.

**Why migration 009 was not needed**: `ai_proposals.proposal_kind`/`structured_json`
have no DB `CHECK` (free TEXT, Rust-validated) - 7 new kind strings and their JSON
shapes required zero schema change. `matter_parties` already existed with a role
taxonomy needing only one new allowed value, `government_body`, added in Rust code.
`verified_facts`, ledger tables, and `fact_conflicts` were inspected and found
*unsuitable* for representing unverified understanding items without misrepresenting
them as verified/committed state - the correct answer per the milestone's own
instruction was to *not* force them in, not to invent a parallel table that would
just duplicate what `ai_proposals` already provides for free.

**`understanding.rs`** (new module) - two pure read models, no writes, no AI calls:
- `build_matter_timeline`: unions approved `understanding_event` proposals (only
  when `eventDate` is known - "unknown stays unknown", so a dateless event is never
  given a fabricated sort key, though it stays fully visible in the review screen)
  with verified `medical_events`/`wage_records`, `insurance_claim_status_history`,
  `negotiation_events`, and `calendar_events`. Sorted strictly by business/event
  date; `insertedAt` (audit time) is a separate field, never the sort key.
  `liability_facts` has no date column and is not part of the timeline. Matter
  isolation and `status='verified'`/`status='approved'` filters are re-applied at
  every query, not trusted from any cache.
- `build_matter_brief`: a generated summary (parties, entities, chronology, claims,
  amounts, contradictions, missing-information questions, verified-fact/open-conflict
  counts) built from the same authoritative sources plus pending+approved
  understanding items - every non-approved item is labeled `pending: true` in its
  own JSON so the frontend never renders it as settled.

**Frontend**: `UnderstandingTab.tsx` (new) - one "סרוק והבן את התיק" action running
`extract_matter_understanding`, and a review queue reusing the existing
`list_ai_proposals`/`review_ai_proposal` commands, grouped into the sections the
milestone specified (Events, Entities, Claims, Amounts, Contradictions, Questions;
`understanding_date` items render inside the Events group). `MatterTimelineTab.tsx`
and `MatterBriefTab.tsx` (new, both read-only) round out the three new matter tabs
("הבנת התיק" / "ציר זמן" / "תדריך תיק"). No existing tab or command changed shape.

**Tests**: 16 new tests in `ai.rs` (entity/event/claim/amount/date/contradiction
schema and safety-boundary coverage; sourceId/stale/cross-matter rejection reused
against the new kinds; malformed-bundle fail-closed; item-level partial
approve/reject within one run; provider-extra-field stripping; canonicalization
determinism; a Windows-gated close/reopen persistence test matching the pattern
`integrity_tests::core_entities_survive_a_full_app_close_and_reopen` already
established, since a second real `DbState::open` depends on the Windows-only
keyring backend) plus 3 new tests in `understanding.rs` (business-date-not-
insertion-order sort, matter isolation, pending-content labeling in the brief).
216/216 local tests pass (`cargo test --locked -- --test-threads=1`); the Windows
Release Gate is the authority on the full count including the Windows-gated test,
not asserted here.

### C2 addendum — historical/backfill semantics, issues, event date precision (2026-08-28)

A follow-up C2 instruction expanded the milestone's scope after the section above
was written; this addendum documents what changed on top of it, on the same
`codex/c2-matter-understanding` branch, same architecture, still no migration 009.

- **New item type: `understanding_issue`** - a neutral description of a gap or open
  question in the matter (`issueType`: `liability_disputed`/`missing_response`/
  `disputed_mechanism`/`wage_loss_relevant`/`medical_continuity_unclear`/
  `missing_documentation`/`other`, plus a free-text `description`). Distinct from
  `suggestedQuestions` (a literal question to ask) and from `contradictions` (two
  specific conflicting items) - an issue is neither. Same source-grounding, same
  item-level approval, same no-domain-write-on-approval rule as every other kind.
- **Event schema gained `datePrecision`** (`exact`/`month`/`year`/`approximate`/
  `unknown`, Rust-validated) and **`documentDate`** (the date the source document
  was itself written, independent of `eventDate`, the date the event happened). The
  distinction from the ingestion/audit timestamp (`ai_runs.started_at`) is absolute
  and was already true by construction (nothing in `extract_document_version`'s or
  `ai.rs`'s pipeline ever derives a proposal field from a timestamp) - now directly
  proven by two new tests using a historical fixture date far in the past against
  the test suite's real current-time run timestamp.
- **`entities` gained an optional `context`** field (role/context string), matching
  the spec's "name / entity type / role-context / sourceIds" shape.
- **Historical vs. incremental UX**: `UnderstandingTab.tsx` labels the action button
  "בניית תמונת תיק מחומר קיים" (build a case picture from existing material) when a
  matter has zero prior Matter Understanding proposals, and "עדכן את הבנת התיק"
  (update) otherwise - purely a frontend label computed from already-fetched data,
  calling the exact same `extract_matter_understanding` command either way. No
  second data model, no second architecture, per the instruction's own constraint.
- **`understanding.rs`**: `TimelineItem` gained `datePrecision` (populated only for
  `understanding_event` rows, `None` for every domain source with an exact recorded
  date) so the Timeline can visibly distinguish an approximate/unknown-precision
  date from an exact one, surfaced in both `MatterTimelineTab.tsx` and
  `MatterBriefTab.tsx`. `build_matter_brief` gained an `issues` section
  ("מידע שלא נמצא בחומר שנקלט" - information not found in the currently ingested
  sources, never "does not exist") and a `pendingReviewCount` aggregate across every
  item kind, so "pending review items" is a real, computed brief section rather than
  an implicit property of individual items.
- **Prompt wording** (`ai.rs`'s `schema_instruction` for the bundle capability) and
  UI copy now state the "not found in currently ingested sources, never does not
  exist" boundary explicitly, matching the instruction's conceptual-boundary
  requirement.
- **9 new tests** (223/223 local total, up from 216): valid issue proposal + unknown
  `issueType` rejected; `datePrecision`/`documentDate` accepted and an invalid
  `datePrecision` rejected; event date proven to persist exactly as stated against
  the run's real (different) audit timestamp; a historical-backfill scenario proving
  the timeline sorts a 2015 event by its 2015 date, never by today's
  approval/ingestion time; an empty bundle proven to never synthesize a "does not
  exist" proposal; a rejected item proven to remain queryable in `ai_proposals` with
  its original content and review note intact; an existing `calendar_events` row
  proven to appear exactly once across repeated timeline reads (no duplication).

## Phase C, milestone C3 — Medical Evidence Intelligence (2026-08-28)

Built on `codex/c3-medical-evidence-intelligence`, branched from the exact green
C2-merge-to-main commit `d798363e4d3043b3918d98cd52abd3821b4b5168`. Same architecture
as C2 - reuses `ai_runs`/`ai_proposals`/`ai_review.rs`/B5a retrieval unchanged, no
second AI pipeline, no new "agent memory" store. **No migration 009** - same
reasoning as C2: `ai_proposals.proposal_kind`/`structured_json` are free-TEXT and
Rust-validated, so 15 new item-type schemas required zero DB schema change, and the
pre-existing Medical Ledger (`medical_events`, migration 006) is read from, never
altered or duplicated.

**New capability `extract_medical_evidence`**: a bundle capability (same pattern as
C2's `extract_matter_understanding`) whose provider output is one JSON object with
up to 15 arrays, each item validated and split into its own `ai_proposals` row:
`medical_encounter`, `medical_complaint`, `medical_finding`, `medical_diagnosis`,
`medical_test`, `medical_treatment`, `medical_medication`, `medical_referral`,
`medical_functional_status`, `medical_disability_determination`,
`medical_prior_history`, `medical_opinion`, `medical_gap_signal`,
`medical_missing_evidence_signal`, `medical_contradiction`. Its `retrieval.rs`
capability profile has **no fixed default query** (unlike the narrower, pre-existing
`extract_medical_event`) - the taxonomy is too broad for one keyword set to
represent honestly, so it boosts the `medical`/`expert_opinion` categories and relies
on a lawyer-typed query to narrow a run, exactly like `extract_matter_understanding`.

**Strict semantic separation, enforced in the type system, not just prose**: each
item type is its own `ProposalPayload` variant with only the fields that type
actually has -
- a **complaint** has no certainty/finding field at all, so it structurally cannot
  be "upgraded" into an objective **finding**;
- a **diagnosis**'s `certainty` (`suspected`/`provisional`/`differential`/
  `confirmed`/`ruled_out`) is Rust-validated against that fixed vocabulary and
  persisted verbatim - never silently changed by TAHRIR;
- a **test**'s `stage` (`ordered`/`performed`/`resulted`/`interpreted`) is a single
  required field with three independent optional date fields
  (`orderedDate`/`performedDate`/`resultDate`) - an "ordered" proposal has no
  `performedDate` at all, so an order can never itself imply the test happened;
- a **disability determination** requires a non-empty `determiningBody` - TAHRIR
  cannot structurally store a percentage without attributing it to a real
  authorized source (BTL committee, authorized/court-appointed expert), and never
  computes the percentage itself;
- a **prior-history** item and a **medical opinion** are stored as neutral
  descriptions/attributed text only - approving either writes no `verified_facts`
  row, so neither can become a TAHRIR-authored causation or "pre-existing condition"
  conclusion;
- a **gap signal** requires `startDate`/`endDate` and a fixed `signalReason`
  (`no_encounter_in_window`/`referral_without_followup`/`other`) but has no
  "outcome" field of any kind - it cannot represent a recovery/abandonment
  conclusion because the schema has nowhere to put one;
- a **missing-evidence signal** has a typed `missingType` plus free-text
  `description`, and likewise has no "confirmed absent" field - only ever a signal
  that something specific was not found in the currently ingested sources.

**Medical time model**: `eventDate`/`datePrecision` (reusing C2's `exact`/`month`/
`year`/`approximate`/`unknown` vocabulary) and `documentDate` are independent
optional fields on every dated item type, populated only from the source text -
never derived from `ai_runs.started_at` or any other ingestion/audit timestamp.
Proven directly by two new tests using a historical fixture date decades in the
past against the test suite's real current-time run timestamp, and by a dedicated
historical-backfill test asserting the Medical Timeline sorts a 2012 item by its
2012 date, never by today's approval date.

**`medical.rs`** (new module) - three pure read models, no writes, no AI calls,
mirroring `understanding.rs`'s pattern exactly:
- `build_medical_timeline`: unions approved dated medical items with verified
  `medical_events` (the pre-existing Ledger). Items whose type has no date field at
  all (complaint/finding/diagnosis/missing-evidence-signal/contradiction) appear as
  **undated**, in their own stable block - never dropped, never given today's date.
- `build_prior_vs_post_incident`: a neutral comparison only, bucketing approved
  items as documented strictly before vs. on/after `matter_profile.
  primary_event_date`. Anything whose own date is unknown, or whose comparison is
  impossible because the matter has no recorded incident date, lands in a third
  `undated` bucket rather than being guessed into either side. Never asserts
  causation - verified by an explicit test that the serialized view never contains
  the word "caused".
- `build_medical_brief`: assembles all 15 item-type sections plus a unified
  chronology, labeling every not-yet-approved item `pending: true`.

**Medical Ledger integration**: approving a C3 item writes no domain row and never
touches `medical_events` - `ai_proposals.status='approved'` is itself the durable,
item-level state, exactly like C2. A lawyer wanting a real Medical Ledger entry
still uses the pre-existing, separate `extract_medical_event`/ledger-verify flow;
C3 does not change that flow's semantics at all.

**Expert-material boundary**: satisfied by the existing three-tier separation
already inherent in the architecture - original source text lives in
`document_pages`, lawyer-approved work product lives in verified/committed domain
tables (`medical_events`, `verified_facts`), and AI-generated analysis lives in
`ai_proposals` and nowhere else. No new metadata column was needed to distinguish
these tiers; a future export/package feature can exclude `ai_proposals` content by
construction, simply by never reading from that table.

**Frontend**: `MedicalEvidenceTab.tsx` (new) - one action button labeled
"בניית תמונה רפואית מחומר קיים" on a matter's first run, "עדכון התמונה הרפואית"
afterward (a frontend label only, computed from already-fetched data - both call
the identical `extract_medical_evidence` command), and a review queue reusing
`list_ai_proposals`/`review_ai_proposal`, grouped into the 15 sections above.
`MedicalTimelineTab.tsx` (new, read-only) shows the dated chronology plus an
explicit "תאריך לא ידוע" (unknown date) section, with an in-tab toggle to the
Prior-vs-Post-Incident neutral comparison (no separate nav tab, same backend
command). `MedicalBriefTab.tsx` (new, read-only) rounds out three new matter tabs
("ראיות רפואיות" / "ציר זמן רפואי" / "תדריך רפואי"), backed by three new commands
(`get_medical_timeline`, `get_prior_vs_post_incident`, `get_medical_brief`). No
existing tab or command changed shape.

**Tests**: 26 new tests in `ai.rs` and 5 new tests in `medical.rs` (31 new, 254/254
local total up from 223), covering per-item schema validation and controlled
vocabularies; the complaint/finding/diagnosis-certainty/test-stage semantic
separations; the disability-determination authorized-source requirement; the
event-date-vs-import-timestamp and historical-backfill boundaries; sourceId/
staleness/cross-matter rejection reused against the new kinds; malformed-bundle
fail-closed handling; item-level partial approve/reject within one run; provider-
extra-field stripping; canonicalization determinism; a treatment-gap-signal test
asserting no recovery/abandonment wording ever appears; a missing-evidence-signal
schema test; a medical-contradiction two-real-sources test; an incremental-update
test proving a later document's processing never overwrites an earlier approved
item; the Timeline/Prior-vs-Post/Brief neutrality and matter-isolation tests in
`medical.rs`; and a Windows-gated close/reopen persistence test matching the
established pattern.

## Phase C, milestone C4 — Wage + Liability Evidence Intelligence (2026-08-28)

Built on `codex/c4-wage-liability-intelligence`, branched from the exact green
C3-merge-to-main commit `a05e7ffa0857c2705fd19c7ad99797274504b085`. Same architecture
as C2/C3 - reuses `ai_runs`/`ai_proposals`/`ai_review.rs`/B5a retrieval unchanged, no
second AI pipeline, no new "agent memory" store. **No migration 009** - same
reasoning as C2/C3: `ai_proposals.proposal_kind`/`structured_json` are free-TEXT and
Rust-validated, so 21 new item-type schemas across two new bundle capabilities
required zero DB schema change, and the pre-existing Wage/Liability Ledgers
(`wage_records`/`liability_facts`, migration 006) are read from, never altered or
duplicated.

**Research conclusions**: confirmed via targeted research that TAHRIR's item
taxonomy tracks real Israeli PI practice - Form 106 (טופס 106, the tax-authority
annual salary summary) is a standard document attached to loss-of-income claims
alongside recent payslips and employer confirmations; National Insurance (BTL)
payments are tracked separately from salary in real claims; and liability disputes
in Israeli tort files commonly turn on objective scene evidence (road markings,
skid marks, vehicle damage) plus conflicting witness/party versions, with
contributory negligence (רשלנות תורמת / אשם תורם) argued as a distinct question from
the underlying factual mechanism - confirming the "fact vs. claim vs. legal
conclusion" separation this milestone's schema enforces structurally.

**Two new bundle capabilities** (same pattern as C2/C3's `extract_matter_
understanding`/`extract_medical_evidence`), one per part of the spec, kept separate
rather than merged into one call because wage and liability evidence are
semantically distinct domains typically found in different documents, and a single
21-array taxonomy would be too broad for either capability's `retrieval.rs` profile
to represent honestly:
- **`extract_wage_evidence`** (Part A): up to 10 arrays - `employment`, `income`,
  `payslips`, `annualIncome`, `absences`, `sickLeaveCertificates`,
  `workLimitations`, `employmentChanges`, `benefitPayments`, `gapSignals` - each
  item validated and split into its own `ai_proposals` row (`wage_employment`
  through `wage_gap_signal`).
- **`extract_liability_evidence`** (Part B): up to 11 arrays -
  `versionStatements`, `witnessStatements`, `sceneEvidence`, `policeEvidence`,
  `vehicleDamage`, `photoVideoEvidence`, `expertOpinions`, `admissions`,
  `insurerPositions`, `courtFindings`, `contradictions` - each split into its own
  `ai_proposals` row (`liability_version_statement` through
  `liability_contradiction`).

Both capabilities' `retrieval.rs` profiles have **no fixed default query** - same
reasoning as `extract_matter_understanding`/`extract_medical_evidence` - and boost
categories `wage` (Part A) and `court`/`expert_opinion`/`correspondence` (Part B),
reusing the exact category sets already defined for the narrower, pre-existing
`extract_wage_record`/`extract_liability_fact` capabilities.

**Strict semantic separation, enforced in the type system, not just prose**:
- an **income record**'s `amountBasis` (`gross`/`net`) is Rust-validated and
  persisted verbatim - TAHRIR never derives one from the other;
- a **payslip** carries independent optional `grossAmountCents`/`netAmountCents` -
  a payslip stating only gross has a genuinely absent, not zero or estimated, net
  figure;
- an **absence** and an **employment change** each carry only a `statedReason`/
  `description` field with no causation/attribution field of any kind - approving
  either writes no `verified_facts` row, so neither can become a TAHRIR-authored
  "caused by the accident" conclusion (verified directly by tests asserting the
  serialized proposal never contains "caused");
- a **benefit/payment** (`btl`/`employer_sick_pay`/`insurance_payment`/`pension`) is
  its own `ProposalPayload` variant, structurally distinct from **income** - a BTL
  payment can never be canonicalized as salary;
- a **version statement** and a **witness statement** each require a real
  `assertedBy`/`witness` and store only a `statement` field - approving either
  writes no `verified_facts` or `liability_facts` row, so a party's or witness's
  account remains a claim, never an established fact;
- **police evidence** stores only `reportType`/`factualContent` - approving it
  writes no `liability_facts` row, so a police document is never auto-promoted into
  a legal determination;
- an **expert opinion** requires a real `expert` and stores `opinionText` as
  attributed text only - never TAHRIR's own conclusion;
- an **insurer position** is a closed `accepts`/`disputes`/`partially_accepts`/
  `no_position_stated` enum plus free-text `detail` - never equated with the truth
  by anything downstream;
- a **court finding**'s `findingType` (`interim_observation`/`factual_finding`/
  `final_judgment`/`procedural_decision`) is Rust-validated and persisted verbatim -
  an interim observation can never be silently upgraded into a final judgment;
- an **admission** requires a non-empty `statement` field holding the source's own
  language - the schema gives the model nowhere to record an admission "inferred"
  from silence or ambiguity;
- **no schema instruction across any of the 11 liability item types mentions a
  fault or negligence percentage field** - verified directly by a test scanning
  every `ProposalKind::schema_instruction()` string.

**Wage/economic time model**: `startDate`/`endDate`/`periodStart`/`periodEnd` are
populated only from source text; a payslip's `month` uses its own `YYYY-MM`
validator (`required_month_field`, distinct from the `YYYY-MM-DD` `optional_date_
field`) since a payslip document states a month, not a day; an annual-income item's
`year` uses a dedicated four-digit `required_year_field`. None are ever derived from
`ai_runs.started_at` or any ingestion/audit timestamp - proven directly by a
historical-backfill test asserting the Wage Timeline sorts a 2015 payslip by its
real 2015 month, never by today's approval date.

**`wage.rs`** (new module, Part A) - three pure read models, no writes, no AI calls,
mirroring `medical.rs`'s pattern exactly: `build_wage_timeline` (unions approved
dated wage items with verified `wage_records`, undated items in their own stable
block), `build_wage_comparison` (a neutral pre/post-incident view over
`matter_profile.primary_event_date` - never computes a loss figure, verified by a
test asserting the serialized view contains neither "caused" nor a loss-amount
field), and `build_wage_brief` (10 sections plus chronology, labeling every
not-yet-approved item `pending: true`).

**`liability.rs`** (new module, Part B) - two pure read models: `build_liability_
brief` (11 sections plus a pending-review count) and `build_liability_matrix` - a
neutral matrix grouping approved version/witness statements and scene evidence by a
shared, model-supplied `issue` label (a short free-text tag, e.g. "traffic light
color", used only to group related items, never to assign truth). `unresolved
Conflict` is a purely textual signal - true iff two or more distinct (trimmed,
case-normalized) statement texts share the same issue - TAHRIR never decides which
one is correct, only that they differ; verified directly by a test asserting the
matrix never outputs a `"winner"` or `"faultPercentage"` key. Items with no `issue`
land in an unassigned row, never dropped.

**Wage/Liability Ledger integration**: approving a C4 item writes no domain row and
never touches `wage_records`/`liability_facts` - exactly like C2/C3. A lawyer
wanting a real Ledger entry still uses the pre-existing, separate `extract_wage_
record`/`extract_liability_fact` ledger-verify flows; C4 does not change either
flow's semantics, confirmed directly by a test that exercises both the new C4 path
and the old narrow ledger path against the same matter and asserts the old path
still produces exactly one `wage_records` row.

**Damage Engine boundary**: `damage.rs` is unchanged - it remains a pure,
stateless calculation function with no `ai_proposals` awareness. C4 never writes to
`damage_inputs`, confirmed directly by a test that approves a wage income item and
asserts the matter's `damage_inputs` row count stays zero.

**Case Health**: unchanged. `case_health.rs`'s existing `SELECT COUNT(*) FROM
ai_proposals WHERE matter_id=?1 AND status='pending'` factor already counts C4's
pending items generically, with no code change needed - confirmed by inspection,
not a new test (the existing factor is capability-agnostic by construction).

**Frontend**: `WageEvidenceTab.tsx`/`LiabilityEvidenceTab.tsx` (new) - the same
first-run/update button-label pattern as C3 ("בניית תמונת שכר מחומר קיים"/"עדכון
תמונת השכר" and "בניית תמונת אחריות מחומר קיים"/"עדכון תמונת האחריות"), review
queues grouped into 10 and 11 sections. `WageTimelineTab.tsx` (new, read-only) with
an in-tab toggle to the neutral Wage Comparison view. `LiabilityBriefTab.tsx` (new,
read-only) with an in-tab toggle to the Liability Evidence Matrix. Five new matter
tabs total ("ראיות שכר" / "ציר זמן שכר" / "תדריך שכר" / "ראיות אחריות" / "תדריך
אחריות"), backed by five new commands (`get_wage_timeline`, `get_wage_comparison`,
`get_wage_brief`, `get_liability_brief`, `get_liability_matrix`). No existing tab or
command changed shape (frontend contract check: 133/133, up from 128 after C3).

**Tests**: 32 new tests in `ai.rs` plus 10 new tests in `wage.rs`/`liability.rs` (42
new; 294/294 local total up from 257 on Windows/255 on Linux before C4), covering
per-item schema validation and controlled vocabularies; the gross/net and BTL-vs-
salary separations; the absence/employment-change no-causation boundaries; the
claim/attributed-opinion/insurer-position/court-finding-type separations; the
no-fault-field schema scan; sourceId/staleness/cross-matter rejection reused
against all 21 new kinds; malformed-bundle fail-closed handling for both bundles;
item-level partial approve/reject and sibling-rejection isolation; provider-extra-
field stripping; a rejected-item-remains-auditable test; two historical-backfill
tests (wage payslip, matching the C3 pattern); two incremental-update tests proving
a later document never overwrites an earlier approved item; a Damage-Engine-
non-mutation test; a Liability-Ledger-non-mutation test; a combined test proving
both existing narrow Ledger flows remain fully functional after C4; the Wage
Timeline/Comparison and Liability Brief/Matrix neutrality and matter-isolation
tests in `wage.rs`/`liability.rs`; and two Windows-gated close/reopen persistence
tests (wage, liability) matching the established pattern.

### C4 v2 addendum — Regime-Aware Liability + Expanded Wage Taxonomy (2026-08-28)

Before this addendum's code was written, the C4 branch had not yet been pushed
and had no Windows CI run against it - the addendum below fully supersedes the
v1 design described above rather than modifying already-shipped, CI-confirmed
behavior (contrast with C2's addendum, which extended an already-merged milestone).

**Research**: confirmed via targeted web research that Israeli road-accident
bodily-injury claims are governed by the Compensation for Road Accident Victims
Law (חוק פיצויים לנפגעי תאונות דרכים) - a statutory regime that is largely
liability-independent of negligence allocation, structurally distinct from an
ordinary tort/negligence claim where duty, breach, and contributory negligence
(רשלנות תורמת / אשם תורם) are live factual/legal questions. This directly shaped
the regime-detection design below: TAHRIR must organize different evidence, and
apply different UI framing, depending on which regime a matter falls under -
never a single generic "who is at fault" model.

**Regime-aware liability, reusing the existing matter-type taxonomy**: new
`liability::liability_regime_for_matter(db, matter_id) -> AppResult<&'static str>`
is a pure, read-only classification with no persisted state of its own -
re-evaluated fresh on every call from `matters.matter_type` (the same taxonomy
`src/types.ts`'s `CASE_TYPES` already exposes; no duplicate classification model
was introduced). `traffic_accident` maps to `ftl_road_accident`; `work_accident`/
`general_negligence`/`medical_malpractice` map to `ordinary_negligence`; anything
else (including an unset matter, `civil_commercial`, `generic_civil`, `other`)
maps to `unknown_requires_review` - never guessed into either regime. Because the
regime is computed fresh from the matter's current `matter_type` and never cached
or written onto any evidence item, changing a matter's classification later can
never mutate already-extracted, already-approved liability evidence - proven
directly by a test that approves an expert-opinion item under one `matter_type`,
changes the matter to `traffic_accident`, and asserts the proposal's
`structured_json` and `status` are byte-identical before and after.

**Both the Liability Brief and Liability Evidence Matrix now surface `regime`
explicitly** in their returned JSON, and the frontend renders a regime-specific
banner: for `ftl_road_accident` it deliberately never headlines "מי אשם?" and
instead emphasizes accident facts, involved vehicles, insurance, competing
descriptions, and statutory/coverage issues requiring review; for
`unknown_requires_review` it shows "יש להגדיר/לאשר את מסלול האחריות המשפטי"
rather than guessing. No liability schema anywhere gained a fault, negligence, or
credibility field regardless of regime - verified directly by a schema-instruction
scan test (extended to also check for "credibility"), and by two regime-specific
tests confirming an FTL matter's own approved liability item and an
ordinary-negligence matter's own approved liability-issue item both remain free
of such language and never auto-write to `liability_facts`.

**New liability item kind - `LiabilityIssue`** (`issueType`:
`disputed_mechanism`/`disputed_traffic_light_state`/`disputed_driver_identity`/
`disputed_vehicle_involvement`/`disputed_employment_relationship`/
`disputed_coverage_issue`/`other`, plus a neutral `description`): a standalone,
reviewable "open factual/coverage issue" item, distinct from a `LiabilityContradiction`
(which requires two specific conflicting sourced items) - an issue can exist even
before any conflicting statement has been extracted. Reported as its own Brief
section (`liabilityIssues`) and its own review-tab section; the existing Matrix's
`issue`-tag grouping over version/witness/scene-evidence items is unchanged.
`policeEvidence.reportType` and `courtFindings.findingType` are now closed,
Rust-validated enums instead of free text: `POLICE_MATERIAL_TYPES` (police report/
investigator report/traffic examiner report/diagram/statement/photograph
reference/other) and an expanded `COURT_FINDING_TYPES` (pleading allegation/
procedural order/interim observation/evidentiary ruling/factual finding/final
judgment) - preserving the real procedural weight of a document so a mere pleading
allegation or an interim remark can never be silently upgraded into a factual
finding or a final judgment.

**Three new wage item kinds, splitting concepts the v1 design had blended**:
- **`WageEmployerConfirmation`** (`employer`, `periodStart`/`periodEnd`,
  `statedSalaryText`, `terminationReasonStated`, `jobDescription`, `hoursText`) -
  an attributed employer statement, structurally distinct from both the ledger-
  facing `WageEmployment` item and from any TAHRIR-computed salary figure.
- **`WageSelfEmployedIncome`** (`documentType`: annual tax return/P&L statement/
  tax assessment/VAT report/accountant confirmation/other, `taxYear`,
  `revenueCents`, `expensesCents`, `profitCents`) - split out from `WageAnnualIncome`
  (which now covers only Form 106/employer-certification annual totals for
  employees) specifically so revenue, expenses, and profit remain three distinct,
  never-equated fields, verified directly by a test asserting `revenueCents !=
  profitCents` in the canonical output.
- **`WagePensionContribution`** (`employerContributionCents`,
  `employeeContributionCents`, `pensionComponent`, `trainingFund`, period) - has
  no pension-loss field of any kind, verified directly by a test scanning both the
  canonical JSON and the schema instruction text for the word "loss".

**Existing wage items gained explicit provenance/precision fields** without
changing their identity: `WageIncome` gained `sourceType` (a cross-cutting
`WAGE_SOURCE_TYPES` enum - payslip/form_106/employer_confirmation/btl_record/
tax_return/tax_assessment/bank_record/claimant_statement/accountant_document/
court_finding/other - for income figures that don't map to one of the more
specific structured kinds) and `periodPrecision` (`WAGE_PERIOD_PRECISIONS`:
exact/month/quarter/tax_year/date_range/unknown, so a tax-year document is
recorded as genuinely a tax year, never fabricated into a single event date).
Two conflicting `WageIncome` items citing different `sourceType`s for the same
period are never merged or auto-resolved - both persist as independent, fully
visible approved proposals, verified directly by a test. `WagePayslip` gained
optional `overtimeCents`/`bonusCents`/`pensionContributionCents` alongside the
existing gross/net split. `WageAnnualIncome` gained `monthsWorked`, and has no
monthly-amount field of any kind, so its total can never be fabricated into a
monthly figure - verified directly by a test. `WageAbsence` gained `unitsText`;
`WageSickLeave` gained `incapacityDegreeText`. `EMPLOYMENT_CHANGE_TYPES` gained
`promotion`/`return_to_work`; `ECONOMIC_GAP_SIGNAL_TYPES` gained
`btl_income_document_missing`.

**Capability renamed** from `extract_wage_evidence` to
`extract_wage_economic_evidence` (this rename happened before the branch was ever
pushed or run on CI, so there was no compatibility concern).

**No migration** - same reasoning as the rest of C2/C3/C4: every new field and
item kind lives in the existing free-TEXT, Rust-validated `ai_proposals.
proposal_kind`/`structured_json` columns.

**Tests**: 10 new tests in this addendum (9 in `ai.rs`, 1 in `wage.rs`; 304/304
local total, up from 294) - covering the annual-income-has-no-monthly-field
schema guarantee, employer-confirmation attribution and no-Ledger-write, the
self-employed revenue/expenses/profit distinctness, the pension-contribution
no-loss-field guarantee, two-conflicting-wage-values-both-visible, the FTL
regime's no-fault-field reinforcement, the ordinary-negligence regime's
evidence-not-conclusion boundary, the unknown-regime fallback (including for an
explicit non-personal-injury matter type), the regime-change-never-mutates-
evidence guarantee, and a historical self-employed tax-year retention test in
`wage.rs` matching the existing historical-backfill pattern.

## Gate F, end-to-end synthetic acceptance — partially covered by a real automated test

The full 24-step checklist below needs a running packaged Windows app, real scanned
Hebrew/Arabic/English PDFs, a live AI provider, and a human clicking through the GUI —
most of that genuinely cannot exist in this headless Linux session. But a meaningful
subset is pure, platform-independent business logic, and rather than leave that
untested, `src-tauri/src/gate_f_partial.rs` (`cargo test gate_f_partial`, passing)
exercises it **for real** — actual `DbState`, actual SQLite/SQLCipher schema and
triggers, actual `scanner`/`extraction`/`search`/`damage`/`legal_docs` modules, no
mocking — end to end: create a matter → bind a folder → scan → hash → tamper with the
source and confirm extraction fails closed (`SourceShaMismatch`, zero pages written) →
restore and extract for real → search by matter/file/fact **and by full-text content of
the extracted page itself** → verify a fact grounded in the extracted page → calculate
and lock a damage total (and confirm the locked-calculation trigger really blocks
further mutation) → draft a legal document, add a confirmed-provenance paragraph,
approve it (and confirm the approved-version trigger really blocks mutation, and that
the parent document is marked approved) → **start a new draft version from the approved
one, confirming a document whose current version isn't approved is refused, the new
version deep-copies its sections/paragraphs/sources, the prior version and its
paragraphs stay untouched, and the parent document flips back to `draft`** → export as
text and confirm the export-audit trigger is really append-only → reopen the encrypted
DB file and confirm the matter/export/damage rows are still there.

Writing this test's step 8 and step 16 originally surfaced two real product gaps — no
full-text search over extracted document content, and no command to start a new version
of an approved legal document. Both are now fixed (`search::search` gained a
`document_pages`-backed branch; `legal_docs::create_new_version` plus the
`create_legal_document_version` command were added) and are covered by the test for
real, not just claimed — see `docs/MISSING_IMPLEMENTATION_MATRIX.md` for the full
writeup of what changed.

While building this test, it surfaced that this sandbox's `keyring` backend doesn't
actually persist an entry across separate `Entry` instances (a `set` immediately
followed by a fresh `get` fails with "No matching entry found") — confirmed directly,
not assumed. That's an environment limitation (the real target, Windows' OS Credential
Manager, does persist), not a TAHRIR bug — but it does mean step 21's DB-reopen and
step 22's "missing key recovery" get real coverage of two different things:
`DbState::open`ing a second time in this environment genuinely can't retrieve the key,
so it correctly fails closed with `RecoveryRequired` — the test asserts exactly that,
which **is** step 22's fail-closed path exercised for real, incidentally. Actual data
persistence (what step 21 is really asking) is verified separately, by reopening the
same encrypted file directly with the already-known key.

Per-step outcome:

1. Create/open Matter — ✅ real (`gate_f_partial.rs`)
2. Scan/index files — ✅ real (`scanner::scan_metadata` + `hash_pending`)
3. Change source after hash, extraction refuses it — ✅ real (`SourceShaMismatch`,
   zero `document_pages` written)
4. OCR Hebrew scanned PDF — ❌ needs the real Tesseract/Poppler Windows `.exe` binaries
   actually executing; this container can't run Windows PE binaries
5. OCR Arabic scanned PDF — ❌ same as #4
6. OCR English scanned PDF — ❌ same as #4
7. Extract native PDF — ⚠️ proxied with `.txt` extraction instead (real code path,
   wrong file format — real PDF text extraction shells out to `pdftotext.exe`, also
   blocked by #4's reasoning)
8. Search text and open source — ✅ real (`search::search`, by matter/file/fact, and now
   also by full-text content of the extracted `document_pages` themselves — the missing
   full-text branch was discovered while first writing this test, and has since been
   added and is covered by the test for real)
9. Configure local provider — ❌ needs a live local OpenAI-compatible endpoint
10. Configure OpenAI with synthetic data and explicit egress approval — ❌ needs live
    network access to a real provider
11. Run AI review — ❌ same as #10
12. Approve/reject fact proposal — ⚠️ the direct "commit a verified fact" half is
    covered for real; the AI-proposal review half needs #11 first
13. Change source and prove stale propagation — ✅ real, as of the v1 P0-1 fix above:
    `scanner::rehash_changed_versions` now sets `document_versions.stale=1` on the
    superseded version and cascades `stale=1` onto any grounded `verified_facts`;
    `integrity_tests.rs` asserts both, plus that `approve_version` refuses a document
    citing a now-stale or invalidated fact
14. Create and lock damage calculation — ✅ real, including the immutability trigger
15. Create legal draft — ✅ real
16. Edit as a new version — ✅ real (`legal_docs::create_new_version` plus the
    `create_legal_document_version` command: validates the current version is
    `approved`, deep-copies its sections/paragraphs/sources onto a new draft version,
    rejects a document whose current version isn't approved, and leaves the prior
    approved version untouched and immutable — all asserted directly by the test, not
    just code-read)
17. Confirm paragraph provenance — ✅ real (a 'confirmed' paragraph is required for
    approval to succeed; the test exercises this rather than using a degenerate
    zero-paragraph draft)
18. Approve immutable version — ✅ real, including the immutability trigger
19. Export DOCX — ⚠️ only the `.txt` export path is implemented and tested for real;
    DOCX export doesn't exist as a feature yet
20. Export PDF or show `PDF_CONVERTER_UNAVAILABLE` — ⚠️ the guard exists in
    `commands::export_legal_document` (verified by reading the source: any
    `outputKind != "txt"` returns `PdfConverterUnavailable`) but couldn't be executed
    directly — it lives behind a `tauri::State` with no public constructor outside a
    running Tauri app
21. Close/reopen and verify audit — ✅ real, see the keyring note above for how the
    two sub-concerns were separated
22. Test missing key recovery — ✅ real, incidentally, see the keyring note above
23. Test upgrade from supported older DB — N/A: there is no earlier real DB to upgrade
    from for a fresh reconstruction
24. Confirm no client-content temp files remain — ✅ real
    (`VerifiedSourceSnapshot`'s `Drop` impl; the test asserts the snapshot directory
    is empty afterward)

**Still needed to actually close this gate:** a human, on a real Windows machine, with
Gate E's packaged installer and real scanned documents, doing steps 4-6, 9-11, and
confirming 7/19/20's real (non-`.txt`) paths. Steps 16/17 no longer block this — both
are implemented and covered by the automated test.

## UX Milestone 1 — Direct Document Intake (2026-08-29)

Built on `codex/ux-m1-direct-intake`, branched from `main` at the C4 merge commit
`2323e1d44f5a1d4b1920371309e83a872b14cee6` (not from the unmerged `codex/c5-action-
orchestrator` - this UX track is independent of Phase C's own numbering). Implements
the first of a four-milestone UX redesign proposal (navigation/workflow document,
not committed to this repo) approved with four specific engineering requirements;
this change delivers exactly Milestone 1 and nothing from Milestones 2-4.

**Problem, confirmed by reading the real code before writing any:** the existing
`scanner.rs`/`matter_folder_bindings` model requires a lawyer to bind a matter to an
Office Root folder before any document is indexed - a genuine prerequisite for that
workflow, but not something normal document intake should depend on. There was no
canonical "drag one file onto a matter" path at all.

**New module `direct_intake.rs`** - the canonical direct-import path, additive only:
- `copy_verified`/`verify_copy`: hashes the source file at its original location,
  copies it into a new `AppState.documents_root/<matter_id>/<uuid>-<filename>` path
  (a fresh field, independent of `office_root` - the scanner stays fully optional),
  re-hashes the copy, and deletes it immediately on any mismatch rather than ever
  registering unverified bytes. `verify_copy` is split out specifically so this
  comparison-and-cleanup contract is unit-testable without forcing a real filesystem
  race.
- `import_one`: registers the verified copy using the *exact* existing `documents`/
  `document_versions`/`file_occurrences` model the scanner itself writes into - a
  `file_occurrence` row pointing at the managed copy (never the original Downloads/
  Desktop path), with `document_id`/`document_version_id` set immediately (no
  scan-then-hash two-step).
- `import_and_process`: imports every given path (one file's failure never blocks
  the rest of the batch), then calls the *unmodified* `intake::process_matter_
  documents` - which already discovers any document version behind a live
  `file_occurrence` regardless of how it got there, so the same extraction/OCR/
  classification pipeline a folder scan uses runs automatically, with zero
  duplicated pipeline logic.

**New commands**: `choose_document_files` (native multi-file picker, the "בחר
קבצים" fallback) and `import_document_files` (`matterId`, `paths[]` → the same
`DocumentIntakeSummary` shape `process_matter_documents` already returns, extended
with `imported`/`importErrors`).

**Frontend**: a `DirectIntakeZone` component on `OverviewTab.tsx` (today's closest
existing screen to the redesign proposal's "Matter Home") - a drop target using
`@tauri-apps/api/webview`'s `getCurrentWebview().onDragDropEvent` (real OS file
paths, Tauri v2) plus the picker button, showing plain-language status only
("נקלטו N מסמכים", "N קבצים דורשים טיפול") - no extraction-state enum, hash, or OCR
engine name anywhere in this view. A successful import calls both the tab's own
`reload` and the matter-level `reload` passed down from `MatterWorkspace.tsx`, so
document/fact counts update immediately without navigating away.

**Acceptance test** (the one specified): open TAHRIR → create a matter → drag a PDF
directly onto the matter's overview screen → automatic local processing begins with
no button press → the same screen updates on its own → the lawyer can see what was
received and what to do next. The backend half of this flow (copy/verify/register/
extract/classify) is covered end-to-end by `direct_intake.rs`'s own tests; the
interactive drag-and-drop half depends on the real Tauri webview and has not been
exercised in a live app in this environment - see the QA note below.

**Tests**: 10 new tests in `direct_intake.rs` covering provenance registration,
the managed copy's independence from the original file (deleted after import, the
occurrence still resolves), that imported content is genuinely read via the
unmodified `process_matter_documents` pipeline (a `.txt` fixture's Hebrew text is
found in `document_pages` afterward), hash-mismatch rejection and cleanup (both the
happy and unhappy path through `verify_copy` directly), a real import failure
writing nothing to the database, per-file failure isolation within a batch, cross-
matter isolation for same-named files, and that the Office Root scanner's own
tables (`matter_folder_bindings`/`matter_suggestions`/`scan_runs`) stay completely
untouched by direct import. Full suite: 314/314 local (304 pre-existing + 10 new) -
zero regressions in provenance, versioning, verification, or legal-rule tests.

**QA**: `npm ci`/`npm audit --audit-level=high` (0 vulnerabilities)/
`npm run contract:check` (135/135)/`npm run qa:static` (all checks pass)/
`npm run build`/`cargo check --locked`/`cargo test --locked -- --test-threads=1`
(314/314)/`git diff --check` (clean) all executed and green before commit. The
interactive drag-and-drop flow itself depends on the real Tauri webview/IPC bridge
and could not be exercised in a plain browser in this environment - full end-to-end
confirmation is deferred to the real Windows build, per this project's established
verification discipline for every other desktop-native feature.

**Explicitly out of scope for this change** (Milestones 2-4 of the same proposal,
not started): the AI policy gate's payload preview/cryptographic binding, the AI
findings stream, and the "עבודת התיק"/"ניסוח" workspace consolidation.
