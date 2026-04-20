# ragent Code Index — Implementation Plan

This document details the implementation plan for the ragent Code Index system described in [CODEINDEX.md](./CODEINDEX.md). Work is organized into 6 milestones, each building on the previous one, with clearly defined tasks and deliverables.

### Guiding Constraints

- **Fully embedded**: Every dependency (tree-sitter, tantivy, SQLite, notify) compiles into the ragent binary. No external tools, databases, servers, or runtime installations may be required.
- **User-controllable**: The indexer is toggled via `/codeindex on|off`. The enabled/disabled state is persisted in the ragent settings DB (per-project) with a global default in `ragent.json`.
- **Non-intrusive**: When disabled, zero overhead. When enabled, background work yields to agents via ragent's existing semaphore system.

---

## Milestone 1: Foundation — Crate Setup, Types, and Scanner ✅

**Goal**: Establish the `ragent-code` crate with core types, file scanning, and content hashing. No parsing yet — just the ability to discover and fingerprint all source files in a project.

**Status**: Complete — 22 tests passing (14 unit + 8 integration)

### Tasks

#### M1.1 — Bootstrap ragent-code crate ✅
- Update `crates/ragent-code/Cargo.toml` with initial dependencies:
  - `blake3` (content hashing)
  - `ignore` (gitignore-aware directory walking)
  - `rusqlite` (bundled, index storage)
  - `tokio` (async runtime)
  - `serde`, `serde_json` (serialization)
  - `tracing` (logging)
  - `chrono` (timestamps)
  - `anyhow` (error handling)
- Create module structure in `src/`: `lib.rs`, `types.rs`, `scanner.rs`, `store.rs`
- Export public API from `lib.rs`
- Verify crate compiles with `cargo check -p ragent-code`

#### M1.2 — Define core types (`types.rs`) ✅
- Define `SymbolKind` enum: `Function`, `Method`, `Struct`, `Class`, `Enum`, `EnumVariant`, `Trait`, `Interface`, `Impl`, `Module`, `Constant`, `Static`, `TypeAlias`, `Field`, `Import`, `Macro`, `Test`, `Unknown`
- Define `Visibility` enum: `Public`, `PubCrate`, `PubSuper`, `Private`
- Define `FileEntry` struct: `path`, `content_hash`, `byte_size`, `language`, `last_indexed`, `mtime_ns`, `line_count`
- Define `Symbol` struct: `id`, `file_id`, `name`, `qualified_name`, `kind`, `visibility`, `start_line`, `end_line`, `start_col`, `end_col`, `parent_id`, `signature`, `doc_comment`, `body_hash`
- Define `ImportEntry` struct: `file_id`, `imported_name`, `source_module`, `alias`, `line`, `kind`
- Define `SymbolRef` struct: `symbol_name`, `file_id`, `line`, `col`, `kind`
- Define `IndexStats` struct for status reporting
- Implement `Display`, `Serialize`, `Deserialize` for all types
- **Tests**: Unit tests for type serialization round-trips

#### M1.3 — Implement file scanner (`scanner.rs`) ✅
- `scan_directory(root: &Path, config: &ScanConfig) -> Vec<ScannedFile>`
  - Uses `ignore::WalkBuilder` for gitignore-aware traversal
  - Hardcoded exclusions: `.git`, `target`, `node_modules`, `__pycache__`, `.ragent`
  - Configurable exclusion patterns from `ScanConfig`
  - Skips files exceeding `max_file_size` (default: 1 MB)
  - Skips binary files (NUL byte check in first 8 KB)
- `hash_file(path: &Path) -> Result<String>` — blake3 hash
- `detect_language(path: &Path) -> Option<String>` — extension-based mapping
- `ScannedFile` struct: `path`, `hash`, `size`, `language`, `mtime`, `line_count`
- Support parallel scanning with `rayon` for initial full scans
- **Tests**: Test scanning a temp directory with mixed files (source, binary, gitignored)

