# T-041 - Pre-rename surface baseline (FR-030)

**Spec ID:** uniformtools
**Task:** T-041
**Requirement:** FR-030 (the tool surface is unchanged in size and every
enumeration agrees)
**Status:** completed
**Dependencies:** none (root task)
**Recorded against:** the working tree at commit `d172b658`
(`Version: 1.0.149`) plus its uncommitted changes, `ragent 1.0.149`, debug
binary `sha256 76db133c6a6905b0d165aa7b2a0cd566e55e45f66ab2981dd7db9f4b27ac0af4`,
built with `rustc 1.99.1 (58cf8f9c3 2026-10-13)` on 2026-10-23. The pre-rename
tree is that tree: no tool has been renamed in it, so this is the state T-031
starts from rather than the state of the last commit alone.

This document is the number T-037 compares against. It is written **before**
T-031 renames a single tool, so the comparison is against a record rather than a
recollection (TC-027 of `TESTPLAN.md`, step 5 of its expected results).

Everything below was measured on the tree as it stands, not estimated. The
`--dry-run`, `/tools list` and `/bug-report` figures were each produced by
running the binary; the registry figures were produced by two throwaway examples
that called the same public API those surfaces call. No Rust source file, test,
dependency or tool changed to produce them: both scratch examples were deleted
after the run, and this task leaves the working tree as it found it.

## 1. Headline numbers

| Surface | Value | What it reports |
|---|---|---|
| `ragent --dry-run` (`tools` section, `total`) | **148 visible tools** | `ToolRegistry::definitions().len()` |
| `ragent --dry-run --json` (`tools` section, `total`) | **148 visible tools** | the same call, JSON encoded |
| `/tools list` (TUI) header | **`Registered Tools (148 total):`** | `ToolRegistry::definitions().len()` |
| `/bug-report` line | **148** in `- **Tools registered**: 148` | `ToolRegistry::definitions().len()` |
| `/tools list` data rows | **148** | one row per definition, no blanks |
| `tool_info` `total` field | **148** | `ToolRegistry::all_tools().len()` |
| `ToolRegistry::list()` | **148** | the sorted registry key set |
| `ToolRegistry::all_tools()` | **148** | every registered tool object |

**The FR-030 number is 148.** After the rename it must still be 148. The rename
changes 13 names and adds, removes, merges and splits nothing.

Every enumeration agrees today, and they agree because they read the same
registry: `list()`, `definitions()`, `all_tools()` and the two derived lengths
are 148 each, and the name lists from `list()` and `definitions()` are
element-for-element identical. That equality is what T-037 has to keep true;
this task only records that it holds before the rename.

## 2. Per-family counts (`--dry-run`)

The family is the leading segment of `permission_category()`, split on `:`, and
`--dry-run` prints the families sorted alphabetically. **21 families, summing to
148.** These counts must not move: FR-028 keeps `permission_category()` values
unchanged, so a family can only keep its count if the rename adds no tool.

| Family | Tools before | Tools after | Renamed within it |
|---|---|---|---|
| `agent` | 3 | 3 | - |
| `ask_user` | 1 | 1 | `ask_user` -> `AskUserQuestion` |
| `bash` | 3 | 3 | `bash` -> `Bash` |
| `codeindex` | 10 | 10 | - |
| `cron` | 5 | 5 | - |
| `file` | 27 | 27 | `edit` -> `Edit`, `glob` -> `Glob`, `grep` -> `Grep`, `write` -> `Write` |
| `git` | 18 | 18 | - |
| `github` | 11 | 11 | - |
| `gitlab` | 19 | 19 | - |
| `model` | 1 | 1 | - |
| `network` | 1 | 1 | - |
| `none` | 8 | 8 | - |
| `plan` | 2 | 2 | `plan_enter` -> `EnterPlanMode`, `plan_exit` -> `ExitPlanMode` |
| `shell` | 1 | 1 | - |
| `skill` | 1 | 1 | `skill_manage` -> `Skill` |
| `spec` | 5 | 5 | - |
| `storage` | 1 | 1 | - |
| `system` | 2 | 2 | - |
| `task` | 4 | 4 | `task_create` -> `TaskCreate`, `task_get` -> `TaskGet`, `task_list` -> `TaskList`, `task_update` -> `TaskUpdate` |
| `team` | 20 | 20 | - |
| `web` | 5 | 5 | - |
| **Total** | **148** | **148** | **13** |

