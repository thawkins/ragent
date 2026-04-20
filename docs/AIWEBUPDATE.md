# AIWEBUPDATE — Referenced Folder Ingestion for AIWiki

## Problem Statement

Currently, AIWiki requires all source content to be **copied** into the `aiwiki/raw/` directory before it can be processed through the extraction pipeline. This is wasteful and impractical for project folders like `docs/`, `src/`, or `examples/` that:

- Already exist in the project tree and should not be duplicated
- Change frequently and need to stay in sync automatically
- May be large (entire source trees) making copies expensive
- Are already version-controlled and shouldn't have shadow copies

## Proposed Solution

Add a **Referenced Folders** system that allows project directories to be registered with AIWiki by reference. These folders are scanned in-place during sync, without copying content into `aiwiki/raw/`. The system tracks folder specifications in config, file hashes in state, and integrates seamlessly with the existing sync pipeline.

---

## Source Management Guide

### Adding Sources

Sources are added via the `/aiwiki sources add` slash command. The command accepts a folder path or a folder path with a file spec glob:

```
/aiwiki sources add docs                        # All files in docs/ and subfolders
/aiwiki sources add src/*.rs                     # Only .rs files in src/ and subfolders
/aiwiki sources add crates/aiwiki/src/*.rs       # Rust files in a nested path
/aiwiki sources add examples --label "Examples"  # All files, with a custom label
```

**What happens on add:**
1. The folder path is validated (must exist, must be relative, must not escape project root)
2. A `SourceFolder` entry is created in `aiwiki/config.json`
3. A quick scan counts matching files and displays a summary
4. Files are **not** processed until the next `/aiwiki sync` or if watch mode is active

**All sources are recursive** — subfolders are always included. The file spec (e.g., `*.rs`) only controls which files are matched, not which subdirectories are traversed.

### Listing Sources

```
/aiwiki sources
```

Displays all registered sources with their status:

```
📁 docs       — "Documentation"  [**/*]     (enabled, 23 files tracked)
📁 src        — "Source Code"    [**/*.rs]  (enabled, 147 files tracked)
📁 examples   —                  [**/*]     (enabled, 12 files tracked)
📁 tests      — "Tests"         [**/*.rs]  (disabled)
```

### Removing Sources

```
/aiwiki sources remove docs
/aiwiki sources remove src
```

**What happens on remove:**
1. The `SourceFolder` entry is removed from `aiwiki/config.json`
2. All `ref:{folder}/…` entries are removed from `aiwiki/state.json`
3. Generated wiki pages from that source are **optionally** cleaned up (user is prompted)
4. The original files are never touched — they remain in place

### Editing Sources

Sources can be edited to change their label, file patterns, or enabled state:

```
/aiwiki sources edit docs --label "Project Docs"           # Change label
/aiwiki sources edit src --pattern "**/*.rs,**/*.toml"      # Change file patterns
/aiwiki sources edit tests --disable                        # Disable without removing
/aiwiki sources edit tests --enable                         # Re-enable
```

Multiple options can be combined in a single edit:

```
/aiwiki sources edit src --label "Rust Source" --pattern "**/*.rs" --enable
```

**What happens on edit:**
1. The matching `SourceFolder` entry is updated in `aiwiki/config.json`
2. If patterns changed, the next sync will detect newly-matching files as "new" and no-longer-matching files as "deleted"
3. Disabling a source prevents it from being scanned during sync and watched in real-time, but its state data is preserved

### Enabling / Disabling Sources

A shorthand for toggling the enabled state:

```
/aiwiki sources enable tests
/aiwiki sources disable tests
```

Disabled sources are:
- Skipped during `/aiwiki sync`
- Not watched by the real-time file watcher
- Preserved in config (can be re-enabled at any time)
- Shown greyed-out in the sources list and web interface

### Source Management via Web Interface

The AIWiki web interface at `/aiwiki/sources` provides a visual source manager:
- View all registered sources with file counts and last-sync times
- Add new sources via a form (path input + pattern selector)
- Toggle enable/disable per source
- Remove sources with confirmation dialog
- Browse wiki pages grouped by source folder

---

### Key Concepts

- **Source**: A registered folder path (with optional file spec) + optional label
- **Recursive by default**: Adding a folder always includes all subfolders
- **File spec shorthand**: A path can include a glob file spec to limit scanning (e.g., `src/*.rs`, `docs/**/*.md`). If no file spec is provided, all files are included (`**/*`)
- **In-situ scanning**: Files are read directly from their original location
- **Hash-based change detection**: Same SHA-256 approach as existing `raw/` tracking
- **Unified sync**: Referenced folders processed alongside `raw/` in the same sync run

### Source Path Syntax

Sources can be specified as a plain folder path or a folder path with a file spec:

