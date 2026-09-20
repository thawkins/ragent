//! Tests for the masterfetch engine-name vocabulary and academic
//! classification helper (spec `researchnoacc`, T-001; FR-003, FR-015).
//!
//! FR-003: the set of academically-classified engine names is defined in
//! exactly one place and reused for research scholarly-hit classification and
//! the research-to-`mf_search` engine-exclusion mapping.
//!
//! FR-015: non-academic engines (e.g. Wikipedia, web engines) are never
//! classified as academic, so "no papers" exclusion cannot disable them.

use ragent_tools_extended::masterfetch::search::{
    ACADEMIC_ENGINES, ENGINE_OPENALEX, ENGINE_WIKIPEDIA, is_academic_engine,
};

#[test]
fn academic_engines_contains_openalex_only() {
    assert_eq!(ACADEMIC_ENGINES, &["openalex"]);
}

#[test]
fn is_academic_engine_true_for_openalex() {
    assert!(is_academic_engine("openalex"));
    assert!(is_academic_engine(ENGINE_OPENALEX));
}

#[test]
fn is_academic_engine_false_for_non_academic_engines() {
    // FR-015: Wikipedia and every web engine must not be academic.
    for name in [
        "wikipedia",
        "langsearch",
        "tavily",
        "perplexity",
        "exa",
        "serper",
    ] {
        assert!(
            !is_academic_engine(name),
            "{name} must not be classified as academic"
        );
    }
}

#[test]
fn is_academic_engine_is_exact_and_case_sensitive() {
    assert!(!is_academic_engine("OpenAlex"));
    assert!(!is_academic_engine("openalex "));
    assert!(!is_academic_engine(""));
    assert!(!is_academic_engine("not_a_real_engine"));
}

#[test]
fn engine_name_constants_are_canonical() {
    assert_eq!(ENGINE_OPENALEX, "openalex");
    assert_eq!(ENGINE_WIKIPEDIA, "wikipedia");
}

#[test]
fn engine_name_constants_match_engine_impls() {
    // The module-root constants must alias each backend's own ENGINE_NAME so
    // the vocabulary has a single source of truth (FR-003).
    use ragent_tools_extended::masterfetch::search::openalex;
    use ragent_tools_extended::masterfetch::search::wikipedia;

    assert_eq!(ENGINE_OPENALEX, openalex::ENGINE_NAME);
    assert_eq!(ENGINE_WIKIPEDIA, wikipedia::ENGINE_NAME);
    assert!(ACADEMIC_ENGINES.contains(&openalex::ENGINE_NAME));
    assert!(!ACADEMIC_ENGINES.contains(&wikipedia::ENGINE_NAME));
}
