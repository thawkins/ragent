//! State tracking for incremental wiki updates.
//!
//! State is stored in `aiwiki/state.json` and tracks SHA-256 hashes
//! of source files to detect new, modified, and deleted files.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use tokio::fs;

/// State tracking for the wiki.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AiwikiState {
    /// Map of file paths (relative to raw/) to their state.
    pub files: HashMap<String, FileState>,
    
    /// Last sync timestamp.
    #[serde(default)]
    pub last_sync: Option<chrono::DateTime<chrono::Utc>>,
    
    /// Total number of pages generated.
    #[serde(default)]
    pub page_count: usize,
    
    /// Total tokens used for LLM operations.
    #[serde(default)]
    pub token_usage: u64,
    
    /// Version of the state schema.
    #[serde(default = "default_version")]
    pub version: String,
}

/// State for a single source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileState {
    /// SHA-256 hash of file contents.
    pub hash: String,
    
    /// Last modified timestamp.
    pub modified: chrono::DateTime<chrono::Utc>,
    
    /// File size in bytes.
    pub size: u64,
    
    /// Associated wiki page paths generated from this file.
    #[serde(default)]
    pub generated_pages: Vec<String>,
}

impl AiwikiState {
    /// Load state from the aiwiki directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed.
    pub async fn load(wiki_dir: impl AsRef<Path>) -> crate::Result<Self> {
        let path = wiki_dir.as_ref().join("state.json");
        let content = fs::read_to_string(&path).await?;
        let state: Self = serde_json::from_str(&content)?;
        Ok(state)
    }
    
    /// Save state to the aiwiki directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be written.
    pub async fn save(&self, wiki_dir: impl AsRef<Path>) -> crate::Result<()> {
        let path = wiki_dir.as_ref().join("state.json");
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&path, content).await?;
        Ok(())
    }
    
    /// Calculate the SHA-256 hash of a file's contents.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read.
    pub async fn calculate_hash(path: impl AsRef<Path>) -> crate::Result<String> {
        let content = fs::read(path).await?;
        let hash = Sha256::digest(&content);
        Ok(hex::encode(hash))
    }
    
    /// Check if a file has changed compared to the stored state.
    ///
    /// Returns `FileChangeStatus` indicating if the file is new, modified,
    /// unchanged, or if there was an error reading it.
    pub async fn check_file(
        &self,
        raw_dir: impl AsRef<Path>,
        relative_path: &str,
    ) -> crate::Result<FileChangeStatus> {
        let full_path = raw_dir.as_ref().join(relative_path);
        
        // Check if file exists
        let _metadata = match fs::metadata(&full_path).await {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(FileChangeStatus::Deleted);
            }
            Err(e) => return Err(e.into()),
        };
        
        let current_hash = Self::calculate_hash(&full_path).await?;
        
        match self.files.get(relative_path) {
            None => Ok(FileChangeStatus::New),
            Some(state) if state.hash != current_hash => Ok(FileChangeStatus::Modified),
            Some(_) => Ok(FileChangeStatus::Unchanged),
        }
    }
    
    /// Update the state for a file after processing.
    pub async fn update_file(
        &mut self,
        raw_dir: impl AsRef<Path>,
        relative_path: impl AsRef<Path>,
        generated_pages: Vec<String>,
    ) -> crate::Result<()> {
        let full_path = raw_dir.as_ref().join(&relative_path);
        let metadata = fs::metadata(&full_path).await?;
        let hash = Self::calculate_hash(&full_path).await?;
        let path_str = relative_path.as_ref().to_string_lossy().to_string();
        
        self.files.insert(
            path_str,
            FileState {
                hash,
                modified: chrono::Utc::now(),
                size: metadata.len(),
                generated_pages,
            },
        );
        
        Ok(())
    }
    
    /// Remove a file from state (e.g., when deleted).
    pub fn remove_file(&mut self, relative_path: impl AsRef<Path>) {
        let path_str = relative_path.as_ref().to_string_lossy().to_string();
        self.files.remove(&path_str);
    }
    
    /// Get all files that need processing (new or modified).
    ///
    /// Returns a list of relative paths that are new, modified, or deleted.
    pub async fn get_changes(
        &self,
        raw_dir: impl AsRef<Path>,
    ) -> crate::Result<Changes> {
        let raw_dir = raw_dir.as_ref();
        let mut new = Vec::new();
        let mut modified = Vec::new();
        let mut deleted = Vec::new();
        
        // Scan current files
        if raw_dir.exists() {
            let mut entries = fs::read_dir(raw_dir).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.is_file() {
                    let relative = path.strip_prefix(raw_dir)
                        .map_err(|e| crate::AiwikiError::State(e.to_string()))?;
                    let relative_str = relative.to_string_lossy().to_string();
                    
                    match self.check_file(raw_dir, &relative_str).await? {
                        FileChangeStatus::New => new.push(relative_str),
                        FileChangeStatus::Modified => modified.push(relative_str),
                        _ => {}
                    }
                }
            }
        }
        
        // Find deleted files (in state but not on disk)
        for path in self.files.keys() {
            let full_path = raw_dir.join(path);
            if !full_path.exists() {
                deleted.push(path.clone());
            }
        }
        
        Ok(Changes {
            new,
            modified,
            deleted,
        })
    }
    
    /// Update the last sync timestamp.
    pub fn mark_synced(&mut self) {
        self.last_sync = Some(chrono::Utc::now());
    }
    
    /// Add token usage to the running total.
    pub fn add_token_usage(&mut self, tokens: u64) {
        self.token_usage += tokens;
    }
    
    /// Get statistics about the wiki state.
    pub fn stats(&self) -> StateStats {
        StateStats {
            total_sources: self.files.len(),
            total_pages: self.page_count,
            last_sync: self.last_sync,
            total_tokens_used: self.token_usage,
            storage_bytes: 0, // Calculated on demand
        }
    }
}

/// Status of a file during change detection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChangeStatus {
    /// File is new (not in state).
    New,
    /// File has been modified (hash changed).
    Modified,
    /// File is unchanged.
    Unchanged,
    /// File has been deleted (in state but not on disk).
    Deleted,
}

/// Collection of changes detected in the raw directory.
#[derive(Debug, Clone, Default)]
pub struct Changes {
    /// New files to process.
    pub new: Vec<String>,
    /// Modified files to re-process.
    pub modified: Vec<String>,
    /// Deleted files to remove from wiki.
    pub deleted: Vec<String>,
}

impl Changes {
    /// Check if there are any changes.
    pub fn is_empty(&self) -> bool {
        self.new.is_empty() && self.modified.is_empty() && self.deleted.is_empty()
    }
    
    /// Total number of changes.
    pub fn len(&self) -> usize {
        self.new.len() + self.modified.len() + self.deleted.len()
    }
}

/// Statistics about the wiki state.
#[derive(Debug, Clone)]
pub struct StateStats {
    /// Number of source files tracked.
    pub total_sources: usize,
    /// Number of generated wiki pages.
    pub total_pages: usize,
    /// Last successful sync timestamp.
    pub last_sync: Option<chrono::DateTime<chrono::Utc>>,
    /// Total tokens used for LLM operations.
    pub total_tokens_used: u64,
    /// Storage usage in bytes.
    pub storage_bytes: u64,
}

fn default_version() -> String {
    "1.0.0".to_string()
}