#### M1.4 — Implement index store (`store.rs`) — file tracking only ✅
- `IndexStore::open(path: &Path) -> Result<IndexStore>` — opens/creates SQLite database
- `IndexStore::open_in_memory() -> Result<IndexStore>` — for testing
- Auto-create schema on first open (migration system for future schema changes)
- Implement `indexed_files` table CRUD:
  - `upsert_file(entry: &FileEntry) -> Result<i64>`
  - `get_file(path: &str) -> Result<Option<FileEntry>>`
  - `list_files() -> Result<Vec<FileEntry>>`
  - `delete_file(path: &str) -> Result<()>`
  - `get_stale_files(current_files: &[ScannedFile]) -> Result<StaleDiff>`
    - Returns: files to add, files to update (hash changed), files to remove (deleted)
- `StaleDiff` struct: `to_add: Vec<ScannedFile>`, `to_update: Vec<ScannedFile>`, `to_remove: Vec<String>`
- All writes in transactions for atomicity
- **Tests**: CRUD operations on in-memory store, stale file detection

#### M1.5 — Integration test: scan → store round-trip ✅
- Create integration test that:
  1. Creates a temp directory with sample Rust/Python files
  2. Scans with `scan_directory()`
  3. Stores results with `IndexStore`
  4. Modifies one file, re-scans
  5. Verifies `get_stale_files()` correctly identifies the changed file
- Verify the full pipeline with `cargo test -p ragent-code`

### Deliverables
- Compiling `ragent-code` crate with scanner and store
- Core type definitions used by all subsequent milestones
- Passing tests for scanning, hashing, language detection, and file storage

---

## Milestone 2: Tree-sitter Parsing and Symbol Extraction ✅

**Goal**: Parse source files into ASTs using tree-sitter and extract structured symbols. Rust language support first, with the framework ready for other languages.

**Status**: Complete — 46 unit + 21 integration tests passing (67 total)

### Tasks

#### M2.1 — Add tree-sitter dependencies ✅
- Added to `ragent-code/Cargo.toml`:
  - `tree-sitter = "0.26"` (latest 0.26.8)
  - `tree-sitter-rust = "0.24"` (latest 0.24.2)
- Verify tree-sitter compiles on the CI target (may need C compiler for grammar)
- Create `src/parser/mod.rs` with `LanguageParser` trait

#### M2.2 — Define parser trait and dispatcher (`parser/mod.rs`) ✅
- `LanguageParser` trait:
  ```rust
  pub trait LanguageParser: Send + Sync {
      fn language_id(&self) -> &str;
      fn parse(&self, source: &[u8]) -> Result<ParsedFile>;
      fn parse_incremental(&self, source: &[u8], old_tree: &Tree) -> Result<ParsedFile>;
  }
  ```
- `ParsedFile` struct: `symbols: Vec<Symbol>`, `imports: Vec<ImportEntry>`, `references: Vec<SymbolRef>`, `tree: Tree` (for caching)
- `ParserRegistry`: maps language string → `Arc<dyn LanguageParser>`
- `parse_file(path: &Path, source: &[u8], language: &str) -> Result<ParsedFile>`
- **Tests**: Registry dispatches to correct parser based on language

#### M2.3 — Implement Rust parser (`parser/rust.rs`) ✅
- Implemented ~700 lines with recursive descent through tree-sitter AST
- Uses `ExtractionContext` pattern with scope tracking for qualified names
- Handles: functions, methods, structs (with fields), enums (with variants), traits, impl blocks, constants, statics, modules, type aliases, macros, use statements, test functions
- Extracts: visibility, doc comments, function signatures, body hashes (blake3)
- Parent-child relationships with `method_context` flag for correct Method vs Function classification
- 20 unit tests covering all symbol types

#### M2.4 — Extend index store for symbols ✅
- Added `symbols`, `imports`, `symbol_refs`, `file_deps` tables with full schema from CODEINDEX.md §4.2–4.5
- Schema version bumped to 2 with `PRAGMA foreign_keys = ON` for cascade deletes
- Implemented CRUD with two-pass symbol insertion (parents first, children second) for correct parent_id mapping
- Added `SymbolFilter` type with name/kind/visibility/file_path/language/limit filters
- Added `FromStr` impls for `SymbolKind` and `Visibility` for DB deserialization
- Implemented: `upsert_symbols`, `query_symbols`, `get_file_symbols`, `symbol_count`, `upsert_imports`, `get_file_imports`, `query_imports`, `upsert_refs`, `find_references`, `set_file_deps`, `get_dependents`, `get_file_id`, `get_stats`
- 9 new unit tests: symbol CRUD, parent mapping, replace-on-upsert, imports, refs, file deps, cascade delete, stats

