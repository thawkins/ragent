# ragent-tools-core

Essential file, shell, and search tools for the ragent agent framework.
Provides the core `Tool` trait, `ToolRegistry`, `ToolContext`, path-security
helpers, and ~25 concrete tool implementations.

## Workspace Dependencies

- ragent-types
- ragent-config

## External Dependencies

- tokio, async-trait, serde, serde_json, anyhow, tracing
- globset, walkdir, similar, lru
- grep-regex, grep-searcher, ignore, regex, rayon
- which, chrono, uuid, dirs, rust_xlsxwriter

Dev-dependencies: tempfile, filetime.

## Public API (crate root)

### Core abstractions

- **ToolOutput** (struct) — `{ content: String, metadata: Option<Value> }` — returned by every tool.
- **CanonicalPathCache** (struct) — Per-step cache of `canonicalize()` results; methods: `new`, `get_or_canonicalize`.
- **ToolContext** (struct) — `{ session_id, working_dir, event_bus, read_timestamps, canonical_cache }` — passed to every `execute()`.
- **Tool** (trait) — `name()`, `description()`, `parameters_schema()`, `permission_category()`, `async execute()`.
- **ToolRegistry** (struct) — `RwLock<HashMap<String, Arc<dyn Tool>>>` + hidden set; methods: `new`, `register`, `get`, `list`, `contains`, `remove`, `clear`, `set_hidden`, `definitions`.
- **create_core_registry** (fn) — Registers all core tools (apply_patch, read, write, create, edit, multiedit, patch, copy, move, rm, mkdir, append, file_info, diff, glob, list, grep, bash, bash_reset, open, agent_complete, think, get_env, calculator).

### Path security

- **check_path_within_root** (fn) — Path escape guard; canonicalises + longest-existing-prefix walk.
- **check_path_within_root_cached** (fn) — Cached variant using `CanonicalPathCache`.
- **is_path_within** (fn) — Component-equality containment check (no string prefix).
- **is_alias_within** (fn) — Walks parents comparing dev/inode (bind-mount / symlink detection).
- **same_file_identity** (fn) — Platform-specific file identity comparison.
- **CleanPath** (trait + impl) — In-place `.`/`..` cleaning without touching disk.

### Compatibility re-exports

- **event** (module) — Re-export of `ragent_types::event::{Event, EventBus}`.
- **sanitize** (module) — Re-export of `ragent_types::sanitize::*`.
- **resource** (module) — Re-export of `ragent_types::resource`.

### Modules (tool implementations)

- **append_file** — `AppendFileTool` (`append_to_file`).
- **apply_patch** — `ApplyPatchTool` (`apply_patch`) — Codex-style patch parser.
- **copy_file** — `CopyFileTool` (`copy_file`).
- **create** — `CreateTool` (`create`).
- **edit** — `EditTool` (`edit`) — 3-lane match cascade.
- **multiedit** — `MultiEditTool` (`multiedit`) — Atomic batch edits.
- **patch** — `PatchTool` (`patch`) — Unified-diff.
- **read** — `ReadTool` (`read`) — LRU-cached file reader.
- **write** — `WriteTool` (`write`).
- **rm** — `RmTool` (`rm`).
- **mkdir** — `MakeDirTool` (`make_directory`).
- **move_file** — `MoveFileTool` (`move_file`).
- **file_info** — `FileInfoTool` (`file_info`).
- **diff** — `DiffFilesTool` (`diff_files`).
- **glob** — `GlobTool` (`glob`) — Rayon-parallel walk.
- **grep** — `GrepTool` (`grep`) — ripgrep library.
- **list** — `ListTool` (`list`).
- **bash** — `BashTool` (`bash`) — 7-layer security, persistent shell state.
- **bash_reset** — `BashResetTool` (`bash_reset`).
- **bg** — `BackgroundCommand` — background shell task lifecycle.
- **open** — `OpenTool` (`open`).
- **agent_complete** — `AgentCompleteTool` (`agent_complete`) — terminal signal.
- **think** — `ThinkTool` (`think`).
- **calculator** — `CalculatorTool` (`calculator`).
- **get_env** — `GetEnvTool` (`get_env`).
- **replace** — Three-lane match cascade helpers (exact -> whitespace-flexible -> indent-normalised).
- **path_util** — `resolve_path(working_dir, path_str)`.
- **edit_common** — Stale-file detection, edit timestamp recording.
- **edit_log** — JSONL edit audit log: `is_edit_log_enabled`, `log_edit_operation`, `edit_log_summary`, `detect_old_str_risks`, `edit_log_analyse`, `edit_log_stats`.
- **cron_log** — JSONL cron execution log: `CronOutcome`, `log_cron_execution`, `read_cron_log`, `clear_cron_logs`.
- **truncate** — `truncate_content`, `truncate_content_head_tail`, `get_truncation_stats`.
- **xlsx** — `write_xlsx(path, content)` helper.
- **file_lock** — `lock_file(path)` per-file mutex.