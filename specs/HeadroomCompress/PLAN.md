# Implementation Plan — HeadroomCompress

## Overview

This plan integrates the `headroom-core` Rust crate into ragent to replace the
naive `truncate_history_with_atomic_tool_calls` truncation with content-aware,
reversible, relevance-ranked compression. The work is broken into seven tasks
ordered by dependency chain and risk.

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add `headroom-core` dependency and feature flags | NFR-001, FR-013, FR-015 | S | Critical | completed | — |
| T-002 | Replace `TokenEstimator` with Headroom tokenizer | FR-001, NFR-004 | M | Critical | completed | T-001 |
| T-003 | Implement content-type detection for message parts | FR-002, FR-003 | M | Critical | completed | T-001 |
| T-004 | Implement CCR store lifecycle and `headroom_retrieve` tool | FR-004, FR-007, FR-008, NFR-005 | L | High | completed | T-001 |
| T-005 | Build Headroom compression pipeline wrapper | FR-003, FR-005, FR-006, FR-017, NFR-002, NFR-003 | L | High | completed | T-002, T-003, T-004 |
| T-006 | Wire pipeline into session processor and `/compact` command | FR-005, FR-006, FR-009, NFR-004 | M | High | completed | T-005 |
| T-007 | Add `/compress` slash command with parameter modes | FR-012 | M | Medium | completed | T-006 |
| T-008 | Add relevance filtering with BM25 scorer | FR-011, FR-010 | L | Medium | completed | T-005 |
| T-009 | Add compression config to `ragent.json` schema | FR-009, Configuration section | S | Critical | completed | T-001 |
| T-010 | Integration tests and backward-compatibility validation | NFR-004, FR-009 | M | High | completed | T-006 |
| T-011 | Add `/compress help` slash command with descriptive output | FR-018 | S | Medium | completed | T-007 |
## Task Details

### T-001 — Add `headroom-core` dependency and feature flags

**Effort:** S · **Priority:** Critical · **Depends on:** —

Add `headroom-core` to the workspace `Cargo.toml` and `ragent-agent/Cargo.toml`.

**Steps:**
1. Add `headroom-core = "0.1"` to `[workspace.dependencies]` in root `Cargo.toml`.
2. Add `headroom-core = { workspace = true }` to `crates/ragent-agent/Cargo.toml`.
3. Define feature flags:
   - `compression` — enables all content-aware compressors (default on)
   - `compression-ml` — enables the ML-based Kompress model and Magika detector (default off)
   - `compression-full` — enables both above plus Redis CCR backend
4. Gate all new compression code behind `#[cfg(feature = "compression")]`.
5. Verify `cargo build` and `cargo test --workspace` pass with and without the feature.
6. Measure debug build time delta and release binary size delta (NFR-001: ≤30s, ≤8MB).

**Requirement coverage:** FR-013 (no merge), FR-015 (no Python), NFR-001 (build impact)

---

### T-002 — Replace `TokenEstimator` with Headroom tokenizer

**Effort:** M · **Priority:** Critical · **Depends on:** T-001

Replace the `chars / 4 + 10` heuristic in `TokenEstimator` with Headroom's
`Tokenizer` trait, which provides model-aware token counting via tiktoken-rs
(for OpenAI models), HfTokenizer (for Cohere/Llama/Mistral), and estimation
fallback.

**Steps:**
1. Create `crates/ragent-agent/src/compression/mod.rs` with a `CompressionTokenizer`
   struct that wraps `headroom_core::tokenizer::get_tokenizer`.
2. In `crates/ragent-agent/src/session/cache.rs`:
   - Replace `TokenEstimator::estimate_message` with calls to `CompressionTokenizer`.
   - Keep `TokenEstimator` as the fallback when `compression` feature is off.
3. Add model-to-tokenizer mapping: given the current model ID (e.g.
   `claude-sonnet-4-20250514`), select the appropriate tiktoken encoding or
   HfTokenizer config. For Anthropic models where no tiktoken encoding exists,
   use the estimating counter with the provider's advertised `context_window`.
4. Update `SessionState::should_compress` to use accurate counts.
5. Update the quick-estimate loop in `processor.rs` (lines ~1105–1129) to use
   the Headroom tokenizer instead of the `text_len / 4 + 10` heuristic.
6. Run existing `test_context_compaction` tests — they must pass unchanged
   (NFR-004).

**Requirement coverage:** FR-001

