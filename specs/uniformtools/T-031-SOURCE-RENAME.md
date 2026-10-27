# T-031 - The source rename (FR-026, FR-030, NFR-002, NFR-003)

**Spec ID:** uniformtools
**Task:** T-031
**Requirements:** FR-026 (the rename is a rename; no alias is registered), FR-030
(the tool surface is unchanged in size and every enumeration agrees), NFR-002
(no new external dependencies), NFR-003 (no unsafe code)
**Status:** completed
**Dependencies:** T-030 (the FR-023 table), T-041 (the pre-rename baseline)

## 1. What the task asks for

> Rename the 13 tools at source: change `Tool::name()` and the registry key for
> `ask_user` (registered via `aliases::AskUserTool` at `tool/mod.rs:1625`),
> `bash`, `edit`, `glob`, `grep`, `plan_enter`, `plan_exit`, `skill_manage`, the
> four `task_*` tools and `write`; add no dependency and no `unsafe` block.

The task names two things per tool - the `Tool::name()` return and the registry
key - because they can move independently. Section 3 shows they cannot here, and
says why the second half of the sentence is nevertheless worth having.

## 2. The thirteen changes

One line each, in `TOOL_RENAMES` order:

| # | Tool type | File | `before` -> `after` |
|---|---|---|---|
| 1 | `aliases::AskUserTool` | `crates/ragent-agent/src/tool/aliases.rs` | `ask_user` -> `AskUserQuestion` |
| 2 | `bash::BashTool` | `crates/ragent-tools-core/src/bash.rs` | `bash` -> `Bash` |
| 3 | `edit::EditTool` | `crates/ragent-tools-core/src/edit.rs` | `edit` -> `Edit` |
| 4 | `plan::PlanEnterTool` | `crates/ragent-agent/src/tool/plan.rs` | `plan_enter` -> `EnterPlanMode` |
| 5 | `plan::PlanExitTool` | `crates/ragent-agent/src/tool/plan.rs` | `plan_exit` -> `ExitPlanMode` |
| 6 | `glob::GlobTool` | `crates/ragent-tools-core/src/glob.rs` | `glob` -> `Glob` |
| 7 | `grep::GrepTool` | `crates/ragent-tools-core/src/grep.rs` | `grep` -> `Grep` |
| 8 | `skill_manage::SkillManageTool` | `crates/ragent-agent/src/tool/skill_manage.rs` | `skill_manage` -> `Skill` |
| 9 | `task::TaskCreateTool` | `crates/ragent-tools-extended/src/task.rs` | `task_create` -> `TaskCreate` |
| 10 | `task::TaskGetTool` | `crates/ragent-tools-extended/src/task.rs` | `task_get` -> `TaskGet` |
| 11 | `task::TaskListTool` | `crates/ragent-tools-extended/src/task.rs` | `task_list` -> `TaskList` |
| 12 | `task::TaskUpdateTool` | `crates/ragent-tools-extended/src/task.rs` | `task_update` -> `TaskUpdate` |
| 13 | `write::WriteTool` | `crates/ragent-tools-core/src/write.rs` | `write` -> `Write` |

Five live in `ragent-tools-core`, four in `ragent-tools-extended`, four in
`ragent-agent`. Every one is the `&'static str` returned by `fn name(&self)` -
`&str` still, borrowed, compared directly (NFR-006). Nothing in this change adds
a lookup, an allocation or a table read to a dispatch path.

`permission_category()` is **unchanged everywhere**, including
`AskUserQuestion`'s, which stays the string `ask_user` (FR-028: a category is
not a tool name). The probe in section 6 confirms 32 distinct families before
and after.

## 3. There is no separate registry key to change

The task says "change `Tool::name()` **and the registry key**". The second half
needs no edit, and that is a fact about the registry worth recording:

```rust
// crates/ragent-tools-core/src/lib.rs:510, crates/ragent-agent/src/tool/mod.rs:1405
pub fn register(&self, tool: Arc<dyn Tool>) {
    tools.insert(tool.name().to_string(), tool);   // <- key == name()
}
```

Every registration path in the tree keys on `tool.name()`. There is no
`register_with(name, tool)`, no `insert("bash", ...)` literal, and the four
registries (`ragent-tools-core`, `ragent-tools-extended`, `ragent-tools-vcs`,
`ragent-agent`) share the pattern, which is why the rename could not produce the
half-renamed state T-046's case 4 detects *from the registration side*.

