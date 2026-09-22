---
status: draft
audit:
  - { time: 1789202834, from: "none", to: "draft", actor: "system" }
---
# GCF Protocol Integration for ragent

## Context

GCF (Graph Compact Format) is a token-efficient, lossless structured-data
encoding from the Blackwell Systems GCF project
(https://github.com/blackwell-systems/gcf). Compared with JSON, GCF typically
reduces serialized payloads by 20-48% for nested objects and by far more for
repeated multi-turn payloads (session dedup + delta encoding), while remaining
100% lossless and readable by the LLM after a short primer. A Rust
implementation (`gcf` crate on crates.io, MIT, zero runtime dependencies)
provides both the **generic profile** (`encode` / `decode` of arbitrary JSON
values) and a **graph profile** (local IDs, edges, section headers, summary
trailer) suitable for encoding tool results that are currently JSON-heavy.

JSON-heavy tool outputs currently flow to the LLM verbatim. Several built-in
tools serialise results with `serde_json::to_string(_pretty)` straight into the
observation content (finance suite, `pdf_read`, `office_read`/`libreoffice_*`,
`browser`, `mcp_tool`, `model_info`). Encoding these payloads as GCF at the
single LLM-view choke point reduces token cost for every turn that carries a
tool result, without changing tool implementations or the TUI rendering.

This spec defines how ragent integrates GCF support:

- a persistent config setting in `ragent.json` (section `gcf.enabled`),
  defaulting to **off** when absent;
- a `/gcf` slash command (`on`, `off`, `show`, `help`) in the TUI;
- an encoding hook at the LLM-view choke point
  (`tool_result_content_for_llm` in `crates/ragent-agent/src/session/history.rs`)
  that GCF-encodes eligible JSON tool results when the feature is enabled.

## Goals

- **G1** — GCF encoding is a persistent, config-gated runtime feature: no
  `gcf` section in the config file means the feature is OFF.
- **G2** — The `/gcf on|off` toggle persists the choice to the config file
  from which ragent loaded its config, and applies it to the live session.
- **G3** — When enabled, JSON-dense tool results are encoded with the GCF
  generic profile before the LLM sees them; TUI display, activity log, memory
  extraction, hooks, and compaction still see the raw JSON.
- **G4** — The LLM is told how to read GCF via a short conditional system-prompt
  primer so encoded results remain fully usable.
- **G5** — Encoding is lossless: any LLM-visible GCF block can be decoded back
  to the original JSON without data loss.

## Non-Goals

- **NG-1** — No MCP proxy, external GCF service, or provider-side compression.
- **NG-2** — No encoding of plain-text tool outputs (file reads, greps, edits):
  GCF is applied to eligible JSON results only.
- **NG-3** — No graph-profile encoding of codeindex outputs in v1 (the generic
  profile is used for all encodable payloads; graph-profile support is deferred).
- **NG-4** — Request-side only exclusion: the feature transforms the
  *tool-result to LLM* direction only; the LLM's request payloads (system
  prompt, user messages) are never GCF-encoded.
- **NG-5** — No provider/model allowlist: encoding applies whenever enabled,
  regardless of connected provider.

## Dependencies

- `gcf` crate (gcf-rust, crates.io, MIT, zero runtime dependencies). If the
  published crate does not satisfy integration needs, vendor the minimal
  generic-profile encoder/decoder under `vendor/gcf/` following the existing
  `vendor/html2text` / `vendor/pdf-extract` pattern.

---

## Requirements

Requirements use EARS notation. **ST** = system state (ubiquitous),
**WHEN** = event-driven, **WHILE** = state-driven, **IF/THEN** = optional
feature behaviour, **SHALL NOT** = unwanted behaviour. Where a requirement
mixes templates the dominant template is listed first.

### FR-001 — GCF config section with default-off (ubiquitous + state-driven)

**[Ubiquitous]** The `Config` struct (`crates/ragent-config`) **shall** expose
`pub gcf: GcfConfig`, where `GcfConfig` contains `pub enabled: bool` with
`Default` = `false`, deserialised with `#[serde(default)]`.

