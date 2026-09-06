//! Edge derivation for the semantic code graph.
//!
//! Edges are derived deterministically from the indexed symbols, imports,
//! and references — no LLM or embeddings are used.  See the parent module
//! docs for an overview.

use super::BuildResult;
use crate::store::IndexStore;
use crate::types::{Confidence, EdgeKind, GraphEdge, Symbol, SymbolKind};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::atomic::Ordering as AtomicOrdering;
use tracing::debug;

/// Extract a trait name from an `impl` signature such as
/// `impl Trait for Type` or `impl Trait<Type> for Type`.
///
/// Returns the first bare identifier found between `impl` and `for`, or
/// `None` if the signature does not match the `impl ... for ...` form.
fn extract_trait_from_signature(sig: &str) -> Option<String> {
    let s = sig.trim();
    if !s.starts_with("impl") {
        return None;
    }
    let rest = s["impl".len()..].trim_start();
    let for_pos = rest.find(" for")?;
    let trait_part = rest[..for_pos].trim();
    // Take the first identifier token (skip generics, lifetimes, etc.).
    let mut token = String::new();
    for ch in trait_part.chars() {
        if ch.is_alphanumeric() || ch == '_' {
            token.push(ch);
        } else {
            if token.is_empty() {
                continue;
            }
            break;
        }
    }
    if token.is_empty() { None } else { Some(token) }
}

/// Shared name-resolution context built once from all symbols.
///
/// This avoids rebuilding the same `HashMap`s for every file during a full
/// reindex — the previous code rebuilt them N times (once per file), which
/// was O(N²) in both time and memory.
struct NameResolution {
    /// symbol_name → Vec<(symbol_id, file_id)>
    name_to_symbols: HashMap<String, Vec<(i64, i64)>>,
    /// symbol_id → file_id
    sym_file_map: HashMap<i64, i64>,
    /// All symbols grouped by file_id for `find_containing_symbol` lookups.
    symbols_by_file: HashMap<i64, Vec<Symbol>>,
}

impl NameResolution {
    /// Build the resolution context from the full symbol set.
    fn from_symbols(all_symbols: &[Symbol]) -> Self {
        let mut name_to_symbols: HashMap<String, Vec<(i64, i64)>> = HashMap::new();
        let mut sym_file_map: HashMap<i64, i64> = HashMap::with_capacity(all_symbols.len());
        let mut symbols_by_file: HashMap<i64, Vec<Symbol>> = HashMap::new();

        for sym in all_symbols {
            name_to_symbols
                .entry(sym.name.clone())
                .or_default()
                .push((sym.id, sym.file_id));
            sym_file_map.insert(sym.id, sym.file_id);
            symbols_by_file
                .entry(sym.file_id)
                .or_default()
                .push(sym.clone());
        }

        Self {
            name_to_symbols,
            sym_file_map,
            symbols_by_file,
        }
    }

    /// Find the symbol that lexically contains a reference at `(file_id, line)`,
    /// using only the symbols for that file (avoids scanning all symbols).
    fn containing_symbol(&self, file_id: i64, line: u32) -> Option<i64> {
        let syms = self.symbols_by_file.get(&file_id)?;
        let mut best: Option<(&Symbol, u32)> = None;
        for sym in syms {
            if line >= sym.start_line && line <= sym.end_line {
                let span = sym.end_line.saturating_sub(sym.start_line);
                match best {
                    Some((_, best_span)) if span >= best_span => {}
                    _ => best = Some((sym, span)),
                }
            }
        }
        best.map(|(sym, _)| sym.id)
    }
}

