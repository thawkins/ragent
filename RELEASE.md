# Release

## Current Version: 0.1.0-alpha.121

### Fixed — `/research` now actually analyses the gathered sources
- **Supporting files contain the captured body, not a placeholder** — `Source::Web`, `Source::Local`, and `Source::Other` gained an inline `body: String` field. The `WebGatherer` passes the fetched page text into `Source::Web.body`; the `LocalGatherer` writes a context-aware excerpt (matching lines plus one line on either side) into `Source::Local.body`. `render_supporting_file` and the synthesis engine consume the inline body, so `research/<name>/sources/web-NN.md` and `local-NN.md` now contain the actual evidence (with `▶` markers for exact matches and a ` ` marker for context).
- **Local source relevance note is now informative** — Replaced the previous "X keyword match(es) for research topic" string with a note that names the matched keywords (truncated to 3, e.g. `…(+N)` for the tail) and a 120-char snippet of the first matching line.
- **Mechanical fallback summary/findings are useful, not skeletal** — When no LLM synthesis is available, the default `Summary` names the captured web titles and local file paths grouped by type, the default `Findings` is one bullet per source with a 240-char excerpt, and the default `Open Questions` suggests concrete gaps and re-running with a configured LLM.
- **Synthesis errors are visible, not silently swallowed** — `ResearchSession::run` emits `SessionEvent::SynthesizeResult { outcome, detail }` with outcome `Llm | FallbackEmpty | FallbackError | NoLlm`. Failures are logged at `error` level (not `warn`) and the message bubbles through to the TUI progress tracker and the CLI JSON emitter. Also fixed `analysis_is_noop` always returning `false` because `Any::type_id` on a trait object returns the trait object's `TypeId` — replaced with `is_noop_marker()` overridden by `NoopAnalysisEngine` to `true`.
- **CLI now wires up the local gatherer** — `ragent research create` now uses the new `ragent_research::cli::FsLocalTool` so the CLI produces useful output without API keys. Web search and LLM synthesis still require credentials and remain off in the CLI.

### Added
- **`ragent_research::cli::FsLocalTool`** — Public filesystem-backed implementation of `LocalTool` for CLI use. Walks the project root, greps line-by-line, reads files, and lists `specs/<id>` directories. Skips `research/`, `target/`, `.git/`, `node_modules/`, and dot-prefixed directories.
- **`ragent_research::session::SynthesizeOutcome`** — `Llm | FallbackEmpty | FallbackError | NoLlm` so callers can attribute the resulting `RESEARCH.md` summary to the path that produced it.
- **`SessionEvent::SynthesizeResult`** — New event emitted after the synthesis phase.
- **`AnalysisEngine::is_noop_marker()`** — Default trait method (`false`) overridden by `NoopAnalysisEngine` (`true`).
- **`LocalGatherer::build_relevance_note`, `build_local_excerpt`, `collect_matched_terms`, `MAX_LOCAL_EXCERPT_LINES`** — Public helpers used by both the gatherer and the synthesis fallback.

### Changed
- **`Source` enum** — `Source::Web`, `Source::Local`, `Source::Other` gained a `body: String` field with `#[serde(default)]`.
- **`LocalGatherer::score_candidates`** — Now returns `(LocalCandidate, Vec<GrepMatch>, Vec<String>)` so the caller has the matched terms and per-line hits needed to build the relevance note and excerpt.
- **`ResearchSession::synthesize`** — Prefers the inline `Source::body` field over reading from the on-disk supporting file.

## Previous Version: 0.1.0-alpha.120

