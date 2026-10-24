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

## The baseline it compares against

`specs/uniformtools/T-041-BASELINE.md`, measured before T-031 renamed a tool:

| Fact | Pre-rename | Post-rename (expected) |
|---|---|---|
| registered tools | 148 | 148 |
| distinct names | 148 | 148 |
| permission families | 21, summing to 148 | 21, summing to 148 |
| canary families | `agent` 3, `team` 20 | unchanged |
| names carrying an ASCII uppercase letter | 0 | 13 |
| sorted-name digest (`\n`-joined, no trailing newline) | `a833f0f1...` | `308d92d9...` |
| symmetric difference between the two name lists | - | 26 (13 removed + 13 added) |

T-041 §2.1 records why the total alone is not the baseline: the stale release
binary in `target/` also reports 148 with a different split (`agent` 4 /
`team` 19). The canary pair is what distinguishes the two builds.

## Delivered as two test files, one per half of the surface

The six enumerations do not live in one crate. Five are reachable from
`ragent-agent`; `/tools list` and `/bug-report` are `ragent-tui` render paths
that `ragent-agent` cannot reach. The guard is therefore split beside the
surfaces it pins, and both halves are registered in their crate's `tests/all.rs`.

### `crates/ragent-agent/tests/test_fr030_surface_agreement.rs` (7 cases)

| # | Case | Surface it pins |
|---|---|---|
| 1 | `test_every_registry_enumeration_reports_the_fr030_total` | `list()`, `definitions()`, `all_tools()` |
| 2 | `test_the_registry_names_match_the_recorded_after_rename_digest` | the names themselves: uppercase count, the `after` set, the digest |
| 3 | `test_the_dry_run_tools_section_agrees_with_the_registry` | `--dry-run` (human) |
| 4 | `test_the_dry_run_json_tools_section_agrees_with_the_registry` | `--dry-run --json` |
| 5 | `test_tool_info_reports_the_registry_total_and_every_name` | `tool_info` |
| 6 | `test_commands_info_catalogue_is_intact_and_names_no_retired_tool` | `commands_info` |
| 7 | `test_the_outbound_provider_tool_list_is_the_registry_surface` | the wire list sent to a provider |

### `crates/ragent-tui/tests/test_fr030_surface_agreement.rs` (2 cases)

| # | Case | Surface it pins |
|---|---|---|
| 1 | `test_tools_list_reports_the_fr030_total_and_every_name` | `/tools list` header and all 148 rows |
| 2 | `test_bug_report_records_the_fr030_total` | the `Tools registered` line in `/bug-report` |

## How each case actually compares, not merely what it counts

**Case 1** asserts three equal lengths *and* element-for-element equality of the
three name lists. A registry that keyed a tool under one spelling and had it
report another would satisfy three equal lengths and still advertise a name it
cannot dispatch. It also asserts each list has as many distinct entries as
entries, so a duplicate registration is a failure rather than a length.

**Case 2** is T-041 §6 in one place. The uppercase count is the sharpest cheap
check - 12 means the rename missed a tool, 14 means it renamed an extra one - and
the case then asserts the capitalised names are *exactly* the table's `after`
column rather than merely the same number of them, so an unrelated capitalised
registration cannot substitute for a missed rename. Finally the digest of the
whole sorted list is compared against T-041 §3.1's predicted `308d92d9...`; that
assertion fails when any single name is wrong and no other assertion has to
enumerate the 148.

The digest form is named in the helper rather than assumed: T-041 §3.1 records
both the trailing-newline (`f43ae3c8...`) and no-trailing-newline
(`308d92d9...`) forms because `sha256sum` on a shell-written file includes the
newline and an in-process digest does not. `names_digest` joins with `\n` and
hashes without a trailing newline, and its doc comment says so, so a future
reader comparing the two does not report a newline as a missing tool.

**Cases 3 and 4** compare the readiness report's `tools` section row for row
against a family table *derived from the same registry* the report reads. That
makes the failure message name which side drifted. Both also assert the 21-family
count, the two canaries, and that the family counts sum to 148 - a family table
that omitted a tool would otherwise still look plausible. Case 4 additionally
compares the JSON encoding's item names against the human encoding's, because
TC-024 step 7 reads the JSON form.

Case 3 is `#[serial]` **and** holds the crate-wide `global_env_lock()`: the
report reads the ambient global config, the project config and both override
variables, so a developer's real `~/.config/ragent/ragent.json` would otherwise
take part. The two locks are different mutexes and taking both is what excludes
this test from every other env-mutating test in the binary - the rule
`scripts/check-test-env-isolation.sh` enforces.

**Case 5** asserts `tool_info`'s `total` is 148 and that its `tools` array
enumerates exactly the registry's names, sorted. It also asserts every entry
carries a non-empty `permission_category` and a non-empty `source`, because a
row the model cannot act on is a catalogue entry that survived the rename in
name only.

**Case 6** treats `commands_info` as what it is - the slash-command catalogue,
not a tool count (T-041 §4). Consistency for it means the built-in entry count
matches `COMMAND_CATALOG`, no slash trigger is duplicated, no entry offers a
retired tool spelling as a structured `subcommands`/`flags` suggestion unless the
same word is itself a slash command, and no description backticks a retired
spelling as a tool. `/bash` is the clear case for the exemption: `/bash` is a
command and `bash` is also a retired tool, and FR-033 governs only surfaces that
offer a *tool* name. It also pins `/tools`'s subcommand list against the `list`,
`show`, `help` the handler accepts.

