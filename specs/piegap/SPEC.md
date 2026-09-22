---
status: draft
audit:
  - { time: 1786691980, from: "none", to: "draft", actor: "system" }
---
# Pie Feature Gap

## Overview

The [pie](https://github.com/c4pt0r/pie) project is a Rust-based terminal AI
coding agent with several features that ragent does not currently implement.
This specification identifies the substantively different features in pie that
are absent from ragent (not merely renamed or functionally equivalent), and
defines requirements for porting each one as a **standalone, independently
selectable** module so that each feature can be built, tested, and activated in
isolation.

### Gap Analysis Method

Each pie feature was compared against ragent's codebase using source-file
inspection and code-index search. Features already present in ragent under a
different name but with equivalent behaviour (e.g., clipboard image paste,
`@file` mentions, thinking-level control, cost tracking, cron scheduling,
context compaction, conversation/session search, permission system, MCP client)
were excluded. Only features that are **absent** or **substantively different**
in ragent are listed below.

### Identified Gaps

| #    | Feature                                                      | pie location                          | ragent status                         |
| ---- | ------------------------------------------------------------ | ------------------------------------- | ------------------------------------- |
| G-01 | Dynamic trigger rules (natural-language event automation)    | `triggers/dynamic.rs`               | Absent                                |
| G-02 | MCP notification push events (server-pushed trigger sources) | `triggers/mcp_notification_hook.rs` | Absent                                |
| G-03 | Stateful loops + triage inbox                                | `triggers/cron.rs`, `inbox.rs`    | Absent                                |
| G-04 | Lifecycle hooks (command + webhook on agent events)          | `hooks.rs`                          | Absent                                |
| G-05 | Portable session archive export/import                       | `session_archive.rs`                | Partial (JSON export only, no import) |
| G-06 | Bug report generation                                        | `bug_report.rs`                     | Absent                                |
| G-07 | Reusable prompt templates                                    | `templates.rs`                      | Absent                                |
| G-08 | LSP integration (after-edit diagnostics)                     | `lsp.rs`, `lsp_supervisor.rs`     | NOT REQUIRED                          |
| G-09 | Compiled extension registry (plugin system)                  | `extensions.rs`                     | NOT REQUIRED                          |
| G-10 | Goal-based autonomous stop hook                              | `goal.rs`                           | Absent                                |
| G-11 | OpenAI Responses API provider (reasoning replay)             | `providers/openai_responses.rs`     | Absent                                |
| G-12 | Browser-based web UI (local web interface)                   | `ui/web.rs`                         | Absent (HTTP API only, no browser UI) |
| G-13 | `/undo` — remove last conversation turn                   | `commands.rs`                       | Absent                                |
| G-14 | Session naming (`/name`)                                   | `commands.rs`                       | Absent                                |

## Requirements

### FR-001 (Ubiquitous) — Standalone Module Independence

The system **shall** implement each gap feature as a self-contained module
that can be compiled, tested, and activated independently of every other gap
feature. No gap feature shall require another gap feature to be present in
order to compile or function, except where an explicit dependency is declared
in the implementation plan.

### FR-002 (Event-Driven) — Dynamic Trigger Rules

**When** the user issues a natural-language trigger request (e.g., "when
$HOME/build.done exists, run cargo test"), the system **shall** create a
session-scoped trigger rule that polls a condition on a configurable interval
and, when the condition matches, executes the specified action in a sub-agent
with a fresh context.

- Rules shall be fire-once by default with an option for repeating.
- Rules shall persist alongside the active session and restore on resume.
- The poll interval shall be configurable via `ragent.json`.
- Rule output shall not interrupt the main chat unless `promote_to_chat` is
  explicitly requested.
- The system shall provide `/triggers` slash commands to list, enable,
  disable, and remove dynamic rules.

### FR-003 (State-Driven) — MCP Notification Push Events

**When** a configured MCP server pushes a notification frame, the system
**shall** normalize the notification into a trigger envelope and route it
through the same trigger runtime as dynamic rules, with deduplication and
cycle suppression.

- MCP servers configured with `inject_summary` shall inject a bounded summary
  into the parent chat without a model call.
- MCP servers configured with `inject_and_run` shall inject a prompt and run
  one model turn in the parent's full tool context.
- Raw notification payloads shall not be persisted as chat content or trigger
  audit unless the source explicitly opts in.

### FR-004 (Event-Driven) — Stateful Loops

**When** the user creates a cron job with the `--stateful` flag, the system
**shall** run the job in a background sub-agent that maintains a cross-run
state file, injects the previous run's notes into each execution, and reports
findings to a global triage inbox using `<loop-state>` and `<inbox>` output
protocol tags.

- Loop state shall be capped at 2000 characters.
- Inbox entries shall be capped at 500 characters each, with at most 16
  findings honored per run.
- The inbox shall be a global JSONL file stored at `<working_dir>/log/inbox/inbox.jsonl`, shared across all sessions.
- The system shall provide `/inbox` slash commands to list, claim, dismiss,
  and clear findings.

### FR-005 (Optional) — Lifecycle Hooks

**If** the user has configured a `hooks.json` file, the system **shall**
execute shell commands and/or POST JSON to webhooks when agent lifecycle
events fire (agent_start, agent_end, turn_start, turn_end, tool_start,
tool_end, compaction).

- Hooks shall be best-effort: failures shall not fail the agent turn.
- Command hooks shall receive environment variables with session, model, and
  tool context.
- Project-local hooks shall be gated behind an explicit opt-in.
- Webhook payloads shall be bounded and redacted.
- Each hook shall support an optional `tool` filter for tool-scoped events.

### FR-006 (Optional) — Portable Session Archive

**If** the user requests a session archive export, the system **shall**
produce a portable archive file containing a manifest with SHA-256 checksums,
the session transcript, and any session-scoped automation sidecars (trigger
rules, cron jobs, loop state).

- Import shall verify checksums and optionally re-activate automation.
- The manifest shall include a sensitivity warning about preserved transcript
  content.
- Import shall be disabled-by-default for automation sidecars unless the user
  explicitly activates them.

### FR-007 (Event-Driven) — Bug Report Generation

**When** the user issues `/bug-report`, the system **shall** generate a
diagnostic dump file containing a snapshot of the current session state
(model, agent, tool count, cost summary), a tail of recent log output, and
the session transcript.

- The dump shall be redacted to strip well-known secret patterns.
- The output file shall be written under the ragent data directory.
- The system shall report the file path to the user.

The bug report should be placed into the "log" folder in the root project directory

### FR-008 (Optional) — Reusable Prompt Templates

**If** the user has placed markdown template files in a templates directory,
the system **shall** load them and make them available via a `/template`
slash command that renders the template with variable substitution and
submits the result as a prompt.

- Project-local templates shall override user-global templates on name
  collision.
- Templates shall support standard environment variables (working directory,
  date).

### FR-009 (State-Driven) — LSP Integration

This feature is NOT Required

### FR-010 (Optional) — Compiled Extension Registry

This feature is NOT required

### FR-011 (State-Driven) — Goal-Based Autonomous Stop Hook

**When** the user sets a goal condition via `/goal <condition>`, the system
**shall** evaluate the goal after each successful model turn using a separate
evaluator model call with a bounded transcript, and shall stop the agent loop
when the goal is achieved.

- The evaluator shall return structured JSON; missing evidence shall default
  to "not done".
- A maximum continuation cap shall prevent unbounded loops.
- Goal state shall be persisted as session metadata.
- The system shall support pause, resume, and clear operations.

### FR-012 (Optional) — OpenAI Responses API Provider

**If** a model descriptor declares `api: "openai-responses"`, the system
**shall** use the OpenAI Responses API endpoint instead of chat completions,
replaying assistant reasoning as `reasoning` input items when the model
descriptor requires it.

- HTTP 409 responses shall be treated as retryable.
- Cache write tokens from non-standard usage fields shall be folded into cost
  reporting.
- The provider shall be gated so non-Responses backends are unaffected.

### FR-013 (Optional) — Browser-Based Web UI

**If** the user runs `ragent web`, the system **shall** start a loopback-only
HTTP server that serves a browser-based interface with the same run-control
semantics as the TUI (serialized turns, queued prompts, abort, feed output).

- The server shall bind to loopback by default.
- Non-loopback bind shall not be supported without an explicit auth token.
- The web UI shall render assistant text, tool calls, tool results, trigger
  events, and slash commands.
- The web UI shall support model switching, prompt submission, and abort.
- API keys, base64 images, and raw payloads shall never be exposed over web
  events.

### FR-014 (Event-Driven) — `/undo` Remove Last Turn

**When** the user issues `/undo`, the system **shall** remove the most recent
user/assistant turn pair from the active session history.

- The removal shall persist to the session store.
- The system shall confirm the removal to the user.

### FR-015 (Optional) — Session Naming

**If** the user issues `/name <display-name>`, the system **shall** set a
human-readable display name on the active session that appears in session
lists and the TUI status bar.

- The name shall be persisted in the session metadata.
- An empty argument shall clear the name.

### FR-016 (Unwanted) — No Cross-Feature Coupling

The system **shall not** create compile-time or runtime dependencies between
gap features unless explicitly declared. A user who enables only the lifecycle
hooks feature shall not be required to also enable triggers, loops, or LSP.

### FR-017 (Unwanted) — No Regression of Existing Features

The system **shall not** alter the behaviour of existing ragent features
(cron, compaction, permission system, MCP client, code index, memory, teams,
research, skills, prompt optimization) when a gap feature is disabled.

### FR-018 (Ubiquitous) — Configuration Discovery

Each gap feature that requires user configuration shall read its configuration
from `ragent.json` (or `ragent.jsonc`) in the standard `.ragent/` directory,
with fallback to `~/.config/ragent/config.json`, following the existing ragent
configuration discovery pattern.

### FR-019 (Ubiquitous) — Test Coverage

Each gap feature shall include tests located in the `tests/` directory of the
implementing crate, following ragent's test organization conventions. Tests
shall not hit real provider APIs.
