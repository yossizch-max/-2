use crate::{
    ai, authorities, damage, extraction, legal_docs, legal_rules, matter_profile, models, scanner, search, security,
    error::{AppError,AppResult}, AppState
};
use chrono::Utc;
use rusqlite::params;
use serde_json::{json,Value};
use sha2::{Digest,Sha256};
use std::path::PathBuf;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use uuid::Uuid;

fn required_string<'a>(payload:&'a Value,key:&str)->AppResult<&'a str>{
    payload.get(key).and_then(Value::as_str)
        .ok_or_else(||AppError::Validation(format!("{key} required")))
}

/// For fields the legal-rules DSL stores as JSON-in-a-TEXT-column (conditions,
/// operations, test-case input/expected output): accepts the value as real nested
/// JSON from the frontend and re-serializes it to the compact string form the DB and
/// `legal_rules.rs`'s parsers expect.
fn required_json_string(payload:&Value,key:&str)->AppResult<String>{
    let value=payload.get(key).ok_or_else(||AppError::Validation(format!("{key} required")))?;
    serde_json::to_string(value).map_err(AppError::Serde)
}

#[tauri::command]
pub fn get_app_health(state: State<'_, AppState>, _payload: Value) -> AppResult<Value> {
    let database = if state.db.read(|conn| Ok(conn.query_row("SELECT 1", [], |r| r.get::<_, i64>(0))?)).is_ok() {
        "ok"
    } else {
        "unreachable"
    };

    let source_index = if state.office_root.lock().map(|g| g.is_some()).unwrap_or(false) {
        "bound"
    } else {
        "not_configured"
    };

    let pdftotext = state.resource_root.join("ocr").join("vendor").join("poppler").join("pdftotext.exe");
    let tesseract = state.resource_root.join("ocr").join("vendor").join("tesseract").join("tesseract.exe");
    let ocr_runtime = if pdftotext.exists() && tesseract.exists() { "ok" } else { "missing" };

    let ai_provider = state.db.read(|conn| Ok(conn.query_row(
        "SELECT COUNT(*) FROM ai_provider_profiles WHERE enabled=1", [], |r| r.get::<_, i64>(0)
    )?)).map(|n: i64| n > 0).unwrap_or(false);

    Ok(json!({
        "database": database,
        "sourceIndex": source_index,
        "ocrRuntime": ocr_runtime,
        "aiProvider": if ai_provider { "enabled" } else { "disabled" },
    }))
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&payload;
    state.db.read(|conn|{
        let raw:Option<String>=conn.query_row(
            "SELECT settings_json FROM app_settings WHERE id=1",[],|r|r.get(0)
        ).ok();
        Ok(match raw{
            Some(text)=>serde_json::from_str(&text)?,
            None=>json!({})
        })
    })
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    if !payload.is_object(){return Err(AppError::Validation("settings payload must be an object".into()));}
    let text=serde_json::to_string(&payload)?;
    state.db.write(|conn|{conn.execute(
        "INSERT INTO app_settings(id,settings_json,updated_at) VALUES(1,?1,?2)
         ON CONFLICT(id) DO UPDATE SET settings_json=excluded.settings_json,updated_at=excluded.updated_at",
        params![text,Utc::now().to_rfc3339()]
    )?;Ok(())})?;
    Ok(json!({"ok":true}))
}

#[tauri::command]
pub fn choose_folder(app: AppHandle, state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=(&state,&payload);
    let picked=app.dialog().file().blocking_pick_folder();
    Ok(json!({"path":picked.map(|p|p.to_string())}))
}

#[tauri::command]
pub fn choose_save_file(app: AppHandle, state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    let default_name=payload.get("defaultName").and_then(Value::as_str).unwrap_or("export.txt");
    let picked=app.dialog().file().set_file_name(default_name).blocking_save_file();
    Ok(json!({"path":picked.map(|p|p.to_string())}))
}

#[tauri::command]
pub fn get_office_root(state: State<'_, AppState>, _payload: Value) -> AppResult<Value> {
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
pub fn scan_office_root(state: State<'_, AppState>, _payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let root=state.office_root.lock().map_err(|_|AppError::Validation("office root mutex".into()))?
        .clone().ok_or_else(||AppError::Validation("office root not set".into()))?;
    Ok(json!({"runId":scanner::scan_metadata(&state.db,&root)?}))
}
}

#[tauri::command]
pub fn list_scan_runs(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&payload;
    state.db.read(|conn|{
        let mut stmt=conn.prepare(
            "SELECT id,root_path,status,started_at,finished_at,discovered_count,hashed_count,error_count
             FROM scan_runs ORDER BY started_at DESC LIMIT 50"
        )?;
        let rows=stmt.query_map([],|r|Ok(json!({
            "id":r.get::<_,String>(0)?,"rootPath":r.get::<_,String>(1)?,"status":r.get::<_,String>(2)?,
            "startedAt":r.get::<_,String>(3)?,"finishedAt":r.get::<_,Option<String>>(4)?,
            "discoveredCount":r.get::<_,i64>(5)?,"hashedCount":r.get::<_,i64>(6)?,"errorCount":r.get::<_,i64>(7)?
        })))?.collect::<Result<Vec<_>,_>>()?;
        Ok(Value::Array(rows))
    })
}

#[tauri::command]
pub fn list_matter_suggestions(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&payload;
    state.db.read(|conn|{
        let mut stmt=conn.prepare(
            "SELECT id,path_display,suggested_title,file_count,created_at
             FROM matter_suggestions WHERE status='pending' ORDER BY created_at DESC"
        )?;
        let rows=stmt.query_map([],|r|Ok(json!({
            "id":r.get::<_,String>(0)?,"pathDisplay":r.get::<_,String>(1)?,
            "suggestedTitle":r.get::<_,String>(2)?,"fileCount":r.get::<_,i64>(3)?,
            "createdAt":r.get::<_,String>(4)?
        })))?.collect::<Result<Vec<_>,_>>()?;
        Ok(Value::Array(rows))
    })
}

#[tauri::command]
pub fn bind_existing_matter(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let suggestion_id=required_string(&payload,"suggestionId")?;
    let matter_id=required_string(&payload,"matterId")?;
    let now=Utc::now().to_rfc3339();
    state.db.write(|conn|{
        let tx=conn.transaction()?;
        let path_display:String=tx.query_row(
            "SELECT path_display FROM matter_suggestions WHERE id=?1 AND status='pending'",
            [suggestion_id],|r|r.get(0)
        ).map_err(|_|AppError::Validation("matter suggestion not pending".into()))?;
        let path_key=path_display.replace('/',"\\").trim_end_matches('\\').to_lowercase();
        tx.execute(
            "INSERT INTO matter_folder_bindings(id,matter_id,path_display,path_key,binding_source,active,last_seen_at)
             VALUES(?1,?2,?3,?4,'suggestion',1,?5)
             ON CONFLICT(matter_id,path_key) DO UPDATE SET active=1,last_seen_at=excluded.last_seen_at",
            params![Uuid::new_v4().to_string(),matter_id,path_display,path_key,now]
        )?;
        let changed=tx.execute(
            "UPDATE matter_suggestions SET status='bound',bound_matter_id=?2,resolved_at=?3 WHERE id=?1 AND status='pending'",
            params![suggestion_id,matter_id,now]
        )?;
        if changed!=1{return Err(AppError::Validation("matter suggestion not pending".into()));}
        tx.commit()?;Ok(())
    })?;
    Ok(json!({"ok":true}))
}

#[tauri::command]
pub fn reject_matter_suggestion(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let suggestion_id=required_string(&payload,"suggestionId")?;
    state.db.write(|conn|{let changed=conn.execute(
        "UPDATE matter_suggestions SET status='rejected',resolved_at=?2 WHERE id=?1 AND status='pending'",
        params![suggestion_id,Utc::now().to_rfc3339()]
    )?;if changed!=1{return Err(AppError::Validation("matter suggestion not pending".into()));}Ok(())})?;
    Ok(json!({"ok":true}))
}

