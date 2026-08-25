use crate::{db::DbState, error::{AppError,AppResult}};
use chrono::Utc;
use rusqlite::params;
use sha2::{Digest,Sha256};
use uuid::Uuid;

/// The trailing template section every document kind gets, used as the fixed
/// insertion point for `fill_from_verified_facts`.
pub const FACTS_SECTION_HEADING:&str="עובדות מאומתות";

const TEMPLATE_DEMAND:&[&str]=&["רקע ועובדות","עילת החבות","הנזק והפיצוי המבוקש","דרישה סופית לתשלום",FACTS_SECTION_HEADING];
const TEMPLATE_CLAIM:&[&str]=&["הצדדים","העובדות","העילות המשפטיות","הנזק","הסעדים המבוקשים",FACTS_SECTION_HEADING];
const TEMPLATE_RESPONSE:&[&str]=&["כללי","תמצית טענות ההגנה","מענה לפרק העובדות","מענה לפרק הנזק",FACTS_SECTION_HEADING];

fn template_sections(kind:&str)->&'static [&'static str]{
    match kind {
        "claim"=>TEMPLATE_CLAIM,
        "response"=>TEMPLATE_RESPONSE,
        _=>TEMPLATE_DEMAND,
    }
}

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
        for (index,heading) in template_sections(kind).iter().enumerate(){
            tx.execute(
                "INSERT INTO legal_document_sections(
                    id,matter_id,legal_document_version_id,section_index,heading
                 ) VALUES(?1,?2,?3,?4,?5)",
                params![Uuid::new_v4().to_string(),matter_id,version_id,index as i64,heading]
            )?;
        }
        tx.commit()?;
        Ok(())
    })?;
    Ok(document_id)
}

fn require_draft(conn:&rusqlite::Connection,matter_id:&str,version_id:&str)->AppResult<()>{
    let status:String=conn.query_row(
        "SELECT status FROM legal_document_versions WHERE id=?1 AND matter_id=?2",
        params![version_id,matter_id],|r|r.get(0)
    ).map_err(|_|AppError::NotFound("legal document version".into()))?;
    if status!="draft"{
        return Err(AppError::Validation("only a draft version can be edited".into()));
    }
    Ok(())
}

/// Appends one confirmed, source-grounded paragraph per verified fact that isn't
/// already linked into this version, into the fixed FACTS_SECTION_HEADING section
/// (created if missing). Idempotent: re-running only adds newly verified facts.
pub fn fill_from_verified_facts(db:&DbState,matter_id:&str,version_id:&str)->AppResult<i64>{
    db.write(|conn|{
        let tx=conn.transaction()?;
        require_draft(&tx,matter_id,version_id)?;

        let section_id:String=match tx.query_row(
            "SELECT id FROM legal_document_sections WHERE legal_document_version_id=?1 AND heading=?2",
            params![version_id,FACTS_SECTION_HEADING],|r|r.get(0)
        ){
            Ok(id)=>id,
            Err(_)=>{
                let next_index:i64=tx.query_row(
                    "SELECT coalesce(max(section_index),-1)+1 FROM legal_document_sections WHERE legal_document_version_id=?1",
                    params![version_id],|r|r.get(0)
                )?;
                let new_id=Uuid::new_v4().to_string();
                tx.execute(
                    "INSERT INTO legal_document_sections(
                        id,matter_id,legal_document_version_id,section_index,heading
                     ) VALUES(?1,?2,?3,?4,?5)",
                    params![new_id,matter_id,version_id,next_index,FACTS_SECTION_HEADING]
                )?;
                new_id
            }
        };

        let already_linked:Vec<String>={
            let mut stmt=tx.prepare(
                "SELECT DISTINCT verified_fact_id FROM legal_document_sources
                 WHERE legal_document_version_id=?1 AND verified_fact_id IS NOT NULL"
            )?;
            let rows=stmt.query_map(params![version_id],|r|r.get(0))?.collect::<Result<Vec<_>,_>>()?;
            rows
        };

        let facts:Vec<(String,String,String,String)>={
            let mut stmt=tx.prepare(
                "SELECT id,subject,predicate,value_text FROM verified_facts
                 WHERE matter_id=?1 AND status='valid' ORDER BY verified_at"
            )?;
            let rows=stmt.query_map(params![matter_id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?
                .collect::<Result<Vec<_>,_>>()?;
            rows
        };

        let mut next_index:i64=tx.query_row(
            "SELECT coalesce(max(paragraph_index),-1)+1 FROM legal_document_paragraphs WHERE section_id=?1",
            params![section_id],|r|r.get(0)
        )?;

        let mut added=0i64;
        for (fact_id,subject,predicate,value_text) in facts {
            if already_linked.iter().any(|id|id==&fact_id){continue;}
            let paragraph_id=Uuid::new_v4().to_string();
            let body_text=format!("{subject} {predicate}: {value_text}");
            tx.execute(
                "INSERT INTO legal_document_paragraphs(
                    id,matter_id,legal_document_version_id,section_id,paragraph_index,paragraph_kind,body_text,provenance_state
                 ) VALUES(?1,?2,?3,?4,?5,'fact',?6,'confirmed')",
                params![paragraph_id,matter_id,version_id,section_id,next_index,body_text]
            )?;
            tx.execute(
                "INSERT INTO legal_document_sources(
                    id,matter_id,legal_document_version_id,paragraph_id,source_kind,verified_fact_id
                 ) VALUES(?1,?2,?3,?4,'verified_fact',?5)",
                params![Uuid::new_v4().to_string(),matter_id,version_id,paragraph_id,fact_id]
            )?;
            next_index+=1;
            added+=1;
        }

        tx.commit()?;
        Ok(added)
    })
}

