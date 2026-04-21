//! Knowledge graph memory system.
//!
//! Extracts entities and relationships from memory content to build a
//! knowledge graph. Entities are projects, tools, patterns, people, or
//! concepts mentioned in memories. Relationships connect entities with
//! typed edges (uses, prefers, depends_on, avoids, etc.).
//!
//! # Storage
//!
//! Entities and relationships are stored in SQLite tables alongside the
//! existing `memories` table. This enables graph-based retrieval alongside
//! vector and FTS search.
//!
//! # Entity extraction
//!
//! Entities are extracted using simple pattern matching and heuristics:
//! - Tool/language names (Rust, Python, Docker, etc.)
//! - Project names (directory-like references)
//! - Pattern references ("X pattern", "Y convention")
//!
//! Relationships are derived from memory categories and tags:
//! - `uses`: when a memory mentions using a tool/technology
//! - `prefers`: when a memory is in the "preference" category
//! - `depends_on`: when a memory describes a dependency
//! - `avoids`: when a memory is in the "error" category about avoiding something
//! - `related_to`: generic connection between entities

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::storage::Storage;

// ── Entity types ──────────────────────────────────────────────────────────────

/// The kind of entity extracted from memory content.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    /// A project or codebase.
    Project,
    /// A tool, library, or framework.
    Tool,
    /// A programming language.
    Language,
    /// A design pattern or convention.
    Pattern,
    /// A person or team.
    Person,
    /// A concept or topic.
    Concept,
}

impl EntityType {
    /// Parse from a string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "project" => Some(Self::Project),
            "tool" => Some(Self::Tool),
            "language" => Some(Self::Language),
            "pattern" => Some(Self::Pattern),
            "person" => Some(Self::Person),
            "concept" => Some(Self::Concept),
            _ => None,
        }
    }

    /// Convert to a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Tool => "tool",
            Self::Language => "language",
            Self::Pattern => "pattern",
            Self::Person => "person",
            Self::Concept => "concept",
        }
    }
}

impl std::fmt::Display for EntityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The type of relationship between two entities.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RelationType {
    /// Entity A uses entity B.
    Uses,
    /// Entity A prefers entity B.
    Prefers,
    /// Entity A depends on entity B.
    DependsOn,
    /// Entity A avoids entity B.
    Avoids,
    /// Generic relationship.
    RelatedTo,
}

impl RelationType {
    /// Parse from a string.
    pub fn from_str_lossy(s: &str) -> Self {
        match s {
            "uses" => Self::Uses,
            "prefers" => Self::Prefers,
            "depends_on" => Self::DependsOn,
            "avoids" => Self::Avoids,
            _ => Self::RelatedTo,
        }
    }

    /// Convert to a string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Uses => "uses",
            Self::Prefers => "prefers",
            Self::DependsOn => "depends_on",
            Self::Avoids => "avoids",
            Self::RelatedTo => "related_to",
        }
    }
}

impl std::fmt::Display for RelationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An entity in the knowledge graph.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    /// Unique identifier.
    pub id: i64,
    /// The entity name (e.g., "Rust", "Docker", "TDD").
    pub name: String,
    /// The type of entity.
    pub entity_type: String,
    /// Number of memories mentioning this entity.
    pub mention_count: i64,
    /// ISO 8601 timestamp when first seen.
    pub created_at: String,
    /// ISO 8601 timestamp when last updated.
    pub updated_at: String,
}

/// A relationship between two entities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    /// Unique identifier.
    pub id: i64,
    /// Source entity ID.
    pub source_id: i64,
    /// Target entity ID.
    pub target_id: i64,
    /// Relationship type.
    pub relation_type: String,
    /// Confidence in this relationship (0.0–1.0).
    pub confidence: f64,
    /// The memory ID that established this relationship.
    pub source_memory_id: Option<i64>,
    /// ISO 8601 timestamp when first seen.
    pub created_at: String,
}

/// Result of extracting entities from a memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    /// Entities found in the memory content.
    pub entities: Vec<ExtractedEntity>,
    /// Relationships inferred between entities.
    pub relationships: Vec<ExtractedRelationship>,
}

