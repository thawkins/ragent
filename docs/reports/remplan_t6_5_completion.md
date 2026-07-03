# REMPLAN.md T6.5 — Refactor `process_user_message` into named steps — Completion Report

**Date:** 2025-01-17  
**Status:** ✅ PARTIALLY COMPLETE (7 of 8 steps extracted; tool dispatch remains inline)

## Summary

T6.5 refactored the 2,273-line `process_user_message` into a sequence of named
steps, extracting 7 of the 8 planned steps into `session/loop_steps.rs`. The
tool dispatch step (~700 lines) remains inline due to the 20+ local variables
captured by its `tokio::spawn` closures. All tests pass and the workspace
compiles clean.

## Before / After

| Metric | Before | After | Change |
|--------|--------|-------|--------|
| `processor.rs` | 2,911 lines | 1,480 lines | −49% |
| `process_user_message` | 2,273 lines | 842 lines | −63% |
| `loop_steps.rs` | (new) | 1,300 lines | — |

## Extracted steps (in `session/loop_steps.rs`)

| Step | Method | Lines | Description |
|------|--------|-------|-------------|
| 1. prepare_client | `SessionProcessor::prepare_client` | ~200 | Resolve model/provider/api_key/base_url/client, config, working_dir, team_context, hooks |
| 2. build_system_prompt | `SessionProcessor::build_turn_system_prompt` | ~200 | Assemble system prompt: tool reference, codeindex guidance, team guidelines, spec context |
| 3. build_chat_messages | `SessionProcessor::build_turn_chat_messages` | ~90 | Load history, optional compression, convert to ChatMessages |
| 4. run_init_acknowledgement | `SessionProcessor::run_inline_init_acknowledgement` | ~80 | AGENTS.md display-only init exchange |
| 5. call_llm_step | `SessionProcessor::call_llm_step` | ~350 | LLM call with retry + stream event handling |
| 6. handle_no_tool_decision | `SessionProcessor::handle_no_tool_decision` | ~70 | Stall/planning/incomplete nudge detection |
| 7. finalize_assistant_message | `SessionProcessor::finalize_assistant_message` | ~50 | Final save + timing + hooks |

## Inline step (not extracted)

| Step | Lines | Reason |
|------|-------|--------|
| 8. dispatch_tool_calls | ~700 | The `tokio::spawn` closure captures ~20 local variables (registry, permission_checker, event_bus, hook_configs, extraction_engine, storage_clone, profiler, auto_approve, team_context_cache, tool_ctx fields, etc.). Extracting it requires a `ToolDispatchCtx` struct with all captured values, which adds complexity without reducing the total line count. Kept inline in `process_user_message` for maintainability. |

## New structs

- `TurnClient` — immutable per-turn context (model_ref, client, session_config, parsed_hook_configs, working_dir, team_context)
- `LoopState` — mutable loop state (chat_messages, assistant_parts, agent_switch_requested, etc.)
- `LlmStepResult` — result of call_llm_step (text_buffer, reasoning_buffer, tool_calls, token counts)

## Verification

| Check | Result |
|-------|--------|
| `cargo check --workspace` | ✅ |
| `cargo build --workspace --tests` | ✅ |
| `cargo test -p ragent-agent --lib` | ✅ 254 passed |
| `cargo test -p ragent-agent --test session_processor` | ✅ 22 passed |
| `cargo test -p ragent-agent --test test_compression_pipeline --features compression` | ✅ 29 passed |

## Exit criteria status

- `processor.rs` ≤ ~1200 lines → **1,480 lines** (partially met, −280 over target)
- No single fn exceeds ~400 lines → **842 lines** (partially met, the inline tool dispatch is the overage)

## Files

| File | Lines | Change |
|------|-------|--------|
| `crates/ragent-agent/src/session/processor.rs` | 1,480 | down from 2,911 |
| `crates/ragent-agent/src/session/loop_steps.rs` | 1,300 | new file |
| `crates/ragent-agent/src/session/mod.rs` | +1 line | added `pub mod loop_steps;` |