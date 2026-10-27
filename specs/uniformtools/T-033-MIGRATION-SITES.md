# T-033 - Migrate every non-test literal site (FR-027)

**Spec ID:** uniformtools
**Task:** T-033
**Requirement:** FR-027 (every non-test site that keys on a renamed name migrates in
the same change - ubiquitous)
**Status:** completed
**Depends on:** T-031 (the source rename) and T-030 (the FR-023 table)

## What the requirement asks

FR-027 is a *ubiquitous* requirement. Every non-test source site that compares, matches,
lists or stores a renamed tool name as a literal shall be updated to the `after`
spelling. The requirement names a floor, not a ceiling ("including at least"), so the
migration was driven by a fresh scan of the tree rather than by the list in the task text:

```text
python3 scan: for each crates/*/src/**/*.rs (tests/ excluded), report every
occurrence of a "before" spelling as a double-quoted, single-quoted or backticked
literal, and separately report every `fn description(&self)` body that names one.
```

The scan ran before the change, was filtered to tool-name sites by reading each hit, and
re-ran after. Measured counts (a quoted/backticked `before` literal, `tests/` and
`benches/` excluded, the FR-023 table excluded):

| Scan | Hits |
|---|---|
| start of this session, every quoted/backticked `before` literal | 166 lines across 54 files |
| after, every quoted/backticked `before` literal | 86 lines across 33 files |
| after, excluding comment lines (the only lines a dispatch site can be on) | 44 lines |
| of those, flagged by `scripts/check-tool-rename.sh` (T-034 predicate (e)) | **0** |
| `description()` bodies naming a retired spelling: before / after | 11 / 0 |

The 44 non-comment lines that still contain a `before` spelling are all reviewed
non-tool names, and the guard proves it: it reports zero unexempted lines because every
one of them is either a built-in non-tool bucket (shell program names, language ids,
config keys, stage ids, `pluralize()` nouns) or one of **four** per-line exemptions with
a written reason. Those four are the FR-028 state-driven values that must **not** change:
`AskUserQuestion`'s `permission_category()` (`"ask_user"`), the two permission-type
comparisons in `session/permissions.rs` (`"edit"`/`"write"`), and the `"bash"` memory
tag. A zero-violation result is therefore a decision, not an omission - which is why the
exemptions were added individually rather than by widening a bucket.

The requirement also calls out the **eleven `description()` strings** separately, because
a description is prompt text the model reads: a stale one instructs the model to call a
tool by a name that now fails. Those were scanned by extracting each
`fn description(&self)` body and matching only within it, so a mention of a *shell*
program named `bash` in a module doc comment is not confused with a mention of the tool.

## What was migrated

### Agent guards (a stale literal silently disables a check)

