# T-019 - The declarative alias table and its inbound resolver

Spec `uniformtools`, task T-019. Requirement FR-015, decisions D5 and D7.
Validated by TESTPLAN case TC-013.

Status: **completed**. Dependency: T-001 (the canonical type additions).

## 1. What the task asks for

> Where `tools.aliases` is configured (or the built-in default table is enabled),
> the system **shall** resolve an inbound tool name to its ragent tool before the
> unknown-tool check, using a case-sensitive table of exact spellings. Where
> `tools.expose_alias_names` is `true`, the outbound tool list **shall** advertise
> each alias alongside the canonical name. Where neither is enabled, behaviour
> **shall** be unchanged.

T-019 owns the first clause only (the table and the inbound resolution, applied
before the unknown-tool check). The second is T-021, and the third is pinned here
as the empty-table case.

## 2. What was built

### `crates/ragent-agent/src/tool/alias_table.rs` (312 lines)

| Item | Purpose |
|---|---|
| `ProviderAlias { spelling, ragent_name }` | one built-in row; `Copy` so a `const` table iterates by value |
| `BUILTIN_PROVIDER_ALIASES: [ProviderAlias; 4]` | the D7 table: `WebFetch -> webfetch`, `WebSearch -> websearch`, `file_search -> codeindex_search`, `web_search -> websearch`, ascending by spelling |
| `AliasTable::from_tools_config` / `from_aliases` | builds the table once per turn from `tools.aliases`; empty when absent / `false` / `{}` |
| `AliasTable::resolve` | exact, case-sensitive lookup (D5: never guessed at) |
| `AliasTable::unresolved_targets` | the `(spelling, target)` pairs whose target the registry does not hold |
| `AliasTable::report_unresolved_targets_once` | logs each such pair once per process (`Mutex<HashSet<(String, String)>>`, poison-tolerant) and returns the count |
| `resolve_inbound_tool_name(registry, table, name) -> Option<String>` | the one resolver |
| `reset_unresolved_targets_for_test` | `#[doc(hidden)]` ledger reset for the once-only cases |

The resolver's three guards are the requirement, in order:

1. **a registration always wins** - `registry.get(name).is_some()` returns `None`
   before the table is read, so a stray entry cannot shadow the tool it names and
   an adopted post-rename spelling (`Bash`) resolves by lookup rather than by a
   table read (NFR-006);
2. **an unmapped name is never guessed at** (D5) - `table.resolve(name)?`, exact
   and case-sensitive, so a near miss still reaches the unknown-tool error with
   its did-you-mean hint;
3. **an unregistered target is ignored here** - `registry.get(target).is_none()`
   returns `None`, so the call is refused naming the spelling the client actually
   sent rather than failing later against a name nobody sent.

`from_aliases` also drops a self-mapping (`Bash -> Bash`) as a no-op (D7) and
skips an entry whose key **or** target is a retired spelling - the resolver half
of FR-031, unreachable from a file (the loader refuses it first) and kept for a
section assembled in memory.

### Where the resolver is applied

`SessionProcessor::prepare_client` builds the table once per turn and calls
`report_unresolved_targets_once` (`session/loop_steps.rs`, `TurnClient::alias_table`).
`SessionProcessor::process_message` rewrites `llm_result.tool_calls[*].name`
through `resolve_inbound_tool_name` **before** the tool-dispatch phase, so the
unknown-tool refusal, the loop restriction, `LOOP_WRITE_TOOLS`, the destructive
classifier, the hooks, the permission layer, the repeat guard, the telemetry key
and the `MessagePart::ToolCall` record all see one canonical name and the
provider spelling never reaches the persisted form (FR-001).

One application point, deliberately: `process_message` is the only tool-executing
loop, so every inbound surface that reaches a tool (TUI prompt, `ragent run`,
`/v1/chat/completions`, ACP server, automation, sub-agents, teams) inherits the
resolution. No provider, backend or UI module names the table.

## 3. Two defects found while landing this

