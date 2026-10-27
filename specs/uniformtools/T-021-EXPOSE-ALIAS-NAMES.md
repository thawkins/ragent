# T-021 - Honouring `expose_alias_names` in the outbound tool list

Spec `uniformtools`, task T-021. Requirement FR-015, validated by TESTPLAN case
TC-013. Status: **completed**. Dependencies: T-004 (the tool-definition
encoder fold) and T-019 (the alias table) - both landed before this task.

## 1. What FR-015 asks for, and which clause this task owns

> Where `tools.aliases` is configured (or the built-in default table is enabled),
> the system **shall** resolve an inbound tool name to its ragent tool before the
> unknown-tool check, using a case-sensitive table of exact spellings. Where
> `tools.expose_alias_names` is `true`, the outbound tool list **shall** advertise
> each alias alongside the canonical name. Where neither is enabled, behaviour
> **shall** be unchanged.

T-019 owned clause 1 (the table and the inbound resolution) and T-020 owned the
config keys. Clause 2 is this task, and clause 3 is the default it must protect:
with the switch off, the list must be the registry's own names, in the registry's
own order - not a re-ordered or re-shaped copy.

Two facts make clause 2 smaller than it looks and decide its shape:

- there is **one** outbound tool list. `ToolRegistry::definitions()` is read by
  the wire request (`session/processor.rs`), the system prompt's tool reference
  (`session/loop_steps.rs`), `/tools`, `tool_info` and the FR-030 count. A filter
  applied anywhere else would let the prompt name a spelling the wire list omits,
  or the reverse.
- `tools.expose_alias_names` is **per-turn config**, and neither the registry
  version nor the tool-name cache observes a config change. A cached list keyed on
  the registry version alone would keep serving the pre-change names after a
  `/config save`, which is precisely the class of bug this task exists to prevent.

## 2. What was built

### `AliasTable::outbound_definitions` (`tool/alias_table.rs`)

The one projection, next to the table it reads:

```rust
pub fn outbound_definitions(
    &self,
    definitions: &[ToolDefinition],
    expose: bool,
) -> Arc<Vec<ToolDefinition>>
```

Returns an `Arc<Vec<ToolDefinition>>` because that is the type the wire path and
the prompt cache take, so a caller never grows a second copy to satisfy a
signature. With `expose == false` or an empty table it is a straight copy of the
input, which is FR-015's third clause expressed in code rather than in a comment.

With the switch on, each entry produces at most one advertised definition:

