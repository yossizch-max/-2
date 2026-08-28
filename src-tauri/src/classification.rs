//! Phase C, milestone C1: deterministic local document classification. Suggests
//! `documents.category` from filename/extracted-text signals after extraction - it
//! never asserts a legal fact, a medical finding, or anything the existing AI
//! proposal -> human review lifecycle is responsible for (see `ai.rs`/`ledger.rs`).
//! Classification only improves organization and retrieval; it must never be used to
//! create a VerifiedFact, a ledger entry, a legal deadline, or any liability/damage
//! conclusion.
//!
//! Deterministic by construction: a fixed-order rule table, an exact substring test
//! per term (no fuzzy matching, no randomness, no wall-clock input), so identical
//! `(file_name, extracted_text)` always produces an identical `ClassificationResult` -
//! directly required by `intake::process_matter_documents`'s "same input -> same
//! output" contract and tested in `classification.rs`'s own test module.
use crate::models::ClassificationResult;

pub const CLASSIFIER_VERSION: &str = "c1-v1";

struct Rule { category: &'static str, terms: &'static [&'static str] }

/// Order is the tie-break: when two categories match the same (highest) number of
/// terms, the one listed first here wins - fixed array order, never hash-map
/// iteration order, so ties resolve identically on every run.
const RULES: &[Rule] = &[
    Rule { category: "medical", terms: &["קופת חולים", "סיכום ביקור", "מיון", "אשפוז", "אבחנה", "MRI", "CT", "רופא", "טיפול"] },
    Rule { category: "wage", terms: &["תלוש שכר", "שכר ברוטו", "מעסיק", "106", "משכורת"] },
    Rule { category: "court", terms: &["בית משפט", "החלטה", "כתב תביעה", "כתב הגנה", "בקשה", "תגובה"] },
    Rule { category: "expert_opinion", terms: &["חוות דעת", "מומחה", "נכות רפואית", "בדיקה רפואית"] },
    Rule { category: "correspondence", terms: &["הנדון", "לכבוד", "מכתב", "דוא\"ל", "חברת ביטוח", "דרישה"] },
];

/// A term matched via the filename or via the extracted text counts identically -
/// both are real signals about what the document is, and a rule's hit count (not
/// which side matched) drives which category wins. Confidence is a simple, honestly
/// bounded function of hit count, not a fabricated precision score - more matching
/// terms is a real (if crude) stronger signal, capped well short of 1.0 since this is
/// a keyword rule, not a lawyer's or a model's judgment.
pub fn classify(file_name: &str, extracted_text: &str) -> ClassificationResult {
    let mut best: Option<(&'static str, Vec<String>)> = None;
    for rule in RULES {
        let hits: Vec<String> = rule.terms.iter()
            .filter(|term| file_name.contains(*term) || extracted_text.contains(*term))
            .map(|term| (*term).to_string())
            .collect();
        if hits.is_empty() { continue; }
        let better = match &best {
            Some((_, existing_hits)) => hits.len() > existing_hits.len(),
            None => true,
        };
        if better { best = Some((rule.category, hits)); }
    }

    match best {
        Some((category, signals)) => ClassificationResult {
            category: category.to_string(),
            confidence: confidence_for_hit_count(signals.len()),
            reason: format!("{} מונח/ים תואמים: {}", signals.len(), signals.join(", ")),
            signals,
            classifier_version: CLASSIFIER_VERSION.to_string(),
        },
        None => ClassificationResult {
            category: "general".to_string(),
            confidence: 0.0,
            reason: "לא נמצאו מונחים תואמים לאף קטגוריה מוכרת".to_string(),
            signals: Vec::new(),
            classifier_version: CLASSIFIER_VERSION.to_string(),
        },
    }
}

fn confidence_for_hit_count(hit_count: usize) -> f64 {
    (0.5 + 0.1 * hit_count as f64).min(0.95)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_obvious_medical_document_is_classified_medical() {
        let r = classify("סיכום ביקור מיון.pdf", "אובחן עם שבר, אבחנה: שבר בכף היד, טיפול שניתן: גבס");
        assert_eq!(r.category, "medical");
        assert!(r.confidence > 0.5);
        assert!(!r.signals.is_empty());
    }

    #[test]
    fn a_wage_slip_is_classified_wage() {
        let r = classify("תלוש.pdf", "תלוש שכר לחודש 01/2026, שכר ברוטו: 12000, מעסיק: חברה בע\"מ");
        assert_eq!(r.category, "wage");
    }

    #[test]
    fn a_court_decision_is_classified_court() {
        let r = classify("decision.pdf", "בבית המשפט השלום, החלטה: הבקשה נדחית, כתב תביעה תוקן");
        assert_eq!(r.category, "court");
    }

    #[test]
    fn an_insurer_correspondence_letter_is_classified_correspondence() {
        let r = classify("letter.pdf", "לכבוד, הנדון: דרישה לתשלום, חברת ביטוח כלל");
        assert_eq!(r.category, "correspondence");
    }

    #[test]
    fn an_expert_opinion_is_classified_expert_opinion() {
        let r = classify("opinion.pdf", "חוות דעת מומחה: נכות רפואית בשיעור 10% לאחר בדיקה רפואית");
        assert_eq!(r.category, "expert_opinion");
    }

    #[test]
    fn an_ambiguous_document_with_no_recognized_terms_falls_back_to_general() {
        let r = classify("scan001.pdf", "תוכן כללי שאינו תואם אף קטגוריה מוכרת בבירור");
        assert_eq!(r.category, "general");
        assert_eq!(r.confidence, 0.0);
        assert!(r.signals.is_empty());
    }

    #[test]
    fn a_filename_only_signal_is_enough_to_classify() {
        let r = classify("תלוש שכר ינואר.pdf", "");
        assert_eq!(r.category, "wage");
    }

    #[test]
    fn a_text_only_signal_is_enough_to_classify() {
        let r = classify("document.pdf", "בית משפט השלום נתן החלטה בעניין הבקשה");
        assert_eq!(r.category, "court");
    }

    #[test]
    fn confidence_increases_with_more_matching_terms_but_never_reaches_one() {
        let weak = classify("x.pdf", "מעסיק");
        let strong = classify("x.pdf", "תלוש שכר שכר ברוטו מעסיק 106 משכורת");
        assert!(strong.confidence > weak.confidence);
        assert!(strong.confidence < 1.0);
    }

    #[test]
    fn identical_input_always_produces_identical_classification() {
        let a = classify("סיכום ביקור.pdf", "אבחנה: שבר, טיפול: גבס, MRI בוצע");
        let b = classify("סיכום ביקור.pdf", "אבחנה: שבר, טיפול: גבס, MRI בוצע");
        assert_eq!(a.category, b.category);
        assert_eq!(a.confidence, b.confidence);
        assert_eq!(a.signals, b.signals);
        assert_eq!(a.reason, b.reason);
    }
}
