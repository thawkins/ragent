# Implementation Plan: Connector System - Claude-Connector-Equivalent Integrations over MCP

**Spec:** [SPEC.md](SPEC.md) · **Test plan:** [TESTPLAN.md](TESTPLAN.md)

## Approach

Add one new crate, `crates/ragent-connectors`, that owns the connector descriptor model, the
connector store and ledger, the catalogue providers, and the session-start bridge into the
existing MCP client. Surface it through three integration points, mirroring the plugin
system's structure so the two subsystems read the same way:

1. **`ragent-connectors` crate** - `descriptor` (the normalised connector model and its
   validation), `store` (paths, scan, `_state.json` ledger, install/remove with
   path-traversal-safe archive extraction), `catalogue` (HTTPS index fetch, byte cap,
   timeout, cache, the compiled default endpoint constant, and the endpoint resolver),
   `provider` (the per-catalogue normalisation strategy), `auth` (auth-shape handling over
   the encrypted credential store), `bridge` (resolve enabled connectors into
   `(bridged_server_id, McpServerConfig)` pairs), and `report`/`help` (shared wording).
2. **MCP integration** - the bridge produces `McpServerConfig` values that the session feeds
   to the existing `McpClient`; enablement is consulted against the durable
   `mcp_state.json` ledger so a connector disabled once stays disabled everywhere. No new
   transport is written: every connector resolves to stdio, SSE, or HTTP exactly as a
   configured MCP server does.
3. **Command surface** - a `/connectors` slash-command family in
   `crates/ragent-tui/src/app/slash.rs` with a `crates/ragent-tui/src/app/connector.rs`
   glue module, registered in `SLASH_COMMANDS` in `crates/ragent-tui/src/app/state.rs` and
   documented in `/connectors help`; a `ragent connectors` CLI parity surface in
   `src/cli.rs`. Configuration gains a `connectors` block in `ragent-config`.

The store, ledger, provider, and fetch machinery deliberately reuse the shapes proven in
`ragent-plugins` (`StoreDirs`, `StoreLedger`, `StoreProvider`, `store_fetch`,
`ScratchSurface`): where a helper is already public, the connector crate calls it rather
than re-deriving it, so the two subsystems cannot drift. The compiled default Claude
connector-catalogue endpoint and its resolver mirror the plugin store's
`DEFAULT_CLAUDE_STORE_URL` / `effective_endpoint_with_source` discipline so the two
default-endpoint regimes read the same way.

### Reuse decisions

- The durable enable ledger is the **existing** `ragent_agent::mcp::enable_state`
  (`mcp_state.json`); the connector crate adds no second enable file.
- Catalogue indices parse through a `ConnectorProvider` trait shaped like
  `ragent_plugins::store_provider::StoreProvider` (tolerant per-entry-skip, `skipped`
  counter) so the browser domain stays catalogue-agnostic.
- Bridged server ids are `<connector-id>.<server>`, the same collision-avoidance shape the
  plugin MCP bridge uses (spec `plugins` FR-030).
- Secrets live in the existing encrypted credential store; the connector manifest stores
  only the credential *name* (FR-005).
- The compiled default Claude connector-catalogue endpoint and its provenance resolver
  mirror `ragent_plugins::store_index` (`DEFAULT_CLAUDE_STORE_URL`,
  `effective_endpoint_with_source`, `EndpointSource`) and reuse that crate's
  absolute-`https`-with-host guard, so a default endpoint is guarded exactly like a
  configured one (FR-037).
- The category filter (FR-039..FR-041) is one shared predicate applied by the browser and
  by the `list`/`search` glue, so the browser and the textual reports can never disagree
  about which connectors a category shows.

## Requirement Coverage Map

