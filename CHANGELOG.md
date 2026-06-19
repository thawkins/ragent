# Changelog

## Version: 0.1.0-alpha.113

### Fixed
- **Research-system spec status overstated (second occurrence)** — `specs/researchsystem/SPEC.md` was again tagged `status: implemented` with a three-step audit trail even though only six of the 56 plan tasks had shipped (T-001/T-002/T-003 foundational types, T-005 crate scaffold, T-004 `ResearchItem`, T-014 web-gatherer, T-016 local-gatherer, T-040 plan-dep parser, T-045 path-traversal rejection, T-051 name-validation tests). The frontmatter is now `status: in_progress` with a two-step audit (`none → draft → in_progress`) that correctly reflects "framework implemented, integration pending". The remaining ~50 tasks (manager CRUD, session orchestrator, supporting-file writers, `RESEARCH.md` assembler, References Index generator, `INDEX.md` cache, TUI slash-command wiring, CLI/HTTP endpoints, spec-integration glue, benchmarks, user docs) remain `pending` and will be promoted in subsequent releases.

## Version: 0.1.0-alpha.112

### Fixed
- **Research-system spec status overstated** — `specs/researchsystem/SPEC.md` was committed with `status: implemented` and a three-step audit trail even though only four of the 22 plan tasks were actually completed (T-001 `ResearchName`, T-002 `ResearchStatus`, T-003 `Source`, T-005 crate scaffold). The frontmatter is now `status: draft` with a single `none → draft` audit transition, and the remaining 18 tasks remain `pending` in `specs/researchsystem/PLAN.md` until the gathering engine, TUI slash command, CLI/HTTP endpoints, and spec integration land in follow-up releases.

## Version: 0.1.0-alpha.111

### Changed
- **`ask_user` tool promoted from alias to standalone** — The previously-delegating `ask_user` tool in `crates/ragent-agent/src/tool/aliases.rs` now publishes `Event::QuestionRequested` and awaits `Event::QuestionAnswered` directly via the event bus. The standalone `question` tool has been deleted from `ragent-agent`, `ragent-tools-core`, and the TUI question-dialog widget module; question rendering is now driven entirely by the existing TUI event handler in `ragent-tui/src/app.rs`.
- **`ask_user` supports multiple-choice** — The optional `options` array parameter renders a selectable list in the TUI question dialog; omitting `options` keeps the previous free-text input behaviour. The tool description and JSON schema now document the new parameter, and `permission_category` is reported as `ask_user` (was `question`).
- **Permission auto-approval key renamed** — `check_permission_with_prompt`'s hardwired always-allow list now matches `ask_user` (was `question`); the corresponding unit test was renamed accordingly.

### Added
- **`ragent-research` crate scaffold** — New workspace member under `crates/ragent-research/` providing `ResearchName` (validated, URL-safe identifier newtype with FR-002 validation), `Source` (Web/Local/Spec/Other enum backing the references index), and `ResearchStatus` (draft/in-progress/complete/archived covering FR-013). Depends only on `ragent-types` and common workspace deps so it can be reused by both the TUI and HTTP layers once the manager/session/io modules are added in follow-up releases.
- **Research system spec + plan** — New `specs/researchsystem/SPEC.md` and `specs/researchsystem/PLAN.md` describing the `/research` slash command, directory conventions, information-gathering session, references index, and integration with the existing spec workflow.

## Version: 0.1.0-alpha.110

### Removed
- **Internal LLM subsystem removed** — The embedded local LLM (Candle GGUF + Foundry Local + LiteRT-LM), the `/internal-llm` slash command family, the TUI `InternalLLM` chat overlay panel, the `InternalLlmConfig` block, the `internal_llm` Cargo feature flag, and all related test files have been removed. Compaction now always uses the provider-compaction fallback. Session titles default to empty (no longer auto-generated). Memory extraction no longer has an LLM prefilter step. The `internal_llm` key in `ragent.json` is silently ignored. This supersedes the prior Foundry Local internal-LLM backend and the LiteRT-LM backend switch work.

## Version: 0.1.0-alpha.109

