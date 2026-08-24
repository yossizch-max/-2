# Reconstruction implementation matrix

This document distinguishes reconstructed executable handlers from command contracts that remain intentionally fail-closed.

Total command contract: **61**

Reconstructed/wired handlers: **25**

Fail-closed historical endpoints: **36**

## Wired

- `get_app_health`
- `get_office_root`
- `set_office_root`
- `scan_office_root`
- `create_matter`
- `list_matters`
- `get_matter`
- `set_matter_stage`
- `list_documents`
- `search_everything`
- `hash_pending_files`
- `extract_document_text`
- `get_document_pages`
- `list_tasks`
- `create_task`
- `complete_task`
- `save_manual_deadline`
- `commit_deadline`
- `plan_ai_context`
- `run_ai_capability`
- `verify_fact`
- `calculate_damage`
- `lock_damage_calculation`
- `save_legal_document_draft`
- `approve_legal_document`

## Fail-closed until reimplementation + tests

- `get_settings`
- `save_settings`
- `choose_folder`
- `list_scan_runs`
- `list_matter_suggestions`
- `bind_existing_matter`
- `reject_matter_suggestion`
- `update_matter`
- `get_document`
- `list_document_versions`
- `open_occurrence`
- `reveal_occurrence`
- `classify_document_manual`
- `update_task`
- `list_calendar_items`
- `create_calendar_item`
- `update_calendar_item`
- `list_waiting_for`
- `save_waiting_for`
- `close_waiting_for`
- `list_deadlines`
- `supersede_deadline`
- `get_ai_settings`
- `save_ai_settings`
- `test_ai_provider`
- `get_ai_run`
- `review_ai_proposal`
- `list_verified_facts`
- `invalidate_fact`
- `list_damage_calculations`
- `save_damage_calculation`
- `list_authorities`
- `save_authority`
- `verify_authority`
- `list_legal_documents`
- `export_legal_document`

A fail-closed endpoint returns `RECONSTRUCTED_COMMAND_NOT_YET_WIRED:<command>`. It must never be converted to a fake success response just to make the UI look complete.
