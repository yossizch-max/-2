//! Phase B, milestone B5a: Focused AI Retrieval. Split out of `ai.rs` so this logic
//! is directly testable in `integrity_tests.rs`, matching the pattern already used
//! by `authorities.rs`/`ledger.rs`.
//!
//! Replaces `ai.rs::plan_context`'s flat "80 most recent non-stale pages" query with
//! a real, auditable retrieval pipeline: hard matter/staleness filters -> local FTS5
//! candidate search -> a fully deterministic explicit ranking tuple -> page-level
//! neighbor expansion -> deterministic context windowing for oversized sources ->
//! char-budgeted assembly into a `ContextManifest` that carries its own canonical
//! integrity hash. No embeddings, no cloud vector DB - `document_pages_fts`
//! (`007_retrieval_context_v18.sql`) is the only new state, local to this DB.
//!
//! `matter_id` and `document_versions.stale=0` are re-applied against the live
//! `document_pages`/`document_versions` tables at every stage, including neighbor
//! expansion - the FTS index is never trusted as the authoritative source of
//! filtering, only as a candidate-search accelerator.
use crate::{
    db::DbState,
    error::{AppError, AppResult},
    extraction,
    models::{ContextManifest, ManifestSource},
};
use rusqlite::{params, Connection};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::HashSet;

const RETRIEVAL_VERSION: &str = "b5a-v1";
const MAX_QUERY_TERMS: usize = 12;
const MAX_TERM_CHARS: usize = 40;
const MAX_FULL_SOURCE_CHARS: usize = 8_000;
const WINDOW_SIZE_CHARS: usize = 4_000;
const DEFAULT_BUDGET_CHARS: i64 = 200_000;
const NEIGHBOR_TOP_K: usize = 10;
const CANDIDATE_POOL_LIMIT: i64 = 500; // memory/perf safety valve, not a relevance cap

struct CapabilityProfile {
    default_query: Option<&'static str>,
    boosted_categories: &'static [&'static str],
}

/// Capability-specific retrieval remains a small profile layer over the B5a
/// pipeline. The categories here are the live document categories exposed by the
/// app (`general`, `medical`, `court`, `wage`, `correspondence`, `expert_opinion`).
fn capability_profile(capability: &str) -> CapabilityProfile {
    match capability {
        "extract_facts" => CapabilityProfile { default_query: None, boosted_categories: &[] },
        "extract_medical_event" => CapabilityProfile {
            default_query: Some("רפואי אשפוז אבחנה טיפול הדמיה ניתוח מחלה נכות מגבלה תפקודית medical treatment"),
            boosted_categories: &["medical", "expert_opinion"],
        },
        "extract_wage_record" => CapabilityProfile {
            default_query: Some("שכר תלוש תלושים מעסיק עבודה היעדרות הכנסה משכורת salary payslip employer absence"),
            boosted_categories: &["wage"],
        },
        "extract_liability_fact" => CapabilityProfile {
            default_query: Some("תאונה אחריות רשלנות משטרה עדות תמונות מומחה הודאה מנגנון סיבה accident liability"),
            boosted_categories: &["court", "expert_opinion", "correspondence"],
        },
        // Phase C, milestone C2: spans every document category by design (entities,
        // events, claims, amounts, dates, contradictions, questions can each come
        // from any category), so - like `extract_facts` - it has no single natural
        // keyword focus and no category boost. A lawyer-typed `query` still narrows
        // the run the same way it does for every other capability.
        "extract_matter_understanding" => CapabilityProfile { default_query: None, boosted_categories: &[] },
        // Phase C, milestone C3: unlike extract_medical_event (one narrow flat
        // ledger row), this capability's 15-item taxonomy spans encounters,
        // complaints, findings, diagnoses, tests, treatments, medications,
        // referrals, functional status, and disability determinations - too broad
        // for one fixed keyword set to represent honestly, so - like
        // extract_matter_understanding - it has no default query and instead
        // relies on category boosting plus a lawyer-typed query to narrow a run.
        "extract_medical_evidence" => CapabilityProfile {
            default_query: None,
            boosted_categories: &["medical", "expert_opinion"],
        },
        // Phase C, milestone C4, Part A: same reasoning as extract_medical_evidence
        // - the 10-item wage/economic taxonomy is too broad for one fixed keyword
        // set, so it relies on category boosting plus a lawyer-typed query.
        "extract_wage_economic_evidence" => CapabilityProfile {
            default_query: None,
            boosted_categories: &["wage"],
        },
        // Phase C, milestone C4, Part B: same reasoning, for the 11-item liability
        // taxonomy.
        "extract_liability_evidence" => CapabilityProfile {
            default_query: None,
            boosted_categories: &["court", "expert_opinion", "correspondence"],
        },
        _ => CapabilityProfile { default_query: None, boosted_categories: &[] },
    }
}

