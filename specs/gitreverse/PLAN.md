# Implementation Plan: GitHub Repository Reverse-Engineering Prompt Generator

## Overview

This plan implements the `gitreverse` specification. The work is divided into
URL parsing, GitHub data fetching, LLM prompt generation, slash-command
wiring, optional flag handling, and verification.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add `parse_repo_url` function to `GitHubClient` | FR-003 | S | Critical | completed | — |
| T-002 | Add `fetch_repo_metadata` async helper returning a typed struct | FR-005 | S | High | completed | T-001 |
| T-003 | Add `fetch_root_tree` async helper returning a `Vec<String>` of file/dir names | FR-006 | S | High | completed | T-002 |
| T-004 | Add `fetch_readme` async helper returning an `Option<String>` (empty if 404) | FR-007 | S | High | completed | T-002 |
| T-005 | Implement `build_reverse_prompt` assembling metadata + tree + README + optional tech into a single context string with 8000-char README truncation | FR-008, NFR-003 | M | High | completed | T-002, T-003, T-004 |
| T-006 | Implement `handle_reverse_command` in `crates/ragent-tui/src/app/slash.rs`: parse args, validate token, show status, spawn async task | FR-001, FR-004, FR-009, FR-010, FR-018 | L | Critical | completed | T-005 |
| T-007 | Add `Reverse` slash-command parsing: positional repo URL, the `/new` scaffold flags (`--language <lang>`, `--type <type>`, `--stack <name>`), `--create <name>`, `help` subcommand | FR-002, FR-011, FR-013 | M | High | completed | T-006 |
| T-008 | Implement `--create <name>` chaining: after LLM generation, invoke `execute_slash_command(&format!("/spec create {} {}", name, generated_prompt))` | FR-012 | M | Medium | completed | T-006, T-007 |
| T-009 | Register `reverse` in `SLASH_COMMANDS` in `crates/ragent-tui/src/app/state.rs` | FR-019 | S | High | completed | T-006 |
| T-010 | Add `reverse` subcommand suggestions to the autocomplete handler in `slash.rs` (the `"spec"` match block near line 110) | FR-019 | S | Low | completed | T-009 |
| T-011 | Add GitHub API error handling: non-success status, 404 not-found, 403/429 rate-limit with reset time | FR-014, FR-015, FR-017 | M | High | completed | T-002, T-003, T-004 |
| T-012 | Add invalid-input rejection: empty, single-word, three-segment identifiers | FR-016 | S | High | completed | T-001 |
| T-013 | Write unit tests for `parse_repo_url` covering full URL, SSH URL, shorthand, `.git` suffix, trailing slash, query string, invalid inputs | FR-003, FR-016, NFR-002 | M | High | completed | T-001, T-012 |
| T-014 | Write unit tests for `build_reverse_prompt` covering all fields present, missing README, truncated README, tech-stack injection | FR-008, NFR-003 | M | Medium | completed | T-005 |
| T-015 | Run `cargo test -p ragent-tui`, `cargo test -p ragent-tools-vcs`, `cargo clippy`, and `cargo fmt` | NFR-002, NFR-004 | S | High | completed | T-013, T-014 |
| T-016 | Update CHANGELOG.md and README.md with the new `/reverse` command | — | S | Low | completed | T-015 |
| T-017 | Extend the `/spec reverse` parser (`parse_reverse`) with the scaffold-only `--folder <path>` target and the `--github` / `--gitlab` hosting flags, forwarding the hosting flags to the shared `/new` parser and rejecting `--folder`/hosting without `--language`+`--type` | FR-026, FR-027 | M | High | completed | T-007 |
| T-018 | Run the shared scaffold engine (`archdoc::run_govcreate_scaffold`) from `handle_reverse_command` before prompt synthesis, reporting the git/remote summary and refusing a non-empty target without aborting the run | FR-027, FR-028 | M | High | completed | T-017 |
| T-019 | Add `--folder`/`--github`/`--gitlab` to the `/spec reverse` help message and autocomplete, and cover the new parser forms with integration tests | FR-026, FR-027, FR-019 | S | Medium | completed | T-017 |
| T-020 | Fix the `--folder` no-op: pass the parsed `ScaffoldRequest` and folder straight to the reverse handler instead of round-tripping through the `/reverse` tokenizer, honour a scaffold-target folder on `/spec create` so the chained `specs/<name>/` lands in the scaffolded project, and report a missing `<repo>` positional as a usage error instead of dropping the command | FR-027, FR-029, FR-026 | M | High | completed | T-017, T-018 |
## Notes

- **T-001** extends `GitHubClient` in `crates/ragent-tools-vcs/src/github/client.rs`
  alongside the existing `detect_repo`. Unlike `detect_repo` (which reads the
  git remote), `parse_repo_url` takes a user-supplied string.
- **T-005** uses `SessionProcessor::process_message` with an agent resolved via
  `apply_selected_model_and_thinking`, mirroring the `/spec create` flow.
- **T-006** follows the `/research` command pattern: set a `⏳`-prefixed status,
  spawn a `tokio::spawn` block, and publish an error event on failure.
- **T-008** chains into `/spec create` by calling `self.execute_slash_command`
  after the LLM response is received. The generated prompt is URL-safe-quoted
  so it forms a single argument.
- **T-011** reuses `GitHubClient::handle_response` which already detects 403/429
  rate-limit headers. The 404 case is handled separately in the metadata fetch.
- **T-015** is the gate before the spec can transition to `implemented`.
- **T-020** removed the text re-render from the `/spec reverse` bridge:
  `render_spec_reverse_args` dropped `--folder` and the scaffold request because
  neither is part of the `/reverse` flag grammar, so the folder was never
  created. The parsed struct is now handed to the handler directly, the scaffold
  runs before the async fetch is spawned, and `/spec create` gained an optional
  `--folder <path>` so the chained spec is written under the scaffolded project.
  A `/spec reverse` whose first token is a flag is also routed to the
  `reverse-usage` form so it reports the missing `<repo>` positional.

## Milestones

1. **Milestone 1 — GitHub data layer** (T-001..T-004)
   - URL parsing and all three API fetch helpers exist and are unit-tested.
2. **Milestone 2 — Prompt assembly** (T-005)
   - Context block is built from metadata, tree, and README.
3. **Milestone 3 — Slash command** (T-006..T-010)
   - `/reverse` is dispatched, displays output, and appears in autocomplete.
4. **Milestone 4 — Optional flags and error handling** (T-007, T-008, T-011, T-012)
   - the `/new` scaffold flags, `--create`, and all error paths work.
5. **Milestone 5 — Verification and docs** (T-013..T-016)
   - Tests pass, clippy is clean, docs updated.