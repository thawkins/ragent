//! Cross-link validation and management for AIWiki.
//!
//! Provides functionality to:
//! - Extract markdown links from wiki pages
//! - Validate that internal links point to existing pages
//! - Track link relationships between pages

use crate::{Aiwiki, Result};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Result of validating wiki links.
#[derive(Debug, Clone, Default)]
pub struct LinkValidationResult {
    /// Valid internal links (source -> target).
    pub valid: Vec<(PathBuf, String)>,
    /// Broken internal links (source -> missing target).
    pub broken: Vec<(PathBuf, String)>,
    /// External links (not validated).
    pub external: Vec<(PathBuf, String)>,
    /// Bidirectional link map (page -> pages that link to it).
    pub back_links: HashMap<PathBuf, Vec<PathBuf>>,
}

impl LinkValidationResult {
    /// Check if there are any broken links.
    pub fn has_broken(&self) -> bool {
        !self.broken.is_empty()
    }

    /// Get count of broken links.
    pub fn broken_count(&self) -> usize {
        self.broken.len()
    }
}

/// Extract all markdown links from a file.
///
/// Returns a vector of (link_text, link_target) tuples.
pub async fn extract_links(path: &Path) -> Result<Vec<(String, String)>> {
    let content = fs::read_to_string(path).await?;
    Ok(extract_links_from_text(&content))
}

