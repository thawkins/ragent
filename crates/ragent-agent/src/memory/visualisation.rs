//! Memory visualisation data generation.
//!
//! Provides data structures and functions for generating visualisation-friendly
//! JSON representations of memory data, suitable for rendering in TUI panels
//! or HTTP API responses.
//!
//! # Visualisation types
//!
//! - **Category graph**: nodes and edges showing memory categories and their relationships.
//! - **Tag cloud**: tags with their frequency counts.
//! - **Access heatmap**: memories ranked by access count and recency.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::storage::{MemoryRow, Storage};

// FR-010: The tag map type used by batch-fetched visualisation functions.
// Inlined as `HashMap<i64, Vec<String>>` throughout — the `implicit_hasher`
// lint is intentionally suppressed because `Storage::get_all_memory_tags`
// returns exactly this concrete type and all callers are internal.

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

/// A tag with its frequency count for tag cloud visualisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagCloudEntry {
    /// The tag string.
    pub tag: String,
    /// Number of memories with this tag.
    pub count: usize,
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
    /// Tag cloud.
    pub tag_cloud: TagCloud,
    /// Access pattern heatmap.
    pub heatmap: AccessHeatmap,
}

// ── Generation functions ─────────────────────────────────────────────────────

/// Generate the complete visualisation data for structured memories.
///
/// # Arguments
///
/// * `storage` - SQLite storage backend.
///
/// # Returns
///
/// A `VisualisationData` struct containing all visualisation components.
///
/// # Performance (FR-010)
///
/// Tags for all memories are fetched in a single SQL query
/// (`get_all_memory_tags`) rather than one query per memory row.
/// The memory list is also fetched once and shared across all
/// three sub-views (graph, tag cloud, heatmap).
pub fn generate_visualisation(storage: &Storage) -> anyhow::Result<VisualisationData> {
    // FR-010: fetch memories and tags once for all three sub-views.
    let memories = storage.list_memories("", 10_000)?;
    let all_tags = storage.get_all_memory_tags()?;

    let graph = generate_graph(&memories, &all_tags);
    let tag_cloud = generate_tag_cloud(&memories, &all_tags);
    let heatmap = generate_heatmap(&memories);

    Ok(VisualisationData {
        graph,
        tag_cloud,
        heatmap,
    })
}

/// Generate a memory category graph.
///
/// Creates nodes for each category, tags, and top memories. Creates edges
/// connecting memories to their categories and tags, and categories to
/// frequently co-occurring tags.
///
/// # Arguments
///
/// * `memories` - Pre-fetched memory rows.
/// * `all_tags` - Pre-fetched tag map (memory_id → tags), obtained from a
///   single `get_all_memory_tags` query (FR-010).
#[allow(clippy::implicit_hasher)]
pub fn generate_graph(memories: &[MemoryRow], all_tags: &HashMap<i64, Vec<String>>) -> MemoryGraph {
    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();

    // Category nodes — single pass per category (count + sum together).
    for cat in crate::memory::store::MEMORY_CATEGORIES {
        let (count, conf_sum) = memories
            .iter()
            .filter(|m| m.category == *cat)
            .fold((0usize, 0.0f64), |(c, s), m| (c + 1, s + m.confidence));
        if count > 0 {
            let avg_confidence = conf_sum / count as f64;
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

    for mem in memories {
        let tags = all_tags.get(&mem.id).map(Vec::as_slice).unwrap_or(&[]);
        for tag in tags {
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

    MemoryGraph { nodes, edges }
}

/// Generate a tag cloud from structured memories.
///
/// # Arguments
///
/// * `memories` - Pre-fetched memory rows.
/// * `all_tags` - Pre-fetched tag map (memory_id → tags), obtained from a
///   single `get_all_memory_tags` query (FR-010).
#[allow(clippy::implicit_hasher)]
pub fn generate_tag_cloud(
    memories: &[MemoryRow],
    all_tags: &HashMap<i64, Vec<String>>,
) -> TagCloud {
    let mut tag_counts: HashMap<String, usize> = HashMap::new();
    for mem in memories {
        let tags = all_tags.get(&mem.id).map(Vec::as_slice).unwrap_or(&[]);
        for tag in tags {
            *tag_counts.entry(tag.clone()).or_insert(0) += 1;
        }
    }

    let mut entries: Vec<TagCloudEntry> = tag_counts
        .iter()
        .map(|tag| TagCloudEntry {
            tag: tag.0.clone(),
            count: *tag.1,
        })
        .collect();

    // Sort by count descending.
    entries.sort_by(|a, b| b.count.cmp(&a.count));

    TagCloud { tags: entries }
}

/// Generate an access pattern heatmap from structured memories.
///
/// Returns memories sorted by access count (descending) with recency info.
///
/// # Arguments
///
/// * `memories` - Pre-fetched memory rows.
pub fn generate_heatmap(memories: &[MemoryRow]) -> AccessHeatmap {
    let mut entries: Vec<AccessHeatmapEntry> = memories
        .iter()
        .map(|m| {
            let preview = ragent_types::truncate_bytes(&m.content, 200);
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

    AccessHeatmap { entries }
}
