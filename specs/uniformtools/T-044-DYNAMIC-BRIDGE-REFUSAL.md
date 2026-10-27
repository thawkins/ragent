# T-044 - Dynamic registrations stay namespaced, and a stale surfaced name is refused once

Spec: `uniformtools` FR-032. Status: completed.

> If a dynamically sourced tool's surfaced name equals a `before` spelling, the
> system **shall** refuse the bridge with an error naming the source, the spelling
> and its `after` replacement, and **shall** keep the rest of that source working.

## The state this task found

The source file and all three test files existed on disk and were tracked by git,
but none of them were reachable from the build:

- `crates/ragent-types/src/lib.rs` had **no `pub mod tool_bridge;`**, so the whole
  rule file was dead: not compiled, not linted, and invisible to every call site.
- `crates/ragent-types/tests/all.rs`, `crates/ragent-agent/tests/all.rs` and
  `crates/ragent-plugins/tests/all.rs` had **no `#[path]` line** for their
  `test_fr032_*` file, so none of the 16 cases had ever compiled either.
- `McpToolWrapper` had no `ragent_prefix_for`, `bridge_server_tools` did not exist,
  and `register_plugin_tools` did no FR-032 check at all.

This is the fifth instance of the same class this spec has hit (T-012's `tool_args.rs`,
T-036's `allowed_tools.rs`, T-034's and T-040's test registrations). A file that no
`mod` line reaches is a file that has never run, and `git ls-files` reports it as
present. The fixes are one declaration and three registrations, and they are as much
part of this task as the rule itself - the rule is only real once it compiles.

The three call sites described below had also not been migrated: each built its
wrappers (or its collision check) itself and consulted nothing.

## What was added

### `crates/ragent-types/src/tool_bridge.rs` (new, 403 lines)

The rule lives in `ragent-types` because three call sites in two crates ask it and
`ragent-plugins` cannot depend on `ragent-agent` (the dependency runs the other
way). `ragent-types` already owns the FR-023 table, so it is the leaf crate both
already depend on; `ragent-plugins`' `Cargo.toml` gained the direct
`ragent-types` dependency it had previously reached only transitively.

| Item | Purpose |
|---|---|
| `MCP_TOOL_PREFIX`, `PLUGIN_TOOL_PREFIX` | the two namespaces in one place |
| `mcp_tool_prefix` / `mcp_tool_name` | the MCP mangling, folded `-` `.` `/` -> `_` |
| `plugin_tool_prefix` / `plugin_tool_name` | the plugin mangling (id deliberately unfolded - it keys the store entry and the `plugin:<id>` permission category) |
| `mcp_source_label` / `plugin_source_label` | one wording per source kind for a refusal or a report |
| `check_dynamic_bridge(source, prefix, surfaced)` | the two clauses |
| `BridgeRefusalReason`, `BridgeRefusal::sentence` | the refusal and its one sentence |
| `report_bridge_refusals_once` + `reset_bridge_refusals_for_test` | the once-per-process ledger |

`check_dynamic_bridge` answers both clauses, in this order:

1. **a retired spelling** - `TOOL_RENAMES.iter().find(|pair| pair.before == surfaced)`.
   The pair itself is returned, so the sentence carries the table's own
   `&'static str` values and can never name a replacement the table does not hold.
2. **namespacing** - `!surfaced.starts_with(prefix)`. A prefix is a per-source
   parameter, not a constant, so a bridge that arrives later (a skill) applies the
   same two clauses with its own namespace instead of inventing a third policy.

The check is about the **surfaced** registry name, not the advertised one. That is
deliberate: `mcp_shell_bash` is not `bash`, calling it cannot make `bash`
resolvable, and a shell server is exactly the kind of source that would offer a
tool called `bash`. Refusing it would break a legitimate integration to satisfy a
sentence written about bare names. The retired-spelling arm still earns its place
as the backstop FR-032 calls it: a bridge that passed the advertised name through
unchanged surfaces it verbatim, and then the arm fires with the useful sentence
("`bash` was retired; spell it `Bash`") instead of the generic namespacing one.

### `crates/ragent-agent/src/tool/mcp_tool.rs`

- `McpToolWrapper::ragent_name_for` now delegates to `mcp_tool_name`, so the
  wrapper, the TUI's stale-server scan and the FR-032 check cannot disagree.
- `McpToolWrapper::ragent_prefix_for` is the new prefix accessor (the TUI used
  `ragent_name_for(id, "")` to reach it before, which is a name that must never be
  registered).
- `BridgedServerTools` + `bridge_server_tools` - the single MCP bridge: it splits a
  server's advertised tools into wrappers and refusals and reports the refusals
  once. A refused tool is simply absent from `wrappers` while its siblings are
  present, so containment is structural rather than a convention.
- `bridge_server_tools_named` (`#[doc(hidden)]`) takes the mangler as a parameter.
  With the real one a refusal can never fire, so the containment property would be
  untestable; the test-supplied mangler drives the shape of the regression the
  backstop exists for.

### Call sites migrated

| Call site | Before | After |
|---|---|---|
| `SessionProcessor::set_mcp_client` | built wrappers itself | `bridge_server_tools` per connected server |
| TUI `App::register_mcp_tools` | built wrappers itself | `bridge_server_tools`; stale scan uses `ragent_prefix_for` |
| TUI `App::tool_source` | reached the prefix via `ragent_name_for(&server.id, "")` | `ragent_prefix_for`, so the `/tools` source column and the stale scan cannot disagree either |
| `PluginManager::register_plugin_tools` | collision check only | also `check_dynamic_bridge` per contributed name, refusals collected and reported once |

Connector servers need no separate call site: a connector's servers are bridged as
`<connector-id>.<server-id>` and reach the registry through the same MCP client, so
they are the ordinary MCP bridge's tools under a dotted id.

`mcp_tool` is not a registry name in this build: the wrapper registers one tool per
advertised server tool (`mcp_<server>_<tool>`) with `permission_category()` `mcp`,
and `mcp_tool` survives only in prose (`README.md`, `SPEC.md` 19.3,
`docs/howtos/tools/mcp.md`). There is therefore no aggregate `mcp_tool` entry a
dynamic source could seize, and nothing extra to refuse; the per-server names are
where the rule applies. The stale prose is noted here rather than edited, because it
is a separate documentation defect and changing it is not part of FR-032.

The task text names the MCP shape as `<server>.<tool>`; the tree's surfaced name
has been `mcp_<server>_<tool>` since MCP support landed, and a bridge's *server id*
is the dotted part (`<plugin-id>.<server>`, `<connector-id>.<server>`). Both halves
survive: the source is `<server>` on the wire and the surfaced name is
`mcp_<sanitised-server>_<tool>`, which is what TC-029's expectations now assert. The
requirement's point is that the surfaced name is namespaced, and it is. Skill-contributed tools
do not exist in this build - a `SKILL.md` `allowed-tools` list *restricts* the
registry (FR-029) - so `check_dynamic_bridge` takes the prefix as a parameter and
no unused `skill_` constant is declared.

`plugin_tool_name` in `ragent-plugins/src/tool_adapter.rs` now delegates to the
`ragent-types` implementation too, so the name the crate registers and the name
FR-032 approves are one derivation.

## Reported once, never fatal

`report_bridge_refusals_once` keeps a process-wide `Mutex<HashSet<(source,
surfaced)>>`, mirroring `alias_table`'s unresolved targets and `allowed_tools`'
stale entries:

- per **source and name**, so a second offender from the same source still reports
  and a second source offering the same spelling is a different file to fix;
- process-wide, so a reconnect or the TUI's housekeeping pass is silent;
- returns the number reported, so a test (and a caller) can tell "nothing to say"
  from "already said";
- poison-tolerant and infallible: a refusal is a contained loss, and a warning must
  never be the thing that takes a session down.

## Tests

`crates/ragent-types/tests/test_fr032_dynamic_bridge.rs` (9 cases, registered in
`crates/ragent-types/tests/all.rs`):

1. `mcp_tool_names_are_always_namespaced_under_the_server`
2. `plugin_tool_names_are_always_namespaced_under_the_plugin`
3. `the_rule_is_prefix_agnostic_so_a_new_bridge_uses_its_own_namespace`
4. `no_dynamic_name_can_equal_a_bare_registry_name` (exhaustive over all 13 pairs)
5. `a_namespaced_tool_bridges_even_when_its_server_side_name_is_retired`
6. `a_bridge_that_stops_namespacing_is_refused_with_the_replacement_named`
7. `any_other_bare_name_is_refused_for_not_being_namespaced`
8. `source_labels_name_the_kind_and_the_id`
9. `a_refusal_is_reported_once_per_source_and_name`

`crates/ragent-agent/tests/test_fr032_bridge_containment.rs` (5 cases, all
`#[serial]` because the ledger is process-wide and the consolidated `--test all`
binary shares one process):

1. `the_real_mangler_namespaces_every_tool_of_a_server`
2. `a_refused_tool_costs_one_tool_and_the_rest_of_the_server_keeps_working`
3. `a_bridge_that_stopped_namespacing_refuses_every_tool_but_still_returns`
4. `a_reconnect_reports_the_same_refusal_once`
5. `the_wrapper_reports_the_same_prefix_the_bridge_checks`

`crates/ragent-plugins/tests/test_fr032_plugin_bridge.rs` (2 cases, through the real
`PluginManager::register_plugin_tools` path):

1. `a_plugin_tool_named_after_a_retired_spelling_registers_under_its_namespace`
2. `no_plugin_tool_can_be_registered_under_any_retired_spelling`

Total: 16 new cases. `crates/ragent-agent/tests/test_nfr006_rename_cost.rs` gained
two `TABLE_READERS` entries for the new `tool_bridge.rs` reads (module import +
`check_dynamic_bridge`), each with a written reason: the check runs once per
bridged tool at registration, and with a namespaced name the lookup cannot match at
all.

## Red proofs (applied, observed, reverted byte-identically)

| Edit | Observed |
|---|---|
| `if false && !surfaced.starts_with(prefix)` | `any_other_bare_name_is_refused_for_not_being_namespaced` failed (`Bash` bridged as if namespaced) |
| retired-spelling `find` pinned to `"bash"` | `a_bridge_that_stops_namespacing_is_refused_with_the_replacement_named` and `the_rule_is_prefix_agnostic_so_a_new_bridge_uses_its_own_namespace` failed |
| ledger `insert` with no `continue` | `a_refusal_is_reported_once_per_source_and_name` failed (`left: 1 / right: 0` on the reconnect assertion) |
| `plugin_tool_name` returning the bare tool name | `a_plugin_tool_named_after_a_retired_spelling_registers_under_its_namespace` failed (`left: [] / right: ["plugin_codex-shell_bash"]`) |
| `continue` removed after a refusal in `bridge_server_tools_named` | `a_refused_tool_costs_one_tool_and_the_rest_of_the_server_keeps_working` (`left: ["mcp_srv_notes_read", "bash", "mcp_srv_keep_me"]`) and `a_bridge_that_stopped_namespacing_refuses_every_tool_but_still_returns` failed |

Restored from `target/temp/t044bak/`; `md5sum` after restore:
`2c479445db9558e039d365374711b345` (`tool_bridge.rs`),
`6c4d0c81cda8384c9d494ec047fa5640` (`mcp_tool.rs`),
`5a7e09f0ae9d20173dd55f120dcdf362` (`tool_adapter.rs`). Files were `touch`ed after
restore: a `cp -p` leaves an older mtime, and cargo then reuses the broken object
file.

## Verification

- `cargo fmt --check` - clean.
- `cargo check --workspace --all-targets` - clean, no warnings.
- `cargo clippy` on `ragent-types`, `ragent-plugins`, `ragent-agent`, `ragent-tui`
  with `--all-targets` - no findings in the touched files (only the pre-existing
  `pub(crate)`-in-private-module noise elsewhere in the workspace).
- `ragent-types --test all` 427 passed; `--doc` 46 passed / 2 ignored.
- `ragent-plugins --test all` 494 passed.
- `ragent-connectors --test all` 369 passed.
- `ragent-agent --test all` 1724 passed / 1 ignored (103 s); `test_mcp_tool_wrapper::`
  17 passed and `test_nfr006_rename_cost::` 6 passed under it.
- `ragent-tui --test all mcp` 48 passed. The full `ragent-tui --test all` run is
  3648 passed / **3 failed**, and those 3 are pre-existing at HEAD and unrelated to
  this change: the `Alt+T` -> `ToggleTasksPanel` input-mapping trio
  (`test_input_more::alt_toggle_keys_map_to_actions`,
  `test_input_queue_menu_binding::test_other_alt_keys_are_unaffected`,
  `test_memory_panel::test_toggle_memory_mutually_excludes_tasks_panel`). The TUI
  code path this task touched (`register_mcp_tools`) is covered by the 48-case MCP
  selection, which is green.
- `ragent run --dry-run` still reports **148 visible tools** across 21 families, so
  FR-030's count is untouched.
- Eight guard scripts pass: `check-inline-tests`, `check-file-tool-containment`,
  `check-shared-guards`, `check-silent-errors`, `check-security-unwraps`,
  `check-poison-locks`, `check-dead-code-reasons`, `check-test-env-isolation`, plus
  `check-tool-rename`.

## Why nothing is consulted per tool call (NFR-006)

The rule runs once per bridged tool, at registration: a server offers its tool list
once per connect, a plugin its declarations once per load, and the TUI's
housekeeping pass re-registers only to repair a stale server. Nothing on the
dispatch path calls `check_dynamic_bridge`. This is why the two new
`TABLE_READERS` entries pass the NFR-006 guard's *dispatch-tree* assertion as well:
`ragent-types` is not a dispatch tree, and the reads are registration-time.
