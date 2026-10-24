# T-045 - Every completion and suggestion source is registry-derived (FR-033)

Status: **completed**. Requirement: FR-033 (spec `uniformtools`). Dependency: T-031
(the rename), whose post-rename registry is the catalogue this task reads.

## What the requirement asks

FR-033 names the sources a suggestion can be drawn from and states one rule for all
of them: the registry is the catalogue, and a retired `before` spelling appears in
none of them. It names the slash-command and tool-name menus, the did-you-mean
candidate pool, and the lists printed by `--dry-run`, `tool_info` and `commands_info`.
It also states the negative the task text repeats: **no new completion surface** -
where a field offers no tool name today, it need not start.

Three of the five sources could only ever have been *assumed* to comply, because a
surface that agrees with the registry today is indistinguishable from one that
hard-codes the same answer. The fourth was genuinely wrong, and not in the retired-name
sense the requirement is usually read as: the `/tools` menu offered **seven forms the
handler rejects** and omitted **one it accepts**.

## What was broken

Four hand-written copies of the `/tools` subcommand list existed, in four places that
disagreed:

| Reader | Offered before | Handler accepted before |
|---|---|---|
| `get_command_suggestions` (`crates/ragent-tui/src/app/slash.rs`) | list, github, gitlab, teams, agents, plan, codeindex, masterfetch, help | list, show, help |
| `get_parameter_hint` (`crates/ragent-tui/src/app/session_ops.rs`) | `[list|help|github|gitlab|teams|agents|plan|codeindex] [on|off]` | - |
| the `/tools` dispatch arm | - | `[]`, `list`, `show`, `--help`, `-h`, `usage`, `help` |
| `command_catalog::TOOLS_SUBS` (`commands_info`) | list, show, help | - |

The residue of a removed feature: `/tools <family> on|off` tool-visibility switching
was deleted, and the menu kept advertising switches the handler had stopped accepting.
That is the same class of defect FR-033 is written against - a catalogue offering what
the system refuses - with a live name instead of a retired one. It is also why the task
text says "add no new completion surface": the temptation is to widen a menu rather than
make it agree.

## What was changed

`command_catalog::TOOLS_SUBS` was already the one table `commands_info` published. It
is now `pub` and is the single source the other three read:

- `get_command_suggestions("tools")` returns it, and `get_parameter_hint("tools")` is
  rendered from it (`[list|show|help]`), so the menu and its hint cannot disagree;
- the `/tools` dispatch arm computes `accepted` from it: a bare `/tools`, any single
  word in the table, or one of the retained `--help`/`-h`/`usage` aliases lists; `help`
  prints a table whose rows are generated from it; anything else prints a usage warning
  whose forms are generated from it. The usage warning therefore lists `/tools show`,
  which the old literal did not.

No other menu changed. The task adds no completion surface - the `/tools` menu is a
**subcommand** menu, not a tool-name menu, and it is the only menu whose vocabulary
could have drifted into tool names (`masterfetch`, `teams`, `agents` were family names
that no longer exist as switches).

## The property the guard states, and why it drives a foreign registry

`crates/ragent-agent/tests/test_fr033_completion_sources.rs` (6 cases) and
`crates/ragent-tui/tests/test_fr033_completion_surfaces.rs` (4 cases), split where
T-037 split the FR-030 guard: four surfaces are reachable from `ragent-agent`, the menu
is a TUI render path.

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
for derived order. A surface that reads a constant, a table or the FR-030 baseline keeps
reporting 148 and fails. This is the difference between "it agrees today" and "it is
derived": case 3 fails with `Some(148)` against `Some(2)` when `tool_info` is changed to
read `create_default_registry()` instead of the registry it was handed, which is the
exact edit a future contributor would make while "simplifying".

Two further points about the pool cases:

