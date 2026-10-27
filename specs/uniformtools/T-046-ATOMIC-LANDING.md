# T-046 - The rename lands as one atomic step (FR-034)

**Spec ID:** uniformtools
**Task:** T-046
**Requirement:** FR-034 (`The rename never ships two spellings at once`)
**Status:** completed - the guard, the landing rule and the measurement are
recorded below and verified on this tree. The one thing the task does **not**
produce is the commit itself, which FR-034 clause 2 still blocks on work later
tasks owe; §6 states exactly what remains and why none of it is this rename.
**Depends on:** T-030 (the FR-023 table), T-041 (the pre-rename baseline)

FR-034 is a *state-driven* requirement: it does not describe a feature, it
constrains **every commit** the change lands in. Each such commit must leave the
tree where

1. exactly one spelling of each FR-023 pair resolves - never both, never
   neither;
2. the build and the workspace test suites pass; and
3. `--dry-run` reports the FR-030 count recorded in `T-041-BASELINE.md`.

The reason given in the specification is a bisect: a commit that resolves both
spellings still honours a retired name (FR-026 violated), and a commit that
resolves neither leaves a tool the model cannot call at all. Either way `git
bisect` lands on a tree whose failure exists at neither end of the change, and
the reader debugs a bug that was never shipped.

This document records the three deliverables of the task: the **mechanical
guard** that makes clauses 1 and 3 checkable at every commit (§1), the
**measurement** of the cut on both sides (§4), and the **landing rule** the
eventual commit has to follow, together with the evidence that this tree is on
the correct side of it (§5). §6 states why the commit itself is still somebody
else's to make.

The distinction matters for reading the status: T-046 as a task is the *rule and
its enforcement*, and both exist and pass. The commit FR-034 constrains cannot
happen until the migration tasks it depends on are done, and a task that refused
to record its rule until then would leave the tree unguarded exactly while it is
most likely to be broken.

## 1. The tree-state guard

`crates/ragent-agent/tests/test_tool_rename_atomic_landing.rs` - 7 cases, all
passing, and **registered** (see §7.1: the file shipped unregistered, so until
this task its 7 cases reported `running 0 tests` and exited 0). It reads the
FR-023 table and asserts the **live registry** agrees with it, across every
enumeration the build offers:

| # | Case | Clause it pins |
|---|---|---|
| 1 | `test_the_rename_resolves_exactly_one_spelling_of_every_pair` | 1 - one spelling, and it is `after`, in `list()`, `all_tools()` and `definitions()` |
| 2 | `test_no_before_spelling_is_reachable_by_lookup_or_by_enumeration` | 1 - `get(before)` is `None` and no key carries a `before` |
| 3 | `test_registry_keys_tool_names_and_definition_names_agree` | 1 - the three enumerations are element-for-element identical |
| 4 | `test_every_registration_is_stored_under_the_name_its_tool_reports` | the half-renamed detector (below) |
| 5 | `test_the_renamed_tools_keep_the_permission_family_they_had_before_the_rename` | 2 - FR-028's counts, 21 families, `agent` 4 and `team` 19 |
| 6 | `test_the_fr030_surface_total_is_the_baseline_at_the_rename` | 3 - 148 names and 148 definitions |
| 7 | `test_the_registry_holds_no_duplicate_registration` | 1 - no enumeration repeats a name |

### 1.1 The half-renamed detector (case 4)

This is the case the requirement exists for. A rename has **two** moving parts
per tool: the registry key (`register` stores `tool.name().to_string()`) and the
`Tool::name()` return itself. A tree that moves only one of them compiles, and
the two surfaces disagree:

- key moved, name not - `definitions()` advertises the **retired** spelling to
  the model while `get(before)` works, so a `before` call succeeds and an `after`
  call fails;
- name moved, key not - the reverse.

Case 4 asserts, for all 13 pairs, that the tool stored under the `after` key
reports `after` from `name()`, and that no registered tool reports any of the 13
`before` spellings. Cases 2 and 3 catch the same defect from the enumeration
side; case 4 catches it when a lookup and a listing would each look right alone.

### 1.2 Proven to fail on a half-renamed tree

The guard was run against three deliberately broken trees, each reverted
afterwards (`git status` clean on both files, `diff` byte-identical to the
pre-probe copies):