#[tauri::command]
pub fn create_matter(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let title=required_string(&payload,"title")?;
    let matter_type=payload.get("matterType").and_then(Value::as_str).unwrap_or("generic_civil");
    matter_profile::validate_case_type(matter_type)?;
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
pub fn list_matters(state: State<'_, AppState>, _payload: Value) -> AppResult<Value> {
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
    let matter_id=required_string(&payload,"matterId")?;
    let title=payload.get("title").and_then(Value::as_str);
    let internal=payload.get("internalNumber").and_then(Value::as_str);
    let external=payload.get("externalNumber").and_then(Value::as_str);
    let matter_type=payload.get("matterType").and_then(Value::as_str);
    if let Some(mt)=matter_type { matter_profile::validate_case_type(mt)?; }
    let status=payload.get("status").and_then(Value::as_str);
    state.db.write(|conn|{let changed=conn.execute(
        "UPDATE matters SET
            title=coalesce(?2,title),
            internal_number=coalesce(?3,internal_number),
            external_number=coalesce(?4,external_number),
            matter_type=coalesce(?5,matter_type),
            status=coalesce(?6,status),
            updated_at=?7
         WHERE id=?1",
        params![matter_id,title,internal,external,matter_type,status,Utc::now().to_rfc3339()]
    )?;if changed!=1{return Err(AppError::NotFound("matter".into()));}Ok(())})?;
    Ok(json!({"ok":true}))
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
pub fn get_matter_profile(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;
    let profile=matter_profile::get_profile(&state.db,matter_id)?;
    Ok(serde_json::to_value(profile)?)
}

#[tauri::command]
pub fn save_matter_profile(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;
    let event_date=payload.get("eventDate").and_then(Value::as_str);
    let court_name=payload.get("courtName").and_then(Value::as_str);
    let btl_claim_number=payload.get("btlClaimNumber").and_then(Value::as_str);
    let case_summary=payload.get("caseSummary").and_then(Value::as_str);
    matter_profile::save_profile(&state.db,matter_id,event_date,court_name,btl_claim_number,case_summary)?;
    Ok(json!({"ok":true}))
}

#[tauri::command]
pub fn list_matter_parties(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;
    let parties=matter_profile::list_parties(&state.db,matter_id)?;
    Ok(serde_json::to_value(parties)?)
}

#[tauri::command]
pub fn add_matter_party(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;
    let role=required_string(&payload,"role")?;
    let name=required_string(&payload,"name")?;
    let contact_details=payload.get("contactDetails").and_then(Value::as_str);
    let notes=payload.get("notes").and_then(Value::as_str);
    let id=matter_profile::add_party(&state.db,matter_id,role,name,contact_details,notes)?;
    Ok(json!({"id":id}))
}

#[tauri::command]
pub fn update_matter_party(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let party_id=required_string(&payload,"partyId")?;
    let matter_id=required_string(&payload,"matterId")?;
    let role=payload.get("role").and_then(Value::as_str);
    let name=payload.get("name").and_then(Value::as_str);
    let contact_details=payload.get("contactDetails").and_then(Value::as_str);
    let notes=payload.get("notes").and_then(Value::as_str);
    matter_profile::update_party(&state.db,party_id,matter_id,role,name,contact_details,notes)?;
    Ok(json!({"ok":true}))
}

#[tauri::command]
pub fn delete_matter_party(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let party_id=required_string(&payload,"partyId")?;
    let matter_id=required_string(&payload,"matterId")?;
    matter_profile::delete_party(&state.db,party_id,matter_id)?;
    Ok(json!({"ok":true}))
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
             v.id,v.content_sha256,coalesce(o.observed_mtime,''),o.id
             FROM documents d LEFT JOIN document_versions v ON v.document_id=d.id AND v.matter_id=d.matter_id
             LEFT JOIN file_occurrences o ON o.document_version_id=v.id
             WHERE d.matter_id=?1 GROUP BY d.id ORDER BY d.updated_at DESC"
        )?;
        let rows=stmt.query_map([matter_id],|r|Ok(json!({
            "id":r.get::<_,String>(0)?,"matterId":r.get::<_,String>(1)?,"fileName":r.get::<_,String>(2)?,
            "category":r.get::<_,String>(3)?,"sourceState":r.get::<_,String>(4)?,
            "extractionState":r.get::<_,String>(5)?,"currentVersionId":r.get::<_,Option<String>>(6)?,
            "currentSha256":r.get::<_,Option<String>>(7)?,"modifiedAt":r.get::<_,String>(8)?,
            "occurrenceId":r.get::<_,Option<String>>(9)?
        })))?.collect::<Result<Vec<_>,_>>()?;
        Ok(Value::Array(rows))
    })
}
}

#[tauri::command]
pub fn get_document(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let document_id=required_string(&payload,"documentId")?;
    state.db.read(|conn|conn.query_row(
        "SELECT d.id,d.matter_id,coalesce(o.file_name,d.logical_title,''),d.category,
         coalesce(o.availability_state,'unknown'),coalesce(v.extraction_state,'not_started'),
         v.id,v.content_sha256,coalesce(o.observed_mtime,'')
         FROM documents d LEFT JOIN document_versions v ON v.document_id=d.id AND v.matter_id=d.matter_id
         LEFT JOIN file_occurrences o ON o.document_version_id=v.id
         WHERE d.id=?1 ORDER BY v.created_at DESC LIMIT 1",
        [document_id],|r|Ok(json!({
            "id":r.get::<_,String>(0)?,"matterId":r.get::<_,String>(1)?,"fileName":r.get::<_,String>(2)?,
            "category":r.get::<_,String>(3)?,"sourceState":r.get::<_,String>(4)?,
            "extractionState":r.get::<_,String>(5)?,"currentVersionId":r.get::<_,Option<String>>(6)?,
            "currentSha256":r.get::<_,Option<String>>(7)?,"modifiedAt":r.get::<_,String>(8)?
        }))
    ).map_err(AppError::Db))
}

#[tauri::command]
pub fn list_document_versions(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let document_id=required_string(&payload,"documentId")?;
    state.db.read(|conn|{
        let mut stmt=conn.prepare(
            "SELECT id,content_sha256,byte_size,observed_mtime,extraction_state,stale,created_at
             FROM document_versions WHERE document_id=?1 ORDER BY created_at DESC"
        )?;
        let rows=stmt.query_map([document_id],|r|Ok(json!({
            "id":r.get::<_,String>(0)?,"contentSha256":r.get::<_,Option<String>>(1)?,
            "byteSize":r.get::<_,Option<i64>>(2)?,"observedMtime":r.get::<_,Option<String>>(3)?,
            "extractionState":r.get::<_,String>(4)?,"stale":r.get::<_,i64>(5)?!=0,
            "createdAt":r.get::<_,String>(6)?
        })))?.collect::<Result<Vec<_>,_>>()?;
        Ok(Value::Array(rows))
    })
}

#[tauri::command]
pub fn open_occurrence(app: AppHandle, state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let occurrence_id=required_string(&payload,"occurrenceId")?;
    let path:String=state.db.read(|conn|conn.query_row(
        "SELECT path_display FROM file_occurrences WHERE id=?1 AND exists_now=1",
        [occurrence_id],|r|r.get(0)
    ).map_err(|_|AppError::NotFound("file occurrence".into())))?;
    app.opener().open_path(&path,None::<&str>).map_err(|e|AppError::Validation(e.to_string()))?;
    Ok(json!({"ok":true}))
}

