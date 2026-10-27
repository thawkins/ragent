# T-020 - The `tools` config section: aliases, alias advertisement, payload trace

Spec `uniformtools`, task T-020. Requirements FR-015 and FR-017. Consumed by
T-019 (the resolver already reads `ToolsConfig`), T-021 (`expose_alias_names`),
T-023 (`wire_trace`) and T-042 (`tools.aliases` retired-spelling refusal).

## 1. What the two requirements ask of configuration

FR-015 is conditional on configuration: *"Where `tools.aliases` is configured (or
the built-in default table is enabled) ... Where `tools.expose_alias_names` is
`true` ... Where neither is enabled, behaviour **shall** be unchanged."* FR-017 is
conditional the same way: *"Where `tools.wire_trace` is enabled in config ... and
**shall** be inert when the setting is absent or `false`."*

So both requirements are gated on a config key that did not exist, and the
clause that matters most is the one nobody writes a test for: **unchanged when
absent**. A key that defaults to `false` in the struct but appears in every
user's `ragent.json` after the first save has already changed behaviour - it
changed what the file says, and it changed what a diff of two users' configs
means. The deliverable is therefore three keys *and* a serialisation discipline.

## 2. What was built

### `crates/ragent-config/src/tools.rs` (494 lines)

- `ToolsConfig { aliases, expose_alias_names, wire_trace }`, plus `is_default()`
  (every field at its default) and `merge(&mut self, &other)`.
- `AliasesConfig`, a five-field value rather than an enum: `specified`,
  `builtin`, `explicit_off`, `was_object`, `entries: BTreeMap<String, String>`.
  A small struct is what makes "user-global `true` + project entry" expressible;
  a two-variant enum cannot hold both after a merge.
- `TOOLS_ALIASES_KEY = "tools.aliases"`, used verbatim in every load-error
  message so the reader is sent to the key they wrote.
- Custom `Deserialize` (boolean or object; anything else is an error naming both
  accepted forms and what was found) and custom `Serialize` (the entries object
  when any entry exists, else `true`, else `{}` when the object form was written
  empty, else `false`).
- `RetiredAliasSpelling` + `ToolsConfig::validate_alias_spellings()` - the FR-031
  half. It is in this file because it reads the same `entries` map, but it is
  **not** called from the deserialiser: a deserialiser cannot name the file it is
  reading, and keeping the shape check pure keeps
  `serde_json::from_str::<ToolsConfig>` usable in a test that must inspect a
  rejected entry. Its caller is T-042's loader hook.

### `crates/ragent-config/src/config.rs`

```rust
#[serde(default, skip_serializing_if = "ToolsConfig::is_default")]
pub tools: ToolsConfig,
```

plus `tools: ToolsConfig::default()` in the explicit `Default` impl (ANTIPAT M-7:
the hand-written `Default` must agree with the serde defaults), and one line in
`merge_inner`:

```rust
base.tools.merge(&overlay.tools);
```

`merge_inner` is the shared body of both `Config::merge` and
`Config::merge_project`, so the section merges identically on the global-to-
project path and on `RAGENT_CONFIG`/`RAGENT_CONFIG_CONTENT`. No `merge_project`
strip clause was added - see section 4.

### `crates/ragent-config/src/lib.rs`

`pub mod tools;` and `pub use tools::{AliasesConfig, RetiredAliasSpelling,
TOOLS_ALIASES_KEY, ToolsConfig};`. The module declaration is the part that is
easy to forget: `tools.rs` had been written in `d6abcd4a` and, until this task,
`lib.rs` did not declare it, `Config` had no `tools` field, and
`crates/ragent-config/tests/test_tools_config.rs` (24 cases) and
`test_retired_alias_spelling.rs` (11 cases) had no `#[path]` line in
`crates/ragent-config/tests/all.rs`. Thirty-five cases across two files had never
compiled. This is the **fourth** orphan-module defect in this spec (after
`tool_args`, `tool_rename` and `tool_input_error`), and the pattern is now
explicit: a module that "exists" is unverified until its `pub mod`, its
`Config`/registry wiring and its `#[path]` registration are all present.

## 3. The merge rules, and why each is different

