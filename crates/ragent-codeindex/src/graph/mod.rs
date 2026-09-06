//! Semantic code graph: typed edges between indexed symbols.
//!
//! This module provides [`SymbolGraph`], the public API for building and
//! querying the semantic edge graph.  The graph is derived deterministically
//! from the existing tree-sitter parse output (symbols, imports, references)
//! — no LLM or embeddings are used.
//!
//! ## Overview
//!
//! - [`SymbolGraph::build`] — derive edges from the indexed symbols and
//!   persist them in the `graph_edges` SQLite table.
//! - [`SymbolGraph::explain`] — show a symbol's node metadata and its
//!   incoming/outgoing connections.
//! - [`SymbolGraph::path`] — compute the shortest path (by hop count) between
//!   two symbols.
//! - [`SymbolGraph::communities`] — run community detection over the graph.
//! - [`SymbolGraph::godnodes`] — list the highest-degree (most-connected)
//!   symbols.
//! - [`SymbolGraph::export_json`] / [`SymbolGraph::export_report`] —
//!   serialise the graph to `graph.json` and `GRAPH_REPORT.md`.
//!
//! The submodules will be populated by later tasks:
//! - `edges` — edge derivation logic (T-004)
//! - `resolve` — cross-file symbol resolution (T-005)
//! - `traverse` — BFS shortest-path and explain queries (T-008, T-009)
//! - `communities` — community detection (T-010)
//! - `export` — JSON and Markdown export (T-011)

use crate::store::IndexStore;
use crate::types::{Confidence, GraphEdge};
use anyhow::Result;

// Submodule stubs — populated by later tasks.
pub mod communities;
pub mod edges;
pub mod export;
pub mod resolve;
pub mod traverse;

// ── Explain Result ──────────────────────────────────────────────────────────

/// The result of an `explain` query: a symbol's metadata plus its
/// connections.
#[derive(Debug, Clone)]
pub struct ExplainResult {
    /// The symbol name.
    pub name: String,
    /// The source file path.
    pub source_file: String,
    /// The starting line number.
    pub line: u32,
    /// The community ID, if community detection has been run.
    pub community: Option<i64>,
    /// The degree (number of edges) of this node.
    pub degree: usize,
    /// Incoming edges (other symbols that reference/call this one).
    pub incoming: Vec<Connection>,
    /// Outgoing edges (symbols this one references/calls).
    pub outgoing: Vec<Connection>,
}

/// A single connection in an `explain` result.
#[derive(Debug, Clone)]
pub struct Connection {
    /// The name of the connected symbol.
    pub symbol: String,
    /// The source file of the connected symbol.
    pub source_file: String,
    /// The edge kind (calls, imports, references, etc.).
    pub kind: String,
    /// The confidence tag (EXTRACTED or INFERRED).
    pub confidence: Confidence,
    /// The line number, if known.
    pub line: Option<u32>,
}

// ── Path Result ─────────────────────────────────────────────────────────────

/// The result of a `path` query: the shortest path between two symbols.
#[derive(Debug, Clone)]
pub struct PathResult {
    /// The number of hops in the path.
    pub hops: usize,
    /// The sequence of `(symbol_name, edge_kind)` pairs forming the path.
    /// The first element is the source symbol with no edge kind; subsequent
    /// elements are `(symbol_name, Some(kind))` showing the edge that led to
    /// that symbol.
    pub steps: Vec<(String, Option<String>)>,
}

// ── God Node ──────────────────────────────────────────────────��──────────────

/// A high-degree (hub) symbol.
#[derive(Debug, Clone)]
pub struct GodNode {
    /// The symbol name.
    pub name: String,
    /// The source file path.
    pub source_file: String,
    /// The degree (number of edges).
    pub degree: usize,
}

// ── Community Info ───────────────────────────────────────────────────────────

/// Information about a detected community.
#[derive(Debug, Clone)]
pub struct CommunityInfo {
    /// The community ID.
    pub id: i64,
    /// The auto-generated label, if any.
    pub label: Option<String>,
    /// The number of symbols in this community.
    pub member_count: usize,
}

// ── Symbol Graph ────────────────────────────────────────────────────────────

/// The semantic code graph built from indexed symbols.
///
/// Holds a reference to the [`IndexStore`] and provides methods for building
/// and querying the typed edge graph.  All methods return `Result` and operate
/// on the store's `graph_edges` and `communities` tables.
///
/// The caller is responsible for locking the store; `SymbolGraph` does not
/// acquire any locks itself.
pub struct SymbolGraph<'a> {
    store: &'a IndexStore,
}

impl<'a> SymbolGraph<'a> {
    /// Create a new `SymbolGraph` backed by the given store.
    #[must_use]
    pub const fn new(store: &'a IndexStore) -> Self {
        Self { store }
    }

    // ── Build ────────────────────────────────────────────────────────────

    /// Build (or rebuild) the semantic edge graph.
    ///
    /// Derives typed edges from the indexed symbols, imports, and references,
    /// then persists them in the `graph_edges` table.
    ///
    /// Returns a [`BuildResult`] with edge counts.
    ///
    /// *Full implementation in T-004 / T-005.*
    pub fn build(&self) -> Result<BuildResult> {
        edges::derive_and_store(self.store, None)
    }