Both are the same class: source that was committed but never compiled or run.

1. **`pub mod alias_table;` was missing from `tool/mod.rs`.** The 312-line module
   and its doc comment landed with `d6abcd4a`, but nothing declared it, so
   `cargo check` did not build it and the resolver did not exist as far as the
   compiler was concerned.
2. **Neither test file was registered.** `crates/ragent-agent/tests/test_tool_alias_table.rs`
   (25 cases) and `test_tool_alias_resolution.rs` (4 cases) were at `HEAD` with no
   `#[path]` line in `crates/ragent-agent/tests/all.rs`, so all 29 cases had never
   run. `git show HEAD:crates/ragent-agent/tests/all.rs | grep tool_alias` is
   empty. Registering them is what makes the rest of this record a measurement
   rather than an assertion.

Two further orphans of the same class were found and are **out of scope here**,
recorded so they are not lost: `crates/ragent-agent/src/tool/allowed_tools.rs`
(T-036) and `crates/ragent-agent/src/session/dispatch.rs` +
`session/tool_input_error.rs` (T-014) are still undeclared.

## 4. Tests

`crates/ragent-agent/tests/test_tool_alias_table.rs` - 25 cases, unit:

- **default off** - absent section, explicit `false`, and `{}` each produce an
  empty table (`len() == 0`), which is FR-015's "behaviour shall be unchanged";
- **built-in table (8)** - requested by `true`; ascending; ASCII and never
  self-mapping (NFR-005, D7); every target registered; no spelling is either
  column of FR-023 (FR-026, FR-031); `code_interpreter`, `Read` and `Bash`
  absent; the table and the registry agree row by row through the resolver;
- **explicit entries (6)** - case-sensitive; self-mapping dropped; an explicit
  entry wins a built-in collision; `true` + entries compose after `merge`;
  ascending; `ProviderAlias` is `Copy + Eq`;
- **resolver (6)** - an alias spelling maps onto its registered target; a
  registered name is untouched even when an entry re-points it; an unmapped name
  is never guessed at (`webfetchh`); an unregistered target resolves to `None`;
  an empty table is inert; the return is `Option<String>` so the hot path
  allocates nothing;
- **FR-031 (3)** - no retired spelling resolves as a key or a target, for every
  pair in the table; an unresolved target reports once and is refused when used;
  a resolved target is never reported unresolved.

`crates/ragent-agent/tests/test_tool_alias_resolution.rs` - 4 cases, end to end
through `SessionProcessor::process_message` with a scripted provider and an
in-memory SQLite session, each asserting the recorded `MessagePart::ToolCall`:

- `test_alias_spelling_dispatches_the_registered_tool` - a `Frobnicate` call runs
  `think`, the record holds `think` not `Frobnicate`, status `Completed`, two
  provider steps;
- `test_unknown_spelling_is_refused_when_aliases_are_off` - the same call with
  `"aliases": false` is recorded under `Frobnicate` with status `Error` and the
  message `Unknown tool: 'Frobnicate'`;
- `test_alias_to_an_unregistered_target_still_refuses` - a target this build does
  not register falls through to the same refusal;
- `test_registered_name_wins_over_a_table_entry` - `think` still runs `think`
  when an entry maps `think -> calculator`.

Both files avoid the built-in targets (`webfetch`, `websearch`) on purpose: they
perform network I/O, and these cases must not touch a network.

## 5. RED proofs (run, observed, reverted byte-identically)

Each restore was verified with `md5sum` **and** `touch`ed, because a `cp -p`
restore keeps the older mtime and lets cargo reuse the broken object file.

