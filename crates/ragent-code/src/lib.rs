//! # ragent-code
//!
//! Codebase indexing and structured search for the ragent AI assistant.
//!
//! This crate provides:
//! - **File scanning** — gitignore-aware directory walking with content hashing
//! - **Symbol extraction** — tree-sitter–based parsing of source code into structured symbols
//! - **Index storage** — SQLite-backed persistent store for files, symbols, and references
//! - **Full-text search** — tantivy-backed full-text search over symbols and documentation
//! - **Background indexing** — file watcher with debounced, batched re-indexing
//!
//! ## Quick Start
//!
//! ```no_run
//! use ragent_code::CodeIndex;
//! use ragent_code::types::{CodeIndexConfig, SearchQuery};
//!
//! let config = CodeIndexConfig::default();
//! let idx = CodeIndex::open(&config).unwrap();
//! let result = idx.full_reindex().unwrap();
//! println!("{result}");
//!
//! let hits = idx.search(&SearchQuery::new("parse_config")).unwrap();
//! for hit in &hits {
//!     println!("{hit}");
//! }
//! ```
//!
//! ## Modules
//!
//! - [`types`] — Core data types: `SymbolKind`, `FileEntry`, `Symbol`, etc.
//! - [`scanner`] — File discovery, hashing, and language detection
//! - [`store`] — SQLite index storage with incremental update support
//! - [`parser`] — Tree-sitter parsing and symbol extraction
//! - [`search`] — Full-text search index backed by tantivy
//! - [`watcher`] — Filesystem event watcher
//! - [`worker`] — Background indexing worker with debounce and batching

/// Core data types shared across the indexing pipeline.
pub mod types;

/// File scanning, content hashing, and language detection.
pub mod scanner;

/// SQLite-backed index storage for files and symbols.
pub mod store;

/// Tree-sitter–based source code parsing and symbol extraction.
pub mod parser;

/// Full-text search index backed by tantivy.
pub mod search;

/// Filesystem event watcher for real-time index updates.
pub mod watcher;

/// Background indexing worker with debounce, dedup, and batching.
pub mod worker;

/// LRU tree cache for incremental tree-sitter parsing.
pub mod tree_cache;

use anyhow::{Context, Result};
use parser::ParserRegistry;
use search::{FtsIndex, FtsSymbol, SearchResult};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use store::IndexStore;
use tracing::{debug, warn};
use tree_cache::TreeCache;
use types::{
    CodeIndexConfig, DepDirection, FileEntry, IndexResult, IndexStats, ScannedFile, SearchQuery,
    Symbol, SymbolFilter, SymbolRef,
};

/// The main entry point for the code index system.
///
/// Owns the SQLite store, tantivy FTS index, tree cache, and parser registry.
/// Thread-safe via internal `Mutex` guards.
pub struct CodeIndex {
    store: Mutex<IndexStore>,
    fts: Mutex<FtsIndex>,
    tree_cache: Mutex<TreeCache>,
    parsers: ParserRegistry,
    project_root: PathBuf,
    config: CodeIndexConfig,
}

impl CodeIndex {
    /// Open (or create) a code index for the given configuration.
    pub fn open(config: &CodeIndexConfig) -> Result<Self> {
        let db_path = config.index_dir.join("index.db");
        let fts_path = config.index_dir.join("fts");

        let store = IndexStore::open(&db_path)
            .with_context(|| format!("cannot open index store: {}", db_path.display()))?;
        let fts = FtsIndex::open(&fts_path)
            .with_context(|| format!("cannot open FTS index: {}", fts_path.display()))?;
        let parsers = ParserRegistry::new();

        Ok(Self {
            store: Mutex::new(store),
            fts: Mutex::new(fts),
            tree_cache: Mutex::new(TreeCache::with_default_capacity()),
            parsers,
            project_root: config.project_root.clone(),
            config: config.clone(),
        })
    }

    /// Open an in-memory code index (for testing).
    pub fn open_in_memory(config: &CodeIndexConfig) -> Result<Self> {
        let store = IndexStore::open_in_memory()?;
        let fts = FtsIndex::open_in_memory()?;
        let parsers = ParserRegistry::new();

        Ok(Self {
            store: Mutex::new(store),
            fts: Mutex::new(fts),
            tree_cache: Mutex::new(TreeCache::with_default_capacity()),
            parsers,
            project_root: config.project_root.clone(),
            config: config.clone(),
        })
    }

