---
spec_id: openharness
---

# Implementation Plan: OpenHarness-Inspired Harness Enhancements

## Overview

This plan implements the `openharness` specification
(`specs/openharness/SPEC.md`): five incremental harness enhancements inspired by
the OpenHarness research (`research/openharness/RESEARCH.md`):

1. **Hook exit-code semantics** — `PreToolUse`/`PostToolUse` hooks that block on
   exit code 2, warn on exit code 1, and error on ≥3, layered on ragent's
   existing JSON-decision hooks (FR-002 – FR-004, FR-016).
2. **Dry-run readiness mode** — a `--dry-run` / `config check` flow that
   previews resolved config, auth, skills, tools, and MCP servers and emits a
   READY/WARNING/BLOCKED verdict (FR-005 – FR-006, FR-017).
3. **Skill catalog progressive disclosure** — a `SkillCatalog` populated at
   startup without reading skill bodies; full bodies loaded on invocation
   (FR-007 – FR-009).
4. **Per-run cost tracking** — a `RunCostSummary` computed at run end from
   accumulated `Usage` records and a model price table (FR-010 – FR-012,
   FR-018).
5. **MCP HTTP transport** — an `http` transport variant for MCP servers with
   auto-reconnect (FR-013 – FR-014).

All work targets existing crates (`ragent-agent`, `ragent-config`,
`ragent-types`, `ragent-tui`, `src/cli.rs`). No new external crate
dependencies are introduced (NFR-002).

## Architecture

```
crates/ragent-agent/src/
├── hooks/
│   └── mod.rs                  # + Blocked/Flagged/Warn variants, exit-code parsing
├── skill/
│   └── mod.rs                  # + SkillCatalogEntry, SkillRegistry::catalog()
├── mcp/
│   └── http.rs                 # NEW: HttpMcpClient (reqwest-based JSON-RPC)
├── cost/
│   └── mod.rs                  # NEW: RunCostSummary, compute_run_cost(), PriceTable
├── session/
│   └── processor.rs            # + publish RunCostSummary at run end

crates/ragent-types/src/
└── event.rs                    # + HookWarning, ToolResultFlagged, RunCostSummary

crates/ragent-config/src/
└── config.rs                   # + models.prices: Vec<PriceEntry>

crates/ragent-tui/src/
└── app/                        # + RunCostSummary banner, HookWarning toast

src/cli.rs                      # + --dry-run flag, config check subcommand
docs/howtos/howto_hooks.md      # NEW: exit-code + JSON-decision docs
```

### Data flow — hook exit codes

```
Agent calls tool
      │
      ▼
run_pre_tool_use_hooks(hooks, wd, tool_name, tool_input)
      │
      ├─► sh -c <hook.command>  ──► exit status
      │
      ├─► exit 2  → PreToolUseResult::Blocked { stderr }   → deny to agent (no exec)
      ├─► exit 1  → warn! + Event::HookWarning             → fall through to normal perms
      ├─► exit ≥3 → error! (hook bug)                       → fall through to normal perms
      └─► exit 0  → parse stdout JSON (existing behavior)  → Allow / Deny / ModifiedInput / NoDecision
```

### Data flow — dry run

```
ragent --dry-run [--json]
      │
      ▼
load + merge all config layers ──► report parse errors (file:line:col)
      │
      ▼
resolve provider + model ──► check env vars / config keys present (no network)
      │
      ▼
SkillRegistry::load → catalog() ──► report skill count + load errors
      │
      ▼
enumerate tools (respecting tool_visibility) ──► report per-family counts
      │
      ▼
for each MCP server: stdio spawn-drop / http HEAD (5s timeout)
      │
      ▼
Verdict: READY / WARNING / BLOCKED  ──► stdout (or JSON) + exit code 0/1
```

