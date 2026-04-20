//! Contradiction detection across wiki pages.

use crate::Aiwiki;
use serde::{Deserialize, Serialize};
use tokio::fs;

/// Detected contradiction between pages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contradiction {
    /// Unique ID for this contradiction.
    pub id: String,
    /// Description of the contradiction.
    pub description: String,
    /// Pages involved.
    pub pages: Vec<String>,
    /// Conflicting statements.
    pub statements: Vec<String>,
    /// Suggested resolution.
    pub suggestion: String,
    /// Severity (high, medium, low).
    pub severity: String,
}

/// Result of contradiction review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    /// List of detected contradictions.
    pub contradictions: Vec<Contradiction>,
    /// Total pages reviewed.
    pub pages_reviewed: usize,
    /// Review timestamp.
    pub timestamp: String,
}

/// Review wiki for contradictions.
///
/// # Arguments
/// * `wiki` - The AIWiki instance
/// * `scope` - Optional scope to limit review (e.g., specific directory)
///
/// # Returns
/// Review result with detected contradictions.
pub async fn review_contradictions(
    wiki: &Aiwiki,
    scope: Option<&str>,
) -> crate::Result<ReviewResult> {
    if !wiki.config.enabled {
        return Err(crate::AiwikiError::Config(
            "AIWiki is disabled. Run `/aiwiki on` to enable.".to_string(),
        ));
    }

    // Load all pages in scope
    let pages = load_wiki_pages(wiki, scope).await?;

    if pages.len() < 2 {
        return Ok(ReviewResult {
            contradictions: Vec::new(),
            pages_reviewed: pages.len(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        });
    }

    // TODO: In a full implementation, this would:
    // 1. Use an LLM to compare statements across pages
    // 2. Extract factual claims
    // 3. Flag inconsistencies
    // 4. Suggest resolutions

    // For now, return empty result (no stub contradictions)
    Ok(ReviewResult {
        contradictions: Vec::new(),
        pages_reviewed: pages.len(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    })
}

/// Page content for contradiction detection.
#[derive(Debug, Clone)]
struct PageData {
    path: String,
    title: String,
    content: String,
    facts: Vec<String>,
}

/// Load all wiki pages for review.
async fn load_wiki_pages(wiki: &Aiwiki, scope: Option<&str>) -> crate::Result<Vec<PageData>> {
    let mut pages = Vec::new();
    let wiki_dir = if let Some(s) = scope {
        wiki.path("wiki").join(s)
    } else {
        wiki.path("wiki")
    };

    if !wiki_dir.exists() {
        return Ok(pages);
    }

    let mut files = Vec::new();
    scan_markdown_files(&wiki_dir, &mut files).await?;

    for file_path in files {
        let content = fs::read_to_string(&file_path).await?;
        let relative_path = file_path
            .strip_prefix(wiki.path("wiki"))
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let title = extract_title(&content).unwrap_or_else(|| relative_path.clone());

        // Extract potential factual claims
        let facts = extract_facts(&content);

        pages.push(PageData {
            path: relative_path,
            title,
            content,
            facts,
        });
    }

    Ok(pages)
}

/// Extract potential factual claims from content.
fn extract_facts(content: &str) -> Vec<String> {
    let mut facts = Vec::new();

    // Skip frontmatter
    let content = if content.starts_with("---") {
        if let Some(end) = content.find("\n---\n") {
            &content[end + 5..]
        } else {
            content
        }
    } else {
        content
    };

    // Look for statements that might be factual claims
    for line in content.lines() {
        let line = line.trim();

        // Skip headers, lists, code blocks
        if line.starts_with('#')
            || line.starts_with('-')
            || line.starts_with('*')
            || line.starts_with('`')
            || line.starts_with('|')
        {
            continue;
        }

        // Look for declarative sentences
        if line.len() > 30 && line.ends_with('.') {
            // Check for indicators of factual claims
            if line.contains("is a")
                || line.contains("has")
                || line.contains("supports")
                || line.contains("requires")
                || line.contains("uses")
                || line.contains("provides")
            {
                facts.push(line.to_string());
            }
        }
    }

    facts.truncate(10); // Limit to first 10 facts
    facts
}

/// Extract title from content.
fn extract_title(content: &str) -> Option<String> {
    if content.starts_with("---") {
        if let Some(end) = content.find("\n---\n") {
            let frontmatter = &content[4..end];
            for line in frontmatter.lines() {
                if let Some(value) = line.strip_prefix("title:") {
                    let value = value.trim();
                    if (value.starts_with('"') && value.ends_with('"'))
                        || (value.starts_with('\'') && value.ends_with('\''))
                    {
                        return Some(value[1..value.len() - 1].to_string());
                    }
                    return Some(value.to_string());
                }
            }
        }
    }

    for line in content.lines() {
        if line.starts_with("# ") {
            return Some(line[2..].to_string());
        }
    }

    None
}

/// Scan for markdown files.
async fn scan_markdown_files(
    dir: &std::path::Path,
    files: &mut Vec<std::path::PathBuf>,
) -> crate::Result<()> {
    let mut entries = fs::read_dir(dir).await?;

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
            Box::pin(scan_markdown_files(&path, &mut sub_files)).await?;
            files.extend(sub_files);
        }
    }

    Ok(())
}

/// Generate a contradiction report as markdown.
pub fn generate_report(result: &ReviewResult) -> String {
    let mut report = format!(
        r#"---
title: "Contradiction Review Report"
date: {}
pages_reviewed: {}
---

# Contradiction Review Report

**Pages Reviewed:** {}  
**Contradictions Found:** {}  
**Generated:** {}

"#,
        result.timestamp,
        result.pages_reviewed,
        result.pages_reviewed,
        result.contradictions.len(),
        result.timestamp
    );

    if result.contradictions.is_empty() {
        report.push_str("✅ No contradictions detected.\n\n");
    } else {
        report.push_str("## Detected Contradictions\n\n");

        for (i, contradiction) in result.contradictions.iter().enumerate() {
            report.push_str(&format!(
                "### {}. {} ({} severity)\n\n",
                i + 1,
                contradiction.description,
                contradiction.severity
            ));

            report.push_str("**Conflicting Statements:**\n\n");
            for (j, statement) in contradiction.statements.iter().enumerate() {
                report.push_str(&format!("{}. \"{}\"\n\n", j + 1, statement));
            }

            report.push_str("**Sources:**\n");
            for page in &contradiction.pages {
                report.push_str(&format!("- {}\n", page));
            }
            report.push('\n');

            report.push_str("**Suggested Resolution:**\n\n");
            report.push_str(&format!("{}\n\n", contradiction.suggestion));
            report.push_str("---\n\n");
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_facts() {
        let content = r#"---
title: Test
---

# Heading

Rust is a systems programming language. It provides memory safety.

- List item
- Another item

This is a paragraph with is a claim.
"#;
        let facts = extract_facts(content);
        assert!(!facts.is_empty());
        assert!(facts.iter().any(|f| f.contains("Rust is")));
    }

    #[test]
    fn test_generate_report() {
        let result = ReviewResult {
            contradictions: Vec::new(),
            pages_reviewed: 5,
            timestamp: "2024-01-01T00:00:00Z".to_string(),
        };

        let report = generate_report(&result);
        assert!(report.contains("No contradictions detected"));
        assert!(report.contains("5"));
    }
}
