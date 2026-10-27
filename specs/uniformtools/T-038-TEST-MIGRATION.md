# T-038 - Migrate every test literal that names a renamed tool (FR-026, FR-030)

**Spec ID:** uniformtools
**Task:** T-038
**Requirements:** FR-026 (the rename is a rename; no alias is registered) and
FR-030 (the tool surface is unchanged in size and every enumeration agrees)
**Status:** completed
**Depends on:** T-031 (the source rename) and T-033 (the non-test literal
migration); T-029 §10 supplies the measured census this task works from.

## What the requirement asks

FR-026 says the adopted spelling is the *only* spelling: a call or lookup using
a `before` name must fail as an unknown tool. FR-030 says the surface is the
same size and every enumeration of it agrees with the registry. Neither
requirement mentions tests, and that is exactly why the test churn is its own
task: a test is where the two requirements meet. A test that names a tool by a
retired spelling is either

- a **fixture** the code under test reads (a scripted model reply, a hook
  matcher, a `MessagePart::ToolCall`, a `ToolDefinition.name`), in which case a
  retired spelling makes the test exercise the *unknown-tool* path while
  claiming to exercise the tool - it passes for the wrong reason, or fails
  outright once the code is honest; or
- an **assertion** about what the code produced, in which case a retired
  spelling asserts a fact the tree no longer has.

The task text names the rule for the second case in one clause, and it is the
whole point of the task:

> where a test asserted a tool name, assert the new name rather than relaxing
> the assertion

A relaxed assertion (`assert!(name == "bash" || name == "Bash")`) would let
T-031 be undone without a single test failing - it would convert a rename
requirement into a spelling preference. No assertion in this migration was
relaxed.

## What the state was when this task started

T-029 §10 measured the pre-rename tree (`1.0.149`) with a pinned scan: fourteen
retired spellings as a double-quoted or backticked literal in a `.rs` file under
`crates/*/tests`, the FR-023 invariant test excluded. It printed **775
occurrences in 160 files**, and section 10 itself records that the figures it
first carried were two short on the occurrence column - the file column
reproduced exactly.

That number was re-measured for this task rather than inherited, on three trees,
with the scan in `target/temp/t038_measure.py` (any-literal is T-029's rule
verbatim; code and comment are its two halves, split at the `//` that starts a
trailing comment):

