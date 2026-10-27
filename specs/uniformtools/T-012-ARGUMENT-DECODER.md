# T-012 - One shared argument decoder, and every decode site routed through it

Spec `uniformtools`, task T-012. Requirement FR-007 (a single shared helper
decodes a completed call's accumulated argument text; at most one decode per call
per step; the provider layer never decodes).

Status: **completed**. Dependency: T-001 (the canonical type additions).

## 1. What the task asks for

> When the accumulated argument text of a completed tool call is decoded, the
> system **shall** call a single shared helper that accepts a JSON object, or a
> JSON string that decodes to one, or empty/whitespace (decoded as `{}`), and
> **shall** run at most one decode per call per step (the result is reused by
> every later stage). The provider layer **shall not** decode arguments.

Three obligations, and the third is a *routing* obligation rather than a helper
one: accepting the three shapes is easy to get right in one place and easy to get
wrong in five.

## 2. What was already there, and what was not

`crates/ragent-types/src/tool_args.rs` was landed by `d6abcd4a` with the helper's
full contract (`decode_tool_arguments`, `ToolArgDecodeError` with `InvalidJson` /
`NotAnObject` / `EncodedStringNotAnObject`, an error `Display` that quotes the
parse failure for FR-008's corrective message) and its own 11-case contract file
`crates/ragent-types/tests/test_tool_args.rs`.

**It was not compiled.** `crates/ragent-types/src/lib.rs` never declared
`pub mod tool_args;`, so the file was dead source: `cargo check` did not build it,
the 11 cases had no `#[path]` line in `crates/ragent-types/tests/all.rs` so they
never ran, and every `ragent_types::tool_args::...` path in the tree would have
failed to compile the moment the module was declared. That is this task's first
fix, and without it every other clause of FR-007 is unobservable.

The harness sites that decoded argument text each had their own rule:

| Site | Rule before | What it got wrong |
|---|---|---|
| `session/processor.rs:3016` (B1 gate) | `serde_json::from_str(&tc.args_json)` | whitespace-only arguments were a corrective error instead of `{}`; a double-encoded payload reached the tool as a JSON *string* |
| `session/processor.rs:136` (`writes_in_spec_dir`) | `serde_json::from_str(&tc.args_json)` | a second JSON rule in the same file as the first |
| `hooks/mod.rs` `candidates_for` | `serde_json::from_str::<Value>(tool_input)` | a guard could not see the arguments of an encoded-string call, so a `Bash(git commit:*)` guard silently did not apply |
| `hooks/mod.rs` `claude_event_payload` | `serde_json::from_str::<Value>(input).unwrap_or(Value::Null)` | a plugin hook received `null` for the same call |
| `tui/app/event_handler.rs:1260` (log preview) | `serde_json::from_str::<Value>(args)` | the log showed the wire text rather than the arguments |
| `tui/app/event_handler.rs:3012` (`update_tool_call_input`) | `serde_json::from_str::<Value>(args_json)` | whitespace args left the part's input `null` |
| `tui/app/event_handler.rs:3102` (`drain_pending_tool_args`) | `serde_json::from_str::<Value>(&args_json)` | the buffered path disagreed with the direct one |

## 3. What this task changed

1. **Declared the module**: `pub mod tool_args;` in `crates/ragent-types/src/lib.rs`
   with its one-line doc, and `#[path = "test_tool_args.rs"] mod test_tool_args;`
   in `crates/ragent-types/tests/all.rs` so the 11 contract cases actually run.
2. **Routed all seven sites** in §2 through
   `ragent_types::tool_args::decode_tool_arguments`, each with a comment naming
   FR-007 at the call. No signature changed; no behaviour changed except that the
   three accepted shapes now behave the same everywhere.
3. **Added the routing guard** `crates/ragent-agent/tests/test_tool_arg_decode_routing.rs`
   (4 cases), because the failure mode FR-007 names is a *new* decode site
   appearing later, which no contract test of the helper can see:

   | Case | Asserts |
   |---|---|
   | `no_harness_source_parses_argument_text_around_the_shared_helper` | every `*.rs` under `crates/ragent-agent/src`, `crates/ragent-tui/src` and `src` that parses `args_json` / `tool_input` / `(args)` with `serde_json::from_str` fails the build, naming the file and the offending lines |
   | `every_harness_decode_site_names_the_shared_helper` | the positive half: the three files that do decode call the helper (a file free of bare parses merely because it never decodes is not enough) |
   | `the_helper_is_the_only_decoder_and_it_handles_all_three_shapes` | the helper still branches on the object, the string and the empty case, and exposes exactly **one** `pub fn decode` - a second entry point would be the second rule |
   | `provider_layer_never_decodes_tool_arguments` | no file under `crates/ragent-llm/src/providers` mentions the helper (the FR-007 "provider layer shall not decode" clause) |
4. **Added the behaviour proofs** that the helper's three shapes reach a tool:
   - `crates/ragent-agent/tests/test_processor_more_paths.rs` +3 cases:
     `test_whitespace_arguments_run_the_tool_with_an_empty_object` (a parameterless
     tool runs on `"  \n\t "`, status `Completed`, `state.input == {}`, no
     corrective error), `test_encoded_string_arguments_run_the_tool_decoded` (a
     double-encoded payload reaches `think` as `{"thought":"decoded"}`, not the wire
     string), and `test_non_object_arguments_are_still_refused` (`[1, 2]` is still
     a corrective error naming "an array" and the tool).
   - `crates/ragent-tui/tests/test_tool_event_state.rs` +1 case:
     `test_tool_args_decode_through_the_shared_helper` (whitespace args leave the
     part at `{}` rather than `null`; an encoded-string payload shows decoded).
5. **Documented the module** in `crates/ragent-types/DOCS.md`.

### Why a second routing file rather than extending the existing one

`crates/ragent-agent/tests/test_tool_argument_decode_sites.rs` already held two of
these scans, but it is **not registered** in that crate's `tests/all.rs` and it
cannot be: its hook cases call `run_pre_tool_use_hooks_decoded` /
`run_post_tool_use_hooks_decoded`, which no source in the workspace defines (the
`*_decoded` entry points were never landed - only `session/dispatch.rs`, itself
not yet a compiled module, calls them). Registering it therefore does not
compile, and a test that does not compile is not a guard. Its `from_str` scan also
covers only `crates/ragent-agent/src` and greps three exact call shapes, so it
would not have caught the TUI sites.

The new file is the compiled guard: it registers, covers all three trees, and
asserts the positive routing of each known decode site as well as the absence of
bare parses. `test_tool_argument_decode_sites.rs` is left untouched - registering
it is a change to whatever task lands the `*_decoded` hooks, not to this one.

## 4. Proof

### Red proofs

Each break was applied, the named case observed red, then the file restored and
verified byte-identical with `md5sum` (and `touch`ed, so cargo could not reuse the
broken object file):

| Break | Failing case | Message observed |
|---|---|---|
| `processor.rs` B1 gate back to `serde_json::from_str(&tc.args_json)` | `test_whitespace_arguments_run_the_tool_with_an_empty_object` **and** `test_encoded_string_arguments_run_the_tool_decoded` | `left: Error, right: Completed` with "Invalid arguments JSON for tool 'tool_info': EOF while parsing a value ..." and "Invalid tool arguments: arguments must be a JSON object, got string" |
| `tui/app/event_handler.rs:3012` back to `serde_json::from_str::<serde_json::Value>(args_json)` | `test_tool_arg_decode_routing::no_harness_source_parses_argument_text_around_the_shared_helper` **and** `test_tool_event_state::test_tool_args_decode_through_the_shared_helper` | "…/event_handler.rs decodes a tool call's argument text with `serde_json::from_str` instead of `ragent_types::tool_args::decode_tool_arguments` (FR-007); offending lines: [...]" and, in the TUI case, `left: Null, right: Object {}` with "whitespace args decode to an empty object (FR-007), not null" |
| `hooks/mod.rs` `candidates_for` back to `serde_json::from_str::<Value>(tool_input)` | the same routing case | the same message naming `hooks/mod.rs` |

The first red proof is the important one: it shows the two behaviour cases are
testing the router and not merely the helper, since the helper itself was
unchanged in that run.

### Verification (green)

- `cargo fmt --all -- --check` - clean
- `cargo test -p ragent-types --test all test_tool_args::` - **11 passed** (the
  file was previously unreachable)
- `cargo test -p ragent-agent --test all` - **1632 passed**, 0 failed,
  1 ignored (includes the 4 routing cases, the 3 new loop cases and the
  pre-existing 23 in `test_processor_more_paths`)
- `cargo test -p ragent-tui --test all` - 3646 passed, **3 failed**, all
  pre-existing (§5)
- `cargo clippy -p ragent-agent -p ragent-tui -p ragent-types --all-targets` - no
  new warning; the only new-looking hit is a pre-existing
  `manual_assert_eq` in `crates/ragent-tui/tests/test_text_selection.rs:279`
- 7 guard scripts PASS: `check-dead-code-reasons`, `check-inline-tests`,
  `check-test-env-isolation`, `check-silent-errors`, `check-security-unwraps`,
  `check-poison-locks`, `check-shared-guards`

## 5. Pre-existing, not caused by this task

`cargo test -p ragent-tui --test all` fails three cases about the **Alt+T /
tasks panel** binding, with nothing to do with argument decoding:
`test_input_more::alt_toggle_keys_map_to_actions`,
`test_input_queue_menu_binding::test_other_alt_keys_are_unaffected` and
`test_memory_panel::test_toggle_memory_mutually_excludes_tasks_panel`. Reproduced
with `crates/ragent-tui/src/app/event_handler.rs` restored to its `HEAD` content
(verified `cmp` equal to `git show HEAD:...`), so the file this task edits is not
the cause; the three cases fail identically on the unmodified file.

Also pre-existing and unrelated: `cargo test -p ragent-llm --test all` carries five
failing source-scan guards for unimplemented tasks (T-006/T-008/T-023) and
`cargo check -p ragent-llm --all-targets` fails to compile that crate's lib-test
target on `tests/inline/anthropic_tests.rs` (`ChatContent` not in scope) - see
`T-002-TOOL-WIRE-PROFILE.md` §5.

## 6. Note for the downstream tasks

- **T-013** (the corrective error and `is_error`) now has one error type to build
  on: `ToolArgDecodeError`'s `Display` already quotes the parse failure, so the
  "quotes the parse error" clause needs no second explanation of the same failure.
- **Any new decode site** must call `decode_tool_arguments`; the routing case scans
  the whole of `crates/ragent-agent/src`, `crates/ragent-tui/src` and `src`, so it
  fails on a site added anywhere in those trees - not only where a decode lives
  today. If a future site legitimately needs raw JSON (a *different* string, not a
  call's arguments), name it something other than `args_json` / `tool_input` /
  `args` rather than weakening the scan.
- **The provider guard is deliberately a string scan** for the helper's name: a
  provider that wanted to decode could still call `serde_json` directly, but it has
  no reason to and the guard catches the plausible mistake (reaching for the shared
  helper from the layer that must forward fragments verbatim).
