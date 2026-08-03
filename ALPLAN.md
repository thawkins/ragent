# ALPLAN — Agent-Loop Performance Remediation Plan

Source data: `/actionloop` average timings (count / avg ms / self avg / max ms).

| operation | count | avg ms | max ms | share of loop.step |
|---|---|---|---|---|
| `loop.step.total` | 67 | 5758.38 | 44569.54 | 100% |
| `loop.llm.total` | 66 | 5645.51 | 44532.04 | ~98% |
| `loop.llm.create_stream` | 66 | 4246.99 | 10902.48 | ~74% |
| `loop.llm.stream` | 66 | 1396.36 | 40278.55 | ~24% |
| `loop.llm.wait_next_event` | 1057 | 87.16 | 40184.37 | inside stream |
| `storage.assistant_interim.update` | 64 | 29.08 | 64.39 | ~0.5% |
| `tool.total:grep` | 19 | 13.93 | 125.52 | tool phase |
| `tool.total:read` | 59 | 0.57 | 3.54 | tool phase |

`loop.llm.*` accounts for ~98% of loop time. The remediation priorities are
ranked by *controllable* cost — the largest buckets that we can actually do
something about.

---

## H1 — `loop.llm.stream`: remove the dead stall-poll wrapper

**Where:** `crates/ragent-agent/src/session/loop_steps.rs:960-1001`

**Symptom:** `loop.llm.wait_next_event` fires 1057 times at avg 87ms (~92s of
aggregate wait). The profiler scope is opened around every single stream event
because the poll wrapper is *inside* the per-event loop, so every iteration
pays the `tokio::select!` scheduling + profiler overhead.

**Root cause (dead code):**
- `last_event_at: Option<Instant> = None` (line 961) is **never reassigned**,
  so the `if let Some(last) = last_event_at` stall branch (line 978) can never
  fire. The `biased; _ = sleep(100ms)` arm always falls through to the `else`
  branch (line 996) which just calls `stream.next().await`.
- The whole `tokio::select! { biased; sleep(100ms) ... }` wrapper is therefore
  a **no-op** that only adds ~100ms-slice scheduling overhead and a profiler
  `scope()`/`scope()` drop per event.
- The `StreamBuffer::reset_timer()` calls (lines 1036/1047/1067) are also
  ineffective — `should_flush()` (stream_buffer.rs:100) checks `last_flush`
  which is only reset on an *explicit flush*, and the interval branch
  (`STREAM_BUFFER_FLUSH_MS=50`) is what actually triggers time-based flushes.

**Remediation:**
1. Delete the outer `tokio::select!` and the `last_event_at` plumbing; call
   `stream.next().await` directly inside the `loop.llm.stream` scope. Stall
   detection is already handled *inside* each provider stream (e.g.
   `ollama.rs:489` wraps `stream.next()` in its own timeout), so the outer
   layer is redundant.
2. Move the `loop.llm.wait_next_event` profiler scope out of the per-event
   loop (open it once around the whole `loop { ... }`).
3. Fix `StreamBuffer` flush timing so the *time-based* flush actually works:
   update `last_flush` in `push_text`/`push_reasoning` when a flush fires, or
   change `should_flush()` to measure elapsed-since-first-event rather than
   relying on `reset_timer()` being called from the now-deleted wrapper.

**Expected effect:** removes 1 profiler scope + 1 `select!` + 1 sleep timer per
stream event. For a token-dense stream (thousands of deltas) this eliminates
thousands of redundant `sleep`/`poll`/`scope` cycles and should shave most of
the non-network portion of `loop.llm.stream`.

**Verify:** `/actionloop` — `loop.llm.wait_next_event` count drops to ~1 per
turn (or the label disappears), `loop.llm.stream` self-avg drops toward the
network floor.

---

## H2 — `loop.llm.create_stream`: reduce request serialisation + reuse the HTTP client

**Where:** provider `chat()` impls — `openai.rs:329`, `anthropic.rs:387`,
`ollama.rs:424`, `gemini.rs:515`, `huggingface.rs:451`, `copilot.rs`,
`generic_openai.rs`, `azure_foundry.rs`, `bedrock.rs`, `router_client.rs:163`.

**Symptom:** `loop.llm.create_stream` avg 4247ms / max 10.9s — the dominant
bucket (~74% of loop time). Most of this is unavoidable provider round-trip
latency (the `chat()` future completes when response headers arrive), but a
meaningful slice is local CPU doing request-body construction.

**Local-CPU contributors:**
1. Every `chat()` call re-builds the full JSON body via
   `build_request_body` + `.json(&body)` — re-serialising the entire tool
   schema set (~111 tools) and full message history on **every loop step**,
   including retry attempts. `openai.rs:182`, `anthropic.rs:293`,
   `ollama.rs:253` each construct `serde_json::Value` trees from scratch.
2. The tool schemas are cached (`get_cached_tool_definitions`,
   `processor.rs:251`) but the *serialised* schema bytes are not — each
   provider re-runs `parameters.to_string()` per tool per call.

**Remediation:**
1. Cache the **serialised tool-definition JSON** once (e.g. a
   `OnceLock<Arc<[u8]>>` / `Arc<str>` in the `SessionProcessor`, keyed off the
   same invalidation as `cached_tool_definitions`). Providers that accept a
   raw `tools` array should reuse these bytes instead of re-serialising.
   `estimate_tool_definition_bytes` (history.rs:331) already sums the sizes;
   extend it (or add a sibling) that returns the cached `Arc<[u8]>`.
