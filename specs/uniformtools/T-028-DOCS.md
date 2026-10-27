# T-028 - The unified tool path, written down

Spec `uniformtools`, task T-028. Requirement NFR-005 (every user-facing string
the change introduces is ASCII), with the doc obligations of NFR-007 (a document
that names a tool uses the new spelling) as its reader-side counterpart.

Status: **completed**. Dependency: T-014 (the one ordered dispatch pipeline).
The plan places this task last in Pass A (`PLAN.md` step 8, "Close-out"), because
it edits the same feature bullets the rename renames.

## 1. What the task asks for

> Update provider docs, `SPEC.md` feature bullets and `CHANGELOG.md` for the
> unified tool path.

Three documents, and one constraint that binds all three: NFR-005 says every
user-facing string the change introduces is ASCII. This task **writes** the
documentation rather than producing those strings, so NFR-005 is honoured here in
the form that applies to prose: every section added is plain ASCII, with `-` as
the dash and `->` for an arrow, and no cell, heading or table entry is blank.

## 2. State found

`d6abcd4a` landed the code for the whole Pass A surface and most of the prose in
module docs, and two prior tasks had already extended `SPEC.md`:

| Document | State found |
|---|---|
| `SPEC.md` | Already carried "Parallel Tool Dispatch Gate", "Unresolved Approvals Block the Result", "Completion and Suggestion Surfaces" and "Tool Surface Agreement". Nothing described the profile *type*, the encoder chain, the dispatch pipeline, the argument decoder, the id rules or `--wire-matrix`. |
| `crates/ragent-llm/DOCS.md` | Already carried a `wire` bullet listing every submodule and its symbols. It did not say what the chain *does* end to end, and it did not record that the router row is nominal. |
| `CHANGELOG.md` | `[Unreleased]` had a `Renamed` table and entries for FR-013, FR-010, FR-022, the nextest migration and FR-020. Nothing in `[Unreleased]` covered FR-002..FR-004, FR-007, FR-012, FR-019, FR-021 or the wire matrix. |

So the gap was not "no documentation" but "no statement of the tool path as a
path": a reader could learn which module held which symbol and still not know
that a provider declares a contract instead of building tool JSON, that both
execution modes call one pipeline, or that `--wire-matrix` prints the contract.

## 3. What landed

### 3.1 `SPEC.md` - "Unified Tool-Calling Wire Path" (under the provider catalogue)

One section after the OpenRouter provider sub-section, before `### 3.2 Tool
System`, so a reader looking at the provider list meets the contract that every
one of those providers declares. It states:

- the nine `ToolWireProfile` fields as a three-column table (field, the values it
  can take, and what it decides), which is the FR-002 contract as a reader can
  check it against `--wire-matrix`;
- the six families and the `PROVIDER_WIRE` id-to-family table, `profile_for` as
  the one lookup the encoders, the accumulator, the gate and the matrix share;
- that there is **no catch-all row**, with the coverage assertion in
  `crates/ragent-llm/tests/test_wire_profile_table.rs` named as the mechanism;
- the `--wire-matrix` output shape (ten columns, one row per id, sorted, no blank
  or `-` cell) and its two guarantees: no provider contacted, no tool executed,
  and a registered id with no row reported by name with a non-zero exit.

### 3.2 `SPEC.md` - "Tool-Call Wire Path and the One Dispatch Pipeline" (under the tool system)

A `### 3.2.1` section between the tool catalogue tables and the existing
`### 3.2.2 Completion and Suggestion Surfaces`, so the tool system's prose runs
catalogue -> path -> surfaces -> agreement. Four rules, one per defect class the
spec exists to close:

1. **A provider declares its contract, it does not build tool JSON** - the three
   encoders and what each reads its decisions from (definitions on
   `tool_envelope` through `encode_tools`, results on `result_carrier` /
   `id_field` through `encode_message`, streamed fragments on `stream_keying`
   through `ToolCallAccumulator`), the synthetic-id rule
   (`<provider>_<counter>_<name>`, deterministic because a call id is replayed),
   the duplicate-id reconciliation, and the one argument decoder
   (`ragent_types::tool_args::decode_tool_arguments`, three accepted shapes, at
   most once per call per step, never in the provider layer) with its single
   refusal value `ToolInputError`.
2. **One ordered dispatch pipeline serves both execution modes** - the slot list
   in the provider's order, the one ordered drain, and the eight stages named
   once in `session::dispatch`, so `experimental.parallel_tool_calls` cannot
   change the history a step records.
3. **An unknown name never executes** - the corrective result, the one
   did-you-mean ranking in `session::tool_suggestion`, and the argument-failure
   refusal raised before dispatch as an error.
4. **A tool result is never indistinguishable from a success** - `is_error`,
   the native flag or the `ERROR: ` prefix, appendable only after a resolved
   verdict, and a tool-use finish with no complete call treated as truncation.

### 3.3 `crates/ragent-llm/DOCS.md` - the chain, and the nominal router row

Two additions to the provider submodule list:

- the `router` bullet now records that its `provider::wire` row is nominal -
  `RouterClient` picks a downstream provider per request and forwards the
  canonical `ChatRequest` to that provider's client, which encodes with *its*
  profile - so a consumer that needs the effective contract for a router session
  resolves the downstream provider rather than reading the row. This is
  FR-002's own "router row is nominal" note reaching the crate that owns it;
  `profile_table`'s module docs had already stopped claiming the router resolves
  its downstream family (T-015).
