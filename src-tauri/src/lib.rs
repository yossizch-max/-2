mod ai;
mod audit;
mod case_health;
mod commands;
mod db;
mod error;
mod ledger;
mod matter_profile;
mod negotiation;
mod retrieval;

use db::DbState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(DbState::open().expect("failed to open local database"))
        .invoke_handler(tauri::generate_handler![
            commands::create_matter,
            commands::list_matters,
            commands::get_matter,
            commands::get_matter_profile,
            commands::save_matter_profile,
            matter_profile::list_matter_parties,
            matter_profile::add_matter_party,
            matter_profile::update_matter_party,
            matter_profile::delete_matter_party,
            matter_profile::list_authorities,
            matter_profile::add_authority,
            matter_profile::update_authority,
            matter_profile::delete_authority,
            commands::ingest_document,
            commands::list_documents,
            commands::delete_document,
            commands::list_document_pages,
            commands::get_extracted_text,
            commands::create_fact,
            commands::list_facts,
            commands::delete_fact,
            commands::save_legal_deadline,
            commands::list_legal_deadlines,
            commands::delete_legal_deadline,
            commands::save_task,
            commands::list_tasks,
            commands::delete_task,
            commands::save_waiting_for,
            commands::list_waiting_for,
            commands::close_waiting_for,
            commands::save_workstream,
            commands::list_workstreams,
            commands::delete_workstream,
            ledger::create_medical_event,
            ledger::update_medical_event,
            ledger::verify_medical_event,
            ledger::supersede_medical_event,
            ledger::delete_medical_event,
            ledger::list_medical_events,
            ledger::create_wage_record,
            ledger::update_wage_record,
            ledger::verify_wage_record,
            ledger::supersede_wage_record,
            ledger::delete_wage_record,
            ledger::list_wage_records,
            ledger::create_liability_fact,
            ledger::update_liability_fact,
            ledger::verify_liability_fact,
            ledger::supersede_liability_fact,
            ledger::delete_liability_fact,
            ledger::list_liability_facts,
            ledger::list_missing_evidence_matrix,
            ai::get_ai_capabilities,
            ai::preview_ai_context,
            ai::run_ai_extract_facts,
            ai::list_ai_proposals,
            ai::approve_ai_proposal,
            ai::reject_ai_proposal,
            case_health::get_case_health,
            negotiation::list_insurance_claims,
            negotiation::save_insurance_claim,
            negotiation::change_insurance_claim_status,
            negotiation::list_insurance_claim_status_history,
            negotiation::list_negotiation_events,
            negotiation::add_negotiation_event,
            negotiation::correct_negotiation_event,
            negotiation::list_negotiation_positions,
            negotiation::add_negotiation_position,
            negotiation::correct_negotiation_position,
            negotiation::get_negotiation_snapshot,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
