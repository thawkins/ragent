---
status: draft
audit:
  - { time: 1787286091, from: "none", to: "draft", actor: "system" }
---
# Specification — CodeIndex Graph Extension (graphCI)

## Introduction

The [graphify](https://github.com/Graphify-Labs/graphify) plugin builds a
knowledge graph from source code using tree-sitter AST parsing: nodes are
concepts (functions, structs, traits, classes, etc.), edges are typed
relationships (calls, imports, inherits, references, mixes_in), communities
are detected via Leiden clustering, and the graph supports `explain`, `path`,
and `query` operations — all without an LLM or embeddings.

The existing `ragent-codeindex` crate already parses source code with
tree-sitter, stores symbols/imports/references in SQLite, provides a
tantivy full-text index, and exposes six LLM tools plus the `/codeindex`
slash command. However, it has no concept of a *typed edge* between symbols,
no community detection, and no graph-traversal queries (shortest path,
explain, scoped subgraph).

This specification defines an extension to `ragent-codeindex` that adds all
graphify capabilities **while retaining every existing capability and
interface** — the existing `/codeindex` sub-commands and the six
`codeindex_*` tools must continue to work unchanged. The extension adds new
sub-commands to `/codeindex` (no new slash commands) and new graph-traversal
tools, all powered by the same tree-sitter AST pipeline.

## Goals

1. Add typed semantic edges (`calls`, `imports`, `inherits`, `references`,
   `mixes_in`, `implements`) between indexed symbols, derived from the
   existing tree-sitter parse data (symbols, imports, references) plus
   lightweight cross-file symbol resolution — no LLM, no embeddings.
2. Add community detection (Leiden-style or label-propagation) over the
   symbol graph so the codebase can be partitioned into subsystems with
   auto-generated labels.
3. Add graph-traversal query operations: `explain <symbol>`, `path <A> <B>`,
   and `query <natural-language question>` (scoped subgraph).
4. Expose these operations through new sub-commands of the existing
   `/codeindex` slash command (`/codeindex graph`, `/codeindex explain`,
   `/codeindex path`, `/codeindex communities`, `/codeindex godnodes`) and
   through new LLM tools (`codeindex_explain`, `codeindex_path`,
   `codeindex_communities`, `codeindex_godnodes`).
5. Emit `graph.json` and `GRAPH_REPORT.md` artifacts on demand via
   `/codeindex graph export`.
6. Tag every edge as `EXTRACTED` (explicitly present in source) or `INFERRED`
   (resolved by the graph resolver) so users can distinguish direct reads
   from inferred connections.
7. Retain full backward compatibility: all existing `/codeindex`
   sub-commands, all six existing `codeindex_*` tools, the SQLite schema,
   and the tantivy FTS index continue to work without modification to their
   public interfaces.

## Requirements

### Ubiquitous requirements

**FR-001** — The system shall derive typed semantic edges between indexed
symbols from the existing tree-sitter parse output (`ParsedFile.symbols`,
`ParsedFile.imports`, `ParsedFile.references`) without invoking an LLM or
embedding model.

**FR-002** — Every semantic edge shall carry a `kind` field drawn from the
fixed set `{ calls, imports, inherits, references, mixes_in, implements }`
and a `confidence` tag of either `EXTRACTED` or `INFERRED`.

**FR-003** — The system shall persist semantic edges in a new `graph_edges`
table within the existing `ragent-codeindex` SQLite database, co-located in
the same `index.db` file, and shall rebuild edges during `full_reindex` and
incremental `index_file` operations.

**FR-004** — The system shall preserve the existing public API surface of
`CodeIndex` (all current `pub` methods) and add new `pub` methods for graph
operations; no existing method signature shall change in a breaking way.

**FR-005** �� The existing `/codeindex` sub-commands (`on`, `off`, `show`,
`lang`, `reindex`, `rebuild`, `help`) shall continue to function identically
after the extension is applied.

**FR-006** — The existing `codeindex_search`, `codeindex_symbols`,
`codeindex_references`, `codeindex_dependencies`, `codeindex_status`, and
`codeindex_reindex` tools shall remain registered and functional, with no
change to their names, parameter schemas, or permission categories.

### Event-driven requirements

**FR-007** — When a `full_reindex` completes, the system shall build (or
rebuild) the semantic edge graph and store the resulting edges in the
`graph_edges` table before returning the `IndexResult`.

**FR-008** — When a single file is indexed via `index_file` (incremental
update from the file watcher), the system shall update edges whose source or
target symbol resides in the newly indexed file, leaving all other edges
intact.

**FR-009** — When the user runs `/codeindex graph build`, the system shall
run the edge-derivation pass over the currently indexed symbols and report
the number of edges created, distinguishing `EXTRACTED` from `INFERRED`
counts.

**FR-010** — When the user runs `/codeindex graph export`, the system shall
write `graph.json` and `GRAPH_REPORT.md` into the project's
`.ragent/codeindex/` directory and print the output paths to the TUI.

**FR-011** — When the user runs `/codeindex explain <symbol>`, the system
shall display the symbol's node metadata (source file, line, community,
degree), its incoming and outgoing edges with kind and confidence tags, and
limit the output to the top 50 connections by degree.

