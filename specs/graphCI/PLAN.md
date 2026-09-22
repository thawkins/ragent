# Implementation Plan — CodeIndex Graph Extension (graphCI)

This plan implements the `graphCI` spec. Work spans the `ragent-codeindex`,
`ragent-tools-extended`, and `ragent-tui` crates.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add `graph_edges` and `communities` SQLite tables with schema migration | FR-003, FR-025 | M | Critical | completed | — |
| T-002 | Define `GraphEdge`, `EdgeKind`, `Confidence` types in `types.rs` | FR-002, FR-004 | S | Critical | completed | — |
| T-003 | Create `graph/mod.rs` with `SymbolGraph` struct and public API | FR-004 | M | Critical | completed | T-001, T-002 |
| T-004 | Implement edge derivation from symbols + imports + refs (`graph/edges.rs`) | FR-001, FR-002, FR-009 | L | Critical | completed | T-003 |
| T-005 | Implement cross-file symbol resolution (`graph/resolve.rs`) | FR-001, FR-002 | M | Critical | completed | T-004 |
| T-006 | Wire edge building into `full_reindex` | FR-007 | M | High | completed | T-004 |
| T-007 | Wire edge updating into incremental `index_file` | FR-008 | M | High | completed | T-004 |
| T-008 | Implement BFS shortest-path traversal (`graph/traverse.rs`) | FR-012 | M | High | completed | T-003 |
| T-009 | Implement explain (node + incident edges) query (`graph/traverse.rs`) | FR-011 | M | High | completed | T-003 |
| T-010 | Implement community detection (`graph/communities.rs`) | FR-013, FR-019 | L | Medium | completed | T-004 |
| T-011 | Implement graph export to `graph.json` + `GRAPH_REPORT.md` (`graph/export.rs`) | FR-010, FR-020 | M | Medium | completed | T-004, T-010 |
| T-012 | Add `codeindex_explain` LLM tool in `ragent-tools-extended` | FR-011, FR-017 | M | High | completed | T-009 |
| T-013 | Add `codeindex_path` LLM tool in `ragent-tools-extended` | FR-012, FR-017 | M | High | completed | T-008 |
| T-014 | Add `codeindex_communities` LLM tool in `ragent-tools-extended` | FR-013, FR-017 | M | Medium | completed | T-010 |
| T-015 | Add `codeindex_godnodes` LLM tool in `ragent-tools-extended` | FR-014, FR-017 | M | Medium | completed | T-004 |
| T-016 | Add `/codeindex graph build` sub-command in TUI slash handler | FR-009, FR-021 | S | High | completed | T-004 |
| T-017 | Add `/codeindex graph export` sub-command in TUI | FR-010 | S | High | completed | T-011 |
| T-018 | Add `/codeindex explain <symbol>` sub-command in TUI | FR-011 | S | High | completed | T-012 |
| T-019 | Add `/codeindex path <A> <B>` sub-command in TUI | FR-012 | S | High | completed | T-013 |
| T-020 | Add `/codeindex communities` sub-command in TUI | FR-013 | S | Medium | completed | T-014 |
| T-021 | Add `/codeindex godnodes` sub-command in TUI | FR-014 | S | Medium | completed | T-015 |
| T-022 | Update `/codeindex help` text and usage string with new sub-commands | FR-005, FR-021, FR-022 | S | High | completed | T-016–T-021 |
| T-023 | Add busy-state fallback for graph tools (`codeindex_utils`) | FR-017 | S | High | completed | T-012 |
| T-024 | Add empty-graph guard message for graph sub-commands | FR-015 | S | High | completed | T-016 |
| T-025 | Add disabled-index guard for graph sub-commands | FR-016 | S | High | completed | T-016 |
| T-026 | Add optional `/codeindex graph lang <lang>` filter | FR-018 | S | Low | completed | T-016 |
| T-027 | Register new graph tools in `ragent-tools-extended` tool registry | FR-006 | S | High | completed | T-012, T-013, T-014, T-015 |
| T-028 | Verify backward compatibility: existing `/codeindex` sub-commands | FR-005, FR-021 | S | Critical | completed | T-022 |
| T-029 | Verify backward compatibility: existing `codeindex_*` tools | FR-006 | S | Critical | completed | T-027 |
| T-030 | Verify existing SQLite tables are read-only by graph layer | FR-025 | S | High | completed | T-004 |
## Task details

### T-001 — Add SQLite tables

In `crates/ragent-codeindex/src/store.rs`, bump `SCHEMA_VERSION` and add
`graph_edges` and `communities` tables to `init_schema`. Add `upsert_edge`,
`delete_edges_for_file`, `query_edges`, `edge_count` methods to
`IndexStore`. Ensure `ON DELETE CASCADE` from `symbols` so edge rows are
removed when symbols are deleted during reindex.

### T-002 — Define edge types

In `crates/ragent-codeindex/src/types.rs`, add:

- `EdgeKind` enum: `Calls`, `Imports`, `Inherits`, `References`, `MixesIn`, `Implements` with `Display`/`FromStr`.
- `Confidence` enum: `Extracted`, `Inferred`.
- `GraphEdge` struct: `source_sym: i64`, `target_sym: i64`, `kind: EdgeKind`, `confidence: Confidence`, `source_file: Option<i64>`, `line: Option<u32>`.