| Guard | Why it is there |
|---|---|
| the spelling is not already in the list | a registration always wins, so an entry mapping a registered name elsewhere cannot advertise a second `Bash` (FR-026, TC-013's last expected result) |
| the target **is** in the list | an entry whose target this build does not register is refused at use (FR-031), so advertising it would offer the model a call that cannot run |
| the spelling is not already present after earlier entries | one name is advertised once, whatever the table holds |

The advertised entry is a copy of its target's definition with the spelling as
the name: same description, same parameter schema, so a call through the spelling
is shaped exactly like a call through the canonical name. The result is sorted by
name, so an advertised spelling sorts among the canonical names rather than being
appended in table order (FR-030's "the enumerations are ordered" property).

### Deliberate non-decisions

- **No FR-023 table read in the projection.** The first draft repeated
  `Rename::after_of(spelling).is_some()` here as a second guard on the outbound
  path. `tests/test_nfr006_rename_cost.rs` rejected it, and it was right to: the
  table already refuses to hold a retired spelling (`from_aliases`, its single
  once-per-turn read), so the second read bought nothing and put a rename scan on
  a path every tool call pays for. The outbound half of FR-031 is therefore
  structural - an entry that cannot exist cannot be advertised - and the guard is
  recorded in the doc comment instead of duplicated in the code.
- **No `ToolDefinition` type change.** The alternative was an `advertised_names`
  field on the definition. That changes a FR-001 canonical type every provider
  encoder reads, for one optional feature; the projection is one function and no
  encoder changed.

### Call sites (one list, one projection)

- `session/processor.rs::get_cached_tool_definitions(alias_table)` - the wire
  list. The cache key became `(tool-registry version, expose_alias_names, the
  alias table)` (`ToolDefinitionCacheKey`), so a config change is a miss rather
  than a stale hit. The `tools.expose_alias_names` value comes from
  `load_config_cached()`, the same mtime-cached config every other per-turn read
  uses.
- `session/loop_steps.rs::build_turn_system_prompt` - the prompt's tool
  reference, both the unfiltered branch and the fused allowlist/loop-set branch,
  so the prompt and the wire list cannot disagree. The function gained an
  `alias_table: &AliasTable` parameter (its single caller is the processor).
- `session/cache.rs::get_tool_definitions` and
  `get_subagent_tool_definitions` - both caches key on
  `(version, expose_alias_names, AliasTable)`.
- The subagent and loop-filtered wire branches read the same projection, so a
  sub-agent's list is the advertised list minus the interactive tools.

## 3. Tests

`crates/ragent-agent/tests/test_tool_alias_exposure.rs` - **new**, 12 unit cases:

- **off is unchanged (2)** - the built-in table with the switch off advertises the
  registry's names in the registry's order; an empty table with the switch on
  changes nothing either;
- **on advertises the spelling (4)** - one entry per resolvable spelling, the
  advertised entry carries the target's description **and** parameter schema, every
  built-in row is advertisable against the real registry, and the result is
  ascending by name with an advertised spelling sorting among the canonical ones;
- **never twice (2)** - a table entry naming the registered `Bash` leaves exactly
  one `Bash`, carrying the registry's own definition (TC-013's last expected
  result); and no built-in entry produces a duplicate name at all;
- **never what cannot be called (2)** - an unregistered target is not advertised;
  a mix of resolvable and unresolvable entries advertises only the resolvable one;
- **never a retired spelling, never a rename (2)** - for every row of
  `TOOL_RENAMES`, the retired spelling is absent from the advertised list (FR-031's
  outbound half); and every canonical entry survives with its own description and
  schema, so advertising adds entries and changes none.

`crates/ragent-agent/tests/test_tool_alias_resolution.rs` - **2 added** end-to-end
cases through `SessionProcessor::process_message` with a scripted provider:

- `test_the_wire_and_the_prompt_advertise_the_alias_spelling` - with
  `{"aliases": {"Frobnicate": "think"}, "expose_alias_names": true}` the captured
  request's tools array carries `Frobnicate` **and** `think`, and the prompt's tool
  reference names `Frobnicate` too;
- `test_the_wire_list_is_unchanged_when_advertising_is_off` - the same entry with
  the switch off sends `think` and not `Frobnicate`.

Both use an explicit entry onto the offline `think` tool, because the built-in
targets (`webfetch`, `websearch`) perform network I/O.

## 4. RED proofs (run, observed, reverted byte-identically)

Each restore was `cp -p` from `target/temp/t021/`, then `md5sum -c` **and**
`touch`ed: a `cp -p` restore keeps the older mtime and cargo will happily reuse
the broken object file, which is how a "restored" tree still fails.

| # | Break | Observed |
|---|---|---|
| 1 | `outbound_definitions` short-circuited (`if true \|\| !expose ...`) | 3 exposure cases FAILED - `test_advertising_on_adds_one_entry_per_resolvable_alias_spelling`, `test_an_advertised_alias_copies_its_targets_definition`, `test_every_builtin_entry_is_advertisable` |
| 2 | the processor's wire list built from `tool_registry.definitions()` (the pre-task code) | `test_the_wire_and_the_prompt_advertise_the_alias_spelling` FAILED - the request's tools array had no `Frobnicate` |
| 3 | the prompt's unfiltered branch passed an empty table and `false` | **no failure** - the prompt assertion in case 2 reads the projection directly, so this half of the prompt path is not covered by a test. Recorded rather than papered over; see section 6. |
| 4 | `SystemPromptCache::get_tool_definitions` returned the raw registry list | **no failure** for the same reason |

Proofs 3 and 4 are recorded as they happened, not as they were hoped for. The
end-to-end case proves the *wire* list and the *projection* the prompt is built
from; it does not reach inside `build_turn_system_prompt`, which is
`pub(crate)` and has no test seam. Making it observable would mean widening a
private method to public API for one assertion, which is the wrong trade for this
task - the honest statement is that the prompt branch is covered by inspection and
by the shared projection, and the wire branch is covered by a test.

## 5. Verification

- `cargo fmt --all -- --check` - clean.
- `cargo check --workspace --all-targets` - clean.
- `cargo clippy -p ragent-agent --all-targets` - no finding in any touched file.
  The remaining noise is pre-existing and elsewhere: 57 `pub(crate) function
  inside private module` (mostly `ragent-research`), 19 `unnecessary structure
  name repetition` (`ragent-llm` `tool_cache.rs`), 9 more of the same
  private-module family, and one `returning the result of a let binding` at
  `crates/ragent-storage/src/storage.rs:1709`.
- `cargo nextest run -p ragent-agent --no-fail-fast` - **2117 passed, 1 skipped,
  0 failed** (was 2103 before this task: +12 exposure cases, +2 end-to-end).
- `cargo nextest run -p ragent-config -p ragent-llm -p ragent-tui --no-fail-fast`
  - 4852 passed, 2 failed, 2 skipped. Both failures are **pre-existing and owned
  by other tasks**: `ragent-llm::test_wire_decode::test_no_module_invents_a_tool_call_id_or_recovers_a_name_from_one`
  (T-010, `ollama_cloud.rs` still mints `_tc_{`) and
  `ragent-tui::test_slash_arms_deep::tools_visibility_save_failure_is_reported`.
- Guard scripts: **11 of 12 PASS**. `check-bench-regression` exits 2 with "no
  criterion results directory found at target/criterion", which is a missing bench
  baseline rather than a code finding.
- No dependency added, no `unsafe` block, no retired spelling, no tool renamed.

## 6. Gotchas

- **A per-turn config switch is not observed by a registry-version cache.** Both
  tool-definition caches keyed on the registry version alone; adding the switch
  to a `*_if` is not enough, the key itself has to carry it. This is the same
  lesson T-019 recorded for the alias table, applied to its output.
- **`collect()` into `Arc<Vec<T>>` does not work.** `Vec<T>: IntoIterator` makes
  `Arc<Vec<T>>: FromIterator<T>` ambiguous for the compiler; the branch allocates
  the `Vec` and `.into()`s it.
- **A guard test can be right to reject your defence in depth.**
  `test_no_dispatch_step_consults_the_rename_table` refuses any FR-023 read inside
  `tool/` outside `from_aliases`. The redundant lookup described in section 2 was
  deleted rather than exempted, because exempting it would have widened the
  dispatch-tree exemption for every future reader of `alias_table.rs`.
- **`cargo nextest run -E 'test(file::)'` on an unregistered test file prints
  "0 tests run" and looks like success.** `test_tool_alias_exposure.rs` was
  registered in `tests/all.rs` before its first run for exactly this reason; the
  count after every run was read, not assumed.
- **The registry's first definition is not `Bash`.** `AskUserQuestion` sorts
  first, so a sortedness case must not pin the first name to a guess; it pins the
  last advertised spelling (which is the point) and the ascending property.