    /// Access the FTS index directly (for testing only).
    #[doc(hidden)]
    pub fn fts_for_test(&self) -> std::sync::MutexGuard<'_, FtsIndex> {
        self.fts.lock().unwrap()
    }

    // ── Query Methods ───────────────────────────────────────────────────

    /// Search the index using full-text search combined with structured filters.
    pub fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        let limit = if query.max_results == 0 {
            20
        } else {
            query.max_results
        };

        let fts = self.fts.lock().unwrap();
        debug!(
            query = %query.query,
            kind = ?query.kind,
            language = ?query.language,
            file_pattern = ?query.file_pattern,
            limit = limit,
            "CodeIndex search"
        );
        let mut results = fts.search(&query.query, limit * 2)?;
        debug!(raw_results = results.len(), "CodeIndex FTS results before filtering");

        // Apply post-FTS filters.
        if let Some(ref kind) = query.kind {
            let kind_str = kind.to_string();
            results.retain(|r| r.kind == kind_str);
        }
        if let Some(ref lang) = query.language {
            let store = self.store.lock().unwrap();
            results.retain(|r| {
                store
                    .get_file(&r.file_path)
                    .ok()
                    .flatten()
                    .and_then(|f| f.language)
                    .as_deref()
                    == Some(lang.as_str())
            });
        }
        if let Some(ref pattern) = query.file_pattern {
            results.retain(|r| r.file_path.contains(pattern.as_str()));
        }

        results.truncate(limit);
        Ok(results)
    }

    /// Query symbols from the structured SQLite index.
    pub fn symbols(&self, filter: &SymbolFilter) -> Result<Vec<Symbol>> {
        let store = self.store.lock().unwrap();
        store.query_symbols(filter)
    }

    /// Find all references to a symbol by name.
    pub fn references(&self, symbol_name: &str, limit: usize) -> Result<Vec<SymbolRef>> {
        let store = self.store.lock().unwrap();
        let mut refs = store.find_references(symbol_name)?;
        if limit > 0 {
            refs.truncate(limit);
        }
        Ok(refs)
    }

    /// Get file dependencies in the given direction.
    pub fn dependencies(&self, path: &str, direction: DepDirection) -> Result<Vec<String>> {
        let store = self.store.lock().unwrap();
        match direction {
            DepDirection::Imports => {
                let file_id = store
                    .get_file_id(path)?
                    .with_context(|| format!("file not indexed: {path}"))?;
                let imports = store.get_file_imports(file_id)?;
                Ok(imports.into_iter().map(|i| i.source_module).collect())
            }
            DepDirection::Dependents => {
                let dep_ids = store.get_dependents(path)?;
                let files = store.list_files()?;
                Ok(dep_ids
                    .into_iter()
                    .filter_map(|id| {
                        files
                            .iter()
                            .find(|f| store.get_file_id(&f.path).ok().flatten() == Some(id))
                    })
                    .map(|f| f.path.clone())
                    .collect())
            }
        }
    }

    /// Get index status and statistics.
    pub fn status(&self) -> Result<IndexStats> {
        let store = self.store.lock().unwrap();
        let mut stats = store.get_stats()?;

        // FTS doc count from tantivy.
        let fts = self.fts.lock().unwrap();
        stats.fts_doc_count = fts.doc_count().unwrap_or(0);
        drop(fts);

        // Calculate on-disk index size if using a real directory.
        if self.config.index_dir.exists() {
            stats.index_size_bytes = dir_size(&self.config.index_dir);
        }

        Ok(stats)
    }

    /// Ensure FTS index is in sync with the SQLite store.
    ///
    /// Detects when the FTS index is empty or significantly diverged from
    /// SQLite (e.g., after schema recreation, corruption, or accumulated
    /// duplicates from multiple reindexes) and rebuilds it from SQLite data.
    pub fn ensure_fts_sync(&self) -> Result<()> {
        let store = self.store.lock().unwrap();
        let sqlite_symbols = store.symbol_count()?;
        drop(store);

        let fts = self.fts.lock().unwrap();
        let fts_docs = fts.doc_count().unwrap_or(0);
        drop(fts);

        if sqlite_symbols == 0 {
            debug!("FTS sync: SQLite has no symbols, nothing to sync");
            return Ok(());
        }

        if fts_docs == 0 {
            debug!(
                sqlite_symbols = sqlite_symbols,
                "FTS empty but SQLite has symbols; rebuilding FTS"
            );
            return self.rebuild_fts();
        }

        // Detect significant divergence (accumulated duplicates or missing docs).
        let ratio = fts_docs as f64 / sqlite_symbols as f64;
        if ratio > 2.0 || ratio < 0.5 {
            debug!(
                fts_docs = fts_docs,
                sqlite_symbols = sqlite_symbols,
                ratio = format!("{ratio:.1}"),
                "FTS/SQLite divergence detected; rebuilding FTS"
            );
            return self.rebuild_fts();
        }

        debug!(
            fts_docs = fts_docs,
            sqlite_symbols = sqlite_symbols,
            "FTS in sync with SQLite"
        );
        Ok(())
    }

    // ── Mutation Methods ────────────────────────────────────────────────

    /// Index a single file: scan, parse, and store.
    ///
    /// Uses the tree cache for incremental re-parsing when possible.
    pub fn index_file(&self, path: &Path) -> Result<()> {
        let abs_path = self.project_root.join(path);
        let content = std::fs::read(&abs_path)
            .with_context(|| format!("cannot read {}", abs_path.display()))?;

        let rel_path = path.to_str().with_context(|| "non-UTF-8 path")?.to_string();

        let hash = scanner::hash_content(&content);
        let language = scanner::detect_language(path);
        #[allow(clippy::naive_bytecount)]
        let line_count = content.iter().filter(|&&b| b == b'\n').count() as u64;
        let mtime_ns = std::fs::metadata(&abs_path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos() as i64);

        let entry = FileEntry {
            path: rel_path.clone(),
            content_hash: hash,
            byte_size: content.len() as u64,
            language: language.clone(),
            last_indexed: chrono::Utc::now(),
            mtime_ns,
            line_count,
        };

        let store = self.store.lock().unwrap();
        let file_id = store.upsert_file(&entry)?;

        // Parse if we have a parser for this language.
        if let Some(ref lang) = language {
            if let Some(parsed) = self.parsers.parse(lang, &content) {
                match parsed {
                    Ok(parsed) => {
                        let sym_count = store.upsert_symbols(file_id, &parsed.symbols)?;
                        store.upsert_imports(file_id, &parsed.imports)?;
                        store.upsert_refs(file_id, &parsed.references)?;
                        debug!("indexed {rel_path}: {sym_count} symbols");

                        // Update FTS.
                        let fts = self.fts.lock().unwrap();
                        fts.remove_file(&rel_path)?;
                        let fts_syms: Vec<FtsSymbol<'_>> = parsed
                            .symbols
                            .iter()
                            .map(|s| symbol_to_fts(s, &rel_path))
                            .collect();
                        fts.add_symbols(&fts_syms)?;
                        drop(fts);

                        // Update tree cache with the new parse tree.
                        if let Some(tree) = parsed.tree {
                            let mut tc = self.tree_cache.lock().unwrap();
                            tc.put(path.to_path_buf(), tree);
                        }
                    }
                    Err(e) => {
                        warn!("parse error for {rel_path}: {e}");
                    }
                }
            }
        }

        Ok(())
    }

    /// Index multiple files in batch. Returns a summary.
    pub fn index_files(&self, paths: &[&Path]) -> Result<IndexResult> {
        let start = Instant::now();
        let mut result = IndexResult::default();

        for path in paths {
            match self.index_file(path) {
                Ok(()) => result.files_added += 1,
                Err(e) => warn!("failed to index {}: {e}", path.display()),
            }
        }

        // Count total symbols after batch.
        let store = self.store.lock().unwrap();
        result.symbols_extracted = store.symbol_count()? as usize;
        result.elapsed_ms = start.elapsed().as_millis() as u64;

        Ok(result)
    }

    /// Perform a full re-index: scan the project, diff against stored state,
    /// and update changed files.
    pub fn full_reindex(&self) -> Result<IndexResult> {
        let start = Instant::now();

        // Scan the project directory.
        let scanned = scanner::scan_directory(&self.project_root, &self.config.scan_config)?;
        debug!("scanned {} files", scanned.len());

        // Compute diff against current index.
        let diff = {
            let store = self.store.lock().unwrap();
            store.get_stale_files(&scanned)?
        };

        let mut result = IndexResult {
            files_added: diff.to_add.len(),
            files_updated: diff.to_update.len(),
            files_removed: diff.to_remove.len(),
            ..Default::default()
        };

        // Apply the diff to the SQLite store.
        {
            let store = self.store.lock().unwrap();
            store.apply_diff(&diff)?;
        }

        // Parse and store symbols for new/updated files.
        // Process in chunks with brief yields between them so that other
        // threads (e.g. the TUI event loop) can acquire the store/FTS locks
        // without starving.
        const CHUNK_SIZE: usize = 10;
        const YIELD_MS: u64 = 5;

        let changed: Vec<&ScannedFile> = diff.to_add.iter().chain(diff.to_update.iter()).collect();
        for (i, sf) in changed.iter().enumerate() {
            let abs_path = self.project_root.join(&sf.path);
            let content = match std::fs::read(&abs_path) {
                Ok(c) => c,
                Err(e) => {
                    warn!("cannot read {}: {e}", abs_path.display());
                    continue;
                }
            };

            let rel_path = sf.path.to_string_lossy().to_string();

            if let Some(ref lang) = sf.language {
                // Parse outside of any lock — this is the CPU-heavy part.
                if let Some(parsed) = self.parsers.parse(lang, &content) {
                    match parsed {
                        Ok(parsed) => {
                            let store = self.store.lock().unwrap();
                            if let Some(file_id) = store.get_file_id(&rel_path)? {
                                let count = store.upsert_symbols(file_id, &parsed.symbols)?;
                                store.upsert_imports(file_id, &parsed.imports)?;
                                store.upsert_refs(file_id, &parsed.references)?;
                                result.symbols_extracted += count;
                            }
                            drop(store);

                            // Update FTS index.
                            let fts = self.fts.lock().unwrap();
                            fts.remove_file(&rel_path)?;
                            let fts_syms: Vec<FtsSymbol<'_>> = parsed
                                .symbols
                                .iter()
                                .map(|s| symbol_to_fts(s, &rel_path))
                                .collect();
                            fts.add_symbols(&fts_syms)?;
                        }
                        Err(e) => {
                            warn!("parse error for {rel_path}: {e}");
                        }
                    }
                }
            }

            // Yield after every chunk to avoid starving other threads.
            if (i + 1) % CHUNK_SIZE == 0 {
                std::thread::sleep(Duration::from_millis(YIELD_MS));
            }
        }

        // Remove deleted files from FTS.
        {
            let fts = self.fts.lock().unwrap();
            for path in &diff.to_remove {
                fts.remove_file(path)?;
            }
        }

        // After incremental update, ensure FTS is in sync with SQLite.
        if let Err(e) = self.ensure_fts_sync() {
            warn!("FTS sync check failed after reindex: {e}");
        }

        result.elapsed_ms = start.elapsed().as_millis() as u64;
        debug!("{result}");

        Ok(result)
    }

    /// Rebuild the FTS index from SQLite symbol data.
    ///
    /// Clears all FTS documents and re-populates from the SQLite symbol store.
    /// Use this to recover from FTS/SQLite mismatches.
    pub fn rebuild_fts(&self) -> Result<()> {
        let fts = self.fts.lock().unwrap();
        fts.clear()?;

        let store = self.store.lock().unwrap();
        let files = store.list_files()?;

        for file in &files {
            if let Some(file_id) = store.get_file_id(&file.path)? {
                let symbols = store.get_file_symbols(file_id)?;
                if !symbols.is_empty() {
                    let fts_syms: Vec<FtsSymbol<'_>> = symbols
                        .iter()
                        .map(|s| symbol_to_fts(s, &file.path))
                        .collect();
                    fts.add_symbols(&fts_syms)?;
                }
            }
        }

        debug!(
            "FTS rebuilt: {} docs from {} files",
            fts.doc_count().unwrap_or(0),
            files.len()
        );
        Ok(())
    }

    /// Remove a file from the index.
    pub fn remove_file(&self, path: &Path) -> Result<()> {
        let path_str = path.to_string_lossy();
        let store = self.store.lock().unwrap();
        store.delete_file(&path_str)?;
        drop(store);

        let fts = self.fts.lock().unwrap();
        fts.remove_file(&path_str)?;
        drop(fts);

        // Remove from tree cache.
        let mut tc = self.tree_cache.lock().unwrap();
        tc.remove(path);

        Ok(())
    }

    /// The project root this index is bound to.
    pub fn project_root(&self) -> &Path {
        &self.project_root
    }
}

