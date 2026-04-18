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
async fn extract_page_info(
    base_dir: &std::path::Path,
    path: &std::path::Path,
) -> Option<PageInfo> {
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

    let title = extract_title(&content)
        .unwrap_or_else(|| {
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
                    return Some(value.trim().trim_matches('"').trim_matches('\'').to_string());
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
pub async fn save_page_content(
    wiki: &Aiwiki,
    path: &str,
    content: &str,
) -> crate::Result<()> {
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
