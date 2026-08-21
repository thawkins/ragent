//! Community detection over the symbol graph.
//!
//! Implements label propagation — a fast, deterministic community detection
//! algorithm that partitions the symbol graph into subsystems without an LLM
//! or embeddings (FR-013).  Each detected community receives an auto-generated
//! label derived from the most frequent symbol-name token or the
//! highest-degree node in the community (FR-019).
//!
//! ## Algorithm
//!
//! Label propagation works as follows:
//!
//! 1. Build an undirected adjacency list from the `graph_edges` table.
//! 2. Initialise every node (symbol) with a unique community label.
//! 3. Iterate: for each node, set its label to the most common label among
//!    its neighbours (ties broken by the numerically smallest label).
//! 4. Repeat until no label changes in a full pass (convergence) or a
//!    maximum iteration count is reached.
//! 5. Persist the final community assignments to the `communities` SQLite
//!    table via [`IndexStore::upsert_community`].
//! 6. Generate a label for each community (see [`label_for_community`]).
//!
//! The algorithm is deterministic because ties are always broken in favour
//! of the smallest label ID.

use crate::graph::CommunityInfo;
use crate::store::IndexStore;
use crate::types::SymbolFilter;
use anyhow::Result;
use std::collections::HashMap;
use tracing::debug;

/// Maximum number of label-propagation iterations before giving up.
const MAX_ITERATIONS: usize = 100;

/// Run community detection over the symbol graph and return the detected
/// communities with their auto-generated labels and member counts (FR-013,
/// FR-019).
///
/// This clears any existing community assignments, runs label propagation
/// over the graph built from the `graph_edges` table, persists the results
/// to the `communities` table, and returns a summary.
///
/// If the graph is empty (no edges), returns an empty list.
pub fn detect_communities(store: &IndexStore) -> Result<Vec<CommunityInfo>> {
    let edges = store.query_all_edges_typed()?;
    if edges.is_empty() {
        // Nothing to detect — clear any stale assignments and return empty.
        store.clear_communities()?;
        return Ok(Vec::new());
    }

    // ── Build the set of all node IDs that appear in edges ───────────────
    let mut node_set: Vec<i64> = Vec::new();
    {
        let mut seen: std::collections::HashSet<i64> = std::collections::HashSet::new();
        for edge in &edges {
            if seen.insert(edge.source_sym) {
                node_set.push(edge.source_sym);
            }
            if seen.insert(edge.target_sym) {
                node_set.push(edge.target_sym);
            }
        }
    }
    node_set.sort_unstable();
    debug!(
        "detect_communities: {} nodes, {} edges",
        node_set.len(),
        edges.len()
    );

    // ── Map node IDs to contiguous indices for efficient arrays ─────────
    let mut id_to_idx: HashMap<i64, usize> = HashMap::with_capacity(node_set.len());
    for (i, &id) in node_set.iter().enumerate() {
        id_to_idx.insert(id, i);
    }

    // ── Build undirected adjacency list ────────────────────────────────
    let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); node_set.len()];
    for edge in &edges {
        let src = id_to_idx[&edge.source_sym];
        let tgt = id_to_idx[&edge.target_sym];
        if src != tgt {
            adjacency[src].push(tgt);
            adjacency[tgt].push(src);
        }
    }
    // Deduplicate neighbours (parallel edges are possible).
    for neighbours in &mut adjacency {
        neighbours.sort_unstable();
        neighbours.dedup();
    }

    // ── Initialise labels: each node starts in its own community ────────
    let mut labels: Vec<usize> = (0..node_set.len()).collect();

    // ── Label propagation ───────────────────────────────────────────────
    for iteration in 0..MAX_ITERATIONS {
        let mut changed = false;
        for node in 0..node_set.len() {
            let neighbours = &adjacency[node];
            if neighbours.is_empty() {
                continue;
            }
            // Count label frequencies among neighbours.
            let mut counts: HashMap<usize, usize> = HashMap::new();
            for &nb in neighbours {
                *counts.entry(labels[nb]).or_default() += 1;
            }
            // Pick the most common label; ties broken by smallest label.
            let new_label = counts
                .into_iter()
                .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
                .map(|(label, _)| label)
                .unwrap_or(labels[node]);
            if new_label != labels[node] {
                labels[node] = new_label;
                changed = true;
            }
        }
        if !changed {
            debug!(
                "detect_communities: converged after {} iterations",
                iteration + 1
            );
            break;
        }
    }

    // ── Relabel communities to contiguous IDs starting at 0 ─────────────
    let mut remap: HashMap<usize, i64> = HashMap::new();
    let mut next_id: i64 = 0;
    for &label in &labels {
        remap.entry(label).or_insert_with(|| {
            let id = next_id;
            next_id += 1;
            id
        });
    }
    let final_labels: Vec<i64> = labels.iter().map(|l| remap[l]).collect();

    // ── Compute degree per node for auto-labelling ──────────────────────
    let mut degree: Vec<usize> = vec![0; node_set.len()];
    for (idx, neighbours) in adjacency.iter().enumerate() {
        degree[idx] = neighbours.len();
    }

    // ── Load symbol names for label generation ─────────────────────────
    let all_symbols = store.query_symbols(&SymbolFilter::default())?;
    let sym_name: HashMap<i64, String> =
        all_symbols.iter().map(|s| (s.id, s.name.clone())).collect();

    // ── Persist community assignments and collect member info ───────────
    store.clear_communities()?;

    // Group nodes by community.
    let mut community_members: HashMap<i64, Vec<(i64, usize)>> = HashMap::new();
    for (idx, &id) in node_set.iter().enumerate() {
        let comm = final_labels[idx];
        community_members
            .entry(comm)
            .or_default()
            .push((id, degree[idx]));
    }

    let mut communities: Vec<CommunityInfo> = Vec::new();
    for (&comm_id, members) in &community_members {
        let label = label_for_community(&comm_id, members, &sym_name);
        for &(sym_id, _) in members {
            store.upsert_community(sym_id, comm_id, label.as_deref())?;
        }
        communities.push(CommunityInfo {
            id: comm_id,
            label,
            member_count: members.len(),
        });
    }

    // Sort by member count descending for stable display.
    communities.sort_by(|a, b| {
        b.member_count
            .cmp(&a.member_count)
            .then_with(|| a.id.cmp(&b.id))
    });

    debug!(
        "detect_communities: {} communities detected",
        communities.len()
    );
    Ok(communities)
}

