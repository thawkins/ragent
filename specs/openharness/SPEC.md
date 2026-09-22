---
status: draft
audit:
  - { time: 1785155514, from: "none", to: "draft", actor: "system" }
---
# Specification: OpenHarness-Inspired Harness Enhancements

## Executive Summary

The `openharness` research (`research/openharness/RESEARCH.md`, 21 findings, 58
sources) studied [HKUDS/OpenHarness](https://github.com/HKUDS/OpenHarness), a
lightweight, composable, provider-agnostic Python agent harness developed at
the University of Hong Kong. OpenHarness ships ~11,700 lines covering ~98% of
Claude Code's tool surface, with 43+ built-in tools, on-demand Markdown skills
compatible with Anthropic's ecosystem, persistent `MEMORY.md`/`CLAUDE.md`
memory, context auto-compaction, multi-level permission modes with
`PreToolUse`/`PostToolUse` hooks, subagent spawning with a team registry, a
`--dry-run` readiness preview, and MCP HTTP transport.

ragent already implements the majority of these capabilities (tools, skills,
memory, compaction, teams, MCP). This specification defines the **delta** — the
OpenHarness-inspired enhancements that close the remaining gaps and strengthen
ragent's harness layer:

1. **Hook exit-code semantics** — `PreToolUse`/`PostToolUse` hooks that block on
   exit code 2 and warn on exit code 1 (Finding 14), complementing ragent's
   existing JSON-decision protocol.
2. **Dry-run readiness mode** — a `--dry-run` CLI flag that previews resolved
   runtime settings, auth state, skills, tools, and MCP servers without
   executing the model or any tool, emitting a ready/warning/blocked verdict
   (Finding 19).
3. **Skill catalog progressive disclosure** — load only a compact skill catalog
   at startup and activate full instructions on demand (Finding 9).
4. **Per-run cost tracking** — a token/cost summary emitted per session run
   (Finding 3).
5. **MCP HTTP transport** — support the `http` MCP transport type alongside the
   existing stdio transport (Finding 18).

The spec is scoped to incremental improvements on existing ragent crates
(`ragent-agent`, `ragent-config`, `ragent-tui`, `ragent-tools-extended`). No new
Python runtime, no Claude Code plugin compatibility, and no `ohmo` chat
platform integration are in scope.

## Related Research

This spec was informed by the following research items:

- [`openharness`](../research/openharness/RESEARCH.md) — see the captured references for context.

## Scope & Objectives

### Scope

**In scope:**

- `PreToolUse`/`PostToolUse` hook exit-code-based blocking semantics (exit 2 =
  block, exit 1 = warn) layered on top of ragent's existing JSON-decision hooks.
- A `--dry-run` CLI subcommand and `ragent config check` readiness flow that
  validates auth, model resolution, tool visibility, skill discovery, and MCP
  server connectivity without invoking the model.
- A skill catalog structure (`SkillCatalog`) that lists skill names, descriptions,
  and trigger phrases at startup and defers loading full skill bodies until a
  skill is invoked (progressive disclosure).
- A per-run cost summary (input tokens, output tokens, estimated cost) emitted
  as a session event and surfaced in the TUI run-complete banner.
- An `http` transport variant for MCP servers in addition to the existing stdio
  transport.

**Out of scope:**

- A full Python OpenHarness port or compatibility shim.
- Claude Code plugin format compatibility (`.claude/plugins`).
- `ohmo` chat-platform integration (Feishu/Slack/Telegram/Discord).
- Subprocess teammate headless worker mode (ragent already uses in-process
  `tokio` tasks for teammates).
- `MEMORY.md`/`CLAUDE.md` format changes (ragent's three-tier memory system is
  unchanged).

### Objectives

1. Give operators a deterministic, model-independent enforcement layer for
   security-critical tool policies via hook exit codes.
2. Let operators validate a ragent deployment before invoking any model or tool
   via a single `--dry-run` command.
3. Reduce session-startup context cost by deferring full skill body loading
   until invocation.
4. Provide per-run cost visibility so operators can detect runaway spend.
5. Broaden MCP server compatibility to include HTTP-transport servers.

---

## Requirements

### FR-001 — Hook registration unchanged (ubiquitous)

The system **shall** continue to load hook configurations from the `hooks` array
in `ragent.json` and dispatch them via the existing `HookTrigger` enum
(`OnSessionStart`, `OnSessionEnd`, `OnError`, `OnPermissionDenied`,
`PreToolUse`, `PostToolUse`). Existing JSON-decision parsing for `PreToolUse`
(`{"decision":"allow"}`, `{"decision":"deny","reason":...}`,
`{"modified_input":...}`) **shall** remain fully supported and unchanged.

> *Ubiquitous requirement — applies to every hook in every session without
> exception; existing behavior must not regress.*

### FR-002 — PreToolUse exit-code blocking (event-driven)

When a `PreToolUse` hook process exits with a non-zero status code, the system
**shall** interpret the exit code as follows **before** parsing stdout JSON:

- **Exit code 2** — the tool call **shall** be blocked. The system **shall**
  return a denial to the agent with the hook's stderr (trimmed, capped at 500
  characters) as the reason, and **shall not** execute the tool.
