//! Inline tests for `entities.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn empty_topic_returns_empty_result() {
    let result = extract_entities_for_competitive_analysis("");
    assert_eq!(result.entities, Vec::new());
    assert_eq!(result.criteria, Vec::<String>::new());
    assert!(!result.inferred);
}

#[test]
fn explicit_entities_are_extracted() {
    let result = extract_entities_for_competitive_analysis(
        "Compare Fireworks AI, Together.ai, and Groq for LLM inference",
    );
    assert!(!result.inferred);
    let names: Vec<_> = result.entities.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"Fireworks AI"), "got {names:?}");
    assert!(names.contains(&"Together.ai"), "got {names:?}");
    assert!(names.contains(&"Groq"), "got {names:?}");
    assert!(result.criteria.iter().any(|c| c.contains("LLM inference")));
}

#[test]
fn explicit_entities_with_vs() {
    let result = extract_entities_for_competitive_analysis("AWS vs Azure vs Google Cloud");
    assert!(!result.inferred);
    let names: Vec<_> = result.entities.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"AWS"), "got {names:?}");
    assert!(names.contains(&"Azure"), "got {names:?}");
    assert!(names.contains(&"Google Cloud"), "got {names:?}");
}

#[test]
fn inferred_entities_when_none_named() {
    let result = extract_entities_for_competitive_analysis("Research the inference market");
    assert!(result.inferred);
    assert!(
        result.entities.len() >= 2,
        "expected at least two inferred entities, got {:?}",
        result.entities
    );
    let names: Vec<_> = result.entities.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"Fireworks AI"), "got {names:?}");
    assert!(names.contains(&"Groq"), "got {names:?}");
    assert_eq!(
        result.entities[0].category.as_deref(),
        Some("inference provider")
    );
}

#[test]
fn inferred_entities_for_vector_databases() {
    let result = extract_entities_for_competitive_analysis("Compare vector database options");
    assert!(result.inferred);
    let names: Vec<_> = result.entities.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"Pinecone"), "got {names:?}");
    assert!(names.contains(&"Qdrant"), "got {names:?}");
}

#[test]
fn comparison_criteria_detected() {
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
fn single_entity_falls_back_to_inference() {
    let result = extract_entities_for_competitive_analysis("How does Groq compare?");
    assert!(result.inferred);
    assert!(result.entities.len() >= 2);
}

#[test]
fn infer_competitive_set_caps_at_max() {
    let set = infer_competitive_set("Research the inference market", 3);
    assert_eq!(set.len(), 3);
}
