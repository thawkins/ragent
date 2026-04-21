//! Background indexing worker with debounce, dedup, and batching.
//!
//! `IndexWorker` receives `WatchEvent`s via a channel, collects them into
//! efficient batches, and applies them to a [`CodeIndex`].
//! The worker is controlled through `IndexWorkerHandle`.

use crate::CodeIndex;
use crate::watcher::WatchEvent;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};
use tracing::{debug, info, trace, warn};

/// Configuration for the background indexing worker.
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    /// Delay after last event before processing a batch (milliseconds).
    pub debounce_ms: u64,
    /// Maximum number of files per indexing batch.
    pub batch_size: usize,
    /// Maximum number of pending events before dropping oldest.
    pub max_queue_size: usize,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            debounce_ms: 500,
            batch_size: 50,
            max_queue_size: 10_000,
        }
    }
}

/// Live statistics from the indexing worker.
#[derive(Debug, Clone, Default)]
pub struct WorkerStats {
    /// Number of files indexed since start.
    pub files_indexed: u64,
    /// Number of files removed since start.
    pub files_removed: u64,
    /// Number of batches processed.
    pub batches_processed: u64,
    /// Whether the worker is currently processing a batch.
    pub is_busy: bool,
}

/// Handle to control the background indexing worker.
pub struct IndexWorkerHandle {
    stop_flag: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
    stats: Arc<SharedStats>,
    manual_tx: mpsc::Sender<ManualCommand>,
}

enum ManualCommand {
    ReindexFile(PathBuf),
    FullReindex,
}

struct SharedStats {
    files_indexed: AtomicU64,
    files_removed: AtomicU64,
    batches_processed: AtomicU64,
    is_busy: AtomicBool,
}

impl SharedStats {
    fn new() -> Self {
        Self {
            files_indexed: AtomicU64::new(0),
            files_removed: AtomicU64::new(0),
            batches_processed: AtomicU64::new(0),
            is_busy: AtomicBool::new(false),
        }
    }

    fn snapshot(&self) -> WorkerStats {
        WorkerStats {
            files_indexed: self.files_indexed.load(Ordering::Relaxed),
            files_removed: self.files_removed.load(Ordering::Relaxed),
            batches_processed: self.batches_processed.load(Ordering::Relaxed),
            is_busy: self.is_busy.load(Ordering::Relaxed),
        }
    }
}

/// Collects and deduplicates events into an actionable batch.
struct EventBatch {
    /// Files to (re-)index — keeps only the latest event per path.
    to_index: HashSet<PathBuf>,
    /// Files to remove from the index.
    to_remove: HashSet<PathBuf>,
}

impl EventBatch {
    fn new() -> Self {
        Self {
            to_index: HashSet::new(),
            to_remove: HashSet::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.to_index.is_empty() && self.to_remove.is_empty()
    }

    fn len(&self) -> usize {
        self.to_index.len() + self.to_remove.len()
    }

    /// Add an event, deduplicating: a later Delete overrides a Create/Change.
    fn push(&mut self, event: WatchEvent) {
        match event {
            WatchEvent::Created(p) | WatchEvent::Changed(p) => {
                self.to_remove.remove(&p);
                self.to_index.insert(p);
            }
            WatchEvent::Deleted(p) => {
                self.to_index.remove(&p);
                self.to_remove.insert(p);
            }
            WatchEvent::Renamed { from, to } => {
                // Treat as delete old + create new.
                self.to_index.remove(&from);
                self.to_remove.insert(from);
                self.to_remove.remove(&to);
                self.to_index.insert(to);
            }
        }
    }

    fn clear(&mut self) {
        self.to_index.clear();
        self.to_remove.clear();
    }
}

/// The background indexing worker.
///
/// Spawns a thread that receives events and applies them to the `CodeIndex`
/// in debounced, deduplicated batches.
pub struct IndexWorker;

impl IndexWorker {
    /// Start the background worker. Returns a handle for control and stats.
    pub fn start(
        index: Arc<CodeIndex>,
        event_rx: mpsc::Receiver<WatchEvent>,
        config: WorkerConfig,
    ) -> IndexWorkerHandle {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stats = Arc::new(SharedStats::new());
        let (manual_tx, manual_rx) = mpsc::channel();

        let handle = {
            let stop = Arc::clone(&stop_flag);
            let st = Arc::clone(&stats);
            let cfg = config;
            std::thread::Builder::new()
                .name("codeindex-worker".into())
                .spawn(move || {
                    worker_loop(index, event_rx, manual_rx, cfg, stop, st);
                })
                .expect("failed to spawn index worker thread")
        };

        IndexWorkerHandle {
            stop_flag,
            thread: Some(handle),
            stats,
            manual_tx,
        }
    }
}

impl IndexWorkerHandle {
    /// Stop the worker gracefully, waiting for it to finish.
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }

    /// Manually queue a single file for re-indexing.
    pub fn queue_reindex(&self, path: PathBuf) {
        let _ = self.manual_tx.send(ManualCommand::ReindexFile(path));
    }

    /// Manually trigger a full reindex.
    pub fn queue_full_reindex(&self) {
        let _ = self.manual_tx.send(ManualCommand::FullReindex);
    }

    /// Get current worker statistics.
    pub fn stats(&self) -> WorkerStats {
        self.stats.snapshot()
    }

    /// Whether the worker has been asked to stop.
    pub fn is_stopped(&self) -> bool {
        self.stop_flag.load(Ordering::SeqCst)
    }
}

impl Drop for IndexWorkerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Main worker loop — runs on a dedicated thread.
fn worker_loop(
    index: Arc<CodeIndex>,
    event_rx: mpsc::Receiver<WatchEvent>,
    manual_rx: mpsc::Receiver<ManualCommand>,
    config: WorkerConfig,
    stop: Arc<AtomicBool>,
    stats: Arc<SharedStats>,
) {
    let debounce = Duration::from_millis(config.debounce_ms);
    let mut batch = EventBatch::new();
    let mut last_event_time: Option<Instant> = None;

    debug!(
        "index worker started (debounce={}ms, batch_size={}, max_queue={})",
        config.debounce_ms, config.batch_size, config.max_queue_size
    );

    loop {
        if stop.load(Ordering::SeqCst) {
            debug!("index worker stopping (stop flag set)");
            break;
        }

        // Check for manual commands (non-blocking).
        while let Ok(cmd) = manual_rx.try_recv() {
            match cmd {
                ManualCommand::ReindexFile(p) => {
                    batch.push(WatchEvent::Changed(p));
                    last_event_time = Some(Instant::now());
                }
                ManualCommand::FullReindex => {
                    info!("manual full reindex requested");
                    stats.is_busy.store(true, Ordering::Relaxed);
                    match index.full_reindex() {
                        Ok(result) => {
                            info!("full reindex complete: {result}");
                            stats.files_indexed.fetch_add(
                                (result.files_added + result.files_updated) as u64,
                                Ordering::Relaxed,
                            );
                            stats
                                .files_removed
                                .fetch_add(result.files_removed as u64, Ordering::Relaxed);
                            stats.batches_processed.fetch_add(1, Ordering::Relaxed);
                        }
                        Err(e) => warn!("full reindex failed: {e}"),
                    }
                    stats.is_busy.store(false, Ordering::Relaxed);
                    batch.clear();
                }
            }
        }

        // Drain watch events (non-blocking).
        loop {
            match event_rx.try_recv() {
                Ok(ev) => {
                    trace!("watch event: {ev:?}");
                    batch.push(ev);
                    last_event_time = Some(Instant::now());

                    // Drop oldest if queue is too large.
                    if batch.len() > config.max_queue_size {
                        warn!(
                            "event queue exceeded max_queue_size, clearing batch and triggering full reindex"
                        );
                        batch.clear();
                        let _ = index.full_reindex();
                        last_event_time = None;
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    debug!("event channel disconnected, worker stopping");
                    // Process remaining batch before exit.
                    if !batch.is_empty() {
                        process_batch(&index, &mut batch, &config, &stats, &stop);
                    }
                    return;
                }
            }
        }

        // Debounce: only process if enough time has elapsed since last event.
        if let Some(last) = last_event_time {
            if last.elapsed() >= debounce && !batch.is_empty() {
                process_batch(&index, &mut batch, &config, &stats, &stop);
                last_event_time = None;
            }
        }

        // Sleep briefly to avoid busy-spinning.
        std::thread::sleep(Duration::from_millis(50));
    }

    // Process any remaining events before shutdown.
    if !batch.is_empty() {
        debug!(
            "processing remaining {} events before shutdown",
            batch.len()
        );
        process_batch(&index, &mut batch, &config, &stats, &stop);
    }

    debug!("index worker exited");
}

/// Process the current batch of events.
fn process_batch(
    index: &Arc<CodeIndex>,
    batch: &mut EventBatch,
    config: &WorkerConfig,
    stats: &SharedStats,
    stop: &AtomicBool,
) {
    stats.is_busy.store(true, Ordering::Relaxed);

    // Collect paths.
    let to_index: Vec<PathBuf> = batch.to_index.drain().collect();
    let to_remove: Vec<PathBuf> = batch.to_remove.drain().collect();

    // Index files in batches.
    for chunk in to_index.chunks(config.batch_size) {
        if stop.load(Ordering::SeqCst) {
            debug!("aborting batch: stop flag set");
            break;
        }

        let paths: Vec<&std::path::Path> = chunk.iter().map(|p| p.as_path()).collect();
        match index.index_files(&paths) {
            Ok(result) => {
                debug!(
                    "indexed batch of {} files: {} symbols",
                    chunk.len(),
                    result.symbols_extracted
                );
                stats.files_indexed.fetch_add(
                    (result.files_added + result.files_updated) as u64,
                    Ordering::Relaxed,
                );
            }
            Err(e) => warn!("batch index failed: {e}"),
        }
    }

    // Remove deleted files.
    for path in &to_remove {
        if stop.load(Ordering::SeqCst) {
            break;
        }
        if let Err(e) = index.remove_file(path) {
            warn!("remove file failed: {e}");
        } else {
            stats.files_removed.fetch_add(1, Ordering::Relaxed);
        }
    }

    stats.batches_processed.fetch_add(1, Ordering::Relaxed);
    stats.is_busy.store(false, Ordering::Relaxed);
    batch.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CodeIndexConfig;
    use std::sync::mpsc as std_mpsc;

    fn make_test_index(dir: &std::path::Path) -> Arc<CodeIndex> {
        let cfg = CodeIndexConfig {
            enabled: true,
            project_root: dir.to_path_buf(),
            ..Default::default()
        };
        Arc::new(CodeIndex::open_in_memory(&cfg).unwrap())
    }

    #[test]
    fn test_event_batch_dedup() {
        let mut b = EventBatch::new();
        b.push(WatchEvent::Changed(PathBuf::from("src/main.rs")));
        b.push(WatchEvent::Changed(PathBuf::from("src/main.rs")));
        b.push(WatchEvent::Changed(PathBuf::from("src/lib.rs")));
        assert_eq!(b.to_index.len(), 2);
        assert!(b.to_remove.is_empty());
    }

    #[test]
    fn test_event_batch_delete_overrides_create() {
        let mut b = EventBatch::new();
        b.push(WatchEvent::Created(PathBuf::from("src/main.rs")));
        b.push(WatchEvent::Deleted(PathBuf::from("src/main.rs")));
        assert!(b.to_index.is_empty());
        assert_eq!(b.to_remove.len(), 1);
    }

    #[test]
    fn test_event_batch_create_after_delete() {
        let mut b = EventBatch::new();
        b.push(WatchEvent::Deleted(PathBuf::from("src/main.rs")));
        b.push(WatchEvent::Created(PathBuf::from("src/main.rs")));
        assert_eq!(b.to_index.len(), 1);
        assert!(b.to_remove.is_empty());
    }

    #[test]
    fn test_event_batch_rename() {
        let mut b = EventBatch::new();
        b.push(WatchEvent::Renamed {
            from: PathBuf::from("old.rs"),
            to: PathBuf::from("new.rs"),
        });
        assert_eq!(b.to_remove.len(), 1);
        assert!(b.to_remove.contains(&PathBuf::from("old.rs")));
        assert_eq!(b.to_index.len(), 1);
        assert!(b.to_index.contains(&PathBuf::from("new.rs")));
    }

    #[test]
    fn test_worker_stats_default() {
        let s = WorkerStats::default();
        assert_eq!(s.files_indexed, 0);
        assert_eq!(s.files_removed, 0);
        assert_eq!(s.batches_processed, 0);
        assert!(!s.is_busy);
    }

    #[test]
    fn test_worker_start_stop() {
        let dir = tempfile::tempdir().unwrap();
        let index = make_test_index(dir.path());
        let (_tx, rx) = std_mpsc::channel();

        let mut handle = IndexWorker::start(index, rx, WorkerConfig::default());
        assert!(!handle.is_stopped());

        handle.stop();
        assert!(handle.is_stopped());
    }

    #[test]
    fn test_worker_processes_events() {
        let dir = tempfile::tempdir().unwrap();
        // Create a source file for the worker to index.
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();
        std::fs::write(src_dir.join("main.rs"), "fn main() {}").unwrap();

        let index = make_test_index(dir.path());
        let (tx, rx) = std_mpsc::channel();

        let config = WorkerConfig {
            debounce_ms: 100,
            batch_size: 50,
            max_queue_size: 1000,
        };

        let mut handle = IndexWorker::start(Arc::clone(&index), rx, config);

        // Send a change event.
        tx.send(WatchEvent::Changed(PathBuf::from("src/main.rs")))
            .unwrap();

        // Wait for the worker to process it.
        std::thread::sleep(Duration::from_millis(500));

        let stats = handle.stats();
        // The file should have been indexed.
        assert!(
            stats.batches_processed >= 1,
            "expected at least 1 batch, got {}",
            stats.batches_processed
        );

        handle.stop();
    }

    #[test]
    fn test_worker_channel_disconnect_stops() {
        let dir = tempfile::tempdir().unwrap();
        let index = make_test_index(dir.path());
        let (tx, rx) = std_mpsc::channel();

        let mut handle = IndexWorker::start(index, rx, WorkerConfig::default());

        // Drop the sender — should cause worker to exit.
        drop(tx);

        // Give the worker time to notice.
        std::thread::sleep(Duration::from_millis(200));
        handle.stop();
    }
}