#[tauri::command]
pub fn reveal_occurrence(app: AppHandle, state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let occurrence_id=required_string(&payload,"occurrenceId")?;
    let path:String=state.db.read(|conn|conn.query_row(
        "SELECT path_display FROM file_occurrences WHERE id=?1 AND exists_now=1",
        [occurrence_id],|r|r.get(0)
    ).map_err(|_|AppError::NotFound("file occurrence".into())))?;
    app.opener().reveal_item_in_dir(&path).map_err(|e|AppError::Validation(e.to_string()))?;
    Ok(json!({"ok":true}))
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
    let matter_id=required_string(&payload,"matterId")?;
    let document_id=required_string(&payload,"documentId")?;
    let category=required_string(&payload,"category")?;
    state.db.write(|conn|{let changed=conn.execute(
        "UPDATE documents SET category=?3,category_source='manual',category_confidence=1.0,updated_at=?4
         WHERE id=?1 AND matter_id=?2",
        params![document_id,matter_id,category,Utc::now().to_rfc3339()]
    )?;if changed!=1{return Err(AppError::NotFound("document".into()));}Ok(())})?;
    Ok(json!({"ok":true}))
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
    let task_id=required_string(&payload,"taskId")?;
    let title=payload.get("title").and_then(Value::as_str);
    let due_at=payload.get("dueAt").and_then(Value::as_str);
    let risk=payload.get("riskClass").and_then(Value::as_str);
    let status=payload.get("status").and_then(Value::as_str);
    state.db.write(|conn|{let changed=conn.execute(
        "UPDATE tasks SET
            title=coalesce(?2,title), due_at=coalesce(?3,due_at),
            risk_class=coalesce(?4,risk_class), status=coalesce(?5,status), updated_at=?6
         WHERE id=?1",
        params![task_id,title,due_at,risk,status,Utc::now().to_rfc3339()]
    )?;if changed!=1{return Err(AppError::NotFound("task".into()));}Ok(())})?;
    Ok(json!({"ok":true}))
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
    let matter_id=required_string(&payload,"matterId")?;
    state.db.read(|conn|{
        let mut stmt=conn.prepare(
            "SELECT id,matter_id,title,starts_at,ends_at,event_kind,status
             FROM calendar_events WHERE matter_id=?1 ORDER BY starts_at"
        )?;
        let rows=stmt.query_map([matter_id],|r|Ok(json!({
            "id":r.get::<_,String>(0)?,"matterId":r.get::<_,String>(1)?,"title":r.get::<_,String>(2)?,
            "startsAt":r.get::<_,String>(3)?,"endsAt":r.get::<_,Option<String>>(4)?,
            "eventKind":r.get::<_,String>(5)?,"status":r.get::<_,String>(6)?
        })))?.collect::<Result<Vec<_>,_>>()?;
        Ok(Value::Array(rows))
    })
}

#[tauri::command]
pub fn create_calendar_item(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;
    let title=required_string(&payload,"title")?;
    let starts_at=required_string(&payload,"startsAt")?;
    let ends_at=payload.get("endsAt").and_then(Value::as_str);
    let event_kind=payload.get("eventKind").and_then(Value::as_str).unwrap_or("general");
    let source_ref=payload.get("sourceRef").and_then(Value::as_str);
    let id=Uuid::new_v4().to_string();
    state.db.write(|conn|{conn.execute(
        "INSERT INTO calendar_events(id,matter_id,title,starts_at,ends_at,event_kind,source_ref,status,created_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,'active',?8)",
        params![id,matter_id,title,starts_at,ends_at,event_kind,source_ref,Utc::now().to_rfc3339()]
    )?;Ok(())})?;
    Ok(json!({"id":id}))
}

#[tauri::command]
pub fn update_calendar_item(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let event_id=required_string(&payload,"eventId")?;
    let title=payload.get("title").and_then(Value::as_str);
    let starts_at=payload.get("startsAt").and_then(Value::as_str);
    let ends_at=payload.get("endsAt").and_then(Value::as_str);
    let status=payload.get("status").and_then(Value::as_str);
    state.db.write(|conn|{let changed=conn.execute(
        "UPDATE calendar_events SET
            title=coalesce(?2,title), starts_at=coalesce(?3,starts_at),
            ends_at=coalesce(?4,ends_at), status=coalesce(?5,status)
         WHERE id=?1",
        params![event_id,title,starts_at,ends_at,status]
    )?;if changed!=1{return Err(AppError::NotFound("calendar event".into()));}Ok(())})?;
    Ok(json!({"ok":true}))
}

#[tauri::command]
pub fn list_waiting_for(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;
    state.db.read(|conn|{
        let mut stmt=conn.prepare(
            "SELECT id,matter_id,party_label,item_label,since_at,follow_up_at,last_contact_at,status
             FROM waiting_for WHERE matter_id=?1 ORDER BY follow_up_at"
        )?;
        let rows=stmt.query_map([matter_id],|r|Ok(json!({
            "id":r.get::<_,String>(0)?,"matterId":r.get::<_,String>(1)?,"partyLabel":r.get::<_,String>(2)?,
            "itemLabel":r.get::<_,String>(3)?,"sinceAt":r.get::<_,String>(4)?,
            "followUpAt":r.get::<_,Option<String>>(5)?,"lastContactAt":r.get::<_,Option<String>>(6)?,
            "status":r.get::<_,String>(7)?
        })))?.collect::<Result<Vec<_>,_>>()?;
        Ok(Value::Array(rows))
    })
}

#[tauri::command]
pub fn save_waiting_for(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;
    let party_label=required_string(&payload,"partyLabel")?;
    let item_label=required_string(&payload,"itemLabel")?;
    let follow_up_at=payload.get("followUpAt").and_then(Value::as_str);
    let source_ref=payload.get("sourceRef").and_then(Value::as_str);
    let id=Uuid::new_v4().to_string();
    state.db.write(|conn|{conn.execute(
        "INSERT INTO waiting_for(id,matter_id,party_label,item_label,since_at,follow_up_at,status,source_ref)
         VALUES(?1,?2,?3,?4,?5,?6,'open',?7)",
        params![id,matter_id,party_label,item_label,Utc::now().to_rfc3339(),follow_up_at,source_ref]
    )?;Ok(())})?;
    Ok(json!({"id":id}))
}

#[tauri::command]
pub fn close_waiting_for(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let id=required_string(&payload,"waitingForId")?;
    state.db.write(|conn|{let changed=conn.execute(
        "UPDATE waiting_for SET status='closed',last_contact_at=?2 WHERE id=?1",
        params![id,Utc::now().to_rfc3339()]
    )?;if changed!=1{return Err(AppError::NotFound("waiting_for item".into()));}Ok(())})?;
    Ok(json!({"ok":true}))
}

#[tauri::command]
pub fn list_deadlines(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;
    state.db.read(|conn|{
        let mut stmt=conn.prepare(
            "SELECT id,matter_id,action,due_at,state,trigger_source_ref,rule_id
             FROM legal_deadlines WHERE matter_id=?1 ORDER BY due_at"
        )?;
        let rows=stmt.query_map([matter_id],|r|Ok(json!({
            "id":r.get::<_,String>(0)?,"matterId":r.get::<_,String>(1)?,"action":r.get::<_,String>(2)?,
            "dueAt":r.get::<_,String>(3)?,"state":r.get::<_,String>(4)?,
            "sourceLabel":r.get::<_,String>(5)?,"ruleLabel":r.get::<_,Option<String>>(6)?
        })))?.collect::<Result<Vec<_>,_>>()?;
        Ok(Value::Array(rows))
    })
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
    let old_id=required_string(&payload,"deadlineId")?;
    let action=required_string(&payload,"action")?;
    let due_at=required_string(&payload,"dueAt")?;
    let source=required_string(&payload,"triggerSourceRef")?;
    let new_id=Uuid::new_v4().to_string();
    state.db.write(|conn|{
        let tx=conn.transaction()?;
        let matter_id:String=tx.query_row(
            "SELECT matter_id FROM legal_deadlines WHERE id=?1 AND state IN ('draft','committed')",
            [old_id],|r|r.get(0)
        ).map_err(|_|AppError::Validation("deadline not supersedable".into()))?;
        tx.execute(
            "INSERT INTO legal_deadlines(id,matter_id,action,due_at,state,trigger_source_ref,created_at)
             VALUES(?1,?2,?3,?4,'draft',?5,?6)",
            params![new_id,matter_id,action,due_at,source,Utc::now().to_rfc3339()]
        )?;
        let changed=tx.execute(
            "UPDATE legal_deadlines SET state='superseded',superseded_by=?2
             WHERE id=?1 AND state IN ('draft','committed')",
            params![old_id,new_id]
        )?;
        if changed!=1{return Err(AppError::Validation("deadline not supersedable".into()));}
        tx.commit()?;Ok(())
    })?;
    Ok(json!({"id":new_id}))
}