/// Extract links from markdown text content.
///
/// Supports standard markdown link syntax:
/// - `[text](url)` - inline links
/// - `[text](url "title")` - inline links with title
fn extract_links_from_text(content: &str) -> Vec<(String, String)> {
    // Regex to match markdown links: [text](url) or [text](url "title")
    // Pattern: [text](target) where target stops at space, ), or "
    let re = Regex::new(r#"\[([^\]]+)\]\(([^)\s\"]+)"#).unwrap();

    re.captures_iter(content)
        .filter_map(|cap| {
            let text = cap.get(1)?.as_str().to_string();
            let target = cap.get(2)?.as_str().to_string();
            Some((text, target))
        })
        .collect()
}

/// Validate all internal links in the wiki.
///
/// Checks that all internal markdown links point to existing wiki pages.
/// External links (http://, https://, mailto:, etc.) are not validated.
pub async fn validate_wiki_links(wiki: &Aiwiki) -> Result<LinkValidationResult> {
    let mut result = LinkValidationResult::default();
    let wiki_dir = wiki.path("wiki");

    // Build set of all existing wiki pages
    let existing_pages = scan_wiki_pages(&wiki_dir).await?;

    // Check each wiki page for links
    for page_path in &existing_pages {
        let links = extract_links(page_path).await?;

        for (_link_text, target) in links {
            if is_external_link(&target) {
                result.external.push((page_path.clone(), target));
            } else {
                // Resolve internal link
                let resolved_target = resolve_internal_link(&wiki_dir, page_path, &target
                );

                if existing_pages.contains(&resolved_target) {
                    result.valid.push((page_path.clone(), target.clone()));

                    // Track back-link
                    result
                        .back_links
                        .entry(resolved_target)
                        .or_default()
                        .push(page_path.clone());
                } else {
                    result.broken.push((page_path.clone(), target));
                }
            }
        }
    }

    Ok(result)
}

/// Scan all markdown files in the wiki directory.
async fn scan_wiki_pages(wiki_dir: &Path) -> Result<HashSet<PathBuf>> {
    let mut pages = HashSet::new();

    if !wiki_dir.exists() {
        return Ok(pages);
    }

    scan_markdown_files_recursive(wiki_dir, &mut pages).await?;

    Ok(pages)
}

/// Recursively scan for markdown files.
async fn scan_markdown_files_recursive(
    dir: &Path,
    pages: &mut HashSet<PathBuf>,
) -> Result<()> {
    let mut entries = fs::read_dir(dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let metadata = entry.metadata().await?;

        if metadata.is_file() {
            if let Some(ext) = path.extension() {
                if ext == "md" || ext == "markdown" {
                    pages.insert(path);
                }
            }
        } else if metadata.is_dir() {
            Box::pin(scan_markdown_files_recursive(&path, pages)).await?;
        }
    }

    Ok(())
}

/// Check if a link target is external.
fn is_external_link(target: &str) -> bool {
    target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("mailto:")
        || target.starts_with("tel:")
        || target.starts_with("//")
}

/// Resolve an internal link to an absolute path.
///
/// Handles:
/// - Relative paths: `./file.md`, `../file.md`
/// - Anchors: `file.md#heading` (stripped)
/// - Wiki-style: `[[Page Name]]` (converted to slug)
fn resolve_internal_link(wiki_dir: &Path, source: &Path, target: &str) -> PathBuf {
    // Strip anchor if present
    let target = target.split('#').next().unwrap_or(target);

    // If it's already an absolute path within wiki
    let target_path = Path::new(target);

    if target_path.is_absolute() {
        return target_path.to_path_buf();
    }

    // Resolve relative to source file
    if let Some(source_dir) = source.parent() {
        let resolved = source_dir.join(target_path);
        if resolved.exists() {
            return resolved;
        }
    }

    // Try resolving from wiki root
    wiki_dir.join(target_path)
}

/// Generate a wiki slug from a title.
///
/// Converts "My Page Title" to "my-page-title".
pub fn slugify(title: &str) -> String {
    title
        .to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', "")
        .replace(' ', "-")
        .replace("--", "-")
        .trim_matches('-')
        .to_string()
}

/// Find orphaned pages (pages with no incoming links).
pub async fn find_orphaned_pages(wiki: &Aiwiki) -> Result<Vec<PathBuf>> {
    let validation = validate_wiki_links(wiki).await?;
    let all_pages = scan_wiki_pages(&wiki.path("wiki")).await?;

    let linked_pages: HashSet<_> = validation.back_links.keys().cloned().collect();

    let orphaned: Vec<_> = all_pages
        .into_iter()
        .filter(|p| !linked_pages.contains(p))
        .collect();

    Ok(orphaned)
}

/// Generate link suggestions for a page based on content.
///
/// Scans the content for potential links to existing wiki pages.
pub async fn suggest_links(wiki: &Aiwiki, content: &str) -> Result<Vec<LinkSuggestion>> {
    let existing_pages = scan_wiki_pages(&wiki.path("wiki")).await?;

    // Build a map of potential link targets
    let mut targets: HashMap<String, PathBuf> = HashMap::new();

    for page in existing_pages {
        // Add filename without extension
        if let Some(stem) = page.file_stem() {
            let name = stem.to_string_lossy().to_lowercase();
            targets.insert(name.clone(), page.clone());
            targets.insert(name.replace('-', " "), page.clone());
        }

        // Add title from frontmatter if available
        if let Ok(content) = fs::read_to_string(&page).await {
            if let Some(title) = extract_title_from_frontmatter(&content
            ) {
                targets.insert(title.to_lowercase(), page.clone());
                targets.insert(slugify(&title), page.clone());
            }
        }
    }

    let mut suggestions = Vec::new();

    // Check content for unlinked mentions
    for (target_name, target_path) in targets {
        if content.to_lowercase().contains(&target_name) {
            // Check if it's already linked
            let links = extract_links_from_text(content);
            let already_linked = links.iter().any(|(_, t)| {
                t.to_lowercase().contains(&target_name)
                    || target_name.contains(&t.to_lowercase())
            });

            if !already_linked {
                suggestions.push(LinkSuggestion {
                    text: target_name.clone(),
                    target: target_path,
                    target_name,
                });
            }
        }
    }

    Ok(suggestions)
}

/// Suggestion for a new link.
#[derive(Debug, Clone)]
pub struct LinkSuggestion {
    /// The text in the content that could be linked.
    pub text: String,
    /// The target page path.
    pub target: PathBuf,
    /// The display name of the target.
    pub target_name: String,
}

/// Extract title from YAML frontmatter.
fn extract_title_from_frontmatter(content: &str) -> Option<String> {
    if !content.starts_with("---") {
        return None;
    }

    if let Some(end) = content.find("\n---\n") {
        let frontmatter = &content[4..end];
        for line in frontmatter.lines() {
            if let Some(value) = line.strip_prefix("title:") {
                return Some(value.trim().trim_matches('"').trim_matches('\'').to_string());
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_links_from_text() {
        let text = r#"# Test

This is a [link to page](page.md) and [another](https://example.com).
See also [internal link](./other.md).
"#;

        let links = extract_links_from_text(text);
        assert_eq!(links.len(), 3);
        assert!(links.iter().any(|(_, t)| t == "page.md"));
        assert!(links.iter().any(|(_, t)| t == "https://example.com"));
        assert!(links.iter().any(|(_, t)| t == "./other.md"));
    }

    #[test]
    fn test_is_external_link() {
        assert!(is_external_link("http://example.com"));
        assert!(is_external_link("https://example.com"));
        assert!(is_external_link("mailto:test@example.com"));
        assert!(!is_external_link("page.md"));
        assert!(!is_external_link("./other.md"));
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("My Page Title"), "my-page-title");
        assert_eq!(slugify("Rust Lifetimes"), "rust-lifetimes");
        assert_eq!(slugify("Hello World!!!"), "hello-world");
    }

    #[test]
    fn test_resolve_internal_link() {
        let wiki_dir = Path::new("/wiki");
        let source = Path::new("/wiki/sources/test.md");

        let resolved = resolve_internal_link(wiki_dir, source, "../concepts/rust.md");
        assert!(resolved.to_string_lossy().contains("concepts"));

        let resolved = resolve_internal_link(wiki_dir, source, "page.md");
        assert!(resolved.to_string_lossy().contains("sources"));
    }
}
