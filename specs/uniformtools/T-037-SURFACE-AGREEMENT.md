# T-037 - The surface is unchanged in size and every enumeration agrees (FR-030)

Status: **completed**. Requirement: FR-030 (spec `uniformtools`). Dependencies:
T-031 (the rename) and T-041 (the recorded pre-rename baseline).

## What the requirement asks

FR-030 has two clauses and they fail differently:

1. the rename **shall** leave the registered tool count unchanged - it adds,
   removes, merges and splits nothing;
2. every enumeration of the surface **shall** remain consistent with the
   registry: `--dry-run`, `tool_info`, `commands_info`, `/tools list`, the
   `Tools registered` line in `/bug-report`, and the outbound tool list sent to
   a provider.

Clause 1 is a single number. Clause 2 is six independent renderers, each of
which could drift from the registry while the number stayed right - a catalogue
that disagrees with the registry is a lie even when every total is 148.

## The numbers, measured on this tree

Everything below was re-measured on the working tree at the time of landing, not
copied from `T-041-BASELINE.md`'s "expected" column:

| Fact | Measured value |
|---|---|
| registered tools (`list()`) | 148 |
| `definitions()` | 148 |
| `all_tools()` | 148 |
| distinct names | 148 |
| names carrying an ASCII uppercase letter | 13 |
| sorted-name sha256 (`\n`-joined, no trailing newline) | `308d92d9d975666cf1f815369e18cc7b457c1c0dcbab004c45f2835100368321` |
| permission families | 21, summing to 148 |
| canary families | `agent` 4, `team` 19 |
| `/tools list` header | `Visible Tools (148 total, 0 disabled):` |
| `--dry-run` total line | `total: 148 visible tools` |
| symmetric difference vs the recorded pre-rename list | 26 (13 removed + 13 added) |

The last row was checked independently rather than asserted: applying the 13
FR-023 pairs to the 148-name before-list quoted in `T-041-BASELINE.md` §3 and
re-sorting reproduces exactly 148 names, 13 uppercase, and the same
`308d92d9...` digest. Nothing else moved.

## What a count alone does not catch, and why the canary is 4 / 19

The family is the leading segment of `permission_category()`, never of the tool
name (T-041 §2.1), and 50 of the 148 names disagree with their family at the
first `_`. `team_wait` is the sharpest case: its category is `agent:spawn`, so
this surface splits `agent` 4 / `team` 19, while a name-prefix reading of the
same 148 names gives `agent` 3 / `team` 20. Both readings total 148, so a
comparison that checked only the total would pass against the wrong one.

This task therefore **corrected** the constant rather than re-asserting it: the
file arrived with `("agent", 3), ("team", 20)`, which is the name-prefix split
and fails on a correct tree. T-041 §8.1 named this file as one of three carrying
the stale pair and assigned the correction to T-037 (T-046 corrected its own
copy the same way). `test_fr028_state_survival.rs`'s comment carried the same
stale pair and is corrected here too. Neither correction relaxes anything: both
numbers still pin a count, and the canary is now the one `--dry-run` prints.

## Delivered as two test files, one per half of the surface

The six enumerations do not live in one crate. Four are reachable from
`ragent-agent`; `/tools list` and `/bug-report` are `ragent-tui` render paths that
`ragent-agent` cannot reach. The guard is split beside the surfaces it pins, and
both halves are registered in their crate's `tests/all.rs`.

### `crates/ragent-agent/tests/test_fr030_surface_agreement.rs` (7 cases)

| # | Case | Surface it pins |
|---|---|---|
| 1 | `test_every_registry_enumeration_reports_the_fr030_total` | `list()`, `definitions()`, `all_tools()` |
| 2 | `test_the_registry_names_match_the_recorded_after_rename_digest` | the names: distinct count, uppercase count, the `after` set, the digest |
| 3 | `test_the_dry_run_tools_section_agrees_with_the_registry` | `--dry-run` (human) |
| 4 | `test_the_dry_run_json_tools_section_agrees_with_the_registry` | `--dry-run --json` |
| 5 | `test_tool_info_reports_the_registry_total_and_every_name` | `tool_info` |
| 6 | `test_commands_info_catalogue_is_intact_and_names_no_retired_tool` | `commands_info` |
| 7 | `test_the_outbound_provider_tool_list_is_the_registry_surface` | the wire list sent to a provider |

### `crates/ragent-tui/tests/test_fr030_surface_agreement.rs` (2 cases)