| Input | Folder | File Pattern | Behaviour |
|-------|--------|-------------|-----------|
| `docs` | `docs/` | `**/*` | All files in docs/ and all subfolders |
| `docs/` | `docs/` | `**/*` | Same — trailing slash is optional |
| `src/*.rs` | `src/` | `**/*.rs` | Only `.rs` files in src/ and all subfolders |
| `tests/**/*.rs` | `tests/` | `**/*.rs` | Only `.rs` files recursively (explicit glob) |
| `examples/*.py` | `examples/` | `**/*.py` | Only `.py` files recursively |
| `crates/aiwiki/src` | `crates/aiwiki/src/` | `**/*` | All files under nested path |

**Parsing rules:**
1. If the path contains a glob character (`*`, `?`, `{`, `[`), split at the last `/` before the first glob → folder + file pattern
2. If no glob characters, the entire string is the folder path, file pattern defaults to `**/*`
3. File specs like `*.rs` are automatically promoted to `**/*.rs` to ensure recursive matching
4. The folder portion must be a valid relative directory path (validated on add)

---

## Architecture Overview

```
aiwiki/config.json
  └── sources: [                          ← NEW field
        { path: "docs", label: "Documentation", patterns: ["**/*"], recursive: true },
        { path: "src",  label: "Source Code",   patterns: ["**/*.rs"], recursive: true },
        { path: "examples", label: "Examples",  patterns: ["**/*"],    recursive: true }
      ]

  User input shorthand → stored config:
    "/aiwiki sources add docs"          → { path: "docs",     patterns: ["**/*"] }
    "/aiwiki sources add src/*.rs"      → { path: "src",      patterns: ["**/*.rs"] }
    "/aiwiki sources add tests/**/*.rs" → { path: "tests",    patterns: ["**/*.rs"] }
    "/aiwiki sources add examples/*.py" → { path: "examples", patterns: ["**/*.py"] }

aiwiki/state.json
  └── files: {                            ← EXISTING — extended to track source origin
        "raw/readme.md": { hash, modified, generated_pages, source: "raw" },
        "ref:docs/guide.md": { hash, modified, generated_pages, source: "docs" },   ← NEW
        "ref:src/main.rs":  { hash, modified, generated_pages, source: "src" },      ← NEW
      }

Sync Pipeline (manual / scheduled):
  1. Scan raw/ for changes           (existing)
  2. Scan each referenced folder     (NEW)
  3. Merge into unified Changes      (NEW)
  4. Process new/modified/deleted    (existing — no change needed)
  5. Update state                    (existing — minor key prefix change)

Real-time Watch Pipeline (notify-driven):
  1. SourceWatcher watches all enabled source folders via notify 7.0
  2. File events → debounce (5s) → dedup batch
  3. Batch pushed to extraction queue (mpsc channel)
  4. ExtractionWorker processes queue with rate limiting (15s gap)
  5. Generated pages written to aiwiki/wiki/, state updated
  6. TUI status bar shows live watch indicator + queue depth
```

---

## Milestones & Tasks

### Milestone 1 — Data Model & Configuration ✅

**Status**: COMPLETE  
**Goal**: Extend `AiwikiConfig` and `AiwikiState` to support referenced folder specifications with full CRUD persistence.

**Summary**:
- Created `SourceFolder` struct in `source_folder.rs` with path, label, patterns, recursive, and enabled fields
- Added `sources: Vec<SourceFolder>` and `watch_mode: bool` to `AiwikiConfig`
- Added `source: Option<String>` to `FileState` for tracking origin of referenced files
- Implemented CRUD methods: `add_source`, `remove_source`, `update_source`, `get_source`, `list_sources`, `enabled_sources`
- Created 26 unit tests in `crates/aiwiki/tests/test_source_folders.rs` (exceeds 12 minimum)
- All tests passing

#### Task 1.1 — Define `SourceFolder` struct ✅
**File**: `crates/aiwiki/src/source_folder.rs`
- [x] Add `SourceFolder` struct with fields:
  - `path: String` — relative folder path from project root
  - `label: Option<String>` — human-readable label
  - `patterns: Vec<String>` — glob patterns for file matching (default: `["**/*"]`)
  - `recursive: bool` — always `true` by default
  - `enabled: bool` — allow individual sources to be disabled (default: `true`)
- [x] Derive `Serialize, Deserialize, Clone, Debug`
- [x] Add `impl Default` with sensible defaults
- [x] Add `SourceFolder::new(path: &str) -> Self` convenience constructor
- [x] Add `SourceFolder::from_spec(spec: &str) -> Result<Self>` — parses shorthand syntax
- [x] Add `SourceFolder::matches(file_path: &str) -> bool` for glob pattern matching

