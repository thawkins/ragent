# T-039 - Documentation names the new spelling and records the old one (NFR-007)

**Spec ID:** uniformtools
**Task:** T-039
**Requirement:** NFR-007 (documentation uses the `after` spelling after the
change, and `CHANGELOG.md` carries a `Renamed` table listing every `before` ->
`after` pair)
**Status:** completed
**Depends on:** T-031 (the source rename) and T-033 (the non-test literal
migration); T-030 supplies the FR-023 table the migration reads.

## What the requirement asks

NFR-007 has two halves, and only one of them is a find-and-replace:

> Every document that names a renamed tool (the crate `DOCS.md` files,
> `README.md`, `SPEC.md`, `QUICKSTART.md`, `TUI-QUICKSTART.md`, the howto pages
> under `docs/`) **shall** use the `after` spelling after the change, and
> `CHANGELOG.md` **shall** carry a `Renamed` table listing every `before` ->
> `after` pair so a reader upgrading an existing config can find the mapping.

The first half is a migration. The second half is a **deliverable that did not
exist**: before this task the `Unreleased` entry carried a table inside the
T-031 bullet, but there was no `Renamed` section, and the table sat after the
narrative that discussed it rather than being findable. A reader upgrading a
`ragent.json` needs the mapping as its own section, not buried 690 lines into a
release note.

A document is not a dispatch site, so nothing fails to compile when it is
wrong - which is exactly why the requirement exists. A howto that tells a user
to call `bash` produces a user whose first hook never fires.

## What the state was when this task started

Measured on the working tree, using one whole-token regex over the NFR-007
document scope (the 4 root documents, the 15 crate `DOCS.md` files and the 129
pages under `docs/howtos/`):

| Scope | Files | Lines naming a retired spelling |
|---|---|---|
| before (HEAD) | 148 | 643 in 59 files |
| after (tree) | 148 | 596 in 55 files |

So **74 lines across 25 documents** were removed with a retired spelling and
**100 lines were added carrying its `after` form** (the extra 26 are roster
entries the long-form names filled in and the explanatory notes). The other
**596 lines still carry a token that is *not* a tool name**, which is the part
that needs a written reason rather than a replacement - see the classification
below, whose counts sum to exactly 596.

The scope is deliberately the NFR-007 list plus the two directories the list
implies (`docs/userdocs/`, whose pages describe tools to users, and the crate
`DOCS.md` files). Everything below reproduces from the shell alone:

```bash
BEF='(^|[^A-Za-z0-9_])(ask_user|bash|edit|plan_enter|plan_exit|glob|grep|skill_manage|task_create|task_get|task_list|task_update|write)([^A-Za-z0-9_]|$)'
AFT='(^|[^A-Za-z0-9_])(AskUserQuestion|Bash|Edit|EnterPlanMode|ExitPlanMode|Glob|Grep|Skill|TaskCreate|TaskGet|TaskList|TaskUpdate|Write)([^A-Za-z0-9_]|$)'
DOCS="README.md SPEC.md QUICKSTART.md TUI-QUICKSTART.md $(ls crates/*/DOCS.md) $(find docs/howtos -name '*.md')"

# the two headline counts: lines naming a retired spelling, before and after
for f in $DOCS; do git show HEAD:"$f" | grep -cE "$BEF"; grep -cE "$BEF" "$f"; done
# the migration: removed lines with a retired spelling / added with the adopted one
git diff -U0 -- $DOCS | grep -E '^-' | grep -v '^---' | grep -cE "$BEF"
git diff -U0 -- $DOCS | grep -E '^\+' | grep -v '^+++' | grep -cE "$AFT"
```

```
lines naming a retired spelling   before 643 in 59 files   ->   after 596 in 55 files
removed lines with a retired spelling                        74  (25 documents)
added lines with the adopted spelling                       100
```

Line counts are quoted rather than occurrence counts because an occurrence count
depends on whether the regex consumes the delimiter either side of a token, and
two adjacent tokens on one line (`bash|edit` in a hook matcher) then count as one
or two on that accident. A line is a line under either measure.

`CHANGELOG.md` is counted separately, because its diff is the new `Renamed`
section rather than a migration: 13 lines removed (the table rows the T-031
bullet carried) and 57 added (the section, its prose and the T-039 entry).