### Added
- **In-process Microsoft Foundry Local backend** — New `FoundryLocalInProcClient` in `crates/ragent-llm/src/providers/foundry_local_inproc_client.rs` loads and runs Foundry Local models inside the ragent process via the `foundry-local-sdk` native core, bypassing the local web service.  Supports model alias resolution, download progress events, device selection (`auto`/`cpu`/`gpu`/`npu`), temperature/max_tokens, tools, and full `StreamEvent` translation (text, tool calls, usage, finish reason).
- **`in_process` provider option** — `provider.foundry_local.in_process` (default `false`) selects the in-process backend; when unset or `false` the existing web-service path is preserved.
- **`RAGENT_FOUNDRY_LOCAL_FORCE_WEB` escape hatch** — Set this environment variable to `1` or `true` to force the web-service path even when `in_process: true` is configured.
- **TUI foundry-mode indicator** — `/internal-llm show` now displays whether the main Foundry Local provider is configured for `in-process` or `web-service` inference when the internal LLM backend is `foundry`.

### Changed
- **Foundry Local provider routing** — `FoundryLocalProvider::create_client()` now branches on the resolved `in_process` flag, returning either `FoundryLocalInProcClient` or the existing `FoundryLocalClient`.
- **Device validation** — `provider.foundry_local.device` values are now validated and rejected if not one of `auto`, `cpu`, `gpu`, or `npu`.
- **Foundry Local documentation** — Updated `PROVIDERS.md` and `SPEC.md` with in-process mode configuration, environment escape hatch, and internal-LLM notes.

### Fixed
- **HuggingFace provider discovery failed** — The HuggingFace `/v1/models` router endpoint is public and now works without an API token; discovery no longer errors out immediately when `HF_TOKEN` is unset.  Added `HUGGING_FACE_HUB_TOKEN` as a recognised token source for consistency with the TUI configured-provider detection.  When dynamic discovery fails or returns no models, the TUI now falls back to the provider's static default catalog instead of showing an empty "No models are currently available" dialog.  Empty discovery results are no longer cached, preventing a transient failure from permanently hiding the default models.
- **Task tool family guidance** — Added a dedicated `## Task Tool Family` section to every primary agent's system prompt that clearly distinguishes `task_complete` (autonomous loop signal — only takes `summary`) from `team_task_complete` (team workflow — only takes `team_name` + `task_id`).  The `task_complete`, `team_task_complete`, and `new_task` tool descriptions and JSON schemas now explicitly warn against the most common parameter-confusion mistakes and reject unknown keys via `additionalProperties: false`.  `task_complete` and `list_tasks` are now hardwired auto-approved so the agent can always finish or inspect background tasks without a permission prompt.

## Version: 0.1.0-alpha.108

### Added
- **Foundry Local internal-LLM backend** — New `FoundryLocalExecutor` in `ragent-agent/src/internal_llm/foundry_executor.rs` routes internal-LLM requests through Microsoft Foundry Local instead of the Candle-based embedded runtime. The `/internal-llm foundry` and `/internal-llm embedded` slash commands switch between backends at runtime, and `from_config()` now dispatches on `config.backend` (`"foundry"`/`"foundry_local"` vs default candle).

### Changed
- **Internal LLM backend routing** — `InternalLlmService::from_config()` now selects the executor based on the configured backend name, supporting both Candle (`embedded`) and Foundry Local (`foundry`/`foundry_local`) paths.
- **TUI /internal-llm commands** — Added `foundry` and `embedded` subcommands to the `/internal-llm` slash command for switching backends. Updated autocomplete list, help text, and slash-command definition.
- **Compiled backends display** — Replaced litertlm feature-flag detection with Foundry Local availability check (`is_foundry_local_available()`) in the TUI show/info panel.
- **Workspace version** — Bumped to `0.1.0-alpha.108`.

### Fixed
- **Microsoft Foundry Local empty SSE stream after model preparation** — `wait_for_model_ready` previously polled `/v1/models`, which lists downloaded/cataloged models and may report a model before it is actually loaded into memory. Chatting with an unloaded model caused the empty/malformed event-stream error seen in the TUI. The readiness check now polls the web service's `/models/loaded` endpoint (the authoritative "loaded into memory" signal) and falls back to `/v1/models` only on older services. The model load id is now taken directly from the SDK's full variant id, making load requests more robust across catalog versions.

## Version: 0.1.0-alpha.107