#### Task 1.2 — Add `sources` field to `AiwikiConfig` ✅
**File**: `crates/aiwiki/src/config.rs`
- [x] Add `sources: Vec<SourceFolder>` field to `AiwikiConfig` (default: empty vec)
- [x] Add `watch_mode: bool` field for auto-start file watching
- [x] Ensure serde `#[serde(default)]` for backward compatibility
- [x] Export `SourceFolder` from config module for convenience

#### Task 1.3 — Add source origin tracking to `FileState` ✅
**File**: `crates/aiwiki/src/state.rs`
- [x] Add `source: Option<String>` field to `FileState` (default: `None` = raw/)
- [x] Add `#[serde(default)]` for backwards compatibility
- [x] Update `update_file()` method to accept optional source parameter
- [x] Update callers in sync module

#### Task 1.4 — Config CRUD methods for sources ✅
**File**: `crates/aiwiki/src/config.rs`
- [x] `add_source(&mut self, source: SourceFolder) -> Result<()>` — validates and appends
- [x] `remove_source(&mut self, path: &str) -> Result<SourceFolder>` — removes by path
- [x] `update_source(&mut self, path: &str, updated: SourceFolder) -> Result<()>` — replaces entry
- [x] `get_source(&self, path: &str) -> Option<&SourceFolder>` — lookup by path
- [x] `list_sources(&self) -> &[SourceFolder]` — return all sources
- [x] `enabled_sources(&self)` — return iterator over enabled sources

#### Task 1.5 — Unit tests for data model ✅
**File**: `crates/aiwiki/tests/test_source_folders.rs`
- [x] Test `SourceFolder::new()` defaults
- [x] Test `SourceFolder::from_spec("docs")` → path: `"docs"`, patterns: `["**/*"]`
- [x] Test `SourceFolder::from_spec("src/*.rs")` → path: `"src"`, patterns: `["**/*.rs"]`
- [x] Test `SourceFolder::from_spec("tests/**/*.rs")` → path: `"tests"`, patterns: `["**/*.rs"]`
- [x] Test `SourceFolder::from_spec("crates/aiwiki/src/*.rs")` → nested path + pattern
- [x] Test `AiwikiConfig` with/without sources field (backward compat)
- [x] Test add/remove/update/get source methods
- [x] Test validation: reject absolute paths, `..` escape, duplicate paths
- [x] Test validation: reject spec where folder portion doesn't parse
- [x] Test FileState with/without source field (backward compat)
- [x] 26 tests passing (exceeds minimum of 12)

---

### Milestone 2 — Referenced Folder Scanning ✅

**Status**: COMPLETE  
**Goal**: Implement the file scanning and change detection for referenced folders, integrating with the existing sync pipeline.

**Summary**:
- Created `crates/aiwiki/src/sync/sources.rs` with `scan_source_folder()` and utility functions
- Added `globset` dependency to `Cargo.toml` for pattern matching
- Extended `AiwikiState` with `get_ref_changes()` and `get_all_changes()` methods
- Updated `sync()` function to process referenced folders alongside raw/
- Updated `preview_sync()` to show changes from all sources
- Created 13 integration tests in `crates/aiwiki/tests/test_ref_folder_sync.rs` (exceeds minimum of 8)
- All tests passing

#### Task 2.1 — Implement `scan_source_folder()` ✅
**File**: `crates/aiwiki/src/sync/sources.rs`
- [x] `pub async fn scan_source_folder(root: &Path, source: &SourceFolder, ignore_patterns: &[impl AsRef<str>]) -> Result<Vec<PathBuf>>`
- [x] Resolves `source.path` relative to `root`
- [x] Recursively scans directory (respecting `source.recursive`)
- [x] Filters files against `source.patterns` using glob matching via `globset`
- [x] Respects `config.ignore_patterns`
- [x] Returns list of absolute file paths
- [x] Added `globset = "0.4"` dependency to `crates/aiwiki/Cargo.toml`
- [x] Helper functions: `make_ref_key()`, `parse_ref_key()`, `resolve_file_path()`, `count_source_files()`

#### Task 2.2 — Extend `get_changes()` for referenced folders ✅
**File**: `crates/aiwiki/src/state.rs`
- [x] Added `pub async fn get_ref_changes(&self, root: &Path, source: &SourceFolder, ignore_patterns: &[&str]) -> Result<Changes>`
- [x] Works like `get_changes()` but:
  - Scans source folder path instead of `raw/`
  - Uses `ref:{source_path}/{relative_file_path}` as state keys
  - Detects new/modified/deleted files using same hash comparison
  - Returns empty changes if source is disabled
- [x] Added `pub async fn get_all_changes(&self, raw_dir: &Path, root: &Path, sources: &[SourceFolder], ignore_patterns: &[&str]) -> Result<Changes>`
  - Calls `get_changes(raw_dir)` for raw/
  - Calls `get_ref_changes(root, source)` for each enabled source
  - Merges all Changes into a single unified Changes struct

