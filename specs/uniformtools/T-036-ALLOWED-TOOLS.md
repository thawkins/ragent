# T-036 - `allowed_tools` resolve exactly, and a stale entry is reported (FR-029)

Status: **completed**. Requirement: FR-029 (spec `uniformtools`). Dependency: T-031
(the rename).

## What the requirement asks

FR-029 is an *unwanted-behaviour* requirement with two clauses:

1. when a skill declares `allowed_tools` entries using the Anthropic spellings the
   rename adopts (`AskUserQuestion`, `Bash`, `Edit`, `Glob`, `Grep`, `EnterPlanMode`,
   `ExitPlanMode`, `Skill`, `TaskCreate`, `TaskGet`, `TaskList`, `TaskUpdate`,
   `Write`), they resolve against the registry **with no alias translation**, because
   the registry now holds those names;
2. an entry that matches no registered tool is **reported once**, naming the entry
   and the skill (or agent) that declared it, rather than silently shipping a smaller
   tool surface.

## What already satisfied clause 1 (and what changed for it: nothing)

The resolution path was already exact, and this task added no step to it:

- `SkillInfo::allowed_tools` is parsed verbatim from the `SKILL.md` frontmatter
  (`allowed-tools`, single string or list) by `skill::loader`.
- `tool::build_allowed_tool_set` inserts each entry **verbatim**; the per-step filter
  in `session/loop_steps.rs` and the `/prompt` mirror in `ragent-tui/src/app/prompt.rs`
  keep a definition when its `name` equals an entry.
- `skill::invoke::resolve_inline_skill_agent` and `resolve_forked_skill_agent` copy the
  list onto `AgentInfo::allowed_tools` unchanged.

Nothing consults the FR-023 table. That *is* the clause, and it is now **asserted**
rather than incidental: `test_the_rename_table_is_not_consulted_when_resolving_an_entry`
requires every retired spelling to be unresolved while its `after` replacement
resolves, and requires `Read` - the spelling the derivation rated `Close` and did not
adopt - to be unresolved too, so `read` is not silently substituted for it.

## What was built for clause 2

### `crates/ragent-agent/src/tool/allowed_tools.rs`

| Item | Purpose |
|---|---|
| `unresolved_allowed_tools(&ToolRegistry, &[String]) -> Vec<&str>` | the exact-name comparison, made once where the restriction is applied; document-tested with adopted, kept and retired spellings in one list |
| `report_unresolved_allowed_tools_once(&ToolRegistry, declarer, &[String]) -> usize` | logs each unresolved entry once per `(declarer, entry)`; returns the count so "first report" and "already reported" are distinguishable |
| `report_skill_allowed_tools_once(&ToolRegistry, skill_name, &[String])` | the one-line form the call sites use, so all three produce the same declarer text and the ledger key cannot disagree |
| `unresolved_entry_report(declarer, entry) -> String` | the report sentence, separated so the wording is assertable without installing a subscriber |
| `unresolved_entries_seen()` / `reset_unresolved_allowed_tools_for_test()` | the process-wide, poison-tolerant ledger, matching `alias_table::report_unresolved_targets_once` and `hooks::report_retired_matcher_spellings_once` |

The ledger is keyed on `(declarer, entry)`, **not** on `entry`. A second skill that
names the same stale spelling has its own manifest to fix, so suppressing it would hide
a second defect; a second *distinct* stale entry in the same skill reports for the same
reason.

### The three call sites - where a restriction is actually applied

| Site | Change |
|---|---|
| `tool/skill_manage.rs` `read_or_load` | reports the skill's list with `&ctx.tool_registry` after the skill is found, before the prompt is returned - so `Skill action=read\|load` names the manifest |
| `ragent-tui/src/app/slash.rs` `invoke_skill_command` | reports with `&self.session_processor.tool_registry` after the pack is found and before the turn is spawned - the `/skill <name>` and bare `/<name>` surfaces share it |
| `skill/invoke.rs` `invoke_forked_skill` | reports with `&processor.tool_registry` before the forked turn runs - the `context: fork` path, whose allowance is the skill's own list |