The "Tools after" column is the expectation, not a measurement: no tool has been
renamed yet. It is derivable from the table alone (13 of the 148 names change,
none move family), which is exactly why T-037 can assert it.

Two families change shape under the rename in a way worth naming, because both
are one-line traps for a reviewer reading a stale list:

- `ask_user` is both a family name and a tool name, and the tool is renamed while
  the family is not. After the rename the family `ask_user` holds a tool called
  `AskUserQuestion`. That is correct (the category is permission vocabulary, not
  the tool name) and it is what FR-028 requires - do not "fix" it.
- `bash` likewise keeps its family name while the tool inside it becomes `Bash`,
  and the family's other two members (`bash_reset`, `bg`) keep their spellings
  because FR-025 does not rename either.

### 2.1 The stale-binary trap

The `--dry-run` total is the same 148 on a rebuild and on the stale release
binary already in `target/`, but **the family split is not**:

| Binary | total | `agent` | `team` |
|---|---|---|---|
| `target/debug/ragent`, built from this tree (1.0.149 plus uncommitted changes) | 148 | 3 | 20 |
| `target/release/ragent`, left over from 1.0.148 | 148 | 4 | 19 |

Both report "148 visible tools", so a T-037 run that compares only the total
against the stale release binary would pass while its family table was wrong.
Take the comparison from a binary built at the change's own commit, and check one
family count as a canary. This is the concrete case this task exists to prevent -
"148" alone is not the baseline; "148 with `agent` 3 and `team` 20" is.

## 3. `/tools list` names before the rename (the 148, ascending)

This is the list TC-027 step 2 copies out, and the list T-037 diffs. It is
ascending, which is the order `list()` and `definitions()` both return and the
order the `/tools` table renders.

```
agent_complete append_to_file apply_patch ask_user bash bash_reset bg
calculator cancel_agent codeindex_communities codeindex_dependencies
codeindex_explain codeindex_godnodes codeindex_path codeindex_references
codeindex_reindex codeindex_search codeindex_status codeindex_symbols
commands_info conversation_search copy_file create cron_add cron_disable
cron_enable cron_list cron_remove diff_files edit file_info get_env
git_add git_branch git_checkout git_cherry_pick git_clone git_commit
git_diff git_fetch git_log git_merge git_pull git_push git_remote
git_reset git_show git_stash git_status git_tag github_close_issue
github_comment_issue github_create_issue github_create_pr
github_get_actions github_get_issue github_get_pr github_list_issues
github_list_prs github_merge_pr github_review_pr gitlab_approve_mr
gitlab_cancel_job gitlab_cancel_pipeline gitlab_close_issue
gitlab_comment_issue gitlab_create_issue gitlab_create_mr gitlab_get_issue
gitlab_get_job gitlab_get_job_log gitlab_get_mr gitlab_get_pipeline
gitlab_list_issues gitlab_list_jobs gitlab_list_mrs gitlab_list_pipelines
gitlab_merge_mr gitlab_retry_job gitlab_retry_pipeline glob grep
http_request initiative list list_agents make_directory memory_forget
memory_recall memory_store mf_cache_clear mf_crawl mf_fetch mf_search
mf_version model_info move_file multi_edit multiedit new_agent open
os_info patch pdf_read pdf_write plan_enter plan_exit ragent_info read rm
session_search skill_manage spec_coverage spec_list spec_read spec_search
spec_task_update task_create task_get task_list task_update
team_approve_plan team_assign_task team_broadcast team_cleanup team_create
team_idle team_memory_read team_memory_write team_message
team_read_messages team_shutdown_ack team_shutdown_teammate team_spawn
team_status team_submit_plan team_task_claim team_task_complete
team_task_create team_task_list team_wait think tool_info update_file
wait_agents webfetch websearch write
```

