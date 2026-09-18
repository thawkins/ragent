//! Architecture-structure extraction for `/spec govcreate` (spec `govdoc` T-008).
//!
//! Implements **FR-007**: given a non-empty [`GatheredCorpus`] (produced by the
//! URL acquisition in T-006 or the local acquisition in T-007), extract the
//! *architecture structure* - named components, their responsibilities,
//! interfaces/contracts, data stores, external dependencies, and the
//! relationships between components.
//!
//! # Two halves (assumption A4)
//!
//! Extraction is LLM-driven, bounded by deterministic pre-processing:
//!
//! 1. [`build_arch_extraction_prompt`] renders a prompt that embeds the bounded
//!    gathered text and demands a strict JSON response matching the
//!    [`ArchitectureStructure`] contract. The authoring stage (T-009) hands this
//!    prompt to the configured model.
//! 2. [`parse_architecture_response`] turns the model's response back into a
//!    typed [`ArchitectureStructure`], tolerating the usual model noise
//!    (markdown code fences, prose before/after the JSON object).
//!
//! # Deterministic fallback (rescue path)
//!
//! When the model output does not parse - or no model was available -
//! [`fallback_structure`] derives a minimal structure mechanically from the
//! source inventory (source paths/URLs, file stems, first-level headings) so
//! the authoring stage always proceeds and never panics. This mirrors the
//! research pipeline's mechanical-fallback precedent
//! (`ragent-research analysis/parser.rs`). [`ArchitectureStructure::from_llm`]
//! records which path produced the structure so the report can surface it.
//!
//! # Testability (NFR-002)
//!
//! Every item in this module is pure: the prompt builder, the parser, the
//! heading extraction, and the fallback all operate on plain data with no I/O
//! and no network access.

use std::collections::BTreeSet;

use serde::Deserialize;

use super::GatheredCorpus;

// ---------------------------------------------------------------------------
// Extraction prompt (FR-007)
// ---------------------------------------------------------------------------

/// Maximum characters of gathered corpus text embedded in the extraction
/// prompt.
///
/// The corpus is already bounded by the acquisition budgets (FR-016; default
/// 200k characters), but the model context window is smaller than that, so the
/// prompt truncates the embedded text deterministically at this cap.
pub const MAX_PROMPT_CORPUS_CHARS: usize = 64_000;

/// Build the architecture-extraction prompt for one gathered corpus (FR-007).
///
/// The returned prompt asks the model to respond with **one JSON object**
/// matching the [`ArchitectureStructure`] contract: named `components` with
/// their `responsibilities`, `interfaces`, `data_stores`,
/// `external_dependencies`, and `relationships` between components. The
/// gathered text is embedded under a numbered source inventory so the model
/// can attribute structure to sources, and is truncated at
/// [`MAX_PROMPT_CORPUS_CHARS`].
#[must_use]
pub fn build_arch_extraction_prompt(corpus: &GatheredCorpus) -> String {
    let mut inventory = String::new();
    for (index, source) in corpus.sources.iter().enumerate() {
        inventory.push_str(&format!(
            "{}. {} [{}]\n",
            index + 1,
            source.url,
            source.summary
        ));
    }

    let body = truncate_chars(&corpus.text, MAX_PROMPT_CORPUS_CHARS);

    format!(
        "You are a software architect. Read the gathered architecture documentation below \
         and extract the SYSTEM ARCHITECTURE STRUCTURE.\n\
         \n\
         Content reference: {reference}\n\
         Gathered sources: {source_count} ({total_chars} chars)\n\
         \n\
         Sources:\n\
         {inventory}\
         \n\
         Identify at minimum:\n\
         - named components (modules, services, subsystems) and their responsibilities\n\
         - interfaces or contracts between components (APIs, protocols, message formats)\n\
         - data stores (databases, files, queues, caches)\n\
         - external dependencies (third-party systems, libraries, services)\n\
         - relationships between components (who calls/reads/writes/depends on whom)\n\
         \n\
         Respond with ONE JSON object only - no prose, no markdown code fences - with \
         exactly these fields:\n\
         {{\n\
           \"components\": [{{\"name\": \"...\", \"responsibilities\": [\"...\"]}}],\n\
           \"interfaces\": [{{\"name\": \"...\", \"between\": [\"component-a\", \"component-b\"], \
         \"contract\": \"...\"}}],\n\
           \"data_stores\": [{{\"name\": \"...\", \"kind\": \"database|file|queue|cache|...\", \
         \"used_by\": [\"component\"]}}],\n\
           \"external_dependencies\": [{{\"name\": \"...\", \"kind\": \"service|library|system\", \
         \"used_by\": [\"component\"]}}],\n\
           \"relationships\": [{{\"from\": \"component\", \"to\": \"component\", \
         \"kind\": \"calls|reads|writes|depends-on\", \"detail\": \"...\"}}]\n\
         }}\n\
         Omit any field that would be an empty list. Use short, concrete names drawn from \
         the documentation. If the documentation names no architecture, return an empty \
         JSON object `{{}}` rather than inventing components.\n\
         \n\
         --- GATHERED DOCUMENTATION ---\n\
         {body}\n\
         --- END GATHERED DOCUMENTATION ---",
        reference = corpus.reference,
        source_count = corpus.sources.len(),
        total_chars = corpus.stats.total_chars,
        inventory = inventory,
        body = body,
    )
}