**FR-012** — When the user runs `/codeindex path <symbolA> <symbolB>`, the
system shall compute and display the shortest path (by hop count) between the
two symbols in the graph, showing each hop as `A --kind--> B` with confidence
tags, or report that no path exists.

**FR-013** — When the user runs `/codeindex communities`, the system shall
run community detection over the symbol graph and display each detected
community with its auto-generated label and member count.

**FR-014** — When the user runs `/codeindex godnodes`, the system shall
display the top-N most-connected symbols (highest degree) with their names,
source files, and edge counts.

### State-driven requirements

**FR-015** — While the `graph_edges` table is empty (no graph has been
built), any graph query sub-command (`explain`, `path`, `communities`,
`godnodes`) shall print a message instructing the user to run
`/codeindex graph build` first.

**FR-016** — While the code index is disabled (not active), all graph
sub-commands shall print the same "not active" message as existing
`/codeindex` sub-commands and return without performing any work.

**FR-017** — While a background reindex is in progress and the SQLite store
lock is held, the new graph tools (`codeindex_explain`, `codeindex_path`,
etc.) shall return a non-blocking `codeindex_busy` response consistent with
the existing `codeindex_*` tools' busy behaviour.

### Optional requirements

**FR-018** — The system may support a `/codeindex graph lang <language>`
filter that restricts graph construction to symbols from a single language,
allowing per-language subgraph analysis.

**FR-019** — The system may support community auto-labelling by deriving a
label from the most frequent symbol-name token or the highest-degree node in
the community, without calling an LLM.

**FR-020** — The `graph.json` export may include node attributes
(`community`, `degree`, `kind`, `source_file`, `line`) and edge attributes
(`kind`, `confidence`) in a format compatible with common graph-visualisation
tools.

### Unwanted requirements

**FR-021** — The system shall not remove, rename, or alter the behaviour of
any existing `/codeindex` sub-command; new sub-commands are additive only.

**FR-022** — The system shall not add any new top-level slash command; all
new functionality is exposed as sub-commands of the existing `/codeindex`
command.

**FR-023** — The system shall not replace the existing tree-sitter AST
parsing pipeline; the graph extension builds on top of the existing
`LanguageParser` trait, `ParserRegistry`, and per-language parsers.

**FR-024** — The system shall not use embeddings, vector stores, or LLM
calls for code-graph construction; edge derivation is purely deterministic
from the parsed AST data.

**FR-025** — The system shall not alter the existing `indexed_files`,
`symbols`, `imports`, `symbol_refs`, or `file_deps` tables; the graph
extension uses a new `graph_edges` table and (optionally) a `communities`
table, and reads from the existing tables in a read-only manner.

## Architecture

### New modules in `ragent-codeindex`

| Module | Responsibility |
|--------|---------------|
| `graph/mod.rs` | Public graph API: `SymbolGraph`, edge types, traversal queries |
| `graph/edges.rs` | Edge derivation from symbols + imports + references; EXTRACTED/INFERRED tagging |
| `graph/resolve.rs` | Cross-file symbol resolution: match a `SymbolRef` to a `Symbol` definition |
| `graph/communities.rs` | Community detection (label propagation or Leiden) over the symbol graph |
| `graph/traverse.rs` | Shortest-path (BFS), explain (node + incident edges), scoped subgraph extraction |
| `graph/export.rs` | Serialize `graph.json` and `GRAPH_REPORT.md` |