---

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add `PreToolUseResult::Blocked { reason }` variant and parse exit codes 1/2/≥3 in `run_pre_tool_use_hooks`; stdout JSON ignored when exit 2 | FR-002, FR-003, FR-016 | M | Critical | completed | — |
| T-002 | Add `PostToolUseResult` enum (`Ok`, `Flagged { reason }`, `Warn { message }`) and parse exit codes 1/2 in `run_post_tool_use_hooks`; publish `Event::HookWarning` (1) and `Event::ToolResultFlagged` (2) | FR-004, FR-016 | M | High | completed | T-001 |
| T-003 | Treat hook spawn failure / timeout as exit ≥3 (hook error) with `tracing::error!`; fall through to normal permission flow so a broken hook cannot lock out tools | FR-016 | S | Critical | completed | T-001 |
| T-004 | Add `Event::HookWarning { session_id, tool_name, command, message }` and `Event::ToolResultFlagged { session_id, tool_name, reason }` to `ragent-types::event` | FR-002, FR-004 | S | High | completed | T-001 |
| T-005 | Unit tests for `PreToolUse` exit codes 1, 2, 3, and 0 with JSON decision, using fixture hook scripts in a temp dir | FR-002, FR-003, NFR-003 | M | Critical | completed | T-001 |
| T-006 | Unit tests for `PostToolUse` exit codes 1 and 2, asserting `Event::HookWarning` and `Event::ToolResultFlagged` are emitted | FR-004, NFR-003 | S | High | completed | T-002, T-004 |
| T-007 | Add `SkillCatalogEntry { name, description, trigger, scope }` and `SkillRegistry::catalog() -> Vec<SkillCatalogEntry>` that derives entries from already-loaded skill metadata without reading bodies | FR-007, FR-009 | M | High | completed | — |
| T-008 | Update `build_system_prompt*` functions to inject the `SkillCatalog` (compact list) instead of full skill bodies at startup; cache invoked skill bodies in a per-session `HashMap<String, String>` loaded on demand | FR-007, FR-008 | M | High | completed | T-007 |
| T-009 | Unit test: `SkillRegistry::catalog()` returns correct entries and does not touch skill body files (assert via a temp-dir registry with spy reads); unit test: `invoke_skill` loads the body on demand and caches it | FR-007, FR-008, FR-009, NFR-003 | M | High | completed | T-007, T-008 |
| T-010 | Create `crates/ragent-agent/src/cost/mod.rs` with `RunCostSummary` struct, `PriceTable` (`HashMap<&'static str, (f64, f64)>`), built-in price entries for Anthropic/OpenAI/Gemini/Ollama, and `compute_run_cost(usage_log, prices) -> RunCostSummary` | FR-010, FR-011, NFR-002 | M | High | completed | — |
| T-011 | Add `models.prices: Vec<PriceEntry>` to `ragent-config::Config` with serde default + merge-overlay; `PriceEntry { model, input_per_1m, output_per_1m }` overrides built-in table | FR-011 | S | Medium | completed | T-010 |
| T-012 | At session run end (agent loop exit), accumulate `Usage` records, call `compute_run_cost`, and publish `Event::RunCostSummary` on the event bus | FR-010 | S | High | completed | T-010, T-004 |
| T-013 | TUI: on `Event::RunCostSummary`, render a one-line `⟡ run complete · {in}+{out} tokens · ${cost} · {dur}s` banner (dismiss on keypress) and log full summary to the log panel | FR-012 | S | Medium | completed | T-012 |
| T-014 | Unit tests for `compute_run_cost`: known usage + known prices → expected cost; unknown model → cost 0.0; empty usage → zero summary | FR-010, FR-011, NFR-003 | S | High | completed | T-010 |
| T-015 | Create `crates/ragent-agent/src/mcp/http.rs` with `HttpMcpClient` that sends JSON-RPC over `reqwest::Client` to a configured `url` with optional `headers` map; implement the existing MCP client trait | FR-013 | L | High | completed | — |
| T-016 | Wire `HttpMcpClient` into the MCP server loader: when a config entry has `"transport": "http"` + `url`, instantiate `HttpMcpClient` instead of the stdio child process; default to `stdio` when `transport` is absent | FR-013 | S | High | completed | T-015 |
| T-017 | Implement auto-reconnect for `HttpMcpClient`: on network error or non-2xx, retry up to 3 times with 1s/2s/4s backoff, then mark `disconnected` + `tracing::warn!`; retry on next tool invocation targeting that server | FR-014 | M | Medium | completed | T-015 |
| T-018 | Integration test for HTTP MCP transport using a local mock HTTP server (existing `httptest`/`wiremock` dev-dep): assert tool listing + tool call round-trip; test reconnect after a forced 503 | FR-013, FR-014, NFR-003 | M | High | completed | T-016, T-017 |
| T-019 | Add `--dry-run` flag to `src/cli.rs` (top-level) and a `config check` subcommand that both invoke a new `run_dry_run()` function returning the readiness report + exit code | FR-005 | M | High | completed | — |
| T-020 | Implement `run_dry_run()`: config load+merge with parse-error reporting, provider/model env-var check (no network), `SkillRegistry::load` + `catalog()`, tool enumeration per family, MCP connectivity check (stdio spawn-drop / http HEAD 5s), verdict computation (READY/WARNING/BLOCKED) | FR-005, FR-017 | L | Critical | completed | T-007, T-019 |
| T-021 | Add `--json` output mode to the dry run: serialise the readiness report as a JSON object with per-section status arrays and a `verdict` field, for CI/CD consumption | FR-006 | S | Medium | completed | T-020 |
| T-022 | Integration test: dry run against a temp-dir config with a known-good state asserts `READY` + exit 0; a broken-MCP state asserts `BLOCKED` + exit 1; a missing-skill state asserts `WARNING` + exit 0 | FR-005, FR-006, FR-017, NFR-003 | M | Critical | completed | T-020 |
| T-023 | TUI: subscribe to `Event::HookWarning` and render a transient toast / log entry; subscribe to `Event::ToolResultFlagged` and render a flagged marker in the log panel | FR-002, FR-004 | S | Low | completed | T-004 |
| T-024 | Ensure `RunCostSummary` is omitted from session export JSON by default; add an opt-in `include_cost` flag to the export path | FR-018 | S | Low | completed | T-012 |
| T-025 | Create `docs/howtos/howto_hooks.md` documenting exit-code convention (2=block, 1=warn, ≥3=error) and the JSON-decision protocol side by side with examples | FR-015, NFR-004 | S | Medium | completed | T-001, T-002 |
| T-026 | Update `QUICKSTART.md` with a `--dry-run` usage example and a `--dry-run --json` CI example; update `SPEC.md` harness section to reference `SkillCatalog` and `RunCostSummary` | NFR-004 | S | Low | completed | T-020 |
| T-027 | Run `cargo test`, `cargo clippy`, `cargo fmt --check`, `cargo test -p ragent-agent`, `cargo test -p ragent-config`, `cargo test -p ragent-tui` to confirm no regressions | NFR-001, NFR-002 | S | Critical | completed | T-001–T-026 |
## Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Exit-code blocking changes break existing `ragent.json` hook configs that exit non-zero while returning JSON allow decisions | Medium | High | FR-003 makes exit 2 absolute; exit 0 + JSON is unchanged. Test with existing config fixtures. |
| `SkillCatalog` deferral breaks an existing caller that reads skill bodies at startup | Low | Medium | FR-009 preserves `SkillRegistry` API; `catalog()` is additive. Audit `build_system_prompt*` callers. |
| Price table goes stale as providers change pricing | High | Low | FR-011 allows `models.prices` config override for hot-fixes; table is per-release. |
| MCP HTTP transport reconnect storm degrades UX | Low | Medium | FR-014 caps retries at 3 with backoff and marks server `disconnected` until next targeted invocation. |
| Dry-run MCP HEAD request hangs on an unresponsive server | Medium | Medium | FR-005/FR-017 enforce a 5-second timeout per HEAD request. |

---

## Definition of Done

1. All 18 functional requirements (FR-001 – FR-018) have at least one passing
   test (NFR-003).
2. `cargo test`, `cargo clippy`, and `cargo fmt --check` are clean across the
   workspace.
3. `docs/howtos/howto_hooks.md` exists and documents the exit-code convention.
4. `QUICKSTART.md` includes a `--dry-run` example.
5. No new external crate dependencies were added (`Cargo.toml` unchanged in
   `[dependencies]`).
6. The `openharness` spec status is moved to `implemented` in its frontmatter.

---

*End of Implementation Plan*