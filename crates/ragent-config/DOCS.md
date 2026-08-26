# ragent-config

Configuration types, defaults, parsing, permission primitives, bash/directory
allowlists, and feature toggles for ragent. Loaded with layered precedence
(defaults -> global -> project -> env).

## Workspace Dependencies

- ragent-types

## External Dependencies

- anyhow
- chrono
- dirs
- globset
- serde
- serde_json
- thiserror
- tracing

## Public API (crate root)

### Modules

- **bash_lists** — Runtime bash command allowlist/denylist management with config persistence.
- **compaction** — Compaction configuration types for context-window summarisation.
- **config** — Core configuration loading, merging, and all top-level config types.
- **dir_lists** — Runtime directory/file glob allowlist/denylist management for permissions.
- **edit_log** — Edit-operation logging toggle with config persistence.
- **finance** — Paid finance-provider configuration (Yahoo/Alpha Vantage).
- **permission** — Permission checking and access-control primitives.
- **telemetry** — OpenTelemetry metrics export configuration.
- **trigger** — Dynamic trigger rule system and MCP notification injection configuration.
- **yolo** — YOLO mode toggle (bypass all command validation) with config persistence.

### Re-exported types

- **CompactionConfig** (struct) — Top-level compaction configuration (auto, threshold, buffer, keep).
- **KeepConfig** (struct) — Recent conversation turns to keep verbatim after compaction.
- **AgentConfig** (struct) — Per-agent configuration overrides.
- **AgentPerfConfig** (struct) — Agent-loop performance configuration (step budget, stall timeout, concurrency).
- **AutoExtractConfig** (struct) — Automatic memory extraction configuration.
- **BrowserConfig** (struct) — Browser automation (CDP) configuration.
- **Capabilities** (struct) — LLM provider capability flags (vision, tools, etc.).
- **ChannelsConfig** (struct) — External messaging channel configuration (Telegram/Discord).
- **Config** (struct) — Top-level ragent configuration; loaded with layered precedence.
- **Cost** (struct) — Per-token cost model for an LLM model.
- **CrossProjectConfig** (struct) — Cross-project memory search configuration.
- **DiscordChannelConfig** (struct) — Discord webhook channel configuration.
- **GitLabIntegrationConfig** (struct) — GitLab integration configuration.
- **GmailConfig** (struct) — Gmail OAuth2 client credentials configuration.
- **McpServerConfig** (struct) — MCP server definition (command, args, env, transport).
- **McpTransport** (enum) — MCP transport type (stdio).
- **MemoryConfig** (struct) — Memory system configuration (blocks, structured store, retrieval).
- **ModelConfig** (struct) — Per-model configuration (costs, capabilities, context window).
- **PieGapConfig** (struct) — Pie feature gap toggles (opt-in feature flags).
- **PriceEntry** (struct) — User-defined price override entry for cost estimation.
- **ProviderConfig** (struct) — LLM provider configuration (API, models, thinking).
- **ResearchConfig** (struct) — Research subsystem configuration (OA recovery, Unpaywall).
- **SddConfig** (struct) — Spec-Driven Development capability toggles.
- **StreamConfig** (struct) — LLM streaming configuration (timeouts, retries).
- **TelegramChannelConfig** (struct) — Telegram bot channel configuration.
- **ToolVisibilityConfig** (struct) — Tool-family visibility switches (hide tool families from LLM).
- **tool_family_names** (fn) — Maps a switch name to its tool family list.
- **Permission** (enum) — Permission types (Read, Edit, Bash, Web, Custom, etc.).
- **PermissionAction** (enum) — Action for a matched rule (Allow, Deny, Ask).
- **PermissionChecker** (struct) — Evaluates permission rules to decide allow/deny/ask.
- **PermissionDecision** (enum) — Result of a permission check.
- **PermissionRequest** (struct) — A request for permission on a specific operation.
- **PermissionRule** (struct) — A single glob-based permission rule.
- **OtelConfig** (struct) / **OtelProtocol** (enum) / **TelemetryConfig** (struct) — OTLP telemetry configuration.
- **McpNotificationMode** (enum) — MCP notification injection mode (None, InjectSummary, InjectAndRun).
- **TriggerConfig** (struct) — Dynamic trigger rule system configuration (poll interval, max rules).

### Module: bash_lists