| Requirement | Task(s) |
| ----------- | ------- |
| FR-001 Connector store discovery | T-003 |
| FR-002 Normalised descriptor | T-002 |
| FR-003 Bridge to `McpServerConfig` and registry ids | T-006 |
| FR-004 `/connectors` command family | T-010, T-011, T-012 |
| FR-005 Secrets in credential store | T-007 |
| FR-006 Attribution and ASCII reports | T-010, T-014 |
| FR-007 `connectors` config block | T-001 |
| FR-008 Session-start loading | T-008 |
| FR-009 `/connectors list` | T-011 |
| FR-010 `/connectors search` | T-011 |
| FR-011 `/connectors add` install (disabled) | T-004, T-009 |
| FR-012 `/connectors enable` | T-008, T-011 |
| FR-013 `/connectors disable` | T-008, T-011 |
| FR-014 `/connectors auth` | T-007, T-011 |
| FR-015 `/connectors test` | T-013 |
| FR-016 Connection/tool failure containment | T-006, T-008, T-013 |
| FR-017 `/connectors help` and usage | T-012 |
| FR-018 Disabled-connector inertness | T-008, T-011 |
| FR-019 connect/disconnect without restart | T-008, T-011 |
| FR-020 Tool surfacing under `mcp_<server>_<tool>` | T-006, T-008 |
| FR-021 Master switch `connectors.enabled` | T-001, T-011 |
| FR-022 `needs auth` reporting and refusal | T-007, T-011 |
| FR-023 Credential requirement surfacing | T-007, T-010 |
| FR-024 Per-catalogue endpoint override | T-001, T-005 |
| FR-025 Unsupported-capability reporting | T-002, T-005, T-011 |
| FR-026 Multi-server connectors | T-006, T-008 |
| FR-027 Id-collision refusal | T-004, T-009 |
| FR-028 Non-https refusal | T-009 |
| FR-029 Archive path-traversal refusal | T-009 |
| FR-030 Remove-while-enabled refusal | T-009 |
| FR-031 Malformed/oversize catalogue refusal | T-005 |
| FR-032 Auth-failure state and no retry loop | T-007, T-008 |
| FR-033 Server-id collision refusal | T-006, T-008 |
| FR-034 Compiled default Claude catalogue endpoint | T-016 |
| FR-035 Default endpoint resolution and override precedence | T-016, T-017 |
| FR-036 `/connectors stores` provenance tagging | T-018 |
| FR-037 `https` guard on the default and configured endpoints | T-016, T-017 |
| FR-038 Single source of the default-endpoint literal | T-016, T-019 |
| FR-039 Category filter restricts displayed connectors | T-020, T-021 |
| FR-040 `ALL` category resets the filter (default) | T-020, T-021 |
| FR-041 `--category` argument on list/search | T-021 |
| NFR-001 Default endpoint declared exactly once | T-019 |
| NFR-002 Endpoint resolved per launch, no rebuild | T-016, T-017 |
| NFR-003 Resolution is pure and offline | T-016 |
| CLI parity | T-014 |
| Fixtures and acceptance walk | T-015 |

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Add `connectors` configuration block to `ragent-config` | FR-007, FR-021, FR-024 | S | High | completed | — |
| T-002 | Define `ConnectorDescriptor` model and validation rules | FR-002, FR-025 | M | Critical | completed | — |
| T-003 | Implement connector store paths, scan, and `_state.json` ledger | FR-001 | M | Critical | completed | T-002 |
| T-004 | Implement connector manifest read/write and install staging | FR-002, FR-011, FR-027 | M | Critical | completed | T-002 |
| T-005 | Implement catalogue fetch, cache, and normalisation providers | FR-024, FR-025, FR-031 | L | High | completed | T-001, T-002 |
| T-006 | Implement the bridge: connectors to `McpServerConfig` and registry ids | FR-003, FR-020, FR-026, FR-033 | M | Critical | completed | T-002, T-003 |
| T-007 | Implement auth-shape handling over the encrypted credential store | FR-005, FR-014, FR-022, FR-023, FR-032 | L | High | completed | T-001, T-002 |
| T-008 | Wire session-start load, connect, and disconnect lifecycle | FR-008, FR-012, FR-013, FR-018, FR-019, FR-032, FR-033 | L | Critical | completed | T-006, T-007 |
| T-009 | Implement install/remove command operations with refusal guards | FR-011, FR-027, FR-028, FR-029, FR-030 | M | High | completed | T-004 |
| T-010 | Add `/connectors` command parsing, dispatch, and registration | FR-004, FR-006, FR-023 | M | High | completed | T-008 |
| T-011 | Implement `list`, `search`, `enable`, `disable`, `connect`, `disconnect`, `auth` glue | FR-009, FR-010, FR-012, FR-013, FR-014, FR-018, FR-019, FR-022, FR-025 | L | Critical | completed | T-010 |
| T-012 | Implement `/connectors help` usage text and autocomplete metadata | FR-004, FR-017 | S | Medium | completed | T-010 |
| T-013 | Implement `/connectors test` isolated connect-and-invoke harness | FR-015, FR-016 | M | High | completed | T-008 |
| T-014 | Add `ragent connectors` CLI parity surface | FR-004, FR-006 | M | Medium | completed | T-011 |
| T-015 | Fixtures, acceptance walk, and quality gates | Acceptance criteria 1-10 | M | High | completed | T-001, T-002, T-003, T-004, T-005, T-006, T-007, T-008, T-009, T-010, T-011, T-012, T-013, T-014 |
| T-016 | Add compiled default Claude connector-catalogue endpoint constant and resolver | FR-034, FR-035, FR-037, FR-038, NFR-002, NFR-003 | M | High | completed | T-001 |
| T-017 | Wire default endpoint resolution into catalogue fetch and store commands | FR-035, FR-037, NFR-002 | S | High | completed | T-005, T-016 |
| T-018 | Extend `/connectors stores` to tag endpoint provenance (`default`/`config`) | FR-036 | S | Medium | completed | T-016, T-017 |
| T-019 | Add guard check and test for the single default-endpoint literal | FR-038, NFR-001 | S | Medium | completed | T-016 |
| T-020 | Add category filter state and category enumeration to the connector browser | FR-039, FR-040 | M | High | completed | T-002, T-003 |
| T-021 | Wire the category filter into `list`/`search` reports and the `--category` argument | FR-039, FR-040, FR-041 | M | High | completed | T-011, T-020 |
## Task Details

