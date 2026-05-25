# Changelog

## Version: 0.1.0-alpha.97

### Changed
- **add rate limiting logic**

## Version: 0.1.0-alpha.96

### Changed
- **fix for azure resource provider**
- **Azure AI Foundry / Azure Resource 429 rate-limit retry** — HTTP 429 (Too Many Requests) responses from Azure AI Foundry and Azure Resource endpoints are now treated as retryable. The `execute_with_retry` helper in `http_client.rs` has been updated to:
  - Detect `429 Too Many Requests` status code and automatically retry up to 4 times
  - Respect the `Retry-After` response header when present (integer seconds format)
  - Fall back to exponential backoff (2ˢ, 4ˢ, 8ˢ, 16ˢ) when no `Retry-After` header is provided
  - Log each retry attempt with the delay duration for transparency
- **AzureFoundryClient now uses `execute_with_retry`** — The chat request in `azure_foundry.rs` now routes through the retry-aware `execute_with_retry` path, so transient 429 errors will be handled automatically instead of surfacing as immediate failures.

### Changed
- **YOLO mode fixes** — Fixed YOLO mode permission bypass logic.
- **AGENTS.md search and inclusion order** — Updated AGENTS.md discovery and inclusion order handling.

## Version: 0.1.0-alpha.93

### Changed
- **Built-in agents no longer hardcode Anthropic Claude** — All 18 built-in agent definitions (`ask`, `general`, `build`, `plan`, `explore`, `title`, `summary`, `compaction`, `rust-coder`, `python-coder`, `go-coder`, `typescript-coder`, `java-coder`, `cpp-coder`, `csharp-coder`, `swift-coder`, `database-agent`, `frontend-agent`) previously hardcoded `model: Some(ModelRef { provider_id: "anthropic", model_id: "claude-sonnet-4-20250514" })`. They now default to `model: None` and auto-resolve the first available model from the provider registry at runtime. This means agents automatically use whatever provider/model the user has configured via `/provider`, `/model`, or `--model` instead of always falling back to Claude.

### Added
- **`resolve_default_model()` helper** — Scans the provider registry and returns the first model from the first provider, used when an agent has no explicit model binding.
- **`resolve_agent_with_model()` / `resolve_agent_with_customs_and_model()`** — Wrappers around `resolve_agent()` / `resolve_agent_with_customs()` that ensure the returned agent always has a model by falling back to `resolve_default_model()` when needed.

### Fixed
- **TUI initial agent setup** — `App::new()` now calls `resolve_default_model()` on the initial agent so startup works even when no model was previously persisted in storage.
- **TUI agent switching** — `apply_selected_model_and_thinking()` now falls back to `resolve_default_model()` when both `selected_model` and `agent.model` are `None`.
- **Server message handler** — `POST /sessions/{id}/messages` now uses `resolve_agent_with_model()` instead of `resolve_agent()` so server requests also auto-resolve the default model.

## Version: 0.1.0-alpha.91

### Fixed
- **TUI 5-minute stall / frozen ESC** — `refresh_memory_stats()` was doing synchronous file I/O (`load_all_blocks` reads ALL memory blocks from disk) + SQLite query on every single event-loop tick (~50 ms) with zero debouncing. When many memory blocks exist or SQLite has lock contention, the entire async runtime blocks for seconds, preventing keyboard events from being processed (ESC appears frozen). Added 5-second debounce to `refresh_memory_stats()` matching the pattern used by `refresh_code_index_stats()`. Also added 2-second debounces to `poll_swarm_unblock()` and `poll_swarm_completion()` which were doing filesystem I/O (`TeamStore::load_by_name`, `TaskStore::open`) on every tick.
- **Question dialog not rendering** — `Event::QuestionRequested` handler in `app.rs` was missing `self.needs_redraw = true`, causing the question dialog to never appear on screen until an unrelated input or event triggered a redraw. Added the flag so the dialog renders immediately when a `question` tool call arrives.

### Changed
- **Agent loop optimization** — Optimize the agent loop to prevent stalls.

## Version: 0.1.0-alpha.90