#[tauri::command]
pub fn get_ai_settings(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&payload;
    state.db.read(|conn|{
        let mut stmt=conn.prepare(
            "SELECT id,provider_kind,base_url,model,enabled,client_data_authorized
             FROM ai_provider_profiles ORDER BY created_at"
        )?;
        let rows=stmt.query_map([],|r|Ok(json!({
            "id":r.get::<_,String>(0)?,"providerKind":r.get::<_,String>(1)?,"baseUrl":r.get::<_,String>(2)?,
            "model":r.get::<_,Option<String>>(3)?,"enabled":r.get::<_,i64>(4)?!=0,
            "clientDataAuthorized":r.get::<_,i64>(5)?!=0
        })))?.collect::<Result<Vec<_>,_>>()?;
        Ok(Value::Array(rows))
    })
}

#[tauri::command]
pub fn save_ai_settings(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let provider_kind=required_string(&payload,"providerKind")?;
    if !matches!(provider_kind,"local"|"openai"){
        return Err(AppError::Validation("unsupported provider kind".into()));
    }
    let base_url=required_string(&payload,"baseUrl")?;
    let model=payload.get("model").and_then(Value::as_str);
    let enabled=payload.get("enabled").and_then(Value::as_bool).unwrap_or(false);
    let client_data_authorized=payload.get("clientDataAuthorized").and_then(Value::as_bool).unwrap_or(false);
    let id=payload.get("id").and_then(Value::as_str).map(str::to_string)
        .unwrap_or_else(||Uuid::new_v4().to_string());
    let now=Utc::now().to_rfc3339();
    state.db.write(|conn|{conn.execute(
        "INSERT INTO ai_provider_profiles(
            id,provider_kind,base_url,model,enabled,client_data_authorized,created_at,updated_at
         ) VALUES(?1,?2,?3,?4,?5,?6,?7,?7)
         ON CONFLICT(id) DO UPDATE SET
            provider_kind=excluded.provider_kind, base_url=excluded.base_url, model=excluded.model,
            enabled=excluded.enabled, client_data_authorized=excluded.client_data_authorized,
            updated_at=excluded.updated_at",
        params![id,provider_kind,base_url,model,enabled as i64,client_data_authorized as i64,now]
    )?;Ok(())})?;
    if let Some(secret)=payload.get("apiKey").and_then(Value::as_str){
        security::set_ai_secret(&id,secret)?;
    }
    Ok(json!({"id":id}))
}

