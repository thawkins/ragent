# ragent-codeindex

Codebase indexing with tree-sitter parsing, SQLite storage, Tantivy full-text
search, file watching, and a semantic code graph. A self-contained leaf crate
with no `ragent-*` workspace dependencies.

## Workspace Dependencies

None. `ragent-codeindex` is a self-contained leaf crate.

## External Dependencies

- blake3, ignore, rusqlite, tokio, serde, serde_json, tracing, chrono, anyhow, rayon, lru
- tree-sitter + 13 language grammars (rust, python, typescript, javascript, go, c, cpp, java, hcl, openscad, cmake, groovy, kotlin, xml)
- tantivy (full-text search), notify (filesystem watching)

Dev-dependencies: tempfile, tokio (full+test-util), criterion.

## Public API (crate root)

### Modules

- **types** — Core data types: `SymbolKind`, `FileEntry`, `Symbol`, `SymbolRef`, `CodeIndexConfig`, etc.
- **scanner** — File discovery, content hashing, language detection.
- **store** — SQLite-backed index storage with incremental update support.
- **parser** — Tree-sitter parsing and symbol extraction (13 language parsers).
- **search** — Full-text search index backed by Tantivy.
- **watcher** — Filesystem event watcher for real-time index updates.
- **worker** — Background indexing worker with debounce, dedup, and batching.
- **tree_cache** — LRU tree cache for incremental tree-sitter parsing.
- **graph** — Semantic code graph: typed edges, community detection, traversal.

### Crate-root types

- **CodeIndex** (struct) — Main entry point; owns the SQLite store, Tantivy FTS index, tree cache, and parser registry; thread-safe via internal `Mutex` guards.
  - `open(config)` / `open_in_memory(config)` — Open or create an index.
  - `search(query)` / `try_search(query)` — Full-text search with structured filters (blocking / non-blocking).
  - `symbols(filter)` / `try_symbols(filter)` — Query symbols from the structured index.
  - `references(name, limit)` / `try_references(...)` — Find all references to a symbol.
  - `dependencies(path, direction)` / `try_dependencies(...)` — Get file dependencies (imports or dependents).
  - `status()` / `try_status()` — Index statistics.
  - `godnodes(n)` / `try_godnodes(n)` — Top-N highest-degree symbols.
  - `path(from, to)` / `try_path(...)` — Shortest path between two symbols.
  - `explain(name)` / `try_explain(name)` — Symbol metadata and edges.
  - `communities()` / `try_communities()` — Community detection.
  - `build_graph()` / `build_graph_for_language(lang)` — Build/rebuild the semantic edge graph.
  - `graph_edge_count()` / `graph_status()` — Graph statistics.
  - `reindex_progress()` — `(done, total)` for current reindex.
  - `ensure_fts_sync()` — Ensure FTS index is in sync with SQLite.
  - `index_file(path)` / `index_files(paths)` / `full_reindex()` / `rebuild_fts()` / `remove_file(path)` — Indexing operations.
  - `project_root()` — Return the project root path.
- **WatchSession** (struct) — Active file-watching session; methods: `stop`, `stats`, `is_stopped`, `queue_reindex(path)`, `queue_full_reindex()`.
- **start_watching(index, config)** (fn) — Start watching a project directory and automatically re-index.

### Module: types

- **SymbolKind** (enum) / **Visibility** (enum) — Symbol classification.
- **FileEntry** / **Symbol** / **ImportEntry** / **SymbolRef** (structs) — Index record types.
- **IndexStats** / **GraphStatus** / **SymbolFilter** / **ScanConfig** / **ScannedFile** / **StaleDiff** / **SearchQuery** / **IndexResult** (structs) — Query and result types.
- **DepDirection** (enum) / **EdgeKind** (enum) / **Confidence** (enum) / **GraphEdge** (struct) — Dependency and graph types.
- **CodeIndexConfig** (struct) — Configuration with `Default`.

### Module: scanner

- **scan_directory(root, config)** (fn) — Gitignore-aware directory scan with parallel hashing.
- **hash_content(content)** / **hash_file(path)** (fns) — blake3 content hashing.
- **detect_language(path)** (fn) — Language detection from file extension (40+ languages).
- **is_binary(content)** (fn) / **count_lines(content)** (fn) — Content inspection.
- **SUPPORTED_LANGUAGES** (const) — All supported language identifiers.

### Module: store

- **IndexStore** (struct) — SQLite-backed store; methods: `open`, `open_in_memory`, `upsert_file`, `get_file`, `list_files`, `delete_file`, `file_count`, `total_bytes`, `language_counts`, `get_stale_files`, `apply_diff`, `upsert_symbols`, `query_symbols`, `symbol_count`, `upsert_imports`, `query_imports`, `upsert_refs`, `find_references`, `set_file_deps`, `get_dependents`, `get_file_deps`, `schema_version`, `get_stats`, `upsert_edge_typed`, `edge_count`, `edge_count_by_kind`, `graph_node_count`, `community_count`, `delete_edges_for_symbols`.

### Module: parser

- **ParsedFile** (struct) — Output of parsing a single file.
- **LanguageParser** (trait) — Language-specific parser; methods: `language_id`, `parse`.
- **ParserRegistry** (struct) — Registry of language parsers; methods: `new`, `get`, `supported_languages`, `parse`.
- Language parser submodules: `rust`, `python`, `typescript`, `go`, `c_cpp`, `java`, `hcl`, `openscad`, `cmake`, `gradle`, `gradle_kts`, `maven`, `util`.

### Module: search

- **SearchResult** (struct) — A single search result.
- **FtsIndex** (struct) — Tantivy-backed FTS index; methods: `open`, `open_in_memory`, `add_symbols`, `remove_file`, `batch_update`, `clear`, `search`, `doc_count`, `sanitize_query`.
- **FtsSymbol** (struct) — Symbol prepared for Tantivy indexing.

### Module: tree_cache

- **TreeCache** (struct) — LRU cache of tree-sitter parse trees; methods: `new`, `with_default_capacity`, `get`, `put`, `remove`, `len`, `is_empty`, `clear`, `capacity`.

### Module: watcher

- **WatchEvent** (enum) — Filesystem event (Created, Changed, Deleted, Renamed).
- **CodeWatcher** (struct) — Directory watcher; methods: `new`, `root`, `should_ignore`.

### Module: worker

- **WorkerConfig** / **WorkerStats** / **IndexWorkerHandle** / **EventBatch** / **IndexWorker** (structs) — Background indexing worker.
- **IndexWorker::start(index, event_rx, config)** — Start the worker.
- **IndexWorkerHandle** methods: `stop`, `queue_reindex`, `queue_full_reindex`, `stats`, `is_stopped`.
- **EventBatch** methods: `new`, `is_empty`, `len`, `push`.

### Module: graph

- **SymbolGraph** (struct) — Graph query/build interface; methods: `new`, `build`, `build_for_language`, `explain`, `path`, `communities`, `godnodes`, `export_json`, `export_report`, `edge_count`, `edge_count_by_confidence`, `all_edges`.
- **ExplainResult** / **Connection** / **PathResult** / **GodNode** / **CommunityInfo** / **BuildResult** (structs) — Graph query results.
- Submodules: `edges`, `resolve`, `traverse`, `communities`, `export`.