### Fixed
- **Compression pipeline threshold gating** — Added `should_compress` and `should_compress_chat_messages` checks before invoking the full compression pipeline, preventing unnecessary overhead and unconditional UI events when the conversation is well within the context window. The initial-history compression and per-iteration compression now both gate on the configured `auto_threshold` (default 0.80) before running. Added 2 new unit tests for the chat-messages threshold helper.

### Changed
- **Workspace version** — Bumped to `0.1.0-alpha.107`.

## Version: 0.1.0-alpha.106

### Added
- **Microsoft Foundry Local provider integration** — Added first-class support for Microsoft Foundry Local as a local LLM provider, including provider setup dialog visibility, `[local]` badge rendering, status-bar abbreviation, health checks, and configuration option merging (`auto_start`, `device`, `models_path`).
- **Headroom compression lifecycle events** — New `Event::CompressionStarted` and `Event::CompressionFinished` events are published by the session processor and consumed by the TUI and SSE stream. They carry `original_tokens`, `compressed_tokens`, `compression_ratio`, and `did_compress` so observers can show live progress and update context-window displays immediately.

### Changed
- **Workspace version** — Bumped to `0.1.0-alpha.106`.
- **Per-iteration compression visibility** — The agent loop now publishes `CompressionStarted`/`CompressionFinished` around every automatic Headroom compression run. The TUI sets `compress_in_progress` while the pipeline is active, so the existing status-bar "compressing" indicator actually appears during automatic compression, and the `ctx:` display is refreshed with the post-compression token count as soon as the run completes.
- **SSE event coverage** — `ragent-server` now serializes `compression_started` and `compression_finished` SSE events.

### Fixed
- **Context window display lag after compression** — `last_input_tokens` was only updated when the provider returned token usage, so the status-bar context percentage stayed at the pre-compression value until the LLM response arrived. The TUI now updates `last_input_tokens` directly from `CompressionFinished`, keeping the `ctx:` display in sync with the actual request size.

## Version: 0.1.0-alpha.105

### Added
- **Context compression pipeline** — New compression module (`ragent-agent/src/compression/`) with multi-strategy history compaction: BM25 relevance scoring, CCR (Critical Content Retention) store, aggressive/conservative/default modes, and `/compress` slash command integration.
- **Model Router Provider** — New intelligent model routing system (`ragent-llm/src/providers/router*.rs`) with 15-dimension classifier (complexity, creativity, code, reasoning, vision, image_attachment, etc.), automatic model selection, and configurable routing rules.
- **Compression config** — New `compression.rs` module in ragent-config for compression pipeline configuration.
- **String utilities** — New `strutil.rs` module in ragent-types for shared string helpers.
- **Spec ID scanner** — New `id_scanner.rs` in ragent-specs for extracting and tracking spec requirement/task IDs.
- **HeadroomCompress spec** — Full specification for the compression feature in `specs/HeadroomCompress/`.
- **ModelRouterProvider spec** — Full specification for the model router in `specs/ModelRouterProvider/`.
- **Config defaults fix** — Added `#[serde(skip_serializing_if)]` to `code_index.enabled`, `internal_llm.enabled`, and `tool_visibility.codeindex` so auto-generated config files don't override code-level defaults. Added 10 regression tests.
- **Compression indicator test** — New TUI test for compression status display.

### Changed
- **Agent system** — Refactored agent module with expanded presets and compression integration.
- **Session processor** — Added compression integration, improved tool call handling, and spec command support.
- **TUI** — Added `/compress` slash command, compression status bar indicator, and improved status bar layout.
- **Spec commands** — Major refactor of spec command handling with expanded `/spec impl` and `/spec implement` support.
- **Bedrock provider** — Refinements to credential handling and SigV4 signing.
- **Multiple tool refinements** — Updated codeindex_search, list_tasks, memory_search, office_write, and spec_list tools.
- **Test improvements** — Updated multiple test files for compatibility with new APIs and module structure.
- **TUI toggle persistence** — `/codeindex`, `/internal-llm`, and `/tools` toggles now save to a project-local `.ragent/ragent.json` (creating the directory if needed) instead of falling back to the global config. They also skip writes when the target value has not changed, avoiding unnecessary file churn.
- **YOLO mode persistence** — YOLO mode state is now saved to the config file (`yolo: true/false`) and restored on startup. Toggling via `/yolo` or the `InputAction::ToggleYolo` keybinding persists the new state immediately.