---

### T-003 — Implement content-type detection for message parts

**Effort:** M · **Priority:** Critical · **Depends on:** T-001

Use `headroom_core::transforms::detect` to classify each `MessagePart` as JSON,
diff, log, search results, code, or prose before compression.

**Steps:**
1. Create `crates/ragent-agent/src/compression/content_type.rs` with a
   `detect_message_content_type(part: &MessagePart) -> ContentType` function.
2. Map `MessagePart::Text` content through `headroom_core::transforms::detect`.
3. Map `MessagePart::ToolCall` output through `headroom_core::transforms::detect`
   (tool outputs are the primary compression target — JSON, search results, logs, diffs).
4. Map `MessagePart::Image` and `MessagePart::Reasoning` as `ContentType::Prose`
   (not compressed).
5. Add unit tests for each content type detection path.
6. Gate behind `#[cfg(feature = "compression")]`.

**Requirement coverage:** FR-002

---

### T-004 — Implement CCR store lifecycle and `headroom_retrieve` tool

**Effort:** L · **Priority:** High · **Depends on:** T-001

Set up the Compress-Cache-Retrieve store so that original content is preserved
and the LLM can retrieve it on demand.

**Steps:**
1. Create `crates/ragent-agent/src/compression/ccr_store.rs`:
   - `CcrManager` struct wrapping `headroom_core::ccr::SqliteCcrStore`.
   - `init(session_dir: &Path) -> Result<CcrManager>` — creates/opens the
     SQLite CCR database alongside the session database.
   - `stash(content: &str) -> String` — calls `compute_key` + `put`, returns the
     `<<ccr:HASH>>` marker string.
   - `retrieve(hash: &str) -> Option<String>` — calls `get`.
   - `evict()` — TTL-based eviction using the 5-minute default.
2. Create `crates/ragent-agent/src/compression/tool.rs`:
   - Implement `headroom_retrieve` as a `Tool` that accepts a `hash` parameter
     and returns the stashed original content.
   - Register in `crates/ragent-agent/src/tool/mod.rs`.
3. Create `crates/ragent-agent/tests/test_compression_ccr.rs`:
   - Test stash/retrieve round-trip.
   - Test TTL expiry.
   - Test eviction at capacity limit.
4. Wire CCR manager into `SessionProcessor` — create on session init, pass to
   compression pipeline.
5. Clean up CCR database file when session is deleted (NFR-005).

**Requirement coverage:** FR-004, FR-007, FR-008, NFR-005

---

### T-005 — Build Headroom compression pipeline wrapper

**Effort:** L · **Priority:** High · **Depends on:** T-002, T-003, T-004

Create the main compression pipeline that replaces `truncate_history_with_atomic_tool_calls`.

**Steps:**
1. Create `crates/ragent-agent/src/compression/pipeline.rs` with a
   `HeadroomCompressionPipeline` struct:
   - Takes `Vec<Message>`, `context_window: usize`, `max_output_tokens: usize`,
     and a reference to the `CcrManager`.
   - Returns `CompressedHistory { messages: Vec<Message>, stats: CompressionStats }`.
2. Pipeline stages (in order):
   a. **Token counting** — use Headroom tokenizer for accurate counts.
   b. **Safety check** — if total tokens ≤ threshold, return unmodified (FR-017:
      never compress system prompt or last user message).
   c. **Content detection** — classify each message part (from T-003).
   d. **Compressor dispatch** — route to SmartCrusher/DiffCompressor/etc. based
     on content type (from T-003). For each compressed part, stash the original
     in CCR and replace with `<<ccr:HASH>>` marker.
   e. **Atomic pair preservation** — ensure tool-call + tool-result pairs remain
     intact (adapted from existing logic).
   f. **Final token check** — if still over budget after content compression,
     fall back to relevance-based message selection (keep most relevant + most
     recent, drop oldest low-relevance).
3. `CompressionStats` struct: `original_tokens`, `compressed_tokens`,
   `compression_ratio`, `content_types: HashMap<ContentType, usize>`,
   `ccr_entries_stashed: usize`.
4. All CPU-intensive Headroom operations run via `tokio::task::spawn_blocking`
   to satisfy NFR-003 (non-blocking).
