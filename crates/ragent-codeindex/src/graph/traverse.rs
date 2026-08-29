//! Graph traversal: BFS shortest-path and explain queries.
//!
//! The shortest-path implementation uses breadth-first search (BFS) over
//! the adjacency list built from the `graph_edges` SQLite table.  BFS
//! guarantees the shortest path by hop count (FR-012).
//!
//! The explain implementation retrieves a symbol's node metadata and its
//! incoming/outgoing edges, limited to the top 50 connections (FR-011).

use crate::graph::{Connection, ExplainResult, PathResult};
use crate::store::IndexStore;
use crate::types::{GraphEdge, SymbolFilter};
use anyhow::Result;
use std::collections::{HashMap, VecDeque};

/// Maximum number of connections (incoming + outgoing) to return in an
/// explain result (FR-011).
const MAX_CONNECTIONS: usize = 50;

/// Compute the shortest path (by hop count) between two symbols.
///
/// Looks up both symbols by name, builds an adjacency list from the
/// `graph_edges` table, runs BFS from the source to the target, and
/// returns a [`PathResult`] with the sequence of hops.  Returns `None`
/// if either symbol is not found or no path exists (FR-012).
pub fn shortest_path(store: &IndexStore, from: &str, to: &str) -> Result<Option<PathResult>> {
    // ── Resolve source and target symbol IDs ────────────────────────────
    let source_id = match find_symbol_id(store, from)? {
        Some(id) => id,
        None => return Ok(None),
    };
    let target_id = match find_symbol_id(store, to)? {
        Some(id) => id,
        None => return Ok(None),
    };

    if source_id == target_id {
        // Trivial path: the symbol is itself.
        let name = symbol_name(store, source_id)?.unwrap_or_else(|| from.to_string());
        return Ok(Some(PathResult {
            hops: 0,
            steps: vec![(name, None)],
        }));
    }

    // ── Build adjacency list ────────────────────────────────────────────
    let edges = store.query_all_edges_typed()?;
    if edges.is_empty() {
        return Ok(None);
    }

    let mut adjacency: HashMap<i64, Vec<&GraphEdge>> = HashMap::new();
    for edge in &edges {
        adjacency.entry(edge.source_sym).or_default().push(edge);
    }

    // ── BFS ─────────────────────────────────────────────────────────────
    let mut visited: HashMap<i64, Option<(&GraphEdge, i64)>> = HashMap::new();
    visited.insert(source_id, None);

    let mut queue: VecDeque<i64> = VecDeque::new();
    queue.push_back(source_id);

    while let Some(current) = queue.pop_front() {
        if current == target_id {
            break;
        }

        // Borrow the neighbour list instead of cloning it for every visited
        // node; the borrow ends before the next loop iteration.
        let empty: Vec<&GraphEdge> = Vec::new();
        for edge in adjacency.get(&current).unwrap_or(&empty) {
            let next = edge.target_sym;
            if visited.contains_key(&next) {
                continue;
            }
            visited.insert(next, Some((edge, current)));
            queue.push_back(next);
        }
    }

    // ── Reconstruct path ────────────────────────────────────────────────
    if !visited.contains_key(&target_id) {
        return Ok(None); // No path found.
    }

    // Walk backwards from target to source, collecting edges.
    let mut path_edges: Vec<&GraphEdge> = Vec::new();
    let mut current = target_id;
    while let Some(Some((edge, prev))) = visited.get(&current) {
        path_edges.push(edge);
        current = *prev;
    }
    path_edges.reverse();

    // Build the steps: (symbol_name, Some(edge_kind)).
    let mut steps: Vec<(String, Option<String>)> = Vec::new();
    let source_name = symbol_name(store, source_id)?.unwrap_or_else(|| from.to_string());
    steps.push((source_name, None));
    for edge in &path_edges {
        let name = symbol_name(store, edge.target_sym)?
            .unwrap_or_else(|| format!("sym#{}", edge.target_sym));
        steps.push((name, Some(edge.kind.to_string())));
    }

    Ok(Some(PathResult {
        hops: path_edges.len(),
        steps,
    }))
}