#### M2.5 — Integration test: scan → parse → store ✅
- 13 end-to-end tests in `tests/test_parse_store_integration.rs`:
  - Full pipeline (scan → parse → store → query)
  - Query by name, kind, visibility, limit
  - Method parent relationship verification
  - Import storage and retrieval
  - Doc comment and signature extraction
  - Qualified names for nested symbols
  - Enum variant parent relationships
  - Incremental re-index: modify file → verify updated signature, removed symbols, untouched files
  - Index stats after indexing

### Deliverables
- Tree-sitter Rust parsing with comprehensive symbol extraction
- Extensible parser framework ready for additional languages
- Symbols, imports, and references stored in SQLite
- Passing integration tests for the full scan → parse → store pipeline

---

## Milestone 3: Search Engine and Query API ✅

**Goal**: Add tantivy full-text search alongside structured SQLite queries. Implement the `CodeIndex` public API that tools will use.

**Status**: Complete — 57 unit + 18 CodeIndex integration + 13 parse-store + 8 scan-store + 1 doc = 97 tests passing

### Tasks

#### M3.1 — Add tantivy dependency ✅
- Added `tantivy = "0.22"` to `ragent-code/Cargo.toml`
- Created `src/search.rs` module with `FtsIndex` struct
- Compilation verified (tantivy adds ~60 new crate deps)

#### M3.2 — Implement tantivy FTS index (`search.rs`) ✅
- Tantivy schema with 9 fields: `name`, `qualified_name`, `kind`, `file_path`, `signature`, `doc_comment` (TEXT stored), `body_snippet` (TEXT not stored), `start_line`, `end_line` (I64)
- `FtsIndex::open(path)` — opens/creates MmapDirectory-backed index on disk
- `FtsIndex::open_in_memory()` — RAM-backed for testing
- `FtsIndex::add_symbols(&[FtsSymbol])` — batch insert with auto-commit
- `FtsIndex::remove_file(path)` — delete by file_path term
- `FtsIndex::search(query, limit)` — QueryParser with boosted fields (name: 10×, qualified: 5×, signature: 3×, doc: 2×, body: 1×)
- `FtsIndex::doc_count()` — total indexed documents
- `SearchResult` struct with `Display` (compact `{}` and detailed `{:#}` modes)
- 11 unit tests: open, add+search by name/doc/body/signature, remove, limit, boost, display, on-disk persistence

#### M3.3 — Implement `CodeIndex` public API (`lib.rs`) ✅
- `CodeIndex` facade owns `Mutex<IndexStore>` + `Mutex<FtsIndex>` + `ParserRegistry`
- `CodeIndex::open(&CodeIndexConfig)` — disk-backed, creates `.ragent/codeindex/` directory
- `CodeIndex::open_in_memory(project_root)` — for testing
- Query methods: `search()`, `symbols()`, `references()`, `dependencies()`, `status()`
- Mutation methods: `index_file()`, `index_files()`, `full_reindex()`, `remove_file()`
- `search()` combines FTS with post-filters (kind, language, file_pattern)
- `full_reindex()` uses scan → diff → apply_diff → parse → FTS update pipeline

#### M3.4 — Search query types and result formatting ✅
- `SearchQuery`: query, kind, language, file_pattern, max_results, include_body
- `IndexResult`: files_added/updated/removed, symbols_extracted, elapsed_ms with Display
- `DepDirection` enum: Imports / Dependents
- `CodeIndexConfig`: enabled, project_root, index_dir, scan_config
- `SearchResult::Display` compact (`function parse_config — src/config.rs:10`) and detailed modes

#### M3.5 — Integration tests ✅
- 18 integration tests in `tests/test_codeindex.rs`:
  - Full reindex, search by name/doc/kind/file_pattern
  - Symbols query by kind/name, status with disk size
  - Remove file (SQLite + FTS), incremental reindex (no-op, change, delete)
  - Direct `index_file()`, in-memory index, max_results limit
  - Method discovery, enum FTS search, display formatting

### Deliverables
- Working full-text search with tantivy (11 unit tests)
- Complete `CodeIndex` public API (18 integration tests)
- Dual search strategy (structured SQLite + FTS) with boosted ranking
- 97 total tests across the crate

---

## Milestone 4: Background Worker and File Watcher ✅

