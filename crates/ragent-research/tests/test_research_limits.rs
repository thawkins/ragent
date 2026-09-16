//! Tests for T-001: the shared reverse-relevance ordering helper and the
//! built-in output-limit defaults (spec `researchmax`; FR-001, FR-002, FR-005,
//! NFR-001).
//!
//! The helper is consumed by both the findings (synthesis) and concept
//! (cluster) paths, so these tests pin its ordering contract directly:
//! most-relevant-first by highest cited source rank, ties broken by cited
//! count then original model order, and uncited entries treated as medium.

use ragent_research::source::Source;
use ragent_research::{
    DEFAULT_ENTRY_RANK, DEFAULT_MAX_CONCEPTS, DEFAULT_MAX_FINDINGS, cap_findings_to_limit,
    cited_source_indices, cited_source_ranks, rank_entries_by_reverse_relevance,
    source_rank_lookup,
};
use std::path::PathBuf;
use std::sync::Arc;

/// A web source whose relevance label drives [`Source::relevance_rank`].
fn source(label: &str) -> Source {
    Source::Web {
        url: "https://example.invalid".to_string(),
        title: "Source".to_string(),
        captured_at: chrono::Utc::now(),
        published_at: None,
        body_path: PathBuf::new(),
        body: String::new(),
        relevance: label.to_string(),
        search_tool: "mf_search".to_string(),
        search_engine: "openalex".to_string(),
        content_type: None,
        page_type: None,
        media_type: "page".to_string(),
        language: None,
        oa_recovery: None,
        author: None,
    }
}

fn ranks_of(entries: &[&str], sources: &[Source]) -> Vec<u8> {
    let lookup = source_rank_lookup(sources);
    entries
        .iter()
        .map(|body| {
            cited_source_ranks(body, &lookup)
                .into_iter()
                .max()
                .unwrap_or(DEFAULT_ENTRY_RANK)
        })
        .collect()
}

#[test]
fn defaults_match_the_spec() {
    assert_eq!(DEFAULT_MAX_CONCEPTS, 5);
    assert_eq!(DEFAULT_MAX_FINDINGS, 20);
    assert_eq!(DEFAULT_ENTRY_RANK, 5);
}

#[test]
fn orders_entries_by_highest_cited_rank() {
    // Source 1 = Very high (8), 2 = Very low (1), 3 = Low (3).
    let sources = vec![source("Very high"), source("Very low"), source("Low")];
    let entries = vec!["weak [#2]", "strong [#1]", "middling [#3]"];
    let lookup = source_rank_lookup(&sources);
    let ordered = rank_entries_by_reverse_relevance(entries, |e| *e, lookup);
    assert_eq!(ordered, vec!["strong [#1]", "middling [#3]", "weak [#2]"]);
}

#[test]
fn ties_break_by_cited_count_then_original_order() {
    // All three cite only rank-3 sources (same highest rank, same distinct
    // count), so the entry citing two sources leads and the remaining two
    // keep their original model order.
    let sources = vec![source("Low"), source("Low")];
    let entries = vec!["a [#1]", "b [#1] and [#2]", "c [#2]"];
    let lookup = source_rank_lookup(&sources);
    let ordered = rank_entries_by_reverse_relevance(entries, |e| *e, lookup);
    assert_eq!(ordered, vec!["b [#1] and [#2]", "a [#1]", "c [#2]"]);
}

#[test]
fn cited_more_sources_outranks_equal_rank() {
    // Two entries whose highest cited source is the same tie on rank; the one
    // that also cites a further source wins on cited count.
    let sources = vec![source("Very high"), source("Low")];
    let entries = vec!["one [#1]", "two [#1] and [#2]"];
    let lookup = source_rank_lookup(&sources);
    let ordered = rank_entries_by_reverse_relevance(entries, |e| *e, lookup);
    assert_eq!(ordered, vec!["two [#1] and [#2]", "one [#1]"]);
}

#[test]
fn uncited_entries_default_to_medium_rank() {
    let sources = vec![source("Very low")];
    let entries = vec!["no citation here", "weak [#1]"];
    let lookup = source_rank_lookup(&sources);
    let ordered = rank_entries_by_reverse_relevance(entries, |e| *e, lookup);
    // Rank 5 (uncited) outranks rank 1 (very low), so the uncited entry leads.
    assert_eq!(ordered, vec!["no citation here", "weak [#1]"]);
    assert_eq!(ranks_of(&ordered, &sources), vec![5, 1]);
}

#[test]
fn malformed_markers_are_ignored_without_panicking() {
    let sources = vec![source("High")];
    let entries = vec!["bogus [#x] and [#] and [#1]", "plain"];
    let lookup = source_rank_lookup(&sources);
    let ordered = rank_entries_by_reverse_relevance(entries, |e| *e, lookup);
    // The valid [#1] (rank 7) wins; the malformed markers fall back to 5.
    assert_eq!(ordered, vec!["bogus [#x] and [#] and [#1]", "plain"]);
}

#[test]
fn accepts_legacy_web_filename_citations() {
    let sources = vec![source("Very high")];
    let lookup = source_rank_lookup(&sources);
    assert_eq!(cited_source_ranks("see web-01.md", &lookup), vec![8]);
}

