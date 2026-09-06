//! Competitive-analysis entity extractor for `/research --mode competitive`.
//!
//! Implements FR-006 and FR-011 of specs/opendeepresearch. The extractor turns a
//! free-form research topic into a list of comparable entities and any
//! comparison dimensions it can detect. It is intentionally deterministic and
//! does not require an LLM call, so the competitive-analysis supervisor can plan
//! sub-topics before any expensive web searches begin.
//!
//! The extractor handles two common input shapes:
//!
//! 1. **Explicit entity lists** — topics that name the entities to compare,
//!    usually separated by commas, "and", "vs", "versus", or "or".
//! 2. **Implicit competitive sets** — topics that describe a market or category
//!    without naming individual competitors. In that case the extractor infers a
//!    default competitive set from category keywords.
//!
//! Detected comparison dimensions (criteria) are also surfaced so downstream
//! synthesizers can build a cross-entity comparison table with explicit axes.

use std::collections::HashSet;

/// Maximum entities the heuristic will return. A small cap keeps the competitive
/// report focused and avoids over-delegation in supervisor mode.
pub const DEFAULT_MAX_ENTITIES: usize = 6;

/// One entity selected for a competitive-analysis report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompetitiveEntity {
    /// Human-readable entity name, e.g. "Fireworks AI".
    pub name: String,
    /// Optional category label when the extractor inferred the entity from a
    /// broad topic, e.g. "inference provider".
    pub category: Option<String>,
}

/// Result of extracting a competitive set from a topic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityExtractionResult {
    /// Comparable entities identified for the topic.
    pub entities: Vec<CompetitiveEntity>,
    /// Comparison criteria/dimensions detected in the topic.
    pub criteria: Vec<String>,
    /// `true` when no explicit entities were named and the set was inferred.
    pub inferred: bool,
}

/// Extract entities and comparison criteria for a competitive-analysis run.
///
/// The function first looks for explicitly named competitors. If fewer than two
/// are found, it falls back to an inferred competitive set based on category
/// keywords in the topic.
///
/// # Examples
///
/// * `"Compare Fireworks AI, Together.ai, and Groq for LLM inference"` →
///   entities `[Fireworks AI, Together.ai, Groq]`, criteria `[LLM inference]`.
/// * `"Research the inference market"` → inferred entities such as
///   `[Fireworks AI, Together.ai, Groq, Replicate, Banana.dev]`,
///   criteria `[inference market]`.
#[must_use]
pub fn extract_entities_for_competitive_analysis(topic: &str) -> EntityExtractionResult {
    let trimmed = topic.trim();
    if trimmed.is_empty() {
        return EntityExtractionResult {
            entities: Vec::new(),
            criteria: Vec::new(),
            inferred: false,
        };
    }

    let criteria = extract_comparison_criteria(trimmed);
    let mut entities = extract_explicit_entities(trimmed);
    let inferred = entities.len() < 2;

    if inferred {
        let inferred_names = infer_competitive_set(trimmed, DEFAULT_MAX_ENTITIES);
        entities = inferred_names
            .into_iter()
            .map(|name| CompetitiveEntity {
                name,
                category: detect_category(trimmed).map(String::from),
            })
            .collect();
    }

    EntityExtractionResult {
        entities,
        criteria,
        inferred,
    }
}

/// Split a topic into candidate entity phrases by common list separators.
fn split_list_items(topic: &str) -> Vec<String> {
    // Order matters: split on longer phrases first so "and" inside a name does
    // not fragment the list prematurely.
    let separators = [
        ", ",
        "; ",
        " and ",
        " plus ",
        " or ",
        " vs ",
        " versus ",
        " compared to ",
        " compared with ",
    ];
    let mut parts: Vec<String> = vec![topic.to_string()];
    for sep in &separators {
        let mut next = Vec::new();
        for part in &parts {
            for chunk in part.split(sep) {
                let trimmed = chunk
                    .trim()
                    .trim_start_matches("and ")
                    .trim_start_matches("or ")
                    .trim_start_matches("plus ")
                    .trim();
                if !trimmed.is_empty() {
                    next.push(trimmed.to_string());
                }
            }
        }
        parts = next;
    }
    parts
}

/// Remove leading comparison verbs/prefixes from a candidate entity phrase.
fn strip_comparison_prefixes(phrase: &str) -> String {
    let lower = phrase.to_lowercase();
    let prefixes = [
        "compare ",
        "comparing ",
        "contrast ",
        "contrasting ",
        "difference between ",
        "differences between ",
        "difference of ",
        "between ",
    ];
    for prefix in &prefixes {
        if lower.starts_with(prefix) {
            return phrase[prefix.len()..].trim().to_string();
        }
    }
    phrase.trim().to_string()
}

/// Remove trailing prepositional clauses that describe comparison dimensions.
fn strip_dimension_suffixes(phrase: &str) -> String {
    let separators = [
        " for ",
        " in ",
        " on ",
        " with ",
        " at ",
        " as ",
        " regarding ",
    ];
    let mut result = phrase.trim().to_string();
    for sep in &separators {
        if let Some(idx) = result.to_lowercase().rfind(sep) {
            result.truncate(idx);
        }
    }
    result
        .trim()
        .trim_end_matches(|c: char| c.is_ascii_punctuation())
        .to_string()
}