**[State-driven]** WHILE the config file contains no `gcf` section, the
effective GCF state **shall** be `disabled` and no tool-result encoding
**shall** occur.

**[State-driven]** WHILE `gcf.enabled` is `false`, the `Config` serialisation
**shall** omit the `gcf` key (via `skip_serializing_if`), so a
never-toggled config file remains byte-identical in intent to pre-feature
configs.

### FR-002 — GCF state is persisted in the config file (event-driven + state-driven)

**[Event-driven]** WHEN `/gcf on` or `/gcf off` completes successfully, the
system **shall** write `gcf.enabled` (`true` / `false` respectively) to the
config file ragent was loaded from (project `.ragent/ragent.json` when
applicable, else the global config), reusing `Config::save_to_source()`,
and **shall** invalidate the in-process config cache so the next load
observes the new value.

**[State-driven]** WHILE the persisted `gcf.enabled` value has not been
changed by any later command, a ragent restart **shall** restore the same
effective GCF state as before the restart.

**[State-driven]** WHEN `Config::save_to_source()` fails, the in-memory
toggle state **shall** still be applied to the live session, and the UI
**shall** indicate the value is unsaved (status text suffix, matching the
existing `/codeindex` behaviour).

### FR-003 — `/gcf` slash command surface (ubiquitous + optional)

**[Ubiquitous]** The TUI **shall** register a `gcf` entry in `SLASH_COMMANDS`
(`crates/ragent-tui/src/app/state.rs`) with description
`Toggle GCF encoding of tool results: /gcf on|off|show|help`, and
`get_command_suggestions` **shall** return `on`, `off`, `show`, `help` for the
`gcf` trigger so autocomplete offers every subcommand.

**[Optional]** IF the user submits `/gcf` with subcommand `on`, THEN the TUI
**shall** set the effective GCF state to enabled, persist FR-002, and append
a confirmation notice bubble.

**[Optional]** IF the user submits `/gcf` with subcommand `off`, THEN the TUI
**shall** set the effective GCF state to disabled, persist FR-002, and append
a confirmation notice bubble.

**[Optional]** IF the user submits `/gcf show`, THEN the TUI **shall** display
the current effective GCF state (on/off) and the source of that state
(persisted config, in-session change, or default-off because no config
section exists).

**[Optional]** IF the user submits `/gcf help` (also accepted: `--help`,
`-h`, bare `/gcf` with no arguments), THEN the TUI **shall** display the
purpose of GCF encoding and the list of `/gcf` subcommands, without changing
any state.

**[Unwanted]** IF the user submits `/gcf` with an unrecognised subcommand,
THEN the TUI **shall** reject it with a usage notice listing the valid
subcommands (`on`, `off`, `show`, `help`) and **shall not** change the GCF
state or the config file.

### FR-004 — GCF encoding of eligible tool results (event-driven + optional)

**[Event-driven]** WHEN a tool execution completes with a result observation
and GCF is enabled, the system **shall** pass the observation through the
encoding hook at `tool_result_content_for_llm`
(`crates/ragent-agent/src/session/history.rs`), which:

1. applies to both the live dispatch path and the session replay path, so
   persisted history rendered for the LLM is encoded with the same rules;
2. encodes only payloads that parse as JSON objects or arrays and exceed a
   minimum size threshold (default 200 characters of raw JSON) — smaller or
   non-JSON observations pass through unmodified;
3. wraps the encoded payload in a labelled GCF block so the LLM can
   distinguish it from plain text.

**[Optional]** IF the raw JSON is longer than the GCF encoding of itself
(plus margin), THEN the hook **shall** emit the raw JSON instead (no
negative-savings encodes).

**[Optional]** IF encoding fails for any reason, THEN the hook **shall** fall
back to the raw observation unchanged (encode must never break a tool
result).

### FR-005 — Raw visibility for non-LLM consumers (state-driven)

**[State-driven]** WHILE a tool result is GCF-encoded for the LLM view, the
following consumers **shall** continue to receive the raw, unencoded
observation:

- TUI message window / activity log rendering;
- memory extraction engine (`engine.on_tool_result(...)`);
- PostToolUse hook chain;
- compaction summariser inputs.

