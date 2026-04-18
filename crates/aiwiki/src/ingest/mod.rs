//! Document ingestion module for AIWiki.
//!
//! Handles ingestion of various document formats (Markdown, PDF, DOCX, ODT)
//! into the AIWiki raw/ directory. Extracts text content where possible
//! and tracks files in the state system.

use async_recursion::async_recursion;
use crate::{Aiwiki, AiwikiState, Result};
use std::path::{Path, PathBuf};
use tokio::fs;

pub mod extractors;
pub use extractors::{extract_text, DocumentType};

/// Result of an ingestion operation.
#[derive(Debug, Clone)]
pub struct IngestionResult {
    /// Path where the file was stored in raw/.
    pub stored_path: PathBuf,
    /// Original source path.
    pub source_path: PathBuf,
    /// Detected document type.
    pub doc_type: DocumentType,
    /// Whether text was extracted.
    pub text_extracted: bool,
    /// Size of the file in bytes.
    pub size_bytes: u64,
    /// SHA-256 hash of the file.
    pub hash: String,
}

/// Ingest a single file into the AIWiki raw/ directory.
///
/// # Arguments
/// * `wiki` - The AIWiki instance
/// * `source_path` - Path to the file to ingest
/// * `options` - Ingestion options
///
/// # Returns
/// The ingestion result with metadata about the stored file.
///
/// # Errors
/// Returns an error if the file cannot be read, the wiki is disabled,
/// or the file type is not supported.
pub async fn ingest_file<P: AsRef<Path>>(
    wiki: &Aiwiki,
    source_path: P,
    options: IngestOptions,
) -> Result<IngestionResult> {
    let source_path = source_path.as_ref();
    
    // Check if wiki is enabled
    if !wiki.config.enabled {
        return Err(crate::AiwikiError::Config(
            "AIWiki is disabled. Run `/aiwiki on` to enable.".to_string()
        ));
    }
    
    // Validate source file exists
    if !source_path.exists() {
        return Err(crate::AiwikiError::Config(
            format!("File not found: {}", source_path.display())
        ));
    }
    
    // Check if it's a file (not a directory)
    let metadata = fs::metadata(source_path).await?;
    if !metadata.is_file() {
        return Err(crate::AiwikiError::Config(
            format!("Path is not a file: {}", source_path.display())
        ));
    }
    
    // Check file size
    let size_bytes = metadata.len();
    if size_bytes > wiki.config.max_file_size {
        return Err(crate::AiwikiError::Config(
            format!(
                "File too large: {} bytes (max: {} bytes)",
                size_bytes, wiki.config.max_file_size
            )
        ));
    }
    
    // Detect document type
    let doc_type = DocumentType::from_path(source_path);
    
    // Generate destination path in raw/
    let file_name = source_path
        .file_name()
        .ok_or_else(|| crate::AiwikiError::Config(
            "Invalid file name".to_string()
        ))?;
    
    let dest_path = if let Some(subdir) = &options.subdirectory {
        wiki.path("raw").join(subdir).join(file_name)
    } else {
        wiki.path("raw").join(file_name)
    };
    
    // Ensure parent directory exists
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent).await?;
    }
    
    // Copy or move the file
    if options.move_file {
        fs::rename(source_path, &dest_path).await?;
    } else {
        fs::copy(source_path, &dest_path).await?;
    }
    
    // Calculate hash
    let hash = AiwikiState::calculate_hash(&dest_path).await?;
    
    // Try to extract text
    let text_extracted = extract_text(&dest_path, doc_type.clone()).await.is_ok();
    
    // Get relative path for state tracking
    let _relative_path = dest_path
        .strip_prefix(&wiki.wiki_dir)
        .map_err(|e| crate::AiwikiError::State(e.to_string()))?;
    
    Ok(IngestionResult {
        stored_path: dest_path,
        source_path: source_path.to_path_buf(),
        doc_type,
        text_extracted,
        size_bytes,
        hash,
    })
}

/// Scan a directory and ingest all supported files.
///
/// # Arguments
/// * `wiki` - The AIWiki instance
/// * `dir_path` - Path to the directory to scan
/// * `options` - Ingestion options
/// * `recursive` - Whether to scan subdirectories
///
/// # Returns
/// A list of ingestion results for all successfully ingested files.
#[async_recursion]
pub async fn scan_directory<P: AsRef<Path> + Send>(
    wiki: &Aiwiki,
    dir_path: P,
    options: IngestOptions,
    recursive: bool,
) -> Result<Vec<IngestionResult>> {
    let dir_path = dir_path.as_ref();
    
    // Check if wiki is enabled
    if !wiki.config.enabled {
        return Err(crate::AiwikiError::Config(
            "AIWiki is disabled. Run `/aiwiki on` to enable.".to_string()
        ));
    }
    
    if !dir_path.exists() {
        return Err(crate::AiwikiError::Config(
            format!("Directory not found: {}", dir_path.display())
        ));
    }
    
    let mut results = Vec::new();
    let mut entries = fs::read_dir(dir_path).await?;
    
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        
        if path.is_file() {
            // Check if file type is supported
            if DocumentType::from_path(&path) != DocumentType::Unknown {
                match ingest_file(wiki, &path, options.clone()).await {
                    Ok(result) => results.push(result),
                    Err(e) => {
                        tracing::warn!("Failed to ingest {}: {}", path.display(), e);
                    }
                }
            }
        } else if recursive && path.is_dir() {
            // Recursively scan subdirectories
            let sub_results = scan_directory(wiki, &path, options.clone(), true).await?;
            results.extend(sub_results);
        }
    }
    
    Ok(results)
}

/// Ingest from the raw/ directory itself (batch ingestion).
///
/// This is useful for processing files that were manually placed
/// in the raw/ directory.
pub async fn ingest_raw_directory(
    wiki: &Aiwiki,
    options: IngestOptions,
) -> Result<Vec<IngestionResult>> {
    let raw_dir = wiki.path("raw");
    scan_directory(wiki, raw_dir, options, true).await
}

/// Options for ingestion operations.
#[derive(Debug, Clone, Default)]
pub struct IngestOptions {
    /// Whether to move files instead of copying.
    pub move_file: bool,
    /// Subdirectory within raw/ to store files.
    pub subdirectory: Option<String>,
    /// Whether to overwrite existing files.
    pub overwrite: bool,
}

impl IngestOptions {
    /// Create new options with move enabled.
    pub fn move_file() -> Self {
        Self {
            move_file: true,
            ..Default::default()
        }
    }
    
    /// Set subdirectory.
    pub fn with_subdirectory(mut self, subdir: impl Into<String>) -> Self {
        self.subdirectory = Some(subdir.into());
        self
    }
    
    /// Set move flag.
    pub fn with_move(mut self, move_file: bool) -> Self {
        self.move_file = move_file;
        self
    }
    
    /// Set overwrite flag.
    pub fn with_overwrite(mut self, overwrite: bool) -> Self {
        self.overwrite = overwrite;
        self
    }
}