/// Derive edges from symbol references (calls and type/field-access refs).
///
/// Emits `Calls` edges for `kind = "call"` refs and `References` edges for
/// `kind = "type"` or `kind = "field_access"` refs.  Only refs whose
/// containing symbol is in `filter_file_id` (when `Some`) are emitted.
fn derive_ref_edges(
    refs: &[crate::types::SymbolRef],
    nr: &NameResolution,
    filter_file_id: Option<i64>,
) -> Vec<GraphEdge> {
    let mut edges = Vec::new();
    for r in refs {
        // When filtering by file, skip refs not in that file.
        if let Some(fid) = filter_file_id {
            if r.file_id != fid {
                continue;
            }
        }

        let target_candidates = match nr.name_to_symbols.get(&r.symbol_name) {
            Some(c) => c,
            None => continue,
        };

        let source_sym_id = match nr.containing_symbol(r.file_id, r.line) {
            Some(id) => id,
            None => continue,
        };

        // When filtering by file, only emit edges whose source is in that file.
        if let Some(fid) = filter_file_id {
            if nr.sym_file_map.get(&source_sym_id) != Some(&fid) {
                continue;
            }
        }

        let kind = match r.kind.as_str() {
            "call" => EdgeKind::Calls,
            "type" | "field_access" => EdgeKind::References,
            _ => continue,
        };

        for &(target_sym_id, target_file_id) in target_candidates {
            if target_sym_id == source_sym_id {
                continue;
            }

            let confidence = if r.file_id == target_file_id {
                Confidence::Extracted
            } else {
                Confidence::Inferred
            };

            edges.push(GraphEdge {
                source_sym: source_sym_id,
                target_sym: target_sym_id,
                kind,
                confidence,
                source_file: Some(r.file_id),
                line: Some(r.line),
            });
        }
    }
    edges
}

/// Derive `Imports` edges from a file's symbols + imports.
fn derive_import_edges(
    file_symbols: &[Symbol],
    imports: &[crate::types::ImportEntry],
    nr: &NameResolution,
) -> Vec<GraphEdge> {
    let mut edges = Vec::new();
    // Invert the loop: resolve each import's candidate list once instead of
    // re-hashing the same import names once per symbol (symbols × imports
    // hash lookups become imports).
    for imp in imports {
        let Some(candidates) = nr.name_to_symbols.get(&imp.imported_name) else {
            continue;
        };
        for sym in file_symbols {
            for &(target_id, target_file_id) in candidates {
                if target_id == sym.id {
                    continue;
                }
                let confidence = if sym.file_id == target_file_id {
                    Confidence::Extracted
                } else {
                    Confidence::Inferred
                };
                edges.push(GraphEdge {
                    source_sym: sym.id,
                    target_sym: target_id,
                    kind: EdgeKind::Imports,
                    confidence,
                    source_file: Some(sym.file_id),
                    line: Some(imp.line),
                });
            }
        }
    }
    edges
}

/// Derive `Inherits` and `Implements` edges from `impl` blocks in a file.
fn derive_impl_edges(file_symbols: &[Symbol], nr: &NameResolution) -> Vec<GraphEdge> {
    let mut edges = Vec::new();
    for sym in file_symbols {
        if sym.kind != SymbolKind::Impl {
            continue;
        }

        let impl_name = &sym.name;
        if let Some(candidates) = nr.name_to_symbols.get(impl_name) {
            for &(target_id, target_file_id) in candidates {
                if target_id == sym.id {
                    continue;
                }
                let confidence = if sym.file_id == target_file_id {
                    Confidence::Extracted
                } else {
                    Confidence::Inferred
                };
                edges.push(GraphEdge {
                    source_sym: sym.id,
                    target_sym: target_id,
                    kind: EdgeKind::Inherits,
                    confidence,
                    source_file: Some(sym.file_id),
                    line: Some(sym.start_line),
                });
            }
        }

        if let Some(ref sig) = sym.signature {
            if let Some(trait_name) = extract_trait_from_signature(sig) {
                if let Some(candidates) = nr.name_to_symbols.get(&trait_name) {
                    for &(target_id, target_file_id) in candidates {
                        if target_id == sym.id {
                            continue;
                        }
                        let confidence = if sym.file_id == target_file_id {
                            Confidence::Extracted
                        } else {
                            Confidence::Inferred
                        };
                        edges.push(GraphEdge {
                            source_sym: sym.id,
                            target_sym: target_id,
                            kind: EdgeKind::Implements,
                            confidence,
                            source_file: Some(sym.file_id),
                            line: Some(sym.start_line),
                        });
                    }
                }
            }
        }
    }
    edges
}