| # | Case | Surface it pins |
|---|---|---|
| 1 | `test_tools_list_reports_the_fr030_total_and_every_name` | `/tools list` header and every row of both blocks |
| 2 | `test_bug_report_records_the_fr030_total` | the `Tools registered` line in `/bug-report` |

## How each case compares, not merely what it counts

**Case 1** asserts three equal lengths *and* element-for-element equality of the
three name lists. A registry that keyed a tool under one spelling and had it
report another would satisfy three equal lengths and still advertise a name it
cannot dispatch. It also asserts each list holds as many distinct entries as
entries, so a duplicate registration fails as a duplicate rather than as a
length.

**Case 2** is T-041 §6 points 3 to 6 in one place. The uppercase count is the
sharpest cheap check - 12 means the rename missed a tool, 14 means it renamed an
extra one - and the case then asserts the capitalised names are *exactly* the
table's `after` column, so an unrelated capitalised registration cannot
substitute for a missed rename. Finally the digest of the whole sorted list is
compared against `308d92d9...`; that assertion fails when any single name is
wrong without any other assertion enumerating 148 names. The digest *form* is
named in the helper (joined by `\n`, hashed with no trailing newline) because
T-041 §3.1 records both forms: `sha256sum` on a shell-written file includes the
final newline and hashes to `f43ae3c8...`, so comparing the two shapes would
report a newline as a missing tool.

**Cases 3 and 4** compare the readiness report's `tools` section row for row
against a family table *derived from the same registry* the report reads, so a
failure names which side drifted. Both also assert the 21-family count, the two
canaries, and that the family counts sum to 148 - a table that silently omitted a
tool would otherwise still look plausible. Case 4 additionally compares the JSON
encoding's item names against the human encoding's, because TC-024 step 7 reads
the JSON form.

Case 3 holds the crate-wide `global_env_lock()` for its whole body **and** is
`#[serial]`: the report reads the ambient global config, the project config and
both override variables, so a developer's real `~/.config/ragent/ragent.json`
would otherwise take part. The two locks are different mutexes and taking both is
what excludes this test from every other env-mutating test in the binary - the
rule `scripts/check-test-env-isolation.sh` enforces. The guard spans the
`run_dry_run(...).await` deliberately, so the file carries
`#![allow(clippy::await_holding_lock, clippy::future_not_send)]` with that reason
written above it (the same pair `test_fr033_completion_sources.rs` and
`test_gcf_raw_consumers.rs` carry).

The `DryRunInputs` literal passes `hidden_tools: Vec::new()` with a comment
saying why: `dry_run::tools_section` calls `set_hidden(hidden)` and then `list()`,
and `list()` returns every key regardless of the hidden set, so the section's
figure is the *registered* count under any visibility configuration. (Before this
task the literal omitted the field entirely, so the file did not compile - see
"why this file had never run".)

**Case 5** asserts `tool_info`'s `total` is 148 and that its `tools` array
enumerates exactly the registry's names, sorted. It also asserts every entry
carries a non-empty `permission_category` and a non-empty `source`, because a row
the model cannot act on has survived the rename in name only.

**Case 6** treats `commands_info` as what it is - the slash-command catalogue, not
a tool count (T-041 §4). Consistency for it means: the built-in entry count
matches `COMMAND_CATALOG`, no slash trigger is duplicated, no entry offers a
retired tool spelling as a structured `subcommands`/`flags` suggestion unless the
same word is itself a slash command, and no description backticks a retired
spelling as a tool. `/bash` is the clear case for the exemption: `/bash` is a
command and `bash` is also a retired tool, and FR-033 governs only surfaces that
offer a *tool* name.

Its last assertion was **repaired, not relaxed**. As written it pinned `/tools`'s
subcommands to `["list", "show", "help"]`; the catalogue actually publishes the
ten-entry `TOOLS_SUBS` table (`list`, `show`, the seven visibility switches,
`help`). The literal was wrong about the surface it claimed to pin, and
correcting it to the ten words would have been a second copy of a table FR-033
made single-source. It now compares `commands_info`'s list against `TOOLS_SUBS`
itself and then asserts every advertised word is either a `tool_visibility`
switch or one of `list`/`show`/`help`, so the catalogue cannot drift from the
handler in either direction.

**Case 7** proves the outbound list end to end rather than by inspection: one real
turn through a scripted provider, then the `ChatRequest` that actually left the
process is compared against the registry. The list is not a parallel copy -
`get_cached_tool_definitions` returns the cached `definitions()` - but a test that
only read the cache would not prove the request builder used it. The case also
checks that no outbound definition lost its description, and that the
`ToolsSent` event the TUI and SSE stream render from the same vector advertises
the same names.

