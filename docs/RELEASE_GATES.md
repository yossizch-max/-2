# Release Gates

Current reconstruction verdict:
**Developer source only. No client use.**

Status as of this reconstruction pass: **A and B verified. D's automated checks
verified, one real WCAG AA failure found and fixed. E: a real Windows build now
succeeds in CI and produces an unsigned installer (see Gate E for the artifact link) —
signing, release manifest and rollback package are still outstanding. C and F remain
fully blocked — see each gate for exactly what is missing and who needs to do it.**

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

## Gate C, Windows OCR runtime — in progress

`config/ocr-runtime.json` now pins a real, verified manifest (not the old fail-closed
placeholder): Tesseract 5.4.0.20240606 (UB-Mannheim, Apache-2.0), Poppler 24.08.0-0
(oschwartz10612/poppler-windows, GPL — invoked as a subprocess, never linked),
heb/ara/eng.traineddata (official tesseract-ocr/tessdata, Apache-2.0). Every URL was
actually downloaded and its SHA256 verified against the pinned value before use — not
guessed. `scripts/vendor-ocr-runtime.ps1` downloads, verifies, and stages all of it into
`src-tauri/resources/ocr/`.

**Incident (2026-08-24):** the first real attempt at this ran the Tesseract Inno Setup
installer with `/VERYSILENT` to extract its contents. It hung for **5.5 hours**
(19:41–01:18 UTC) before GitHub cancelled it — almost certainly a GUI/UAC prompt with no
one on a headless runner to answer it. Fixed by switching to `innoextract`, which reads
the installer's contents directly and **never executes it**, eliminating that failure
mode entirely. Every step in `windows-release-gate.yml` now also carries an explicit
`timeout-minutes` (and the job itself is capped at 90 minutes total) specifically so a
future hang fails fast instead of silently burning hours of runner time again.

Remaining to actually close this gate:
1. Confirm the `innoextract`-based run succeeds end-to-end on a real Windows runner
   (in progress as of this writing).
2. Confirm the OCR runtime files actually land inside the Tauri release bundle output,
   not just in `src-tauri/resources/ocr/` pre-build (Gate C's "verify final Tauri bundle
   contains all required runtime files" — the workflow reports this but doesn't yet hard
   -fail on it, since the exact Tauri v2 resource-staging path wasn't confirmed ahead of
   time).
3. Hebrew/Arabic/English OCR smoke tests against real scanned documents — needs the
   packaged app actually running, which this session cannot do.
4. A human should sanity-check the distribution choices above (UB-Mannheim and
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
  Actions artifact (5.4MB, expires 14 days after the run)
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

## Gate F, end-to-end synthetic acceptance — blocked, cannot be completed from this session
This requires a running, packaged desktop app (Gate E's output) on Windows, real
scanned Hebrew/Arabic/English PDFs, a locally-configured AI provider, and a human doing
each step below by hand. None of that exists in this headless Linux dev container.
Do this after Gates C and E are actually closed.

1. Create/open Matter.
2. Scan/index files.
3. Change source after hash and prove extraction refuses it.
4. OCR Hebrew scanned PDF.
5. OCR Arabic scanned PDF.
6. OCR English scanned PDF.
7. Extract native PDF.
8. Search text and open source.
9. Configure local provider.
10. Configure OpenAI with synthetic data and explicit egress approval.
11. Run AI review.
12. Approve/reject fact proposal.
13. Change source and prove stale propagation.
14. Create and lock damage calculation.
15. Create legal draft.
16. Edit as a new version.
17. Confirm paragraph provenance.
18. Approve immutable version.
19. Export DOCX.
20. Export PDF or show `PDF_CONVERTER_UNAVAILABLE`.
21. Close/reopen and verify audit.
22. Test missing key recovery.
23. Test upgrade from supported older DB.
24. Confirm no client-content temp files remain.