### Added — LLM-driven synthesis for `/research create`
- **`/research create` now analyzes sources with the active LLM** — The TUI research session gained a `Synthesize` phase between gathering and assembly. When an active provider/model is configured, `ResearchSession` sends the captured source bodies to the LLM with a structured prompt requesting `## Summary`, numbered `## Findings` with `[#N]` citations, `## In-Project Cross-References`, and `## Open Questions`. The response is parsed and used to populate `RESEARCH.md`. If the LLM is unavailable, misconfigured, or returns empty output, the session falls back to the existing mechanical summary/findings.
- **New `ragent-research::analysis` module** — Introduces `AnalysisEngine`, `NoopAnalysisEngine`, `LlmAnalysisEngine`, `SourceBody`, and `AnalysisResult`.
- **Updated TUI wiring** — `build_research_session` accepts an optional `ProviderRegistry` and active `ModelRef` and constructs an `LlmAnalysisEngine`.

## Previous Version: 0.1.0-alpha.114

### Added
- **Unified whitespace-tolerant replacement matcher** — `edit`, `multiedit`, and `memory_replace` now share a single seven-pass matcher in `ragent_tools_core::replace` (`find_replacement_range` / `find_replacement_range_diag`). Tolerates CRLF, trailing/leading whitespace, collapsed whitespace (tabs vs spaces, double spaces, mixed indentation), blank-line edge differences, and final-newline mismatches — eliminating `old_str not found` failures caused by common LLM output quirks. `memory_replace` previously used exact-only `String::matches`/`replacen` and now behaves identically to `edit`.

### Changed
- **`multiedit` overlap detection & ordering** — resolves every edit against the original file content, pairwise-checks same-file edits for intersecting byte ranges (clear error naming edit indices + file path), and applies non-overlapping edits highest-end-offset-first so JSON input order no longer matters.
- **`multiedit` / `edit` diagnostics** — `NotFound` errors name the edit index, file, last matching pass attempted, and a best-effort closest-line hint via the new `FindDiag` API.
- **Relative indentation preservation** — `reindent_with` uses the common leading whitespace of all matched file lines and leaves blank lines untouched.

### Fixed
- **`old_str not found` on blank-line / final-newline edge differences** — added blank-line (pass 6) and final-newline (pass 7) normalisation passes.
- **Collapsed-whitespace false `MultipleMatches`** — prefers the candidate whose per-line leading whitespace is closest to the needle's; ties still error.

## Previous Version: 0.1.0-alpha.113

### Fixed
- **Research-system spec status overstated (second occurrence)** — The previous correction in alpha.112 set the spec to `status: draft` with a single `none → draft` audit entry, but the autonomous task batch in the commit after alpha.112 then re-tagged the spec frontmatter as `status: implemented` with a full three-step audit trail (`none → draft → in_progress → implemented`) even though only six of the 56 plan tasks were actually completed (the three foundational types, the crate scaffold, the `ResearchItem` struct, the two gatherers, and the plan-dep parser). The spec frontmatter has been corrected again to `status: in_progress` with a two-step audit (`none → draft → in_progress`) that matches reality: foundational types + scaffold + gatherers + plan-dep parser have shipped, but the `ResearchManager` CRUD methods, the `ResearchSession` orchestrator, the `sources/<NN>.md` writers, the `RESEARCH.md` assembler, the References Index generator, the `research/INDEX.md` cache, the TUI slash-command wiring, the CLI/HTTP endpoints, the spec-integration glue, the benchmarks, and the user docs are still `pending`. The spec status will not be advanced to `implemented` from an autonomous task batch — that transition is reserved for a release where every plan row is `completed`.
- **Workspace version** — Bumped to `0.1.0-alpha.113`.

## Previous Version: 0.1.0-alpha.112

### Fixed
- **Research-system spec status overstated** — The `specs/researchsystem/SPEC.md` frontmatter was tagged `status: implemented` with a three-step audit trail (`none → draft → in_progress → implemented`) even though only four of the 22 plan tasks were completed (the three foundational type definitions and the crate scaffold). The spec frontmatter has been corrected to `status: draft` with a single `none → draft` audit transition that matches reality. The plan task statuses remain as-is: T-001, T-002, T-003, T-005 are `completed`; the remaining 18 tasks are still `pending` and will be re-promoted through the proper lifecycle as the rest of the research system lands.
- **Workspace version** — Bumped to `0.1.0-alpha.112`.

