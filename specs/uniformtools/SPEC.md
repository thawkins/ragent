---
status: draft
audit:
  - { time: 1793052935, from: "none", to: "draft", actor: "system" }
---
# Specification: Unified Tool Calling Across Providers, Mapped to ragent Tools

## Executive Summary

ragent already routes every provider through one canonical request/response
type (`ChatRequest` / `ChatContent` / `ContentPart`) declared in
`crates/ragent-types/src/llm.rs`. The abstraction is real but **leaky**: each
provider family re-implements tool-definition serialisation, tool-call stream
parsing, tool-result encoding, and tool-call-id handling in its own module, and
at least one provider silently discards tool history. The result is that the
same ragent tool behaves differently depending on which model the user picked,
and adding a provider means re-deriving the whole tool-calling contract from
scratch.

This specification defines the **unification layer**:

1. **One declared wire contract per provider** (`ToolWireProfile`) instead of
   implicit conventions scattered across 30 provider modules.
2. **One tool-definition encoder** keyed by wire profile, replacing the
   per-provider copies (and closing the one provider that bypasses the existing
   cache entirely).
3. **One tool-call accumulator** that tolerates index-keyed deltas (Anthropic,
   OpenAI chat-completions) and id-keyed deltas (OpenAI Responses), and
   distinguishes parallel calls instead of relying on a single mutable slot.
4. **One tool-result encoder** that emits the provider-appropriate carrier
   (`role: "tool"` message, `tool_result` content block, `functionResponse`,
   `function_call_output`) from the same history record.
5. **One dispatch pipeline** shared by the sequential and parallel execution
   paths, with arguments parsed and schema-validated exactly once, before any
   guard, hook, permission check, or backend call.
6. **An explicit provider-name to ragent-name alias table**, so a model or an
   ACP/MCP client that names a tool with its OpenAI or Claude Code spelling
   (`Bash`, `Read`, `Write`, `Edit`, `Glob`, `Grep`, `WebFetch`, `web_search`,
   `file_search`) resolves to the corresponding ragent tool.
7. **Explicit, non-silent failure** where a provider cannot carry a canonical
   construct, replacing today's silent drops.
8. **One rename pass over the tool surface** (FR-023 to FR-030): where
   `tools.xlsx` records a ragent tool and a Claude Code tool as an exact match
   one-to-one, the ragent tool takes the Claude spelling - `bash` becomes
   `Bash`, `edit` becomes `Edit`, `write` becomes `Write`, and ten others
   follow - with no alias kept behind. A Claude skill that names `Bash`, `Edit`
   or `Write` then resolves because the registry genuinely calls the tool that,
   not because a translation layer guessed. This is the reason the feature
   exists: a skill ported from Claude Code should not need editing.
9. **A retirement that cannot be undone by configuration** (FR-031 to FR-034).
   The old spelling is not merely absent from the registry: a config key, an
   environment variable, a CLI flag, a bridged MCP or plugin tool, or a
   completion list that reintroduced it would turn the rename back into an
   alias by another route. Each of those routes is refused, and a guard fails
   the build if one comes back.

The scope is the existing `ragent-types`, `ragent-llm`, `ragent-agent` and
`ragent-config` crates. No new external dependency is introduced.

---

## Current State

Facts verified in the tree at the time of writing.