**Goal**: Implement real-time background indexing. Files are automatically re-indexed when they change, with debouncing and batching for efficiency.

**Status**: ✅ Complete

### Tasks

#### M4.1 — Add notify dependency and create watcher module ✅
- Added `notify = "7.0"` and `lru` (workspace) to `ragent-code/Cargo.toml`
- Created `src/watcher.rs` with `CodeWatcher` and `WatchEvent` enum
- Filters: `.git/`, `target/`, `node_modules/`, `__pycache__/`, `.ragent/`, etc.
- Maps `notify::Event` variants to `WatchEvent::{Created, Changed, Deleted, Renamed}`
- 8 unit tests (ignore filters, relativize, FS events)

#### M4.2 — Implement background worker (`worker.rs`) ✅
- `IndexWorker::start(Arc<Mutex<CodeIndex>>, Receiver, WorkerConfig) -> IndexWorkerHandle`
- Spawns a dedicated thread (not tokio) for the worker loop
- Debounce: collects events for `debounce_ms` (default 500ms) after last event
- Dedup: `EventBatch` with `HashSet<PathBuf>` — delete overrides create, rename = delete+create
- Batching: processes up to `batch_size` files per lock acquisition
- `IndexWorkerHandle`: `stop()`, `queue_reindex()`, `queue_full_reindex()`, `stats()`
- `WorkerStats`: files_indexed, files_removed, batches_processed, is_busy
- Queue overflow protection: triggers full reindex if queue exceeds `max_queue_size`
- 8 unit tests (event batch dedup, worker start/stop, event processing, channel disconnect)

#### M4.3 — Tree cache for incremental parsing ✅
- Created `src/tree_cache.rs` with `TreeCache` (LRU, default capacity 1000)
- `ParsedFile` now carries `Option<tree_sitter::Tree>` for cache storage
- `RustParser` returns the tree in `ParsedFile::tree`
- `CodeIndex::index_file()` stores trees in cache after successful parse
- `CodeIndex::remove_file()` evicts from cache on file deletion
- 6 unit tests (put/get, remove, LRU eviction, clear, capacity)

#### M4.4 — Integrate watcher + worker into `CodeIndex` ✅
- `start_watching(Arc<Mutex<CodeIndex>>, WorkerConfig) -> Result<WatchSession>`
- `WatchSession`: owns `CodeWatcher` + `IndexWorkerHandle`, provides `stop()`, `stats()`, `is_stopped()`, `queue_reindex()`, `queue_full_reindex()`
- Performs initial `full_reindex()` on watch start to catch offline changes
- Dropping `WatchSession` stops both watcher and worker gracefully

#### M4.5 — Concurrency controls ✅
- Worker uses `Mutex<CodeIndex>` — at most 1 concurrent batch
- `AtomicBool` stop flag checked between files in batch (mid-batch cancel)
- Worker thread joins on `stop()` for graceful shutdown
- `IndexWorkerHandle::drop()` auto-stops to prevent resource leaks

### Implementation Notes
- 132 total tests across the crate (79 unit + 18 CodeIndex + 13 M4 + 13 parse-store + 8 scan-store + 1 doc)
- Worker uses a dedicated OS thread (not tokio) for simplicity and predictability
- `WatchSession` is a returned handle — no circular Arc references in `CodeIndex`
- Event deduplication uses `HashSet`-based `EventBatch` with semantic merging

---

## Milestone 5: Tool Integration ✅

**Goal**: Register 6 new tools in the ragent ToolRegistry so agents can query the code index during conversations.

**Status**: ✅ Complete — 16 integration tests passing.

### Implementation Notes

- Added `ragent-code` as dependency of `ragent-core` in Cargo.toml
- Added `code_index: Option<Arc<ragent_code::CodeIndex>>` to `ToolContext`
- Created 6 tool files in `crates/ragent-core/src/tool/`:
  - `codeindex_search.rs` — FTS search with kind/language/file_pattern filters
  - `codeindex_symbols.rs` — Structured symbol query with name/kind/visibility filters
  - `codeindex_references.rs` — Find references to a symbol by name
  - `codeindex_dependencies.rs` — File-level import/dependent queries
  - `codeindex_status.rs` — Index stats (files, symbols, languages, size)
  - `codeindex_reindex.rs` — Trigger full re-index
