//! LLM-based content extraction for AIWiki.
//!
//! Defines the `LlmExtractor` trait that callers implement to provide LLM access,
//! prompt templates for structured extraction, and response parsing.

use serde::{Deserialize, Serialize};

/// Trait for making LLM completion calls.
///
/// Implemented by the TUI layer using the active provider/model.
/// The aiwiki crate itself has no direct LLM dependency.
#[async_trait::async_trait]
pub trait LlmExtractor: Send + Sync {
    /// Send a prompt to the LLM and return the full text response.
    ///
    /// # Arguments
    /// * `system` - System prompt setting the extraction context
    /// * `user` - User prompt containing the document text and instructions
    /// * `max_tokens` - Maximum tokens for the response
    /// * `temperature` - Sampling temperature (0.0 for deterministic)
    ///
    /// # Returns
    /// The complete text response from the LLM.
    async fn complete(
        &self,
        system: &str,
        user: &str,
        max_tokens: u32,
        temperature: f32,
    ) -> Result<String, String>;
}

/// Result of extracting content from a source document via LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    /// Summary of the source document.
    pub summary: String,
    /// Title derived from the content.
    pub title: String,
    /// Key entities extracted (people, organizations, places, etc.).
    pub entities: Vec<ExtractedEntity>,
    /// Key concepts extracted (ideas, theories, technologies, etc.).
    pub concepts: Vec<ExtractedConcept>,
    /// Suggested tags/categories.
    pub tags: Vec<String>,
}

/// An entity extracted from source text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedEntity {
    /// Entity name.
    pub name: String,
    /// Entity type (person, organization, place, technology, etc.).
    pub entity_type: String,
    /// Comprehensive description of the entity (2-5 paragraphs minimum).
    pub description: String,
    /// External web links supporting the description (0-10 links).
    #[serde(default)]
    pub external_links: Vec<ExternalLink>,
    /// Optional Mermaid diagram code to visualize the entity.
    #[serde(default)]
    pub diagram: Option<String>,
}

/// A concept extracted from source text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedConcept {
    /// Concept name.
    pub name: String,
    /// Explanation of the concept (2-5 paragraphs minimum).
    pub description: String,
    /// Related entity or concept names for cross-linking.
    pub related: Vec<String>,
    /// External web links supporting the description (0-10 links).
    pub external_links: Vec<ExternalLink>,
    /// Optional Mermaid diagram code to visualize the concept.
    #[serde(default)]
    pub diagram: Option<String>,
}

/// An external link supporting a concept description.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalLink {
    /// URL of the external resource.
    pub url: String,
    /// Brief description of what the link provides.
    pub description: String,
}

/// System prompt for the extraction LLM call.
pub const EXTRACTION_SYSTEM_PROMPT: &str = r#"You are a knowledge extraction assistant. Your job is to analyze source documents and extract structured information for a wiki knowledge base.

You MUST respond with valid JSON only. No markdown, no explanation, no commentary outside the JSON object.

Content Requirements:
- All descriptions MUST be 2-5 paragraphs (at least 3-4 sentences per paragraph)
- Include specific details, examples, context, and depth
- Avoid single-line or minimal descriptions
- Add 0-10 external web links (sources, documentation, references) to support descriptions
- Include relevant Mermaid diagrams to visualize relationships, workflows, or architectures
  Supported diagram types: flowchart, sequence, class, state, er, gantt, pie, mindmap
  Use proper Mermaid syntax, NOT ASCII art
  Mermaid rules:
  - Use simple alphanumeric node IDs (e.g. node1, wildcardStar) — never bare special characters
  - Wrap display text containing special characters in square brackets: id["text with * or ?"]
  - Never use *, **, ?, {}, or file paths as bare node identifiers
  - For generics like Option<T>, use tilde notation: Option~T~
  - Prefer flowchart over mindmap for complex diagrams

For concepts:
- Each concept needs 2-5 paragraphs with examples and context
- Optionally include a diagram field with Mermaid code

