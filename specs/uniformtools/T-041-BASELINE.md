# T-041 - Pre-rename surface baseline (FR-030)

**Spec ID:** uniformtools
**Task:** T-041
**Requirement:** FR-030 (the tool surface is unchanged in size and every
enumeration agrees)
**Status:** completed
**Dependencies:** none (root task)
**Recorded against:** the working tree at commit `d6abcd4a`
(`spec/uniformtools: Claude-compatible tool renames and wire profiles`) on top of
`d172b658` (`Version: 1.0.149`), `ragent 1.0.149`, debug binary
`sha256 231360cab75f63210040d52b7a93edc25a32df04194252bb6038c77ba613ccf3`, built
with `rustc 1.99.1 (58cf8f9c3 2026-10-13)` on 2026-10-25. That tree is the
**pre-rename** tree: no tool is renamed in it, because T-031 is still pending.
This is the state T-031 starts from.

This document is the number T-037 compares against. It is written **before**
T-031 renames a single tool, so the comparison is against a record rather than a
recollection (TC-027 of `TESTPLAN.md`, step 5 of its expected results).

Everything below was measured on the tree as it stands, not estimated and not
copied from an earlier draft. The `--dry-run`, `/tools list` and `/bug-report`
figures were each produced by running the code; the registry figures were
produced by throwaway examples that called the same public API those surfaces
call. No Rust source file, test, dependency or tool changed to produce them: all
nine scratch examples were deleted after the run, and this task leaves the
working tree as it found it.

**This revision re-measures a first draft that did not reproduce.** Six figures
in it were wrong: the `/tools list` header, the `/bug-report` line, the
`agent`/`team` family split, which build reports which split, the unconditional
row count, and which registry call `--dry-run` counts. Section 8 lists every
correction with the reason. The names, the digests, the `--dry-run` total of 148
and the family *count* (21) in the first draft did reproduce.

## 1. Headline numbers

| Surface | Value | What it reports |
|---|---|---|
| `ragent --dry-run` (`tools` section, `total`) | **148** | `ToolRegistry::list().len()` - the registered key set |
| `ragent run --dry-run --json <prompt>` (`tools.total`) | **148** | the same call, JSON encoded |
| `/tools list` (TUI) header | **`Visible Tools (148 total, 0 disabled):`** | `definitions().len()`, `hidden_definitions().len()` |
| `/tools list` data rows | **148** | one row per name, visible block plus disabled block |
| `/bug-report` line | **no tool count** - it writes `- **Tools visible**: 1` | the count of *enabled visibility switches*, not tools |
| `tool_info` `total` field | **148** | `all_tools().len()` |
| `ToolRegistry::list()` | **148** | the sorted registry key set |
| `ToolRegistry::all_tools()` | **148** | every registered tool object |
| `ToolRegistry::definitions()` | **148** with nothing hidden | the visible subset |
| `commands_info` `total` | **85** | slash-command catalogue entries, not tools |

**The FR-030 number is 148.** After the rename it must still be 148. The rename
changes 13 names and adds, removes, merges and splits nothing.

Every *registry* enumeration agrees today, and they agree because they read the
same registry: `list()`, `definitions()` (with nothing hidden), `all_tools()` are
148 each, the name lists from all three are element-for-element identical, and
all three are sorted. That equality is what T-037 has to keep true; this task
only records that it holds before the rename.

The two **render** surfaces do not agree with the registry today, and that is a
finding, not a measurement error:

- **`/bug-report` does not report a tool count at all.** FR-030 names "the
  `Tools registered` line in `/bug-report`"; there is no such line in this tree.
  The line that looks like one, `- **Tools visible**: 1`, counts enabled
  tool-family visibility switches (Section 4). A reader who took it for a tool
  count would record 1. T-037 cannot assert 148 against this surface without
  first making the surface report 148.
- **`--dry-run` calls its number "148 visible tools"** but derives it from
  `list()`, which ignores the hidden set (Section 3.3). With the shipped default
  configuration 56 tools are hidden and the same line still prints 148.

Neither defect is a rename problem and neither is fixed here: T-041 records the
surface as it is. They are listed in Section 6 as the two things T-037 must
decide about rather than silently pass.

## 2. Per-family counts (`--dry-run`)

The family is the leading segment of `permission_category()`, split on `:`, and
`--dry-run` prints the families sorted alphabetically. **21 families, summing to
148.** These counts must not move: FR-028 keeps `permission_category()` values
unchanged, so a family can only keep its count if the rename adds no tool.