/// A manually-added paragraph is always paragraph_kind='argument': lawyer-authored
/// legal text, editorial framing, prayer for relief - not a factual claim. Paragraphs
/// asserting facts (paragraph_kind='fact') can only be created by
/// `fill_from_verified_facts`, which grounds them in a real verified fact atomically -
/// this keeps "a fact requires a source" true by construction rather than by convention.
pub fn add_paragraph(db:&DbState,matter_id:&str,version_id:&str,section_id:&str,body_text:&str)->AppResult<String>{
    let paragraph_id=Uuid::new_v4().to_string();
    db.write(|conn|{
        require_draft(conn,matter_id,version_id)?;
        let section_version:String=conn.query_row(
            "SELECT legal_document_version_id FROM legal_document_sections WHERE id=?1 AND matter_id=?2",
            params![section_id,matter_id],|r|r.get(0)
        ).map_err(|_|AppError::NotFound("legal document section".into()))?;
        if section_version!=version_id{
            return Err(AppError::Validation("section does not belong to this version".into()));
        }
        let next_index:i64=conn.query_row(
            "SELECT coalesce(max(paragraph_index),-1)+1 FROM legal_document_paragraphs WHERE section_id=?1",
            params![section_id],|r|r.get(0)
        )?;
        conn.execute(
            "INSERT INTO legal_document_paragraphs(
                id,matter_id,legal_document_version_id,section_id,paragraph_index,paragraph_kind,body_text,provenance_state
             ) VALUES(?1,?2,?3,?4,?5,'argument',?6,'needs_review')",
            params![paragraph_id,matter_id,version_id,section_id,next_index,body_text]
        )?;
        Ok(())
    })?;
    Ok(paragraph_id)
}

fn paragraph_version_status(conn:&rusqlite::Connection,matter_id:&str,version_id:&str,paragraph_id:&str)->AppResult<String>{
    conn.query_row(
        "SELECT v.status FROM legal_document_versions v
         JOIN legal_document_paragraphs p ON p.legal_document_version_id=v.id
         WHERE p.id=?1 AND p.matter_id=?2 AND v.id=?3",
        params![paragraph_id,matter_id,version_id],|r|r.get(0)
    ).map_err(|_|AppError::NotFound("legal document paragraph".into()))
}

/// Editing a paragraph's text always drops it back to 'needs_review' - the DB schema
/// has no immutability trigger on legal_document_paragraphs itself (only on
/// legal_document_versions/legal_document_sources), so `require_draft` is what
/// actually stops this from touching an approved version.
pub fn update_paragraph(db:&DbState,matter_id:&str,version_id:&str,paragraph_id:&str,body_text:&str)->AppResult<()>{
    db.write(|conn|{
        let status=paragraph_version_status(conn,matter_id,version_id,paragraph_id)?;
        if status!="draft"{
            return Err(AppError::Validation("only a draft version can be edited".into()));
        }
        conn.execute(
            "UPDATE legal_document_paragraphs SET body_text=?1,provenance_state='needs_review'
             WHERE id=?2 AND matter_id=?3",
            params![body_text,paragraph_id,matter_id]
        )?;
        Ok(())
    })
}

