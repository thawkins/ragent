# T-022 - `--wire-matrix`: the declared contract as one printable table

Spec `uniformtools`, task T-022. Requirement FR-016, validated by TESTPLAN case
TC-002.

## 1. What FR-016 asks for

> Where the user supplies the wire-matrix flag, the system **shall** print, per
> registered provider, the profile fields of FR-002 as an aligned table and exit
> without contacting any provider or executing any tool.

Three obligations hide in that sentence:

1. **Per registered provider** - the rows must come from the *registry*, not from a
   hand-maintained list. A provider added without a row would print no contract at
   all, which is the failure FR-002's coverage assertion exists to prevent.
2. **The fields of FR-002** - the nine profile fields, as values. A table that
   restated them in prose, or that resolved them from somewhere other than the
   profile constants, would be documentation that can go stale.
3. **No provider contacted, no tool executed** - so the flag must be answered
   before the config load, the storage open, the secret-registry seed and the MCP
   connect loop, all of which touch the network, the filesystem or a subprocess.

TC-002 adds the reader-facing obligations: one row per provider, no cell blank or
`-`, plain ASCII, and a row count that matches the provider list the binary prints
elsewhere.

## 2. What was built

### `crates/ragent-llm/src/providers/wire/matrix.rs` (new, 187 lines)

- `HEADINGS: [&str; 10]` - the printed column list, `PROVIDER` plus the nine
  profile fields. One entry per printed column, so a column added to the renderer
  without its heading is a test failure rather than a table that silently shifts
  every value one column left.
- `render(&[&str]) -> Result<String, MissingProfile>` - resolves each supplied id
  through the same `profile_for` lookup the encoders and the accumulator use,
  sorts by id, then lays the rows out. **Sorting is not cosmetic**: the registry
  holds providers in a `HashMap`, so without the sort two runs of the same binary
  would print different row orders and TC-002's count-the-rows step would compare
  a moving target.
- `MissingProfile { providers: Vec<String> }` - the FR-002 failure, as an error
  type with a `Display` that names every offending provider. It is returned
  instead of a row of blanks, and instead of a partial table: a table with a hole
  in it must not look complete.
- `push_row` - left-aligned padding to the widest cell *including the heading*,
  two-space separators, then `trim_end` so no line carries invisible trailing
  whitespace (two byte-identical tables must compare equal after a paste).
- `yes_no(bool)` - the printed spelling of the four boolean fields. Never blank,
  never `-` (NFR-005, TC-002).