| Family | Tools before | Tools after | Renamed within it |
|---|---|---|---|
| `agent` | 4 | 4 | - |
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
| `team` | 19 | 19 | - |
| `web` | 5 | 5 | - |
| **Total** | **148** | **148** | **13** |

The "Tools after" column is the expectation, not a measurement: no tool has been
renamed yet. It is derivable from the table alone (13 of the 148 names change,
none moves family), which is exactly why T-037 can assert it.

The family membership, measured - one line per family, grouped by
`permission_category()`, the category spelled out so a reader can check the
grouping rather than take it on trust:

```text
agent (4)      agent:spawn: cancel_agent new_agent team_wait | agent:control: wait_agents
ask_user (1)   ask_user: ask_user
bash (3)       bash:execute: bash bash_reset bg
codeindex (10) codeindex:read: codeindex_communities codeindex_dependencies codeindex_explain
               codeindex_godnodes codeindex_path codeindex_references codeindex_search
               codeindex_status codeindex_symbols | codeindex:write: codeindex_reindex
cron (5)       cron:write: cron_add cron_disable cron_enable cron_remove | cron:read: cron_list
file (27)      file:write: append_to_file apply_patch copy_file create edit make_directory
               memory_forget memory_store move_file multi_edit multiedit patch pdf_write rm
               update_file write | file:read: conversation_search diff_files file_info get_env glob
               grep list memory_recall pdf_read read session_search
git (18)       git:write: git_add git_checkout git_cherry_pick git_clone git_commit git_fetch
               git_merge git_pull git_push git_remote git_reset git_stash git_tag | git:read:
               git_branch git_diff git_log git_show git_status
github (11)    github:write: github_close_issue github_comment_issue github_create_issue
               github_create_pr github_merge_pr github_review_pr | github:read: github_get_actions
               github_get_issue github_get_pr github_list_issues github_list_prs
gitlab (19)    gitlab:write: gitlab_approve_mr gitlab_cancel_job gitlab_cancel_pipeline
               gitlab_close_issue gitlab_comment_issue gitlab_create_issue gitlab_create_mr
               gitlab_merge_mr gitlab_retry_job gitlab_retry_pipeline | gitlab:read:
               gitlab_get_issue gitlab_get_job gitlab_get_job_log gitlab_get_mr gitlab_get_pipeline
               gitlab_list_issues gitlab_list_jobs gitlab_list_mrs gitlab_list_pipelines
model (1)      model:read: model_info
network (1)    network:fetch: http_request
none (8)       none: agent_complete calculator commands_info list_agents os_info ragent_info think
               tool_info
plan (2)       plan: plan_enter plan_exit
shell (1)      shell:execute: open
skill (1)      skill:manage: skill_manage
spec (5)       spec:read: spec_coverage spec_list spec_read spec_search | spec:write:
               spec_task_update
storage (1)    storage:write: initiative
system (2)     system: mf_cache_clear mf_version
task (4)       task: task_create task_get task_list task_update
team (19)      team:manage: team_approve_plan team_assign_task team_cleanup team_create
               team_shutdown_teammate team_spawn team_task_create | team:communicate:
               team_broadcast team_idle team_memory_read team_memory_write team_message
               team_read_messages team_shutdown_ack team_submit_plan | team:read: team_status
               team_task_list | team:tasks: team_task_claim team_task_complete
web (5)        web: mf_crawl mf_fetch mf_search webfetch websearch
```

Two families change shape under the rename in a way worth naming, because both
are one-line traps for a reviewer reading a stale list:

- `ask_user` is both a family name and a tool name, and the tool is renamed while
  the family is not. After the rename the family `ask_user` holds a tool called
  `AskUserQuestion`. That is correct (the category is permission vocabulary, not
  the tool name) and it is what FR-028 requires - do not "fix" it.
- `bash` likewise keeps its family name while the tool inside it becomes `Bash`,
  and the family's other two members (`bash_reset`, `bg`) keep their spellings
  because FR-025 does not rename either.

### 2.1 The family is not the name prefix, and two tools prove it

The family comes from `permission_category()`, never from the tool name. **50 of
the 148 names disagree with their family at the first `_` segment**, so a canary
read off the name list is a guess. The two entries that matter most:

- **`team_wait` is in `agent`, not `team`.** Its category is `agent:spawn`, so
  `agent` holds 4 (three `agent:*` tools plus `team_wait`) and `team` holds 19
  (all `team:*` tools). A reader who assumes the 20 `team_*`-named tools form the
  `team` family gets 20 and 3 - both wrong, and wrong by exactly one at each
  other's expense.
