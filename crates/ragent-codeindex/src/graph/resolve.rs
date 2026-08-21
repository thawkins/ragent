//! Cross-file symbol resolution.
//!
//! When a symbol reference (call, type use, import) names a symbol that
//! exists in a different file, the resolver determines which definition is
//! the most likely target.  Candidates are ranked by:
//!
//! 1. **Same file** — the definition is in the same file as the reference.
//! 2. **Same module** — the definition is in a file that shares a module
//!    path prefix (directory) with the reference.
//! 3. **Same language** — the definition is in a file with the same
//!    language as the reference.
//! 4. **Highest visibility** — `pub` > `pub(crate)` > `pub(super)` > private.
//! 5. **First match** — the first definition in database order.
//!
//! This resolution is deterministic and does not use an LLM or embeddings
//! (FR-001).  The resulting confidence tag is `EXTRACTED` for same-file
//! matches and `INFERRED` for cross-file matches (FR-002).

use crate::store::IndexStore;
use crate::types::{Symbol, SymbolFilter, SymbolKind, Visibility};
use anyhow::Result;

/// The outcome of a symbol resolution attempt.
#[derive(Debug, Clone)]
pub struct ResolvedSymbol {
    /// The resolved symbol's database ID.
    pub id: i64,
    /// The file ID of the resolved symbol.
    pub file_id: i64,
    /// Whether the resolution was same-file (`true` → `EXTRACTED`) or
    /// cross-file (`false` → `INFERRED`).
    pub same_file: bool,
}

/// Resolve a symbol name to the best matching symbol definition.
///
/// `source_file_id` is the file that contains the reference (the file
/// where the call/import/type-use appears).  If `None`, no same-file
/// preference is applied.
///
/// Returns `None` if no symbol with the given name exists in the index.
pub fn resolve_symbol(
    name: &str,
    source_file_id: Option<i64>,
    store: &IndexStore,
) -> Result<Option<ResolvedSymbol>> {
    let candidates = find_candidates(name, store)?;
    if candidates.is_empty() {
        return Ok(None);
    }
    let resolved = rank_candidates(name, source_file_id, &candidates, store)?;
    Ok(Some(resolved))
}

/// Resolve a symbol name and return *all* matching symbol IDs, sorted by
/// rank (best first).  This is useful when an edge should connect to every
/// candidate (e.g. overloaded functions, or names that appear in multiple
/// modules).
pub fn resolve_all_symbols(
    name: &str,
    source_file_id: Option<i64>,
    store: &IndexStore,
) -> Result<Vec<ResolvedSymbol>> {
    let candidates = find_candidates(name, store)?;
    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    // Sort all candidates by rank (best first).
    let mut ranked: Vec<((u8, u8, u8, u8, i64), &Symbol)> = candidates
        .iter()
        .map(|s| (compute_rank(s, source_file_id, store), s))
        .collect();
    ranked.sort_by(|a, b| a.0.cmp(&b.0));

    let result: Vec<ResolvedSymbol> = ranked
        .iter()
        .map(|(_, s)| {
            let same_file = source_file_id == Some(s.file_id);
            ResolvedSymbol {
                id: s.id,
                file_id: s.file_id,
                same_file,
            }
        })
        .collect();
    Ok(result)
}

/// Find all symbols whose name exactly matches `name`.
fn find_candidates(name: &str, store: &IndexStore) -> Result<Vec<Symbol>> {
    let filter = SymbolFilter {
        name: Some(name.to_string()),
        ..Default::default()
    };
    store.query_symbols(&filter)
}

/// Rank candidates and return the best one.
fn rank_candidates(
    _name: &str,
    source_file_id: Option<i64>,
    candidates: &[Symbol],
    store: &IndexStore,
) -> Result<ResolvedSymbol> {
    // Compute a rank for each candidate (lower is better).
    let mut best: Option<((u8, u8, u8, u8, i64), &Symbol)> = None;
    for sym in candidates {
        let rank = compute_rank(sym, source_file_id, store);
        match &best {
            None => best = Some((rank, sym)),
            Some((best_rank, _)) if rank < *best_rank => best = Some((rank, sym)),
            _ => {}
        }
    }

    let sym = best.map(|(_, s)| s).unwrap_or_else(|| &candidates[0]);
    let same_file = source_file_id == Some(sym.file_id);
    Ok(ResolvedSymbol {
        id: sym.id,
        file_id: sym.file_id,
        same_file,
    })
}

/// Compute a rank tuple for a candidate symbol.  Lower is better.
///
/// The rank is a tuple compared lexicographically:
/// 1. Same file? (0 = yes, 1 = no)
/// 2. Same module? (0 = yes, 1 = no, 2 = unknown)
/// 3. Same language? (0 = yes, 1 = no, 2 = unknown)
/// 4. Visibility (0=pub, 1=pub(crate), 2=pub(super), 3=private)
/// 5. Symbol ID (for stable tie-breaking)
fn compute_rank(
    sym: &Symbol,
    source_file_id: Option<i64>,
    store: &IndexStore,
) -> (u8, u8, u8, u8, i64) {
    // Same file?
    let same_file_rank: u8 = if source_file_id == Some(sym.file_id) {
        0
    } else {
        1
    };

    // Same module (directory)?
    let same_module_rank: u8 = if same_file_rank == 0 {
        0
    } else if let Some(src_id) = source_file_id {
        if let (Some(src_file), Some(sym_file)) = (
            store.get_file_by_id(src_id).ok().flatten(),
            store.get_file_by_id(sym.file_id).ok().flatten(),
        ) {
            if parent_dir(&src_file.path) == parent_dir(&sym_file.path) {
                0
            } else {
                1
            }
        } else {
            2
        }
    } else {
        2
    };

    // Same language?
    let same_language_rank: u8 = if let Some(src_id) = source_file_id {
        if let (Some(src_file), Some(sym_file)) = (
            store.get_file_by_id(src_id).ok().flatten(),
            store.get_file_by_id(sym.file_id).ok().flatten(),
        ) {
            if src_file.language == sym_file.language {
                0
            } else {
                1
            }
        } else {
            2
        }
    } else {
        2
    };

    // Visibility (lower is better).
    let visibility_rank: u8 = match sym.visibility {
        Visibility::Public => 0,
        Visibility::PubCrate => 1,
        Visibility::PubSuper => 2,
        Visibility::Private => 3,
    };

    (
        same_file_rank,
        same_module_rank,
        same_language_rank,
        visibility_rank,
        sym.id,
    )
}

/// Extract the parent directory from a file path.
fn parent_dir(path: &str) -> String {
    match path.rfind('/') {
        Some(pos) => path[..pos].to_string(),
        None => String::new(),
    }
}

/// Determine whether a symbol kind is a "definition" (not a reference or
/// import).  Only definitions are valid resolution targets.
#[must_use]
pub fn is_definition_kind(kind: SymbolKind) -> bool {
    !matches!(kind, SymbolKind::Import | SymbolKind::Unknown)
}
