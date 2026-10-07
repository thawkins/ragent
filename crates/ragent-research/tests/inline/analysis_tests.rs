//! Inline tests for `analysis.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::parser::{
    mechanical_fallback_findings, parse_analysis_response, parse_bullet_list, parse_numbered_list,
    reorder_findings_by_dependency, truncate_body, validate_citations_and_dates,
};
use super::prompt::SynthesisPromptConfig;
use super::*;
use prompt::{SynthesisPromptBuilder, build_synthesis_prompt};

#[test]
fn parse_analysis_response_extracts_all_sections() {
    let text = "## Executive Summary\n\nThis is the executive summary.\n\n## Findings\n\n1. First finding.\n2. Second finding.\n\n## Top 10 Implications\n\n1. Implication A.\n2. Implication B.\n\n## In-Project Cross-References\n\n* `src/lib.rs` - main entry\n* `src/foo.rs` - helper\n\n## Open Questions\n\n* What about X?\n* How does Y work?\n";
    let result = parse_analysis_response(text);
    assert_eq!(result.summary, "This is the executive summary.");
    assert_eq!(result.findings, vec!["First finding.", "Second finding."]);
    assert_eq!(
        result.top_implications,
        vec!["Implication A.", "Implication B."]
    );
    assert_eq!(result.cross_references.len(), 2);
    assert_eq!(result.cross_references[0].path, "src/lib.rs");
    assert_eq!(result.cross_references[0].relevance, "main entry");
    assert_eq!(
        result.open_questions,
        vec!["What about X?", "How does Y work?"]
    );
}

#[test]
fn reorder_puts_dependencies_first_and_renumbers_references() {
    // Element 0 is the child, element 1 is the root.
    let findings = vec![
                    "**Observation:** child. **Analysis:** a. **Cross-reference / Dependencies:** Builds on Finding 2. **Implication:** i.".into(),
                    "**Observation:** root. **Analysis:** b. **Cross-reference / Dependencies:** No direct dependencies. **Implication:** j.".into(),
                ];
    let ordered = reorder_findings_by_dependency(&findings);
    assert_eq!(ordered.len(), 2);
    // Root must come before its dependant.
    assert!(
        ordered[0].contains("No direct dependencies."),
        "first finding should be the root, got: {}",
        ordered[0]
    );
    assert!(
        ordered[1].contains("Finding 1"),
        "dependant should reference the renumbered root, got: {}",
        ordered[1]
    );
    assert!(
        !ordered[1].contains("Finding 2"),
        "dependant must not retain the old root number"
    );
}
#[test]
fn reorder_preserves_original_order_for_unrelated_findings() {
    let findings = vec![
        "A - no deps".into(),
        "B - no deps".into(),
        "C - no deps".into(),
    ];
    let ordered = reorder_findings_by_dependency(&findings);
    assert_eq!(ordered, findings);
}

#[test]
fn reorder_handles_chains_and_multiple_dependencies() {
    // Original order: leaf (depends on old 2 and 3), mid (depends on old 3), root.
    let findings = vec![
        "Leaf depends on Finding 2 and Finding 3.".into(),
        "Mid depends on Finding 3.".into(),
        "Root has no dependencies.".into(),
    ];
    let ordered = reorder_findings_by_dependency(&findings);
    assert_eq!(ordered[0], "Root has no dependencies.");
    // Mid is now Finding 2 and only depends on the root (Finding 1).
    assert!(
        ordered[1].contains("Finding 1"),
        "mid should reference root, got: {}",
        ordered[1]
    );
    assert!(
        !ordered[1].contains("Finding 3"),
        "mid should not retain old root number"
    );
    // Leaf is now Finding 3 and depends on mid (Finding 2) and root (Finding 1).
    assert!(ordered[2].contains("Finding 1") && ordered[2].contains("Finding 2"));
}
#[test]
fn reorder_breaks_cycles_without_dropping_findings() {
    let findings = vec![
        "A depends on Finding 2.".into(),
        "B depends on Finding 1.".into(),
    ];
    let ordered = reorder_findings_by_dependency(&findings);
    assert_eq!(ordered.len(), 2);
    assert!(
        ordered[0].contains("Finding 2") || ordered[1].contains("Finding 1"),
        "cycle should be broken by keeping original order, got: {ordered:?}"
    );
}

