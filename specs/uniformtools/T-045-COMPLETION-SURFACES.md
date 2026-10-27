# T-045 - Every completion and suggestion source is registry-derived (FR-033)

Status: **completed**. Requirement: FR-033 (spec `uniformtools`). Dependency: T-031.

## What the requirement asks

FR-033 names the sources a suggestion can be drawn from and states one rule for all
of them: the registry is the catalogue, and a retired `before` spelling appears in
none of them. It names the slash-command and tool-name menus, the did-you-mean
candidate pool, and the lists printed by `--dry-run`, `tool_info` and `commands_info`.
It also states the negative the task text repeats: **no new completion surface** -
where a field offers no tool name today, it need not start.

## The state this task found

Two things were true before any edit, and both change what the task had to do.

**1. Both test files existed, were tracked by git, and had never compiled.** That is
the same orphan-file class the previous five tasks in this spec kept hitting
(`tool_args.rs`, `allowed_tools.rs`, `tool_bridge.rs`, `test_tool_rename_guard.rs`,
`test_nfr006_rename_cost.rs`). `crates/ragent-agent/tests/test_fr033_completion_sources.rs`
and `crates/ragent-tui/tests/test_fr033_completion_surfaces.rs` were registered in
neither `tests/all.rs`, so none of the 10 cases had ever run. Two edits fixed it:

| File | Edit |
|---|---|
| `crates/ragent-agent/tests/all.rs` | `#[path = "test_fr033_completion_sources.rs"] mod ...;` after `test_fr032_bridge_containment` |
| `crates/ragent-tui/tests/all.rs` | `#[path = "test_fr033_completion_surfaces.rs"] mod ...;` after `test_force_cleanup_modal` |

**2. The `/tools` visibility switches still exist**, and both the handler and
`TOOLS_SUBS` accept all seven. An earlier draft of this record described them as
removed; they are not, and `crates/ragent-tui/tests/test_tools_visibility_command.rs`
(5 cases) exercises the toggles. The real defect is narrower and is exactly the class
FR-033 is written against.

## What was actually broken

Four hand-written copies of one `/tools` form list existed, and they disagreed:

| Reader | Offered before | Handler accepted |
|---|---|---|
| `get_command_suggestions` (`crates/ragent-tui/src/app/slash.rs`) | list, github, gitlab, teams, agents, plan, codeindex, masterfetch, help | list, show, github, gitlab, teams, agents, plan, codeindex, masterfetch, help |
| `get_parameter_hint` (`crates/ragent-tui/src/app/session_ops.rs`) | `[list\|help\|github\|gitlab\|teams\|agents\|plan\|codeindex] [on\|off]` | - |
| the `/tools` dispatch arm | - | the ten words above, plus `--help`/`-h`/`usage` |
| `command_catalog::TOOLS_SUBS` (`commands_info`) | list, show, + the seven switches, help | - |

Three concrete disagreements:

- the **menu omitted `show`**, a form the handler accepts (a completion list that
  cannot complete a working form);
- the **hint omitted `show` and `masterfetch`**, and its trailing `[on|off]` is not a
  subcommand at all - it is a second argument;
- the **help page and two usage warnings were literals** naming only the seven
  switches, so `/tools show` - which works - appeared nowhere a reader could learn it.

## What was changed

`command_catalog::TOOLS_SUBS` is now `pub` and is the one table every one of those
four readers uses:

- `get_command_suggestions("tools")` returns it (map + collect), so the menu offers
  exactly the accepted words;
- `get_parameter_hint("tools")` is rendered from it - `[list|show|github|...|help]` -
  so the menu and its hint cannot disagree;
- the `/tools` dispatch arm computes `forms` (`` `/tools <word>` `` per table entry)
  and `switches` (the table entries the session actually has a switch for, via
  `tool_visibility_state`) from the table, and prints both in its help page and in
  every usage warning; the accepted set is unchanged in behaviour, because the
  table's words are the handler's words;
- `commands_info` publishes the same table (unchanged reader, now `pub`).

No other menu changed. The task adds **no** completion surface: `/tools` is a
*subcommand* menu, not a tool-name menu.