fn has_valid_grounding(conn:&rusqlite::Connection,matter_id:&str,paragraph_id:&str)->AppResult<bool>{
    let grounded:i64=conn.query_row(
        "SELECT count(*) FROM legal_document_sources s
         JOIN verified_facts f ON f.id=s.verified_fact_id AND f.matter_id=s.matter_id
         WHERE s.matter_id=?1 AND s.paragraph_id=?2 AND s.source_kind='verified_fact'
           AND f.status='valid' AND f.stale=0",
        params![matter_id,paragraph_id],|r|r.get(0)
    )?;
    Ok(grounded>0)
}

/// A 'fact' paragraph may only be confirmed while it still has at least one live
/// source: a verified_fact that is currently status='valid' and not stale. An
/// 'argument' paragraph (lawyer-authored legal text) has no such requirement - it's
/// confirmed by editorial review, not source grounding.
pub fn confirm_paragraph(db:&DbState,matter_id:&str,version_id:&str,paragraph_id:&str)->AppResult<()>{
    db.write(|conn|{
        let status=paragraph_version_status(conn,matter_id,version_id,paragraph_id)?;
        if status!="draft"{
            return Err(AppError::Validation("only a draft version can be edited".into()));
        }
        let kind:String=conn.query_row(
            "SELECT paragraph_kind FROM legal_document_paragraphs WHERE id=?1 AND matter_id=?2",
            params![paragraph_id,matter_id],|r|r.get(0)
        )?;
        if kind=="fact" && !has_valid_grounding(conn,matter_id,paragraph_id)?{
            return Err(AppError::Validation(
                "a factual paragraph requires at least one currently valid, non-stale verified fact as a source before it can be confirmed".into()
            ));
        }
        conn.execute(
            "UPDATE legal_document_paragraphs SET provenance_state='confirmed' WHERE id=?1 AND matter_id=?2",
            params![paragraph_id,matter_id]
        )?;
        Ok(())
    })
}

pub fn delete_paragraph(db:&DbState,matter_id:&str,version_id:&str,paragraph_id:&str)->AppResult<()>{
    db.write(|conn|{
        let status=paragraph_version_status(conn,matter_id,version_id,paragraph_id)?;
        if status!="draft"{
            return Err(AppError::Validation("only a draft version can be edited".into()));
        }
        conn.execute(
            "DELETE FROM legal_document_paragraphs WHERE id=?1 AND matter_id=?2",
            params![paragraph_id,matter_id]
        )?;
        Ok(())
    })
}