#[test]
fn reorder_is_noop_for_empty_or_single_finding() {
    assert!(reorder_findings_by_dependency(&[]).is_empty());
    let single = vec!["Only finding.".into()];
    assert_eq!(reorder_findings_by_dependency(&single), single);
}

#[test]
fn parse_analysis_response_reorders_findings_by_dependency() {
    let text = "## Findings\n\n1. **Headline:** Two\n\n**Observation:** two. **Analysis:** a. **Cross-reference / Dependencies:** Depends on Finding 2. **Implication:** i.\n2. **Headline:** One\n\n**Observation:** one. **Analysis:** b. **Cross-reference / Dependencies:** No direct dependencies. **Implication:** j.\n";
    let result = parse_analysis_response(text);
    assert_eq!(result.findings.len(), 2);
    assert!(
        result.findings[0].contains("No direct dependencies."),
        "first finding should be the root"
    );
    assert!(
        result.findings[1].contains("Finding 1"),
        "second finding should reference renumbered root"
    );
}

#[test]
fn parse_numbered_list_ignores_wrapped_lines() {
    let body = "1. First\n   continuation\n2. Second\n";
    assert_eq!(
        parse_numbered_list(body),
        vec!["First\ncontinuation", "Second"]
    );
}
#[test]
fn parse_numbered_list_handles_number_on_its_own_line() {
    let body = "1.\n\n**Observation:** obs1\n\n**Analysis:** a1\n\n2.\n\n**Observation:** obs2\n\n**Analysis:** a2\n";
    assert_eq!(
        parse_numbered_list(body),
        vec![
            "**Observation:** obs1\n**Analysis:** a1",
            "**Observation:** obs2\n**Analysis:** a2"
        ]
    );
}
#[test]
fn parse_numbered_list_number_with_content_same_line() {
    let body =
        "1. **Observation:** obs1\n**Analysis:** a1\n2. **Observation:** obs2\n**Analysis:** a2\n";
    assert_eq!(
        parse_numbered_list(body),
        vec![
            "**Observation:** obs1\n**Analysis:** a1",
            "**Observation:** obs2\n**Analysis:** a2"
        ]
    );
}
#[test]
fn parse_bullet_list_handles_dash_and_star() {
    let body = "* one\n- two\n* three\n";
    assert_eq!(parse_bullet_list(body), vec!["one", "two", "three"]);
}

#[test]
fn truncate_body_adds_ellipsis_when_cut() {
    let body = "a".repeat(5000);
    let truncated = truncate_body(&body, 4000);
    assert!(truncated.len() < 5000);
    assert!(truncated.contains("... (truncated for prompt size)"));
}

// -- researchprompt T-011: builder + parser/fallback tests -------------

/// Helper: build a minimal [`SourceBody`] with the given index and
/// optional publication date.
fn src_body(index: usize, published_at: Option<DateTime<Utc>>) -> SourceBody {
    SourceBody {
        index,
        kind: "web".to_string(),
        title: format!("Source {index}"),
        path_or_url: format!("https://example.com/{index}"),
        relevance: String::new(),
        body: format!("Body of source {index}"),
        published_at,
        author: None,
    }
}

#[test]
fn output_format_executive_summary_shortens_instructions() {
    let sources = vec![src_body(1, None)];
    let prompt = SynthesisPromptBuilder::new("topic")
        .sources(&sources)
        .output_format(OutputFormat::ExecutiveSummary)
        .build();
    assert!(prompt.contains("very concise executive summary"));
    assert!(prompt.contains("At most 5 high-level findings"));
}

