# T-043 - The escape-hatch audit (FR-031)

**Spec ID:** uniformtools
**Task:** T-043
**Requirement:** FR-031 (`No escape hatch re-enables a retired spelling`)
**Status:** completed
**Depends on:** T-030 (the FR-023 table)

FR-031 says that if any supported mechanism - a configuration key, an environment
variable, a CLI flag, or a plugin, connector or agent setting - would make a
retired `before` spelling resolvable, the system **shall not** honour it, and the
system **shall not** add any new key, variable or flag whose purpose is to
restore a retired spelling.

This document is the audit the task asks for. It records what was inspected,
what was found, what was exempted and why, and how each claim is now checked by
`crates/ragent-agent/tests/test_escape_hatch_audit.rs` rather than by a sentence
in a changelog. **No key, variable or flag was added.**

## Headline result

Exactly one supported mechanism can name a tool, and therefore exactly one can
restore a spelling: the `tools.aliases` table. It is FR-031's own clause and its
per-entry rejection of a retired `before` key or target is implemented by T-042.
Every other candidate surface was inspected and none names a rename spelling.

| # | Surface | Inspected | Naming a rename spelling | Checked by |
|---|---|---|---|---|
| 1 | `ragent.json` keys | 339 field declarations and serde `rename`/`alias` values across the 24 sources of `crates/ragent-config` | 1, exempted with a written reason (below) | `test_no_config_field_name_is_a_rename_spelling` |
| 2 | environment variables | 67 reads over 35 distinct names in non-test production source | 0 | `test_no_environment_variable_is_named_a_rename_spelling` |
| 3 | the resolution path itself | `tool/alias_table.rs` and `tool/rename.rs` | 0 - neither reads an environment nor loads a config | `test_tool_name_resolution_reads_no_environment_and_no_config_file` |
| 4 | CLI flags, aliases and subcommands | 79 names in `src/main.rs` and `src/cli.rs` | 0 | `test_no_cli_flag_or_subcommand_is_a_rename_spelling` |
| 5 | the `tools` section's key set | 3 keys | 3, all non-tool-name and pinned | `test_tools_section_has_no_fourth_key_that_could_name_a_tool` |
| 6 | plugin, connector, agent, hook and skill settings | 387 settings across 28 files | 0 | `test_no_dynamic_source_setting_is_named_a_rename_spelling` |
| 7 | the built-in alias table | 4 rows | 0 | `test_a_retired_spelling_never_resolves_through_the_builtin_table` |

## What "named" means

Every predicate compares an **identifier** - a field name, a variable name, a
flag name - against the rename spellings as a **whole name**, for both columns of
every FR-023 pair. Both columns, because a mechanism *called* `Bash` would be an
equally effective alias for `bash`.

A substring rule would be wrong in both directions, and the tree contains both
counter-examples:

- `GIT_BASH` (`crates/ragent-tools-core/src/bash.rs:102`) contains `BASH`, but it
  is the Windows Git Bash install path, not a switch. It reads a *path*, and the
  path is used to locate an executable.
- `bash.allowlist` / `bash.denylist` and `edit_log` / `skill_dirs` contain a
  rename spelling as a word-fragment or a second segment without being the
  spelling itself.

A mechanism that restored a spelling would have to be called the spelling, or it
would not be the compatibility switch FR-031 forbids.

## The one exemption, and why it is not an escape hatch

```text
crates/ragent-config/src/config.rs:96    pub bash: BashConfig,
```

`bash` here is the **shell-command allowlist/denylist section** of
`ragent.json` (`bash.allowlist`, `bash.denylist`), named for the `bash:execute`
permission family that FR-028 keeps unchanged across the rename. It configures
which shell commands may run; it names no tool, registers no tool and resolves no
tool name. Deleting or renaming it would be a breaking config change unrelated to
the rename, and FR-028 forbids exactly that for permission categories.

The exemption lives in one reviewed list (`EXEMPTIONS` in the test) with the
reason written beside it, and `test_escape_hatch_exemptions_are_few_and_each_names_a_rename_spelling`
fails when the list grows, when an entry names a spelling that is not in the
FR-023 table (stale), when a reason is blank, or when the file it names is gone.

## Surface-by-surface detail

