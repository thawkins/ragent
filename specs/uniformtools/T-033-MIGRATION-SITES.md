# T-033 - Migrate every non-test literal site (FR-027)

Status: **completed**. Requirement: FR-027 (spec `uniformtools`). Dependency: T-031
(the source rename) and T-030 (the FR-023 table).

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

The scan ran before the change (224 hits across 70 files), was filtered to tool-name
sites by reading each hit, and re-ran after (below).

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
| `team/mod.rs` `PLAN_GATED_TOOLS` | `write`, `edit`, `bash` | `Write`, `Edit`, `Bash` |
| `memory/extract.rs` bash / edit branches | `bash`, `edit`, `write` | `Bash`, `Edit`, `Write` |
| `backend/container.rs` routing + `bash` gate | `bash`, `write`, `glob`, `grep`, `append_file`, `mkdir` | `Bash`, `Write`, `Glob`, `Grep`, `append_to_file`, `make_directory` |
| `research_adapter.rs` registry lookups | `Glob`/`Grep` (already adopted) | unchanged, doc text updated |
| `tool/aliases.rs` delegation targets | `write`, `bash` | `Write`, `Bash` (Rust types + doc text) |
| `skill/bundled.rs` `allowed_tools` | `Bash`/`Grep`/`Glob` (already adopted) | bodies updated |

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

### In-container routing table

`container_script` matches the tool name the backend is asked to run. It named
`read`, `create`/`write`, `append_file`, `rm`, `mkdir`, `list`, `glob` and `grep`. The
rename's first pass had already moved `write`, `glob`, `grep` and `bash` (in
`execute_tool`); `append_file` and `mkdir` were still keyed on names the registry no
longer holds, and the two names the registry actually has are `append_to_file` and
`make_directory`. Both arms now use the registered spelling, so every tool the sandbox
claims to serve routes. The module docs and the `MAX_OUTPUT_LINES` doc comment were
updated to match.

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
`canonical_tool_name` still resolves a retired spelling through `Rename::after_of` so a
session resumed from saved history (FR-028 replays stored records verbatim) keeps its
rich rendering.

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
- `pluralize(count, "edit", "edits")`, the `"bash"` memory **tag**, the edit log's own
  stored `edits-*.jsonl` label;
- `"ask_user"` as `AskUserQuestion`'s `permission_category()` (FR-028);
- the code-index scanner's `"shell"` dialect and the `"sh" | "bash"` language aliases;
- model- or remote-supplied names, which the text-tool-call extractor and the SSE
  decoders pass through verbatim.

## Tests

No new test file: FR-027 is a migration, and its proof is the scan plus the existing
suites that assert on the sites it moved. Five existing assertions named an old
spelling as the expected string and were updated to the new one rather than relaxed:

- `crates/ragent-agent/tests/test_skill_manage.rs` - the unknown-action message now
  reads "Unknown Skill action".
- `crates/ragent-agent/tests/test_memory_extraction.rs` - the error-resolution candidate's
  tag is `Bash`.
- `crates/ragent-agent/tests/test_backend_misc_more.rs` - the routed-tool table drives
  `Write`, `Glob`, `Grep`, `append_to_file` and `make_directory` (it previously drove
  `write`, `glob`, `grep`, `append_file` and `mkdir`, none of which `container_script`
  matched, so those cases were silently exercising the refuse path).
- `crates/ragent-specs/tests/test_spec_commands.rs` - two cases asserted the add prompt
  contains "`edit` tool"; the prompt now says "`Edit` tool".

## Verification

- `cargo fmt --check` - clean.
- `cargo clippy --workspace --all-targets` - no new warning (only the pre-existing
  `pub(crate)`-inside-a-private-module noise in `ragent-tools-vcs`, `ragent-research`
  and others).
- `cargo check --workspace --all-targets` - clean.
- `cargo test -p ragent-agent --test all` - 1833 passed, 1 ignored.
- `cargo test -p ragent-tools-core --test all` - 395 passed.
- `cargo test -p ragent-tools-extended --test all` - 1784 passed, 11 ignored.
- `cargo test -p ragent-tui --test all` - 3697 passed.
- `cargo test -p ragent-config --test all` - 415 passed.
- `cargo test -p ragent-specs --test all` - 432 passed.
- `cargo test -p ragent-server --test all` - 192 passed.
- `cargo test -p ragent-research --test all` - 471 passed.
- `cargo test -p ragent-tools-vcs --test all` - 397 passed.
- `cargo test -p ragent-storage --test all` - 346 passed.
- `cargo test -p ragent-llm --test all` - 253 passed, 2 ignored.
- `cargo test -p ragent-codeindex --test all` - 447 passed, 2 ignored.
- `cargo test -p ragent-telemetry --test all` / `--doc` - 231 / 9 passed.
- Eight guard scripts pass: `check-inline-tests`, `check-dead-code-reasons`,
  `check-silent-errors`, `check-security-unwraps`, `check-poison-locks`,
  `check-test-env-isolation`, `check-file-tool-containment`, `check-shared-guards`.

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
5. **`permission_category()` is not a tool name.** `AskUserQuestion` reports category
   `ask_user`, which reads like a retired tool name but is FR-028's state-driven value and
   must not change. The same applies to the `Permission` enum's display strings.