/// An entity extracted from a single memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedEntity {
    /// The entity name.
    pub name: String,
    /// The inferred entity type.
    pub entity_type: EntityType,
}

/// A relationship extracted from a single memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedRelationship {
    /// Source entity name.
    pub source: String,
    /// Target entity name.
    pub target: String,
    /// Relationship type.
    pub relation_type: RelationType,
}

/// Knowledge graph data for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraph {
    /// All entities in the graph.
    pub entities: Vec<Entity>,
    /// All relationships in the graph.
    pub relationships: Vec<Relationship>,
}

// ── Known entity patterns ─────────────────────────────────────────────────────

/// Languages we recognise for entity extraction.
const KNOWN_LANGUAGES: &[&str] = &[
    "Rust",
    "Python",
    "JavaScript",
    "TypeScript",
    "Go",
    "Java",
    "C",
    "C++",
    "C#",
    "Ruby",
    "PHP",
    "Swift",
    "Kotlin",
    "Scala",
    "Haskell",
    "Elixir",
    "Clojure",
    "Lua",
    "R",
    "SQL",
    "Shell",
    "Bash",
];

/// Tools/frameworks we recognise for entity extraction.
const KNOWN_TOOLS: &[&str] = &[
    "Docker",
    "Kubernetes",
    "K8s",
    "Git",
    "GitHub",
    "GitLab",
    "Terraform",
    "Ansible",
    "Jenkins",
    "CircleCI",
    "Nginx",
    "Apache",
    "Redis",
    "PostgreSQL",
    "MySQL",
    "SQLite",
    "MongoDB",
    "Elasticsearch",
    "Kafka",
    "RabbitMQ",
    "React",
    "Vue",
    "Angular",
    "Svelte",
    "Next.js",
    "Nuxt",
    "Django",
    "Flask",
    "FastAPI",
    "Express",
    "Actix",
    "Tokio",
    "Axum",
    "Cargo",
    "Clippy",
    "Rustfmt",
    "WebAssembly",
    "WASM",
];

// ── Entity extraction ────────────────────────────────────────────────────────

/// Extract entities and relationships from memory content.
///
/// Uses pattern matching and heuristics to identify known languages,
/// tools, and patterns, then infers relationships based on category
/// and context.
///
/// # Arguments
///
/// * `content` - The memory content text.
/// * `category` - The memory category (affects relationship inference).
/// * `tags` - Tags attached to the memory.
///
/// # Returns
///
/// An `ExtractionResult` containing extracted entities and relationships.
pub fn extract_entities(content: &str, category: &str, tags: &[String]) -> ExtractionResult {
    let mut entities: Vec<ExtractedEntity> = Vec::new();
    let mut relationships: Vec<ExtractedRelationship> = Vec::new();

    let content_lower = content.to_lowercase();

    // Extract known languages.
    for lang in KNOWN_LANGUAGES {
        let lang_lower = lang.to_lowercase();
        if content_lower.contains(&lang_lower) {
            entities.push(ExtractedEntity {
                name: lang.to_string(),
                entity_type: EntityType::Language,
            });
        }
    }

    // Extract known tools.
    for tool in KNOWN_TOOLS {
        let tool_lower = tool.to_lowercase();
        if content_lower.contains(&tool_lower) {
            entities.push(ExtractedEntity {
                name: tool.to_string(),
                entity_type: EntityType::Tool,
            });
        }
    }

    // Extract patterns from content (e.g., "X pattern", "Y convention", "Z approach").
    extract_pattern_entities(content, &mut entities);

    // Infer relationships based on category and context.
    infer_relationships(&entities, category, tags, &mut relationships);

    ExtractionResult {
        entities,
        relationships,
    }
}

