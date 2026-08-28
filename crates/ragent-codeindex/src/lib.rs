//! # ragent-codeindex
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
//! use ragent_codeindex::CodeIndex;
//! use ragent_codeindex::types::{CodeIndexConfig, SearchQuery};
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
//! - [`store`] — `SQLite` index storage with incremental update support
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

/// Semantic code graph: typed edges, community detection, and traversal.
pub mod graph;

use anyhow::{Context, Result};
use parser::ParserRegistry;
use search::{FtsIndex, FtsSymbol, SearchResult};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};
use store::IndexStore;
use tracing::{debug, warn};
use tree_cache::TreeCache;
use types::{
    CodeIndexConfig, DepDirection, FileEntry, GraphStatus, IndexResult, IndexStats, ScannedFile,
    SearchQuery, Symbol, SymbolFilter, SymbolRef,
};
/// The main entry point for the code index system.
///
/// Owns the `SQLite` store, tantivy FTS index, tree cache, and parser registry.
/// Thread-safe via internal `Mutex` guards.
pub struct CodeIndex {
    store: Mutex<IndexStore>,
    fts: Mutex<FtsIndex>,
    tree_cache: Mutex<TreeCache>,
    parsers: ParserRegistry,
    project_root: PathBuf,
    config: CodeIndexConfig,
    /// Total files to process in the current reindex (0 when idle).
    reindex_total: AtomicU32,
    /// Files processed so far in the current reindex.
    reindex_done: AtomicU32,
    /// M-029: cached total on-disk size of `index_dir`. Recomputing this on
    /// every `status()` poll rescanned the whole tree; the cache is refreshed
    /// only on a full reindex (where the size meaningfully changes).
    cached_index_size: std::sync::Mutex<Option<u64>>,
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
            reindex_total: AtomicU32::new(0),
            reindex_done: AtomicU32::new(0),
            cached_index_size: std::sync::Mutex::new(None),
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
            reindex_total: AtomicU32::new(0),
            reindex_done: AtomicU32::new(0),
            cached_index_size: std::sync::Mutex::new(None),
        })
    }

    /// Access the FTS index directly (for testing only).
    #[doc(hidden)]
    pub fn fts_for_test(&self) -> std::sync::MutexGuard<'_, FtsIndex> {
        self.fts_guard()
    }

    /// Try to acquire the store mutex (for testing concurrency behaviour).
    #[doc(hidden)]
    pub fn try_lock_store_for_test(&self) -> Option<std::sync::MutexGuard<'_, IndexStore>> {
        self.store.try_lock().ok()
    }

    /// Try to acquire the FTS mutex (for testing concurrency behaviour).
    #[doc(hidden)]
    pub fn try_lock_fts_for_test(&self) -> Option<std::sync::MutexGuard<'_, FtsIndex>> {
        self.fts.try_lock().ok()
    }

    /// Lock the store, recovering a poisoned guard so a panicking thread
    /// during indexing cannot cascade panics into every subsequent user call.
    fn store_guard(&self) -> std::sync::MutexGuard<'_, IndexStore> {
        self.store
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Lock the FTS index, recovering a poisoned guard (see [`Self::store_guard`]).
    fn fts_guard(&self) -> std::sync::MutexGuard<'_, FtsIndex> {
        self.fts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Lock the tree cache, recovering a poisoned guard (see [`Self::store_guard`]).
    fn tree_cache_guard(&self) -> std::sync::MutexGuard<'_, TreeCache> {
        self.tree_cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    // ── Query Methods ───────────────────────────────────────────────────

    /// Search the index using full-text search combined with structured filters.
    pub fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        let limit = if query.max_results == 0 {
            20
        } else {
            query.max_results
        };

        // Lock order: FTS first, then (only when needed for the language
        // filter) the SQLite store. Holding both locks in the opposite order
        // (store then FTS) is what writers do, so acquiring FTS first and
        // dropping it before locking store prevents deadlocks between readers
        // and the background indexing worker.
        let mut results = {
            let fts = self.fts_guard();
            debug!(
                query = %query.query,
                kind = ?query.kind,
                language = ?query.language,
                file_pattern = ?query.file_pattern,
                limit = limit,
                "CodeIndex search"
            );
            let results = fts.search(&query.query, limit * 2)?;
            debug!(
                raw_results = results.len(),
                "CodeIndex FTS results before filtering"
            );
            results
        };

        // Apply post-FTS filters.
        if let Some(ref kind) = query.kind {
            let kind_str = kind.to_string();
            results.retain(|r| r.kind == kind_str);
        }
        if let Some(ref lang) = query.language {
            let store = self.store_guard();
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

    /// Non-blocking variant of [`search()`].
    ///
    /// Returns `None` if either the FTS or the `SQLite` store lock is currently
    /// held by a background re-index. Callers should retry briefly or fall
    /// back to a simpler search tool rather than blocking the agent loop.
    pub fn try_search(&self, query: &SearchQuery) -> Result<Option<Vec<SearchResult>>> {
        let limit = if query.max_results == 0 {
            20
        } else {
            query.max_results
        };

        let fts = match self.fts.try_lock() {
            Ok(g) => g,
            Err(_) => return Ok(None),
        };
        let mut results = fts.search(&query.query, limit * 2)?;
        drop(fts);

        if let Some(ref kind) = query.kind {
            let kind_str = kind.to_string();
            results.retain(|r| r.kind == kind_str);
        }
        if let Some(ref lang) = query.language {
            let store = match self.store.try_lock() {
                Ok(g) => g,
                Err(_) => return Ok(None),
            };
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
        Ok(Some(results))
    }

    /// Query symbols from the structured `SQLite` index.
    pub fn symbols(&self, filter: &SymbolFilter) -> Result<Vec<Symbol>> {
        let store = self.store_guard();
        store.query_symbols(filter)
    }

    /// Non-blocking variant of [`symbols()`].
    ///
    /// Returns `None` if the `SQLite` store lock is currently held (e.g. by a
    /// background reindex). Callers should retry briefly or fall back to
    /// `grep` rather than blocking the agent loop.
    pub fn try_symbols(&self, filter: &SymbolFilter) -> Result<Option<Vec<Symbol>>> {
        let store = match self.store.try_lock() {
            Ok(g) => g,
            Err(_) => return Ok(None),
        };
        Ok(Some(store.query_symbols(filter)?))
    }

    /// Find all references to a symbol by name.
    pub fn references(&self, symbol_name: &str, limit: usize) -> Result<Vec<SymbolRef>> {
        let store = self.store_guard();
        let mut refs = store.find_references(symbol_name)?;
        if limit > 0 {
            refs.truncate(limit);
        }
        Ok(refs)
    }

    /// Non-blocking variant of [`references()`].
    ///
    /// Returns `None` if the `SQLite` store lock is currently held.
    pub fn try_references(
        &self,
        symbol_name: &str,
        limit: usize,
    ) -> Result<Option<Vec<SymbolRef>>> {
        let store = match self.store.try_lock() {
            Ok(g) => g,
            Err(_) => return Ok(None),
        };
        let mut refs = store.find_references(symbol_name)?;
        if limit > 0 {
            refs.truncate(limit);
        }
        Ok(Some(refs))
    }

    /// Get file dependencies in the given direction.
    pub fn dependencies(&self, path: &str, direction: DepDirection) -> Result<Vec<String>> {
        let store = self.store_guard();
        match direction {
            DepDirection::Imports => {
                let file_id = store
                    .get_file_id(path)?
                    .with_context(|| format!("file not indexed: {path}"))?;
                let imports = store.get_file_imports(file_id)?;
                Ok(imports.into_iter().map(|i| i.source_module).collect())
            }
            DepDirection::Dependents => {
                // H-004: resolve dependent file IDs to paths with a single
                // `id → path` map (one query) instead of a fresh `get_file_id`
                // query per dependent (which was O(D) queries per call).
                let id_to_path: std::collections::HashMap<i64, String> =
                    store.list_files_with_ids()?.into_iter().collect();
                let dep_ids = store.get_dependents(path)?;
                Ok(dep_ids
                    .into_iter()
                    .filter_map(|id| id_to_path.get(&id).cloned())
                    .collect())
            }
        }
    }

    /// Non-blocking variant of [`dependencies()`].
    ///
    /// Returns `None` if the `SQLite` store lock is currently held.
    pub fn try_dependencies(
        &self,
        path: &str,
        direction: DepDirection,
    ) -> Result<Option<Vec<String>>> {
        let store = match self.store.try_lock() {
            Ok(g) => g,
            Err(_) => return Ok(None),
        };
        let deps = match direction {
            DepDirection::Imports => {
                let file_id = store
                    .get_file_id(path)?
                    .with_context(|| format!("file not indexed: {path}"))?;
                let imports = store.get_file_imports(file_id)?;
                imports.into_iter().map(|i| i.source_module).collect()
            }
            DepDirection::Dependents => {
                // H-004: single `id → path` map (one query) instead of a
                // per-dependent `get_file_id` query.
                let id_to_path: std::collections::HashMap<i64, String> =
                    store.list_files_with_ids()?.into_iter().collect();
                let dep_ids = store.get_dependents(path)?;
                dep_ids
                    .into_iter()
                    .filter_map(|id| id_to_path.get(&id).cloned())
                    .collect()
            }
        };
        Ok(Some(deps))
    }

    /// Get index status and statistics.
    pub fn status(&self) -> Result<IndexStats> {
        let store = self.store_guard();
        let mut stats = store.get_stats()?;

        // FTS doc count from tantivy.
        let fts = self.fts_guard();
        stats.fts_doc_count = fts.doc_count().unwrap_or(0);
        drop(fts);

        // Calculate on-disk index size if using a real directory. M-029:
        // served from a cache (refreshed only on full reindex) so a status
        // poll does not rescan the whole tree every time.
        if self.config.index_dir.exists() {
            stats.index_size_bytes = self.index_size_cached();
        }

        Ok(stats)
    }

    /// Non-blocking variant of [`status()`].
    ///
    /// Returns `None` if the store or FTS lock is currently held (e.g. by
    /// a background reindex).  This is intended for UI status-bar polling
    /// so it never stalls the render loop.
    pub fn try_status(&self) -> Option<IndexStats> {
        let store = match self.store.try_lock() {
            Ok(s) => s,
            Err(_) => return None,
        };
        let mut stats = store.get_stats().ok()?;
        drop(store);

        if let Ok(fts) = self.fts.try_lock() {
            stats.fts_doc_count = fts.doc_count().unwrap_or(0);
        }

        if self.config.index_dir.exists() {
            stats.index_size_bytes = self.index_size_cached();
        }

        Some(stats)
    }

    /// M-029: return the cached on-disk size of `index_dir`, computing it on
    /// the first call and on full reindex.
    fn index_size_cached(&self) -> u64 {
        let mut guard = self
            .cached_index_size
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(size) = *guard {
            return size;
        }
        let size = dir_size(&self.config.index_dir);
        *guard = Some(size);
        size
    }

    /// Invalidate the cached index-size (called after a full reindex so the
    /// next status poll recomputes it).
    fn invalidate_index_size_cache(&self) {
        if let Ok(mut guard) = self.cached_index_size.lock() {
            *guard = None;
        }
    }

    /// List the top-N highest-degree (most-connected) symbols in the graph
    /// (FR-014).
    ///
    /// Computes the degree of each symbol from the `graph_edges` table and
    /// returns the top `n` sorted by degree (descending).  Blocks until the
    /// store lock is acquired.
    pub fn godnodes(&self, n: usize) -> Result<Vec<graph::GodNode>> {
        let store = self.store_guard();
        let graph = graph::SymbolGraph::new(&store);
        graph.godnodes(n)
    }

    /// Non-blocking variant of [`godnodes()`] (FR-017).
    ///
    /// Returns `None` if the `SQLite` store lock is currently held (e.g. by
    /// a background reindex).  Callers should retry briefly or return a
    /// `codeindex_busy` response rather than stalling the agent loop.
    pub fn try_godnodes(&self, n: usize) -> Result<Option<Vec<graph::GodNode>>> {
        let store = match self.store.try_lock() {
            Ok(g) => g,
            Err(_) => return Ok(None),
        };
        let graph = graph::SymbolGraph::new(&store);
        Ok(Some(graph.godnodes(n)?))
    }

    /// Compute the shortest path (by hop count) between two symbols in the
    /// semantic code graph (FR-012).
    ///
    /// Returns `Ok(None)` if either symbol is not found or no path exists.
    /// Blocks until the store lock is acquired.
    pub fn path(&self, from: &str, to: &str) -> Result<Option<graph::PathResult>> {
        let store = self.store_guard();
        let graph = graph::SymbolGraph::new(&store);
        graph.path(from, to)
    }

    /// Non-blocking variant of [`path()`] (FR-017).
    ///
    /// Returns `Ok(None)` if the `SQLite` store lock is currently held (e.g. by
    /// a background reindex).  Returns `Ok(Some(None))` if the lock was
    /// acquired but no path exists between the two symbols.  Returns
    /// `Ok(Some(Some(result)))` when a path is found.  Callers should retry
    /// briefly on `Ok(None)` or return a `codeindex_busy` response rather than
    /// stalling the agent loop.
    pub fn try_path(&self, from: &str, to: &str) -> Result<Option<Option<graph::PathResult>>> {
        let store = match self.store.try_lock() {
            Ok(g) => g,
            Err(_) => return Ok(None),
        };
        let graph = graph::SymbolGraph::new(&store);
        Ok(Some(graph.path(from, to)?))
    }

    /// Explain a symbol: show its node metadata and connections (FR-011).
    ///
    /// Returns `Ok(None)` if the symbol is not found.  Blocks until the store
    /// lock is acquired.
    pub fn explain(&self, name: &str) -> Result<Option<graph::ExplainResult>> {
        let store = self.store_guard();
        let graph = graph::SymbolGraph::new(&store);
        graph.explain(name)
    }

    /// Non-blocking variant of [`explain()`] (FR-017).
    ///
    /// Returns `Ok(None)` if the `SQLite` store lock is currently held (e.g. by
    /// a background reindex).  Returns `Ok(Some(None))` if the lock was acquired
    /// but the symbol was not found.  Returns `Ok(Some(Some(result)))` when
    /// the symbol is found.
    pub fn try_explain(&self, name: &str) -> Result<Option<Option<graph::ExplainResult>>> {
        let store = match self.store.try_lock() {
            Ok(g) => g,
            Err(_) => return Ok(None),
        };
        let graph = graph::SymbolGraph::new(&store);
        Ok(Some(graph.explain(name)?))
    }

    /// Run community detection over the symbol graph (FR-013).
    ///
    /// Runs label propagation over the `graph_edges` table, persists the
    /// community assignments to the `communities` table, and returns a
    /// summary of detected communities with auto-generated labels and member
    /// counts.  Blocks until the store lock is acquired.
    pub fn communities(&self) -> Result<Vec<graph::CommunityInfo>> {
        let store = self.store_guard();
        let graph = graph::SymbolGraph::new(&store);
        graph.communities()
    }

    /// Non-blocking variant of [`communities()`] (FR-017).
    ///
    /// Returns `Ok(None)` if the `SQLite` store lock is currently held (e.g. by
    /// a background reindex).  Returns `Ok(Some(vec))` when the lock was
    /// acquired and community detection completed (the vec may be empty if
    /// the graph has no edges).
    pub fn try_communities(&self) -> Result<Option<Vec<graph::CommunityInfo>>> {
        let store = match self.store.try_lock() {
            Ok(g) => g,
            Err(_) => return Ok(None),
        };
        let graph = graph::SymbolGraph::new(&store);
        Ok(Some(graph.communities()?))
    }

    /// Build (or rebuild) the semantic edge graph from the currently indexed
    /// symbols (FR-009).
    ///
    /// Derives typed edges from the indexed symbols, imports, and references,
    /// then persists them in the `graph_edges` table.  Returns a
    /// [`graph::BuildResult`] with edge counts distinguishing `EXTRACTED` from
    /// `INFERRED`.  Blocks until the store lock is acquired.
    pub fn build_graph(&self) -> Result<graph::BuildResult> {
        let store = self.store_guard();
        graph::SymbolGraph::new(&store).build()
    }

    /// Build (or rebuild) the semantic edge graph restricted to symbols from a
    /// single language (FR-018).
    ///
    /// Like [`build_graph()`] but only derives edges for files whose detected
    /// language matches `language`.  Useful for per-language subgraph analysis.
    pub fn build_graph_for_language(&self, language: &str) -> Result<graph::BuildResult> {
        let store = self.store_guard();
        graph::SymbolGraph::new(&store).build_for_language(language)
    }

    /// Return the total number of edges in the `graph_edges` table.
    ///
    /// Used by the TUI empty-graph guard (FR-015) to check whether the graph
    /// has been built before running graph query sub-commands.
    pub fn graph_edge_count(&self) -> Result<u64> {
        let store = self.store_guard();
        store.edge_count()
    }

    /// Return summary statistics for the semantic edge graph.
    ///
    /// Aggregates edge/node/community counts so the TUI `/codeindex status`
    /// command can report on the graph data set alongside the index stats.
    /// Blocks until the store lock is acquired.
    pub fn graph_status(&self) -> Result<GraphStatus> {
        let store = self.store_guard();
        Ok(GraphStatus {
            total_edges: store.edge_count()?,
            edges_extracted: store.edge_count_by_confidence_typed(types::Confidence::Extracted)?,
            edges_inferred: store.edge_count_by_confidence_typed(types::Confidence::Inferred)?,
            nodes: store.graph_node_count()?,
            edges_calls: store.edge_count_by_kind("calls")?,
            edges_imports: store.edge_count_by_kind("imports")?,
            edges_inherits: store.edge_count_by_kind("inherits")?,
            edges_references: store.edge_count_by_kind("references")?,
            edges_mixes_in: store.edge_count_by_kind("mixes_in")?,
            edges_implements: store.edge_count_by_kind("implements")?,
            communities: store.community_count()?,
        })
    }

    /// Returns `(done, total)` for the current reindex operation.
    ///
    /// Both are 0 when no reindex is running.  Lock-free (atomic reads).
    pub fn reindex_progress(&self) -> (u32, u32) {
        (
            self.reindex_done.load(Ordering::Relaxed),
            self.reindex_total.load(Ordering::Relaxed),
        )
    }

    /// Ensure FTS index is in sync with the `SQLite` store.
    ///
    /// Detects when the FTS index is empty or significantly diverged from
    /// `SQLite` (e.g., after schema recreation, corruption, or accumulated
    /// duplicates from multiple reindexes) and rebuilds it from `SQLite` data.
    pub fn ensure_fts_sync(&self) -> Result<()> {
        let store = self.store_guard();
        let sqlite_symbols = store.symbol_count()?;
        drop(store);

        let fts = self.fts_guard();
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
        if !(0.5..=2.0).contains(&ratio) {
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
    ///
    /// Re-parses the file, updates its symbols/imports/refs in the store,
    /// and updates the semantic edge graph for symbols in this file (FR-008).
    pub fn index_file(&self, path: &Path) -> Result<()> {
        let start = Instant::now();
        let rel_path = path
            .strip_prefix(&self.project_root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        // Resolve the on-disk path.  `path` may be project-relative or
        // absolute; join against the project root only when it is relative.
        let abs_path = if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.project_root.join(path)
        };

        // ── Read and hash the file ────────────────────────────────────────
        let content = std::fs::read(&abs_path)
            .with_context(|| format!("cannot read file: {}", abs_path.display()))?;
        let hash = scanner::hash_content(&content);
        #[allow(clippy::naive_bytecount)]
        let line_count = content.iter().filter(|&&b| b == b'\n').count() as u64;
        let mtime_ns = std::fs::metadata(&abs_path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos() as i64);

        // Determine the language from the file extension.
        let language = scanner::detect_language(path);
        if language.is_none() {
            debug!("index_file: skipping non-code file: {}", path.display());
            return Ok(());
        }

        // H-007: skip re-parsing/upserting when the stored content hash
        // already matches (the file is unchanged since the last index). This
        // avoids always re-parsing unchanged files on every watcher event.
        {
            let store = self.store_guard();
            if let Some(existing) = store.get_file(&rel_path)?
                && existing.content_hash == hash
            {
                return Ok(());
            }
        }

        // ── Store the file entry ──────────────────────────────────────────
        let file_id = {
            let store = self.store_guard();
            let entry = FileEntry {
                path: rel_path.clone(),
                content_hash: hash,
                byte_size: content.len() as u64,
                language: language.clone(),
                last_indexed: chrono::Utc::now(),
                mtime_ns,
                line_count,
            };
            store.upsert_file(&entry)?
        };

        // ── Collect old symbol IDs before upsert (for edge cleanup) ────
        let old_symbol_ids: Vec<i64> = {
            let store = self.store_guard();
            store
                .get_file_symbols(file_id)?
                .iter()
                .map(|s| s.id)
                .collect()
        };

        // ── Parse and store symbols/imports/refs ─────────────────────────
        if let Some(ref lang) = language
            && let Some(parser) = self.parsers.get(lang)
        {
            match parser.parse(&content) {
                Ok(parsed) => {
                    let store = self.store_guard();
                    store.upsert_symbols(file_id, &parsed.symbols)?;
                    store.upsert_imports(file_id, &parsed.imports)?;
                    store.upsert_refs(file_id, &parsed.references)?;
                    let deps: Vec<(String, String)> = parsed
                        .imports
                        .iter()
                        .map(|imp| (imp.source_module.clone(), imp.kind.clone()))
                        .collect();
                    store.set_file_deps(file_id, &deps)?;
                    drop(store);

                    // Update FTS.
                    let fts = self.fts_guard();
                    let fts_syms: Vec<FtsSymbol<'_>> = parsed
                        .symbols
                        .iter()
                        .map(|s| symbol_to_fts(s, &rel_path))
                        .collect();
                    fts.batch_update(&[rel_path.as_str()], &fts_syms)?;
                }
                Err(e) => {
                    warn!("index_file: parse error in {}: {e}", path.display());
                }
            }
        }

        // ── Update graph edges for this file (FR-008) ──────────────────
        // Delete edges involving this file's symbols, then re-derive
        // edges for those symbols.
        {
            let store = self.store_guard();

            let file_symbols = store.get_file_symbols(file_id)?;
            // Dedup current + stale symbol ids using a HashSet to avoid the
            // O(n) `contains` scan per element (O(n²) overall).
            let mut symbol_ids: Vec<i64> = file_symbols.iter().map(|s| s.id).collect();
            let mut seen: std::collections::HashSet<i64> = symbol_ids.iter().copied().collect();
            for old_id in &old_symbol_ids {
                if seen.insert(*old_id) {
                    symbol_ids.push(*old_id);
                }
            }
            if !symbol_ids.is_empty() {
                store.delete_edges_for_symbols(&symbol_ids)?;
            }
            if let Err(e) = graph::edges::derive_edges_for_file(&store, file_id) {
                warn!(
                    "index_file: graph edge update failed for {}: {e}",
                    path.display()
                );
            }
        }

        debug!(
            "index_file: {} in {}ms",
            path.display(),
            start.elapsed().as_millis()
        );

        Ok(())
    }

    /// Index multiple files in batch. Returns a summary.
    pub fn index_files(&self, paths: &[&Path]) -> Result<IndexResult> {
        let start = Instant::now();
        let mut result = IndexResult::default();

        // Set progress counters so TUI can show indexing indicator.
        let total = paths.len() as u32;
        self.reindex_total.store(total, Ordering::Relaxed);
        self.reindex_done.store(0, Ordering::Relaxed);

        for (i, path) in paths.iter().enumerate() {
            match self.index_file(path) {
                Ok(()) => {
                    result.files_added += 1;
                    self.reindex_done.store(i as u32 + 1, Ordering::Relaxed);
                }
                Err(e) => warn!("failed to index {}: {e}", path.display()),
            }
        }

        // Count total symbols after batch.
        let store = self.store_guard();
        result.symbols_extracted = store.symbol_count()? as usize;
        result.elapsed_ms = start.elapsed().as_millis() as u64;

        // Clear progress counters.
        self.reindex_total.store(0, Ordering::Relaxed);
        self.reindex_done.store(0, Ordering::Relaxed);

        Ok(result)
    }

    /// Perform a full re-index: scan the project, diff against stored state,
    /// and update changed files.
    pub fn full_reindex(&self) -> Result<IndexResult> {
        let start = Instant::now();

        // Scan the project directory.
        let scan_start = Instant::now();
        let scanned = scanner::scan_directory(&self.project_root, &self.config.scan_config)?;
        let scan_ms = scan_start.elapsed().as_millis();
        debug!("scanned {} files ({}ms)", scanned.len(), scan_ms);

        // Compute diff against current index.
        let diff_start = Instant::now();
        let diff = {
            let store = self.store_guard();
            store.get_stale_files(&scanned)?
        };
        let diff_ms = diff_start.elapsed().as_millis();
        debug!(
            "stale diff: {} add, {} update, {} remove ({}ms)",
            diff.to_add.len(),
            diff.to_update.len(),
            diff.to_remove.len(),
            diff_ms
        );

        // Set progress counters early so the TUI can show the percentage
        // even while apply_diff holds the store lock.
        let changed_count = (diff.to_add.len() + diff.to_update.len()) as u32;
        self.reindex_total.store(changed_count, Ordering::Relaxed);
        self.reindex_done.store(0, Ordering::Relaxed);

        let mut result = IndexResult {
            files_added: diff.to_add.len(),
            files_updated: diff.to_update.len(),
            files_removed: diff.to_remove.len(),
            ..Default::default()
        };

        // Apply the diff to the SQLite store.
        let apply_start = Instant::now();
        {
            let store = self.store_guard();
            store.apply_diff(&diff)?;
        }
        let apply_ms = apply_start.elapsed().as_millis();
        debug!("apply_diff done ({}ms)", apply_ms);

        // Parse and store symbols for new/updated files.
        // Process in chunks: parse all files in each chunk outside locks
        // (CPU-heavy), then batch-write to SQLite in a single transaction
        // and FTS in a single commit per chunk. This reduces:
        //   - SQLite disk syncs from N to N/CHUNK_SIZE
        //   - FTS commits from 2N to N/CHUNK_SIZE
        //   - Lock acquisitions from 2N to 2*(N/CHUNK_SIZE)
        // Brief yields between chunks let the TUI event loop acquire locks.
        const CHUNK_SIZE: usize = 20;
        const YIELD_MS: u64 = 1;

        let changed: Vec<&ScannedFile> = diff.to_add.iter().chain(diff.to_update.iter()).collect();

        for chunk in changed.chunks(CHUNK_SIZE) {
            // Phase 1: Parse all files in this chunk with NO locks held.
            let mut parsed_results: Vec<(String, parser::ParsedFile)> = Vec::new();
            for sf in chunk {
                let abs_path = self.project_root.join(&sf.path);
                let content = match std::fs::read(&abs_path) {
                    Ok(c) => c,
                    Err(e) => {
                        warn!("cannot read {}: {e}", abs_path.display());
                        continue;
                    }
                };

                let rel_path = sf.path.to_string_lossy().to_string();

                if let Some(ref lang) = sf.language
                    && let Some(parsed) = self.parsers.parse(lang, &content)
                {
                    match parsed {
                        Ok(parsed) => {
                            parsed_results.push((rel_path, parsed));
                        }
                        Err(e) => {
                            warn!("parse error for {rel_path}: {e}");
                        }
                    }
                }
            }

            // Phase 2: Batch-write to SQLite in a single transaction.
            if !parsed_results.is_empty() {
                let store = self.store_guard();
                store.begin_transaction()?;
                for (rel_path, parsed) in &parsed_results {
                    if let Some(file_id) = store.get_file_id(rel_path)? {
                        let count = store.upsert_symbols(file_id, &parsed.symbols)?;
                        store.upsert_imports(file_id, &parsed.imports)?;
                        store.upsert_refs(file_id, &parsed.references)?;
                        result.symbols_extracted += count;
                    }
                }
                store.commit_transaction()?;
                drop(store);
            }

            // Phase 3: Batch-update FTS with a single writer and commit.
            if !parsed_results.is_empty() {
                let fts = self.fts_guard();
                let remove_paths: Vec<&str> =
                    parsed_results.iter().map(|(p, _)| p.as_str()).collect();
                let fts_syms: Vec<FtsSymbol<'_>> = parsed_results
                    .iter()
                    .flat_map(|(rel_path, parsed)| {
                        parsed
                            .symbols
                            .iter()
                            .map(move |s| symbol_to_fts(s, rel_path))
                    })
                    .collect();
                fts.batch_update(&remove_paths, &fts_syms)?;
                drop(fts);
            }

            // Update progress counter.
            let done_so_far = self.reindex_done.load(Ordering::Relaxed) + chunk.len() as u32;
            self.reindex_done.store(done_so_far, Ordering::Relaxed);

            // Yield between chunks so the TUI event loop can acquire locks.
            std::thread::sleep(Duration::from_millis(YIELD_MS));
        }

        // Remove deleted files from FTS in a single batch.
        if !diff.to_remove.is_empty() {
            let fts = self.fts_guard();
            let remove_paths: Vec<&str> = diff
                .to_remove
                .iter()
                .map(std::string::String::as_str)
                .collect();
            fts.batch_update(&remove_paths, &[])?;
        }

        // After incremental update, ensure FTS is in sync with SQLite.
        let fts_sync_start = Instant::now();
        if let Err(e) = self.ensure_fts_sync() {
            warn!("FTS sync check failed after reindex: {e}");
        }
        let fts_sync_ms = fts_sync_start.elapsed().as_millis();

        // ── Build semantic edge graph (FR-007) ──────────────────────────
        let graph_start = Instant::now();
        let (edges_extracted, edges_inferred) = {
            let store = self.store_guard();
            match graph::edges::derive_and_store(&store) {
                Ok(build_result) => {
                    debug!(
                        "full_reindex: graph built: {} edges ({} EXTRACTED, {} INFERRED) in {}ms",
                        build_result.edges_total,
                        build_result.edges_extracted,
                        build_result.edges_inferred,
                        build_result.elapsed_ms
                    );
                    (build_result.edges_extracted, build_result.edges_inferred)
                }
                Err(e) => {
                    warn!("full_reindex: graph build failed: {e}");
                    (0, 0)
                }
            }
        };
        result.edges_extracted = edges_extracted;
        result.edges_inferred = edges_inferred;
        let graph_ms = graph_start.elapsed().as_millis();

        result.elapsed_ms = start.elapsed().as_millis() as u64;
        debug!(
            "full_reindex: scan={}ms, diff={}ms, apply={}ms, fts_sync={}ms, graph={}ms, total={}ms",
            scan_ms, diff_ms, apply_ms, fts_sync_ms, graph_ms, result.elapsed_ms
        );

        // Clear progress counters.
        self.reindex_total.store(0, Ordering::Relaxed);
        self.reindex_done.store(0, Ordering::Relaxed);
        // M-029: the on-disk index size changed with this reindex, so the next
        // status poll recomputes it.
        self.invalidate_index_size_cache();

        Ok(result)
    }

    /// Rebuild the FTS index from `SQLite` symbol data.
    ///
    /// Clears all FTS documents and re-populates from the `SQLite` symbol store.
    /// Use this to recover from FTS/SQLite mismatches.
    pub fn rebuild_fts(&self) -> Result<()> {
        let fts = self.fts_guard();
        fts.clear()?;

        let store = self.store_guard();
        let files = store.list_files()?;

        // H-006: batch all symbols from all files into a single writer/commit
        // pass instead of calling `add_symbols` (which allocates a fresh
        // 15 MB `IndexWriter` + commits) per file. We first collect every
        // (Symbol, file_path) pair into owned buffers so the borrowed
        // `FtsSymbol` slice can be built once and written in a single commit.
        let mut syms_owned: Vec<(types::Symbol, String)> = Vec::new();
        for file in &files {
            if let Some(file_id) = store.get_file_id(&file.path)? {
                for s in store.get_file_symbols(file_id)? {
                    syms_owned.push((s, file.path.clone()));
                }
            }
        }
        if !syms_owned.is_empty() {
            let fts_syms: Vec<FtsSymbol<'_>> = syms_owned
                .iter()
                .map(|(s, path)| symbol_to_fts(s, path))
                .collect();
            fts.add_symbols(&fts_syms)?;
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
        let store = self.store_guard();
        store.delete_file(&path_str)?;
        drop(store);

        let fts = self.fts_guard();
        fts.remove_file(&path_str)?;
        drop(fts);

        // Remove from tree cache.
        let mut tc = self.tree_cache_guard();
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
    #[must_use]
    pub fn stats(&self) -> worker::WorkerStats {
        self.worker_handle.stats()
    }

    /// Whether the session has been stopped.
    #[must_use]
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
/// The `CodeIndex` is wrapped in `Arc` since the background worker needs
/// shared ownership. `CodeIndex` has internal mutexes for thread safety.
///
/// Returns a [`WatchSession`] that must be kept alive for watching to continue.
/// Dropping it stops the watcher and worker.
///
/// On start, performs an initial diff scan and queues changed files.
pub fn start_watching(
    index: std::sync::Arc<CodeIndex>,
    config: worker::WorkerConfig,
) -> Result<WatchSession> {
    let project_root = index.project_root.clone();

    let (tx, rx) = std::sync::mpsc::channel();

    let watcher =
        watcher::CodeWatcher::new(&project_root, tx).context("cannot start file watcher")?;

    let worker_handle = worker::IndexWorker::start(std::sync::Arc::clone(&index), rx, config);

    // Perform initial diff scan in background to avoid blocking the caller.
    let bg = std::sync::Arc::clone(&index);
    std::thread::Builder::new()
        .name("codeindex-init-reindex".into())
        .spawn(move || match bg.full_reindex() {
            Ok(result) => debug!("initial reindex on watch start: {result}"),
            Err(e) => warn!("initial reindex failed: {e}"),
        })
        .ok();

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

/// Get a static str for a `SymbolKind` (avoids allocation in `FtsSymbol`).
const fn leak_kind_str(kind: types::SymbolKind) -> &'static str {
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
