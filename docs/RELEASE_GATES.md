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
runs, Settings UI — deliberately zero Israeli substantive law) is now built and tested
(see "Legal rules infrastructure, Phase A" below) but **not yet reconfirmed on Windows
CI** — that run is still needed before Gate C/E can be called current again. What's
left needs a human on a real Windows machine with a fresh installer: real OCR, real AI
provider calls, and DOCX export, which doesn't exist in this reconstruction yet.**

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
a CI failure.

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
- `cargo test --locked -- --test-threads=1` — 44/44 tests pass (grown from the original
  4 as Gates F and the external-audit response added real coverage). ✅
- Build does not modify either lockfile — verified by hashing both files before and
  after `npm ci` + `cargo check --locked` + `cargo test --locked` + `npm run build`;
  identical. ✅

## Gate C, Windows OCR runtime — succeeded end-to-end (run #6, reconfirmed on current source at run #12)

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
[run #12](https://github.com/yossizch-max/-2/actions/runs/32866131285), commit
`d0cdb3e`, 2026-08-25 (round-2 and round-3 fixes were separately confirmed at
[run #10](https://github.com/yossizch-max/-2/actions/runs/32851140829), commit
`7a3ec11`, and [run #11](https://github.com/yossizch-max/-2/actions/runs/32854305830),
commit `721cc03`, same day). Build took ~42 minutes total (no cross-run cache:
everything, including SQLCipher/OpenSSL, compiles from source every run).

- Node/npm versions recorded (in the run log) ✅
- rustc/cargo versions recorded (in the run log) ✅
- frontend release build ✅
- Rust locked compile (`cargo check --locked`) ✅
- Rust locked tests (`cargo test --locked`, 24/24 as of run #12, up from 19 at run #11 as
  the OCR-RAII and authority-passage regression tests were added) ✅
- NSIS bundle ✅ — produced and uploaded as the `tahrir-windows-installer-unsigned`
  Actions artifact. First produced without OCR at 5.4MB (run #3); as of
  [run #6](https://github.com/yossizch-max/-2/actions/runs/32813326367) it includes the
  full OCR runtime (see Gate C) at 61.4MB, and [run #12]
  (https://github.com/yossizch-max/-2/actions/runs/32866131285) reconfirms the artifact
  at 61,431,884 bytes on the current source. Each run's artifact expires 14 days after
  that run.
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
