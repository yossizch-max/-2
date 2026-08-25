
PRAGMA foreign_keys=ON;
PRAGMA user_version=12;

CREATE TABLE IF NOT EXISTS matters(
 id TEXT PRIMARY KEY, title TEXT NOT NULL, internal_number TEXT, external_number TEXT,
 matter_type TEXT NOT NULL DEFAULT 'generic_civil', status TEXT NOT NULL DEFAULT 'active',
 workflow_stage TEXT NOT NULL DEFAULT 'intake', ai_policy TEXT NOT NULL DEFAULT 'off',
 created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS matter_folder_bindings(
 id TEXT PRIMARY KEY, matter_id TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
 path_display TEXT NOT NULL, path_key TEXT NOT NULL, binding_source TEXT NOT NULL,
 confidence REAL, active INTEGER NOT NULL DEFAULT 1, last_seen_at TEXT,
 UNIQUE(matter_id,path_key)
);
CREATE TABLE IF NOT EXISTS documents(
 id TEXT PRIMARY KEY, matter_id TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
 logical_title TEXT, category TEXT NOT NULL DEFAULT 'general', category_source TEXT,
 category_confidence REAL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
 UNIQUE(id,matter_id)
);
CREATE TABLE IF NOT EXISTS document_versions(
 id TEXT PRIMARY KEY, document_id TEXT NOT NULL, matter_id TEXT NOT NULL,
 content_sha256 TEXT, byte_size INTEGER, observed_mtime TEXT, extractor_version TEXT,
 extraction_state TEXT NOT NULL DEFAULT 'not_started', stale INTEGER NOT NULL DEFAULT 0,
 created_at TEXT NOT NULL,
 FOREIGN KEY(document_id,matter_id) REFERENCES documents(id,matter_id) ON DELETE CASCADE,
 UNIQUE(id,matter_id)
);
CREATE TABLE IF NOT EXISTS file_occurrences(
 id TEXT PRIMARY KEY, matter_id TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
 document_id TEXT, document_version_id TEXT, path_display TEXT NOT NULL, path_key TEXT NOT NULL,
 file_name TEXT NOT NULL, extension TEXT, byte_size INTEGER NOT NULL, observed_mtime TEXT NOT NULL,
 availability_state TEXT NOT NULL DEFAULT 'unknown', volume_serial TEXT, file_id_128 TEXT,
 discovered_at TEXT NOT NULL, last_seen_at TEXT NOT NULL, exists_now INTEGER NOT NULL DEFAULT 1,
 FOREIGN KEY(document_id,matter_id) REFERENCES documents(id,matter_id),
 FOREIGN KEY(document_version_id,matter_id) REFERENCES document_versions(id,matter_id),
 UNIQUE(path_key)
);
CREATE TABLE IF NOT EXISTS scan_runs(
 id TEXT PRIMARY KEY, root_path TEXT NOT NULL, status TEXT NOT NULL, started_at TEXT NOT NULL,
 finished_at TEXT, discovered_count INTEGER NOT NULL DEFAULT 0, hashed_count INTEGER NOT NULL DEFAULT 0,
 error_count INTEGER NOT NULL DEFAULT 0, cursor_path TEXT, partial INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS document_pages(
 id TEXT PRIMARY KEY, matter_id TEXT NOT NULL, document_version_id TEXT NOT NULL,
 page_number INTEGER, anchor_kind TEXT NOT NULL DEFAULT 'page', block_index INTEGER NOT NULL DEFAULT 0,
 display_text TEXT NOT NULL, normalized_text TEXT NOT NULL, text_sha256 TEXT NOT NULL,
 extraction_method TEXT NOT NULL, extraction_confidence REAL, created_at TEXT NOT NULL,
 FOREIGN KEY(document_version_id,matter_id) REFERENCES document_versions(id,matter_id) ON DELETE CASCADE,
 UNIQUE(document_version_id,anchor_kind,page_number,block_index)
);
CREATE TABLE IF NOT EXISTS extraction_runs(
 id TEXT PRIMARY KEY, matter_id TEXT NOT NULL REFERENCES matters(id), document_version_id TEXT NOT NULL,
 source_sha256 TEXT NOT NULL, status TEXT NOT NULL, error_code TEXT, started_at TEXT NOT NULL,
 finished_at TEXT, FOREIGN KEY(document_version_id,matter_id) REFERENCES document_versions(id,matter_id)
);
CREATE TABLE IF NOT EXISTS tasks(
 id TEXT PRIMARY KEY, matter_id TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
 title TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'open', due_at TEXT, risk_class TEXT NOT NULL,
 source_ref TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS legal_deadlines(
 id TEXT PRIMARY KEY, matter_id TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
 action TEXT NOT NULL, due_at TEXT NOT NULL, state TEXT NOT NULL DEFAULT 'draft',
 trigger_source_ref TEXT NOT NULL, rule_id TEXT, ruleset_version TEXT, calculation_snapshot_json TEXT,
 committed_at TEXT, superseded_by TEXT REFERENCES legal_deadlines(id), created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS calendar_events(
 id TEXT PRIMARY KEY, matter_id TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
 title TEXT NOT NULL, starts_at TEXT NOT NULL, ends_at TEXT, event_kind TEXT NOT NULL,
 source_ref TEXT, status TEXT NOT NULL DEFAULT 'active', created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS waiting_for(
 id TEXT PRIMARY KEY, matter_id TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
 party_label TEXT NOT NULL, item_label TEXT NOT NULL, since_at TEXT NOT NULL, follow_up_at TEXT,
 last_contact_at TEXT, status TEXT NOT NULL DEFAULT 'open', source_ref TEXT
);
CREATE TABLE IF NOT EXISTS ai_provider_profiles(
 id TEXT PRIMARY KEY, provider_kind TEXT NOT NULL, base_url TEXT NOT NULL, model TEXT,
 enabled INTEGER NOT NULL DEFAULT 0, client_data_authorized INTEGER NOT NULL DEFAULT 0,
 created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS ai_runs(
 id TEXT PRIMARY KEY, matter_id TEXT REFERENCES matters(id) ON DELETE CASCADE, capability TEXT NOT NULL,
 provider_profile_id TEXT REFERENCES ai_provider_profiles(id), model TEXT, status TEXT NOT NULL,
 context_manifest_sha256 TEXT NOT NULL, client_egress_approved INTEGER NOT NULL DEFAULT 0,
 started_at TEXT NOT NULL, finished_at TEXT
);
CREATE TABLE IF NOT EXISTS ai_run_chunks(
 id TEXT PRIMARY KEY, ai_run_id TEXT NOT NULL REFERENCES ai_runs(id) ON DELETE CASCADE,
 chunk_index INTEGER NOT NULL, request_sha256 TEXT NOT NULL, response_sha256 TEXT,
 status TEXT NOT NULL, error_code TEXT, UNIQUE(ai_run_id,chunk_index)
);
CREATE TABLE IF NOT EXISTS ai_proposals(
 id TEXT PRIMARY KEY, ai_run_id TEXT NOT NULL REFERENCES ai_runs(id) ON DELETE CASCADE,
 matter_id TEXT NOT NULL REFERENCES matters(id), proposal_kind TEXT NOT NULL,
 structured_json TEXT NOT NULL, source_manifest_json TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'pending',
 reviewed_at TEXT, review_note TEXT
);
CREATE TABLE IF NOT EXISTS verified_facts(
 id TEXT PRIMARY KEY, matter_id TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
 subject TEXT NOT NULL, predicate TEXT NOT NULL, value_text TEXT NOT NULL, fact_date TEXT,
 legal_relevance_tag TEXT, status TEXT NOT NULL DEFAULT 'valid', stale INTEGER NOT NULL DEFAULT 0,
 created_from_proposal_id TEXT REFERENCES ai_proposals(id), verified_at TEXT NOT NULL,
 superseded_by TEXT REFERENCES verified_facts(id), UNIQUE(id,matter_id)
);
CREATE TABLE IF NOT EXISTS verified_fact_sources(
 id TEXT PRIMARY KEY, matter_id TEXT NOT NULL, verified_fact_id TEXT NOT NULL,
 document_version_id TEXT NOT NULL, document_page_id TEXT NOT NULL, display_quote TEXT NOT NULL,
 normalized_quote TEXT NOT NULL, source_text_sha256 TEXT NOT NULL,
 FOREIGN KEY(verified_fact_id,matter_id) REFERENCES verified_facts(id,matter_id) ON DELETE CASCADE,
 FOREIGN KEY(document_version_id,matter_id) REFERENCES document_versions(id,matter_id),
 FOREIGN KEY(document_page_id) REFERENCES document_pages(id)
);
CREATE TABLE IF NOT EXISTS fact_conflicts(
 id TEXT PRIMARY KEY, matter_id TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
 fact_a_id TEXT NOT NULL REFERENCES verified_facts(id), fact_b_id TEXT NOT NULL REFERENCES verified_facts(id),
 status TEXT NOT NULL DEFAULT 'unresolved', resolution_note TEXT, created_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS damage_calculations(
 id TEXT PRIMARY KEY, matter_id TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
 regime TEXT NOT NULL, life_state TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'draft',
 gross_cents INTEGER NOT NULL DEFAULT 0 CHECK(gross_cents>=0),
 deductions_cents INTEGER NOT NULL DEFAULT 0 CHECK(deductions_cents>=0),
 net_cents INTEGER NOT NULL DEFAULT 0 CHECK(net_cents>=0),
 ruleset_id TEXT NOT NULL, ruleset_version TEXT NOT NULL, integrity_sha256 TEXT,
 locked_at TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, UNIQUE(id,matter_id)
);
CREATE TABLE IF NOT EXISTS damage_inputs(
 id TEXT PRIMARY KEY, matter_id TEXT NOT NULL, calculation_id TEXT NOT NULL,
 input_key TEXT NOT NULL, value_kind TEXT NOT NULL, value_text TEXT NOT NULL,
 source_kind TEXT NOT NULL, source_ref TEXT, legal_source TEXT, legal_source_date TEXT,
 FOREIGN KEY(calculation_id,matter_id) REFERENCES damage_calculations(id,matter_id) ON DELETE CASCADE,
 UNIQUE(calculation_id,input_key)
);
CREATE TABLE IF NOT EXISTS legal_authorities(
 id TEXT PRIMARY KEY, matter_id TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
 citation TEXT NOT NULL, title TEXT NOT NULL, court TEXT, decision_date TEXT,
 source_document_version_id TEXT, status TEXT NOT NULL DEFAULT 'draft',
 verified_at TEXT, revoked_at TEXT, integrity_sha256 TEXT,
 FOREIGN KEY(source_document_version_id,matter_id) REFERENCES document_versions(id,matter_id),
 UNIQUE(id,matter_id)
);
CREATE TABLE IF NOT EXISTS legal_authority_passages(
 id TEXT PRIMARY KEY, matter_id TEXT NOT NULL, authority_id TEXT NOT NULL,
 source_page_id TEXT, passage_text TEXT NOT NULL, passage_sha256 TEXT NOT NULL,
 issue_tag TEXT, approved INTEGER NOT NULL DEFAULT 0,
 FOREIGN KEY(authority_id,matter_id) REFERENCES legal_authorities(id,matter_id) ON DELETE CASCADE,
 FOREIGN KEY(source_page_id) REFERENCES document_pages(id)
);
CREATE TABLE IF NOT EXISTS legal_documents(
 id TEXT PRIMARY KEY, matter_id TEXT NOT NULL REFERENCES matters(id) ON DELETE CASCADE,
 document_kind TEXT NOT NULL, title TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'draft',
 current_version_id TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, UNIQUE(id,matter_id)
);
CREATE TABLE IF NOT EXISTS legal_document_versions(
 id TEXT PRIMARY KEY, matter_id TEXT NOT NULL, legal_document_id TEXT NOT NULL,
 parent_version_id TEXT REFERENCES legal_document_versions(id), version_number INTEGER NOT NULL,
 status TEXT NOT NULL DEFAULT 'draft', content_sha256 TEXT NOT NULL, damage_calculation_id TEXT,
 damage_integrity_sha256 TEXT, approval_sha256 TEXT, approved_at TEXT, created_at TEXT NOT NULL,
 FOREIGN KEY(legal_document_id,matter_id) REFERENCES legal_documents(id,matter_id) ON DELETE CASCADE,
 FOREIGN KEY(damage_calculation_id,matter_id) REFERENCES damage_calculations(id,matter_id),
 UNIQUE(id,matter_id), UNIQUE(legal_document_id,version_number)
);
CREATE TABLE IF NOT EXISTS legal_document_sections(
 id TEXT PRIMARY KEY, matter_id TEXT NOT NULL, legal_document_version_id TEXT NOT NULL,
 section_index INTEGER NOT NULL, heading TEXT NOT NULL,
 FOREIGN KEY(legal_document_version_id,matter_id) REFERENCES legal_document_versions(id,matter_id) ON DELETE CASCADE,
 UNIQUE(legal_document_version_id,section_index)
);
CREATE TABLE IF NOT EXISTS legal_document_paragraphs(
 id TEXT PRIMARY KEY, matter_id TEXT NOT NULL, legal_document_version_id TEXT NOT NULL,
 section_id TEXT NOT NULL REFERENCES legal_document_sections(id) ON DELETE CASCADE,
 paragraph_index INTEGER NOT NULL, paragraph_kind TEXT NOT NULL, body_text TEXT NOT NULL,
 provenance_state TEXT NOT NULL DEFAULT 'needs_review',
 FOREIGN KEY(legal_document_version_id,matter_id) REFERENCES legal_document_versions(id,matter_id) ON DELETE CASCADE,
 UNIQUE(legal_document_version_id,section_id,paragraph_index)
);
CREATE TABLE IF NOT EXISTS legal_document_sources(
 id TEXT PRIMARY KEY, matter_id TEXT NOT NULL, legal_document_version_id TEXT NOT NULL,
 paragraph_id TEXT NOT NULL REFERENCES legal_document_paragraphs(id) ON DELETE CASCADE,
 source_kind TEXT NOT NULL, verified_fact_id TEXT, authority_passage_id TEXT, document_page_id TEXT,
 FOREIGN KEY(legal_document_version_id,matter_id) REFERENCES legal_document_versions(id,matter_id) ON DELETE CASCADE,
 FOREIGN KEY(verified_fact_id,matter_id) REFERENCES verified_facts(id,matter_id),
 FOREIGN KEY(authority_passage_id) REFERENCES legal_authority_passages(id),
 FOREIGN KEY(document_page_id) REFERENCES document_pages(id)
);
CREATE TABLE IF NOT EXISTS office_templates(
 id TEXT PRIMARY KEY, template_kind TEXT NOT NULL, title TEXT NOT NULL, file_path TEXT NOT NULL,
 file_sha256 TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'active', created_at TEXT NOT NULL, updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS legal_export_audit(
 id TEXT PRIMARY KEY, matter_id TEXT NOT NULL REFERENCES matters(id),
 legal_document_version_id TEXT NOT NULL, output_kind TEXT NOT NULL, output_path TEXT NOT NULL,
 output_sha256 TEXT NOT NULL, exported_at TEXT NOT NULL, converter_kind TEXT,
 FOREIGN KEY(legal_document_version_id,matter_id) REFERENCES legal_document_versions(id,matter_id)
);
CREATE TABLE IF NOT EXISTS domain_events(
 id TEXT PRIMARY KEY, matter_id TEXT REFERENCES matters(id) ON DELETE CASCADE,
 event_type TEXT NOT NULL, aggregate_type TEXT NOT NULL, aggregate_id TEXT NOT NULL,
 payload_json TEXT NOT NULL, idempotency_key TEXT, created_at TEXT NOT NULL,
 UNIQUE(idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_bindings_path ON matter_folder_bindings(path_key);
CREATE INDEX IF NOT EXISTS idx_docs_matter ON documents(matter_id,category);
CREATE INDEX IF NOT EXISTS idx_versions_doc ON document_versions(document_id,created_at);
CREATE INDEX IF NOT EXISTS idx_occ_matter ON file_occurrences(matter_id,exists_now);
CREATE INDEX IF NOT EXISTS idx_occ_version ON file_occurrences(document_version_id);
CREATE INDEX IF NOT EXISTS idx_pages_version ON document_pages(document_version_id,page_number);
CREATE INDEX IF NOT EXISTS idx_tasks_matter_due ON tasks(matter_id,status,due_at);
CREATE INDEX IF NOT EXISTS idx_deadlines_due ON legal_deadlines(state,due_at);
CREATE INDEX IF NOT EXISTS idx_events_start ON calendar_events(matter_id,starts_at);
CREATE INDEX IF NOT EXISTS idx_waiting_followup ON waiting_for(status,follow_up_at);
CREATE INDEX IF NOT EXISTS idx_ai_runs_matter ON ai_runs(matter_id,started_at);
CREATE INDEX IF NOT EXISTS idx_ai_proposals_status ON ai_proposals(matter_id,status);
CREATE INDEX IF NOT EXISTS idx_facts_matter ON verified_facts(matter_id,status,stale);
CREATE INDEX IF NOT EXISTS idx_fact_sources_fact ON verified_fact_sources(verified_fact_id);
CREATE INDEX IF NOT EXISTS idx_damage_matter ON damage_calculations(matter_id,status);
CREATE INDEX IF NOT EXISTS idx_authorities_matter ON legal_authorities(matter_id,status);
CREATE INDEX IF NOT EXISTS idx_legal_docs_matter ON legal_documents(matter_id,status);
CREATE INDEX IF NOT EXISTS idx_legal_versions_doc ON legal_document_versions(legal_document_id,version_number);
CREATE INDEX IF NOT EXISTS idx_exports_doc ON legal_export_audit(legal_document_version_id,exported_at);
CREATE INDEX IF NOT EXISTS idx_domain_events_matter ON domain_events(matter_id,created_at);

CREATE TRIGGER IF NOT EXISTS trg_locked_calc_no_update BEFORE UPDATE ON damage_calculations
WHEN OLD.status='locked' BEGIN SELECT RAISE(ABORT,'LOCKED_DAMAGE_CALCULATION'); END;
CREATE TRIGGER IF NOT EXISTS trg_locked_calc_no_delete BEFORE DELETE ON damage_calculations
WHEN OLD.status='locked' BEGIN SELECT RAISE(ABORT,'LOCKED_DAMAGE_CALCULATION'); END;
CREATE TRIGGER IF NOT EXISTS trg_locked_input_no_insert BEFORE INSERT ON damage_inputs
WHEN EXISTS(SELECT 1 FROM damage_calculations c WHERE c.id=NEW.calculation_id AND c.status='locked')
BEGIN SELECT RAISE(ABORT,'LOCKED_DAMAGE_INPUTS'); END;
CREATE TRIGGER IF NOT EXISTS trg_locked_input_no_update BEFORE UPDATE ON damage_inputs
WHEN EXISTS(SELECT 1 FROM damage_calculations c WHERE c.id=OLD.calculation_id AND c.status='locked')
BEGIN SELECT RAISE(ABORT,'LOCKED_DAMAGE_INPUTS'); END;
CREATE TRIGGER IF NOT EXISTS trg_locked_input_no_delete BEFORE DELETE ON damage_inputs
WHEN EXISTS(SELECT 1 FROM damage_calculations c WHERE c.id=OLD.calculation_id AND c.status='locked')
BEGIN SELECT RAISE(ABORT,'LOCKED_DAMAGE_INPUTS'); END;

CREATE TRIGGER IF NOT EXISTS trg_verified_authority_no_update BEFORE UPDATE ON legal_authorities
WHEN OLD.status='verified' BEGIN SELECT RAISE(ABORT,'VERIFIED_AUTHORITY_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_verified_authority_no_delete BEFORE DELETE ON legal_authorities
WHEN OLD.status='verified' BEGIN SELECT RAISE(ABORT,'VERIFIED_AUTHORITY_IMMUTABLE'); END;

CREATE TRIGGER IF NOT EXISTS trg_approved_legal_version_no_update BEFORE UPDATE ON legal_document_versions
WHEN OLD.status='approved' BEGIN SELECT RAISE(ABORT,'APPROVED_LEGAL_VERSION_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_approved_legal_version_no_delete BEFORE DELETE ON legal_document_versions
WHEN OLD.status='approved' BEGIN SELECT RAISE(ABORT,'APPROVED_LEGAL_VERSION_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_approved_legal_sources_no_insert BEFORE INSERT ON legal_document_sources
WHEN EXISTS(SELECT 1 FROM legal_document_versions v WHERE v.id=NEW.legal_document_version_id AND v.status='approved')
BEGIN SELECT RAISE(ABORT,'APPROVED_LEGAL_SOURCES_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_approved_legal_sources_no_update BEFORE UPDATE ON legal_document_sources
WHEN EXISTS(SELECT 1 FROM legal_document_versions v WHERE v.id=OLD.legal_document_version_id AND v.status='approved')
BEGIN SELECT RAISE(ABORT,'APPROVED_LEGAL_SOURCES_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_approved_legal_sources_no_delete BEFORE DELETE ON legal_document_sources
WHEN EXISTS(SELECT 1 FROM legal_document_versions v WHERE v.id=OLD.legal_document_version_id AND v.status='approved')
BEGIN SELECT RAISE(ABORT,'APPROVED_LEGAL_SOURCES_IMMUTABLE'); END;

CREATE TRIGGER IF NOT EXISTS trg_approved_legal_sections_no_insert BEFORE INSERT ON legal_document_sections
WHEN EXISTS(SELECT 1 FROM legal_document_versions v WHERE v.id=NEW.legal_document_version_id AND v.status='approved')
BEGIN SELECT RAISE(ABORT,'APPROVED_LEGAL_SECTIONS_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_approved_legal_sections_no_update BEFORE UPDATE ON legal_document_sections
WHEN EXISTS(SELECT 1 FROM legal_document_versions v WHERE v.id=OLD.legal_document_version_id AND v.status='approved')
BEGIN SELECT RAISE(ABORT,'APPROVED_LEGAL_SECTIONS_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_approved_legal_sections_no_delete BEFORE DELETE ON legal_document_sections
WHEN EXISTS(SELECT 1 FROM legal_document_versions v WHERE v.id=OLD.legal_document_version_id AND v.status='approved')
BEGIN SELECT RAISE(ABORT,'APPROVED_LEGAL_SECTIONS_IMMUTABLE'); END;

CREATE TRIGGER IF NOT EXISTS trg_approved_legal_paragraphs_no_insert BEFORE INSERT ON legal_document_paragraphs
WHEN EXISTS(SELECT 1 FROM legal_document_versions v WHERE v.id=NEW.legal_document_version_id AND v.status='approved')
BEGIN SELECT RAISE(ABORT,'APPROVED_LEGAL_PARAGRAPHS_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_approved_legal_paragraphs_no_update BEFORE UPDATE ON legal_document_paragraphs
WHEN EXISTS(SELECT 1 FROM legal_document_versions v WHERE v.id=OLD.legal_document_version_id AND v.status='approved')
BEGIN SELECT RAISE(ABORT,'APPROVED_LEGAL_PARAGRAPHS_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_approved_legal_paragraphs_no_delete BEFORE DELETE ON legal_document_paragraphs
WHEN EXISTS(SELECT 1 FROM legal_document_versions v WHERE v.id=OLD.legal_document_version_id AND v.status='approved')
BEGIN SELECT RAISE(ABORT,'APPROVED_LEGAL_PARAGRAPHS_IMMUTABLE'); END;

CREATE TRIGGER IF NOT EXISTS trg_export_audit_no_update BEFORE UPDATE ON legal_export_audit BEGIN SELECT RAISE(ABORT,'EXPORT_AUDIT_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_export_audit_no_delete BEFORE DELETE ON legal_export_audit BEGIN SELECT RAISE(ABORT,'EXPORT_AUDIT_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_domain_event_no_update BEFORE UPDATE ON domain_events BEGIN SELECT RAISE(ABORT,'DOMAIN_EVENT_IMMUTABLE'); END;
CREATE TRIGGER IF NOT EXISTS trg_domain_event_no_delete BEFORE DELETE ON domain_events BEGIN SELECT RAISE(ABORT,'DOMAIN_EVENT_IMMUTABLE'); END;

CREATE TRIGGER IF NOT EXISTS trg_legal_source_matter_guard BEFORE INSERT ON legal_document_sources
WHEN NEW.verified_fact_id IS NOT NULL AND
 (SELECT matter_id FROM verified_facts WHERE id=NEW.verified_fact_id) <> NEW.matter_id
BEGIN SELECT RAISE(ABORT,'CROSS_MATTER_SOURCE'); END;

CREATE TRIGGER IF NOT EXISTS trg_legal_damage_matter_guard BEFORE INSERT ON legal_document_versions
WHEN NEW.damage_calculation_id IS NOT NULL AND
 (SELECT matter_id FROM damage_calculations WHERE id=NEW.damage_calculation_id) <> NEW.matter_id
BEGIN SELECT RAISE(ABORT,'CROSS_MATTER_DAMAGE'); END;

CREATE TABLE IF NOT EXISTS app_settings(
 id INTEGER PRIMARY KEY CHECK(id=1), settings_json TEXT NOT NULL, updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS matter_suggestions(
 id TEXT PRIMARY KEY, path_display TEXT NOT NULL, path_key TEXT NOT NULL UNIQUE,
 suggested_title TEXT NOT NULL, file_count INTEGER NOT NULL DEFAULT 0,
 status TEXT NOT NULL DEFAULT 'pending', bound_matter_id TEXT REFERENCES matters(id),
 created_at TEXT NOT NULL, resolved_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_matter_suggestions_status ON matter_suggestions(status,created_at);
