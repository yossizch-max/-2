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
        conn.execute(
            "UPDATE legal_documents SET status='approved',updated_at=?3
             WHERE matter_id=?1 AND current_version_id=?2",
            params![matter_id,version_id,Utc::now().to_rfc3339()]
        )?;
        Ok(approval_sha)
    })
}

/// Starts a new draft version from an approved legal document, deep-copying its
/// sections/paragraphs/sources as the editable starting point. The prior version
/// (and its provenance chain) is left untouched and immutable.
pub fn create_new_version(db:&DbState,matter_id:&str,legal_document_id:&str)->AppResult<String>{
    let new_version_id=Uuid::new_v4().to_string();
    let now=Utc::now().to_rfc3339();
    db.write(|conn|{
        let tx=conn.transaction()?;

        let (old_version_id,old_version_number,old_status):(String,i64,String)=tx.query_row(
            "SELECT v.id,v.version_number,v.status
             FROM legal_documents d
             JOIN legal_document_versions v
               ON v.id=d.current_version_id AND v.matter_id=d.matter_id
             WHERE d.id=?1 AND d.matter_id=?2",
            params![legal_document_id,matter_id],
            |r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))
        ).map_err(|_|AppError::NotFound("legal document".into()))?;
        if old_status!="approved"{
            return Err(AppError::Validation("only an approved version can start a new draft version".into()));
        }

        let content_sha256:String=tx.query_row(
            "SELECT content_sha256 FROM legal_document_versions WHERE id=?1 AND matter_id=?2",
            params![old_version_id,matter_id],|r|r.get(0)
        )?;
        tx.execute(
            "INSERT INTO legal_document_versions(
                id,matter_id,legal_document_id,parent_version_id,version_number,status,content_sha256,created_at
             ) VALUES(?1,?2,?3,?4,?5,'draft',?6,?7)",
            params![new_version_id,matter_id,legal_document_id,old_version_id,old_version_number+1,content_sha256,now]
        )?;

        let sections:Vec<(String,i64,String)>={
            let mut stmt=tx.prepare(
                "SELECT id,section_index,heading FROM legal_document_sections
                 WHERE legal_document_version_id=?1 ORDER BY section_index"
            )?;
            let rows=stmt.query_map(params![old_version_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?
                .collect::<Result<Vec<_>,_>>()?;
            rows
        };

        for (old_section_id,section_index,heading) in sections {
            let new_section_id=Uuid::new_v4().to_string();
            tx.execute(
                "INSERT INTO legal_document_sections(
                    id,matter_id,legal_document_version_id,section_index,heading
                 ) VALUES(?1,?2,?3,?4,?5)",
                params![new_section_id,matter_id,new_version_id,section_index,heading]
            )?;

            let paragraphs:Vec<(String,i64,String,String,String)>={
                let mut stmt=tx.prepare(
                    "SELECT id,paragraph_index,paragraph_kind,body_text,provenance_state
                     FROM legal_document_paragraphs WHERE section_id=?1 ORDER BY paragraph_index"
                )?;
                let rows=stmt.query_map(params![old_section_id],|r|
                    Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))
                )?.collect::<Result<Vec<_>,_>>()?;
                rows
            };

            for (old_paragraph_id,paragraph_index,paragraph_kind,body_text,provenance_state) in paragraphs {
                let new_paragraph_id=Uuid::new_v4().to_string();
                tx.execute(
                    "INSERT INTO legal_document_paragraphs(
                        id,matter_id,legal_document_version_id,section_id,paragraph_index,paragraph_kind,body_text,provenance_state
                     ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                    params![new_paragraph_id,matter_id,new_version_id,new_section_id,paragraph_index,paragraph_kind,body_text,provenance_state]
                )?;

                let sources:Vec<(String,Option<String>,Option<String>,Option<String>)>={
                    let mut stmt=tx.prepare(
                        "SELECT source_kind,verified_fact_id,authority_passage_id,document_page_id
                         FROM legal_document_sources WHERE paragraph_id=?1"
                    )?;
                    let rows=stmt.query_map(params![old_paragraph_id],|r|
                        Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))
                    )?.collect::<Result<Vec<_>,_>>()?;
                    rows
                };

                for (source_kind,verified_fact_id,authority_passage_id,document_page_id) in sources {
                    tx.execute(
                        "INSERT INTO legal_document_sources(
                            id,matter_id,legal_document_version_id,paragraph_id,source_kind,verified_fact_id,authority_passage_id,document_page_id
                         ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                        params![Uuid::new_v4().to_string(),matter_id,new_version_id,new_paragraph_id,source_kind,verified_fact_id,authority_passage_id,document_page_id]
                    )?;
                }
            }
        }

        tx.execute(
            "UPDATE legal_documents SET current_version_id=?1,status='draft',updated_at=?2
             WHERE id=?3 AND matter_id=?4",
            params![new_version_id,now,legal_document_id,matter_id]
        )?;

        tx.commit()?;
        Ok(())
    })?;
    Ok(new_version_id)
}