`report_skill_allowed_tools_once` therefore takes the **skill name and list**, not a
`&SkillInfo`: the forked path only carries a `SkillInvocation`, which has the name and
the list but is not a `SkillInfo`. Taking the two values keeps all three sites on one
signature and one declarer text (`skill `<name>``), so the ledger cannot be keyed two
ways by two call sites.

Deliberately **not** on the system-prompt path: that path walks every discovered
manifest on every turn, including third-party plugin skills the session never runs, and
would warn about entries that constrain nothing that turn (NFR-006). Reporting where
the restriction is applied means the warning appears when the manifest is used, and the
ledger makes a second invocation silent.

`AgentInfo::allowed_tools` is only ever populated from a skill's list, so no agent
profile can be the declarer today. The module doc records that, and
`report_unresolved_allowed_tools_once` takes a declarer string, so a loader that starts
reading an `allowed_tools` key from a profile can report it without a new code path.

## Tests - `crates/ragent-agent/tests/test_fr029_allowed_tools.rs` (10 cases)

| # | Case | Pins |
|---|---|---|
| 1 | `test_every_adopted_claude_spelling_resolves_without_translation` | all 13 adopted spellings are registered **and** are exactly the table's `after` column |
| 2 | `test_the_rename_table_is_not_consulted_when_resolving_an_entry` | every `before` is unresolved while its `after` resolves; `Read` is unresolved and `read` is registered |
| 3 | `test_a_skill_declaring_adopted_spellings_resolves_every_entry` | the TC-026 manifest (`read`, `Grep`, `Glob`) reports nothing |
| 4 | `test_an_unknown_entry_is_reported_once_naming_the_entry_and_the_skill` | the report, its count, its silence on repeat, and the sentence naming entry + skill |
| 5 | `test_a_second_skill_naming_the_same_entry_still_reports` | the ledger is per declarer; a second entry in one skill is a second defect |
| 6 | `test_a_correct_manifest_is_silent` | empty and resolving lists are silent; every bundled skill's own list resolves |
| 7 | `test_the_allowed_set_holds_the_entry_verbatim` | `build_allowed_tool_set` stores the entry and gains no retired spelling |
| 8 | `test_an_adopted_spelling_matches_exactly_one_definition` | one definition per adopted spelling; a stale allowance admits nothing from it while the always-allowed control tools stay |
| 9 | `test_skill_manage_reports_a_stale_entry_when_it_applies_the_restriction` | the `Skill` wiring: driving `Skill action=read` on a manifest with `ReadFile` consumes the ledger entry |
| 10 | `test_all_three_restriction_sites_report_and_the_prompt_path_does_not` | all three sites name the helper, the three per-turn prompt paths name neither helper, and the skill form is defined exactly once |

A further case lives in the TUI crate, because the `/skill` trigger is its own call
site: `crates/ragent-tui/tests/test_skill_command.rs`
`test_skill_load_reports_a_stale_allowed_tool_entry` drives
`App::execute_slash_command("/skill typo")` over a pack declaring `Bash` + `ReadFile`,
then asserts the `(skill `typo`, ReadFile)` ledger entry is already consumed - i.e. the
slash surface reported it while applying the restriction.

Case 10 is a source scan rather than a runtime drive because the forked resolver needs a
full `SessionProcessor`; the property worth pinning there is chiefly the *negative* -
that the per-turn prompt path reports nothing - and a scan states that directly.

### RED proofs (each run, observed failing, then restored with `cp -p` + `touch`)

| Injected defect | Failing case(s) | Outcome |
|---|---|---|
| `unresolved_allowed_tools` translates a retired spelling through `Rename::after_of` (an alias layer) | 2 | `the retired spelling `ask_user` must not resolve` - `left: []`, `right: ["ask_user"]` |
| the `Skill` (`skill_manage`) call site removed | 9 | `left: 1`, `right: 0` |
| the TUI `/skill` call site removed | the TUI case above | `left: 1`, `right: 0` |
| the forked-resolver call site removed | 10 | `crates/ragent-agent/src/skill/invoke.rs must report a stale `allowed_tools` entry where it applies the restriction (FR-029)` |
| a report added to `ragent-tui/src/app/prompt.rs` (the per-turn path) | 10 | `... builds the tool surface every turn; reporting there would warn about manifests the session never runs ... (FR-029, NFR-006)` |

