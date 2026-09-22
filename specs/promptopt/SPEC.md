---
status: draft
---
# Specification: Prompt Preprocessor — Optimization Frameworks

## Overview

This specification defines a mechanism that lets ragent preprocess a user prompt
through one or more prompt-optimization methodologies before the prompt is sent
to the active LLM.  The system re-uses and extends the existing
`ragent-prompt_opt` crate, which currently provides 12 template-based or
LLM-driven optimization frameworks such as CO-STAR, CRISPE, Chain-of-Thought,
DRAW, RISE, O1-STYLE, Meta Prompting, VARI, Q*, and platform adapters for OpenAI,
Claude, and Microsoft.

The goal is to make prompt optimization a first-class, configurable stage in the
agent pipeline: users can request optimization via slash commands, the TUI, the
HTTP API, or agent configuration; the framework transforms the prompt according
to the chosen method; and the optimized result is used transparently for the
next agent turn.

## Background

The `ragent-prompt_opt` crate already exposes:

- `OptMethod` — an enum with 12 optimization frameworks.
- `Completer` — an async trait abstracting a single LLM completion.
- `optimize(method, input, completer)` — an async function that sends the
  framework-specific meta-prompt plus the raw input to a completer and returns
  the optimized prompt.
- `system_prompt(method)` and `OptMethod::from_str`/`name`/`description` helpers.

These building blocks are currently reachable through the `/opt` slash command
and the `POST /opt` HTTP endpoint (template-only or LLM-driven depending on the
build).  This specification turns that capability into a general preprocessing
hook so that any incoming prompt can be routed through an optimizer before it
reaches the main agent loop.

## Requirements

### FR-001 — Optimization stage availability (ubiquitous)

The agent pipeline shall provide an optional prompt-preprocessing stage that
runs after the raw user input is received and before the input is passed to the
main agent loop.

### FR-002 — Framework registry (ubiquitous)

The preprocessor shall support at least the 12 frameworks already defined by
`ragent-prompt_opt`: CO-STAR, CRISPE, Chain-of-Thought, DRAW, RISE, O1-STYLE,
Meta Prompting, VARI, Q*, OpenAI, Claude, and Microsoft.

### FR-003 — Framework selection by name (ubiquitous)

The user shall be able to select a framework by its canonical short name
(`co_star`, `crispe`, `cot`, `draw`, `rise`, `o1_style`, `meta`,
`variational`, `q_star`, `openai`, `claude`, `microsoft`) or by any alias
accepted by `OptMethod::from_str`.

### FR-004 — Template-only path (event-driven)

WHEN the selected framework is configured for template-only optimization OR no
LLM completer is available, the preprocessor SHALL apply the corresponding
static transformation directly without an external API call.

### FR-005 — LLM-driven path (event-driven)

WHEN the selected framework is configured for LLM-driven optimization and a
valid completer is available, the preprocessor SHALL send the framework's
system prompt and the trimmed user input to the completer and return the
optimized prompt.

### FR-006 — Multiple sequential frameworks (state-driven)

WHILE the user has enabled multiple optimization frameworks, the preprocessor
shall chain them in order, passing the output of each framework as the input to
the next, and return the final prompt.

### FR-007 — Default framework configuration (state-driven)

IF the user has configured a default optimization framework in
`ragent.json`, the preprocessor shall use that framework automatically for
every incoming prompt unless the user explicitly overrides it.

### FR-008 — Per-message override (event-driven)

WHEN the user prefixes a message with `/opt <framework>` or uses the
`/opt` slash command, the preprocessor SHALL use the specified framework for
that message regardless of the default configuration.

### FR-009 — Preprocessor enable/disable (state-driven)

IF prompt optimization is disabled in configuration or by an explicit user
command, the preprocessor shall pass the raw input through unchanged.

### FR-010 — Configuration schema extension (ubiquitous)

The `ragent.json` configuration schema shall include a `prompt_opt` block that
specifies `enabled`, `default_method`, `mode` (`template` or `llm`), and an
optional ordered list of `methods`.

### FR-011 — HTTP API exposure (ubiquitous)

The HTTP server shall expose `POST /opt` accepting `{ "method": "...",
"prompt": "..." }` and returning `{ "method": "...", "optimized": "..." }`.

### FR-012 — TUI integration (ubiquitous)

The TUI shall support the `/opt` slash command with the same method names and
aliases, and shall display the optimized prompt before submitting it to the
agent when the user confirms.

### FR-013 — Help discovery (optional)

The TUI and HTTP API MAY expose a `/opt help` (or `GET /opt/help`) endpoint
that returns a markdown table of available frameworks and their descriptions.

### FR-014 — Preview and confirmation (optional)

The TUI MAY show a preview of the optimized prompt and ask the user to confirm
before using it for the active turn.

### FR-015 — Optimization errors do not block the agent (unwanted)

IF optimization fails (unknown framework, LLM error, or timeout), the system
shall fall back to the original raw prompt and surface a warning instead of
aborting the user request.

### FR-016 — No data leakage of system prompts (unwanted)

The optimization meta-prompts shall be treated as internal system prompts;
they shall not be exposed to the user or to external logging at levels lower
than `trace`.

### FR-017 — Completion cost attribution (unwanted)

When LLM-driven optimization is used, the token cost and latency of the
optimization call shall be tracked separately from the main agent LLM call.

## Non-functional requirements

### NFR-001 — Latency

Template-only optimization shall complete in under 10 ms for prompts up to
4,000 tokens on typical developer hardware.

### NFR-002 — Testability

Each framework path shall be unit-testable with a mock `Completer`, and the
preprocessor shall be testable with a configurable input/output harness.

### NFR-003 — Backward compatibility

Existing `/opt` callers and `POST /opt` consumers shall continue to work
without changes after the preprocessing stage is introduced.

### NFR-004 — Documentation

The feature shall be documented in `docs/promptopt.md` and the root
`QUICKSTART.md` shall receive a new `/opt` example.

## Scope

### In scope

- Preprocessing hook in the agent pipeline.
- Configuration schema for `prompt_opt`.
- `/opt` slash command in the TUI.
- `POST /opt` HTTP endpoint.
- Template-only and LLM-driven modes.
- Single-framework and chained multi-framework optimization.
- Help table and error fallback behavior.

### Out of scope

- Automatic selection of the best framework based on prompt content.
- Custom user-defined frameworks beyond the 12 built-in methods.
- Embedding-based or neural prompt optimization.
- Optimization of multi-modal (image/audio) prompts.