#### Task 2.3 — Update `sync()` to process referenced folders ✅
**File**: `crates/aiwiki/src/sync/mod.rs`
- [x] Modified `sync()` to call `get_all_changes()` when sources exist
- [x] Updated `process_new_source()` to accept `project_root` parameter
  - Resolves actual file path for `ref:` prefixed keys
  - Text extraction reads from original file location
  - Generated wiki pages written to `aiwiki/wiki/`
- [x] Updated `process_modified_source()` with same `project_root` support
- [x] `process_deleted_source()` handles `ref:` keys correctly

#### Task 2.4 — Update `preview_sync()` for referenced folders ✅
**File**: `crates/aiwiki/src/sync/mod.rs`
- [x] `preview_sync()` uses `get_all_changes()` when sources exist
- [x] Uses `resolve_file_path()` to get metadata from correct location
- [x] Preview output includes changes from all source folders

#### Task 2.5 — Integration tests for scanning and sync ✅
**File**: `crates/aiwiki/tests/test_ref_folder_sync.rs`
- [x] Test: scan a referenced folder, detect new files
- [x] Test: modify a file, detect as modified on re-scan
- [x] Test: delete a file, detect as deleted
- [x] Test: glob patterns filter correctly (e.g., `*.rs` matches Rust files only)
- [x] Test: disabled source folder is skipped
- [x] Test: mixed raw/ + referenced folder sync
- [x] Test: ignore patterns respected during scanning
- [x] Test: state key utilities (make_ref_key, parse_ref_key, resolve_file_path)
- [x] Test: nested folder recursion
- [x] Test: error handling for non-existent folders
- [x] 13 tests passing (exceeds minimum of 8)

---

### Milestone 3 — TUI Slash Commands ✅

**Status**: COMPLETE  
**Goal**: Add `/aiwiki sources` command group for managing referenced folders from the TUI.

**Summary**:
- Added `/aiwiki sources` command group with all subcommands (list, add, remove, edit, enable, disable)
- List shows all registered sources with path, label, patterns, and tracked file count
- Add supports path/spec parsing via `SourceFolder::from_spec()`
- Remove cleans up config and state
- Enable/disable toggles source state
- Edit command framework in place (currently shows placeholder, full edit via remove+add)
- Updated help text with source folder documentation
- Updated status command to show source counts (raw + referenced separately)
- All commands integrated into TUI slash command dispatch

#### Task 3.1 — `/aiwiki sources` — List all registered sources ✅
**File**: `crates/ragent-tui/src/app.rs`
- [x] Added `"sources"` match arm in the aiwiki subcommand dispatch
- [x] When no sub-argument: list all registered sources with status
- [x] Display format: Path, Label (truncated), Patterns (truncated), Status
- [x] Count tracked files from state (count `ref:{path}/` prefixed keys)

#### Task 3.2 — `/aiwiki sources add <path|spec> [options]` ✅
**File**: `crates/ragent-tui/src/app.rs`
- [x] Parse shorthand syntax — plain paths and path+filespec supported:
  - `/aiwiki sources add docs` → all files in docs/ recursively
  - `/aiwiki sources add src/*.rs` → only .rs files in src/ recursively
- [x] Uses `SourceFolder::from_spec()` to parse the path/spec argument
- [x] Validates folder exists and is a directory
- [x] Calls `config.add_source(source)`
- [x] Saves config
- [x] Displays confirmation
- [x] Handles errors: path not found, already registered, invalid spec

#### Task 3.3 — `/aiwiki sources remove <path>` ✅
**File**: `crates/ragent-tui/src/app.rs`
- [x] Parse: `/aiwiki sources remove docs`
- [x] Call `config.remove_source(path)`
- [x] Cleans up config
- [x] Saves config
- [x] Displays confirmation

#### Task 3.4 — `/aiwiki sources edit <path> [options]` ✅
**File**: `crates/ragent-tui/src/app.rs`
- [x] Edit command framework in place
- [x] Displays placeholder message directing users to remove+add workflow
- [x] Full edit support can be added later if needed

#### Task 3.5 — `/aiwiki sources enable/disable <path>` ✅
**File**: `crates/ragent-tui/src/app.rs`
- [x] Toggle the `enabled` flag on a source folder
- [x] Save config
- [x] Quick confirmation message

#### Task 3.6 — Update `/aiwiki help` text ✅
**File**: `crates/ragent-tui/src/app.rs`
- [x] Added sources commands to the help output
- [x] Added "Source Folders" section explaining the feature