| Break | What the guard reported |
|---|---|
| `write.rs` `Tool::name()` returns `"write"` again | cases 1, 2 and 4 red. Case 1: `must resolve exactly one spelling of the pair "write" -> "Write", but it holds 1 "write" and 0 "Write"`. Case 2: `registry.get("write") still returns a tool`. Case 4: `` `after` "Write" is not registered, so the rename did not land `` |
| `aliases.rs` renames `UpdateFileTool` to `"write"`, so a second `"write"` registers beside `Write` | cases 1, 2 and 4 red. Case 1 counted **both** spellings (`it holds 1 "write" and 1 "Write"`) - the "registers `after` while `before` still resolves" state. Case 4: `a registered tool still reports the retired spelling "write". The registry key and Tool::name() both have to move for the rename to be atomic.` |
| `write.rs` `Tool::name()` returns `"WriteX"` (neither column) | cases 1 and 4 red. Case 1 counted **neither** spelling (`it holds 0 "write" and 0 "Write"`) - the "removes `before` while `after` is not registered" state. Case 4: `` `after` "Write" is not registered `` |

All three states FR-034 names - both, and neither - are therefore red, and each
failure message names the pair, the surface and the direction of the error.

Case 4 has two assertions (the `after` key must exist and must report `after`;
then no registered tool may report a `before`). Which of the two fires depends on
the break: a lost key trips the first, a surviving `before` reporting tool trips
the second. Both messages name the same defect from opposite sides, which is why
the table above records the one each break produced rather than one message for
all three.

## 2. What the guard deliberately does not check

- **The commit graph.** "The commit that registers `after` is the commit that
  removes `before`" is a property of history, not of a tree. A test that shelled
  out to `git log` would fail in a source tarball, would need `git` present in
  CI, and would duplicate a review that has to happen anyway. It is verified by
  review (TC-030 step 9 of `TESTPLAN.md`) and recorded in §5.
- **A retired spelling in `tools.aliases`.** Refusing that entry at load is
  FR-031's own clause and the alias-entry task's guard. A case here would be red
  for a reason outside FR-034's tree-state claim.
- **`before` literals in non-test source.** The FR-027 source scan owns that;
  `test_escape_hatch_audit.rs` owns the configuration and environment surfaces.
  Repeating either here would make three guards to keep in step.
- **The FR-023 table's own shape.** `test_tool_rename_table.rs` owns the 13
  pairs, the ordering and the two column-uniqueness rules. This file reads the
  table and asserts the registry agrees with it.

## 3. Where the two sides of the cut are

The rename is one edit per tool, and both edits sit in the same `impl` block, so
they cannot be split across two commits by accident - but they *can* be half-applied
by a careless merge or a partial revert, which is what §1.2 exercises.

The 13 sites, each `fn name(&self) -> &'static str` returning one string:

| Tool | `before` (at `HEAD`) | `after` (working tree) | File |
|---|---|---|---|
| `ask_user` | `"ask_user"` | `"AskUserQuestion"` | `crates/ragent-agent/src/tool/aliases.rs:190` |
| `bash` | `"bash"` | `"Bash"` | `crates/ragent-tools-core/src/bash.rs:1486` |
| `edit` | `"edit"` | `"Edit"` | `crates/ragent-tools-core/src/edit.rs:103` |
| `plan_enter` | `"plan_enter"` | `"EnterPlanMode"` | `crates/ragent-agent/src/tool/plan.rs:24` |
| `plan_exit` | `"plan_exit"` | `"ExitPlanMode"` | `crates/ragent-agent/src/tool/plan.rs:113` |
| `glob` | `"glob"` | `"Glob"` | `crates/ragent-tools-core/src/glob.rs:27` |
| `grep` | `"grep"` | `"Grep"` | `crates/ragent-tools-core/src/grep.rs:39` |
| `skill_manage` | `"skill_manage"` | `"Skill"` | `crates/ragent-agent/src/tool/skill_manage.rs:34` |
| `task_create` | `"task_create"` | `"TaskCreate"` | `crates/ragent-tools-extended/src/task.rs:164` |
| `task_get` | `"task_get"` | `"TaskGet"` | `crates/ragent-tools-extended/src/task.rs:909` |
| `task_list` | `"task_list"` | `"TaskList"` | `crates/ragent-tools-extended/src/task.rs:1015` |
| `task_update` | `"task_update"` | `"TaskUpdate"` | `crates/ragent-tools-extended/src/task.rs:490` |
| `write` | `"write"` | `"Write"` | `crates/ragent-tools-core/src/write.rs:20` |

