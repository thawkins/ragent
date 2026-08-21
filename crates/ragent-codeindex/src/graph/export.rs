//! Graph export: serialise to `graph.json` and `GRAPH_REPORT.md`.
//!
//! Produces two output formats:
//!
//! - **`graph.json`** — a JSON document with `nodes` and `edges` arrays.
//!   Each node carries attributes (`id`, `name`, `kind`, `source_file`,
//!   `line`, `community`, `degree`) and each edge carries attributes
//!   (`source`, `target`, `kind`, `confidence`, `line`).  This format is
//!   compatible with common graph-visualisation tools (FR-020).
//! - **`GRAPH_REPORT.md`** — a human-readable Markdown summary with
//!   graph statistics, top god-nodes, and a community breakdown (FR-010).
//!
//! Both functions are read-only: they query the `graph_edges`,
//! `communities`, `symbols`, and `indexed_files` tables without modifying
//! any data (FR-025).

use crate::store::IndexStore;
use crate::types::{Confidence, SymbolFilter, SymbolKind};
use anyhow::Result;
use serde_json::{Value, json};
use std::collections::HashMap;

// ── JSON Export (FR-020) ───────────────────────────────────────────────────

/// Serialise the graph to a JSON string.
///
/// Returns a JSON object with `nodes` and `edges` arrays.  Each node
/// includes `id`, `name`, `kind`, `source_file`, `line`, `community`, and
/// `degree`.  Each edge includes `source`, `target`, `kind`, `confidence`,
/// and `line`.  When the graph is empty, returns
/// `{"nodes":[],"edges":[]}`.
pub fn to_json(store: &IndexStore) -> Result<String> {
    let edges = store.query_all_edges_typed()?;
    let symbols = store.query_symbols(&SymbolFilter::default())?;
    let communities = store.query_all_communities()?;

    // Build a community map: symbol_id -> community_id.
    let community_map: HashMap<i64, i64> = communities
        .iter()
        .map(|(sym_id, comm, _)| (*sym_id, *comm))
        .collect();

    // Build a degree map: symbol_id -> degree (count of incident edges).
    let mut degree_map: HashMap<i64, usize> = HashMap::new();
    for edge in &edges {
        *degree_map.entry(edge.source_sym).or_default() += 1;
        *degree_map.entry(edge.target_sym).or_default() += 1;
    }

    // Build a file-path lookup: file_id -> path.
    let mut file_paths: HashMap<i64, String> = HashMap::new();
    for sym in &symbols {
        if let std::collections::hash_map::Entry::Vacant(e) = file_paths.entry(sym.file_id) {
            if let Ok(Some(file)) = store.get_file_by_id(sym.file_id) {
                e.insert(file.path);
            }
        }
    }

    // Serialise nodes.
    let nodes: Vec<Value> = symbols
        .iter()
        .map(|sym| {
            let source_file = file_paths.get(&sym.file_id).cloned().unwrap_or_default();
            json!({
                "id": sym.id,
                "name": sym.name,
                "kind": sym.kind.to_string(),
                "source_file": source_file,
                "line": sym.start_line,
                "community": community_map.get(&sym.id),
                "degree": degree_map.get(&sym.id).copied().unwrap_or(0),
            })
        })
        .collect();

    // Serialise edges.
    let edge_values: Vec<Value> = edges
        .iter()
        .map(|edge| {
            json!({
                "source": edge.source_sym,
                "target": edge.target_sym,
                "kind": edge.kind.to_string(),
                "confidence": edge.confidence.to_string(),
                "line": edge.line,
            })
        })
        .collect();

    let graph = json!({
        "nodes": nodes,
        "edges": edge_values,
    });

    // Pretty-print for readability.
    Ok(serde_json::to_string_pretty(&graph)?)
}

// ── Markdown Report (FR-010) ───────────────────────────────────────────────

