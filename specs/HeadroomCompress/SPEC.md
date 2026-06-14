---
status: draft
audit:
  - { time: 1780871610, from: "none", to: "draft", actor: "system" }
---
# HeadroomCompress — Advanced Context Compression via Headroom Crates

## Overview

ragent's current context compression strategy is a simple truncation algorithm
(`truncate_history_with_atomic_tool_calls`) that drops the oldest messages until
the estimated token count falls below the context window threshold. It uses a
rough `chars / 4` token estimation and preserves atomic tool-call pairs but does
no content-aware compression, no relevance filtering, and no reversible storage.

The [Headroom](https://github.com/chopratejas/headroom) project (Apache 2.0)
provides a mature Rust crate ecosystem (`headroom-core`) implementing six
content-aware compression algorithms (SmartCrusher, CodeCompressor,
LogCompressor, DiffCompressor, SearchCompressor, and a pipeline orchestrator)
plus a Compress-Cache-Retrieve (CCR) layer, BM25 relevance scoring, and
accurate token counting (`tiktoken-rs` + `HfTokenizer` + estimation fallback).
Its crates are designed as **libraries** with clean public APIs — ideal for
direct dependency integration without merging the full Headroom project.

This specification defines how ragent will integrate `headroom-core` as a
crate dependency to replace its naive compaction with content-aware, reversible,
relevance-ranked compression for both the `/compact` slash command and automatic
context-window exhaustion handling.

## Goals

1. **Information loss reduction** — Replace head-truncation with content-aware
   compression that keeps the most relevant parts of every message, not just
   the most recent ones.
2. **Reversibility** — Store original message content in a local CCR store so
   the LLM can retrieve full tool outputs on demand via a `headroom_retrieve`
   tool call.
3. **Accurate token counting** — Replace `chars / 4` estimation with
   model-aware tiktoken counting so compaction triggers at the right threshold.
4. **Content-type-aware compression** — Apply specialised compressors (JSON
   minification, log summarisation, diff hunk selection, code AST compression)
   based on detected content type, rather than treating all text identically.
5. **Minimal dependency surface** — Use `headroom-core` as a Cargo dependency
   only; do not merge Headroom's proxy, Python bindings, or MCP server into
   ragent.

## Requirements

### Ubiquitous

**FR-001** The system shall use `headroom-core`'s `Tokenizer` trait for all
token-counting operations related to context window management, replacing the
existing `chars / 4 + 10` heuristic in `TokenEstimator`.

**FR-002** The system shall apply content-type detection (`headroom_core::transforms::detect`)
to every message part before compression, classifying content as JSON, diff,
log, search results, code, or prose.

**FR-003** The system shall apply the appropriate Headroom compressor to each
message part based on its detected content type:
- `SmartCrusher` for JSON tool outputs
- `DiffCompressor` for diff/patch content
- `LogCompressor` for log output
- `SearchCompressor` for code-search results
- Prose left as-is or lightly truncated when no specialised compressor applies

**FR-004** The system shall store the original (pre-compression) content of every
compressed message part in a local CCR store using `headroom_core::ccr`, keyed
by a BLAKE3 hash, and insert a `<<ccr:HASH>>` marker in the compressed output
so the LLM can request the original via a `headroom_retrieve` tool call.

### Event-Driven

**FR-005** When the estimated token count of the conversation history exceeds
80% of the model's context window, the system shall automatically trigger
context compression using the Headroom compression pipeline before the next
LLM call, without requiring user intervention.

**FR-006** When the user invokes the `/compact` slash command, the system shall
run the full Headroom compression pipeline on the current conversation history
and report the compression ratio (before/after tokens) to the user.

**FR-007** When a tool call contains `<<ccr:HASH>>` markers in its response, the
system shall register a `headroom_retrieve` tool that, when invoked by the LLM,
looks up the original content from the CCR store and returns it verbatim.

### State-Driven

**FR-008** When the CCR store size exceeds a configurable threshold (default
1000 entries, 5-minute TTL per entry), the system shall evict the least-recently-
accessed entries automatically, mirroring `headroom_core::ccr::DEFAULT_CAPACITY`
and `DEFAULT_TTL`.

**FR-009** When the `headroom` config section is absent or `enabled: false`,
the system shall fall back to the existing `truncate_history_with_atomic_tool_calls`
behaviour, preserving full backward compatibility.

### Optional

**FR-010** The system may use `headroom_core::signals::line_importance`
scoring to rank individual lines within large tool outputs, keeping only the
highest-scoring lines when aggressive compression is needed (context window >
95% full).

**FR-011** The system may use `headroom_core::relevance::BM25Scorer` to rank
conversation messages by relevance to the current user query, preferentially
keeping high-relevance messages over low-relevance recent ones during
compression.

**FR-012** The system may expose a `/compress` slash command (distinct from
`/compact`) that allows the user to choose compression parameters:
- `/compress` — default pipeline (all compressors)
- `/compress aggressive` — maximum compression with relevance filtering
- `/compress conservative` — preserve more content, only apply lossless
  compressors (JSON minification, tag protection)

### Event-Driven

**FR-018** When the user invokes `/compress help`, the system shall display a
formatted list of all `/compress` subcommands and their descriptions, including:

| Subcommand | Description |
|---|---|
| `/compress` | Run the default compression pipeline (all enabled compressors) |
| `/compress aggressive` | Maximum compression with relevance filtering and line-importance scoring |
| `/compress conservative` | Only apply lossless compressors (JSON minification, tag protection) |
| `/compress help` | Display this help text |
| `/compress stats` | Show compression statistics for the current session (tokens before/after, CCR entries, content types) |

The output shall also show the current compression configuration status (enabled/disabled, tokenizer backend, CCR backend) and the automatic compression threshold.

### Unwanted

**FR-013** The system shall NOT merge the Headroom project source code into the
ragent repository. All Headroom functionality shall be consumed via the
`headroom-core` crate dependency.

**FR-014** The system shall NOT enable the Headroom HTTP proxy mode. ragent
compresses locally within the agent loop, not by intercepting provider API calls.

**FR-015** The system shall NOT require a running Python runtime. All compression
shall be performed by the Rust `headroom-core` crate natively.

**FR-016** The system shall NOT store CCR data on remote servers. All CCR storage
shall be local SQLite (the `headroom_core::ccr::SqliteCcrStore` backend), scoped
to the session.

**FR-017** The system shall NOT compress system prompts or the most recent user
message. These must always pass through unmodified to preserve instruction
fidelity.

## Configuration

```jsonc
{
  "compression": {
    "enabled": true,
    " // Threshold to trigger automatic compression (fraction of context window)
    "auto_threshold": 0.80,
    " // CCR store configuration
    "ccr": {
      "backend": "sqlite",          // "sqlite" | "memory"
      "capacity": 1000,
      "ttl_secs": 300
    },
    " // Per-content-type compressor toggles
    "compressors": {
      "json": true,                 // SmartCrusher
      "diff": true,                 // DiffCompressor
      "log": true,                  // LogCompressor
      "search": true,               // SearchCompressor
      "code": false,                // CodeCompressor (AST, experimental)
      "prose": false                // Kompress-base (ML, optional)
    },
    " // Relevance filtering (optional)
    "relevance": {
      "enabled": false,
      "scorer": "bm25",            // "bm25" | "hybrid"
      "keep_top_k": 20             // Keep at most K most relevant messages
    },
    " // Tokenizer backend
    "tokenizer": {
      "backend": "auto"            // "auto" | "tiktoken" | "estimate"
    }
  }
}
```

## Architecture

### Data Flow

```
Conversation History
        │
        ▼
┌──────────────────────┐
│  Token Counter       │ ← headroom_core::tokenizer (tiktoken / HfTokenizer / estimate)
│  (accurate count)    │
└─────────┬────────────┘
          │ tokens > threshold?
          ▼ Yes
┌──────────────────────┐
│  Content Detector    │ ← headroom_core::transforms::detect (JSON / diff / log / search / code / prose)
└─────────┬────────────┘
          │
          ▼
┌──────────────────────┐
│  Compression Pipeline│ ← headroom_core::transforms::pipeline (SmartCrusher / DiffCompressor / etc.)
│  + CCR Stash        │ ← headroom_core::ccr (store originals, insert <<ccr:HASH>> markers)
└─────────┬────────────┘
          │ compressed messages + CCR markers
          ▼
┌──────────────────────┐
│  Relevance Filter    │ ← headroom_core::relevance::BM25Scorer (optional)
│  (keep top-K)         │
└─────────┬────────────┘
          │
          ▼
┌──────────────────────┐
│  Reassemble History   │ ← preserve system prompt + last user message unmodified
└─────────┬────────────┘
          │
          ▼
      LLM Provider
```

### CCR Retrieval Flow

```
LLM calls headroom_retrieve(hash)
        │
        ▼
┌──────────────────────┐
│  CCR Store Lookup    │ ← SqliteCcrStore::get(hash)
└─────────┬────────────┘
          │ original content
          ▼
    Returned to LLM as tool result
```

### Crate Dependency

```
ragent-agent
   ├── headroom-core (crate dependency, not merged)
   │      ├── tokenizer (tiktoken-rs + HfTokenizer + estimation)
   │      ├── transforms (SmartCrusher, DiffCompressor, LogCompressor, etc.)
   │      ├── ccr (InMemoryCcrStore, SqliteCcrStore)
   │      ├── signals (line importance, keyword detection)
   │      ├── relevance (BM25Scorer, HybridScorer)
   │      └── compression_policy (CompressionPolicy)
   └── ragent-storage (existing — session persistence)
```

## Non-Functional Requirements

**NFR-001** Adding `headroom-core` as a dependency shall not increase the
debug build time by more than 30 seconds or the release binary size by more
than 8 MB.

**NFR-002** The compression pipeline shall complete in under 500 ms for a
conversation history of up to 200 messages on commodity hardware.

**NFR-003** The compression pipeline shall be fully async-compatible and
shall not block the agent loop's tokio runtime. All Headroom calls shall
run via `spawn_blocking` where they are CPU-intensive.

**NFR-004** All existing tests for `truncate_history_with_atomic_tool_calls`
shall continue to pass unchanged when the `compression.enabled` config is
`false`.

**NFR-005** The CCR SQLite store shall be created in the session's data
directory (alongside the existing SQLite session database) and shall be
cleaned up when the session is deleted.

## Affected Files

### New Files
- `crates/ragent-agent/src/compression/mod.rs` — Module root for compression
- `crates/ragent-agent/src/compression/pipeline.rs` — Headroom pipeline wrapper
- `crates/ragent-agent/src/compression/ccr_store.rs` — CCR store lifecycle
- `crates/ragent-agent/src/compression/tool.rs` — `headroom_retrieve` tool impl
- `crates/ragent-agent/src/compression/config.rs` — Compression config types
- `crates/ragent-agent/tests/test_compression_pipeline.rs` — Integration tests
- `crates/ragent-agent/tests/test_compression_ccr.rs` — CCR store tests
- `crates/ragent-agent/tests/test_headroom_retrieve.rs` — CCR tool tests

### Modified Files
- `Cargo.toml` ��� Add `headroom-core` workspace dependency
- `crates/ragent-agent/Cargo.toml` — Add `headroom-core` dependency
- `crates/ragent-agent/src/session/processor.rs` — Replace compaction call site
- `crates/ragent-agent/src/session/cache.rs` — Replace `TokenEstimator` with
  Headroom tokenizer
- `crates/ragent-agent/src/session/mod.rs` — Re-export new types
- `crates/ragent-agent/src/tool/mod.rs` — Register `headroom_retrieve` tool
- `crates/ragent-config/src/config.rs` — Add `CompressionConfig` struct
- `crates/ragent-tui/src/app/state.rs` — Add `/compress` slash command
- `crates/ragent-tui/src/app.rs` — Handle `/compress` and updated `/compact`
- `crates/ragent-tui/tests/test_compress_help.rs` — Test `/compress help` output

## Open Questions

1. **Kompress-base model** — The ML-based text compressor (`kompress-base`)
   requires downloading a ~30 MB ONNX model file. Should this be opt-in via
   feature flag? (Proposed: yes, behind `compression-ml` feature.)
2. **Magika content detection** — The `magika` crate adds ONNX Runtime
   dependency. For ragent's use case, the simpler keyword-based detection
   in `headroom_core::signals` may suffice. (Proposed: start with keyword
   detection, add magika behind a feature flag later.)
3. **Concurrency model** — The CCR store must be `Send + Sync`. Headroom's
   `InMemoryCcrStore` uses `DashMap` which is already thread-safe. The
   `SqliteCcrStore` uses `rusqlite::Connection` which is not `Send`. We need
   to wrap it in a `Mutex` or use a connection pool. (Proposed: `Mutex` wrap
   for simplicity; the CCR store is not on a hot path.)
4. **Feature-flag granularity** — Should each compressor (JSON, diff, log,
   search) be individually feature-gated? (Proposed: single `compression`
   feature flag that enables all; `compression-ml` for the ML model.)