#[tauri::command]
pub fn test_ai_provider(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let profile_id=required_string(&payload,"profileId")?;
    let (provider_kind,base_url,client_data_authorized):(String,String,bool)=state.db.read(|conn|conn.query_row(
        "SELECT provider_kind,base_url,client_data_authorized FROM ai_provider_profiles WHERE id=?1",
        [profile_id],|r|Ok((r.get(0)?,r.get(1)?,r.get::<_,i64>(2)?!=0))
    ).map_err(AppError::Db))?;
    if provider_kind=="local"{
        ai::validate_loopback(&base_url)?;
    }else if provider_kind=="openai"{
        if base_url!="https://api.openai.com/v1"{
            return Err(AppError::Validation("OpenAI endpoint is fixed".into()));
        }
    }else{
        return Err(AppError::Validation("unsupported provider kind".into()));
    }
    Ok(json!({"ok":true,"providerKind":provider_kind,"readyForClientData":client_data_authorized}))
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
    let run_id=required_string(&payload,"runId")?;
    state.db.read(|conn|{
        let run=conn.query_row(
            "SELECT id,matter_id,capability,status,model,started_at,finished_at
             FROM ai_runs WHERE id=?1",
            [run_id],|r|Ok(json!({
                "id":r.get::<_,String>(0)?,"matterId":r.get::<_,Option<String>>(1)?,
                "capability":r.get::<_,String>(2)?,"status":r.get::<_,String>(3)?,
                "model":r.get::<_,Option<String>>(4)?,"startedAt":r.get::<_,String>(5)?,
                "finishedAt":r.get::<_,Option<String>>(6)?
            }))
        ).map_err(AppError::Db)?;
        let mut stmt=conn.prepare(
            "SELECT id,proposal_kind,structured_json,status,reviewed_at,review_note
             FROM ai_proposals WHERE ai_run_id=?1"
        )?;
        let mut proposals=stmt.query_map([run_id],|r|{
            let structured:String=r.get(2)?;
            Ok(json!({
                "id":r.get::<_,String>(0)?,"proposalKind":r.get::<_,String>(1)?,
                "structured":serde_json::from_str::<Value>(&structured).unwrap_or(Value::Null),
                "status":r.get::<_,String>(3)?,"reviewedAt":r.get::<_,Option<String>>(4)?,
                "reviewNote":r.get::<_,Option<String>>(5)?
            }))
        })?.collect::<Result<Vec<_>,_>>()?;

        let mut excerpt_stmt=conn.prepare(
            "SELECT p.page_number,p.display_text,
             (SELECT o.file_name FROM file_occurrences o
              WHERE o.document_version_id=p.document_version_id AND o.exists_now=1 LIMIT 1)
             FROM document_pages p WHERE p.id=?1"
        )?;
        for proposal in proposals.iter_mut(){
            let source_ids:Vec<String>=proposal["structured"]["sourceIds"].as_array()
                .map(|a|a.iter().filter_map(|v|v.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            let mut excerpts=Vec::with_capacity(source_ids.len());
            for source_id in &source_ids{
                if let Ok((page,text,file_name))=excerpt_stmt.query_row([source_id],|r|{
                    Ok((r.get::<_,Option<i64>>(0)?,r.get::<_,String>(1)?,r.get::<_,Option<String>>(2)?))
                }){
                    let truncated:String=text.chars().take(400).collect();
                    excerpts.push(json!({
                        "sourceId":source_id,"page":page,
                        "fileName":file_name,
                        "excerpt":truncated,
                        "truncated":text.chars().count()>400
                    }));
                }
            }
            proposal["sourceExcerpts"]=Value::Array(excerpts);
        }

        let mut run=run; run["proposals"]=Value::Array(proposals);
        Ok(run)
    })
}

#[tauri::command]
pub fn review_ai_proposal(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let proposal_id=required_string(&payload,"proposalId")?;
    let decision=required_string(&payload,"decision")?;
    if !matches!(decision,"approved"|"rejected"|"needs_revision"){
        return Err(AppError::Validation("invalid review decision".into()));
    }
    let note=payload.get("reviewNote").and_then(Value::as_str);
    if decision=="approved"{
        let verified_fact_id=ai::approve_proposal(&state.db,proposal_id,note)?;
        return Ok(json!({"ok":true,"verifiedFactId":verified_fact_id}));
    }
    state.db.write(|conn|{let changed=conn.execute(
        "UPDATE ai_proposals SET status=?2,reviewed_at=?3,review_note=?4 WHERE id=?1 AND status='pending'",
        params![proposal_id,decision,Utc::now().to_rfc3339(),note]
    )?;if changed!=1{return Err(AppError::Validation("proposal not pending".into()));}Ok(())})?;
    Ok(json!({"ok":true}))
}

#[tauri::command]
pub fn list_verified_facts(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;
    state.db.read(|conn|{
        let mut stmt=conn.prepare(
            "SELECT f.id,f.matter_id,f.subject,f.predicate,f.value_text,f.status,f.stale,f.verified_at,
             coalesce((SELECT display_quote FROM verified_fact_sources s WHERE s.verified_fact_id=f.id LIMIT 1),''),
             (SELECT o.id FROM verified_fact_sources s
              JOIN file_occurrences o ON o.document_version_id=s.document_version_id
              WHERE s.verified_fact_id=f.id AND o.exists_now=1 LIMIT 1)
             FROM verified_facts f WHERE f.matter_id=?1 AND f.status='valid' ORDER BY f.verified_at DESC"
        )?;
        let rows=stmt.query_map([matter_id],|r|Ok(json!({
            "id":r.get::<_,String>(0)?,"matterId":r.get::<_,String>(1)?,"subject":r.get::<_,String>(2)?,
            "predicate":r.get::<_,String>(3)?,"value":r.get::<_,String>(4)?,"status":r.get::<_,String>(5)?,
            "stale":r.get::<_,i64>(6)?!=0,"verifiedAt":r.get::<_,String>(7)?,"sourceLabel":r.get::<_,String>(8)?,
            "occurrenceId":r.get::<_,Option<String>>(9)?
        })))?.collect::<Result<Vec<_>,_>>()?;
        Ok(Value::Array(rows))
    })
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
    let fact_id=required_string(&payload,"factId")?;
    state.db.write(|conn|{let changed=conn.execute(
        "UPDATE verified_facts SET status='invalidated' WHERE id=?1 AND status='valid'",
        [fact_id]
    )?;if changed!=1{return Err(AppError::Validation("fact not invalidatable".into()));}Ok(())})?;
    Ok(json!({"ok":true}))
}

#[tauri::command]
pub fn list_damage_calculations(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;
    state.db.read(|conn|{
        let mut stmt=conn.prepare(
            "SELECT id,matter_id,regime,life_state,status,gross_cents,deductions_cents,net_cents,integrity_sha256
             FROM damage_calculations WHERE matter_id=?1 ORDER BY updated_at DESC"
        )?;
        struct Calc{id:String,matter_id:String,regime:String,life_state:String,status:String,
            gross:i64,deductions:i64,net:i64,integrity:Option<String>}
        let calcs=stmt.query_map([matter_id],|r|Ok(Calc{
            id:r.get(0)?,matter_id:r.get(1)?,regime:r.get(2)?,life_state:r.get(3)?,status:r.get(4)?,
            gross:r.get(5)?,deductions:r.get(6)?,net:r.get(7)?,integrity:r.get(8)?
        }))?.collect::<Result<Vec<_>,_>>()?;
        let mut input_stmt=conn.prepare(
            "SELECT input_key,value_text,source_kind FROM damage_inputs WHERE calculation_id=?1"
        )?;
        let mut out=Vec::with_capacity(calcs.len());
        for c in calcs{
            let inputs=input_stmt.query_map([&c.id],|r|{
                let cents=r.get::<_,String>(1)?.parse::<i64>().map_err(|_|
                    rusqlite::Error::InvalidColumnType(1,"value_text".to_string(),rusqlite::types::Type::Text)
                )?;
                Ok(json!({"key":r.get::<_,String>(0)?,"cents":cents,"source":r.get::<_,String>(2)?}))
            })?.collect::<Result<Vec<_>,_>>()?;
            out.push(json!({
                "id":c.id,"matterId":c.matter_id,"regime":c.regime,"lifeState":c.life_state,"status":c.status,
                "grossCents":c.gross,"deductionsCents":c.deductions,"netCents":c.net,
                "integritySha256":c.integrity,"inputs":inputs
            }));
        }
        Ok(Value::Array(out))
    })
}

#[tauri::command]
pub fn save_damage_calculation(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;
    let regime=required_string(&payload,"regime")?;
    let life_state=required_string(&payload,"lifeState")?;
    let ruleset_id=payload.get("rulesetId").and_then(Value::as_str).unwrap_or("default");
    let ruleset_version=payload.get("rulesetVersion").and_then(Value::as_str).unwrap_or("1");
    let inputs:Vec<models::DamageInput>=serde_json::from_value(
        payload.get("inputs").cloned().unwrap_or_else(||json!([]))
    )?;
    let result=damage::calculate(regime,life_state,&inputs)?;

    let id=Uuid::new_v4().to_string();
    let now=Utc::now().to_rfc3339();
    state.db.write(|conn|{
        let tx=conn.transaction()?;
        tx.execute(
            "INSERT INTO damage_calculations(
                id,matter_id,regime,life_state,status,gross_cents,deductions_cents,net_cents,
                ruleset_id,ruleset_version,created_at,updated_at
             ) VALUES(?1,?2,?3,?4,'draft',?5,?6,?7,?8,?9,?10,?10)",
            params![id,matter_id,regime,life_state,result.gross_cents,result.deductions_cents,
                result.net_cents,ruleset_id,ruleset_version,now]
        )?;
        for input in &inputs{
            tx.execute(
                "INSERT INTO damage_inputs(id,matter_id,calculation_id,input_key,value_kind,value_text,source_kind)
                 VALUES(?1,?2,?3,?4,'cents',?5,?6)",
                params![Uuid::new_v4().to_string(),matter_id,id,input.key,input.cents.to_string(),input.source]
            )?;
        }
        tx.commit()?;Ok(())
    })?;
    Ok(json!({"id":id,"grossCents":result.gross_cents,"deductionsCents":result.deductions_cents,"netCents":result.net_cents}))
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
    let id=required_string(&payload,"calculationId")?;
    state.db.write(|conn|{
        let tx=conn.transaction()?;
        let (regime,life_state,gross,deductions,net):(String,String,i64,i64,i64)=tx.query_row(
            "SELECT regime,life_state,gross_cents,deductions_cents,net_cents
             FROM damage_calculations WHERE id=?1 AND status='draft'",
            [id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))
        ).map_err(|_|AppError::Validation("calculation not lockable".into()))?;

        let inputs:Vec<models::DamageInput>={
            let mut stmt=tx.prepare(
                "SELECT input_key,value_text,source_kind FROM damage_inputs WHERE calculation_id=?1"
            )?;
            let rows=stmt.query_map([id],|r|{
                let key:String=r.get(0)?;
                let value_text:String=r.get(1)?;
                let source:String=r.get(2)?;
                let cents=value_text.parse::<i64>().map_err(|_|
                    rusqlite::Error::InvalidColumnType(1,"value_text".to_string(),rusqlite::types::Type::Text)
                )?;
                Ok(models::DamageInput{key,cents,source})
            })?.collect::<Result<Vec<_>,_>>()?;
            rows
        };
        let recomputed=damage::verify_for_lock(&regime,&life_state,&inputs,gross,deductions,net)?;

        let changed=tx.execute(
            "UPDATE damage_calculations SET status='locked',integrity_sha256=?2,locked_at=?3,updated_at=?3
             WHERE id=?1 AND status='draft'",
            params![id,recomputed.integrity_sha256,Utc::now().to_rfc3339()]
        )?;
        if changed!=1{return Err(AppError::Validation("calculation not lockable".into()));}
        tx.commit()?;
        Ok(())
    })?;
    Ok(json!({"ok":true}))
}

#[tauri::command]
pub fn list_authorities(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;
    state.db.read(|conn|{
        let mut stmt=conn.prepare(
            "SELECT id,matter_id,citation,title,court,decision_date,status,source_document_version_id,
             (SELECT count(*) FROM legal_authority_passages p WHERE p.authority_id=a.id AND p.approved=1)
             FROM legal_authorities a WHERE matter_id=?1 ORDER BY citation"
        )?;
        let rows=stmt.query_map([matter_id],|r|Ok(json!({
            "id":r.get::<_,String>(0)?,"matterId":r.get::<_,String>(1)?,"citation":r.get::<_,String>(2)?,
            "title":r.get::<_,String>(3)?,"court":r.get::<_,Option<String>>(4)?,
            "decisionDate":r.get::<_,Option<String>>(5)?,"status":r.get::<_,String>(6)?,
            "sourceDocumentVersionId":r.get::<_,Option<String>>(7)?,
            "approvedPassageCount":r.get::<_,i64>(8)?
        })))?.collect::<Result<Vec<_>,_>>()?;
        Ok(Value::Array(rows))
    })
}

