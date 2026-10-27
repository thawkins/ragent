# T-048 - One implementation, two entry points: the NFR-008 guard as a `cargo test` case and as `scripts/check-tool-rename.sh` (NFR-009)

Requirement: NFR-009. Dependencies: T-047 (the seven predicates), T-034 (the
literal scan and the shell wrapper).

## 1. The requirement this task lands

NFR-009, verbatim:

> The NFR-008 check **shall** read only files inside the repository, **shall**
> make no network request, **shall not** open `tools.xlsx`, and **shall** finish
> in a few seconds so it can run on every `cargo test` and in the CI guard set
> without a timeout exception. Its failure messages **shall** be ASCII, **shall**
> name the predicate letter it failed and the exact edit required, and **shall**
> exit non-zero. The check **shall** be usable both as a `cargo test` case and as
> a `scripts/` guard beside the existing `check-*` scripts, from one
> implementation.

Six clauses. Five of them were already true of `scripts/check_tool_rename.py` and
one was **false**, because the sixth - "from one implementation" - had two
implementations in the sense that mattered: the crate test and the script agreed
only because a human kept them in step, and nothing proved it.

## 2. State found

`scripts/check_tool_rename.py` (1954 lines) held the logic and
`scripts/check-tool-rename.sh` (a wrapper) held a **second copy of the self-test
fixture source**, while `crates/ragent-agent/tests/test_tool_rename_guard.rs`
held a **third**. Three spellings of one fixture, kept in step by review.

The four defects the audit found, before a line was written:

| # | Defect | Consequence |
|---|---|---|
| 1 | `--self-test` existed in two places (the shell wrapper and the crate test's `SeedFile`), each writing the same fixture | a change to the fixture in one place silently stopped the other from proving anything |
| 2 | the guard's output was prose only - no machine-readable form | the crate test could only assert substrings of the human report, so "the two forms agree" was untestable |
| 3 | the guard had no `--root`, so every case that needed a fixture had to write **into the checkout** | the module needs a nextest process group to serialise itself against its own siblings, which is a rule about a test, not about the guard |
| 4 | `_dependency_map` globbed `root.glob("Cargo.toml")`, which matches only entries **below** root | the root package (the `ragent` binary) had **no dependency edges at all**, and the `ragent-agent` manifest was only read through the test crate `crates/ragent-agent/tests/` - so predicate (g) was structurally blind in two packages |

Defect 4 is the serious one, and it was found by this task rather than by the
task that owns the predicate: a wholesale forwarder seeded into
`crates/ragent-agent/src/` was **not reported**, and the only reason the real
tree's `update_file` forwarder was ever reported is that the guard also read
`crates/ragent-agent/tests/Cargo.toml`.

## 3. What was built

### One implementation (defect 1)

`scripts/check-tool-rename.sh` is now 67 lines with **no predicate logic and no
fixture**: it `exec`s the Python file with the caller's arguments. The fixture
source and its path live once, in the Python module (`SELF_TEST_SOURCE`,
`SELF_TEST_FIXTURE`), and `--self-test` writes, scans, asserts and removes it in
one call, so an interrupted run cannot leave a seed behind.

The crate test calls `--self-test`; it does not seed anything itself.

### `--json` (defect 2)

`report(root)` returns the verdict, every hit (predicate, path, line, literals,
the adopted spelling, the offending text, the predicate-specific detail, the
exact fix), every exemption problem, and the line a clean run prints. The crate
test reads it, and the case
`test_the_script_form_and_the_test_form_report_the_same_thing` runs **both**
front doors over the checkout and asserts the same verdict, the same hits in the
same order, one `fix: ` line per hit, and the same success wording. That is the
clause "from one implementation", asserted rather than reviewed.

`--check` is the human form and prints exactly what the script always printed,
so CI, `pre-flight.sh`, and every existing case are unchanged by the split.

### `--root` (defect 3)

`--root DIR` points the one implementation at another tree. The crate test copies
`crates/`, `src/` and the workspace `Cargo.toml` into `target/temp/<case>/` and
seeds fixtures **there**, so a fixture is never visible to a sibling test
process. Three of the four checkout-seeding cases are gone; only `--self-test`
(the guard's own contract, which must run in the real tree) still writes inside
the checkout.

`--root` also made one latent bug reachable, and fixing it is part of the task: a
different root must be judged against **its own** FR-023 table. The guard used a
module-level pair set loaded from its own checkout, so a copy with no table
scanned clean. `_pairs_of(root)` now re-reads the table for a non-default root
and fails closed when it is missing.

### Defect 4: the dependency map

`root.glob("Cargo.toml")` does not match root itself. The root manifest is now
added explicitly, and `_delegation_targets` sees every package. Both the seeded
(g) case and the real tree are affected: the seeded forwarder case only passed
before because the scratch copy contained `crates/ragent-agent/tests/Cargo.toml`.

### Two further honest fixes

- **predicate (f) is now file-boundary-free.** The duplicate check skipped a
  second declaration inside one file (`owner[0] != source.relative`), which is
  the same defect as a duplicate across two files. It now reports the later site
  **and** the earlier one, so a reader fixing the report does not fix only the
  copy they were shown. It also stopped reading a `` `backticked` `` word in a
  statement as a registry key (a key is a string literal), and skips a `name()`
  body that forwards to `self.inner.`, which registers the wrapped tool's name
  and is not a second declaration of its own.
- **two reviewed exemption entries added**, each with its written reason: the
  `update_file` alias tool's `execute` (predicate (g), a designed forwarder - see
  its new doc comment), and the guard's own self-test fixture (an audit line that
  **silences nothing**, because the fixture exists to be reported). A seed entry
  is allowed to match no line outside a self-test run (`SELF_TEST_SEEDS`), which
  is what let the audit list be complete without turning the self-test's failure
  into a pass.