| Site | Before | After |
|---|---|---|
| `session/permissions.rs` `INTERACTIVE_TOOLS` | `ask_user` | `AskUserQuestion` |
| `session/permissions.rs` `SKILL_ALWAYS_ALLOWED_TOOLS` | - | `AskUserQuestion` (already adopted) |
| `session/permissions.rs` `is_hardwired_auto_approved_tool` | `ask_user`, `"task_"` prefix | `AskUserQuestion`, explicit `TASK_TOOLS` list |
| `session/permissions.rs` `is_destructive_tool` bash branch | `bash` | `Bash` |
| `session/loop_state.rs` `LOOP_WRITE_TOOLS` | `write`, `edit` | `Write`, `Edit` |
| `session/loop_state.rs` `LOOP_ALWAYS_ALLOWED_TOOLS` | `ask_user` | `AskUserQuestion` |
| `session/loop_state.rs` loop bash path-token / write checks | `bash` | `Bash` |
| `session/history.rs` write-tool list | `write`, `edit` | `Write`, `Edit` |
| `session/processor.rs` bash sub-command permission path | `bash` | `Bash` |
| `team/mod.rs` `PLAN_GATED_TOOLS` | n/a | **not a site in this tree**: no `PLAN_GATED_TOOLS` const exists, and the plan gate the task text means (`plan_gate_denial`) lives in `session/dispatch.rs`, which `session/mod.rs` never declares - an orphan module of the same class T-012/T-013 found. Nothing to migrate until that module is registered; recorded so the omission is a finding, not a miss |
| `memory/extract.rs` bash / edit branches | `bash`, `edit`, `write` | `Bash`, `Edit`, `Write` |
| `backend/container.rs` routing + `bash` gate | `bash`, `write`, `glob`, `grep`, `append_file`, `mkdir` | `Bash`, `Write`, `Glob`, `Grep`, `append_to_file`, `make_directory` |
| `research_adapter.rs` registry lookups | `Glob`/`Grep` (already adopted) | unchanged, doc text updated |
| `tool/aliases.rs` delegation targets | `write`, `bash` | `Write`, `Bash` (Rust types + doc text) |
| `skill/bundled.rs` `allowed_tools` | `Bash`/`Grep`/`Glob` (already adopted) | bodies: `Grep`/`Glob`/`Write` already adopted, no change needed |
| `tools-core/src/edit.rs` edit-log `tool` field | `"edit"` (14 call sites) | `"Edit"` |
| `ragent-tui/src/widgets/message_widget.rs` `canonical_tool_name` | alias map only | alias map, then `Rename::after_of` |
| agent/codeindex prompt text | `grep`, `glob`, `bash tool` | `Grep`, `Glob`, `Bash tool` |
| doc (`///`, `//!`) prose naming a tool | retired spelling | adopted spelling (see below) |

The `"task_"` **prefix** was replaced by an explicit `TASK_TOOLS` list of the four
registered `Task*` spellings. A prefix is a looser predicate than the rule it encodes:
it named a spelling the rename retired, and it would have auto-approved any future tool
whose name merely started with those letters.

The repeat guard needed no literal change: `tool_call_key` hashes the *resolved* tool
name plus the canonicalised arguments, so it keys on `Bash` automatically once the
registry resolves `Bash`. It is recorded here because the task lists it - the check is
that no tool-name literal exists on that path, which the scan confirms.

`permission_category()` values are **not** touched: FR-028 keeps `bash:execute`,
`file:read`, `ask_user`, `plan` and friends unchanged, so `ask_user` remains the
`AskUserQuestion` tool's category and `Permission::{Edit,Bash,PlanEnter,PlanExit}` keep
their display strings.

### `canonical_tool_name` resolves a retired spelling

`crates/ragent-tui/src/widgets/message_widget.rs`'s `canonical_tool_name` mapped only
`update_file` -> `Write` and `run_code` -> `Bash`. FR-028 replays a stored
`MessagePart::ToolCall` verbatim, so a session resumed from history written before the
rename still carries `"bash"`/`"grep"`/`"task_create"` and would have fallen through to
the generic argument dump. The function now resolves any retired spelling through
`ragent_types::tool_rename::Rename::after_of`, so a replayed record keeps its rich
rendering while a live call is unaffected (it is already canonical). The two existing
cases that assert the alias mapping pass unchanged; one new case covers the fallback
(see Tests).

### In-container routing table

`container_script` matches the tool name the backend is asked to run. It named
`read`, `create`/`write`, `append_file`, `rm`, `mkdir`, `list`, `glob` and `grep`. The
rename's first pass had already moved `write`, `glob`, `grep` and `bash` (in
`execute_tool`); `append_file` and `mkdir` were still keyed on names the registry no
longer holds, and the two names the registry actually has are `append_to_file` and
`make_directory`. Both arms now use the registered spelling, so every tool the sandbox
claims to serve routes. The module docs and the `MAX_OUTPUT_LINES` doc comment were
updated to match.

