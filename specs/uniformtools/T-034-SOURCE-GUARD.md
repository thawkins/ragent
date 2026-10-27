# T-034 - A source guard for the retired spellings (FR-027)

Status: **completed**. Requirement: FR-027 (last clause). Dependency: T-033
(the literal migration this guard is the build-time catch for) and T-030 (the
FR-023 table the guard reads).

## What the requirement asks

FR-027 ends with one sentence that is not a migration but a mechanism:

> A guard **shall** fail the build when a renamed `before` name appears as a
> quoted or backticked literal in non-test source outside the FR-023 table and
> the changelog entry.

The rest of FR-027 is a one-time edit; this clause is what keeps the edit from
decaying. A stale literal is invisible to the compiler (it is a `&str`), so
without the guard the only thing standing between the tree and a silent
re-introduction of a retired name is a reviewer noticing a string.

## What the state was when this task started

**The guard already existed and already worked. The crate test that proves it
had never run.**

`scripts/check_tool_rename.py` (774 lines, 39 reviewed per-line exemptions) and
`scripts/check-tool-rename.sh` landed with the rename commit `d6abcd4a`, and
`crates/ragent-agent/tests/test_tool_rename_guard.rs` (2 cases) landed with them
and was **tracked by git** - but `crates/ragent-agent/tests/all.rs` had no
`#[path]` line for it, so neither case had ever compiled or run:

```text
$ cargo test -p ragent-agent --test all test_tool_rename_guard::
running 0 tests
test result: ok. 0 passed; 0 failed;      <-- exit 0, and nothing proved anything
```

This is the **sixth** instance of this spec's orphan-file class (T-012 `tool_args.rs`,
T-036 `allowed_tools.rs`, T-037's two surface guards, T-040's `test_nfr006_rename_cost.rs`,
T-046's `test_tool_rename_atomic_landing.rs`), and the second one where the orphan
was the spec's own guard for the requirement under review. `PLAN.md`'s T-046 row and
memory notes both record the pattern.

Three other defects were found in the same pass:

1. **The guard was not wired into CI or `pre-flight.sh`.** `PLAN.md` line 95 claims
   `check-tool-rename.sh` is "wired into CI beside check-dead-code-reasons.sh"; it
   was not - `grep tool-rename .github/workflows/` returned nothing, and
   `pre-flight.sh` did not run it either. A guard nobody invokes is a comment.
2. **`scripts/check-tool-rename.sh` carried non-ASCII in its own sources and its
   own diagnostics** (em-dashes in the header comment, in the seeded fixture, and
   in both self-test messages). NFR-009 requires ASCII diagnostics, and AGENTS.md
   forbids non-ASCII in comments, so the *guard* violated the rule it enforces on
   other files.
3. **The shell wrapper's header described the wrong predicate set.** It claimed to
   scan "double-quoted, single-quoted, backticked" literals and to exempt a
   variable called `EXEMPT_LINES` that does not exist (the Python symbol is
   `EXEMPTIONS`). A guard's own documentation being wrong about what it does is how
   a reader decides a hit is a false positive.

## What this task did

### The registration (the defect that mattered)

`crates/ragent-agent/tests/all.rs` gains one line:

```rust
#[path = "test_tool_rename_guard.rs"]
mod test_tool_rename_guard;
```

### Two cases added, pinning the parts of the guard nothing asserted

The file had two cases: "the guard passes on the checked-in tree" and "the guard
fails on a seeded stale literal". Both are about *the scan*. Neither says anything
about the two things the task text is actually specific about - the exemption
structure - so two cases were added rather than one:

| # | Case | What it pins |
|---|---|---|
| 1 | `test_guard_passes_on_the_checked_in_tree` | exit 0, ASCII, the `(e)` predicate letter |
| 2 | `test_guard_fails_on_a_seeded_retired_literal` | non-zero, ASCII, `(e)`, the `before` spelling, and the offending **file** |
| 3 | `test_every_exemption_is_reviewable_and_the_two_structural_ones_are_named` | `--exemptions` succeeds; it names **both** structural exemptions (the FR-023 table and `CHANGELOG.md`); and it prints exactly one non-blank `reason:` line per per-line entry |
| 4 | `test_a_retired_literal_in_a_test_file_is_out_of_scope` | a retired literal seeded into `crates/*/tests/` passes, and the tests file is not reported |

Case 4 is the negative half that keeps the guard honest in the other direction:
FR-030's surface cases and FR-023's invariant cases *must* name retired spellings
to assert that they are retired, so a guard that scanned `tests/` would force those
assertions to be weakened. The scope decision is now a case, not a paragraph.

Case 3 is why the exemption list is an audit: an entry with no reason is
unreviewable, and the task's rule is that "an entry for a tool-name site is a
defect". Printing the reason is what makes the review possible; asserting the
reason is printed is what makes the review habitual.

The four cases share `GUARD_SEED_LOCK` (one mutex for the file, not
`global_env_lock`) because they touch a seeded sibling file no other test reads.

### The exemption list, audited