/// Re-derive edges for a single file's symbols (incremental update).
///
/// Only emits edges where the source symbol is in the given file.  This
/// is used by `index_file` to update edges for a single file without
/// rebuilding the entire graph (FR-008).
pub fn derive_edges_for_file(store: &IndexStore, file_id: i64) -> Result<()> {
    let file_symbols = store.get_file_symbols(file_id)?;
    if file_symbols.is_empty() {
        return Ok(());
    }

    // Load all symbols for name resolution (cross-file targets).
    // H-005: build the NameResolution map from a single symbol load here.
    let all_symbols = store.query_symbols(&crate::types::SymbolFilter::default())?;
    let nr = NameResolution::from_symbols(&all_symbols);

    let mut edges: Vec<GraphEdge> = Vec::new();

    // Refs for this file only (SQL-level filter — avoids loading all refs).
    let file_refs = store.get_file_refs(file_id)?;
    edges.extend(derive_ref_edges(&file_refs, &nr, Some(file_id)));

    // Imports for this file.
    let imports = store.get_file_imports(file_id)?;
    edges.extend(derive_import_edges(&file_symbols, &imports, &nr));

    // Impl blocks for this file.
    edges.extend(derive_impl_edges(&file_symbols, &nr));

    // Persist edges in a single transaction (avoids per-edge auto-commit).
    store.begin_transaction()?;
    let result = store.upsert_edges_batch(&edges);
    if result.is_ok() {
        store.commit_transaction()?;
    } else {
        // Roll back on error: BEGIN is still active, so issue a ROLLBACK.
        let _ = store.conn.execute_batch("ROLLBACK");
    }
    result?;

    debug!(
        "derive_edges_for_file: {} edges for file_id={}",
        edges.len(),
        file_id
    );

    Ok(())
}

/// Progress counter shared by `full_reindex` with [`derive_and_store`] so the
/// per-file loop can report incremental progress to lock-free atomics polled
/// by the UI.
type ProgressCounter = std::sync::atomic::AtomicU32;

/// A consistent read-snapshot of everything full-graph edge derivation needs.
///
/// Loading the snapshot takes four SQL scans under one brief store lock; the
/// CPU-heavy derivation ([`derive_edges_from_inputs`]) then runs with **no**
/// store lock held, so FTS search and other store readers stay available for
/// the whole derivation.  Only the final [`persist_edges`] call re-acquires
/// the store lock, and it holds it for a single write transaction touching
/// the `graph_edges` table only.
pub struct GraphInputs {
    /// Every indexed symbol (drives name resolution and per-file grouping).
    pub all_symbols: Vec<Symbol>,
    /// Every symbol reference (drives calls/references edges).
    pub all_refs: Vec<crate::types::SymbolRef>,
    /// Imports grouped by `file_id` (drives imports edges).
    pub imports_by_file: HashMap<i64, Vec<crate::types::ImportEntry>>,
    /// All indexed files in path order (drives iteration order + language).
    pub files: Vec<crate::types::FileEntry>,
    /// `path -> file_id` for resolving each file's symbol/import groups.
    pub file_ids: HashMap<String, i64>,
}

/// Snapshot the store inputs needed for full-graph edge derivation.
///
/// Runs four read-only SQL scans (symbols, refs, imports, files) under the
/// caller's store guard; keep that guard scope short.  Pair with
/// [`derive_edges_from_inputs`] + [`persist_edges`] so the derivation runs
/// without the lock (FR-026 phased graph build).
pub fn load_graph_inputs(store: &IndexStore) -> Result<GraphInputs> {
    let all_symbols = store.query_symbols(&crate::types::SymbolFilter::default())?;
    let all_refs = store.query_all_refs()?;
    let files = store.list_files()?;
    let file_ids: HashMap<String, i64> = store
        .list_files_with_ids()?
        .into_iter()
        .map(|(id, path)| (path, id))
        .collect();

    let mut imports_by_file: HashMap<i64, Vec<crate::types::ImportEntry>> = HashMap::new();
    for (file_id, imp) in store.list_all_imports()? {
        imports_by_file.entry(file_id).or_default().push(imp);
    }

    Ok(GraphInputs {
        all_symbols,
        all_refs,
        imports_by_file,
        files,
        file_ids,
    })
}

