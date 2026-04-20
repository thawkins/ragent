//! Extraction worker for processing file changes from the watcher.
//!
//! `ExtractionWorker` runs as a background task that:
//! - Drains events from the watch channel
//! - Debounces events (5s window) to batch rapid changes
//! - Dedupes events (later events override earlier for same file)
//! - Rate-limits LLM calls (5s gap between extractions)
//! - Processes changed files through the LLM extractor
//! - Removes wiki pages for deleted files

use crate::extraction::LlmExtractor;
use crate::ingest::extractors::{DocumentType, extract_text};
use crate::pages::write_pages;
use crate::sync::make_ref_key;
use crate::sync::source_watcher::WatchEvent;
use crate::{Aiwiki, Result, extraction};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};
use tokio::fs;
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, info, trace, warn};

/// Shared progress state for the watch session.
pub struct WatcherProgress {
    /// Current queue depth (number of events waiting).
    pub queue_depth: AtomicU32,
    /// Whether extraction is currently in progress.
    pub is_processing: AtomicBool,
    /// Last file that was processed.
    pub last_file: Mutex<Option<String>>,
}

impl WatcherProgress {
    /// Create new watcher progress.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            queue_depth: AtomicU32::new(0),
            is_processing: AtomicBool::new(false),
            last_file: Mutex::new(None),
        })
    }
}

/// Worker that processes file change events and runs LLM extraction.
pub struct ExtractionWorker {
    progress: Arc<WatcherProgress>,
    stop_flag: Arc<AtomicBool>,
}

/// Batch of events to be processed together.
#[derive(Debug)]
struct EventBatch {
    /// Files to extract (path -> source).
    to_extract: HashMap<PathBuf, String>,
    /// Files to remove (path -> source).
    to_remove: HashMap<PathBuf, String>,
    /// Files that were created in this batch (not pre-existing).
    created: std::collections::HashSet<PathBuf>,
}

impl Default for EventBatch {
    fn default() -> Self {
        Self {
            to_extract: HashMap::new(),
            to_remove: HashMap::new(),
            created: std::collections::HashSet::new(),
        }
    }
}

impl EventBatch {
    fn new() -> Self {
        Self::default()
    }

    fn add_created(&mut self, path: PathBuf, source: String) {
        self.created.insert(path.clone());
        self.to_extract.insert(path, source);
    }

    fn add_changed(&mut self, path: PathBuf, source: String) {
        // A change event means the file already existed on disk, so
        // clear any "created in this batch" flag.
        self.created.remove(&path);
        self.to_extract.insert(path, source);
    }

    fn add_deleted(&mut self, path: PathBuf, source: String) {
        self.to_extract.remove(&path);
        // If the file was created in this same batch, the create and
        // delete cancel out — no need to track a removal.
        if !self.created.remove(&path) {
            self.to_remove.insert(path, source);
        }
    }

    fn is_empty(&self) -> bool {
        self.to_extract.is_empty() && self.to_remove.is_empty()
    }
}

impl ExtractionWorker {
    /// Create a new extraction worker.
    pub fn new(progress: Arc<WatcherProgress>, stop_flag: Arc<AtomicBool>) -> Self {
        Self {
            progress,
            stop_flag,
        }
    }