39 entries across 20 files. Each was re-read against its reason; the counts below
are what the guard reports (`--exemptions`), not a recollection.

| Class | Entries | Why it is not a tool name |
|---|---|---|
| permission-type vocabulary (FR-028) | 11 | `"edit"`, `"bash"`, `"write"`, `"plan_enter"`, `"plan_exit"`, `"ask_user"` are permission *types*: `Permission::from`/`Display` arms, the `file:` family comparison, and `AskUserQuestion::permission_category()` |
| shell program names | 5 | `Command::new("bash")`, `which::which("bash")`, `OsStr::new("bash")` for `bash -n`/`bash -c`, and the `ShellType::PowerShell` fallback - the binary, not the tool |
| shell program allow/deny lists | 3 | `SAFE_COMMANDS`, `ALLOWED_EXECUTABLES`, `DENIED_CONNECTOR_COMMANDS` - programs the shell may run or may not wrap |
| config section names | 2 | `bash.allowlist`/`bash.denylist` live under a section called `bash`; `/config show` merges the same key (`union_fields`) |
| language ids / runtimes | 4 | `"sh" \| "bash" => "shell"`, the scaffold language table and its help text, the `bash` toolchain runtime |
| GovCreate stage ids | 6 | `"write"` is a pipeline stage label (`writing spec files`), and the stage is not a tool call |
| English nouns/verbs | 4 | `KEYWORD_MAP` stem, `TESTABLE_VERBS` entry, and `pluralize(_, "edit", "edits")` twice |
| stored values | 1 | the `"bash"` memory **tag** - a value written once and replayed, which FR-028 keeps unchanged |
| slash-command triggers | 3 | `/bash` is a command (`add`/`remove allow\|deny`, `show`, `help`), not a tool |

39 entries, and that is the whole list.

`crates/ragent-config/src/permission.rs` (8 entries) and the permission-class files
are exactly the class the task names, and FR-028 requires them to stay.

### `crates/ragent-agent/src/agent/oasf.rs` needs **no** entry, and that is the finding

The task text names the OASF doc list as one of "the only entries expected". It
needs none, and the guard says so rather than being relaxed to accommodate it:

```
$ sed -n '160,161p' crates/ragent-agent/src/agent/oasf.rs
    /// Permission type: `"read"`, `"edit"`, `"bash"`, `"web"`, `"ask_user"`,
    /// `"todo"`, `"plan_enter"`, `"plan_exit"`. The legacy `"question"` string
```

Both lines are **doc-comment prose**, and comment prose is outside the scan's
scope - a comment cannot key on a name. The literals do not appear in code in that
file at all. Recording this as "needs an entry" would have meant adding two
exemptions for lines the guard never looks at, and an exemption that matches no
line is a **failure** by this guard's own reverse check (see red proof 2). So the
`--exemptions` listing states the scope decision explicitly, in its own section,
separated from the exemptions - a reader asking "why is `oasf.rs` not listed?"
gets an answer instead of a silent absence.

### The wiring

- `.github/workflows/ci.yml`, `security-guards` job: `--self-test` then the guard,
  beside `check-silent-errors` and the rest, so the CI gate runs what `PLAN.md`
  claimed it ran.