## What was migrated

The mechanical half, by document, with the shape of each change:

| Document | What moved |
|---|---|
| `README.md` | the per-family tool rosters: `read, Write, create, Edit, ..., Glob, list, Grep`, `Bash, bash_reset`, `TaskCreate, TaskUpdate, TaskGet, TaskList`, `EnterPlanMode, ExitPlanMode`, `Skill`, `AskUserQuestion`. The File roster also gained the *registered* long forms the document had never caught up with (`move_file`, `copy_file`, `make_directory`, `append_to_file`, `diff_files`) because a roster that says `mkdir` names a tool no build has ever registered |
| `SPEC.md` | the `tools` array in the section 11.2 agent-schema example (`["read", "Edit", "Bash", "codeindex_search"]`), the sandbox routed-tool subset (`append_to_file` / `make_directory`, neither of which the container backend could match before), the `Skill action="..."` JSONC examples in section 19B.2, the architecture diagram's `Tools[File ops, Bash, ...]` node, the reachable-tools sentence, and the `command` field's `(for Bash)` |
| `QUICKSTART.md` | the plugin-namespace paragraph: a plugin's tool `Bash`/`Edit` is namespaced by `plugin_<id>_`; rewritten so it reads as prose rather than a tool list |
| `TUI-QUICKSTART.md` | **no change was needed, and that is a finding rather than an omission**: its 16 remaining tokens are all the shell program (`7-layer bash validation`, a fenced `bash` block), the English verb (`write the YOLO flag`), or the rename's own narrative. It is in scope and was read line by line, and it never names a tool by a retired spelling |
| `crates/ragent-tools-core/DOCS.md` | `create_core_registry`'s registration list, which named `write`, `edit`, `glob`, `grep`, `bash`, `mkdir`, `append`, `copy`, `move`, `diff` - **none of those ten names is what the function registers**: five are retired spellings and five are short forms (`copy`, `move`, `mkdir`, `append`, `diff`) no build has ever registered. Now `Write`, `Edit`, `Glob`, `Grep`, `Bash`, `make_directory`, `append_to_file`, `copy_file`, `move_file`, `diff_files` |
| `docs/howtos/tools/*.md` (7 pages) | every per-tool heading and example invocation: `## Write`, `## Edit`, `## Glob`, `## Grep`, `## Bash`, `## AskUserQuestion`, `## Skill`, `## EnterPlanMode`/`## ExitPlanMode`, `## TaskCreate`/`## TaskUpdate`/`## TaskGet`/`## TaskList`, and the `text` examples under each (`Write path="..."`, `Grep pattern="..."`, `Bash command="..."`, `Skill action="list"`, `TaskCreate subject="..."`) |
| `docs/howtos/bash.md` | "the bash tool" -> "the Bash tool" in the section intro, the config paragraph and the version footer |
| `docs/howtos/codeindex.md` | "instead of grep" in the quoted system instruction and the section 7 heading; both now say `Grep`, which is what `prompt_builders.rs` actually emits |
| `docs/howtos/hooks.md` | the two PreToolUse examples compared `"$TOOL" = "bash"` / `"write"`; a hook comparing the retired spelling never matches, so both now compare `Bash` / `Write`, and the two explanatory comments say "Bash commands" / "a `Write` call" |
| `docs/howtos/permissions.md` | the hardwired auto-approve snippet was **stale code, not just a stale name**: it showed a `task_` prefix test and `tool_name == "ask_user"`, neither of which the tree has held since T-033. Replaced with the real function (`TASK_TOOLS` list, `AskUserQuestion`, and the three `*_info` tools it also omits), the table row for task tools, and the decision-flow box's `ask_user` |
| `docs/howtos/config.md` | the permission-type table mixed tool names into permission types (`Edit` / `Write` / `bash` / `EnterPlanMode` / `ExitPlanMode` in the *Name* column). Those are permission types and FR-028 keeps them, so the column now spells the permission spelling (`edit`, `write`, `bash`, `plan_enter`, `plan_exit`) with a note naming the tool |
| `docs/howtos/sandbox-backends.md` | the worked example's prompt `List the files ... with \`bash: ls -la\`` |
| `docs/howtos/slashcommands/{bash,editlog,task,yolo}.md` | "the bash tool's 7-layer model", the edit-log's list of tools it records, `task_create`/`task_update`/`task_get` in the `/task` transcripts |
| `docs/userdocs/introduction.md`, `docs/userdocs/TEAMS.md` | the built-in-tool sentence and the three plan-mode rows that said "no write/bash tools active" |
| `crates/ragent-config/src/config.rs` | `ToolVisibilityConfig::plan`'s doc comment named `plan_enter, plan_exit` |
| `crates/ragent-agent/src/skill/mod.rs` | a doc comment naming "the `skill_manage reload` action" |
| `crates/ragent-agent/src/tool/skill_manage.rs` | the not-found hint a *user* reads: "run skill_manage action=reload" -> "run Skill action=reload" |
| `crates/ragent-tui/src/app/session_ops.rs` | the log line "plan_exit called but agent stack is empty" - a user-facing string naming a tool that no longer exists by that name |
| `crates/ragent-tui/src/app/event_handler.rs` | a comment "so plan_exit can restore it" |
| `crates/ragent-tui/tests/test_message_widget_summaries.rs` | a doc comment describing what `plan_exit` emits |
| `crates/ragent-tui/tests/test_session_models_state_deep.rs` | a comment recording that the assertion matches only the prose half of the migrated log line |