### 3.1 Digests, so a list can be compared without pasting it

The names above, sorted and joined with a single `\n`, with **no** trailing
newline:

| List | sha256 |
|---|---|
| before the rename (measured) | `a833f0f15b5ac3b8e9bb5cb1c0f35aa51ab84e424f7659bddab8dc409c143d1b` |
| after the rename (expected, from the FR-023 pairs) | `308d92d9d975666cf1f815369e18cc7b457c1c0dcbab004c45f2835100368321` |

With one trailing newline the same two lists hash as
`e5c5ff82df582adf3080cdb9b4fedd2895a23a562d623017c46cec9ef2ad8129` and
`f43ae3c88ff20fbff39e847592b7f441014afa35b6e8b99ad2126f59f27ab80c`. Both
variants are recorded because `sha256sum` on a file the shell wrote with
`printf '%s\n'` includes the newline and the in-process digest does not; a
mismatch between them is a newline, not a missing tool. Do not compare a digest
taken one way against one taken the other.

The "after" digest is a **prediction**, derived by applying the 13 FR-023 pairs
to the measured list. It is not evidence that the rename has happened. T-037
recomputes it and compares; TC-024 step 1 compares the totals.

### 3.2 Properties of the before-list T-037 can assert cheaply

Measured, not assumed:

| Property | Before | After (expected) |
|---|---|---|
| entries | 148 | 148 |
| distinct entries | 148 | 148 |
| symmetric difference with the other list | 26 (13 removed + 13 added) | the same 26 |
| longest name, bytes | 22 (`codeindex_dependencies`) | 22 (unchanged) |
| names containing an ASCII uppercase letter | 0 | 13 |
| names containing whitespace | 0 | 0 |
| names not matching `^[a-z][a-z0-9_]*$` | 0 | n/a (13 gain uppercase) |

The uppercase count is the sharpest single check: before the rename no
registered tool name carries an uppercase letter, and after it exactly the 13
adopted Claude spellings do. A rename that missed a tool leaves the count at 12;
one that renamed an extra tool takes it to 14.

## 4. Surface facts worth pinning beside the count

- **`mf_screenshot` is absent from the 148, by design.** It registers only when
  `headless-browser` is enabled *and* an engine binary resolves on `PATH`
  (spec `openhands` FR-029). The default build therefore advertises 148, not 149.
  If T-037 is run on a machine with a drivable engine the number is legitimately
  149; record which build was measured rather than treating the difference as a
  rename defect.
- **148 is the built-in registry, not a live session.** Bridged MCP tools
  (`<server>.<tool>`) and plugin tools (`plugin_<id>_<tool>`) are registered into
  the tool registry at run time - MCP by the TUI's `register_mcp_tools` - so a
  long-running TUI session with bridges live can list more than 148 rows. Both the
  `--dry-run` figure and the `/tools list` figure recorded here are 148 because
  neither measured a bridged registry: `--dry-run` does spawn and report MCP
  servers (its own `mcp` section says so) but never registers their tools, since
  that is a session-time step, and the `/tools list` figure came from an `App`
  built on `create_default_registry()` with no MCP client, no plugin and no
  connector. Compare like with like.
- **`multiedit` is a real registered name.** It is the deprecated alias wrapper
  (`crates/ragent-agent/src/tool/mod.rs`), not a stale literal, and it is one of
  the 148. It is not in FR-023 and keeps its spelling.
- **`commands_info` is not a tool count.** It returns the slash-command catalogue:
  **85 built-in entries** today, plus any plugin-contributed ones. Its `/tools`
  entry declares subcommands `["list", "show", "help"]`. FR-030 lists
  `commands_info` among the enumerations that must stay consistent with the
  registry, and consistency for this tool means the catalogue it prints contains
  no retired tool name - not that its count equals 148.