5. Create `crates/ragent-agent/tests/test_compression_pipeline.rs`:
   - Test with JSON-heavy tool outputs (SmartCrusher).
   - Test with diff content (DiffCompressor).
   - Test with log output (LogCompressor).
   - Test with search results (SearchCompressor).
   - Test that system prompt and last user message are never modified.
   - Test fallback when `compression` feature is off.

**Requirement coverage:** FR-003, FR-004, FR-005, FR-006, FR-017, NFR-002, NFR-003

---

### T-006 — Wire pipeline into session processor and `/compact` command

**Effort:** M · **Priority:** High · **Depends on:** T-005

Replace the `truncate_history_with_atomic_tool_calls` call site in the session
processor with the new Headroom pipeline, and update the `/compact` slash command.

**Steps:**
1. In `crates/ragent-agent/src/session/processor.rs`:
   - Replace the compaction block (lines ~1102–1138) with:
     ```rust
     #[cfg(feature = "compression")]
     let compacted_history = self.compression_pipeline.compress(&history, context_window, 8192)?;
     #[cfg(not(feature = "compression"))]
     let compacted_history = truncate_history_with_atomic_tool_calls(&history, context_window, 8192);
     ```
   - Wire the `CcrManager` into `SessionProcessor` (passed on construction).
2. In `crates/ragent-tui/src/app.rs`:
   - Update `/compact` handler to display compression stats (before/after tokens,
     compression ratio, content types compressed).
3. Update `SessionState::should_compress` to use Headroom tokenizer counts.
4. Verify backward compatibility: when `compression` feature is off, behaviour
   is identical to the current implementation (NFR-004).

**Requirement coverage:** FR-005, FR-006, FR-009, NFR-004

---

### T-007 — Add `/compress` slash command with parameter modes

**Effort:** M · **Priority:** Medium · **Depends on:** T-006

Add a new `/compress` slash command that gives users fine-grained control over
compression.

**Steps:**
1. Add `/compress` to `SLASH_COMMANDS` in `crates/ragent-tui/src/app/state.rs`.
2. Parse subcommands in `crates/ragent-tui/src/app.rs`:
   - `/compress` — default pipeline (all compressors enabled per config)
   - `/compress aggressive` — maximum compression with relevance filtering (FR-010)
   - `/compress conservative` — only lossless compressors (JSON minification, tag protection)
3. Each mode passes different `CompressionPolicy` parameters to the pipeline.
4. Display compression stats after completion.

**Requirement coverage:** FR-012

---

### T-008 — Add relevance filtering with BM25 scorer

**Effort:** L · **Priority:** Medium · **Depends on:** T-005

Integrate `headroom_core::relevance::BM25Scorer` to rank messages by relevance
to the current query, enabling intelligent message selection during aggressive
compression.

**Steps:**
1. Add `relevance` config section to `CompressionConfig` (already in config
   schema from T-009).
2. Create `crates/ragent-agent/src/compression/relevance.rs`:
   - `RelevanceFilter` struct wrapping `BM25Scorer`.
   - `rank_messages(messages, current_query) -> Vec<(usize, f64)>` — returns
     message indices with BM25 scores.
   - `select_top_k(messages, current_query, k) -> Vec<Message>` — keeps the K
     most relevant messages plus always-kept messages (system, last user).
3. Wire into the compression pipeline: when `relevance.enabled` is true and
   compression is in "aggressive" mode, use BM25 scores to decide which older
   messages to drop.
4. Gate behind `#[cfg(feature = "compression")]`.

**Requirement coverage:** FR-010, FR-011

---

### T-009 — Add compression config to `ragent.json` schema

**Effort:** S · **Priority:** Critical · **Depends on:** T-001

Add the `CompressionConfig` struct to `ragent-config` and support it in the
JSON config schema.

**Steps:**
1. Create `crates/ragent-config/src/compression.rs`:
   - `CompressionConfig` struct (mirrors the Configuration section in SPEC.md).
   - `CcrConfig`, `CompressorConfig`, `RelevanceConfig`, `TokenizerConfig`.
   - `Default` impl: `enabled: true`, `auto_threshold: 0.80`, CCR defaults
     matching `headroom_core::ccr` defaults.
2. Add `compression: CompressionConfig` to the main `Config` struct.
3. Add serde deserialization with backward compatibility (missing key = default).
4. Register in `crates/ragent-config/src/lib.rs` and `mod.rs`.
5. Add config parsing tests.

**Requirement coverage:** FR-009, Configuration section

---

### T-010 — Integration tests and backward-compatibility validation