## 4. The sixteen cases

| # | Case | What it pins |
|---|---|---|
| 1 | `test_self_test_detects_a_seeded_retired_literal` | `--self-test` exits 0 only when it **detected** its own seed, names `(e)`, names `file:line`, and removes the fixture |
| 2 | `test_a_failure_names_the_predicate_letter_and_the_exact_edit` | a failing scratch run is ASCII, names `(e)`, the file and line, and a `fix: ` line; the JSON hit carries the same edit |
| 3 | `test_every_exemption_is_reviewable_and_the_two_structural_ones_are_named` | both structural exemptions named; one non-blank `reason:` per declared entry |
| 4 | `test_a_retired_literal_in_a_test_file_is_out_of_scope` | a `tests/` literal passes **and the same literal under `src/` fails**, so the boundary is the directory, not the literal |
| 5 | `test_the_script_form_and_the_test_form_report_the_same_thing` | the two front doors over the checkout: same verdict, same hits, same fix text, same success line |
| 6 | `test_the_guard_finishes_in_a_few_seconds` | a full-tree run inside a 10 s budget |
| 7 | `test_the_guard_reads_only_the_repository_and_never_a_workbook` | the module imports nothing that can open a socket or a workbook; `--root` confines every reported path; a copy with no `tools.xlsx` judges identically |
| 8 | `test_a_root_without_the_rename_table_fails_closed` | a copy missing the FR-023 table is an error naming the table, not a clean pass |
| 9 | `test_nfr008_predicate_a_alias_table_entry_is_reported` | key half `(a)`, target half `(e)` on the retired target, self-mapping `(a2)`, all in one run |
| 10 | `test_nfr008_predicate_a2_self_mapping_is_reported` | `(a2)` fires on an entry that maps no retired name |
| 11 | `test_nfr008_predicate_b_resolver_arm_is_reported` | `(b)` on an arm appended to the real resolver file, plus `(e)` quoting the retired spelling |
| 12 | `test_nfr008_predicate_c_default_value_is_reported` | `(c)` inside a `fn default_*()` body |
| 13 | `test_nfr008_predicates_d_and_f_registry_key_and_duplicate_are_reported` | `(d)` and `(f)`, with the duplicate reported at both sites |
| 14 | `test_nfr008_predicate_g_wholesale_forwarder_is_reported` | `(g)`, and the `detail` naming the tool the body forwards to |
| 15 | `test_nfr008_predicate_g_ignores_a_forwarder_that_decides` | the alias shape FR-026 permits stays silent |
| 16 | `test_one_run_reports_every_predicate_at_once` | four independent violations, one run, every letter and every fix |

Case 5 is the task's own clause. Cases 6 and 7 are NFR-009's cost and
hermeticity clauses, which had never been asserted by anything.

## 5. Red proofs

Nine edits, each applied, observed, and reverted byte-identically from
`target/temp/t048/proofs/` with `cp -p`, `touch` and `md5sum -c`
(`check_tool_rename.py` `a4acb0b7...`, `test_tool_rename_guard.rs`
`bb3196d5...`).

| # | Edit | Observed |
|---|---|---|
| 1 | `(a)` checks the key half only | `[('e', ['plan_exit']), ('a', ['bash']), ('e', ['bash'])]` - the target half of an alias row needs the `(a)` arm |
| 2 | the `(a)` arm disabled (`if False and ...`) | case 9 reds, `the report must carry a (a) hit on .../__guard_alias_table.rs` |
| 3 | the reverse `(f)` report removed | case 13 still passes - the proof is **weak**, because the seeded duplicate is later in scan order than the tool it collides with; recorded as such |
| 4 | `(f)` becomes "first declaration wins, no report" | case 13 reds, `the report must carry a (f) hit on .../__guard_registry_keys.rs` |
| 5 | the root `Cargo.toml` dropped from the dependency map | case 14 reds, `the guard must fail on a seeded wholesale forwarder` - the exact defect section 3 fixes |
| 6 | the crate test stops copying the workspace manifest, keeping fix 5 | case 14 reds identically, proving the two halves of the (g) path are each load-bearing |
| 7 | the self-test message loses `(e)` | case 1 reds, `the self-test must report the predicate letter (e)` |
| 8 | `_pairs_of` always returns the module-level pairs | a copy with no FR-023 table reports `"ok": true` - case 8's defect |
| 9 | `SELF_TEST_SEEDS` honoured for every entry | the run reports `exemption matches no line ... 'if tool == "bash" {'` on the checkout |