/// Derive the full edge set from a [`GraphInputs`] snapshot — pure in-memory.
///
/// Performs no store access at all, so callers may run it without any lock
/// held.  Mirrors the derivation loop of the former long-held-lock
/// `derive_and_store`: ref edges for all files, then import + impl edges per
/// file (files without a detected language are skipped, matching the
/// original behaviour).
pub fn derive_edges_from_inputs(
    inputs: &GraphInputs,
    graph_done: Option<&ProgressCounter>,
) -> Vec<GraphEdge> {
    let nr = NameResolution::from_symbols(&inputs.all_symbols);
    let mut all_edges = derive_ref_edges(&inputs.all_refs, &nr, None);

    for file in &inputs.files {
        if file.language.is_none() {
            continue;
        }
        let Some(&file_id) = inputs.file_ids.get(&file.path) else {
            continue;
        };
        let Some(file_symbols) = nr.symbols_by_file.get(&file_id) else {
            continue;
        };
        if file_symbols.is_empty() {
            continue;
        }
        if let Some(imports) = inputs.imports_by_file.get(&file_id) {
            all_edges.extend(derive_import_edges(file_symbols, imports, &nr));
        }
        all_edges.extend(derive_impl_edges(file_symbols, &nr));
        if let Some(done) = graph_done {
            done.fetch_add(1, AtomicOrdering::Relaxed);
        }
    }

    all_edges
}

/// Derive the full edge set restricted to a single language — pure in-memory.
///
/// Like [`derive_edges_from_inputs`] but only emits edges whose source and
/// target symbols live in files of `language` (mirrors the former
/// `derive_and_store_for_language` filter).
pub fn derive_edges_from_inputs_for_language(
    inputs: &GraphInputs,
    language: &str,
    graph_done: Option<&ProgressCounter>,
) -> Vec<GraphEdge> {
    let target_set: std::collections::HashSet<i64> = inputs
        .files
        .iter()
        .filter(|f| f.language.as_deref() == Some(language))
        .filter_map(|f| inputs.file_ids.get(&f.path).copied())
        .collect();

    let nr = NameResolution::from_symbols(&inputs.all_symbols);

    let mut all_edges: Vec<GraphEdge> = Vec::new();

    // 1. Ref-derived edges, filtered to target-language files.
    for r in &inputs.all_refs {
        if !target_set.contains(&r.file_id) {
            continue;
        }
        let target_candidates = match nr.name_to_symbols.get(&r.symbol_name) {
            Some(c) => c,
            None => continue,
        };
        let source_sym_id = match nr.containing_symbol(r.file_id, r.line) {
            Some(id) => id,
            None => continue,
        };
        if !target_set.contains(&nr.sym_file_map[&source_sym_id]) {
            continue;
        }
        let kind = match r.kind.as_str() {
            "call" => EdgeKind::Calls,
            "type" | "field_access" => EdgeKind::References,
            _ => continue,
        };
        for &(target_sym_id, target_file_id) in target_candidates {
            if target_sym_id == source_sym_id {
                continue;
            }
            let confidence = if r.file_id == target_file_id {
                Confidence::Extracted
            } else {
                Confidence::Inferred
            };
            all_edges.push(GraphEdge {
                source_sym: source_sym_id,
                target_sym: target_sym_id,
                kind,
                confidence,
                source_file: Some(r.file_id),
                line: Some(r.line),
            });
        }
    }

    // 2. Import + impl edges for target-language files.
    for file in &inputs.files {
        if file.language.as_deref() != Some(language) {
            continue;
        }
        let Some(&file_id) = inputs.file_ids.get(&file.path) else {
            continue;
        };
        let Some(file_symbols) = nr.symbols_by_file.get(&file_id) else {
            continue;
        };
        if file_symbols.is_empty() {
            continue;
        }
        if let Some(imports) = inputs.imports_by_file.get(&file_id) {
            all_edges.extend(derive_import_edges(file_symbols, imports, &nr));
        }
        all_edges.extend(derive_impl_edges(file_symbols, &nr));
    }

    if let Some(done) = graph_done {
        done.store(target_set.len() as u32, AtomicOrdering::Relaxed);
    }

    all_edges
}

