---
status: draft
audit:
  - { time: 1784327573, from: "none", to: "draft", actor: "system" }
---
# Replace Headroom-Based Context-Window Compaction with OpenCode-Derived Summarisation

## Context

ragent currently manages context-window pressure through a **content-aware
compression pipeline** built on the external `headroom-core` crate. The pipeline
applies per-content-type compressors (JSON, diff, log, search, code, prose),
stashes originals behind `<<ccr:HASH>>` markers, and relies on an in-memory
CCR store and a `headroom_retrieve` tool bridge. This scheme is complex, adds
a non-trivial git dependency, duplicates several concepts already present in
OpenCode's proven compaction implementation, and has drifted from the simpler,
more reliable pattern used by `~/Projects/opencode/packages/core/src/session/compaction.ts`.

OpenCode's compaction model is straightforward: when the estimated token
load exceeds the model context window minus a configured buffer, the runner
selects a prefix of the conversation (keeping recent turns verbatim), sends
a summarisation prompt to the same LLM, and inserts the resulting summary as
a special `compaction` message. The next runner turn loads history from the
compaction message forward, while the system-context baseline and any later
system updates remain visible.

This specification defines the plan to **remove the Headroom-based compression
scheme** and **replace it with a Rust port of OpenCode's summarisation-based
compaction**, integrated into ragent's existing session storage and event model.

## Requirements

### Ubiquitous requirements

FR-001: The context-window compaction subsystem **shall** always provide a
fallback summarisation path that does not depend on an external compression
crate.

FR-002: Every turn of the agent loop **shall** estimate request token usage
using a fast local estimator and, when the provider reports actual input
tokens, **shall** prefer the provider-reported value for compaction decisions.

### Event-driven requirements

FR-003: When the estimated request tokens exceed `context_window - max(output_tokens,
compaction_buffer)`, the runner **shall** emit a `CompactionStarted` event and
invoke the summarisation pipeline before sending the user prompt to the LLM.

FR-004: When a provider response fails with a context-overflow error before any
assistant tokens have been produced, the runner **shall** trigger emergency
summarisation, retry the turn once, and emit a `CompactionFinished` event with
the outcome.

FR-005: When summarisation completes successfully, the system **shall** insert
a synthetic `compaction` message containing the summary and the verbatim recent
turns, then resume the session from that compaction point.

### State-driven requirements

FR-006: While a session has no compaction messages, the runner **shall** load
all messages since the system-context baseline for the LLM request.

FR-007: While the latest compaction message is newer than the system-context
baseline, the runner **shall** include the compaction summary plus all messages
from the compaction sequence onward, and **shall** include any system-context
updates after the baseline but not already covered by the summary.

FR-008: While the `auto` field in the compaction configuration is `false`, the
runner **shall** disable automatic pre-send summarisation and rely solely on
emergency overflow summarisation.

### Optional requirements

FR-009: The runner **may** expose a `/compact` (or `/compress`) slash command
that performs a one-shot summarisation of the current session and replaces the
in-memory message list with the compaction message.

FR-010: The summarisation prompt **may** include a previous compaction summary
when one exists, asking the LLM to update rather than recreate it.

FR-011: The configuration **may** allow users to override the default buffer,
keep-tokens and summary output-token values.

### Unwanted requirements

FR-012: The system **shall not** retain the `headroom-core` git dependency once
the new summarisation-based compaction is active.

FR-013: The system **shall not** use `<<ccr:HASH>>` markers, CCR stashing, or a
`headroom_retrieve` tool after the migration.

FR-014: The system **shall not** silently drop tool outputs, user images, or
other structured message parts during summarisation; parts that cannot be
summarised **shall** be represented textually or omitted explicitly from the
summary context.

## Glossary

- **Compaction** — the process of replacing a long message history with a
  shorter structured summary plus a small number of recent verbatim turns.
- **Headroom** — the current `headroom-core`-based compression pipeline.
- **Context epoch / baseline** — the persisted system-context snapshot that
  anchors a session; messages before this baseline are already represented by
  the system context unless superseded by a newer compaction.
- **Recent turns** — the last N user/assistant/tool-turn pairs kept verbatim
  after compaction so the LLM can continue active work.
- **Provider-reported tokens** — the `input_tokens` value returned by the LLM
  provider in the previous turn's usage payload.

## References

- `~/Projects/opencode/packages/core/src/session/compaction.ts`
- `~/Projects/opencode/packages/core/src/session/runner/llm.ts`
- `~/Projects/opencode/packages/core/src/config/compaction.ts`
- `crates/ragent-agent/src/compression/pipeline.rs` (current Headroom wrapper)
- `crates/ragent-config/src/compression.rs` (current config schema)
- `crates/ragent-types/src/event/mod.rs` (event definitions)
- `crates/ragent-storage/src/storage.rs` (message persistence)
