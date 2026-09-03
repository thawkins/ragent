#![allow(clippy::assert_is_empty)]
//! External tests for `tests` from `crates/ragent-agent/src/memory/knowledge_graph.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_agent::memory::knowledge_graph::*;

#[test]
fn test_extract_known_languages() {
    let result = extract_entities(
        "This project uses Rust and TypeScript for development",
        "fact",
        &[],
    );
    let names: Vec<&str> = result.entities.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"Rust"));
    assert!(names.contains(&"TypeScript"));
}

#[test]
fn test_extract_known_tools() {
    let result = extract_entities("We deploy with Docker and Kubernetes", "fact", &[]);
    let names: Vec<&str> = result.entities.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"Docker"));
    assert!(names.contains(&"Kubernetes"));
}

#[test]
fn test_infer_uses_relationship() {
    let result = extract_entities("Rust uses Tokio for async", "fact", &[]);
    // Should have at least one relationship connecting Rust and Tokio.
    assert!(!result.relationships.is_empty());
}

#[test]
fn test_category_affects_relationship_type() {
    let result_pref = extract_entities("Rust Docker", "preference", &[]);
    let result_err = extract_entities("Rust Docker", "error", &[]);

    // Preference category should produce "prefers" relationships.
    if !result_pref.relationships.is_empty() {
        assert_eq!(
            result_pref.relationships[0].relation_type,
            RelationType::Prefers
        );
    }
    // Error category should produce "avoids" relationships.
    if !result_err.relationships.is_empty() {
        assert_eq!(
            result_err.relationships[0].relation_type,
            RelationType::Avoids
        );
    }
}

#[test]
fn test_entity_type_roundtrip() {
    let types = vec![
        EntityType::Project,
        EntityType::Tool,
        EntityType::Language,
        EntityType::Pattern,
        EntityType::Person,
        EntityType::Concept,
    ];
    for t in types {
        assert_eq!(EntityType::from_str(t.as_str()), Some(t));
    }
}

#[test]
fn test_relation_type_roundtrip() {
    let types = vec![
        RelationType::Uses,
        RelationType::Prefers,
        RelationType::DependsOn,
        RelationType::Avoids,
        RelationType::RelatedTo,
    ];
    for t in types {
        assert_eq!(RelationType::from_str_lossy(t.as_str()), t);
    }
}

#[test]
fn test_empty_content() {
    let result = extract_entities("", "fact", &[]);
    assert!(result.entities.is_empty());
    assert!(result.relationships.is_empty());
}

#[test]
fn test_pattern_extraction() {
    let result = extract_entities(
        "We follow the TDD pattern and clean architecture convention",
        "pattern",
        &[],
    );
    assert!(
        result
            .entities
            .iter()
            .any(|e| e.entity_type == EntityType::Pattern),
        "should extract pattern entities"
    );
}

#[test]
fn test_knowledge_graph_serialisation() {
    let graph = KnowledgeGraph {
        entities: vec![Entity {
            id: 1,
            name: "Rust".to_string(),
            entity_type: "language".to_string(),
            mention_count: 5,
            created_at: "2025-07-15T10:30:00Z".to_string(),
            updated_at: "2025-07-15T10:30:00Z".to_string(),
        }],
        relationships: vec![Relationship {
            id: 1,
            source_id: 1,
            target_id: 2,
            relation_type: "uses".to_string(),
            confidence: 0.8,
            source_memory_id: Some(42),
            created_at: "2025-07-15T10:30:00Z".to_string(),
        }],
    };
    let json = serde_json::to_string_pretty(&graph).unwrap();
    assert!(json.contains("\"entities\""));
    assert!(json.contains("\"relationships\""));
    assert!(json.contains("Rust"));
}
