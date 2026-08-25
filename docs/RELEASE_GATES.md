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
F remains fully blocked — see that gate for what's missing.**

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