// ---------------------------------------------------------------------------
// Structure model (FR-007)
// ---------------------------------------------------------------------------

/// One named component of the system (FR-007).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Component {
    /// The component name as it appears in the documentation.
    pub name: String,
    /// What the component is responsible for.
    pub responsibilities: Vec<String>,
}

/// An interface or contract between components (FR-007).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Interface {
    /// Interface name (API, protocol, or message format).
    pub name: String,
    /// The components this interface connects.
    pub between: Vec<String>,
    /// A short description of the contract.
    #[serde(rename = "contract")]
    pub contract_desc: String,
}

/// A data store used by the system (FR-007).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct DataStore {
    /// Store name.
    pub name: String,
    /// Store kind (database, file, queue, cache, ...).
    pub kind: String,
    /// Components that use this store.
    pub used_by: Vec<String>,
}

/// An external dependency of the system (FR-007).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ExternalDependency {
    /// Dependency name.
    pub name: String,
    /// Dependency kind (service, library, external system).
    pub kind: String,
    /// Components that depend on it.
    pub used_by: Vec<String>,
}

/// A relationship between two components (FR-007).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Relationship {
    /// The component the relationship starts from.
    pub from: String,
    /// The component the relationship points at.
    pub to: String,
    /// The relationship kind (calls, reads, writes, depends-on, ...).
    pub kind: String,
    /// A short description of the relationship.
    pub detail: String,
}

/// The extracted architecture structure (FR-007).
///
/// This is the serde-deserialisable JSON contract the model is asked to
/// produce; `Default`-derived empty fields let the model omit empty lists.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ArchitectureStructure {
    /// Named components with their responsibilities.
    pub components: Vec<Component>,
    /// Interfaces/contracts between components.
    pub interfaces: Vec<Interface>,
    /// Data stores.
    pub data_stores: Vec<DataStore>,
    /// External dependencies.
    pub external_dependencies: Vec<ExternalDependency>,
    /// Relationships between components.
    pub relationships: Vec<Relationship>,
    /// `true` when this structure came from [`fallback_structure`] rather than
    /// a clean LLM parse. The fallback sets this itself; the field is not part
    /// of the model's JSON contract and is skipped on deserialise.
    #[serde(skip)]
    pub from_fallback: bool,
}