### 1. `ragent.json` keys

The scan covers every `struct`/`enum` **body** in `crates/ragent-config/src`, not
just `pub` fields and not every `name:` line. That distinction is load-bearing in
both directions:

- a **private** deserialised field is still a config key, so restricting the scan
  to `pub` would have missed a `bash: bool` on a private helper struct;
- a **struct literal** (`bash: BashConfig::default()`) and a **match arm**
  (`bash: Permission::Bash`) look identical to a field declaration, and scanning
  every `name:` line reports both as false positives.

Three probes proved the scan rather than the prose:

| Probe | Reported |
|---|---|
| a private `write: bool` on `ToolsConfig` | `crates/ragent-config/src/tools.rs:69 declares write` |
| an `env::var_os("write")` in `mcp/npx_cache.rs` | `crates/ragent-agent/src/mcp/npx_cache.rs:80 reads write` |
| a private `Skill: Option<String>` on the private `CodexManifest` | `crates/ragent-plugins/src/manifest.rs:410 declares Skill` |

The third probe is why the config scan was widened from `pub` fields to struct
and enum bodies: the first draft read only `pub` fields and let that private
manifest setting through. All three edits were reverted, and `git status` on the
three files is clean.

Serde `rename`/`alias` values are scanned too: they are a second accepted JSON
spelling for one field, so `#[serde(alias = "bash")]` would be an escape hatch
while the field itself was innocently named.

No CLI or config mechanism routes through a deprecated key: `Config` sets no
`deny_unknown_fields`, so an unknown key is *ignored* rather than honoured - which
is the correct FR-031 outcome, because an ignored key cannot restore anything. The
existing `serde(alias = "defaultAgent")` and the `mcp_servers`/`mcpServers` plugin
aliases are case/dialect spellings of non-tool keys.

### 2. Environment variables

67 reads over 35 distinct names. Every one was inspected; none is a rename
spelling. The nearest neighbours and why they are not switches:

| Variable | Why it is not an escape hatch |
|---|---|
| `GIT_BASH`, `LOCALAPPDATA`, `PATHEXT`, `PATH` | a filesystem location used to find an executable |
| `RAGENT_TOOL_NAME`, `RAGENT_TOOL_INPUT`, `RAGENT_TOOL_OUTPUT`, `RAGENT_TOOL_SUCCESS` | **written** into a hook's child process (`hooks/mod.rs:621-622, 883-886`), never read by ragent |
| `RAGENT_CONFIG`, `RAGENT_CONFIG_CONTENT`, `XDG_CONFIG_HOME`, `HOME` | config *file* locations and overrides, not tool names |
| `RAGENT_TOKEN`, `RAGENT_GITHUB_CLIENT_ID`, `RAGENT_UPDATE_SHA256`, `RAGENT_MCP_CONNECT_TIMEOUT_SECS` | a server token, a client id, an update digest and a timeout |
| `AWS_*`, `GITHUB_*`, `GITLAB_*`, `OLLAMA_*`, `OPENALEX_EMAIL`, `OPENROUTER_API_KEY`, `USER`, `USERNAME`, `HOSTNAME`, `RUST_BACKTRACE`, `npm_config_cache` | provider credentials, identity and transport settings |

The predicate fails if the scan finds **zero** reads, so deleting the reads
cannot satisfy it, and the reader-helper list is explicit so a new helper cannot
route a read around the check.

`RAGENT_TOOL_ALIAS` - the variable TC-029's step 11 uses to probe for a switch -
is read nowhere. Empirically: `ragent --dry-run --json` is byte-identical with and
without `RAGENT_TOOL_ALIAS=bash=Bash` set (identical SHA-256, with `--model`
pinned so the run is deterministic).

### 3. The resolution path

`tool/alias_table.rs` (the table and `resolve_inbound_tool_name`) and
`tool/rename.rs` (the FR-023 table) contain no `std::env`, no `env::var`, no
`read_env`, no `Config::load` and no `load_config`. Both take their inputs as
arguments - the registry and an `AliasesConfig` value the caller already holds -
so neither an environment variable nor a config file can change which spellings
resolve. This also protects NFR-006: resolution stays a short linear scan with
no per-step I/O.

### 4. CLI flags

