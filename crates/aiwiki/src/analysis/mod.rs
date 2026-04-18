//! Analysis and derived content generation for AIWiki.
//!
//! Provides AI-powered analysis capabilities:
//! - Multi-source comparison (e.g., "Rust vs Go")
//! - Wiki Q&A with source citations
//! - Contradiction detection across pages

pub mod qa;
pub mod contradiction;

pub use qa::{ask_wiki, QaResult, Citation};
pub use contradiction::{review_contradictions, Contradiction, ReviewResult, generate_report};

use crate::Aiwiki;
use serde::{Deserialize, Serialize};
use tokio::fs;

/// Result of an analysis operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    /// Path to the generated analysis file.
    pub path: String,
    /// Title of the analysis.
    pub title: String,
    /// Sources used in the analysis.
    pub sources: Vec<String>,
    /// Word count.
    pub word_count: usize,
}

/// Request to generate an analysis.
#[derive(Debug, Clone)]
pub struct AnalysisRequest {
    /// Topic to analyze (e.g., "Rust vs Go").
    pub topic: String,
    /// Sources to compare (file paths or page references).
    pub sources: Vec<String>,
    /// Type of analysis.
    pub analysis_type: AnalysisType,
}

/// Type of analysis to generate.
#[derive(Debug, Clone)]
pub enum AnalysisType {
    /// Compare multiple sources (e.g., languages, frameworks).
    Comparison,
    /// Synthesize common themes.
    Synthesis,
    /// Evaluate trade-offs.
    TradeOffs,
    /// Custom analysis prompt.
    Custom(String),
}

impl AnalysisType {
    /// Get the slug for this analysis type.
    pub fn slug(&self) -> String {
        match self {
            Self::Comparison => "comparison".to_string(),
            Self::Synthesis => "synthesis".to_string(),
            Self::TradeOffs => "tradeoffs".to_string(),
            Self::Custom(s) => s.to_lowercase().replace(' ', "-"),
        }
    }
}

/// Generate an analysis comparing multiple wiki sources.
///
/// # Arguments
/// * `wiki` - The AIWiki instance
/// * `request` - Analysis request with topic and sources
///
/// # Returns
/// The generated analysis result.
pub async fn generate_analysis(
    wiki: &Aiwiki,
    request: &AnalysisRequest,
) -> crate::Result<AnalysisResult> {
    if !wiki.config.enabled {
        return Err(crate::AiwikiError::Config(
            "AIWiki is disabled. Run `/aiwiki on` to enable.".to_string()
        ));
    }

    // Read source content
    let mut source_contents = Vec::new();
    for source in &request.sources {
        let source_path = wiki.path("wiki").join(source);
        if source_path.exists() {
            let content = fs::read_to_string(&source_path).await?;
            source_contents.push((source.clone(), content));
        }
    }

    if source_contents.is_empty() {
        return Err(crate::AiwikiError::Config(
            "No valid sources found for analysis".to_string()
        ));
    }

    // Generate slug from topic
    let slug = slugify(&request.topic);
    let filename = format!("{}.md", slug);
    let output_path = wiki.path("wiki/analyses").join(&filename);

    // Ensure directory exists
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    // Generate analysis content
    // TODO: In a full implementation, this would call an LLM
    // For now, create a template with source references
    let content = generate_analysis_content(request, &source_contents).await?;

    fs::write(&output_path, &content).await?;

    let word_count = content.split_whitespace().count();

    Ok(AnalysisResult {
        path: format!("analyses/{}", filename),
        title: request.topic.clone(),
        sources: request.sources.clone(),
        word_count,
    })
}

/// Generate analysis content (stub for LLM integration).
async fn generate_analysis_content(
    request: &AnalysisRequest,
    sources: &[(String, String)],
) -> crate::Result<String> {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
    
    let mut content = format!(
        r#"---
title: "{}"
type: analysis
analysis_type: {}
date: {}
sources:
"#,
        request.topic,
        request.analysis_type.slug(),
        now
    );

    for (source, _) in sources {
        content.push_str(&format!("  - {}\n", source));
    }

    content.push_str(r#"---

"#);

    // Add topic header
    content.push_str(&format!("# {}\n\n", request.topic));

    // Add summary
    content.push_str("## Summary\n\n");
    content.push_str("This analysis compares the following sources:\n\n");
    for (source, _) in sources {
        content.push_str(&format!("- **{}**\n", source));
    }
    content.push_str("\n");

    // Add placeholder sections based on analysis type
    match request.analysis_type {
        AnalysisType::Comparison => {
            content.push_str("## Comparison\n\n");
            content.push_str("_Generated analysis comparing key aspects..._\n\n");
            
            content.push_str("### Key Differences\n\n");
            content.push_str("- Aspect 1: ...\n");
            content.push_str("- Aspect 2: ...\n");
            content.push_str("- Aspect 3: ...\n\n");
            
            content.push_str("### Similarities\n\n");
            content.push_str("- ...\n\n");
        }
        AnalysisType::Synthesis => {
            content.push_str("## Common Themes\n\n");
            content.push_str("_Generated synthesis of shared concepts..._\n\n");
            content.push_str("1. Theme 1\n");
            content.push_str("2. Theme 2\n");
            content.push_str("3. Theme 3\n\n");
        }
        AnalysisType::TradeOffs => {
            content.push_str("## Trade-offs\n\n");
            content.push_str("_Generated analysis of trade-offs..._\n\n");
            
            content.push_str("### Pros and Cons\n\n");
            content.push_str("| Option | Pros | Cons |\n");
            content.push_str("|--------|------|------|\n");
            content.push_str("| ... | ... | ... |\n\n");
            
            content.push_str("### When to Choose\n\n");
            content.push_str("- Choose X when...\n");
            content.push_str("- Choose Y when...\n\n");
        }
        AnalysisType::Custom(_) => {
            content.push_str("## Analysis\n\n");
            content.push_str("_Custom analysis..._\n\n");
        }
    }

    // Add sources section
    content.push_str("## Sources\n\n");
    for (source, source_content) in sources {
        let preview: String = source_content.chars().take(200).collect();
        content.push_str(&format!(
            "### {}\n\n{}...\n\n",
            source, preview
        ));
    }

    Ok(content)
}

/// Convert text to URL-friendly slug.
fn slugify(text: &str) -> String {
    text.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != ' ', "")
        .replace(' ', "-")
        .replace("--", "-")
        .trim_matches('-')
        .to_string()
}

