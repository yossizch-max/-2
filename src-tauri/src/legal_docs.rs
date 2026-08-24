use crate::{db::DbState, error::{AppError,AppResult}};
use chrono::Utc;
use rusqlite::params;
use sha2::{Digest,Sha256};
use uuid::Uuid;

pub fn create_draft(db:&DbState,matter_id:&str,title:&str,kind:&str)->AppResult<String>{
    let document_id=Uuid::new_v4().to_string();
    let version_id=Uuid::new_v4().to_string();
    let now=Utc::now().to_rfc3339();
    db.write(|conn|{
        let tx=conn.transaction()?;
        tx.execute(
            "INSERT INTO legal_documents(
                id,matter_id,document_kind,title,status,current_version_id,created_at,updated_at
             ) VALUES(?1,?2,?3,?4,'draft',?5,?6,?6)",
            params![document_id,matter_id,kind,title,version_id,now]
        )?;
        tx.execute(
            "INSERT INTO legal_document_versions(
                id,matter_id,legal_document_id,version_number,status,content_sha256,created_at
             ) VALUES(?1,?2,?3,1,'draft',?4,?5)",
            params![version_id,matter_id,document_id,hex::encode(Sha256::digest(b"empty")),now]
        )?;
        tx.commit()?;
        Ok(())
    })?;
    Ok(document_id)
}

pub fn approve_version(db:&DbState,matter_id:&str,version_id:&str)->AppResult<String>{
    db.write(|conn|{
        let pending:i64=conn.query_row(
            "SELECT count(*) FROM legal_document_paragraphs
             WHERE matter_id=?1 AND legal_document_version_id=?2
               AND provenance_state<>'confirmed'",
            params![matter_id,version_id], |r|r.get(0)
        )?;
        if pending>0{
            return Err(AppError::Validation("paragraph provenance review pending".into()));
        }

        let content=conn.query_row(
            "SELECT coalesce(group_concat(body_text,'\n'),'')
             FROM legal_document_paragraphs
             WHERE matter_id=?1 AND legal_document_version_id=?2",
            params![matter_id,version_id], |r|r.get::<_,String>(0)
        )?;

        let approval_sha=hex::encode(Sha256::digest(
            format!("{matter_id}:{version_id}:{content}")
        ));
        let changed=conn.execute(
            "UPDATE legal_document_versions
             SET status='approved',approval_sha256=?3,approved_at=?4
             WHERE matter_id=?1 AND id=?2 AND status='draft'",
            params![matter_id,version_id,approval_sha,Utc::now().to_rfc3339()]
        )?;
        if changed!=1{return Err(AppError::Validation("version not approvable".into()));}
        Ok(approval_sha)
    })
}
