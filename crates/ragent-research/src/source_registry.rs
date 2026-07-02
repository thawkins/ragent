//! Pluggable source registry for research (T-015, FR-015).
//!
//! [`SourceRegistry`] abstracts the set of sources that `/research` can use.
//! The default [`BuiltinSourceRegistry`] exposes the built-in source kinds
//! (web search, local files, prior specs). Future implementations can query
//! the MCP client and skill registry at runtime to add new sources without
//! modifying the core research loop.

/// One available source kind that the research engine can consume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchSourceKind {
    /// Stable identifier for the source kind.
    pub id: String,
    /// Human-readable label.
    pub label: String,
    /// Description of what this source provides.
    pub description: String,
}

/// Abstraction over source discovery.
#[async_trait::async_trait]
pub trait SourceRegistry: Send + Sync {
    /// Discover all available research source kinds.
    async fn discover(&self) -> anyhow::Result<Vec<ResearchSourceKind>>;
}

/// Default registry that exposes the built-in source kinds.
#[derive(Debug, Default, Clone, Copy)]
pub struct BuiltinSourceRegistry;

impl BuiltinSourceRegistry {
    /// Create a new built-in source registry.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl SourceRegistry for BuiltinSourceRegistry {
    async fn discover(&self) -> anyhow::Result<Vec<ResearchSourceKind>> {
        Ok(vec![
            ResearchSourceKind {
                id: "web".to_string(),
                label: "Web search".to_string(),
                description: "Search the public web and fetch page bodies.".to_string(),
            },
            ResearchSourceKind {
                id: "local".to_string(),
                label: "Local files".to_string(),
                description: "Scan in-project files and optional extra directories.".to_string(),
            },
            ResearchSourceKind {
                id: "spec".to_string(),
                label: "Prior specs".to_string(),
                description: "Cross-reference prior spec documents under specs/.".to_string(),
            },
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn builtin_registry_lists_sources() {
        let reg = BuiltinSourceRegistry::new();
        let sources = reg.discover().await.unwrap();
        assert_eq!(sources.len(), 3);
        assert!(sources.iter().any(|s| s.id == "web"));
        assert!(sources.iter().any(|s| s.id == "local"));
        assert!(sources.iter().any(|s| s.id == "spec"));
    }
}