    /// Run the extraction worker loop.
    ///
    /// # Arguments
    ///
    /// * `rx` - Channel receiver for watch events
    /// * `wiki` - AIWiki instance for state and config access
    /// * `extractor` - LLM extractor for content generation
    /// * `project_root` - Root path of the project
    pub async fn run(
        &self,
        mut rx: mpsc::Receiver<WatchEvent>,
        wiki: Arc<Mutex<Aiwiki>>,
        extractor: Arc<dyn LlmExtractor + Send + Sync>,
        project_root: PathBuf,
    ) {
        let mut last_event_time: Option<Instant> = None;
        let debounce_duration = Duration::from_secs(5);
        let rate_limit_duration = Duration::from_secs(5);
        let mut last_extraction_time: Option<Instant> = None;

        loop {
            // Check if we should stop
            if self.stop_flag.load(Ordering::Relaxed) {
                info!("Extraction worker stopping");
                break;
            }

            // Try to receive events (non-blocking)
            let mut batch = EventBatch::new();

            // Collect all available events
            while let Ok(event) = rx.try_recv() {
                trace!("Received watch event: {:?}", event);
                last_event_time = Some(Instant::now());

                match event {
                    WatchEvent::Created { path, source } => {
                        batch.add_created(path, source);
                    }
                    WatchEvent::Changed { path, source } => {
                        batch.add_changed(path, source);
                    }
                    WatchEvent::Deleted { path, source } => {
                        batch.add_deleted(path, source);
                    }
                }
            }

            // Update queue depth
            let pending_count = batch.to_extract.len() + batch.to_remove.len();
            self.progress
                .queue_depth
                .store(pending_count as u32, Ordering::Relaxed);

            // Debounce: wait until 5s after last event
            if let Some(last_time) = last_event_time {
                let elapsed = last_time.elapsed();
                if elapsed < debounce_duration {
                    // Not enough time passed, sleep and retry
                    let sleep_duration = debounce_duration - elapsed;
                    tokio::time::sleep(sleep_duration.min(Duration::from_millis(100))).await;
                    continue;
                }
            }

            // Process batch if we have events and debounce passed
            if !batch.is_empty()
                && last_event_time
                    .map(|t| t.elapsed() >= debounce_duration)
                    .unwrap_or(true)
            {
                // Rate limit: ensure 5s gap between extractions
                if let Some(last_extraction) = last_extraction_time {
                    let elapsed = last_extraction.elapsed();
                    if elapsed < rate_limit_duration {
                        let sleep_duration = rate_limit_duration - elapsed;
                        trace!("Rate limiting: sleeping for {:?}", sleep_duration);
                        tokio::time::sleep(sleep_duration).await;
                    }
                }

                self.progress.is_processing.store(true, Ordering::Relaxed);

                // Process deletions first
                for (path, source) in batch.to_remove {
                    if let Err(e) = self.process_deletion(&wiki, &path, &source).await {
                        warn!("Failed to process deletion for {:?}: {}", path, e);
                    }
                }

                // Process extractions
                for (path, source) in batch.to_extract {
                    // Update last file
                    let last_file = path.to_string_lossy().to_string();
                    *self.progress.last_file.lock().await = Some(last_file.clone());

                    if let Err(e) = self
                        .process_extraction(&wiki, &extractor, &path, &source, &project_root)
                        .await
                    {
                        warn!("Failed to process extraction for {:?}: {}", path, e);
                    }

                    // Rate limit between extractions
                    last_extraction_time = Some(Instant::now());
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }

                self.progress.is_processing.store(false, Ordering::Relaxed);

                // Clear last_event_time since we processed this batch
                last_event_time = None;
            }

            // Sleep to prevent busy-spin
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Process a file deletion event.
    async fn process_deletion(
        &self,
        wiki: &Arc<Mutex<Aiwiki>>,
        path: &Path,
        source: &str,
    ) -> Result<()> {
        let wiki_guard = wiki.lock().await;

        // Build state key
        let state_key = if source == "raw" {
            path.to_string_lossy().to_string()
        } else {
            // For single file sources, check if path matches source
            let is_single_file = path.to_string_lossy() == source
                || path.file_name().map(|n| n.to_string_lossy())
                    == Path::new(source).file_name().map(|n| n.to_string_lossy());
            if is_single_file {
                format!("ref:{}/", source)
            } else {
                make_ref_key(source, &path.to_string_lossy())
            }
        }; // Remove generated pages for this source
        if let Some(file_state) = wiki_guard.state.files.get(&state_key) {
            let wiki_dir = wiki_guard.root.join("wiki");
            for page_id in &file_state.generated_pages {
                let page_path = wiki_dir.join(format!("{}.md", page_id));
                if page_path.exists() {
                    fs::remove_file(&page_path).await?;
                    debug!("Removed wiki page: {}", page_path.display());
                }
            }
        }

        // Remove from state
        drop(wiki_guard);
        let mut wiki_guard = wiki.lock().await;
        wiki_guard.state.remove_file(&state_key);
        wiki_guard.state.save(&wiki_guard.root).await?;

        info!("Processed deletion: {}", state_key);
        Ok(())
    }

    /// Process a file extraction event.
    async fn process_extraction(
        &self,
        wiki: &Arc<Mutex<Aiwiki>>,
        extractor: &Arc<dyn LlmExtractor + Send + Sync>,
        path: &Path,
        source: &str,
        project_root: &Path,
    ) -> Result<()> {
        let wiki_guard = wiki.lock().await;

        // Build state key and resolve full path
        let (state_key, full_path) = if source == "raw" {
            let key = path.to_string_lossy().to_string();
            let full = wiki_guard.root.join("raw").join(path);
            (key, full)
        } else {
            // For single file sources, the path IS the source
            let is_single_file = path.to_string_lossy() == source
                || path.file_name().map(|n| n.to_string_lossy())
                    == Path::new(source).file_name().map(|n| n.to_string_lossy());
            // For single file sources, state key is "ref:source/" (source path with trailing slash)
            // For directory sources, state key is "ref:source/path"
            let key = if is_single_file {
                format!("ref:{}/", source)
            } else {
                make_ref_key(source, &path.to_string_lossy())
            };
            let full = if is_single_file {
                project_root.join(source)
            } else {
                project_root.join(source).join(path)
            };
            (key, full)
        }; // Check if file exists and hash
        if !full_path.exists() {
            warn!("File no longer exists: {}", full_path.display());
            return Ok(());
        }

        let content = fs::read(&full_path).await?;
        let hash = crate::state::AiwikiState::hash_bytes(&content);

        // Check if hash changed
        if let Some(existing) = wiki_guard.state.files.get(&state_key) {
            if existing.hash == hash {
                trace!("File unchanged, skipping: {}", state_key);
                return Ok(());
            }
        }

        // Determine document type
        let doc_type = DocumentType::from_path(&full_path);

        // Extract text
        let text = if doc_type.supports_extraction() {
            match extract_text(&full_path, doc_type.clone()).await {
                Ok(t) if !t.trim().is_empty() => t,
                _ => {
                    debug!(
                        "Empty or failed text extraction for {}",
                        full_path.display()
                    );
                    return Ok(());
                }
            }
        } else {
            debug!("Unsupported doc type for {}", full_path.display());
            return Ok(());
        };

        // Build prompt
        let filename = full_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let prompt = extraction::build_extraction_prompt(
            &filename,
            &text,
            wiki_guard.config.extraction.extract_entities,
            wiki_guard.config.extraction.extract_concepts,
        );

        // Capture config values before dropping lock
        let max_tokens = wiki_guard.config.extraction.max_tokens;
        let temperature = wiki_guard.config.extraction.temperature;

        // Call LLM
        drop(wiki_guard); // Release lock during LLM call

        let response = extractor
            .complete(
                extraction::EXTRACTION_SYSTEM_PROMPT,
                &prompt,
                max_tokens,
                temperature,
            )
            .await
            .map_err(|e| crate::AiwikiError::ExtractionError(e.to_string()))?;

        // Parse response
        let result = extraction::parse_extraction_response(&response)
            .map_err(|e| crate::AiwikiError::ExtractionError(e.to_string()))?;

        // Write pages
        let mut wiki_guard = wiki.lock().await;
        let wiki_dir = wiki_guard.root.join("wiki");
        let page_ids = write_pages(&wiki_dir, &state_key, &result).await?;

        // Update state by inserting directly into files HashMap
        let metadata = fs::metadata(&full_path).await?;
        wiki_guard.state.files.insert(
            state_key.clone(),
            crate::state::FileState {
                hash,
                modified: chrono::Utc::now(),
                size: metadata.len(),
                generated_pages: page_ids.clone(),
                source: Some(source.to_string()),
            },
        );
        wiki_guard.state.save(&wiki_guard.root).await?;

        info!(
            "Processed extraction for {}: {} pages",
            state_key,
            page_ids.len()
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_batch() {
        let mut batch = EventBatch::new();

        // Add created
        batch.add_created(PathBuf::from("test.md"), "docs".to_string());
        assert!(!batch.is_empty());
        assert_eq!(batch.to_extract.len(), 1);

        // Add changed (should replace created)
        batch.add_changed(PathBuf::from("test.md"), "docs".to_string());
        assert_eq!(batch.to_extract.len(), 1);

        // Add deleted (should move to remove)
        batch.add_deleted(PathBuf::from("test.md"), "docs".to_string());
        assert_eq!(batch.to_extract.len(), 0);
        assert_eq!(batch.to_remove.len(), 1);

        // Create then delete = nothing
        let mut batch2 = EventBatch::new();
        batch2.add_created(PathBuf::from("temp.md"), "docs".to_string());
        batch2.add_deleted(PathBuf::from("temp.md"), "docs".to_string());
        assert!(batch2.is_empty());
    }

    #[test]
    fn test_watcher_progress() {
        let progress = WatcherProgress::new();
        assert_eq!(progress.queue_depth.load(Ordering::Relaxed), 0);
        assert!(!progress.is_processing.load(Ordering::Relaxed));
    }
}