### T-001 - Add `connectors` configuration block to `ragent-config`

Add a `ConnectorsConfig` struct (master `enabled` defaulting true, optional `store_dir`,
`stores` endpoints and shared budgets, `credentials` name map) beside the existing
`PluginsConfig` in `crates/ragent-config/src/connectors.rs`, with `is_enabled()` and
`stores_or_default()` helpers mirroring `PluginsConfig`. Add the optional field to the
top-level `Config`. `connectors.enabled: false` makes the subsystem inert (FR-021).

**Verify:** a partial `connectors` block overlays the compiled defaults; an absent block
leaves the subsystem enabled with default endpoints.

### T-002 - Define `ConnectorDescriptor` model and validation rules

Define the descriptor (id, name, description, category, tags, source, provenance, auth
shape, auth scopes, credential name, server list, unsupported labels) and its validation:
a connector id is a non-empty single path component; at least one server must be declared;
each server's transport must be representable as `McpServerConfig`. Record unexpressible
capabilities as `unsupported` labels rather than dropping them (FR-025). This module owns
the id-collision and server-id-collision predicates used later (FR-027, FR-033). The
`category` field is the value the category filter compares against (FR-039).

**Verify:** a descriptor with no servers and one with an unknown transport both fail
validation with a named reason; an unexpressible capability is retained as a label.

### T-003 - Implement connector store paths, scan, and `_state.json` ledger

Add store paths (`<workdir>/.ragent/connectors/`, global `~/.config/ragent/connectors/`),
a symlink-safe directory walk, and a per-store `_state.json` ledger holding per-connector
`enabled` and telemetry counters. Project-local connectors win an id collision. Scanning
reads manifests only and never connects (FR-001). Model the module on
`ragent_plugins::store` so the two ledgers are recognisably the same shape.

**Verify:** a connector placed only under the global store is discovered when no project
copy exists; a project copy shadows the global one; a symlink escaping the store is skipped.

### T-004 - Implement connector manifest read/write and install staging

