//! Watch session for AIWiki - manages the source watcher and extraction worker.
//!
//! `AiwikiWatchSession` ties together:
//! - `SourceWatcher` for file system event monitoring
//! - `ExtractionWorker` for processing events and LLM extraction
//! - Shared state for TUI status display

use crate::extraction::LlmExtractor;
use crate::sync::extraction_worker::{ExtractionWorker, WatcherProgress};
use crate::sync::source_watcher::SourceWatcher;
use crate::{Aiwiki, AiwikiConfig};
use anyhow::{Context, Result};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{Mutex, mpsc};
use tracing::{debug, info};

/// Manages a running watch session for AIWiki.
pub struct AiwikiWatchSession {
    /// The source watcher instance.
    _watcher: SourceWatcher,
    /// Handle to the extraction worker task.
    worker_handle: Option<tokio::task::JoinHandle<()>>,
    /// Flag to signal worker shutdown.
    stop_flag: Arc<AtomicBool>,
    /// Shared progress state for TUI display.
    progress: Arc<WatcherProgress>,
}

impl AiwikiWatchSession {
    /// Start a new watch session.
    ///
    /// # Arguments
    ///
    /// * `wiki_root` - Path to the AIWiki directory (contains config.json, state.json)
    /// * `config` - AIWiki configuration
    /// * `wiki` - AIWiki instance for state and config access
    /// * `extractor` - LLM extractor for content generation
    ///
    /// # Returns
    ///
    /// Returns a new watch session that runs until `stop()` is called.
    pub fn start(
        wiki_root: &Path,
        config: &AiwikiConfig,
        wiki: Arc<Mutex<Aiwiki>>,
        extractor: Arc<dyn LlmExtractor + Send + Sync>,
    ) -> Result<Self> {
        let project_root = wiki_root
            .parent()
            .context("Wiki root must have a parent directory")?;

        let (tx, rx) = mpsc::channel(100);
        let raw_dir = wiki_root.join("raw");

        // Create source watcher
        let watcher = SourceWatcher::new(project_root, &config.sources, &raw_dir, tx)
            .context("Failed to create source watcher")?;

        debug!("Watching {} source paths", watcher.watched_paths().len());

        // Create stop flag and progress
        let stop_flag = Arc::new(AtomicBool::new(false));
        let progress = WatcherProgress::new();

        // Create and spawn extraction worker
        let project_root_owned = project_root.to_path_buf();
        let worker_handle = tokio::spawn({
            let progress = progress.clone();
            let stop_flag = stop_flag.clone();
            async move {
                let worker = ExtractionWorker::new(progress, stop_flag);
                worker.run(rx, wiki, extractor, project_root_owned).await;
            }
        });

        info!("AIWiki watch session started");

        Ok(Self {
            _watcher: watcher,
            worker_handle: Some(worker_handle),
            stop_flag,
            progress,
        })
    }

    /// Stop the watch session gracefully.
    pub async fn stop(&mut self) {
        info!("Stopping AIWiki watch session");
        self.stop_flag.store(true, Ordering::Relaxed);

        if let Some(handle) = self.worker_handle.take() {
            // Give the worker a chance to finish gracefully
            tokio::time::timeout(std::time::Duration::from_secs(5), handle)
                .await
                .ok();
        }

        info!("AIWiki watch session stopped");
    }

    /// Get the shared progress state for TUI display.
    pub fn progress(&self) -> &Arc<WatcherProgress> {
        &self.progress
    }

    /// Check if the watch session is currently active (not stopped).
    pub fn is_active(&self) -> bool {
        !self.stop_flag.load(Ordering::Relaxed) && self.worker_handle.is_some()
    }

    /// Get the number of events currently queued for processing.
    pub fn queue_depth(&self) -> u32 {
        self.progress.queue_depth.load(Ordering::Relaxed)
    }

    /// Check if extraction is currently in progress.
    pub fn is_processing(&self) -> bool {
        self.progress.is_processing.load(Ordering::Relaxed)
    }

    /// Get the last file that was processed.
    pub async fn last_file(&self) -> Option<String> {
        self.progress.last_file.lock().await.clone()
    }
}

impl Drop for AiwikiWatchSession {
    fn drop(&mut self) {
        // Signal stop if not already done
        if self.worker_handle.is_some() {
            self.stop_flag.store(true, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AiwikiConfig;
    use crate::source_folder::SourceFolder;

    struct MockExtractor;

    #[async_trait::async_trait]
    impl LlmExtractor for MockExtractor {
        async fn complete(
            &self,
            _system: &str,
            _user: &str,
            _max_tokens: u32,
            _temperature: f32,
        ) -> Result<String, String> {
            Ok(r#"{"title": "Test", "summary": "Test", "entities": [], "concepts": [], "tags": []}"#.to_string())
        }
    }

    #[tokio::test]
    async fn test_watch_session_lifecycle() {
        let temp_dir = tempfile::tempdir().unwrap();
        let wiki_root = temp_dir.path().join("aiwiki");
        std::fs::create_dir(&wiki_root).unwrap();
        std::fs::create_dir(wiki_root.join("raw")).unwrap();
        std::fs::create_dir(wiki_root.join("wiki")).unwrap();

        // Create minimal config
        let config = AiwikiConfig::default();
        let config_json = serde_json::to_string_pretty(&config).unwrap();
        std::fs::write(wiki_root.join("config.json"), config_json).unwrap();

        // Create minimal state
        let state = crate::state::AiwikiState::default();
        let state_json = serde_json::to_string_pretty(&state).unwrap();
        std::fs::write(wiki_root.join("state.json"), state_json).unwrap();

        // Load wiki
        let wiki = Arc::new(Mutex::new(Aiwiki::new(temp_dir.path()).await.unwrap()));
        let extractor: Arc<dyn LlmExtractor + Send + Sync> = Arc::new(MockExtractor);

        // Start session
        let mut session = AiwikiWatchSession::start(&wiki_root, &config, wiki, extractor).unwrap();
        assert!(session.is_active());

        // Stop session
        session.stop().await;
        assert!(!session.is_active());
    }
}
