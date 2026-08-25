mod ai;
mod authorities;
mod commands;
mod damage;
mod db;
mod error;
mod extraction;
#[cfg(test)]
mod gate_f_partial;
#[cfg(test)]
mod integrity_tests;
mod legal_docs;
mod legal_rules;
mod models;
mod scanner;
mod search;
mod security;
mod source_snapshot;

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
            commands::list_tasks,
            commands::create_task,
            commands::update_task,
            commands::complete_task,
            commands::list_calendar_items,
            commands::create_calendar_item,
            commands::update_calendar_item,
            commands::list_waiting_for,
            commands::save_waiting_for,
            commands::close_waiting_for,
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
            commands::review_ai_proposal,
            commands::list_verified_facts,
            commands::verify_fact,
            commands::invalidate_fact,
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
            commands::commit_legal_engine_run
        ])
        .run(tauri::generate_context!())
        .expect("error while running TAHRIR");
}