Define the on-disk connector manifest format (the serialised descriptor minus secrets) and
the read/write helpers. Add the staging routine that copies or expands an install source
into the store under the connector id, refusing when the id already exists unless `--force`
(FR-027). No MCP connection is made at any point in this task (FR-011).

**Verify:** staging a local directory produces a manifest in the store; a second staging of
the same id is refused; `--force` overwrites.

### T-005 - Implement catalogue fetch, cache, and normalisation providers

Add the HTTPS catalogue fetch with the configured timeout and byte cap, a `cache_ttl_secs`
cache, and a `ConnectorProvider` trait with `parse_index` implemented for the shipped
catalogue shape. A malformed document, an unrecognised shape, or an oversize body aborts the
fetch with a named reason and leaves the cache untouched (FR-031). An entry that cannot be
normalised is skipped and counted, not fatal (FR-025). Per-catalogue endpoint overrides
resolve before each fetch (FR-024). Reuse the tolerant discipline of
`ragent_plugins::store_provider`.

**Verify:** a two-entry catalogue with one unexpressible entry returns one connector and a
`skipped` count of one; an oversize body aborts without replacing the cache.

### T-006 - Implement the bridge: connectors to `McpServerConfig` and registry ids

Resolve each enabled connector's server definitions into `(bridged_server_id,
McpServerConfig)` pairs with `bridged_server_id = <connector-id>.<server>` (FR-003, FR-026).
Reject a bridged id that collides with an existing `ragent.json` `mcp` key or with another
connector's bridged id (FR-033). The bridge is pure over the store and ledger and performs
no connection. Shape the module on `ragent_plugins::bridge::scanned_plugin_mcp_servers`.

**Verify:** a two-server connector yields two pairs with `<id>.` prefixes; a bridged id
colliding with a configured `mcp` key is rejected.

### T-007 - Implement auth-shape handling over the encrypted credential store

Implement the five auth shapes (none, env, token, oauth, password). A secret value is
written to the encrypted credential store under the configured name and never into a
manifest or `ragent.json` (FR-005). Report the auth state without echoing the secret
(FR-014). Surface a catalogue entry's declared credential requirement (FR-023). A connector
with an auth shape and no valid credential reports `needs auth` and refuses to connect
(FR-022); a rejected or expired credential records `auth failed` and does not retry in a
loop (FR-032).

**Verify:** storing a token encrypts it and reports a non-secret state line; a connector
with no stored token reports `needs auth` and refuses to connect.

### T-008 - Wire session-start load, connect, and disconnect lifecycle

On session start, load each enabled connector, consult the durable `mcp_state.json` ledger,
connect each bridged server through the existing `McpClient`, register tools under
`mcp_<server>_<tool>`, and record state `connected` or `errored` with the cause (FR-008,
FR-020). Provide the connect/disconnect-without-restart entry points (FR-019) and the
enable/disable transitions that register and deregister exactly the contributed tools
(FR-012, FR-013). A disabled connector is fully inert (FR-018). A connect failure records
`errored` and leaves the session running (FR-016); per-server state is independent for a
multi-server connector (FR-026); a server-id collision is refused (FR-033); an auth failure
records `auth failed` and stops (FR-032).

**Verify:** enabling a connector surfaces its tools; disabling deregisters exactly those
tools; a failing server on a two-server connector still reports the other as connected.

### T-009 - Implement install/remove command operations with refusal guards

Add the install and remove operations behind the command glue: install from a catalogue id,
a local directory, a local `.zip`/`.tar.gz`, or an `https://` URL (FR-011); refuse a
non-`https` scheme (FR-028); refuse an archive whose entries escape the store (FR-029);
refuse removal while enabled (FR-030); refuse an id collision without `--force` (FR-027).
Reuse the path-traversal-safe extraction approach from the plugin store.

**Verify:** a `../../escape` archive entry is refused with no write outside the store; a
removal of an enabled connector changes nothing and instructs the user to disable first.

### T-010 - Add `/connectors` command parsing, dispatch, and registration