/// List existing analyses.
pub async fn list_analyses(wiki: &Aiwiki) -> crate::Result<Vec<AnalysisResult>> {
    let analyses_dir = wiki.path("wiki/analyses");
    let mut analyses = Vec::new();

    if !analyses_dir.exists() {
        return Ok(analyses);
    }

    let mut entries = fs::read_dir(&analyses_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().map(|e| e == "md").unwrap_or(false) {
            if let Some(result) = load_analysis_info(&path, wiki).await {
                analyses.push(result);
            }
        }
    }

    analyses.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(analyses)
}

/// Load analysis info from a file.
async fn load_analysis_info(path: &std::path::Path, wiki: &Aiwiki) -> Option<AnalysisResult> {
    let content = fs::read_to_string(path).await.ok()?;
    let relative_path = path.strip_prefix(wiki.path("wiki")).ok()?;
    let path_str = relative_path.to_string_lossy().to_string();

    // Extract title from frontmatter or filename
    let title = extract_frontmatter_title(&content)
        .or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .map(|s| s.replace('-', " "))
        })
        .unwrap_or_else(|| "Untitled".to_string());

    // Extract sources from frontmatter
    let sources = extract_sources(&content);

    let word_count = content.split_whitespace().count();

    Some(AnalysisResult {
        path: path_str,
        title,
        sources,
        word_count,
    })
}

/// Extract title from YAML frontmatter.
fn extract_frontmatter_title(content: &str) -> Option<String> {
    if content.starts_with("---") {
        if let Some(end) = content.find("\n---\n") {
            let frontmatter = &content[4..end];
            for line in frontmatter.lines() {
                if let Some(value) = line.strip_prefix("title:") {
                    let value = value.trim();
                    // Remove quotes if present
                    if (value.starts_with('"') && value.ends_with('"')) 
                        || (value.starts_with('\'') && value.ends_with('\'')) {
                        return Some(value[1..value.len()-1].to_string());
                    }
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

/// Extract sources list from frontmatter.
fn extract_sources(content: &str) -> Vec<String> {
    let mut sources = Vec::new();
    
    if content.starts_with("---") {
        if let Some(end) = content.find("\n---\n") {
            let frontmatter = &content[4..end];
            let mut in_sources = false;
            
            for line in frontmatter.lines() {
                if line.starts_with("sources:") {
                    in_sources = true;
                    continue;
                }
                if in_sources {
                    if line.starts_with("  - ") {
                        sources.push(line[4..].to_string());
                    } else if !line.starts_with(" ") {
                        in_sources = false;
                    }
                }
            }
        }
    }
    
    sources
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::qa;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Rust vs Go"), "rust-vs-go");
        assert_eq!(slugify("  Hello World!!!  "), "hello-world");
    }

    #[test]
    fn test_analysis_type_slug() {
        assert_eq!(AnalysisType::Comparison.slug(), "comparison");
        assert_eq!(AnalysisType::Synthesis.slug(), "synthesis");
        assert_eq!(AnalysisType::TradeOffs.slug(), "tradeoffs");
        assert_eq!(AnalysisType::Custom("My Analysis".to_string()).slug(), "my-analysis");
    }

    #[test]
    fn test_extract_frontmatter_title() {
        let content = r#"---
title: "My Analysis"
type: analysis
---

Content here.
"#;
        assert_eq!(extract_frontmatter_title(content), Some("My Analysis".to_string()));
    }

    #[test]
    fn test_extract_sources() {
        let content = r#"---
title: Test
type: analysis
sources:
  - page1.md
  - page2.md
---

Content.
"#;
        let sources = extract_sources(content);
        assert_eq!(sources, vec!["page1.md", "page2.md"]);
    }
}