| # | Break | Observed |
|---|---|---|
| 1 | the resolver loop in `process_message` short-circuited (`if true { continue; }`) | `test_alias_spelling_dispatches_the_registered_tool` FAILED at "the canonical name is what history records" - the pipeline really is what applies the resolver, not the table |
| 2 | the registry-first guard removed from `resolve_inbound_tool_name` | `test_resolver_leaves_a_registered_name_untouched` FAILED with `left: Some("calculator")` - a table entry would re-point a registered tool |
| 3 | the unregistered-target guard removed | 3 FAILED - both unit cases (`test_resolver_ignores_a_target_this_build_does_not_register`, `test_an_unresolved_alias_target_is_reported_once_and_refused_when_used`) and the end-to-end refusal case |

Restored to `9de4eb9737f2e3181c8bda5992cfa5ba` (`processor.rs`) and
`169588839487db234759f6f5e84be8f6` (`alias_table.rs`); 29 cases green after each.

## 6. Verification

- `cargo fmt --check` - clean.
- `cargo clippy -p ragent-agent --all-targets` - no new warning; only the
  pre-existing `pub(crate)`-inside-private-module noise (63) and
  `unnecessary structure name repetition` (19, all in `ragent-llm` /
  `ragent-research`).
- `cargo check -p ragent-agent --all-targets` - clean.
- `cargo test -p ragent-agent --test all` - **1661 passed, 0 failed, 1 ignored**.
- `cargo test -p ragent-config --test all` - **412 passed, 0 failed** (the
  `tools.aliases` load-time refusal of T-042 is unaffected).
- Eight guard scripts pass: `check-inline-tests`, `check-dead-code-reasons`,
  `check-silent-errors`, `check-security-unwraps`, `check-poison-locks`,
  `check-test-env-isolation`, `check-file-tool-containment`,
  `check-shared-guards`.
- `cargo test -p ragent-agent --doc` - 54 passed, 4 failed, all 4 **pre-existing
  at `HEAD`** and unrelated to this task: `agent/mod.rs::read_git_status`,
  `session/archive.rs` (both doctests), and `tool/mod.rs::ToolContext`, whose
  example never gained `allowed_roots` / `canonical_cache` and does not import
  `create_default_registry`. Confirmed by reading the same doctest at `HEAD`;
  not touched here (out of scope, and it is a one-line fix for whoever owns
  `ToolContext`).

## 7. Gotchas

- **A committed-but-undeclared module is invisible to `cargo check`.** Both the
  module and its tests were in `HEAD`, and neither compiled nor ran. The only
  symptom is that a green suite proves nothing about the feature - which is
  exactly how this task's 29 cases had been "passing" by not existing.
- **`registry.get(name)` first is not an optimisation, it is FR-026.** Without
  it, a table entry naming a registered tool re-points it, which red proof 2
  shows as `Some("calculator")` for a `think` call.
- **The unresolved-target ledger is process-wide**, so a case asserting "first
  call reports, second is silent" needs `reset_unresolved_targets_for_test()`
  **and** `#[serial_test::serial]`: another file's table for the same pair would
  otherwise steal the first report.
- **`cp -p` for a restore is a trap.** The older mtime lets cargo keep the
  broken object file and the "restored" file still fails. Always `touch`.
- **One application point is correct, not lazy.** The resolver lives in
  `process_message`, not in each inbound surface: nine call sites reach
  `process_message` (TUI, CLI, `/v1`, ACP server, automation, sub-agents, teams,
  skill fork, loop dialog), and a per-surface resolver would be nine chances to
  forget FR-001's canonical record.

## 8. Follow-ups this task does not cover

- **T-020** adds the config keys (`tools.aliases`, `tools.expose_alias_names`,
  `tools.wire_trace`) and their merge rules. The table already reads
  `ToolsConfig`, and `crates/ragent-config/src/tools.rs` exists in this tree, so
  T-020's work is largely landed - it needs its own record and verification.
- **T-021** honours `expose_alias_names` in the outbound tool list. Today
  `definitions()` advertises registry names only, which is FR-030's honest count
  and FR-015's default.
- **T-042** owns the load-time refusal of a retired spelling in
  `tools.aliases`; this task owns the resolver half, and
  `test_resolver_never_consults_a_retired_spelling_from_the_table` is the shared
  case.
