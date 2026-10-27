# T-030 - The rename table and its invariant test (FR-023)

Spec `uniformtools`, task T-030. Requirement FR-023, validated by TESTPLAN case
TC-019.

Status: **completed**. Dependency: T-029 (the derivation).

## 1. What the task asks for

> Add `crates/ragent-agent/src/tool/rename.rs`: the checked-in table (one
> `(before, after)` slice, ascending by `after`) and its invariant test - every
> `before` registered, every `after` unregistered and a valid identifier, both
> columns unique. The test must not open the workbook:
> `crates/ragent-tools-core/src/xlsx.rs` is a writer only and adding a reader
> would breach NFR-002.

FR-023 adds the "one module every migration site reads" clause and the
manual-cross-check clause. The 13 pairs themselves are T-029's output and are not
re-derived here.

## 2. Where the table lives, and why not where the task says

The task names `crates/ragent-agent/src/tool/rename.rs`. The table is there - as
a **re-export**:

```rust
pub use ragent_types::tool_rename::{Rename, TOOL_RENAMES};
```

The table's body is `crates/ragent-types/src/tool_rename.rs` (170 lines: the
`Rename` struct, `TOOL_RENAMES: [Rename; 13]`, and four lookups - `after_of`,
`before_of`, `is_retired`, `is_rename_spelling`).

The move is forced by the dependency graph, not by taste. FR-031 (task T-042)
requires the **config loader** to refuse a `tools.aliases` entry that names a
retired spelling, so `ragent-config` must read the table. `ragent-config` sits
*below* `ragent-agent`, which depends on it, so a table in `ragent-agent` could
not be read from the loader without a dependency cycle. It therefore sits in
`ragent-types`, the leaf crate both already depend on.

FR-023's "one module that every migration site reads" is satisfied: every path in
the tree still spells `ragent_agent::tool::rename::{Rename, TOOL_RENAMES}`, so the
move changed no call site. The re-export is what keeps that true, and it is why
this task's file is 21 lines instead of 170.

`crates/ragent-types/src/lib.rs` gained `pub mod tool_rename;` and
`crates/ragent-agent/src/tool/mod.rs` gained `pub mod rename;`. Both were required
for the task to exist at all - see section 4.

## 3. The table

`TOOL_RENAMES: [Rename; 13]`, ascending by `after`, as T-029 derived it:

| # | before | after |
|---|---|---|
| 1 | `ask_user` | `AskUserQuestion` |
| 2 | `bash` | `Bash` |
| 3 | `edit` | `Edit` |
| 4 | `plan_enter` | `EnterPlanMode` |
| 5 | `plan_exit` | `ExitPlanMode` |
| 6 | `glob` | `Glob` |
| 7 | `grep` | `Grep` |
| 8 | `skill_manage` | `Skill` |
| 9 | `task_create` | `TaskCreate` |
| 10 | `task_get` | `TaskGet` |
| 11 | `task_list` | `TaskList` |
| 12 | `task_update` | `TaskUpdate` |
| 13 | `write` | `Write` |

A fixed-size array rather than a slice: `[Rename; 13]` makes the count part of the
type, so a reader cannot mistake it for an open set and `TOOL_RENAMES.len()` is a
`const`-foldable 13. `Rename` is `Copy`, so iteration is by value with no clone.

## 4. The three defects this task had to fix before it could be a task

All three are the same class - source committed by `d6abcd4a` that never compiled
or never ran.

1. **`pub mod tool_rename;` was missing from `crates/ragent-types/src/lib.rs`.**
   The 170-line module landed with the module body, but nothing declared it, so
   `cargo check` never built it. `ragent_agent::tool::rename`'s `pub use` would
   have been the only thing to name it, and it did not compile either.
2. **`pub mod rename;` was missing from `crates/ragent-agent/src/tool/mod.rs`.**
   Same defect one level up: `crates/ragent-agent/src/tool/rename.rs` existed at
   `HEAD` with no declaration, so the re-export path
   `ragent_agent::tool::rename` did not resolve and every test that reads
   `TOOL_RENAMES` could not name it.
3. **`test_tool_rename_table.rs` was registered nowhere.**
   `git show HEAD:crates/ragent-agent/tests/all.rs | grep rename` is empty, so the
   file's 8 cases had never run. Its assertions were the *post-rename* ones
   (`every_before_is_retired`, `every_after_is_registered`) - which describe a
   tree T-031 has not produced yet, so registering it unmodified would have failed
   6 of 8 cases on a correct pre-rename tree.

