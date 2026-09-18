//! Unit tests for `archdoc::extract` - architecture-structure extraction for
//! `/spec govcreate` (spec `govdoc` T-008, FR-007, NFR-002).
//!
//! Everything under test is pure (NFR-002): the extraction prompt builder, the
//! model-response parser, and the deterministic fallback structure. No test
//! performs I/O or network access.

use ragent_tools_extended::archdoc::url_source::{
    AcquisitionStats, ExcludedSource, ExclusionReason,
};
use ragent_tools_extended::archdoc::{
    ArchitectureStructure, GatheredCorpus, GatheredSource, MAX_PROMPT_CORPUS_CHARS,
    build_arch_extraction_prompt, fallback_structure, parse_architecture_response,
};
use ragent_tools_extended::masterfetch::PageType;

// ===========================================================================
// Fixture helpers
// ===========================================================================

fn source(url: &str, summary: &str, content: &str) -> GatheredSource {
    GatheredSource {
        url: url.to_string(),
        summary: summary.to_string(),
        page_type: PageType::Docs,
        content: content.to_string(),
    }
}

fn corpus(sources: Vec<GatheredSource>) -> GatheredCorpus {
    let total_chars: usize = sources
        .iter()
        .map(|source| source.content.chars().count())
        .sum();
    let text = sources
        .iter()
        .map(|source| {
            format!(
                "## {}\n{}\n\n{}\n\n",
                source.summary, source.url, source.content
            )
        })
        .collect();
    let source_count = sources.len();
    GatheredCorpus {
        reference: "docs/arch".to_string(),
        sources,
        excluded: Vec::new(),
        text,
        stats: AcquisitionStats {
            pages_fetched: source_count,
            total_chars,
            elapsed_ms: 10,
        },
        budget_reached: None,
    }
}

fn populated_corpus() -> GatheredCorpus {
    corpus(vec![
        source(
            "https://docs.example.gov/arch/payments",
            "Payments",
            "# Payments Service\nHandles card authorisation.\n## Ledger\nDouble-entry postings.",
        ),
        source(
            "docs/arch/ledger.md",
            "Ledger",
            "# Ledger\nOwns the double-entry postings table.",
        ),
    ])
}

// ===========================================================================
// build_arch_extraction_prompt (FR-007)
// ===========================================================================

#[test]
fn test_prompt_requires_all_fr007_dimensions() {
    let prompt = build_arch_extraction_prompt(&populated_corpus());
    for required in [
        "components",
        "responsibilities",
        "interfaces",
        "data_stores",
        "external_dependencies",
        "relationships",
    ] {
        assert!(prompt.contains(required), "prompt missing '{required}'");
    }
    assert!(prompt.contains("SYSTEM ARCHITECTURE STRUCTURE"));
    assert!(prompt.contains("JSON"));
}

#[test]
fn test_prompt_embeds_reference_inventory_and_text() {
    let populated = populated_corpus();
    let prompt = build_arch_extraction_prompt(&populated);
    assert!(prompt.contains("Content reference: docs/arch"));
    assert!(prompt.contains("Gathered sources: 2"));
    // Numbered source inventory.
    assert!(prompt.contains("1. https://docs.example.gov/arch/payments"));
    assert!(prompt.contains("2. docs/arch/ledger.md"));
    // Gathered text embedded between the sentinels.
    assert!(prompt.contains("--- GATHERED DOCUMENTATION ---"));
    assert!(prompt.contains("--- END GATHERED DOCUMENTATION ---"));
    assert!(prompt.contains("# Payments Service"));
}

#[test]
fn test_prompt_truncates_gathered_text_at_cap() {
    let long = "a".repeat(MAX_PROMPT_CORPUS_CHARS + 10_000);
    let populated = corpus(vec![source("docs/big.md", "Big", &long)]);
    let prompt = build_arch_extraction_prompt(&populated);
    assert!(
        !prompt.contains(&"a".repeat(MAX_PROMPT_CORPUS_CHARS + 1)),
        "gathered text must be truncated at MAX_PROMPT_CORPUS_CHARS"
    );
    assert_eq!(
        MAX_PROMPT_CORPUS_CHARS, 64_000,
        "prompt corpus cap must stay a named constant"
    );
}

// ===========================================================================
// parse_architecture_response
// ===========================================================================

const VALID_JSON: &str = r#"{
  "components": [
    {"name": "Payments", "responsibilities": ["card authorisation"]},
    {"name": "Ledger", "responsibilities": []}
  ],
  "interfaces": [
    {"name": "Authorise", "between": ["Payments", "Ledger"], "contract": "REST /authorise"}
  ],
  "data_stores": [
    {"name": "ledger-db", "kind": "database", "used_by": ["Ledger"]}
  ],
  "external_dependencies": [
    {"name": "card-scheme", "kind": "service", "used_by": ["Payments"]}
  ],
  "relationships": [
    {"from": "Payments", "to": "Ledger", "kind": "writes", "detail": "posts entries"}
  ]
}"#;