- **Exit code 1** — the tool call **shall** be allowed to proceed, but the
  system **shall** emit a `tracing::warn!` log entry containing the hook
  command, tool name, and stderr, and **shall** publish an `Event::HookWarning`
  on the event bus so the TUI can surface it.
- **Exit code ≥ 3** — the system **shall** treat the hook as failed (not
  blocked) and **shall** fall through to the normal permission flow with a
  `tracing::error!` diagnostic, since such codes indicate a hook bug rather
  than a policy decision.

> *Event-driven requirement — triggers when a PreToolUse hook process exits
> non-zero.*

### FR-003 — PreToolUse exit-code precedence over JSON (state-driven)

While a `PreToolUse` hook exits with code 2, the system **shall** ignore any
`{"decision":"allow"}` or `{"modified_input":...}` JSON in the hook's stdout
and **shall** treat the call as blocked. A non-zero blocking exit code takes
absolute precedence over any stdout JSON that would otherwise permit the call.

> *State-driven requirement — holds whenever a blocking exit code is present;
> stdout cannot override a block.*

### FR-004 — PostToolUse exit-code warning (event-driven)

When a `PostToolUse` hook exits with code 2, the system **shall** mark the tool
result as policy-violated and **shall** publish an `Event::ToolResultFlagged`
with the hook's stderr as the reason. The tool result already returned to the
agent **shall not** be retroactively suppressed (the call already executed),
but the flag **shall** appear in the session log and TUI. When a `PostToolUse`
hook exits with code 1, the system **shall** emit a `tracing::warn!` and
publish `Event::HookWarning`.

> *Event-driven requirement — triggers when a PostToolUse hook exits non-zero
> after a tool has already run.*

### FR-005 — Dry-run subcommand (event-driven)

When the user invokes `ragent --dry-run` (or `ragent config check`), the system
**shall** execute a readiness check that, in order:

1. Loads and merges all config layers (global, project, `RAGENT_CONFIG`,
   `RAGENT_CONFIG_CONTENT`) and reports any JSON parse errors with file path,
   line, column, and the offending source line.
2. Resolves the active provider and model and reports whether the required
   environment variables / API keys are present, **without** making any network
   call to the provider.
3. Discovers skills (bundled, personal, project) and reports the count and any
   load errors.
4. Enumerates registered tools (respecting `tool_visibility`) and reports the
   count per tool family.
5. For each configured MCP server, performs a lightweight connectivity check
   (stdio: spawn + immediate drop; http: a HEAD request with a 5-second timeout)
   and reports ready/blocked.
6. Emits a single readiness verdict: `READY` (no warnings or blocks),
   `WARNING` (one or more non-fatal issues), or `BLOCKED` (one or more issues
   that would prevent a normal session from starting).

