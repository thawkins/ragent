# Agent Loop Performance Baseline

**Generated:** PERFPLAN Milestone F-3
**Machine:** reference development machine (Linux x86_64, stable Rust toolchain)
**Harness:** `cargo bench -p ragent-bench --bench agent_loop`

## Purpose

This document records the baseline timings for the agent action loop hot
path so the CI regression guard (Milestone F-5) can detect ≥10% regressions
before they ship. The benches are hermetic — no network, no real LLM — using
synthetic fixtures and `ragent_bench::MockLlmClient`.

## Baseline results

These are the median timings captured on the reference machine. Absolute
numbers vary by hardware; the regression guard compares against the
`target/criterion` baseline committed alongside this report, so relative
drift is what matters.

| Benchmark                                   | Median         | Throughput        |
|---------------------------------------------|----------------|-------------------|
| `history_to_chat_messages/10`               | ~1.7 µs        | 5.8 Melem/s       |
| `history_to_chat_messages/50`               | ~8.9 µs        | 5.6 Melem/s       |
| `history_to_chat_messages/200`              | ~34 µs         | 5.9 Melem/s       |
| `history_to_chat_messages/800`              | ~140 µs        | 5.7 Melem/s       |
| `tool_result_content_for_llm/short`        | ~34 ns         | —                 |
| `tool_result_content_for_llm/long`          | ~90 µs         | —                 |
| `estimate_request_bytes/10`                | ~39 ns         | 259 Melem/s       |
| `estimate_request_bytes/100`               | ~235 ns        | 426 Melem/s       |
| `estimate_request_bytes/500`               | ~1.05 µs       | 475 Melem/s       |
| `estimate_tool_definition_bytes/10`         | ~20 µs         | 500 Kelem/s       |
| `estimate_tool_definition_bytes/50`         | ~140 µs        | 358 Kelem/s       |
| `estimate_tool_definition_bytes/111`        | ~245 µs        | 453 Kelem/s       |
| `interim_save_hash/1`                       | ~372 ns        | 2.7 Melem/s       |
| `interim_save_hash/5`                       | ~2.24 µs       | 2.2 Melem/s       |
| `interim_save_hash/20`                      | ~8.4 µs        | 2.4 Melem/s       |
| `mock_llm_chat_stream/text_only`            | ~993 ns        | —                 |
| `mock_llm_chat_stream/single_tool_call`     | ~1.24 µs       | —                 |

## What is measured

- **`history_to_chat_messages`** — per-turn history→`ChatMessage` conversion.
  Linear in message count. P-22-adjacent (the function awaits image reads
  via `spawn_blocking`, so it stays `async`).
- **`tool_result_content_for_llm`** — per-tool-call result truncation (P-16).
  The short path is a single byte-length check; the long path truncates.
- **`estimate_request_bytes`** — per-step request-size estimate (P-7). With
  the P-7 cache, the tool-definition byte sum is pre-computed once per turn,
  so the per-step estimate only pays for the actual tool-call inputs.
- **`estimate_tool_definition_bytes`** — one-time tool-definition byte sum.
  Called once when `cached_tool_definitions` is populated (P-7).
- **`interim_save_hash`** — per-step change-detection hash (P-12). Hashes
  `serde_json::to_vec` bytes instead of `Value::to_string()`, avoiding the
  per-step JSON pretty-print allocation for every tool-call input/output.
- **`mock_llm_chat_stream`** — `MockLlmClient` stream throughput (F-1).
  The text-only variant measures TTFT + stream drain; the single-tool-call
  variant measures tool-call assembly throughput.

## Regenerating the baseline

```bash
cargo bench -p ragent-bench --bench agent_loop -- --save-baseline main
```

The `target/criterion` directory holds the saved baseline. Commit it (or
the relevant subset) so the CI guard can compare against it.

## CI guard

`scripts/check-bench-regression.sh` (Milestone F-5) compares the current
bench median against the saved baseline and fails when any benchmark
regresses by more than 10%. Wired into `pre-flight.sh`.