# T-040 - The rename adds no per-step cost (NFR-006)

Status: **completed**. Requirement: NFR-006 (spec `uniformtools`). Dependency: T-031
(the source rename) and T-030 (the FR-023 table).

## What the requirement asks

NFR-006 is a *cost* requirement with three clauses, and they fail in three
different ways:

1. **tool names remain borrowed `&str` values compared directly** - broken by a
   `fn name(&self) -> String`, or by a resolver that allocates a `String` to
   compare against;
2. **the FR-023 table is read only by tests and by migration sites, and no
   dispatch step consults it to translate a name** - broken by the
   plausible-looking `Rename::after_of(name)` in the dispatch pipeline, added "so
   a stale spelling still works", which is precisely the alias FR-026 forbids and
   also puts a 13-row linear scan in front of every tool call;
3. **resolution, the repeat guard and the definition cache key on the new
   spelling with the same cost as the old one** - broken by leaving one of them
   keyed on a spelling the registry no longer holds.

Clauses 2 and 3 are now mechanically checked; clause 1 is checked by naming the
borrowed types explicitly, which makes an allocating signature a compile error in
the test rather than a slow path in the binary.

## What was built

No production code changed. `crates/ragent-agent/tests/test_nfr006_rename_cost.rs`
(registered in `tests/all.rs` as `test_nfr006_rename_cost`) - 6 cases:

| # | Case | Clause |
|---|---|---|
| 1 | `test_no_dispatch_step_consults_the_rename_table` | no read inside `session/`, `tool/` or `backend/` except the two named once-per-turn sites |
| 2 | `test_the_rename_table_is_read_only_by_listed_migration_sites` | every workspace read is a listed migration site, and every listed site still matches a read |
| 3 | `test_tool_names_stay_borrowed_and_the_table_lookup_borrows_too` | `&str` / `Option<&'static str>` named explicitly for all 13 pairs |
| 4 | `test_resolution_keys_on_the_new_spelling` | the adopted spelling is the registry key; `get(before)` is `None`; the folded fallback answers with the registered spelling |
| 5 | `test_the_definition_cache_keys_on_the_new_spelling` | `definitions()` shares one `Arc` and holds the adopted names; the serialised cache fingerprints the name, so `Bash` and `bash` are different entries |
| 6 | `test_the_repeat_guard_keys_on_the_new_spelling` | five identical `Bash` calls pass, the sixth is denied; `bash`/`Bash` alternating never accumulates; a different tool resets |

### The scan is function-granular, and why that is not pedantry

A file-level exemption would make clause 2 meaningless. `alias_table.rs` holds
*three* functions the pipeline touches and exactly **one** that reads the table:
`from_aliases` (once per turn, building the alias table) is legitimate;
`resolve_inbound_tool_name` (once per tool call) is the dispatch path. Exempting
the file would permit a rename lookup inside the resolver - the red proof below
does exactly that and the guard catches it because the hit is attributed to
`resolve_inbound_tool_name`.

The scan reports `file:line: (fn) text`, attributes a hit to the nearest preceding
`fn` *signature* (so a call to `foo(...)` is not a definition), and skips comment
lines - several doc comments in this tree state that the table is not consulted on
a dispatch path, and a guard that demanded an exemption for prose explaining
itself would train the next reader to add one.

The 13 exemptions, each with a written reason, are: the table and its four
lookups (`ragent-types`), its `lib.rs` declaration and re-export, the
`ragent-agent` re-export, `AliasTable::from_aliases`, the hook matcher's
`retired_matcher_spellings`, `ToolsConfig::validate_alias_spellings` and its
`after_spelling_of` helper, and the TUI's `canonical_tool_name` (which paints a
*stored* pre-rename record that FR-028 replays verbatim).

The check runs in both directions: a read that is not listed fails, and a listed
exemption that no longer matches anything also fails. Without the reverse check the
list rots into a superset, and the next reader inherits permission for a function
they did not write. (The reverse check found a real one on the first run: an
exemption I had written for `alias_table.rs` at module scope, where the file has
`use super::rename::Rename;` - the type import spells none of the scanned symbols,
so the entry could never match. Removed.)