The routed-tool test table in `tests/test_backend_misc_more.rs` was migrated in the same
change (`append_file` -> `append_to_file`, `mkdir` -> `make_directory`, including the
`container_path` escape case that drove `mkdir`), because those cases had been passing for
the wrong reason - see Gotcha 4.

### Tool descriptions (prompt text the model reads)

Eleven shipped `description()` bodies named a *different* tool by its old spelling:

- `bash_reset` and `rm` said "use `bash`";
- `append_to_file`, `create` and `multi_edit` said "use `edit`";
- `list` said "use `glob`";
- `codeindex_dependencies`, `codeindex_references`, `codeindex_search` and
  `codeindex_symbols` said "use `grep`" / "`grep` or `glob`" (their `fallback_tools`
  metadata strings too);
- `edit` said "switch to `write`/`patch`" and "prefer `create`/`write`".

A post-scan of every `description()` body in the tree returns zero occurrences of a
retired spelling.

### Agent and code-index prompt text

The system-prompt builders carried prose that named the tools: the code-index guidance
section (`session/prompt_builders.rs`) said "instead of grep", `Grep` was capitalised in
the directive language but `grep`/`glob` remained in the decision-flow bullets, the
"when `grep` IS appropriate" heading and both `grep(pattern: ...)` examples. The
built-in agent profiles in `agent/mod.rs` said "Use 'grep' or 'search' … 'glob' to find
files by name", "Use read, grep, glob, and list tools", "Use bash commands" and "Test
changes when possible using the bash tool". All now name the adopted spelling.

### TUI display

`crates/ragent-tui/src/widgets/message_widget.rs` is the reason FR-027 is user-visible.
Its `tool_input_summary` / `tool_result_summary` / `tool_inline_diff` arms and the
per-tool render branches were keyed on the retired spellings, so a real `Bash` call fell
through to the generic argument dump. The arms now key on `Bash`, `Write`, `Edit`,
`Grep`, `Glob`, `AskUserQuestion`, `EnterPlanMode`/`ExitPlanMode` and the four
`Task*` spellings; `make_directory` and `append_to_file` are the canonical spellings
(the short `mkdir` / `append_file` names were never registered). The style-guide doc
comment above the summaries was rewritten to the registered names, and
`canonical_tool_name` now resolves a retired spelling through `Rename::after_of` (with
`update_file`/`run_code` still mapped first), so a session resumed from saved history
(FR-028 replays stored records verbatim) keeps its rich rendering instead of falling
through to the generic argument dump. The `/editlog help` text and the `layout.rs`
comment that name the tool were migrated with it.

### Stored tool records and doc prose

Two further classes of literal were migrated once the scan was read line by line:

- **The edit log's `tool` field.** `crates/ragent-tools-core/src/edit.rs` passed the
  literal `"edit"` as the recorded tool name in all fourteen `log_edit_operation` /
  `EntryExtras` call sites, so every new `log/editlog/edits-*.jsonl` record named a tool
  the registry no longer holds. All fourteen now record `Edit`. The **file-name** label
  (`edits-<timestamp>.jsonl`) is not a tool name and is unchanged, and records already
  written keep whatever they stored (FR-028: a stored record is replayed verbatim).
- **`///` and `//!` doc prose that names a tool.** A doc comment is not a dispatch site,
  but a doc comment that names the wrong tool is a doc that tells the next reader a
  falsehood, so every comment naming a *tool* in the touched files was migrated -
  `tool/mod.rs`, `tool/aliases.rs`, `tool/team_create.rs`, `tool/allowed_tools.rs`,
  `tool/skill_manage.rs`, `research_adapter.rs`, `memory/extract.rs`,
  `session/permissions.rs`, `backend/container.rs`, `bang_command.rs`,
  `codeindex/lib.rs`, `research/local_gatherer.rs`, `storage/storage.rs`,
  `tools-core`'s `edit_common.rs` / `edit_log.rs` / `replace.rs` / `list.rs` / `lib.rs` /
  `askpass.rs`, `tools-extended/src/task.rs`, `telemetry/sensitive.rs`,
  `tui/src/app/{research,session_ops}.rs`. Prose that names a *shell* program, a
  language id, a config section, a permission type or a stage id was left alone.

