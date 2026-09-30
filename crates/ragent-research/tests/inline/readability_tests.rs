//! Inline tests for `readability.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::analysis::AnalysisResult;

#[test]
fn polish_removes_control_chars_and_empty_paragraphs() {
    let mut analysis = AnalysisResult {
        summary: "Summary\x01 text\n\n\n".to_string(),
        findings: vec![
            "Finding\x02 one.".to_string(),
            "\x03   ".to_string(),
            "Finding three.".to_string(),
        ],
        top_implications: vec!["   ".to_string(), "Implication".to_string()],
        cross_references: Vec::new(),
        open_questions: vec!["Question\n\n\n".to_string()],
    };
    let result = polish_analysis(&mut analysis);
    assert!(!result.is_empty());
    assert_eq!(result.control_chars_removed, 2); // two control chars remain in original fields
    assert_eq!(result.empty_paragraphs_removed, 2); // empty finding + empty implication
    assert!(!analysis.summary.contains('\x01'));
    assert_eq!(analysis.findings.len(), 2);
    assert_eq!(analysis.top_implications.len(), 1);
    assert!(!analysis.open_questions[0].contains("\n\n\n"));
}

#[test]
fn audit_passes_well_formed_analysis() {
    let analysis = AnalysisResult {
        summary: "Summary".to_string(),
        findings: vec![format!(
            "**Headline:** H\n\n\
             **Observation:** O\n\n\
             **Analysis:** A\n\n\
             **Cross-reference / Dependencies:** C\n\n\
             **Implication:** I"
        )],
        top_implications: vec!["Imp".to_string()],
        cross_references: Vec::new(),
        open_questions: vec!["Q".to_string()],
    };
    let audit = audit_readability(&analysis);
    assert!(audit.passed);
    assert_eq!(audit.score, 100);
    assert!(audit.issues.is_empty());
}

#[test]
fn audit_fails_empty_analysis() {
    let analysis = AnalysisResult::default();
    let audit = audit_readability(&analysis);
    assert!(!audit.passed);
    assert!(
        audit.score < 70,
        "empty analysis should score below passing threshold: {}",
        audit.score
    );
    assert!(!audit.issues.is_empty());
}

#[test]
fn audit_flags_missing_labels_and_long_paragraphs() {
    let analysis = AnalysisResult {
        summary: "Summary".to_string(),
        findings: vec![
            "**Headline:** H\n\n**Observation:** O".to_string(),
            "a".repeat(1300),
        ],
        top_implications: vec!["Imp".to_string()],
        cross_references: Vec::new(),
        open_questions: Vec::new(),
    };
    let audit = audit_readability(&analysis);
    assert!(!audit.passed);
    assert!(audit.missing_label_count > 0);
    assert!(audit.long_paragraph_count > 0);
}