- **32 distinct `permission_category()` values** across the 148, collapsing to the
  21 families above. The rename changes neither number (FR-028).
- **The outbound provider tool list is the same `definitions()` call.**
  `SessionProcessor::get_cached_tool_definitions` returns
  `tool_registry.definitions()` (cached), so the list ragent sends to a provider
  is by construction the same 148 the CLI, the TUI and `tool_info` report. That is
  why FR-030 can require the outbound list to agree with the registry: it is not
  a parallel copy that could drift, and the rename must not introduce one.

## 5. How each number was produced

Reproducible from the recorded commit alone; nothing here needs the workbook.

| Number | Command |
|---|---|
| `--dry-run` total and per-family counts | `ragent --dry-run`, read the `tools:` section; or `ragent --dry-run --json`, read that section's `total` and each family item |
| `/tools list` header and rows | TUI: `/tools list`; headless: render the message from an `App` built on `create_default_registry()` |
| `/bug-report` total | TUI: `/bug-report`, then read the written report's `## Tool Configuration` section, first line |
| registry counts and name list | a scratch example calling `create_default_registry()`, then `list()`, `definitions()`, `all_tools()` |
| family counts independently | the same example grouped by `permission_category().split(':').next()` |

The registry-side figures and the surface-side figures were produced by two
different code paths - one through `ragent`'s CLI/TUI rendering, one through the
library API - and agree on every name, so the baseline is not an artefact of a
single renderer.

## 6. What T-037 compares against this record

1. **Total: 148.** `--dry-run`, `/tools list`, `/bug-report`, `tool_info` and the
   outbound provider tool list. Any figure other than 148 is a regression.
2. **Family counts: the 21 rows of section 2, unchanged.** Check `agent` 3 and
   `team` 20 specifically, they are the stale-binary canary.
3. **Names: 26 lines differ from section 3**, being exactly the 13 `before`
   spellings removed and the 13 `after` spellings added, each pair per FR-023,
   and nothing else. Sorted diff, not a character diff: 13 `-` lines and 13 `+`
   lines. Give both files a final newline before diffing, or the last entry of
   the file that lacks one is reported as a spurious 14th change.
4. **Uppercase count: 13**, up from 0.
5. **Distinct count: 148**, so the rename created no duplicate registration.
6. **Digest: `308d92d9...`** over the sorted names joined by `\n` without a
   trailing newline (or `f43ae3c8...` for the trailing-newline form).

Points 3 and 4 together are what "the surface is unchanged apart from the names"
means in a form a machine can check; points 1, 2 and 5 are the FR-030 count.

## 7. This record is evidence, not an implementation

No tool was renamed by this task, no Rust source file, test, dependency or guard
changed, and no crate was rebuilt beyond the ordinary `cargo build` the
measurement needed. The files this task adds or edits are all documents:

- this document, which holds the measurement;
- the `CHANGELOG.md` Unreleased entry, which states the headline numbers so a
  reader of the change sees the number T-037 compares against without opening the
  spec directory (TC-027, step 5 expected result);
- `PLAN.md`'s T-041 row, marked completed with a one-line pointer to this file;
- `TESTPLAN.md` TC-024 and TC-027, which now name 148 and this document as the
  record they compare against rather than describing a future measurement.

Note for T-034: this document, like `T-029-DERIVATION.md`, `PLAN.md`,
`TESTPLAN.md` and `SPEC.md`, contains the 13 `before` spellings as backticked
literals. The FR-027 guard is scoped to non-test **source** files, where the only
structural exemption is the FR-023 table (`crates/ragent-agent/src/tool/rename.rs`)
beside the `CHANGELOG.md` entry; the specification and design documents already
carry these spellings today and are not source. If a later guard revision widens
its scope to markdown, this file is a documentation exemption of the same kind as
the derivation document, not a licence to keep a retired name in code.