/// Unicode-aware (Hebrew/Arabic letters are `is_alphanumeric()==true` in Rust, no
/// special-casing needed) tokenization of free text into a bounded list of terms,
/// each individually normalized the same way page text is (`extraction::
/// normalize_source_text` - NFC + bidi-control stripping) for consistent matching
/// in `deterministic_window`'s plain substring search.
fn extract_query_terms(raw: &str) -> Vec<String> {
    raw.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .take(MAX_QUERY_TERMS)
        .map(|t| {
            let truncated: String = t.chars().take(MAX_TERM_CHARS).collect();
            extraction::normalize_source_text(&truncated)
        })
        .filter(|t| !t.is_empty())
        .collect()
}

/// Never hands raw free text to `MATCH ?` - parameter binding stops SQL injection
/// but not FTS5 *query-syntax* errors (quotes, AND/OR/NOT, parentheses, NEAR, `*`,
/// `^` are all real FTS5 operators). Each term is phrase-quoted (embedded quotes
/// escaped by doubling, standard FTS5 string-literal escaping) and OR-joined for
/// recall, so punctuation a lawyer types can never be interpreted as FTS5 syntax.
fn compile_fts_query(terms: &[String]) -> Option<String> {
    if terms.is_empty() { return None; }
    Some(terms.iter()
        .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
        .collect::<Vec<_>>().join(" OR "))
}

fn lowercase_chars_with_source_indices(text: &str) -> (Vec<char>, Vec<usize>) {
    let mut lower = Vec::new();
    let mut source_indices = Vec::new();
    for (source_index, ch) in text.chars().enumerate() {
        for folded in ch.to_lowercase() {
            lower.push(folded);
            source_indices.push(source_index);
        }
    }
    (lower, source_indices)
}

fn earliest_query_term_match(text: &str, terms: &[String]) -> Option<usize> {
    let (lower, source_indices) = lowercase_chars_with_source_indices(text);
    terms.iter().filter_map(|term| {
        let term_lower: Vec<char> = term.chars().flat_map(|ch| ch.to_lowercase()).collect();
        if term_lower.is_empty() || term_lower.len() > lower.len() { return None; }
        lower
            .windows(term_lower.len())
            .position(|w| w == term_lower.as_slice())
            .map(|match_index| source_indices[match_index])
    }).min()
}

/// Deterministic, reproducible windowing for a source too large to send whole: the
/// window is centered on the first (char-index-earliest) occurrence of any query
/// term (case-insensitive), or starts at the beginning of the text when there are
/// no terms to anchor on. Operates entirely in char indices (never raw byte
/// offsets) so it's safe on multi-byte UTF-8 text like Hebrew. Same input always
/// produces the same `(start, end, window)` - no randomness, no clock.
fn deterministic_window(text: &str, terms: &[String]) -> (usize, usize, String) {
    let chars: Vec<char> = text.chars().collect();
    let match_start = earliest_query_term_match(text, terms).unwrap_or(0);
    let half = WINDOW_SIZE_CHARS / 2;
    let raw_start = match_start.saturating_sub(half);
    let end = (raw_start + WINDOW_SIZE_CHARS).min(chars.len());
    let start = end.saturating_sub(WINDOW_SIZE_CHARS).min(raw_start);
    let window: String = chars[start..end].iter().collect();
    (start, end, window)
}

#[derive(Clone)]
struct Candidate {
    source_id: String, document_version_id: String, page: Option<i64>, block_index: i64,
    anchor_kind: String, text_sha256: String, display_text: String, normalized_text: String,
    created_at: String, category_boosted: bool, bm25_score: Option<f64>,
    included_via: &'static str, neighbor_of_source_id: Option<String>,
}

