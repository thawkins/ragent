# T-035 - Permission categories, hook matchers and stored history survive the rename (FR-028)

Status: **completed**. Requirement: FR-028 (spec `uniformtools`). Dependency: T-031
(the source rename) and T-030 (the FR-023 table).

## What the requirement asks

FR-028 is a *state-driven* requirement: it holds for the lifetime of a config file or
a saved session, both of which outlive the rename. It names four duties, and they are
four different kinds of state:

1. `permission_category()` values are **unchanged** by the rename - they are derived
   independently of the tool name (`file:read`, `bash:execute`, `plan`, `task`,
   `skill:manage`), so a `"permission": "..."` rule written before the rename keeps
   governing the tool after it;
2. a hook whose `matcher` names only a `before` spelling is **reported once at load**
   with the renamed replacement, "because a silently non-matching hook is disabled
   without its author knowing";
3. a `MessagePart::ToolCall` recorded before the rename is **replayed to the provider
   verbatim, unrewritten**;
4. a tool-name-keyed classifier that does not recognise a stored name treats it as
   **non-destructive** rather than failing or panicking.

Duties 1, 3 and 4 were already true of the tree, and the point of this task is to make
them *provably* true rather than incidentally true - a rename is exactly the change that
breaks them silently, and nothing in the compiler notices a string. Duty 2 did not exist
and is the only new production code.

## What was built

### Duty 2 - the load-time report (`crates/ragent-agent/src/hooks/mod.rs`)

- **`HookConfig::retired_matcher_spellings()`** - the stale part of a matcher, as
  `(retired spelling, adopted spelling)` pairs in matcher order. It splits exactly as
  `matches_decoded_tool` splits (`|` and `,`), takes the bare tool name before any
  `Name(pattern)` guard, and resolves it through `Rename::after_of` /
  `Rename::before_of`, so both columns are the table's own `&'static str` and no
  allocation happens on the reporting path. A matcher naming the *adopted* spelling is
  silent: that is the correct spelling, and warning about it would be noise on every
  hook the rename has already fixed.
- **`report_retired_matcher_spellings_once(&[HookConfig]) -> usize`** - logs one
  `warn!` per stale entry, naming the spelling, its replacement and the trigger, and
  returns how many it reported so a test can tell "first report" from "already
  reported".
- **`retired_matcher_spellings_seen()`** - a process-wide, poison-tolerant
  `Mutex<HashSet<(String, String)>>`, matching the `OnceLock` registries already in this
  crate (`alias_table::unresolved_targets_seen`, `agent`, `reference`). Process-wide
  rather than per-session because the hook list is rebuilt from the configuration on
  every turn: a naive `warn!` would emit the same sentence on every step and train the
  reader to ignore it. Per *entry* rather than per list, so adding a different stale
  matcher still reports.
- **`reset_retired_matcher_spellings_for_test()`** and
  **`retired_matcher_spellings_for_test()`** - `#[doc(hidden)]` helpers, because the
  ledger is process-wide and this crate's test binary is one process, and because an
  integration test in another crate reads the pairs as data.

### When the report fires

FR-028 says "at load". Two call sites, and they are the two places a hook list is
created:

| Site | Why |
|---|---|
| `crates/ragent-tui/src/app/init.rs`, immediately after `Config::load()` | the interactive surface's load: the warning is in the log panel before any turn runs, which is what TC-023 step 4 asks for |
| `crates/ragent-agent/src/session/loop_steps.rs`, where `parsed_hook_configs` is derived per turn | the headless surfaces (`ragent run`, the server, ACP) have no separate load step, so the point where the turn's hooks are built is their load |

The ledger makes the second site free once the first has reported. Neither site rewrites
the matcher: the file belongs to its author, and a load-time rewrite would leave the
on-disk text disagreeing with the running session - which is worse than a warning,
because the next reader would believe the file.

### Duties 1, 3 and 4 - the assertions

No production change was needed for 1 and 3; the tree already derived categories
independently of names and already forwarded `tool.clone()` unchanged. Duty 4's
classifier also already answered `false` for an unknown name - and that is the property
worth pinning, because the obvious "tidy-up" of `is_destructive_git_invocation`'s `_ =>
true` arm (which is *reachable only for a listed name*) would break it if anyone moved it
one layer up. The doc comment on `is_destructive_tool` now says so.

`permission_category()` needed **no** edit for the rename, and the two places it looks
like it did are the requirement: the family `ask_user` holds a tool now called
`AskUserQuestion`, and `bash:execute` is still reported by the tool now called `Bash`.
`T-041-BASELINE.md` records both as deliberate; `docs/howtos/permissions.md` now says so
to a reader.

## Tests

`crates/ragent-agent/tests/test_fr028_state_survival.rs` - 9 cases, registered in
`tests/all.rs` as `test_fr028_state_survival`.

| # | Case | Clause |
|---|---|---|
| 1 | `test_every_renamed_tool_keeps_the_permission_category_it_had_before_the_rename` | all 13 renamed tools report the literal category they reported pre-rename |
| 2 | `test_the_permission_category_vocabulary_is_unchanged` | the 32 distinct categories, as a set, plus the two collisions by name |
| 3 | `test_a_hook_matcher_naming_only_a_retired_spelling_is_reported_once` | the report, and its per-entry-per-process shape (`#[serial_test::serial]`) |
| 4 | `test_a_hook_matcher_naming_the_adopted_spelling_or_no_matcher_is_silent` | every adopted spelling, `None`, an untouched name, an empty matcher |
| 5 | `test_a_guard_matcher_contributes_its_bare_tool_name` | `bash(git commit:*)` reports; `Bash(git commit:*)` does not |
| 6 | `test_a_matcher_naming_several_retired_spellings_reports_each` | a list matcher reports every stale part, in order, and no adopted one |
| 7 | `test_a_stored_tool_call_record_replays_to_the_provider_verbatim` | every retired spelling, and a name that never existed (`#[tokio::test]`) |
| 8 | `test_a_stored_record_survives_a_storage_round_trip_unrewritten` | in-memory `Storage`, read back, same spelling |
| 9 | `test_an_unrecognised_stored_tool_name_is_classified_non_destructive` | `Bash` still destructive; `bash` and a nonsense name are not |

