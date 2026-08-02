//! External tests for `tests` from `crates/ragent-agent/src/memory/visualisation.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_agent::memory::visualisation::*;

#[test]
fn test_graph_node_serialisation() {
    let node = GraphNode {
        id: "cat:fact".to_string(),
        label: "fact".to_string(),
        node_type: "category".to_string(),
        count: 5,
        avg_confidence: Some(0.85),
    };
    let json = serde_json::to_string(&node).unwrap();
    assert!(json.contains("\"cat:fact\""));
    assert!(json.contains("\"type\":\"category\""));
}

#[test]
fn test_tag_cloud_sorting() {
    // Test that sort_by count descending works.
    let mut tags = [
        TagCloudEntry {
            tag: "rust".to_string(),
            count: 10,
        },
        TagCloudEntry {
            tag: "python".to_string(),
            count: 3,
        },
        TagCloudEntry {
            tag: "debugging".to_string(),
            count: 7,
        },
    ];
    tags.sort_by(|a, b| b.count.cmp(&a.count));
    assert_eq!(tags[0].tag, "rust");
    assert_eq!(tags[1].tag, "debugging");
    assert_eq!(tags[2].tag, "python");
}

#[test]
fn test_visualisation_data_structure() {
    let data = VisualisationData {
        graph: MemoryGraph {
            nodes: Vec::new(),
            edges: Vec::new(),
        },
        tag_cloud: TagCloud { tags: Vec::new() },
        heatmap: AccessHeatmap {
            entries: Vec::new(),
        },
    };
    let json = serde_json::to_string_pretty(&data).unwrap();
    assert!(json.contains("\"graph\""));
    assert!(json.contains("\"tag_cloud\""));
    assert!(json.contains("\"heatmap\""));
}