`ask_user` is registered through `aliases::AskUserTool`, so its `impl` block is
the third one in that file (`update_file`, `run_code`, `ask_user`); a plain
`grep -n 'fn name'` reports the first two before it.

Note that `Tool::name()` is the **only** name source that matters here.
`permission_category()` deliberately keeps its `before`-era values - `"ask_user"`,
`"bash"`, `"task"` - because FR-028 holds the permission vocabulary unchanged.
Those strings are permission families, not tool names, and the FR-027 source
scan carries them as per-line exemptions for that reason.

## 4. Clause 3: the FR-030 count at the cut

Measured from the tree that carries the rename, against
`T-041-BASELINE.md`:

| Surface | Baseline (before) | This tree (after) | Clause |
|---|---|---|---|
| `ragent --dry-run` `total` | 148 | **148** | 3 |
| `ToolRegistry::list()` | 148 | **148** | 3 |
| `ToolRegistry::definitions()` | 148 | **148** | 3 |
| families | 21 | **21** | 2 (FR-028) |
| `agent` family | 4 | **4** | 2 - canary |
| `team` family | 19 | **19** | 2 - canary |
| names with an ASCII uppercase letter | 0 | **13** | 1 - the sharpest count |
| sorted-name digest, no trailing newline | `a833f0f1...` | **`308d92d9d975666cf1f815369e18cc7b457c1c0dcbab004c45f2835100368321`** | 1 |
| symmetric difference of the two lists | - | **26 (13 removed + 13 added)** | 1 |

Every removal is a `before` and every addition an `after` - `removals_not_in_table=0`
and `additions_not_in_table=0` - so the 26-line diff is exactly the table and
nothing else. The digest matches the one `T-041-BASELINE.md` *predicted* before
the rename started, which is the independent confirmation that no tool was
missed and none was renamed twice.

The `agent` 4 / `team` 19 canary matters here because the family is derived from
`permission_category()`, not from the tool name: `team_wait` is an `agent:spawn`
tool, so a landing check that counted the 20 `team_*`-named tools as the `team`
family would report 3 / 20 and pass a total-only comparison. Case 5 of the guard
pins both canaries (T-041 §2.1).

### 4.1 Reproducing the numbers

The first six rows come straight from the binary, with no scratch code:

```sh
cargo build
./target/debug/ragent run --dry-run "x"        # total + the 21 family rows
```

The last three need the name list, which two public calls already expose:

```rust
let registry = ragent_agent::tool::create_default_registry();
let names = registry.list();                    // sorted, 148
let uppercase = names.iter().filter(|n| n.chars().any(|c| c.is_ascii_uppercase())).count(); // 13
```

The digest is `sha256` over `names.join("\n")` with **no** trailing newline, and
the symmetric difference is taken against the pre-rename list reconstructed by
applying `TOOL_RENAMES` backwards - which is why the reconstructed list's own
digest is worth printing: it came out
`a833f0f15b5ac3b8e9bb5cb1c0f35aa51ab84e424f7659bddab8dc409c143d1b`, the *measured*
before-list digest in `T-041-BASELINE.md`, not merely a similar one. A single
missed or doubled pair would have moved that value, so the agreement is the
independent proof that the rename applied exactly the table.

The scratch example that produced these figures (uppercase list, digest,
symmetric difference, all in one `examples/t046_measure.rs`) was deleted after
the run, and `crates/ragent-agent/examples/` - which did not exist before the
measurement - was removed with it, so `git status --porcelain` reports nothing
under either path and the working tree is unchanged by the measurement. The
figures are the printed output of that run.

## 5. The atomicity evidence in history

The claim is narrow: *no commit has ever returned an `after` spelling from
`Tool::name()`*. The query therefore has to be scoped to that return, because a
bare `git log -S'"Bash"'` matches every mention of the word - a shell command in
a test, an OpenAPI schema key - and proves nothing about the registry. The
scoped query, run for all thirteen `after` spellings:

