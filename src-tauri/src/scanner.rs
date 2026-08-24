use crate::{
    db::DbState,
    error::{AppError, AppResult},
    source_snapshot::hash_file,
};
use chrono::Utc;
use rusqlite::params;
use std::path::{Path, PathBuf};
use uuid::Uuid;
use walkdir::WalkDir;

fn path_key(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase()
}

fn ignored_name(name: &str) -> bool {
    name.starts_with("~$") || name.ends_with(".tmp") || name.ends_with(".part")
}

pub fn scan_metadata(db: &DbState, root: &Path) -> AppResult<String> {
    let run_id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();
    db.write(|conn| {
        conn.execute(
            "INSERT INTO scan_runs(id,root_path,status,started_at) VALUES(?1,?2,'running',?3)",
            params![run_id, root.to_string_lossy(), now],
        )?;
        Ok(())
    })?;

    let mut batch: Vec<(PathBuf, String, i64, String)> = Vec::with_capacity(250);
    for entry in WalkDir::new(root).follow_links(false).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() { continue; }
        let name = entry.file_name().to_string_lossy().to_string();
        if ignored_name(&name) { continue; }
        let metadata = entry.metadata().map_err(|e| {
            AppError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
        })?;
        let mtime = metadata.modified().ok().map(|x| format!("{x:?}")).unwrap_or_default();
        batch.push((entry.path().to_path_buf(), name, metadata.len() as i64, mtime));
        if batch.len() >= 250 {
            flush_batch(db, &run_id, root, &batch)?;
            batch.clear();
        }
    }
    if !batch.is_empty() { flush_batch(db, &run_id, root, &batch)?; }

    db.write(|conn| {
        conn.execute(
            "UPDATE scan_runs SET status='complete',finished_at=?2 WHERE id=?1",
            params![run_id, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    })?;
    Ok(run_id)
}

fn suggestion_folder(root: &Path, path: &Path) -> Option<PathBuf> {
    let rel = path.strip_prefix(root).ok()?;
    let first = rel.components().next()?;
    Some(root.join(first.as_os_str()))
}

fn flush_batch(db: &DbState, run_id: &str, root: &Path, rows: &[(PathBuf, String, i64, String)]) -> AppResult<()> {
    db.write(|conn| {
        let tx = conn.transaction()?;
        for (path, name, size, mtime) in rows {
            let key = path_key(path);
            let id = Uuid::new_v4().to_string();
            let matched = tx.execute(
                "INSERT INTO file_occurrences(
                    id,matter_id,path_display,path_key,file_name,byte_size,observed_mtime,
                    availability_state,discovered_at,last_seen_at
                 )
                 SELECT ?1,m.id,?2,?3,?4,?5,?6,'local',?7,?7
                 FROM matters m
                 JOIN matter_folder_bindings b ON b.matter_id=m.id AND b.active=1
                 WHERE ?3=b.path_key OR ?3 LIKE b.path_key || '\\%'
                 ORDER BY length(b.path_key) DESC LIMIT 1
                 ON CONFLICT(path_key) DO UPDATE SET
                    byte_size=excluded.byte_size,
                    observed_mtime=excluded.observed_mtime,
                    last_seen_at=excluded.last_seen_at,
                    exists_now=1",
                params![id, path.to_string_lossy(), key, name, size, mtime, Utc::now().to_rfc3339()],
            )?;
            if matched == 0 {
                if let Some(folder) = suggestion_folder(root, path) {
                    let folder_key = path_key(&folder);
                    let folder_name = folder.file_name()
                        .map(|x| x.to_string_lossy().to_string())
                        .unwrap_or_else(|| folder.to_string_lossy().to_string());
                    tx.execute(
                        "INSERT INTO matter_suggestions(
                            id,path_display,path_key,suggested_title,file_count,status,created_at
                         ) VALUES(?1,?2,?3,?4,1,'pending',?5)
                         ON CONFLICT(path_key) DO UPDATE SET file_count=file_count+1
                         WHERE matter_suggestions.status='pending'",
                        params![Uuid::new_v4().to_string(), folder.to_string_lossy(), folder_key, folder_name, Utc::now().to_rfc3339()],
                    )?;
                }
            }
        }
        tx.execute(
            "UPDATE scan_runs SET discovered_count=discovered_count+?2 WHERE id=?1",
            params![run_id, rows.len() as i64],
        )?;
        tx.commit()?;
        Ok(())
    })
}

pub fn hash_pending(db: &DbState, matter_id: &str) -> AppResult<usize> {
    let pending: Vec<(String, String, i64)> = db.read(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id,path_display,byte_size
             FROM file_occurrences
             WHERE matter_id=?1 AND exists_now=1 AND availability_state='local'
               AND document_version_id IS NULL",
        )?;
        let rows = stmt.query_map([matter_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })?;

    let mut count = 0usize;
    for (occurrence_id, display_path, indexed_size) in pending {
        let path = PathBuf::from(&display_path);
        let before = std::fs::metadata(&path)?;
        if before.len() as i64 != indexed_size { continue; }

        let sha = hash_file(&path)?;
        let after = std::fs::metadata(&path)?;
        if after.len() != before.len() || after.modified().ok() != before.modified().ok() { continue; }

        db.write(|conn| {
            let tx = conn.transaction()?;
            let document_id = Uuid::new_v4().to_string();
            let version_id = Uuid::new_v4().to_string();
            let now = Utc::now().to_rfc3339();

            tx.execute(
                "INSERT INTO documents(id,matter_id,logical_title,created_at,updated_at)
                 SELECT ?1,matter_id,file_name,?3,?3 FROM file_occurrences WHERE id=?2",
                params![document_id, occurrence_id, now],
            )?;
            tx.execute(
                "INSERT INTO document_versions(
                    id,document_id,matter_id,content_sha256,byte_size,observed_mtime,created_at
                 )
                 SELECT ?1,?2,matter_id,?3,byte_size,observed_mtime,?4
                 FROM file_occurrences WHERE id=?5",
                params![version_id, document_id, sha, now, occurrence_id],
            )?;
            tx.execute(
                "UPDATE file_occurrences SET document_id=?2,document_version_id=?3 WHERE id=?1",
                params![occurrence_id, document_id, version_id],
            )?;
            tx.commit()?;
            Ok(())
        })?;
        count += 1;
    }
    Ok(count)
}