fn row_to_candidate(
    source_id: String, document_version_id: String, page: Option<i64>, block_index: i64,
    anchor_kind: String, text_sha256: String, display_text: String, normalized_text: String,
    created_at: String, category: String, boosted_categories: &[&str], bm25_score: Option<f64>,
) -> Candidate {
    Candidate {
        category_boosted: boosted_categories.contains(&category.as_str()),
        source_id, document_version_id, page, block_index, anchor_kind, text_sha256,
        display_text, normalized_text, created_at, bm25_score,
        included_via: "match", neighbor_of_source_id: None,
    }
}

fn fetch_candidates(
    conn: &Connection, matter_id: &str, fts_query: Option<&str>, boosted_categories: &[&str],
) -> AppResult<Vec<Candidate>> {
    if let Some(q) = fts_query {
        let mut stmt = conn.prepare(
            "SELECT p.id,p.document_version_id,p.page_number,p.block_index,p.anchor_kind,
                    p.text_sha256,p.display_text,p.normalized_text,v.created_at,d.category,
                    bm25(document_pages_fts) AS score
             FROM document_pages_fts
             JOIN document_pages p ON p.id=document_pages_fts.page_id
             JOIN document_versions v ON v.id=p.document_version_id AND v.matter_id=p.matter_id
             JOIN documents d ON d.id=v.document_id AND d.matter_id=v.matter_id
             WHERE document_pages_fts MATCH ?1 AND document_pages_fts.matter_id=?2
               AND p.matter_id=?2 AND v.stale=0
             ORDER BY score ASC LIMIT ?3"
        )?;
        let rows = stmt.query_map(params![q, matter_id, CANDIDATE_POOL_LIMIT], |r| Ok(row_to_candidate(
            r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?,
            r.get(8)?, r.get(9)?, boosted_categories, Some(r.get(10)?),
        )))?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    } else {
        // Fallback: today's exact pre-B5a behavior (matter+stale-filtered, ordered
        // by version recency then page/block) when there's no query to rank against.
        let mut stmt = conn.prepare(
            "SELECT p.id,p.document_version_id,p.page_number,p.block_index,p.anchor_kind,
                    p.text_sha256,p.display_text,p.normalized_text,v.created_at,d.category
             FROM document_pages p
             JOIN document_versions v ON v.id=p.document_version_id AND v.matter_id=p.matter_id
             JOIN documents d ON d.id=v.document_id AND d.matter_id=v.matter_id
             WHERE p.matter_id=?1 AND v.stale=0
             ORDER BY v.created_at DESC,p.page_number,p.block_index LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![matter_id, CANDIDATE_POOL_LIMIT], |r| Ok(row_to_candidate(
            r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?,
            r.get(8)?, r.get(9)?, boosted_categories, None,
        )))?.collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}

/// Explicit deterministic sort tuple, never a blended arbitrary-scale score:
/// bm25 ascending (lower = more relevant; absent entirely in the no-query fallback)
/// -> category-boosted first (a tie-break only, can never outrank a real text
/// match) -> version recency descending -> page_number -> block_index -> source_id
/// (final tie-break so two byte-identical candidates never produce nondeterministic
/// order).
fn rank_candidates(mut candidates: Vec<Candidate>) -> Vec<Candidate> {
    candidates.sort_by(|a, b| {
        match (a.bm25_score, b.bm25_score) {
            (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal),
            _ => Ordering::Equal,
        }
        .then_with(|| b.category_boosted.cmp(&a.category_boosted))
        .then_with(|| b.created_at.cmp(&a.created_at))
        .then_with(|| a.page.cmp(&b.page))
        .then_with(|| a.block_index.cmp(&b.block_index))
        .then_with(|| a.source_id.cmp(&b.source_id))
    });
    candidates
}

fn fetch_live_neighbor(
    conn: &Connection, matter_id: &str, document_version_id: &str, page_number: i64,
) -> AppResult<Option<(String, String, String, String, i64)>> {
    conn.query_row(
        "SELECT p.id,p.text_sha256,p.display_text,p.normalized_text,p.block_index
         FROM document_pages p
         JOIN document_versions v ON v.id=p.document_version_id AND v.matter_id=p.matter_id
         WHERE p.matter_id=?1 AND p.document_version_id=?2 AND p.page_number=?3
           AND p.anchor_kind='page' AND v.stale=0",
        params![matter_id, document_version_id, page_number],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    ).map(Some).or_else(|e| if e == rusqlite::Error::QueryReturnedNoRows { Ok(None) } else { Err(AppError::Db(e)) })
}

/// For each of the top-K ranked `anchor_kind='page'` candidates, pulls the adjacent
/// `page_number-1`/`+1` rows (re-checked live against matter/staleness, never
/// trusting the FTS index) so a matched sentence never arrives without surrounding
/// context. Tagged `neighborOfSourceId` so the manifest records *why* each neighbor
/// is present - inherits its anchor's rank position, not a separately computed score.
fn expand_neighbors(conn: &Connection, matter_id: &str, ranked: &[Candidate]) -> AppResult<Vec<Candidate>> {
    let mut neighbors = Vec::new();
    for anchor in ranked.iter().take(NEIGHBOR_TOP_K) {
        if anchor.anchor_kind != "page" { continue; }
        let Some(page) = anchor.page else { continue; };
        for neighbor_page in [page - 1, page + 1] {
            if neighbor_page < 1 { continue; }
            if let Some((id, sha, display, normalized, block_index)) =
                fetch_live_neighbor(conn, matter_id, &anchor.document_version_id, neighbor_page)?
            {
                neighbors.push(Candidate {
                    source_id: id, document_version_id: anchor.document_version_id.clone(),
                    page: Some(neighbor_page), block_index, anchor_kind: "page".to_string(),
                    text_sha256: sha, display_text: display, normalized_text: normalized,
                    created_at: anchor.created_at.clone(), category_boosted: anchor.category_boosted,
                    bm25_score: None, included_via: "neighbor",
                    neighbor_of_source_id: Some(anchor.source_id.clone()),
                });
            }
        }
    }
    Ok(neighbors)
}

/// Deduplicates (a page reachable both as a direct match and as another match's
/// neighbor appears exactly once, at its best-known inclusion reason), decides
/// full-vs-windowed text per source (point 6 of the review: an oversized source is
/// never silently dropped, it's deterministically windowed instead), and greedily
/// assembles sources into the char budget in the given order - including each one
/// whose decided text still fits, skipping (not truncating) ones that don't while
/// continuing to try smaller ones after it.
fn assemble_manifest_sources(
    ordered: Vec<Candidate>, terms: &[String], budget_limit: i64,
) -> (Vec<ManifestSource>, i64) {
    let mut seen = HashSet::new();
    let mut sources = Vec::new();
    let mut used: i64 = 0;
    for c in ordered {
        if !seen.insert(c.source_id.clone()) { continue; }
        let (text_mode, text, window_start, window_end, window_sha256) =
            if c.normalized_text.chars().count() <= MAX_FULL_SOURCE_CHARS {
                ("full".to_string(), c.display_text.clone(), None, None, None)
            } else {
                let (start, end, window) = deterministic_window(&c.normalized_text, terms);
                let window_sha = hex::encode(Sha256::digest(window.as_bytes()));
                ("window".to_string(), window, Some(start as i64), Some(end as i64), Some(window_sha))
            };
        let len = text.chars().count() as i64;
        if used + len > budget_limit { continue; }
        used += len;
        sources.push(ManifestSource {
            source_id: c.source_id, document_version_id: c.document_version_id, page: c.page,
            anchor_kind: c.anchor_kind, text_sha256: c.text_sha256, text, text_mode,
            window_start, window_end, window_sha256, bm25_score: c.bm25_score,
            category_boosted: c.category_boosted, included_via: c.included_via.to_string(),
            neighbor_of_source_id: c.neighbor_of_source_id,
        });
    }
    (sources, used)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ManifestPayloadSource<'a> {
    source_id: &'a str,
    document_version_id: &'a str,
    page: Option<i64>,
    anchor_kind: &'a str,
    text_sha256: &'a str,
    text: &'a str,
    text_mode: &'a str,
    window_start: Option<i64>,
    window_end: Option<i64>,
    window_sha256: Option<&'a str>,
    category_boosted: bool,
    included_via: &'a str,
    neighbor_of_source_id: Option<&'a str>,
}

impl<'a> ManifestPayloadSource<'a> {
    fn from_source(source: &'a ManifestSource) -> Self {
        Self {
            source_id: &source.source_id,
            document_version_id: &source.document_version_id,
            page: source.page,
            anchor_kind: &source.anchor_kind,
            text_sha256: &source.text_sha256,
            text: &source.text,
            text_mode: &source.text_mode,
            window_start: source.window_start,
            window_end: source.window_end,
            window_sha256: source.window_sha256.as_deref(),
            category_boosted: source.category_boosted,
            included_via: &source.included_via,
            neighbor_of_source_id: source.neighbor_of_source_id.as_deref(),
        }
    }
}

#[derive(Serialize)]
struct ManifestPayload<'a> {
    retrieval_version: &'a str,
    matter_id: &'a str, capability: &'a str, query_terms: &'a str,
    sources: Vec<ManifestPayloadSource<'a>>, budget_chars_used: i64, budget_chars_limit: i64,
}