2. Have providers build the request body with `serde_json::to_vec` into a
   pre-sized `Vec<u8>` rather than building a `Value` tree then `.json()`ing
   it. This halves allocations.
3. Reuse the `reqwest::Client` across turns instead of rebuilding it in
   `prepare_client` (`loop_steps.rs:254-288`). The clients are currently
   created per turn via `create_client`; keep a per-provider client cache so
   connection-pool warm-up (TLS, keep-alive) is amortised across turns/steps.
   The router (`router_client.rs:373`) creates a fresh downstream client on
   **every** delegated call — cache it keyed by `(provider, model)`.

**Expected effect:** removes ~111 tool-schema re-serialisations per request and
avoids re-establishing connections; for a local provider (Ollama) this moves
the `create_stream` floor from serialisation+connect to pure model-inference
latency. On a 1.8s `create_stream` this could reclaim a few hundred ms; on the
10.9s max (likely model-load) it removes the CPU overhead before the wait.

**Verify:** `/actionloop` — `loop.llm.create_stream` avg drops; CPU profiler
shows `serde_json::to_string`/`to_vec` hot paths reduced.

---

## H3 — `storage.assistant_interim.update`: stop rewriting the FTS index every step

**Where:** `crates/ragent-storage/src/storage.rs:969-990`
(`update_message`), `crates/ragent-agent/src/session/processor.rs:1622-1679`
(interim save).

**Symptom:** avg 29ms × 64 = ~1.9s of aggregate loop time. Every interim save
calls `update_message`, which:
1. re-serialises the full assistant `parts` to JSON (`serde_json::to_string`),
2. **DELETEs the row from `messages_fts` and re-INSERTs it** (storage.rs:978-988)
   — a full FTS index rebuild of that message's text every step,
3. runs inside a `spawn_blocking` (processor.rs:1676) but still blocks a
   blocking-thread + holds the SQLite write lock (WAL).

**Remediation:**
1. Skip the FTS delete+re-insert when the interim message's *text content is
   unchanged*. The interim save already computes an `FxHash` of `assistant_parts`
   (processor.rs:1632) to decide whether to persist at all; extend the
   condition so that when the only change is a tool-call status transition
   (no text/parts text change), the FTS sync is skipped. Add a
   `update_message_parts_skip_fts` variant, or make `update_message` take a
   `sync_fts: bool`.
2. Better: use an FTS **external-content** table (`content='messages'` +
   `content_rowid`) so the FTS index is automatically maintained by the
   `parts` column and you never DELETE+INSERT manually. `storage.rs:555-560`
   currently creates a standalone FTS5 table; switching to external-content
   removes the manual sync entirely.
3. Defer the interim FTS sync: keep the `messages` row update cheap, and
   enqueue an async FTS-sync task (already how `warm_message_search_index`
   batches). The search index is eventually-consistent anyway.

**Expected effect:** drops `storage.assistant_interim.update` from ~29ms to
sub-ms on unchanged-text steps (the majority), and removes the write-lock
hold time that can stall concurrent readers.

**Verify:** `/actionloop` — `storage.assistant_interim.update` avg < 2ms;
`session_search`/`conversation_search` results still correct after an edit.

---

## H4 — `loop.llm.create_stream`: surface provider progress so timeouts don't feel like hangs

**Where:** `loop_steps.rs:852-956`, `ollama.rs:441-468`.

**Symptom:** the 10.9s `create_stream` max and the `loop.llm.stream` 40s max
correlate with a local model being cold-loaded. The user sees a long silence.

**Remediation (low priority, UX):**
1. In `call_llm_step`, when `create_stream` exceeds a short threshold (e.g.
   2s), publish an `AgentNotice` "waiting for model response…" so the TUI
   status bar updates. The `RequestStarted` event (loop_steps.rs:852) is
   already published; add a timer that emits a second notice if no first
   stream event arrives.
2. For Ollama, expose `auto_start`/model-load state in the notice.

**Expected effect:** no raw time saved, but the perceived latency is bounded
and the 100ms poll (H1) is already removed.

---

## Non-issues (no action required)

- `loop.step.total` self-avg 162ms vs 5758ms total — the loop *overhead*
  (setup, publish_tools, tool-phase prep, interim-save hash) is ~3% of the
  step; the rest is LLM wait (H1–H3). No per-step micro-optimisation pays off.
- `tool.total:grep` (13.9ms avg) and `tool.total:read` (0.57ms avg) — tool
  execution is negligible.
- `prompt.collect_context` (25.7ms), `storage.user_message.create` (15.3ms),
  `llm.create_client` (9.9ms) — one-shot per-turn costs; negligible vs 5.7s
  steps. H2's client caching will shrink `llm.create_client` further.

---

## Rollout order

1. **H1** — pure deletion of dead code, lowest risk, removes per-event overhead.
   No behavioural change (stall detection lives in providers). Re-run
   `/actionloop`.
2. **H3** — FTS skip / external-content. Isolated to storage; verify with
   existing `session_search` tests.
3. **H2** — tool-schema byte cache + client reuse. Touches providers; keep the
   public `LlmClient::chat` signature unchanged and gate the cache behind the
   existing invalidation path.
4. **H4** — UX notice. Optional polish after 1–3.

Success criteria (post-rollout `/actionloop`):

- `loop.step.total` avg reduced in proportion to `loop.llm.stream` +
  `storage.assistant_interim.update` savings.
- `loop.llm.wait_next_event` count collapses to ~1 per turn (or disappears).
- `loop.llm.create_stream` avg falls to the provider's raw round-trip floor.