Three kinds of value, three rules, and none of them is interchangeable.

| Value kind | Rule | Why not the other rule |
|---|---|---|
| `expose_alias_names`, `wire_trace` | OR | They are opt-in capabilities. Overlay-wins would let a project file that omits the key switch off a user-global `true` (the `Default` is `false`, so "absent" is indistinguishable from "off" in a naive merge). This is the same rule `sdd` and `piegap` use. |
| `aliases` entries | per-spelling union, overlay wins a collision | Replacing the map wholesale would make a one-spelling project override discard the user's whole table. The backend registry's per-id replacement is the same shape. |
| `aliases: false` | clear everything | `false` is the author saying "I do not want a table". An unconditional union makes a user-global `true` impossible to switch off *from the file that sets it*, which is the trap the OR rules above avoid by being opt-in in the first place. This is the one exception, and it is why `explicit_off` exists as a field separate from "no entries". |

`{}` is deliberately **not** the switch: it contributes no spelling and clears
nothing. Collapsing `{}` into `false` would let a project file that merely
declares an empty table silently wipe the user-global one, and it would rewrite
the author's `{}` as `false` on any round-trip through the redacting `Debug`
impl or `/config show`.

An overlay that omits the key changes nothing (`if !other.specified { return; }`
in `merge_with`), which is what makes a partial project file safe.

## 4. Why `merge_project` keeps the section

`merge_project` strips keys that widen privilege from untrusted repository
content: `yolo`, `permission` allow rules, `bash`/`dirs` lists, `telemetry`,
`hooks`, `mcp`, `acp`, `openai` and every provider `api` block. `tools` is not on
that list, and the reason is in what the section can actually do.

An alias resolves a **name**, not a capability. After resolution the call
carries a canonical ragent tool name and then goes through the unknown-tool
check, the loop's `LOOP_WRITE_TOOLS` restriction, the destructive-action
classifier, the hooks, the permission layer (with its 120 s prompt and YOLO
bypass), the repeat guard and the 7-layer bash validation exactly as if the model
had sent that name. A project file cannot use an alias entry to auto-approve a
tool, reach a tool the registry does not hold, or re-point a registered name (a
registration always wins, FR-026). `expose_alias_names` and `wire_trace` are a
prompt-text choice and a local diagnostic respectively; neither grants an action.

The refusal FR-031 needs is a **per-entry** rejection of a retired spelling at
load time, not a blanket ban on the section, and that distinction is what lets a
project declare `"web_search": "websearch"` for its own model family without the
user having to repeat it globally. A test pins it
(`test_merge_project_keeps_the_tools_section`).

## 5. The serialisation contract

- The whole section is omitted while `is_default()`. A build that has never seen
  `tools` therefore writes a `ragent.json` byte-identical to one written before
  the feature existed, so the first unrelated `save_to_source` cannot introduce
  the key into a user's file.
- Inside a non-default section, each key is omitted when it is at its own
  default (`skip_serializing_if = "is_false"` on both booleans,
  `skip_serializing_if = "AliasesConfig::is_unspecified"` on `aliases`), so
  `{"wire_trace": true}` stays `{"wire_trace": true}` rather than growing two
  `false` siblings.
- `{}` round-trips as `{}`; `false` round-trips as `false`; `true` as `true`; an
  object as an object. A merged state that carries *both* the built-in request
  and explicit entries serialises as the entries object, because one JSON key
  cannot express both - that state only arises from a multi-layer merge, and the
  rendering is read by `/config show` and the redacting `Debug`, never written
  back over a user's file (which is persisted from its own bytes).

## 6. Tests

### `crates/ragent-config/tests/test_tools_config.rs` (24 cases, value-level)

