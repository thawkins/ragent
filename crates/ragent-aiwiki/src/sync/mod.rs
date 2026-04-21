//! Sync module for AIWiki - handles incremental updates and synchronization.
//!
//! This module provides:
//! - Stale page detection (compare source hashes to wiki pages)
//! - Sync orchestration (process new/modified/deleted sources)
//! - Cross-link validation and management
//! - Auto-sync file watcher
//! - Referenced source folder support

use crate::extraction::{self, LlmExtractor};
use crate::ingest::extractors::{DocumentType, extract_text};
use crate::{Aiwiki, AiwikiError, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tokio::fs;

mod links;
pub use links::{LinkValidationResult, extract_links, validate_wiki_links};

mod watcher;
pub use watcher::{FileWatcher, WatcherConfig};

mod sources;
pub use sources::{
    count_source_files, get_source_name, make_ref_key, parse_ref_key, resolve_file_path,
    scan_source_folder,
};

mod source_watcher;
pub use source_watcher::{SourceWatcher, WatchEvent};

mod extraction_worker;
pub use extraction_worker::{ExtractionWorker, WatcherProgress};

mod watch_session;
pub use watch_session::AiwikiWatchSession;

/// Shared progress counter for sync operations.
///
/// Allows the TUI to observe progress while sync runs in a background task.
pub struct SyncProgress {
    /// Number of files processed so far.
    pub current: AtomicU32,
    /// Total number of files to process.
    pub total: AtomicU32,
}

impl SyncProgress {
    /// Create a new progress tracker.
    pub fn new() -> Self {
        Self {
            current: AtomicU32::new(0),
            total: AtomicU32::new(0),
        }
    }
}

/// Result of a sync operation.
#[derive(Debug, Clone, Default)]
pub struct SyncResult {
    /// Number of new source files processed.
    pub new_count: usize,
    /// Number of updated source files re-processed.
    pub updated_count: usize,
    /// Number of deleted sources removed.
    pub deleted_count: usize,
    /// Number of wiki pages updated.
    pub pages_updated: usize,
    /// Number of wiki pages created.
    pub pages_created: usize,
    /// Number of wiki pages removed.
    pub pages_removed: usize,
    /// Broken internal links found during sync.
    pub broken_links: Vec<BrokenLink>,
    /// Any errors encountered during sync.
    pub errors: Vec<String>,
}

impl SyncResult {
    /// Check if the sync made any changes.
    pub fn has_changes(&self) -> bool {
        self.new_count > 0
            || self.updated_count > 0
            || self.deleted_count > 0
            || self.pages_updated > 0
            || self.pages_created > 0
            || self.pages_removed > 0
    }

    /// Get total number of source files processed.
    pub fn total_sources(&self) -> usize {
        self.new_count + self.updated_count + self.deleted_count
    }

    /// Get total number of wiki pages affected.
    pub fn total_pages(&self) -> usize {
        self.pages_updated + self.pages_created + self.pages_removed
    }

    /// Generate a human-readable summary.
    pub fn summary(&self) -> String {
        if !self.has_changes() {
            return "No changes detected. Wiki is up to date.".to_string();
        }

        let mut parts = Vec::new();

        if self.new_count > 0 {
            parts.push(format!("{} new sources", self.new_count));
        }
        if self.updated_count > 0 {
            parts.push(format!("{} updated sources", self.updated_count));
        }
        if self.deleted_count > 0 {
            parts.push(format!("{} deleted sources", self.deleted_count));
        }
        if self.pages_created > 0 {
            parts.push(format!("{} new pages", self.pages_created));
        }
        if self.pages_updated > 0 {
            parts.push(format!("{} updated pages", self.pages_updated));
        }
        if self.pages_removed > 0 {
            parts.push(format!("{} removed pages", self.pages_removed));
        }

        format!("Sync complete: {}", parts.join(", "))
    }
}

/// Information about a broken link found during sync.
#[derive(Debug, Clone)]
pub struct BrokenLink {
    /// File containing the broken link.
    pub source_file: PathBuf,
    /// The link text that was broken.
    pub link_text: String,
    /// The target that was not found.
    pub target: String,
}

/// Perform a full sync of the AIWiki.
///
/// This function:
/// 1. Scans the raw/ directory for changes
/// 2. Detects new, modified, and deleted source files
/// 3. Extracts text and calls the LLM to generate wiki content
/// 4. Creates wiki pages in sources/, entities/, concepts/
/// 5. Validates cross-links
/// 6. Updates state.json
///
/// # Arguments
/// * `wiki` - The AIWiki instance to sync
/// * `force` - If true, re-process all files even if unchanged
/// * `extractor` - Optional LLM extractor for content generation.
///   If `None`, sync only tracks file hashes without generating pages.
///
/// # Returns
/// A SyncResult with details about what was done.
pub async fn sync(
    wiki: &Aiwiki,
    force: bool,
    extractor: Option<&dyn LlmExtractor>,
    progress: Option<&Arc<SyncProgress>>,
) -> Result<SyncResult> {
    if !wiki.config.enabled {
        return Err(AiwikiError::Config(
            "AIWiki is disabled. Run `/aiwiki on` to enable.".to_string(),
        ));
    }

    let mut result = SyncResult::default();
    let raw_dir = wiki.path("raw");

    // Detect changes
    let changes = if force {
        // Force sync: treat all existing files as modified
        let mut new = Vec::new();
        let modified = wiki.state.files.keys().cloned().collect::<Vec<_>>();
        scan_all_files(&raw_dir, &mut new).await?;

        // Also scan all source folders
        for source in wiki.config.enabled_sources() {
            let files =
                scan_source_folder(&wiki.root, source, &wiki.config.ignore_patterns).await?;
            for file in files {
                let source_path = wiki.root.join(&source.path);
                let state_key = if source.is_file {
                    // For single file sources, use the filename as the relative path
                    let file_name = source_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&source.path)
                        .to_string();
                    make_ref_key(&source.path, &file_name)
                } else {
                    // For directories, calculate relative path from source_path
                    let relative = file
                        .strip_prefix(&source_path)
                        .map_err(|e| AiwikiError::Config(e.to_string()))?;
                    make_ref_key(&source.path, &relative.to_string_lossy())
                };
                if !wiki.state.files.contains_key(&state_key) {
                    new.push(state_key);
                }
            }
        }
        // Remove files that are already tracked from "new"
        new.retain(|f| !wiki.state.files.contains_key(f));

        crate::state::Changes {
            new,
            modified,
            deleted: Vec::new(),
        }
    } else {
        // Use get_all_changes if sources exist
        let changes = if !wiki.config.sources.is_empty() {
            wiki.state
                .get_all_changes(
                    &raw_dir,
                    &wiki.root,
                    &wiki.config.sources,
                    &wiki
                        .config
                        .ignore_patterns
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>(),
                )
                .await?
        } else {
            wiki.state.get_changes(&raw_dir).await?
        };

        // If an extractor is available, re-process tracked files that have
        // no generated pages (e.g. from a previous sync without an LLM).
        let mut changes = changes;
        if extractor.is_some() {
            for (path, file_state) in &wiki.state.files {
                if file_state.generated_pages.is_empty()
                    && !changes.new.contains(path)
                    && !changes.modified.contains(path)
                {
                    changes.modified.push(path.clone());
                }
            }
        }
        changes
    };

    // Process new files
    let total_llm_files = changes.new.len() + changes.modified.len();
    let mut llm_file_idx = 0;

    // Report total to progress tracker
    if let Some(p) = progress {
        p.total.store(
            (total_llm_files + changes.deleted.len()) as u32,
            Ordering::Relaxed,
        );
    }

    for path in &changes.new {
        match process_new_source(wiki, path, extractor, Some(wiki.root.as_path())).await {
            Ok(pages) => {
                result.new_count += 1;
                result.pages_created += pages.len();
            }
            Err(e) => {
                result
                    .errors
                    .push(format!("Failed to process new file {}: {}", path, e));
            }
        }
        llm_file_idx += 1;
        if let Some(p) = progress {
            p.current.store(llm_file_idx as u32, Ordering::Relaxed);
        }
        if llm_file_idx < total_llm_files {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }

    // Process modified files
    for path in &changes.modified {
        match process_modified_source(wiki, path, extractor, Some(wiki.root.as_path())).await {
            Ok(pages) => {
                result.updated_count += 1;
                result.pages_updated += pages.len();
            }
            Err(e) => {
                result
                    .errors
                    .push(format!("Failed to update file {}: {}", path, e));
            }
        }
        llm_file_idx += 1;
        if let Some(p) = progress {
            p.current.store(llm_file_idx as u32, Ordering::Relaxed);
        }
        if llm_file_idx < total_llm_files {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    }

    // Process deleted files
    for path in &changes.deleted {
        match process_deleted_source(wiki, path).await {
            Ok(pages_removed) => {
                result.deleted_count += 1;
                result.pages_removed += pages_removed;
            }
            Err(e) => {
                result
                    .errors
                    .push(format!("Failed to remove file {}: {}", path, e));
            }
        }
        if let Some(p) = progress {
            p.current.fetch_add(1, Ordering::Relaxed);
        }
    }

    // Validate cross-links
    let validation = validate_wiki_links(wiki).await?;
    result.broken_links = validation
        .broken
        .into_iter()
        .map(|(source, target)| BrokenLink {
            source_file: source.clone(),
            link_text: target.clone(),
            target,
        })
        .collect();

    // Reload the state that process_new_source / process_modified_source saved,
    // then update the sync timestamp and page count on top of it.
    let mut state = crate::AiwikiState::load(&wiki.wiki_dir)
        .await
        .unwrap_or_else(|_| wiki.state.clone());
    state.page_count = count_wiki_pages(&wiki.path("wiki")).await;
    state.mark_synced();
    state.save(&wiki.wiki_dir).await?;

    Ok(result)
}

/// Check if the wiki needs syncing (has any changes).
///
/// Returns true if there are new, modified, or deleted source files
/// that haven't been processed yet.
pub async fn needs_sync(wiki: &Aiwiki) -> Result<bool> {
    if !wiki.config.enabled {
        return Ok(false);
    }

    let raw_dir = wiki.path("raw");
    let changes = wiki.state.get_changes(&raw_dir).await?;
    Ok(!changes.is_empty())
}

/// Get a report of what would be synced (dry run).
pub async fn preview_sync(wiki: &Aiwiki) -> Result<SyncPreview> {
    if !wiki.config.enabled {
        return Err(AiwikiError::Config(
            "AIWiki is disabled. Run `/aiwiki on` to enable.".to_string(),
        ));
    }

    let raw_dir = wiki.path("raw");

    // Get changes from all sources
    let changes = if !wiki.config.sources.is_empty() {
        wiki.state
            .get_all_changes(
                &raw_dir,
                &wiki.root,
                &wiki.config.sources,
                &wiki
                    .config
                    .ignore_patterns
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>(),
            )
            .await?
    } else {
        wiki.state.get_changes(&raw_dir).await?
    };

    let mut new_files = Vec::new();
    let mut modified_files = Vec::new();
    let mut deleted_files = Vec::new();

    for path in &changes.new {
        let full_path = resolve_file_path(&wiki.root, &raw_dir, path);
        if let Ok(metadata) = fs::metadata(&full_path).await {
            new_files.push((path.clone(), metadata.len()));
        }
    }

    for path in &changes.modified {
        let full_path = resolve_file_path(&wiki.root, &raw_dir, path);
        if let Ok(metadata) = fs::metadata(&full_path).await {
            modified_files.push((path.clone(), metadata.len()));
        }
    }

    for path in &changes.deleted {
        deleted_files.push(path.clone());
    }

    Ok(SyncPreview {
        new_files,
        modified_files,
        deleted_files,
        total_changes: changes.len(),
    })
}

/// Preview of changes that would be synced.
#[derive(Debug, Clone)]
pub struct SyncPreview {
    /// New files that would be processed.
    pub new_files: Vec<(String, u64)>,
    /// Modified files that would be re-processed.
    pub modified_files: Vec<(String, u64)>,
    /// Deleted files whose pages would be removed.
    pub deleted_files: Vec<String>,
    /// Total number of changes.
    pub total_changes: usize,
}

impl SyncPreview {
    /// Check if there are any changes to sync.
    pub fn is_empty(&self) -> bool {
        self.total_changes == 0
    }

    /// Generate a human-readable preview.
    pub fn to_string(&self) -> String {
        if self.is_empty() {
            return "No changes to sync. Wiki is up to date.".to_string();
        }

        let mut lines = vec![format!("Sync Preview: {} changes", self.total_changes)];

        if !self.new_files.is_empty() {
            lines.push(format!("\nNew files ({}):", self.new_files.len()));
            for (path, size) in &self.new_files {
                lines.push(format!("  • {} ({} bytes)", path, size));
            }
        }

        if !self.modified_files.is_empty() {
            lines.push(format!("\nModified files ({}):", self.modified_files.len()));
            for (path, size) in &self.modified_files {
                lines.push(format!("  • {} ({} bytes)", path, size));
            }
        }

        if !self.deleted_files.is_empty() {
            lines.push(format!("\nDeleted files ({}):", self.deleted_files.len()));
            for path in &self.deleted_files {
                lines.push(format!("  • {}", path));
            }
        }

        lines.join("\n")
    }
}

/// Process a new source file: extract text, call LLM, generate wiki pages.
///
/// If no extractor is provided, only tracks the file hash without generating pages.
///
/// # Arguments
///
/// * `wiki` - The AIWiki instance
/// * `relative_path` - The state key (e.g., "readme.md" or "ref:docs/guide.md")
/// * `extractor` - Optional LLM extractor for generating wiki pages
/// * `project_root` - Optional project root for resolving ref: paths
async fn process_new_source(
    wiki: &Aiwiki,
    relative_path: &str,
    extractor: Option<&dyn LlmExtractor>,
    project_root: Option<&Path>,
) -> Result<Vec<PathBuf>> {
    let raw_dir = wiki.path("raw");

    // Resolve the actual file path
    let (full_path, source_label) = if relative_path.starts_with("ref:") {
        let root = project_root.ok_or_else(|| {
            AiwikiError::Config("Project root required for ref: paths".to_string())
        })?;
        // Use resolve_file_path which correctly handles single file sources
        let source_path = resolve_file_path(root, &raw_dir, relative_path);
        let source = get_source_name(relative_path);
        (source_path, Some(source.to_string()))
    } else {
        (raw_dir.join(relative_path), None)
    };

    let mut generated_page_paths: Vec<String> = Vec::new();

    if let Some(llm) = extractor {
        // Extract text from the source file
        let doc_type = DocumentType::from_path(&full_path);
        if doc_type.supports_extraction() {
            let text = extract_text(&full_path, doc_type).await.map_err(|e| {
                AiwikiError::Config(format!("Text extraction failed for {relative_path}: {e}"))
            })?;

            if text.trim().is_empty() {
                tracing::info!("Empty text extracted from {relative_path}, skipping LLM");
            } else {
                // Build extraction prompt and call LLM
                let source_name = Path::new(relative_path)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| relative_path.to_string());

                let prompt = extraction::build_extraction_prompt(
                    relative_path,
                    &text,
                    wiki.config.extraction.extract_entities,
                    wiki.config.extraction.extract_concepts,
                );

                let response = llm
                    .complete(
                        extraction::EXTRACTION_SYSTEM_PROMPT,
                        &prompt,
                        wiki.config.extraction.max_tokens,
                        wiki.config.extraction.temperature,
                    )
                    .await
                    .map_err(|e| {
                        AiwikiError::Config(format!(
                            "LLM extraction failed for {relative_path}: {e}"
                        ))
                    })?;

                let result = extraction::parse_extraction_response(&response).map_err(|e| {
                    AiwikiError::Config(format!(
                        "Failed to parse LLM response for {relative_path}: {e}"
                    ))
                })?;

                generated_page_paths =
                    crate::pages::write_pages(&wiki.wiki_dir, &source_name, &result)
                        .await
                        .map_err(|e| {
                            AiwikiError::Config(format!(
                                "Failed to write pages for {relative_path}: {e}"
                            ))
                        })?;
            }
        } else {
            tracing::info!("Unsupported doc type for {relative_path}, skipping extraction");
        }
    }

    // Update state to track the file and its generated pages
    let mut state = crate::AiwikiState::load(&wiki.wiki_dir)
        .await
        .unwrap_or_else(|_| wiki.state.clone());

    // For ref: paths, calculate hash directly from full_path
    let hash = crate::AiwikiState::calculate_hash(&full_path).await?;
    let metadata = fs::metadata(&full_path).await?;

    state.files.insert(
        relative_path.to_string(),
        crate::FileState {
            hash,
            modified: chrono::Utc::now(),
            size: metadata.len(),
            generated_pages: generated_page_paths.clone(),
            source: source_label,
        },
    );

    state.save(&wiki.wiki_dir).await?;

    Ok(generated_page_paths
        .into_iter()
        .map(PathBuf::from)
        .collect())
}

/// Process a modified source file: re-extract text, regenerate wiki pages.
///
/// Removes old generated pages and creates new ones.
///
/// # Arguments
///
/// * `wiki` - The AIWiki instance
/// * `relative_path` - The state key (e.g., "readme.md" or "ref:docs/guide.md")
/// * `extractor` - Optional LLM extractor for generating wiki pages
/// * `project_root` - Optional project root for resolving ref: paths
async fn process_modified_source(
    wiki: &Aiwiki,
    relative_path: &str,
    extractor: Option<&dyn LlmExtractor>,
    project_root: Option<&Path>,
) -> Result<Vec<PathBuf>> {
    // Remove previously generated pages for this source
    if let Some(file_state) = wiki.state.files.get(relative_path) {
        for page_path in &file_state.generated_pages {
            let full_page = wiki.wiki_dir.join(page_path);
            if full_page.exists() {
                let _ = fs::remove_file(&full_page).await;
            }
        }
    }

    // Re-process as if new
    process_new_source(wiki, relative_path, extractor, project_root).await
}

/// Process a deleted source file.
async fn process_deleted_source(wiki: &Aiwiki, relative_path: &str) -> Result<usize> {
    // Get the list of pages that were generated from this source
    let generated_pages = wiki
        .state
        .files
        .get(relative_path)
        .map(|f| f.generated_pages.clone())
        .unwrap_or_default();

    // Remove the source from state
    let mut state = wiki.state.clone();
    state.remove_file(relative_path);
    state.save(&wiki.wiki_dir).await?;

    // TODO: In Milestone 2.5+, also remove/update generated pages

    Ok(generated_pages.len())
}

/// Scan all files in the raw directory recursively.
async fn scan_all_files(dir: &Path, files: &mut Vec<String>) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    let base_path = dir.to_path_buf();
    let mut stack = vec![base_path.clone()];

    while let Some(current) = stack.pop() {
        let mut entries = fs::read_dir(&current).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let metadata = entry.metadata().await?;

            if metadata.is_file() {
                if let Ok(relative) = path.strip_prefix(&base_path) {
                    files.push(relative.to_string_lossy().to_string());
                }
            } else if metadata.is_dir() {
                stack.push(path);
            }
        }
    }

    Ok(())
}