### Fixed
- **Microsoft Foundry Local visibility** — The provider was already registered in the LLM layer and listed by `ragent models`, but was missing from the TUI provider setup dialog and the user-facing provider list. Added `foundry_local` to `PROVIDER_LIST`, the provider setup/reset flows, configured-provider detection, health checks, and status-bar abbreviation, and rendered a `[local]` badge alongside Ollama. Also merged `provider.foundry_local` options (`auto_start`, `device`, `models_path`) from `ragent.json` into the client creation path. Updated README.md and SPEC.md to include Microsoft Foundry Local.
- **TUI provider picker scrolling** — The provider setup dialog used a fixed 50% height and rendered all providers in a single paragraph, so on typical 24-row terminals the bottom of the list (including Microsoft Foundry Local) was clipped off-screen and unreachable. The dialog is now taller (capped at 22 rows) and the provider list scrolls to keep the selected item visible, with a "(more providers below)" hint when the list overflows.
- **Per-iteration context compression** — The Headroom compression pipeline was only run once at the start of an agent run, so the LLM request payload kept growing as the agent loop appended assistant tool uses and tool results each turn. Once the payload exceeded the model's context window, the provider returned an error and the task failed. The agent loop now re-runs compression before every LLM call when the configured `auto_threshold` (default 0.80) is exceeded, satisfying FR-005. Added `compress_chat_messages` round-trip helpers and 6 unit tests in `ragent-agent/src/compression/pipeline.rs`.
- **Config defaults for CodeIndex and InternalLLM** — When `Config::load()` created a default config file (no existing config), it serialised all fields including default values like `"enabled": true` for `code_index` and `"enabled": false` for `internal_llm`. These explicit values then overrode any future code-level default changes. Added `#[serde(skip_serializing_if)]` annotations to `code_index.enabled`, `internal_llm.enabled`, and `tool_visibility.codeindex` so default values are omitted from serialised output, allowing code-level defaults to take effect automatically. Added 10 regression tests in `test_code_index_config.rs`.
- **Foundry Local always compiled** — Removed the empty `foundry-local` feature flag from the root `Cargo.toml` defaults. The `foundry-local-sdk` dependency in `ragent-llm` is already unconditional, so the Microsoft Foundry Local provider is now always present with no compile-time gate.

## Version: 0.1.0-alpha.104

### Added
- **Amazon Bedrock provider** — Full AWS Bedrock support with SigV4 request signing (no AWS SDK dependency), dual API clients (Anthropic Messages API for Claude models, Converse API for all other models), 9 default models, short alias mapping, `@bedrock` suffix stripping, `ListFoundationModels` discovery, and credential resolution chain (env vars → AWS profile INI → session tokens).
- **xAI Grok provider** — New `xai.rs` provider for the xAI Grok API, registered in the default provider registry.
- **Spec implementation commands** — `/spec impl` and `/spec implement` slash commands for spec lifecycle: generates implementation plans, tracks progress against requirements, and runs implementation tasks.

### Changed
- **Copilot provider improvements** — Updated copilot.rs with refinements to the GitHub Copilot provider.
- **HuggingFace provider improvements** — Updated huggingface.rs with refinements.
- **Provider registry** — Added Bedrock and xAI to `create_default_registry()`.
- **Spec module expanded** — New `impl_runner.rs` and `plan_parser.rs` modules in ragent-specs.

## Version: 0.1.0-alpha.103

### Fixed
- **Bash syntax validation on Windows** — `validate_bash_syntax()` previously used a hardcoded `sh -n -c` command, which fails on Windows because `sh` is not available. Now uses the discovered shell program: `bash -n -c` on Unix, the Git Bash executable path on Windows (Git Bash), and skips validation entirely for PowerShell. This eliminates the "program not found" error when running bash commands on Windows 11.

## Version: 0.1.0-alpha.102

