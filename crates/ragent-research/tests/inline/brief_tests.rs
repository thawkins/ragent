//! Inline tests for `brief.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn empty_topic_returns_empty_brief() {
    assert_eq!(generate_research_brief("   ", None, None), "");
}

#[test]
fn brief_contains_mission_and_approach() {
    let brief = generate_research_brief("Rust async runtimes", None, None);
    assert!(brief.contains("**Mission:**"));
    assert!(brief.contains("Rust async runtimes"));
    assert!(brief.contains("**Approach:**"));
    assert!(brief.contains("**Output expectation:**"));
    assert!(brief.contains("**Success criteria:**"));
}

#[test]
fn supervisor_mode_mentions_supervisor_graph() {
    let brief = generate_research_brief(
        "Explain Rust async runtimes",
        Some(ResearchMode::Supervisor),
        None,
    );
    assert!(brief.contains("supervisor/researcher graph"));
    assert!(brief.contains("parallel researchers"));
}

#[test]
fn competitive_mode_mentions_entity_comparison() {
    let brief = generate_research_brief(
        "Compare Fireworks AI, Together.ai, and Groq for LLM inference",
        Some(ResearchMode::Competitive),
        None,
    );
    assert!(brief.contains("competitive-analysis mode"));
    assert!(brief.contains("comparison table"));
}

#[test]
fn comparison_table_format_shortens_output() {
    let brief = generate_research_brief(
        "Compare Fireworks AI and Together.ai",
        Some(ResearchMode::Competitive),
        Some(OutputFormat::ComparisonTable),
    );
    assert!(brief.contains("per-entity profiles"));
    assert!(brief.contains("Markdown comparison table"));
}

#[test]
fn extracts_entities_from_topic() {
    let brief = generate_research_brief(
        "Compare Fireworks AI, Together.ai, and Groq for LLM inference",
        None,
        None,
    );
    assert!(
        brief.contains("Key entities to cover:") || brief.contains("Scope note:"),
        "brief missing entity/scope clause:\n{brief}"
    );
    assert!(brief.contains("Fireworks AI"));
    assert!(brief.contains("Groq"));
}
