# ragent Code Index — Design Document

## 1. Overview

**ragent Code Index** is a built-in codebase indexing, search, and retrieval system for ragent. It provides agents with deep, structured understanding of the codebase they operate within — beyond simple text search (grep) or single-file LSP queries.

The system is **fully embedded** — every component compiles into the ragent binary with zero external tools, services, or runtime installations required:

- **tree-sitter** (compiled-in C grammars) for language-aware AST parsing
- **SQLite** (bundled via `rusqlite`) for persistent index storage
- **tantivy** (pure Rust) for full-text search
- **notify** (pure Rust) for real-time filesystem watching

### Design Principles

- **Zero external dependencies**: No databases to install, no servers to run, no cloud services, no PATH requirements. Everything ships inside the ragent binary.
- **User-controllable**: The indexer can be enabled or disabled at any time via the `/codeindex on|off` slash command. State is persisted in ragent config so it survives restarts.
- **Non-intrusive**: When disabled, the code index has zero CPU/memory/disk overhead. When enabled, background indexing yields to other work and respects ragent's existing concurrency semaphores.

### Goals

- **Structured code understanding**: Extract functions, structs, classes, traits, enums, imports, and their relationships — not just text lines
- **Incremental indexing**: Only re-process files that changed, using content hashing
- **Background updates**: File watcher detects changes and queues re-indexing automatically
- **Fast search**: Sub-100ms symbol lookup and full-text code search across large codebases
- **Language-agnostic**: Support multiple languages via compiled-in tree-sitter grammars (Rust first, then Python, TypeScript, Go, C/C++, Java)
- **Agent-native**: Exposed as ragent tools that agents can call naturally during conversations
- **Self-contained**: No external databases, no cloud services — everything runs locally in a single SQLite file + tantivy directory

### Non-Goals

- Replacing LSP for real-time editing feedback (LSP remains for hover/diagnostics)
- Embedding-based semantic search (potential future extension, not MVP)
- Cross-repository indexing (scoped to one project at a time)
- Requiring any external tool installation (everything is compiled in)

---

## 2. Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        ragent-code crate                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐   ┌──────────────┐   ┌────────────────────┐  │
│  │ File Scanner  │──▶│   Parser     │──▶│  Symbol Extractor  │  │
│  │ (ignore crate)│   │ (tree-sitter)│   │  (per-language)    │  │
│  └──────┬───────┘   └──────────────┘   └────────┬───────────┘  │
│         │                                         │              │
│         │  ┌──────────────┐                       ▼              │
│         │  │ File Watcher  │            ┌──────────────────┐    │
│         │  │ (notify crate)│───queue───▶│ Background Worker│    │
│         │  └──────────────┘            │ (tokio task)     │    │
│         │                               └────────┬─────────┘    │
│         ▼                                         ▼              │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                    Index Store (SQLite)                    │   │
│  │  ┌────────────┐ ┌─────────┐ ┌─────────┐ ┌────────────┐  │   │
│  │  │indexed_files│ │ symbols │ │ imports │ │ references │  │   │
│  │  └────────────┘ └─────────┘ └─────────┘ └────────────┘  │   │
│  └──────────────────────────────────────────────────────────┘   │
│         │                                                        │
│         ▼                                                        │
│  ┌──────────────────┐   ┌──────────────────────────────────┐   │
│  │ Tantivy FTS Index │   │       Tool Interface             │   │
│  │ (full-text search)│   │  codeindex_search                │   │
│  └──────────────────┘   │  codeindex_symbols                │   │
│                          │  codeindex_references             │   │
│                          │  codeindex_dependencies           │   │
│                          │  codeindex_status                 │   │
│                          │  codeindex_reindex                │   │
│                          └──────────────────────────────────┘   │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Component Overview

| Component | Crate / Module | Purpose |
|-----------|----------------|---------|
| **File Scanner** | `ragent-code::scanner` | Walk directory trees, respect `.gitignore`, compute content hashes |
| **File Watcher** | `ragent-code::watcher` | Real-time filesystem change detection via `notify` crate |
| **Parser** | `ragent-code::parser` | Tree-sitter AST parsing with per-language grammar support |
| **Symbol Extractor** | `ragent-code::parser::*` | Per-language AST walkers extract symbols, imports, and references inline |
| **Index Store** | `ragent-code::store` | SQLite persistence for files, symbols, imports, references |
| **Search Engine** | `ragent-code::search` | Tantivy full-text index + structured SQLite queries |
| **Tree Cache** | `ragent-code::tree_cache` | LRU cache of tree-sitter parse trees for incremental re-parsing |
| **Background Worker** | `ragent-code::worker` | Async indexing worker with debounce, dedup, and batching |
| **Tool Interface** | `ragent-core::tool::codeindex_*` | Agent-facing tools registered in the ToolRegistry |

---

## 3. Home Crate: `ragent-code`

The existing `ragent-code` workspace crate (currently an empty stub) becomes the home for all indexing logic. This keeps the core crate focused and allows independent compilation/testing.

```
crates/ragent-code/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public API: CodeIndex struct, full_reindex(), search, status
│   ├── scanner.rs          # File discovery, content hashing, language detection
│   ├── watcher.rs          # Filesystem change detection via notify crate
│   ├── worker.rs           # Background indexing worker with debounce and batching
│   ├── store.rs            # SQLite index schema, CRUD operations, transaction control
│   ├── search.rs           # Tantivy FTS index: add, remove, batch_update, search
│   ├── tree_cache.rs       # LRU tree cache for incremental tree-sitter parsing
│   ├── types.rs            # Shared types (Symbol, SymbolKind, FileEntry, etc.)
│   └── parser/
│       ├── mod.rs          # LanguageParser trait, ParserRegistry dispatcher, ParsedFile
│       ├── rust.rs         # Rust: functions, structs, enums, traits, impls, use, refs
│       ├── python.rs       # Python: functions, classes, imports, decorators
│       ├── typescript.rs   # TypeScript/JavaScript: functions, classes, interfaces, imports
│       ├── go.rs           # Go: functions, structs, interfaces, imports, methods
│       ├── c_cpp.rs        # C/C++: functions, structs, enums, classes, includes
│       └── java.rs         # Java: classes, interfaces, enums, methods, imports
├── tests/
│   ├── test_codeindex.rs               # Core CodeIndex integration tests
│   ├── test_parse_store_integration.rs # Parse → store pipeline tests
│   ├── test_scan_store_integration.rs  # Scan → store pipeline tests
│   ├── test_m4_integration.rs          # Watcher + worker integration tests
│   └── test_fts_diag.rs               # FTS diagnostic tests
└── benches/
    └── bench_file_ops.rs
```