/// Extract pattern/convention entities from content text.
fn extract_pattern_entities(content: &str, entities: &mut Vec<ExtractedEntity>) {
    // Match "X pattern", "X convention", "X approach", "X methodology".
    let pattern_keywords = [
        "pattern",
        "convention",
        "approach",
        "methodology",
        "paradigm",
    ];
    let words: Vec<String> = content.split_whitespace().map(|w| w.to_string()).collect();
    for (i, word) in words.iter().enumerate() {
        let lower: String = word
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
            .to_lowercase();
        if pattern_keywords.contains(&lower.as_str()) && i > 0 {
            let prev: String = words[i - 1]
                .chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>();
            if !prev.is_empty() && prev.len() > 2 {
                entities.push(ExtractedEntity {
                    name: format!("{} {}", prev, word),
                    entity_type: EntityType::Pattern,
                });
            }
        }
    }
}

/// Infer relationships between entities based on category and tags.
fn infer_relationships(
    entities: &[ExtractedEntity],
    category: &str,
    tags: &[String],
    relationships: &mut Vec<ExtractedRelationship>,
) {
    if entities.len() < 2 {
        return;
    }

    // Determine the default relationship type based on category.
    let default_relation = match category {
        "preference" => RelationType::Prefers,
        "error" => RelationType::Avoids,
        "workflow" => RelationType::Uses,
        _ => RelationType::RelatedTo,
    };

    // Connect entities with relationships.
    // For each pair of entities, create a relationship.
    // If tags contain "uses", "depends_on", etc., use those.
    for i in 0..entities.len() {
        for j in (i + 1)..entities.len() {
            let relation = determine_relation(&entities[i], &entities[j], tags, &default_relation);
            relationships.push(ExtractedRelationship {
                source: entities[i].name.clone(),
                target: entities[j].name.clone(),
                relation_type: relation,
            });
        }
    }
}

/// Determine the relationship type between two entities.
fn determine_relation(
    source: &ExtractedEntity,
    target: &ExtractedEntity,
    tags: &[String],
    default: &RelationType,
) -> RelationType {
    // Check if tags specify a relationship type.
    let tags_lower: Vec<String> = tags.iter().map(|t| t.to_lowercase()).collect();
    if tags_lower.contains(&"uses".to_string()) {
        return RelationType::Uses;
    }
    if tags_lower.contains(&"depends_on".to_string())
        || tags_lower.contains(&"dependency".to_string())
    {
        return RelationType::DependsOn;
    }
    if tags_lower.contains(&"prefers".to_string()) || tags_lower.contains(&"preferred".to_string())
    {
        return RelationType::Prefers;
    }
    if tags_lower.contains(&"avoids".to_string()) || tags_lower.contains(&"avoid".to_string()) {
        return RelationType::Avoids;
    }

    // Infer from entity types.
    if matches!(source.entity_type, EntityType::Language)
        && matches!(target.entity_type, EntityType::Tool)
    {
        return RelationType::Uses;
    }
    if matches!(source.entity_type, EntityType::Tool)
        && matches!(target.entity_type, EntityType::Language)
    {
        return RelationType::Uses;
    }
    if matches!(source.entity_type, EntityType::Pattern) {
        return RelationType::RelatedTo;
    }

    default.clone()
}

/// Store extracted entities and relationships in the database.
///
/// Creates or updates entity records and creates relationship edges.
/// If an entity already exists, its mention count is incremented.
///
/// # Arguments
///
/// * `result` - The extraction result to store.
/// * `memory_id` - The ID of the source memory.
/// * `storage` - SQLite storage backend.
///
/// # Returns
///
/// The number of entities and relationships stored.
pub fn store_extraction(
    result: &ExtractionResult,
    memory_id: i64,
    storage: &Storage,
) -> Result<(usize, usize)> {
    let mut entity_count = 0;
    let mut relationship_count = 0;

    // Store entities and collect their IDs.
    let mut entity_ids: HashMap<String, i64> = HashMap::new();
    for entity in &result.entities {
        let id = storage.upsert_entity(&entity.name, entity.entity_type.as_str(), memory_id)?;
        entity_ids.insert(entity.name.clone(), id);
        entity_count += 1;
    }

    // Store relationships.
    for rel in &result.relationships {
        if let (Some(&source_id), Some(&target_id)) =
            (entity_ids.get(&rel.source), entity_ids.get(&rel.target))
        {
            storage.create_relationship(
                source_id,
                target_id,
                rel.relation_type.as_str(),
                0.7, // Default confidence for extracted relationships
                Some(memory_id),
            )?;
            relationship_count += 1;
        }
    }

    Ok((entity_count, relationship_count))
}