## Deliberately not migrated

A literal that is not a tool name is not this requirement's business, and changing it
would be a behavioural change with no relation to the rename:

- shell program names: `Command::new("bash")`, `which::which("bash")`,
  `OsStr::new("bash")`, `ShellType::Bash => "bash"`, the bash validator's command lists;
- language ids and the scaffold's language tables: `"sh" | "bash" => Language::Shell`,
  the `sh`/`bash` help text and the shell manifest template;
- the `bash.allowlist` / `bash.denylist` **config section** name and the
  `permission_category()` values (FR-028 keeps both);
- `Permission`'s `From<&str>` arms and `Display` strings (`"edit"`, `"bash"`,
  `"plan_enter"`, `"plan_exit"`), which are permission *types*, not tool names;
- the OASF doc list in `agent/oasf.rs`, which documents permission-type spellings;
- the `"write"` stage id in `archdoc/govcreate_run.rs` and the `"write"` verb in
  `specs/validate.rs`'s testable-verb table;
- `pluralize(count, "edit", "edits")`, the `"bash"` memory **tag** (it names the shell
  the failure came from, and a stored tag is a value, not a lookup) and the edit log's
  file-name label `edits-*.jsonl` (a filename, not a tool name; the log entry's `tool`
  **field** is migrated - see above);
- `"ask_user"` as `AskUserQuestion`'s `permission_category()` (FR-028);
- the code-index scanner's `"shell"` dialect and the `"sh" | "bash"` language aliases;
- model- or remote-supplied names, which the text-tool-call extractor and the SSE
  decoders pass through verbatim.

## Tests

FR-027 is a migration, so its proof is the scan plus the suites that assert on the sites
it moved. No new test *file* was added; one new **case** was (see the end of this
section, because it is the only behaviour change). Seven existing assertions named an old
spelling as the expected string and were updated to the new one rather than relaxed:

- `crates/ragent-agent/tests/test_skill_manage.rs` - the unknown-action message now
  reads "Unknown Skill action".
- `crates/ragent-agent/tests/test_memory_extraction.rs` - the cases now drive `"Bash"`
  as the tool name (they drove `"bash"`, which no longer reaches the extractor's
  `tool_name == "Bash"` branch). The error-resolution candidate's **tag** is still
  `"bash"` and the assertion for it is unchanged, with a comment recording why.
- `crates/ragent-tools-core/tests/test_edit_integration.rs` - four cases assert the
  edit-log entry's `tool` field, which is now `"Edit"`.
- `crates/ragent-tui/tests/test_slash_deep2.rs` - the `/editlog show` fixture JSON now
  carries `"tool":"Edit"` and the tool-column assertion looks for `Edit`.

One **new** case was added: `test_tool_display::test_canonical_tool_name_resolves_a_retired_spelling`
in `crates/ragent-tui/tests/test_tool_display.rs`. `canonical_tool_name` gaining the
`Rename::after_of` fallback is the only behaviour change in this task, so it is the only
one that needs a case of its own; it pins all four renamed families, a live canonical
name, and a name the table never held.

## Red proofs

Two of the changes are behavioural and were proved by breaking them and restoring
byte-identically (`md5sum`-checked):

1. **Container routing arms back to `append_file` / `mkdir`.** With those two arm patterns
   reverted, `test_backend_misc_more::container_scripts_reach_the_image_check_for_every_routed_tool`
   fails - the cases take the refuse path and the assertion that the call reaches the
   image check (a provisioning failure) never holds. This is the defect the task exists
   to fix: the arms and the test table had drifted from the registry *together*, so the
   test was green while proving nothing. Restored: the file's md5 matches the backup.