For entities:
- Each entity needs 2-5 paragraphs describing its significance, history, and context
- Optionally include a diagram field with Mermaid code

For sources:
- The summary must be 2-4 paragraphs with comprehensive coverage"#;

/// Build the user prompt for extraction.
///
/// # Arguments
/// * `filename` - Name of the source file
/// * `text` - Extracted text content from the document
/// * `extract_entities` - Whether to extract entities
/// * `extract_concepts` - Whether to extract concepts
pub fn build_extraction_prompt(
    filename: &str,
    text: &str,
    extract_entities: bool,
    extract_concepts: bool,
) -> String {
    let truncated = if text.len() > 30_000 {
        &text[..30_000]
    } else {
        text
    };

    let mut instructions =
        String::from("Analyze the following document and extract structured information.\n\n");
    instructions.push_str(&format!("Source file: {filename}\n\n"));
    instructions.push_str("Return a JSON object with these fields:\n");
    instructions.push_str("- \"title\": a concise title for this document\n");
    instructions.push_str("- \"summary\": a comprehensive summary (2-4 paragraphs)\n");
    instructions.push_str("- \"tags\": array of relevant tags/categories\n");

    if extract_entities {
        instructions.push_str(
                  "- \"entities\": array of {\"name\": str, \"entity_type\": str, \"description\": str, \"external_links\": [{\"url\": str, \"description\": str}], \"diagram\": str (optional)}\n"
              );
        instructions
            .push_str("  Entity types: person, organization, place, technology, product, event\n");
        instructions.push_str(
            "  Each entity MUST have 2-5 paragraphs (at least 3-4 sentences per paragraph).\n",
        );
        instructions.push_str("  Include specific details, history, significance, and context.\n");
        instructions
            .push_str("  Optionally include 0-10 external web links and a Mermaid diagram.\n");
    }
    if extract_concepts {
        instructions.push_str(
            "- \"concepts\": array of {\"name\": str, \"description\": str, \"related\": [str], \"external_links\": [{\"url\": str, \"description\": str}]\n"
        );
        instructions.push_str(
            "  Concepts are ideas, theories, methodologies, or abstract topics discussed.\n",
        );
        instructions.push_str(
            "  Each concept MUST have 2-5 paragraphs (at least 3-4 sentences per paragraph).\n",
        );
        instructions.push_str(
            "  Include specific details, examples, and context - no single-line descriptions.\n",
        );
        instructions.push_str(
            "  Optionally include 0-10 external web links with brief descriptions to support the content.\n"
        );
    }

    instructions.push_str("\nDocument text:\n---\n");
    instructions.push_str(truncated);
    instructions.push_str("\n---\n");

    instructions
}

/// Parse the LLM JSON response into an `ExtractionResult`.
///
/// Handles common issues like markdown code fences around JSON.
///
/// # Errors
/// Returns an error string if the response cannot be parsed.
pub fn parse_extraction_response(response: &str) -> Result<ExtractionResult, String> {
    // Strip markdown code fences if present
    let json_str = response
        .trim()
        .strip_prefix("```json")
        .or_else(|| response.trim().strip_prefix("```"))
        .unwrap_or(response.trim());
    let json_str = json_str.strip_suffix("```").unwrap_or(json_str).trim();

    // Try parsing the full structure
    if let Ok(result) = serde_json::from_str::<ExtractionResult>(json_str) {
        return Ok(result);
    }

    // Fallback: parse as generic JSON value and extract fields manually
    let value: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse LLM response as JSON: {e}\nResponse: {json_str}"))?;

    let title = value
        .get("title")
        .and_then(|v| v.as_str())
        .unwrap_or("Untitled")
        .to_string();

    let summary = value
        .get("summary")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let tags: Vec<String> = value
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let entities: Vec<ExtractedEntity> = value
        .get("entities")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    let concepts: Vec<ExtractedConcept> = value
        .get("concepts")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();

    Ok(ExtractionResult {
        title,
        summary,
        entities,
        concepts,
        tags,
    })
}