#[test]
fn output_format_comparison_table_includes_table_request() {
    let sources = vec![src_body(1, None)];
    let prompt = SynthesisPromptBuilder::new("topic")
        .sources(&sources)
        .output_format(OutputFormat::ComparisonTable)
        .build();
    assert!(prompt.contains("## Comparison Table"));
    assert!(prompt.contains("markdown table"));
}

#[test]
fn output_format_source_bibliography_annotated_entries() {
    let sources = vec![src_body(1, None)];
    let prompt = SynthesisPromptBuilder::new("topic")
        .sources(&sources)
        .output_format(OutputFormat::SourceBibliography)
        .build();
    assert!(prompt.contains("annotated bibliography"));
}

#[test]
fn builder_default_includes_top_5_implications_section() {
    let sources = vec![src_body(1, None), src_body(2, None)];
    let legacy = build_synthesis_prompt("topic", &sources);
    let builder = SynthesisPromptBuilder::new("topic")
        .sources(&sources)
        .build();
    // The default-config builder now asks for the Top 10 Implications
    // section, so it is no longer byte-identical to the legacy six-section
    // prompt. Both the legacy wrapper and the builder must include it.
    assert!(legacy.contains("## Top 10 Implications"));
    assert!(builder.contains("## Top 10 Implications"));
    assert!(legacy.contains("## In-Project Cross-References"));
    assert!(builder.contains("## Open Questions"));
}

#[test]
fn builder_emits_five_required_finding_labels() {
    let sources = vec![src_body(1, None)];
    let prompt = SynthesisPromptBuilder::new("topic")
        .sources(&sources)
        .build();
    assert!(prompt.contains("**Headline:**"));
    assert!(prompt.contains("**Observation:**"));
    assert!(prompt.contains("**Analysis:**"));
    assert!(prompt.contains("**Cross-reference / Dependencies:**"));
    assert!(prompt.contains("**Implication:**"));
}

#[test]
fn builder_emits_top_5_implications_section() {
    let sources = vec![src_body(1, None)];
    let prompt = SynthesisPromptBuilder::new("topic")
        .sources(&sources)
        .build();
    assert!(prompt.contains("## Top 10 Implications"));
    assert!(prompt.contains("rank the top 10 implications"));
}

#[test]
fn builder_date_spread_paragraph_adds_sixth_label_and_published_line() {
    let sources = vec![
        src_body(
            1,
            Some(DateTime::from_naive_utc_and_offset(
                chrono::NaiveDate::from_ymd_opt(2026, 1, 15)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap(),
                Utc,
            )),
        ),
        src_body(2, None),
    ];
    let config = SynthesisPromptConfig {
        date_spread_paragraph: true,
        ..Default::default()
    };
    let prompt = SynthesisPromptBuilder::new("topic")
        .sources(&sources)
        .config(config)
        .build();
    assert!(
        prompt.contains("**Sources Cited / Date Spread:**"),
        "date-spread paragraph must be required when configured"
    );
    assert!(
        prompt.contains("Published (UTC): 2026-01-15"),
        "dated web sources must surface their publication date in the source block"
    );
    assert!(
        prompt.contains("Published (UTC): undated"),
        "undated web sources must be labelled undated in the source block"
    );
}

#[test]
fn builder_recency_rule_emits_recency_instructions() {
    let sources = vec![src_body(1, None)];
    let config = SynthesisPromptConfig {
        recency_rule: true,
        ..Default::default()
    };
    let prompt = SynthesisPromptBuilder::new("topic")
        .sources(&sources)
        .config(config)
        .build();
    assert!(
        prompt.contains("Recency-weighting rule"),
        "recency rule block must be emitted when configured"
    );
    assert!(prompt.contains("prefer the more recently published source"));
}