### New SQLite tables

```sql
CREATE TABLE IF NOT EXISTS graph_edges (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    source_sym    INTEGER NOT NULL REFERENCES symbols(id) ON DELETE CASCADE,
    target_sym    INTEGER NOT NULL REFERENCES symbols(id) ON DELETE CASCADE,
    kind          TEXT NOT NULL,          -- calls|imports|inherits|references|mixes_in|implements
    confidence    TEXT NOT NULL,          -- EXTRACTED|INFERRED
    source_file   INTEGER REFERENCES indexed_files(id) ON DELETE CASCADE,
    line          INTEGER,
    UNIQUE(source_sym, target_sym, kind)
);
CREATE INDEX IF NOT EXISTS idx_edges_source ON graph_edges(source_sym);
CREATE INDEX IF NOT EXISTS idx_edges_target ON graph_edges(target_sym);
CREATE INDEX IF NOT EXISTS idx_edges_kind ON graph_edges(kind);

CREATE TABLE IF NOT EXISTS communities (
    sym_id        INTEGER PRIMARY KEY REFERENCES symbols(id) ON DELETE CASCADE,
    community     INTEGER NOT NULL,
    label         TEXT
);
CREATE INDEX IF NOT EXISTS idx_communities_community ON communities(community);
```

### New `/codeindex` sub-commands

| Sub-command | Description |
|-------------|-------------|
| `/codeindex graph build` | Build/rebuild the semantic edge graph |
| `/codeindex graph export` | Write `graph.json` and `GRAPH_REPORT.md` |
| `/codeindex graph lang <lang>` | (Optional) Restrict graph to one language |
| `/codeindex explain <symbol>` | Show a symbol's node + connections |
| `/codeindex path <A> <B>` | Shortest path between two symbols |
| `/codeindex communities` | List detected communities with labels |
| `/codeindex godnodes` | Top-N highest-degree symbols |

### New LLM tools

| Tool | Permission | Purpose |
|------|-----------|---------|
| `codeindex_explain` | `codeindex:read` | Explain a symbol's connections |
| `codeindex_path` | `codeindex:read` | Shortest path between two symbols |
| `codeindex_communities` | `codeindex:read` | List communities |
| `codeindex_godnodes` | `codeindex:read` | List high-degree hub symbols |

## Edge derivation rules

| Edge kind | Source data | Confidence |
|-----------|-------------|------------|
| `calls` | `symbol_refs` with `kind = "call"` resolved to a `Symbol` definition | `EXTRACTED` if same-file; `INFERRED` if cross-file resolved by name match |
| `imports` | `imports` table entry → matching `Symbol` in the target module | `EXTRACTED` |
| `inherits` | Tree-sitter `base_class` / `superclass` / `impl` target node → `Symbol` | `EXTRACTED` if same-file; `INFERRED` if cross-file |
| `references` | `symbol_refs` with `kind = "type"` or `kind = "field_access"` | `EXTRACTED` if same-file; `INFERRED` if cross-file |
| `mixes_in` | (Python/TS mixin) detected via `kind = "mixin"` reference | `INFERRED` |
| `implements` | (Java/TS) `implements` clause → `Interface` symbol | `EXTRACTED` if same-file; `INFERRED` if cross-file |

Cross-file resolution uses name matching against the `symbols` table: a
`SymbolRef.symbol_name` is matched to a `Symbol.name` (or
`qualified_name`) in a different file. When multiple candidates exist, the
resolver prefers same-module, then same-language, then highest visibility.

## Out of scope

- Parsing non-code assets (docs, PDFs, images, video) — graphify's "beyond
  code" semantic pass is excluded; this spec covers code only.
- LLM-based community labelling — auto-labelling (FR-019) is deterministic
  only.
- Embedding-based semantic search — the existing tantivy FTS is sufficient.
- A `graph.html` interactive visualiser — the spec covers `graph.json` and
  `GRAPH_REPORT.md`; an HTML renderer is a future enhancement.
- Modifying the `file_deps` table semantics — file-level dependencies remain
  as-is; `graph_edges` is symbol-level.