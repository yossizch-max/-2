# Reconstruction implementation matrix

This document distinguishes reconstructed executable handlers from command contracts that remain intentionally fail-closed.

Total command contract: **61**

Wired handlers: **61**

Fail-closed historical endpoints: **0**

All 61 commands in the Tauri contract have real handlers backed by the SQLCipher schema
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