Add the `connectors` trigger to `SLASH_COMMANDS`, the dispatch arm in `app/slash.rs`, and a
`crates/ragent-tui/src/app/connector.rs` glue module that parses the subcommand and forwards
to the shared entry points. Reports carry the `From: /connectors <sub>` attribution and are
ASCII-only (FR-006). A bare `/connectors` and an unknown subcommand fall through to the
usage block. A `connectors.enabled: false` subsystem reports as disabled (FR-021). The
parser accepts the optional `--category <name>` argument on `list` and `search` (FR-041,
completed in T-021).

**Verify:** `/connectors` appears in the autocomplete menu and dispatches; a disabled
subsystem reports that fact for every non-help subcommand.

### T-011 - Implement `list`, `search`, `enable`, `disable`, `connect`, `disconnect`, `auth` glue

Implement the management subcommands: `list` prints one row per connector (id, name,
category, state, auth state, server and tool counts) plus a summary and any unsupported
labels (FR-009, FR-025); `search` queries the catalogue and prints matches, reporting an
empty set as a message (FR-010); `enable`/`disable` drive the lifecycle (FR-012, FR-013,
FR-018); `connect`/`disconnect` toggle the connection without a restart (FR-019); `auth`
drives T-007 and reports `needs auth` where applicable (FR-014, FR-022). Malformed arguments
render an `[err]` row that changes no state. Keep the parse-and-run glue in one place so the
TUI and CLI share one wording.

**Verify:** each subcommand returns the documented report for a fixture connector; a missing
argument renders an `[err]` row and changes no state.

### T-012 - Implement `/connectors help` usage text and autocomplete metadata

Add the usage block documenting every subcommand, its arguments, and the accepted `<source>`
forms, plus the subcommand token list the autocomplete menu is seeded from. Rendering help
is a pure string function with no store access and creates no files (FR-017). Model the
module on `ragent_plugins::help`.

**Verify:** `/connectors help`, a bare `/connectors`, and an unknown subcommand all print the
same usage block and create no files.

### T-013 - Implement `/connectors test` isolated connect-and-invoke harness

Connect the connector's servers in isolation, invoke one advertised tool once with
schema-valid sample arguments generated from the tool's JSON schema, report per-step
`[ ok ]`/`[fail]` results with wall-clock connect time, then disconnect without touching the
live session (FR-015). A connect or tool failure is reported per-step and never panics
(FR-016). Model the harness on `ragent_plugins::harness`.

**Verify:** a healthy connector reports `[ ok ]` connect and tool steps; a connector whose
server fails to start reports `[fail]` on the connect step and the live session is unchanged.

### T-014 - Add `ragent connectors` CLI parity surface

Add the `connectors` subcommand to the CLI dispatcher so `ragent connectors <sub>` prints
the same wording as the TUI family (FR-004, FR-006), reusing the shared parse-and-run glue
from T-010/T-011 and honouring `--category` where the TUI does (FR-041). Model the parity on
the `ragent plugins` CLI surface.

**Verify:** `ragent connectors help` and the TUI `/connectors help` print identical usage
text; `ragent connectors list --category <name>` prints the same rows as the TUI.

### T-015 - Fixtures, acceptance walk, and quality gates

Add fixture connectors and a fixture catalogue covering: a healthy stdio connector, a
two-server connector, a connector with an auth shape, a connector with an unexpressible
entry, and a connector whose server fails to start. Give the fixtures distinct `category`
values so the category filter is walkable. Walk acceptance criteria 1-10 manually and record
results. Run the workspace gates: `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets`, `cargo test --workspace`, and the shared guard
scripts.

**Verify:** every acceptance criterion is observed in a running session; all gates pass with
zero warnings.

**Delivered fixture inventory** - all under `assets/connectors/fixtures/`, the same
artifacts the manual TESTPLAN walk stages:

