use crate::{
    ai, damage, extraction, legal_docs, models, scanner, search,
    error::{AppError,AppResult}, AppState
};
use chrono::Utc;
use rusqlite::params;
use serde_json::{json,Value};
use std::path::PathBuf;
use tauri::State;
use uuid::Uuid;

fn required_string<'a>(payload:&'a Value,key:&str)->AppResult<&'a str>{
    payload.get(key).and_then(Value::as_str)
        .ok_or_else(||AppError::Validation(format!("{key} required")))
}

#[tauri::command]
pub fn get_app_health(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Ok(json!({"database":"ok","sourceIndex":"ok","ocrRuntime":"runtime_checked_at_use","aiProvider":"configured_per_profile"}))
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:get_settings".into()))
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:save_settings".into()))
}

#[tauri::command]
pub fn choose_folder(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:choose_folder".into()))
}

#[tauri::command]
pub fn get_office_root(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let value=state.office_root.lock().map_err(|_|AppError::Validation("office root mutex".into()))?.clone();
    Ok(json!({"path":value.map(|x|x.to_string_lossy().to_string())}))
}
}

#[tauri::command]
pub fn set_office_root(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let path=required_string(&payload,"path")?;
    *state.office_root.lock().map_err(|_|AppError::Validation("office root mutex".into()))?=Some(PathBuf::from(path));
    Ok(json!({"ok":true}))
}
}

#[tauri::command]
pub fn scan_office_root(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let root=state.office_root.lock().map_err(|_|AppError::Validation("office root mutex".into()))?
        .clone().ok_or_else(||AppError::Validation("office root not set".into()))?;
    Ok(json!({"runId":scanner::scan_metadata(&state.db,&root)?}))
}
}

#[tauri::command]
pub fn list_scan_runs(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:list_scan_runs".into()))
}

#[tauri::command]
pub fn list_matter_suggestions(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:list_matter_suggestions".into()))
}

#[tauri::command]
pub fn bind_existing_matter(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:bind_existing_matter".into()))
}

#[tauri::command]
pub fn reject_matter_suggestion(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:reject_matter_suggestion".into()))
}

#[tauri::command]
pub fn create_matter(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let title=required_string(&payload,"title")?;
    let matter_type=payload.get("matterType").and_then(Value::as_str).unwrap_or("generic_civil");
    let internal=payload.get("internalNumber").and_then(Value::as_str);
    let id=Uuid::new_v4().to_string(); let now=Utc::now().to_rfc3339();
    state.db.write(|conn|{
        conn.execute(
            "INSERT INTO matters(id,title,internal_number,matter_type,status,workflow_stage,created_at,updated_at)
             VALUES(?1,?2,?3,?4,'active','intake',?5,?5)",
            params![id,title,internal,matter_type,now]
        )?; Ok(())
    })?;
    Ok(json!({"id":id}))
}
}

#[tauri::command]
pub fn list_matters(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    state.db.read(|conn|{
    let mut stmt=conn.prepare(
        "SELECT m.id,m.title,m.internal_number,m.external_number,m.matter_type,m.status,m.workflow_stage,
         (SELECT path_display FROM matter_folder_bindings b WHERE b.matter_id=m.id AND active=1 LIMIT 1),
         (SELECT count(*) FROM documents d WHERE d.matter_id=m.id),
         (SELECT count(*) FROM verified_facts f WHERE f.matter_id=m.id AND f.status='valid'),
         (SELECT count(*) FROM ai_proposals p WHERE p.matter_id=m.id AND p.status='pending'),
         m.updated_at FROM matters m ORDER BY m.updated_at DESC"
    )?;
    let rows=stmt.query_map([],|r|Ok(json!({
        "id":r.get::<_,String>(0)?,"title":r.get::<_,String>(1)?,
        "internalNumber":r.get::<_,Option<String>>(2)?,"externalNumber":r.get::<_,Option<String>>(3)?,
        "matterType":r.get::<_,String>(4)?,"status":r.get::<_,String>(5)?,
        "workflowStage":r.get::<_,String>(6)?,"folderPath":r.get::<_,Option<String>>(7)?,
        "documentCount":r.get::<_,i64>(8)?,"verifiedFactCount":r.get::<_,i64>(9)?,
        "pendingReviewCount":r.get::<_,i64>(10)?,"updatedAt":r.get::<_,String>(11)?
    })))?.collect::<Result<Vec<_>,_>>()?;
    Ok(Value::Array(rows))
})
}

