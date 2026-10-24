# T-046 - The rename lands as one atomic step (FR-034)

**Spec ID:** uniformtools
**Task:** T-046
**Requirement:** FR-034 (`The rename never ships two spellings at once`)
**Status:** completed (the guard, the landing rule and the measurement). The
*commit* it governs is still owed by T-033 and T-038 - see §6.
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
passing. It reads the FR-023 table and asserts the **live registry** agrees with
it, across every enumeration the build offers:

| # | Case | Clause it pins |
|---|---|---|
| 1 | `test_the_rename_resolves_exactly_one_spelling_of_every_pair` | 1 - one spelling, and it is `after`, in `list()`, `all_tools()` and `definitions()` |
| 2 | `test_no_before_spelling_is_reachable_by_lookup_or_by_enumeration` | 1 - `get(before)` is `None` and no key carries a `before` |
| 3 | `test_registry_keys_tool_names_and_definition_names_agree` | 1 - the three enumerations are element-for-element identical |
| 4 | `test_every_registration_is_stored_under_the_name_its_tool_reports` | the half-renamed detector (below) |
| 5 | `test_the_renamed_tools_keep_the_permission_family_they_had_before_the_rename` | 2 - FR-028's counts, 21 families, `agent` 3 and `team` 20 |
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
| `agent` family | 3 | **3** | 2 - canary |
| `team` family | 20 | **20** | 2 - canary |
| names with an ASCII uppercase letter | 0 | **13** | 1 - the sharpest count |
| sorted-name digest, no trailing newline | `a833f0f1...` | **`308d92d9d975666cf1f815369e18cc7b457c1c0dcbab004c45f2835100368321`** | 1 |
| symmetric difference of the two lists | - | **26 (13 removed + 13 added)** | 1 |

Every removal is a `before` and every addition an `after` - `removals_not_in_table=0`
and `additions_not_in_table=0` - so the 26-line diff is exactly the table and
nothing else. The digest matches the one `T-041-BASELINE.md` *predicted* before
the rename started, which is the independent confirmation that no tool was
missed and none was renamed twice.

The `agent` 3 / `team` 20 canary matters here: the stale `target/release/ragent`
left over from 1.0.148 also reports 148, but splits it `agent` 4 / `team` 19. A
landing check that compared only the total against that binary would pass while
its family table was wrong, so case 5 of the guard pins both canaries.

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

The three scratch examples that produced these figures (uppercase list, digest,
symmetric difference) were deleted after the run - `git status --porcelain
crates/ragent-agent/examples/` shows only the pre-existing `tool_table.rs` - so
the working tree is unchanged by the measurement.

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
(`d172b658`, `Version: 1.0.149`) all thirteen sites still return `before`; the
working tree returns `after` at all thirteen. There is therefore no commit in
which both spellings resolve and none in which neither does - the cut has
exactly one side with the old spelling and the new side only exists in the
uncommitted tree.

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
fails it. The failures are not in the rename; they are the un-migrated literals
of two later tasks:

| Crate | Test binary | Result | Failures | Cause |
|---|---|---|---|---|
| `ragent-agent` | `all` | 1785 passed | 35 | `tests/**` still assert `before` names (T-038) |
| `ragent-tools-core` | `all` | 393 passed | 2 | registry-surface cases assert `before` names (T-038) |
| `ragent-tools-extended` | `all` | 1778 passed | 6 | task-tool identity cases assert `before` names (T-038) |
| `ragent-server` | `all` | 191 passed | 1 | OpenAI-surface case names `bash` (T-038) |

Every one of the 44 is a `before` literal in a test file: `tool 'edit' should be
registered`, `default registry must expose bash`, `task_create registered`,
`assertion failed: names.contains(&"write")`, and the two whole-surface failures
whose message prints the post-rename list back at the reader. None is a defect in
the rename. The guard in §1 is **green on this same tree** - it is not one of the
44 - which is the property that matters: the rename it checks is complete, and
the red cells are the migration debt later tasks owe.

A further live regression is T-033's, not a test literal:
`crates/ragent-agent/src/research_adapter.rs:338-339` looks up `registry.get("glob")`
and `get("grep")`, which now return `None`, so `build_local_gatherer` returns
`None` and `/research` loses local file cross-referencing (`has_local: false`).

So the rename cannot be committed until T-033 migrates the non-test literals and
T-038 migrates the test literals - which is the FR-034 rule working as intended:
it is refusing to let this tree land early. When it does land, the landing is:

1. complete T-033 and T-038;
2. `cargo fmt --all -- --check`, `cargo clippy --all-targets`, and the full
   workspace test suites green;
3. `ragent --dry-run` reports 148 with `agent` 3 / `team` 20;
4. **one** commit carrying T-031, T-033 and T-038 together (FR-034's own
   dependency note, and `PLAN.md`'s "T-046 is the landing rule for T-031
   itself");
5. TC-030 step 9 of `TESTPLAN.md` reads the resulting history and confirms it is
   one commit.

This task's durable output is the guard in §1 and the measurement in §4, both of
which hold the line at whatever commit that turns out to be. Nothing in §1
depends on the commit existing yet.