| Fixture | Cover | Category | Servers |
| ------- | ----- | -------- | ------- |
| `echo/connector.json` | healthy stdio connector | `productivity` | 1 |
| `two-server/connector.json` | multi-server connector (`alpha`, `beta`) | `developer` | 2 |
| `needs-token/connector.json` | `token` auth shape, credential `NEEDS_TOKEN_VALUE` | `data` | 1 |
| `needs-env/connector.json` | `env` auth shape, credential `ECHO_TOKEN` | `data` | 1 |
| `bad-server/connector.json` | command absent, so the connect step fails | `developer` | 1 |
| `unsupported/connector.json` | `grpc` transport, unexpressible label | `developer` | 1 |
| `duplicate/connector.json` | re-declares the id `echo` (id-collision refusal) | `data` | 1 |
| `stores/index.json` | 3-entry fixture catalogue (`echo`, `unsupported`, `github`) | - | - |
| `stores/index-bad.json` | truncated JSON (malformed-document case) | - | - |
| `stores/index-big.json` | >128 KiB document (oversize-body case) | - | - |
| `archives/echo.zip` | the `echo` fixture packaged | - | - |
| `archives/needs-token.tar.gz` | the `needs-token` fixture packaged | - | - |
| `archives/escape.zip` | one entry whose path escapes the store | - | - |

The acceptance walk is automated in
`crates/ragent-connectors/tests/test_connector_fixtures.rs` (18 tests: fixture inventory,
acceptance criteria 1-10, TC-003/TC-010/TC-011/TC-012, and the TC-024 `--category` case), so
a regression in a fixture fails the same walk the manual plan describes. The manual walk
follows TESTPLAN.md Prerequisites 4-12, which stage these fixtures under
`~/scratch/connectors-tests/` and point the catalogue config at `stores/index.json`.

### T-016 - Add compiled default Claude connector-catalogue endpoint constant and resolver

Add a single public constant for the compiled default Claude connector-catalogue endpoint
(FR-034, FR-038) plus a resolver that returns the effective endpoint *and* its provenance:
a non-empty `connectors.stores.claude.url` override is `config`, an absent, empty, or
whitespace-only override falls back to the compiled default as `default`, with no error
(FR-035). The chosen value is parsed through the same absolute-`https`-with-host guard the
plugin store uses (`StoreEndpoint`), so both the default and configured paths are guarded
(FR-037). Resolution is a pure offline function called once per launch (NFR-002, NFR-003).
Model the module on `ragent_plugins::store_index` so the two default-endpoint disciplines
read the same way.

**Verify:** with no `connectors` block the resolver reports the compiled default with
provenance `default` and performs no network access; a configured non-`https` override is
refused with a named scheme error.

### T-017 - Wire default endpoint resolution into catalogue fetch and store commands

Resolve the Claude catalogue endpoint (T-016) at the catalogue-fetch call site and in the
`/connectors stores` handler, replacing any hard-coded or duplicated endpoint literal
(FR-035, FR-038). An absent or blank override keeps the compiled default active; a
non-`https` value is refused before any fetch is attempted (FR-037). Resolution happens per
launch, so a config edit takes effect without a rebuild (NFR-002).

**Verify:** a fresh install with no `connectors.stores` block fetches the compiled default
endpoint; clearing a previously configured override restores the default on the next
launch.

### T-018 - Extend `/connectors stores` to tag endpoint provenance (`default`/`config`)

Extend the `/connectors stores` report so each catalogue row prints its endpoint and a
provenance tag: `default` when the compiled default is in effect, `config` when a non-empty
override is in effect (FR-036). Reuse the provenance value returned by the T-016 resolver so
the tag can never disagree with the endpoint actually contacted. `--check` continues to
probe the endpoint it reports.

**Verify:** `/connectors stores` on a fresh install tags the Claude endpoint `default`; after
setting `connectors.stores.claude.url` it tags that row `config` and shows the override.

### T-019 - Add guard check and test for the single default-endpoint literal

Add a test and a guard check proving the compiled default Claude connector-catalogue endpoint
is declared exactly once and that no other module carries a second copy of the literal
(FR-038, NFR-001). The check fails when a duplicated default-endpoint literal appears,
mirroring the plugin-store default-endpoint guard.

**Verify:** the guard passes on the current tree; introducing a second copy of the endpoint
literal makes the guard and the test fail with the duplicated location named.

### T-020 - Add category filter state and category enumeration to the connector browser

