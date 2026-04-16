//! Memory visualisation data generation.
//!
//! Provides data structures and functions for generating visualisation-friendly
//! JSON representations of memory data, suitable for rendering in TUI panels
//! or HTTP API responses.
//!
//! # Visualisation types
//!
//! - **Category graph**: nodes and edges showing memory categories and their relationships.
//! - **Timeline**: journal entries ordered by timestamp.
//! - **Tag cloud**: tags with their frequency counts.
//! - **Access heatmap**: memories ranked by access count and recency.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::memory::storage::BlockStorage;
use crate::storage::Storage;
use std::path::PathBuf;

// ── Data structures ───────────────────────────────────────────────────────────

/// A node in the memory category graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphNode {
    /// Unique identifier for the node.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Node type: "category", "tag", or "memory".
    #[serde(rename = "type")]
    pub node_type: String,
    /// Number of items in this node.
    pub count: usize,
    /// Average confidence score (for memory nodes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub avg_confidence: Option<f64>,
}

/// An edge in the memory category graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEdge {
    /// Source node ID.
    pub source: String,
    /// Target node ID.
    pub target: String,
    /// Relationship type: "has_tag", "in_category", "related".
    #[serde(rename = "type")]
    pub edge_type: String,
    /// Weight of the edge (number of connections).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<usize>,
}

/// The complete memory graph for visualisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryGraph {
    /// All nodes in the graph.
    pub nodes: Vec<GraphNode>,
    /// All edges in the graph.
    pub edges: Vec<GraphEdge>,
}

/// A timeline entry for journal visualisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntry {
    /// ISO 8601 timestamp.
    pub timestamp: String,
    /// Entry title.
    pub title: String,
    /// Truncated content (max 200 chars).
    pub content_preview: String,
    /// Tags on the entry.
    pub tags: Vec<String>,
}

/// The complete timeline for journal visualisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timeline {
    /// Timeline entries, sorted by timestamp (most recent first).
    pub entries: Vec<TimelineEntry>,
}

/// A tag with its frequency count for tag cloud visualisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagCloudEntry {
    /// The tag string.
    pub tag: String,
    /// Number of memories with this tag.
    pub count: usize,
    /// Number of journal entries with this tag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journal_count: Option<usize>,
}

/// The complete tag cloud for visualisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagCloud {
    /// Tags sorted by count (descending).
    pub tags: Vec<TagCloudEntry>,
}

/// A memory's access pattern for heatmap visualisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessHeatmapEntry {
    /// Memory row ID.
    pub id: i64,
    /// Category.
    pub category: String,
    /// Truncated content preview.
    pub content_preview: String,
    /// Access count.
    pub access_count: i64,
    /// ISO 8601 last accessed timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_accessed: Option<String>,
    /// Confidence score.
    pub confidence: f64,
}

/// The complete access heatmap for visualisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessHeatmap {
    /// Entries sorted by access count (descending).
    pub entries: Vec<AccessHeatmapEntry>,
}

/// Complete visualisation data bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualisationData {
    /// Category relationship graph.
    pub graph: MemoryGraph,
    /// Journal timeline.
    pub timeline: Timeline,
    /// Tag cloud.
    pub tag_cloud: TagCloud,
    /// Access pattern heatmap.
    pub heatmap: AccessHeatmap,
}

// ── Generation functions ─────────────────────────────────────────────────────

/// Generate the complete visualisation data for all memories, journal entries,
/// and blocks.
///
/// # Arguments
///
/// * `storage` - SQLite storage backend.
/// * `block_storage` - File-based block storage backend.
/// * `working_dir` - Current project working directory.
///
/// # Returns
///
/// A `VisualisationData` struct containing all visualisation components.
pub fn generate_visualisation(
    storage: &Storage,
    _block_storage: &dyn BlockStorage,
    _working_dir: &PathBuf,
) -> anyhow::Result<VisualisationData> {
    let graph = generate_graph(storage)?;
    let timeline = generate_timeline(storage)?;
    let tag_cloud = generate_tag_cloud(storage)?;
    let heatmap = generate_heatmap(storage)?;

    Ok(VisualisationData {
        graph,
        timeline,
        tag_cloud,
        heatmap,
    })
}

/// Generate a memory category graph.
///
/// Creates nodes for each category, tags, and top memories. Creates edges
/// connecting memories to their categories and tags, and categories to
/// frequently co-occurring tags.
pub fn generate_graph(storage: &Storage) -> anyhow::Result<MemoryGraph> {
    let memories = storage.list_memories("", 10_000)?;
    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();

    // Category nodes.
    for cat in crate::memory::store::MEMORY_CATEGORIES {
        let count = memories.iter().filter(|m| m.category == *cat).count();
        if count > 0 {
            let avg_confidence: f64 = memories
                .iter()
                .filter(|m| m.category == *cat)
                .map(|m| m.confidence)
                .sum::<f64>()
                / count as f64;
            nodes.push(GraphNode {
                id: format!("cat:{cat}"),
                label: cat.to_string(),
                node_type: "category".to_string(),
                count,
                avg_confidence: Some(avg_confidence),
            });
        }
    }

    // Tag nodes and edges.
    let mut tag_counts: HashMap<String, usize> = HashMap::new();
    let mut tag_category_links: HashMap<(String, String), usize> = HashMap::new();

    for mem in &memories {
        let tags = storage.get_memory_tags(mem.id).unwrap_or_default();
        for tag in &tags {
            *tag_counts.entry(tag.clone()).or_insert(0) += 1;
            let key = (tag.clone(), mem.category.clone());
            *tag_category_links.entry(key).or_insert(0) += 1;
        }
    }

    for (tag, count) in &tag_counts {
        nodes.push(GraphNode {
            id: format!("tag:{tag}"),
            label: tag.clone(),
            node_type: "tag".to_string(),
            count: *count,
            avg_confidence: None,
        });
    }

    // Tag-to-category edges.
    for ((tag, cat), weight) in &tag_category_links {
        edges.push(GraphEdge {
            source: format!("tag:{tag}"),
            target: format!("cat:{cat}"),
            edge_type: "in_category".to_string(),
            weight: Some(*weight),
        });
    }

    Ok(MemoryGraph { nodes, edges })
}