### T-003 — `SymbolGraph` struct

Create `crates/ragent-codeindex/src/graph/mod.rs` with a `SymbolGraph` struct
that holds a reference to `IndexStore` and exposes: `build()`,
`build_for_language(lang)`, `explain(name)`, `path(a, b)`, `communities()`,
`godnodes(n)`, `export_json()`, `export_report()`. All methods return
`Result` and use the non-blocking `try_lock` pattern where appropriate.

### T-004 — Edge derivation

Create `crates/ragent-codeindex/src/graph/edges.rs`. Implement
`derive_edges(store: &IndexStore) -> Vec<GraphEdge>` that:

1. Reads all symbols.
2. For each `SymbolRef` with `kind = "call"` → emit a `Calls` edge to the
   resolved `Symbol`.
3. For each `ImportEntry` → emit an `Imports` edge to the matching symbol.
4. For `type` references → emit a `References` edge.
5. For Rust `impl` blocks → emit `Implements`/`Inherits` edges.
6. Tag `EXTRACTED` for same-file, `INFERRED` for cross-file resolved.

### T-005 — Cross-file resolution

Create `crates/ragent-codeindex/src/graph/resolve.rs` with
`resolve_symbol(name, source_file_id, store) -> Option<i64>` that queries the
`symbols` table and ranks candidates by: same-file > same-module >
same-language > highest visibility > first match.

### T-006 — Wire into `full_reindex`

In `crates/ragent-codeindex/src/lib.rs`, after the symbol-extraction phase of
`full_reindex`, call `SymbolGraph::build()` and persist edges. Add edge
counts to `IndexResult` (new fields: `edges_extracted`, `edges_inferred`).

### T-007 — Wire into `index_file`

In the single-file path, after symbols are stored, delete edges where
`source_sym` or `target_sym` belongs to the file's symbols, then re-derive
edges for that file's symbols only.

### T-008 — Shortest path (BFS)

In `graph/traverse.rs`, implement `shortest_path(graph, source_id,
target_id) -> Option<Vec<i64>>` using BFS over an adjacency list built from
`graph_edges`. Return the sequence of symbol IDs forming the path.

### T-009 — Explain query

In `graph/traverse.rs`, implement `explain(graph, symbol_name) ->
ExplainResult` returning the symbol's metadata plus incoming and outgoing
edges (up to 50) with kind, confidence, and the connected symbol's name and
file.

### T-010 — Community detection

In `graph/communities.rs`, implement label-propagation community detection
(iterative, convergence-based) over the symbol graph. Assign each symbol a
community ID. Auto-label communities by the highest-degree member name.

### T-011 — Graph export

In `graph/export.rs`, implement:

- `to_json(graph) -> Value`: serialise nodes (symbols) and edges to a JSON
  object compatible with common visualisers.
- `to_report(graph) -> String`: generate a Markdown report with top
  communities, god nodes, and suggested questions.
- Write both to `.ragent/codeindex/graph.json` and
  `.ragent/codeindex/GRAPH_REPORT.md`.

### T-012–T-015 — LLM tools

In `crates/ragent-tools-extended/src/`, create
`codeindex_explain.rs`, `codeindex_path.rs`, `codeindex_communities.rs`,
`codeindex_godnodes.rs`. Each implements the `Tool` trait, uses
`codeindex:read` permission, checks for `code_index` availability, and uses
the `with_retry`/`busy_output` pattern from existing codeindex tools.

### T-016–T-021 — TUI sub-commands

In `crates/ragent-tui/src/app/slash.rs`, extend the existing `codeindex`
match arm to handle `graph`, `explain`, `path`, `communities`, `godnodes`
sub-commands. Each checks for an active `code_index` first (FR-016), then
checks for empty graph (FR-015), then delegates to the `SymbolGraph` API.

### T-022 — Update help text

Update the `/codeindex help` table and the usage string to include the new
sub-commands. Do not modify the existing rows.

### T-027 — Tool registration

In `crates/ragent-tools-extended/src/lib.rs`, register the four new tools
alongside the existing six `codeindex_*` tools in `register_default_tools`.

### T-028–T-029 — Backward compatibility verification

Confirm via manual testing that all pre-existing `/codeindex` sub-commands
and all six existing `codeindex_*` tools produce identical output before and
after the extension.

## Acceptance criteria

1. `/codeindex graph build` on the ragent codebase produces a non-zero edge
   count with both `EXTRACTED` and `INFERRED` edges.
2. `/codeindex explain CodeIndex` shows the node, its community, degree, and
   at least 5 connections.
3. `/codeindex path CodeIndex ParserRegistry` finds a path of ≤ 4 hops.
4. `/codeindex communities` lists at least 3 communities with labels.
5. `/codeindex godnodes` lists the top 10 symbols by degree.
6. `/codeindex graph export` writes both `graph.json` and `GRAPH_REPORT.md`.
7. All pre-existing `/codeindex` sub-commands work unchanged.
8. All pre-existing `codeindex_*` tools remain registered and callable.
9. The existing SQLite tables are not modified.
10. No new top-level slash command is added.