/// Extract entities explicitly named in the topic.
fn extract_explicit_entities(topic: &str) -> Vec<CompetitiveEntity> {
    let core = strip_comparison_prefixes(topic);
    let core = strip_dimension_suffixes(&core);
    let items = split_list_items(&core);

    let stop_words: HashSet<&str> = [
        "the",
        "a",
        "an",
        "this",
        "that",
        "these",
        "those",
        "compare",
        "comparing",
        "contrast",
        "versus",
        "vs",
        "and",
        "or",
        "for",
        "with",
        "between",
        "in",
        "on",
        "of",
        "to",
        "market",
        "landscape",
        "analysis",
        "report",
        "review",
        "overview",
    ]
    .iter()
    .copied()
    .collect();

    let mut entities = Vec::new();
    let mut seen = HashSet::new();

    for item in items {
        let normalized = normalize_entity_name(&item);
        if normalized.split_whitespace().count() > 3 {
            // Long fragments are usually descriptive phrases, not entity names.
            continue;
        }
        let lower = normalized.to_lowercase();
        if normalized.len() < 2 || stop_words.contains(lower.as_str()) {
            continue;
        }
        if seen.insert(lower.clone()) {
            entities.push(CompetitiveEntity {
                name: normalized,
                category: None,
            });
        }
    }

    entities
}

/// Normalize whitespace and strip surrounding punctuation from a candidate
/// entity name while preserving internal punctuation such as dots and dashes.
fn normalize_entity_name(name: &str) -> String {
    let trimmed = name
        .trim()
        .trim_start_matches(|c: char| c.is_ascii_punctuation() || c == '"' || c == '\'')
        .trim_end_matches(|c: char| c.is_ascii_punctuation() || c == '"' || c == '\'')
        .to_string();
    trimmed.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Infer a default competitive set when the topic names no explicit entities.
///
/// The inference is keyword-driven. It looks for category markers such as
/// "inference", "LLM", "vector database", or "browser" and returns a
/// reasonable default set of well-known competitors. If no keyword matches, a
/// generic set of category placeholders is returned so downstream agents still
/// have something to research.
#[must_use]
pub fn infer_competitive_set(topic: &str, max_entities: usize) -> Vec<String> {
    let lower = topic.to_lowercase();
    let defaults: Vec<&str> = if lower.contains("llm inference") || lower.contains("inference") {
        vec![
            "Fireworks AI",
            "Together.ai",
            "Groq",
            "Replicate",
            "Banana.dev",
            "Baseten",
        ]
    } else if lower.contains("vector database") || lower.contains("vector db") {
        vec![
            "Pinecone", "Weaviate", "Milvus", "Qdrant", "Chroma", "pgvector",
        ]
    } else if lower.contains("browser") || lower.contains("web browser") {
        vec!["Chrome", "Firefox", "Safari", "Edge", "Arc", "Brave"]
    } else if lower.contains("cloud") && lower.contains("provider") {
        vec![
            "AWS",
            "Azure",
            "Google Cloud",
            "Oracle Cloud",
            "IBM Cloud",
            "DigitalOcean",
        ]
    } else if lower.contains("database") || lower.contains("db") {
        vec![
            "PostgreSQL",
            "MySQL",
            "MongoDB",
            "SQLite",
            "CockroachDB",
            "TiDB",
        ]
    } else if lower.contains("search engine") || lower.contains("search") {
        vec![
            "Google",
            "Bing",
            "DuckDuckGo",
            "Perplexity",
            "Kagi",
            "Brave Search",
        ]
    } else if lower.contains("code editor") || lower.contains("ide") {
        vec![
            "VS Code",
            "JetBrains",
            "Zed",
            "Cursor",
            "Sublime Text",
            "Neovim",
        ]
    } else if lower.contains("rust") && lower.contains("runtime") {
        vec!["Tokio", "async-std", "smol", "Actix", "embassy"]
    } else {
        vec!["Option A", "Option B", "Option C"]
    };

    defaults
        .into_iter()
        .take(max_entities.max(2))
        .map(|s| {
            let trimmed = s.trim().to_string();
            if trimmed.is_empty() {
                "Unknown".to_string()
            } else {
                trimmed
            }
        })
        .collect()
}

/// Detect a category label from the topic when entities are inferred.
fn detect_category(topic: &str) -> Option<&'static str> {
    let lower = topic.to_lowercase();
    if lower.contains("llm inference") || lower.contains("inference") {
        Some("inference provider")
    } else if lower.contains("vector database") || lower.contains("vector db") {
        Some("vector database")
    } else if lower.contains("browser") || lower.contains("web browser") {
        Some("web browser")
    } else if lower.contains("cloud") && lower.contains("provider") {
        Some("cloud provider")
    } else if lower.contains("database") || lower.contains("db") {
        Some("database")
    } else if lower.contains("search engine") || lower.contains("search") {
        Some("search engine")
    } else if lower.contains("code editor") || lower.contains("ide") {
        Some("code editor/IDE")
    } else if lower.contains("rust") && lower.contains("runtime") {
        Some("Rust async runtime")
    } else {
        None
    }
}