- All tools return structured "not available" response with `fallback_tools` metadata when `code_index` is `None`
- Registered all 6 tools in `create_default_registry()`
- Added `/codeindex` slash command with `on|off|show|reindex|help` sub-commands
- Added `build_codeindex_guidance_section()` to system prompt assembly
- Permission categories: `codeindex:read` (5 tools), `codeindex:write` (reindex)
- Test file: `crates/ragent-core/tests/test_codeindex_tools.rs` (16 tests)

### Tasks

#### M5.1 — Create tool implementations in ragent-core ✅
#### M5.2 — Implement `codeindex_search` tool ✅
#### M5.3 — Implement `codeindex_symbols` tool ✅
#### M5.4 — Implement `codeindex_references` tool ✅
#### M5.5 — Implement `codeindex_dependencies` tool ✅
#### M5.6 — Implement `codeindex_status` and `codeindex_reindex` tools ✅
#### M5.7 — Register tools in default registry ✅
#### M5.8 — Wire `CodeIndex` into `ToolContext` with enable/disable lifecycle ✅
#### M5.9 — System prompt integration ✅

---

## Milestone 6: Additional Languages, Polish, and Documentation ✅

**Goal**: Add support for Python, TypeScript, Go, C/C++, and Java. Performance tuning, benchmarks, and user documentation.

### Tasks

#### M6.1 — Add Python parser (`parser/python.rs`) ✅
- Add `tree-sitter-python` dependency
- Extract: functions (`def`), classes, methods, decorators (`@property`, `@staticmethod`), imports (`import`, `from ... import`), module-level constants, type hints
- Visibility: `_private`, `__dunder__` conventions
- **Tests**: Parse sample Python files with classes, decorators, type hints

#### M6.2 — Add TypeScript/JavaScript parser (`parser/typescript.rs`) ✅
- Add `tree-sitter-typescript` and `tree-sitter-javascript` dependencies
- Extract: functions, arrow functions, classes, methods, interfaces, type aliases, enums, `import`/`export` statements, React components (function returning JSX)
- Visibility: `export` vs non-exported
- Handle `.ts`, `.tsx`, `.js`, `.jsx` extensions
- **Tests**: Parse sample TS files with interfaces, generics, React components

#### M6.3 — Add Go parser (`parser/go.rs`) ✅
- Add `tree-sitter-go` dependency
- Extract: functions, methods (receiver-based), structs, interfaces, constants, type aliases, `import` blocks
- Visibility: capitalized names = exported
- **Tests**: Parse sample Go files with interfaces, methods, packages

#### M6.4 — Add C/C++ parser (`parser/c_cpp.rs`) ✅
- Add `tree-sitter-c` and `tree-sitter-cpp` dependencies
- Extract: functions, structs, classes (C++), enums, typedefs, `#include` directives, namespaces (C++), templates (C++)
- Visibility: header-based (public if in `.h`, implementation if in `.c`/`.cpp`)
- **Tests**: Parse sample C/C++ files with structs, classes, templates

#### M6.5 — Add Java parser (`parser/java.rs`) ✅
- Add `tree-sitter-java` dependency
- Extract: classes, interfaces, methods, fields, enums, annotations, `import` statements, packages
- Visibility: `public`, `protected`, `private`, package-private
- **Tests**: Parse sample Java files with annotations, generics, inner classes

#### M6.6 — Performance benchmarks (`benches/bench_indexing.rs`) ✅
- Benchmark: per-language parse throughput (all 7 languages)
- Benchmark: repeated Rust parsing (10/50/100 iterations)
- Benchmark: store upsert (index single file)
- Benchmark: FTS search (Config, helper queries)
- Benchmark: full index of 7 files across all languages

#### M6.7 — Configuration and enablement persistence ✅
- Added `CodeIndexConfig` to `Config` in `ragent-core/src/config/mod.rs`
  - `enabled: bool` (default: `true`)
  - `max_file_size: u64` (default: 1 MB)
  - `extra_exclude_dirs: Vec<String>`
  - `extra_exclude_patterns: Vec<String>`
- Serializable via serde, stored in `ragent.json` under `code_index`

#### M6.8 — TUI status bar integration ✅ (completed in M5)

#### M6.9 — `/codeindex` slash command ✅ (completed in M5)