| Layer | Location | What exists |
|---|---|---|
| Canonical types | `crates/ragent-types/src/llm.rs` | `ChatMessage`, `ChatContent`, `ContentPart{Text,ToolUse,ToolResult,ImageUrl}`, `ToolDefinition{name,description,parameters}`, `StreamEvent{...ToolCallStart,ToolCallDelta,ToolCallEnd...}`, `ChatRequest` |
| Client trait | `crates/ragent-llm/src/llm.rs:36` | `LlmClient::chat(&self, ChatRequest) -> Result<Stream<StreamEvent>>` |
| History | `crates/ragent-types/src/message/mod.rs` | `Message`, `MessagePart{ToolCall{tool,call_id,state}}`, `ToolCallState{status,input,output,error,duration_ms}` |
| Tool trait | `crates/ragent-tools-core/src/lib.rs:170` | `name`, `description`, `parameters_schema`, `permission_category`, `async execute(Value, &ToolContext) -> Result<ToolOutput>` |
| Registry | `crates/ragent-agent/src/tool/mod.rs:1389,1481,1558` | `register`, `definitions() -> Arc<Vec<ToolDefinition>>` (sorted, cached), `create_default_registry()` |
| Definition cache | `crates/ragent-llm/src/providers/tool_cache.rs` | `ToolFormat{OpenAi,Anthropic,Gemini,Bedrock,HuggingFace}`, `cached_tools`, `invalidate_tool_cache` |
| OpenAI-family body | `crates/ragent-llm/src/providers/openai_compat.rs` | `OpenAiCompatSpec`, `base_body`, `with_messages`; emits `tool_calls` + `role:"tool"` |
| OpenAI-family stream | `crates/ragent-llm/src/providers/sse.rs` | `OpenAiSseSpec`, `parse_openai_sse_stream` |
| Argument validation | `crates/ragent-tools-core/src/schema.rs` | `validate_required_args(schema, input)` |
| Refusal values | `crates/ragent-agent/src/session/tool_input_error.rs` | `ToolInputError{Undecodable,SchemaViolation,UnknownTool}` - the sentence, the record status and therefore the `is_error` flag of a call refused before it ran (FR-008, FR-009, FR-014; D8) |
| Dispatch | `crates/ragent-agent/src/session/processor.rs:305` | `dispatch_tool_with_permissions`, `dispatch_tool_leaf`, `schema_violation` |
| Loop | `crates/ragent-agent/src/session/loop_steps.rs` | `LlmStepResult{tool_calls: Vec<PendingToolCall>}`, event accumulation |
| Permissions | `crates/ragent-agent/src/session/permissions.rs` | `PermissionAction{Allow,Deny,Ask}`, 120 s prompt timeout, YOLO bypass, `TOOL_REPEAT_LIMIT = 5` |
| Tool config section | `crates/ragent-config/src/tools.rs` | `ToolsConfig{aliases, expose_alias_names, wire_trace}` behind `Config::tools`; `AliasesConfig` as a five-field four-state value (absent / `true` / `false` / object) with `is_unspecified`, `is_enabled`, `uses_builtin_table`, `entries`, custom serde in both directions, plus `ToolsConfig::merge` and `is_default`; every switch off and the whole section omitted from a serialised config while default (FR-015, FR-017; T-020) |
| Escape-hatch audit | `crates/ragent-agent/tests/test_escape_hatch_audit.rs` | Seven predicates over the whole tree (config field and serde names, environment-variable names, the resolution path's purity, CLI flag/alias/subcommand names, the `tools` key set, plugin/connector/agent/hook/skill settings, the built-in alias table), each with an inspection floor, plus the pinned one-entry `EXEMPTIONS` list; the evidence lives in `T-043-ESCAPE-HATCH-AUDIT.md` (FR-031; T-043) |

### Divergences that this specification closes

Each row was confirmed by reading the named file.

1. **Tool-definition envelopes are duplicated.** `tool_cache.rs` builds five
   envelopes but `providers/openai_responses.rs:311` builds its own `tools`
   array inline and never calls `cached_tools`, so its payload is not covered
   by the cache or by any cache-correctness test.
2. **A provider discards tool history.** `providers/openai_responses.rs:287`
   maps message parts through a filter that keeps only `ContentPart::Text`
   (`_ => None`), so on the Responses API every prior `ToolUse` and
   `ToolResult` is dropped from the request. Multi-turn tool use on that path
   is broken and nothing reports it.
3. **Tool-call ids are provider-shaped.** Anthropic supplies `tool_use.id`;
   OpenAI chat-completions supplies `tool_calls[].id`; Gemini supplies none, so
   `providers/gemini.rs` invents `fc_<name>_<n>` and later recovers the tool
   name by splitting that synthetic id on `_`. A tool whose name contains an
   underscore breaks the recovery.
4. **Parallel tool calls are only partly handled.** Anthropic keys open blocks
   by SSE content-block index (`BTreeMap<u64, String>`, `anthropic.rs:423`), but
   Bedrock Converse tracks one `active_tool_call_id: String` (`bedrock.rs:986`),
   so two concurrent `toolUse` blocks in one stream overwrite each other.
5. **Error results are unrepresentable.** `ContentPart::ToolResult` has no
   error flag, so Anthropic's `is_error` is never sent and OpenAI's lack of one
   is never compensated with a textual marker.
6. **Reasoning is dropped from history.** `parts_to_chat_content` and its sync
   twin skip `MessagePart::Reasoning` outright (`history.rs:883`, `:924`).
   Anthropic extended thinking requires the signed thinking block to be echoed
   when the turn continues into tool use.
7. **`SharedChatRequest` is misnamed.** It is an `Arc`-sharing handle for
   request bodies (an allocation optimisation), not a request normaliser, and
   it normalises nothing.

---

## Design Decisions

### D1 - The canonical form stays `ContentPart`; no second model is introduced

`ContentPart` is already the provider-neutral vocabulary every encoder matches
on. The fix is to complete it, not to replace it:

- add `is_error: bool` to `ToolResult`;
- add an optional `reasoning` echo channel (`ContentPart::Reasoning {
  text, signature: Option<String> }`) that is populated from persisted history
  and read only by providers whose profile declares `echo_reasoning`.

Rationale: a second parallel representation would need a translation on every
hop, and every translation is a place to lose the tool-call id.

D1 keeps the record's JSON **untyped**: `ToolCallState::input`/`::output`
(`ragent-types/src/message/mod.rs`), `ContentPart::ToolUse::input` and
`ToolDefinition::parameters` (`ragent-types/src/llm.rs`) all stay
`serde_json::Value`. FR-001's "decoded JSON `input`" is a decode guarantee, not
a type guarantee: the arguments shape is owned by the callee, and the callee set
is open at run time (148 built-in tools today, plus every MCP server and plugin
registered after the binary was compiled), so no compile-time type can describe
it. `ToolUse::input` is replayed to the provider every step, so a fixed field set
would drop keys it does not model (FR-006, FR-018), and
`ToolDefinition::parameters` is forwarded third-party JSON Schema that a typed
subset could only narrow (FR-003) or that would need a schema crate (NFR-002
forbids one). The three former `TODO: Replace Value with a typed struct` markers
are resolved by this decision and recorded in full in
`T-025-TYPED-ARGS-DECISION.md`, which the guard
`crates/ragent-types/tests/test_typed_args_decision.rs` pins.

### D2 - `ToolWireProfile` is the single source of truth per provider family

One `const` profile per wire family, listing:

| Field | Meaning |
|---|---|
| `tool_envelope` | `OpenAiFunction` / `AnthropicTool` / `GeminiFunctionDeclaration` / `BedrockToolSpec` / `ResponsesFlatFunction` |
| `result_carrier` | `ToolRoleMessage` / `UserToolResultBlock` / `GeminiFunctionResponse` / `ResponsesFunctionCallOutput` |
| `id_field` | the JSON key the provider expects for the call id |
| `args_encoding` | `JsonString` (OpenAI) or `JsonObject` (Anthropic, Gemini) |
| `stream_keying` | `ByIndex` or `ByCallId` |
| `supports_parallel` | whether the loop may dispatch concurrently |
| `supports_is_error` | whether `is_error` is transmitted |
| `echo_reasoning` | whether signed reasoning must be replayed |
| `synthetic_ids` | whether ids must be generated when the provider omits them |

A provider with no profile does not compile (the profile is a required field of
its client constructor). The type itself carries no `Default` impl, no optional
field and no serde attribute: a client that states no profile has no fallback
contract to inherit, which is what makes FR-002's coverage assertion a
build-time guarantee rather than a convention.

### D3 - One encoder, four call sites

`ToolWireProfile` drives a single `encode_tools(profile, &[ToolDefinition])`,
a single `encode_message(profile, &ChatMessage)`, and a single
`decode_tool_events(profile, frame)`. The existing `tool_cache.rs` builders are
folded into `encode_tools`; `openai_responses.rs` stops building its own array.

### D4 - Arguments are decoded once, in the harness, not in the provider

Providers emit `ToolCallDelta{args_json}` fragments verbatim. The harness
accumulates them and calls one `decode_tool_arguments` helper
(`ragent_types::tool_args`, T-012) that accepts a JSON object, or a JSON string
that parses to an object, or empty/whitespace (which means `{}`). Anything else
is a corrective, LLM-visible error and the tool does not run. The helper is
reachable from `crates/ragent-types/src/lib.rs` and every decode site in the
loop, the hook layer and the TUI routes through it; the provider layer names it
nowhere, and `crates/ragent-agent/tests/test_tool_arg_decode_routing.rs` fails
the build when a new bare `serde_json::from_str` appears on argument text.

### D5 - Alias resolution is declarative and bidirectional

A static table maps provider spellings to ragent tool names. It is used
inbound (a model or client names `Bash`; ragent resolves it to `bash`) and
outbound when `tools.expose_alias_names` is enabled (ragent advertises `Bash`
in addition to `bash`). Unmapped names are never guessed: an unmapped name is
an unknown-tool error carrying the did-you-mean hint that
`processor.rs` already builds.

### D6 - Silent loss becomes a hard, logged refusal

Where a profile cannot carry a canonical construct, the encoder returns
`Err(WireLoss{construct, provider})` rather than dropping it. The loop turns
that into a turn-level error naming the construct and the provider. This is the
behaviour the Responses-API tool-history gap needs.

### D7 - D5's alias table is narrowed by the rename, and the rename wins

D5 remains correct for the names this feature does **not** adopt, but its worked
example is now wrong: after FR-023 to FR-030 the registry's tool for Claude's
`Bash` is itself called `Bash`, so no inbound translation is needed for it and
none is permitted (FR-026 forbids an alias for a renamed tool).

The alias table therefore keeps only the entries whose provider spelling is
**not** a ragent tool name after the rename - the OpenAI spellings
(`web_search`, `file_search`, `code_interpreter`) and any Claude spelling that
FR-024 or FR-025 refused (`WebFetch`, `WebSearch`). Where an entry would map a
spelling onto the identical string, it is deleted rather than kept as a no-op.

Two consequences follow, and both are requirements:

- the renamed set resolves **exactly**, with no table lookup, which is the
  cheapest and most reliable possible match (NFR-006);
- `tools.expose_alias_names` (FR-015) can no longer advertise a second spelling
  for a renamed tool, because FR-026 forbids the second spelling existing at
  all. It advertises only the remaining unmapped provider spellings.

### D8 - A pre-execution refusal is one value, not three decisions

FR-008 and FR-009 both refuse a call before it runs, and both must produce a
result that (a) says the tool did not run, (b) names the tool, (c) states the
cause in the model's own terms, and (d) is *marked* as an error (FR-014). Four
obligations per cause, two causes, and nothing in the type system keeping them
aligned - so before this specification each gate decided all four separately and
they drifted: a decode failure was formatted one way and a schema violation
another, and the `is_error` flag was derived again at every append site.

`crates/ragent-agent/src/session/tool_input_error.rs` makes the refusal itself
the unit: one `ToolInputError` value owns its model-facing sentence (via
`Display`), its record status, and - through `ToolCallStatus::is_error` - the
result flag, so a gate cannot state a refusal one way and record it another. Both
loop gates build that value: the decode gate in the pre-dispatch position, and
the schema gate in the in-task position through `into_anyhow()`, because the
task's single error exit already reformats an error into the result text and
status. `is_error` is then read once, at the one append site, from the status the
refusal named.

The module is also where FR-009's full sentence is delivered: the violation
names the offending parameter **and its declared type**, which
`ragent-tools-core`'s validator states directly rather than leaving each caller
to look the type up from the same schema a second time.

Declared but uncompiled is the worst of both positions - the invariant looks
enforced and is not. This module existed for a commit before `T-013` declared it,
and the moment its own contract cases could run one of them failed, because the
validator's message did not yet carry the declared type.

### D9 - The two optional features are configured by one section, and silence is the default

FR-015 and FR-017 are both conditional on configuration, which makes the
*default* the requirement: with no `tools` section, everything behaves exactly as
it did before this specification. That is a claim about the file on disk as much
as about control flow, so the section is omitted while every switch is at its
default and each key is omitted while it is at its own; a build that has never
seen `tools` writes a byte-identical `ragent.json`, and the first unrelated
`save_to_source` cannot introduce a key the user never wrote.

`tools.aliases` is a four-state value, not a boolean: absent (this layer has no
opinion), `true` (use the built-in provider-spelling table), `false` (switch
resolution off and drop inherited entries), and an object (contribute these
entries). `{}` is an object, not the `false` switch - collapsing the two would
let a project file that merely declares an empty table wipe the user-global one
and would rewrite the author's `{}` as `false`. The merged rules follow from the
kinds of value: the two opt-in booleans OR (so a layer that enables one cannot be
silently switched off by a layer that omits the key), entries union per spelling
with the overlay winning a collision (so one project entry does not discard the
user's table), and only an explicit `false` clears.

The section is **not** stripped from an untrusted project overlay. An alias
resolves a name, not a capability: after resolution the call runs through the
same unknown-tool check, loop restriction, destructive classifier, hooks,
permission layer and bash validation as a call the model spelled canonically, and
a registered name always wins so no entry can shadow or re-point a tool. The
refusal FR-031 needs is a per-entry rejection of a retired spelling at load time,
which is why it lives in the section's own validator rather than in
`merge_project`.

---

## Requirements

### FR-001 - One canonical tool-call record (ubiquitous)

The system **shall** represent every tool invocation, from every provider, as
exactly one canonical record carrying a non-empty `id`, a `name`, and decoded
JSON `input`, and **shall** store that record in conversation history via
`MessagePart::ToolCall { tool, call_id, state }` without any provider-specific
field, key name, or envelope leaking into the persisted form.

> *Ubiquitous requirement - holds for every provider, every turn, and every
> persisted session.*

### FR-002 - Declared wire profile per provider family (ubiquitous)

Every LLM client **shall** declare a `ToolWireProfile` covering tool envelope,
result carrier, id field, argument encoding, stream keying, parallel support,
`is_error` support, reasoning echo, and synthetic-id need. A client constructed
without a profile **shall** fail to compile. A test **shall** assert that the
set of profiles covers every registered provider id.

> *Ubiquitous requirement - a provider cannot exist in the build without
> declaring its tool-calling contract.*

### FR-003 - One tool-definition encoder (ubiquitous)

The system **shall** serialise tool definitions through a single
profile-driven encoder for all providers, and **shall** serve the result from
the existing process-global cache so that a cache hit is byte-identical to a
miss. A provider **shall not** construct its own tool array. A test **shall**
assert byte equality between `cached_tools(profile)` and a freshly encoded
array for every profile, and **shall** assert that no provider module contains
a hand-built `"parameters"` / `"input_schema"` / `"toolSpec"` JSON literal
outside the encoder.

> *Ubiquitous requirement - the definition format is defined once.*

### FR-004 - Index-keyed and id-keyed stream accumulation (event-driven)

When a provider stream emits tool-call events, the shared accumulator **shall**
key open calls by the provider's declared `stream_keying`: SSE content-block
index where the provider supplies an index, call id where it supplies an id.
Each `ToolCallDelta` **shall** be attributed to the call that its key names,
and exactly one `ToolCallEnd` **shall** be produced per call id.

> *Event-driven requirement - triggers on every tool-call stream event.*

### FR-005 - Parallel tool calls are distinguished (event-driven)

When a single provider response contains two or more tool calls, the
accumulator **shall** keep them independent: a delta belonging to call B
**shall not** be appended to call A's argument buffer, regardless of arrival
interleaving, and the completed set **shall** preserve the provider's ordering.
This applies to the Bedrock Converse path, which today tracks a single active
call id.

> *Event-driven requirement - triggers when a response carries more than one
> tool call.*

### FR-006 - Provider-appropriate tool-result carrier (event-driven)

When a tool result is appended to history, the active provider's encoder
**shall** emit the carrier its profile declares, and the id in that carrier
**shall** be byte-identical to the id the model returned for that call:

- `ToolRoleMessage` - a `role: "tool"` message carrying the id in the profile's
  `id_field`, plus `tool_name` when the profile requests it;
- `UserToolResultBlock` - a user-role message containing a `tool_result` block
  with `tool_use_id` and `content`;
- `GeminiFunctionResponse` - a `functionResponse` part naming the tool;
- `ResponsesFunctionCallOutput` - a `function_call_output` item.

> *Event-driven requirement - triggers whenever a tool result is appended.*

### FR-007 - One argument decoding path (event-driven)

When the accumulated argument text of a completed tool call is decoded, the
system **shall** call a single shared helper that accepts a JSON object, or a
JSON string that decodes to one, or empty/whitespace (decoded as `{}`), and
**shall** run at most one decode per call per step (the result is reused by
every later stage). The provider layer **shall not** decode arguments.

> *Event-driven requirement - triggers once per completed tool call.*

### FR-008 - Corrective error on undecodable arguments (event-driven)

When argument decoding fails, the system **shall not** execute the tool, and
**shall** append a tool result whose text names the tool, quotes the parse
error, and instructs the model to resend one complete JSON object, with the
record's status set to `Error`.

> *Event-driven requirement - triggers on malformed argument JSON.*

### FR-009 - Corrective error on schema violation (event-driven)

When `validate_required_args` reports a violation against the tool's declared
`parameters_schema()`, the system **shall not** execute the tool, and **shall**
append a tool result naming the offending parameter and its declared type.

> *Event-driven requirement - triggers when required arguments are missing or
> mistyped.*

### FR-010 - Sequential execution when the provider cannot parallelise
(state-driven)

While the active provider profile declares `supports_parallel == false`, the
system **shall** execute the step's tool calls sequentially even when
`experimental.parallel_tool_calls` is `true`, and **shall** log one
`tracing::info!` naming the provider and the reason.

> *State-driven requirement - holds for as long as a non-parallelising profile
> is active.*

### FR-011 - Reasoning echo for profiles that require it (state-driven)

While the active profile declares `echo_reasoning == true`, the history encoder
**shall** replay the stored reasoning block, including its signature, ahead of
the tool-use block of the same assistant turn. While the profile declares
`echo_reasoning == false`, reasoning **shall** continue to be omitted.

> *State-driven requirement - holds for extended-thinking sessions.*

### FR-012 - One dispatch pipeline for every execution mode (ubiquitous)

The system **shall** run every tool call through one ordered pipeline -
decode arguments, validate schema, unknown-tool check, repeat guard, pre-tool
hooks, permission verdict, backend dispatch, result normalisation - and the
sequential and parallel paths **shall** both call that pipeline rather than
duplicating any stage.

> *Ubiquitous requirement - the stages and their order are identical in every
> mode.*

### FR-013 - Approval blocks the result, not just the call (state-driven)

While a tool call's permission verdict is unresolved, the system **shall not**
append a tool result for it to history, so no turn can continue on an
unapproved call's output.

> *State-driven requirement - holds for the lifetime of a pending approval.*

### FR-014 - Error results are marked (ubiquitous)

Every tool result **shall** carry an `is_error` flag derived from its record
status. Where the profile declares `supports_is_error == true` the flag
**shall** be transmitted natively; where it declares `false` the encoder
**shall** prefix the content with the literal marker `ERROR: ` and no other
content change **shall** be made.

> *Ubiquitous requirement - an error result is never indistinguishable from a
> successful one.*

### FR-015 - Declarative tool-name alias resolution (optional)

Where `tools.aliases` is configured (or the built-in default table is enabled),
the system **shall** resolve an inbound tool name to its ragent tool before the
unknown-tool check, using a case-sensitive table of exact spellings. Where
`tools.expose_alias_names` is `true`, the outbound tool list **shall** advertise
each alias alongside the canonical name. Where neither is enabled, behaviour
**shall** be unchanged.

> *Optional requirement - applies only when alias resolution is enabled.*

### FR-016 - Wire-matrix inspection command (optional)

Where the user supplies the wire-matrix flag, the system **shall** print, per
registered provider, the profile fields of FR-002 as an aligned table and exit
without contacting any provider or executing any tool.

> *Optional requirement - applies only when the flag is supplied.*

### FR-017 - Payload trace (optional)

Where `tools.wire_trace` is enabled in config, the system **shall** write the
exact per-step request body for the active provider to
`target/temp/wire-trace/<session>-<step>.json` with every credential redacted
through the existing sanitizer, and **shall** be inert when the setting is
absent or `false`.

> *Optional requirement - applies only when tracing is enabled.*

### FR-018 - Silent construct loss is refused (unwanted)

If a profile cannot carry a canonical construct that the history contains (for
example, a tool-use part on a carrier that has no tool-items slot), the system
**shall not** silently drop it; it **shall** return an error naming the
construct and the provider, and **shall not** issue the request.

> *Unwanted-behaviour requirement - guards against the existing Responses-API
> history drop.*

### FR-019 - Missing tool-call id is synthesised deterministically (unwanted)

If a provider returns a tool call without an id, the system **shall** generate
one of the form `<provider>_<monotonic counter>_<tool name>`, **shall** record
the tool name separately in the canonical record rather than recovering it by
parsing the id, and **shall** log one `tracing::debug!` naming the provider.

> *Unwanted-behaviour requirement - guards against name recovery via id
> string-splitting.*

### FR-020 - Duplicate call ids in one response are reconciled (unwanted)

If two tool calls in one response carry the same id, the system **shall** keep
the first, **shall** re-key the later ones to unique synthetic ids, and
**shall** log one `tracing::warn!` per collision.

> *Unwanted-behaviour requirement - guards against a stream that reuses ids
> across content blocks.*

### FR-021 - Unknown tool names never execute (unwanted)

If a resolved tool name is not registered, the system **shall not** execute
anything, and **shall** return a corrective tool result naming the unknown tool
and the closest registered name when one is within the existing similarity
threshold.

> *Unwanted-behaviour requirement - guards against a hallucinated tool name
> running an unintended action.*

### FR-022 - Tool-use finish with no parsed call is treated as truncation
(unwanted)

If a provider reports a tool-use finish reason but the accumulator produced zero
complete tool calls, the system **shall** treat the step as an incomplete
response and **shall** enter the existing text-recovery path before deciding
the turn is finished.

> *Unwanted-behaviour requirement - guards against a lost tool call ending a
> turn silently.*

### FR-023 - The rename set is one checked-in, invariant-tested table
(ubiquitous)

The system **shall** express the tool-name renames of this feature as a single
checked-in table of `(before, after)` pairs, ascending by `after`, exposed from
one module that every migration site reads, and derived from `tools.xlsx` by
the rules of FR-024 and FR-025.

A test **shall** assert that (a) every `before` is a name the default registry
registers, (b) every `after` is a valid ASCII identifier that the default
registry does not register, (c) the `after` column is unique, and (d) the
`before` column is unique. The test **shall not** open the workbook:
`crates/ragent-tools-core/src/xlsx.rs` is a writer only, and adding a reader to
compare against it in a test would breach NFR-002. Cross-checking the table
against the workbook is a manual step recorded in the test plan.

Under the workbook as delivered, the table is exactly:

| Before | After | Anthropic sheet | ragent sheet |
|---|---|---|---|
| `ask_user` | `AskUserQuestion` | Exact | Exact |
| `bash` | `Bash` | Exact | Exact |
| `edit` | `Edit` | Exact | Exact |
| `plan_enter` | `EnterPlanMode` | Exact | Exact |
| `plan_exit` | `ExitPlanMode` | Exact | Exact |
| `glob` | `Glob` | Exact | Exact |
| `grep` | `Grep` | Exact | Exact |
| `skill_manage` | `Skill` | Exact | Exact |
| `task_create` | `TaskCreate` | Exact | Exact |
| `task_get` | `TaskGet` | Exact | Exact |
| `task_list` | `TaskList` | Exact | Exact |
| `task_update` | `TaskUpdate` | Exact | Exact |
| `write` | `Write` | Exact | Exact |

> *Ubiquitous requirement - every rename in the build is one row of one table.*

### FR-024 - A tool is renamed if and only if both sheets record an exact
match (ubiquitous)

A `(before, after)` pair **shall** appear in the FR-023 table if and only if all
three clauses hold for the workbook's `Anthropic Tools` and `ragent Tools`
sheets:

- the `Anthropic Tools` row whose `Tool` is `after` carries `Mapping = Exact`;
- the `ragent Tools` row whose `Tool` is `before` carries
  `Mapping = Exact`;
- that ragent row's `Anthropic equivalent` cell names `after` as one of its
  `/`-separated entries.

A pair that fails any clause **shall not** be renamed, and **shall** be listed
with the clause it failed wherever the refusal is reported. Under the workbook
as delivered this excludes `new_agent` (the Anthropic sheet rates `Agent`
`Close`), `read` (`Read` is `Close` there), the five `cron_*` tools (their
ragent rows are `Close` although `CronCreate`, `CronDelete` and `CronList` are
`Exact`), and `TodoWrite`, which no ragent tool names.

> *Ubiquitous requirement - eligibility is a property of the workbook, not of a
> reviewer's judgement.*

### FR-025 - One-to-one claims are resolved by exact spelling, or refused
(unwanted)

If two or more ragent tools name the same `after` under FR-024, the system
**shall** rename the claimant whose current name equals `after` once each
`_`-separated segment is capitalised (`bash` -> `Bash`, `edit` -> `Edit`,
`write` -> `Write`), and **shall** leave every other claimant under its current
name.

If no claimant satisfies that test, the system **shall** rename none of them
and **shall** record the refusal together with the competing names.

Under the workbook as delivered the tie-break keeps `bash`, `edit` and `write`
renamable while `bash_reset`, `apply_patch`, `patch`, `create` and `update_file`
keep their names, and it refuses `WebFetch` (`webfetch`, `mf_fetch`) and
`WebSearch` (`websearch`, `mf_search`), neither of which has an exact spelling
match.

> *Unwanted-behaviour requirement - guards against two ragent tools racing for
> one Claude spelling and one of them silently disappearing.*

### FR-026 - The rename is a rename; no alias is registered (ubiquitous)

For every row of FR-023 the system **shall** expose the tool under its `after`
name only. `Tool::name()`, the registry map key, `definitions()`, the `/tools`
listing, `tool_info`, `commands_info`, the `--dry-run` report and the TUI
**shall** all show `after`. A call or lookup using `before` **shall** fail as an
unknown tool, carrying the existing did-you-mean hint (which will name `after`
wherever the similarity threshold allows).

The system **shall not** register `before` as a second tool, **shall not** keep
a forwarding wrapper or adapter under `before`, and **shall not** accept
`before` at any inbound tool-name resolution stage.

> *Case folding is not a second spelling.* Matching an inbound name without
> regard to ASCII letter case (`bash` reaching `Bash`, `WRITE` reaching
> `Write`) is a spelling match between two forms of the **same** word, not the
> retired second vocabulary this requirement refuses: no registry key, listing,
> `tool_info` row or outbound definition carries a `before` spelling,
> `ToolRegistry::get` still refuses one, and the `allowed_tools` and
> `tools.aliases` surfaces still resolve exactly. Case folding therefore applies
> to the dispatch lookup only, and never to a surface FR-026 or FR-031 names.

> *Ubiquitous requirement - the whole point of the rename is that the spelling
> Claude skills already use is the spelling that resolves; a kept alias would
> keep two vocabularies alive and re-introduce the drift.*

### FR-027 - Every in-tree site that keys on a renamed name migrates in the
same change (ubiquitous)

Every non-test source site that compares, matches, lists or stores a renamed
tool name as a literal **shall** be updated to the `after` spelling, including
at least: `SKILL_ALWAYS_ALLOWED_TOOLS`
(`crates/ragent-agent/src/session/permissions.rs:561`), `LOOP_WRITE_TOOLS`
(`crates/ragent-agent/src/session/loop_state.rs:412`), the write-tool list in
`crates/ragent-agent/src/session/history.rs:1001`, the `bash` special cases in
`crates/ragent-agent/src/session/processor.rs:331`,
`crates/ragent-agent/src/session/permissions.rs` and
`crates/ragent-agent/src/backend/container.rs`, the legacy alias targets in
`crates/ragent-agent/src/tool/aliases.rs`, the tool-repeat guard keys, the
bundled skill `allowed_tools` lists
(`crates/ragent-agent/src/skill/bundled.rs`), and the per-tool
pretty-printers and summaries in
`crates/ragent-tui/src/widgets/message_widget.rs`.

Tool *descriptions* **shall** be migrated too, because they are prompt text the
model reads: at least eleven shipped `description()` strings tell the model to
call a renamed tool by its old spelling (`bash_reset` and `rm` say `bash`,
`append_to_file`, `create` and `multi_edit` say `edit`, `list` says `glob`, four
`codeindex_*` tools say `grep`, and `edit` says `write`). A stale description
instructs the model to make a call that now fails.

A guard **shall** fail the build when a renamed `before` name appears as a
quoted or backticked literal in non-test source outside the FR-023 table and
the changelog entry.

> *Ubiquitous requirement - a stale literal is a silent behavioural change; the
> compiler cannot catch a string.*

### FR-028 - Permission categories, hook matchers and stored history survive
the rename (state-driven)

While permission rules, hook matchers and stored tool records are evaluated,
the system **shall** apply the following:

- `permission_category()` values **shall** be unchanged by the rename (they are
  derived independently of the tool name, for example `file:read`, `bash:execute`,
  `plan`, `task`, `skill:manage`);
- a hook whose `matcher` names only a `before` spelling **shall** be reported
  once at load with the renamed replacement, because a silently non-matching
  hook is disabled without its author knowing;
- a `MessagePart::ToolCall` recorded before the rename **shall** be replayed to
  the provider verbatim, unrewritten;
- a tool-name-keyed classifier that does not recognise a stored name **shall**
  treat it as non-destructive rather than failing or panicking.

> *State-driven requirement - holds for the lifetime of a config file or a
> saved session, both of which outlive the rename.*

### FR-029 - Claude-spelled skill `allowed_tools` resolve exactly (unwanted)

When a skill or agent profile declares `allowed_tools` entries using the
Anthropic spellings this feature adopts (`AskUserQuestion`, `Bash`, `Edit`,
`Glob`, `Grep`, `EnterPlanMode`, `ExitPlanMode`, `Skill`, `TaskCreate`,
`TaskGet`, `TaskList`, `TaskUpdate`, `Write`), the system **shall** resolve them
against the registry with no alias translation, because the registry now holds
those names.

If an `allowed_tools` entry matches no registered tool, the system **shall**
report it once, naming the entry and the skill or agent that declared it,
rather than silently shipping a smaller tool surface.

> *Unwanted-behaviour requirement - guards against a typo in a skill manifest
> removing a capability without a word.*

### FR-030 - The tool surface is unchanged in size and every enumeration
agrees (ubiquitous)

The rename **shall** leave the registered tool count unchanged from its value
before the change and **shall not** add, remove, merge or split a tool. Every
enumeration of the surface **shall** remain consistent with the registry:
`--dry-run`, `tool_info`, `commands_info`, `/tools list`, the `Tools
registered` line in `/bug-report`, and the outbound tool list sent to a
provider.

> *Ubiquitous requirement - a rename that changes the count is a regression, and
> a catalogue that disagrees with the registry is a lie.*

### FR-031 - No escape hatch re-enables a retired spelling (unwanted)

If any supported mechanism - a configuration key, an environment variable, a CLI
flag, or a plugin, connector or agent setting - would make a retired `before`
spelling resolvable, the system **shall** not honour it.

`tools.aliases` (FR-015) **shall** map provider spellings onto registered ragent
names only, and **shall** reject two entry forms when the configuration is
loaded, each with an error naming the file, the offending spelling and the
`after` replacement so the reader is sent to the name that does exist:

- a **source key** equal to a `before` spelling - including the self-mapping
  form, which asks for two spellings of one tool;
- a **target value** equal to a `before` spelling.

A target value that names no currently registered tool is not a rename problem,
because a plugin or MCP tool may be bridged after config load; such an entry
**shall** be reported once as unresolved and **shall** be refused at the moment
it is used, under FR-032, rather than silently ignored.

`tools.expose_alias_names` **shall** never advertise a `before` spelling. The
system **shall not** add any new key, variable or flag whose purpose is to
restore a retired spelling.

> *Unwanted-behaviour requirement - guards against the compatibility flag that
> quietly undoes the rename for one user and nobody else.*

### FR-032 - No dynamic registration path reintroduces a retired spelling
(unwanted)

If a dynamically sourced tool's surfaced name equals a `before` spelling, the
system **shall** refuse the bridge with an error naming the source, the spelling
and its `after` replacement, and **shall** keep the rest of that source working.

A tool that arrives from a dynamic source - an MCP server bridged as
`<server>.<tool>` or through `mcp_tool`, a plugin tool `plugin_<id>_<tool>`, a
connector's servers, or a skill-contributed tool - **shall** be namespaced so
that it cannot occupy a bare ragent name at all; the refusal above is the
backstop for a name that reaches the bare namespace anyway. A refused bridge
**shall** leave the session usable and **shall** be reported once, not once per
turn.

> *Unwanted-behaviour requirement - the registry is the only place a tool name
> may come from; a remote server must not be able to resurrect a retired one.*

### FR-033 - Every completion and suggestion source is registry-derived
(ubiquitous)

The system **shall** build every completion, autocomplete and suggestion surface
that offers a tool name from the registry, and **shall** keep the same rule for
the slash-command and tool-name menus, the did-you-mean candidate pool, and the
tool lists printed by `--dry-run`, `tool_info` and `commands_info`. This adds no
new completion surface: where a field does not offer tool names today, it need
not start.

A retired `before` spelling **shall not** appear in any of them, and typing one
**shall** produce no completion entry - only the ordinary unknown-tool refusal of
FR-026, with the FR-021 hint when a registered name is close.

> *Ubiquitous requirement - a suggestion list is a catalogue, and a catalogue
> that offers a dead name teaches the mistake back.*

### FR-034 - The rename never ships two spellings at once (state-driven)

While the change is being landed, every commit **shall** leave the tree in a
state where exactly one spelling of each FR-023 pair resolves, the build and the
workspace test suites pass, and `--dry-run` reports the FR-030 count. A commit
that registers `after` while `before` still resolves, or that removes `before`
while `after` is not yet registered, **shall not** be landed.

> *State-driven requirement - holds for every commit in the change, so a bisect
> never lands on a tree that resolves both names or neither.*

### NFR-001 - No per-step re-serialisation

The unification **shall** not increase per-step serialisation work: tool
definitions are encoded once per distinct tool set and served from the existing
cache for every subsequent step, and argument decoding happens exactly once per
call.

### NFR-002 - No new external dependencies

The change **shall** not add an external crate; it uses `serde_json`, `futures`
and the crates already in the workspace.

### NFR-003 - No unsafe code

The change **shall** introduce no `unsafe` block.

### NFR-004 - Testability

Each requirement **shall** be exercisable without a live provider: a loopback
SSE server can replay recorded frames per profile, and the encoders are pure
functions over `&[ToolDefinition]` and `&ChatMessage`.

### NFR-005 - ASCII output

Every user-facing string the change introduces (error text, the wire matrix,
trace filenames) **shall** be ASCII.

### NFR-006 - The rename costs nothing at runtime

The rename **shall** add no per-step work: tool names remain borrowed `&str`
values compared directly, the FR-023 table **shall** be read only by tests and
by migration sites, and no dispatch step **shall** consult it to translate a
name. Resolution, the repeat guard and the definition cache **shall** key on the
new spelling with the same cost as the old one.

### NFR-007 - Documentation names the new spelling and records the old one

Every document that names a renamed tool (the crate `DOCS.md` files,
`README.md`, `SPEC.md`, `QUICKSTART.md`, `TUI-QUICKSTART.md`, the howto pages
under `docs/`) **shall** use the `after` spelling after the change, and
`CHANGELOG.md` **shall** carry a `Renamed` table listing every `before` ->
`after` pair so a reader upgrading an existing config can find the mapping.

### NFR-008 - The no-alias rule is enforced mechanically, not by review alone

A test or guard **shall** fail the build when any of the following predicates
holds, so the rule of FR-026 cannot erode after the change lands:

- **(a)** a `before` spelling appears as a key or a value in the alias table,
  including a key mapped onto the identical string;
- **(b)** a `before` spelling appears in a resolver match arm;
- **(c)** a `before` spelling appears in a default configuration value;
- **(d)** a `before` spelling appears as a registry key;
- **(e)** a `before` spelling appears as a quoted or backticked literal in any
  non-test source file outside the FR-023 table and the `CHANGELOG.md` entry,
  except on a line carrying a written exemption reason - this is the FR-027
  guard, extended rather than duplicated;
- **(f)** one tool name is registered more than once;
- **(g)** a registered tool's `execute` delegates wholly to another registered
  tool's `execute`, so a wrapper could stand in for a retired name.

The check **shall** report every offending file and line for every predicate in
one run, not merely the first, so a single pass fixes the whole set.

### NFR-009 - The guard is hermetic and cheap to run

The NFR-008 check **shall** read only files inside the repository, **shall** make
no network request, **shall not** open `tools.xlsx`, and **shall** finish in a few
seconds so it can run on every `cargo test` and in the CI guard set without a
timeout exception. Its failure messages **shall** be ASCII, **shall** name the
predicate letter it failed and the exact edit required, and **shall** exit
non-zero. The check **shall** be usable both as a `cargo test` case and as a
`scripts/` guard beside the existing `check-*` scripts, from one implementation.

### The guard is one implementation, in two forms

NFR-009's last clause is a design rule, not a packaging preference: the check is
`scripts/check_tool_rename.py`, and both front doors run **that program**.
`scripts/check-tool-rename.sh` is a wrapper that `exec`s it with the caller's
arguments and holds no predicate logic and no fixture, and
`crates/ragent-agent/tests/test_tool_rename_guard.rs` reads its `--json` report
and asserts the same verdict, the same hits and the same ASCII wording the
script prints. A predicate change lands in one file, and a change that makes the
two forms disagree fails a case rather than surviving review.

`--root DIR` is part of the same rule: it lets a case point the one
implementation at a private copy of the repository under `target/temp/`, so a
seeded violation is never visible to a sibling test process and no fixture is
written into the checkout. `--self-test` (the guard's own contract) is the one
run that still seeds inside the tree. A scan of a different root is judged
against **that** root's FR-023 table and fails closed when the table is absent,
because the retired spellings are read from the tree being judged rather than
from the checkout the program happens to live in.

---

## Out of Scope

- Adding new providers.
- Changing the permission model, the hook protocol, or the 7-layer bash
  validation.
- Redesigning the TUI rendering of tool calls.
- Server-side hosted tools that have no local execution (OpenAI
  `code_interpreter`, Anthropic computer use). These are mapped as
  `No equivalent` and are not executed by ragent.
- Any Python or TypeScript harness. ragent is the harness; the deliverable is
  its Rust provider layer.

---

## Acceptance

The specification is met when:

1. `cargo fmt --check`, `cargo clippy --all-targets` and the crate test suites
   for `ragent-types`, `ragent-llm` and `ragent-agent` pass.
2. A recorded-frame replay test per profile proves FR-004, FR-005, FR-006 and
   FR-014.
3. The wire matrix (FR-016) lists every registered provider with no empty
   field.
4. The Responses-API path carries tool history instead of dropping it (FR-018),
   proven by an encoder test.
5. The manual test plan in `TESTPLAN.md` passes end to end against at least one
   OpenAI-family and one Anthropic-family provider.
6. The FR-023 table holds exactly the 13 pairs listed there, its invariant test
   passes, and a fresh derivation from `tools.xlsx` by the FR-024 and FR-025 rules
   reproduces it with no pair added or removed.
7. `AskUserQuestion`, `Bash`, `Edit`, `Glob`, `Grep`, `EnterPlanMode`,
   `ExitPlanMode`, `Skill`, `TaskCreate`, `TaskGet`, `TaskList`, `TaskUpdate`
   and `Write` are each registered exactly once, and none of the 13 retired
   spellings resolves to a tool at any inbound path (FR-026).
8. `cargo fmt --check`, `cargo clippy --all-targets` and the full workspace test
   suites pass with no test weakened: every test that asserted a retired name
   now asserts the new one (FR-027, FR-030).
9. The registered tool count is unchanged, and the `/tools` listing, `tool_info`,
   `commands_info`, `--dry-run` and `/bug-report` all agree with it (FR-030).
10. `CHANGELOG.md` carries a `Renamed` table naming all 13 pairs, and no
    non-test source file still keys on a retired spelling outside the FR-023
    table (FR-027, NFR-007).
11. A `tools.aliases` block whose source key or target value is a retired
    spelling is refused at load with an error naming the file, the spelling and
    its replacement, and no configuration key, environment variable or CLI flag
    restores a retired name (FR-031).
12. A dynamically bridged tool - MCP, plugin, connector or skill-contributed -
    whose surfaced name equals a retired spelling is refused once, with its
    source named, and the rest of that source keeps working (FR-032).
13. No completion, autocomplete or suggestion surface offers a retired spelling,
    and typing one yields only the unknown-tool refusal of FR-026 (FR-033).
14. Every commit in the change resolves exactly one spelling of each FR-023
    pair, and the guard of NFR-008 fails on each predicate it lists when a
    fixture violates it (FR-034, NFR-008, NFR-009).