/// A deterministic, canonical rendering of a version's full authored content: section
/// headings in order, then each paragraph's kind/text/provenance in order, then each
/// paragraph's sources - plus the linked damage calculation's own integrity hash, if
/// this version cites one. This is what `approval_sha256` actually binds, so that
/// approved content can't drift (heading reworded, paragraph reordered, a source
/// silently unlinked) without changing the hash - not just the paragraph bodies.
pub fn canonical_content(conn:&rusqlite::Connection,matter_id:&str,version_id:&str)->AppResult<String>{
    let mut out=String::new();

    if let Some(damage_calculation_id)=conn.query_row(
        "SELECT damage_calculation_id FROM legal_document_versions WHERE id=?1 AND matter_id=?2",
        params![version_id,matter_id],|r|r.get::<_,Option<String>>(0)
    )?{
        let (status,integrity):(String,Option<String>)=conn.query_row(
            "SELECT status,integrity_sha256 FROM damage_calculations WHERE id=?1 AND matter_id=?2",
            params![damage_calculation_id,matter_id],|r|Ok((r.get(0)?,r.get(1)?))
        ).map_err(|_|AppError::NotFound("linked damage calculation".into()))?;
        if status!="locked"{
            return Err(AppError::Validation("this version cites a damage calculation that is not locked".into()));
        }
        out.push_str(&format!("damage:{damage_calculation_id}:{}\n",integrity.unwrap_or_default()));
    }

    let mut section_stmt=conn.prepare(
        "SELECT id,section_index,heading FROM legal_document_sections
         WHERE matter_id=?1 AND legal_document_version_id=?2 ORDER BY section_index"
    )?;
    let sections:Vec<(String,i64,String)>=section_stmt.query_map(params![matter_id,version_id],|r|
        Ok((r.get(0)?,r.get(1)?,r.get(2)?))
    )?.collect::<Result<Vec<_>,_>>()?;

    for (section_id,section_index,heading) in sections {
        out.push_str(&format!("§{section_index}:{heading}\n"));
        let mut para_stmt=conn.prepare(
            "SELECT id,paragraph_index,paragraph_kind,body_text,provenance_state
             FROM legal_document_paragraphs WHERE section_id=?1 ORDER BY paragraph_index"
        )?;
        let paragraphs:Vec<(String,i64,String,String,String)>=para_stmt.query_map(params![section_id],|r|
            Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))
        )?.collect::<Result<Vec<_>,_>>()?;

        for (paragraph_id,paragraph_index,kind,body_text,provenance) in paragraphs {
            out.push_str(&format!("  ¶{paragraph_index}[{kind}/{provenance}]:{body_text}\n"));
            let mut src_stmt=conn.prepare(
                "SELECT source_kind,coalesce(verified_fact_id,''),coalesce(authority_passage_id,''),coalesce(document_page_id,'')
                 FROM legal_document_sources WHERE paragraph_id=?1 ORDER BY id"
            )?;
            let sources:Vec<(String,String,String,String)>=src_stmt.query_map(params![paragraph_id],|r|
                Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))
            )?.collect::<Result<Vec<_>,_>>()?;
            for (source_kind,fact,authority,page) in sources {
                out.push_str(&format!("    src:{source_kind}:{fact}:{authority}:{page}\n"));
            }
        }
    }
    Ok(out)
}

pub fn compute_approval_hash(conn:&rusqlite::Connection,matter_id:&str,version_id:&str)->AppResult<String>{
    let content=canonical_content(conn,matter_id,version_id)?;
    Ok(hex::encode(Sha256::digest(format!("{matter_id}:{version_id}:{content}"))))
}

pub fn approve_version(db:&DbState,matter_id:&str,version_id:&str)->AppResult<String>{
    db.write(|conn|{
        let tx=conn.transaction()?;

        let pending:i64=tx.query_row(
            "SELECT count(*) FROM legal_document_paragraphs
             WHERE matter_id=?1 AND legal_document_version_id=?2
               AND provenance_state<>'confirmed'",
            params![matter_id,version_id], |r|r.get(0)
        )?;
        if pending>0{
            return Err(AppError::Validation("paragraph provenance review pending".into()));
        }

        // Re-validate at approval time, not just at confirm time: a fact confirmed
        // earlier in the drafting session may have been invalidated or gone stale
        // since. approve_version is the last real gate before this content becomes
        // immutable, so it must not trust a provenance_state set in the past.
        let ungrounded:i64=tx.query_row(
            "SELECT count(*) FROM legal_document_paragraphs p
             WHERE p.matter_id=?1 AND p.legal_document_version_id=?2 AND p.paragraph_kind='fact'
               AND NOT EXISTS(
                 SELECT 1 FROM legal_document_sources s
                 JOIN verified_facts f ON f.id=s.verified_fact_id AND f.matter_id=s.matter_id
                 WHERE s.paragraph_id=p.id AND s.source_kind='verified_fact'
                   AND f.status='valid' AND f.stale=0
               )",
            params![matter_id,version_id], |r|r.get(0)
        )?;
        if ungrounded>0{
            return Err(AppError::Validation(
                "one or more factual paragraphs cite a fact that is no longer valid or has gone stale - re-verify or remove it before approving".into()
            ));
        }

        let approval_sha=compute_approval_hash(&tx,matter_id,version_id)?;
        let changed=tx.execute(
            "UPDATE legal_document_versions
             SET status='approved',approval_sha256=?3,approved_at=?4
             WHERE matter_id=?1 AND id=?2 AND status='draft'",
            params![matter_id,version_id,approval_sha,Utc::now().to_rfc3339()]
        )?;
        if changed!=1{return Err(AppError::Validation("version not approvable".into()));}
        tx.execute(
            "UPDATE legal_documents SET status='approved',updated_at=?3
             WHERE matter_id=?1 AND current_version_id=?2",
            params![matter_id,version_id,Utc::now().to_rfc3339()]
        )?;
        tx.commit()?;
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
