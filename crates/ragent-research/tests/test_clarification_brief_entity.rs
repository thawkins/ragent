//! Crate-level tests for the opendeepresearch clarification, brief generation,
//! and entity extraction APIs (T-019, FR-005, FR-011).

use ragent_research::{
    CompetitiveEntity, OutputFormat, ResearchMode, extract_comparison_criteria,
    extract_entities_for_competitive_analysis, generate_research_brief, infer_competitive_set,
    needs_clarification,
};

// ---------------------------------------------------------------------------
// Clarification tests (FR-005 / FR-017)
// ---------------------------------------------------------------------------

#[test]
fn clarification_empty_topic() {
    let q = needs_clarification("");
    assert!(q.is_some(), "empty topic should always be clarified");
}

#[test]
fn clarification_short_topic() {
    let q = needs_clarification("rust");
    assert!(q.is_some(), "single-word topic should be clarified");
    let text = q.unwrap();
    assert!(
        text.contains("narrow this down"),
        "unexpected short-topic question: {text}"
    );
}

#[test]
fn clarification_broad_phrase() {
    assert!(
        needs_clarification("research the inference market").is_some(),
        "broad market phrase should be clarified"
    );
    assert!(
        needs_clarification("tell me about Rust").is_some(),
        "'tell me about' should be clarified"
    );
}

#[test]
fn clarification_specific_comparison_is_clear() {
    assert!(
        needs_clarification("Compare Fireworks AI and Together.ai for LLM inference").is_none(),
        "explicit comparison with two entities should not need clarification"
    );
}

#[test]
fn clarification_year_makes_topic_concrete() {
    assert!(
        needs_clarification("Rust async runtimes in 2024").is_none(),
        "year-bearing topic should be concrete"
    );
}

// ---------------------------------------------------------------------------
// Research-brief tests (FR-004 / FR-009)
// ---------------------------------------------------------------------------

#[test]
fn brief_empty_topic_returns_empty() {
    assert_eq!(generate_research_brief("   ", None, None), "");
}

#[test]
fn brief_contains_required_sections() {
    let brief = generate_research_brief("Rust async runtimes", None, None);
    for expected in [
        "**Mission:**",
        "**Approach:**",
        "**Output expectation:**",
        "**Success criteria:**",
    ] {
        assert!(
            brief.contains(expected),
            "brief missing section {expected}:\n{brief}"
        );
    }
}

#[test]
fn brief_supervisor_mode_mentions_graph() {
    let brief = generate_research_brief(
        "Explain Rust async runtimes",
        Some(ResearchMode::Supervisor),
        None,
    );
    assert!(
        brief.contains("supervisor/researcher graph"),
        "supervisor brief missing graph phrase:\n{brief}"
    );
    assert!(
        brief.contains("parallel researchers"),
        "supervisor brief missing parallel researchers:\n{brief}"
    );
}

#[test]
fn brief_competitive_mode_mentions_comparison_table() {
    let brief = generate_research_brief(
        "Compare Fireworks AI, Together.ai, and Groq for LLM inference",
        Some(ResearchMode::Competitive),
        None,
    );
    assert!(
        brief.contains("competitive-analysis mode"),
        "competitive brief missing mode phrase:\n{brief}"
    );
    assert!(
        brief.contains("comparison table"),
        "competitive brief missing comparison table:\n{brief}"
    );
}

#[test]
fn brief_comparison_table_format_shortens_artifact() {
    let brief = generate_research_brief(
        "Compare Fireworks AI and Together.ai",
        Some(ResearchMode::Competitive),
        Some(OutputFormat::ComparisonTable),
    );
    assert!(
        brief.contains("per-entity profiles"),
        "comparison-table brief missing profiles:\n{brief}"
    );
    assert!(
        brief.contains("Markdown comparison table"),
        "comparison-table brief missing table phrase:\n{brief}"
    );
}

#[test]
fn brief_entities_surfaced_in_scope_note() {
    let brief = generate_research_brief(
        "Compare Fireworks AI, Together.ai, and Groq for LLM inference",
        None,
        None,
    );
    assert!(
        brief.contains("Fireworks AI"),
        "brief missing Fireworks AI entity:\n{brief}"
    );
    assert!(
        brief.contains("Groq"),
        "brief missing Groq entity:\n{brief}"
    );
    assert!(
        brief.contains("Key entities to cover:") || brief.contains("Scope note:"),
        "brief missing entity/scope clause:\n{brief}"
    );
}