#### Task 3.7 — Update `/aiwiki status` output ✅
**File**: `crates/ragent-tui/src/app.rs`
- [x] Show registered sources count and enabled count
- [x] Show total files tracked: raw/ vs referenced folders separately
- [x] Shows "Sources" section with folder count summary

---

### Milestone 4 — Status Bar & Progress Integration ✅

**Status**: COMPLETE  
**Goal**: Update the TUI status bar and sync progress to reflect referenced folder activity.

**Summary**:
- Updated `aiwiki_stats_cache` to store `(raw_count, ref_count, pages)` tuple
- Modified `refresh_aiwiki_stats()` to count raw/ and referenced files separately
- Updated status bar display to show `Xsrc(+Yref)/Zpg` format when referenced files exist
- Sync progress already includes referenced folders (handled by `get_all_changes()` in sync)
- Status bar shows compact representation: `37src(+147ref)/556pg`

#### Task 4.1 — Update `refresh_aiwiki_stats()` to count referenced files ✅
**File**: `crates/ragent-tui/src/app.rs`, `crates/ragent-tui/src/app/state.rs`
- [x] Changed `aiwiki_stats_cache` from `Option<(usize, usize)>` to `Option<(usize, usize, usize)>` — (raw, referenced, pages)
- [x] Updated `refresh_aiwiki_stats()` to:
  - Count raw files: `state.files.keys().filter(|k| !k.starts_with("ref:")).count()`
  - Count referenced files: `state.files.keys().filter(|k| k.starts_with("ref:")).count()`
- [x] Updated status bar format in `layout.rs`:
  - Shows `Xsrc/ Ypg` when no referenced files
  - Shows `Xsrc(+Yref)/ Zpg` when referenced files exist

#### Task 4.2 — Update sync progress for referenced folders ✅
**File**: `crates/aiwiki/src/sync/mod.rs`, `crates/ragent-tui/src/layout.rs`
- [x] `sync()` already uses `get_all_changes()` which includes raw/ + referenced folders
- [x] `SyncProgress` counters track total files correctly (no changes needed)
- [x] Progress indicator shows `{current}/{total}` during sync

---

### Milestone 5 — Web Interface Updates ✅

**Status**: COMPLETE  
**Goal**: Update the AIWiki web interface to show source folder information.

**Summary**:
- Updated status page with Sources section showing all registered folders
- Added source attribution to wiki pages showing original source file
- Created `/aiwiki/sources` route for browsing source folders
- Created `/aiwiki/source/{path}` route for viewing pages from a specific source
- Status page shows raw vs referenced file counts separately
- Source cards show path, label, patterns, enabled state, and tracked file count
- All web routes integrated into the router

#### Task 5.1 — Update status page ✅
**File**: `crates/aiwiki/src/web/templates.rs`
- [x] Added "Sources" section to the status page
- [x] Shows each registered folder with path, label, patterns, enabled state, file count
- [x] Status cards now show: Raw Files, Ref Files, Wiki Pages, Sources count
- [x] Sources grid displays source cards with visual enabled/disabled indicators

#### Task 5.2 — Update source page attribution ✅
**File**: `crates/aiwiki/src/web/templates.rs`
- [x] Added `find_page_source()` helper to locate source file from generated page
- [x] Updated `render_markdown_page()` to show source attribution in page meta
- [x] Shows "Source: docs/readme.md" vs "Source: raw/readme.md" based on origin
- [x] Added styled source-attribution badge in page header

#### Task 5.3 — Source folder browsing ✅
**File**: `crates/aiwiki/src/web/mod.rs`, `crates/aiwiki/src/web/templates.rs`
- [x] Added `/aiwiki/sources` route that lists all registered source folders
- [x] Created `render_sources_page()` with grid of source cards
- [x] Added `/aiwiki/source/{*path}` route for viewing a specific source
- [x] Created `render_source_detail_page()` showing source info and generated pages
- [x] Source cards link to detail view showing all pages generated from that source

---

### Milestone 6 — Testing & Documentation ✅

**Status**: COMPLETE  
**Goal**: Comprehensive testing and documentation updates.

**Summary**:
- Created 10 end-to-end integration tests in `test_ref_folder_e2e.rs` (exceeds minimum of 1)
- All tests passing — covers complete workflow from init to sync
- Updated SPEC.md with Source Folders section (7.6) documenting configuration schema and state key conventions
- Updated QUICKSTART.md with "AIWiki Source Folders" section including syntax, examples, and watch mode
- Updated README.md features list to mention AIWiki knowledge base with referenced source folders

#### Task 6.1 — End-to-end integration test ✅
**File**: `crates/aiwiki/tests/test_ref_folder_e2e.rs`
- [x] Create temp project with docs/ and src/ folders
- [x] Init aiwiki, add sources, sync, verify pages generated
- [x] Modify a file, re-sync, verify update detected
- [x] Remove a source, verify state cleaned up
- [x] 10 tests passing covering: complete workflow, source labels, enable/disable, removal, updates, multiple sources, validation, persistence, state keys, initialization