The category expectations are **literals**, not derived from the FR-023 table. Deriving
them would make the assertion agree with whatever the tree happened to do - the one thing
FR-028 forbids - and the 32-value set is the cross-check against
`T-041-BASELINE.md` section 4.

## RED proofs (run, observed, reverted)

1. Disabling the rename lookup in `retired_matcher_spellings` (`if false && let Some(after)
   = ...`) failed **three** cases - 1, 5 and 6 - with `left: []`. The report is what those
   cases measure, not an incidental side effect.
2. Emptying the loop in `report_retired_matcher_spellings_once` failed case 3 only, which
   is the split the two functions are meant to have: the accessor is the derivation, the
   reporter is the once-per-entry ledger.
3. Rewriting the name on the way out in `parts_to_chat_content_sync`
   (`Rename::after_of(tool).unwrap_or(tool)`) failed case 7 with
   `left: "Bash" / right: "bash"` - so the verbatim replay is measured, and a "helpful"
   canonicalisation is caught.
4. Answering `true` for an unrecognised name in `is_destructive_tool` failed case 9.
5. Changing `AskUserQuestion`'s `permission_category()` to `"ask_user:legacy"` failed cases
   1 **and** 2, so both the per-tool table and the vocabulary set are real assertions.

Every file was restored from `target/temp/*.bak` and `touch`ed: a `cp -p` restore keeps
the older mtime, so cargo reuses the broken object file and the "reverted" tree still
fails (or worse, passes for the wrong reason).

## Gotchas

- **The `_ => true` in `is_destructive_git_invocation` is not the same wildcard as the one
  in `is_destructive_tool`.** The inner one is reached only for a name already in
  `DESTRUCTIVE_GIT_TOOLS`, and defaulting to destructive there is the safe direction for a
  checkpoint. The outer answer for an unknown name must be `false`. Moving the inner
  wildcard up, or "simplifying" the two into one `match`, silently makes every unknown
  stored name destructive - which case 9 catches, and which is why the doc comment states
  the distinction.
- **A process-wide ledger in a one-binary-per-crate test suite needs `#[serial]`.** Case 3
  takes `serial_test::serial` *and* resets the ledger, or an unrelated test that reports
  the same pair steals the first report.
- **The hook matcher report cannot go in `parse_hook_configs`.** It is a pure parse helper
  called from more than one place and has no ledger; and a `warn!` there would fire on
  every turn, because the parse is per turn.
- **`all_tools()` returns `Vec<Arc<dyn Tool>>`, not an iterator.** `.map(...)` on it does
  not compile, and `registry.all_tools().iter()...collect()` borrows a temporary - the
  `Vec` has to be bound first.
- `clippy::assertions_on_constants` / `assert_is_empty` fire on
  `assert!(vec.is_empty())` in a test; `assert_eq!(vec, Vec::new())` says the same thing
  and lints clean.

## An incidental find: a guard that had quietly stopped checking four tools

`scripts/check-file-tool-containment.sh` keeps a `FILE_TOOLS` list of the tools that read or
write a caller-supplied path, and locates each one's source file by matching a whole-line
`"<name>"` literal. The list was still spelled pre-rename (`write edit glob grep`), so the
guard found no source file for those four, printed

```
WARN: no source file found for file tool 'write' (moved or renamed?)
```

and `continue`d - **exit 0, four file tools unchecked**. Before T-031 it checked 17; after
it, 13. The warning was accurate and nobody was reading it.

This is the same class of defect FR-027 and FR-034 exist for, one directory outside their
reach (`scripts/`, not non-test Rust source), and it is the reason a stale literal in a
*guard* is worse than one in a message. The four entries are now the adopted spellings and
the guard reports `OK: 17 file-tool source file(s)`. The list carries a comment saying why
the spelling matters, so the next reader does not "restore" it.

## Docs updated

- `docs/howtos/hooks.md` - the hook entry table gained the `matcher` field (it was
  documented in the code but not in the guide) and a new subsection "Tool names in a
  `matcher` use the current spellings", which states the report and that a stale spelling
  is a warning, not an alias.
- `docs/howtos/permissions.md` - a new subsection "A category is permission vocabulary,
  not a tool name", naming the `bash` / `AskUserQuestion` traps and why a rule keeps
  working.
- `CHANGELOG.md` - Unreleased entry.
- `specs/uniformtools/PLAN.md` - T-035 marked completed.
- `specs/uniformtools/T-035-STATE-SURVIVAL.md` - this record.

## Verification

`cargo fmt --check`; `cargo clippy -p ragent-agent -p ragent-tui --all-targets` (only the
pre-existing `pub(crate)`-inside-private-module, `get(0)`, and one pre-existing
`MutexGuard`-across-await warning); `cargo check --workspace --all-targets`;
`cargo test -p ragent-agent --test all` 1842 passed / 1 ignored (including the 9 new
cases, `test_session_text_helpers` 40, `hooks` 36, `test_loop_checkpoints` 9,
`test_escape_hatch_audit` 8); the eight guard scripts
(`check-inline-tests`, `check-dead-code-reasons`, `check-silent-errors`,
`check-security-unwraps`, `check-poison-locks`, `check-test-env-isolation`,
`check-file-tool-containment`, `check-shared-guards`) pass.