#[tauri::command]
pub fn get_matter(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let id=required_string(&payload,"matterId")?;
    state.db.read(|conn|conn.query_row(
        "SELECT id,title,internal_number,external_number,matter_type,status,workflow_stage,updated_at FROM matters WHERE id=?1",
        [id],|r|Ok(json!({"id":r.get::<_,String>(0)?,"title":r.get::<_,String>(1)?,
        "internalNumber":r.get::<_,Option<String>>(2)?,"externalNumber":r.get::<_,Option<String>>(3)?,
        "matterType":r.get::<_,String>(4)?,"status":r.get::<_,String>(5)?,
        "workflowStage":r.get::<_,String>(6)?,"updatedAt":r.get::<_,String>(7)?}))
    ).map_err(AppError::Db))
}
}

#[tauri::command]
pub fn update_matter(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:update_matter".into()))
}

#[tauri::command]
pub fn set_matter_stage(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let matter_id=required_string(&payload,"matterId")?; let stage=required_string(&payload,"stage")?;
    state.db.write(|conn|{conn.execute(
        "UPDATE matters SET workflow_stage=?2,updated_at=?3 WHERE id=?1",
        params![matter_id,stage,Utc::now().to_rfc3339()]
    )?;Ok(())})?;
    Ok(json!({"ok":true}))
}
}

#[tauri::command]
pub fn list_documents(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let matter_id=required_string(&payload,"matterId")?;
    state.db.read(|conn|{
        let mut stmt=conn.prepare(
            "SELECT d.id,d.matter_id,coalesce(o.file_name,d.logical_title,''),d.category,
             coalesce(o.availability_state,'unknown'),coalesce(v.extraction_state,'not_started'),
             v.id,v.content_sha256,coalesce(o.observed_mtime,'')
             FROM documents d LEFT JOIN document_versions v ON v.document_id=d.id AND v.matter_id=d.matter_id
             LEFT JOIN file_occurrences o ON o.document_version_id=v.id
             WHERE d.matter_id=?1 GROUP BY d.id ORDER BY d.updated_at DESC"
        )?;
        let rows=stmt.query_map([matter_id],|r|Ok(json!({
            "id":r.get::<_,String>(0)?,"matterId":r.get::<_,String>(1)?,"fileName":r.get::<_,String>(2)?,
            "category":r.get::<_,String>(3)?,"sourceState":r.get::<_,String>(4)?,
            "extractionState":r.get::<_,String>(5)?,"currentVersionId":r.get::<_,Option<String>>(6)?,
            "currentSha256":r.get::<_,Option<String>>(7)?,"modifiedAt":r.get::<_,String>(8)?
        })))?.collect::<Result<Vec<_>,_>>()?;
        Ok(Value::Array(rows))
    })
}
}

#[tauri::command]
pub fn get_document(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:get_document".into()))
}

#[tauri::command]
pub fn list_document_versions(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:list_document_versions".into()))
}

#[tauri::command]
pub fn open_occurrence(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:open_occurrence".into()))
}

#[tauri::command]
pub fn reveal_occurrence(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:reveal_occurrence".into()))
}

#[tauri::command]
pub fn search_everything(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let query=required_string(&payload,"query")?;
    Ok(serde_json::to_value(search::search(&state.db,query)?)?)
}
}

#[tauri::command]
pub fn hash_pending_files(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let matter_id=required_string(&payload,"matterId")?;
    Ok(json!({"hashed":scanner::hash_pending(&state.db,matter_id)?}))
}
}

#[tauri::command]
pub fn extract_document_text(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let document_id=required_string(&payload,"documentId")?;
    Ok(json!({"blocks":extraction::extract_document(&state.db,document_id,&state.resource_root)?}))
}
}

#[tauri::command]
pub fn get_document_pages(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let version_id=required_string(&payload,"documentVersionId")?;
    state.db.read(|conn|{
        let mut stmt=conn.prepare(
            "SELECT id,page_number,anchor_kind,block_index,display_text,text_sha256,extraction_method
             FROM document_pages WHERE document_version_id=?1 ORDER BY page_number,block_index"
        )?;
        let rows=stmt.query_map([version_id],|r|Ok(json!({
            "id":r.get::<_,String>(0)?,"pageNumber":r.get::<_,Option<i64>>(1)?,
            "anchorKind":r.get::<_,String>(2)?,"blockIndex":r.get::<_,i64>(3)?,
            "text":r.get::<_,String>(4)?,"textSha256":r.get::<_,String>(5)?,
            "method":r.get::<_,String>(6)?
        })))?.collect::<Result<Vec<_>,_>>()?;
        Ok(Value::Array(rows))
    })
}
}

#[tauri::command]
pub fn classify_document_manual(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:classify_document_manual".into()))
}