---

## 4. Data Model

### 4.1 Indexed Files

Tracks every file in the project with its content hash for change detection.

```sql
CREATE TABLE indexed_files (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    path        TEXT NOT NULL UNIQUE,      -- relative to project root
    content_hash TEXT NOT NULL,            -- blake3 hash of file contents
    byte_size   INTEGER NOT NULL,
    language    TEXT,                       -- detected language (rust, python, etc.)
    last_indexed TEXT NOT NULL,            -- ISO 8601 UTC timestamp
    mtime_ns    INTEGER NOT NULL,         -- file mtime in nanoseconds
    line_count  INTEGER NOT NULL DEFAULT 0,
    symbol_count INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_files_path ON indexed_files(path);
CREATE INDEX idx_files_language ON indexed_files(language);
CREATE INDEX idx_files_hash ON indexed_files(content_hash);
```

### 4.2 Symbols

Every extracted code symbol — functions, structs, traits, enums, constants, modules, classes, methods.

```sql
CREATE TABLE symbols (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id         INTEGER NOT NULL REFERENCES indexed_files(id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    qualified_name  TEXT,                   -- e.g., "module::Struct::method"
    kind            TEXT NOT NULL,          -- fn, struct, enum, trait, impl, const,
                                           -- mod, class, method, interface, type_alias
    visibility      TEXT,                   -- pub, pub(crate), pub(super), private
    start_line      INTEGER NOT NULL,
    end_line        INTEGER NOT NULL,
    start_col       INTEGER NOT NULL,
    end_col         INTEGER NOT NULL,
    parent_id       INTEGER REFERENCES symbols(id) ON DELETE CASCADE,
    signature       TEXT,                   -- function signature / type definition
    doc_comment     TEXT,                   -- extracted doc comments
    body_hash       TEXT                    -- blake3 hash of symbol body for diff detection
);

CREATE INDEX idx_symbols_name ON symbols(name);
CREATE INDEX idx_symbols_kind ON symbols(kind);
CREATE INDEX idx_symbols_file ON symbols(file_id);
CREATE INDEX idx_symbols_parent ON symbols(parent_id);
CREATE INDEX idx_symbols_qualified ON symbols(qualified_name);
```

### 4.3 Imports

Tracks use/import/require/include statements for dependency analysis.

```sql
CREATE TABLE imports (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    file_id         INTEGER NOT NULL REFERENCES indexed_files(id) ON DELETE CASCADE,
    imported_name   TEXT NOT NULL,          -- what is imported (e.g., "std::collections::HashMap")
    source_module   TEXT,                   -- where it's imported from
    alias           TEXT,                   -- optional alias (as X)
    line            INTEGER NOT NULL,
    kind            TEXT NOT NULL           -- use, mod, extern_crate, import, require, include
);

CREATE INDEX idx_imports_file ON imports(file_id);
CREATE INDEX idx_imports_name ON imports(imported_name);
CREATE INDEX idx_imports_source ON imports(source_module);
```

### 4.4 References (Cross-file Symbol Usage)

Tracks where symbols are referenced across the codebase.

```sql
CREATE TABLE symbol_refs (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    symbol_name TEXT NOT NULL,              -- name of the referenced symbol
    file_id     INTEGER NOT NULL REFERENCES indexed_files(id) ON DELETE CASCADE,
    line        INTEGER NOT NULL,
    col         INTEGER NOT NULL,
    kind        TEXT NOT NULL               -- call, type_ref, field_access, trait_bound, macro_use
);

CREATE INDEX idx_refs_symbol ON symbol_refs(symbol_name);
CREATE INDEX idx_refs_file ON symbol_refs(file_id);
```

### 4.5 File Dependencies

Derived from imports — which files depend on which other files.

```sql
CREATE TABLE file_deps (
    source_file_id  INTEGER NOT NULL REFERENCES indexed_files(id) ON DELETE CASCADE,
    target_path     TEXT NOT NULL,          -- resolved target file path
    kind            TEXT NOT NULL,          -- use, mod, import, require, include
    PRIMARY KEY (source_file_id, target_path, kind)
);

CREATE INDEX idx_deps_source ON file_deps(source_file_id);
CREATE INDEX idx_deps_target ON file_deps(target_path);
```

---

## 5. Symbol Extraction via Tree-sitter

### 5.1 Language Detection

File extension → language mapping:

| Extension | Language | Tree-sitter Grammar |
|-----------|----------|---------------------|
| `.rs` | Rust | `tree-sitter-rust` |
| `.py` | Python | `tree-sitter-python` |
| `.js`, `.jsx` | JavaScript | `tree-sitter-javascript` |
| `.ts`, `.tsx` | TypeScript | `tree-sitter-typescript` |
| `.go` | Go | `tree-sitter-go` |
| `.c`, `.h` | C | `tree-sitter-c` |
| `.cpp`, `.hpp`, `.cc` | C++ | `tree-sitter-cpp` |
| `.java` | Java | `tree-sitter-java` |
| `.tf`, `.tfvars` | Terraform (HCL) | `tree-sitter-hcl` |
| `.scad` | OpenSCAD | `tree-sitter-openscad-ng` |
| `.cmake`, `CMakeLists.txt` | CMake | `tree-sitter-cmake` |
| `.gradle` | Gradle (Groovy DSL) | `tree-sitter-groovy` |
| `.gradle.kts` | Gradle (Kotlin DSL) | `tree-sitter-kotlin-ng` |
| `pom.xml` | Maven (XML) | `tree-sitter-xml` |
| `.toml` | TOML | `tree-sitter-toml` |
| `.json` | JSON | `tree-sitter-json` |
| `.yaml`, `.yml` | YAML | `tree-sitter-yaml` |
| `.md` | Markdown | `tree-sitter-markdown` |