The dry run **shall not** invoke the LLM, execute any tool, or spawn any
subagent. The verdict **shall** be printed to stdout and **shall** exit with
code 0 for `READY`/`WARNING` and code 1 for `BLOCKED`.

> *Event-driven requirement — triggers when the user explicitly requests a
> dry run.*

### FR-006 — Dry-run readiness report format (ubiquitous)

The dry-run output **shall** be a human-readable, sectioned report with one
section per check (`config`, `auth`, `skills`, `tools`, `mcp`) and a final
`Verdict:` line. Each section **shall** list its items with a status marker
(`✓`, `⚠`, `✗`) and a one-line description. The report **shall** also be
serialisable as JSON when `--dry-run --json` is passed, for CI/CD consumption.

> *Ubiquitous requirement — every dry run produces the same structured
> report.*

### FR-007 — Skill catalog at startup (state-driven)

While skills are being registered at session startup, the system **shall**
populate a `SkillCatalog` containing, for each skill, only the skill `name`,
a one-line `description`, the `trigger` phrase, and the `scope`
(bundled/personal/project). The full skill body **shall not** be read into the
system prompt at startup. The catalog **shall** be injected into the system
prompt as a compact list so the model knows which skills exist and how to
invoke them.

> *State-driven requirement — holds during the skill-registration phase of
> every session startup.*

### FR-008 — Skill body on-demand loading (event-driven)

When a skill is invoked (by the user via `/name` or by the agent via the skill
trigger phrase), the system **shall** read the full skill body from disk at
that point, perform argument substitution and context injection, and inject
the result as a user message — exactly as the existing `invoke_skill` path
does today. The body **shall** be loaded at most once per session and cached
in memory thereafter.

> *Event-driven requirement — triggers when a skill is actually invoked.*

### FR-009 — Skill catalog backward compatibility (ubiquitous)

The system **shall** preserve the existing `SkillRegistry` public API
(`load`, `register`, `get`, `list_all`, `list_user_invocable`,
`list_agent_invocable`). The `SkillCatalog` **shall** be derivable from a
loaded `SkillRegistry` via a new `SkillRegistry::catalog()` method that
returns `Vec<SkillCatalogEntry>` without reading any skill bodies from disk.

> *Ubiquitous requirement — existing skill callers must continue to compile
> and behave unchanged.*

### FR-010 — Per-run cost summary event (event-driven)

When a session run completes (the agent loop exits, whether by
`task_complete`, user stop, or error), the system **shall** compute a
`RunCostSummary` from the accumulated `Usage` records for that run and
publish it as an `Event::RunCostSummary` on the event bus. The summary
**shall** include: `input_tokens`, `output_tokens`, `cache_read_tokens`,
`cache_write_tokens`, `tool_calls`, `estimated_cost_usd` (computed from
per-model price tables), and `duration_secs`.

> *Event-driven requirement — triggers at the end of every session run.*

### FR-011 — Cost price table (ubiquitous)

The system **shall** maintain a price table mapping model identifiers to
per-1M-token input and output prices (USD). The table **shall** be
populated for the built-in provider models (Anthropic, OpenAI, Gemini,
Ollama) and **shall** default to `0.0` for unknown models so the cost
summary is always computable. The price table **shall** be overridable via
a `models.prices` array in `ragent.json`.

> *Ubiquitous requirement — every model has an entry, even if the price is
> zero.*

### FR-012 — TUI run-complete banner (state-driven)

While the TUI is active and an `Event::RunCostSummary` is received, the TUI
**shall** display a one-line summary banner in the chat area showing
`⟡ run complete · {input}+{output} tokens · ${cost} · {duration}s` and
**shall** log the full `RunCostSummary` to the log panel. The banner
**shall** be dismissible by any key press.

> *State-driven requirement — holds whenever the TUI is visible and a run
> completes.*

### FR-013 — MCP HTTP transport (optional)