### Added
- **Git tool summaries in TUI** — `tool_input_summary` and `tool_output_summary` in `message_widget.rs` now provide human-readable summaries for all 16 git tools (`git_add`, `git_branch`, `git_checkout`, `git_cherry_pick`, `git_clone`, `git_commit`, `git_diff`, `git_fetch`, `git_log`, `git_merge`, `git_pull`, `git_push`, `git_remote`, `git_reset`, `git_show`, `git_stash`), showing actions like "🌿 add -A", "🌿 commit --amend", "🌿 merge feature-branch", etc.
- **GitHub tool summaries in TUI** — Added summaries for all 10 GitHub tools (`github_list_issues`, `github_get_issue`, `github_create_issue`, `github_comment_issue`, `github_close_issue`, `github_list_prs`, `github_get_pr`, `github_create_pr`, `github_merge_pr`, `github_review_pr`), displaying actions like "📋 issue #42 created", "📋 PR #7 merged", etc.
- **GitLab tool summaries in TUI** — Added summaries for all 14 GitLab tools (`gitlab_list_projects`, `gitlab_get_project`, `gitlab_list_issues`, `gitlab_get_issue`, `gitlab_create_issue`, `gitlab_close_issue`, `gitlab_list_prs`, `gitlab_get_pr`, `gitlab_create_pr`, `gitlab_get_pipeline`, `gitlab_list_jobs`, `gitlab_get_job`, `gitlab_retry_job`, `gitlab_cancel_job`), displaying actions like "🦊 project retrieved", "🦊 issue #5 created", "🦊 pipeline #3", etc.
- **Tool output summaries** — `tool_output_summary` function extended to cover git, GitHub, and GitLab tools with descriptive output strings.

## Version: 0.1.0-alpha.89

### Changed
- **README.md rebuilt** — Rewrote from scratch to reflect the current specification. Expanded feature list to ~111 tools across 15 categories, corrected provider list (10 providers), added missing systems (memory, spec management, skills, teams/swarm, autopilot, MCP client, config error reporting), updated architecture table with all 15 crates, and refreshed project status.
- **STATS.md updated** — Complete rewrite showing project-wide metrics (175,840 lines, 468 files, 1,670 tests) and a per-crate breakdown with file counts, line counts, test files, descriptions, ASCII bar chart, and architecture ratios.
- **SPEC.md cover page** — Added styled HTML cover page with title, author, version, date, and repository link.

## Version: 0.1.0-alpha.88

### Fixed
- **Context compaction bug** — Fixed `compact_history_with_atomic_tool_calls` which was breaking the compaction loop prematurely when the oldest message was part of a tool call pair, preventing any trimming. Now uses prefix sums and a proper scan to find the correct truncation point while preserving atomic tool call pairs.

### Changed
- **SPEC.md audit and corrections** — Comprehensive review of SPEC.md against actual codebase implementation. Corrected tool counts (File Ops 14, Execution 3, Memory 8, Team 20), removed non-existent tools (`execute_python`, `file_ops_tool`, dead aliases), fixed version string to `alpha.88`, added alpha.87/alpha.88 to Appendix A, replaced MCP stub (§19) and Auto-Update stub (§20) with real documentation, expanded §5.2 config schema to include `tool_visibility`, `dirs`, `bash`, `gitlab`, `internal_llm`, `stream`, added missing slash commands (`/config show`, `/dirs`, `/profile`, `/theme`, `/status`, `/mouse`) to §6.2, and expanded §5.3 environment variable table with all provider-specific keys.

## Version: 0.1.0-alpha.87

### Fixed
- **Read tool instructions** — Clarified in AGENTS.md that `end_line` is an absolute line number, not a count or offset.
- **Remote push instructions** — Strengthened AGENTS.md guidelines to explicitly prohibit pushing without explicit user instruction.

### Changed
- **SPEC.md reorganization** — Reorganized sections into logical order (1-20), fixed numbering, added Blueprints subsection (14.7), and merged GitHub/GitLab into a single peer section (18).

## Version: 0.1.0-alpha.86

### Added
- **Azure Resource (File) provider** — New `azure_resource` provider that reads Azure endpoint definitions from `azureresources.json` in `~/.config/ragent/` or `.ragent/`. Supports multiple resource entries with per-endpoint API keys, environment-variable-based keys, custom context windows, capability tags, and thinking configuration.
- **Azure Resource documentation** — Added `docs/userdocs/azure-resource.md` with full JSON schema, field reference, copy-pasteable example, and troubleshooting guide.
- **Azure Resource integration tests** — Added `crates/ragent-tui/tests/test_azure_resource_flow.rs` covering provider listing, persistence round-trip, stale selection cleanup, ModelInfo conversion, and backend resolution.

