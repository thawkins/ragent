//! Directory initialization for AIWiki.
//!
//! Creates the aiwiki/ directory structure with all required
//! subdirectories and configuration files.

use crate::{AiwikiConfig, AiwikiError, Result};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Default .gitignore content for the raw/ directory.
const RAW_GITIGNORE: &str = r#"# AIWiki raw files - not tracked in git
# These are source documents (PDFs, large files, etc.)
# Only the generated wiki/ content is tracked
*
!.gitignore
"#;

/// Default .gitignore content for the wiki/ directory.
const WIKI_GITIGNORE: &str = r#"# AIWiki generated content - tracked in git
# This directory contains AI-generated markdown pages
"#;

/// Default log.md content.
const LOG_MD_TEMPLATE: &str = r#"# AIWiki Log

## Initialization

- **Date**: {date}
- **Version**: {version}

This wiki was initialized with the following configuration:
- Wiki Name: {name}
- Sync Mode: {sync_mode}
- LLM Model: {llm_model}

## Operations

<!-- Operations will be logged here -->
"#;

/// Initializer for setting up a new AIWiki instance.
pub struct Initializer {
    /// Project root directory.
    root: PathBuf,
    /// Wiki directory path.
    wiki_dir: PathBuf,
}

impl Initializer {
    /// Create a new initializer for the given project root.
    ///
    /// # Errors
    ///
    /// Returns an error if the root path is invalid.
    pub fn new(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().canonicalize()?;
        let wiki_dir = root.join("aiwiki");
        
        Ok(Self { root, wiki_dir })
    }
    
    /// Check if a wiki already exists at this location.
    pub fn exists(&self) -> bool {
        self.wiki_dir.join("config.json").exists()
    }
    
          /// Initialize the wiki directory structure.
        ///
        /// Creates all required directories and files. Returns an error
        /// if the wiki already exists (use `exists()` to check first).
        /// Automatically enables the wiki (sets `enabled: true` in config).
        ///
        /// # Errors
        ///
        /// Returns an error if:
        /// - The wiki already exists
        /// - Directory creation fails
        /// - File writing fails
        pub async fn init(&self, config: Option<AiwikiConfig>) -> Result<()> {
            if self.exists() {
                return Err(AiwikiError::AlreadyInitialized);
            }
            
            let mut config = config.unwrap_or_default();
            // Auto-enable the wiki when initialized
            config.enabled = true;
            config.validate()?;
            
            // Create directory structure
            self.create_directories().await?;
            
            // Write configuration
            config.save(&self.wiki_dir).await?;
            
            // Write .gitignore files
            self.write_gitignores().await?;
            
            // Create initial log.md
            self.create_log(&config).await?;
            
            tracing::info!("AIWiki initialized at {:?}", self.wiki_dir);
            
            Ok(())
        }    
    /// Create all required directories.
    async fn create_directories(&self) -> Result<()> {
        let dirs = [
            &self.wiki_dir,
            &self.wiki_dir.join("raw"),
            &self.wiki_dir.join("wiki"),
            &self.wiki_dir.join("wiki/entities"),
            &self.wiki_dir.join("wiki/concepts"),
            &self.wiki_dir.join("wiki/sources"),
            &self.wiki_dir.join("wiki/analyses"),
            &self.wiki_dir.join("static"),
            &self.wiki_dir.join("static/css"),
            &self.wiki_dir.join("static/js"),
        ];
        
        for dir in &dirs {
            fs::create_dir_all(dir).await?;
        }
        
        Ok(())
    }
    
    /// Write .gitignore files to raw/ and wiki/.
    async fn write_gitignores(&self) -> Result<()> {
        // raw/ is gitignored (contains large/binary files)
        fs::write(self.wiki_dir.join("raw/.gitignore"), RAW_GITIGNORE).await?;
        
        // wiki/ is tracked but we still add a .gitignore for completeness
        fs::write(self.wiki_dir.join("wiki/.gitignore"), WIKI_GITIGNORE).await?;
        
        Ok(())
    }
    
    /// Create the initial log.md file.
    async fn create_log(&self, config: &AiwikiConfig) -> Result<()> {
        let log_path = self.wiki_dir.join("wiki/log.md");
        let content = LOG_MD_TEMPLATE
            .replace("{date}", &chrono::Utc::now().to_rfc3339())
            .replace("{version}", &config.version)
            .replace("{name}", &config.name)
            .replace("{sync_mode}", &format!("{:?}", config.sync_mode))
            .replace("{llm_model}", &config.llm_model);
        
        fs::write(&log_path, content).await?;
        Ok(())
    }
    
    /// Get the path to the wiki directory.
    pub fn wiki_dir(&self) -> &Path {
        &self.wiki_dir
    }
    
    /// Get the path to the project root.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// Convenience function to initialize a wiki at the given root.
///
/// # Errors
///
/// Returns an error if initialization fails.
pub async fn init_wiki(root: impl AsRef<Path>, config: Option<AiwikiConfig>) -> Result<()> {
    let initializer = Initializer::new(root)?;
    initializer.init(config).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_init_creates_structure() {
        let temp = TempDir::new().unwrap();
        let initializer = Initializer::new(temp.path()).unwrap();
        
        assert!(!initializer.exists());
        
        initializer.init(None).await.unwrap();
        
        assert!(initializer.exists());
        assert!(temp.path().join("aiwiki/config.json").exists());
        assert!(temp.path().join("aiwiki/raw").exists());
        assert!(temp.path().join("aiwiki/wiki").exists());
        assert!(temp.path().join("aiwiki/wiki/entities").exists());
        assert!(temp.path().join("aiwiki/wiki/concepts").exists());
        assert!(temp.path().join("aiwiki/wiki/sources").exists());
        assert!(temp.path().join("aiwiki/wiki/analyses").exists());
        assert!(temp.path().join("aiwiki/static").exists());
    }
    
    #[tokio::test]
    async fn test_init_fails_if_exists() {
        let temp = TempDir::new().unwrap();
        let initializer = Initializer::new(temp.path()).unwrap();
        
        initializer.init(None).await.unwrap();
        
        let result = initializer.init(None).await;
        assert!(matches!(result, Err(AiwikiError::AlreadyInitialized)));
    }
    
    #[tokio::test]
    async fn test_init_creates_gitignores() {
        let temp = TempDir::new().unwrap();
        let initializer = Initializer::new(temp.path()).unwrap();
        
        initializer.init(None).await.unwrap();
        
        let raw_gitignore = fs::read_to_string(temp.path().join("aiwiki/raw/.gitignore"))
            .await
            .unwrap();
        assert!(raw_gitignore.contains("not tracked in git"));
    }
    
    #[tokio::test]
    async fn test_init_creates_log() {
        let temp = TempDir::new().unwrap();
        let initializer = Initializer::new(temp.path()).unwrap();
        
        initializer.init(None).await.unwrap();
        
        let log = fs::read_to_string(temp.path().join("aiwiki/wiki/log.md"))
            .await
            .unwrap();
        assert!(log.contains("AIWiki Log"));
        assert!(log.contains("Project Wiki")); // default name
    }
}