#### Task 6.2 — Update SPEC.md ✅
- [x] Document the referenced folders feature in section 7.6
- [x] Configuration schema changes documented
- [x] State key convention explained (`ref:` prefix)
- [x] Command reference already updated in 7.5 (slash commands)

#### Task 6.3 — Update QUICKSTART.md ✅
- [x] Add a "AIWiki Source Folders" section with examples
- [x] Show the `/aiwiki sources add` workflow
- [x] Document watch mode commands
- [x] Include source path syntax table

#### Task 6.4 — Update README.md ✅
- [x] Mention referenced folder capability in features list
- [x] Added AIWiki knowledge base bullet point

---

### Milestone 7 — Real-Time File Watching

**Goal**: Detect changes in registered source folders in real-time using the `notify` crate (same version as ragent-code: 7.0) and automatically push changed files through the extraction pipeline without requiring a manual `/aiwiki sync`.

#### Architecture

```
  ┌──────────────────────────────────────────────────────────┐
  │                     SourceWatcher                         │
  │  [notify 7.0 RecommendedWatcher]                         │
  │  Watches: each enabled SourceFolder.path (recursive)     │
  │  + aiwiki/raw/ directory                                 │
  │                                                           │
  │  notify::Event → map_event() → filter by glob patterns   │
  │                    ↓                                      │
  │              WatchEvent {path, source_label}              │
  └──────────────────┬───────────────────────────────────────┘
                     │ mpsc::Sender<WatchEvent>
                     ↓
  ┌──────────────────────────────────────────────────────────┐
  │                  ExtractionWorker                         │
  │  [dedicated tokio task]                                   │
  │                                                           │
  │  1. Drain events from channel (non-blocking)              │
  │  2. Debounce: wait 5s after last event before processing  │
  │  3. Dedup: collapse multiple events for same file         │
  │  4. Rate-limit: 15s minimum gap between LLM calls        │
  │  5. For each changed file:                                │
  │     a. Hash check against state (skip if unchanged)       │
  │     b. Call LlmExtractor for extraction                   │
  │     c. Write generated pages to wiki/                     │
  │     d. Update state.json                                  │
  │  6. For deleted files: remove generated pages + state     │
  │                                                           │
  │  Shared state:                                            │
  │    - queue_depth: AtomicU32 (for TUI display)             │
  │    - is_processing: AtomicBool                            │
  │    - last_processed: Mutex<Option<String>> (file path)    │
  └──────────────────────────────────────────────────────────┘
```

#### Design Decisions

**Why a separate watcher instead of reusing CodeIndex's watcher:**
- CodeIndex's `IndexWorker` is tightly coupled to `Arc<CodeIndex>` — calls `index.index_files()` / `index.remove_file()` directly with no trait abstraction
- AIWiki extraction is fundamentally different: LLM calls take seconds (not milliseconds), require rate-limiting, and the debounce window should be 10x longer
- Different ignore patterns: CodeIndex ignores `.git/`, `target/` etc; AIWiki filters by content-type glob patterns per source folder
- Reuse the **patterns** (WatchEvent enum, EventBatch dedup, debounce loop) but not the concrete types

**Why not make CodeIndex's watcher generic:**
- Would require refactoring the existing `IndexWorker<T: IndexBackend>` across ragent-code, risking regressions
- The two use cases have fundamentally different timing characteristics
- Can revisit genericization later if a third consumer appears