#[test]
fn builder_few_shot_appends_exemplars() {
    let sources = vec![src_body(1, None)];
    let exemplar = "**Observation:** example obs [#1].\n\n**Analysis:** a.\n\n\
         **Cross-reference / Dependencies:** No direct dependencies.\n\n\
         **Implication:** i."
        .to_string();
    let config = SynthesisPromptConfig {
        few_shot_examples: vec![exemplar],
        ..Default::default()
    };
    let prompt = SynthesisPromptBuilder::new("topic")
        .sources(&sources)
        .config(config)
        .build();
    assert!(prompt.contains("Few-shot exemplar findings"));
    assert!(prompt.contains("### Exemplar Finding 1"));
    assert!(prompt.contains("example obs [#1]"));
}

#[test]
fn builder_few_shot_caps_at_two_exemplars() {
    let sources = vec![src_body(1, None)];
    let make = |n: usize| {
        format!(
            "**Observation:** obs {n} [#1].\n\n**Analysis:** a.\n\n\
             **Cross-reference / Dependencies:** No direct dependencies.\n\n\
             **Implication:** i."
        )
    };
    let config = SynthesisPromptConfig {
        few_shot_examples: vec![make(1), make(2), make(3)],
        ..Default::default()
    };
    let prompt = SynthesisPromptBuilder::new("topic")
        .sources(&sources)
        .config(config)
        .build();
    assert!(prompt.contains("### Exemplar Finding 1"));
    assert!(prompt.contains("### Exemplar Finding 2"));
    assert!(
        !prompt.contains("### Exemplar Finding 3"),
        "few-shot block must cap at two exemplars to bound context cost"
    );
}

#[test]
fn parse_with_outcome_clean_response_returns_llm() {
    let text = "## Executive Summary\n\nAn executive summary.\n\n## Findings\n\n\
         1. **Headline:** Observation summary\n\n**Observation:** obs [#1].\n\n\
         **Analysis:** a.\n\n\
         **Cross-reference / Dependencies:** No direct dependencies.\n\n\
         **Implication:** i.\n";
    let sources = vec![src_body(1, None)];
    let (result, outcome) = parse_analysis_response_with_outcome(text, &sources);
    assert_eq!(outcome, AnalysisOutcome::Llm);
    assert_eq!(result.findings.len(), 1);
    assert!(result.findings[0].contains("**Observation:** obs [#1]"));
}

#[test]
fn parse_with_outcome_empty_response_falls_back() {
    let sources = vec![src_body(1, None)];
    let (result, outcome) = parse_analysis_response_with_outcome("", &sources);
    assert_eq!(outcome, AnalysisOutcome::FallbackEmpty);
    // FR-011 / T-010: fallback always produces >=1 finding.
    assert!(!result.findings.is_empty());
    assert!(result.findings[0].contains("**Observation:**"));
    assert!(result.findings[0].contains("**Analysis:**"));
    assert!(result.findings[0].contains("**Analysis:**"));
    assert!(result.findings[0].contains("**Cross-reference / Dependencies:**"));
    assert!(result.findings[0].contains("**Implication:**"));
}

#[test]
fn parse_with_outcome_no_findings_section_falls_back() {
    // A response that only has a summary (no ## Findings) is malformed.
    let text = "## Executive Summary\n\nOnly an executive summary, no findings section.\n";
    let sources = vec![src_body(1, None)];
    let (result, outcome) = parse_analysis_response_with_outcome(text, &sources);
    assert_eq!(outcome, AnalysisOutcome::FallbackEmpty);
    assert!(!result.findings.is_empty());
}

#[test]
fn parse_with_outcome_finding_missing_labels_falls_back() {
    // A finding that lacks the required bold labels is malformed.
    let text = "## Findings\n\n1. Just a plain finding with no labels and no citation.\n";
    let sources = vec![src_body(1, None)];
    let (result, outcome) = parse_analysis_response_with_outcome(text, &sources);
    assert_eq!(outcome, AnalysisOutcome::FallbackEmpty);
    assert!(!result.findings.is_empty());
    // The mechanical fallback inserts the missing labels as placeholders.
    assert!(result.findings[0].contains("**Observation:**"));
}