/// Generate a `GRAPH_REPORT.md` report.
///
/// Produces a human-readable Markdown document with:
/// - Graph statistics (node count, edge count, extracted/inferred breakdown)
/// - Top god-nodes (highest-degree symbols)
/// - Community breakdown
///
/// When the graph is empty, returns a message instructing the user to run
/// `/codeindex graph build` first.
pub fn to_report(store: &IndexStore) -> Result<String> {
    let edges = store.query_all_edges_typed()?;
    let symbols = store.query_symbols(&SymbolFilter::default())?;
    let communities = store.query_all_communities()?;

    if edges.is_empty() {
        return Ok(
            "# Graph Report\n\n*No graph data available. Run `/codeindex graph build` first.*\n"
                .to_string(),
        );
    }

    let mut report = String::new();
    report.push_str("# Graph Report\n\n");

    // ── Statistics ──────────────────────────────────────────────────────
    let extracted_count = edges
        .iter()
        .filter(|e| e.confidence == Confidence::Extracted)
        .count();
    let inferred_count = edges
        .iter()
        .filter(|e| e.confidence == Confidence::Inferred)
        .count();

    report.push_str("## Statistics\n\n");
    report.push_str(&format!("- **Nodes:** {}\n", symbols.len()));
    report.push_str(&format!("- **Edges:** {}\n", edges.len()));
    report.push_str(&format!("- **Extracted:** {}\n", extracted_count));
    report.push_str(&format!("- **Inferred:** {}\n", inferred_count));
    if !communities.is_empty() {
        report.push_str(&format!("- **Communities:** {}\n", communities.len()));
    }
    report.push('\n');

    // ── Top God-Nodes (by degree) ───────────────────────────────────────
    let mut degree_map: HashMap<i64, usize> = HashMap::new();
    for edge in &edges {
        *degree_map.entry(edge.source_sym).or_default() += 1;
        *degree_map.entry(edge.target_sym).or_default() += 1;
    }

    // Build a symbol lookup: id -> (name, file_path, kind).
    let mut sym_lookup: HashMap<i64, (String, String, SymbolKind)> = HashMap::new();
    for sym in &symbols {
        let file_path = store
            .get_file_by_id(sym.file_id)
            .ok()
            .flatten()
            .map(|f| f.path)
            .unwrap_or_default();
        sym_lookup.insert(sym.id, (sym.name.clone(), file_path, sym.kind));
    }

    let mut god_nodes: Vec<(i64, usize)> = degree_map.iter().map(|(&id, &deg)| (id, deg)).collect();
    god_nodes.sort_by(|a, b| b.1.cmp(&a.1));
    let top_n = god_nodes.iter().take(20).collect::<Vec<_>>();

    if !top_n.is_empty() {
        report.push_str("## Top God-Nodes (Highest Degree)\n\n");
        report.push_str("| # | Symbol | Kind | Source File | Degree |\n");
        report.push_str("|---|--------|------|-------------|--------|\n");
        for (i, &(sym_id, degree)) in top_n.iter().enumerate() {
            let (name, file, kind) = sym_lookup
                .get(&sym_id)
                .cloned()
                .unwrap_or_else(|| (format!("sym#{sym_id}"), String::new(), SymbolKind::Unknown));
            report.push_str(&format!(
                "| {} | `{}` | {} | `{}` | {} |\n",
                i + 1,
                name,
                kind,
                file,
                degree,
            ));
        }
        report.push('\n');
    }

    // ── Community Breakdown ──��─────────────────────────────────────────
    if !communities.is_empty() {
        // Group symbols by community.
        let mut comm_groups: HashMap<i64, Vec<(i64, Option<String>)>> = HashMap::new();
        for (sym_id, comm, label) in &communities {
            comm_groups
                .entry(*comm)
                .or_default()
                .push((*sym_id, label.clone()));
        }

        // Sort communities by member count (descending).
        let mut sorted_comms: Vec<(i64, Vec<(i64, Option<String>)>)> =
            comm_groups.into_iter().collect();
        sorted_comms.sort_by(|a, b| b.1.len().cmp(&a.1.len()));

        report.push_str("## Communities\n\n");
        report.push_str("| Community | Label | Members |\n");
        report.push_str("|-----------|-------|--------|\n");
        for (comm_id, members) in &sorted_comms {
            let label = members
                .first()
                .and_then(|(_, l)| l.clone())
                .unwrap_or_default();
            report.push_str(&format!(
                "| {} | {} | {} |\n",
                comm_id,
                label,
                members.len()
            ));
        }
        report.push('\n');
    }

    // ── Edge Kind Distribution ─────────────────────────────────────────
    let mut kind_counts: HashMap<String, usize> = HashMap::new();
    for edge in &edges {
        *kind_counts.entry(edge.kind.to_string()).or_default() += 1;
    }
    let mut sorted_kinds: Vec<(String, usize)> = kind_counts.into_iter().collect();
    sorted_kinds.sort_by(|a, b| b.1.cmp(&a.1));

    report.push_str("## Edge Kind Distribution\n\n");
    report.push_str("| Kind | Count |\n");
    report.push_str("|------|-------|\n");
    for (kind, count) in &sorted_kinds {
        report.push_str(&format!("| {} | {} |\n", kind, count));
    }

    Ok(report)
}