- a **Unified tool path** bullet states the chain end to end: profile ->
  `encode_tools` (cached per `(ToolFormat, fingerprint)`, a hit equals a miss) ->
  `encode_message` (`error_marked` when `supports_is_error` is false) ->
  `ToolCallAccumulator`, with arguments decoded only by
  `ragent_types::tool_args::decode_tool_arguments` in the harness, and with
  `--wire-matrix` as the printable view.

### 3.4 `CHANGELOG.md` - one `[Unreleased]` / `Added` entry

Placed at the head of `[Unreleased]`'s `Added` section (above the T-011 FR-020
entry), because it is the entry that tells an upgrader the tool path changed
shape rather than a single behaviour. It names the requirement ids the path
satisfies, the two `SPEC.md` sections by title, the `ragent-llm` `DOCS.md`
addition, and the two facts a reader needs to trust it: **documentation only, no
behaviour changed**, and **every string introduced is ASCII (NFR-005)**.

The `### Renamed` table NFR-007 requires already exists in `[Unreleased]` (T-039's
deliverable) and is left untouched, including its two explanatory bullets.

## 4. Deliberately unchanged

- **No Rust file, test, dependency or tool was touched.** Every symbol the new
  prose names already existed; the task is verification-by-being-written-down, so
  a symbol was checked against source before it was named (below).
- **`README.md`, `QUICKSTART.md`, `TUI-QUICKSTART.md` and the howto pages** are
  NFR-007's list, and their tool-name spellings are T-039's deliverable. This
  task adds no feature bullet to them: the unified tool path is an internal
  contract with one user-visible surface (`--wire-matrix`), which `SPEC.md`
  documents and `CHANGELOG.md` announces.
- **The existing `SPEC.md` sections are not rewritten.** The parallel gate,
  unresolved verdict, completion surfaces and surface agreement sections stay
  exactly as their tasks landed them; the new sections sit beside them and the
  tool path points at the gate ("see the gate below") rather than restating it.

## 5. Every named symbol checked against source

Prose that names a symbol is a claim the tree can falsify, so each one was read
before it was written:

| Named in the new prose | Source |
|---|---|
| `ToolWireProfile` and its nine fields | `wire/mod.rs` |
| `ToolEnvelope` (5), `ResultCarrier` (4), `IdField`, `ArgsEncoding`, `StreamKeying` | `wire/mod.rs` |
| `encode_tools`, `encode_message`, `error_marked`, `CachedTools` | `wire/encode.rs` |
| `ToolCallAccumulator`, `StreamKey`, `from_frame` | `wire/decode.rs` |
| `matrix::HEADINGS` (ten columns), `render`, `MissingProfile` | `wire/matrix.rs` |
| `PROVIDER_WIRE`, `FAMILIES`, `profile_for` | `wire/profile_table.rs` |
| `decode_tool_arguments`, `ToolArgDecodeError` | `ragent-types/src/tool_args.rs` |
| `ToolInputError` | `session/tool_input_error.rs` |
| the pipeline and its three terminal constructors | `session/dispatch.rs` |
| `suggest_tool_name` (one ranking) | `session/tool_suggestion.rs`, re-exported by `session/dispatch` |
| `PendingVerdicts`, `result_is_appendable` | `session/pending.rs` |
| `finish_was_tool_use_without_a_call` | `session/loop_steps.rs` |
| the coverage assertion | `crates/ragent-llm/tests/test_wire_profile_table.rs` |

Two claims were narrowed during that pass. The id-field row originally read "a
JSON key, or `synthesised`", which is true and unusable; it now names
`tool_call_id` as the concrete case. The argument-decoder sentence originally
called the accepted shapes "tolerant", which describes an intent rather than the
three shapes FR-007 actually names; it now lists them.

## 6. Verification

- **ASCII (NFR-005).** Every added line in `SPEC.md`, `CHANGELOG.md` and
  `crates/ragent-llm/DOCS.md` is checked for non-ASCII bytes; the section
  headings, table separators and arrows are plain ASCII (`->`, `-`).
- **No code change to verify.** `git status --porcelain` for this task lists one
  new file (`specs/uniformtools/T-028-DOCS.md`) and three modified documents;
  no `.rs` file, `Cargo.toml` or script is in the set, so `cargo fmt` has nothing
  to format and no test result can differ.
- **Markdown structure.** The new `SPEC.md` sections use `####` and `###` at the
  depths their neighbours use (`####` inside `### 3.1 Provider System`, `###`
  for a numbered section beside `### 3.2.2`), and both tables carry a header row
  and a separator so a renderer draws them rather than flattening them.

## 7. Hand-off

The one thing this task cannot close is the pair of `PLAN.md` rows above it that
are still `pending` and that the new prose depends on being true:

- **T-027** ("prove no provider builds its own tool JSON") is the guard for the
  sentence "No provider module builds tool JSON of its own (FR-003)". The
  sentence is true of the tree today by inspection; the guard is what keeps it
  true, and the `ragent-llm` `DOCS.md` bullet deliberately states the claim
  without a guard reference it cannot yet cite.
- **T-026** (recorded-frame replay fixtures) is NFR-004's evidence, and the new
  `SPEC.md` section asserts behaviour per family. A reader who wants the proof
  rather than the claim finds it in the per-family test files
  (`test_wire_encode.rs`, `test_wire_decode.rs`, `test_wire_profile_table.rs`,
  `inline/bedrock_tests.rs`) until T-026 consolidates the fixtures.
