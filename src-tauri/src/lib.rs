mod ai;
mod ai_review;
mod authorities;
mod case_health;
mod classification;
mod commands;
mod damage;
mod db;
mod error;
mod extraction;
mod fact_conflicts;
#[cfg(test)]
mod gate_f_partial;
mod intake;
#[cfg(test)]
mod integrity_tests;
#[cfg(test)]
mod intake_tests;
mod ledger;
mod legal_docs;
mod legal_rules;
mod liability;
mod matter_profile;
mod medical;
mod models;
mod negotiation;
mod negotiation_ops;
mod requirements;
mod retrieval;
mod scanner;
mod search;
mod security;
mod source_snapshot;
mod understanding;
mod wage;
mod workstreams;

use db::DbState;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

pub struct AppState {
    pub db: DbState,
    pub office_root: Mutex<Option<PathBuf>>,
    pub resource_root: PathBuf,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let data_dir=app.path().app_data_dir().expect("app data dir");
            let resource_dir=app.path().resource_dir().unwrap_or_else(|_|PathBuf::from("."));
            let db=DbState::open(data_dir.join("tahrir-office.db")).expect("database initialization");
            app.manage(AppState{
                db,
                office_root:Mutex::new(None),
                resource_root:resource_dir.join("resources"),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_health,
            case_health::get_case_health,
            commands::get_settings,
            commands::save_settings,
            commands::choose_folder,
            commands::choose_save_file,
            commands::get_office_root,
            commands::set_office_root,
            commands::scan_office_root,
            commands::list_scan_runs,
            commands::list_matter_suggestions,
            commands::bind_existing_matter,
            commands::reject_matter_suggestion,
            commands::create_matter,
            commands::list_matters,
            commands::get_matter,
            commands::update_matter,
            commands::set_matter_stage,
            commands::get_matter_profile,
            commands::save_matter_profile,
            commands::list_matter_parties,
            commands::add_matter_party,
            commands::update_matter_party,
            commands::delete_matter_party,
            commands::list_matter_workstreams,
            commands::update_matter_workstream,
            commands::list_matter_requirements,
            commands::update_matter_requirement,
            commands::create_medical_event,
            commands::create_wage_record,
            commands::create_liability_fact,
            commands::update_ledger_entry_draft,
            commands::add_ledger_source,
            commands::verify_ledger_entry,
            commands::list_medical_events,
            commands::list_wage_records,
            commands::list_liability_facts,
            commands::list_ledger_entry_sources,
            commands::list_documents,
            commands::get_document,
            commands::list_document_versions,
            commands::open_occurrence,
            commands::reveal_occurrence,
            commands::search_everything,
            commands::hash_pending_files,
            commands::extract_document_text,
            commands::get_document_pages,
            commands::classify_document_manual,
            commands::process_matter_documents,
            commands::list_extraction_runs,
            commands::list_tasks,
            commands::create_task,
            commands::update_task,
            commands::complete_task,
            commands::list_calendar_items,
            commands::create_calendar_item,
            commands::update_calendar_item,
            commands::list_waiting_for,
            commands::save_waiting_for,
            negotiation_ops::close_waiting_for,
            commands::list_deadlines,
            commands::save_manual_deadline,
            commands::commit_deadline,
            commands::supersede_deadline,
            commands::get_ai_settings,
            commands::save_ai_settings,
            commands::test_ai_provider,
            commands::plan_ai_context,
            commands::run_ai_capability,
            commands::get_ai_run,
            ai_review::list_ai_proposals,
            commands::review_ai_proposal,
            commands::list_verified_facts,
            commands::verify_fact,
            commands::invalidate_fact,
            fact_conflicts::list_fact_conflicts,
            fact_conflicts::resolve_fact_conflict,
            commands::list_damage_calculations,
            commands::save_damage_calculation,
            commands::calculate_damage,
            commands::lock_damage_calculation,
            commands::list_authorities,
            commands::save_authority,
            commands::list_authority_passages,
            commands::add_authority_passage,
            commands::approve_authority_passage,
            commands::verify_authority,
            commands::list_legal_documents,
            commands::save_legal_document_draft,
            commands::approve_legal_document,
            commands::create_legal_document_version,
            commands::get_legal_document_version,
            commands::fill_legal_document_facts,
            commands::add_legal_document_paragraph,
            commands::update_legal_document_paragraph,
            commands::confirm_legal_document_paragraph,
            commands::delete_legal_document_paragraph,
            commands::export_legal_document,
            commands::list_legal_rulesets,
            commands::get_legal_ruleset,
            commands::create_legal_ruleset,
            commands::update_draft_legal_ruleset,
            commands::add_legal_ruleset_source,
            commands::add_legal_rule,
            commands::add_legal_rule_test_case,
            commands::review_legal_rule_test_case,
            commands::run_legal_rule_tests,
            commands::submit_legal_ruleset_for_review,
            commands::approve_legal_ruleset,
            commands::supersede_legal_ruleset,
            commands::preview_legal_engine_run,
            commands::commit_legal_engine_run,
            negotiation::list_insurance_claims,
            negotiation::save_insurance_claim,
            negotiation_ops::change_insurance_claim_status,
            negotiation::list_insurance_claim_status_history,
            negotiation::list_negotiation_events,
            negotiation::add_negotiation_event,
            negotiation::correct_negotiation_event,
            negotiation::list_negotiation_positions,
            negotiation::add_negotiation_position,
            negotiation::correct_negotiation_position,
            negotiation_ops::get_negotiation_snapshot,
            commands::get_matter_timeline,
            commands::get_matter_brief,
            commands::get_medical_timeline,
            commands::get_prior_vs_post_incident,
            commands::get_medical_brief,
            commands::get_wage_timeline,
            commands::get_wage_comparison,
            commands::get_wage_brief,
            commands::get_liability_brief,
            commands::get_liability_matrix
        ])
        .run(tauri::generate_context!())
        .expect("error while running TAHRIR");
}