Registered by this task. Defaults (`is_default`, `is_unspecified`,
`is_enabled`); the `true` form; the `false` form and its round-trip; the object
form and its round-trip; the two load errors (non-boolean/non-object value, and a
non-string target) each naming what was found; the two booleans parsing
independently of `aliases`; per-key serialisation; the OR merge; the per-spelling
union with the overlay winning a collision; the built-in request surviving an
overlay entry; `false` clearing the inherited table; `false` leaving the other
switches alone; `merge_project` keeping the section; and the four `{}`-related
cases (empty object is specified-but-disabled, round-trips as `{}`, does not
clear an inherited table, does not keep the built-in request, does not undo a
lower layer's explicit `false`).

### `crates/ragent-config/tests/test_tools_config_load.rs` (5 cases, NEW)

The value-level file proves the arithmetic; this one proves the **loader and the
disk** agree with it, in a sandbox that redirects `XDG_CONFIG_HOME`, the current
directory and both config-selecting environment variables into a private temp
tree (`#[serial]`, `#![allow(unsafe_code)]`, cache invalidated on entry and
drop):

| case | what it pins |
|---|---|
| `test_global_and_project_layers_merge_the_alias_entries` | global `WebFetch` + `wire_trace`, project `web_search` + `expose_alias_names` -> both entries present, both booleans on, `config_paths` names both layers |
| `test_a_project_explicit_false_switches_the_global_table_off` | global table + project `false` -> resolution off, entries empty, not `unspecified` |
| `test_save_to_source_writes_the_section_and_it_reloads` | a populated section is written to the loaded file and reloads identically |
| `test_a_never_configured_save_writes_no_tools_key` | a default section is absent from the file on disk, not merely from `to_string` |
| `test_a_non_boolean_switch_is_refused_naming_the_file` | `"expose_alias_names": "yes"` stops the load, naming `ragent.json` and the expected type |

The last case exists because the section is not `deny_unknown_fields` (a build
must be able to read a newer config, and the type is what carries the meaning).
A wrong *type* on a recognised key must still stop the load: a trace that was
asked for and quietly disabled is precisely the failure TC-018's manual steps
cannot see.

## 7. Red proofs (run, observed, restored byte-identically, `md5sum`-checked)

| break | observed |
|---|---|
| `base.tools.merge(&overlay.tools)` removed from `merge_inner` | 7 value cases + 3 loader cases fail (`the section round-trips`: left is the default section, right the populated one); the two cases that do not merge still pass |
| `skip_serializing_if = "ToolsConfig::is_default"` removed | `test_serialised_default_omits_the_tools_key` fails with `"tools": {}` in the rendered default config, and the loader's on-disk case fails with `"tools": {}` in the saved file |
| the `#[path] = "test_tools_config.rs"` line removed from `tests/all.rs` | 0 cases run (`412 filtered out`) - a green suite that proves nothing, the orphan symptom in its purest form |

Backups were `cp -p`'d to `target/temp/t020/` and restored with `cp -p` plus
`touch` (an older mtime lets cargo reuse the broken object file, which then still
fails after a "restore"), and every restore was verified with `md5sum`.

## 8. Verification

- `cargo test -p ragent-config --test all` -> **417 passed, 0 failed** (412
  before this task's 5 new cases; the 24 value cases and the 11 FR-031 cases now
  actually run).
- `cargo test -p ragent-config --doc` -> 8 passed.
- `cargo clippy -p ragent-config --all-targets` -> clean.
- `cargo fmt --all -- --check` -> clean.
- `bash scripts/check-test-env-isolation.sh` -> OK (the new sandbox file is
  `#[serial]` throughout; the guard rejects a file-local mutex mixed with
  `#[serial]` in one crate).

## 9. Follow-ups this task does not cover

- **T-021** reads `expose_alias_names` in the outbound tool list;
  `ToolRegistry::definitions()` advertises registry names only today, which is
  FR-030's honest count and FR-015's default.
- **T-023** writes the trace files. This task only defines the switch; the
  recording lives in `crates/ragent-llm/src/providers/wire/trace.rs` and is inert
  until the loop sets it from `config.tools.wire_trace`. That call is *not* yet
  in `crates/ragent-agent/src/session/loop_steps.rs`, so
  `test_wire_trace::test_agent_loop_sets_the_switch_from_config` currently fails -
  a T-023 failure, not a T-020 one (it asserts on the agent loop's source text,
  which this task did not touch).
- **T-042** calls `ToolsConfig::validate_alias_spellings()` from
  `Config::parse_file` and the `RAGENT_CONFIG_CONTENT` route and renders
  `RetiredAliasSpelling::reason()` with the file name prepended.
