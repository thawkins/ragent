# Headroom Usage Audit — Spec `compact`

This document maps every `headroom-core` dependency, concept, and call site in
the ragent workspace. It is the reference for removing the Headroom-based
context-window compression scheme (FR-012, FR-013) and replacing it with the
OpenCode-derived summarisation compaction described in
[`SPEC.md`](SPEC.md) / [`PLAN.md`](PLAN.md).

## 1. Dependency

| File | Symbol / Line | What to do |
|------|---------------|------------|
| `crates/ragent-agent/Cargo.toml` | `[dependencies.headroom-core]` git dep, tag `v0.10.17` | Remove entirely in T-011. |

## 2. Core `ragent-agent::compression` module tree (delete in T-011)

| File | Key symbols | Headroom API / concept used | Removal notes |
|------|-------------|----------------------------|---------------|
| `crates/ragent-agent/src/compression/mod.rs` | `pub use headroom_core;`, re-exports `compress_history`, `count_tokens`, `CompressionMode`, etc. | `headroom_core` crate | Delete file. Move any retained token estimator helper into the new `compaction` module. |
| `crates/ragent-agent/src/compression/pipeline.rs` | `count_tokens`, `count_tokens_text`, `should_compress`, `compress_history`, `compress_chat_messages`, `compress_history_with_mode`, `compress_help`, `ContentType`, `CompressionMode`, `CompressionResult`, `CompressionStats`, `ChatCompressionResult` | `headroom_core::tokenizer::Tokenizer`, `headroom_core::tokenizer::EstimatingCounter`, `headroom_core::transforms::DiffCompressor` | Delete file. The replacement implementation lives in a new `ragent-agent::session::compaction` module. |
| `crates/ragent-agent/src/compression/ccr_store.rs` | `CcrStoreHandle`, `SharedCcrStore`, `create_shared_ccr_store`, `create_test_ccr_store`, `compute_ccr_key`, `ccr_marker`, `parse_ccr_markers`, `<<ccr:HASH>>` | BLAKE3 hashing for CCR markers | Delete file. No CCR store is needed for summarisation compaction. |
| `crates/ragent-agent/src/compression/relevance.rs` | `Bm25Scorer`, `rank_messages` | Custom BM25 relevance scorer | Delete file. OpenCode compaction keeps recent turns verbatim and summarises the rest; no BM25 ranking. |

## 3. Agent session call sites

| File | Symbol / Lines | Current behaviour | Migration action |
|------|----------------|-------------------|------------------|
| `crates/ragent-agent/src/session/history.rs:357-399` | `should_compress_with_reported` | Uses `crate::compression::pipeline::should_compress_chat_messages`; prefers provider-reported `input_tokens`. | Replace with `compaction::should_compact` that uses OpenCode’s `context - max(output, buffer)` trigger. |
| `crates/ragent-agent/src/session/history.rs:401-450` | `emergency_compress_chat_messages` | Publishes `Event::CompressionStarted/Finished`, calls `compress_chat_messages`, resets `last_reported_input_tokens`. | Replace with `compaction::compact_after_overflow`; keep event publishing. |
| `crates/ragent-agent/src/session/loop_steps.rs:582-644` | `build_turn_chat_messages` auto-compress path | Calls `compress_history` when `compression.enabled` and `should_compress`. | Replace with `compaction::compact_if_needed`; rebuild history from compaction message. |
| `crates/ragent-agent/src/session/loop_steps.rs:872-885, 1073-1085, 1313-1343` | Emergency overflow helpers | `emergency_compress_on_overflow` | Route through new `compact_after_overflow`. |
| `crates/ragent-agent/src/session/processor.rs:641-693` | Per-iteration compression | Calls `compress_chat_messages` and publishes `CompressionStarted/Finished`. | Replace with `compaction::compact_if_needed`; update `compressed_this_turn` flag. |
| `crates/ragent-agent/src/session/cache.rs` | Context window pre-compression references | Mentions compression in docs only. | Update doc comments to refer to compaction. |
| `crates/ragent-agent/src/session/mod.rs` | Re-exports `compression::{SharedCcrStore, compress_history, CompressionStats, ...}` | Public API re-exports | Remove re-exports; replace with new `compaction` module public API. |

## 4. Configuration

| File | Symbol / Lines | Current schema | Migration action |
|------|----------------|----------------|------------------|
| `crates/ragent-config/src/compression.rs` | `CompressionConfig`, `CcrConfig`, `CompressorConfig`, `RelevanceConfig`, `TokenizerConfig` | `compression` section with `enabled`, `auto_threshold`, `ccr`, `compressors`, `relevance`, `tokenizer` | Delete file. Create `ragent-config/src/compaction.rs` with `CompactionConfig { auto, buffer, keep }` (T-002). |
| `crates/ragent-config/src/config.rs` | `pub compression: CompressionConfig` | Root config field | Rename to `compaction: CompactionConfig`; add alias `compression` for one release if backward compatibility is desired. |

## 5. TUI integration

