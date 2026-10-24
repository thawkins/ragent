# T-042 - Reject a retired spelling in `tools.aliases` at config load (FR-031)

Status: **completed**. Requirement: FR-031 (spec `uniformtools`). Dependencies: T-020
(`tools.aliases`), T-030 (the FR-023 table).

## What the requirement asks

FR-031 is an *unwanted-behaviour* requirement: if any supported mechanism - a
configuration key, an environment variable, a CLI flag, or a plugin, connector or agent
setting - would make a retired `before` spelling resolvable, the system shall not honour
it. For `tools.aliases` it names three duties:

1. refuse a **source key** equal to a `before` spelling when the configuration is
   loaded, with an error naming the file, the spelling and the `after` replacement;
2. refuse a **target value** equal to a `before` spelling, in the same shape;
3. treat a target that names no *currently registered* tool differently - it is not a
   rename problem, because a plugin or MCP tool may be bridged after load, so it is
   reported once as unresolved and **refused when used** rather than ignored.

The self-mapping form is called out explicitly by the task ("which asks for two
spellings of one tool"), and the resolver must never consult a `before` entry.

## What was built

### The table moved to `ragent-types`

`crates/ragent-types/src/tool_rename.rs` now holds `TOOL_RENAMES: [Rename; 13]` and the
`Rename` helpers, plus a new `Rename::is_rename_spelling` (either column). The move was
forced by the dependency graph: `ragent-config` must read the table at load time and
already sits *below* `ragent-agent`, which depends on it, so a table in `ragent-agent`
could not be read without a cycle. `crates/ragent-agent/src/tool/rename.rs` became a
re-export shim, so every migration site, guard and test path
(`ragent_agent::tool::rename::{Rename, TOOL_RENAMES}`) is unchanged.

### The load-time refusal (`crates/ragent-config/src/tools.rs`)

- `ToolsConfig::validate_alias_spellings() -> Vec<RetiredAliasSpelling>` returns every
  offending entry, not the first: a table copied from an old `--dry-run` dump carries
  several, and fixing them one reload at a time is the failure mode the single message
  exists to avoid.
- `RetiredAliasSpelling` carries the key, the target, the offending spelling, the `after`
  replacement, `in_source_key` and `self_mapping`; `reason()` renders the sentence and
  `Display`/`Error` are implemented so a caller produces the loader's exact text.
- The three rules, in order: key == target and either column is a renamed spelling
  (self-mapping); key is a `before` spelling; target is a `before` spelling. A
  self-mapping of a spelling the rename never touched (`"think": "think"`) is **not**
  refused - it asks for no second spelling of one renamed tool, and the alias table drops
  it as a no-op (D7). That distinction is why the check is a rename-table lookup and not a
  `key == value` test.
- The check runs in `Config::parse_file` (which serves the global file, the project file
  and `RAGENT_CONFIG`) and again on the `RAGENT_CONFIG_CONTENT` route, where `label` names
  the environment variable instead of a path.
- It deliberately lives outside `AliasesConfig::deserialize`: a deserialiser cannot name
  the file it is reading, and keeping the shape check pure keeps
  `serde_json::from_str::<ToolsConfig>` usable in a test that must inspect a rejected
  entry.

### The readiness path carries the same check (`crates/ragent-agent/src/dry_run.rs`)

`ragent run --dry-run` parses the same files the real loader reads, through its own
`parse_config_str`. Left alone it would report `Config loaded` for a file the next real
load rejects, so the readiness path calls the same
`ToolsConfig::validate_alias_spellings` and renders the same sentences. Two cases pin it:
a project config carrying `"bash": "Bash"` blocks the config section (exit 1) with the
spelling and its replacement in the item message, and a project config carrying an
unregistered target still loads.

### The resolver half (`crates/ragent-agent/src/tool/alias_table.rs`)

- `AliasTable::from_aliases` skips an entry whose key **or** target is a retired spelling.
  From a file this arm is unreachable (the loader refused the entry first), and that is
  the point: FR-031's guarantee must not depend on every caller loading through the
  loader. A test, a program building `ToolsConfig` directly, or a future caller gets the
  same refusal.
- `unresolved_targets(registry)` returns the `(spelling, target)` pairs whose target the
  registry does not hold.
- `report_unresolved_targets_once(registry)` logs each pair **once per process**, behind a
  poison-tolerant `Mutex<HashSet<(String, String)>>`, and returns how many it reported this
  call. Per-entry, not per-turn: the table is rebuilt from the config every turn, so a
  naive `warn!` would repeat the same sentence on every step and train the reader to
  ignore it; a re-pointed entry is a different mistake and does report.
  `reset_unresolved_targets_for_test()` (`#[doc(hidden)]`) clears the ledger.
- `SessionProcessor::process_message` calls `report_unresolved_targets_once` once per turn,
  right after building the table.
- "Refused when used" needed no new code: `resolve_inbound_tool_name` already falls
  through to the unknown-tool refusal when the target is absent from the registry, so the
  call is refused naming the spelling the client actually sent. The new test pins that
  behaviour next to the once-only report so the two halves stay together.

## Tests

`crates/ragent-config/tests/test_retired_alias_spelling.rs` (11 cases, registered in
`tests/all.rs`). Every case drives the real loader through `RAGENT_CONFIG` - the uncached
path - so no cwd change and no project file is involved; the guard restores the
environment on drop, and each case is `#[serial]`.

| case | what it pins |
|---|---|
| `test_a_retired_source_key_is_refused_at_load_naming_file_spelling_and_replacement` | `"bash": "Bash"` stops the load; the message names the file, `bash`, `Bash`, and `tools.aliases` |
| `test_a_retired_alias_target_is_refused_at_load_naming_file_spelling_and_replacement` | `"WebFetch": "bash"` stops the load and names the entry, the dead target and `Bash` |
| `test_the_self_mapping_form_of_an_adopted_spelling_is_refused` | `"Bash": "Bash"` is refused as a self-mapping |
| `test_the_self_mapping_form_of_a_retired_spelling_is_refused` | `"bash": "bash"` is refused and sent to `Bash` |
| `test_every_offending_entry_is_reported_in_one_error` | three offenders produce one message naming both distinct mistakes |
| `test_a_provider_spelling_onto_a_registered_tool_loads_cleanly` | `WebFetch -> webfetch` still loads and survives |
| `test_an_unregistered_alias_target_is_not_a_load_error` | FR-031's third clause: kept, not refused |
| `test_the_self_mapping_of_an_untouched_spelling_is_accepted` | `think -> think` is not a rename problem |
| `test_both_boolean_forms_are_accepted` | `true` / `false` carry no spelling |
| `test_the_inline_environment_route_is_refused_and_names_itself` | `RAGENT_CONFIG_CONTENT` is refused and names itself |
| `test_every_pair_in_the_rename_table_is_refused_as_key_and_as_target` | the check reads the table: every `before` in both columns, every `after` self-mapped |

Four cases appended to `crates/ragent-agent/tests/test_tool_alias_table.rs` (the in-memory
half): `test_resolver_never_consults_a_retired_spelling_from_the_table` (every pair, both
columns, so the case grows with the table), plus the once-only-report / refused-when-used
pair and a "a resolved target is never reported as unresolved" control. The two ledger
cases are `#[serial_test::serial]`, because the ledger is process-wide and the crate's
test binary is shared.

## RED proofs (run, observed, reverted)

1. Removing `Self::reject_retired_alias_spellings(&config.tools, &label)?` from
   `parse_file` made **five** loader cases fail (the three refused forms, the
   several-at-once case, both self-mapping cases) - the refusal is what those tests
   actually measure.
2. Removing the retired-entry arm from `AliasTable::from_aliases` made
   `test_resolver_never_consults_a_retired_spelling_from_the_table` fail with
   `left: Some("think")` for `ask_user`, which also proves the resolver half is not
   redundant with the loader half.
3. Emptying the offender list in the dry-run path made
   `test_project_config_with_a_retired_alias_spelling_is_blocked` fail, so the readiness
   report really is checked rather than incidentally correct.

All three files were restored from `target/temp/*.bak` and `touch`ed (a `cp -p` restore
keeps the older mtime and lets cargo reuse the broken object file), then re-run green.

## Gotchas

- A `#[test]` that asserts "the first call reports and the second is silent" needs the
  ledger reset *and* a serial lock: the ledger is process-wide, so an unrelated test in
  another file that calls `report_unresolved_targets_once` for the same pair steals the
  first report. Hence `reset_unresolved_targets_for_test()` plus `#[serial_test::serial]`.
- `clippy::used_underscore_binding` rejects holding the `TempDir` as `_temp` and then
  reading it in `path()`; the guard stores it as `temp`.
- The escape-hatch audit's purity predicate walks named source files, so moving the table
  meant adding `crates/ragent-types/src/tool_rename.rs` to that list - otherwise the guard
  would have kept asserting purity over a shim and stopped covering the real table.

## Verification

`cargo fmt --check`; `cargo clippy -p ragent-types -p ragent-config -p ragent-agent
--all-targets` (only the pre-existing `pub(crate)`-inside-private-module noise);
`cargo test -p ragent-config --test all` 415 passed; `cargo test -p ragent-config --doc`
8 passed; `cargo test -p ragent-agent --test all test_tool_alias_table::` 25 passed;
eight guard scripts pass (`check-inline-tests`, `check-dead-code-reasons`,
`check-silent-errors`, `check-security-unwraps`, `check-poison-locks`,
`check-test-env-isolation`, `check-file-tool-containment`, `check-shared-guards`).