#[test]
fn mechanical_fallback_never_returns_empty_vec() {
    // FR-011 / T-010 non-empty guarantee: exercise several degenerate
    // inputs and confirm at least one finding is always produced.
    for input in [
        "",
        "   \n\n  ",
        "## Executive Summary\n\nonly executive summary",
        "no headings at all",
    ] {
        let findings = mechanical_fallback_findings(input);
        assert!(
            !findings.is_empty(),
            "input {input:?} should yield >=1 finding"
        );
        for f in &findings {
            assert!(f.contains("**Observation:**"));
            assert!(f.contains("**Analysis:**"));
            assert!(f.contains("**Cross-reference / Dependencies:**"));
            assert!(f.contains("**Implication:**"));
        }
    }
}

#[test]
fn mechanical_fallback_preserves_raw_text_in_placeholder() {
    // An empty model response hits the placeholder branch that quotes the
    // raw model output (FR-011 / T-010 non-empty guarantee).
    let findings = mechanical_fallback_findings("");
    assert_eq!(findings.len(), 1);
    assert!(
        findings[0].contains("(findings could not be structured - see below)"),
        "placeholder must use the spec's wording, got: {}",
        findings[0]
    );
    assert!(
        findings[0].contains("(no model response was returned)"),
        "empty-response placeholder must explain the model returned no content, got: {}",
        findings[0]
    );
}

#[test]
fn mechanical_fallback_preserves_nonempty_raw_in_placeholder() {
    // A whitespace-only response has no extractable structure and hits the
    // placeholder branch (extract_candidate_findings returns [] because
    // parse_numbered_list and the paragraph splitter both yield nothing).
    let findings = mechanical_fallback_findings("   \n\n  \n");
    assert_eq!(findings.len(), 1);
    assert!(
        findings[0].contains("(findings could not be structured - see below)"),
        "placeholder must use the spec's wording for whitespace-only input, got: {}",
        findings[0]
    );
}

#[test]
fn validate_citations_flags_out_of_range() {
    // Source list has 2 entries; a [#5] citation is out of range.
    let sources = vec![src_body(1, None), src_body(2, None)];
    let mut findings = vec![
        "**Observation:** obs [#1] and [#5].\n\n**Analysis:** a.\n\n\
         **Cross-reference / Dependencies:** No direct dependencies.\n\n\
         **Implication:** i."
            .to_string(),
    ];
    let warnings = validate_citations_and_dates(&mut findings, &sources);
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("out of range") || w.contains("source(s) were captured")),
        "expected an out-of-range citation warning, got {warnings:?}"
    );
    assert!(
        findings[0].contains("[#5?] (out of range"),
        "out-of-range citation must be rewritten inline, got: {}",
        findings[0]
    );
    assert!(
        findings[0].contains("[#1]"),
        "in-range citations must be preserved verbatim"
    );
}

#[test]
fn validate_dates_flags_unsupported_claim() {
    let valid = DateTime::from_naive_utc_and_offset(
        chrono::NaiveDate::from_ymd_opt(2026, 1, 15)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap(),
        Utc,
    );
    let sources = vec![src_body(1, Some(valid))];
    // The finding cites [#1] (valid) but claims a date (1999-12-31) that
    // is not among the captured sources' publication dates.
    let mut findings = vec![
        "**Observation:** obs [#1].\n\n**Analysis:** a.\n\n\
         **Cross-reference / Dependencies:** No direct dependencies.\n\n\
         **Implication:** i.\n\n\
         **Sources Cited / Date Spread:** [#1] published 1999-12-31..2026-01-15."
            .to_string(),
    ];
    let warnings = validate_citations_and_dates(&mut findings, &sources);
    assert!(
        warnings
            .iter()
            .any(|w| w.contains("1999-12-31") && w.contains("not among")),
        "expected an unsupported-date warning, got {warnings:?}"
    );
    assert!(
        findings[0].contains("(unsupported date)"),
        "unsupported date must be rewritten inline, got: {}",
        findings[0]
    );
    // The valid date (2026-01-15) must be preserved.
    assert!(findings[0].contains("2026-01-15"));
}

