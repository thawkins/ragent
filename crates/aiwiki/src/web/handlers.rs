//! Web request handlers for AIWiki.

use crate::Aiwiki;
use tokio::fs;

/// Information about a wiki page.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PageInfo {
    /// Path to the page (relative to wiki/).
    pub path: String,
    /// Page title from frontmatter or filename.
    pub title: String,
    /// Page type (entity, concept, source, analysis).
    pub page_type: String,
    /// Last modified timestamp.
    pub modified: Option<String>,
    /// Word count.
    pub word_count: usize,
}

/// List all pages in the wiki.
pub async fn list_wiki_pages(wiki: &Aiwiki) -> crate::Result<Vec<PageInfo>> {
    let mut pages = Vec::new();
    let wiki_dir = wiki.path("wiki");

    scan_pages_recursive(&wiki_dir, &wiki_dir, &mut pages).await?;

    // Sort by path
    pages.sort_by(|a, b| a.path.cmp(&b.path));

    Ok(pages)
}

/// Recursively scan for markdown pages.
async fn scan_pages_recursive(
    base_dir: &std::path::Path,
    current_dir: &std::path::Path,
    pages: &mut Vec<PageInfo>,
) -> crate::Result<()> {
    let mut entries = fs::read_dir(current_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let metadata = entry.metadata().await?;

        if metadata.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "md" || ext == "markdown" {
                    if let Some(page) = extract_page_info(base_dir, &path).await {
                        pages.push(page);
                    }
                }
            }
        } else if metadata.is_dir() {
            // Recursive call - use Box::pin for async recursion
            let mut sub_pages = Vec::new();
            Box::pin(scan_pages_recursive(base_dir, &path, &mut sub_pages)).await?;
            pages.extend(sub_pages);
        }
    }

    Ok(())
}

/// Extract page info from a markdown file.
async fn extract_page_info(base_dir: &std::path::Path, path: &std::path::Path) -> Option<PageInfo> {
    let relative_path = path.strip_prefix(base_dir).ok()?;
    let path_str = relative_path.to_string_lossy().to_string();

    // Get file metadata
    let modified = fs::metadata(path)
        .await
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| {
            chrono::DateTime::<chrono::Local>::from(t)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        });

    // Read content to extract title and word count
    let content = match fs::read_to_string(path).await {
        Ok(c) => c,
        Err(_) => return None,
    };

    let title = extract_title(&content).unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled")
            .to_string()
    });

    let word_count = content.split_whitespace().count();

    // Determine page type from path
    let page_type = if path_str.starts_with("entities/") {
        "entity"
    } else if path_str.starts_with("concepts/") {
        "concept"
    } else if path_str.starts_with("sources/") {
        "source"
    } else if path_str.starts_with("analyses/") {
        "analysis"
    } else {
        "page"
    }
    .to_string();

    Some(PageInfo {
        path: path_str,
        title,
        page_type,
        modified,
        word_count,
    })
}

/// Extract title from markdown frontmatter or content.
fn extract_title(content: &str) -> Option<String> {
    // Try to extract from YAML frontmatter
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

    // Try to extract from first H1
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") {
            return Some(trimmed[2..].to_string());
        }
    }

    None
}

/// Get page content for editing.
pub async fn get_page_content(wiki: &Aiwiki, path: &str) -> crate::Result<Option<String>> {
    let page_path = wiki.path("wiki").join(path);

    if !page_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&page_path).await?;
    Ok(Some(content))
}

/// Save page content.
pub async fn save_page_content(wiki: &Aiwiki, path: &str, content: &str) -> crate::Result<()> {
    let page_path = wiki.path("wiki").join(path);

    // Ensure parent directory exists
    if let Some(parent) = page_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    // Update the 'updated' timestamp in frontmatter if present
    let updated_content = update_timestamp(content);

    fs::write(&page_path, updated_content).await?;
    Ok(())
}

/// Get all unique entity types with counts.
pub async fn get_entity_types(wiki: &Aiwiki) -> crate::Result<Vec<(EntityTypeInfo, usize)>> {
    let entities_dir = wiki.path("wiki").join("entities");
    if !entities_dir.exists() {
        return Ok(Vec::new());
    }

    let mut type_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    let mut entries = tokio::fs::read_dir(&entities_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            if let Some(entity_type) = extract_entity_type(&path).await {
                *type_counts.entry(entity_type).or_insert(0) += 1;
            }
        }
    }

    // Convert to sorted vec
    let mut result: Vec<(EntityTypeInfo, usize)> = type_counts
        .into_iter()
        .map(|(entity_type, count)| {
            (
                EntityTypeInfo {
                    name: entity_type.clone(),
                    slug: entity_type.to_lowercase().replace(' ', "-"),
                },
                count,
            )
        })
        .collect();

    result.sort_by(|a, b| a.0.name.cmp(&b.0.name));
    Ok(result)
}

/// Information about an entity type.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EntityTypeInfo {
    /// Display name of the entity type.
    pub name: String,
    /// URL-safe slug for the type.
    pub slug: String,
}