- **`agent_complete`, `list_agents` and `tool_info` are in `none`**, and
  `mf_fetch`/`mf_crawl`/`mf_search` are in `web` while `mf_version` and
  `mf_cache_clear` are in `system`.

So the canary pair is **`agent` 4 and `team` 19**, and it is sharp for the reason
a canary should be: a change that re-categorises one tool moves both numbers at
once. The first draft of this record stated `agent` 3 / `team` 20, which is the
split a name-prefix reading produces rather than the one the registry produces.

`--dry-run` prints the whole table, so T-037 compares all 21 rows, not just the
pair; the pair exists to fail with a readable message.

## 3. `/tools list` names before the rename (the 148, ascending)

This is the list TC-027 step 2 copies out, and the list T-037 diffs. It is
ascending, which is the order `list()` and `definitions()` both return and the
order the `/tools` table renders.

```text
agent_complete append_to_file apply_patch ask_user bash bash_reset bg
calculator cancel_agent codeindex_communities codeindex_dependencies
codeindex_explain codeindex_godnodes codeindex_path codeindex_references
codeindex_reindex codeindex_search codeindex_status codeindex_symbols
commands_info conversation_search copy_file create cron_add cron_disable
cron_enable cron_list cron_remove diff_files edit file_info get_env git_add
git_branch git_checkout git_cherry_pick git_clone git_commit git_diff
git_fetch git_log git_merge git_pull git_push git_remote git_reset git_show
git_stash git_status git_tag github_close_issue github_comment_issue
github_create_issue github_create_pr github_get_actions github_get_issue
github_get_pr github_list_issues github_list_prs github_merge_pr
github_review_pr gitlab_approve_mr gitlab_cancel_job gitlab_cancel_pipeline
gitlab_close_issue gitlab_comment_issue gitlab_create_issue gitlab_create_mr
gitlab_get_issue gitlab_get_job gitlab_get_job_log gitlab_get_mr
gitlab_get_pipeline gitlab_list_issues gitlab_list_jobs gitlab_list_mrs
gitlab_list_pipelines gitlab_merge_mr gitlab_retry_job gitlab_retry_pipeline
glob grep http_request initiative list list_agents make_directory
memory_forget memory_recall memory_store mf_cache_clear mf_crawl mf_fetch
mf_search mf_version model_info move_file multi_edit multiedit new_agent open
os_info patch pdf_read pdf_write plan_enter plan_exit ragent_info read rm
session_search skill_manage spec_coverage spec_list spec_read spec_search
spec_task_update task_create task_get task_list task_update team_approve_plan
team_assign_task team_broadcast team_cleanup team_create team_idle
team_memory_read team_memory_write team_message team_read_messages
team_shutdown_ack team_shutdown_teammate team_spawn team_status
team_submit_plan team_task_claim team_task_complete team_task_create
team_task_list team_wait think tool_info update_file wait_agents webfetch
websearch write
```

The order is `list()`, `definitions()` and `all_tools()` - all three compared
equal element for element on this tree.

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