- **BashLists** (struct) — In-memory snapshot of merged bash allowlist and denylist.
- **Scope** (enum) — Config file target (Project, Global).
- **load_from_config** / **get_allowlist** / **get_denylist** / **is_allowlisted** / **matches_denylist** (fns) — Access and query.
- **add_allowlist** / **remove_allowlist** / **add_denylist** / **remove_denylist** (fns) — Mutate and persist.

### Module: compaction

- **CompactionConfig** (struct) / **KeepConfig** (struct) — Config types.
- **keep_fraction** / **summary_output_tokens** / **tool_output_max_chars** (methods) — Derived limits.

### Module: config

- **Config** (struct) — Top-level config; methods: `load`, `save`, `save_to_source`, `global_config_dir`, `global_config_path`, `backup_global_config`, `restore_global_config`, `effective_hidden_tools`, `merge`, `thinking_config_for_model`.
- Supporting structs: `ToolVisibilityConfig`, `ToolVisibilitySpecified`, `AgentPerfConfig`, `StreamConfig`, `CodeIndexConfig`, `CodeIndexSpecified`, `ProviderConfig`, `ApiConfig`, `ModelConfig`, `Cost`, `Capabilities`, `AgentConfig`, `BashConfig`, `DirsConfig`, `CommandDef`, `McpServerConfig`, `PriceEntry`, `McpTransport`, `ExperimentalFlags`, `MemoryConfig`, `StructuredMemoryConfig`, `SemanticConfig`, `RetrievalConfig`, `AutoExtractConfig`, `DecayConfig`, `CrossProjectConfig`, `GitLabIntegrationConfig`, `BrowserConfig`, `ChannelsConfig`, `TelegramChannelConfig`, `DiscordChannelConfig`, `GmailConfig`, `SddConfig`, `PieGapConfig`, `ResearchConfig`.
- **tool_family_names** (fn) — Maps visibility switch name to tool family list.

### Module: dir_lists

- **BUILTIN_ALLOWLIST** / **BUILTIN_DENYLIST** (consts) — Built-in glob patterns.
- **get_builtin_lists** (fn) — Returns built-in patterns.
- **DirLists** (struct) — In-memory snapshot of merged directory allowlist/denylist.
- **CompiledDirLists** (struct) — Compiled glob patterns with `is_allowed` / `is_denied`.
- **Scope** (enum) — Config file target (Project, Global).
- **load_from_config** / **get_allowlist** / **get_denylist** / **get_compiled_allowlist** / **get_compiled_denylist** / **invalidate_compiled_caches** (fns) — Access/query/invalidate.
- **add_allowlist** / **remove_allowlist** / **add_denylist** / **remove_denylist** (fns) — Mutate and persist.

### Module: edit_log

- **is_enabled** / **set_enabled** / **toggle** / **persist_edit_log** / **sync_from_config** / **toggle_persist** (fns) — Edit-log state management.

### Module: finance

- **FinanceProviderConfig** (struct) — Finance provider selection and credentials.
- **is_paid_provider_configured** / **is_explicitly_configured** / **yahoo_fallback_enabled** (methods) — Provider state queries.

### Module: permission

- **PermissionAction** (enum) — Allow, Deny, Ask.
- **Permission** (enum) — Permission type (Read, Edit, Bash, Web, Question, PlanEnter, PlanExit, Task, ExternalDirectory, DoomLoop, Custom).
- **PermissionRule** (struct) — Glob-based rule.
- **PermissionRequest** (struct) — Request for a specific operation.
- **PermissionChecker** (struct) — Methods: `new`, `check`, `record_always`.
- **PermissionDecision** (type) — Result of a permission check.

### Module: telemetry

- **OtelProtocol** (enum) — Http or Grpc.
- **OtelConfig** (struct) — OTLP export config; method: `validate`.
- **TelemetryConfig** (struct) — Top-level telemetry config; methods: `merge`, `is_enabled`.

### Module: trigger

- **TriggerConfig** (struct) — Methods: `poll_interval`, `is_enabled`, `is_empty`.
- **McpNotificationMode** (enum) — None, InjectSummary, InjectAndRun; methods: `is_none`, `is_inject_summary`, `is_inject_and_run`.

### Module: yolo

- **is_enabled** / **set_enabled** / **toggle** / **persist_yolo** / **sync_from_config** / **toggle_persist** (fns) — YOLO mode state.