What the sentence does catch is the adapter. `register_extracted_core_tools`
reads the core registry and republishes each tool through
`ExtractedCoreToolAdapter`, whose `fn name(&self) -> &str { self.inner.name() }`
forwards rather than storing a copy - so the core rename propagates through the
adapter with no second edit. The test that pins it is
`test_tool_rename_atomic_landing::test_every_registration_is_stored_under_the_name_its_tool_reports`.

## 4. What else had to move in the same change

A rename the compiler cannot check breaks every `match` arm keyed on the string.
The migration list is T-033's; this task lands the sites whose absence would make
the tree *wrong* rather than merely stale (a guard that no longer guards, a route
that no longer routes, a summary that falls through to the raw dump). All of it
is in the same change, per FR-034.

### Agent guards (`crates/ragent-agent/src/session/`)

| Site | Before | After |
|---|---|---|
| `permissions.rs` `INTERACTIVE_TOOLS` | `ask_user` | `AskUserQuestion` |
| `permissions.rs` `SKILL_ALWAYS_ALLOWED_TOOLS` | `ask_user` | `AskUserQuestion` |
| `permissions.rs` `is_hardwired_auto_approved_tool` | `ask_user`, `"task_"` **prefix** | `AskUserQuestion`, explicit `TASK_TOOLS` |
| `permissions.rs` `is_destructive_tool` bash arm | `tool_name == "bash"` | `== "Bash"` |
| `loop_state.rs` `LOOP_WRITE_TOOLS` | `write`, `edit` | `Write`, `Edit` |
| `loop_state.rs` `LOOP_ALWAYS_ALLOWED_TOOLS` | `ask_user` | `AskUserQuestion` |
| `loop_state.rs` path checks | `tool_name == "bash"` x2 | `== "Bash"` |
| `history.rs` write-tool list | `write`, `edit` | `Write`, `Edit` |
| `processor.rs` spec-dir write match x2 | `write`, `edit` | `Write`, `Edit` |
| `processor.rs` dispatch | `tc.name == "bash"` | `== "Bash"` |
| `memory/extract.rs` | `bash` x3, `edit`/`write` x2 | `Bash`, `Edit`/`Write` |
| `backend/container.rs` routing arms | `"write"`, `"glob"`, `"grep"` | `"Write"`, `"Glob"`, `"Grep"` |
| `research_adapter.rs` lookups | `get("glob")`, `get("grep")` | `get("Glob")`, `get("Grep")` |

`TASK_TOOLS` replaces a **prefix** test (`tool_name.starts_with("task_")`). The
prefix was doing two jobs and doing both badly: it named a retired spelling, and
it admitted any future tool whose name began with those five characters. An
explicit four-name list is the same policy stated as a set. This is the one site
where "migrate the literal" was not enough and the test had to change shape; the
case in `tests/test_session_text_helpers.rs` now also asserts the retired
spelling is *not* auto-approved, so the prefix cannot come back unnoticed.

### Configuration (`crates/ragent-config/src/config.rs`)

`tool_family_names("plan")` returned `["plan_enter", "plan_exit"]`, so
`/tools plan on` hid and then re-showed two names that no longer existed, and the
TUI's `test_slash_tools_plan_on_shows_plan_tools` caught it - the "off" half
passed vacuously (neither name was visible anyway) and the "on" half failed.

### TUI (`crates/ragent-tui/src/`)

`message_widget.rs` holds 28 arms keyed on a tool name: the input summaries, the
result summaries, `tool_inline_diff`, the two `canonical_tool_name` alias targets
(`update_file` -> `Write`, `run_code` -> `Bash`) and the raw-argument fallback
gate. Before the change every renamed call fell through to
`<none:bash>`-style generic output; after it they render. Also migrated:
`layout.rs`'s `tool != "edit"` gate, `event_handler.rs`'s task-cache
invalidation, and `slash.rs`'s `/task` help + delegate text and the
`skill_manage` mention in the `/graphify` completion message.

### Prompt text (`description()` bodies)

Nine bodies named a *different* tool by its old spelling. These are read by the
model, so a stale one instructs it to make a call that now fails:

- `bash_reset` and `rm` said "`bash`";
- `append_to_file`, `create` and `multi_edit` said "`edit`";
- `list` said "`glob`";
- `codeindex_search` said "`grep`" (plus six sibling `codeindex_*` tools'
  `fallback_tools` metadata and busy messages);