impl ArchitectureStructure {
    /// `true` when the structure carries no extracted information at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
            && self.interfaces.is_empty()
            && self.data_stores.is_empty()
            && self.external_dependencies.is_empty()
            && self.relationships.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Response parsing
// ---------------------------------------------------------------------------

/// Parse a model response into an [`ArchitectureStructure`].
///
/// The model is asked for one bare JSON object, but responses commonly wrap it
/// in markdown code fences or lead/trail with prose, so this parses the first
/// `{` .. last `}` span of the response. Returns `None` when no JSON object
/// parses as the contract; the caller then falls back to
/// [`fallback_structure`].
#[must_use]
pub fn parse_architecture_response(response: &str) -> Option<ArchitectureStructure> {
    let start = response.find('{')?;
    let end = response.rfind('}')?;
    if end <= start {
        return None;
    }
    match serde_json::from_str::<ArchitectureStructure>(&response[start..=end]) {
        Ok(structure) => Some(structure),
        Err(e) => {
            // A prose `{` before the actual JSON (e.g. "shape {components: …}")
            // widens the span past the real object; log the offsets so a
            // fallback-heavy run shows whether extraction or parsing failed.
            tracing::debug!(
                span_start = start,
                span_len = end - start + 1,
                error = %e,
                "architecture response did not parse; caller falls back to the \
                 deterministic structure"
            );
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Deterministic fallback
// ---------------------------------------------------------------------------

/// Maximum heading lines harvested from the corpus text for the fallback.
const MAX_FALLBACK_HEADINGS: usize = 20;

/// Maximum fallback components derived from the source inventory.
const MAX_FALLBACK_COMPONENTS: usize = 25;

/// Derive a minimal [`ArchitectureStructure`] mechanically from the corpus
/// source inventory (FR-007 rescue path).
///
/// Each gathered source contributes one candidate component named after its
/// file stem (local paths) or last URL-segment (web pages); the first
/// `#`-style heading lines of each source's content become that candidate's
/// responsibilities. Names are de-duplicated case-insensitively and the
/// fallback is capped at [`MAX_FALLBACK_COMPONENTS`] components. The result
/// always has `from_fallback: true`.
///
/// The fallback never panics and never fabricates interfaces, data stores, or
/// relationships: it only names what the inventory demonstrably contains, so
/// the authoring stage can proceed with an honest, if coarse, structure.
#[must_use]
pub fn fallback_structure(corpus: &GatheredCorpus) -> ArchitectureStructure {
    let mut seen = BTreeSet::new();
    let mut components = Vec::new();

    for source in &corpus.sources {
        if components.len() >= MAX_FALLBACK_COMPONENTS {
            break;
        }
        let Some(name) = source_component_name(&source.url) else {
            continue;
        };
        if !seen.insert(name.to_ascii_lowercase()) {
            continue;
        }
        components.push(Component {
            name,
            responsibilities: first_headings(&source.content, MAX_FALLBACK_HEADINGS),
        });
    }

    ArchitectureStructure {
        components,
        from_fallback: true,
        ..ArchitectureStructure::default()
    }
}

/// Derive a component name from a source locator: the file stem for local
/// paths, or the last non-empty URL path segment for web URLs. Returns `None`
/// when no usable name exists.
fn source_component_name(locator: &str) -> Option<String> {
    let trimmed = locator.trim().trim_end_matches('/');
    let without_query = trimmed.split(['?', '#']).next().unwrap_or(trimmed);
    let segment = without_query.rsplit('/').next()?;
    let stem = segment.rsplit_once('.').map_or(segment, |(stem, _)| stem);
    let cleaned: String = stem
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let name = cleaned.trim_matches('-');
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// Collect the first markdown-style heading text lines (`#`, `##`, `###`)
/// from `content`, capped at `limit` entries.
fn first_headings(content: &str, limit: usize) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let heading = line.strip_prefix('#')?;
            // Only real headings: `#` followed by space or more `#` then space.
            let rest = heading.trim_start_matches('#');
            let text = rest.strip_prefix(' ')?;
            let text = text.trim();
            if text.is_empty() {
                None
            } else {
                Some(text.to_string())
            }
        })
        .take(limit)
        .collect()
}

/// Truncate `text` to at most `max` characters (UTF-8 boundary safe).
fn truncate_chars(text: &str, max: usize) -> &str {
    if text.chars().count() <= max {
        text
    } else {
        text.char_indices()
            .nth(max)
            .map_or(text, |(i, _)| &text[..i])
    }
}