#### M6.10 — Update documentation ✅
- Updated `README.md` with code index feature description and architecture table
- Updated `QUICKSTART.md` with `/codeindex` slash commands
- Updated `CHANGELOG.md` with M6 feature entry
- Fixed all `cargo doc` warnings in ragent-code
- `cargo doc -p ragent-code --no-deps` builds clean

#### M6.11 — Enhance `@` reference system (deferred)
- Deferred to future milestone — requires deeper integration with reference resolution system

### Deliverables
- 7 supported languages (Rust, Python, TypeScript/JSX, JavaScript/JSX, Go, C, C++, Java)
- Performance benchmarks via Criterion
- Configuration persistence via `ragent.json`
- TUI integration (status bar + `/codeindex` slash command with on/off/show/reindex/help)
- Complete documentation (README, QUICKSTART, CHANGELOG, cargo doc clean)

---

## Dependency Graph

```
M1 (Foundation)
 │
 ├──▶ M2 (Parsing)
 │     │
 │     └──▶ M3 (Search)
 │           │
 │           ├──▶ M4 (Background Worker)
 │           │     │
 │           │     └──▶ M5 (Tool Integration)
 │           │           │
 │           │           └──▶ M6 (Languages + Polish)
 │           │
 │           └──▶ M6.6 (Benchmarks — can start after M3)
 │
 └──▶ M4.1 (Watcher — can start after M1)
```

- **M1** must complete before any other milestone
- **M2** depends on M1 (types, store)
- **M3** depends on M2 (parsed symbols to search)
- **M4** depends on M1 (scanner) and M3 (CodeIndex API to call)
- **M5** depends on M3 (CodeIndex API) and M4 (watcher lifecycle)
- **M6** depends on M2 (parser framework) and M5 (tool integration)
- **M4.1** (watcher module alone) can start in parallel with M2
- **M6.6** (benchmarks) can start once M3 delivers a working CodeIndex

---

## Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|------------|
| Tree-sitter C compilation on CI | Build failures | Use `tree-sitter` bundled feature; test early in M2.1 |
| Tantivy crate size / build time | Slow CI builds | Feature-gate FTS behind `fts` feature flag; SQLite-only mode as fallback |
| Large codebase memory pressure | OOM during initial index | Batch processing with yields; configurable `max_files` limit; stream parsing |
| File watcher event storms | Worker overwhelmed | Debouncing (500ms), queue size limit (10,000), backpressure via channel capacity |
| Tree-sitter grammar version mismatches | Parse failures on valid code | Pin grammar versions; graceful fallback (skip unparseable files, log warning) |
| SQLite write contention with ragent-core Storage | Lock timeouts | Separate SQLite database file (`.ragent/codeindex.db`), independent from main DB |
| Incremental parse correctness | Stale/wrong symbols | Always verify: if incremental parse yields different symbol count than full parse, fall back to full |

---

## Testing Strategy

### Unit Tests (per module)
- `test_scanner.rs` — file discovery, hashing, language detection, binary skipping
- `test_parser.rs` — per-language parsing correctness with sample source files
- `test_extractor.rs` — symbol extraction from AST nodes
- `test_store.rs` — SQLite CRUD, stale file detection, concurrent access
- `test_search.rs` — tantivy indexing, query parsing, result ranking
- `test_worker.rs` — event debouncing, batching, graceful shutdown

### Integration Tests
- Full pipeline: scan → parse → store → search → verify results
- Incremental: modify file → verify only changed symbols updated
- Watcher: create/delete/rename files → verify index consistency
- Tool: invoke tool with mock ToolContext → verify output formatting

### Benchmarks
- Initial index of ragent codebase (~250 files)
- Incremental reindex after single file change
- Symbol search latency (exact, prefix, fuzzy)
- Full-text search latency
- Memory usage profile during indexing

---

## Success Criteria

1. **Functional**: Agent can use `codeindex_search` to find symbols that `grep` would miss (e.g., "all public functions returning Result" or "structs implementing Tool trait")
2. **Incremental**: Changing one file re-indexes only that file in < 50ms
3. **Background**: File changes are automatically detected and indexed without agent intervention
4. **Performant**: Searches return in < 100ms on a 1000-file codebase
5. **Reliable**: Index survives crashes (SQLite transactions), rebuilds automatically if corrupted
6. **Zero-config**: Works out of the box for Rust projects, configurable for multi-language
