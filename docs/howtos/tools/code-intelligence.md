# Tools — Code Intelligence

Symbol-aware search, references, and graph queries over the automatic
codebase index (tree-sitter parsing, Tantivy FTS, semantic edge graph). All
codeindex tools are read-only and hardwired always-allowed.

| Tool | Description |
|------|-------------|
| `codeindex_search` | Search symbols by name/keyword. |
| `codeindex_symbols` | Query symbols with kind/file filters. |
| `codeindex_references` | Find all references to a symbol. |
| `codeindex_dependencies` | Query file import/dependent edges. |
| `codeindex_status` | Show index status and statistics. |
| `codeindex_reindex` | Trigger a full re-index. |
| `codeindex_explain` | Explain a graph node and its edges. |
| `codeindex_path` | Shortest path between two symbols. |
| `codeindex_communities` | Community detection over the graph. |
| `codeindex_godnodes` | Top-N most-connected symbols. |

**Use cases:** finding function definitions, tracing callers, dependency
analysis, code graph exploration.

**System instruction:** "MUST use codeindex instead of `grep` for code
symbol queries. Use `grep` only for arbitrary text patterns."

Tools that need the semantic graph (`codeindex_explain`, `codeindex_path`,
`codeindex_communities`, `codeindex_godnodes`) require the graph to be built
first via the `/codeindex graph build` TUI command; they return a
`codeindex_busy` response while the index is temporarily locked.

---

## codeindex_search

Search the codebase index for symbols, functions, types, and documentation.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `query` | string | yes | Symbol name, keyword, or phrase to find | `"parse_config"` |
| `kind` | enum | no | Filter by symbol kind: `function`, `struct`, `enum`, `trait`, `impl`, `const`, `static`, `type_alias`, `module`, `macro`, `field`, `variant`, `interface`, `class`, `method` | `"function"` |
| `language` | string | no | Filter by programming language | `"rust"`, `"python"`, `"typescript"` |
| `file_pattern` | string | no | Filter by file path substring | `"src/parser"`, `".rs"` |
| `max_results` | integer | no | Maximum results (max 100) | `20` |

**Example:**
```text
codeindex_search query="parse_config" kind="function"
```

---

## codeindex_symbols

Query symbols (functions, structs, enums, traits) with location, signature,
and documentation.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `name` | string | no | Case-insensitive leading-fragment match | `"parse"` |
| `kind` | enum | no | Symbol kind (same list as `codeindex_search`) | `"struct"` |
| `file_path` | string | no | Filter by file path substring | `"crates/ragent-config"` |
| `language` | string | no | Filter by language | `"rust"` |
| `visibility` | enum | no | `public`, `private`, or `crate` | `"public"` |
| `limit` | integer | no | Maximum results (max 200) | `50` |

---

## codeindex_references

Find all references to a symbol by name, grouped by file with reference kinds
(`call`, `type`, `field_access`).

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `symbol` | string | yes | Symbol name to find references for | `"SessionState"` |
| `limit` | integer | no | Maximum references (max 200) | `50` |

---

## codeindex_dependencies

Query file-level dependency edges from the index.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `path` | string | yes | Relative file path | `"crates/ragent-agent/src/lib.rs"` |
| `direction` | enum | no | `imports` (what this file uses, default) or `dependents` (what uses this file) | `"dependents"` |

---

## codeindex_status

Show whether the index is enabled, whether the FTS search index and semantic
edge graph are built or building, plus files indexed, symbols extracted,
languages, index size, and timestamps. No arguments required.

---

## codeindex_reindex

Trigger a full re-index of the codebase (scans all files, extracts symbols,
updates the search index; may take a while on large repositories). Safe to
call after bulk edits. No arguments required.

---

## codeindex_explain

Show a symbol's graph metadata (source file, line, community, degree) and up
to 50 incoming/outgoing edges with kind and confidence tags.

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `symbol` | string | yes | Name of the symbol to explain |

---

## codeindex_path

Find the shortest path (by hop count) between two symbols, displayed as
`A --kind--> B` hops with confidence tags.

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `from` | string | yes | Source symbol name |
| `to` | string | yes | Target symbol name |

---

## codeindex_communities

Run community detection over the code graph and display each community with
its auto-generated label and member count. No arguments required.

---

## codeindex_godnodes

Show the top-N most-connected symbols (highest degree) with source files and
edge counts.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `n` | integer | no | Maximum god-nodes returned (max 100) | `10` |

**Example:**
```text
codeindex_search query="parse_config" kind="function"
codeindex_references symbol="parse_config"
codeindex_path from="parse_config" to="App::run"
```
