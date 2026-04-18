//! Search functionality for AIWiki web interface.

use crate::web::templates::SearchResult;
use crate::Aiwiki;

/// Search the wiki for pages matching the query.
pub async fn search_wiki(
    wiki: &Aiwiki,
    query: &str,
    page_type_filter: Option<String>,
) -> crate::Result<Vec<SearchResult>> {
    if query.is_empty() {
        return Ok(Vec::new());
    }

    let query_lower = query.to_lowercase();
    let wiki_dir = wiki.path("wiki");
    let mut results = Vec::new();

    // Scan all markdown files
    let mut files_to_scan = Vec::new();
    scan_files_recursive(&wiki_dir, &wiki_dir, &mut files_to_scan).await?;

    // Search each file
    for file_path in files_to_scan {
        if let Some(result) = search_in_file(
            &file_path,
            &wiki_dir,
            &query_lower,
            page_type_filter.as_deref(),
        )
        .await
        {
            results.push(result);
        }
    }

    // Sort by relevance score (descending)
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

    // Limit to top 20 results
    results.truncate(20);

    Ok(results)
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

/// Search within a single file.
async fn search_in_file(
    file_path: &std::path::Path,
    base_dir: &std::path::Path,
    query: &str,
    page_type_filter: Option<&str>,
) -> Option<SearchResult> {
    let relative_path = file_path.strip_prefix(base_dir).ok()?;
    let path_str = relative_path.to_string_lossy().to_string();

    // Read file content
    let content = match tokio::fs::read_to_string(file_path).await {
        Ok(c) => c,
        Err(_) => return None,
    };

    // Extract title
    let title = extract_title(&content)
        .unwrap_or_else(|| {
            file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Untitled")
                .to_string()
        });

    // Determine page type
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

    // Apply page type filter
    if let Some(filter) = page_type_filter {
        if page_type != filter {
            return None;
        }
    }

    // Calculate relevance score
    let content_lower = content.to_lowercase();
    let mut score = 0.0f32;

    // Title match (highest weight)
    if title.to_lowercase().contains(query) {
        score += 10.0;
    }

    // Content frequency
    let occurrences = content_lower.matches(query).count();
    score += occurrences.min(10) as f32; // Cap at 10

    // Exact phrase bonus
    if content_lower.contains(query) {
        score += 5.0;
    }

    // If no match at all, skip
    if score == 0.0 {
        return None;
    }

    // Generate excerpt around first match
    let excerpt = generate_excerpt(&content_lower, &content, query, 200);

    let word_count = content.split_whitespace().count();

    Some(SearchResult {
        path: path_str,
        title,
        page_type,
        word_count,
        score,
        excerpt,
    })
}

/// Extract title from frontmatter or content.
fn extract_title(content: &str) -> Option<String> {
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

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("# ") {
            return Some(trimmed[2..].to_string());
        }
    }

    None
}

/// Generate an excerpt around the first match.
fn generate_excerpt(
    content_lower: &str,
    original: &str,
    query: &str,
    max_len: usize,
) -> String {
    if let Some(pos) = content_lower.find(query) {
        let start = pos.saturating_sub(max_len / 2);
        let end = (pos + query.len() + max_len / 2).min(original.len());

        let mut excerpt = String::new();

        if start > 0 {
            excerpt.push_str("...");
        }

        // Get the slice from original (not lowercase)
        let slice = &original[start..end];
        excerpt.push_str(slice);

        if end < original.len() {
            excerpt.push_str("...");
        }

        // Highlight the query
        excerpt.replace(
            &query.to_lowercase(),
            &format!("<mark>{}</mark>", query),
        )
    } else {
        // No match in content (might only be in title)
        original.chars().take(max_len).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_excerpt() {
        let content = "This is a long document with the keyword in the middle of it.";
        let content_lower = content.to_lowercase();
        let excerpt = generate_excerpt(&content_lower, content, "keyword", 30);
        assert!(excerpt.contains("keyword"));
        assert!(excerpt.starts_with("...") || excerpt.starts_with("This"));
    }

    #[test]
    fn test_extract_title_from_frontmatter() {
        let content = "---\ntitle: My Page\n---\n\n# Heading\n";
        assert_eq!(extract_title(content), Some("My Page".to_string()));
    }

    #[test]
    fn test_extract_title_from_heading() {
        let content = "# My Heading\n\nSome content.";
        assert_eq!(extract_title(content), Some("My Heading".to_string()));
    }
}