- **FR-033 forbids the dead answer, not a suboptimal one.** The pool ranks structural
  matches above edit distance, so `task_create` is answered by `create` (a containment
  match on a live tool) rather than by `TaskCreate`. That is a weaker hint and it is
  FR-021's ranking policy, so case 2 asserts *reachability* and *not-a-retired-spelling*
  and leaves the ranking alone. An earlier draft asserted the case-only nine pairs are
  each answered by their replacement; that is a claim FR-021 does not make and the tree
  does not satisfy, and it was removed rather than weakening the pool to fit it.
- **The candidate set is clean, not merely the answers.** Case 2 also asserts no
  registered name is a `before` spelling, which is the statement that the pool cannot
  suggest one.

## The one exemption, pinned

`bash` is a retired *tool* spelling and also a live *slash command* (`/bash` manages the
shell allowlist). Slash vocabulary and tool vocabulary share words, and FR-033 governs
only surfaces that offer a **tool** name, so case 9 exempts a retired spelling that is
also a trigger - and then asserts the set of such spellings is exactly `{"bash"}`, so a
second retired spelling cannot quietly become a command. The first draft of that case
failed on exactly this, which is the exemption proving it is load-bearing rather than
decorative.

## Red proofs

Five edits applied, observed and reverted; each file restored with `cp -p` from a
`target/temp/t045/*.bak` copy, `touch`ed (an older mtime makes cargo reuse the broken
object file), and verified with `md5sum -c`.

| Proof | Case(s) that failed |
|---|---|
| `/tools` menu reverted to the old hand-written list | 7 (`["list","github",...]` vs `["list","show","help"]`) |
| `/tools` parameter hint reverted | 7 (`[list|help|github|...]` vs `[list|show|help]`) |
| `tool_info` reading `create_default_registry()` | 3 (`Some(148)` vs `Some(2)`) |
| `--dry-run` total pinned to `148usize` | 4 (`"148 visible tools"` vs `"3 visible tools"`) |
| suggestion pool drawn from `TOOL_RENAMES` instead of `registry.list()` | 1 and 2 (`ask_user` answered with `ask_user`) |

Two more proofs target the TUI half:

| Proof | Case that failed |
|---|---|
| a retired spelling added to the `/memory` suggestion list | 9 (`/memory` offers `task_get`) |
| the `@` picker widened to list registered tool names | 10 (`@` offers `websearch`) |

A seventh, partially applied proof is worth recording: making the handler reject `show`
while the menu still offered it failed case 8, which is the other direction of the same
rule (`/tools show` offered but refused). That edit did not compile as written (the
catalogue drives `accepted`, so removing `show` from the table also removes it from the
menu); the observed failure is what the case is for.

## Verification

- `cargo fmt --check` clean; `cargo check --workspace --all-targets` clean.
- `cargo clippy -p ragent-agent -p ragent-tui --all-targets`: no new warning (only the
  pre-existing `redundant_pub_crate` noise, the pre-existing `manual_assert` in the
  FR-030 guard, and one pre-existing test `MutexGuard`-across-`await`); the new file's
  `await_holding_lock` is a documented file-level allow, matching `test_loop_telemetry.rs`.
- `cargo test -p ragent-agent --test all`: 1883 passed, 1 ignored.
- `cargo test -p ragent-tui --test all`: 3704 passed.
- Eight guard scripts pass: `check-inline-tests`, `check-file-tool-containment`,
  `check-shared-guards`, `check-silent-errors`, `check-security-unwraps`,
  `check-poison-locks`, `check-dead-code-reasons`, `check-test-env-isolation`.
- FR-030 unchanged: 148 tools, 21 families (verified by `test_fr030_surface_agreement`).

## Documentation

- `SPEC.md` §3.2 "Completion and Suggestion Surfaces" (under the Tool System section)
  records the rule and the one table.
- `docs/howtos/slashcommands/tools.md` and `INDEX.md` no longer advertise the removed
  visibility switches, and the recorded usage warning shows the real form list.
- `docs/howtos/slashcommands/codeindex.md` and `docs/howtos/tutorial.md` lose the
  `/tools <family> on|off` and `/tools github on` instructions.
- `CHANGELOG.md` Unreleased entry.
