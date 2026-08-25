# TAHRIR alpha.16.1, Reconstructed Canonical Source

This package reconstructs the TAHRIR product and engineering architecture from the preserved Master Specs, Workflow Catalog, alpha.16 Professional UI preview and alpha.16 Deep QA report after the original source ZIP became unavailable/empty.

**This is not a byte-for-byte recovery of the lost alpha.16 source.**

## Product

TAHRIR is a local-first legal workspace for a solo/small law office.

- Office folders remain the source of truth for original files.
- Local SQLCipher SQLite is the source of truth for knowledge created by TAHRIR.
- Every legal object is Matter-scoped.
- Document is logical identity; DocumentVersion is content identity.
- AI proposes structured, source-grounded work. A lawyer approves.
- Money and legal deadlines are deterministic, not LLM calculations.
- Critical states use versions, supersession, revocation or immutable approval.
- The product remains useful with AI disabled.

## Included code

Frontend:
- React + TypeScript
- three-pane RTL shell
- Today
- Matters
- Action Center
- Calendar
- Search
- Templates
- AI settings
- System Health
- Matter workspace
- Documents
- Verified Facts + AI Review
- Damage
- Tasks + deadlines
- Legal documents
- Authorities
- Ctrl+K command palette with dialog/focus semantics

Backend:
- Tauri 2 / Rust
- SQLCipher + OS keyring design
- ordered schema migration v12
- 33 user tables
- batched metadata scanner
- Stage B content hashing
- exact-byte OCR source snapshot + SHA verification
- PDF native extraction and OCR runtime path
- DOCX document-level provenance, never synthetic page 1
- source-text normalization for Hebrew/bidi
- provider-gated AI
- deterministic damage core in integer cents
- legal-document approval immutability
- search
- 62-command Tauri contract

## Important reconstruction status

The package contains a complete architectural source tree. It is a **developer reconstruction**:
historical alpha.16 command *internals* (exact original logic/behavior) could not be recovered
from the missing source.

All 62 commands in the Tauri contract now have real, DB-backed handlers (see
`docs/MISSING_IMPLEMENTATION_MATRIX.md` for the specific design decisions made while wiring them).
Where a real implementation would require something this reconstruction doesn't have — a
DOCX/PDF converter, for example — the command fails closed with a stable error
(`PdfConverterUnavailable`) instead of faking success.

This package must not be used for client work until the Windows release gates pass.

## Build

A real release requires regenerated lockfiles from the exact dependency manifests.

```powershell
npm install --package-lock-only
npm ci
npm run contract:check
npm run qa:static
npm run build

cd src-tauri
cargo generate-lockfile
cargo check --locked
cargo test --locked -- --test-threads=1
cd ..

npm run desktop:build
```

Then complete `docs/RELEASE_GATES.md`.