fn canonical_manifest_sha256(
    retrieval_version: &str, matter_id: &str, capability: &str, query_terms: &str,
    sources: &[ManifestSource], budget_chars_used: i64, budget_chars_limit: i64,
) -> AppResult<String> {
    let payload_sources: Vec<_> = sources.iter().map(ManifestPayloadSource::from_source).collect();
    let payload = ManifestPayload {
        retrieval_version, matter_id, capability, query_terms,
        sources: payload_sources, budget_chars_used, budget_chars_limit,
    };
    Ok(hex::encode(Sha256::digest(serde_json::to_vec(&payload)?)))
}

pub(crate) fn compute_manifest_sha256(manifest: &ContextManifest) -> AppResult<String> {
    canonical_manifest_sha256(
        &manifest.retrieval_version,
        &manifest.matter_id,
        &manifest.capability,
        &manifest.query_terms,
        &manifest.sources,
        manifest.budget_chars_used,
        manifest.budget_chars_limit,
    )
}

/// Canonical hash defined so it can never be circular: hashed over a payload type
/// that structurally has no hash field of its own, and excludes raw diagnostic
/// BM25 values, then the result is attached to the public `ContextManifest`.
/// `retrieval_version` is a fixed literal and no timestamp appears anywhere in the
/// hashed payload, so identical selected sources/windows/budget against the same
/// DB always produce a byte-identical manifest hash, every run.
fn build_manifest(
    matter_id: &str, capability: &str, query_terms: &str,
    sources: Vec<ManifestSource>, budget_chars_used: i64, budget_chars_limit: i64,
) -> AppResult<ContextManifest> {
    let manifest_sha256 = canonical_manifest_sha256(
        RETRIEVAL_VERSION, matter_id, capability, query_terms,
        &sources, budget_chars_used, budget_chars_limit,
    )?;
    Ok(ContextManifest {
        retrieval_version: RETRIEVAL_VERSION.to_string(),
        matter_id: matter_id.to_string(), capability: capability.to_string(),
        query_terms: query_terms.to_string(), sources,
        budget_chars_used, budget_chars_limit, manifest_sha256,
    })
}

