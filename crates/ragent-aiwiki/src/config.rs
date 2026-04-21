//! Configuration management for AIWiki.
//!
//! The configuration is stored in `aiwiki/config.json` and controls
//! wiki behavior including sync mode, extraction settings, source folders,
//! and file watching.

use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

pub use crate::source_folder::SourceFolder;

/// Default wiki name.
pub const DEFAULT_WIKI_NAME: &str = "Project Wiki";

/// Default sync mode.
pub const DEFAULT_SYNC_MODE: SyncMode = SyncMode::Manual;

/// Default LLM model for extraction.
pub const DEFAULT_LLM_MODEL: &str = "claude-sonnet-4-20250514";

/// Default enabled state.
pub const DEFAULT_ENABLED: bool = false;

/// Default watch mode state.
pub const DEFAULT_WATCH_MODE: bool = false;

/// Wiki configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiwikiConfig {
    /// Wiki name/title.
    pub name: String,

    /// Whether AIWiki is enabled/active.
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Sync mode for automatic updates.
    #[serde(default)]
    pub sync_mode: SyncMode,

    /// LLM model for extraction.
    #[serde(default = "default_llm_model")]
    pub llm_model: String,

    /// Extraction configuration.
    #[serde(default)]
    pub extraction: ExtractionConfig,

    /// Files/directories to ignore in raw/.
    #[serde(default)]
    pub ignore_patterns: Vec<String>,

    /// Maximum file size to process (in bytes).
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,

    /// Version of the config schema.
    #[serde(default = "default_version")]
    pub version: String,

    /// Referenced source folders for in-place scanning.
    #[serde(default)]
    pub sources: Vec<SourceFolder>,

    /// Whether to auto-start file watching on initialization.
    #[serde(default)]
    pub watch_mode: bool,
}

impl Default for AiwikiConfig {
    fn default() -> Self {
        Self {
            name: DEFAULT_WIKI_NAME.to_string(),
            enabled: DEFAULT_ENABLED,
            sync_mode: DEFAULT_SYNC_MODE,
            llm_model: default_llm_model(),
            extraction: ExtractionConfig::default(),
            ignore_patterns: default_ignore_patterns(),
            max_file_size: default_max_file_size(),
            version: default_version(),
            sources: Vec::new(),
            watch_mode: DEFAULT_WATCH_MODE,
        }
    }
}

impl AiwikiConfig {
    /// Load configuration from the aiwiki directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub async fn load(wiki_dir: impl AsRef<Path>) -> crate::Result<Self> {
        let path = wiki_dir.as_ref().join("config.json");
        let content = fs::read_to_string(&path).await?;
        let config: Self = serde_json::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to the aiwiki directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub async fn save(&self, wiki_dir: impl AsRef<Path>) -> crate::Result<()> {
        let path = wiki_dir.as_ref().join("config.json");
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content).await?;
        Ok(())
    }

    /// Validate the configuration.
    ///
    /// Returns Ok(()) if valid, or an error with a message.
    pub fn validate(&self) -> crate::Result<()> {
        if self.name.is_empty() {
            return Err(crate::AiwikiError::Config(
                "Wiki name cannot be empty".to_string(),
            ));
        }
        if self.max_file_size == 0 {
            return Err(crate::AiwikiError::Config(
                "max_file_size must be greater than 0".to_string(),
            ));
        }
        Ok(())
    }

    /// Add a source folder to the configuration.
    ///
    /// Validates and appends the source. Returns an error if the path
    /// is already registered.
    ///
    /// # Errors
    ///
    /// Returns an error if a source with the same path already exists.
    pub fn add_source(&mut self, source: SourceFolder) -> crate::Result<()> {
        // Check for duplicate path
        if self.sources.iter().any(|s| s.path == source.path) {
            return Err(crate::AiwikiError::Config(format!(
                "Source folder '{}' is already registered",
                source.path
            )));
        }

        self.sources.push(source);
        Ok(())
    }

    /// Remove a source folder from the configuration.
    ///
    /// Returns the removed source folder, or an error if not found.
    ///
    /// # Errors
    ///
    /// Returns an error if no source with the given path exists.
    pub fn remove_source(&mut self, path: &str) -> crate::Result<SourceFolder> {
        let pos = self
            .sources
            .iter()
            .position(|s| s.path == path)
            .ok_or_else(|| {
                crate::AiwikiError::Config(format!("Source folder '{}' not found", path))
            })?;

        Ok(self.sources.remove(pos))
    }

    /// Update an existing source folder.
    ///
    /// Replaces the source folder entry at the given path with the updated version.
    ///
    /// # Errors
    ///
    /// Returns an error if no source with the given path exists.
    pub fn update_source(&mut self, path: &str, updated: SourceFolder) -> crate::Result<()> {
        let pos = self
            .sources
            .iter()
            .position(|s| s.path == path)
            .ok_or_else(|| {
                crate::AiwikiError::Config(format!("Source folder '{}' not found", path))
            })?;

        self.sources[pos] = updated;
        Ok(())
    }

    /// Get a source folder by path.
    ///
    /// Returns a reference to the source folder if found, None otherwise.
    pub fn get_source(&self, path: &str) -> Option<&SourceFolder> {
        self.sources.iter().find(|s| s.path == path)
    }

    /// List all source folders.
    ///
    /// Returns a slice of all registered source folders.
    pub fn list_sources(&self) -> &[SourceFolder] {
        &self.sources
    }

    /// Get only enabled source folders.
    ///
    /// Returns an iterator over source folders with `enabled: true`.
    pub fn enabled_sources(&self) -> impl Iterator<Item = &SourceFolder> {
        self.sources.iter().filter(|s| s.enabled)
    }
}

/// Sync mode for automatic wiki updates.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    /// Manual sync only (user runs `/aiwiki sync`).
    #[default]
    Manual,
    /// Sync on server startup.
    OnStartup,
    /// Real-time sync via file watcher.
    Realtime,
}

/// Configuration for LLM extraction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionConfig {
    /// Maximum tokens for LLM responses.
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,

    /// Temperature for LLM sampling.
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    /// Whether to extract entities.
    #[serde(default = "default_true")]
    pub extract_entities: bool,

    /// Whether to extract concepts.
    #[serde(default = "default_true")]
    pub extract_concepts: bool,

    /// Whether to generate cross-links.
    #[serde(default = "default_true")]
    pub generate_links: bool,
}

impl Default for ExtractionConfig {
    fn default() -> Self {
        Self {
            max_tokens: default_max_tokens(),
            temperature: default_temperature(),
            extract_entities: true,
            extract_concepts: true,
            generate_links: true,
        }
    }
}

// Default value helpers for serde.
fn default_llm_model() -> String {
    DEFAULT_LLM_MODEL.to_string()
}

fn default_enabled() -> bool {
    DEFAULT_ENABLED
}

fn default_max_file_size() -> u64 {
    50 * 1024 * 1024 // 50 MB
}

fn default_version() -> String {
    "1.0.0".to_string()
}

fn default_ignore_patterns() -> Vec<String> {
    vec![
        "*.tmp".to_string(),
        "*.temp".to_string(),
        ".DS_Store".to_string(),
        "Thumbs.db".to_string(),
    ]
}

fn default_max_tokens() -> u32 {
    4096
}

fn default_temperature() -> f32 {
    0.0
}

const fn default_true() -> bool {
    true
}