Where an MCP server configuration entry specifies `"transport": "http"`
with a `url` field, the system **shall** connect to the server via HTTP
(using the existing `reqwest::Client`) instead of spawning a stdio child
process. The HTTP client **shall** send JSON-RPC requests to the configured
`url` and **shall** support a configurable `headers` map for
authentication. Servers without a `transport` field **shall** default to
`stdio` (unchanged behavior).

> *Optional requirement — only applies to MCP server entries that opt into
> HTTP transport.*

### FR-014 — MCP HTTP auto-reconnect (event-driven)

When an MCP HTTP transport connection fails (network error or non-2xx
response), the system **shall** retry up to 3 times with exponential
backoff (1s, 2s, 4s) and, on persistent failure, **shall** mark the MCP
server as `disconnected` and **shall** emit a `tracing::warn!`. The server
**shall** be retried on the next tool invocation that targets it.

> *Event-driven requirement — triggers when an HTTP MCP connection fails.*

### FR-015 — Hook exit-code documentation (ubiquitous)

The system **shall** document the exit-code convention (2 = block, 1 = warn,
≥3 = error) in the `HookConfig` doc comment, the `ragent.json` schema
description, and the `docs/howtos/howto_hooks.md` file (creating it if
absent). The JSON-decision protocol **shall** be documented as an
alternative, mutually-complementary mechanism.

> *Ubiquitous requirement — the convention is documented wherever hooks
> are described.*

### FR-016 — No silent hook failures (unwanted)

The system **shall not** silently swallow a `PreToolUse` or `PostToolUse`
hook that fails to spawn (e.g., `sh` not found) or times out. A spawn
failure or timeout **shall** be treated as exit code ≥3 (hook error) and
**shall** produce a `tracing::error!` entry with the hook command and the
failure reason. The tool call **shall** fall through to the normal
permission flow in this case so a broken hook cannot lock the user out of
all tool use.

> *Unwanted requirement — prevents a misconfigured hook from silently
> disabling the permission system or bricking the session.*

### FR-017 — Dry-run does not touch the network (unwanted)

The dry-run readiness check **shall not** make any authenticated API call
to any LLM provider. Auth validation is limited to checking that the
relevant environment variables or config-file credentials are present and
non-empty. The only network calls permitted during a dry run are the MCP
HTTP connectivity HEAD requests (FR-005 step 5), each with a 5-second
timeout.

> *Unwanted requirement — prevents a dry run from accidentally consuming
> tokens or leaking credentials.*

### FR-018 — Cost summary not persisted in plaintext (unwanted)

The `RunCostSummary` **shall not** be written to the session transcript on
disk in a way that exposes the per-run dollar cost to anyone reading the
exported session. The summary **shall** be available via the event bus and
the `/cost` slash command in the TUI, but **shall** be omitted from session
export JSON unless the export explicitly requests cost data.

> *Unwanted requirement — protects cost data from leaking via session
> exports.*

---

## Non-Functional Requirements

### NFR-001 — Performance

The skill catalog load at startup (FR-007) **shall** add less than 5 ms to
session startup time for a registry of 50 skills, measured against the
existing `SkillRegistry::load` baseline. The dry-run readiness check
(FR-005) **shall** complete in under 2 seconds excluding MCP HTTP timeouts.

### NFR-002 — Minimal new dependencies

The implementation **shall** introduce no new external crate dependencies.
HTTP MCP transport (FR-013) reuses the existing `reqwest::Client`. The price
table (FR-011) is a `HashMap<&'static str, (f64, f64)>`.

### NFR-003 — Testability

Each requirement **shall** have at least one unit or integration test:
- FR-002/FR-003: a `PreToolUse` hook script exiting 1, 2, and 3 with
  fixture tool inputs.
- FR-004: a `PostToolUse` hook exiting 1 and 2.
- FR-005/FR-006: a dry run against a temp-dir config with known-good and
  known-bad states, asserting the verdict and exit code.
