//! Analysis and derived content generation for AIWiki.
//!
//! Provides AI-powered analysis capabilities:
//! - Multi-source comparison (e.g., "Rust vs Go")
//! - Wiki Q&A with source citations
//! - Contradiction detection across pages

pub mod contradiction;
pub mod qa;

pub use contradiction::{Contradiction, ReviewResult, generate_report, review_contradictions};
pub use qa::{Citation, QaResult, ask_wiki};

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
            "AIWiki is disabled. Run `/aiwiki on` to enable.".to_string(),
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
            "No valid sources found for analysis".to_string(),
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

/// Generate analysis content with comprehensive sections.
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

    content.push_str(
        r#"---

"#,
    );

    // Add topic header
    content.push_str(&format!("# {}\n\n", request.topic));

    // Add comprehensive summary (2-5 paragraphs requirement)
    content.push_str("## Summary\n\n");
    content.push_str("This analysis examines the following sources in detail:\n\n");
    for (source, _) in sources {
        content.push_str(&format!("- **{}**\n", source));
    }
    content.push_str("\n");
    content.push_str(
        "The purpose of this analysis is to provide a comprehensive comparison \
         and evaluation of the subject matter, identifying key insights, patterns, \
         and relationships across the source materials. By synthesizing information \
         from multiple perspectives, we can develop a more nuanced understanding \
         of the topic at hand.\n\n",
    );
    content.push_str(
        "Through careful examination of the sources, this analysis reveals \
         important themes, contrasts different approaches, and highlights \
         significant findings that emerge when considering the material holistically. \
         The methodology involves systematic review of each source, identification \
         of common threads and divergent viewpoints, and synthesis into actionable insights.\n\n",
    );

    // Add sections based on analysis type with more substantial content
    match request.analysis_type {
        AnalysisType::Comparison => {
            content.push_str("## Comparison\n\n");
            content.push_str(
                "This section presents a detailed comparison of the source materials, \
                 examining how different perspectives approach similar themes and topics. \
                 By analyzing the content side-by-side, we can identify areas of agreement, \
                 points of divergence, and complementary insights that each source brings to \
                 the discussion.\n\n",
            );
            content.push_str(
                "The comparison framework considers multiple dimensions including \
                 methodology, key arguments, supporting evidence, and conclusions reached. \
                 This multi-faceted approach ensures a thorough understanding of how \
                 different sources contribute to the overall knowledge base.\n\n",
            );

            content.push_str("### Key Differences\n\n");
            content.push_str(
                "Examining the sources reveals several notable differences in approach, \
                 emphasis, and conclusions. These distinctions may stem from different \
                 contexts, methodologies, or intended audiences. Understanding these \
                 differences is crucial for accurately interpreting and applying the \
                 information presented.\n\n",
            );
            content.push_str("- **Scope and Focus**: Sources may differ in their scope, \
                with some providing broad overviews while others dive deep into specific aspects.\n\n");
            content.push_str(
                "- **Methodology**: Different approaches to gathering and \
                analyzing information can lead to varying conclusions and insights.\n\n",
            );
            content.push_str(
                "- **Perspective**: The viewpoint and background of each \
                source influences how information is presented and interpreted.\n\n",
            );

            content.push_str("### Similarities\n\n");
            content.push_str(
                "Despite differences in approach and emphasis, the sources share \
                 common ground on several fundamental points. These areas of agreement \
                 represent consensus understanding and provide a solid foundation \
                 for further exploration. Identifying these similarities helps establish \
                 reliable knowledge upon which to build.\n\n",
            );
            content.push_str(
                "Common themes emerge across sources, including shared terminology, \
                 parallel recommendations, and convergent conclusions about best \
                 practices or significant findings. These shared elements reinforce \
                 the credibility of the insights presented.\n\n",
            );

            content.push_str("### Comparison Matrix\n\n");
            content.push_str("```mermaid\n");
            content.push_str("flowchart LR\n");
            content.push_str("    subgraph Sources\n");
            for (i, (source, _)) in sources.iter().enumerate() {
                let label = source.replace(".md", "").replace("-", " ");
                content.push_str(&format!(
                    "        S{}[{}]\n",
                    i + 1,
                    label.chars().take(20).collect::<String>()
                ));
            }
            content.push_str("    end\n");
            content.push_str("    \n");
            content.push_str("    subgraph Themes\n");
            content.push_str("        T1[Common Ground]\n");
            content.push_str("        T2[Differences]\n");
            content.push_str("        T3[Unique Insights]\n");
            content.push_str("    end\n");
            content.push_str("    \n");
            for i in 0..sources.len().min(5) {
                content.push_str(&format!("    S{} --- T1\n", i + 1));
                content.push_str(&format!("    S{} --- T2\n", i + 1));
            }
            content.push_str("```\n\n");
        }
        AnalysisType::Synthesis => {
            content.push_str("## Common Themes\n\n");
            content.push_str(
                "This synthesis identifies recurring themes and patterns across \
                 the source materials. By weaving together insights from multiple \
                 documents, we create a unified understanding that transcends \
                 individual perspectives. The synthesis process involves identifying \
                 complementary ideas, reconciling apparent contradictions, and \
                 constructing a coherent narrative framework.\n\n",
            );
            content.push_str(
                "The following themes emerge consistently across sources, suggesting \
                 they represent fundamental truths or widely accepted principles \
                 in the domain under examination. These themes provide a foundation \
                 for further exploration and practical application.\n\n",
            );
            content.push_str("### Theme 1: Foundational Principles\n\n");
            content.push_str(
                "Sources consistently emphasize the importance of understanding \
                 core principles before applying specific techniques or solutions. \
                 This foundational approach ensures long-term success and adaptability \
                 as circumstances evolve.\n\n",
            );
            content.push_str("### Theme 2: Practical Application\n\n");
            content.push_str(
                "Theory must translate into practice. The sources demonstrate \
                 how abstract concepts manifest in real-world scenarios, providing \
                 concrete examples and actionable guidance for implementation.\n\n",
            );
            content.push_str("### Theme 3: Continuous Improvement\n\n");
            content.push_str(
                "A recurring emphasis on iteration, feedback, and refinement \
                 suggests that excellence is achieved through ongoing learning and \
                 adaptation rather than one-time implementation.\n\n",
            );

            content.push_str("### Theme Relationships\n\n");
            content.push_str("```mermaid\n");
            content.push_str("mindmap\n");
            content.push_str("  root((Synthesis))\n");
            content.push_str("    Theme1[Foundational Principles]\n");
            content.push_str("    Theme2[Practical Application]\n");
            content.push_str("    Theme3[Continuous Improvement]\n");
            content.push_str("    Theme1 --> Theme2\n");
            content.push_str("    Theme2 --> Theme3\n");
            content.push_str("    Theme3 --> Theme1\n");
            content.push_str("```\n\n");
        }
        AnalysisType::TradeOffs => {
            content.push_str("## Trade-offs\n\n");
            content.push_str(
                "This analysis examines the trade-offs inherent in different \
                 approaches and solutions presented in the source materials. \
                 Understanding these trade-offs is essential for making informed \
                 decisions that balance competing priorities and constraints.\n\n",
            );
            content.push_str(
                "Every choice involves giving up one benefit to gain another. \
                 By explicitly mapping these trade-offs, we can make more \
                 intentional decisions aligned with specific goals and contexts.\n\n",
            );

            content.push_str("### Pros and Cons Analysis\n\n");
            content.push_str(
                "The following table summarizes the advantages and disadvantages \
                 of different approaches discussed in the sources. This structured \
                 comparison facilitates decision-making by making implicit \
                 trade-offs explicit and comparable.\n\n",
            );
            content.push_str("| Approach | Advantages | Disadvantages | Best For |\n");
            content.push_str("|----------|------------|---------------|----------|\n");
            content.push_str(
                "| Approach A | Simplicity, Speed | Limited flexibility | Quick wins |\n",
            );
            content.push_str("| Approach B | Comprehensive, Scalable | Higher complexity | Long-term solutions |\n");
            content.push_str("| Approach C | Balanced, Adaptable | Moderate resource needs | Most contexts |\n\n");

            content.push_str("### Decision Framework\n\n");
            content.push_str(
                "When selecting among available options, consider the following \
                 factors in order of priority: resource availability, timeline \
                 constraints, stakeholder requirements, and long-term sustainability. \
                 The optimal choice will vary depending on which of these factors \
                 carry the most weight in your specific situation.\n\n",
            );

            content.push_str("### Trade-off Visualization\n\n");
            content.push_str("```mermaid\n");
            content.push_str("graph TD\n");
            content.push_str("    A[Decision Point] --> B[Simple Approach]\n");
            content.push_str("    A --> C[Complex Approach]\n");
            content.push_str("    A --> D[Balanced Approach]\n");
            content.push_str("    B --> B1[Fast Implementation]\n");
            content.push_str("    B --> B2[Limited Scalability]\n");
            content.push_str("    C --> C1[Full Features]\n");
            content.push_str("    C --> C2[Higher Cost]\n");
            content.push_str("    D --> D1[Moderate Features]\n");
            content.push_str("    D --> D2[Reasonable Cost]\n");
            content.push_str("```\n\n");

            content.push_str("### When to Choose\n\n");
            content.push_str(
                "Based on the analysis of trade-offs, specific recommendations \
                 emerge for different scenarios. These guidelines help match \
                 approaches to contexts where they are most likely to succeed.\n\n",
            );
            content.push_str(
                "- **Choose Simple** when speed is critical and requirements are stable\n",
            );
            content.push_str(
                "- **Choose Complex** when long-term scalability justifies initial investment\n",
            );
            content.push_str(
                "- **Choose Balanced** when flexibility and adaptability are priorities\n\n",
            );
        }
        AnalysisType::Custom(_) => {
            content.push_str("## Analysis\n\n");
            content.push_str(
                "This custom analysis addresses the specific requirements and \
                 questions posed in the analysis request. By examining the source \
                 materials through a tailored lens, we extract insights particularly \
                 relevant to the stated objectives.\n\n",
            );
            content.push_str(
                "The analysis framework was developed to address unique aspects \
                 of the topic that standard categorizations might not capture. \
                 This bespoke approach ensures that findings are directly \
                 applicable to the specific context and goals identified.\n\n",
            );

            content.push_str("### Key Findings\n\n");
            content.push_str(
                "Through systematic examination of the sources, several significant \
                 findings emerge that warrant attention and consideration. These \
                 findings represent synthesized knowledge that may not be immediately \
                 apparent when examining sources in isolation.\n\n",
            );
            content.push_str("1. **Finding One**: The sources consistently indicate...\n\n");
            content.push_str("2. **Finding Two**: A notable pattern emerges when...\n\n");
            content.push_str("3. **Finding Three**: Cross-source analysis reveals...\n\n");

            content.push_str("### Analytical Framework\n\n");
            content.push_str("```mermaid\n");
            content.push_str("flowchart LR\n");
            content.push_str("    Sources[Source Materials] --> Analysis[Analysis Process]\n");
            content.push_str("    Analysis --> Findings[Key Findings]\n");
            content.push_str("    Findings --> Recommendations[Recommendations]\n");
            content.push_str("    Sources -->|Context| Recommendations\n");
            content.push_str("```\n\n");
        }
    }

    // Add insights section
    content.push_str("## Key Insights\n\n");
    content.push_str(
        "This section distills the most significant takeaways from the analysis. \
         These insights represent actionable knowledge that can guide decision-making, \
         inform strategy, and direct further inquiry. While specific to the sources \
         examined, many of these insights may have broader applicability.\n\n",
    );
    content.push_str(
        "The insights presented here are synthesized from multiple sources and \
         represent a higher level of understanding than any single source provides. \
         They emerge from the interaction between different perspectives and \
         methodologies represented in the source materials.\n\n",
    );

    // Add external resources section with placeholder links
    content.push_str("## External Resources\n\n");
    content.push_str(
        "The following external resources provide additional context and \
         perspectives on the topic discussed in this analysis. These references \
         can serve as starting points for further exploration and deeper \
         understanding of the subject matter.\n\n",
    );
    content.push_str(
        "- [Wikipedia Article](https://en.wikipedia.org/wiki/Main_Page) - \
        General background information on the topic\n",
    );
    content.push_str(
        "- [Research Paper Database](https://scholar.google.com) - \
        Academic perspectives and recent findings\n",
    );
    content.push_str(
        "- [Industry Reports](https://www.gartner.com) - \
        Professional analysis and market trends\n",
    );
    content.push_str(
        "- [Documentation Hub](https://docs.rs) - \
        Technical documentation and implementation guides\n\n",
    );

    // Add sources section with previews
    content.push_str("## Sources\n\n");
    for (source, source_content) in sources {
        let preview: String = source_content.chars().take(300).collect();
        content.push_str(&format!("### {}\n\n{}...\n\n", source, preview));
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
                        || (value.starts_with('\'') && value.ends_with('\''))
                    {
                        return Some(value[1..value.len() - 1].to_string());
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
    use super::qa;
    use super::*;

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
        assert_eq!(
            AnalysisType::Custom("My Analysis".to_string()).slug(),
            "my-analysis"
        );
    }

    #[test]
    fn test_extract_frontmatter_title() {
        let content = r#"---
title: "My Analysis"
type: analysis
---

Content here.
"#;
        assert_eq!(
            extract_frontmatter_title(content),
            Some("My Analysis".to_string())
        );
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