#[test]
fn validate_leaves_valid_finding_untouched() {
    let valid = DateTime::from_naive_utc_and_offset(
        chrono::NaiveDate::from_ymd_opt(2026, 1, 15)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap(),
        Utc,
    );
    let sources = vec![src_body(1, Some(valid))];
    let original = "**Observation:** obs [#1].\n\n**Analysis:** a.\n\n\
         **Cross-reference / Dependencies:** No direct dependencies.\n\n\
         **Implication:** i.\n\n\
         **Sources Cited / Date Spread:** [#1] published 2026-01-15."
        .to_string();
    let mut findings = vec![original.clone()];
    let warnings = validate_citations_and_dates(&mut findings, &sources);
    assert!(
        warnings.is_empty(),
        "no warnings expected, got {warnings:?}"
    );
    assert_eq!(findings[0], original, "valid finding must be unchanged");
}

#[test]
fn parse_with_outcome_preserves_valid_summary_on_fallback() {
    // A response with a valid ## Executive Summary but malformed findings (no
    // required bold labels) must preserve the parsed summary rather
    // than discarding it for a diagnostic placeholder.
    let text = "## Executive Summary\n\nThis is a valid executive summary that must be preserved.\n\n\
         ## Findings\n\n1. Just a plain finding with no labels and no citation.\n";
    let sources = vec![src_body(1, None)];
    let (result, outcome) = parse_analysis_response_with_outcome(text, &sources);
    assert_eq!(outcome, AnalysisOutcome::FallbackEmpty);
    assert!(
        result
            .summary
            .contains("This is a valid executive summary that must be preserved."),
        "valid executive summary must be preserved on fallback, got: {}",
        result.summary
    );
}

#[test]
fn parse_with_outcome_strips_control_chars_from_clean_parse() {
    // A clean response that contains C0 control characters (e.g. 0x01)
    // must have them stripped from findings and summary.
    let text = "## Executive Summary\n\nExecutive summary with \x01 control char.\n\n\
         ## Findings\n\n\
         1. **Headline:** Obs\x02summary\n\n**Observation:** obs [#1].\x03\n\n\
         **Analysis:** a.\n\n\
         **Cross-reference / Dependencies:** No direct dependencies.\n\n\
         **Implication:** i.\n\n\
         ## Top 10 Implications\n\n1. \x04Implication.\n";
    let sources = vec![src_body(1, None)];
    let (result, outcome) = parse_analysis_response_with_outcome(text, &sources);
    assert_eq!(outcome, AnalysisOutcome::Llm);
    assert!(
        !result.summary.contains('\x01'),
        "control chars must be stripped from summary, got: {:?}",
        result.summary
    );
    assert!(
        !result.findings[0].contains('\x02') && !result.findings[0].contains('\x03'),
        "control chars must be stripped from findings, got: {:?}",
        result.findings[0]
    );
    assert!(
        !result.top_implications[0].contains('\x04'),
        "control chars must be stripped from top implications, got: {:?}",
        result.top_implications[0]
    );
}

#[test]
fn parse_with_outcome_strips_control_chars_from_fallback() {
    // A malformed response with control chars must sanitize them before
    // mechanical extraction so the placeholder finding is clean.
    let text = "## Summary\n\n\x01Bad summary\x02\n\nNo findings here.\n";
    let sources = vec![src_body(1, None)];
    let (result, outcome) = parse_analysis_response_with_outcome(text, &sources);
    assert_eq!(outcome, AnalysisOutcome::FallbackEmpty);
    for finding in &result.findings {
        assert!(
            !finding.contains('\x01') && !finding.contains('\x02'),
            "control chars must be stripped from fallback findings, got: {:?}",
            finding
        );
    }
}