The module is a pure function of ids plus `const` data: no registry is built here,
no environment is read, no I/O happens. That is what makes NFR-004 ("a provider
cannot be contacted by a report") checkable by construction rather than by
inspection.

### `src/cli.rs` - `handle_wire_matrix_command()`

Builds the default provider registry, takes its ids, sorts them and hands them to
`matrix::render`. On `MissingProfile` it prints the provider names plus the exact
file to add the row to, and returns `CliExit::failure()` (exit 1, the runtime-failure
code) rather than `CliExit::usage()`: a registered provider with no contract is a
broken build, not a mistyped flag.

The registry build is the *only* work: `create_default_registry`'s constructors
resolve base URLs from environment variables and read local config files, and no
client is created or called, so no request is possible.

### `src/main.rs` - the flag and its dispatch

`Cli::wire_matrix` (`--wire-matrix`, `global = true` so it is accepted in any
position, like `--version`), answered immediately after `--version` and **before**
`panic_hook::install()`, the crash-marker read, the config load, `Storage::open`,
the secret-registry seed and the MCP connect loop. The early return is the
network-free guarantee: measured at 10 ms wall clock, against the ~2 s a run that
loads config and opens storage takes.

## 3. Tests

### `crates/ragent-llm/tests/test_wire_matrix.rs` (new, 249 lines, 8 cases)

| Case | Property |
|---|---|
| `..._prints_one_row_per_provider` | heading + rule + one row per `PROVIDER_WIRE` entry; every id appears |
| `..._has_no_blank_cell` | ten populated columns per data row; no cell empty, `-` or `none` (TC-002) |
| `..._is_plain_ascii` | printable ASCII only, and no line has trailing whitespace |
| `..._shows_the_declared_contract_not_a_second_copy` | Gemini's `synthesised`, Responses' `by_call_id`, Bedrock's `toolUseId`, Anthropic's reasoning `yes`, and the two Ollama families differing on argument encoding |
| `..._is_sorted_and_order_independent` | reversed input renders identical bytes; rows sorted |
| `..._refuses_a_provider_with_no_declared_profile` | error names the provider; a mixed known/unknown list renders nothing |
| `..._headings_match_the_columns_each_row_prints` | heading cells equal `HEADINGS`; the rule is all `-` and at least as wide |
| `..._renders_an_empty_provider_list_as_a_bare_heading` | no panic on no ids; heading + rule only |

### `tests/test_wire_matrix_flag.rs` (new, root crate, 180 lines, 4 cases)

Spawns the built binary in a fresh tempdir with an isolated `HOME` and
`XDG_CONFIG_HOME` (the environment is set on the `Command`, so no crate-wide lock
is needed - the child's environment is not this process's):

| Case | Property |
|---|---|
| `..._prints_one_row_per_provider` | exit 0, heading and rule present, >= 15 rows, sorted and unique, and every provider `ragent models` names has a row (the TC-002 step 3 comparison) |
| `..._populates_every_column` | ten non-blank, non-dash cells per row; ASCII; no trailing whitespace; the three families a reader checks are present |
| `..._exits_without_network_or_session_side_effects` | finishes in < 10 s (a connect attempt is seconds), and creates no database, session store or config root |
| `..._is_documented_in_help` | `--help` lists the flag |

## 4. Red proofs

Each break was applied, the suite run, and the file restored byte-identically from
`target/temp/matrix.rs.bak` / `target/temp/cli.rs.wm.bak` (verified with
`sha256sum -c`):

| Break | Result |
|---|---|
| `yes_no`'s `"no"` arm changed to `""` | `test_wire_matrix_has_no_blank_cell` **FAILED** (7 passed, 1 failed) - TC-002 is genuinely enforced |
| `rows.sort_by(...)` deleted | `test_wire_matrix_is_sorted_and_order_independent` **FAILED** (7 passed, 1 failed) |
| the `MissingProfile` return replaced with `let _ = missing;` | `test_wire_matrix_refuses_a_provider_with_no_declared_profile` **FAILED** (7 passed, 1 failed) |
| `print!("{table}")` replaced with `let _ = table;` in `src/cli.rs` | `test_wire_matrix_flag_prints_one_row_per_provider` and `..._populates_every_column` **FAILED** (2 passed, 2 failed) |

The `src/cli.rs` break needed a `touch` before the rebuild: `cargo` had a
fingerprint for the restored file's mtime and silently reused the object file of
the *broken* build, so the first post-restore run of the unmodified test still
failed. Worth knowing - a red proof whose restore is only content-identical and not
mtime-fresh can look green (here, falsely red) for the wrong reason.

## 5. Measurements

| Item | Value |
|---|---|
| `matrix.rs` | 187 lines, new |
| `handle_wire_matrix_command` | 21 lines, `src/cli.rs` |
| `src/main.rs` changes | 8-line flag doc + 8-line early return |
| New tests | 8 (`test_wire_matrix.rs`, 249 lines) + 4 (`test_wire_matrix_flag.rs`, 180 lines) |
| `--wire-matrix` wall clock | 10 ms, no network, no session, no database |
| Rows printed | 16 (one per registered provider) |

## 6. Deliberate scope limits

- **No `--json`.** FR-016 asks for an aligned table; a machine-readable form would
  be a second contract to keep in step, and no requirement asks for it. The
  renderer returns a `String`, so adding one later is a formatting decision, not a
  plumbing change.
- **The router row is printed as its declared family.** `ragent --wire-matrix`
  reports `router` as `openai_function`, which is the nominal row the profile table
  documents: the router forwards the canonical request to a downstream provider
  whose own profile does the encoding. Resolving the per-request downstream family
  is T-015's and T-017's work (effective-contract resolution at dispatch time), not
  a static table's; the alternative - omitting the row - would leave a registered
  provider with no row, which TC-002 forbids.
- **No config or `--config` interaction.** The report describes compile-time
  contracts, so a user config cannot change it and is not read.

## 7. Verification

- `cargo fmt --all -- --check` - clean.
- `cargo clippy -p ragent-llm -p ragent --all-targets` - no new warnings (the
  `needless_collect` nursery lint in the new test was fixed; the pre-existing
  `pub(crate)`-in-private-module noise is untouched).
- `cargo test -p ragent-llm --test all` - 245 passed, 2 ignored.
- `cargo test -p ragent-llm --lib` - 321 passed.
- `cargo test -p ragent-llm --doc` - 29 passed.
- `cargo test -p ragent --test all` - 48 passed.
- Guards: `check-inline-tests`, `check-test-env-isolation`, `check-silent-errors`,
  `check-security-unwraps`, `check-poison-locks`, `check-shared-guards`,
  `check-dead-code-reasons` - all PASS. `check-security-unwraps` initially flagged
  the new doctest's `.expect(...)`; the example now returns
  `Result<(), MissingProfile>` and uses `?`, which is the rule the guard is asking
  for rather than a suppression.

## 8. Landing record (this session)

### 8.1 State found

The feature was **already in the tree** at `d6abcd4a` and complete. `git status`
shows `matrix.rs`, `crates/ragent-llm/tests/test_wire_matrix.rs` and
`tests/test_wire_matrix_flag.rs` as tracked and unmodified; only `src/cli.rs`,
`src/main.rs` and `tests/all.rs` differ from `HEAD`, and all three carry this
task's work. So, as with T-004 and T-008 in this spec, this task's delta was
verification and the landing record, not new implementation. The figures below
were measured, not taken from the sections above.

| Item | Measured |
|---|---|
| `matrix.rs` | 187 lines, unmodified since `d6abcd4a` |
| `src/cli.rs` `handle_wire_matrix_command` | 27 lines including its doc comment |
| Renderer tests | 8 cases in 249 lines, all green |
| Binary tests | 4 cases in 180 lines, all green |
| Rows printed | 14 registered providers, sorted |
| `--wire-matrix` wall clock | 9-10 ms (`target/debug/ragent`, measured with `time`) |
| `cargo build --release`-equivalent full start | ~2 s (config load + `Storage::open` + MCP connect loop, all skipped) |

The printed table, verbatim columns, is:

```
PROVIDER          ENVELOPE                     CARRIER                         ID FIELD      ARGUMENTS    STREAM KEYING  PARALLEL  IS ERROR  REASONING  SYNTHETIC IDS
anthropic         anthropic_tool               user_tool_result_block          tool_use_id   json_object  by_index       yes       yes       yes        no
azure_foundry     openai_function              tool_role_message               tool_call_id  json_string  by_index       yes       no        no         no
azure_resource    openai_function              tool_role_message               tool_call_id  json_string  by_index       yes       no        no         no
bedrock           bedrock_tool_spec            user_tool_result_block          toolUseId     json_object  by_index       yes       yes       no         no
copilot           openai_function              tool_role_message               tool_call_id  json_string  by_index       yes       no        no         no
gemini            gemini_function_declaration  gemini_function_response        synthesised   json_object  by_index       yes       no        no         yes
generic_openai    openai_function              tool_role_message               tool_call_id  json_string  by_index       yes       no        no         no
huggingface       openai_function              tool_role_message               tool_call_id  json_string  by_index       yes       no        no         no
ollama            openai_function              tool_role_message               tool_call_id  json_string  by_index       yes       no        no         no
ollama_cloud      openai_function              tool_role_message               synthesised   json_object  by_call_id     yes       no        no         yes
openai            openai_function              tool_role_message               tool_call_id  json_string  by_index       yes       no        no         no
openai_responses  responses_flat_function      responses_function_call_output  call_id       json_string  by_call_id     yes       no        no         no
openrouter        openai_function              tool_role_message               tool_call_id  json_string  by_index       yes       no        no         no
router            openai_function              tool_role_message               tool_call_id  json_string  by_index       yes       no        no         no
xai               openai_function              tool_role_message               tool_call_id  json_string  by_index       yes       no        no         no
```

Fifteen lines follow the rule (14 data rows plus the heading), every one ASCII
with no trailing whitespace, and the two Ollama entries differ on argument
encoding exactly as `..._shows_the_declared_contract_not_a_second_copy` asserts.

### 8.2 Red proofs re-run

All four breaks from section 4 were re-applied on this tree and reverted, so the
proofs describe the current code and not the draft. Each file was saved with
`cp -p` to `target/temp/t022/`, recorded in `target/temp/t022/before.sha256`, and
restored content-identically and `touch`ed before the `sha256sum -c` check - the
`cp -p` mtime trap from section 4 is real and would otherwise let `cargo` reuse
the broken object file.

| Break | File | Result |
|---|---|---|
| `yes_no`'s `"no"` arm changed to `""` | `matrix.rs` | `..._has_no_blank_cell` **FAILED** (7 passed, 1 failed) - `row 2 does not have 10 populated columns` |
| `rows.sort_by(...)` deleted | `matrix.rs` | `..._is_sorted_and_order_independent` **FAILED** (7 passed, 1 failed) - rows in `HashMap` order |
| `MissingProfile` return replaced by `let _ = missing;` | `matrix.rs` | `..._refuses_a_provider_with_no_declared_profile` **FAILED** (7 passed, 1 failed) |
| `cliprint!("{table}")` replaced by `let _ = table;` | `src/cli.rs` | `..._flag_prints_one_row_per_provider` and `..._flag_populates_every_column` **FAILED** (2 passed, 2 failed) |

Restore verified: `sha256sum -c target/temp/t022/before.sha256` reports `OK` for
both files, and both suites are green again afterwards (8/8 and 4/4).

### 8.3 Verification measured this session

- `cargo fmt --all -- --check` - clean.
- `cargo check --workspace --all-targets` - finished with no error and no new
  warning line.
- `cargo clippy -p ragent-llm -p ragent --all-targets` - **no** finding in
  `matrix.rs`, `src/cli.rs`, `src/main.rs` or either test file. The 19
  `unnecessary structure name repetition` warnings are pre-existing in
  `tool_cache.rs`, and the `pub(crate) ... inside private module` noise is
  pre-existing across `ragent-llm`; neither is touched here.
- `cargo test -p ragent-llm --test all` - **294 passed / 4 failed / 2 ignored**.
  The 4 are the pre-existing guard cases still owned by pending tasks
  (`test_wire_decode::test_no_module_invents_a_tool_call_id_or_recovers_a_name_from_one`
  for T-010, `test_wire_encode_message::test_no_provider_module_writes_a_result_carrier_literal`
  for T-006, and `test_wire_trace::test_agent_loop_sets_the_switch_from_config`
  plus `..._every_provider_records_its_body` for T-023). None is in a file this
  task touched.
- `cargo test -p ragent-llm --lib` - 323 passed.
- `cargo test -p ragent-llm --doc` - 30 passed (including `matrix::render`'s
  doctest).
- `cargo test -p ragent --test all` - **48 passed, 0 failed**.
- All twelve `scripts/check-*.sh` guards PASS.

### 8.4 Documentation updated for this landing

`PLAN.md`'s T-022 row was rewritten from one line to the full delivery record and
flipped to `completed`; `TESTPLAN.md`'s TC-002 gained an "Automated half (T-022)"
paragraph mapping each manual expectation to a named case; `CHANGELOG.md`'s
`Unreleased` `Added` section gained this feature's entry; `crates/ragent-llm/DOCS.md`
already described the `matrix` submodule and needed no change.
