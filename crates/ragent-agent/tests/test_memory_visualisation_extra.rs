//! Tests-only coverage for memory visualisation (`generate_graph`,
//! `generate_tag_cloud`, `generate_heatmap`, `generate_visualisation`) driven by
//! real in-memory storage rows.

use std::collections::HashMap;
use std::sync::Arc;

use ragent_agent::memory::visualisation::{generate_graph, generate_heatmap, generate_tag_cloud};
use ragent_agent::storage::{MemoryRow, Storage};

fn row(id: i64, content: &str, category: &str, confidence: f64, access: i64) -> MemoryRow {
    MemoryRow {
        id,
        content: content.to_string(),
        category: category.to_string(),
        source: "test".to_string(),
        confidence,
        project: String::new(),
        session_id: "s1".to_string(),
        created_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
        access_count: access,
        last_accessed: None,
    }
}

#[test]
fn generate_graph_builds_category_tag_nodes_and_edges() {
    let memories = vec![
        row(1, "a", "fact", 0.8, 1),
        row(2, "b", "fact", 0.6, 2),
        row(3, "c", "pattern", 0.9, 3),
    ];
    let mut tags = HashMap::new();
    tags.insert(1, vec!["rust".to_string(), "testing".to_string()]);
    tags.insert(2, vec!["rust".to_string()]);
    tags.insert(3, vec!["testing".to_string()]);

    let graph = generate_graph(&memories, &tags);

    let node_ids: Vec<&str> = graph.nodes.iter().map(|n| n.id.as_str()).collect();
    assert!(node_ids.contains(&"cat:fact"));
    assert!(node_ids.contains(&"cat:pattern"));
    assert!(node_ids.contains(&"tag:rust"), "nodes: {node_ids:?}");
    assert!(node_ids.contains(&"tag:testing"));

    // The `fact` category node aggregates two memories and averages confidence.
    let fact = graph
        .nodes
        .iter()
        .find(|n| n.id == "cat:fact")
        .expect("fact node");
    assert_eq!(fact.count, 2);
    assert!((fact.avg_confidence.expect("avg") - 0.7).abs() < 1e-9);

    // Tag nodes carry a count but no confidence.
    let rust = graph
        .nodes
        .iter()
        .find(|n| n.id == "tag:rust")
        .expect("rust node");
    assert_eq!(rust.count, 2);
    assert!(rust.avg_confidence.is_none());

    // Edges: rust -> fact (2 links) and testing -> pattern (1 link).
    assert!(
        graph
            .edges
            .iter()
            .any(|e| { e.source == "tag:rust" && e.target == "cat:fact" && e.weight == Some(2) })
    );
    assert!(graph.edges.iter().any(|e| {
        e.source == "tag:testing" && e.target == "cat:pattern" && e.weight == Some(1)
    }));
}

#[test]
fn generate_graph_on_empty_input_is_empty() {
    let graph = generate_graph(&[], &HashMap::new());
    assert!(graph.nodes.is_empty());
    assert!(graph.edges.is_empty());
}

#[test]
fn generate_tag_cloud_orders_by_count_descending() {
    let memories = vec![
        row(1, "a", "fact", 0.5, 0),
        row(2, "b", "fact", 0.5, 0),
        row(3, "c", "fact", 0.5, 0),
    ];
    let mut tags = HashMap::new();
    tags.insert(1, vec!["common".to_string()]);
    tags.insert(2, vec!["common".to_string()]);
    tags.insert(3, vec!["common".to_string(), "rare".to_string()]);

    let cloud = generate_tag_cloud(&memories, &tags);
    assert_eq!(cloud.tags.len(), 2);
    assert_eq!(cloud.tags[0].tag, "common");
    assert_eq!(cloud.tags[0].count, 3);
    assert_eq!(cloud.tags[1].tag, "rare");
    assert_eq!(cloud.tags[1].count, 1);
}

#[test]
fn generate_heatmap_orders_by_access_count_and_previews_content() {
    let memories = vec![
        row(1, "low access", "fact", 0.5, 1),
        row(2, "high access", "pattern", 0.9, 42),
    ];
    let heatmap = generate_heatmap(&memories);
    assert_eq!(heatmap.entries.len(), 2);
    assert_eq!(heatmap.entries[0].id, 2, "highest access first");
    assert_eq!(heatmap.entries[0].access_count, 42);
    assert_eq!(heatmap.entries[0].content_preview, "high access");
    assert_eq!(heatmap.entries[1].id, 1);
}

#[test]
fn generate_visualisation_wires_all_three_views_from_storage() {
    let storage = Storage::open_in_memory().expect("storage");
    storage
        .create_memory(
            "rust project fact",
            "fact",
            "test",
            0.8,
            "",
            "s1",
            &["rust".to_string()],
        )
        .expect("m1");
    storage
        .create_memory(
            "rust pattern",
            "pattern",
            "test",
            0.7,
            "",
            "s1",
            &["rust".to_string()],
        )
        .expect("m2");

    let data = ragent_agent::memory::visualisation::generate_visualisation(&storage)
        .expect("visualisation");
    assert!(!data.graph.nodes.is_empty(), "graph nodes expected");
    assert!(!data.tag_cloud.tags.is_empty(), "tag cloud expected");
    assert_eq!(data.heatmap.entries.len(), 2);
    // The `rust` tag links both memories to their categories.
    assert!(data.graph.nodes.iter().any(|n| n.id == "tag:rust"));
}

#[test]
fn generate_visualisation_on_empty_storage_is_empty() {
    let storage = Arc::new(Storage::open_in_memory().expect("storage"));
    let data = ragent_agent::memory::visualisation::generate_visualisation(&storage)
        .expect("visualisation");
    assert!(data.graph.nodes.is_empty());
    assert!(data.tag_cloud.tags.is_empty());
    assert!(data.heatmap.entries.is_empty());
}