/// Retrieve the full knowledge graph from the database.
///
/// Returns all entities and relationships for visualisation or retrieval.
pub fn get_knowledge_graph(storage: &Storage) -> Result<KnowledgeGraph> {
    let entities = storage.list_entities()?;
    let relationships = storage.list_relationships()?;
    Ok(KnowledgeGraph {
        entities,
        relationships,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_known_languages() {
        let result = extract_entities(
            "This project uses Rust and TypeScript for development",
            "fact",
            &[],
        );
        let names: Vec<&str> = result.entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"Rust"));
        assert!(names.contains(&"TypeScript"));
    }

    #[test]
    fn test_extract_known_tools() {
        let result = extract_entities("We deploy with Docker and Kubernetes", "fact", &[]);
        let names: Vec<&str> = result.entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"Docker"));
        assert!(names.contains(&"Kubernetes"));
    }

    #[test]
    fn test_infer_uses_relationship() {
        let result = extract_entities("Rust uses Tokio for async", "fact", &[]);
        // Should have at least one relationship connecting Rust and Tokio.
        assert!(!result.relationships.is_empty());
    }

    #[test]
    fn test_category_affects_relationship_type() {
        let result_pref = extract_entities("Rust Docker", "preference", &[]);
        let result_err = extract_entities("Rust Docker", "error", &[]);

        // Preference category should produce "prefers" relationships.
        if !result_pref.relationships.is_empty() {
            assert_eq!(
                result_pref.relationships[0].relation_type,
                RelationType::Prefers
            );
        }
        // Error category should produce "avoids" relationships.
        if !result_err.relationships.is_empty() {
            assert_eq!(
                result_err.relationships[0].relation_type,
                RelationType::Avoids
            );
        }
    }

    #[test]
    fn test_entity_type_roundtrip() {
        let types = vec![
            EntityType::Project,
            EntityType::Tool,
            EntityType::Language,
            EntityType::Pattern,
            EntityType::Person,
            EntityType::Concept,
        ];
        for t in types {
            assert_eq!(EntityType::from_str(t.as_str()), Some(t));
        }
    }

    #[test]
    fn test_relation_type_roundtrip() {
        let types = vec![
            RelationType::Uses,
            RelationType::Prefers,
            RelationType::DependsOn,
            RelationType::Avoids,
            RelationType::RelatedTo,
        ];
        for t in types {
            assert_eq!(RelationType::from_str_lossy(t.as_str()), t);
        }
    }

    #[test]
    fn test_empty_content() {
        let result = extract_entities("", "fact", &[]);
        assert!(result.entities.is_empty());
        assert!(result.relationships.is_empty());
    }

    #[test]
    fn test_pattern_extraction() {
        let result = extract_entities(
            "We follow the TDD pattern and clean architecture convention",
            "pattern",
            &[],
        );
        let pattern_names: Vec<String> = result
            .entities
            .iter()
            .filter(|e| e.entity_type == EntityType::Pattern)
            .map(|e| e.name.clone())
            .collect();
        // Should extract "TDD pattern" and "clean architecture convention"
        assert!(pattern_names.len() >= 1);
    }

    #[test]
    fn test_knowledge_graph_serialisation() {
        let graph = KnowledgeGraph {
            entities: vec![Entity {
                id: 1,
                name: "Rust".to_string(),
                entity_type: "language".to_string(),
                mention_count: 5,
                created_at: "2025-07-15T10:30:00Z".to_string(),
                updated_at: "2025-07-15T10:30:00Z".to_string(),
            }],
            relationships: vec![Relationship {
                id: 1,
                source_id: 1,
                target_id: 2,
                relation_type: "uses".to_string(),
                confidence: 0.8,
                source_memory_id: Some(42),
                created_at: "2025-07-15T10:30:00Z".to_string(),
            }],
        };
        let json = serde_json::to_string_pretty(&graph).unwrap();
        assert!(json.contains("\"entities\""));
        assert!(json.contains("\"relationships\""));
        assert!(json.contains("Rust"));
    }
}
