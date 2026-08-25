# Reconstruction implementation matrix

This document distinguishes reconstructed executable handlers from command contracts that remain intentionally fail-closed.

Total command contract: **62** (the original alpha.16 61-command contract plus
`create_legal_document_version`, added after Gate F testing surfaced it as a real product gap —
see "Gaps found and fixed during Gate F" below)

Wired handlers: **62**

Fail-closed historical endpoints: **0**

All 62 commands in the Tauri contract have real handlers backed by the SQLCipher schema
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
