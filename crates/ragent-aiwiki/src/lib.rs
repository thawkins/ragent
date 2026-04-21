//! AIWiki - An embedded, project-scoped knowledge base system for ragent.
//!
//! AIWiki compiles knowledge into an interconnected wiki of markdown pages
//! with automatic cross-linking and incremental updates. It is inspired by
//! axiom-wiki but designed to be embedded in ragent as a native component.
//!
//! # Architecture
//!
//! The crate is organized into modules:
//! - `config`: Wiki configuration management
//! - `state`: File state tracking for incremental updates
//! - `init`: Directory initialization
//! - `error`: Error types
//!
//! # Example Usage
//!
//! ```rust,no_run
//! use ragent_aiwiki::{Aiwiki, Initializer};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let root = std::env::current_dir()?;
//! Initializer::new(&root)?.init(None).await?;
//! let _wiki = Aiwiki::new(root).await?;
//! # Ok(())
//! # }
//! ```

pub mod analysis;
pub mod config;
pub mod error;
pub mod export_import;
pub mod extraction;
pub mod ingest;
pub mod init;
pub mod pages;
pub mod source_folder;
pub mod state;
pub mod sync;
pub mod web;

pub use analysis::{
    AnalysisRequest, AnalysisResult, AnalysisType, Citation, Contradiction, QaResult, ReviewResult,
};
pub use analysis::{ask_wiki, generate_analysis, list_analyses, review_contradictions};
pub use config::{AiwikiConfig, DEFAULT_ENABLED, ExtractionConfig, SourceFolder, SyncMode};
pub use error::{AiwikiError, Result};
pub use export_import::{export_obsidian_vault, export_single_markdown, import_markdown};
pub use extraction::{ExtractedConcept, ExtractedEntity, ExtractionResult, LlmExtractor};
pub use ingest::{
    IngestOptions, IngestionResult, ingest_file, ingest_raw_directory, scan_directory,
};
pub use init::Initializer;
pub use state::{AiwikiState, FileState};
pub use sync::{
    SyncPreview, SyncProgress, SyncResult, WatcherConfig, create_watcher, needs_sync, preview_sync,
    sync,
};
pub use web::SearchResult;
pub use web::search_wiki;

use std::path::{Path, PathBuf};

/// Main AIWiki handle for managing a wiki instance.
#[derive(Debug)]
pub struct Aiwiki {
    /// Root directory of the wiki (project root).
    pub root: PathBuf,
    /// Path to the aiwiki/ directory.
    pub wiki_dir: PathBuf,
    /// Configuration loaded from config.json.
    pub config: AiwikiConfig,
    /// State tracking loaded from state.json.
    pub state: AiwikiState,
}

impl Aiwiki {
    /// Create a new AIWiki instance for the given project root.
    ///
    /// This loads the configuration and state if they exist,
    /// or returns an error if the wiki has not been initialized.
    ///
    /// # Errors
    ///
    /// Returns an error if the root path is invalid or if config/state
    /// files exist but cannot be parsed.
    pub async fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().canonicalize()?;
        let wiki_dir = root.join("aiwiki");

        let config = if wiki_dir.join("config.json").exists() {
            AiwikiConfig::load(&wiki_dir).await?
        } else {
            return Err(AiwikiError::NotInitialized);
        };

        let state = if wiki_dir.join("state.json").exists() {
            AiwikiState::load(&wiki_dir).await?
        } else {
            AiwikiState::default()
        };

        Ok(Self {
            root,
            wiki_dir,
            config,
            state,
        })
    }

    /// Check if a wiki exists at the given project root.
    pub fn exists(root: impl AsRef<Path>) -> bool {
        let wiki_dir = root.as_ref().join("aiwiki");
        wiki_dir.join("config.json").exists()
    }

    /// Check if AIWiki is enabled at the given project root.
    ///
    /// Returns true if the wiki exists AND is enabled in config.
    /// Returns false if the wiki doesn't exist or is disabled.
    pub async fn is_enabled(root: impl AsRef<Path>) -> bool {
        let root = root.as_ref();
        if !Self::exists(root) {
            return false;
        }

        match Self::new(root).await {
            Ok(wiki) => wiki.config.enabled,
            Err(_) => false,
        }
    }

    /// Enable or disable AIWiki.
    ///
    /// Updates the config.json with the new enabled state.
    ///
    /// # Errors
    ///
    /// Returns an error if the config cannot be loaded or saved.
    pub async fn set_enabled(&mut self, enabled: bool) -> Result<()> {
        self.config.enabled = enabled;
        self.config.save(&self.wiki_dir).await
    }

    /// Get the path to a subdirectory within the wiki.
    pub fn path(&self, subdir: &str) -> PathBuf {
        self.wiki_dir.join(subdir)
    }

    /// Reset the wiki to a clean state.
    ///
    /// Clears the state (tracked files, page counts, token usage) and removes
    /// all generated wiki pages while preserving the raw/ directory and configuration.
    ///
    /// # Arguments
    ///
    /// * `preserve_raw` - If true, keeps files in raw/ directory. If false, removes them too.
    ///
    /// # Errors
    ///
    /// Returns an error if state cannot be saved or files cannot be removed.
    pub async fn reset(&mut self, preserve_raw: bool) -> Result<()> {
        // Reset the state to default
        self.state = AiwikiState::default();
        self.state.save(&self.wiki_dir).await?;

        // Remove all generated wiki pages
        let wiki_dir = self.wiki_dir.join("wiki");
        if wiki_dir.exists() {
            // Remove subdirectories but keep log.md if it exists
            let entries = tokio::fs::read_dir(&wiki_dir).await?;
            let mut entries = entries;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                // Skip log.md and .gitignore
                if file_name == "log.md" || file_name == ".gitignore" {
                    continue;
                }

                if entry.file_type().await?.is_dir() {
                    // Remove subdirectories (entities, concepts, sources, analyses)
                    tokio::fs::remove_dir_all(&path).await?;
                } else if entry.file_type().await?.is_file() {
                    // Remove other files
                    tokio::fs::remove_file(&path).await?;
                }
            }

            // Recreate the subdirectories
            tokio::fs::create_dir_all(wiki_dir.join("entities")).await?;
            tokio::fs::create_dir_all(wiki_dir.join("concepts")).await?;
            tokio::fs::create_dir_all(wiki_dir.join("sources")).await?;
            tokio::fs::create_dir_all(wiki_dir.join("analyses")).await?;
        }

        // Optionally clear raw/ directory
        if !preserve_raw {
            let raw_dir = self.wiki_dir.join("raw");
            if raw_dir.exists() {
                let entries = tokio::fs::read_dir(&raw_dir).await?;
                let mut entries = entries;
                while let Some(entry) = entries.next_entry().await? {
                    let path = entry.path();
                    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

                    // Preserve .gitignore
                    if file_name == ".gitignore" {
                        continue;
                    }

                    if entry.file_type().await?.is_file() {
                        tokio::fs::remove_file(&path).await?;
                    } else if entry.file_type().await?.is_dir() {
                        tokio::fs::remove_dir_all(&path).await?;
                    }
                }
            }
        }

        Ok(())
    }
}

/// Re-export commonly used types at crate root.
pub mod prelude {
    pub use super::{Aiwiki, AiwikiConfig, AiwikiError, AiwikiState, Result};
}