### 5.2 Per-Language Query Strategy

Each language module defines tree-sitter queries (S-expression patterns) to extract symbols. Example for Rust:

```scheme
;; Functions
(function_item
  name: (identifier) @fn.name
  parameters: (parameters) @fn.params
  return_type: (_)? @fn.return_type
  body: (block) @fn.body) @fn.def

;; Structs
(struct_item
  name: (type_identifier) @struct.name
  body: (field_declaration_list)? @struct.body) @struct.def

;; Enums
(enum_item
  name: (type_identifier) @enum.name
  body: (enum_variant_list) @enum.body) @enum.def

;; Traits
(trait_item
  name: (type_identifier) @trait.name
  body: (declaration_list) @trait.body) @trait.def

;; Impl blocks
(impl_item
  type: (_) @impl.type
  trait: (_)? @impl.trait
  body: (declaration_list) @impl.body) @impl.def

;; Use statements
(use_declaration
  argument: (_) @use.path) @use.def

;; Constants
(const_item
  name: (identifier) @const.name
  type: (_) @const.type) @const.def

;; Type aliases
(type_item
  name: (type_identifier) @type.name
  type: (_) @type.value) @type.def

;; Macros
(macro_definition
  name: (identifier) @macro.name) @macro.def
```

### 5.3 Symbol Kind Taxonomy

```rust
pub enum SymbolKind {
    Function,
    Method,
    Struct,
    Class,
    Enum,
    EnumVariant,
    Trait,
    Interface,
    Impl,
    Module,
    Constant,
    Static,
    TypeAlias,
    Field,
    Import,
    Macro,
    Test,
    Unknown,
}
```

### 5.4 Visibility Detection

For Rust, visibility is determined by checking for `visibility_modifier` nodes:
- `pub` → `Public`
- `pub(crate)` → `PubCrate`
- `pub(super)` → `PubSuper`
- No modifier → `Private`

Other languages have analogous patterns (Python `_`/`__` prefixes, TypeScript `export`, Java `public`/`private`/`protected`).

---

## 6. Incremental Indexing Strategy

### 6.1 Change Detection

```
For each file in project:
  1. Compute blake3 hash of file contents
  2. Compare with stored hash in indexed_files table
  3. If hash differs OR file not in index:
     → Queue for re-indexing
  4. If file in index but not on disk:
     → Queue for removal
```

### 6.2 Indexing Pipeline

The `full_reindex()` method processes changed files in a **3-phase chunked pipeline** to minimize lock contention and I/O overhead:

```
File Change Detected
        │
        ▼
  ┌─────────────┐
  │ Scan & Diff  │  Compute stale files (hash comparison)
  │ apply_diff() │  Update file entries in SQLite (single transaction)
  └──────┬──────┘
         │
         ▼
  ┌─────────────────────────────────────────────────┐
  │  For each chunk of 20 files:                     │
  │                                                  │
  │  Phase 1: PARSE (no locks held)                  │
  │  ┌─────────────┐                                 │
  │  │ Read files   │  Read from disk                 │
  │  │ Parse ASTs   │  tree-sitter (CPU-heavy)        │
  │  │ Extract syms │  Symbols, imports, references   │
  │  └──────┬──────┘                                 │
  │         │                                        │
  │  Phase 2: STORE (store lock, single transaction) │
  │  ┌─────────────┐                                 │
  │  │ BEGIN        │  One SQLite transaction          │
  │  │ upsert_*()   │  All symbols/imports/refs        │
  │  │ COMMIT       │  Single disk sync per chunk      │
  │  └──────┬──────┘                                 │
  │         │                                        │
  │  Phase 3: FTS (fts lock, single commit)          │
  │  ┌─────────────┐                                 │
  │  │ batch_update │  Remove old + add new symbols    │
  │  │ writer.commit│  Single tantivy commit           │
  │  └──────┬──────┘                                 │
  │         │                                        │
  │  ┌──────┴──────┐                                 │
  │  │ 5ms yield   │  Let TUI event loop acquire locks│
  │  └─────────────┘                                 │
  └─────────────────────────────────────────────────┘
```

This architecture ensures that the CPU-heavy parsing work happens entirely outside any lock, and the I/O-heavy store/FTS writes are batched into single transactions per chunk.

### 6.3 Batching & Debouncing

- File watcher events are debounced with a **500ms** quiet period
- Changes are batched into groups of up to **50 files** per transaction
- A full re-index processes files in **chunks of 20** with a 5ms yield between chunks to avoid starving the TUI event loop
- Each chunk uses a **single SQLite transaction** (BEGIN/COMMIT) for all upsert operations, reducing disk syncs from ~12,000 to ~20 for a 400-file project
- Each chunk uses a **single tantivy writer + commit** via `batch_update()`, reducing FTS commits from ~800 to ~20
- Lock acquisitions are reduced from 2×N (per file) to 2×(N/chunk_size) per chunk
- The background worker respects the existing `MAX_CONCURRENT_TOOLS` semaphore

### 6.4 Tree-sitter Incremental Parsing

For files that change frequently (e.g., during active editing), we cache the previous `tree-sitter::Tree` and use incremental re-parsing:

```rust
// On file change:
if let Some(old_tree) = cache.get(path) {
    old_tree.edit(&input_edit);
    let new_tree = parser.parse(new_source, Some(&old_tree))?;
    cache.insert(path, new_tree);
} else {
    let tree = parser.parse(source, None)?;
    cache.insert(path, tree);
}
```

The tree cache is bounded to **1000 entries** (LRU eviction) to limit memory usage.

---

## 7. File Watching

### 7.1 Watcher Design

Uses the `notify` crate (v7) in **debounced mode**:

```rust
pub struct CodeWatcher {
    watcher: RecommendedWatcher,
    change_tx: mpsc::Sender<WatchEvent>,
}

pub enum WatchEvent {
    FileChanged(PathBuf),
    FileCreated(PathBuf),
    FileDeleted(PathBuf),
    FileRenamed { from: PathBuf, to: PathBuf },
}
```

### 7.2 Ignored Paths

Inherits from the `ignore` crate (same as ripgrep) plus hardcoded exclusions:

- `.git/`, `.hg/`, `.svn/`
- `target/`, `node_modules/`, `__pycache__/`, `dist/`, `build/`
- `.ragent/` (ragent's own data directory)
- Binary files (detected by extension + content sniffing)
- Files > 1 MB (configurable threshold)

### 7.3 Lifecycle

```
App Start
    │
    ├─ Load existing index from SQLite
    ├─ Diff filesystem vs index (incremental scan)
    ├─ Queue changed files for re-indexing
    ├─ Start file watcher on project root
    └─ Start background worker
         │
         ├─ Process initial queue (changed files)
         └─ Loop: await watcher events → debounce → batch → index
```

---

## 8. Search Engine

### 8.1 Dual Search Strategy

**Structured queries** (SQLite) for precise lookups:
- Find symbol by exact name
- List all symbols of a specific kind
- Find all functions in a file
- Query import chains
- Dependency graph traversal

**Full-text search** (tantivy) for fuzzy/natural language queries:
- "authentication middleware" → finds `auth_middleware()`, `AuthMiddleware` struct
- "parse config" → finds `parse_config()`, `ConfigParser`, etc.
- Supports prefix matching, fuzzy matching, and boolean operators

### 8.2 Tantivy Index Schema

```rust
let mut schema_builder = Schema::builder();
schema_builder.add_text_field("name", TEXT | STORED);
schema_builder.add_text_field("qualified_name", TEXT | STORED);
schema_builder.add_text_field("kind", STRING | STORED);
schema_builder.add_text_field("file_path", STRING | STORED);
schema_builder.add_text_field("signature", TEXT | STORED);
schema_builder.add_text_field("doc_comment", TEXT | STORED);
schema_builder.add_text_field("body_snippet", TEXT);  // first 500 chars of body
schema_builder.add_i64_field("start_line", INDEXED | STORED);
schema_builder.add_i64_field("end_line", STORED);
```

### 8.3 Search Ranking

Results are ranked by a composite score:
1. **Exact name match** (highest weight: 10.0)
2. **Qualified name match** (weight: 5.0)
3. **Signature match** (weight: 3.0)
4. **Doc comment match** (weight: 2.0)
5. **Body snippet match** (weight: 1.0)

Public symbols get a 1.5x boost. Test functions get a 0.5x penalty.

---

## 9. Tool Interface

Six new tools are registered in the `ToolRegistry`, all with permission `"codeindex:read"` (except `codeindex_reindex` which is `"codeindex:write"`).

### 9.1 `codeindex_search`

Search the codebase for symbols, code patterns, or natural language queries.

```json
{
  "name": "codeindex_search",
  "parameters": {
    "query": "string (required) — search query",
    "kind": "string (optional) — filter by symbol kind: fn, struct, enum, trait, class, etc.",
    "language": "string (optional) — filter by language: rust, python, typescript, etc.",
    "file_pattern": "string (optional) — glob pattern to restrict search scope",
    "max_results": "integer (optional, default: 20) — maximum results to return",
    "include_body": "boolean (optional, default: false) — include symbol body/implementation"
  }
}
```

**Example output:**
```
Found 3 symbols matching "detect_provider":

1. fn detect_provider (public)
   File: crates/ragent-tui/src/app.rs:882-1017
   Signature: pub fn detect_provider(storage: &Storage) -> Option<ConfiguredProvider>
   Doc: Detects the configured provider from environment, database, or auto-discovery.

2. fn test_detect_provider_disabled_flag_skips_provider
   File: crates/ragent-tui/tests/test_provider_detection.rs:85-104
   Kind: test

3. fn test_detect_provider_preferred_from_db
   File: crates/ragent-tui/tests/test_provider_detection.rs:35-52
   Kind: test
```

### 9.2 `codeindex_symbols`

List all symbols defined in a file or directory.

```json
{
  "name": "codeindex_symbols",
  "parameters": {
    "path": "string (required) — file or directory path",
    "kind": "string (optional) — filter by symbol kind",
    "recursive": "boolean (optional, default: false) — recurse into subdirectories",
    "depth": "integer (optional, default: 1) — nesting depth (0 = top-level only)"
  }
}
```

### 9.3 `codeindex_references`

Find all references to a symbol across the codebase.

```json
{
  "name": "codeindex_references",
  "parameters": {
    "symbol": "string (required) — symbol name to find references for",
    "kind": "string (optional) — reference kind: call, type_ref, field_access",
    "max_results": "integer (optional, default: 50)"
  }
}
```

### 9.4 `codeindex_dependencies`

Show dependency relationships between files.

```json
{
  "name": "codeindex_dependencies",
  "parameters": {
    "path": "string (required) — file path to analyze",
    "direction": "string (optional, default: 'both') — 'imports' (what this file uses), 'importers' (what uses this file), or 'both'",
    "depth": "integer (optional, default: 1) — depth of dependency traversal"
  }
}
```

### 9.5 `codeindex_status`

Show index status and statistics.

```json
{
  "name": "codeindex_status",
  "parameters": {}
}
```

**Example output:**
```
Code Index Status
─────────────────
Project: /home/user/Projects/ragent
Index location: .ragent/codeindex.db

Files indexed: 247 / 253 (6 binary/excluded)
Symbols extracted: 4,821
  Functions: 1,203  Structs: 342  Enums: 89  Traits: 47  Impls: 567
  Methods: 1,892  Constants: 201  Modules: 134  Macros: 56  Tests: 290

Languages: Rust (198), TOML (22), Markdown (15), JSON (8), YAML (4)

Last full index: 2026-04-13T04:30:00Z (12 min ago)
Background watcher: active (3 files queued)
Index size: 2.4 MB (SQLite) + 1.1 MB (FTS)
```

### 9.6 `codeindex_reindex`

Trigger a full or partial reindex.

```json
{
  "name": "codeindex_reindex",
  "parameters": {
    "path": "string (optional) — specific file/directory to reindex, or omit for full reindex",
    "force": "boolean (optional, default: false) — force reindex even if hashes match"
  }
}
```

### 9.7 Disabled Behavior — Graceful Fallback

When the code index is disabled (via `/codeindex off` or config), all six `codeindex_*` tools remain registered in the `ToolRegistry` but return a structured "not available" error. This is **intentional** — the tools must stay visible so the LLM knows they exist and can be enabled, but the error message explicitly guides the LLM to fall back to alternative tools.

**Error response when disabled:**

```
[CodeIndex Not Available]

The code index is currently disabled for this project.
Fall back to alternative tools:
  - Use `grep` or `search` for text-based code search
  - Use `glob` for file discovery
  - Use `lsp_symbols`, `lsp_definition`, `lsp_references` for code intelligence (if LSP is connected)
  - Use `read` to read specific files

To enable the code index, the user can run: /codeindex on
```

The error is returned as a normal `ToolOutput` (not an exception) with `metadata: {"status": "disabled", "fallback_tools": ["grep", "search", "glob", "lsp_symbols", "lsp_definition", "lsp_references", "read"]}`. This allows the LLM to parse the fallback tool list programmatically.

---

## 10. System Prompt Integration

When the code index is enabled and active, a guidance section is injected into the LLM system prompt — following the same pattern used for LSP guidance and team coordination. This section tells the LLM:

1. That the code index is available and what it provides
2. When to prefer it over grep/LSP
3. How to interpret its output

### 10.1 Injection Point

The guidance is injected in `session/processor.rs` after the LSP guidance section, conditionally:

```rust
// Inject code index guidance when the index is active.
if let Some(code_index) = &self.code_index {
    let ci_guidance = build_codeindex_guidance_section(code_index).await;
    system_prompt.push_str(&ci_guidance);
}
```

When the code index is `None` (disabled), no guidance is injected — the LLM simply won't know about the code index tools and won't attempt to use them. If the LLM does call a `codeindex_*` tool (e.g., from conversation history or hallucination), the disabled error response (§9.7) guides it to alternatives.

### 10.2 Guidance Text

```
## Code Intelligence — Code Index

A **code index** is active for this project, providing fast structured search across
the entire codebase. The index tracks {file_count} files and {symbol_count} symbols
(functions, structs, enums, traits, classes, methods, imports, and their relationships).

**Indexed languages:** {languages}

### When to use Code Index tools

| Task | Preferred Tool | Why |
|------|---------------|-----|
| Find a function/struct/trait by name | `codeindex_search` | Instant structured lookup; no regex needed |
| List all symbols in a file | `codeindex_symbols` | Shows full symbol tree with nesting and types |
| Find all callers/users of a symbol | `codeindex_references` | Cross-file reference tracking |
| Understand file dependencies | `codeindex_dependencies` | Import/export graph traversal |
| Search for a code *pattern* (regex) | `grep` | Code index is name-based, not pattern-based |
| Read file contents | `read` | Code index stores metadata, not full file text |
| Get type info / hover docs | `lsp_hover` | LSP has richer type inference |

### Preference order for code exploration

1. **`codeindex_search`** — for finding symbols by name or concept (fastest, project-wide)
2. **`lsp_*` tools** — for type info, hover docs, go-to-definition on a specific location
3. **`grep`** — for raw text/regex pattern matching
4. **`glob`** — for finding files by name pattern
5. **`read`** — for reading specific file contents

### Tips

- `codeindex_search` supports natural-language-style queries: "authentication middleware",
  "error handling", "database connection". It searches names, signatures, and doc comments.
- Use `kind` filter to narrow results: `codeindex_search(query: "parse", kind: "fn")` finds
  only functions, not structs or types.
- If `codeindex_search` returns no results, fall back to `grep` — the symbol may not be
  indexed (e.g., in a language not yet supported, or in a generated file).
- Use `codeindex_symbols(path: "src/lib.rs")` to get an overview of a file before reading it.
```

### 10.3 Dynamic Content

The guidance text includes dynamic placeholders filled at prompt-build time:

| Placeholder | Source | Example |
|-------------|--------|---------|
| `{file_count}` | `code_index.status().files_indexed` | `247` |
| `{symbol_count}` | `code_index.status().total_symbols` | `4,821` |
| `{languages}` | `code_index.status().languages` | `Rust (198), Python (12), TypeScript (8)` |

This gives the LLM accurate context about what the index covers, helping it decide when to use code index tools vs. falling back to grep/LSP.

### 10.4 No-Index Scenario

When the code index is disabled or not yet built:

- **No system prompt guidance is injected** — the LLM doesn't know about code index tools
- **Tools remain registered** but return the "not available" error (§9.7)
- **The LLM naturally falls back** to its default tool preference (grep → glob → read)
- **If the LLM encounters the disabled error**, the error text explicitly lists fallback tools

This ensures a seamless experience whether the code index is on or off.

---

## 11. Slash Command Interface

The code index is controlled by the user via a unified `/codeindex` slash command with subcommands. This provides a single, discoverable entry point for all index operations.

### 10.1 Command Reference

#### `/codeindex on`

Enables the code index for the current project. Persists the setting in the ragent config (`codeindex.enabled = true`). If the index does not yet exist, triggers an initial full index in the background. If the index is already active, reports that it is already enabled. Note: if the index was previously disabled, a restart may be required to fully re-initialize the `CodeIndex` instance.

```
/codeindex on
```
```
✓ Code index enabled for /home/user/Projects/ragent
  Starting initial index... (use /codeindex show to monitor progress)
```

If already enabled:
```
Code index is already active — 247 files, 4821 symbols indexed
```

#### `/codeindex off`

Disables the code index for the current project. Persists the setting in the ragent config (`codeindex.enabled = false`). Drops the `CodeIndex` reference and clears the stats cache immediately, releasing all memory and file handles. The index files on disk are preserved (not deleted) so re-enabling is fast.

```
/codeindex off
```
```
✓ Code index disabled
  Index preserved on disk. Re-enable with /codeindex on (restart required)
```

#### `/codeindex show`

Displays the current status of the code index, including enablement state, index statistics, disk size, and location.

```
/codeindex show
```
```
Code Index Status
─────────────────
State:    enabled (watching for changes)
Project:  /home/user/Projects/ragent
Location: .ragent/codeindex/
Size:     2.4 MB (SQLite) + 1.1 MB (FTS) = 3.5 MB total

Files indexed: 247 / 253 (6 binary/excluded)
Symbols extracted: 4,821
  Functions: 1,203  Structs: 342  Enums: 89  Traits: 47  Impls: 567
  Methods: 1,892  Constants: 201  Modules: 134  Macros: 56  Tests: 290

Languages: Rust (198), TOML (22), Markdown (15), JSON (8), YAML (4)

Last full index: 2026-04-13T04:30:00Z (12 min ago)
Background watcher: active (3 files queued)
```

When the index is disabled:
```
Code Index Status
─────────────────
State:    disabled
Project:  /home/user/Projects/ragent
Location: .ragent/codeindex/
Size:     2.4 MB (SQLite) + 1.1 MB (FTS) = 3.5 MB total

Index exists but is not active. Enable with /codeindex on
```

When no index exists:
```
Code Index Status
─────────────────
State:    disabled (no index)
Project:  /home/user/Projects/ragent

No index has been created. Enable with /codeindex on
```

#### `/codeindex reindex`

Triggers a full reindex of the project. Runs synchronously via `full_reindex()`, scanning for changed files and processing them through the 3-phase chunked pipeline. Progress is displayed in the status bar with a `⟳indexing N%` indicator.

```
/codeindex reindex
```
```
✓ Reindex complete: 247 files, 4821 symbols in 3.2s
```

If the reindex encounters an error:
```
✗ Reindex failed: <error message>
```

#### `/codeindex clear`

Deletes the index files from disk and resets to a clean state. The enabled/disabled setting is preserved — if enabled, a fresh full index begins automatically.

```
/codeindex clear
```
```
✓ Index cleared (removed 3.5 MB from .ragent/codeindex/)
  Starting fresh index...
```

#### `/codeindex help`

Shows all available subcommands with a brief explanation of each.

```
/codeindex help
```
```
Code Index Commands
───────────────────
  /codeindex on       Enable code indexing for this project.
                      Starts background file watcher and incremental indexing.
                      Setting is persisted across sessions.

  /codeindex off      Disable code indexing for this project.
                      Stops the file watcher and background worker.
                      Index files are preserved on disk for fast re-enable.

  /codeindex show     Show index status: enabled/disabled, file count,
                      symbol count, disk size, index location, and
                      watcher activity.

  /codeindex reindex  Trigger a full reindex of the project.
                      Runs in background — progress shown in status bar.

  /codeindex clear    Delete all index files and start fresh.
                      Enabled/disabled setting is preserved.

  /codeindex help     Show this help message.
```

### 10.2 Enablement State Persistence

The enabled/disabled state is stored in two places for robustness:

1. **ragent settings database** (`storage.set_setting("codeindex_enabled", "true"|"false")`) — per-project, survives config file edits
2. **ragent.json config** (`codeindex.enabled` field) — provides a global default that can be overridden per-project in the DB

Precedence: DB setting > config file > default (`true`).

When a user runs `/codeindex off`, the DB setting is written immediately. When they run `/codeindex on`, the DB setting is updated and the index lifecycle begins.

### 10.3 State Transitions

```
                    /codeindex on
    ┌─────────┐  ─────────────────▶  ┌──────────┐
    │ DISABLED │                      │ ENABLED  │
    │          │  ◀─────────────────  │          │
    └─────────┘    /codeindex off     └──────────┘
         │                                  │
         │  /codeindex clear                │  /codeindex clear
         ▼                                  ▼
    ┌─────────┐                       ┌──────────┐
    │ DISABLED │                      │ INDEXING  │
    │ (clean)  │                      │ (fresh)   │
    └─────────┘                       └──────────┘
```

---

## 12. Integration Points

### 12.1 With ragent-core

The `CodeIndex` struct is held as an `Option<Arc<CodeIndex>>` in the `ToolContext`, similar to `lsp_manager` and `storage`. Tools access it via context.

```rust
// In ToolContext (ragent-core)
pub struct ToolContext {
    // ... existing fields ...
    pub code_index: Option<Arc<CodeIndex>>,
}
```

### 12.2 With ragent-tui

The TUI manages the `CodeIndex` lifecycle:
1. On session start with a project directory → check if codeindex is enabled (DB setting → config → default)
2. If enabled: create `CodeIndex`, spawn background thread for `full_reindex()`
3. Show index status in the status bar (file count, symbol count, indexing indicator)
4. Handle `/codeindex` slash commands (on/off/show/reindex/rebuild/help)
5. On `/codeindex off`: drop `CodeIndex` reference, clear stats cache, update DB setting
6. On `/codeindex on`: report status (restart required to re-initialize), update DB setting
7. On session end / app quit → `CodeIndex` is dropped, releasing all resources

#### Status Bar Display

The status bar shows codeindex information in the format:

```
 📚 247 files │ 4821 syms │ ⟳indexing 42%
```

- **File/symbol counts** are refreshed every 5s using non-blocking `try_status()` (see §12.5)
- **`⟳indexing N%`** appears in yellow during active indexing, using lock-free atomic progress counters
- During active indexing, the poll interval drops to 1s for responsive progress updates
- When indexing completes, the indicator disappears and polling returns to the 5s interval

#### Non-blocking UI Polling

The TUI event loop polls `CodeIndex` status using `try_status()` which uses `try_lock()` on internal mutexes. If the background reindex thread holds the locks, the poll returns `None` and the UI keeps its cached stats — **never blocking the render loop**. This prevents the UI freeze that would occur with a standard `Mutex::lock()` when the background thread is actively writing.

```rust
// In refresh_code_index_stats():
match idx.try_status() {
    Some(stats) => { /* update cache, reset timer */ }
    None        => { /* keep stale cache, mark as busy */ }
}
```

#### Progress Tracking

Indexing progress is tracked via two `AtomicU32` counters on the `CodeIndex` struct:
- `reindex_total`: Set to the number of changed files at the start of `full_reindex()`
- `reindex_done`: Incremented after each chunk completes

The `reindex_progress() -> (u32, u32)` method reads both atomics without any lock, providing a responsive progress percentage even while the store/FTS locks are held.

### 12.3 With Existing Tools

- **grep/search**: Continue to work as-is for raw text search. `codeindex_search` complements them with structured, symbol-aware search.
- **LSP tools**: Continue to work for real-time hover/diagnostics. `codeindex_*` tools provide project-wide queries that LSP may not support well (e.g., "find all structs implementing trait X").
- **Reference (`@`) system**: The fuzzy matcher in `reference/fuzzy.rs` could optionally use the code index for faster, more accurate file/symbol matching.

### 12.4 Configuration

New section in `ragent.json`:

```json
{
  "codeindex": {
    "enabled": true,
    "max_file_size_kb": 1024,
    "excluded_patterns": ["*.generated.*", "vendor/"],
    "languages": ["rust", "python", "typescript", "go"],
    "watch_debounce_ms": 500,
    "index_path": ".ragent/codeindex",
    "fts_enabled": true
  }
}
```

The `enabled` field in `ragent.json` provides the **global default**. Per-project overrides are stored in the ragent settings database via `/codeindex on|off` and take precedence. This allows users to enable indexing globally but disable it for specific large/noisy projects, or vice versa.

| Setting Source | Key | Precedence |
|----------------|-----|------------|
| Settings DB (per-project) | `codeindex_enabled` | Highest — set by `/codeindex on\|off` |
| `ragent.json` config | `codeindex.enabled` | Middle — global default |
| Hardcoded default | `true` | Lowest — indexing on by default |

---

## 13. Performance Architecture

### 13.1 Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Initial full index (1000 files) | < 10 seconds | 3-phase chunked pipeline with batched writes |
| Incremental re-index (1 file) | < 50 ms | Single parse + single transaction |
| Symbol search by name | < 10 ms | SQLite indexed query |
| Full-text search | < 100 ms | Tantivy query |
| File watcher latency | < 1 second | Debounce window + batch processing |
| Memory usage (idle) | < 50 MB | LRU tree cache + tantivy reader |
| Index size (1000 files) | < 10 MB | SQLite + tantivy combined |

### 13.2 Concurrency Model

`CodeIndex` uses internal `Mutex<IndexStore>` and `Mutex<FtsIndex>` for thread safety. The background reindex runs on a plain `std::thread` (not a tokio task) to avoid blocking the async runtime.

```
┌────────────────────┐     ┌─────────────────────┐
│   TUI Event Loop   │     │  Background Thread   │
│   (tokio runtime)  │     │  (std::thread)       │
│                    │     │                      │
│  try_status()  ────┼──X──┼── full_reindex()     │
│  (try_lock,        │     │  (holds locks during  │
│   never blocks)    │     │   Phase 2 & 3 only)  │
│                    │     │                      │
│  reindex_progress()┼─────┼── AtomicU32 counters │
│  (lock-free read)  │     │  (lock-free write)   │
└────────────────────┘     └─────────────────────┘
```

Key design decisions:
- **`try_lock()` for UI**: The TUI never calls `Mutex::lock()` on store/FTS. Instead, `try_status()` uses `try_lock()` and returns `None` if the lock is held, keeping the UI responsive.
- **Atomic progress counters**: `reindex_total` and `reindex_done` are `AtomicU32` — the TUI reads them without any lock for real-time progress display.
- **Adaptive polling**: The TUI polls status every 5s when idle, every 1s during active indexing, and forces a redraw on busy-state transitions.
- **5ms yield between chunks**: After each chunk of 20 files, the background thread sleeps for 5ms, giving the TUI event loop a window to acquire locks if needed.

### 13.3 Batch Optimization Details

The `full_reindex()` method implements three key batching optimizations:

#### SQLite Transaction Batching

Without batching, each `upsert_symbols()`, `upsert_imports()`, and `upsert_refs()` call auto-commits as a separate SQLite transaction. For 400 files × ~30 symbols per file = ~12,000 individual disk syncs.

With batching, `begin_transaction()` and `commit_transaction()` wrap all upsert operations for an entire chunk (20 files) in a single transaction:

```rust
store.begin_transaction()?;
for (rel_path, parsed) in &parsed_results {
    store.upsert_symbols(file_id, &parsed.symbols)?;
    store.upsert_imports(file_id, &parsed.imports)?;
    store.upsert_refs(file_id, &parsed.references)?;
}
store.commit_transaction()?;
```

**Impact**: ~12,000 disk syncs → ~20 (one per chunk). **~99.8% reduction in SQLite I/O.**

#### FTS Commit Batching

Without batching, each file requires two tantivy operations: `remove_file()` (creates writer, commits) and `add_symbols()` (creates writer, commits). For 400 files = 800 writer allocations and commits.

The `batch_update()` method combines all remove + add operations into a single writer and commit:

```rust
fts.batch_update(&remove_paths, &fts_syms)?;
```

**Impact**: ~800 tantivy commits → ~20 (one per chunk). **~97.5% reduction in FTS overhead.**

#### Lock Hold Duration Reduction

Without batching, each file acquires both the store lock and the FTS lock:
- Store lock held ~4-6ms per file (inserts + disk sync)
- FTS lock held ~1.5-2.5ms per file (writer allocation + commit)
- Total lock time: ~2.4-4.4 seconds for 400 files

With batching, parsing (the most expensive CPU work) happens outside all locks. Locks are only held during the write phases:
- Store lock held for one transaction per chunk (~10-15ms for 20 files)
- FTS lock held for one commit per chunk (~5-10ms for 20 files)
- Total lock time: ~300-500ms for 400 files

**Impact**: ~90% reduction in total lock contention time.

### 13.4 FTS Sync Recovery

After a full reindex, `ensure_fts_sync()` verifies the FTS document count matches the SQLite symbol count. If divergence exceeds a 2:1 or 1:2 ratio, the FTS index is automatically rebuilt from SQLite data via `rebuild_fts()`. This handles crash recovery and accumulated drift.

### 13.5 Cached Status Queries

To avoid per-frame database queries, `IndexStats` are cached with a 5-second TTL. The cache is updated by `refresh_code_index_stats()` in the TUI event loop using `try_status()`. When a refresh attempt is skipped (lock busy), the timer is not reset — ensuring the next poll retries promptly.

---

## 14. Security Considerations

- **Path traversal**: All paths are canonicalized and validated against the project root before indexing
- **File size limits**: Files exceeding `max_file_size_kb` (default 1 MB) are skipped
- **Binary detection**: Binary files are skipped via extension list + content sniffing (NUL byte check in first 8 KB)
- **No code execution**: Tree-sitter parsing is purely syntactic — no macros expanded, no code evaluated
- **Credential safety**: The index does not store file contents verbatim (only signatures, doc comments, and body hashes). The FTS index stores snippets, so `.ragent/codeindex/` should be in `.gitignore`
- **Concurrency**: Uses non-blocking `try_lock()` for UI thread, batched writes with brief yields between chunks, and lock-free atomic counters for progress tracking — designed to never starve the TUI event loop

---

## 15. Dependencies

All dependencies are Rust crates compiled into the ragent binary. **No external tools, databases, or runtime installations are required.**

### New Crate Dependencies (for `ragent-code`)

| Crate | Version | Purpose | Notes |
|-------|---------|---------|-------|
| `tree-sitter` | `0.24` | Core parsing runtime | Compiles C runtime into binary |
| `tree-sitter-rust` | `0.23` | Rust grammar | Compiled-in C grammar |
| `tree-sitter-python` | `0.23` | Python grammar | Compiled-in C grammar |
| `tree-sitter-javascript` | `0.23` | JavaScript grammar | Compiled-in C grammar |
| `tree-sitter-typescript` | `0.23` | TypeScript grammar | Compiled-in C grammar |
| `tree-sitter-go` | `0.23` | Go grammar | Compiled-in C grammar |
| `tree-sitter-c` | `0.23` | C grammar | Compiled-in C grammar |
| `tree-sitter-cpp` | `0.23` | C++ grammar | Compiled-in C grammar |
| `tree-sitter-java` | `0.23` | Java grammar | Compiled-in C grammar |
| `tantivy` | `0.22` | Full-text search engine | Pure Rust, no external deps |
| `notify` | `7.0` | Filesystem watcher | Pure Rust, uses OS APIs (inotify/kqueue/FSEvents) |

> **Build note**: Tree-sitter grammars are compiled from C source at build time via `cc` (already available in the Rust toolchain). No runtime C compiler or grammar files are needed — the grammars are baked into the binary.

### Already Available in Workspace

| Crate | Used For |
|-------|----------|
| `rusqlite` | Index storage (same engine as ragent-core Storage) |
| `blake3` | Content hashing |
| `ignore` | .gitignore-aware directory walking |
| `tokio` | Async runtime, background tasks, channels |
| `serde` / `serde_json` | Serialization |
| `tracing` | Logging |
| `rayon` | Parallel processing for initial index builds |

---

## 16. Future Extensions

These are explicitly out of scope for the initial implementation but inform architectural decisions:

1. **Embedding-based semantic search**: Add an optional embedding column to the symbols table, use a local model (e.g., `all-MiniLM-L6-v2` via `candle` or `ort`) to generate embeddings, and add cosine similarity search. The tantivy index can be augmented with a vector store.

2. **Cross-reference resolution**: Resolve imported symbols to their definitions in other files, building a full call graph. Requires type inference which tree-sitter alone cannot provide — may integrate with `rust-analyzer` query API.

3. **Change impact analysis**: Given a modified symbol, identify all transitively affected symbols/tests. Useful for "which tests should I run?" queries.

4. **Multi-project indexing**: Index multiple related repositories (e.g., monorepo workspaces) with shared symbol namespaces.

5. **MCP exposure**: Expose the code index as an MCP server so external tools (Cursor, VS Code) can query it.

6. **Incremental tree-sitter re-parsing**: The `tree_cache` stores parsed trees, but they are not currently passed back to `parser.parse(source, Some(&old_tree))` for incremental re-parsing. Implementing this could improve re-parse time by 10-20% for frequently-edited files.

7. **Adaptive chunk sizing**: Dynamically adjust `CHUNK_SIZE` and `YIELD_MS` based on actual lock contention (e.g., measuring how often `try_status()` returns `None`). This would optimize the trade-off between throughput and UI responsiveness.

8. **Parallel parsing within chunks**: Currently, files within a chunk are parsed sequentially. Since parsing is CPU-bound and lock-free, it could be parallelized with `rayon` for multi-core speedup during the parse phase.

---

## 17. References

- [tree-sitter documentation](https://tree-sitter.github.io/tree-sitter/)
- [tree-sitter Rust bindings](https://docs.rs/tree-sitter/latest/tree_sitter/)
- [tantivy full-text search](https://docs.rs/tantivy/latest/tantivy/)
- [notify filesystem watcher](https://docs.rs/notify/latest/notify/)
- [CocoIndex article](https://dev.to/badmonster0/build-a-real-time-codebase-index-in-5-minutes-with-cocoindex-rust-tree-sitter-eo3) — inspiration for the design
- [ragent tool architecture](crates/ragent-core/src/tool/mod.rs) — Tool trait and ToolContext
- [ragent storage](crates/ragent-core/src/storage/mod.rs) — SQLite patterns