## Previous Version: 0.1.0-alpha.111

### Changed
- **`ask_user` tool promoted from alias to standalone** — The previously-delegating `ask_user` tool in `crates/ragent-agent/src/tool/aliases.rs` now publishes `Event::QuestionRequested` / awaits `Event::QuestionAnswered` directly via the event bus. The standalone `question` tool has been deleted from `ragent-agent`, `ragent-tools-core`, and the TUI question-dialog widget module; the question-rendering responsibility now lives in the TUI's existing event-driven `QuestionRequested` handler in `ragent-tui/src/app.rs`.
- **`ask_user` supports multiple-choice** — The optional `options` array parameter renders a selectable list in the TUI question dialog; omitting `options` keeps the previous free-text input. The tool description and JSON schema now document the new parameter and the `permission_category` is reported as `ask_user` (was `question`).
- **Permission auto-approval key renamed** — `check_permission_with_prompt`'s hardwired always-allow list now matches `ask_user` (was `question`); the corresponding unit test was renamed accordingly.
- **Workspace version** — Bumped to `0.1.0-alpha.111`.

### Added
- **`ragent-research` crate scaffold** — New workspace member under `crates/ragent-research/` providing `ResearchName` (validated, URL-safe identifier newtype), `Source` (Web/Local/Spec/Other enum for the references index), and `ResearchStatus` (draft/in-progress/complete/archived). Follows the requirements in `specs/researchsystem/SPEC.md` (FR-002, FR-011, FR-013). The crate depends only on `ragent-types` and the common workspace deps so it can be reused by both the TUI and HTTP layers once the manager/session/io modules are added.
- **Research system spec + plan** — New `specs/researchsystem/SPEC.md` and `specs/researchsystem/PLAN.md` describing the `/research` slash command, directory conventions, information-gathering session, references index, and integration with the existing spec workflow.

## Previous Version: 0.1.0-alpha.110

### Removed
- **Internal LLM subsystem removed** — The embedded local LLM (Candle GGUF + Foundry Local + LiteRT-LM), the `/internal-llm` slash command family, the TUI `InternalLLM` chat overlay panel, the `InternalLlmConfig` block, the `internal_llm` Cargo feature flag, the `ragent_llm::embedded` module, and all related test files have been removed. Compaction now always uses the provider-compaction fallback. Session titles default to empty. Memory extraction no longer has an LLM prefilter step. The `internal_llm` key in `ragent.json` is silently ignored.

### Changed
- **Workspace version** — Bumped to `0.1.0-alpha.110`.

## Previous Version: 0.1.0-alpha.109

### Added
- **In-process Microsoft Foundry Local backend** — New `FoundryLocalInProcClient` in `crates/ragent-llm/src/providers/foundry_local_inproc_client.rs` loads and runs Foundry Local models inside the ragent process via the `foundry-local-sdk` native core, bypassing the local web service.  Supports model alias resolution, download progress events, device selection (`auto`/`cpu`/`gpu`/`npu`), temperature/max_tokens, tools, and full `StreamEvent` translation (text, tool calls, usage, finish reason).
- **`in_process` provider option** — `provider.foundry_local.in_process` (default `false`) selects the in-process backend; when unset or `false` the existing web-service path is preserved.
- **`RAGENT_FOUNDRY_LOCAL_FORCE_WEB` escape hatch** — Set this environment variable to `1` or `true` to force the web-service path even when `in_process: true` is configured.
- **TUI foundry-mode indicator** — `/internal-llm show` now displays whether the main Foundry Local provider is configured for `in-process` or `web-service` inference when the internal LLM backend is `foundry`.