#[tauri::command]
pub fn save_authority(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;
    let citation=required_string(&payload,"citation")?;
    let title=required_string(&payload,"title")?;
    let court=payload.get("court").and_then(Value::as_str);
    let decision_date=payload.get("decisionDate").and_then(Value::as_str);
    let source_version=payload.get("sourceDocumentVersionId").and_then(Value::as_str);
    let id=Uuid::new_v4().to_string();
    state.db.write(|conn|{conn.execute(
        "INSERT INTO legal_authorities(
            id,matter_id,citation,title,court,decision_date,source_document_version_id,status
         ) VALUES(?1,?2,?3,?4,?5,?6,?7,'draft')",
        params![id,matter_id,citation,title,court,decision_date,source_version]
    )?;Ok(())})?;
    Ok(json!({"id":id}))
}

#[tauri::command]
pub fn list_authority_passages(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;
    let authority_id=required_string(&payload,"authorityId")?;
    state.db.read(|conn|{
        let mut stmt=conn.prepare(
            "SELECT p.id,p.source_page_id,p.passage_text,p.issue_tag,p.approved,
             dp.page_number,
             (SELECT o.file_name FROM file_occurrences o
              WHERE o.document_version_id=dp.document_version_id AND o.exists_now=1 LIMIT 1)
             FROM legal_authority_passages p
             LEFT JOIN document_pages dp ON dp.id=p.source_page_id
             WHERE p.matter_id=?1 AND p.authority_id=?2"
        )?;
        let rows=stmt.query_map(params![matter_id,authority_id],|r|Ok(json!({
            "id":r.get::<_,String>(0)?,"sourcePageId":r.get::<_,String>(1)?,
            "passageText":r.get::<_,String>(2)?,"issueTag":r.get::<_,Option<String>>(3)?,
            "approved":r.get::<_,i64>(4)?!=0,"page":r.get::<_,Option<i64>>(5)?,
            "fileName":r.get::<_,Option<String>>(6)?
        })))?.collect::<Result<Vec<_>,_>>()?;
        Ok(Value::Array(rows))
    })
}

#[tauri::command]
pub fn add_authority_passage(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;
    let authority_id=required_string(&payload,"authorityId")?;
    let source_page_id=required_string(&payload,"sourcePageId")?;
    let passage_text=required_string(&payload,"passageText")?;
    let issue_tag=payload.get("issueTag").and_then(Value::as_str);
    let id=authorities::add_passage(&state.db,matter_id,authority_id,source_page_id,passage_text,issue_tag)?;
    Ok(json!({"id":id}))
}

#[tauri::command]
pub fn approve_authority_passage(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;
    let authority_id=required_string(&payload,"authorityId")?;
    let passage_id=required_string(&payload,"passageId")?;
    authorities::approve_passage(&state.db,matter_id,authority_id,passage_id)?;
    Ok(json!({"ok":true}))
}

#[tauri::command]
pub fn verify_authority(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;
    let authority_id=required_string(&payload,"authorityId")?;
    let integrity_sha=authorities::verify(&state.db,matter_id,authority_id)?;
    Ok(json!({"integritySha256":integrity_sha}))
}

#[tauri::command]
pub fn list_legal_documents(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;
    state.db.read(|conn|{
        let mut stmt=conn.prepare(
            "SELECT id,matter_id,document_kind,title,status,current_version_id,updated_at
             FROM legal_documents WHERE matter_id=?1 ORDER BY updated_at DESC"
        )?;
        let rows=stmt.query_map([matter_id],|r|Ok(json!({
            "id":r.get::<_,String>(0)?,"matterId":r.get::<_,String>(1)?,"kind":r.get::<_,String>(2)?,
            "title":r.get::<_,String>(3)?,"status":r.get::<_,String>(4)?,
            "currentVersionId":r.get::<_,Option<String>>(5)?,"updatedAt":r.get::<_,String>(6)?
        })))?.collect::<Result<Vec<_>,_>>()?;
        Ok(Value::Array(rows))
    })
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
pub fn create_legal_document_version(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let _=&state;
    {
    let matter_id=required_string(&payload,"matterId")?;let legal_document_id=required_string(&payload,"legalDocumentId")?;
    Ok(json!({"versionId":legal_docs::create_new_version(&state.db,matter_id,legal_document_id)?}))
}
}

#[tauri::command]
pub fn get_legal_document_version(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;
    let version_id=required_string(&payload,"versionId")?;
    state.db.read(|conn|{
        let (legal_document_id,version_number,status):(String,i64,String)=conn.query_row(
            "SELECT legal_document_id,version_number,status FROM legal_document_versions WHERE id=?1 AND matter_id=?2",
            params![version_id,matter_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))
        ).map_err(|_|AppError::NotFound("legal document version".into()))?;

        struct Section{id:String,index:i64,heading:String}
        let mut section_stmt=conn.prepare(
            "SELECT id,section_index,heading FROM legal_document_sections
             WHERE legal_document_version_id=?1 ORDER BY section_index"
        )?;
        let sections=section_stmt.query_map(params![version_id],|r|Ok(Section{
            id:r.get(0)?,index:r.get(1)?,heading:r.get(2)?
        }))?.collect::<Result<Vec<_>,_>>()?;

        let mut paragraph_stmt=conn.prepare(
            "SELECT id,paragraph_index,paragraph_kind,body_text,provenance_state
             FROM legal_document_paragraphs WHERE section_id=?1 ORDER BY paragraph_index"
        )?;
        let mut out_sections=Vec::with_capacity(sections.len());
        for s in sections {
            let paragraphs=paragraph_stmt.query_map(params![s.id],|r|Ok(json!({
                "id":r.get::<_,String>(0)?,"index":r.get::<_,i64>(1)?,"kind":r.get::<_,String>(2)?,
                "bodyText":r.get::<_,String>(3)?,"provenanceState":r.get::<_,String>(4)?
            })))?.collect::<Result<Vec<_>,_>>()?;
            out_sections.push(json!({
                "id":s.id,"index":s.index,"heading":s.heading,"paragraphs":paragraphs
            }));
        }

        Ok(json!({
            "id":version_id,"legalDocumentId":legal_document_id,"versionNumber":version_number,
            "status":status,"sections":out_sections
        }))
    })
}

#[tauri::command]
pub fn fill_legal_document_facts(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;let version_id=required_string(&payload,"versionId")?;
    Ok(json!({"added":legal_docs::fill_from_verified_facts(&state.db,matter_id,version_id)?}))
}

#[tauri::command]
pub fn add_legal_document_paragraph(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;let version_id=required_string(&payload,"versionId")?;
    let section_id=required_string(&payload,"sectionId")?;let body_text=required_string(&payload,"bodyText")?;
    Ok(json!({"id":legal_docs::add_paragraph(&state.db,matter_id,version_id,section_id,body_text)?}))
}

#[tauri::command]
pub fn update_legal_document_paragraph(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;let version_id=required_string(&payload,"versionId")?;
    let paragraph_id=required_string(&payload,"paragraphId")?;let body_text=required_string(&payload,"bodyText")?;
    legal_docs::update_paragraph(&state.db,matter_id,version_id,paragraph_id,body_text)?;
    Ok(json!({"ok":true}))
}

#[tauri::command]
pub fn confirm_legal_document_paragraph(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;let version_id=required_string(&payload,"versionId")?;
    let paragraph_id=required_string(&payload,"paragraphId")?;
    legal_docs::confirm_paragraph(&state.db,matter_id,version_id,paragraph_id)?;
    Ok(json!({"ok":true}))
}

