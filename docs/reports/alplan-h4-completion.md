# ALPLAN H4 Completion Report

## Task
Execute ALPLAN task H4: surface provider progress during long `create_stream` waits so timeouts do not feel like hangs.

## Changes Made

### `crates/ragent-agent/src/session/loop_steps.rs`
- Added `std::sync::atomic::{AtomicBool, Ordering}` to imports.
- In `call_llm_step`, immediately before each `loop.llm.create_stream` attempt, spawn a background `tokio::task` that:
  - Sleeps for 2 seconds.
  - If the first stream event has not yet arrived, publishes `Event::AgentNotice` with a user-visible message.
  - Uses an Ollama-specific message when the provider is `ollama`:
    `"Waiting for model response (Ollama may be loading the model)..."`
  - Uses a generic message for all other providers:
    `"Waiting for model response..."`
- Abort the background task as soon as `chat()` returns (the stream has been created).
- Set an atomic `first_event_arrived` flag when the first `StreamEvent` arrives, so duplicate notices cannot fire after streaming begins.

### `crates/ragent-agent/tests/test_compaction_integration.rs`
- Added the `llm_client_cache` field (introduced by ALPLAN H2) to the three `SessionProcessor` struct literals in this test file so tests compile against the current `SessionProcessor` definition.

## Verification
- `cargo check --all-targets` passes.
- `cargo test -p ragent-agent` passes.

## Expected Effect
No raw latency reduction. The 2-second threshold publishes a status notice during cold-model loads or slow provider handshakes, bounding perceived latency and preventing the UI from appearing hung.

## Rollout Context
H4 was implemented on top of pre-existing working-tree changes for ALPLAN H1 (stall-poll wrapper removal) and H2 (per-provider LLM client cache) in `loop_steps.rs`.