The four Rust files above are prompt text or user-facing text, not dispatch
sites: two are comments, one is a message the model reads, one is a log line the
user reads. They belong to this task because NFR-007's subject is *what the
documentation says*, and a system prompt or a log line is documentation to the
reader it is addressed to. `cargo fmt` was run after each.

## The `Renamed` table

`CHANGELOG.md` gained a `### Renamed` section at the end of the `Unreleased`
entry, immediately before `## [1.0.149]`:

- all 13 FR-023 pairs, one row each, `before` -> `after` in that order;
- the statement that nothing is added, removed, merged or split (148 tools
  before and after), so a reader is not left wondering whether the table is a
  complete list of *changes*;
- the sentence that there is no alias behind it - `ToolRegistry::get` answers
  `None` for the `before` column - so a stale `ragent.json`, hook `matcher`,
  `allowed_tools` entry or slash alias stops resolving and is reported;
- the two cases in the table that are **not** migrations, because they are the
  two a reader will most plausibly get wrong:
  - the permission vocabulary is unchanged (`"bash"` / `"bash:execute"` still
    governs `Bash`; `"ask_user"` still governs `AskUserQuestion`;
    `Permission::{Edit,Bash,PlanEnter,PlanExit}` keep their display strings);
  - `read` is absent because the workbook rates Claude's `Read` as `Close`, so
    a Claude-spelled `allowed_tools` entry naming `Read` is reported rather
    than silently narrowed.

The T-031 bullet keeps its narrative but no longer carries a second copy of the
table; it now points at the new section. The guard's `--exemptions` listing
already names `CHANGELOG.md` as a structural exemption "the Renamed table and
its narrative document all thirteen spellings", so the table the guard's own
output promised exists.

The first draft of this section was placed *before* the T-020 bullet (which sits
in the middle of `### Added`), which put a `### Renamed` heading inside the
`Added` list. Moving it to the end of the entry - after `### Changed` - is what
makes it a sibling of `### Added` rather than a sub-heading of it.

## Deliberately not migrated

Every one of these names a string that is not a tool name, and changing it would
be a behavioural change with no relation to the rename. The classification is
mutually exclusive and first-match-wins, so the counts sum to the 596 remaining
lines:

| Lines | Class | Why it is not a tool name |
|---|---|---|
| 119 | a fenced code block or shell command | the fence language (a fenced `bash` block), a shell script (`#!/usr/bin/env bash`), a `bash -n` validation probe, `base64 ... | bash` in the obfuscation rules, `/bin/bash` |
| 84 | a filesystem path or Rust module | `ragent-tools-core::bash`, `crates/ragent-tools-core/src/write.rs`, `ragent-config`'s `bash_lists`, `dir_lists`, `edit_common`, `edit_log`, `log/editlog/` |
| 127 | FR-028 permission vocabulary | `"permission": "bash"`, `file:write`, `bash:execute`, `codeindex:write`, `skill:manage`, `Permission::Edit`, the `ask_user` category. FR-028 keeps every one |
| 17 | the `bash` config section or `/bash` command | `"bash": { "allowlist": ... }`, "the `bash` block", `/bash add allow`, `bash.allowlist`. A config key and a slash command, not a tool |
| 25 | the shell program `bash` | "7-layer bash validation", "bash security", "bash banned commands", "bash-timeout harness" |
| 14 | retirement / rename prose | the `CHANGELOG.md` narrative, "the spelling this release retires", the FR-023 discussion in `permissions.md` |
| 210 | the English verb, a common noun or a config key | "read and write files", "write the YOLO flag", "cannot edit or run shell commands", "a `file:write` rule", "Rayon parallel glob walk", `bash/dirs` lists |

Those seven counts sum to 596, which is the after-tree line count in the table
above. The rules are ordered and first-match-wins, and the 596 lines are short
enough to re-read in one pass - which is what the classification was:

```bash
for f in $DOCS; do grep -nHE "$BEF" "$f"; done | less   # 596 lines, all classes
```

No scratch classifier is offered as a durable artifact, because a regex that
encoded these seven reasons would be a second implementation of the judgement
the table records, and it would rot. What a reviewer re-runs is the census (the
two counts above, from the shell) and the reading of the 596 lines.

Two further classes did not exist in this scope but would in the repository:
`tools.xlsx` column names and the `docs/competitive/` analyses, which describe
*other* agents' tools.

The `docs/howtos/tools/pdf/*.pdf`, `docs/howtos/pdf/*.pdf` and
`docs/howtos/slashcommands/pdf/*.pdf` artifacts are **not** regenerated. They are
generated files (131 of them, all committed by a release step), the howto
markdown is their source, and regenerating 131 PDFs in a source-migration task
would bury a 74-line change under binary churn in the same commit. The
`pdftotext` check below records the state so the release step that regenerates
them knows it is due.

## Verification

- `cargo fmt --all -- --check` - clean.
- `cargo check --workspace --all-targets` - clean, no new warning.
- `cargo clippy -p ragent-config -p ragent-tui --all-targets` - clean in every
  file this task touched (the reported warnings are the pre-existing
  `redundant_pub_crate` / `assert_eq!`-with-`assert!` ones in
  `event_handler.rs:37` and `test_text_selection.rs:279`).
- `cargo test -p ragent-tui --test all` - 3654 passed, 3 failed. The three are
  the pre-existing `Alt+T` -> `ToggleTasksPanel` cases
  (`test_input_more::alt_toggle_keys_map_to_actions`,
  `test_input_queue_menu_binding::test_other_alt_keys_are_unaffected`,
  `test_memory_panel::test_toggle_memory_mutually_excludes_tasks_panel`), which
  no tool name reaches.
- The two TUI cases this task touched directly:
  `test_session_models_state_deep::` 68 passed,
  `test_message_widget_summaries::` 17 passed.
- `cargo test -p ragent-agent --test all` - **1741 passed / 1 ignored / 0
  failed**, including `test_skill_manage::` 12 passed (the not-found hint is
  asserted by none of them, and the wording change keeps the `Skill` spelling
  the tests drive).
- `cargo test -p ragent-config --test all` - 417 passed.
- `cargo test -p ragent-tools-core --test all` - 394 passed.
- `bash scripts/check-tool-rename.sh` - `OK: ... 39 reviewed exemption(s) all
  match`, so the doc migration introduced no new source literal and broke none
  of the 39 recorded exemptions.
- `./target/debug/ragent run --dry-run` - `total: 148 visible tools`, 21
  families, unchanged. `docs/howtos/tools/INDEX.md`'s "148 registered tools"
  and `README.md`'s "148 across 21 categories" still agree with the registry.
- ASCII (NFR-005): every line this task *edited* keeps the non-ASCII budget it
  had. Two edited lines in the doc scope keep a pre-existing em-dash
  (`docs/howtos/codeindex.md`: the quoted **MANDATORY** instruction, and
  `docs/howtos/permissions.md`: the decision-flow box rule), and no line this
  task added introduces a character the line it replaced did not already carry.
  The `Renamed` section is pure ASCII.