- `edit` said "`write`/`patch`" and "prefer `create`/`write`".

### Test assertions (updated, never relaxed)

38 test files changed. Where a test asserted a tool name, the assertion now
states the new name. The five that were *behaviourally* wrong rather than merely
stale are worth naming because each was passing for the wrong reason:

1. `test_loop_capture.rs` drove `name: "write"`, which after the rename reached
   no tool, so the loop's write never landed and three rollback cases failed on
   the *precondition*. They now drive `Write`.
2. `test_security_analyzer_loop.rs` wrote via `"write"` and asserted the file
   existed - it failed the same way. `"Write"`.
3. `test_backend_misc_more.rs`'s routed-tool table drove `"write"`, `"glob"`,
   `"grep"`, none of which `container_script` matched, so those cases exercised
   the refuse path instead of the route. They now prove the branches build.
4. `test_container_backend.rs`'s (container-gated) round-trip used
   `NamedTool::new("write"/"grep"/"glob")`; the tool name is what the backend
   routes on, so the case could not round-trip.
5. `test_slash_tools_plan_on_shows_plan_tools` failed on the "on" half only,
   because the "off" half asserted the absence of tools that were absent anyway.

### The FR-023 invariant test flipped

`crates/ragent-agent/tests/test_tool_rename_table.rs` asserted, for the
pre-rename tree, that every `before` **is** registered and every `after` **is
not**. T-030's record said T-031 must flip both in the same change, and this task
did:

| Case (before) | Case (after) |
|---|---|
| `..._every_before_is_registered_by_the_default_registry` | `..._every_before_is_retired_by_the_default_registry` |
| `..._every_after_is_unregistered_by_the_default_registry` | `..._every_after_is_registered_by_the_default_registry` |

Both assertions keep their strength: the retired-spelling case still fails if a
`before` resolves, and the adopted-spelling case still fails if an `after` does
not - which is exactly the half-renamed state FR-034 refuses. Neither was
relaxed to `is_ok()` or deleted.

## 5. Deliberately not migrated (T-033's audit, restated)

A quoted literal is not always a tool name, and the remaining 89 non-test hits
are all of this kind. Changing them would be a behavioural change unrelated to
the rename:

- shell program names: `Command::new("bash")`, `which::which("bash")`,
  `OsStr::new("bash")`, `ShellType::Bash => "bash"`, the bash safe-command list,
  the `grep`/`rg` entry in the skill-context command table;
- language ids: `"sh" | "bash" => Language::Shell` (scanner and scaffold);
- the `bash.allowlist`/`bash.denylist` **config section** name
  (`bash_lists.rs`, `patch_config(scope, "bash", ...)`);
- `Permission`'s `From`/`Display` strings (`"edit"`, `"bash"`, `"plan_enter"`,
  `"plan_exit"`) and `permission_category()` values, including
  `AskUserQuestion`'s `"ask_user"` (FR-028);
- the OASF doc list in `agent/oasf.rs`, which documents permission types;
- the `"write"` stage id in `archdoc/govcreate_run.rs` and the `"write"` verb in
  `specs/validate.rs`'s testable-verb table;
- `pluralize(count, "edit", "edits")`, the `"bash"` memory **tag** and the
  `edits-*.jsonl` edit-log label;
- the `/bash` slash-command trigger in `command_catalog.rs` / `state.rs` /
  `slash.rs` (a command, not a tool);
- `"bash"` in `ragent-connectors`'s descriptor and `toolchain.rs`'s
  `LanguageRuntime::many(&["bash"])` (a runtime, not a tool);
- `team/classify.rs`'s keyword table (the verb "write");
- the FR-023 table itself and the guard's own fixtures.