### Added
- **Windows shell support for BashTool** — The `BashTool` now runs on Windows with automatic shell discovery (Git Bash preferred, PowerShell fallback). All 7 security layers remain active regardless of platform. Windows-specific directory-escape detection blocks `C:\`, `D:\`, and `\` paths. PowerShell syntax validation is skipped (PowerShell self-validates at runtime). State files are stored in `%LOCALAPPDATA%\ragent\shell\` on Windows.

### Changed
- **Refactored `is_directory_escape_attempt`** — Split into `is_directory_escape_attempt` (public, calls inner) and `is_directory_escape_attempt_inner` (testable inner function with explicit `on_windows` parameter). This enables testing Windows-specific path detection on any platform.
- **Shell discovery caching** — Added `OnceLock`-based process-global shell type cache so shell discovery only runs once per process.

### Fixed
- **Directory escape test** — `test_directory_escape_absolute` now uses `tempfile::tempdir()` for a real filesystem path, avoiding `canonicalize()` hangs on nonexistent paths.

## Version: 0.1.0-alpha.101

### Fixed
- fixed AGENTS.md load path

## Version: 0.1.0-alpha.100

### Added
- **Sub-agent suspend/resume/kill lifecycle** — New `suspend_task()`, `resume_task()`, and `kill_task()` methods on the task manager. Sub-agents can now be paused, resumed, or forcibly terminated with a 10-second force-kill escalation timeout. New `TaskStatus::Suspended` and `TaskStatus::Terminating` states, plus `SubagentSuspended`, `SubagentResumed`, and `SubagentKilled` events.
- **Teammate suspend/resume events** — `TeammateSuspended` and `TeammateResumed` events for team coordination, enabling lead agents to pause and resume teammates.
- **Enhanced active-agents panel** — TUI active agents panel now shows per-agent status (running/suspended), supports suspend/resume/kill actions, and renders agent step counts and elapsed time.
- **Enhanced teams panel** — TUI teams panel shows teammate statuses, suspend/resume buttons, and per-teammate progress indicators.
- **SSE events for sub-agent lifecycle** — Server-sent events now stream `SubagentSuspended`, `SubagentResumed`, `SubagentKilled`, `TeammateSuspended`, and `TeammateResumed` event types.

### Changed
- **Permission check indentation fix** — Re-indented `check_permission_with_prompt()` to correct a long-standing indentation issue.
- **AGENTS.md discovery sorting** — Improved instruction file priority sorting with properly formatted ordering logic.
- **Azure Resource provider refactoring** — Cleaned up `azure_resource.rs` provider implementation.
- **HTTP client retry logic** — Updated `execute_with_retry` in `http_client.rs`.

### Fixed
- **Instruction file discovery priority bug** — `collect_agents_md_content_with_discovery()` in `agent/mod.rs` was incorrectly calculating file depth (missing `saturating_sub(1)`), causing `AGENTS.md` in the project root to have depth=1 instead of depth=0. This meant root instruction files were treated as subdirectories, and the sorting step mixed root, global, and subdirectory candidates together. Now root files are correctly identified (depth=0), each priority tier is sorted independently, and concatenated in strict order: project root → global directory → project subdirectories. Added integration test `test_root_agents_md_beats_subdirectory_agents_md` to verify the fix.

## Version: 0.1.0-alpha.99

### Changed
- **updated splash screen text**

## Version: 0.1.0-alpha.98

### Added
- **Azure Resource Provider API type switch** — Added `api_type` field to `azureresources.json` entries. When set to `"anthropic"`, requests are routed to `{endpoint}/anthropic/v1/messages` using the Anthropic Messages API format with Azure-style `api-key` authentication. When set to `"openai"` or omitted, the existing OpenAI-compatible path (`{endpoint}/openai/v1/chat/completions`) is used.  
  - New `AzureAnthropicClient` wrapper in `azure_resource.rs` reuses `AnthropicClient` body construction and SSE parsing but sends `api-key` header instead of `x-api-key`.
  - `AzureResourceProvider::create_client` now branches on `api_type` by looking up the model ID in the cached entries.
  - TUI `SelectAzureResource` flow now persists `azure_resource` as the provider (instead of `azure_foundry`) and stores `azure_resource_api_base` alongside `azure_resource_last_selection`.
  - Session processor `resolve_api_key` and base-URL resolution now handle `azure_resource` provider directly.
  - Added unit tests for parser validation (`test_api_type_openai_accepted`, `test_api_type_anthropic_accepted`, `test_api_type_missing_defaults_to_openai`, `test_api_type_invalid_skipped_with_warning`).
  - Added integration tests for `create_client` branching (`test_azure_anthropic_create_client_branches_correctly`, `test_azure_openai_branch_unchanged`).
  - Updated `specs/AzureResource/FILEFORMAT.md` with `api_type` documentation.

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