#[test]
fn test_parse_clean_json_populates_all_lists() {
    let structure = parse_architecture_response(VALID_JSON).expect("clean JSON must parse");
    assert_eq!(structure.components.len(), 2);
    assert_eq!(structure.components[0].name, "Payments");
    assert_eq!(
        structure.components[0].responsibilities,
        vec!["card authorisation".to_string()]
    );
    assert_eq!(structure.interfaces.len(), 1);
    assert_eq!(structure.interfaces[0].between.len(), 2);
    assert_eq!(structure.data_stores[0].kind, "database");
    assert_eq!(structure.external_dependencies[0].name, "card-scheme");
    assert_eq!(structure.relationships[0].kind, "writes");
    assert!(!structure.from_fallback);
    assert!(!structure.is_empty());
}

#[test]
fn test_parse_tolerates_code_fences_and_prose() {
    let fenced = format!("Here is the structure:\n```json\n{VALID_JSON}\n```\nHope that helps.");
    let structure = parse_architecture_response(&fenced).expect("fenced JSON must parse");
    assert_eq!(structure.components.len(), 2);
}

#[test]
fn test_parse_rejects_non_object_content() {
    assert!(parse_architecture_response("[1, 2, 3]").is_none());
    assert!(parse_architecture_response("\"not an object\"").is_none());
    assert!(parse_architecture_response("42").is_none());
}

#[test]
fn test_parse_rejects_garbage() {
    assert!(parse_architecture_response("").is_none());
    assert!(parse_architecture_response("the model rambled with no braces at all").is_none());
    assert!(parse_architecture_response("{unclosed").is_none());
}

#[test]
fn test_parse_rejects_unknown_fields() {
    // The contract denies unknown fields so model drift surfaces as a
    // fallback, not silent acceptance.
    let drifted = r#"{"components": [], "bogus": true}"#;
    assert!(parse_architecture_response(drifted).is_none());
}

#[test]
fn test_parse_omitted_fields_default_to_empty() {
    let minimal = r#"{"components": [{"name": "Gateway", "responsibilities": []}]}"#;
    let structure = parse_architecture_response(minimal).expect("minimal JSON must parse");
    assert_eq!(structure.components.len(), 1);
    assert!(structure.interfaces.is_empty());
    assert!(structure.data_stores.is_empty());
    assert!(structure.external_dependencies.is_empty());
    assert!(structure.relationships.is_empty());
}

// ===========================================================================
// fallback_structure (deterministic rescue)
// ===========================================================================

#[test]
fn test_fallback_names_components_from_sources() {
    let structure = fallback_structure(&populated_corpus());
    assert!(structure.from_fallback);
    let names: Vec<&str> = structure
        .components
        .iter()
        .map(|component| component.name.as_str())
        .collect();
    assert!(names.contains(&"payments"), "URL segment name: {names:?}");
    assert!(names.contains(&"ledger"), "file stem name: {names:?}");
}

#[test]
fn test_fallback_uses_headings_as_responsibilities() {
    let structure = fallback_structure(&populated_corpus());
    let payments = structure
        .components
        .iter()
        .find(|component| component.name == "payments")
        .expect("payments component");
    assert_eq!(
        payments.responsibilities,
        vec!["Payments Service".to_string(), "Ledger".to_string()]
    );
}

#[test]
fn test_fallback_deduplicates_names_case_insensitively() {
    let populated = corpus(vec![
        source("docs/SAD.md", "one", "# Alpha"),
        source("docs/sad.md", "two", "# Beta"),
    ]);
    let structure = fallback_structure(&populated);
    assert_eq!(structure.components.len(), 1);
}

#[test]
fn test_fallback_sanitises_and_empties() {
    let populated = corpus(vec![
        source("docs/My Design v2.md", "one", ""),
        source("docs/.md", "two", ""),
        source("https://example.gov/?page=home", "three", ""),
    ]);
    let structure = fallback_structure(&populated);
    // "My Design v2" sanitises to "My-Design-v2"; ".md" and "/" yield no name.
    assert_eq!(structure.components.len(), 1);
    assert_eq!(structure.components[0].name, "My-Design-v2");
    assert!(structure.components[0].responsibilities.is_empty());
    // The fallback never fabricates interfaces/stores/relationships.
    assert!(structure.interfaces.is_empty());
    assert!(structure.data_stores.is_empty());
    assert!(structure.external_dependencies.is_empty());
    assert!(structure.relationships.is_empty());
}

#[test]
fn test_fallback_on_empty_corpus_is_empty_structure() {
    let structure = fallback_structure(&corpus(Vec::new()));
    assert!(structure.from_fallback);
    assert!(structure.components.is_empty());
    assert!(structure.is_empty());
}

#[test]
fn test_fallback_never_panics_on_minimal_corpus() {
    // One empty-content source with an unnameable locator: the rescue path
    // used when acquisition produced a bare, contentless corpus.
    let mut minimal = corpus(Vec::new());
    minimal.excluded.push(ExcludedSource {
        url: "https://blocked.example.gov/".to_string(),
        reason: ExclusionReason::NoContent,
    });
    minimal.sources.push(source("/", "root", ""));
    let structure = fallback_structure(&minimal);
    assert!(structure.from_fallback);
    assert!(structure.components.is_empty());
}

// ===========================================================================
// ArchitectureStructure helpers
// ===========================================================================

#[test]
fn test_is_empty_only_when_all_lists_empty() {
    assert!(ArchitectureStructure::default().is_empty());
    let parsed = parse_architecture_response(
        r#"{"relationships": [{"from": "a", "to": "b", "kind": "calls", "detail": ""}]}"#,
    )
    .expect("relationships-only JSON must parse");
    assert!(!parsed.is_empty());
}
