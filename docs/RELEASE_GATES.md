# Release Gates

Current reconstruction verdict:
**Developer source only. No client use.**

## Gate A, source integrity
- source snapshot created before extraction
- snapshot SHA equals indexed DocumentVersion SHA
- extraction reads snapshot only
- snapshot reverified before persistence
- source mismatch creates zero `document_pages`
- provider refusal prose is never persisted

## Gate B, reproducible dependencies
- generate and review `package-lock.json`
- generate and review `Cargo.lock`
- `npm ci`
- `npm audit --audit-level=high`
- `cargo check --locked`
- `cargo test --locked -- --test-threads=1`
- build does not modify either lockfile

## Gate C, Windows OCR runtime
- pin Tesseract package/version
- verify Tesseract SHA256
- pin Poppler package/version
- verify Poppler SHA256
- verify `heb.traineddata`
- verify `ara.traineddata`
- verify `eng.traineddata`
- verify final Tauri bundle contains all required runtime files
- Hebrew OCR smoke
- Arabic OCR smoke
- English OCR smoke

## Gate D, product consistency
- `aria-current` on active navigation
- command palette uses `role="dialog"`
- command palette uses `aria-modal="true"`
- focus trap
- focus restoration
- visible `:focus-visible`
- WCAG AA small text
- no stale text claiming legal-document engine is blocked

## Gate E, real Windows build
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

## Gate F, end-to-end synthetic acceptance
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