- `pre-flight.sh`: the same two invocations, so a local pre-flight matches CI.
- `scripts/check-tool-rename.sh`: ASCII throughout (5 em-dashes replaced in the
  header comment, the seeded fixture's comment, and both self-test messages), and
  the header rewritten to describe what the guard actually does - it reads the
  thirteen pairs out of the FR-023 table, scans double-quoted and backticked
  literals only (single quotes are Rust chars), skips two structural exemptions,
  does not scan comment prose, and checks `EXEMPTIONS` in both directions.

## Red proofs

Nine edits to the guard and its exemption list, each observed and reverted
byte-identically (`md5sum`-verified against `target/temp/guard_backup.py`,
`d3db7ce5ab0cb462d39776a0551a5dc4`):

| # | Proof | Observed |
|---|---|---|
| 1 | the memory-tag exemption deleted | exit 1, `(e) crates/ragent-agent/src/memory/extract.rs:525: retired tool spelling "bash" ... migrate to "Bash"` |
| 2 | `team/classify.rs`'s entry text changed to a line that does not exist | exit 1, both the unexempted hit **and** `exemption matches no line, so it is unreviewable and its reason is stale` |
| 3 | that entry's declared literals widened to `("write", "bash")` | exit 1, `exemption '"write",' declares ['write', 'bash'] but the line carries ['write']` |
| 4 | the entry's literals changed to a spelling that is not retired | exit 1, the hit is reported again (the entry is discarded, not honoured) |
| 5 | a backticked literal seeded into `crates/ragent-tools-core/src/` | exit 1, `(e) .../__e_probe.rs:1: retired tool spelling "bash"` |
| 6 | a literal seeded into the root `src/` crate | exit 1, `(e) src/__f_probe.rs:1: retired tool spelling "skill_manage" ... migrate to "Skill"` |
| 7 | `CHANGELOG.md` dropped from the structural-exemption report | case 3 fails, `the CHANGELOG.md entry must be named as a structural exemption` |
| 8 | `--exemptions`' declared count incremented by one | case 3 fails, `the listing declares 40 per-line exemptions but printed 39 reason(s) - an entry without a reason is an unreviewable one` |
| 9 | `--exemptions`' `reason:` line removed | case 3 fails, `the listing declares 39 per-line exemptions but printed 0 reason(s)` |

Proofs 8 and 9 are why case 3 compares the **declared** count against the printed
one instead of merely asserting the list is non-empty. Proof 9 (every reason line
removed) is caught by either check, but a single entry losing its reason is not:
`!reasons.is_empty()` stays satisfied by the other 38, while `printed == declared`
fails. Proof 8 is the other direction - a header claiming more entries than it
lists - and only the comparison catches it, because the printed list is a
well-formed subset either way. The check is therefore on the arithmetic, not on
the presence of output.

Three more were run as **negative** proofs, asserting a clean run rather than a
failure, because they pin properties whose regression would be *silent*:

| # | Proof | Observed |
|---|---|---|
| 10 | `"write "` and `"write_file"` seeded (substrings of a retired name) | exit 0 - exact equality, not containment |
| 11 | a retired literal seeded into `crates/*/tests/` | exit 0, and the file is not named - tests are out of scope (this is case 4, run by hand first) |
| 12 | the FR-023 table's own `before:` fields | never reported - structural exemption 1 |

## Verification

- `cargo fmt --all -- --check`: clean.
- `cargo clippy -p ragent-agent --all-targets`: no finding in
  `test_tool_rename_guard.rs` (only the pre-existing `pub(crate)`-in-a-private-module
  noise across `ragent-tools-vcs`).
- `cargo test -p ragent-agent --test all test_tool_rename_guard::`: **4 passed,
  0 failed** (was `running 0 tests`).
- `cargo test -p ragent-agent --test all`: **1739 passed, 0 failed, 1 ignored**,
  of which 4 are this file's cases. Before the registration the file contributed
  **0** to that total (`running 0 tests`), so it accounts for 4 of the 1739.
- `bash scripts/check-tool-rename.sh`: exit 0,
  `39 reviewed exemption(s) all match`.
- `bash scripts/check-tool-rename.sh --self-test`: exit 0, and the seeded fixture
  is removed (`crates/ragent-tools-core/src/` holds no `__*selftest.rs` after).
- The other eight guards pass: `check-inline-tests`, `check-file-tool-containment`,
  `check-shared-guards`, `check-silent-errors`, `check-security-unwraps`,
  `check-poison-locks`, `check-dead-code-reasons`, `check-test-env-isolation`.
- NFR-009 cost: **0.79 s** on the full tree (670 scanned files), so it can run on
  every `cargo test` and in the CI guard set with no timeout exception.
- No probe file survives: `git status --porcelain` shows the same untracked set
  before and after (`session/tool_suggestion.rs` and the four spec records from
  earlier tasks - nothing from this one).

## Gotchas

1. **A retired literal in a *test* is correct, in *src* it is a bug.** The scope
   boundary is the whole reason FR-030's digest case and FR-023's invariant cases
   can exist. Widening the scan to `tests/` would not harden the guard, it would
   force 775 literal assertions (T-038's remit) to be relaxed.
2. **Comment prose is out of scope by decision, not by oversight.** ~29 backticked
   and 27 quoted occurrences of retired spellings live in `//`, `///` and `//!`
   lines, including the OASF list the task names. A comment cannot key on a name,
   and the rename's own documentation must state the old spelling to be useful.
   The one place this matters is `tool_bridge.rs`, which documents the refusal by
   spelling the retired name; it needs no exemption for the same reason.
3. **The reverse check is what makes it an audit.** An entry that matches no line,
   or fewer lines than it declares, or a different literal set, fails the run. That
   is how `oasf.rs` was correctly left with *no* entry rather than two dead ones.
4. **`cp -p` restores an old mtime, and cargo then reuses the broken object.** Every
   red proof here edits a Python file so this does not bite, but the Rust seed
   files created during the proofs are removed rather than restored for the same
   reason: a left-behind fixture would be scanned by the next run.
5. **`--self-test` seeds a real file inside the scanned tree.** If the run is
   interrupted between `cat > "$seed"` and `rm -f "$seed"`, the next guard run
   reports the fixture. The two invocations are adjacent for this reason; a stale
   `__tool_rename_guard_selftest.rs` in `crates/ragent-tools-core/src/` means a
   self-test was killed, not that the tree regressed.

## Documentation

- `scripts/check-tool-rename.sh`'s header rewritten (the `EXEMPT_LINES` /
  "single-quoted" errors above) and made ASCII.
- `.github/workflows/ci.yml` and `pre-flight.sh` now invoke the guard and its
  self-test.
- `CHANGELOG.md` `Unreleased` / `Added`.
- `PLAN.md` T-034 -> completed, with the orphan registration recorded in the row.
- `TESTPLAN.md` TC-026 gains an "Automated half (T-034)" paragraph naming the four
  cases and which step each automates.