/// Count all `.md` files under the wiki directory (excluding log.md).
async fn count_wiki_pages(wiki_dir: &Path) -> usize {
    let mut count = 0usize;
    let mut stack = vec![wiki_dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let mut entries = match fs::read_dir(&current).await {
            Ok(e) => e,
            Err(_) => continue,
        };
        while let Some(entry) = entries.next_entry().await.ok().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                if path.file_name().and_then(|n| n.to_str()) != Some("log.md") {
                    count += 1;
                }
            }
        }
    }
    count
}

/// Create a file watcher for automatic syncing.
pub async fn create_watcher(wiki: &Aiwiki, config: WatcherConfig) -> Result<FileWatcher> {
    if !wiki.config.enabled {
        return Err(AiwikiError::Config(
            "AIWiki is disabled. Run `/aiwiki on` to enable.".to_string(),
        ));
    }

    let raw_dir = wiki.path("raw");
    FileWatcher::new(raw_dir, config).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_sync_result_summary() {
        let result = SyncResult {
            new_count: 2,
            updated_count: 1,
            deleted_count: 0,
            pages_created: 3,
            pages_updated: 1,
            pages_removed: 0,
            broken_links: Vec::new(),
            errors: Vec::new(),
        };

        let summary = result.summary();
        assert!(summary.contains("2 new sources"));
        assert!(summary.contains("1 updated"));
        assert!(summary.contains("3 new pages"));
    }

    #[test]
    fn test_sync_preview_empty() {
        let preview = SyncPreview {
            new_files: Vec::new(),
            modified_files: Vec::new(),
            deleted_files: Vec::new(),
            total_changes: 0,
        };

        assert!(preview.is_empty());
        assert_eq!(
            preview.to_string(),
            "No changes to sync. Wiki is up to date."
        );
    }

    #[test]
    fn test_sync_preview_with_changes() {
        let preview = SyncPreview {
            new_files: vec![("file1.md".to_string(), 1024)],
            modified_files: vec![("file2.md".to_string(), 2048)],
            deleted_files: vec!["old_file.md".to_string()],
            total_changes: 3,
        };

        assert!(!preview.is_empty());
        let text = preview.to_string();
        assert!(text.contains("New files"));
        assert!(text.contains("Modified files"));
        assert!(text.contains("Deleted files"));
    }
}