## Version: 0.1.0-alpha.85

### Changed
- **Version bump** — Incremented pre-release version for release.

## Version: 0.1.0-alpha.84

### Added
- **Azure test script** — Added `scripts/getresult.sh` for testing Azure AI Foundry chat completions.

## Version: 0.1.0-alpha.83

### Fixed
- **SPEC.md fixes** — Fixed malformed benchmark runner table, corrected Team Lifecycle mermaid diagram syntax, replaced misplaced GitLab Integration section with proper content, and updated version references throughout.

## Version: 0.1.0-alpha.82

### Added
- **azProvider fixes** — Applied fixes to Azure provider implementation.
- **`/config show`** — Added `/config show` slash command to display current configuration.

## Version: 0.1.0-alpha.81

### Fixed
- **Azure endpoint logging** — TUI log panel now displays the full endpoint URL for Azure AI Foundry requests.

## Version: 0.1.0-alpha.78

### Fixed
- **Azure endpoint logging** — TUI log panel now displays the full endpoint URL for Azure AI Foundry requests, not just the `[provider/model]` prefix.

## Version: 0.1.0-alpha.77

### Added
- **Azure endpoint logging** — Azure AI Foundry provider now logs the resolved endpoint via `tracing::info!` when connecting.

## Version: 0.1.0-alpha.76

### Added
- **Azure AI Foundry provider** — New `azure_foundry` provider for Microsoft Azure AI Foundry models. Supports OpenAI-compatible endpoints with `api-key` header authentication, dynamic model discovery, streaming chat completions, tool calling, vision, and reasoning levels (o1, o3-mini). Configurable via `AZURE_AI_FOUNDRY_API_KEY` and `AZURE_AI_FOUNDRY_BASE` environment variables or `ragent.json`.

## Version: 0.1.0-alpha.75

### Fixed
- **SPEC.md mermaid diagrams** — Fixed 2 diagrams (Figure 1 and Figure 7) where closing fences/`end` keywords were on the same line as node definitions, which broke rendering. All 14 diagrams now pass syntax validation and render correctly.

## Version: 0.1.0-alpha.73

### Fixed
- **/model selection** — Fixed `/model` selection handling.

## Version: 0.1.0-alpha.72

### Added
- **gen-spec-pdf.sh script** — New `scripts/gen-spec-pdf.sh` for converting Markdown specifications (with Mermaid diagrams) to PDF using Pandoc and Chromium.

### Changed
- **SPEC.md updates** — Removed LSP references and added a dedicated Spec Management section documenting the spec tool suite.

## Version: 0.1.0-alpha.71

### Added
- **Startup ASCII art banner** — The TUI now displays an ASCII art rendering of the application name on startup, followed by the version number and the exact date/time the binary was compiled.

## Version: 0.1.0-alpha.70

### Changed
- **Update concurrency** — Improved concurrency handling across the codebase.
- **Fix todos** — Resolved outstanding todo issues.

## Version: 0.1.0-alpha.68

### Added
- **`/codeindex` language filtering** — The `/codeindex` slash command now supports an optional `lang` parameter to filter code index results by programming language (e.g., `/codeindex lang rust`).

### Changed
- **Benchmark data cleanup** — Removed unused benchmark dataset files from `benches/data/` across multiple languages and suites, significantly reducing repository size.

## Version: 0.1.0-alpha.67

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.67.

## Version: 0.1.0-alpha.66

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.66.

## Version: 0.1.0-alpha.65

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.65.

## Version: 0.1.0-alpha.64

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.64.

## Version: 0.1.0-alpha.63

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.63.

## Version: 0.1.0-alpha.62

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.62.

## Version: 0.1.0-alpha.61

### Added
- **Instruction file discovery logging** — New `InstructionFileDiscovery` struct and `collect_agents_md_content_with_discovery()` function track which AGENTS.md-style files were found and where. Logs discovery summary via tracing and emits `AgentNotice` events for visibility.

### Changed
- **AgentNotice display** — TUI now displays `AgentNotice` events in the message window ("📋 **Agent Notice**" prefix) in addition to the status bar, making instruction file discovery visible to users.
- **Improved formatting** — AGENTS.md acknowledgment messages now include a newline separator for better readability.

## Version: 0.1.0-alpha.60

### Added
- **Global AGENTS.md search path** — Extended `collect_agents_md_content()` to search `~/.local/share/ragent/` for instruction files. Falls back to global files only when no local project files exist.