## Red proofs (run, observed, reverted)

Each was applied, the file `touch`ed (a `cp -p` restore keeps the older mtime, so
cargo reuses the broken object file and the "reverted" tree still fails), the suite
run, and the file restored byte-identically (verified by `md5sum` against the
backup).

1. `Rename::after_of(name)` inside `resolve_inbound_tool_name` (the per-call
   resolver) - cases 1 **and** 2 failed naming
   `crates/ragent-agent/src/tool/alias_table.rs:306: (resolve_inbound_tool_name)`.
   This is the exact change NFR-006 exists to prevent, and both the dispatch-tree
   check and the exemption check see it.
2. `Rename::after_of` in `tool_call_key` (the repeat guard's hash) - cases 1, 2
   **and** 6 failed; case 6 with "`bash` and `Bash` are different key values, so
   neither accumulates (step 5)". Translating the guard's name made a retired
   spelling spend the adopted tool's budget, which is the observable harm.
3. Content-blind serialised-cache key (`(format, 0u64)`) - case 5 failed with
   `left != right` on the two byte buffers: without the name fingerprint the
   retired and adopted spellings collapse to one entry.
4. Never serving the sorted-definition `Arc` - case 5 failed with "the
   sorted-definition cache is reused while the registry is unchanged".
5. A helper `owned_after_of(&str) -> Option<String>` added beside the table -
   cases 1 and 2 failed. The scan measures *reads of the table*, not just reads on
   the dispatch path, so a convenience allocator added anywhere is reported.
6. `Rename::after_of` changed to `Option<String>` - `cargo check` failed with
   `expected &str, found String` in `ragent-config`, which is the same compile-time
   property case 3 pins from the test side.

Proofs 1, 2 and 4 were run against the whole `--test all` binary filtered to
`test_nfr006_rename_cost::`, so no other child file's state affected the result.

## Gotchas

- **`Arc::ptr_eq` is not assertable across a consolidated test binary.** The
  `definitions()` cache is per registry and safe, but `ragent_llm`'s serialised
  cache is a process-global `OnceLock`, and other child files call
  `invalidate_tool_cache()`. The cache assertion is therefore byte equality (which
  is exactly FR-003's claim) and not pointer identity.
- **`definitions()` on a fresh `create_default_registry()` is already warm** for
  the pointer check, because the *first* call populates and the second reuses - but
  only if nothing registers in between. A test that registers and then compares
  pointers to an earlier `Arc` is comparing two different `Vec`s by design.
- **The word-changing pairs are not reachable by case folding.**
  `ask_user` -> `AskUserQuestion` differs by more than case, so
  `registry.resolve("ask_user")` must be `None`. Asserting "the retired spelling
  resolves somewhere" would be the opposite of FR-026; the case splits the 13 into
  the 9 case-only pairs (which the folded fallback reaches) and the 4 word-changing
  ones (which it must not).
- **The comment filter is load-bearing in both directions.** Without it the guard
  reports doc comments that *state* the property ("nothing consults the table on a
  dispatch path") and the natural fix is to add exemptions, which is how a guard
  becomes a formality.

## Docs updated

- `specs/uniformtools/T-040-RENAME-COST.md` - this record.
- `specs/uniformtools/PLAN.md` - T-040 marked completed.
- `CHANGELOG.md` - Unreleased entry.

## Verification

`cargo fmt --check`; `cargo check -p ragent-agent --tests`;
`cargo clippy -p ragent-agent --all-targets` (no hit in the new file; only the
pre-existing `pub(crate)`-in-private-module noise elsewhere);
`cargo test -p ragent-agent --test all test_nfr006_rename_cost::` 6 passed; the
eight guard scripts (`check-inline-tests`, `check-file-tool-containment`,
`check-shared-guards`, `check-silent-errors`, `check-security-unwraps`,
`check-poison-locks`, `check-dead-code-reasons`, `check-test-env-isolation`) pass.
No production file was modified - `git diff --stat` on the six files touched by the
red proofs is empty.