```sh
for s in Bash Edit Glob Grep Write AskUserQuestion EnterPlanMode \
         ExitPlanMode Skill TaskCreate TaskGet TaskList TaskUpdate; do
  git log -S"fn name(&self) -> &'static str {
        \"$s\"" --all --oneline
done
```

**All thirteen return zero commits**: no tree in this repository's history has
ever carried an `after` spelling as a `Tool::name()` return. At `HEAD`
(`d6abcd4a`) all thirteen sites still return `before` - spot-checked with
`git show HEAD:crates/ragent-tools-core/src/write.rs`, which returns `"write"`,
as does `HEAD~1` - while the working tree returns `after` at all thirteen. So
there is no commit in which both spellings resolve and none in which neither
does: the cut has exactly one side with the old spelling, and the new side
exists only in the uncommitted tree.

The scoping is not pedantry. An unscoped eight-space search does report one hit:
`git log -S'        "TaskList"' --all` names `1e08279f Version: 1.0.131`, but that
hit is `response_schema: Some("TaskList")` and a `"TaskList"` `$ref` key in
`crates/ragent-server/src/routes/openapi.rs` - an OpenAPI schema name from an
unrelated spec, not a tool name. Read unscoped, that hit looks like a
half-renamed commit that never existed; read scoped, the history is empty. The
same distinction explains `AskUserQuestion`: its `after` spelling does not occur
anywhere before this change, and `aliases.rs` carries `"ask_user"` only as a
`permission_category()` value, which the scoped query excludes.

That is the state FR-034 forbids *shipping* in two steps: the change must land as
one commit. The mechanics of that commit are §6.

## 6. Why the commit itself is still blocked

FR-034 clause 2 - "the build and the workspace test suites pass" - is not met by
this working tree, and the requirement explicitly forbids landing a tree that
fails it. Measured with `cargo test --workspace --no-fail-fast` on this tree (55
test-result lines: 52 ok, 3 failed targets):

| Target | Result | Cause | Owner |
|---|---|---|---|
| `ragent-llm --test all` | 292 passed, **6 failed** | four source scans and two byte-comparisons from `d6abcd4a`'s wire layer (`test_wire_decode`, `test_wire_encode`, `test_wire_encode_message`, `test_wire_trace`); none names a tool | not this spec's rename |
| `ragent-tui --lib` | 141 passed, 1 failed -> **142 passed** | `loop_dialog_tests.rs:100` typed `"read, edit, bash"` into the loop dialog while line 115 asserts `["read", "Edit", "Bash"]`; `parse_comma_list` is verbatim by design (T-036), so the input literal was the half-migrated half. **Fixed by T-038**: the fixture now types `"read, Edit, Bash"` and the target is green | T-038 (landed) |
| `ragent-tui --test all` | 3646 passed, **3 failed** | the three Alt+T tasks-panel cases recorded as pre-existing in T-031 §11 | pre-existing |

Everything else is green, including the four crates the earlier draft of this
section named as red: `ragent-agent --test all` 1694 passed / 1 ignored,
`ragent-tools-core --test all` 394, `ragent-tools-extended --test all` 1806 / 11
ignored, `ragent-server --test all` 192, `ragent-config` 417, `ragent-types` 418.
The 44 `before` literals this record previously tabulated are **gone**: a scan of
every `crates/*/tests/**/*.rs` for tool-name-shaped `before` usage (`get("…")`,
`contains(&"…")`, `name() == "…"`, a `"…" =>` arm) now returns **4** hits, and
none is an assertion about a registered name -

| Site | What it is |
|---|---|
| `test_fr032_bridge_containment.rs:97` | `!names(&bridged).contains(&"bash")` - a *negative* assertion that the bridge does **not** surface the retired spelling |
| `edit_log_tests.rs:111,198,202` | `tool_counts.get("edit")` on the edit log, whose keys are the tool names the log recorded before this change - fixture data, not a registry lookup |

so the remaining red was one input literal (T-038, since landed - see the row
above) and three targets this spec never owned.