### Changed
- **AGENTS.md precedence** — Local project instruction files now completely replace global files, rather than being appended to them. This enables cleaner project-specific overrides of global guidelines.

## Version: 0.1.0-alpha.59

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.59.

## Version: 0.1.0-alpha.58

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.58.

## Version: 0.1.0-alpha.57

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.57.

## Version: 0.1.0-alpha.56

### Added
- **Multilingual benchmark suites** — Added benchmark test files for Go, Java, JavaScript/TypeScript, Python, and Ruby to expand language coverage for the benchmark system.

## Version: 0.1.0-alpha.55

### Fixed
- **Permission milestone test fixes** — Fixed failing unit tests in `test_permission_system.rs` related to context window limits, message ordering, and compact history behaviour. Removed brittle assertions and added robust assertions for actual system state.

## Version: 0.1.0-alpha.54

### Fixed
- **Permission dialog timeout** — Fixed permission dialog timeout from 30 seconds to 120 seconds in `processor.rs`. Added `created_at` and `timeout_secs` fields to `PermissionRequest` struct in `permission/mod.rs`.

### Changed
- **Permission dialog countdown timer** — Implemented live countdown timer in permission dialog title in `crates/ragent-tui/src/input.rs`.

## Version: 0.1.0-alpha.53

### Fixed
- **Permission dialog live update** — Fixed permission dialog countdown not visually decrementing by changing main event loop to always redraw.

## Version: 0.1.0-alpha.52

### Changed
- **Bash safe command display** — Changed `SAFE_COMMANDS` from private to `pub const` in `bash.rs` and updated `/bash show` TUI command to display the built-in safe command list.

## Version: 0.1.0-alpha.51

### Changed
- **Bash permission command name extraction** — Added `extract_command_name()` helper in `processor.rs` to extract just the first word from a bash command before permission checking. Modified bash permission check loop to use command names.

## Version: 0.1.0-alpha.50

### Changed
- **Bash denylist word-boundary matching** — Split `DENIED_PATTERNS` into `DENIED_COMMANDS` (word-boundary matched via command name extraction) and `DENIED_PATTERNS` (substring matched). Added `extract_command_names()` helper in `bash.rs`.

## Version: 0.1.0-alpha.49

### Added
- **Permission dialog countdown** — Added countdown timer to permission approval dialog in TUI.
- **Config parse error enhancement** — Improved config file parser to show clear, actionable errors.
- **Codeindex hardwired permissions** — Made codeindex tools always allowed without permission checks.

## Version: 0.1.0-alpha.48

### Fixed
- **Permission milestones** — Fixed various issues in permission system and bash security layers.

## Version: 0.1.0-alpha.47

### Changed
- **Crate reorganisation** — Extracted `ragent-types`, `ragent-config`, `ragent-storage`, and `ragent-llm` from `ragent-core`.

## Version: 0.1.0-alpha.46

### Added
- **Permission system** — Implemented core permission system with 20 passing tests.

## Version: 0.1.0-alpha.45

### Added
- **Bash security** — Implemented 7-layer bash security system.

## Version: 0.1.0-alpha.44

### Added
- **Permission dialog** — Added permission approval dialog with timeout.

## Version: 0.1.0-alpha.43

### Added
- **Permission checker** — Implemented permission checker with allow/deny/ask rules.

## Version: 0.1.0-alpha.42

### Added
- **Permission rules** — Added permission rule evaluation with last-match-wins semantics.

## Version: 0.1.0-alpha.41

### Added
- **Permission request flow** — Implemented permission request flow with EventBus integration.

## Version: 0.1.0-alpha.40

### Added
- **Permission system foundation** — Added Permission enum, PermissionAction, PermissionRule, and PermissionChecker.

## Version: 0.1.0-alpha.39

### Added
- **Code index tools** — Added `codeindex_search`, `codeindex_symbols`, `codeindex_references`, `codeindex_dependencies`, `codeindex_status`, and `codeindex_reindex` tools.

## Version: 0.1.0-alpha.38

### Added
- **Code index** — Implemented codebase indexing with tree-sitter parsing and Tantivy FTS.

## Version: 0.1.0-alpha.37

### Added
- **Memory system** — Implemented three-tier memory system with file blocks, structured SQLite store, and semantic search.

## Version: 0.1.0-alpha.36

### Added
- **Teams** — Implemented multi-agent coordination with named teammates and shared task lists.