**Case 7** proves the outbound list end to end rather than by inspection: a real
turn through a scripted provider, then the `ChatRequest` that actually left the
process is compared against the registry. The list is not a parallel copy -
`get_cached_tool_definitions` returns the cached `definitions()` - but a test that
only read the cache would not prove the request builder used it. The case also
checks the `ToolsSent` event the TUI and SSE stream render from the same vector.

**The TUI cases** are the mirror: `/tools list`'s header, its 148 data rows and
their names, and `/bug-report`'s written report - the line and the
`## Tool Configuration` section T-041 §5 reads. Both build the `App` on
`create_default_registry()`, so the rows are compared against the same registry
the renderer reads.

## Red proofs

The guard was seen to fail for the right reason three times. Each was reverted
before the next.

### 1. A missing registration (found, not injected)

The first run of case 1 failed at 147, not 148:

```text
FR-030: `ToolRegistry::list()` reports 147 names, not the 148
`T-041-BASELINE.md` recorded before the rename
```

The cause was a leftover red-proof edit still in the working tree:
`create_default_registry()` had `plan::PlanEnterTool` commented out with a
`// RED PROOF: one registration removed` marker, so `EnterPlanMode` was not
registered. Restoring the registration took the surface to 148 and cases 1, 3,
4, 5 and 7 all passed. This is the requirement's headline defect occurring in a
real tree - a rename task that also dropped a registration - and the guard caught
it rather than letting a 147-tool surface ship as "the rename changed nothing".
The marker was left in the tree by an earlier session; nothing in this task
commented out a registration.

### 2. The wrong digest

`AFTER_RENAME_DIGEST` was rewritten to the pre-rename value `2364a1e0...` and
case 2 failed:

```text
left:  "308d92d9d975666cf1f815369e18cc7b457c1c0dcbab004c45f2835100368321"
right: "2364a1e0b8dc22bf1d25677664b62605891cfe19fc45b4752dd8bc36246cfe4f"
```

The restored file was `touch`ed before re-running: a `cp -p` restore leaves an
older mtime, and cargo can then reuse the object file built from the broken
source, so the unmodified test still appears to fail. Both digests are recorded
in the failure output, so a reader sees which list the tree actually holds.

### 3. The baseline itself, measured independently

The `--dry-run` figures were not taken on trust from T-041. The binary was built
at this tree and run:

```text
tools: ready
  [  ready] total: 148 visible tools
  [  ready] agent: 3 tools
  ... 21 families ...
  [  ready] team: 20 tools
```

and the registry's own name list was dumped through the public API, sorted and
digested in the shell: **148 names, 148 distinct, 13 uppercase, digest
`308d92d9d975666cf1f815369e18cc7b457c1c0dcbab004c45f2835100368321`** - the
post-rename figure T-041 §3.1 predicted. Applying the 13 FR-023 pairs to the
recorded pre-rename list and re-sorting reproduces the same 148 lines and the
same digest, so the symmetric difference is exactly the table's 26 lines and
nothing else moved.

The shell digest and the in-process digest agree only because the shell form was
taken with `printf '%s' "$(cat ...)"` (no trailing newline); `sha256sum` on the
file itself yields `f43ae3c8...`, the other recorded form. That is the newline
trap T-041 §3.1 documents, observed.

## What was not changed

No registration was added or removed by this task beyond restoring the one a
previous session had commented out for its own red proof. No tool, name,
permission category, dependency or `unsafe` block changed. The files this task
adds or edits are two test files, their two `tests/all.rs` entries, this
document, the `CHANGELOG.md` Unreleased entry and the `PLAN.md` T-037 row.

## What this task deliberately leaves to its neighbours

- **FR-033's completion surfaces** (T-045). Case 6 asserts the `commands_info`
  catalogue is intact and names no retired spelling *as a tool*; it does not
  assert that no completion menu can offer one.
- **FR-034's pair-by-pair registry state** (T-046,
  `test_tool_rename_atomic_landing.rs`). This file asserts the *list a user is
  shown*; the atomic-landing guard asserts the resolution state behind it. They
  overlap on purpose and neither is a copy of the other.
- **FR-028's 32 permission categories** (`test_fr028_state_survival.rs`). Case 3
  reads the categories only to derive the family table `--dry-run` prints.
- **NFR-006's per-step cost** (T-040).

## Verification

All green, run on this tree:

- `cargo fmt --check`.
- `cargo clippy -p ragent-agent -p ragent-tui --all-targets` - no new lint; the
  only remaining noise is the pre-existing `pub(crate)`-in-private-module set.
- `cargo test -p ragent-agent --test all` - **1858 passed, 1 ignored**.
- `cargo test -p ragent-tui --test all` - see the run recorded beside this
  document's landing.
- `cargo test -p ragent-agent --test all test_fr030_surface_agreement::` -
  7 passed.
- `cargo test -p ragent-tui --test all test_fr030_surface_agreement::` -
  2 passed.
- `scripts/check-test-env-isolation.sh` and the other guard scripts pass;
  `check-bench-regression.sh` needs a `cargo bench --save-baseline` run that no
  task in this spec performs, and is unaffected.