`scripts/check-tool-rename.sh` (T-034's predicate (e)) reports 28 remaining
lines, every one of them in this list. That count is the guard doing its job on
T-033's exemptions rather than a defect in this task; the guard's exemption list
is T-034's to extend, and it is deliberately per-line with a written reason.

## 6. Measurement after the change

Read off a throwaway example that called the same public API `--dry-run` and
`tool_info` call, then deleted:

| Surface | Value |
|---|---|
| `ToolRegistry::list().len()` | **148** (the T-041 baseline, unchanged) |
| `definitions().len()` | 148 |
| `all_tools().len()` | 148 |
| names whose first character is uppercase | **13** - exactly the `after` column |
| distinct `permission_category()` values | 32 (unchanged) |
| `get(before)` for all thirteen | `None` |
| a registry key holding `before` | none |

FR-030's "unchanged in size" holds: 13 names changed, nothing added, removed,
merged or split.

## 7. Red proofs (run, observed, reverted byte-identically)

Each proof was applied, the file `touch`ed (a `cp -p` restore keeps the older
mtime and cargo reuses the broken object file), the filtered suite run, the file
restored from a `cp -p` backup and verified by `md5sum`:

```
2907aeb4add000925f9df16346834f8d  crates/ragent-tools-core/src/write.rs
3c8f950ecb5c9996ae9de467300b8901  crates/ragent-agent/src/session/permissions.rs
```

| # | Break | Failed cases |
|---|---|---|
| 1 | `write.rs`'s `Tool::name()` back to `"write"` | 2 - `..._every_before_is_retired...` ("`before` \"write\" is still a name the default registry registers") and `..._every_after_is_registered...` ("`after` \"Write\" is not a name the default registry registers"). Both halves of the half-renamed state, from one edit |
| 2 | `TASK_TOOLS.contains(&tool_name)` back to `tool_name.starts_with("task_")` | 1 - `test_hardwired_auto_approved_predicate_branches`, `assertion failed: is_hardwired_auto_approved_tool("TaskCreate")` |
| 3 | `research_adapter.rs`'s two lookups back to `"glob"`/`"grep"` | 1 - `local_gatherer_drives_agent_local_tool_methods`, "Glob/Grep/read/list must enable local gathering" |

Proof 1 is the one worth keeping: a single `Tool::name()` edit is caught by two
independent assertions that describe opposite failures, so neither the "kept the
old spelling" nor the "lost the new one" case can pass unnoticed.

## 8. Requirements and how they are met

| Requirement | Evidence |
|---|---|
| FR-026 - the tool is exposed under `after` only | 148 names, 13 uppercase, `get(before)` `None` for all thirteen; no second registration (T-032's guard) |
| FR-030 - the surface is unchanged in size | 148 / 148 / 148; 13 names differ from the pre-rename set and nothing else |
| NFR-002 - no new external dependency | `git diff --stat -- '*Cargo.toml' Cargo.lock` is empty |
| NFR-003 - no unsafe block | the only `unsafe` in a touched crate is the pre-existing `libc::killpg` in `bash.rs:1224`, untouched by this change |
| FR-023 invariants after the rename | `test_tool_rename_table::` 9 passed, including both flipped cases |
| FR-034 one spelling resolves | `test_tool_rename_atomic_landing` and the two flipped FR-023 cases |

## 9. Verification

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | clean |
| `cargo check -p ragent-tools-core -p ragent-tools-extended -p ragent-agent --all-targets` | clean |
| `cargo check --workspace --all-targets --exclude ragent-llm` | clean (the `ragent-llm` exclusion is a pre-existing `cannot find type ChatContent` in `tests/inline/anthropic_tests.rs`, unmodified since `d6abcd4a`) |
| `cargo test -p ragent-agent --test all` | 1679 passed, 1 ignored |
| `cargo test -p ragent-tools-core --test all` | 394 passed |
| `cargo test -p ragent-tools-extended --test all` | 1806 passed, 11 ignored |
| `cargo test -p ragent-config --test all` | 417 passed |
| `cargo test -p ragent-specs --test all` | 432 passed |
| `cargo test -p ragent-server --test all` | 192 passed |
| `cargo test -p ragent-research --test all` | 471 passed |
| `cargo test -p ragent-storage --test all` | 346 passed |
| `cargo test -p ragent-types --test all` | 418 passed |
| `cargo test -p ragent-tools-vcs --test all` | 397 passed |
| `cargo test -p ragent-codeindex --test all` | 447 passed, 2 ignored |
| `cargo test -p ragent-telemetry --test all` | 231 passed |
| `cargo test -p ragent-tools-core --doc` | 5 passed |
| `cargo test -p ragent-llm --doc` | 30 passed |
| `cargo test -p ragent --test all` | 44 passed |
| `cargo test -p ragent-tui --test all` | 3715 passed, 3 failed - the pre-existing Alt+T tasks-panel trio (`test_input_more::alt_toggle_keys_map_to_actions`, `test_input_queue_menu_binding::test_other_alt_keys_are_unaffected`, `test_memory_panel::test_toggle_memory_mutually_excludes_tasks_panel`), reproduced with `event_handler.rs` restored to `HEAD` |
| `cargo clippy -p ragent-tools-core -p ragent-tools-extended -p ragent-agent --all-targets` | no new warning (only the pre-existing `pub(crate)`-in-private-module set) |
| `scripts/check-inline-tests.sh`, `check-dead-code-reasons.sh`, `check-test-env-isolation.sh`, `check-silent-errors.sh`, `check-security-unwraps.sh`, `check-poison-locks.sh`, `check-shared-guards.sh`, `check-file-tool-containment.sh` | all PASS |
| `scripts/check-tool-rename.sh` | FAIL on 28 lines - T-033's exemption list, not this task's sites (section 5) |

`cargo test -p ragent-llm --test all` shows 5 failures. Four are pre-existing and
belong to other tasks: `test_wire_trace::test_agent_loop_sets_the_switch_from_config`
(T-023's `set_enabled` call) and `test_wire_trace::test_every_provider_records_its_body`
(T-023's per-provider recording), plus two source scans in `test_wire_decode.rs` /
`test_wire_encode_message.rs` that name provider files unmodified by this change
(`gemini.rs`'s `fc_` id minting is T-010's). None is caused by the rename: none
of those files is touched here, and the same five fail with the rename reverted.

## 10. Gotchas

1. **A tool name and a permission category look identical in source.** The two
   that cost the most time are `"ask_user"` (the `AskUserQuestion` tool's
   *category*, FR-028 - must not move) and `"write"`/`"edit"` in
   `Permission::from` / `Display` (permission *types*). Read the surrounding
   function before changing a string; a blind substitution breaks the permission
   layer while every tool test still passes.
2. **A test that drove the old name was not testing what its name said.** Three
   loop/rollback cases drove `"write"`, reached no tool, and asserted on a file
   that was never written - and passed, because the assertion they actually hit
   first was a precondition they had also broken. Expect the rename to turn green
   tests red for this reason, and fix the *name*, not the assertion.
3. **`"bash"` is the shell far more often than it is the tool.** `which("bash")`,
   `OsStr::new("bash")`, `ShellType::Bash => "bash"`, the bash validator's
   allowlist, the `bash.allowlist` config section, the `/bash` slash trigger, a
   memory tag, a language id and a runtime name are all *not* tool names. The
   scan in section 5 is the list of what stayed.
4. **`container_script`'s arms are the routing contract.** The backend is asked
   to run a tool *by the name its `Tool::name()` returns*, so an arm keyed on
   `"write"` after the rename matches nothing and the container refuses the call.
   The `docker`/`podman` path has no compile-time link to the arm, so nothing
   catches this but a routing test.
5. **`cargo fmt` before trusting a red proof.** A restore that predates
   `cargo fmt` can leave a file that fails `fmt --check` and, worse, a stale
   object file that makes the *next* run lie.
6. **The FR-023 test is two-sided on purpose.** Flipping only one of the two
   cases leaves a test that passes in the half-renamed state - precisely the tree
   FR-034 forbids - so a future editor must flip both or neither.

## 11. Docs updated

- `specs/uniformtools/PLAN.md` - T-031 marked completed with what landed.
- `specs/uniformtools/T-031-SOURCE-RENAME.md` - this record.
- `CHANGELOG.md` - the `Renamed` table and its migration note (already in
  `[Unreleased]`).
- `specs/uniformtools/TESTPLAN.md` - TC-019 step 8 now states the post-rename
  polarity, so the manual case cannot be run against the wrong expectation.
- `crates/ragent-agent/tests/test_tool_rename_table.rs` - module doc rewritten
  for the post-rename state, so the next reader does not "restore" the old
  polarity.
- Two doctests that asserted the old name were fixed rather than left to fail:
  `crates/ragent-tools-core/src/lib.rs`'s `create_core_registry` example
  (`assert!(registry.contains("Bash"))`) and
  `crates/ragent-agent/src/tool/mod.rs`'s `list()` example
  (`names.contains(&"Bash".to_string())`).

`crates/ragent-tools-core/DOCS.md`'s `create_core_registry` bullet still lists the
lowercase spellings. That is documentation, not source, so it is T-039's scope
and is recorded there rather than fixed here.