## Version: 0.1.0-alpha.35

### Added
- **Swarm mode** — Implemented swarm decomposition for parallel task execution.

## Version: 0.1.0-alpha.34

### Added
- **Autopilot mode** — Implemented autonomous operation mode.

## Version: 0.1.0-alpha.33

### Added
- **Custom agents** — Implemented OASF-based custom agent profiles.

## Version: 0.1.0-alpha.32

### Added
- **Skills system** — Implemented loadable skill packs.

## Version: 0.1.0-alpha.31

### Added
- **Prompt optimization** — Implemented `/opt` slash command with 12 methods.

## Version: 0.1.0-alpha.30

### Added
- **MCP client** — Implemented Model Context Protocol client support.

## Version: 0.1.0-alpha.29

### Added
- **Background agents** — Implemented sub-agent spawning and management.

## Version: 0.1.0-alpha.28

### Added
- **Event bus** — Implemented internal tokio pub/sub for real-time UI updates.

## Version: 0.1.0-alpha.27

### Added
- **Snapshot & undo** — Implemented file snapshots before edits.

## Version: 0.1.0-alpha.26

### Added
- **Project guidelines** — Implemented auto-loading of `AGENTS.md` from project root.

## Version: 0.1.0-alpha.25

### Added
- **Agent presets** — Implemented coder, task, architect, ask, debug, code-review agents.

## Version: 0.1.0-alpha.24

### Added
- **Permission system** — Implemented configurable permission rules.

## Version: 0.1.0-alpha.23

### Added
- **Session management** — Implemented persistent conversation history in SQLite.

## Version: 0.1.0-alpha.22

### Added
- **HTTP server** — Implemented axum-based REST + SSE API.

## Version: 0.1.0-alpha.21

### Added
- **Terminal UI** — Implemented full-screen ratatui interface.

## Version: 0.1.0-alpha.20

### Added
- **Tool system** — Implemented core tool registry and dispatch.

## Version: 0.1.0-alpha.19

### Added
- **GitHub integration** — Implemented GitHub tools for issues and PRs.

## Version: 0.1.0-alpha.18

### Added
- **GitLab integration** — Implemented GitLab tools for issues, MRs, and pipelines.

## Version: 0.1.0-alpha.17

### Added
- **Office tools** — Implemented office_read, office_write, office_info, libre_read, libre_write, libre_info.

## Version: 0.1.0-alpha.16

### Added
- **PDF tools** — Implemented pdf_read and pdf_write.

## Version: 0.1.0-alpha.15

### Added
- **Web tools** — Implemented webfetch, websearch, and http_request.

## Version: 0.1.0-alpha.14

### Added
- **Bash tool** — Implemented bash execution with security restrictions.

## Version: 0.1.0-alpha.13

### Added
- **File tools** — Implemented read, write, create, edit, multiedit, patch, copy_file, move_file, rm, mkdir, append_file, file_info, diff_files, glob, and list.

## Version: 0.1.0-alpha.12

### Added
- **Provider system** — Implemented Anthropic, OpenAI, and Ollama providers.

## Version: 0.1.0-alpha.11

### Added
- **Configuration** — Implemented ragent.json configuration loading.

## Version: 0.1.0-alpha.10

### Added
- **Storage** — Implemented SQLite-backed storage.

## Version: 0.1.0-alpha.9

### Added
- **Event system** — Implemented EventBus with tokio broadcast channels.

## Version: 0.1.0-alpha.8

### Added
- **Message system** — Implemented chat message types and serialization.

## Version: 0.1.0-alpha.7

### Added
- **LLM client** — Implemented HTTP client for LLM providers.

## Version: 0.1.0-alpha.6

### Added
- **Types** — Implemented shared types and IDs.

## Version: 0.1.0-alpha.5

### Added
- **CLI** — Implemented clap-based CLI with run, serve, session, auth, models, and config commands.

## Version: 0.1.0-alpha.4

### Added
- **TUI** — Implemented ratatui terminal interface.

## Version: 0.1.0-alpha.3

### Added
- **Server** — Implemented axum HTTP server with REST + SSE endpoints.

## Version: 0.1.0-alpha.2

### Added
- **Tools** — Implemented core tool system.

## Version: 0.1.0-alpha.1

### Added
- **Initial project scaffolding** — Created Cargo workspace with core crates.

## Version: 0.1.0-alpha.0

### Added
- **Initial commit** — Project created.
