# Data Model

Schema version in this reconstruction: `PRAGMA user_version = 12`.

The original alpha.16 QA reported schema version 11 and 31 user tables. This reconstruction keeps the 31-table scale and increments the schema version for hardening changes.

## 33 user tables

1. `matters`
2. `matter_folder_bindings`
3. `documents`
4. `document_versions`
5. `file_occurrences`
6. `scan_runs`
7. `document_pages`
8. `extraction_runs`
9. `tasks`
10. `legal_deadlines`
11. `calendar_events`
12. `waiting_for`
13. `ai_provider_profiles`
14. `ai_runs`
15. `ai_run_chunks`
16. `ai_proposals`
17. `verified_facts`
18. `verified_fact_sources`
19. `fact_conflicts`
20. `damage_calculations`
21. `damage_inputs`
22. `legal_authorities`
23. `legal_authority_passages`
24. `legal_documents`
25. `legal_document_versions`
26. `legal_document_sections`
27. `legal_document_paragraphs`
28. `legal_document_sources`
29. `office_templates`
30. `legal_export_audit`
31. `domain_events`
32. `app_settings`
33. `matter_suggestions`

`app_settings` is a single-row key/value blob (`id=1`) for local application settings.

`matter_suggestions` tracks top-level office-root folders discovered during a scan that do not
match any active `matter_folder_bindings` path, so a lawyer can bind them to a new or existing
Matter (`bind_existing_matter`) or dismiss them (`reject_matter_suggestion`).

## Integrity principles

### Cross-Matter isolation
Composite Matter-aware foreign keys prevent a legal document in Matter A from consuming:
- a Verified Fact from Matter B
- a damage calculation from Matter B

### Damage immutability
Once `damage_calculations.status = locked`:
- calculation cannot update
- calculation cannot delete
- inputs cannot insert/update/delete

### Authority immutability
A verified authority cannot be mutated or deleted.

Revocation should be represented by a new governed transition or replacement record, not a silent edit.

### Legal-document immutability
Approved legal-document versions cannot update/delete.

Sources cannot be inserted/changed/deleted after approval.

Manual edits create child versions.

### Append-only audit
`legal_export_audit` and `domain_events` are append-only.

## Source provenance

`document_pages.page_number` is nullable because not every file format is page-native.

`anchor_kind` distinguishes:
- `page`
- `document`
- future `paragraph`
- future stable block anchoring

Original display text and normalized text are both stored.

This is required for Hebrew source verification because whitespace and bidi control characters can differ from what the lawyer sees.