2. **`canonical_tool_name`'s `Rename::after_of` fallback removed.** With `other => other`
   restored, `test_canonical_tool_name_resolves_a_retired_spelling` fails on the first
   `bash` assertion. Restored: md5 matches the backup.

Both restores were followed by `touch` on the restored file: cargo reuses an object file
with an older mtime than its dependencies, so a `cp -p`ed restore can silently keep the
broken build and report the unmodified test as failing.
- `crates/ragent-agent/tests/test_backend_misc_more.rs` - the routed-tool table drives
  `Write`, `Glob`, `Grep`, `append_to_file` and `make_directory` (it previously drove
  `write`, `glob`, `grep`, `append_file` and `mkdir`, none of which `container_script`
  matched, so those cases were silently exercising the refuse path).
- `crates/ragent-specs/tests/test_spec_commands.rs` - two cases asserted the add prompt
  contains "`edit` tool"; the prompt now says "`Edit` tool".

## Findings (recorded, not fixed here)

Two things the scan turned up that belong to another task:

- **`crates/ragent-agent/src/session/dispatch.rs` is an orphan module.**
  `session/mod.rs` declares `acp_dispatch` and `remote_dispatch` but not `dispatch`, so
  the unified-dispatch pipeline (including `plan_gate_denial`, which `dispatch.rs:232`
  calls as `crate::team::plan_gate_denial` - a function that exists in no compiled
  module) is dead source. This is the same defect class T-012 (`tool_args`),
  T-013 (`tool_input_error`) and T-030 (`tool_rename`) each found, and it is why the
  task text's `PLAN_GATED_TOOLS` has nothing to point at: no such const exists in the
  tree. It is T-014's to land, not T-033's, because registering the module changes which
  pipeline compiles rather than which literal a comparison uses.
- **The orphan files keep their own retired literals.** `session/dispatch.rs`,
  `tool/allowed_tools.rs`'s doc examples and `tests/all.rs`'s missing `#[path]` lines
  mean those files contribute nothing to a build. Their literals are migrated here where
  cheap (the doc examples, because a reader will copy them), and left where migration
  would be a change to unbuilt code (the dispatch comments already explain the rename
  correctly, and rewriting prose in dead source has no reader).

## Verification

- `cargo fmt --check` - clean.
- `cargo clippy --workspace --all-targets` - no new warning (only the pre-existing
  `pub(crate)`-inside-a-private-module noise in `ragent-tools-vcs`, `ragent-research`
  and others).
- `cargo check --workspace --all-targets` - clean.
- `cargo test -p ragent-agent --test all` - 1694 passed, 1 ignored.
- `cargo test -p ragent-tools-core --test all` - 394 passed (the `Edit` tool's name is
  asserted by `test_lib_registry`, which was migrated with the rename in T-031).
- `cargo test -p ragent-tools-extended --test all` - 1806 passed, 11 ignored.
- `cargo test -p ragent-tui --test all` - 3647 passed, 3 failed. The three failures are
  pre-existing and unrelated to this task: `test_input_more::alt_toggle_keys_map_to_actions`,
  `test_input_queue_menu_binding::test_other_alt_keys_are_unaffected` and
  `test_memory_panel::test_toggle_memory_mutually_excludes_tasks_panel` all assert the
  Alt+T -> `ToggleTasksPanel` mapping. They fail because T-014/T-036's concurrent work on
  the same tree removed/changed that key binding; none of the three reads a tool name,
  and this task touches no input mapping.