/// Extract entity_type from frontmatter of an entity markdown file.
async fn extract_entity_type(path: &std::path::Path) -> Option<String> {
    let content = match tokio::fs::read_to_string(path).await {
        Ok(c) => c,
        Err(_) => return None,
    };

    // Look for entity_type in frontmatter
    if content.starts_with("---") {
        if let Some(end) = content.find("\n---\n") {
            let frontmatter = &content[4..end];
            for line in frontmatter.lines() {
                if let Some(value) = line.strip_prefix("entity_type:") {
                    let entity_type = value
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string();
                    if !entity_type.is_empty() {
                        return Some(entity_type);
                    }
                }
            }
        }
    }

    None
}

/// Get all entities of a specific type.
pub async fn get_entities_by_type(
    wiki: &Aiwiki,
    entity_type: &str,
) -> crate::Result<Vec<PageInfo>> {
    let wiki_dir = wiki.path("wiki");
    let entities_dir = wiki_dir.join("entities");
    if !entities_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entities = Vec::new();
    let mut entries = tokio::fs::read_dir(&entities_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("md") {
            if let Some(file_entity_type) = extract_entity_type(&path).await {
                // Case-insensitive comparison
                if file_entity_type.to_lowercase() == entity_type.to_lowercase() {
                    if let Some(page_info) = extract_page_info(&wiki_dir, &path).await {
                        entities.push(page_info);
                    }
                }
            }
        }
    }

    // Sort by title
    entities.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(entities)
}

/// Data for the entity page navigation sidebar.
#[derive(Debug, Clone)]
pub struct EntitySidebarData {
    /// Entity types with their counts, sorted alphabetically.
    pub types: Vec<(String, usize)>,
    /// Initial letters that have entities, with counts, sorted alphabetically.
    pub letters: Vec<(char, usize)>,
    /// All entity entries (title, path, entity_type, initial letter), sorted by title.
    pub entities: Vec<EntitySidebarEntry>,
}

/// A single entity entry for the sidebar.
#[derive(Debug, Clone)]
pub struct EntitySidebarEntry {
    /// Display title.
    pub title: String,
    /// Page path relative to wiki/.
    pub path: String,
    /// Entity type from frontmatter.
    pub entity_type: String,
}

/// Build sidebar navigation data for entity pages.
///
/// Scans all entity markdown files to collect types and initial letters.
pub async fn get_entity_sidebar_data(wiki: &Aiwiki) -> crate::Result<EntitySidebarData> {
    let entities_dir = wiki.path("wiki").join("entities");
    if !entities_dir.exists() {
        return Ok(EntitySidebarData {
            types: Vec::new(),
            letters: Vec::new(),
            entities: Vec::new(),
        });
    }

    let mut entries = Vec::new();
    let mut dir = tokio::fs::read_dir(&entities_dir).await?;

    while let Some(entry) = dir.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }

        let content = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(_) => continue,
        };

        let title = extract_title(&content).unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string()
        });

        let entity_type =
            extract_entity_type_from_content(&content).unwrap_or_else(|| "unknown".to_string());

        let rel_path = format!(
            "entities/{}",
            path.file_name().and_then(|n| n.to_str()).unwrap_or("")
        );

        entries.push(EntitySidebarEntry {
            title,
            path: rel_path,
            entity_type,
        });
    }

    entries.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));

    // Collect type counts
    let mut type_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for e in &entries {
        *type_counts.entry(e.entity_type.clone()).or_insert(0) += 1;
    }
    let mut types: Vec<(String, usize)> = type_counts.into_iter().collect();
    types.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    // Collect initial letter counts
    let mut letter_counts: std::collections::HashMap<char, usize> =
        std::collections::HashMap::new();
    for e in &entries {
        if let Some(ch) = e.title.chars().next() {
            let upper = ch.to_uppercase().next().unwrap_or(ch);
            *letter_counts.entry(upper).or_insert(0) += 1;
        }
    }
    let mut letters: Vec<(char, usize)> = letter_counts.into_iter().collect();
    letters.sort_by_key(|&(ch, _)| ch);

    Ok(EntitySidebarData {
        types,
        letters,
        entities: entries,
    })
}

/// Extract entity_type from content string (without reading file).
fn extract_entity_type_from_content(content: &str) -> Option<String> {
    if content.starts_with("---") {
        if let Some(end) = content.find("\n---\n") {
            let frontmatter = &content[4..end];
            for line in frontmatter.lines() {
                if let Some(value) = line.strip_prefix("entity_type:") {
                    let entity_type = value
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string();
                    if !entity_type.is_empty() {
                        return Some(entity_type);
                    }
                }
            }
        }
    }
    None
}

/// Update the 'updated' timestamp in frontmatter.
fn update_timestamp(content: &str) -> String {
    if !content.starts_with("---") {
        return content.to_string();
    }

    if let Some(end) = content.find("\n---\n") {
        let frontmatter = &content[..end + 5];
        let body = &content[end + 5..];

        let mut updated_frontmatter = String::new();
        let mut found_updated = false;

        for line in frontmatter.lines() {
            if line.starts_with("updated:") {
                updated_frontmatter.push_str(&format!(
                    "updated: {}",
                    chrono::Local::now().format("%Y-%m-%d %H:%M")
                ));
                found_updated = true;
            } else {
                updated_frontmatter.push_str(line);
            }
            updated_frontmatter.push('\n');
        }

        // Add updated field if not present
        if !found_updated {
            updated_frontmatter.insert_str(
                4,
                &format!(
                    "updated: {}\n",
                    chrono::Local::now().format("%Y-%m-%d %H:%M")
                ),
            );
        }

        return updated_frontmatter + body;
    }

    content.to_string()
}