The last two are why case 10 exists as a source scan: the forked resolver cannot be
driven without a full `SessionProcessor` + provider, and the property that matters most
there is the *negative* one - that no per-turn path reports - which a scan states
exactly and a runtime drive could only infer from the absence of a warning.

Restoration note: `cp -p` preserves the *old* mtime, so cargo reuses the broken object
file and the "restored" tree still fails. After copying a backup back, `touch` the file;
the `md5sum` before/after is the proof the source is byte-identical again.

## Gotcha that cost the most time

`allowed-tools: Bash, ReadFile` parses as **one** entry, the whole string
`"Bash, ReadFile"` - YAML reads a bare comma list as a scalar. The case-9 probe showed
`allowed_tools=["Bash, ReadFile"]`, which made the test pass for the wrong reason: one
unresolved entry, so the count happened to match. The fixture writes the block form
(`allowed-tools:` then `- Bash` / `- ReadFile`), and case 3 keeps the scalar form for a
*single* entry. Claude Code's documented syntax is a comma list, so a manifest copied
from it restricts to exactly one unregistered name; that is another reason FR-029's
report exists. The same trap is documented for users in `docs/howtos/tools/skills.md`.

Second gotcha: `allowed_tools.rs` existed on disk but was **not declared** in
`crates/ragent-agent/src/tool/mod.rs`, so nothing compiled it and its public API did not
exist; `test_fr029_allowed_tools.rs` had no `#[path]` line in `tests/all.rs` either, so
it never ran. Both are now wired (`pub mod allowed_tools;`, and the registration after
`test_fr028_state_survival`).

## Docs

- `docs/howtos/tools/skills.md` gained "Tool restrictions (`allowed-tools`)": the exact
  spellings, the two groups (adopted Claude names, and the tools that kept their
  ragent names), the `Read` trap, case sensitivity, the empty-list trap, the warning
  text, and the comma-list trap.
- `SPEC.md` §12 gains **12.5 Tool restrictions (`allowed-tools`)** and §12.3's skill
  example was replaced with a real `SKILL.md` frontmatter block (the old one showed a
  `prompt:`/`tools:`/`context_files:` YAML shape the loader does not read, and
  `tools: [read, edit, bash]` named two retired spellings).
- `QUICKSTART.md`'s skill-features bullet now shows `[Bash, read]` and states the
  exact-resolution rule instead of `[bash, read]`.
- `crates/ragent-agent/DOCS.md` lists `allowed_tools` alongside `alias_table` and
  `rename` as a data-module rather than a tool.
- `CHANGELOG.md` Unreleased `Added` entry; `specs/uniformtools/PLAN.md`
  T-036 -> `completed`.

## Verification

- `cargo fmt --check` clean.
- `cargo test -p ragent-agent --test all` -> **1713 passed, 0 failed, 1 ignored**.
- `cargo test -p ragent-agent --test all test_fr029_allowed_tools::` -> 10 passed.
- `cargo test -p ragent-tui --test all test_skill_command::` -> 8 passed.
- `cargo test -p ragent-tui --test all` -> 3648 passed, 3 failed. The three are
  pre-existing and unrelated: `test_input_more::alt_toggle_keys_map_to_actions`,
  `test_input_queue_menu_binding::test_other_alt_keys_are_unaffected` and
  `test_memory_panel::test_toggle_memory_mutually_excludes_tasks_panel`, all the
  `Alt+T` -> `ToggleTasksPanel` input mapping, which this task does not touch.
- `cargo test -p ragent-agent --doc allowed_tools` -> 1 passed (the module's own
  doc example, which is the module's documented contract).
- `cargo clippy -p ragent-agent -p ragent-tui --all-targets` -> no warning in the new
  module or either test; the remaining warnings are the pre-existing
  `redundant_pub_crate` set plus one pre-existing `manual_assert_eq` in
  `crates/ragent-tui/tests/test_text_selection.rs`.
- All 9 `scripts/check-*.sh` guards pass (the bench guard reports "no criterion
  results directory", which is its documented no-baseline state, not a failure).