#[test]
fn repeated_citations_count_once() {
    let sources = vec![source("Low")];
    let lookup = source_rank_lookup(&sources);
    assert_eq!(cited_source_indices("[#1] and [#1] and [#1]"), vec![1]);
    assert_eq!(cited_source_ranks("[#1] and [#1]", &lookup), vec![3]);
}

#[test]
fn helper_accepts_borrowed_entry_types() {
    // The helper is used by both the findings (`Vec<String>`) and concept
    // (`Vec<Rc/Arc<...>>`) paths, so it must work through `body_of` adapters
    // rather than requiring an owned `String`.
    let sources = vec![source("Very high"), source("Low")];
    let lookup = source_rank_lookup(&sources);
    let entries = vec![
        Arc::new("low [#2]".to_string()),
        Arc::new("high [#1]".to_string()),
    ];
    let ordered = rank_entries_by_reverse_relevance(entries, |e| e.as_str(), lookup);
    assert_eq!(ordered[0].as_str(), "high [#1]");
    assert_eq!(ordered[1].as_str(), "low [#2]");
}

#[test]
fn unknown_indices_yield_no_rank() {
    let sources = vec![source("High")];
    let lookup = source_rank_lookup(&sources);
    assert!(cited_source_ranks("[#9]", &lookup).is_empty());
    assert!(cited_source_ranks("", &lookup).is_empty());
}

#[test]
fn empty_input_returns_empty() {
    let sources: Vec<Source> = Vec::new();
    let lookup = source_rank_lookup(&sources);
    let ordered = rank_entries_by_reverse_relevance(Vec::<&str>::new(), |e| *e, lookup);
    assert!(ordered.is_empty());
}

// ── T-002: AnalysisConfig carries the two output limits ───────────────────

/// `AnalysisConfig::default()` must seed both limits from the shared constants
/// (FR-002, FR-014) so a run that sets neither flag is capped identically
/// across every front end.
#[test]
fn analysis_config_defaults_to_the_spec_limits() {
    let cfg = ragent_research::AnalysisConfig::default();
    assert_eq!(cfg.max_concepts, DEFAULT_MAX_CONCEPTS);
    assert_eq!(cfg.max_findings, DEFAULT_MAX_FINDINGS);
}

// ── T-003: cap_findings_to_limit orders then truncates and renumbers ───────

/// The cap keeps the most-relevant findings, drops the least-relevant ones,
/// and renumbers the survivors contiguously from 1 (FR-003, FR-010).
#[test]
fn cap_findings_keeps_top_n_and_renumbers() {
    // Source 1 = Very high (8), 3 = Very low (1), 4 = Medium (5).
    let sources = vec![
        source("Very high"),
        source("Low"),
        source("Very low"),
        source("Medium"),
    ];
    let findings = vec![
        "1. weak point [#3]".to_string(),
        "2. strong point [#1]".to_string(),
        "3. uncited point".to_string(),
        "4. medium point [#4]".to_string(),
    ];
    let capped = cap_findings_to_limit(findings, 2, &sources);
    // Rank 8 wins outright; among the rank-5 ties the cited finding outranks
    // the uncited one, which is dropped along with the rank-1 finding.
    assert_eq!(
        capped,
        vec![
            "1. strong point [#1]".to_string(),
            "2. medium point [#4]".to_string(),
        ]
    );
}

/// `max_findings == 0` disables the cap (FR-016): every finding survives, but
/// the list is still ordered by reverse relevance and renumbered.
#[test]
fn cap_findings_zero_is_unbounded() {
    let sources = vec![source("Very high"), source("Low")];
    let findings = vec!["1. low [#2]".to_string(), "2. high [#1]".to_string()];
    let capped = cap_findings_to_limit(findings, 0, &sources);
    assert_eq!(
        capped,
        vec!["1. high [#1]".to_string(), "2. low [#2]".to_string()]
    );
}

/// A limit at or above the entry count keeps every entry unchanged in count and
/// never pads or invents entries (FR-015, FR-022).
#[test]
fn cap_findings_never_pads_when_fewer_than_limit() {
    let sources = vec![source("High")];
    let findings = vec!["1. only one [#1]".to_string()];
    assert_eq!(
        cap_findings_to_limit(findings.clone(), 20, &sources),
        findings
    );
    assert_eq!(
        cap_findings_to_limit(findings, usize::MAX, &sources).len(),
        1
    );
}

/// Even a limit of 1 retains the single most-relevant finding rather than
/// producing an empty list (FR-020).
#[test]
fn cap_findings_retains_at_least_one() {
    let sources = vec![source("Very high"), source("Very low")];
    let findings = vec!["1. weak [#2]".to_string(), "2. strong [#1]".to_string()];
    let capped = cap_findings_to_limit(findings, 1, &sources);
    assert_eq!(capped, vec!["1. strong [#1]".to_string()]);
}

/// An empty findings list stays empty (no synthetic entry is invented).
#[test]
fn cap_findings_empty_stays_empty() {
    let sources: Vec<Source> = Vec::new();
    assert!(cap_findings_to_limit(Vec::new(), 10, &sources).is_empty());
}