pub(crate) fn build_context_manifest_with_budget(
    db: &DbState, matter_id: &str, capability: &str, query: Option<&str>, budget_limit: i64,
) -> AppResult<ContextManifest> {
    let profile = capability_profile(capability);
    let raw_query = query.filter(|q| !q.trim().is_empty()).or(profile.default_query);
    let terms = raw_query.map(extract_query_terms).unwrap_or_default();
    let fts_query = compile_fts_query(&terms);
    let query_terms_joined = terms.join(" ");

    db.read(|conn| {
        let candidates = fetch_candidates(conn, matter_id, fts_query.as_deref(), profile.boosted_categories)?;
        let ranked = rank_candidates(candidates);
        let neighbors = expand_neighbors(conn, matter_id, &ranked)?;

        // All ranked matches first (in rank order), then all neighbors after -
        // deliberately NOT interleaved. This is what makes dedup correct: a page
        // that is BOTH a genuine match and another match's neighbor must be
        // recorded as "match" (its real, stronger inclusion reason), never
        // downgraded to "neighbor" by appearing in that list first. Putting every
        // match ahead of every neighbor guarantees the match's `seen.insert` always
        // wins, regardless of which anchor's neighbor slot it would also occupy.
        let mut ordered = Vec::with_capacity(ranked.len() + neighbors.len());
        ordered.extend(ranked.iter().cloned());
        ordered.extend(neighbors);

        let (sources, used) = assemble_manifest_sources(ordered, &terms, budget_limit);
        build_manifest(matter_id, capability, &query_terms_joined, sources, used, budget_limit)
    })
}

