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
    let started_at = Utc::now().to_rfc3339();
    db.write(|conn| {
        conn.execute(
            "INSERT INTO scan_runs(id,root_path,status,started_at) VALUES(?1,?2,'running',?3)",
            params![run_id, root.to_string_lossy(), started_at],
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
            flush_batch(db, &run_id, root, &batch, &started_at)?;
            batch.clear();
        }
    }
    if !batch.is_empty() { flush_batch(db, &run_id, root, &batch, &started_at)?; }

    // Reaching here means the walk above completed without an early `?` return, i.e.
    // this was a full, uninterrupted scan of `root` - so it's authoritative: anything
    // previously known under this root that wasn't touched this run (its last_seen_at
    // is still older than started_at) genuinely isn't there anymore. A failed/partial
    // scan returns before this point and never mass-marks anything missing.
    let root_key = path_key(root);
    db.write(|conn| {
        // julianday(), not raw string comparison: chrono's to_rfc3339() uses
        // variable-precision fractional seconds (no digits at all when they're
        // exactly zero), so two RFC3339 strings aren't guaranteed lexicographically
        // sortable against each other. julianday() parses ISO8601 into a comparable
        // numeric value regardless of the source strings' formatting.
        conn.execute(
            "UPDATE file_occurrences SET exists_now=0
             WHERE exists_now=1 AND julianday(last_seen_at)<julianday(?1)
               AND (path_key=?2 OR path_key LIKE ?2 || '\\%')",
            params![started_at, root_key],
        )?;
        Ok(())
    })?;

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

fn flush_batch(db: &DbState, run_id: &str, root: &Path, rows: &[(PathBuf, String, i64, String)], started_at: &str) -> AppResult<()> {
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
                params![id, path.to_string_lossy(), key, name, size, mtime, started_at],
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
    Ok(hash_never_versioned(db, matter_id)? + rehash_changed_versions(db, matter_id)?)
}

fn hash_never_versioned(db: &DbState, matter_id: &str) -> AppResult<usize> {
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

/// A file that was already hashed into a DocumentVersion can legitimately change on
/// disk later (the lawyer edits/replaces the source). `flush_batch`'s `ON CONFLICT`
/// refreshes `file_occurrences.byte_size`/`observed_mtime` on every rescan even for an
/// occurrence that already has a DocumentVersion - so a mismatch between the
/// occurrence's current metadata and the metadata recorded on its DocumentVersion is
/// exactly the "source changed after hashing" signal, with no separate dirty flag
/// needed. When a real content hash change confirms that, this creates a new
/// DocumentVersion under the SAME logical Document, marks the old version stale, moves
/// the occurrence onto the new version, and cascades staleness onto any VerifiedFact
/// grounded in the old version's pages.
fn rehash_changed_versions(db: &DbState, matter_id: &str) -> AppResult<usize> {
    let pending: Vec<(String, String, String, i64)> = db.read(|conn| {
        let mut stmt = conn.prepare(
            "SELECT o.id,o.path_display,o.document_id,o.byte_size
             FROM file_occurrences o
             JOIN document_versions dv ON dv.id=o.document_version_id AND dv.matter_id=o.matter_id
             WHERE o.matter_id=?1 AND o.exists_now=1 AND o.availability_state='local'
               AND o.document_version_id IS NOT NULL
               AND (o.byte_size IS NOT dv.byte_size OR o.observed_mtime IS NOT dv.observed_mtime)",
        )?;
        let rows = stmt.query_map([matter_id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    })?;

    let mut count = 0usize;
    for (occurrence_id, display_path, document_id, indexed_size) in pending {
        let path = PathBuf::from(&display_path);
        let before = std::fs::metadata(&path)?;
        if before.len() as i64 != indexed_size { continue; }

        let sha = hash_file(&path)?;
        let after = std::fs::metadata(&path)?;
        if after.len() != before.len() || after.modified().ok() != before.modified().ok() { continue; }

        db.write(|conn| {
            let tx = conn.transaction()?;
            let old_version_id: String = tx.query_row(
                "SELECT document_version_id FROM file_occurrences WHERE id=?1",
                [&occurrence_id], |r| r.get(0),
            )?;
            let old_sha: String = tx.query_row(
                "SELECT content_sha256 FROM document_versions WHERE id=?1",
                [&old_version_id], |r| r.get(0),
            )?;

            if old_sha == sha {
                // Metadata-only touch (re-saved with identical bytes): content identity
                // is genuinely unchanged, so just sync the version's recorded metadata
                // rather than spawning a pointless new version.
                tx.execute(
                    "UPDATE document_versions SET
                        byte_size=(SELECT byte_size FROM file_occurrences WHERE id=?1),
                        observed_mtime=(SELECT observed_mtime FROM file_occurrences WHERE id=?1)
                     WHERE id=?2",
                    params![occurrence_id, old_version_id],
                )?;
                tx.commit()?;
                return Ok(());
            }

            let new_version_id = Uuid::new_v4().to_string();
            let now = Utc::now().to_rfc3339();
            tx.execute(
                "INSERT INTO document_versions(
                    id,document_id,matter_id,content_sha256,byte_size,observed_mtime,created_at
                 )
                 SELECT ?1,?2,matter_id,?3,byte_size,observed_mtime,?4
                 FROM file_occurrences WHERE id=?5",
                params![new_version_id, document_id, sha, now, occurrence_id],
            )?;
            tx.execute("UPDATE document_versions SET stale=1 WHERE id=?1", [&old_version_id])?;
            tx.execute(
                "UPDATE file_occurrences SET document_version_id=?1 WHERE id=?2",
                params![new_version_id, occurrence_id],
            )?;
            tx.execute(
                "UPDATE verified_facts SET stale=1 WHERE id IN (
                    SELECT verified_fact_id FROM verified_fact_sources WHERE document_version_id=?1
                 )",
                [&old_version_id],
            )?;
            tx.commit()?;
            Ok(())
        })?;
        count += 1;
    }
    Ok(count)
}