### FR-006 — System-prompt GCF primer (state-driven + optional)

**[State-driven]** WHILE GCF is enabled, the assembled system prompt **shall**
include a short `## GCF Encoding Primer` section (added alongside the
existing conditional sections in `crates/ragent-agent/src/agent/mod.rs`)
explaining: the GCF block marker, the generic profile grammar (headers, pipe
rows, scalar grammar), that GCF blocks are lossless encodings of JSON, and
that the LLM should read them as structured data.

**[State-driven]** WHILE GCF is disabled (including default-off), the
`## GCF Encoding Primer` section **shall not** appear in any system prompt.

### FR-007 — Lossless round-trip (ubiquitous)

**[Ubiquitous]** The encoding hook **shall** produce GCF blocks that decode,
via the `gcf` crate's decoder, to JSON equal to the original observation
payload (lossless guarantee), verified by implementation tests accompanying
the encoder tasks.

### FR-008 — Malformed config values rejected (unwanted)

**[Unwanted]** IF the config file contains a `gcf` section with a malformed
value (e.g. `"gcf": {"enabled": "yes"}`), THEN config load **shall** fail
with the existing actionable JSON parse diagnostics (file, line, column,
caret) and **shall not** silently coerce the value to a default.

**[Unwanted]** IF the config file contains `gcf` set to a non-object shape
(e.g. `"gcf": true`), THEN config load **shall** reject the shape with the
same actionable diagnostics rather than guessing intent.

### FR-009 — Config-file portability (state-driven)

**[State-driven]** WHILE a config file with `gcf.enabled: true` is used by a
ragent build, the GCF state and `/gcf` toggle behaviour **shall** operate
identically regardless of whether the config file came from the project
directory (`.ragent/ragent.json`) or the global user config, and `/gcf on`
**shall** write the setting back to the same source the config was loaded
from.

---

## Acceptance Criteria

- AC-1: With no `gcf` section in `ragent.json`, `/gcf show` reports OFF and
  tool results are never encoded (FR-001, FR-002).
- AC-2: `/gcf on` persists `gcf.enabled: true` to the loaded config source and
  survives restart; `/gcf off` persists `false` (FR-002, FR-003, FR-009).
- AC-3: With GCF on, a JSON-dense tool result (e.g. `stock_quote`) reaches the
  LLM as a GCF block; with GCF off the same result reaches the LLM as raw
  JSON (FR-004).
- AC-4: The GCF block decodes back to the exact original JSON (FR-007).
- AC-5: TUI, activity log, memory extraction, hooks, and compaction see raw
  JSON in both states (FR-005).
- AC-6: With GCF on, the system prompt contains the GCF primer; with GCF off
  it does not (FR-006).
- AC-7: Unknown `/gcf` subcommands and malformed `gcf` config values are
  rejected without state changes (FR-003, FR-008).
- AC-8: Bare `/gcf`, `/gcf help`, `/gcf --help`, `/gcf -h` all display help
  and never change state (FR-003).

---

## Verification Approach

- Config default-off: serde round-trip + fresh-load tests in
  `crates/ragent-config/tests/test_gcf_config.rs` (implementation-level).
- Toggle persistence: TUI dispatch tests in `crates/ragent-tui/tests/`
  mirroring `test_codeindex_toggle_persistence.rs`, plus manual TESTPLAN
  steps below.
- Encoder hook: unit tests for JSON-threshold, size-margin, fallback, and
  round-trip decode in `crates/ragent-agent/tests/`.
- Primer presence: system-prompt assembly tests in
  `crates/ragent-agent/tests/test_builtin_agents.rs` pattern.
- Manual end-to-end verification via TESTPLAN.md.

---

## Open Questions

- OQ-1: Confirm the exact `gcf` crate API (encode/decode entry points and
  whether a labelled-block wrapper is provided) at implementation start; if
  the crate's published API differs from the README, adapt FR-004's block
  wrapper to the crate's canonical output form.
- OQ-2: Whether `/gcf` should also be available in non-TUI runs (CLI `ragent
  run`); v1 scope is TUI-only, consistent with other config toggles.