/// Persist a derived edge set: clear + bulk-insert in one transaction, then
/// return the confidence-split [`BuildResult`] summary.
///
/// Touches only the `graph_edges` table; keep the caller's store-guard scope
/// limited to this call so FTS/store readers are blocked for a single write
/// transaction, not the derivation.
pub fn persist_edges(store: &IndexStore, edges: &[GraphEdge]) -> Result<BuildResult> {
    let start = std::time::Instant::now();

    // Persist: clear + bulk-insert in one transaction. The clear stays
    // inside the transaction so a crash between clear and commit cannot leave
    // the edge table permanently empty (which would trip the TUI empty-graph
    // guard until the next rebuild).
    store.begin_transaction()?;
    let result = (|| -> anyhow::Result<()> {
        store.clear_edges()?;
        store.upsert_edges_batch(edges)
    })();
    if result.is_ok() {
        store.commit_transaction()?;
    } else {
        let _ = store.conn.execute_batch("ROLLBACK");
    }
    result?;

    let edges_extracted = store.edge_count_by_confidence_typed(Confidence::Extracted)? as usize;
    let edges_inferred = store.edge_count_by_confidence_typed(Confidence::Inferred)? as usize;

    debug!(
        "persist_edges: {} edges ({} EXTRACTED, {} INFERRED) in {}ms",
        edges_extracted + edges_inferred,
        edges_extracted,
        edges_inferred,
        start.elapsed().as_millis()
    );

    Ok(BuildResult {
        edges_total: edges_extracted + edges_inferred,
        edges_extracted,
        edges_inferred,
        elapsed_ms: start.elapsed().as_millis() as u64,
    })
}

/// Rebuild the entire semantic edge graph for all indexed files.
///
/// Loads a [`GraphInputs`] snapshot, derives all edges in memory, and
/// persists them in a single transaction.  This is the full-graph path
/// used by `SymbolGraph::build` (FR-007); `CodeIndex` uses the split
/// load/derive/persist phases directly so the store lock is released during
/// the CPU-heavy derivation (FR-026).
///
/// `graph_done` (when supplied) is incremented once per file processed in the
/// import/impl loop so callers can display live progress.
pub fn derive_and_store(
    store: &IndexStore,
    graph_done: Option<&ProgressCounter>,
) -> Result<BuildResult> {
    let start = std::time::Instant::now();

    let inputs = load_graph_inputs(store)?;
    if inputs.all_symbols.is_empty() {
        store.clear_edges()?;
        return Ok(BuildResult {
            edges_total: 0,
            edges_extracted: 0,
            edges_inferred: 0,
            elapsed_ms: start.elapsed().as_millis() as u64,
        });
    }

    let all_edges = derive_edges_from_inputs(&inputs, graph_done);
    let mut result = persist_edges(store, &all_edges)?;
    result.elapsed_ms = start.elapsed().as_millis() as u64;

    debug!(
        "derive_and_store: {} edges in {}ms",
        result.edges_total, result.elapsed_ms
    );

    Ok(result)
}

/// Rebuild the semantic edge graph restricted to files of a single language.
///
/// Loads a [`GraphInputs`] snapshot, derives edges in memory for files whose
/// detected language matches, then persists in a single transaction.  Like
/// [`derive_and_store`], the CPU-heavy derivation needs no store access;
/// callers that want maximum concurrency should use the split phases
/// directly (FR-026).
pub fn derive_and_store_for_language(store: &IndexStore, language: &str) -> Result<BuildResult> {
    let start = std::time::Instant::now();

    let inputs = load_graph_inputs(store)?;

    let all_edges = derive_edges_from_inputs_for_language(&inputs, language, None);
    let mut result = persist_edges(store, &all_edges)?;
    result.elapsed_ms = start.elapsed().as_millis() as u64;

    Ok(result)
}