// -- Milestone E-001: SourceSummarizer / HeuristicSummarizer tests ----

#[test]
fn heuristic_summarizer_returns_body_unchanged_when_within_budget() {
    let s = HeuristicSummarizer;
    let body = "Short body.";
    assert_eq!(s.summarize(body, 100), body);
}

#[test]
fn heuristic_summarizer_truncates_to_budget_chars() {
    let s = HeuristicSummarizer;
    let body = "a".repeat(500);
    let summarized = s.summarize(&body, 100);
    assert!(
        summarized.chars().count() <= 160,
        "summarized body must be approximately within budget, got {} chars",
        summarized.chars().count()
    );
    assert!(
        summarized.contains("... (summarized"),
        "truncation marker must be present"
    );
}

#[test]
fn heuristic_summarizer_snaps_to_paragraph_boundary() {
    let s = HeuristicSummarizer;
    let body = "First paragraph with enough text to fill the budget.\n\nSecond paragraph that should be cut.";
    let summarized = s.summarize(body, 60);
    assert!(
        summarized.contains("First paragraph"),
        "should keep the first paragraph"
    );
    assert!(
        !summarized.contains("Second paragraph"),
        "should cut at the paragraph boundary"
    );
}

#[test]
fn summarize_source_bodies_preserves_metadata() {
    let bodies = vec![SourceBody {
        index: 5,
        kind: "web".to_string(),
        title: "Test".to_string(),
        path_or_url: "https://example.com".to_string(),
        relevance: "High".to_string(),
        body: "a".repeat(500),
        published_at: None,
        author: None,
    }];
    let summarizer = HeuristicSummarizer;
    let summarized = summarize_source_bodies(&bodies, &summarizer, 100);
    assert_eq!(summarized.len(), 1);
    assert_eq!(summarized[0].index, 5);
    assert_eq!(summarized[0].title, "Test");
    assert_eq!(summarized[0].relevance, "High");
    assert!(summarized[0].body.chars().count() < 200);
}

// -- Milestone E-002: chunking + merge tests ---------------------------

#[test]
fn total_body_chars_sums_all_bodies() {
    let bodies = vec![
        SourceBody {
            index: 1,
            kind: "web".to_string(),
            title: "A".to_string(),
            path_or_url: String::new(),
            relevance: String::new(),
            body: "hello".to_string(),
            published_at: None,
            author: None,
        },
        SourceBody {
            index: 2,
            kind: "web".to_string(),
            title: "B".to_string(),
            path_or_url: String::new(),
            relevance: String::new(),
            body: "world!".to_string(),
            published_at: None,
            author: None,
        },
    ];
    assert_eq!(total_body_chars(&bodies), 11);
}

#[test]
fn chunk_source_bodies_splits_on_budget() {
    let make = |i: usize, body: &str| SourceBody {
        index: i,
        kind: "web".to_string(),
        title: format!("S{i}"),
        path_or_url: String::new(),
        relevance: String::new(),
        body: body.to_string(),
        published_at: None,
        author: None,
    };
    let bodies = vec![
        make(1, &"a".repeat(40)),
        make(2, &"b".repeat(40)),
        make(3, &"c".repeat(40)),
    ];
    let chunks = chunk_source_bodies(&bodies, 50);
    assert_eq!(
        chunks.len(),
        3,
        "each source should be its own chunk at budget 50"
    );
}

#[test]
fn chunk_source_bodies_groups_small_sources() {
    let make = |i: usize, body: &str| SourceBody {
        index: i,
        kind: "web".to_string(),
        title: format!("S{i}"),
        path_or_url: String::new(),
        relevance: String::new(),
        body: body.to_string(),
        published_at: None,
        author: None,
    };
    let bodies = vec![make(1, "small1"), make(2, "small2"), make(3, "small3")];
    let chunks = chunk_source_bodies(&bodies, 100);
    assert_eq!(chunks.len(), 1, "all small sources should fit in one chunk");
}