    /// Build the graph restricted to symbols from a single language.
    ///
    /// *Full implementation in T-004 (with language filter).*
    pub fn build_for_language(&self, language: &str) -> Result<BuildResult> {
        edges::derive_and_store_for_language(self.store, language)
    }

    // ── Queries ──────────────────────────────────────────────────────────

    /// Explain a symbol: show its node metadata and connections.
    ///
    /// Looks up the symbol by name, retrieves its incoming and outgoing edges
    /// (up to 50 connections), and returns a structured [`ExplainResult`].
    ///
    /// *Full implementation in T-009.*
    pub fn explain(&self, name: &str) -> Result<Option<ExplainResult>> {
        traverse::explain(self.store, name)
    }

    /// Compute the shortest path (by hop count) between two symbols.
    ///
    /// Returns `None` if no path exists.
    ///
    /// *Full implementation in T-008.*
    pub fn path(&self, from: &str, to: &str) -> Result<Option<PathResult>> {
        traverse::shortest_path(self.store, from, to)
    }

    /// Run community detection and list the detected communities with
    /// auto-generated labels and member counts (FR-013, FR-019).
    ///
    /// Runs label propagation over the symbol graph, persists the community
    /// assignments to the `communities` table, and returns a summary.
    pub fn communities(&self) -> Result<Vec<CommunityInfo>> {
        communities::detect_communities(self.store)
    }

    /// List the top-N highest-degree (most-connected) symbols.
    ///
    /// Computes the degree of each symbol from the `graph_edges` table and
    /// returns the top `n` sorted by degree (descending).
    pub fn godnodes(&self, n: usize) -> Result<Vec<GodNode>> {
        let edges = self.store.query_all_edges_typed()?;
        if edges.is_empty() {
            return Ok(Vec::new());
        }

        // Count degree (total incident edges) per symbol ID.
        let mut degree_map: std::collections::HashMap<i64, usize> =
            std::collections::HashMap::new();
        for edge in &edges {
            *degree_map.entry(edge.source_sym).or_default() += 1;
            *degree_map.entry(edge.target_sym).or_default() += 1;
        }

        // Look up symbol names and file paths.
        let all_symbols = self
            .store
            .query_symbols(&crate::types::SymbolFilter::default())?;
        let sym_lookup: std::collections::HashMap<i64, (String, String)> = all_symbols
            .iter()
            .map(|s| {
                let file = self
                    .store
                    .get_file_by_id(s.file_id)
                    .map(|opt| opt.map(|f| f.path).unwrap_or_default())
                    .unwrap_or_default();
                (s.id, (s.name.clone(), file))
            })
            .collect();

        // Build GodNode list sorted by degree.
        let mut nodes: Vec<GodNode> = degree_map
            .into_iter()
            .filter_map(|(sym_id, degree)| {
                sym_lookup.get(&sym_id).map(|(name, file)| GodNode {
                    name: name.clone(),
                    source_file: file.clone(),
                    degree,
                })
            })
            .collect();
        nodes.sort_by(|a, b| b.degree.cmp(&a.degree));
        nodes.truncate(n);
        Ok(nodes)
    }

    // ── Export ───────────────────────────────────────────────────────────

    /// Serialise the graph to a JSON string.
    ///
    /// *Full implementation in T-011.*
    pub fn export_json(&self) -> Result<String> {
        export::to_json(self.store)
    }

    /// Generate a `GRAPH_REPORT.md` report.
    ///
    /// *Full implementation in T-011.*
    pub fn export_report(&self) -> Result<String> {
        export::to_report(self.store)
    }

    // ── Stats ────────────────────────────────────────────────────────────

    /// Return the total number of edges in the graph.
    pub fn edge_count(&self) -> Result<u64> {
        self.store.edge_count()
    }

    /// Return the number of edges filtered by confidence tag.
    pub fn edge_count_by_confidence(&self, confidence: Confidence) -> Result<u64> {
        self.store.edge_count_by_confidence_typed(confidence)
    }

    /// Return all edges in the graph.
    pub fn all_edges(&self) -> Result<Vec<GraphEdge>> {
        self.store.query_all_edges_typed()
    }
}

// ── Build Result ────────────────────────────────────────────────────────────

/// Summary of a graph build operation.
#[derive(Debug, Clone, Default)]
pub struct BuildResult {
    /// Total number of edges created.
    pub edges_total: usize,
    /// Number of edges tagged `EXTRACTED`.
    pub edges_extracted: usize,
    /// Number of edges tagged `INFERRED`.
    pub edges_inferred: usize,
    /// Elapsed time in milliseconds.
    pub elapsed_ms: u64,
}

impl std::fmt::Display for BuildResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Graph built: {} edges ({} EXTRACTED, {} INFERRED) in {}ms",
            self.edges_total, self.edges_extracted, self.edges_inferred, self.elapsed_ms,
        )
    }
}
