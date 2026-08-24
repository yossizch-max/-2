# Release Gates

Current reconstruction verdict:
**Developer source only. No client use.**

Status as of this reconstruction pass (Linux dev container, no Windows machine, no
elevated CI permissions available in this session): **A and B verified below. D's
automated checks verified, one real WCAG AA failure found and fixed. C, E and F cannot
be completed from this environment — see each gate for exactly what is missing and who
needs to do it.**

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

## Gate C, Windows OCR runtime — blocked, cannot be completed from this session
`scripts/vendor-ocr-runtime.ps1` is **intentionally fail-closed**: even given a fully
filled-in `config/ocr-runtime.example.json` with real, licensed Tesseract/Poppler/
tessdata URLs and SHA256 pins, the script downloads and verifies them and then always
`throw`s with `FAIL-CLOSED: runtime extraction rules must be approved for the selected
distributions before release` — by design, so nobody ships a Windows binary blob without
a human explicitly approving *which* distribution and *how* it gets extracted/staged.

To close this gate, a human (not this session) must:
1. Pick and license/approve specific Tesseract, Poppler and tessdata (heb/ara/eng)
   Windows distributions.
2. Fill in `config/ocr-runtime.example.json` with the real pinned version/URL/SHA256
   for each.
3. Extend `vendor-ocr-runtime.ps1` with the approved extraction/staging logic for
   those specific distributions (it currently refuses to guess).
4. Run it on a Windows machine and confirm `src-tauri/resources/ocr/{vendor,tessdata}`
   is populated, then do the Hebrew/Arabic/English OCR smoke tests.

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

## Gate E, real Windows build — blocked, cannot be completed from this session
This reconstruction is a Tauri 2 app: SQLCipher, the Windows OS credential store via
`keyring`, and WebView2 are all real Windows-native dependencies that cannot be
cross-compiled with confidence from this Linux container. This gate needs an actual
Windows machine (or CI runner). The repo ships
`.github/workflows/windows-release-gate.yml` (`windows-2025` runner) for exactly this,
triggered manually (`workflow_dispatch`), but this session's GitHub integration got
`403 Resource not accessible by integration` attempting to dispatch it — it lacks
`actions: write` on this repo. To close this gate, a human needs to either:
- grant that permission so a future session can trigger it, or
- run `scripts/windows-build-gate.ps1` (or the workflow) themselves on Windows/via the
  GitHub UI's Actions tab.

Even a successful run of that workflow only produces an **unsigned** NSIS installer —
code signing, the release SHA256/manifest and the rollback package still need to be
done by whoever owns the signing certificate.
- Node/npm versions recorded
- rustc/cargo versions recorded
- frontend release build
- Rust locked compile
- Rust locked tests
- NSIS bundle
- Windows code signing
- release SHA256
- release manifest
- rollback package

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