- FR-007/FR-008/FR-009: a `SkillRegistry::catalog()` test asserting no
  skill bodies are read, plus an `invoke_skill` test asserting the body is
  loaded on demand.
- FR-010/FR-011: a `RunCostSummary` computation test with a fixture
  `Usage` log and a fixture price table.
- FR-013/FR-014: a mocked HTTP MCP server test using a local `wiremock`
  or `httptest` server (already in dev-dependencies) for transport and
  reconnect behavior.

### NFR-004 — Documentation

The `docs/howtos/howto_hooks.md` file **shall** be created (or updated if it
exists) documenting the exit-code convention and the JSON-decision protocol
side by side. The `QUICKSTART.md` **shall** gain a `--dry-run` usage
example. The `SPEC.md` harness section **shall** reference the new
`SkillCatalog` and `RunCostSummary` types.

---

## Constraints & Assumptions

### Constraints

1. The implementation must compile and test cleanly with `cargo test`,
   `cargo clippy`, and `cargo fmt --check` on Rust edition 2024.
2. Hook exit-code semantics must be additive — existing `ragent.json` hook
   configs that produce JSON decisions and exit 0 must behave exactly as
   before.
3. The `SkillCatalog` must not change the on-disk skill file format
   (`SKILL.md`).

### Assumptions

1. ragent's existing `EventBus` can carry two new event variants
   (`HookWarning`, `ToolResultFlagged`, `RunCostSummary`) without breaking
   existing subscribers.
2. The `reqwest::Client` shared across the tools layer is available for MCP
   HTTP transport without additional connection-pool tuning.
3. Per-model price data is static enough to ship a built-in table updated
   per release, with the `models.prices` config override for hot-fixes.

---

## Interfaces & Dependencies

### Internal Interfaces

| Component | Interface | Purpose |
|-----------|-----------|---------|
| `ragent-agent::hooks` | `PreToolUseResult` gains `Blocked { reason }` variant; `run_pre_tool_use_hooks` returns it on exit 2 | Exit-code blocking |
| `ragent-agent::hooks` | New `PostToolUseResult` enum (`Ok`, `Flagged { reason }`, `Warn { message }`) | PostToolUse exit codes |
| `ragent-agent::skill` | `SkillRegistry::catalog() -> Vec<SkillCatalogEntry>` | Progressive disclosure |
| `ragent-agent::skill` | `SkillCatalogEntry { name, description, trigger, scope }` | Catalog entry type |
| `ragent-types::event` | `Event::HookWarning`, `Event::ToolResultFlagged`, `Event::RunCostSummary` | New event bus variants |
| `ragent-agent::cost` | `RunCostSummary`, `compute_run_cost(usage, prices) -> RunCostSummary` | Cost tracking |
| `ragent-config::config` | `models.prices: Vec<PriceEntry>` field | Price override |
| `src/cli.rs` | `--dry-run` flag + `config check` subcommand | Readiness flow |
| `ragent-agent::mcp` | `HttpMcpClient` implementing the existing MCP client trait | HTTP transport |

### External Dependencies

| Dependency | Version | Purpose |
|------------|---------|---------|
| (none new) | — | All features reuse existing workspace crates |

---

## Glossary

| Term | Definition |
|------|------------|
| **Agent harness** | The non-model infrastructure layer surrounding an LLM: tools, memory, permissions, execution loops (Finding 6). |
| **EARS** | Easy Approach to Requirements Syntax — the `shall`-based requirement notation used in this spec. |
| **Progressive disclosure** | Loading only a compact catalog at startup and full instructions on demand, to keep context windows lean (Finding 9). |
| **Readiness verdict** | The `READY`/`WARNING`/`BLOCKED` result of a dry run (Finding 19). |
| **PreToolUse / PostToolUse** | Lifecycle hooks fired before/after a tool execution, capable of blocking or flagging the call (Findings 13–14). |
| **RunCostSummary** | A per-run aggregate of token usage and estimated dollar cost (Finding 3). |

---

*End of Specification*