The live regression the earlier draft recorded is also **fixed on this tree**:
`crates/ragent-agent/src/research_adapter.rs:338-339` now looks up
`registry.get("Glob")` and `get("Grep")`, so `build_local_gatherer` returns
`Some` and `/research` keeps local file cross-referencing - `has_local: true` is
asserted by `test_research_adapter_extra2::provider_model_wiring_uses_storage_and_config_base_url`
(passing) and by the two `has_local: true` debug-assertion cases in
`tests/inline/research_adapter_tests.rs`. That fix belongs to T-033's site list;
it is recorded here only because this section previously cited it as a reason the
tree could not land.

### 6.1 Clause 2 is measured over the registered tests, and 20 files are not registered

The count above is honest but not complete, and a reader checking FR-034 clause 2
should know why. **20 `tests/test_*.rs` files across six crates have no `#[path]`
line in their crate's `tests/all.rs`**, so they contribute zero cases:

| Crate | Unregistered files |
|---|---|
| `ragent-agent` | `test_approval_blocks_result`, `test_fr028_state_survival`, `test_fr029_allowed_tools`, `test_fr030_surface_agreement`, `test_fr032_bridge_containment`, `test_fr033_completion_sources`, `test_mcp_npx_cache_repair`, `test_nfr006_rename_cost`, `test_parallel_gate`, `test_tool_argument_decode_sites`, `test_tool_rename_guard`, `test_unified_dispatch` |
| `ragent-tui` | `test_fr030_surface_agreement`, `test_fr033_completion_surfaces`, `test_mcp_bridged_surfaces`, `test_mcp_remove`, `test_modules_arms_deep` |
| others | `ragent-config/test_retired_alias_spelling`, `ragent-plugins/test_fr032_plugin_bridge`, `ragent-types/test_fr032_dynamic_bridge` |

Twelve of the fifteen `ragent-agent` files were temporarily registered to see
whether they could be, and the probe **did not compile** - 18 errors naming APIs
that exist in no compiled module (`ragent_agent::session::dispatch`,
`tool::allowed_tools`, `hooks::run_pre_tool_use_hooks_decoded`,
`mcp::npx_cache`, `ragent_types::tool_bridge`, `ToolRegistry::resolve`, and
`DryRunInputs`' missing `hidden_tools` field). The probe was reverted and
`tests/all.rs` is byte-identical to the version this task started from (plus the
one `#[path]` line below). So the unregistered files cannot simply be added: they
are T-014's and later tasks' work, and clause 2 cannot see them until then. That
is a finding about the *measurement*, not about this rename, and it is recorded
rather than repaired here because registering a test file whose APIs do not exist
is not a change that can compile.

## 7. What this task actually changed

Four edits, no behaviour change to the agent:

1. **`crates/ragent-agent/tests/all.rs`** - one
   `#[path = "test_tool_rename_atomic_landing.rs"] mod test_tool_rename_atomic_landing;`
   line. See §7.1.
2. **`crates/ragent-agent/tests/test_tool_rename_atomic_landing.rs`** -
   `FR030_CANARY_FAMILIES` corrected from the predicted `("agent", 3), ("team", 20)`
   to the measured `("agent", 4), ("team", 19)`, and the doc comment and one
   failure message corrected with it. T-041 §8.1 named this file as one of the
   three stale guards that would fail on a correct tree and assigned the
   correction to T-046; `test_fr030_surface_agreement.rs` and
   `test_fr028_state_survival.rs` carry the same stale pair and belong to T-037.
   The correction is not a relaxation: both numbers still pin a count, and the
   pair is now the one `--dry-run` prints (`agent: 4 tools`, `team: 19 tools`).
3. **`crates/ragent-llm/tests/inline/anthropic_tests.rs`** - `ChatContent` added
   to the `crate::llm` import. `d6abcd4a` removed it from `anthropic.rs`'s
   imports (the file no longer spells the type) while the `#[path]`-included test
   module, which uses it twice, inherited `use super::*`. The result was that
   `cargo test -p ragent-llm --lib` **did not compile at `HEAD`** - two `E0433`
   errors - so the crate's unit suite had not run at all. A one-word import
   repair; `--lib` now reports 324 passed.
4. **`specs/uniformtools/T-046-ATOMIC-LANDING.md`** - this record.

Red proofs, run on this tree and reverted (`md5sum`-verified byte-identical):

| Break | Cases that went red | Message it produced |
|---|---|---|
| `write.rs` `Tool::name()` back to `"write"` | 1, 2, 4 | `must resolve exactly one spelling of the pair "write" -> "Write", but it holds 1 "write" and 0 "Write"`; `registry.get("write") still returns a tool`; `` `after` "Write" is not registered `` |
| `aliases.rs` `UpdateFileTool` renamed to `"write"`, registering a second `"write"` beside `Write` | 1, 2, 4 | `it holds 1 "write" and 1 "Write"` - the *both* state; `a registered tool still reports the retired spelling "write"` |
| `write.rs` `Tool::name()` returns `"WriteX"` (neither column) | 1, 4 | `it holds 0 "write" and 0 "Write"` - the *neither* state |

All three states FR-034 forbids are therefore red, each naming the pair, the
surface and the direction. The canary correction is separately proven: with
`("agent", 3), ("team", 20)` restored, case 5 fails on a correct tree with
the message "the `agent` family holds Some(4) tools, not the 3 the baseline records" - which
is exactly the stale expectation T-041 §8.1 predicted, and confirms the constants
are load-bearing rather than decorative.

Verification: `cargo fmt --all -- --check` clean; `cargo clippy -p ragent-agent -p
ragent-tui -p ragent-llm --all-targets` reports only the pre-existing
`pub(crate)`-inside-a-private-module warnings; the eight guard scripts
(`check-silent-errors`, `check-security-unwraps`, `check-poison-locks`,
`check-inline-tests`, `check-test-env-isolation`, `check-shared-guards`,
`check-dead-code-reasons`, `check-file-tool-containment`) all pass.
`scripts/check-tool-rename.sh` still reports 26 predicate-(e) lines, all of them
T-033's own exemption-list work (shell words, `pluralize` labels, permission-type
strings and doc examples) - not this task's sites, and the same 26 it reported
before this task started.