- `cargo test -p ragent-config --test all` - 417 passed.
- `cargo test -p ragent-specs --test all` - 432 passed.
- `cargo test -p ragent-server --test all` - 192 passed.
- `cargo test -p ragent-research --test all` - 471 passed.
- `cargo test -p ragent-tools-vcs --test all` - 397 passed.
- `cargo test -p ragent-connectors --test all` - 369 passed.
- `cargo test -p ragent-storage --test all` - 346 passed.
- `cargo test -p ragent-types --test all` - 418 passed.
- `cargo test -p ragent-codeindex --test all` - 447 passed, 2 ignored.
- `cargo test -p ragent-telemetry --test all` - 231 passed.
- `cargo test -p ragent-llm --test all` - 293 passed, 5 failed, 2 ignored. All five failures
  are **pre-existing on this tree** and belong to a later task in this spec, not to
  T-033: `test_wire_decode`'s "no module invents a tool-call id" and "no module keeps its
  own tool-call map", `test_wire_encode_message`'s "no provider writes a result-carrier
  literal", and both `test_wire_trace` cases assert the T-014/T-015 encoder fold, which is
  not landed. None of them reads a tool-name literal.
- Nine guard scripts pass: `check-inline-tests`, `check-dead-code-reasons`,
  `check-silent-errors`, `check-security-unwraps`, `check-poison-locks`,
  `check-test-env-isolation`, `check-file-tool-containment`, `check-shared-guards`,
  `check-tool-rename` (T-034's predicate (e) - it now reports zero unexempted lines, and
  the four per-line exemptions it needed were added with their reasons in
  `scripts/check_tool_rename.py`).

## Gotchas

1. **A quoted literal is not always a tool name.** `"bash"` is a shell, a language id, a
   config section, a memory tag and a permission type as well as a retired tool name.
   Each hit was read before being changed; a blind `sed` over the tree would have
   rewritten the bash validator's shell-program allowlist and broken `Bash`.
2. **`description()` bodies need a separate scan.** A module-level `//!` comment naming
   `bash` is prose about the shell; a `description()` body naming it is an instruction to
   the model. Only the second is FR-027's business, so the scan extracted each
   `description()` body rather than matching whole files.
3. **`SKILL_ALWAYS_ALLOWED_TOOLS` was already correct.** `INTERACTIVE_TOOLS` had been
   migrated to `AskUserQuestion` earlier but `is_hardwired_auto_approved_tool` still
   compared against `ask_user`, so the two halves of the same rule disagreed about which
   spelling the tool has. Reading the file rather than trusting the task text caught it.
4. **The container test was passing for the wrong reason.** Its routed-tool table drove
   `"write"`, `"glob"`, `"grep"`, `"append_file"` and `"mkdir"`. `container_script`
   returned `Ok(None)` for every one of them, so those cases took the refuse path and
   asserted a provisioning failure the route never actually reached - the test looked
   green while proving nothing about the translation. Migrating the arms turned them into
   real routes, so the table had to migrate in the same change or it would have started
   failing; it now proves the script branches build.
5. **The edit log's `tool` field is a tool name; its filename is not.** Fourteen
   `log_edit_operation` / `EntryExtras` call sites in `crates/ragent-tools-core/src/edit.rs`
   passed `"edit"` as the recorded name, and two test files asserted it. The label in the
   filename (`edits-<timestamp>.jsonl`) is not a tool name and stays; the field is.
6. **A doc comment is not a dispatch site, but a wrong one still misleads.** The scan
   reported ~30 `///`/`//!` lines naming a retired spelling. None of them changes
   behaviour, so none is strictly FR-027's subject; the ones that name a *tool* were
   migrated anyway (a doc that tells the next reader the wrong name is a doc that costs
   a debugging session), and the ones that name a shell program, a language id, a config
   section, a permission type or a stage id were left alone. `check-tool-rename.sh`
   ignores comments wholesale, so this split is a judgement call recorded here, not
   something the guard enforces.
7. **`permission_category()` is not a tool name.** `AskUserQuestion` reports category
   `ask_user`, which reads like a retired tool name but is FR-028's state-driven value and
   must not change. The same applies to the `Permission` enum's display strings.