One further consolidation, because FR-033 asks for one catalogue and the pool had two:
`suggest_tool_name` lived twice, once as `session::dispatch::suggest_tool_name` and
once inline as `processor::unknown_tool_error`, each with its own ranking and its own
`edit_distance`. Both now read the new
`crates/ragent-agent/src/session/tool_suggestion.rs`
(`suggest_tool_name` + one `edit_distance`); `dispatch::suggest_tool_name` is a
one-line re-export so no call site or test path moved.

## The property the guard states, and why it drives a foreign registry

10 cases, split where T-037 split the FR-030 guard: four surfaces are reachable from
`ragent-agent`, the menu is a TUI render path.

| # | Case | File | Surface |
|---|---|---|---|
| 1 | `test_the_did_you_mean_pool_is_the_registry_not_the_rename_table` | agent | `suggest_tool_name` |
| 2 | `test_no_retired_spelling_is_ever_suggested` | agent | `suggest_tool_name` |
| 3 | `test_tool_info_names_are_the_registry_it_was_handed` | agent | `tool_info` |
| 4 | `test_the_dry_run_tool_section_is_built_from_the_registry_it_was_handed` | agent | `--dry-run` |
| 5 | `test_commands_info_publishes_the_one_tools_subcommand_table` | agent | `commands_info` |
| 6 | `test_the_prompt_tool_reference_is_the_definitions_it_was_handed` | agent | system prompt |
| 7 | `test_the_tools_menu_offers_exactly_the_one_subcommand_table` | TUI | `/tools` menu + hint |
| 8 | `test_every_offered_tools_subcommand_is_handled` | TUI | menu against handler |
| 9 | `test_the_slash_menu_offers_no_retired_tool_spelling` | TUI | whole command surface |
| 10 | `test_the_prompt_field_offers_no_tool_names` | TUI | the `@` picker |

**Every derived surface is driven with a registry it has never seen** - one stub tool
under a made-up `zzq_` name, registered in reverse so insertion order cannot stand in
for derived order. A surface that reads a constant, a table or the FR-030 baseline
keeps reporting 148 and fails. That is the difference between "it agrees today" and
"it is derived", and the red proofs below are the evidence.

Two points about the pool cases:

- **FR-033 forbids the dead answer, not a suboptimal one.** The pool ranks structural
  matches above edit distance, so `task_create` is answered by `create` (a containment
  match on a live tool) rather than by `TaskCreate`. That is FR-021's ranking policy,
  so case 2 asserts *reachability* and *not-a-retired-spelling* and leaves the ranking
  alone. An earlier draft asserted each case-only pair is answered by its replacement;
  that is a claim FR-021 does not make and the tree does not satisfy, and it was
  removed rather than weakening the pool to fit it.
- **The candidate set is clean, not merely the answers.** Case 2 also asserts no
  registered name is a `before` spelling, which is the statement that the pool cannot
  suggest one.

## The one exemption, pinned

`bash` is a retired *tool* spelling and also a live *slash command* (`/bash` manages the
shell allowlist). Slash vocabulary and tool vocabulary share words, and FR-033 governs
only surfaces that offer a **tool** name, so case 9 exempts a retired spelling that is
also a trigger - and then asserts the set of such spellings is exactly `{"bash"}`, so a
second retired spelling cannot quietly become a command. That set is computed from
`TOOL_RENAMES` and `SLASH_COMMANDS`, so case 9 fails if the exemption stops being
load-bearing; the first draft of that case asserted the wrong spelling (`Bash`, the
`after`) and failed, which is how the assertion was corrected.

## Red proofs

Seven edits applied, observed and reverted. Each file was copied to
`target/temp/t045/*.bak` first, restored with `cp -p`, then `touch`ed - `cp -p`
preserves the old mtime and cargo then reuses the broken object file - and verified
with `md5sum`.