/// Generate an auto-label for a community (FR-019).
///
/// The label is derived from the highest-degree node's symbol name in the
/// community.  If symbol names are unavailable, falls back to `None`.
fn label_for_community(
    comm_id: &i64,
    members: &[(i64, usize)],
    sym_name: &HashMap<i64, String>,
) -> Option<String> {
    // Find the highest-degree node (ties broken by lowest symbol ID).
    let best = members
        .iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))?;

    sym_name.get(&best.0).map(|name| {
        // Use the last path segment of a qualified name if present,
        // otherwise the bare name.
        if let Some(pos) = name.rfind("::") {
            format!("community_{}:{}", comm_id, &name[pos + 2..])
        } else {
            format!("community_{}:{}", comm_id, name)
        }
    })
}

/// List detected communities with labels and member counts.
///
/// Returns communities that have already been persisted to the `communities`
/// table.  If no community detection has been run, returns an empty list.
/// Use [`detect_communities`] to run detection first.
pub fn list_communities(store: &IndexStore) -> Result<Vec<CommunityInfo>> {
    let rows = store.query_all_communities()?;
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    // Group by community ID.
    let mut map: HashMap<i64, (Option<String>, usize)> = HashMap::new();
    for (_sym_id, comm, label) in &rows {
        let entry = map.entry(*comm).or_insert_with(|| (label.clone(), 0));
        entry.1 += 1;
        // Prefer a non-None label if we see one.
        if entry.0.is_none() && label.is_some() {
            entry.0 = label.clone();
        }
    }

    let mut communities: Vec<CommunityInfo> = map
        .into_iter()
        .map(|(id, (label, count))| CommunityInfo {
            id,
            label,
            member_count: count,
        })
        .collect();

    communities.sort_by(|a, b| {
        b.member_count
            .cmp(&a.member_count)
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok(communities)
}
