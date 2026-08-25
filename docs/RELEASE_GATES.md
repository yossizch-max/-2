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
fail-closed source-tamper rejection, search, fact verification, damage lock, legal-doc
approval, export+audit, DB reopen) — genuinely executed, not mocked. What's left needs a
human on a real Windows machine with Gate E's installer: real OCR, real AI provider
calls, and two features (new legal-document versions, DOCX export) that don't exist in
this reconstruction yet.**

**This is still not a client-ready release.** An unsigned installer from a
reconstruction that hasn't passed Gates C or F must not be used for real client work.

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
- `cargo test --locked -- --test-threads=1` — 4/4 tests pass. ✅
- Build does not modify either lockfile — verified by hashing both files before and
  after `npm ci` + `cargo check --locked` + `cargo test --locked` + `npm run build`;
  identical. ✅

## Gate C, Windows OCR runtime — succeeded end-to-end (run #6)

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
Gate C's "verify final Tauri bundle contains all required runtime files" is satisfied —
the resulting installer artifact (`tahrir-windows-installer-unsigned`, this run) is
61.4MB, up from 5.4MB before OCR vendoring existed, consistent with the OCR runtime
actually being embedded.

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
`windows-2025` GitHub Actions runner:
[run #3](https://github.com/yossizch-max/-2/actions/runs/32759664126), commit
`879daa3`, 2026-08-24. Build took ~41 minutes total (no cross-run cache: everything,
including SQLCipher/OpenSSL, compiles from source every run).

- Node/npm versions recorded (in the run log) ✅
- rustc/cargo versions recorded (in the run log) ✅
- frontend release build ✅
- Rust locked compile (`cargo check --locked`) ✅
- Rust locked tests (`cargo test --locked`, 4/4) ✅
- NSIS bundle ✅ — produced and uploaded as the `tahrir-windows-installer-unsigned`
  Actions artifact. First produced without OCR at 5.4MB (run #3); as of
  [run #6](https://github.com/yossizch-max/-2/actions/runs/32813326367) it includes the
  full OCR runtime (see Gate C) at 61.4MB. Each run's artifact expires 14 days after
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
13. Change source and prove stale propagation — ⚠️ re-extraction rejection is the same
    mechanism as #3, covered for real; the `document_versions.stale` column itself is
    never actually set by any of the 62 commands in this reconstruction — a real
    product gap, flagged here rather than silently worked around in the test
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
