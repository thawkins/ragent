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