### Changed
- **Foundry Local provider routing** — `FoundryLocalProvider::create_client()` now branches on the resolved `in_process` flag, returning either `FoundryLocalInProcClient` or the existing `FoundryLocalClient`.
- **Device validation** — `provider.foundry_local.device` values are now validated and rejected if not one of `auto`, `cpu`, `gpu`, or `npu`.
- **Foundry Local documentation** — Updated `PROVIDERS.md` and `SPEC.md` with in-process mode configuration, environment escape hatch, and internal-LLM notes.
- **Workspace version** — Bumped to `0.1.0-alpha.109`.

### Fixed
- **HuggingFace provider discovery failed** — The HuggingFace `/v1/models` router endpoint is public and now works without an API token; discovery no longer errors out immediately when `HF_TOKEN` is unset.  Added `HUGGING_FACE_HUB_TOKEN` as a recognised token source for consistency with the TUI configured-provider detection.  When dynamic discovery fails or returns no models, the TUI now falls back to the provider's static default catalog instead of showing an empty "No models are currently available" dialog.  Empty discovery results are no longer cached, preventing a transient failure from permanently hiding the default models.
- **Task tool family guidance** — Added a dedicated `## Task Tool Family` section to every primary agent's system prompt that clearly distinguishes `task_complete` (autonomous loop signal — only takes `summary`) from `team_task_complete` (team workflow — only takes `team_name` + `task_id`).  The `task_complete`, `team_task_complete`, and `new_task` tool descriptions and JSON schemas now explicitly warn against the most common parameter-confusion mistakes and reject unknown keys via `additionalProperties: false`.  `task_complete` and `list_tasks` are now hardwired auto-approved so the agent can always finish or inspect background tasks without a permission prompt.

## Previous Version: 0.1.0-alpha.108

### Added
- **Foundry Local internal-LLM backend** — New `FoundryLocalExecutor` in `ragent-agent/src/internal_llm/foundry_executor.rs` routes internal-LLM requests through Microsoft Foundry Local instead of the Candle-based embedded runtime. The `/internal-llm foundry` and `/internal-llm embedded` slash commands switch between backends at runtime, and `from_config()` now dispatches on `config.backend` (`"foundry"`/`"foundry_local"` vs default candle).

### Changed
- **Internal LLM backend routing** — `InternalLlmService::from_config()` now selects the executor based on the configured backend name, supporting both Candle (`embedded`) and Foundry Local (`foundry`/`foundry_local`) paths.
- **TUI /internal-llm commands** — Added `foundry` and `embedded` subcommands to the `/internal-llm` slash command for switching backends. Updated autocomplete list, help text, and slash-command definition.
- **Compiled backends display** — Replaced litertlm feature-flag detection with Foundry Local availability check (`is_foundry_local_available()`) in the TUI show/info panel.
- **Workspace version** — Bumped to `0.1.0-alpha.108`.

## Previous Version: 0.1.0-alpha.107

### Added
- **Microsoft Foundry Local provider integration** — Added first-class support for Microsoft Foundry Local as a local LLM provider, including provider setup dialog visibility, `[local]` badge rendering, status-bar abbreviation, health checks, and configuration option merging (`auto_start`, `device`, `models_path`).
- **Headroom compression lifecycle events** — New `Event::CompressionStarted` and `Event::CompressionFinished` events carry per-iteration compression statistics and are surfaced in the TUI status bar and SSE stream.

### Changed
- **Per-iteration compression visibility** — Automatic Headroom compression now publishes start/finish events, activates the status-bar "compressing" indicator, and refreshes the `ctx:` display immediately with the compressed token count.
- **Workspace version** — Bumped to `0.1.0-alpha.106`.

### Fixed
- **Context window display lag after compression** — The status-bar context percentage now updates as soon as compression finishes instead of waiting for the LLM response.

## Previous Release: 0.1.0-alpha.105
- **Bedrock provider** — Refinements to credential handling and SigV4 signing.
- **Multiple tool refinements** — Updated codeindex_search, list_tasks, memory_search, office_write, and spec_list tools.
- **Test improvements** — Updated multiple test files for compatibility with new APIs and module structure.

## Previous: 0.1.0-alpha.104