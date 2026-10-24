---
spec_id: uniformtools
---

# Implementation Plan: Unified Tool Calling, and the Tool Rename It Enables

## Overview

This plan implements `specs/uniformtools/SPEC.md` (FR-001 - FR-034,
NFR-001 - NFR-009) in three passes that touch the same surface from opposite
directions:

- **Pass A (FR-001 - FR-022)** makes the tool-calling contract explicit: one
  declared wire profile per provider family, one encoder, one decoder, one
  dispatch pipeline, and a declarative alias table for provider spellings that
  ragent does not adopt.
- **Pass B (FR-023 - FR-030)** renames thirteen ragent tools to the Claude Code
  spellings `tools.xlsx` records as exact matches, and retires the old
  spellings completely. This is the reason the feature exists: a skill ported
  from Claude Code resolves because the registry genuinely holds the name it
  uses, not because a translation layer guessed.
- **Pass C (FR-031 - FR-034, NFR-008, NFR-009)** makes that retirement hold. A
  rename that can be undone by a config key, a bridged MCP tool or a completion
  list is a preference, not a rename, so Pass C refuses every such route and
  adds the guard that keeps the refusal honest.

Pass B is independent of Pass A in content but ordered after it, because the
alias table Pass A introduces must be *narrowed* by Pass B (SPEC decision D7):
an alias that maps `Bash` onto `bash` stops being legal the moment `Bash` is
the registered name.

Pass C brackets Pass B rather than following it: its config-loader and
landing-discipline tasks run before the rename, its registry-reader tasks
after, and its guard task in the same change as the rename. Pass C adds
requirements only - it changes no earlier requirement, task or decision.

Nothing here adds a provider or a dependency. Work lands in `ragent-types`,
`ragent-llm`, `ragent-agent`, `ragent-config`, `ragent-tools-core` and
`ragent-tui`.

## Architecture

```
crates/ragent-types/src/
  llm.rs                      # + ContentPart::is_error, ContentPart::Reasoning
  message/mod.rs              # + ToolCallState::is_error (derived from status)

crates/ragent-llm/src/
  providers/
    wire/
      mod.rs                  # NEW: ToolWireProfile, ToolEnvelope, ResultCarrier,
                              #      StreamKeying, ArgsEncoding
      encode.rs               # NEW: encode_tools(), encode_message(), encode_result()
      decode.rs               # NEW: ToolCallAccumulator, decode_tool_events()
      profile_table.rs        # NEW: one const per provider family + coverage test
    tool_cache.rs             # folded into wire/encode.rs (API preserved)
    anthropic.rs              # - build_request_body tool/message branches
    bedrock.rs                # - single active_tool_call_id (FR-005)
    openai_compat.rs          # - message/tool encoding
    openai_responses.rs       # - inline tools array, - Text-only filter (FR-018)
    gemini.rs                 # - synthetic-id name recovery (FR-019)
    sse.rs                    # + profile-driven keying
  lib.rs                      # + wire module export

crates/ragent-agent/src/
  session/
    processor.rs              # + one dispatch pipeline, one decode site
    loop_steps.rs             # + ToolCallAccumulator use, FR-022 recovery
    history.rs                # + is_error propagation, reasoning echo
  tool/
    rename.rs                 # NEW: the FR-023 table (before, after), ascending
                              #      by after; read by migration sites and tests
    bridge.rs                 # NEW: one namespacing + retired-name refusal used
                              #      by every dynamic source (FR-032)
    aliases.rs                # - mappings that become no-ops after the rename
    mod.rs                    # + alias application in resolve/lookup
  backend/
    container.rs              # + renamed literal in the in-container bash path

crates/ragent-config/src/
  config.rs                   # + tools.aliases, tools.expose_alias_names,
                              #      tools.wire_trace; reject a retired spelling
                              #      in an alias source or an unregistered target
  tool_alias.rs               # NEW: alias entry validation shared by the loader
                              #      and the guard (FR-031)
  permission.rs               # permission-type strings, deliberately unchanged

crates/ragent-tui/src/
  widgets/message_widget.rs   # + renamed per-tool printers and summaries
  app/state.rs                # slash-command and tool-name menu entries come
                              #      from the registry only (no dead names)
  app/slash.rs                # slash-command menu filter, no retired spelling

scripts/
  check-tool-rename.sh        # NEW: the FR-027 + NFR-008 guard, wired into CI
                              #      beside check-dead-code-reasons.sh

src/cli.rs                    # + --wire-matrix (FR-016)
```

### Data flow - one step, any provider

```
SessionProcessor::call_llm_step
        |
        v
ChatRequest { messages: Arc<Vec<ChatMessage>>, tools: Arc<Vec<ToolDefinition>> }
        |
        v
encode_tools(profile, &tools)  --- cache hit ---> Arc<CachedTools>
encode_message(profile, msg)    (or Err(WireLoss) -> FR-018)
        |
        v
LlmClient::chat  ->  Stream<StreamEvent>
        |
        v
ToolCallAccumulator::apply(profile, event)     (FR-004, FR-005, FR-019, FR-020)
        |
        v
Vec<PendingToolCall>  (complete calls only)
        |
        v
one dispatch pipeline  (FR-012)
  decode_tool_arguments -> validate_required_args -> unknown-tool ->
  repeat guard -> pre-tool hooks -> permission verdict -> backend -> result
        |
        v
MessagePart::ToolCall { tool, call_id, state{ is_error } }  (FR-001, FR-014)
        |
        v
encode_result(profile, result)   (FR-006, FR-014)
```

### Data flow - name resolution, after all three passes

```
model or client names a tool
        |
        v
registry.get(name) --------------- hit -------------> dispatch pipeline
        |                                                   ^
        | miss                                              |
        v                                                   |
alias table (tools.aliases) -- hit -> retry lookup --------+
        |                                                   |
        | miss                                              |
        v                                                   |
unknown-tool result + did-you-mean hint (FR-021) -----------+
```

After Pass B the alias table holds only provider spellings that are **not**
ragent names - the OpenAI spellings and the two Claude spellings FR-025
refused. A renamed tool resolves on the registry lookup, not on a table read.
After Pass C an alias entry that names a retired spelling is refused when the
config loads (FR-031), so the table cannot grow a second spelling back:

```
config load
        |
        v
tools.aliases entry -------------------- source or target is a `before`? --> refuse
        |                                                   (name the file, the
        | accepted                                           spelling, its `after`)
        v
alias table (provider spellings only) -- hit -> registry lookup
```

A dynamically sourced tool never reaches the bare namespace at all (FR-032):

```
MCP <server>.<tool> | plugin_<id>_<tool> | connector server | skill tool
        |
        v
namespacing step (tool/bridge.rs)
        |
        +-- surfaced name equals a `before`? --> refuse that one tool, report once,
                                                 keep the rest of the source usable
        |
        +-- otherwise --> register under its namespaced name
```

## Tasks

Three tables, one per pass. Every task is `Pending`; no task is done.

### Pass A - unified tool-calling layer (FR-001 - FR-022)

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Extend canonical types: `ToolResult.is_error` and a `Reasoning` content part | FR-014, FR-011 | S | Critical | completed | — |
| T-002 | Declare `ToolWireProfile` with envelope, carrier, id field, argument encoding, stream keying, parallel, `is_error`, reasoning echo, synthetic-id flags | FR-002 | M | Critical | completed | T-001 |
| T-003 | Define one profile per provider family plus a coverage test asserting every registered provider id has a profile | FR-002 | M | Critical | completed | T-002 |
| T-004 | Fold `tool_cache.rs` builders into a single profile-driven `encode_tools` and keep the cache API and invalidation behaviour. The five `build_*` functions are gone; `wire/encode.rs` derives the envelope from `ToolWireProfile::tool_envelope` in one `match`, and the Bedrock whole-object vs inner-array payload difference is one arm of it. `cached_tools`, `invalidate_tool_cache`, `CachedTools` and its four accessors keep their signatures, so no call site changed; `ToolFormat` survives as the cache key (`format_profile`), and the HuggingFace `t_` prefix is a `name_prefix` argument rather than a sixth envelope. Keying, store-outside-the-lock and the double-checked insert are unchanged, so a hit stays byte-identical to a miss. Recorded in `specs/uniformtools/T-004-ENCODER-FOLD.md`: `tests/test_wire_encode.rs` (12 cases, including the exact pre-fold bytes for all five builders) and 8 cache cases in `tests/inline/tool_cache_tests.rs`; red on `input_schema` -> `inputSchema` and on a hand-built `functionDeclarations` in `gemini.rs`, both reverted byte-identically | FR-003, NFR-001 | M | Critical | completed | T-003 |
| T-005 | Delete the inline `tools` array in `openai_responses.rs` and route it through `encode_tools`. `ResponsesApiClient::build_request_body` now calls `cached_tools(ToolFormat::OpenAiResponses, &request.tools)` and assigns `CachedTools::responses_tools_array()`, so no provider module in the tree constructs a definition array. `ToolFormat` gained the `OpenAiResponses` key (resolving to the `OPENAI_RESPONSES` profile in `format_profile`), plus `ToolFormat::ALL` and `ToolFormat::tool_envelope` so the FR-003 coverage assertion is total and two-directional without a hand-written list; the TUI's `tool_format_for_provider` gained the `openai_responses` arm so the context panel stops measuring a Responses session with the chat envelope (532 vs 532 before). Recorded in `specs/uniformtools/T-005-RESPONSES-ENCODER.md`: `test_wire_encode.rs` (a new two-sided source scan for the deleted array, the six-module envelope scan, six-key hit-equals-miss, the rewritten coverage case), `tests/inline/tool_cache_tests.rs` (chat and Responses as separate entries over the same slice; mapping, hit-equals-miss and empty-list cases driven by `ToolFormat::ALL`), `tests/inline/openai_responses_tests.rs` (body equals the encoder payload exactly; the empty list still omits `tools`), and `test_context_panel_t005.rs` (metadata measured with the provider's own envelope); red four ways, all reverted byte-identically | FR-003 | S | Critical | completed | T-004 |
| T-006 | Implement one profile-driven `encode_message` covering all four result carriers, and remove the per-provider message branches. Delivered as `wire/encode.rs`'s `encode_message(profile, &ChatMessage, &MessageContext)`: the carrier comes from `ResultCarrier`, the id key from `IdField`, the argument shape from `ArgsEncoding`, and the two families that share a carrier name but differ in part shape (`UserToolResultBlock` = Anthropic `tool_result` vs Converse `toolResult`; `ToolRoleMessage` = OpenAI id-keyed vs native Ollama name-keyed) are told apart by the profile's envelope + `args_encoding` in one derivation (`MessageFamily::of`), not by the caller. Seven copies of the message loop are gone - `anthropic`, both `bedrock` clients, `gemini`, `huggingface`, `ollama_cloud`, `openai_compat`, `openai_responses` - 8 arms over 7 files. `MessageContext` carries only the four facts a profile cannot (`tool_names`, `name_prefix`, `allow_images`, `mirror_tool_name`) and `tool_names_by_id` is the one id-to-name scan the two name-keyed carriers need. Two dialect flags died with the loops: `OpenAiCompatSpec::strict_single_text_collapse` (the shared encoder collapses a single *text* part and nothing else, which is the strict rule Copilot asked for) and `bedrock::mime_to_converse_format` (moved to `wire::encode` beside its only caller). Side effect of the same encoder: the Responses input list now carries `function_call` / `function_call_output` items, so the tool history that FR-018 names stops being filtered out. Recorded in `specs/uniformtools/T-006-MESSAGE-ENCODER.md`: `crates/ragent-llm/tests/test_wire_encode_message.rs` (23 cases - one per carrier, the id byte-identical through every declared profile, the absorbed dialects, and two source scans in opposite directions); red-proved six ways (tool_result literal renamed so only the scans could catch it, a `"tool_call_id":` literal re-introduced in `huggingface.rs`, the Gemini id-to-name map bypassed, the declared `args_encoding` ignored, the Converse role policy relaxed, the Responses text-only filter restored), each reverted byte-identically | FR-006 | L | Critical | completed | T-004 |
| T-007 | Replace the Responses-API text-only part filter with a real tool-item encoding path; raise `WireLoss` instead of dropping parts | FR-018 | M | Critical | completed | T-006 |
| T-008 | Add `ToolCallAccumulator` keyed by profile `StreamKeying`, replacing the per-provider block maps in `anthropic.rs`, `bedrock.rs`, `gemini.rs`, `sse.rs`. Delivered as `crates/ragent-llm/src/providers/wire/decode.rs` (`StreamKey::{Index,CallId}`, `from_frame`, `open`/`set_id`/`delta`/`delta_last`/`end`/`end_last`/`end_all`, `defer`/`flush_deferred`): one start and one end per call with the id pinned at start, `ByIndex` closed in index order, `ByCallId` in open order, a delta naming no open call dropped rather than invented, and the `dedup_tool_call_start` opt-out deleted. Converted **nine** streams - the four named plus `copilot.rs`, `huggingface.rs`, `ollama.rs` and `ollama_cloud.rs`, each of which held a copy of the same loop - deleting five distinct open-call representations. The Converse path keeps last-opened attribution via `delta_last`/`end_last` because per-block keys are T-009 (FR-005); `tc_<index>` and Gemini's `fc_<name>_<n>` stay because FR-019/FR-020 are T-010/T-011. `StreamEvent` unchanged, no dependency, no `unsafe`. `crates/ragent-llm/tests/test_wire_decode.rs` (11 cases) incl. a source scan of all nine modules; proven red three times (last-open attribution, dropped started-guard, empty tool name from `sse.rs`). Recorded in `specs/uniformtools/T-008-STREAM-ACCUMULATOR.md` | FR-004 | L | Critical | completed | T-003 |
| T-009 | Fix Bedrock Converse parallel tool calls: replace the single `active_tool_call_id` with per-index tracking. The Converse stream keys each block by `contentBlockIndex` (required on `contentBlockStart`, `contentBlockDelta` and `contentBlockStop`) on all three arms, so two parallel `toolUse` blocks keep their own argument fragments and close in the model's own order; a frame that omits the index is skipped with a warning (FUNC-032) instead of being folded onto the call opened last. The Anthropic-Messages path behind the same provider applies the same rule, and `delta_last`/`end_last` are **deleted** from `wire::decode` so the last-opened guess cannot come back - a helper left in place is the FR-005 failure one call away. `crates/ragent-llm/tests/inline/bedrock_tests.rs` gained the two-case proof (`..._keeps_two_parallel_tool_use_blocks_apart`: four fragments interleaved 1/0/1/0 reassemble to `{"path":"a.rs"}` and `{"pattern":"fn main"}`, two starts, two ends in stop order; `..._drops_a_frame_that_names_no_block_index`), and `test_wire_decode.rs` gained a source scan over five modules for `delta_last`/`end_last`/`open.last_mut()` plus a pure-state Converse case. Proven red three times (all blocks keyed by index 1 - the old single-slot behaviour; a delta ignoring `contentBlockIndex`; `end_last` re-added), each reverted byte-identically. Recorded in `specs/uniformtools/T-009-CONVERSE-PARALLEL-CALLS.md` | FR-005 | M | Critical | completed | T-008 |
| T-010 | Stop recovering tool names by splitting synthetic ids; carry the name on the record and emit `<provider>_<counter>_<name>` ids. Delivered as the accumulator's own id minting: `ToolCallAccumulator::new(profile, provider)` takes the provider id (FR-019's id prefix and the debug line's subject), `synthetic_id` formats `<provider>_<counter>_<name>` from a per-stream counter and logs one `tracing::debug!`, `open` stores the minted id on the call *before* emitting the start so start/delta/end agree, and `defer(name, args)` no longer takes an id at all. `fallback_id` and its `tc_<index>` are deleted; `OpenCall::display_id` now returns `Option<String>` because an unstarted index-keyed call genuinely has none, and a delta reaching one is dropped rather than sent under an invented id. Every provider-side spelling is gone (`gemini.rs`'s `fc_<name>_<n>` and its `next_tool_call_id`, `ollama_cloud.rs`'s `ollama_cloud_tc_<index>` / `ollama_cloud_done_tc_<index>` - which now key id-less calls by a *stream-scoped* counter, since the frame-local `idx` restarted at zero and folded every unnamed call onto the first - and `sse.rs`'s shared fallback, which carries a new `OpenAiSseSpec::provider_id` because a call id is replayed in the next request and must read as an id, not a display label). The name-recovery half is deleted from `wire/encode.rs`: the Gemini `functionResponse` name comes from `tool_names_by_id` only, and an id the history no longer carries goes out unnamed and logged rather than as a split prefix. The name was already on every record (`ToolCallStart { id, name }`, `ContentPart::ToolUse`, `MessagePart::ToolCall`), so no type changed. `test_wire_decode.rs` gained the id-shape case and a ten-module source scan for `format!("fc_`, `format!("tc_`, `_tc_{` and `tool_use_id.split`/`rsplit`/`.id.split('_')`; `test_wire_encode_message.rs`'s orphan-id case was rewritten from asserting the derivation to asserting its absence; `ollama_cloud_tests.rs` gained the two-unnamed-calls case. Proven red three times (the split fallback back in `encode.rs`, `tc_<counter>` back in `synthetic_id`, a `fc_` id re-minted in `gemini.rs`), each restored byte-identically and verified by `md5sum`. Recorded in `specs/uniformtools/T-010-SYNTHETIC-IDS.md` | FR-019 | S | High | completed | T-008 |
| T-011 | Reconcile duplicate call ids within one response: keep first, re-key the rest, warn once per collision. Delivered inside `ToolCallAccumulator`: a per-response `emitted_ids` ledger of every id a start has gone out under, a `start_id(requested, name)` that returns the first claim unchanged and re-keys a later claim to a freshly minted `<provider>_<counter>_<name>` with one `tracing::warn!` naming the duplicated id, and a `synthetic_id` that walks its counter past every id the response has used so the replacement is never itself the duplicate. The warning sits on the start-emitting branch only, so a provider that repeats the whole frame every step takes `open`'s existing `started_id` return and cannot multiply the line (TC-016); a repeated frame under the same *key* stays idempotence rather than a collision, which is why a `ByCallId` family re-keys nothing. The ledger survives `end`, because two sequential calls in one response collide exactly as two parallel ones do, and `synthetic_id`/`mint_for_unnamed` were split so the FR-019 debug line and the FR-020 warning can tell their different stories. The same task folded `azure_resource.rs`'s `AzureAnthropicClient` onto the accumulator - it parsed the Anthropic Messages shape with a private `BTreeMap<u64, String>` and emitted `StreamEvent::ToolCallStart` directly, outside the accumulator, skipping the end of any block the server left unnamed - making it the tenth accumulator site. `test_wire_decode.rs` gained five cases (parallel duplicate, sequential duplicate, a re-key that must skip an occupied synthetic id, a repeated frame that must not be a collision, and a capturing `tracing::Subscriber` counting WARN lines across repeated frames - the only way to see "warn once"), `test_openai_provider_extra.rs` the same collision end to end through the real SSE parser, `test_azure_resource_anthropic.rs` the parallel-block case through `AzureAnthropicClient`, and both source scans now cover `azure_resource.rs`. Proven red four ways, each restored byte-identically (`md5sum`); the third break needed its test rewritten first, which the task note records. Recorded in `specs/uniformtools/T-011-DUPLICATE-CALL-IDS.md` | FR-020 | S | High | completed | T-008 |
| T-012 | Add the shared `decode_tool_arguments` helper (object, JSON-string, whitespace) and route all decode sites through it | FR-007 | M | Critical | completed | T-001 |
| T-013 | Wire the corrective argument-decode error and the schema-violation error into one place that also sets `is_error` | FR-008, FR-009, FR-014 | M | Critical | completed | T-012 |
| T-014 | Unify sequential and parallel execution onto one ordered dispatch pipeline, including the unknown-tool refusal with its did-you-mean hint | FR-012, FR-021 | L | Critical | completed | T-013 |
| T-015 | Gate parallel dispatch on the profile: force sequential and log once when `supports_parallel` is false | FR-010 | S | High | completed | T-014, T-003 |
| T-016 | Block result append while a permission verdict is unresolved | FR-013 | S | High | completed | T-014 |
| T-017 | Propagate `is_error` from `ToolCallState.status` into history and out through `encode_result`, using the `ERROR: ` prefix where the profile lacks native support | FR-014 | M | High | in_progress | T-006, T-013 |
| T-018 | Implement reasoning echo: persist the reasoning signature and replay it for profiles with `echo_reasoning` | FR-011 | M | Medium | pending | T-001, T-006 |
| T-019 | Add the declarative alias table and resolver, applied before the unknown-tool check | FR-015 | M | High | completed | T-001 |
| T-020 | Add `tools.aliases` / `tools.expose_alias_names` / `tools.wire_trace` config keys with merge and default behaviour | FR-015, FR-017 | S | Medium | completed | T-019 |
| T-021 | Honour `expose_alias_names` in outbound tool lists | FR-015 | S | Low | completed | T-020, T-004 |
| T-022 | Add `--wire-matrix` CLI flag printing the per-provider profile table without network access | FR-016 | S | Medium | completed | T-003 |
| T-023 | Add `tools.wire_trace` payload dump to `target/temp/wire-trace/`, redacted, inert by default | FR-017 | M | Low | completed | T-020 |
| T-024 | Treat tool-use finish with zero complete calls as truncation and enter text recovery | FR-022 | S | High | completed | T-008 |
| T-025 | Convert the three `TODO: Replace Value with a typed struct` markers in `ragent-types` into a documented typed-argument decision, or leave them and record why (`specs/uniformtools/T-025-TYPED-ARGS-DECISION.md`) | FR-001 | S | Low | completed | T-001 |
| T-026 | Add recorded-frame replay fixtures (one per provider family) and per-profile encoder/decoder tests | NFR-004 | L | Critical | pending | T-006, T-008 |
| T-027 | Prove no provider builds its own tool JSON: add a source guard scanning `crates/ragent-llm/src/providers/` for hand-built envelope literals | FR-003 | S | Medium | completed | T-005 |
| T-028 | Update provider docs, `SPEC.md` feature bullets and `CHANGELOG.md` for the unified tool path | NFR-005 | S | Medium | completed | T-014 |
| T-029 | Derive the rename set from `tools.xlsx` by the FR-024 two-sheet rule and the FR-025 exact-spelling tie-break; record the 13 accepted pairs and the 2 refused targets (`WebFetch`, `WebSearch`) plus every exclusion reason (`specs/uniformtools/T-029-DERIVATION.md`) | FR-024, FR-025 | S | Critical | completed | — |
| T-030 | Add `crates/ragent-agent/src/tool/rename.rs`: the checked-in table (one `(before, after)` slice, ascending by `after`) and its invariant test - every `before` registered, every `after` unregistered and a valid identifier, both columns unique. The test must not open the workbook: `crates/ragent-tools-core/src/xlsx.rs` is a writer only and adding a reader would breach NFR-002 | FR-023 | S | Critical | completed | T-029 |
| T-031 | Rename the 13 tools at source: change `Tool::name()` and the registry key for `ask_user` (registered via `aliases::AskUserTool` at `tool/mod.rs:1625`), `bash`, `edit`, `glob`, `grep`, `plan_enter`, `plan_exit`, `skill_manage`, the four `task_*` tools and `write`; add no dependency and no `unsafe` block | FR-026, FR-030, NFR-002, NFR-003 | M | Critical | completed | T-030 |
| T-032 | Prove no alias survives the rename: no second registration under a `before` name, no forwarding wrapper or adapter, and a `before` call resolves to the unknown-tool error with its did-you-mean hint at every inbound path (TUI prompt, `ragent run`, the OpenAI-compatible `/v1` surface, ACP) | FR-026 | S | Critical | completed | T-031, T-014 |
| T-033 | Migrate every non-test literal site: `SKILL_ALWAYS_ALLOWED_TOOLS`, `INTERACTIVE_TOOLS` and the `ask_user` special cases in `permissions.rs`, `LOOP_WRITE_TOOLS` and the `ask_user` entry in `loop_state.rs`, the history write-tool list, the `bash` special cases in `processor.rs` / `permissions.rs`, and the in-container routing table in `backend/container.rs` whose arms name `read`, `write`, `glob`, `grep`, `list`, `rm`, `mkdir`, `create` and `bash` by tool name, the legacy alias delegation targets in `tool/aliases.rs`, the repeat-guard keys, the bundled-skill bodies, the agent and codeindex prompt text that says `grep`/`glob`, the eleven `description()` strings that name a tool by its old spelling, and the TUI pretty-printers and summaries in `message_widget.rs` | FR-027 | L | Critical | completed | T-031 |
| T-034 | Add a source guard that fails the build when a renamed `before` name appears as a quoted or backticked literal in non-test source. The guard allows exactly two structural exemptions (the FR-023 table itself and the `CHANGELOG.md` entry) plus a per-line exemption list where each entry carries the reason it is not a tool name; the only entries expected are the permission-type spellings in `crates/ragent-config/src/permission.rs` and the OASF doc list in `crates/ragent-agent/src/agent/oasf.rs`, both of which FR-028 keeps unchanged. Every exemption is reviewed individually - an entry for a tool-name site is a defect | FR-027 | S | High | completed | T-033 |
| T-035 | Keep `permission_category()` values unchanged across the rename, report a hook `matcher` that names only a `before` spelling, replay stored `MessagePart::ToolCall` records verbatim, and make unknown stored names non-destructive in the classifier. Delivered as `HookConfig::retired_matcher_spellings` + `report_retired_matcher_spellings_once` (process-wide per-entry ledger) called at TUI load and at the per-turn hook derivation, with the three already-true duties pinned by `crates/ragent-agent/tests/test_fr028_state_survival.rs` (9 cases: 13 category literals, the 32-value vocabulary, the report and its silence, `bash(git commit:*)` vs `Bash(...)`, a list matcher, verbatim replay for every pair, a storage round trip, and the non-destructive unknown name). Recorded in `specs/uniformtools/T-035-STATE-SURVIVAL.md` | FR-028 | M | High | completed | T-031 |
| T-036 | Resolve Claude-spelled `allowed_tools` entries against the registry with no translation, and report any entry that matches no tool, naming the entry and its declaring skill or agent. Delivered as `crates/ragent-agent/src/tool/allowed_tools.rs` (`unresolved_allowed_tools`, `report_unresolved_allowed_tools_once` with a process-wide per-`(declarer, entry)` ledger, `report_skill_allowed_tools_once`, `unresolved_entry_report`) called from the three places a skill's list becomes a restriction (`skill_manage`, the TUI `/skill` trigger, `invoke_forked_skill`) - not from the per-turn prompt path, which would warn about manifests the session never runs. The resolution path was already exact and is now asserted: `crates/ragent-agent/tests/test_fr029_allowed_tools.rs` (9 cases) proves all 13 adopted spellings resolve, every retired spelling does not, `Read` is reported rather than served by `read`, the report names entry and skill once per declarer, and the `skill_manage` wiring consumes the ledger. Recorded in `specs/uniformtools/T-036-ALLOWED-TOOLS.md` | FR-029 | S | High | completed | T-031 |
| T-037 | Prove the surface is unchanged: same registered count, and `--dry-run`, `tool_info`, `commands_info`, `/tools list`, `/bug-report` and the outbound provider tool list all agree with the registry. Delivered as two guard files split beside the surfaces they pin: `crates/ragent-agent/tests/test_fr030_surface_agreement.rs` (7 cases - the three registry enumerations, the sorted-name digest `308d92d9...` with the 13-uppercase check against the FR-023 `after` column, `--dry-run` human and JSON with the whole 21-row family table plus the `agent` 3 / `team` 20 canaries, `tool_info`, `commands_info`, and one real turn's outbound `ChatRequest` compared against `definitions()` with its `ToolsSent` event) and `crates/ragent-tui/tests/test_fr030_surface_agreement.rs` (2 cases - `/tools list`'s header and 148 rendered rows, and `/bug-report`'s written `Tools registered` line). Measured on this tree, not read from the baseline: 148 names, 148 distinct, 13 uppercase, digest `308d92d9d975666cf1f815369e18cc7b457c1c0dcbab004c45f2835100368321`, 21 families summing to 148; applying the 13 FR-023 pairs to the recorded before-list reproduces the same set. The guard caught a real 147-tool surface on its first run - a previous session had left `plan::PlanEnterTool` commented out with a `// RED PROOF` marker, so `EnterPlanMode` was unregistered. Recorded in `specs/uniformtools/T-037-SURFACE-AGREEMENT.md` | FR-030 | M | High | completed | T-031, T-041 |
| T-038 | Update the ~159 test files (770 literal hits) to the new spellings; where a test asserted a tool name, assert the new name rather than relaxing the assertion | FR-026, FR-030 | L | Critical | completed | T-031, T-033 |
| T-039 | Update prompt text and docs to the new spellings and add the `Renamed` before/after table to `CHANGELOG.md`: agent and spec-authored prompts, bundled skill bodies, crate `DOCS.md`, `README.md`, `SPEC.md`, `QUICKSTART.md`, `TUI-QUICKSTART.md` and the `docs/howtos/` pages | NFR-007 | M | Medium | completed | T-031, T-033 |
| T-040 | Prove the rename adds no per-step cost: names stay borrowed `&str` comparisons, no dispatch step consults the FR-023 table, and the definition cache and repeat guard key on the new spelling. Delivered as `crates/ragent-agent/tests/test_nfr006_rename_cost.rs` (6 cases): a **function-granular** source scan (a file-granular exemption would permit a rename lookup inside `resolve_inbound_tool_name`, the per-call resolver, because `alias_table.rs` also holds the legitimate once-per-turn `from_aliases`), which reports `file:line: (fn) text`, skips comment lines, and runs in both directions so the 13-entry list cannot rot into a superset - it caught a real dead entry on the first run; plus the runtime halves (the adopted spelling is the registry key with `get(before)` `None` and the folded fallback answering with the registered spelling, `definitions()` sharing one sorted `Arc` of adopted names while the serialised cache fingerprints the name so `Bash` and `bash` are two entries, and the repeat guard accumulating five `Bash` calls and denying the sixth while `bash`/`Bash` alternating never accumulates). Red-proofed five ways, each restored byte-identically and verified by `md5sum`: a table read in the per-call resolver, one in the guard's key hash, a content-blind cache key, never serving the definition `Arc`, and an `owned_after_of -> Option<String>` helper; making `Rename::after_of` return `Option<String>` also fails to compile, which is the borrowed-name clause from the test side. No production file changed. Recorded in `specs/uniformtools/T-040-RENAME-COST.md` | NFR-006 | S | Low | completed | T-031 |
| T-041 | Capture the pre-rename surface baseline before T-031 starts - the `--dry-run` total, the per-family counts, the `/tools list` names and the `/bug-report` total - into the change description, so T-037 compares against a recorded number rather than a recollection. Recorded in `specs/uniformtools/T-041-BASELINE.md`: 148 tools, 21 families summing to 148, the 148-name list with digests, and 0 names carrying an uppercase letter before the rename against 13 after it | FR-030 | S | High | completed | — |
| T-042 | Reject a retired spelling in `tools.aliases` when the config loads: a source key or a target value equal to a `before` name - including the self-mapping form - is a load error naming the file, the spelling and its `after` replacement, and the resolver never consults a `before` entry; an alias target that names no currently registered tool is reported once as unresolved and refused when used. Delivered as `ToolsConfig::validate_alias_spellings` (+ `RetiredAliasSpelling`) called from `Config::parse_file` and the `RAGENT_CONFIG_CONTENT` route, and as the retired-entry arm plus `unresolved_targets` / `report_unresolved_targets_once` in `AliasTable`. The FR-023 table moved to `crates/ragent-types/src/tool_rename.rs` (re-exported as `ragent_agent::tool::rename`) so `ragent-config` can read it without depending on `ragent-agent`. Recorded in `specs/uniformtools/T-042-RETIRED-ALIAS-REFUSAL.md` | FR-031 | S | Critical | completed | T-020, T-030 |
| T-043 | Audit every remaining escape hatch for a retired spelling - config keys, environment variables, CLI flags, and plugin, connector and agent settings - and record in the change description that no key, variable or flag can restore a `before` name; add none. Recorded in `specs/uniformtools/T-043-ESCAPE-HATCH-AUDIT.md`: 339 `ragent.json` keys and serde spellings, 67 environment reads over 35 names, 79 CLI names, 387 plugin/connector/agent/hook/skill settings, and the built-in table - one reasoned exemption (`bash.allowlist`'s section in `config.rs`), nothing added; the seven predicates now run as `crates/ragent-agent/tests/test_escape_hatch_audit.rs` (8 cases) | FR-031 | S | High | completed | T-030 |
| T-044 | Keep dynamic registrations namespaced and refuse a bridge whose surfaced name equals a `before` spelling: MCP (`<server>.<tool>` and `mcp_tool`), plugin tools, connector servers and skill-contributed tools; report the refusal once and leave the session usable. Recorded in `specs/uniformtools/T-044-DYNAMIC-BRIDGE-REFUSAL.md`: one rule in `crates/ragent-types/src/tool_bridge.rs` (`check_dynamic_bridge` + `report_bridge_refusals_once`, plus the MCP and plugin manglings the three call sites now share), one MCP bridge (`ragent_agent::tool::mcp_tool::bridge_server_tools`) used by `set_mcp_client` and the TUI's `register_mcp_tools`, and a FR-032 check in `PluginManager::register_plugin_tools`; 16 new test cases across `ragent-types` (9), `ragent-agent` (5, all `#[serial]` on the process-wide ledger) and `ragent-plugins` (2); proven red by disabling the prefix clause, pinning the retired-spelling lookup, removing the once-ledger's `continue`, and un-namespacing `plugin_tool_name`; 148 tools and 21 families unchanged | FR-032 | M | High | completed | T-031 |
| T-045 | Derive every surface that offers a tool name from the registry - the slash-command and tool-name menus, the did-you-mean candidate pool, and the lists printed by `--dry-run`, `tool_info` and `commands_info` - so no retired spelling can be completed; add no new completion surface where none offers tool names today. Recorded in `specs/uniformtools/T-045-COMPLETION-SURFACES.md`: the `/tools` menu, its parameter hint, the `/tools` handler and the `commands_info` mirror had four hand-written copies of one list whose menu offered seven forms the handler rejects and omitted `show` it accepts - `command_catalog::TOOLS_SUBS` is now `pub` and is the one source all four read (help page and usage warning too); 10 new test cases (`crates/ragent-agent/tests/test_fr033_completion_sources.rs` 6, `crates/ragent-tui/tests/test_fr033_completion_surfaces.rs` 4) drive `suggest_tool_name`, `tool_info`, the `--dry-run` tools section, the prompt tool reference and the `/tools` menu with a **foreign** registry so a hard-coded surface fails, assert no answer is a retired spelling and every answer resolves, and pin that the `@` file picker offers paths not tool names (no new surface); `/bash` is the one retired spelling that is also a command and the guard pins that exemption; proven red five ways (menu reverted, hint reverted, `tool_info` reading `create_default_registry()`, `--dry-run` total pinned to 148, suggestion pool drawn from the FR-023 table); 148 tools and 21 families unchanged | FR-033 | M | High | completed | T-031 |
| T-046 | Land the source rename as one atomic step: after the change registers `after`, the same commit removes `before`, so no commit resolves both spellings or neither and a bisect never lands on a half-renamed tree. Recorded in `specs/uniformtools/T-046-ATOMIC-LANDING.md`: `crates/ragent-agent/tests/test_tool_rename_atomic_landing.rs` (7 cases) asserts the live registry resolves one `after` spelling per pair across `list`/`all_tools`/`definitions`, refuses every `before` by lookup and key, agrees element for element, stores each key under the name its tool reports, keeps 21 families (`agent` 3 / `team` 20) and the FR-030 total of 148; proven red on all three half-renamed states; the tree measures 148 tools, 13 uppercase names, digest `308d92d9...` and a 26-name symmetric difference that is exactly the table; the scoped `git log` query returns zero commits for all thirteen `after` spellings, so the landing is one commit once T-033 and T-038 clear FR-034 clause 2 (44 remaining `before` literals in tests, `has_local: false`) | FR-034 | S | Critical | completed | T-030, T-041 |
| T-047 | Extend the FR-027 guard into the NFR-008 no-alias check and implement all seven named predicates: (a) a `before` key or value in the alias table, (b) a `before` resolver match arm, (c) a `before` default configuration value, (d) a `before` registry key, (e) a `before` non-test literal (the existing T-034 scan), (f) a duplicate registration, (g) a tool that delegates wholly to another registered tool. Structural exemptions stay the FR-023 table and the `CHANGELOG.md` entry, every per-line (e) exemption keeps its written reason, and one run reports every offending file and line for every predicate | NFR-008 | M | High | pending | T-034, T-042 |
| T-048 | Ship the NFR-008 check as one implementation usable both as a `cargo test` case and as `scripts/check-tool-rename.sh` beside the existing `check-*` guards: repository files only, no network request, no `tools.xlsx` read, a few seconds on a full run, and ASCII failure messages naming the predicate letter and the exact edit | NFR-009 | S | Medium | pending | T-047 |
## Sequencing

Three groups. Pass A is an ordinary refactor chain; Pass B is a rename that the
compiler cannot check; Pass C is the mechanical refusal of the routes that would
undo it.

Pass A:

1. **Types and contract** - T-001 -> T-002 -> T-003.
2. **Encoding and decoding** - T-004 -> T-005 -> T-006 -> T-007, and
   T-008 -> T-009 -> T-010 -> T-011.
3. **Dispatch and behaviour** - T-012 -> T-013 -> T-014 -> T-015 / T-016 ->
   T-017, with T-018 and T-019 parallelisable after T-006.
4. **Surface and proof** - T-022, T-021, T-023, T-024, then T-026 and T-027 as
   the gate, and T-028 to close.

Pass B:

5. **Baseline and derivation** - T-041 and T-029 can run on day one. T-041 must
   finish before T-031 edits a single tool.
6. **Table then rename** - T-030 -> T-031. This ordering cannot move: the table
   exists before anything reads it, so the invariant test can fail first and
   prove the derivation rather than the implementation.
7. **Migration and proof** - after T-031, in parallel: T-032, T-033 -> T-034,
   T-035, T-036, T-037, T-040. Then T-038 (tests) and T-039 (docs).
8. **Close-out** - T-028 last, because it edits the same feature bullets the
   rename renames.

Land Pass B as its own change if at all possible. A diff that renames 13 tools
across 78 non-test files and also rewrites provider encoders is unreviewable,
and a reviewer who cannot read the diff will approve it.

Pass C (the no-alias guarantee) brackets Pass B rather than following it:

9. **Before the rename lands** - T-042 and T-043 start once T-030 commits the
   table, because rejecting a retired alias is a config-loader change that needs
   only the list, not the renamed registry. T-046 is the landing rule for T-031
   itself: the commit that registers `after` is the commit that removes
   `before`.
10. **After the rename lands** - T-044 (dynamic bridges) and T-045 (completion
    surfaces) read the renamed registry, then T-047 extends the T-034 guard and
    T-048 bounds its cost. T-047 is the only task in this plan that must land
    after both T-034 and T-031, so it closes the pass.

A tree that has run T-042 and T-046 but not T-047 still relies on review to keep
the alias table honest. Treat T-047 as part of the same change as T-031, not as
a follow-up - a rule enforced only until the next commit is not enforced.

The full dependency graph, restricted to the tasks that gate each other:

```
Pass A
  T-001 -> T-002 -> T-003 -> T-004 -> T-005 -> T-027
                            |          +-> T-006 -> T-007
                            |                 |  +-> T-017
                            |                 |  +-> T-018 (also T-001)
                            |                 +-> T-026 (also T-008)
                            +-> T-008 -> T-009, T-010, T-011, T-024, T-026
                            +-> T-015 (also T-014), T-022
  T-001 -> T-012 -> T-013 -> T-014 -> T-015, T-016, T-028
                                   |     +-> T-017
                                   +-> T-032 (also T-031)
  T-001 -> T-019 -> T-020 -> T-021 (also T-004), T-023, T-042
  T-001 -> T-025

Pass B
  T-041 -------------> T-037 (also T-031), T-046 (also T-030)
  T-029 -> T-030 -> T-031 -> T-032, T-033, T-035, T-036, T-037, T-040,
                                  T-044, T-045
                       |     +-> T-038 (also T-033), T-039 (also T-033)
                       +-> T-042, T-043, T-046

Pass C
  T-033 -> T-034 -> T-047 -> T-048
  T-042 -----------------> T-047
```

Three roots - T-001, T-029 and T-041 - can all start on day one. Every other
task has at least one dependency. T-047 is the only task with parents in both
Pass B and Pass C, which is why it is the last to land before the guard script
is published, and T-048 is the only task that depends on it.

## Rename Derivation

The rename set is not a judgement call; `tools.xlsx` decides it. Both sheets it
reads are present in the workbook as delivered, so the table in `SPEC.md`
FR-023 is reproducible from the file alone:

```
Anthropic Tools sheet          ragent Tools sheet
  Tool == after                  Tool == before
  Mapping == Exact               Mapping == Exact
                                 Anthropic equivalent names after
            \                    /
             \                  /
              -> pairwise candidates -> resolve collisions (FR-025)
```

Run against the delivered workbook this yields 15 pairwise candidates, of which
13 are renamed and 2 are refused:

| Anthropic target | ragent claimants | Outcome |
|---|---|---|
| `AskUserQuestion` | `ask_user` | rename (one claimant) |
| `Bash` | `bash`, `bash_reset` | rename `bash` (exact spelling); `bash_reset` keeps its name |
| `Edit` | `apply_patch`, `edit`, `patch` | rename `edit` (exact spelling); the other two keep their names |
| `EnterPlanMode` | `plan_enter` | rename |
| `ExitPlanMode` | `plan_exit` | rename |
| `Glob` | `glob` | rename |
| `Grep` | `grep` | rename |
| `Skill` | `skill_manage` | rename |
| `TaskCreate` | the four `task_*` tools | rename `task_create` (exact spelling) |
| `TaskGet` | the four `task_*` tools | rename `task_get` (exact spelling) |
| `TaskList` | the four `task_*` tools | rename `task_list` (exact spelling) |
| `TaskUpdate` | the four `task_*` tools | rename `task_update` (exact spelling) |
| `WebFetch` | `mf_fetch`, `webfetch` | **refused** - neither name equals `WebFetch` |
| `WebSearch` | `mf_search`, `websearch` | **refused** - neither name equals `WebSearch` |
| `Write` | `create`, `update_file`, `write` | rename `write` (exact spelling); the other two keep their names |

Excluded before the collision step, because a clause of FR-024 fails:

| Candidate | Clause that fails |
|---|---|
| `new_agent` -> `Agent` | `Anthropic Tools` rates `Agent` `Close`, not `Exact` |
| `read` -> `Read` | `Anthropic Tools` rates `Read` `Close`, not `Exact` |
| `cron_add` -> `CronCreate` | the `ragent Tools` row for `cron_add` is `Close` |
| `cron_remove` -> `CronDelete` | the `ragent Tools` row for `cron_remove` is `Close` |
| `cron_list` -> `CronList` | the `ragent Tools` row for `cron_list` is `Close` |
| (none) -> `TodoWrite` | no `ragent Tools` row names `TodoWrite` |

The five `cron_*` tools are listed together above because only three Anthropic
targets survive the first sheet; `cron_enable` and `cron_disable` fail the same
ragent-screen clause and no Anthropic target claims them either.

T-029 records this derivation in `specs/uniformtools/T-029-DERIVATION.md`,
including the per-clause accounting that proves the 15 candidates reduce to
these 13 pairs and 2 refusals; T-030 commits the 13 pairs, T-038 carries the
test churn. Note the scale before starting: a quoted-or-backticked scan of the
13 retired names finds 381 occurrences in 77 non-test source files, plus 773
in 160 test files. Every non-test occurrence is a decision - rename it, or
justify it in the guard's exemption list with a comment - and none is a
mechanical substitution, because `bash` also names the shell and `read` also
names an action.

## Risk Notes

- **T-006 and T-014 are the highest-risk Pass A tasks.** Both touch paths that
  every provider and every turn crosses. Land them behind the recorded-frame
  fixtures from T-026 (write the fixture first, then the encoder) so a
  regression is localised to one profile rather than one model.
- **T-009 is a live bug fix**, not a refactor. Bedrock users running two tool
  calls in one response currently lose one call's arguments. Verify with a
  recorded frame carrying two interleaved `toolUse` blocks before touching the
  rest of the accumulator.
- **T-015 must not silently change behaviour for providers that do support
  parallel calls.** The gate only forces sequential execution; it never enables
  concurrency that the user did not already ask for.
- **T-019's alias table must stay exact-match.** Fuzzy matching here would let a
  near-miss name reach a destructive tool. The existing did-you-mean hint stays
  a hint.
- **T-019 and T-031 collide at the table.** The alias table T-019 adds is correct
  only for the names Pass B does not adopt. After T-031 it must be pruned: delete
  every entry that maps a spelling onto the identical string, and delete every
  entry whose target became a ragent name. Do not delete the resolver - the
  OpenAI spellings and the two refused Claude spellings (`WebFetch`,
  `WebSearch`) still need it, and a skill written against them should keep
  working. SPEC decision D7 is the rule; the pruning lands with T-033, and
  TC-028 in the test plan checks it.
- **T-023 writes files.** It must use `target/temp/` and stay behind an
  explicitly-enabled config key, per the workspace rule that a test or default
  run leaves the tree byte-identical.
- **T-031 is the highest-blast-radius task in this plan.** It renames tools that
  several crates key on by string, and the compiler catches none of it. Do not
  start it until T-030's invariant test passes, T-041's baseline is recorded,
  and T-033's migration list is enumerated from a fresh scan, not from memory.
  Expect the rename to break tests that were passing for the wrong reason - if a
  test asserted a name and now fails, update the assertion to the new name;
  never relax it.
- **Do not add a compatibility alias to soften T-031.** FR-026 forbids it and
  the temptation will be strong the first time a stale literal surfaces. A
  single retained alias makes the rename pointless: a Claude skill would keep
  working by accident on the aliased tool and fail on every other, which is
  harder to diagnose than a clean failure.
- **`bash` and `read` are the two names that no longer mean what the prose
  says.** After the rename, `Bash` is the tool and `bash` is the shell; `Read`
  is absent entirely (the workbook rates it `Close`), so `read` stays the tool
  and "read" stays the verb. Every non-test literal has to be read in context. A
  blind substitution corrupts comments, documentation, and - worst - the
  `Command::new("bash")` spawn in
  `crates/ragent-agent/src/session/verification.rs`, which must keep the shell
  spelling.
- **Two string sites are permission vocabulary, not tool names.** `Permission::
  from` accepts `plan_enter` and `plan_exit` as permission-type spellings, and
  `oasf.rs` documents the same list. They name a permission category, which
  FR-028 requires to stay unchanged. Decide once - either keep them and give
  the guard (T-034) an explicit, commented per-line exemption, or drop the
  `plan_enter` spelling and rely on `plan` - and record the decision in the
  change description. Do not let the guard silently allowlist the whole file.
- **The rename is user-visible churn with no user-visible feature.** Anyone with
  a hook, a permission rule written against a tool name, or a habit of typing
  the old spelling in a prompt is affected. The `CHANGELOG.md` `Renamed` table
  (T-039) is not optional politeness; it is the migration note.
- **The guard (T-047) must fail on the alias table, not only on source
  literals.** T-034's scan catches a `before` name written in Rust; it does not
  catch one written in a JSON config default or a YAML skill manifest that the
  code loads at run time. T-042 covers the config path by refusing the entry at
  load; T-047 covers the default tables by reading them, so a shipped default
  alias of `bash` -> `Bash` fails the build rather than surviving in a file
  nobody reads. Add a fixture for each NFR-008 predicate: a `before` name in the
  alias table, in a resolver match arm, in a default configuration value, in a
  registry key, in a non-test source literal (the T-034 case), a second
  registration of one name, a tool whose `execute` only forwards to another
  registered tool, an alias entry whose source is a `before` name, an alias
  entry whose target is a `before` name, and an alias entry mapping a spelling
  onto the identical string. Each fixture asserts the guard names the file and
  line, and the self-mapping fixture asserts a failure even though it maps no
  retired name - a no-op entry is dead weight the resolver should not carry.
- **T-047 is not a follow-up.** If it lands after T-031 in a separate change,
  the window between them has no mechanical protection and the alias table is
  whatever the reviewer of T-031 believed. Land T-042, T-046 and T-047 in the
  same change as T-031; T-043, T-044, T-045 and T-048 may follow.
- **A refused dynamic bridge must not take the session down.** T-044 refuses an
  MCP or plugin tool whose name equals a retired spelling, and the refusal is a
  real loss of a capability the user installed. Report it once, name the server
  or plugin and the `after` spelling, and keep every other tool from that source
  working. Refusing the whole server because one of its tools collides punishes
  the wrong party.
- **Pass C must not lengthen the hot path.** T-042's check runs when the
  configuration loads, T-045's registry derivation replaces a static list rather
  than adding a filter to it, and T-044's namespacing happens once per bridged
  tool at registration. Nothing in Pass C is consulted per tool call, which is
  what keeps NFR-006 true after the rename.