// ── Watch Session ──────────────────────────────────────────────────────────

/// An active file-watching session that automatically re-indexes files when
/// they change on disk.
///
/// Created by [`start_watching`]. Dropping the session stops the watcher and
/// worker. You can also call [`WatchSession::stop`] explicitly.
pub struct WatchSession {
    worker_handle: worker::IndexWorkerHandle,
    _watcher: watcher::CodeWatcher,
}

impl WatchSession {
    /// Stop watching and wait for the worker to finish.
    pub fn stop(&mut self) {
        self.worker_handle.stop();
    }

    /// Get current worker statistics.
    pub fn stats(&self) -> worker::WorkerStats {
        self.worker_handle.stats()
    }

    /// Whether the session has been stopped.
    pub fn is_stopped(&self) -> bool {
        self.worker_handle.is_stopped()
    }

    /// Manually queue a single file for re-indexing.
    pub fn queue_reindex(&self, path: PathBuf) {
        self.worker_handle.queue_reindex(path);
    }

    /// Manually trigger a full reindex.
    pub fn queue_full_reindex(&self) {
        self.worker_handle.queue_full_reindex();
    }
}

/// Start watching a project directory for changes and automatically re-index.
///
/// The `CodeIndex` must be wrapped in `Arc<Mutex<>>` since the background
/// worker needs shared ownership.
///
/// Returns a [`WatchSession`] that must be kept alive for watching to continue.
/// Dropping it stops the watcher and worker.
///
/// On start, performs an initial diff scan and queues changed files.
pub fn start_watching(
    index: std::sync::Arc<Mutex<CodeIndex>>,
    config: worker::WorkerConfig,
) -> Result<WatchSession> {
    let project_root = {
        let idx = index.lock().unwrap();
        idx.project_root.clone()
    };

    let (tx, rx) = std::sync::mpsc::channel();

    let watcher =
        watcher::CodeWatcher::new(&project_root, tx).context("cannot start file watcher")?;

    let worker_handle = worker::IndexWorker::start(std::sync::Arc::clone(&index), rx, config);

    // Perform initial diff scan to catch changes that happened while not watching.
    {
        let idx = index.lock().unwrap();
        match idx.full_reindex() {
            Ok(result) => debug!("initial reindex on watch start: {result}"),
            Err(e) => warn!("initial reindex failed: {e}"),
        }
    }

    Ok(WatchSession {
        worker_handle,
        _watcher: watcher,
    })
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Convert a `Symbol` into an `FtsSymbol` for tantivy indexing.
fn symbol_to_fts<'a>(sym: &'a Symbol, file_path: &'a str) -> FtsSymbol<'a> {
    FtsSymbol {
        name: &sym.name,
        qualified_name: sym.qualified_name.as_deref(),
        kind: leak_kind_str(sym.kind),
        file_path,
        signature: sym.signature.as_deref(),
        doc_comment: sym.doc_comment.as_deref(),
        body_snippet: None,
        start_line: sym.start_line,
        end_line: sym.end_line,
    }
}

/// Get a static str for a SymbolKind (avoids allocation in FtsSymbol).
fn leak_kind_str(kind: types::SymbolKind) -> &'static str {
    match kind {
        types::SymbolKind::Function => "function",
        types::SymbolKind::Method => "method",
        types::SymbolKind::Struct => "struct",
        types::SymbolKind::Class => "class",
        types::SymbolKind::Enum => "enum",
        types::SymbolKind::EnumVariant => "enum_variant",
        types::SymbolKind::Trait => "trait",
        types::SymbolKind::Interface => "interface",
        types::SymbolKind::Impl => "impl",
        types::SymbolKind::Module => "module",
        types::SymbolKind::Constant => "constant",
        types::SymbolKind::Static => "static",
        types::SymbolKind::TypeAlias => "type_alias",
        types::SymbolKind::Field => "field",
        types::SymbolKind::Import => "import",
        types::SymbolKind::Macro => "macro",
        types::SymbolKind::Test => "test",
        types::SymbolKind::Unknown => "unknown",
    }
}

/// Recursively compute the size of a directory in bytes.
fn dir_size(path: &Path) -> u64 {
    let mut size = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            if meta.is_file() {
                size += meta.len();
            } else if meta.is_dir() {
                size += dir_size(&entry.path());
            }
        }
    }
    size
}
