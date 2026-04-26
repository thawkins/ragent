# Release

## Current Version: 0.1.0-alpha.52

### Added
- **Local Git workspace tools (Milestone 1)** — 7 new `git_*` tools in `ragent-tools-vcs` that let the LLM inspect any local git repository:
  `git_status`, `git_log`, `git_diff`, `git_branch`, `git_show`, `git_remote`, `git_tag`
  - Permission categories: `git:read` (auto-allow) and `git:write` (prompt before execution)
  - All tools execute the `git` CLI in the working directory with `GIT_TERMINAL_PROMPT=0`
    to prevent interactive credential hangs
  - Each tool returns both human-readable content and structured JSON metadata
- **22 integration tests** for git tools covering status, log, diff, branch, show, remote, and tag operations
- **Local Git workspace tools (Milestone 2)** — 6 new `git_*` tools in `ragent-tools-vcs` that let the LLM manipulate the local git repository:
  `git_add`, `git_reset`, `git_checkout`, `git_commit`, `git_stash`, `git_cherry_pick`
  - All tools use `git:write` permission category (prompted before execution)
  - 13 new integration tests covering add, commit, reset, checkout, stash, and cherry-pick operations

### Changed
- **LSP documentation removal** — Removed the remaining LSP-specific spec, README, keybinding, audit, and competitive-analysis references; deleted the dedicated LSP exploration docs; and aligned user-facing documentation around CodeIndex and the current MCP/code-intelligence surfaces.

### Removed
- **LSP subsystem entirely removed** — All LSP client, tool, discovery, and configuration functionality has been removed. Code intelligence is now provided exclusively by the CodeIndex system (`codeindex_search`, `codeindex_symbols`, `codeindex_references`, `codeindex_dependencies`). This eliminates external language server dependencies, reduces binary size, and simplifies configuration.
- **AIWiki feature removed** — Removed the AIWiki crate, slash commands, tools, runtime/config wiring, examples, and dedicated user documentation.
- **Final AIWiki cleanup** — Removed the AIWiki removal plan document (AIREM.md) and purged the last remaining AIWiki reference from a comment in `crates/ragent-agent/src/config/mod.rs`.

## Previous: 0.1.0-alpha.51

### Added
- **Profiler module** — Added a new profiler in `crates/ragent-agent/src/session/profiler.rs` for performance instrumentation of agent session processing, with corresponding tests in `crates/ragent-agent/tests/test_profiler.rs`.

### Fixed
- **Question tool multiple-choice support** — Fixed the `question` tool to correctly handle multiple-choice prompts and updated TUI rendering/tests accordingly (new `test_question_multiple_choice.rs`).