Proof 3 is recorded as a non-finding rather than dropped: it shows the (f) case
cannot distinguish "report both sites" from "report the later site", which is
why the implementation reports both for the reader's benefit and the case does
not claim to prove it.

## 6. Recorded limits

- **The `fix:` label is not asserted for `(g)` hits.** The human report prints
  `fix: <edit>` but does not repeat the adopted spelling for a predicate with no
  retired literal, and the only way to make the `fix:` label a case is to run the
  guard over a tree with a hit - which the checkout cannot be, by construction.
  Cases 2 and 16 assert the label on seeded runs instead.
- **`--self-test` still writes into the checkout.** It has to: it is the guard's
  own contract, and a self-test of a private copy would prove the copy. That one
  case is why the module keeps its nextest process group.
- **The cost case is a 10 s budget, not a measurement.** The measured run is
  about 2.2 s warm on the full tree; the budget leaves room for a loaded CI
  runner while still failing a change that makes the guard walk the repository.
- **`tools.xlsx` absence is proven by a copy that lacks it, not by a chroot.**
  The import check is the second half: the module cannot open a workbook at all.
  The forbidden-spelling list in that case names the open-symbol in halves
  (`"Workbook::"`), because this very file is scanned by the FR-023 invariant case
  (`test_tool_rename_table::test_tool_rename_invariant_test_does_not_open_the_workbook`),
  which fails any `crates/ragent-agent/tests/*.rs` naming the whole symbol. It
  caught the first draft of this task, which is the case doing its job.
- **The full `ragent-agent` suite found one regression this task had introduced**
  and it is fixed: the workbook case above. `cargo nextest run -p ragent-agent`
  is now 2161 run, 2161 passed, 1 skipped.
- **`cargo check --workspace --all-targets` reported two dead fields** in the new
  JSON reader (`GuardReport::problems`, `GuardHit::nfr008`). Rather than keep
  them for symmetry, they were removed: nothing asserted them, and a field a case
  never reads is a field a reader will assume is checked.

## 7. Verification

- `cargo fmt --all --check` clean.
- `python3 scripts/check_tool_rename.py` exit 0, `44 reviewed exemption(s) all match`.
- `bash scripts/check-tool-rename.sh --self-test` exit 0, and no
  `crates/ragent-tools-core/src/__tool_rename_guard_selftest.rs` survives.
- `cargo nextest run -p ragent-agent -E 'test(test_tool_rename_guard::)'`:
  **16 run, 16 passed** (was 12 cases, 5 of them failing on the pre-existing
  predicate (g) hit that the exemption now reviews).
- The full-tree run is 2.2 s warm (NFR-009's "a few seconds").
- `cargo check --workspace --all-targets` and `cargo clippy -p ragent-agent
  --all-targets` name no line this task added (after the two dead fields and the
  one `starts_with('*')` lint were fixed).
- `cargo nextest run -p ragent-agent`: **2161 run, 2161 passed, 1 skipped**.
- All twelve `scripts/check-*.sh` guards pass, and all twelve pass their
  `--self-test`.

## 8. Files

| File | Change |
|---|---|
| `scripts/check_tool_rename.py` | `--json`, `--root`, `--self-test`, `--check`; one fixture constant; per-root FR-023 pairs; root manifest in the dependency map; `(f)` reports both sites, string-literal keys only, forwarding `name()` skipped; two reviewed exemptions |
| `scripts/check-tool-rename.sh` | rewritten as a 67-line wrapper with no predicate logic and no fixture |
| `crates/ragent-agent/tests/test_tool_rename_guard.rs` | rewritten: 16 cases, a scratch-copy harness, a JSON reader, the two entry points compared |
| `crates/ragent-agent/src/tool/aliases.rs` | doc comment on `UpdateFileTool::execute` naming the designed forward |
| `crates/ragent-agent/src/tool/mod.rs` | doc comment on `LegacyMultiEditAlias` explaining why `name()` returns a constant |
| `.config/nextest.toml` | the group comment now says which two cases still seed into the checkout |
| `specs/uniformtools/PLAN.md` | this task's row -> completed |
| `specs/uniformtools/TESTPLAN.md` | TC-030's automated half, part 2 |
| `specs/uniformtools/SPEC.md` | "The guard is one implementation, in two forms" |
| `specs/uniformtools/T-048-ONE-IMPLEMENTATION.md` | this record |
| `CHANGELOG.md` | one `### Added` bullet |
| `crates/ragent-agent/DOCS.md` | the guard's one-implementation note |