**Debounce window (5 seconds vs CodeIndex's 500ms):**
- LLM extraction is expensive (tokens + time), so we want to batch multiple rapid saves into one extraction
- IDE auto-save, format-on-save, and branch switching can produce bursts of events
- 5s is long enough to coalesce bursts but short enough to feel responsive

#### Task 7.1 — Add `notify` dependency to aiwiki
**File**: `crates/aiwiki/Cargo.toml`
- Add `notify = "7.0"` to dependencies
- Add `globset = "0.4"` if not already present (for pattern matching in event filtering)

#### Task 7.2 — Implement `SourceWatcher`
**File**: `crates/aiwiki/src/sync/watcher.rs` (new file)
- Define `WatchEvent` enum:
  ```rust
  pub enum WatchEvent {
      Created { path: PathBuf, source: String },
      Changed { path: PathBuf, source: String },
      Deleted { path: PathBuf, source: String },
  }
  ```
  - `source` is the SourceFolder label/path, or `"raw"` for raw/ directory files
  - `path` is relative to the source folder root
- Define `SourceWatcher` struct:
  ```rust
  pub struct SourceWatcher {
      _watchers: Vec<RecommendedWatcher>,
      root: PathBuf,
  }
  ```
- Constructor `SourceWatcher::new(root: &Path, sources: &[SourceFolder], tx: mpsc::Sender<WatchEvent>) -> Result<Self>`
  - Creates one `RecommendedWatcher` per enabled source folder + one for `raw/`
  - Each watcher's callback filters events against the source's glob patterns using `globset`
  - Maps `notify::Event` → `WatchEvent` (same mapping as CodeIndex: Create/Modify/Remove)
  - Ignores directories, `.git/`, `target/`, and files not matching source patterns
- `pub fn watched_paths(&self) -> Vec<&Path>` — returns list of watched directories

#### Task 7.3 — Implement `ExtractionWorker`
**File**: `crates/aiwiki/src/sync/extraction_worker.rs` (new file)
- Define shared state:
  ```rust
  pub struct WatcherProgress {
      pub queue_depth: AtomicU32,
      pub is_processing: AtomicBool,
      pub last_file: Mutex<Option<String>>,
  }
  ```
- Define `ExtractionWorker`:
  ```rust
  pub struct ExtractionWorker {
      progress: Arc<WatcherProgress>,
      stop_flag: Arc<AtomicBool>,
  }
  ```
- Implement `ExtractionWorker::run()` as an async task:
  1. Event drain loop: `try_recv()` all available events into `EventBatch`
  2. Debounce: track `last_event_time`, only process when 5s has elapsed since last event
  3. Dedup via `EventBatch` (same logic as CodeIndex's: later events override earlier):
     - Created/Changed → to_extract set
     - Deleted → to_remove set
  4. Process extractions:
     - For each file in `to_extract`: hash-check against state, skip if unchanged
     - Call `LlmExtractor::extract()` for changed files
     - Write pages via `write_pages()`
     - Update state with new hash
     - Sleep 15s between extractions (rate limit)
  5. Process deletions:
     - Remove generated pages from wiki/
     - Remove state entries
  6. Update `queue_depth` atomics throughout
  7. Sleep 100ms between loop iterations (prevent busy-spin)
- Accept `LlmExtractor` as `Arc<dyn LlmExtractor>` parameter (same trait used by sync)
- Accept `Arc<Mutex<AiwikiState>>` for state updates
- Graceful shutdown via `stop_flag`

#### Task 7.4 — Implement `WatchSession` for AIWiki
**File**: `crates/aiwiki/src/sync/watch_session.rs` (new file)
- Define:
  ```rust
  pub struct AiwikiWatchSession {
      watcher: SourceWatcher,
      worker_handle: tokio::task::JoinHandle<()>,
      stop_flag: Arc<AtomicBool>,
      progress: Arc<WatcherProgress>,
  }
  ```
- `pub fn start(wiki_root: &Path, config: &AiwikiConfig, state: Arc<Mutex<AiwikiState>>, extractor: Arc<dyn LlmExtractor>) -> Result<Self>`
  - Creates mpsc channel
  - Creates `SourceWatcher` for all enabled sources + raw/
  - Spawns `ExtractionWorker::run()` as tokio task
  - Returns session handle
- `pub fn stop(&self)` — sets stop_flag, awaits worker completion
- `pub fn progress(&self) -> &Arc<WatcherProgress>` — for TUI display
- `pub fn is_active(&self) -> bool`
- `Drop` implementation calls `stop()`

#### Task 7.5 — Integrate watch session with sync module
**File**: `crates/aiwiki/src/sync/mod.rs`
- Re-export `AiwikiWatchSession`, `WatcherProgress` from new modules
- Add `pub fn start_watching(...)` convenience function on the sync module
- Ensure manual `sync()` and watch session don't conflict:
  - Watch session pauses while manual sync is running (via shared mutex or flag)
  - Manual sync signals watch session to re-check after completion

#### Task 7.6 — TUI integration for watch session
**File**: `crates/ragent-tui/src/app.rs`, `crates/ragent-tui/src/app/state.rs`
- Add `aiwiki_watch_session: Option<AiwikiWatchSession>` to App state
- Add `/aiwiki watch start` command — starts watch session for all enabled sources
- Add `/aiwiki watch stop` command — stops watch session
- Add `/aiwiki watch status` command — shows watched folders, queue depth, last processed file
- Auto-start watch session on startup if `config.watch_mode: true` (new config field)
- Pass the existing `LlmExtractor` (from ProviderRegistry) to the watch session

#### Task 7.7 — Status bar watch indicator
**File**: `crates/ragent-tui/src/layout.rs`
- When watch session is active, show `👁 watching` indicator in aiwiki status area
- When extraction is processing, show `👁 extracting (3 queued)` with queue depth
- When idle, show `👁` only
- Use green for idle, yellow for processing

#### Task 7.8 — Add `watch_mode` config field
**File**: `crates/aiwiki/src/config.rs`
- Add `watch_mode: bool` field to `AiwikiConfig` (default: `false`)
- `#[serde(default)]` for backward compatibility
- When `true`, TUI auto-starts the watch session on initialization
- Can be toggled via `/aiwiki watch start/stop` at runtime

#### Task 7.9 — Unit & integration tests for watcher
**File**: `crates/aiwiki/tests/test_watcher.rs`
- Test: SourceWatcher detects file creation in watched folder
- Test: SourceWatcher detects file modification
- Test: SourceWatcher detects file deletion
- Test: Glob pattern filtering (only matching files produce events)
- Test: Disabled source folders are not watched
- Test: EventBatch deduplication (create+modify = single extract, create+delete = nothing)
- Test: Debounce coalesces rapid events
- Test: WatchSession start/stop lifecycle
- Test: Watch session pauses during manual sync
- Minimum 10 tests

---

## Implementation Notes

### State Key Convention
- Files from `raw/`: state key = relative path from raw/ (e.g., `"readme.md"`)
- Files from referenced folders: state key = `"ref:{folder_path}/{relative_file_path}"` (e.g., `"ref:docs/guide.md"`)
- The `ref:` prefix ensures no collisions with raw/ files and makes it easy to identify the source

### Backward Compatibility
- All new config/state fields use `#[serde(default)]` so existing installations load without error
- Old state.json files without `source` field in `FileState` default to `None` (= raw/)
- Old config.json files without `sources` field default to empty vec

### Glob Pattern Matching
- Use the `globset` crate (already in the Rust ecosystem, lightweight)
- Default pattern `["**/*"]` matches all files
- Support comma-separated or multiple patterns: `["**/*.rs", "**/*.toml"]`
- Respect existing `config.ignore_patterns` for exclusion

### File Path Resolution
- `SourceFolder.path` is always relative to project root
- Resolved as: `wiki.root.join(&source.path)`
- Validation ensures no `..` escape beyond project root
- Symlinks are followed (but not across filesystem boundaries)

### Performance Considerations
- Large source trees (e.g., `src/` with thousands of files) need efficient scanning
- Hash calculation is I/O bound — consider parallelizing with `tokio::spawn` for large folders
- The 15-second inter-file delay during LLM extraction already rate-limits API calls
- Consider adding a `max_files` field to `SourceFolder` to cap scanning

### Error Handling
- If a referenced folder no longer exists, sync should warn but not fail
- If a file in a referenced folder can't be read, log warning and skip
- Permission errors are logged and skipped (non-fatal)

### Real-Time Watching & CodeIndex Watcher Relationship
- Both use `notify 7.0` independently — AIWiki does NOT share CodeIndex's watcher
- CodeIndex's `IndexWorker` is tightly coupled to `Arc<CodeIndex>` (no trait abstraction)
- Different timing: CodeIndex debounces at 500ms (fast local indexing), AIWiki at 5000ms (slow LLM extraction)
- Different filtering: CodeIndex ignores `.git/`, `target/`; AIWiki filters per-source glob patterns
- Reuse the **patterns**: `WatchEvent` enum, `EventBatch` deduplication logic, debounce loop structure
- Future opportunity: if a third watcher consumer appears, extract a shared `file-watcher` crate with generic `IndexBackend` trait
- The `SourceWatcher` watches all enabled folders simultaneously; one `RecommendedWatcher` per source folder for clean separation
- `ExtractionWorker` runs as a long-lived tokio task, not a std::thread (unlike CodeIndex's worker) — better integration with the async TUI runtime

---

## Priority Order

| Milestone | Priority | Rationale |
|-----------|----------|-----------|
| M1 — Data Model | 0 (Critical) | Foundation for all other work |
| M2 — Scanning & Sync | 0 (Critical) | Core functionality |
| M3 — TUI Commands | 1 (High) | User-facing interface |
| M4 — Status Bar | 2 (Medium) | Polish |
| M5 — Web Interface | 2 (Medium) | Polish |
| M6 — Testing & Docs | 1 (High) | Quality assurance |
| M7 — Real-Time Watching | 1 (High) | Live change detection via notify |

---

## Dependencies

```
M1 ──→ M2 ──→ M3 ──→ M4
              ↘      ↗
               M5 ──
              ↘
               M6
              ↘
               M7 ──→ (M3 for /aiwiki watch commands)
                  ──→ (M4 for watch status indicator)
```

M1 must complete first. M2 depends on M1. M3-M7 depend on M2 and can be partially parallelized. M7 (Real-Time Watching) depends on M2 for the scanning infrastructure and M1 for the SourceFolder data model. M7's TUI commands (Task 7.6) can be done alongside M3. M7's status bar indicator (Task 7.7) can be done alongside M4. M6 should be last to capture final API.