**TUI case 1** asserts `Visible Tools (<n> total, <m> disabled):` with
`n + m = 148`, reads the data rows of **both** blocks, and compares the sorted set
against the registry. This is the correction T-041 §3.3 spelled out: `/tools list`
is not a fixed 148-row block, and a check that looked only inside the visible
block would read 92 on a default-config machine and call a correct tree a
regression. The file arrived asserting a `Registered Tools (148 total):` header
that no renderer writes - the header `render_tool_visibility_table` composes is
`Visible Tools (N total, M disabled):` - so case 1 failed on a correct tree.

**TUI case 2** reads the written report's `## Tool Configuration` section and its
`- **Tools registered**: 148` line.

## What this task changed outside the two test files

Three things, all of them because the guard could not pass on a correct tree
otherwise. None changes a tool, a tool name, a permission category, a dependency
or an `unsafe` block.

1. **`/bug-report` now writes the line FR-030 names.** `handle_bug_report` wrote
   only `- **Tools visible**: {n}`, where `n` is the count of *enabled* family
   switches out of seven - 1 on the shipped default configuration, never a tool
   count (T-041 §4). It now takes `self.session_processor.tool_registry.list().len()`
   and writes `- **Tools registered**: {registered}` as the first line of
   `## Tool Configuration`, keeping `Tools visible` beside it because the switch
   count is a different, still-useful fact. This is the choice T-041 §6 point 8
   left open ("either T-037 asserts the surface as it will be after the change
   adds the line, or it records the absence") resolved in favour of adding it, as
   FR-030 requires.
2. **`FR030_CANARY_FAMILIES` corrected** from `("agent", 3), ("team", 20)` to
   `("agent", 4), ("team", 19)`, with its doc comment and the
   `test_fr028_state_survival.rs` cross-reference corrected to say *why* the pair
   is what it is (category-derived, not name-derived).
3. **The two `tests/all.rs` registrations.** Neither file had ever been compiled
   or run - the seventh instance of this spec's orphan-file class.

## Why this file had never run

Both guard files were committed in `d6abcd4a` and neither crate's `tests/all.rs`
named them, so no case had ever compiled. Registering the agent half then failed
to build:

```text
error[E0063]: missing field `hidden_tools` in initializer of `DryRunInputs`
  --> crates/ragent-agent/tests/test_fr030_surface_agreement.rs:357:18
```

`DryRunInputs` gained the field when tool visibility landed (`src/main.rs` passes
`config.effective_hidden_tools()`), and the unregistered test was never rebuilt.
With the field supplied and the two registrations added, all nine cases compile
and pass - after the three corrections above.

## Red proofs

Each break was applied, the named case was seen to fail for the named reason, and
the file was restored from `target/temp/t037/*.bak` with `cp -p` and then
`touch`ed (a `cp -p` restore keeps the older mtime and cargo can reuse the object
file built from the broken source, so an unmodified test can still look red).
Every restore was `md5sum`-verified byte-identical.

| # | Break | Cases that went red | Message it produced |
|---|---|---|---|
| 1 | `create_default_registry()` lost `plan::PlanEnterTool` | 1, 2, 4, 5, 7 and `--dry-run` (3) | `ToolRegistry::list() reports 147 names, not the 148`; `147 distinct names in a surface of 148`; `--dry-run reports "147 visible tools"`; `tool_info reports Number(147)`; `the outbound provider list carries 147 tools` |
| 2 | `AFTER_RENAME_DIGEST` set to the pre-rename `a833f0f1...` | 2 | `left: "308d92d9..."` / `right: "a833f0f1..."` |
| 3 | `FR030_CANARY_FAMILIES` back to `("agent", 3), ("team", 20)` | 3 | `the agent family holds Some(4) tools, not the 3 the baseline records` |
| 4 | the `Tools registered` line removed from `handle_bug_report` | TUI 2 | `the bug report has no Tools registered line` |
| 5 | `/tools list` renderer skipped one definition (`defs.iter().skip(1)`) | TUI 1 | `` `/tools list` rendered 147 rows across its visible and disabled blocks, not the 148 the header accounts for `` |

Break 1 is the requirement's headline defect, and it is not hypothetical for this
spec: a previous session left `plan::PlanEnterTool` commented out of
`create_default_registry()` with a `// RED PROOF` marker while it worked on
another task, dropping `EnterPlanMode` and taking the surface to 147. That edit
was not in the tree when this task ran - the registration is present at
`crates/ragent-agent/src/tool/mod.rs:1623`, which is why case 1 passed at 148 on
its first run here - so break 1 above is that state reproduced deliberately, to
record what it looks like. Had the guard been registered at the time, it would
have failed on the next build instead of letting a 147-tool surface sit in the
tree as "the rename changed nothing".

Break 3 also demonstrates that the corrected canary is load-bearing rather than
decorative: with the stale pair restored, case 3 fails on a correct tree with
exactly the message T-041 §8.1 predicted.

## Verification

Run on this tree:

- `cargo fmt --all -- --check` - clean.
- `cargo clippy -p ragent-agent -p ragent-tui --all-targets` - no finding in either
  guard file; the remaining output is the pre-existing `pub(crate)`-inside-a-
  private-module set and other crates' lints.
- `cargo check --workspace --all-targets` - no errors.
- `cargo test -p ragent-agent --test all` - **1737 passed, 1 ignored, 0 failed**
  (103 s).
- `cargo test -p ragent-tui --test all` - **3654 passed, 3 failed**, the three
  pre-existing `Alt+T` -> `ToggleTasksPanel` failures unrelated to this task:
  `test_input_more::alt_toggle_keys_map_to_actions`,
  `test_input_queue_menu_binding::test_other_alt_keys_are_unaffected`,
  `test_memory_panel::test_toggle_memory_mutually_excludes_tasks_panel`. A fourth
  failure (`test_slash_router::reload_with_router_section_succeeds`) appeared in
  one full run and not the next; it passes in isolation and under
  `--test-threads=1`, and is an env-race in that file's own harness, not a
  consequence of this change.
- `cargo test -p ragent-agent --test all test_fr030_surface_agreement::` - 7 passed.
- `cargo test -p ragent-tui --test all test_fr030_surface_agreement::` - 2 passed.
- All nine guard scripts pass: `check-inline-tests`,
  `check-file-tool-containment`, `check-shared-guards`, `check-silent-errors`,
  `check-security-unwraps`, `check-poison-locks`, `check-dead-code-reasons`,
  `check-test-env-isolation`, `check-tool-rename`.
- `ragent run --dry-run "hello"` on the built binary - `total: 148 visible tools`,
  21 family rows, `agent: 4 tools`, `team: 19 tools`, unchanged from the FR-030
  baseline.

## What this task deliberately leaves to its neighbours

- **FR-033's completion surfaces** (T-045). Case 6 asserts the `commands_info`
  catalogue is intact and its `/tools` row equals `TOOLS_SUBS`; it does not
  assert that no completion menu can offer a retired spelling.
- **FR-034's pair-by-pair registry state** (T-046,
  `test_tool_rename_atomic_landing.rs`). This file asserts the *list a user is
  shown*; the atomic-landing guard asserts the resolution state behind it. They
  overlap on purpose and neither is a copy of the other.
