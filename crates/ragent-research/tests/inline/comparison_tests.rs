//! Inline tests for `comparison.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

fn entity(name: &str) -> CompetitiveEntity {
    CompetitiveEntity {
        name: name.to_string(),
        category: None,
    }
}

#[test]
fn empty_entities_yields_empty_body() {
    let body = build_comparison_table_body(&[], &["pricing".into()], &[]);
    assert_eq!(body, "");
}

#[test]
fn table_includes_entities_and_criteria() {
    let entities = vec![entity("Groq"), entity("Fireworks AI")];
    let criteria = vec!["pricing".to_string(), "speed/latency".to_string()];
    let profiles = vec![
        CompetitiveProfile::new(
            &entities[0],
            "Groq offers aggressive per-token pricing and very low latency.",
        ),
        CompetitiveProfile::new(
            &entities[1],
            "Fireworks AI emphasizes batch pricing and throughput over raw latency.",
        ),
    ];
    let body = build_comparison_table_body(&entities, &criteria, &profiles);
    assert!(body.contains("## Comparison Criteria"));
    assert!(body.contains("## Comparison Table"));
    assert!(body.contains("## Entity Profiles"));
    assert!(body.contains("| Groq |"));
    assert!(body.contains("| Fireworks AI |"));
    assert!(body.contains("pricing |"));
    assert!(body.contains(" speed/latency |"));
}

#[test]
fn missing_criterion_renders_dash() {
    let entities = vec![entity("A")];
    let criteria = vec!["pricing".to_string(), "speed/latency".to_string()];
    let profiles = vec![CompetitiveProfile::new(
        &entities[0],
        "A is fast and cheap.",
    )];
    let body = build_comparison_table_body(&entities, &criteria, &profiles);
    assert!(body.contains("| - |"));
}

#[test]
fn compact_profile_summary_skips_researcher_header() {
    let summary = "# Researcher researcher-2: Research Together.ai for 'topic'\n\n\
                   Together AI competes on catalog breadth and workflow depth.";
    let cell = compact_profile_summary(summary);
    assert!(cell.starts_with("Together AI competes"));
    assert!(!cell.contains("Researcher"));
}

#[test]
fn criterion_cell_skips_researcher_header() {
    let summary = "# Researcher researcher-3: Research Groq for 'topic'\n\n\
                   Groq delivers the fastest LLM inference via custom silicon.";
    let prepared = prepare_summary(summary);
    let cell = criterion_cell(&prepared, "LLM inference");
    assert!(cell.contains("fastest LLM inference"));
    assert!(!cell.contains("Researcher"));
}

#[test]
fn dotted_criterion_does_not_truncate_mid_keyword() {
    // The end-of-sentence scan must not stop at the '.' inside the
    // matched keyword itself.
    let entities = vec![entity("Acme")];
    let criteria = vec!["pricing".to_string(), "speed/latency".to_string()];
    let profiles = vec![CompetitiveProfile::new(
        &entities[0],
        "Acme ships first-class Node.js support with a native SDK.",
    )];
    let body = build_comparison_table_body(&entities, &criteria, &profiles);
    assert!(body.contains("Node.js support with a native SDK"), "{body}");
}

#[test]
fn length_changing_lowercase_does_not_panic() {
    // U+0130 (I) lowercases to a 3-byte sequence, shifting every later
    // byte offset between the lowercased probe text and the original
    // summary; the non-ASCII keyword can then land mid-character in the
    // original. Must degrade gracefully instead of panicking.
    let entities = vec![entity("Test")];
    let criteria = vec!["pricing".to_string(), "speed/latency".to_string()];
    let profiles = vec![CompetitiveProfile::new(
        &entities[0],
        "IOmega pricing cheap.",
    )];
    let body = build_comparison_table_body(&entities, &criteria, &profiles);
    assert!(
        body.contains("Omega pricing cheap.") || body.contains("| - |"),
        "{body}"
    );
}

#[test]
fn entity_profile_preserves_newlines_and_drops_researcher_header() {
    let entities = vec![entity("Groq")];
    let criteria = vec!["pricing".to_string(), "speed/latency".to_string()];
    let summary = "# Researcher researcher-3: Research Groq for 'topic'\n\n\
                   ## Summary\n\nGroq is the fastest open-model inference provider.\n\n\
                   ## Findings\n\n- LPU silicon delivers 750 tok/s on Llama 3 8B.";
    let profiles = vec![CompetitiveProfile::new(&entities[0], summary)];
    let body = build_comparison_table_body(&entities, &criteria, &profiles);
    let idx = body.find("## Entity Profiles").expect("profiles section");
    let section = &body[idx..];
    assert!(!section.contains("Researcher"), "{section}");
    assert!(
        section.contains("#### Summary\n\nGroq is the fastest"),
        "{section}"
    );
    assert!(
        section.contains("#### Findings\n\n- LPU silicon delivers"),
        "{section}"
    );
}

#[test]
fn entity_profile_empty_summary_renders_placeholder() {
    let entities = vec![entity("Groq")];
    let profiles = vec![CompetitiveProfile::new(&entities[0], "")];
    let body = build_comparison_table_body(&entities, &["speed".to_string()], &profiles);
    assert!(body.contains("_(no researcher summary available for this entity)_"));
    assert!(
        body.contains("| - |"),
        "empty profile cell must render an em dash"
    );
}