#[tauri::command]
pub fn list_tasks(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let matter_id=payload.get("matterId").and_then(Value::as_str);
    state.db.read(|conn|{
        let sql=if matter_id.is_some(){
            "SELECT id,matter_id,title,due_at,status,risk_class FROM tasks WHERE matter_id=?1 ORDER BY due_at"
        }else{
            "SELECT id,matter_id,title,due_at,status,risk_class FROM tasks WHERE ?1 IS NULL ORDER BY due_at"
        };
        let mut stmt=conn.prepare(sql)?;
        let rows=stmt.query_map([matter_id],|r|Ok(json!({
            "id":r.get::<_,String>(0)?,"matterId":r.get::<_,String>(1)?,"title":r.get::<_,String>(2)?,
            "dueAt":r.get::<_,Option<String>>(3)?,"status":r.get::<_,String>(4)?,"riskClass":r.get::<_,String>(5)?
        })))?.collect::<Result<Vec<_>,_>>()?;Ok(Value::Array(rows))
    })
}
}

#[tauri::command]
pub fn create_task(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let matter_id=required_string(&payload,"matterId")?; let title=required_string(&payload,"title")?;
    let due=payload.get("dueAt").and_then(Value::as_str); let risk=payload.get("riskClass").and_then(Value::as_str).unwrap_or("one_click");
    let id=Uuid::new_v4().to_string(); let now=Utc::now().to_rfc3339();
    state.db.write(|conn|{conn.execute(
        "INSERT INTO tasks(id,matter_id,title,status,due_at,risk_class,created_at,updated_at)
         VALUES(?1,?2,?3,'open',?4,?5,?6,?6)",
        params![id,matter_id,title,due,risk,now]
    )?;Ok(())})?;Ok(json!({"id":id}))
}
}

#[tauri::command]
pub fn update_task(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:update_task".into()))
}

#[tauri::command]
pub fn complete_task(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let id=required_string(&payload,"taskId")?;
    state.db.write(|conn|{conn.execute(
        "UPDATE tasks SET status='done',updated_at=?2 WHERE id=?1",
        params![id,Utc::now().to_rfc3339()]
    )?;Ok(())})?;Ok(json!({"ok":true}))
}
}

#[tauri::command]
pub fn list_calendar_items(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:list_calendar_items".into()))
}

#[tauri::command]
pub fn create_calendar_item(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:create_calendar_item".into()))
}

#[tauri::command]
pub fn update_calendar_item(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:update_calendar_item".into()))
}

#[tauri::command]
pub fn list_waiting_for(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:list_waiting_for".into()))
}

#[tauri::command]
pub fn save_waiting_for(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:save_waiting_for".into()))
}

#[tauri::command]
pub fn close_waiting_for(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:close_waiting_for".into()))
}

#[tauri::command]
pub fn list_deadlines(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:list_deadlines".into()))
}

#[tauri::command]
pub fn save_manual_deadline(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let matter_id=required_string(&payload,"matterId")?; let action=required_string(&payload,"action")?;
    let due=required_string(&payload,"dueAt")?; let source=required_string(&payload,"triggerSourceRef")?;
    let id=Uuid::new_v4().to_string();
    state.db.write(|conn|{conn.execute(
        "INSERT INTO legal_deadlines(id,matter_id,action,due_at,state,trigger_source_ref,created_at)
         VALUES(?1,?2,?3,?4,'draft',?5,?6)",
        params![id,matter_id,action,due,source,Utc::now().to_rfc3339()]
    )?;Ok(())})?;Ok(json!({"id":id}))
}
}

#[tauri::command]
pub fn commit_deadline(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let id=required_string(&payload,"deadlineId")?;
    state.db.write(|conn|{let changed=conn.execute(
        "UPDATE legal_deadlines SET state='committed',committed_at=?2 WHERE id=?1 AND state='draft'",
        params![id,Utc::now().to_rfc3339()]
    )?;if changed!=1{return Err(AppError::Validation("deadline not committable".into()));}Ok(())})?;
    Ok(json!({"ok":true}))
}
}

#[tauri::command]
pub fn supersede_deadline(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:supersede_deadline".into()))
}

#[tauri::command]
pub fn get_ai_settings(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:get_ai_settings".into()))
}

#[tauri::command]
pub fn save_ai_settings(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:save_ai_settings".into()))
}

#[tauri::command]
pub fn test_ai_provider(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:test_ai_provider".into()))
}

#[tauri::command]
pub fn plan_ai_context(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let matter_id=required_string(&payload,"matterId")?; let capability=required_string(&payload,"capability")?;
    ai::plan_context(&state.db,matter_id,capability)
}
}