// ---------------------------------------------------------------------------
// Entity-extraction tests (FR-006 / FR-011)
// ---------------------------------------------------------------------------

#[test]
fn entity_extraction_empty_topic() {
    let result = extract_entities_for_competitive_analysis("");
    assert_eq!(result.entities, Vec::<CompetitiveEntity>::new());
    assert_eq!(result.criteria, Vec::<String>::new());
    assert!(!result.inferred);
}

#[test]
fn entity_extraction_explicit_list() {
    let result = extract_entities_for_competitive_analysis(
        "Compare Fireworks AI, Together.ai, and Groq for LLM inference",
    );
    assert!(!result.inferred, "explicit entities should not be inferred");
    let names: Vec<&str> = result.entities.iter().map(|e| e.name.as_str()).collect();
    for expected in ["Fireworks AI", "Together.ai", "Groq"] {
        assert!(
            names.contains(&expected),
            "missing entity {expected}, got {names:?}"
        );
    }
    assert!(
        result.criteria.iter().any(|c| c.contains("LLM inference")),
        "criteria missing LLM inference: {:?}",
        result.criteria
    );
}

#[test]
fn entity_extraction_explicit_vs_list() {
    let result = extract_entities_for_competitive_analysis("AWS vs Azure vs Google Cloud");
    assert!(!result.inferred);
    let names: Vec<&str> = result.entities.iter().map(|e| e.name.as_str()).collect();
    for expected in ["AWS", "Azure", "Google Cloud"] {
        assert!(
            names.contains(&expected),
            "missing entity {expected}, got {names:?}"
        );
    }
}

#[test]
fn entity_extraction_infers_from_category() {
    let result = extract_entities_for_competitive_analysis("Research the inference market");
    assert!(result.inferred, "no explicit entities => inferred set");
    assert!(
        result.entities.len() >= 2,
        "expected at least two inferred entities, got {:?}",
        result.entities
    );
    let names: Vec<&str> = result.entities.iter().map(|e| e.name.as_str()).collect();
    assert!(
        names.contains(&"Fireworks AI"),
        "inference missing Fireworks AI, got {names:?}"
    );
    assert!(
        names.contains(&"Groq"),
        "inference missing Groq, got {names:?}"
    );
    assert_eq!(
        result.entities[0].category.as_deref(),
        Some("inference provider"),
        "category should be inference provider, got {:?}",
        result.entities[0].category
    );
}

#[test]
fn entity_extraction_infers_vector_databases() {
    let result = extract_entities_for_competitive_analysis("Compare vector database options");
    assert!(result.inferred);
    let names: Vec<&str> = result.entities.iter().map(|e| e.name.as_str()).collect();
    assert!(
        names.contains(&"Pinecone"),
        "vector-db inference missing Pinecone, got {names:?}"
    );
    assert!(
        names.contains(&"Qdrant"),
        "vector-db inference missing Qdrant, got {names:?}"
    );
}

#[test]
fn entity_extraction_detects_criteria() {
    let result = extract_entities_for_competitive_analysis(
        "Compare AWS, Azure, and Google Cloud for pricing and speed",
    );
    let criteria = result.criteria.join(", ");
    assert!(
        criteria.contains("pricing") || criteria.contains("speed/latency"),
        "criteria missing pricing/speed: {criteria}"
    );
}

#[test]
fn entity_extraction_single_entity_falls_back_to_inference() {
    let result = extract_entities_for_competitive_analysis("How does Groq compare?");
    assert!(result.inferred);
    assert!(
        result.entities.len() >= 2,
        "single entity should fall back to inferred set, got {:?}",
        result.entities
    );
}

#[test]
fn entity_extraction_caps_inferred_set() {
    let set = infer_competitive_set("Research the inference market", 3);
    assert_eq!(
        set.len(),
        3,
        "inferred set should be capped at max_entities, got {:?}",
        set
    );
}

#[test]
fn entity_extraction_criteria_include_price_dimensions() {
    let criteria = extract_comparison_criteria(
        "Compare AWS, Azure, and Google Cloud for pricing, latency, and support",
    );
    let joined = criteria.join(", ");
    assert!(
        joined.contains("pricing") || joined.contains("cost"),
        "pricing dimension not detected: {joined}"
    );
    assert!(
        joined.contains("speed/latency") || joined.contains("latency"),
        "latency dimension not detected: {joined}"
    );
}

#[test]
fn entity_extraction_result_struct_clone_and_eq() {
    let a = extract_entities_for_competitive_analysis("Compare AWS and Azure");
    let b = a.clone();
    assert_eq!(a, b);
}