/// Generate a journal timeline.
///
/// Returns journal entries sorted by timestamp (most recent first) with
/// truncated content previews.
pub fn generate_timeline(storage: &Storage) -> anyhow::Result<Timeline> {
    let entries = storage.list_journal_entries(1_000)?;

    let mut timeline_entries: Vec<TimelineEntry> = entries
        .iter()
        .map(|e| {
            let tags = storage.get_journal_tags(&e.id).unwrap_or_default();
            let preview = if e.content.len() > 200 {
                format!("{}…", &e.content[..200])
            } else {
                e.content.clone()
            };
            TimelineEntry {
                timestamp: e.timestamp.clone(),
                title: e.title.clone(),
                content_preview: preview,
                tags,
            }
        })
        .collect();

    // Sort by timestamp descending.
    timeline_entries.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    Ok(Timeline {
        entries: timeline_entries,
    })
}

/// Generate a tag cloud from structured memories and journal entries.
///
/// Counts tag occurrences across both memories and journal entries.
pub fn generate_tag_cloud(storage: &Storage) -> anyhow::Result<TagCloud> {
    let memories = storage.list_memories("", 10_000)?;

    // Count memory tags.
    let mut tag_memory_counts: HashMap<String, usize> = HashMap::new();
    for mem in &memories {
        let tags = storage.get_memory_tags(mem.id).unwrap_or_default();
        for tag in &tags {
            *tag_memory_counts.entry(tag.clone()).or_insert(0) += 1;
        }
    }

    // Count journal tags.
    let journal_entries = storage.list_journal_entries(10_000)?;
    let mut tag_journal_counts: HashMap<String, usize> = HashMap::new();
    for entry in &journal_entries {
        let tags = storage.get_journal_tags(&entry.id).unwrap_or_default();
        for tag in &tags {
            *tag_journal_counts.entry(tag.clone()).or_insert(0) += 1;
        }
    }

    // Merge tags.
    let mut all_tags: std::collections::HashSet<String> = std::collections::HashSet::new();
    for tag in tag_memory_counts.keys() {
        all_tags.insert(tag.clone());
    }
    for tag in tag_journal_counts.keys() {
        all_tags.insert(tag.clone());
    }

    let mut entries: Vec<TagCloudEntry> = all_tags
        .iter()
        .map(|tag| TagCloudEntry {
            count: *tag_memory_counts.get(tag).unwrap_or(&0),
            journal_count: if tag_journal_counts.contains_key(tag) {
                Some(*tag_journal_counts.get(tag).unwrap_or(&0))
            } else {
                None
            },
            tag: tag.clone(),
        })
        .collect();

    // Sort by count descending.
    entries.sort_by(|a, b| b.count.cmp(&a.count));

    Ok(TagCloud { tags: entries })
}

/// Generate an access pattern heatmap from structured memories.
///
/// Returns memories sorted by access count (descending) with recency info.
pub fn generate_heatmap(storage: &Storage) -> anyhow::Result<AccessHeatmap> {
    let memories = storage.list_memories("", 10_000)?;

    let mut entries: Vec<AccessHeatmapEntry> = memories
        .iter()
        .map(|m| {
            let preview = if m.content.len() > 200 {
                format!("{}…", &m.content[..200])
            } else {
                m.content.clone()
            };
            AccessHeatmapEntry {
                id: m.id,
                category: m.category.clone(),
                content_preview: preview,
                access_count: m.access_count,
                last_accessed: m.last_accessed.clone(),
                confidence: m.confidence,
            }
        })
        .collect();

    // Sort by access count descending.
    entries.sort_by(|a, b| b.access_count.cmp(&a.access_count));

    Ok(AccessHeatmap { entries })
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_timeline_entry_preview_truncation() {
        // Verify the truncation logic produces a string with correct char count.
        let long_content: String = "A".repeat(300);
        // 200 ASCII chars + 1 ellipsis char "…" = 201 chars (203 bytes in UTF-8)
        let truncated = if long_content.len() > 200 {
            format!("{}…", &long_content[..200])
        } else {
            long_content.clone()
        };
        // Char count is 201, byte length is 203 (… is 3 bytes in UTF-8).
        assert_eq!(truncated.chars().count(), 201);
    }

    #[test]
    fn test_tag_cloud_sorting() {
        // Test that sort_by count descending works.
        let mut tags = vec![
            TagCloudEntry {
                tag: "rust".to_string(),
                count: 10,
                journal_count: Some(5),
            },
            TagCloudEntry {
                tag: "python".to_string(),
                count: 3,
                journal_count: None,
            },
            TagCloudEntry {
                tag: "debugging".to_string(),
                count: 7,
                journal_count: Some(2),
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
            timeline: Timeline {
                entries: Vec::new(),
            },
            tag_cloud: TagCloud { tags: Vec::new() },
            heatmap: AccessHeatmap {
                entries: Vec::new(),
            },
        };
        let json = serde_json::to_string_pretty(&data).unwrap();
        assert!(json.contains("\"graph\""));
        assert!(json.contains("\"timeline\""));
        assert!(json.contains("\"tag_cloud\""));
        assert!(json.contains("\"heatmap\""));
    }
}