| Tree | any-literal (T-029's rule) | code literals | comment prose |
|---|---|---|---|
| `1.0.149` (pre-rename) | **775 / 160 files** | 633 / 142 files | 102 / 45 files |
| `d6abcd4a` (`HEAD` when T-038 started) | 839 / 168 files | 648 / 143 files | 135 / 57 files |
| now | **295 / 77 files** | **201 / 63 files** | 46 / 19 files |

The 1.0.149 row reproduces T-029's headline figures exactly, which is the check
that the scan is the same scan. The `d6abcd4a` row is larger because that commit
added the FR-023/FR-028/FR-029/FR-031/FR-032 guard files, and those files name
retired spellings **by design** - they are the evidence that the spellings are
retired.

## The census: 648 code literals at `d6abcd4a`, 447 migrated, 201 kept

Every kept literal is in one of twelve classes, each of which is a spelling that
is **not a tool name** or an assertion that a spelling **is** retired. Nothing
was kept by file-level exemption "because it is awkward".

| # | Class | Kept | Files | Why it is not a tool name |
|---|---|---|---|---|
| a | error-message / panic / assert text | 60 | 24 | `std::fs::write(..).expect("write")` is the *English verb* in a message; the assertion is on the `expect` string, not on a tool |
| b | FR-028 permission type, category or request field | 31 | 11 | FR-028 keeps `Permission::{Edit,Bash,PlanEnter,PlanExit}` and `permission_category()` (`bash:execute`, `file:write`, `ask_user`) **unchanged by the rename** - they are permission vocabulary, derived independently of the tool name |
| c | a shell program named `bash` | 9 | 3 | `Command::new("bash")`, `parts(&["bash", "payload.sh"])`, `for shell in ["sh", "bash", ...]` - the binary, not the tool |
| d | the `bash.allowlist` / `bash.denylist` config section | 10 | 2 | a `ragent.json` **section** named for the `bash:execute` permission family (T-042/T-043's audited exemption) |
| e | a language id / scaffold table entry | 1 | 1 | `("bash", Language::Shell)` - the scaffold's language table |
| f | a GovCreate stage id | 6 | 1 | the `write` **stage** of the archdoc pipeline (`stopped_before: "write"`, `stage_ids`, `GovCreateProgress::Started`), not a tool |
| g | an agent capability tag | 5 | 2 | `AgentRegistry::register("b", vec!["write"], ..)` - a capability the router matches on, not a registered tool |
| h | a stored memory tag | 1 | 1 | the `bash` memory tag names the shell the failure came from; a stored tag is a value, not a lookup |
| i | a scratch-directory tag / JSON-schema enum data | 1 | 1 | `temp_dir("write")` names a directory; `"enum": ["read", "write"]` is schema data in a bench fixture |
| j | English prose in a description or an error wording | 2 | 2 | `description().to_lowercase().contains("write")`; `"include glob"` in a `Grep` error message |
| k | a retirement assertion, in a file this spec added | 58 | 10 | the FR-023 invariant, FR-028 replay, FR-029 allowed-tools, FR-031 alias refusal, FR-032 bridge refusal and NFR-006 cost proofs **must** name the retired spelling to assert it is retired |
| l | a retirement assertion, in an older file | 17 | 13 | the same proofs in files that predate this spec (e.g. `canonical_tool_name("bash") == "Bash"`, a stored `MessagePart::ToolCall` replayed verbatim) |
| | **total kept** | **201** | 63 | |

Classes (a) to (j) are the same classes T-033's "Deliberately not migrated"
section records, because they are the same spellings; the test tree simply holds
them too. Classes (k) and (l) are the half that has no non-test analogue: a
*guard* may never widen its own exemption list to cover an assertion that a
name is retired, and a test tree is where those assertions live.

## What was migrated

The remaining **447 code literals across 81 files**, plus 89 comment-prose lines
across 39 files. Both halves matter, and for different reasons:

- A **code literal** is what the code under test reads. `tool: "bash"` in a
  `MessagePart::ToolCall` feeds the TUI's pretty-printer, the server's SSE
  encoder, the storage record and the compaction serialiser. Leaving it retired
  means those paths were being tested through the fallback arm, not through the
  `Bash` arm they exist to cover.
- **Comment prose** is not read by the compiler, but a `///` that names a tool by
  a retired spelling tells the next reader a falsehood - the same argument T-033
  makes for the non-test tree. `//! Tests for the legacy \`edit\`/\`multiedit\`
  aliases` became `//! Tests for the legacy \`Edit\`/\`multiedit\` aliases`,
  because the registry holds `Edit` and the test asserts it does.

The migration was mechanical in form (a literal-for-literal replacement of the
thirteen pairs) and **not** mechanical in effect: 196 of the replacements changed
what the test feeds the code, and 12 test files then needed their fixtures
updated to match the assertion the mechanical pass had already corrected. Those
twelve are the interesting part of the diff:

| File | What the mechanical pass exposed |
|---|---|
| `test_compaction_serializer.rs` | the assertion expected `[Assistant tool call]: bash(ls -la)` from a fixture that now records `Bash`; **the assertion was kept and the expected string moved to `Bash`**, the fixture left as the adopted spelling |
| `test_text_toolcalls.rs` | the extractor returns *what the model wrote*, so the fixture JSON must name `Bash`/`Grep` for the assertion on `.name` to be meaningful; a case that had been passing with `"bash"` in the text and `"bash"` in the assertion now has `"Bash"` in both |
| `test_hooks.rs` | `PreToolUseResult::HookWarning.tool` is asserted to be the tool as given; the `"write"` fixture became `"Write"` and so did the assertion - proving the hook reports the spelling it was handed rather than normalising it |
| `inline/skill_loader.rs` | the YAML fixture declares `allowed-tools: bash`; FR-029 stores the *declared* spelling verbatim, so the case only passes with the adopted spelling on **both** sides |
| `inline/loop_dialog_tests.rs` | `build_spec_from_state` splits the comma list and stores it unvalidated, so the input field must say `read, Edit, Bash` for the assertion `vec!["read", "Edit", "Bash"]` to hold |
| `test_misc_modules_deep.rs` | the same `tool_set_field` round trip in the wider module test |
| `test_event_swarm_research_deep.rs`, `test_hook_warning_flagged.rs`, `test_widgets_and_events_extra.rs` | the TUI status/log strings embed the tool name (`hook warning: <tool>`, `[warn] <tool> flagged`, `security: allow <tool>`); the expected strings follow the fixture |
| `test_text_selection.rs` | the mechanical pass rewrote a `contains("Bash") \|\| contains("bash")` into `contains("Bash") \|\| contains("Bash")` - a duplicate condition, harmless in Rust but proof that a blind rewrite produces nonsense; the disjunction collapsed to `contains("Bash")` |
| `test_activity_projection_branches.rs` | a `PermissionDecision` / `ToolCall` fixture is a live event, not replayed history, so it takes the adopted spelling; `MutationRejected.attempted` is a free-text description and stayed `"edit"` |
| `test_fixtures.rs` (plugins) | "Seed the surface with the built-in `Bash` tool: the plugin must never be able to displace it" - the seed, the collision fixture and the message all had to agree |

Two fixtures were **reverted** after the first run, because the mechanical pass
had changed the wrong side: `test_hooks.rs`'s first `"write"` argument (a
matcher probe, covered above) and `test_permission_helpers.rs`'s
`is_interactive_tool("ask_user")`. In the latter the case is a *negative*
assertion - the retired spelling is not interactive - so the original spelling is
the assertion, and it stays (class l).

## Red proofs

Six, each restored byte-identically and `md5sum`-verified. The point of each is
that the assertion is load-bearing: it fails when the thing it asserts is not
true, so a reader can see the migration did not merely make the compiler quiet.

| # | Change | Failing case | Evidence |
|---|---|---|---|
| 1 | `is_interactive_tool("AskUserQuestion")` reverted to `("ask_user")` | `test_permission_helpers::test_is_interactive_tool_only_ask_user` | `assertion failed: is_interactive_tool("ask_user")` - the retired spelling is not interactive |
| 2 | `test_hooks.rs`'s `sess-004` fixture reverted to `"write"`, assertion left on `Write` | `test_hooks::test_pre_tool_use_exit_code_1_warns_and_returns_no_decision` | `left: "write" / right: "Write"` - the hook reports the spelling it was handed |
| 3 | `test_tool_event_state.rs`'s assertion reverted to `"write"` | `test_tool_event_state::test_tool_call_batch_creates_missing_part` | `left: "Write" / right: "write"` - the batch event creates the part with the adopted spelling |
| 4 | `test_text_toolcalls.rs`'s fixture JSON reverted to `"bash"` | `test_text_toolcalls::test_extract_bare_json_array` | `left: "bash" / right: "Bash"` - the extractor returns what the model wrote, so the fixture is the assertion's input |
| 5 | `test_hook_warning_flagged.rs`'s expected string reverted to `hook warning on bash` | `test_hook_warning_flagged::test_hook_warning_current_session_sets_transient_status_and_logs` | `log should identify the tool, got: hook warning on Bash (hooks/warn.sh): suspicious network call` |
| 6 | `crates/ragent-tools-core/src/write.rs`'s `Tool::name()` reverted to `"write"` (the production side the tests are about) | `test_tool_rename_table::test_tool_rename_every_after_is_registered_by_the_default_registry` **and** `..._every_before_is_retired_by_the_default_registry` | two failures, one per half of FR-023 - which is the property the whole migration rests on |

Proof 6 is the one that ties this task to T-030/T-031: the test-side migration is
only meaningful if the production side really did move, and reverting the
production side makes the tests that were *migrated* fail rather than the ones
that were kept.

`md5sum` of the four backup files, compared against the tree after every proof:

```
4851d58e4074a9a5b75b560fa7438591  test_hooks.rs
16c5ff8713c6037835b45de86cbba28b  test_permission_helpers.rs
e7067732b4ab73d2984d4ed4c796d151  test_tool_event_state.rs
2907aeb4add000925f9df16346834f8d  write.rs
```

Each restored file was `touch`ed afterwards: `cp -p` preserves the old mtime, so
cargo would otherwise reuse the object it built from the broken source and the
"restored" tree would keep failing.

## Verification

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | clean |
| `cargo check --workspace --all-targets` | no errors, no new warnings |
| `cargo clippy -p ragent-agent -p ragent-tui -p ragent-config -p ragent-types -p ragent-tools-core --all-targets` | no finding in a touched file (the `redundant_pub_crate` and `unnecessary structure name repetition` warnings are pre-existing, in production modules this task did not open) |
| `ragent-agent --test all` | **1741 passed**, 0 failed, 1 ignored (103 s) |
| `ragent-agent --lib` | 361 passed |
| `ragent-tui --test all` | **3654 passed**, 3 failed - the three pre-existing `Alt+T` -> `ToggleTasksPanel` cases (`test_input_more::alt_toggle_keys_map_to_actions`, `test_input_queue_menu_binding::test_other_alt_keys_are_unaffected`, `test_memory_panel::test_toggle_memory_mutually_excludes_tasks_panel`). Identical count and identity to the baseline before this task |
| `ragent-tui --lib` | 142 passed (the `loop_dialog_tests` case this task fixed was the one failure there) |
| `ragent-config`, `ragent-storage`, `ragent-telemetry`, `ragent-plugins`, `ragent-types`, `ragent-tools-core`, `ragent-tools-extended`, `ragent-server`, `ragent-research`, `ragent-surface` | all pass (`--no-fail-fast`, no `FAILED` line) |
| `ragent-llm --test all` | 292 passed, **6 failed** - pre-existing, and none in a file this task touched: the six `test_wire_*` guard cases that assert T-004/T-005/T-006/T-017/T-019 are not done yet (`gemini.rs` still mints `format!("fc_`, `anthropic.rs` does not use the shared accumulator, the folded encoder's key order differs, `bedrock.rs` writes `"tool_result"`, and the trace switch is not driven) |
| the twelve guard scripts | all pass |
| assertion count in `crates/*/tests` | 35 980 at `1.0.149` -> **37 183** now (+1 203); **no test file lost a single assertion** |

The one flake seen: a full `cargo test --workspace --all-targets` run reported
162 `ragent-tui` failures with `current dir: Os { code: 2, kind: NotFound }`,
because a parallel crate's test `set_current_dir`s into a `TempDir` that the
other test then deletes under it - the crate's `global_env_lock` only serialises
tests in *its own* binary. Running `ragent-tui --test all` alone is clean apart
from the three known cases, and the same 162 does not appear in any single-crate
run; it is a cross-binary artefact of `--all-targets`, not a T-038 regression,
and this task touched none of the affected files.

## What this task did not do, and why

- **It did not widen the FR-027 guard's scope to `tests/`.** T-034 deliberately
  excludes test files, and this task is the reason: 201 kept literals are
  assertions that a spelling is retired, and a guard that forbade them would force
  every one to be relaxed - the exact defect the task text forbids.
- **It did not change any production file.** The only `src/` change in this task's
  diff is the temporary red proof 6, restored byte-identically; `git diff` over
  `crates/*/src` is empty for T-038.
- **It did not migrate class (a) to (j).** Each is a spelling that is not a tool
  name; changing one would be a behavioural change with no relation to the rename,
  and T-033's record already reasons each class through on the non-test side.
- **It did not fix the six `ragent-llm` wire-guard failures or the three TUI
  `Alt+T` failures.** Both predate this task, both are other tasks' deliverables
  (T-004/T-005/T-006/T-017/T-019, and the `Alt+T` binding is a separate change),
  and fixing them here would bury a 136-file migration under two unrelated bugs.

## Finding: one of the files this task migrated cannot run

`crates/ragent-config/tests/test_retired_alias_spelling.rs` is one of the files
this task migrated (5 kept literals, class (k)), and a registration audit across
every `tests/all.rs` shows it has **no `#[path]` line** - so its cases have never
compiled. It is the eighth instance of this spec's orphan-file class (T-012,
T-013, T-019, T-030's table test, T-034's guard test, T-036, T-037, T-040), and
`specs/uniformtools/T-046-ATOMIC-LANDING.md` §7 records 20 more across six
crates.

It is **not** fixed here, deliberately: the file exercises
`ToolsConfig::validate_alias_spellings`, which is **T-042's** deliverable and
T-042 is still `pending`. Registering the file would add a hard dependency from
T-038's change to an unlanded task's API and would make this 136-file test diff
fail to build. The registration belongs in T-042's change, alongside the code it
tests.

A second registration gap was found alongside it and is the same shape:
`crates/ragent-tui/tests/` has three unregistered files
(`test_mcp_bridged_surfaces.rs`, `test_mcp_remove.rs`, `test_modules_arms_deep.rs`)
and `crates/ragent-agent/tests/` has eight (four under `inline/`, plus
`test_approval_blocks_result.rs`, `test_mcp_npx_cache_repair.rs`,
`test_parallel_gate.rs`, `test_tool_argument_decode_sites.rs`,
`test_unified_dispatch.rs`). None of them was touched by this task.

Full audit, for whoever lands them:

```
crates/ragent-agent: 8 unregistered
  inline/test_compaction_convert.rs      inline/test_compaction_estimator.rs
  inline/test_compaction_runner.rs       test_approval_blocks_result.rs
  test_mcp_npx_cache_repair.rs           test_parallel_gate.rs
  test_tool_argument_decode_sites.rs     test_unified_dispatch.rs
crates/ragent-config: 1
  test_retired_alias_spelling.rs         (T-042)
crates/ragent-tui: 3
  test_mcp_bridged_surfaces.rs           test_mcp_remove.rs
  test_modules_arms_deep.rs
```

## Files changed

136 files under `crates/*/tests/`, `tests/` and `crates/*/benches/`
(+2 088 / -769 lines). By crate: `ragent-agent` 61, `ragent-tui` 27,
`ragent-telemetry` 8, `ragent-tools-core` 8, `ragent-types` 6,
`ragent-storage` 5, `ragent-tools-extended` 5, `ragent-llm` 4,
`ragent-plugins` 3, `ragent-server` 3, `ragent-bench` 2, `ragent-config` 2,
`ragent-research` 1, `ragent-specs` 1.