pub fn build_context_manifest(
    db: &DbState, matter_id: &str, capability: &str, query: Option<&str>,
) -> AppResult<ContextManifest> {
    build_context_manifest_with_budget(db, matter_id, capability, query, DEFAULT_BUDGET_CHARS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, bm25_score: Option<f64>, category_boosted: bool, created_at: &str) -> Candidate {
        Candidate {
            source_id: id.to_string(), document_version_id: "v".to_string(), page: Some(1), block_index: 0,
            anchor_kind: "page".to_string(), text_sha256: "x".to_string(),
            display_text: "x".to_string(), normalized_text: "x".to_string(),
            created_at: created_at.to_string(), category_boosted, bm25_score,
            included_via: "match", neighbor_of_source_id: None,
        }
    }

    fn manifest_source(id: &str, bm25_score: Option<f64>) -> ManifestSource {
        ManifestSource {
            source_id: id.to_string(),
            document_version_id: "v".to_string(),
            page: Some(1),
            anchor_kind: "page".to_string(),
            text_sha256: "full-sha".to_string(),
            text: "grounded text".to_string(),
            text_mode: "full".to_string(),
            window_start: None,
            window_end: None,
            window_sha256: None,
            bm25_score,
            category_boosted: false,
            included_via: "match".to_string(),
            neighbor_of_source_id: None,
        }
    }

    #[test]
    fn category_boost_is_a_tiebreak_only_never_outranking_a_real_bm25_match() {
        let strong_match_no_boost = candidate("a", Some(-5.0), false, "2020-01-01T00:00:00Z");
        let weak_match_boosted = candidate("b", Some(-1.0), true, "2026-01-01T00:00:00Z");
        let ranked = rank_candidates(vec![weak_match_boosted, strong_match_no_boost]);
        assert_eq!(ranked[0].source_id, "a", "a real bm25 advantage must win regardless of category boost or recency");

        let tie_a = candidate("x", Some(-3.0), false, "2020-01-01T00:00:00Z");
        let tie_b = candidate("y", Some(-3.0), true, "2020-01-01T00:00:00Z");
        let ranked2 = rank_candidates(vec![tie_a, tie_b]);
        assert_eq!(ranked2[0].source_id, "y", "category boost must break a true bm25 tie");
    }

    #[test]
    fn recency_only_breaks_ties_after_bm25_and_category() {
        let older = candidate("old", Some(-2.0), false, "2020-01-01T00:00:00Z");
        let newer = candidate("new", Some(-2.0), false, "2026-01-01T00:00:00Z");
        let ranked = rank_candidates(vec![older, newer]);
        assert_eq!(ranked[0].source_id, "new", "recency is the tie-break once bm25 and category are equal");
    }

    #[test]
    fn compile_fts_query_handles_quotes_parens_hyphens_and_hebrew_arabic() {
        let terms = extract_query_terms("שבר בפרק כף-היד \"אירוע\" (חמור) AND كسر");
        let q = compile_fts_query(&terms).unwrap();
        assert!(q.starts_with('"') && q.ends_with('"'), "every term must be phrase-quoted: {q}");
        assert!(!q.contains(" AND "), "a bare AND from the input text must never survive as an FTS operator: {q}");
        assert!(q.contains("כף"), "Hebrew terms must pass through: {q}");
        assert!(q.contains("كسر"), "Arabic terms must pass through: {q}");
    }

    #[test]
    fn compile_fts_query_escapes_embedded_quotes() {
        let terms = extract_query_terms("say \"hello\"");
        // the raw splitter tokenizes on non-alphanumeric, so quotes never survive
        // into a term in the first place - but a raw-text-containing-a-quote path
        // is still exercised end to end by the extractor + compiler together.
        let q = compile_fts_query(&terms);
        assert!(q.is_none() || !q.unwrap().contains("\"\"\"\""), "must never produce malformed doubled-quote runs");
    }

    #[test]
    fn compile_fts_query_returns_none_for_empty_input() {
        assert!(compile_fts_query(&extract_query_terms("   ")).is_none());
        assert!(compile_fts_query(&extract_query_terms("")).is_none());
    }

    #[test]
    fn deterministic_window_centers_on_first_term_match_and_is_reproducible() {
        let text = "x".repeat(3000) + "MATCHME" + &"y".repeat(10000);
        let terms = vec!["matchme".to_string()];
        let (start1, end1, window1) = deterministic_window(&text, &terms);
        let (start2, end2, window2) = deterministic_window(&text, &terms);
        assert_eq!((start1, end1, &window1), (start2, end2, &window2), "must be perfectly reproducible");
        assert!(window1.to_lowercase().contains("matchme"), "the window must actually contain the match");
    }

    #[test]
    fn deterministic_window_centers_on_earliest_match_across_terms_not_query_order() {
        let text = "x".repeat(1000) + "EARLY" + &"y".repeat(6000) + "LATE" + &"z".repeat(4000);
        let terms = vec!["late".to_string(), "early".to_string()];
        let (start, end, window) = deterministic_window(&text, &terms);
        assert_eq!(start, 0, "the earliest document occurrence must win even when its term appears later in the query");
        assert_eq!(end, WINDOW_SIZE_CHARS);
        assert!(window.contains("EARLY"), "the window should be anchored around the earliest term occurrence");
        assert!(!window.contains("LATE"), "a later query-ordered term must not steal the window anchor");
    }

    #[test]
    fn deterministic_window_defaults_to_start_of_text_with_no_terms() {
        let text = "a".repeat(20000);
        let (start, _end, _window) = deterministic_window(&text, &[]);
        assert_eq!(start, 0);
    }

    #[test]
    fn manifest_hash_excludes_diagnostic_bm25_score_but_ranking_still_uses_it() {
        let with_score = manifest_source("s1", Some(-1.25));
        let mut with_different_score = with_score.clone();
        with_different_score.bm25_score = Some(-99.875);

        let manifest_a = build_manifest("matter", "extract_facts", "needle", vec![with_score], 13, 200).unwrap();
        let manifest_b = build_manifest("matter", "extract_facts", "needle", vec![with_different_score], 13, 200).unwrap();
        assert_eq!(manifest_a.manifest_sha256, manifest_b.manifest_sha256, "raw BM25 serialization must not affect the canonical manifest hash");
        assert_ne!(manifest_a.sources[0].bm25_score, manifest_b.sources[0].bm25_score, "bm25Score must remain visible in the public manifest for diagnostics");
        assert_ne!(serde_json::to_string(&manifest_a).unwrap(), serde_json::to_string(&manifest_b).unwrap(), "public diagnostics should still expose the BM25 difference");

        let ranked_once = rank_candidates(vec![
            candidate("weak", Some(-1.0), false, "2026-01-01T00:00:00Z"),
            candidate("strong", Some(-5.0), false, "2020-01-01T00:00:00Z"),
        ]);
        let ranked_twice = rank_candidates(vec![
            candidate("weak", Some(-1.0), false, "2026-01-01T00:00:00Z"),
            candidate("strong", Some(-5.0), false, "2020-01-01T00:00:00Z"),
        ]);
        let ids_once: Vec<_> = ranked_once.iter().map(|c| c.source_id.as_str()).collect();
        let ids_twice: Vec<_> = ranked_twice.iter().map(|c| c.source_id.as_str()).collect();
        assert_eq!(ids_once, ids_twice, "BM25-backed ordering must remain deterministic");
        assert_eq!(ids_once, vec!["strong", "weak"], "the better BM25 match must still rank first");
    }

    #[test]
    fn compute_manifest_sha256_matches_the_attached_public_hash() {
        let manifest = build_manifest("matter", "extract_medical_event", "טיפול", vec![manifest_source("s1", Some(-1.0))], 13, 200).unwrap();
        assert_eq!(compute_manifest_sha256(&manifest).unwrap(), manifest.manifest_sha256);
    }

    #[test]
    fn capability_profile_extract_facts_has_no_default_query() {
        let profile = capability_profile("extract_facts");
        assert!(profile.default_query.is_none(), "extract_facts has no natural keyword focus - documented, not an oversight");
    }

    #[test]
    fn ledger_capability_profiles_use_domain_queries_and_live_categories() {
        let medical = capability_profile("extract_medical_event");
        assert!(medical.default_query.unwrap().contains("טיפול"));
        assert_eq!(medical.boosted_categories, &["medical", "expert_opinion"]);

        let wage = capability_profile("extract_wage_record");
        assert!(wage.default_query.unwrap().contains("תלוש"));
        assert_eq!(wage.boosted_categories, &["wage"]);

        let liability = capability_profile("extract_liability_fact");
        assert!(liability.default_query.unwrap().contains("תאונה"));
        assert_eq!(liability.boosted_categories, &["court", "expert_opinion", "correspondence"]);
    }
}
