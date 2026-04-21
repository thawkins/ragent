//! Wiki page generation from extraction results.
//!
//! Creates markdown wiki pages in the appropriate subdirectories
//! (sources/, entities/, concepts/) from `ExtractionResult` data.

use crate::extraction::{ExtractedConcept, ExtractedEntity, ExtractionResult};
use std::collections::HashSet;
use std::path::Path;
use tokio::fs;

/// Strip markdown code fences from diagram content if present.
///
/// LLMs sometimes return diagram content wrapped in ` ```mermaid ... ``` `
/// fences. Since the caller already wraps the content in fences, this
/// function strips them to avoid double-fencing which breaks rendering.
fn strip_mermaid_fences(raw: &str) -> &str {
    let trimmed = raw.trim();
    // Check if content starts with a ```mermaid fence
    if let Some(after_fence) = trimmed.strip_prefix("```mermaid") {
        // Find the closing fence
        let body = after_fence.trim_start_matches(|c: char| c == '\r' || c == '\n');
        if let Some(end) = body.rfind("```") {
            let inner = &body[..end];
            return inner.trim();
        }
        // No closing fence found — return body as-is (stripped of leading newline)
        return body.trim();
    }
    // Also handle bare ``` fences without the mermaid language tag
    if let Some(after_fence) = trimmed.strip_prefix("```") {
        // Check if the first non-empty line is a mermaid diagram type
        let first_content_line = after_fence
            .lines()
            .map(|l| l.trim())
            .find(|l| !l.is_empty())
            .unwrap_or("");
        if first_content_line.starts_with("flowchart")
            || first_content_line.starts_with("graph")
            || first_content_line.starts_with("sequenceDiagram")
            || first_content_line.starts_with("classDiagram")
            || first_content_line.starts_with("stateDiagram")
            || first_content_line.starts_with("erDiagram")
            || first_content_line.starts_with("gantt")
            || first_content_line.starts_with("pie")
            || first_content_line.starts_with("mindmap")
            || first_content_line.starts_with("subgraph")
        {
            let body = after_fence.trim_start_matches(|c: char| c == '\r' || c == '\n');
            if let Some(end) = body.rfind("```") {
                let inner = &body[..end];
                return inner.trim();
            }
            return body.trim();
        }
    }
    trimmed
}

/// Write all wiki pages for a source extraction result.
///
/// Creates:
/// - One source summary page in `wiki/sources/`
/// - One page per entity in `wiki/entities/`
/// - One page per concept in `wiki/concepts/`
///
/// # Arguments
/// * `wiki_dir` - Path to the `aiwiki/` directory
/// * `source_name` - Base name of the source file (without extension)
/// * `result` - The extraction result from the LLM
///
/// # Returns
/// Vector of relative paths (from wiki_dir) for all generated pages.
pub async fn write_pages(
    wiki_dir: &Path,
    source_name: &str,
    result: &ExtractionResult,
) -> crate::Result<Vec<String>> {
    let mut generated = Vec::new();

    // Ensure directories exist
    let sources_dir = wiki_dir.join("wiki").join("sources");
    let entities_dir = wiki_dir.join("wiki").join("entities");
    let concepts_dir = wiki_dir.join("wiki").join("concepts");
    fs::create_dir_all(&sources_dir).await?;
    fs::create_dir_all(&entities_dir).await?;
    fs::create_dir_all(&concepts_dir).await?;

    // Write source summary page
    let source_page = write_source_page(&sources_dir, source_name, result).await?;
    generated.push(format!("wiki/sources/{source_page}"));

    // Write entity pages
    for entity in &result.entities {
        let slug = slugify(&entity.name);
        let entity_page = write_entity_page(&entities_dir, &slug, entity, source_name).await?;
        generated.push(format!("wiki/entities/{entity_page}"));
    }

    // Build set of concept slugs being created in this batch
    let batch_concept_slugs: HashSet<String> =
        result.concepts.iter().map(|c| slugify(&c.name)).collect();

    // Write concept pages
    for concept in &result.concepts {
        let slug = slugify(&concept.name);
        let concept_page = write_concept_page(
            &concepts_dir,
            &slug,
            concept,
            source_name,
            &batch_concept_slugs,
        )
        .await?;
        generated.push(format!("wiki/concepts/{concept_page}"));
    }

    Ok(generated)
}

