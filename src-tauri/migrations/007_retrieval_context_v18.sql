-- Phase B, milestone B5a: Focused AI Retrieval. Additive to 001-006, same
-- discipline: every statement here is re-run via execute_batch on every
-- DbState::open() call, so everything is CREATE ... IF NOT EXISTS / idempotent.
--
-- Adds a local FTS5 index over document_pages.normalized_text, replacing
-- ai.rs::plan_context's flat "80 most recent non-stale pages" query with real
-- relevance retrieval. No embeddings, no external service - FTS5 ships inside the
-- SQLite build this project already bundles (SQLITE_ENABLE_FTS5 confirmed present
-- at runtime by db::tests::fts5_is_available_in_this_sqlite_build, which runs on
-- every cargo test including real Windows CI - not assumed, verified).
--
-- Kept in sync with zero changes to extraction.rs, via three triggers on the real
-- document_pages table. The delete trigger alone correctly handles every removal
-- path - a direct delete, or a cascade via document_versions/documents/matters'
-- ON DELETE CASCADE (cascaded deletes do fire a child table's own triggers,
-- verified empirically during the B4 hardening pass) - with no Rust-side cleanup
-- and no guard table needed, since document_pages itself carries no immutability
-- trigger to conflict with (unlike the B4 ledger tables).
--
-- remove_diacritics 2 is documented by SQLite as targeting Latin-script diacritics -
-- it does not guarantee Hebrew nikud normalization, and unicode61 has no Hebrew
-- stemmer. Still the right default (real Unicode-aware word-boundary tokenization),
-- just not oversold as Hebrew-aware.
CREATE VIRTUAL TABLE IF NOT EXISTS document_pages_fts USING fts5(
 page_id UNINDEXED, matter_id UNINDEXED, normalized_text,
 tokenize='unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS trg_document_pages_fts_insert AFTER INSERT ON document_pages
BEGIN
 INSERT INTO document_pages_fts(page_id,matter_id,normalized_text)
 VALUES(NEW.id,NEW.matter_id,NEW.normalized_text);
END;
CREATE TRIGGER IF NOT EXISTS trg_document_pages_fts_update AFTER UPDATE OF normalized_text ON document_pages
BEGIN
 UPDATE document_pages_fts SET normalized_text=NEW.normalized_text WHERE page_id=NEW.id;
END;
CREATE TRIGGER IF NOT EXISTS trg_document_pages_fts_delete AFTER DELETE ON document_pages
BEGIN
 DELETE FROM document_pages_fts WHERE page_id=OLD.id;
END;

-- Backfill every document_pages row that already existed before this migration ran
-- (the triggers above only cover changes from this point onward). Idempotent AND
-- incremental: only ever inserts rows genuinely still missing from the FTS index,
-- so after the first launch this is a cheap no-op scan, never a full rebuild, safe
-- to re-run on every DbState::open() like every other statement in this file.
INSERT INTO document_pages_fts(page_id,matter_id,normalized_text)
SELECT dp.id,dp.matter_id,dp.normalized_text FROM document_pages dp
WHERE NOT EXISTS(SELECT 1 FROM document_pages_fts f WHERE f.page_id=dp.id);

PRAGMA user_version = 18;