#[tauri::command]
pub fn run_ai_capability(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let matter_id=required_string(&payload,"matterId")?;let capability=required_string(&payload,"capability")?;
    let profile_id=required_string(&payload,"profileId")?;let approved=payload.get("externalEgressApproved").and_then(Value::as_bool).unwrap_or(false);
    Ok(json!({"runId":ai::run_capability(&state.db,matter_id,capability,profile_id,approved)?}))
}
}

#[tauri::command]
pub fn get_ai_run(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:get_ai_run".into()))
}

#[tauri::command]
pub fn review_ai_proposal(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:review_ai_proposal".into()))
}

#[tauri::command]
pub fn list_verified_facts(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:list_verified_facts".into()))
}

#[tauri::command]
pub fn verify_fact(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let matter_id=required_string(&payload,"matterId")?;let subject=required_string(&payload,"subject")?;
    let predicate=required_string(&payload,"predicate")?;let value=required_string(&payload,"value")?;
    let page_id=required_string(&payload,"documentPageId")?;let quote=required_string(&payload,"displayQuote")?;
    let id=Uuid::new_v4().to_string();let source_id=Uuid::new_v4().to_string();let now=Utc::now().to_rfc3339();
    state.db.write(|conn|{
        let tx=conn.transaction()?;
        let (version_id,text_sha):(String,String)=tx.query_row(
            "SELECT document_version_id,text_sha256 FROM document_pages WHERE id=?1 AND matter_id=?2",
            params![page_id,matter_id],|r|Ok((r.get(0)?,r.get(1)?))
        ).map_err(|_|AppError::InvalidSourceReference)?;
        tx.execute(
            "INSERT INTO verified_facts(id,matter_id,subject,predicate,value_text,status,verified_at)
             VALUES(?1,?2,?3,?4,?5,'valid',?6)",
            params![id,matter_id,subject,predicate,value,now]
        )?;
        tx.execute(
            "INSERT INTO verified_fact_sources(
                id,matter_id,verified_fact_id,document_version_id,document_page_id,
                display_quote,normalized_quote,source_text_sha256
             ) VALUES(?1,?2,?3,?4,?5,?6,?6,?7)",
            params![source_id,matter_id,id,version_id,page_id,quote,text_sha]
        )?;
        tx.commit()?;Ok(())
    })?;Ok(json!({"id":id}))
}
}

#[tauri::command]
pub fn invalidate_fact(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:invalidate_fact".into()))
}

#[tauri::command]
pub fn list_damage_calculations(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:list_damage_calculations".into()))
}

#[tauri::command]
pub fn save_damage_calculation(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:save_damage_calculation".into()))
}

#[tauri::command]
pub fn calculate_damage(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let regime=required_string(&payload,"regime")?;let life_state=required_string(&payload,"lifeState")?;
    let inputs:Vec<models::DamageInput>=serde_json::from_value(
        payload.get("inputs").cloned().ok_or_else(||AppError::Validation("inputs required".into()))?
    )?;
    Ok(serde_json::to_value(damage::calculate(regime,life_state,&inputs)?)?)
}
}

#[tauri::command]
pub fn lock_damage_calculation(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let id=required_string(&payload,"calculationId")?;
    let integrity=required_string(&payload,"integritySha256")?;
    state.db.write(|conn|{let changed=conn.execute(
        "UPDATE damage_calculations SET status='locked',integrity_sha256=?2,locked_at=?3,updated_at=?3
         WHERE id=?1 AND status='draft'",
        params![id,integrity,Utc::now().to_rfc3339()]
    )?;if changed!=1{return Err(AppError::Validation("calculation not lockable".into()));}Ok(())})?;
    Ok(json!({"ok":true}))
}
}

#[tauri::command]
pub fn list_authorities(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:list_authorities".into()))
}

#[tauri::command]
pub fn save_authority(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:save_authority".into()))
}

#[tauri::command]
pub fn verify_authority(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:verify_authority".into()))
}

#[tauri::command]
pub fn list_legal_documents(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:list_legal_documents".into()))
}

#[tauri::command]
pub fn save_legal_document_draft(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let matter_id=required_string(&payload,"matterId")?;let title=required_string(&payload,"title")?;
    let kind=required_string(&payload,"kind")?;
    Ok(json!({"id":legal_docs::create_draft(&state.db,matter_id,title,kind)?}))
}
}

#[tauri::command]
pub fn approve_legal_document(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let matter_id=required_string(&payload,"matterId")?;let version_id=required_string(&payload,"versionId")?;
    Ok(json!({"approvalSha256":legal_docs::approve_version(&state.db,matter_id,version_id)?}))
}
}

#[tauri::command]
pub fn export_legal_document(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    Err(AppError::Validation("RECONSTRUCTED_COMMAND_NOT_YET_WIRED:export_legal_document".into()))
}
