# Implementation Plan: Prompt Preprocessor — Optimization Frameworks

## Overview

This plan implements the `promptopt` specification by wiring the existing
`ragent-prompt_opt` crate into ragent as a configurable prompt-preprocessing
stage.  The work covers configuration, the core preprocessor, TUI slash-command
integration, HTTP API wiring, and documentation.

## Tasks

| ID | Title | Requirement | Effort | Priority | Dependencies |
|----|-------|-------------|--------|----------|--------------|
| T-001 | Add `prompt_opt` configuration types to `ragent-config` | FR-010 | S | High | None |
| T-002 | Expose template-only optimizer in `ragent-prompt_opt` | FR-004 | M | High | T-001 |
| T-003 | Implement `PromptPreprocessor` pipeline stage | FR-001, FR-002, FR-006, FR-009 | M | High | T-001, T-002 |
| T-004 | Integrate preprocessor into agent session input path | FR-001, FR-007, FR-015 | M | Critical | T-003 |
| T-005 | Extend `/opt` slash command for single and chained methods | FR-003, FR-008, FR-012 | M | High | T-003 |
| T-006 | Add `POST /opt` and `GET /opt/help` HTTP endpoints | FR-011, FR-013 | S | Medium | T-003 |
| T-007 | Track optimization cost and latency separately | FR-017 | S | Medium | T-004 |
| T-008 | Surface fallback warnings in TUI and HTTP responses | FR-015 | S | Medium | T-004, T-006 |
| T-009 | Write unit/integration tests for preprocessor and endpoints | NFR-002 | M | High | T-003, T-006 |
| T-010 | Write user documentation (`docs/promptopt.md` + QUICKSTART update) | NFR-004 | S | Low | T-005, T-006 |

## Task details

### T-001 — Add `prompt_opt` configuration types to `ragent-config`

Define a `PromptOptConfig` struct in `ragent-config` with fields:

- `enabled: bool`
- `default_method: Option<String>`
- `mode: PromptOptMode` (`Template` or `Llm`)
- `methods: Vec<String>` (ordered list for chaining)

Deserialize with `serde`, default to `enabled = false`, and add it to the root
configuration so that `ragent.json` accepts a `prompt_opt` block.

### T-002 — Expose template-only optimizer in `ragent-prompt_opt`

Extend `ragent-prompt_opt` with a function such as
`optimize_template(method, input) -> String` that fills the framework-specific
skeleton using only the raw input, without calling an LLM.  Keep the existing
LLM-driven `optimize` function intact.

### T-003 — Implement `PromptPreprocessor` pipeline stage

Create a small crate-internal (or new crate) `PromptPreprocessor` that:

- Accepts a list of framework names and a mode.
- Resolves each name through `OptMethod::from_str`.
- For `Template` mode, calls `optimize_template` for each method in order.
- For `Llm` mode, calls `optimize` with a `Completer` adapter over the active
  provider.
- Returns the final prompt or an error that the caller can use for fallback.

### T-004 — Integrate preprocessor into agent session input path

In `ragent-agent` (or the session/orchestration layer), insert the
`PromptPreprocessor` immediately after the user input is normalized and before it
is handed to the agent loop.  Honor the default method and enabled flag from
`PromptOptConfig`.  On failure, return the original input and emit a warning
notice.

### T-005 — Extend `/opt` slash command for single and chained methods

Update the TUI slash-command handler for `/opt` so it supports:

- `/opt help`
- `/opt <method> <prompt...>`
- `/opt <method1,method2> <prompt...>`

When used in the input box, apply the preprocessor to the remainder of the
message and, if `FR-014` is enabled, show a preview/confirmation dialog.

### T-006 — Add `POST /opt` and `GET /opt/help` HTTP endpoints

In `ragent-server`, expose:

- `POST /opt` with JSON body `{ "method": "...", "prompt": "..." }`, returning
  `{ "method": "...", "optimized": "..." }`.
- `GET /opt/help` returning the markdown help table from
  `OptMethod::help_table()`.

### T-007 — Track optimization cost and latency separately

When the LLM-driven path is used, record the completion's token usage and
duration under a new `optimization_cost` field in the session cost tracker.
This must not be merged with the main agent turn cost.

### T-008 — Surface fallback warnings in TUI and HTTP responses

When the preprocessor fails:

- In the TUI, emit an `AgentNotice` warning and continue with the raw prompt.
- In the HTTP API, return HTTP 200 with the raw prompt in `optimized` and a
  `warning` field explaining the failure.

### T-009 — Write unit/integration tests for preprocessor and endpoints

Add tests covering:

- Unknown method resolution.
- Template-only transformation for at least three frameworks.
- Chained multi-framework transformation.
- LLM-driven path using a mock `Completer`.
- `POST /opt` and `GET /opt/help` round trips.
- Fallback behavior on failure.

### T-010 — Write user documentation

Create `docs/promptopt.md` describing configuration, slash-command usage,
frameworks, and examples.  Update `QUICKSTART.md` with a one-line `/opt`
example.

## Definition of done

- `cargo test -p ragent-prompt_opt` and new integration tests pass.
- The `/opt` slash command works in the TUI for single and chained methods.
- `POST /opt` returns optimized prompts and `GET /opt/help` returns the help
  table.
- Unknown methods fall back gracefully with a warning.
- `docs/promptopt.md` and `QUICKSTART.md` are updated.
- No compiler warnings are introduced.