The pre-rename digest was computed twice by two independent implementations (an
in-process SHA-256 written into the probe, and Python's `hashlib` over the
probe's own output) and the two agree, so the list and the digest are not each
other's only witness.

### 3.2 Properties of the before-list T-037 can assert cheaply

Measured, not assumed:

| Property | Before | After (expected) |
|---|---|---|
| entries | 148 | 148 |
| distinct entries | 148 | 148 |
| element-for-element agreement of `list`, `definitions`, `all_tools` | yes | must hold |
| sorted ascending | yes | must hold |
| symmetric difference with the other list | 26 (13 removed + 13 added) | the same 26 |
| longest name, bytes | 22 (`codeindex_dependencies`) | 22 (unchanged) |
| names containing an ASCII uppercase letter | 0 | 13 |
| names containing whitespace | 0 | 0 |
| names not matching `^[a-z][a-z0-9_]*$` | 0 | 13 gain an uppercase letter |
| names whose first `_` segment is their family | 98 of 148 (50 differ) | unchanged |

The uppercase count is the sharpest single check: before the rename no
registered tool name carries an uppercase letter, and after it exactly the 13
adopted Claude spellings do. A rename that missed a tool leaves the count at 12;
one that renamed an extra tool takes it to 14.

### 3.3 What `/tools list` actually renders

Two numbers, not one, and the pair is the point:

```text
Visible Tools (148 total, 0 disabled):          <- nothing hidden
   ...148 rows...

Visible Tools (92 total, 56 disabled):          <- the shipped default config
   ...92 rows...
Disabled by visibility (56):
   ...56 rows...
```

The header is `Visible Tools (N total, M disabled):`, where `N` is
`definitions().len()` and `M` is `hidden_definitions().len()`. **N + M = 148 and
the total row count is 148 whichever configuration is in force** - that is the
invariant T-037 can assert without pinning a visibility configuration. The
`(92, 56)` pair above comes from `Config::default()`, whose switches are
`codeindex` on and `masterfetch` on with the other five off, hiding 56 tools.

`/tools list` is therefore *not* a fixed 148-row surface; it is a fixed 148-name
surface laid out in one or two blocks depending on configuration. A T-037 check
that counts rows only inside the visible block will read 92 on a default-config
machine and call it a regression.

## 4. Surface facts worth pinning beside the count

- **`mf_screenshot` is absent from the 148, by design.** It registers only when
  `headless-browser` is enabled *and* an engine binary resolves on `PATH`
  (`crates/ragent-tools-extended/src/lib.rs`, `headless_engine_present()`; spec
  `openhands` FR-029). The default build therefore advertises 148, not 149. If
  T-037 is run on a machine with a drivable engine the number is legitimately
  149; record which build was measured rather than treating the difference as a
  rename defect.
- **148 is the built-in registry, not a live session.** Bridged MCP tools
  (`<server>.<tool>`) and plugin tools (`plugin_<id>_<tool>`) are registered into
  the tool registry at run time - MCP by the TUI's `register_mcp_tools` - so a
  long-running TUI session with bridges live can list more than 148 rows. Both the
  `--dry-run` figure and the `/tools list` figure recorded here are 148 because
  neither measured a bridged registry: `--dry-run` does spawn and report MCP
  servers (its own `mcp` section said so on this machine, listing eight servers)
  but never registers their tools, since that is a session-time step, and the
  `/tools list` figure came from an `App` built on `create_default_registry()`
  with no MCP client, no plugin and no connector. Compare like with like.
- **The `/bug-report` number is a switch count.** `handle_bug_report` writes
  `- **Tools visible**: {n}` where `n` is
  `tool_visibility.iter_switches().filter(|(_, on)| *on).count()` - the number of
  *enabled family switches* out of seven, which is 1 on the shipped default
  configuration. It is not a tool count and there is no `Tools registered` line
  anywhere in the tree. FR-030 asks for one; this task records that it is absent
  rather than inventing a number for it. (T-037 has since added the line; see
  §6 point 8.)
- **The `--dry-run` total ignores visibility.** `dry_run::tools_section` calls
  `set_hidden(hidden_tools)` and then `list()`, and `list()` returns every
  registered key regardless of the hidden set. So the line labelled
  "148 visible tools" is the registered set, not the visible subset
  (`definitions().len()` would be 92 under the default configuration). The label
  and the number disagree; the number is the one FR-030 means.
- **`multiedit` is a real registered name.** It is the deprecated alias wrapper
  (`crates/ragent-agent/src/tool/mod.rs`), not a stale literal, and it is one of
  the 148. It is not in FR-023 and keeps its spelling. `update_file` is likewise
  a registered alias of `write` and keeps its spelling.
- **`commands_info` is not a tool count.** It returns the slash-command
  catalogue: **85 built-in entries** on this tree, plus any plugin-contributed
  ones (0 here). Its own catalogue entry for `/tools` declares the subcommands
  `["list", "show", "github", "gitlab", "teams", "agents", "plan", "codeindex", "masterfetch", "help"]`,
  and no catalogued trigger is a retired tool spelling. FR-030 lists
  `commands_info` among the enumerations that must stay consistent with the
  registry, and consistency for this tool means the catalogue it prints contains
  no retired tool name - not that its count equals 148.
- **32 distinct `permission_category()` values** across the 148, collapsing to the
  21 families above. The rename changes neither number (FR-028).
- **The outbound provider tool list is the same `definitions()` call.**
  `SessionProcessor::get_cached_tool_definitions` returns
  `tool_registry.definitions()` (cached), so the list ragent sends to a provider
  is by construction the same names the CLI, the TUI and `tool_info` report. That
  is why FR-030 can require the outbound list to agree with the registry: it is
  not a parallel copy that could drift, and the rename must not introduce one.
  Note the same visibility caveat as above: the outbound list is the *visible*
  set, so it is 92 under the default configuration and 148 with nothing hidden.

## 5. How each number was produced

Reproducible from the recorded commit alone; nothing here needs the workbook.

| Number | Command |
|---|---|
| `--dry-run` total and per-family counts | `ragent --dry-run`, read the `tools:` section; or `ragent run --dry-run --json <prompt>`, read that section's `total` and one item per family |
| `/tools list` header and rows | TUI: `/tools list`; headless: render the message from an `App` built on `create_default_registry()` |
| `/bug-report` total | TUI: `/bug-report`, then read the written report's `## Tool Configuration` section - which reports no tool count (Section 4) |
| registry counts and name list | a scratch example calling `create_default_registry()`, then `list()`, `definitions()`, `all_tools()` |
| family counts independently | the same example grouped by `permission_category().split(':').next()` |
| `tool_info` total | the same example, executing `tool_info` against that registry |
| `commands_info` total | the same example, executing `commands_info` |

The registry-side figures and the surface-side figures were produced by
different code paths - one through `ragent`'s CLI/TUI rendering, one through the
library API - and agree on every name, so the baseline is not an artefact of a
single renderer.

## 6. What T-037 compares against this record

1. **Total: 148** from `--dry-run` (both encodings), `tool_info` and the
   registry's three enumerations. Any other figure is a regression.
2. **Family counts: the 21 rows of section 2, unchanged.** Check `agent` 4 and
   `team` 19 specifically, and remember that `team_wait` is in `agent`
   (Section 2.1).
3. **Names: 26 lines differ from section 3**, being exactly the 13 `before`
   spellings removed and the 13 `after` spellings added, each pair per FR-023,
   and nothing else. Sorted diff, not a character diff: 13 `-` lines and 13 `+`
   lines. Give both files a final newline before diffing, or the last entry of
   the file that lacks one is reported as a spurious 14th change.
4. **Uppercase count: 13**, up from 0.
5. **Distinct count: 148**, so the rename created no duplicate registration.
6. **Digest: `308d92d9...`** over the sorted names joined by `\n` without a
   trailing newline (or `f43ae3c8...` for the trailing-newline form).
7. **`/tools list` row count: `N + M = 148`.** Do not assert 148 rows in the
   visible block; under the default configuration that block holds 92.
8. **`/bug-report`: decide, do not assume.** The `Tools registered` line FR-030
   names does not exist on this tree (Section 4). Either T-037 asserts the
   surface *as it will be after* T-031's change adds the line, or it records the
   absence. Asserting 148 against `- **Tools visible**: 1` would fail on a
   correct tree. *Decided by T-037*: the line was added to `handle_bug_report`
   (`- **Tools registered**: {registry.list().len()}`, the first line of
   `## Tool Configuration`) and asserted at 148, with the switch count kept beside
   it as `Tools visible`.

Points 3 and 4 together are what "the surface is unchanged apart from the names"
means in a form a machine can check; points 1, 2 and 5 are the FR-030 count.

## 7. This record is evidence, not an implementation

No tool was renamed by this task, no Rust source file, test, dependency or guard
changed, and no crate was rebuilt beyond the ordinary `cargo run` the
measurement needed. The files this task adds or edits are all documents:

- this document, which holds the measurement;
- the `CHANGELOG.md` Unreleased entry, which states the headline numbers so a
  reader of the change sees the number T-037 compares against without opening the
  spec directory (TC-027, step 5 expected result);
- `PLAN.md`'s T-041 row, marked completed with a one-line pointer to this file;
- `TESTPLAN.md` TC-024 and TC-027, which now name 148 and this document as the
  record they compare against rather than describing a future measurement, and
  which carry the corrected `agent` 4 / `team` 19 canary.

Note for T-034: this document, like `T-029-DERIVATION.md`, `PLAN.md`,
`TESTPLAN.md` and `SPEC.md`, contains the 13 `before` spellings as backticked
literals. The FR-027 guard is scoped to non-test **source** files, where the only
structural exemption is the FR-023 table (`crates/ragent-types/src/tool_rename.rs`)
beside the `CHANGELOG.md` entry; the specification and design documents already
carry these spellings today and are not source. If a later guard revision widens
its scope to markdown, this file is a documentation exemption of the same kind as
the derivation document, not a licence to keep a retired name in code.

## 8. Corrections to the first draft

The first draft of this record was written without a full re-measurement. This
revision measured the tree and corrected four sections. Every figure below was
produced by the probe described in Section 5, and each correction is stated with
the reason so a reader can re-derive it rather than trusting the prose.

| First draft | Measured | Why the draft was wrong |
|---|---|---|
| `/tools list` header: `Registered Tools (148 total):` | `Visible Tools (148 total, 0 disabled):` | There is no `Registered Tools` string anywhere in the tree. The header is built by `render_tool_visibility_table` from `definitions().len()` and `hidden_definitions().len()`. |
| `/bug-report`: `- **Tools registered**: 148` | `- **Tools visible**: 1`, and no `Tools registered` line at all | The count is `iter_switches().filter(enabled).count()` - enabled visibility *switches*, of which one is on by default. FR-030 names a line the tree does not have. |
| `agent` 3 / `team` 20 | `agent` 4 / `team` 19 | `team_wait`'s category is `agent:spawn`, so it counts in `agent`. 3/20 is what a reading based on the name prefix produces. |
| Section 2.1, "the stale-binary trap": `target/release/ragent` built from 1.0.148 reports 148 with `agent` 4 / `team` 19 | No `target/release/ragent` exists in this tree; the debug binary built from this tree reports 148 with `agent` 4 / `team` 19 | The two rows of that table were transposed. The real, checkable trap is the family-versus-name-prefix trap in Section 2.1, which needs no second binary. |
| `/tools list` rows: 148 | 148 across the visible and disabled blocks, but 92 in the visible block under the default configuration | The draft measured with nothing hidden and stated the figure as unconditional. |
| `--dry-run` total reported as `definitions().len()` | `list().len()` | `tools_section` calls `list()`, which ignores the hidden set. The label says "visible"; the number is the registered set. |

The names, the two digests, the `--dry-run` total of 148, the family count of
21, the 32 categories, the absence of `mf_screenshot`, the `multiedit` and
`update_file` notes and the `commands_info` count of 85 all reproduced exactly
and are unchanged.

### 8.1 The documents are corrected; the three guard constants are not

The wrong pair also appears outside this document. The **records and the plan
row** have been corrected in place, because a document describing a pending task
must state the number that task will assert:

- `specs/uniformtools/T-037-SURFACE-AGREEMENT.md` - the baseline table row and
  the paragraph that narrates the canary.
- `specs/uniformtools/T-046-ATOMIC-LANDING.md` - the case-5 row, the "why the
  canary matters" paragraph and the landing checklist step 3.
- `specs/uniformtools/PLAN.md` - the T-037 and T-046 task rows.

The **three test constants** are *not* corrected here, and deliberately so:

| File | Where |
|---|---|
| `crates/ragent-agent/tests/test_fr030_surface_agreement.rs` | `FR030_CANARY_FAMILIES` (line 78) and the module doc (line 25) |
| `crates/ragent-agent/tests/test_tool_rename_atomic_landing.rs` | `FR030_CANARY_FAMILIES` (line 73) and the doc (line 71) |
| `crates/ragent-agent/tests/test_fr028_state_survival.rs` | a comment (line 39) |

They will fail on a correct tree as written, and the pair must become
`("agent", 4), ("team", 19)` when T-037 and T-046 land. Nothing is red today:
all three files belong to those two pending tasks and none is registered in
`crates/ragent-agent/tests/all.rs`. Correcting them is not this task's to do -
T-041 measures and records, T-037 and T-046 assert - and editing them here would
have made this record's own claim ("no test changed") false, which is the reason
they are named instead. A reader who finds a failing guard on a correct tree
should read this section first: the failure is a stale expectation in the guard,
not a miscounted surface.

**Both corrections have since landed**, as this section predicted: T-046
corrected `test_tool_rename_atomic_landing.rs` and T-037 corrected
`test_fr030_surface_agreement.rs` and the `test_fr028_state_survival.rs` comment.
The corrected pair is now asserted, not merely documented - with the stale one
restored, `test_fr030_surface_agreement.rs` case 3 fails with "the `agent` family
holds Some(4) tools, not the 3 the baseline records".

The same pair explains how the first draft went wrong: the stale-release-binary
story it told ("1.0.148 reports `agent` 4 / `team` 19 while the landed tree
reports `agent` 3 / `team` 20") had the two builds' splits swapped. This tree -
the pre-rename one both guards compare against - splits `agent` 4 / `team` 19,
and it has no second binary to differ from.