/// Write a source summary page.
///
/// # Returns
/// The filename of the generated page (e.g., `my-document.md`).
async fn write_source_page(
    dir: &Path,
    source_name: &str,
    result: &ExtractionResult,
) -> crate::Result<String> {
    let slug = slugify(source_name);
    let filename = format!("{slug}.md");
    let path = dir.join(&filename);

    let mut content = String::new();
    content.push_str("---\n");
    content.push_str(&format!("title: \"{}\"\n", escape_yaml(&result.title)));
    content.push_str(&format!("source: \"{source_name}\"\n"));
    content.push_str("type: source\n");
    if !result.tags.is_empty() {
        content.push_str(&format!("tags: [{}]\n", result.tags.join(", ")));
    }
    content.push_str(&format!(
        "generated: \"{}\"\n",
        chrono::Utc::now().to_rfc3339()
    ));
    content.push_str("---\n\n");
    content.push_str(&format!("# {}\n\n", result.title));
    content.push_str(&result.summary);
    content.push('\n');

    // Cross-links section
    if !result.entities.is_empty() || !result.concepts.is_empty() {
        content.push_str("\n## Related\n\n");

        if !result.entities.is_empty() {
            content.push_str("### Entities\n\n");
            for entity in &result.entities {
                let link = slugify(&entity.name);
                content.push_str(&format!(
                    "- [{}](../entities/{}.md) — {}\n",
                    entity.name, link, entity.entity_type
                ));
            }
            content.push('\n');
        }

        if !result.concepts.is_empty() {
            content.push_str("### Concepts\n\n");
            for concept in &result.concepts {
                let link = slugify(&concept.name);
                content.push_str(&format!("- [{}](../concepts/{}.md)\n", concept.name, link));
            }
            content.push('\n');
        }
    }

    fs::write(&path, &content).await?;
    Ok(filename)
}

/// Write an entity page. Appends to existing page if the entity already exists.
///
/// # Returns
/// The filename of the generated page.
async fn write_entity_page(
    dir: &Path,
    slug: &str,
    entity: &ExtractedEntity,
    source_name: &str,
) -> crate::Result<String> {
    let filename = format!("{slug}.md");
    let path = dir.join(&filename);

    if path.exists() {
        // Append source reference to existing entity page
        let existing = fs::read_to_string(&path).await?;
        if !existing.contains(source_name) {
            let addition = format!("\n### From: {source_name}\n\n{}\n", entity.description);
            fs::write(&path, format!("{existing}{addition}")).await?;
        }
    } else {
        let mut content = String::new();
        content.push_str("---\n");
        content.push_str(&format!("title: \"{}\"\n", escape_yaml(&entity.name)));
        content.push_str(&format!("entity_type: \"{}\"\n", entity.entity_type));
        content.push_str("type: entity\n");
        content.push_str(&format!(
            "generated: \"{}\"\n",
            chrono::Utc::now().to_rfc3339()
        ));
        content.push_str("---\n\n");
        content.push_str(&format!("# {}\n\n", entity.name));
        content.push_str(&format!("**Type:** {}\n\n", entity.entity_type));
        content.push_str(&format!("### From: {source_name}\n\n"));
        content.push_str(&entity.description);
        content.push('\n');

        // Mermaid diagram section
        if let Some(ref diagram) = entity.diagram {
            let clean = strip_mermaid_fences(diagram);
            if !clean.is_empty() {
                content.push_str("\n## Diagram\n\n");
                content.push_str("```mermaid\n");
                content.push_str(clean);
                content.push_str("\n```\n");
            }
        }

        // External links section (0-10 links maximum)
        if !entity.external_links.is_empty() {
            let limited_links: Vec<_> = entity.external_links.iter().take(10).collect();
            content.push_str("\n## External Resources\n\n");
            for link in limited_links {
                content.push_str(&format!(
                    "- [{}]({}) - {}\n",
                    escape_yaml(&link.description),
                    link.url,
                    escape_yaml(&link.description)
                ));
            }
        }

        content.push_str("\n## Sources\n\n");
        let source_link = slugify(source_name);
        content.push_str(&format!(
            "- [{}](../sources/{}.md)\n",
            source_name, source_link
        ));

        fs::write(&path, &content).await?;
    }

    Ok(filename)
}