#[tauri::command]
pub fn delete_legal_document_paragraph(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;let version_id=required_string(&payload,"versionId")?;
    let paragraph_id=required_string(&payload,"paragraphId")?;
    legal_docs::delete_paragraph(&state.db,matter_id,version_id,paragraph_id)?;
    Ok(json!({"ok":true}))
}

#[tauri::command]
pub fn export_legal_document(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;
    let version_id=required_string(&payload,"legalDocumentVersionId")?;
    let output_kind=required_string(&payload,"outputKind")?;
    let output_path=required_string(&payload,"outputPath")?;

    if output_kind!="txt"{
        return Err(AppError::PdfConverterUnavailable);
    }

    let content:String=state.db.read(|conn|{
        let (status,stored_hash):(String,Option<String>)=conn.query_row(
            "SELECT status,approval_sha256 FROM legal_document_versions WHERE id=?1 AND matter_id=?2",
            params![version_id,matter_id],|r|Ok((r.get(0)?,r.get(1)?))
        ).map_err(|_|AppError::NotFound("legal document version".into()))?;
        if status!="approved"{
            return Err(AppError::Validation("only approved versions can be exported".into()));
        }
        // Defense in depth: the immutability triggers should make this impossible, but
        // export is the last checkpoint before client-facing content leaves the app -
        // recompute the same canonical hash approve_version bound and refuse to export
        // if it no longer matches what was actually approved.
        let recomputed=legal_docs::compute_approval_hash(conn,matter_id,version_id)?;
        if stored_hash.as_deref()!=Some(recomputed.as_str()){
            return Err(AppError::Validation(
                "approved content integrity check failed - the document no longer matches what was approved, refusing to export".into()
            ));
        }
        let mut stmt=conn.prepare(
            "SELECT p.body_text FROM legal_document_paragraphs p
             JOIN legal_document_sections s ON s.id=p.section_id
             WHERE p.legal_document_version_id=?1
             ORDER BY s.section_index,p.paragraph_index"
        )?;
        let paragraphs=stmt.query_map([version_id],|r|r.get::<_,String>(0))?
            .collect::<Result<Vec<_>,_>>()?;
        Ok(paragraphs.join("\n\n"))
    })?;

    std::fs::write(output_path,&content)?;
    let output_sha256=hex::encode(Sha256::digest(content.as_bytes()));
    let id=Uuid::new_v4().to_string();
    state.db.write(|conn|{conn.execute(
        "INSERT INTO legal_export_audit(
            id,matter_id,legal_document_version_id,output_kind,output_path,output_sha256,exported_at,converter_kind
         ) VALUES(?1,?2,?3,?4,?5,?6,?7,'native_text')",
        params![id,matter_id,version_id,output_kind,output_path,output_sha256,Utc::now().to_rfc3339()]
    )?;Ok(())})?;
    Ok(json!({"id":id,"outputSha256":output_sha256}))
}

// --- Legal rules infrastructure (Phase A): thin command wrappers over legal_rules.rs.
// No Israeli substantive law is decided here - only whether a governed Ruleset exists,
// is properly sourced/tested, and may be used. See legal_rules.rs's module doc comment.

#[tauri::command]
pub fn list_legal_rulesets(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let engine_kind=payload.get("engineKind").and_then(Value::as_str);
    state.db.read(|conn|{
        let mut stmt=conn.prepare(
            "SELECT id,engine_kind,jurisdiction,title,version,status,effective_from,effective_to,
             approved_at,approved_by,superseded_by,
             (SELECT count(*) FROM legal_ruleset_sources s WHERE s.ruleset_id=r.id),
             (SELECT count(*) FROM legal_rule_test_cases t WHERE t.ruleset_id=r.id),
             (SELECT count(*) FROM legal_rule_test_cases t WHERE t.ruleset_id=r.id AND t.review_status='approved')
             FROM legal_rulesets r WHERE ?1 IS NULL OR engine_kind=?1
             ORDER BY engine_kind,jurisdiction,title,version"
        )?;
        let rows=stmt.query_map([engine_kind],|r|Ok(json!({
            "id":r.get::<_,String>(0)?,"engineKind":r.get::<_,String>(1)?,"jurisdiction":r.get::<_,String>(2)?,
            "title":r.get::<_,String>(3)?,"version":r.get::<_,String>(4)?,"status":r.get::<_,String>(5)?,
            "effectiveFrom":r.get::<_,Option<String>>(6)?,"effectiveTo":r.get::<_,Option<String>>(7)?,
            "approvedAt":r.get::<_,Option<String>>(8)?,"approvedBy":r.get::<_,Option<String>>(9)?,
            "supersededBy":r.get::<_,Option<String>>(10)?,
            "sourceCount":r.get::<_,i64>(11)?,"testCaseCount":r.get::<_,i64>(12)?,"approvedTestCaseCount":r.get::<_,i64>(13)?
        })))?.collect::<Result<Vec<_>,_>>()?;
        Ok(Value::Array(rows))
    })
}

#[tauri::command]
pub fn get_legal_ruleset(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let ruleset_id=required_string(&payload,"rulesetId")?;
    state.db.read(|conn|{
        let ruleset=conn.query_row(
            "SELECT id,engine_kind,jurisdiction,title,version,status,effective_from,effective_to,
             description,created_at,created_by,submitted_for_review_at,approved_at,approved_by,
             superseded_by,integrity_sha256
             FROM legal_rulesets WHERE id=?1",
            [ruleset_id],|r|Ok(json!({
                "id":r.get::<_,String>(0)?,"engineKind":r.get::<_,String>(1)?,"jurisdiction":r.get::<_,String>(2)?,
                "title":r.get::<_,String>(3)?,"version":r.get::<_,String>(4)?,"status":r.get::<_,String>(5)?,
                "effectiveFrom":r.get::<_,Option<String>>(6)?,"effectiveTo":r.get::<_,Option<String>>(7)?,
                "description":r.get::<_,Option<String>>(8)?,"createdAt":r.get::<_,String>(9)?,
                "createdBy":r.get::<_,Option<String>>(10)?,"submittedForReviewAt":r.get::<_,Option<String>>(11)?,
                "approvedAt":r.get::<_,Option<String>>(12)?,"approvedBy":r.get::<_,Option<String>>(13)?,
                "supersededBy":r.get::<_,Option<String>>(14)?,"integritySha256":r.get::<_,Option<String>>(15)?
            }))
        ).map_err(|_|AppError::NotFound("legal ruleset".into()))?;

        let mut source_stmt=conn.prepare(
            "SELECT id,source_kind,citation,pinpoint,document_version_id,document_page_id,
             source_sha256,verified_at,verified_by FROM legal_ruleset_sources WHERE ruleset_id=?1 ORDER BY created_at"
        )?;
        let sources=source_stmt.query_map([ruleset_id],|r|Ok(json!({
            "id":r.get::<_,String>(0)?,"sourceKind":r.get::<_,String>(1)?,"citation":r.get::<_,String>(2)?,
            "pinpoint":r.get::<_,Option<String>>(3)?,"documentVersionId":r.get::<_,Option<String>>(4)?,
            "documentPageId":r.get::<_,Option<String>>(5)?,"sourceSha256":r.get::<_,String>(6)?,
            "verifiedAt":r.get::<_,Option<String>>(7)?,"verifiedBy":r.get::<_,Option<String>>(8)?
        })))?.collect::<Result<Vec<_>,_>>()?;

        let mut rule_stmt=conn.prepare(
            "SELECT id,rule_key,rule_type,priority,conditions_json,operation_json,explanation_template,source_id
             FROM legal_rules WHERE ruleset_id=?1 ORDER BY priority"
        )?;
        let rules=rule_stmt.query_map([ruleset_id],|r|{
            let conditions:String=r.get(4)?;
            let operation:String=r.get(5)?;
            Ok(json!({
                "id":r.get::<_,String>(0)?,"ruleKey":r.get::<_,String>(1)?,"ruleType":r.get::<_,String>(2)?,
                "priority":r.get::<_,i64>(3)?,
                "conditions":serde_json::from_str::<Value>(&conditions).unwrap_or(Value::Null),
                "operation":serde_json::from_str::<Value>(&operation).unwrap_or(Value::Null),
                "explanationTemplate":r.get::<_,Option<String>>(6)?,"sourceId":r.get::<_,Option<String>>(7)?
            }))
        })?.collect::<Result<Vec<_>,_>>()?;

        let mut tc_stmt=conn.prepare(
            "SELECT id,name,input_json,expected_output_json,review_status,reviewed_by,reviewed_at
             FROM legal_rule_test_cases WHERE ruleset_id=?1 ORDER BY created_at"
        )?;
        let test_cases=tc_stmt.query_map([ruleset_id],|r|{
            let input:String=r.get(2)?;
            let expected:String=r.get(3)?;
            Ok(json!({
                "id":r.get::<_,String>(0)?,"name":r.get::<_,String>(1)?,
                "input":serde_json::from_str::<Value>(&input).unwrap_or(Value::Null),
                "expectedOutput":serde_json::from_str::<Value>(&expected).unwrap_or(Value::Null),
                "reviewStatus":r.get::<_,String>(4)?,"reviewedBy":r.get::<_,Option<String>>(5)?,
                "reviewedAt":r.get::<_,Option<String>>(6)?
            }))
        })?.collect::<Result<Vec<_>,_>>()?;

        let mut ruleset=ruleset;
        ruleset["sources"]=Value::Array(sources);
        ruleset["rules"]=Value::Array(rules);
        ruleset["testCases"]=Value::Array(test_cases);
        Ok(ruleset)
    })
}