| # | Proof | Case(s) that failed | Observed |
|---|---|---|---|
| 1 | `/tools` menu reverted to the old hand-written list | 7 | `left: ["list","github",...]` vs `right: ["list","show","github",...]` |
| 2 | `/tools` parameter hint reverted | 7 | `left: Some("[list\|help\|github\|...] [on\|off]")` vs `right: Some("[list\|show\|...\|help]")` |
| 3 | `tool_info` reading `crate::tool::create_default_registry()` | 3 | `left: Some(148)` vs `right: Some(2)` |
| 4 | `--dry-run` total pinned to 148 names | 4 | `left: "148 visible tools"` vs `right: "3 visible tools"` |
| 5 | suggestion pool drawn from `TOOL_RENAMES` | 1 and 2 | `left: None` for the stub name; `left: Some("ask_user")` answered with itself |
| 6 | handler reduced to `["list"]` while the menu still offers `show` | 8 | `` `/tools show` is offered by the autocomplete menu but the handler rejects it `` |
| 7 | a retired spelling (`task_get`) added to the `/memory` suggestion list | 9 | `` the `/memory` menu entry offers `task_get` `` |
| 8 | the `@` picker widened to list registered tool names | 10 | `` the `@` file picker offers "websearch", which is a registered tool name `` |

Proof 6 is the other direction of the same rule (offered but refused), and proof 8 is
the negative half of FR-033 - it is the only one whose "fix" would be to add a surface,
and it fails, so the clause is enforced rather than merely written down.

Restored file digests (`md5sum`), all matching the pre-proof copies:

```
b687cc40c13882fe3dc97e282730c30a  crates/ragent-tui/src/app/slash.rs
d31e6ddec7d042c0e210c314a2dd1c59  crates/ragent-tui/src/app/session_ops.rs
9d92315328b70060b07b88b417bf56eb  crates/ragent-agent/src/tool/tool_info.rs
f0f6746f9f26012bebda70e0325311a1  crates/ragent-agent/src/dry_run.rs
71ae9b1f1d7f1fe4dfe2601ad367ef68  crates/ragent-agent/src/session/tool_suggestion.rs
```

## Verification

- `cargo fmt --check` clean.
- `cargo clippy -p ragent-agent -p ragent-tui --all-targets`: no finding in a touched
  file (only the pre-existing `redundant_pub_crate` and
  `unnecessary_structure_name_repetition` noise elsewhere in the workspace).
- `cargo test -p ragent-agent --test all`: **1730 passed, 0 failed, 1 ignored**
  (was 1724 before this task's 6 cases were registered).
- `cargo test -p ragent-tui --test all`: **3652 passed, 3 failed** - the three
  pre-existing `Alt+T` -> `ToggleTasksPanel` failures
  (`test_input_more::alt_toggle_keys_map_to_actions`,
  `test_input_queue_menu_binding::test_other_alt_keys_are_unaffected`,
  `test_memory_panel::test_toggle_memory_mutually_excludes_tasks_panel`), unrelated to
  this task and failing at HEAD.
- Nine guard scripts pass: `check-inline-tests`, `check-file-tool-containment`,
  `check-shared-guards`, `check-silent-errors`, `check-security-unwraps`,
  `check-poison-locks`, `check-dead-code-reasons`, `check-test-env-isolation`,
  `check-tool-rename`.
- FR-030 unchanged: `ragent run --dry-run "hello"` reports **148 visible tools across
  21 families** (`agent` 4, `bash` 3, `team` 19, ...).

## Outstanding, deliberately not fixed here

Registering the two FR-033 files exposed a neighbour with the same problem:
`crates/ragent-agent/tests/test_fr030_surface_agreement.rs` and
`crates/ragent-tui/tests/test_fr030_surface_agreement.rs` are tracked and unregistered
too, so T-037's guard has never run either. Registering the agent half fails to compile
- `isolated_dry_run()` there builds `DryRunInputs` without the `hidden_tools` field the
struct has required since the visibility work landed - so it is a repair, not a
one-line registration, and T-037 is still `pending` in `PLAN.md`. Left as found rather
than folded into this task's diff, and recorded here so the next reader does not
rediscover it.

## Documentation

- `docs/howtos/slashcommands/tools.md` gains a "One table, four readers (FR-033)"
  section naming the four readers, the three disagreements that were fixed, and the
  rule for adding a form (`TOOLS_SUBS` and nowhere else); its invalid-switch example
  now shows the derived list, which includes `/tools show`.
- `docs/howtos/slashcommands/INDEX.md` line 45 restated from the real form list.
- `SPEC.md` §3.2 "Completion and Suggestion Surfaces" records the rule and the table.
- `CHANGELOG.md` Unreleased Added.