/// Explain a symbol: show its node metadata and connections (FR-011).
///
/// Looks up the symbol by name, retrieves its source file and line, its
/// community assignment (if community detection has been run), its degree
/// (number of incident edges), and its incoming/outgoing edges (limited to
/// the top 50 connections).  Returns `None` if the symbol is not found.
pub fn explain(store: &IndexStore, name: &str) -> Result<Option<ExplainResult>> {
    // ── Resolve the symbol ───────────────────────────────────────────────
    let symbol = match find_symbol(store, name)? {
        Some(s) => s,
        None => return Ok(None),
    };

    // ── Look up the source file path ────────────────────────────────────
    let source_file = store
        .get_file_by_id(symbol.file_id)?
        .map(|f| f.path)
        .unwrap_or_default();

    // ── Look up community assignment ────────────────────────────────────
    let community = store
        .query_all_communities()?
        .into_iter()
        .find(|(sym_id, _, _)| *sym_id == symbol.id)
        .map(|(_, community_id, _)| community_id);

    // ── Get all incident edges ──────────────────────────────────────────
    let incident = store.query_edges_for_symbol_typed(symbol.id)?;

    // ── Split into incoming and outgoing ────────────────────────────────
    let mut incoming: Vec<Connection> = Vec::new();
    let mut outgoing: Vec<Connection> = Vec::new();

    // Build a lookup of all symbols for name/file resolution.
    let all_symbols = store.query_symbols(&SymbolFilter::default())?;
    let sym_lookup: HashMap<i64, &crate::types::Symbol> =
        all_symbols.iter().map(|s| (s.id, s)).collect();

    for edge in &incident {
        let conn = edge_to_connection(edge, symbol.id, &sym_lookup, store)?;
        if edge.source_sym == symbol.id {
            // Outgoing: this symbol is the source.
            outgoing.push(conn);
        } else {
            // Incoming: this symbol is the target.
            incoming.push(conn);
        }
    }

    let degree = incoming.len() + outgoing.len();

    // ── Limit to MAX_CONNECTIONS ────────────────────────────────────────
    // Sort by nothing in particular (edges are returned in DB order); just
    // truncate if the total exceeds the limit.  We split the limit
    // proportionally: half for incoming, half for outgoing, with the
    // remainder going to whichever is larger.
    let total = incoming.len() + outgoing.len();
    if total > MAX_CONNECTIONS {
        let half = MAX_CONNECTIONS / 2;
        let rem = MAX_CONNECTIONS - half;
        let inc_limit = if incoming.len() >= outgoing.len() {
            half + rem
        } else {
            half
        };
        let out_limit = MAX_CONNECTIONS - inc_limit.min(incoming.len());
        incoming.truncate(inc_limit);
        outgoing.truncate(out_limit.max(half));
    }

    Ok(Some(ExplainResult {
        name: symbol.name,
        source_file,
        line: symbol.start_line,
        community,
        degree,
        incoming,
        outgoing,
    }))
}

/// Convert a [`GraphEdge`] to a [`Connection`] for the explain result.
///
/// `explained_sym_id` is the ID of the symbol being explained; the
/// connection refers to the *other* symbol in the edge.
fn edge_to_connection(
    edge: &GraphEdge,
    explained_sym_id: i64,
    sym_lookup: &HashMap<i64, &crate::types::Symbol>,
    store: &IndexStore,
) -> Result<Connection> {
    // The "other" symbol is the one that is NOT the explained symbol.
    let other_id = if edge.source_sym == explained_sym_id {
        edge.target_sym
    } else {
        edge.source_sym
    };

    let (symbol_name, source_file) = match sym_lookup.get(&other_id) {
        Some(s) => {
            let file = store
                .get_file_by_id(s.file_id)?
                .map(|f| f.path)
                .unwrap_or_default();
            (s.name.clone(), file)
        }
        None => (format!("sym#{other_id}"), String::new()),
    };

    Ok(Connection {
        symbol: symbol_name,
        source_file,
        kind: edge.kind.to_string(),
        confidence: edge.confidence,
        line: edge.line,
    })
}

/// Find the first symbol ID matching the given name (exact match, case-
/// sensitive, first result).
fn find_symbol_id(store: &IndexStore, name: &str) -> Result<Option<i64>> {
    let filter = SymbolFilter {
        name: Some(name.to_string()),
        limit: Some(1),
        ..Default::default()
    };
    let symbols = store.query_symbols(&filter)?;
    Ok(symbols.first().map(|s| s.id))
}

/// Find the first symbol matching the given name (exact match, case-
/// sensitive, first result).  Returns the full [`Symbol`] so the caller
/// has access to file_id, start_line, etc.
fn find_symbol(store: &IndexStore, name: &str) -> Result<Option<crate::types::Symbol>> {
    let filter = SymbolFilter {
        name: Some(name.to_string()),
        limit: Some(1),
        ..Default::default()
    };
    let symbols = store.query_symbols(&filter)?;
    Ok(symbols.into_iter().next())
}

/// Look up a symbol's name by its ID.
///
/// H-003: uses a single keyed `SELECT name FROM symbols WHERE id = ?` instead
/// of loading *all* symbols and linearly searching (which was O(N) per call —
/// quadratic when reconstructing a path or explaining a symbol with many
/// connections).
fn symbol_name(store: &IndexStore, sym_id: i64) -> Result<Option<String>> {
    store.get_symbol_name(sym_id)
}
