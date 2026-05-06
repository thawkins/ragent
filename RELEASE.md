# Release

## Current Version: 0.1.0-alpha.58

### Added
- **Benchmark runner subsystem** — New `ragent-bench` crate with suite registry (`quick`, `standard`, `agentic` profiles), workbook output (XLSX with `run`, `metrics`, `cases`, `artifacts` sheets), data initialization (sample fixtures and full upstream download), and a `LiveBenchModelRunner` that drives generation via the current provider/model. Includes HumanEval, MBPP, and a Phase-6 native suite adapter.
- **`/bench` TUI slash commands** — Full benchmark workflow in the terminal: `/bench list`, `/bench init <suite-or-all>`, `/bench run <target>` (background task with progress), `/bench status`, `/bench open last`, and `/bench cancel`. Results written to `benches/<suite>/<YYYY-MM-DD>/<provider>/<model>.xlsx`.
- **Benchmark documentation** — New `docs/userdocs/bench.md` and `docs/userdocs/bench.pdf` covering usage, architecture, and workbook schema.
- **XLSX native writer** — `ragent-tools-core/src/xlsx.rs` for creating Office Open XML (OOXML) workbooks without external dependencies.

### Changed
- **HuggingFace model discovery rewritten** — Switched from the HuggingFace Hub API (`/api/models?pipeline_tag=text-generation`) to the authenticated router API (`/v1/models`), keeping only models with at least one live provider and filtering by text input/output modalities. Now captures per-provider context length, max output, and pricing.
- **Tool alias cleanup** — Removed 13 deprecated aliases (`view_file`, `read_file`, `get_file_contents`, `open_file`, `list_files`, `list_directory`, `find_files`, `search_in_repo`, `file_search`, `replace_in_file`, `run_shell_command`, `run_terminal_cmd`, `execute_bash`, `execute_code`). Only `update_file` and `run_code` remain as aliases for `write` and `bash` respectively.
- **Unified config types** — `ragent-agent` now re-exports `Config`, `StreamConfig`, `MemoryConfig`, `ToolVisibilityConfig`, `AgentConfig`, and other config types directly from `ragent-config`, removing the `ragent-agent/src/config/` module entirely. Permission rules are converted between config and runtime types via a new `config_permission_rule_to_runtime()` helper.
- **Enhanced codeindex grep guidance** — System prompt now tells the LLM that `grep` requires the `pattern` parameter.
- **AGENTS.md expanded** — Full tool reference section, code intelligence decision flow, shell execution rules, test organization, and other project guidelines added.
- **Processor import cleanup** — `bash_lists` and `dir_lists` references migrated from `crate::` to `ragent_config::`.
- **Cargo.lock updated** — New dependencies for benchmark crate (flate2, sha2, rust_xlsxwriter, uuid).

## Previous: 0.1.0-alpha.54

### Changed
- **Remove some features and add more feature switches** — Removed the legacy journal subsystem (journal tools, journal viewer panel, journal API routes, journal memory backend) in favour of the newer structured-memory and embedding-based memory stores. Added stream-config default overrides (`timeout_secs`, `max_retries`, `retry_backoff_secs`) so agents can tune network resilience without editing code. Updated tool-visibility metadata across agent and config crates. Simplified TUI layout and status-bar rendering. Reduced binary size and API surface.