#[test]
fn merge_chunk_results_concatenates_findings_and_renumbers() {
    let part1 = AnalysisResult {
        summary: "Summary 1".to_string(),
        findings: vec![
            "1. **Headline:** A\n\n**Observation:** obs [#1].\n\n**Analysis:** a.\n\n**Cross-reference / Dependencies:** No direct dependencies.\n\n**Implication:** i.".to_string(),
            "2. **Headline:** B\n\n**Observation:** obs [#2].\n\n**Analysis:** b.\n\n**Cross-reference / Dependencies:** No direct dependencies.\n\n**Implication:** j.".to_string(),
        ],
        top_implications: vec!["Adopt A.".to_string()],
        cross_references: Vec::new(),
        open_questions: vec!["Q1?".to_string()],
    };
    let part2 = AnalysisResult {
        summary: "Summary 2".to_string(),
        findings: vec![
            "1. **Headline:** C\n\n**Observation:** obs [#3].\n\n**Analysis:** c.\n\n**Cross-reference / Dependencies:** No direct dependencies.\n\n**Implication:** k.".to_string(),
        ],
        top_implications: vec!["Adopt A.".to_string(), "Consider C.".to_string()],
        cross_references: Vec::new(),
        open_questions: vec!["Q2?".to_string()],
    };
    let merged = merge_chunk_results(&[part1, part2]);
    assert_eq!(merged.findings.len(), 3);
    // Findings should be renumbered 1, 2, 3.
    assert!(merged.findings[0].starts_with("1. "));
    assert!(merged.findings[1].starts_with("2. "));
    assert!(merged.findings[2].starts_with("3. "));
    // Summaries should be joined.
    assert!(merged.summary.contains("Summary 1"));
    assert!(merged.summary.contains("Summary 2"));
    // Top implications merged and deduped.
    assert_eq!(
        merged.top_implications,
        vec!["Adopt A.".to_string(), "Consider C.".to_string()]
    );
    // Open questions merged.
    assert_eq!(merged.open_questions, vec!["Q1?", "Q2?"]);
}

#[test]
fn merge_chunk_results_dedup_cross_references() {
    let cr = CrossReference {
        path: "src/lib.rs".to_string(),
        relevance: "main".to_string(),
    };
    let part1 = AnalysisResult {
        summary: String::new(),
        findings: Vec::new(),
        top_implications: Vec::new(),
        cross_references: vec![cr.clone()],
        open_questions: Vec::new(),
    };
    let part2 = AnalysisResult {
        summary: String::new(),
        findings: Vec::new(),
        top_implications: Vec::new(),
        cross_references: vec![
            cr.clone(),
            CrossReference {
                path: "src/main.rs".to_string(),
                relevance: "entry".to_string(),
            },
        ],
        open_questions: Vec::new(),
    };
    let merged = merge_chunk_results(&[part1, part2]);
    assert_eq!(merged.cross_references.len(), 2);
    assert_eq!(merged.cross_references[0].path, "src/lib.rs");
    assert_eq!(merged.cross_references[1].path, "src/main.rs");
}

#[test]
fn merge_chunk_results_single_part_is_clone() {
    let part = AnalysisResult {
        summary: "Only".to_string(),
        findings: vec!["1. Finding.".to_string()],
        top_implications: vec!["Only implication.".to_string()],
        cross_references: Vec::new(),
        open_questions: Vec::new(),
    };
    let merged = merge_chunk_results(std::slice::from_ref(&part));
    assert_eq!(merged, part);
}

#[test]
fn merge_chunk_results_empty_returns_default() {
    let merged = merge_chunk_results(&[]);
    assert_eq!(merged, AnalysisResult::default());
}
