use crate::{db::DbState, error::AppResult, models::SearchHit};

pub fn search(db: &DbState, query: &str) -> AppResult<Vec<SearchHit>> {
    let like = format!("%{}%", query);
    db.read(|conn| {
        let mut out = Vec::new();

        let mut matters = conn.prepare(
            "SELECT id,title,coalesce(internal_number,'')
             FROM matters WHERE title LIKE ?1 OR internal_number LIKE ?1 LIMIT 30"
        )?;
        for row in matters.query_map([&like], |r| Ok(SearchHit {
            kind: "matter".into(), matter_id: None, id: r.get(0)?,
            title: r.get(1)?, subtitle: r.get(2)?,
        }))? { out.push(row?); }

        let mut files = conn.prepare(
            "SELECT id,matter_id,file_name,coalesce(extension,'')
             FROM file_occurrences WHERE file_name LIKE ?1 LIMIT 50"
        )?;
        for row in files.query_map([&like], |r| Ok(SearchHit {
            kind: "file".into(), matter_id: Some(r.get(1)?), id: r.get(0)?,
            title: r.get(2)?, subtitle: r.get(3)?,
        }))? { out.push(row?); }

        let mut facts = conn.prepare(
            "SELECT id,matter_id,subject || ' · ' || predicate,value_text
             FROM verified_facts
             WHERE status='valid' AND (subject LIKE ?1 OR predicate LIKE ?1 OR value_text LIKE ?1)
             LIMIT 30"
        )?;
        for row in facts.query_map([&like], |r| Ok(SearchHit {
            kind: "verified_fact".into(), matter_id: Some(r.get(1)?), id: r.get(0)?,
            title: r.get(2)?, subtitle: r.get(3)?,
        }))? { out.push(row?); }

        let mut pages = conn.prepare(
            "SELECT d.id, p.matter_id,
                    coalesce(d.logical_title, fo.file_name, d.category),
                    substr(p.display_text, 1, 160)
             FROM document_pages p
             JOIN document_versions dv ON dv.id=p.document_version_id AND dv.matter_id=p.matter_id
             JOIN documents d ON d.id=dv.document_id AND d.matter_id=dv.matter_id
             LEFT JOIN file_occurrences fo ON fo.document_version_id=dv.id AND fo.exists_now=1
             WHERE p.normalized_text LIKE ?1
             GROUP BY d.id
             LIMIT 30"
        )?;
        for row in pages.query_map([&like], |r| Ok(SearchHit {
            kind: "document_page".into(), matter_id: Some(r.get(1)?), id: r.get(0)?,
            title: r.get(2)?, subtitle: r.get(3)?,
        }))? { out.push(row?); }

        Ok(out)
    })
}