79 names inspected: 2 explicit `long = "…"` names, 2 `alias`/`visible_alias`
values (`no-prompt`, `web-time`), 6 `#[command(name = "…")]` subcommands and the
field-derived flags of every bare `#[arg(long)]`. A bare `long` is the interesting
case, because clap derives the flag from the field name below it - `bash: bool`
would silently become `--bash`, which is exactly the loud compatibility flag
FR-031 forbids. The test resolves that derivation the way clap does (`_` to `-`).

Verified by adding `#[arg(long, global = true)] bash: bool` to `Cli`; the test
reported `src/main.rs:87 derives the flag --bash`. Reverted.

`--tool-alias` is not a flag. The parser rejects it and exits **2**:

```text
error: unexpected argument '--tool-alias' found

  tip: to pass '--tool-alias' as a value, use '-- --tool-alias'

Usage: ragent run [OPTIONS] <PROMPT>
```

The flag failing loudly and the variable being ignored are different results, and
both are recorded here because a reader rebuilding the audit needs to know which
is which.

### 5. The `tools` section

Exactly `aliases`, `expose_alias_names` and `wire_trace`, pinned by asserting the
serialised key set of a fully populated section. This matters because a fourth key
in `tools` could only be one that names a tool - FR-031 forbids adding one, and
`tools.aliases` is the single key that can carry a tool name (with T-042's
per-entry rejection).

### 6. Dynamic-source settings

387 settings across 28 files: the plugin manifest shapes (Codex, Claude and the
host-API declarations), the OASF agent manifest and Markdown profile frontmatter,
the hook configuration, the skill frontmatter, and every connector source. Zero
collisions.

Three of these deserve a written reason, which they get in the test's module doc:

- **`allowed_tools` / `allowed-tools` restrict, never resolve.** A list entry that
  names no registered tool simply matches nothing, so it cannot make a retired
  spelling resolvable. FR-029 covers the exact-resolution rule for Claude
  spellings.
- **A hook `matcher` is compared against an already-resolved name**, so it can only
  fail to match. FR-028 requires such a hook to be reported at load rather than
  honoured.
- **Source identifiers are namespaced.** A plugin id, connector id, MCP server id,
  backend id, ACP agent id or `agent.<name>` key names a *source*, and every tool
  such a source contributes is namespaced (`plugin_<id>_<tool>`,
  `mcp_<server>_<tool>`), so no id can occupy a bare ragent name. That refusal is
  FR-032's, and it is checked there.
- **`agent.options` and a provider `options` map are free-form, but read from a
  closed key set** - `acp` on an agent, and `model_id`, `auto_start`, `device`,
  `models_path`, `profile`, `region`, `reasoning_level` and the legacy `thinking`
  on a provider - so a tool name placed there is never read.

### 7. The built-in alias table

The one table a config can switch on without writing an entry
(`tools.aliases: true`), so the one place ragent's own source could reintroduce a
retired spelling. All four rows target a registered tool and no row's spelling is
a rename spelling; the test asserts every `before` resolves to `None` through the
table on the default registry.

## What was added: nothing

No configuration key, no environment variable, no CLI flag, no plugin/connector/
agent setting was added by this task. The change is one test file, one test
registration, this document and a changelog entry. The workspace's key, variable
and flag vocabulary is unchanged, which is what FR-031's "shall not add any new
key, variable or flag" asks for.

## Verification

- `cargo test -p ragent-agent --test all test_escape_hatch_audit::` - 8 passed.
- Each predicate was proved to fire, one violation at a time, and every edit was
  reverted: a config field named `write` (`crates/ragent-config/src/tools.rs:69`),
  an environment read named `write` (`crates/ragent-agent/src/mcp/npx_cache.rs:80`),
  a derived `--bash` CLI flag (`src/main.rs:87`), and a manifest setting named
  `Skill`. The stale-exemption branch was proved by emptying `EXEMPTIONS`, which
  reported `crates/ragent-config/src/config.rs:96 declares bash`.
- `cargo fmt --check` clean; `cargo clippy -p ragent-agent --all-targets` shows no
  new warning.
- No test writes into the repository: every file the predicates read is opened
  read-only, and the scratch trees the CLI probes used lived under
  `target/temp/` and were deleted.