| File | Symbol / Lines | Current behaviour | Migration action |
|------|----------------|-------------------|------------------|
| `crates/ragent-tui/src/app/compress.rs` | `handle_compress_command`, `start_provider_compaction_for_session`, `apply_compaction_summary` | `/compress` slash command; fallback provider summarisation already exists here and can be reused. | Rename module to `compact.rs` (or keep `compress.rs`). Reimplement `handle_compress_command` using `compaction` service. Preserve `apply_compaction_summary` logic but store a dedicated `compaction` message instead of an assistant summary. |
| `crates/ragent-tui/src/app/session_ops.rs:40-75` | `should_auto_compact_before_send`, `start_compaction` | 92% threshold trigger; calls provider fallback compaction. | Reimplement on top of `compaction::compact_if_needed`. |
| `crates/ragent-tui/src/app/event_handler.rs:204-242` | `Event::CompressionStarted/Finished` handling | Updates `compress_in_progress`, logs ratio, updates `last_input_tokens`. | Keep event handling; rename status strings to "compacting". |
| `crates/ragent-tui/src/app/slash.rs:480-487, 1350` | Slash command registration, telemetry display | Registers `compress` command; displays `context_compressions` counters. | Register `/compact`; keep telemetry counters if names remain. |
| `crates/ragent-tui/src/app/state.rs:606-607, 1317` | `compress` slash entry, `compress_in_progress` | UI state | Rename to `compact_in_progress` or keep generic flag. |
| `crates/ragent-tui/src/app/init.rs:234` | `compress_in_progress: false` | Initial state | Update initial state name if renamed. |
| `crates/ragent-tui/src/app/input_handler.rs` | References to `compress` command | Input handling | Update references. |

## 6. Types and events

| File | Symbol / Lines | Current definition | Migration action |
|------|----------------|--------------------|------------------|
| `crates/ragent-types/src/event/mod.rs:241-258` | `Event::CompressionStarted`, `Event::CompressionFinished` | Existing events | Keep event variants (telemetry compatibility) but populate them from the new summarisation path. Optionally add `reason` and `message_id` fields matching OpenCode. |
| `crates/ragent-types/src/message/mod.rs` (or equivalent) | `Role`, `Message` | No dedicated compaction role | Add `Role::Compaction` or a `type` discriminator so storage can store summary messages (T-003). |

## 7. Telemetry

| File | Symbol / Lines | Current metric | Migration action |
|------|----------------|----------------|------------------|
| `crates/ragent-telemetry/src/counters.rs` | `context_compressions` | Counter incremented on compression | Keep counter; increment from new compaction path. |
| `crates/ragent-telemetry/src/instruments.rs` | `context_compression_ratio_last` | Last ratio gauge | Keep gauge; populate from new path. |
| `crates/ragent-telemetry/src/recorder.rs` | Recorder wiring | Event consumption | Ensure new `CompactionStarted/Finished` events still update telemetry. |

## 8. Storage

| File | Symbol / Lines | Current schema | Migration action |
|------|----------------|----------------|------------------|
| `crates/ragent-storage/src/storage.rs:328-339` | `messages` table (`role`, `parts`, `created_at`) | Stores messages by role | Add migration or alter `role` enum to include `compaction`. Compaction messages are loaded by `created_at` ordering. |

## 9. Tests and benchmarks

| File | What it tests | Migration action |
|------|---------------|------------------|
| `crates/ragent-agent/tests/inline/compression_pipeline.rs` | Headroom tokenizer, `hello()`, diff compressor | Delete in T-011. |
| `crates/ragent-agent/tests/test_compression_pipeline.rs` | Pipeline behaviour | Delete; write new tests for summarisation compaction (T-012). |
| `crates/ragent-tui/tests/test_compression_indicator.rs` | TUI compression indicator | Rewrite for `/compact` indicator if needed, or delete. |
| `crates/ragent-agent/benches/agent_loop.rs` | Agent loop benchmarks referencing compression | Update or remove compression benchmark code. |

## 10. Removal order (recommended)

1. **T-002 / T-003 / T-004 / T-005 / T-006** — Build the replacement
   (`CompactionConfig`, compaction message type, serialiser, estimator,
   prompt template) before touching Headroom code.
2. **T-007** — Implement `compaction::compact_if_needed` and
   `compaction::compact_after_overflow` with unit tests.
3. **T-008 / T-009** — Wire the new compaction into `loop_steps.rs` and
   `processor.rs`; keep `Event::CompressionStarted/Finished` publishing.
4. **T-010** — Update `/compress` → `/compact` in the TUI.
5. **T-011** — Only now delete the `compression` module tree, remove the
   `headroom-core` dependency, and delete `ragent-config/src/compression.rs`.
6. **T-012** — Replace tests and benchmarks.
7. **T-013** — Update docs.

## 11. Grep commands used to produce this audit

```bash
# Core headroom references
grep -ri "headroom" --include="*.rs" --include="*.toml" .

# CCR markers / retrieve tool
grep -ri "headroom_retrieve\|<<ccr:\|parse_ccr_markers" --include="*.rs" .

# Compression pipeline entry points
grep -ri "compress_history\|compress_chat_messages\|CompressionMode\|CompressionResult\|CompressionStats" --include="*.rs" .

# CCR store
grep -ri "SharedCcrStore\|CcrStoreHandle\|create_shared_ccr_store\|create_test_ccr_store" --include="*.rs" .

# Call sites
grep -ri "compression::pipeline\|compression::relevance\|ccr_store::" --include="*.rs" .

# TUI / telemetry
grep -ri "compress_in_progress\|handle_compress_command\|context_compressions\|context_compression_ratio" --include="*.rs" .
```

## 12. Risk notes

- The TUI already has a **provider-fallback compaction** path in
  `crates/ragent-tui/src/app/compress.rs` (`start_provider_compaction_for_session`,
  `apply_compaction_summary`). This is conceptually closer to the OpenCode model
  than the Headroom CCR pipeline, so parts of it can be reused.
- `Event::CompressionStarted/Finished` are wired into telemetry and the TUI
  status bar. Keep these event names for compatibility.
- The `ragent-agent` crate currently re-exports several compression symbols
  from `session/mod.rs`; these are part of the public crate API. Removing them
  is acceptable only because this is a pre-1.0 beta release, but downstream
  callers inside the workspace must be updated at the same time.