**Effort:** M · **Priority:** High · **Depends on:** T-006

End-to-end testing of the full compression pipeline and verification that
existing behaviour is preserved when the feature is off.

**Steps:**
1. Create `crates/ragent-agent/tests/test_compression_integration.rs`:
   - Test: full conversation history (system + 50 messages) triggers auto-compression.
   - Test: `/compact` command runs and reports stats.
   - Test: `headroom_retrieve` tool returns stashed originals.
   - Test: CCR TTL expiry works correctly.
   - Test: `compression.enabled = false` falls back to old compaction.
2. Verify all existing `test_context_compaction` tests pass unchanged.
3. Run `cargo test --workspace` with and without `compression` feature.
4. Benchmark: measure compression time for 200-message history (NFR-002: <500ms).
5. Measure binary size delta with and without `compression` feature (NFR-001).

**Requirement coverage:** NFR-001, NFR-002, NFR-004, FR-009

## Dependency Graph

```
T-001 ──┬── T-002 ──────────────┐
         ├── T-003 ──────────────┤
         ├── T-004 ──────────────┤
         └── T-009 ───────���──────┤
                                  │
                     T-005 ───────┤ (depends on T-002, T-003, T-004)
                       │
                     T-006 ───────┤ (depends on T-005)
                       │
                              ┌── T-007 ─────────┤ (depends on T-006)
                              │        └── T-011 ──┤ (depends on T-007)
                              └── T-008 ─────────┤ (depends on T-005)                                  │
                     T-010 ───────┘ (depends on T-006)
```

## Estimated Timeline

| Phase | Tasks | Duration |
|-------|-------|----------|
| Phase 1: Foundation | T-001, T-009 | 1–2 days |
| Phase 2: Core | T-002, T-003, T-004 | 3–4 days |
| Phase 3: Pipeline | T-005 | 2–3 days |
| Phase 4: Integration | T-006, T-010 | 2–3 days |
| Phase 5: Enhancement | T-007, T-008, T-011 | 2–4 days |
| **Total** | | **11–17 days** |

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| `headroom-core` not yet published on crates.io | Clone the repo as a path dependency or git dependency initially; publish-or-fork decision later |
| `headroom-core` pulls heavy deps (tiktoken, HF hub, ONNX) | Gate heavy deps behind `compression-ml` feature; core `compression` feature uses only estimator + keyword detection |
| `SqliteCcrStore` not `Send + Sync` | Wrap in `Mutex<SqliteCcrStore>` or use `InMemoryCcrStore` (DashMap, already `Send + Sync`) as default |
| tiktoken encoding name mismatch for Anthropic models | Fall back to `EstimatingCounter` with per-model `chars_per_token` calibration (Anthropic ≈ 3.5, GPT-4 ≈ 4) |
| Compression pipeline too slow for real-time use | Profile with `criterion`; use `spawn_blocking`; fall back to old compaction if >500ms |
### T-011 — Add `/compress help` slash command with descriptive output

**Effort:** S · **Priority:** Medium · **Depends on:** T-007

Add a `/compress help` subcommand (and bare `/compress help` alias) that prints a
formatted table of all compression subcommands, their descriptions, and the
current session's compression configuration status.

**Steps:**
1. In `crates/ragent-tui/src/app.rs`, extend the existing `/compress` handler
   (added in T-007) to match the `help` subcommand and render a static table
   of subcommands + descriptions matching FR-018's table.
2. Append a dynamic configuration status section showing:
   - `compression.enabled` (true/false)
   - `compression.tokenizer.backend` (auto/tiktoken/estimate)
   - `compression.ccr.backend` (sqlite/memory)
   - `compression.auto_threshold` (percentage)
   - Current session CCR entry count (queried from `CcrManager`)
3. In `crates/ragent-tui/src/app/state.rs`, ensure `SLASH_COMMANDS` includes
   the `/compress` trigger with a description that mentions `/compress help`
   for details (e.g. `"Advanced context compression: /compress [default|aggressive|conservative|help|stats]"`).
4. Add a `/compress stats` subcommand that queries the `CompressionStats` from
   the last compression run (stored in session state) and displays tokens
   before/after, compression ratio, CCR entries stashed, and content types
   compressed.
5. Add a test in `crates/ragent-tui/tests/` verifying that `/compress help`
   output contains all five subcommand rows and the configuration status keys.

**Requirement coverage:** FR-018