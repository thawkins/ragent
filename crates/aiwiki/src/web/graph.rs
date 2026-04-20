//! Graph visualization data for AIWiki.

use crate::Aiwiki;
use serde::Serialize;

/// Node in the graph.
#[derive(Debug, Clone, Serialize)]
pub struct GraphNode {
    /// Unique ID for the node.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Node type (entity, concept, source, analysis, index).
    pub node_type: String,
    /// Size for visualization.
    pub size: u32,
    /// URL for navigation.
    pub url: Option<String>,
}

/// Link between nodes.
#[derive(Debug, Clone, Serialize)]
pub struct GraphLink {
    /// Source node ID.
    pub source: String,
    /// Target node ID.
    pub target: String,
    /// Link weight/strength.
    pub value: u32,
}

/// Complete graph data.
#[derive(Debug, Clone, Serialize)]
pub struct GraphData {
    /// All nodes in the graph.
    pub nodes: Vec<GraphNode>,
    /// All links between nodes.
    pub links: Vec<GraphLink>,
}

/// Build the page graph for visualization.
pub async fn build_graph(wiki: &Aiwiki, filter: Option<&str>) -> GraphData {
    let wiki_dir = wiki.path("wiki");
    let mut nodes = Vec::new();
    let mut links = Vec::new();
    let mut node_ids = std::collections::HashSet::new();

    // Find all markdown files
    let mut files = Vec::new();
    if let Err(_) = scan_files_recursive(&wiki_dir, &wiki_dir, &mut files).await {
        return GraphData {
            nodes: Vec::new(),
            links: Vec::new(),
        };
    }

    // Create nodes for each page
    for file_path in &files {
        if let Some(node) = create_node_from_path(file_path, &wiki_dir, filter).await {
            node_ids.insert(node.id.clone());
            nodes.push(node);
        }
    }

    // Extract links between pages
    for file_path in &files {
        if let Some(file_links) = extract_links_from_file(file_path, &wiki_dir).await {
            for target_path in file_links {
                let source_id = path_to_id(file_path, &wiki_dir);
                let target_id = target_path;

                if node_ids.contains(&source_id) && node_ids.contains(&target_id) {
                    links.push(GraphLink {
                        source: source_id,
                        target: target_id,
                        value: 1,
                    });
                }
            }
        }
    }

    // Deduplicate links
    links.sort_by(|a, b| a.source.cmp(&b.source).then(a.target.cmp(&b.target)));
    links.dedup_by(|a, b| a.source == b.source && a.target == b.target);

    GraphData { nodes, links }
}

/// Scan for markdown files.
async fn scan_files_recursive(
    base_dir: &std::path::Path,
    current_dir: &std::path::Path,
    files: &mut Vec<std::path::PathBuf>,
) -> crate::Result<()> {
    let mut entries = tokio::fs::read_dir(current_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let metadata = entry.metadata().await?;

        if metadata.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "md" || ext == "markdown" {
                    files.push(path);
                }
            }
        } else if metadata.is_dir() {
            let mut sub_files = Vec::new();
            Box::pin(scan_files_recursive(base_dir, &path, &mut sub_files)).await?;
            files.extend(sub_files);
        }
    }

    Ok(())
}

/// Create a graph node from a file path.
async fn create_node_from_path(
    file_path: &std::path::Path,
    base_dir: &std::path::Path,
    filter: Option<&str>,
) -> Option<GraphNode> {
    let relative_path = file_path.strip_prefix(base_dir).ok()?;
    let id = relative_path
        .to_string_lossy()
        .to_string()
        .replace('/', "-");
    let path_str = relative_path.to_string_lossy().to_string();

    // Determine node type from path
    let node_type = if path_str.starts_with("entities/") {
        "entity"
    } else if path_str.starts_with("concepts/") {
        "concept"
    } else if path_str.starts_with("sources/") {
        "source"
    } else if path_str.starts_with("analyses/") {
        "analysis"
    } else if path_str == "index.md" || path_str == "README.md" {
        "index"
    } else {
        "page"
    }
    .to_string();

    // Apply filter
    if let Some(f) = filter {
        if node_type != f {
            return None;
        }
    }

    // Read content to get title
    let content = tokio::fs::read_to_string(file_path)
        .await
        .unwrap_or_default();
    let title = extract_title(&content).unwrap_or_else(|| {
        file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string()
    });

    // Size based on content length
    let size = (content.len() / 100).min(20).max(5) as u32;

    Some(GraphNode {
        id,
        label: title,
        node_type,
        size,
        url: Some(format!("/aiwiki/page/{}", path_str)),
    })
}

/// Extract links from a markdown file.
async fn extract_links_from_file(
    file_path: &std::path::Path,
    base_dir: &std::path::Path,
) -> Option<Vec<String>> {
    let content = tokio::fs::read_to_string(file_path).await.ok()?;
    let relative_path = file_path.strip_prefix(base_dir).ok()?;
    let current_dir = relative_path.parent()?;

    let mut links = Vec::new();

    // Simple link extraction
    for line in content.lines() {
        // Markdown links [text](url)
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '[' {
                // Skip to closing ]
                while let Some(c) = chars.next() {
                    if c == ']' {
                        break;
                    }
                }
                // Check for (
                if chars.peek() == Some(&'(') {
                    chars.next(); // consume (
                    let mut url = String::new();
                    while let Some(c) = chars.next() {
                        if c == ')' || c == ' ' {
                            break;
                        }
                        url.push(c);
                    }

                    // Only process internal links
                    if !url.starts_with("http") && !url.starts_with("mailto") {
                        let target = if url.starts_with('/') {
                            url.trim_start_matches('/').to_string()
                        } else {
                            // Relative path
                            current_dir.join(&url).to_string_lossy().to_string()
                        };

                        // Convert to node ID
                        links.push(target.replace('/', "-"));
                    }
                }
            }
        }
    }

    Some(links)
}

/// Convert a path to a node ID.
fn path_to_id(path: &std::path::Path, base_dir: &std::path::Path) -> String {
    path.strip_prefix(base_dir)
        .ok()
        .map(|p| p.to_string_lossy().to_string().replace('/', "-"))
        .unwrap_or_default()
}

/// Extract title from frontmatter.
fn extract_title(content: &str) -> Option<String> {
    if content.starts_with("---") {
        if let Some(end) = content.find("\n---\n") {
            let frontmatter = &content[4..end];
            for line in frontmatter.lines() {
                if let Some(value) = line.strip_prefix("title:") {
                    return Some(
                        value
                            .trim()
                            .trim_matches('"')
                            .trim_matches('\'')
                            .to_string(),
                    );
                }
            }
        }
    }

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") {
            return Some(trimmed[2..].to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_to_id() {
        let base = std::path::Path::new("/wiki");
        let path = std::path::Path::new("/wiki/concepts/rust.md");
        assert_eq!(path_to_id(path, base), "concepts-rust.md");
    }

    #[test]
    fn test_extract_title() {
        let content = "---\ntitle: My Page\n---\n\nContent.";
        assert_eq!(extract_title(content), Some("My Page".to_string()));
    }
}