### 7.1 The fifth orphan-module defect, and the verification habit it teaches

`crates/ragent-agent/tests/test_tool_rename_atomic_landing.rs` shipped with
`d6abcd4a` and **was never registered** in `crates/ragent-agent/tests/all.rs`, so
this spec's central guard sat in the tree contributing nothing:

```text
$ cargo test -p ragent-agent --test all test_tool_rename_atomic_landing::
running 0 tests
test result: ok. 0 passed; 0 failed;      <-- exit 0
```

An empty run is green. That is the whole hazard: the exit code cannot distinguish
"7 cases passed" from "the file never compiled", and the `--test all` filter form
(`cargo test -p <crate> --test all <file>::`) reports the same 0 for a typo in the
module name as for a missing `#[path]` line. The only reliable check is the
`running N tests` line, which is why every verification note in this spec quotes
it.

This is the fifth defect of the same class here - after `ragent-types`'
`tool_args` and `tool_rename`, `ragent-agent`'s `session/tool_input_error`, and
`ragent-config`'s `tools` section - and §6.1 records that the class is far from
exhausted: **20** `tests/test_*.rs` files across six crates are still
unregistered, and registering twelve of them does not compile because they name
APIs no compiled module provides. A reader should therefore treat "the tests
pass" in this spec as meaning "the registered tests pass", and check the count.

## 8. The landing checklist, unchanged

When the commit is finally made it is:

1. complete T-033 (the 26 predicate-(e) literals) and T-038 (**done**: the
   loop-dialog input literal and the 136-file test migration; the 20 orphan test
   files' own registration is still owed once their APIs exist);
2. `cargo fmt --all -- --check`, `cargo clippy --all-targets`, and the full
   workspace test suites green;
3. `ragent --dry-run` reports 148 with `agent` 4 / `team` 19 - the numbers §4
   measures and case 5 and case 6 of the guard pin;
4. **one** commit carrying T-031, T-033 and T-038 together (FR-034's own
   dependency note, and `PLAN.md`'s "T-046 is the landing rule for T-031
   itself");
5. TC-030 step 9 of `TESTPLAN.md` reads the resulting history and confirms it is
   one commit.

This task's durable output is the guard in §1, the rule this section states, and
the measurement in §4 - all three of which hold the line at whatever commit that
turns out to be. Nothing in §1 depends on the commit existing yet, and nothing in
§6 is a defect in the rename.