### What a reviewer re-runs (TC-025)

| Step | Command | Result |
|---|---|---|
| 1 | `grep -rn "\`bash\`" README.md SPEC.md QUICKSTART.md TUI-QUICKSTART.md docs/howtos crates/*/DOCS.md` | 20 hits, **every one** in one of the four classes above (config section, permission vocabulary, shell program, retirement prose). No hit uses the spelling to name the tool |
| 2 | `grep -rn "\`Bash\`" <same>` | 34 hits, including every place the document tells a caller what to invoke |
| 3 | `sed -n '/^### Renamed/,/^## \[1.0.149\]/p' CHANGELOG.md` | one table, 13 rows, `before` first |
| 4 | repeat 1-2 for `write`/`Write`, `grep`/`Grep` | same shape: `write` survives only as `file:write`, a shell verb, or `write.rs` |
| 5 | `grep -rnP '[^\x00-\x7F]' <docs>` | unchanged from HEAD for every edited line |

## Findings (recorded, not fixed here)

- **`docs/howtos/permissions.md`'s hardwired-rules snippet was wrong, and wrong
  in a way that hid the rename.** It showed `tool_name.starts_with("task_")`
  and `tool_name == "ask_user"`. T-033 had already replaced both in the real
  function, so the snippet had been describing a function the tree no longer
  contained - and it was the one place a reader would look to learn what a
  hardwired tool is. Fixed here because NFR-007 covers it; recorded because the
  drift predates the rename and a documentation test would have caught it.
- **`docs/howtos/tools/INDEX.md` counts tool families that no longer match.**
  It lists 22 categories (18 file tools, 4 shell, 20 teams) while
  `--dry-run` prints 21 families and the registry holds 148 names. This is
  FR-030's subject and T-037's to close; this task only kept the *names* right.
- **The 131 committed howto PDFs are stale.** `pdftotext` on
  `docs/howtos/tools/pdf/shell.md.pdf` still shows `bash` in the tool table. They
  are regenerated by a release step, not by a source change; regenerating them
  here would put 131 binary files in a documentation commit.
- **`docs/howtos/tools/pdf/` is not in `.gitlab-ci.yml`, `ci.yml` or a script.**
  No checked-in command regenerates the set, which is why the release record in
  `CHANGELOG.md` describes it as a manual step. Not this task's to add.

## Files changed

Documentation - exactly the 25 documents in the scope whose diff removes a line
carrying a retired spelling (a further eight documents in the same diff carry
another task's edits and are not this task's):

`README.md`, `SPEC.md`, `QUICKSTART.md`, `crates/ragent-tools-core/DOCS.md`,
`docs/howtos/bash.md`, `docs/howtos/codeindex.md`, `docs/howtos/config.md`,
`docs/howtos/custom-agents.md`, `docs/howtos/hooks.md`,
`docs/howtos/loopprogramming.md`, `docs/howtos/permissions.md`,
`docs/howtos/sandbox-backends.md`, `docs/howtos/slashcommands/bash.md`,
`docs/howtos/slashcommands/editlog.md`, `docs/howtos/slashcommands/task.md`,
`docs/howtos/slashcommands/yolo.md`, `docs/howtos/tools/file-operations.md`,
`docs/howtos/tools/interactive.md`, `docs/howtos/tools/planning.md`,
`docs/howtos/tools/search.md`, `docs/howtos/tools/shell.md`,
`docs/howtos/tools/skills.md`, `docs/howtos/tools/task-management.md`,
`docs/userdocs/TEAMS.md`, `docs/userdocs/introduction.md`.

`CHANGELOG.md` (the `Renamed` section).

Rust (7 files, comments/prompt text/log text only, each followed by
`cargo fmt`): `crates/ragent-config/src/config.rs`,
`crates/ragent-agent/src/skill/mod.rs`,
`crates/ragent-agent/src/tool/skill_manage.rs`,
`crates/ragent-tui/src/app/session_ops.rs`,
`crates/ragent-tui/src/app/event_handler.rs`,
`crates/ragent-tui/tests/test_message_widget_summaries.rs`,
`crates/ragent-tui/tests/test_session_models_state_deep.rs`.

No production behaviour changed: `--dry-run` is 148 tools over 21 families
before and after.