- **FR-028's 32 permission categories** (`test_fr028_state_survival.rs`). Case 3
  reads the categories only to derive the family table `--dry-run` prints.
- **NFR-006's per-step cost** (T-040).

## Files touched

| File | Change |
|---|---|
| `crates/ragent-agent/tests/test_fr030_surface_agreement.rs` | `hidden_tools` supplied; canary corrected to 4/19; `commands_info` `/tools` row pinned to `TOOLS_SUBS`; module doc, failure message and two clippy allows corrected; registered |
| `crates/ragent-tui/tests/test_fr030_surface_agreement.rs` | `/tools list` case rewritten around `n + m = 148` and both blocks; module doc records the two measured facts; registered |
| `crates/ragent-tui/src/app/slash.rs` | `handle_bug_report` writes `Tools registered` from `registry.list().len()` |
| `crates/ragent-agent/tests/all.rs`, `crates/ragent-tui/tests/all.rs` | one `#[path]` registration each |
| `crates/ragent-agent/tests/test_fr028_state_survival.rs` | stale canary pair in a comment corrected |
| `SPEC.md` | new §3.2.3 "Tool Surface Agreement" |
| `docs/howtos/slashcommands/bug-report.md` | `Tool Configuration` row describes the new line |
| `CHANGELOG.md` | Unreleased entry |
| `specs/uniformtools/PLAN.md` | T-037 row |
| this document | rewritten from the pre-implementation draft |
