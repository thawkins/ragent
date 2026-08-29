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

/// Rebuild the entire semantic edge graph for all indexed files.
///
/// Clears any existing edges, then derives edges for every indexed file
/// and returns a [`BuildResult`] summary.  This is the full-graph path
/// used by `full_reindex` (FR-007) and `SymbolGraph::build`.
///
/// Performance: loads all symbols and all refs **once**, builds the
/// name-resolution maps **once**, derives all edges in a single in-memory
/// pass, and persists them in a single transaction.  This is O(N) in the
/// number of files/symbols/refs — the previous implementation was O(N²)
/// because it reloaded all symbols and all refs for every file.
pub fn derive_and_store(store: &IndexStore) -> Result<BuildResult> {
    let start = std::time::Instant::now();

    // Load all symbols ONCE and build name-resolution maps ONCE.
    let all_symbols = store.query_symbols(&crate::types::SymbolFilter::default())?;
    if all_symbols.is_empty() {
        store.clear_edges()?;
        return Ok(BuildResult {
            edges_total: 0,
            edges_extracted: 0,
            edges_inferred: 0,
            elapsed_ms: start.elapsed().as_millis() as u64,
        });
    }
    let nr = NameResolution::from_symbols(&all_symbols);

    // Load all refs ONCE.
    let all_refs = store.query_all_refs()?;

    // Collect all edges in memory, then persist in a single transaction.
    let mut all_edges: Vec<GraphEdge> = Vec::new();

    // 1. Ref-derived edges (calls, references) for all files at once.
    all_edges.extend(derive_ref_edges(&all_refs, &nr, None));

    // 2. Import + impl edges per file (need per-file symbol/import lists).
    let files = store.list_files()?;
    for file in &files {
        if file.language.is_none() {
            continue;
        }
        let file_id = match store.get_file_id(&file.path)? {
            Some(id) => id,
            None => continue,
        };

        let file_symbols = store.get_file_symbols(file_id)?;
        if file_symbols.is_empty() {
            continue;
        }

        let imports = store.get_file_imports(file_id)?;
        all_edges.extend(derive_import_edges(&file_symbols, &imports, &nr));
        all_edges.extend(derive_impl_edges(&file_symbols, &nr));
    }

    // Persist: clear + bulk-insert in one transaction. The clear stays
    // inside the transaction so a crash between clear and commit cannot leave
    // the edge table permanently empty (which would trip the TUI empty-graph
    // guard until the next rebuild).
    store.begin_transaction()?;
    let result = (|| -> anyhow::Result<()> {
        store.clear_edges()?;
        store.upsert_edges_batch(&all_edges)
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
        "derive_and_store: {} edges ({} EXTRACTED, {} INFERRED) in {}ms",
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

/// Rebuild the semantic edge graph restricted to files of a single language.
///
/// Clears existing edges, then derives edges only for files whose detected
/// language matches `language`.  Used by `SymbolGraph::build_for_language`.
///
/// Like [`derive_and_store`], this loads all symbols and all refs once and
/// persists in a single transaction.
pub fn derive_and_store_for_language(store: &IndexStore, language: &str) -> Result<BuildResult> {
    let start = std::time::Instant::now();

    // Load all symbols ONCE for name resolution.
    let all_symbols = store.query_symbols(&crate::types::SymbolFilter::default())?;
    if all_symbols.is_empty() {
        store.clear_edges()?;
        return Ok(BuildResult {
            edges_total: 0,
            edges_extracted: 0,
            edges_inferred: 0,
            elapsed_ms: start.elapsed().as_millis() as u64,
        });
    }
    let nr = NameResolution::from_symbols(&all_symbols);

    // Load all refs ONCE.
    let all_refs = store.query_all_refs()?;

    // Collect file IDs matching the language filter.
    let files = store.list_files()?;
    let target_file_ids: Vec<i64> = {
        let mut ids = Vec::new();
        for file in &files {
            if file.language.as_deref() != Some(language) {
                continue;
            }
            if let Some(id) = store.get_file_id(&file.path)? {
                ids.push(id);
            }
        }
        ids
    };

    let target_set: std::collections::HashSet<i64> = target_file_ids.iter().copied().collect();

    let mut all_edges: Vec<GraphEdge> = Vec::new();

    // 1. Ref-derived edges, filtered to target-language files.
    for r in &all_refs {
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
    for &file_id in &target_file_ids {
        let file_symbols = store.get_file_symbols(file_id)?;
        if file_symbols.is_empty() {
            continue;
        }
        let imports = store.get_file_imports(file_id)?;
        all_edges.extend(derive_import_edges(&file_symbols, &imports, &nr));
        all_edges.extend(derive_impl_edges(&file_symbols, &nr));
    }

    // Persist: clear + bulk-insert in one transaction (same crash-safety
    // reasoning as `derive_and_store_for_language` above).
    store.begin_transaction()?;
    let result = (|| -> anyhow::Result<()> {
        store.clear_edges()?;
        store.upsert_edges_batch(&all_edges)
    })();
    if result.is_ok() {
        store.commit_transaction()?;
    } else {
        let _ = store.conn.execute_batch("ROLLBACK");
    }
    result?;

    let edges_extracted = store.edge_count_by_confidence_typed(Confidence::Extracted)? as usize;
    let edges_inferred = store.edge_count_by_confidence_typed(Confidence::Inferred)? as usize;

    Ok(BuildResult {
        edges_total: edges_extracted + edges_inferred,
        edges_extracted,
        edges_inferred,
        elapsed_ms: start.elapsed().as_millis() as u64,
    })
}