#[tauri::command]
pub fn create_legal_ruleset(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let engine_kind=required_string(&payload,"engineKind")?;
    let jurisdiction=required_string(&payload,"jurisdiction")?;
    let title=required_string(&payload,"title")?;
    let version=required_string(&payload,"version")?;
    let effective_from=payload.get("effectiveFrom").and_then(Value::as_str);
    let effective_to=payload.get("effectiveTo").and_then(Value::as_str);
    let description=payload.get("description").and_then(Value::as_str);
    let created_by=payload.get("createdBy").and_then(Value::as_str);
    let id=legal_rules::create_ruleset(&state.db,engine_kind,jurisdiction,title,version,effective_from,effective_to,description,created_by)?;
    Ok(json!({"id":id}))
}

#[tauri::command]
pub fn update_draft_legal_ruleset(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let ruleset_id=required_string(&payload,"rulesetId")?;
    let title=payload.get("title").and_then(Value::as_str);
    let effective_from=payload.get("effectiveFrom").and_then(Value::as_str);
    let effective_to=payload.get("effectiveTo").and_then(Value::as_str);
    let description=payload.get("description").and_then(Value::as_str);
    legal_rules::update_draft_ruleset(&state.db,ruleset_id,title,effective_from,effective_to,description)?;
    Ok(json!({"ok":true}))
}

#[tauri::command]
pub fn add_legal_ruleset_source(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let ruleset_id=required_string(&payload,"rulesetId")?;
    let source_kind=required_string(&payload,"sourceKind")?;
    let citation=required_string(&payload,"citation")?;
    let pinpoint=payload.get("pinpoint").and_then(Value::as_str);
    let document_version_id=payload.get("documentVersionId").and_then(Value::as_str);
    let document_page_id=payload.get("documentPageId").and_then(Value::as_str);
    let verified_by=payload.get("verifiedBy").and_then(Value::as_str);
    let id=legal_rules::add_source(&state.db,ruleset_id,source_kind,citation,pinpoint,document_version_id,document_page_id,verified_by)?;
    Ok(json!({"id":id}))
}

#[tauri::command]
pub fn add_legal_rule(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let ruleset_id=required_string(&payload,"rulesetId")?;
    let rule_key=required_string(&payload,"ruleKey")?;
    let rule_type=required_string(&payload,"ruleType")?;
    let priority=payload.get("priority").and_then(Value::as_i64).unwrap_or(0);
    let conditions_json=required_json_string(&payload,"conditions")?;
    let operation_json=required_json_string(&payload,"operation")?;
    let explanation_template=payload.get("explanationTemplate").and_then(Value::as_str);
    let source_id=payload.get("sourceId").and_then(Value::as_str);
    let id=legal_rules::add_rule(&state.db,ruleset_id,rule_key,rule_type,priority,&conditions_json,&operation_json,explanation_template,source_id)?;
    Ok(json!({"id":id}))
}

#[tauri::command]
pub fn add_legal_rule_test_case(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let ruleset_id=required_string(&payload,"rulesetId")?;
    let name=required_string(&payload,"name")?;
    let input_json=required_json_string(&payload,"input")?;
    let expected_json=required_json_string(&payload,"expectedOutput")?;
    let id=legal_rules::add_test_case(&state.db,ruleset_id,name,&input_json,&expected_json)?;
    Ok(json!({"id":id}))
}

#[tauri::command]
pub fn review_legal_rule_test_case(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let ruleset_id=required_string(&payload,"rulesetId")?;
    let test_case_id=required_string(&payload,"testCaseId")?;
    let approved=payload.get("approved").and_then(Value::as_bool)
        .ok_or_else(||AppError::Validation("approved required".into()))?;
    let reviewed_by=required_string(&payload,"reviewedBy")?;
    legal_rules::review_test_case(&state.db,ruleset_id,test_case_id,approved,reviewed_by)?;
    Ok(json!({"ok":true}))
}

#[tauri::command]
pub fn run_legal_rule_tests(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let ruleset_id=required_string(&payload,"rulesetId")?;
    let results=legal_rules::run_tests(&state.db,ruleset_id)?;
    Ok(json!(results.into_iter().map(|(name,passed,detail)|json!({
        "name":name,"passed":passed,"detail":detail
    })).collect::<Vec<_>>()))
}

#[tauri::command]
pub fn submit_legal_ruleset_for_review(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let ruleset_id=required_string(&payload,"rulesetId")?;
    legal_rules::submit_for_review(&state.db,ruleset_id)?;
    Ok(json!({"ok":true}))
}

#[tauri::command]
pub fn approve_legal_ruleset(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let ruleset_id=required_string(&payload,"rulesetId")?;
    let approved_by=required_string(&payload,"approvedBy")?;
    let integrity_sha256=legal_rules::approve_ruleset(&state.db,ruleset_id,approved_by)?;
    Ok(json!({"integritySha256":integrity_sha256}))
}

#[tauri::command]
pub fn supersede_legal_ruleset(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let old_ruleset_id=required_string(&payload,"oldRulesetId")?;
    let new_ruleset_id=required_string(&payload,"newRulesetId")?;
    legal_rules::supersede_ruleset(&state.db,old_ruleset_id,new_ruleset_id)?;
    Ok(json!({"ok":true}))
}

#[tauri::command]
pub fn preview_legal_engine_run(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let ruleset_id=required_string(&payload,"rulesetId")?;
    let context_json=required_json_string(&payload,"context")?;
    let outcome=legal_rules::preview_engine_run(&state.db,ruleset_id,&context_json)?;
    Ok(json!({
        "matchedRuleKey":outcome.matched_rule_key,"explanation":outcome.explanation,
        "registers":outcome.registers,"trace":outcome.trace,
        "rulesetVersion":outcome.ruleset_version,"rulesetIntegritySha256":outcome.ruleset_integrity_sha256
    }))
}

#[tauri::command]
pub fn commit_legal_engine_run(state: State<'_, AppState>, payload: Value) -> AppResult<Value> {
    let matter_id=required_string(&payload,"matterId")?;
    let ruleset_id=required_string(&payload,"rulesetId")?;
    let context_json=required_json_string(&payload,"context")?;
    let id=legal_rules::commit_engine_run(&state.db,matter_id,ruleset_id,&context_json)?;
    Ok(json!({"id":id}))
}
