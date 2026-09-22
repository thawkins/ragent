# Implementation Plan — Pie Feature Gap

## Overview

Each task implements one gap feature as a standalone, independently
selectable module. Tasks are ordered by architectural dependency: foundational
shared types (trigger envelope, hook types) land first, then features that
build on them, then fully independent features.

**FR-009 (LSP Integration)** and **FR-010 (Compiled Extension Registry)** are
marked "NOT Required" in the spec. No implementation tasks are defined for
them. The original task IDs T-013, T-014, and T-015 are retired and skipped to
preserve the numbering of the remaining tasks.

**Effort**: S = small (≤1 day), M = medium (1–3 days), L = large (3–7 days)
**Priority**: Critical, High, Medium, Low

## Tasks

| ID    | Title                                                                                    | Requirement                    | Effort | Priority | Status    | Dependencies |
| ----- | ---------------------------------------------------------------------------------------- | ------------------------------ | ------ | -------- | --------- | ------------ |
| T-001 | Trigger envelope types + trigger runtime (dedup, cycle suppression)                      | FR-002, FR-003                 | L      | High     | completed | pending      |
| T-002 | Dynamic trigger rules — natural-language polling + sub-agent action                     | FR-002                         | L      | High     | completed | pending      |
| T-003 | MCP notification push-event adapter (inject_summary / inject_and_run)                    | FR-003                         | L      | High     | completed | pending      |
| T-004 | `/triggers` slash commands (list, enable, disable, remove, status)                     | FR-002, FR-003                 | M      | High     | completed | pending      |
| T-005 | Stateful loop cron mode +`<loop-state>`/`<inbox>` tag protocol                       | FR-004                         | L      | Medium   | completed | pending      |
| T-006 | Global triage inbox JSONL store +`/inbox` slash commands                               | FR-004                         | M      | Medium   | completed | pending      |
| T-007 | Lifecycle hooks system —`hooks.json` loader + command/webhook runner                  | FR-005                         | L      | Medium   | completed | pending      |
| T-008 | Hook event listener wiring (agent_start/end, turn_start/end, tool_start/end, compaction) | FR-005                         | M      | Medium   | completed | pending      |
| T-009 | Portable session archive export (manifest + transcript + sidecars + SHA-256)             | FR-006                         | L      | Low      | completed | pending      |
| T-010 | Portable session archive import (verify checksums + optional automation activation)      | FR-006                         | L      | Low      | completed | pending      |
| T-011 | `/bug-report` slash command — diagnostic dump + redaction (output to `log/` folder) | FR-007                         | M      | Low      | completed | pending      |
| T-012 | Reusable prompt templates — loader +`/template` slash command                         | FR-008                         | M      | Low      | completed | pending      |
| T-016 | Goal-based autonomous stop hook — evaluator model call +`/goal` commands              | FR-011                         | L      | Medium   | completed | pending      |
| T-017 | OpenAI Responses API provider — reasoning replay + 409 retry + cache-write usage        | FR-012                         | L      | Low      | completed | pending      |
| T-018 | Browser-based web UI — loopback HTTP server + SSE + browser-served HTML/JS              | FR-013                         | L      | Low      | completed | pending      |
| T-019 | `/undo` slash command — remove last user/assistant turn pair                          | FR-014                         | S      | Medium   | completed | pending      |
| T-020 | Session naming —`/name` slash command + persisted display name                        | FR-015                         | S      | Low      | completed | pending      |
| T-021 | Feature-gate configuration blocks in`ragent.json` for all gap features                 | FR-016, FR-018                 | M      | High     | completed | pending      |
| T-022 | Integration tests — verify standalone compilation with each feature disabled            | FR-001, FR-016, FR-017, FR-019 | M      | High     | completed | pending      |

## Requirement Coverage

| Requirement | Covered by tasks           |
| ----------- | -------------------------- |
| FR-001      | T-022                      |
| FR-002      | T-001, T-002, T-004, T-021 |
| FR-003      | T-001, T-003, T-004, T-021 |
| FR-004      | T-005, T-006, T-021        |
| FR-005      | T-007, T-008, T-021        |
| FR-006      | T-009, T-010               |
| FR-007      | T-011                      |
| FR-008      | T-012, T-021               |
| FR-009      | — (NOT required)          |
| FR-010      | — (NOT required)          |
| FR-011      | T-016, T-021               |
| FR-012      | T-017                      |
| FR-013      | T-018                      |
| FR-014      | T-019                      |
| FR-015      | T-020                      |
| FR-016      | T-021, T-022               |
| FR-017      | T-022                      |
| FR-018      | T-021                      |
| FR-019      | T-022                      |

## Architecture Notes

### Crate Placement

| Feature                    | Target crate(s)                                                                                  |
| -------------------------- | ------------------------------------------------------------------------------------------------ |
| Trigger envelope + runtime | `ragent-types` (types), `ragent-agent` (runtime)                                             |
| Dynamic triggers           | `ragent-agent` (logic), `ragent-tui` (slash commands)                                        |
| MCP notification hooks     | `ragent-agent` (hook abstraction), `ragent-tools-extended` (MCP adapter)                     |
| Stateful loops + inbox     | `ragent-agent` (loop logic), `ragent-storage` (inbox JSONL), `ragent-tui` (slash commands) |
| Lifecycle hooks            | `ragent-agent` (hook runner), `ragent-config` (config types), `ragent-tui` (wiring)        |
| Session archive            | `ragent-agent` (archive logic), `ragent-storage` (sidecar read/write)                        |
| Bug report                 | `ragent-tui` (slash command), `ragent-agent` (diagnostic data)                               |
| Prompt templates           | `ragent-agent` (loader), `ragent-tui` (slash command)                                        |
| Goal stop hook             | `ragent-agent` (evaluator + hook), `ragent-tui` (slash commands)                             |
| OpenAI Responses provider  | `ragent-llm` (new provider)                                                                    |
| Web UI                     | `ragent-tui` (web module) or new `ragent-web` crate                                          |
| `/undo`                  | `ragent-agent` (session history), `ragent-tui` (slash command)                               |
| Session naming             | `ragent-storage` (metadata), `ragent-tui` (slash command)                                    |

### Feature-Gate Strategy

Each gap feature shall be gated behind a `serde(default)` boolean in `Config`
so it can be independently enabled/disabled in `ragent.json`. Features that
require background tasks (triggers, hooks) shall no-op cleanly when disabled.
The existing `yolo` toggle pattern shall be followed.

### Dependency Graph

```
T-001 (trigger types + runtime)
├── T-002 (dynamic triggers)
├── T-003 (MCP notification hooks)
│   └── T-004 (/triggers commands)
└── T-005 (stateful loops)
    └── T-006 (inbox)

T-007 (lifecycle hooks loader) → T-008 (event wiring)

Standalone (no deps):
  T-009, T-010 (session archive)
  T-011 (bug report)
  T-012 (prompt templates)
  T-016 (goal stop hook)
  T-017 (OpenAI Responses provider)
  T-018 (web UI)
  T-019 (/undo)
  T-020 (session naming)

T-021 (config gates) — depends on all feature tasks
T-022 (integration tests) — depends on all
```