Add a category-filter state to the connector browser: the distinct category set is derived
from the loaded connector catalogue plus the installed connectors, and a sentinel `ALL`
entry that represents "no filter" (FR-040). While a filter is active the browser displays
only connectors whose declared category matches the selected category (FR-039). `ALL` is
the default filter on open, so a browser with no selection shows every connector. Category
comparison is exact and case-insensitive against the descriptor's `category` field (T-002);
a connector with an empty category is visible only under `ALL`.

**Verify:** selecting a category shows only that category's rows and the match count; the
`ALL` entry restores every row; a browser opened with no selection is on `ALL`.

### T-021 - Wire the category filter into `list`/`search` reports and the `--category` argument

Parse an optional `--category <name>` argument on `/connectors list` and
`/connectors search <query>` and apply the same filter used by the browser (FR-039, FR-041).
Each report states the active category (`ALL` when unfiltered) and the match count; a
category with no matching connectors prints an empty-result message rather than an error
(FR-041). `--category ALL` clears any filter and behaves identically to the unfiltered
report (FR-040). Malformed or unknown category values render an `[err]` row and change no
state, matching the family's error policy.

**Verify:** `list --category data` prints only data-category connectors and the active
filter; `list --category ALL` prints every connector; `search --category nosuch` prints the
empty-result message, not an error.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
| ---- | ---------- | ------ | ---------- |
| Connector and plugin MCP bridges double-register a server | Medium | High | Bridged ids are prefixed (`<owner>.<server>`); T-006 rejects a colliding id (FR-033). |
| OAuth flow complexity exceeds a single exchange | High | Medium | Ship authorization-code first; device-code deferred (Open Question 2); a token shape is always available as the fallback. |
| Catalogue shape varies by vendor | High | Medium | A `ConnectorProvider` per shape behind one trait; unexpressible entries are skipped and counted (FR-025). |
| Secrets leak into reports or manifests | Low | Critical | Secrets only ever enter the encrypted store; reports render a name or a state, never a value (FR-005, FR-014). |
| Archive extraction escapes the store | Low | Critical | Path-traversal-safe extraction with an explicit refusal case (FR-029) covered by a fixture. |
| Default catalogue endpoint drifts or is duplicated across modules | Medium | Medium | Single public constant (FR-038) with one resolver (T-016) and a guard check that fails on a duplicate literal (T-019, NFR-001). |
| A configured endpoint silently falls back to the default when malformed | Low | High | The default and configured paths share one guard; a malformed or non-`https` value is refused with a named reason rather than substituted (FR-037). |
| The browser and the text reports disagree about a category | Low | Medium | The category predicate is one shared function (T-020) that both the browser and the `list`/`search` glue (T-021) call. |

## Notes on Assumptions

- A1 (a connector wraps MCP servers plus metadata) keeps the session-visible tool mechanism
  unchanged; the connector layer adds catalogue, auth, and management only.
- A2/A3 (store and durable ledger patterned on the plugin system) means the connector store,
  its `_state.json`, and the global `mcp_state.json` never disagree about whether a server
  should start.
- A5 (secrets separate from configuration) is the only place the design chooses differently
  from a naive connector record, and it is what makes the reports safe to print.
- A4 + the default-endpoint section: the Claude connector catalogue ships a compiled default
  endpoint (FR-034) resolved through one connector-provider-friendly resolver (T-016), so a
  fresh install browses the standard Claude connector catalog with no configuration, exactly
  as the plugin browser has a compiled default Claude store endpoint.
- FR-035 names `connectors.stores.claude.url` as the override key, so the compiled default
  is the `claude` catalogue entry; any other catalogue (`community`) keeps its own compiled
  default when only `claude` is overridden (FR-024).
- Resolving the default endpoint is offline (NFR-003); the endpoint is contacted only when a
  catalogue fetch or `/connectors stores --check` explicitly requests it, so a launch with
  an unused catalogue makes no network call.
- The category filter (FR-039..FR-041) is text and browser surface only: it narrows what is
  shown, never what is discovered, enabled, or connected, so filtering cannot start or stop
  a server.