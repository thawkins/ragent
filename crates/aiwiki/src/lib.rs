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
//! use aiwiki::{Aiwiki, AiwikiConfig};
//!
//! # async fn example() -> anyhow::Result<()> {
//! let wiki = Aiwiki::new(std::env::current_dir()?).await?;
//! wiki.init().await?;
//! # Ok(())
//! # }
//! ```

pub mod config;
pub mod error;
pub mod extraction;
pub mod init;
pub mod ingest;
pub mod pages;
pub mod state;
pub mod sync;
pub mod analysis;
pub mod export_import;
pub mod web;

pub use config::{AiwikiConfig, SyncMode, ExtractionConfig, DEFAULT_ENABLED};
pub use error::{AiwikiError, Result};
pub use init::Initializer;
pub use ingest::{ingest_file, scan_directory, ingest_raw_directory, IngestOptions, IngestionResult};
pub use state::{AiwikiState, FileState};
pub use sync::{sync, needs_sync, preview_sync, create_watcher, SyncResult, SyncPreview, SyncProgress, WatcherConfig};
pub use analysis::{generate_analysis, list_analyses, ask_wiki, review_contradictions};
pub use analysis::{AnalysisRequest, AnalysisType, AnalysisResult, QaResult, Citation, Contradiction, ReviewResult};
pub use export_import::{export_single_markdown, export_obsidian_vault, import_markdown};
pub use extraction::{LlmExtractor, ExtractionResult, ExtractedEntity, ExtractedConcept};
pub use web::search_wiki;
pub use web::SearchResult;

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
        }}

/// Re-export commonly used types at crate root.
pub mod prelude {
    pub use super::{Aiwiki, AiwikiConfig, AiwikiError, AiwikiState, Result};
}