Defect 3 is the one worth naming: an unregistered test is not a weak test, it is
*no* test, and it also means nobody had read the file since it was written.
Registering it forced the assertions to be reconciled with FR-023's own sentence
("every `before` is a name the default registry registers, and every `after` is
not"), which is what the requirement actually says.

## 5. The invariant test - 9 cases

`crates/ragent-agent/tests/test_tool_rename_table.rs`, registered in
`crates/ragent-agent/tests/all.rs` as `test_tool_rename_table`.

| # | Case | Clause |
|---|---|---|
| 1 | `test_tool_rename_table_holds_exactly_the_thirteen_pinned_pairs` | FR-023: the exact 13 pairs, spelled as the spec pins them |
| 2 | `test_tool_rename_table_is_ascending_by_after` | FR-023: "ascending by `after`" |
| 3 | `test_tool_rename_every_before_is_registered_by_the_default_registry` | FR-023(a) |
| 4 | `test_tool_rename_every_after_is_unregistered_by_the_default_registry` | FR-023(b) |
| 5 | `test_tool_rename_every_after_is_a_valid_identifier` | FR-023(b), the identifier half |
| 6 | `test_tool_rename_after_column_is_unique` | FR-023(c) |
| 7 | `test_tool_rename_before_column_is_unique` | FR-023(d) |
| 8 | `test_tool_rename_lookup_helpers_agree_with_the_table` | the table and its four lookups cannot disagree |
| 9 | `test_tool_rename_invariant_test_does_not_open_the_workbook` | FR-023: "shall not open the workbook" |

Cases 1 and 5 are not redundant with 3, 4, 6 and 7. A table holding the right
pairs in the right order can still be *wrong* in a way the invariants cannot see -
it could hold twelve pairs, or fourteen, or a different spelling of an adopted
name - and cases 1 and 5 are what make the table a specification rather than a
well-formed blob. Case 8 exists because `after_of`/`before_of` are hand-written
linear scans over the same array: nothing else forces them to agree with it.

### The workbook clause is pinned, not documented

FR-023's last sentence is a *negative* requirement on a test file, which is the
easiest kind to lose - the natural next step for a reader of the table is "check
it against `tools.xlsx`", and the only way to do that is an XLSX **reader**.
`crates/ragent-tools-core/src/xlsx.rs` is a writer (`write_xlsx`, backed by
`rust_xlsxwriter`; there is no `read_xlsx`), so a reader means a new external
crate and NFR-002 says no. Case 9 therefore asserts both halves independently, so
neither can rot alone:

1. `crates/ragent-agent/Cargo.toml` names no XLSX reader (`calamine`,
   `umya-spreadsheet`, `simple_excel_writer`) - the *capability*, whether or not
   anything calls it yet;
2. no `src/` or `tests/` file in this crate, and not
   `crates/ragent-types/src/tool_rename.rs`, names a workbook opener
   (`write_xlsx(`, `read_xlsx(`, `Workbook::open`, `calamine::`).

`tools.xlsx` in prose is deliberately allowed: both module docs say *why* they do
not open it, and a guard that demanded an exemption for prose explaining itself
trains the next reader to add exemptions. Case 9 skips its own file, which names
the openers in order to forbid them. Cross-checking the table against the workbook
stays a manual step - TC-019.

### The two registry-facing cases describe the pre-rename tree on purpose

FR-023 orders the work as **table then rename**: it states the invariants against
the registry "as it stands when the table lands". This task lands the table on a
tree where `bash` is still registered and `Bash` is not, so cases 3 and 4 assert
exactly that.

T-031 flips both in the **same commit** that renames the tools (FR-034's
atomic-landing rule), and renames the two cases with them
(`..._every_before_is_retired...`, `..._every_after_is_registered...`). The
module doc says so at the top of both files so the next author does not "fix" the
assertion by relaxing it. The table's own facts - cases 1, 2, 5, 6, 7, 8, 9 - do
not change across that flip.

## 6. Red proofs (run, observed, reverted byte-identically)

Each proof was applied to the table, the file `touch`ed (a `cp -p` restore keeps
the older mtime, so cargo reuses the broken object file and the "reverted" tree
still fails), the filtered suite run, and the file restored from a `cp -p` backup
and verified by `md5sum`:

```
57397a2b1fcea242b450550474e99fc2  crates/ragent-types/src/tool_rename.rs
351cdeb5ff039ec941c9a10f8ae725b3  crates/ragent-agent/src/tool/rename.rs
5080eee3e1c82f7c5a4b9c2636efeb2b  crates/ragent-agent/Cargo.toml
```

| # | Break | Failed cases |
|---|---|---|
| 1 | `bash`'s `after` -> `read` (a registered name) | 4 - case 4 with "`after` \"read\" is already a name the default registry registers, so the rename would collide with an existing tool"; also 1, 2, 8 |
| 2 | `edit`'s `after` -> `Bash` (a duplicate column) | 4 - case 6 "the `after` column repeats" |
| 3 | `ask_user`'s `after` -> `Ask-User-Question` (not an identifier) | 2 - case 5 "is not an ASCII identifier". Case 1 also fails, which is the point: the pinned-pairs case is a second, independent net |
| 4 | `grep`'s `before` -> `grep_all` (unregistered) | 1 - case 3, naming the spelling and the reason |
| 5 | `glob`'s `before` -> `grep` (a duplicate column) | 2 - case 7 "the `before` column repeats"; case 8 `left: Some("Glob") right: Some("Grep")` |
| 6 | `const _PROBE: &str = "read_xlsx(";` appended to `tool/rename.rs` | 1 - case 9, naming the file and the opener |
| 7 | a `calamine` line appended to `crates/ragent-agent/Cargo.toml` | 1 - case 9, naming the reader and NFR-002 |

Proof 7 is worth its own note: the first attempt added `calamine = "0.26"` as a
real dev-dependency, which failed to *compile* (the version does not build against
this toolchain) - a red proof that fails the build is worthless, because the test
never runs. Appending a comment naming the crate keeps the manifest valid and
still trips the string check, which is what the case actually asserts.

## 7. Gotchas

1. **`grep rename crates/ragent-agent/tests/all.rs` was the whole task.** The
   table and its test both existed at `HEAD`; nothing ran. `scripts/check-inline-tests.sh`
   and the `tests/all.rs` registration convention are what stop this recurring, but
   neither catches a *source* module with no `pub mod` line - see section 4.
2. **A `cp -p` restore is not a revert.** It preserves the older mtime and cargo
   reuses the broken artefact, so a proof appears to stay red after the file is
   back. `touch` the restored file, or restore without `-p`.
3. **`test_tool_rename_guard.rs`, `test_tool_rename_atomic_landing.rs` and
   `test_nfr006_rename_cost.rs` are still unregistered** (`HEAD` and this tree
   alike). They belong to T-034, T-046 and T-040, which record their own
   registration; this task registered only its own file.
4. **The table is `ragent-types`', not `ragent-agent`'s.** Editing the re-export
   shim edits nothing; the pairs are in `crates/ragent-types/src/tool_rename.rs`.
5. **Do not "helpfully" sort at read time.** The array is ascending by `after` and
   case 2 pins it, so a migration site can binary-search or scan without sorting
   first - and a reader that sorted would hide a table edit that broke the order.

## 8. Docs updated

- `specs/uniformtools/PLAN.md` - T-030 row marked completed with what landed.
- `crates/ragent-types/DOCS.md` - `tool_rename` module bullet.
- `crates/ragent-agent/DOCS.md` - the `tool` module bullet already names `rename`
  as a data-module rather than a tool; it was corrected to say the table is
  re-exported from `ragent_types::tool_rename`.
- `CHANGELOG.md` - `[Unreleased]` entry.
- This record.

## 9. Verification

| Check | Result |
|---|---|
| `cargo fmt --check` | clean |
| `cargo check -p ragent-agent -p ragent-types -p ragent-config --all-targets` | clean |
| `cargo test -p ragent-agent --test all test_tool_rename_table::` | 9 passed, 0 failed |
| `cargo test -p ragent-agent --test all` | 1670 passed, 1 ignored, 0 failed |
| `cargo test -p ragent-types -p ragent-config --test all` | 418 passed, 0 failed |
| `cargo clippy -p ragent-types -p ragent-agent --all-targets` | no new warning (only the pre-existing `pub(crate)`-in-private-module set) |
| `scripts/check-inline-tests.sh` | PASS |
| `scripts/check-dead-code-reasons.sh` | PASS |
| `scripts/check-shared-guards.sh` | PASS |
| `scripts/check-test-env-isolation.sh` | PASS |
| `scripts/check-poison-locks.sh` | PASS |
| `scripts/check-security-unwraps.sh` | PASS |
| `scripts/check-silent-errors.sh` | PASS |
| `scripts/check-file-tool-containment.sh` | PASS |
| `scripts/check-connector-endpoint-literal.sh` | PASS |
| `scripts/check-team-duplication.sh` | PASS |
| `scripts/check-vcs-duplication.sh` | PASS |
| `scripts/check-tool-rename.sh` | FAIL - pre-existing, T-033's 44 unmigrated `before` literals in `crates/ragent-tui/src/widgets/message_widget.rs`; unrelated to this table |
| `scripts/check-bench-regression.sh` | FAIL - pre-existing, needs `cargo bench` first (`target/criterion` absent) |

No test writes into the repository; case 9 reads manifests and sources only.