/// Write a concept page. Appends to existing page if the concept already exists.
///
/// Only emits "Related" links for concepts that exist on disk or are being
/// created in the same batch (`batch_slugs`).
///
/// # Returns
/// The filename of the generated page.
async fn write_concept_page(
    dir: &Path,
    slug: &str,
    concept: &ExtractedConcept,
    source_name: &str,
    batch_slugs: &HashSet<String>,
) -> crate::Result<String> {
    let filename = format!("{slug}.md");
    let path = dir.join(&filename);

    if path.exists() {
        let existing = fs::read_to_string(&path).await?;
        if !existing.contains(source_name) {
            let addition = format!("\n### From: {source_name}\n\n{}\n", concept.description);
            fs::write(&path, format!("{existing}{addition}")).await?;
        }
    } else {
        let mut content = String::new();
        content.push_str("---\n");
        content.push_str(&format!("title: \"{}\"\n", escape_yaml(&concept.name)));
        content.push_str("type: concept\n");
        content.push_str(&format!(
            "generated: \"{}\"\n",
            chrono::Utc::now().to_rfc3339()
        ));
        content.push_str("---\n\n");
        content.push_str(&format!("# {}\n\n", concept.name));
        content.push_str(&format!("### From: {source_name}\n\n"));
        content.push_str(&concept.description);
        content.push('\n');

        // Mermaid diagram section
        if let Some(ref diagram) = concept.diagram {
            let clean = strip_mermaid_fences(diagram);
            if !clean.is_empty() {
                content.push_str("\n## Diagram\n\n");
                content.push_str("```mermaid\n");
                content.push_str(clean);
                content.push_str("\n```\n");
            }
        }

        // External links section (0-10 links maximum)
        if !concept.external_links.is_empty() {
            // Limit to 10 links maximum
            let limited_links: Vec<_> = concept.external_links.iter().take(10).collect();
            content.push_str("\n## External Resources\n\n");
            for link in limited_links {
                content.push_str(&format!(
                    "- [{}]({}) - {}\n",
                    escape_yaml(&link.description),
                    link.url,
                    escape_yaml(&link.description)
                ));
            }
        }

        if !concept.related.is_empty() {
            // Only link to concepts that exist on disk or in the current batch
            let valid_related: Vec<_> = concept
                .related
                .iter()
                .filter(|rel| {
                    let rel_slug = slugify(rel);
                    // Skip self-references
                    if rel_slug == slug {
                        return false;
                    }
                    // Accept if in current batch or already on disk
                    batch_slugs.contains(&rel_slug) || dir.join(format!("{rel_slug}.md")).exists()
                })
                .collect();

            if !valid_related.is_empty() {
                content.push_str("\n## Related\n\n");
                for rel in &valid_related {
                    let link = slugify(rel);
                    content.push_str(&format!("- [{}]({}.md)\n", rel, link));
                }
            }
        }

        content.push_str("\n## Sources\n\n");
        let source_link = slugify(source_name);
        content.push_str(&format!(
            "- [{}](../sources/{}.md)\n",
            source_name, source_link
        ));

        fs::write(&path, &content).await?;
    }

    Ok(filename)
}

/// Convert a name to a URL-friendly slug.
fn slugify(name: &str) -> String {
    name.to_lowercase()
        .replace(|c: char| !c.is_alphanumeric() && c != '-', "-")
        .replace("--", "-")
        .trim_matches('-')
        .to_string()
}

/// Escape a string for YAML frontmatter values.
fn escape_yaml(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_mermaid_fences_with_mermaid_tag() {
        let input = "```mermaid\nflowchart LR\n  A --> B\n```";
        assert_eq!(strip_mermaid_fences(input), "flowchart LR\n  A --> B");
    }

    #[test]
    fn test_strip_mermaid_fences_bare_fences() {
        let input = "```\nflowchart LR\n  A --> B\n```";
        assert_eq!(strip_mermaid_fences(input), "flowchart LR\n  A --> B");
    }

    #[test]
    fn test_strip_mermaid_fences_no_fences() {
        let input = "flowchart LR\n  A --> B";
        assert_eq!(strip_mermaid_fences(input), "flowchart LR\n  A --> B");
    }

    #[test]
    fn test_strip_mermaid_fences_with_whitespace() {
        let input = "  ```mermaid\n  flowchart LR\n    A --> B\n  ```  ";
        assert_eq!(strip_mermaid_fences(input), "flowchart LR\n    A --> B");
    }

    #[test]
    fn test_strip_mermaid_fences_sequence_diagram() {
        let input = "```\nsequenceDiagram\n  A->>B: msg\n```";
        assert_eq!(strip_mermaid_fences(input), "sequenceDiagram\n  A->>B: msg");
    }

    #[test]
    fn test_strip_mermaid_fences_preserves_non_mermaid_code() {
        // Should not strip fences for non-mermaid code
        let input = "```\nlet x = 42;\n```";
        assert_eq!(strip_mermaid_fences(input), "```\nlet x = 42;\n```");
    }
}