/// Extract comparison criteria/dimensions from the topic.
///
/// The heuristic looks for trailing prepositional phrases ("for X", "in Y",
/// "regarding Z") and for common comparison keywords. When neither yields a
/// dimension, a generic attribute grid (licensing/pricing, model support,
/// deployment, UX, quality) is returned so competitive runs always render
/// per-attribute comparison columns (FR-016) instead of a bare Entity/Profile
/// listing.
#[must_use]
pub fn extract_comparison_criteria(topic: &str) -> Vec<String> {
    let lower = topic.to_lowercase();
    let mut criteria = Vec::new();

    // Trailing dimension phrase.
    let dimension_separators = [" for ", " in ", " on ", " regarding ", " about "];
    for sep in &dimension_separators {
        if let Some(idx) = lower.rfind(sep) {
            let dim = topic[idx + sep.len()..]
                .trim()
                .trim_end_matches(|c: char| c.is_ascii_punctuation())
                .to_string();
            if !dim.is_empty() && dim.len() < 80 {
                criteria.push(dim);
            }
            break;
        }
    }

    // Comparison keywords that imply axes.
    let keyword_axes: [(&str, &str); 5] = [
        ("speed", "speed/latency"),
        ("latency", "speed/latency"),
        ("price", "pricing"),
        ("cost", "pricing"),
        ("features", "features"),
    ];
    for (keyword, axis) in &keyword_axes {
        if lower.contains(keyword) && !criteria.iter().any(|c| c.contains(axis)) {
            criteria.push(axis.to_string());
        }
    }

    // Generic attribute-grid fallback: a plain "compare A, B, C" topic names
    // no dimensions, but the comparison table still needs per-attribute
    // columns and per-entity researchers still need dimension guidance
    // (FR-016).
    if criteria.is_empty() {
        criteria.push("licensing and pricing".to_string());
        criteria.push("model and provider support".to_string());
        criteria.push("deployment and integration".to_string());
        criteria.push("UX and workflow".to_string());
        criteria.push("quality and performance".to_string());
    }

    criteria
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_topic_returns_empty_result() {
        let result = extract_entities_for_competitive_analysis("");
        assert_eq!(result.entities, Vec::new());
        assert_eq!(result.criteria, Vec::<String>::new());
        assert!(!result.inferred);
    }

    #[test]
    fn explicit_entities_are_extracted() {
        let result = extract_entities_for_competitive_analysis(
            "Compare Fireworks AI, Together.ai, and Groq for LLM inference",
        );
        assert!(!result.inferred);
        let names: Vec<_> = result.entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"Fireworks AI"), "got {names:?}");
        assert!(names.contains(&"Together.ai"), "got {names:?}");
        assert!(names.contains(&"Groq"), "got {names:?}");
        assert!(result.criteria.iter().any(|c| c.contains("LLM inference")));
    }

    #[test]
    fn explicit_entities_with_vs() {
        let result = extract_entities_for_competitive_analysis("AWS vs Azure vs Google Cloud");
        assert!(!result.inferred);
        let names: Vec<_> = result.entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"AWS"), "got {names:?}");
        assert!(names.contains(&"Azure"), "got {names:?}");
        assert!(names.contains(&"Google Cloud"), "got {names:?}");
    }

    #[test]
    fn inferred_entities_when_none_named() {
        let result = extract_entities_for_competitive_analysis("Research the inference market");
        assert!(result.inferred);
        assert!(
            result.entities.len() >= 2,
            "expected at least two inferred entities, got {:?}",
            result.entities
        );
        let names: Vec<_> = result.entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"Fireworks AI"), "got {names:?}");
        assert!(names.contains(&"Groq"), "got {names:?}");
        assert_eq!(
            result.entities[0].category.as_deref(),
            Some("inference provider")
        );
    }

    #[test]
    fn inferred_entities_for_vector_databases() {
        let result = extract_entities_for_competitive_analysis("Compare vector database options");
        assert!(result.inferred);
        let names: Vec<_> = result.entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"Pinecone"), "got {names:?}");
        assert!(names.contains(&"Qdrant"), "got {names:?}");
    }

    #[test]
    fn comparison_criteria_detected() {
        let result = extract_entities_for_competitive_analysis(
            "Compare AWS, Azure, and Google Cloud for pricing and speed",
        );
        let criteria = result.criteria.join(", ");
        assert!(
            criteria.contains("pricing") || criteria.contains("speed/latency"),
            "criteria missing pricing/speed: {criteria}"
        );
    }

    #[test]
    fn single_entity_falls_back_to_inference() {
        let result = extract_entities_for_competitive_analysis("How does Groq compare?");
        assert!(result.inferred);
        assert!(result.entities.len() >= 2);
    }

    #[test]
    fn infer_competitive_set_caps_at_max() {
        let set = infer_competitive_set("Research the inference market", 3);
        assert_eq!(set.len(), 3);
    }
}
