---
status: draft
audit:
  - { time: 1790828228, from: "none", to: "draft", actor: "system" }
---
# Specification: Connector System - Claude-Connector-Equivalent Integrations over MCP

## Overview

This specification defines a **connector system** for ragent: named, user-facing
integrations (Google Drive, Slack, GitHub, Git, Postgres, Puppeteer, and the wider
community catalogue) that reach external systems through the **Model Context Protocol
(MCP)**. A *connector* is the user-facing unit — a catalogue entry with a display name, a
category, an authentication requirement, and one or more MCP servers — while the MCP
server remains the transport ragent already speaks.

The capability being reproduced is the one Claude exposes as *connectors*: pick an
integration from a catalogue, authenticate it, and its tools appear in the session. In
ragent those tools already flow through [`McpClient`](../../crates/ragent-agent/src/mcp/mod.rs)
and the plugin `mcpServers` bridge
([`ragent_plugins::bridge`](../../crates/ragent-plugins/src/bridge.rs)), so the connector
system is a **management and catalogue layer** on top of machinery that exists.

The system is managed through a single slash-command family, `/connectors`, patterned
after the `/plugins` family (spec `plugins` FR-006):

| Subcommand                    | Purpose                                                            |
| ----------------------------- | ------------------------------------------------------------------ |
| `/connectors list [--verbose]`| List installed connectors, their state, and their bridged servers. |
| `/connectors search <query>`  | Search the connector catalogue by name, category, or tag.          |
| `/connectors add <id\|source>`| Install a connector from the catalogue or a local/URL source.      |
| `/connectors remove <id>`     | Uninstall a connector (refused while enabled).                     |
| `/connectors enable <id>`     | Enable a connector and connect its MCP server(s) now.              |
| `/connectors disable <id>`    | Disable a connector and disconnect its MCP server(s).              |
| `/connectors connect <id>`    | Connect an enabled connector's server(s) without a session restart.|
| `/connectors disconnect <id>` | Disconnect a connector's server(s) while leaving it enabled.       |
| `/connectors auth <id>`       | Manage a connector's credentials/token and report its auth state.  |
| `/connectors test <id>`       | Connect in isolation, invoke one tool, and report per-step results.|
| `/connectors stores`          | Report each catalogue endpoint and its source; `--check` probes it.|
| `/connectors help`            | Show usage for all subcommands.                                    |

### Why this exists

Users arriving from Claude expect a catalogue of integrations they can browse, add, and
authenticate without hand-editing JSON or knowing what "stdio transport" means. ragent
already has the transport (`McpClient`), the enable/disable ledger (`mcp_state.json`), an
MCP discovery scanner, and a plugin-store browser (`StoreProvider`, the Codex and Claude
marketplaces). What is missing is the **connector abstraction**: a catalogue entry that
names an integration, points at one or more MCP servers, and carries its authentication
shape, managed through one coherent surface.

### Worked examples

```text
/connectors search drive                        # find the Google Drive connector
/connectors add google-drive                    # install from the catalogue
/connectors auth google-drive                   # begin OAuth / paste a token
/connectors enable google-drive                 # connect and expose its tools
/connectors list                                # show state and bridged servers
/connectors test google-drive                   # isolated connect + one tool call
/connectors disable google-drive                # disconnect, keep installed
/connectors remove google-drive                 # uninstall
```

## Assumptions and interpretation (read before implementing)

The feature prompt leaves several decisions open. This specification fixes them explicitly
so the work is testable; each is listed under `## Open Questions` so a reviewer can
overturn it deliberately.

- **A1 - A connector is an MCP server plus metadata.** A connector wraps one or more MCP
  servers (`McpServerConfig`) in a catalogue descriptor that adds a display name, category,
  tags, an authentication shape, and provenance. The session-visible tools are exactly the
  MCP tools those servers advertise; no second tool mechanism is introduced.
- **A2 - Connectors reuse the plugin store pattern.** Connector manifests live in a
  connector store under the project `.ragent/connectors/` directory, falling back to the
  user-global `~/.config/ragent/connectors/`, scanned at session start. Enable/disable
  state persists per store in a `_state.json` ledger, mirroring
  [`ragent_plugins::store`](../../crates/ragent-plugins/src/store.rs).
- **A3 - Enablement is durable and global, like MCP servers.** Whether a connector's
  servers actually start is recorded in the existing global enable ledger
  (`<state dir>/mcp_state.json`, spec `plugins`/`mcp` FR semantics), so a connector disabled
  once stays disabled in every project. A connector's servers are registered under the id
  `<connector-id>.<server>` so two connectors cannot collide on a server name.
- **A4 - The catalogue is served through store providers.** The connector catalogue is
  fetched from an HTTPS index endpoint and normalised through a `ConnectorProvider`
  strategy, reusing the tolerant per-entry-skip discipline of
  [`StoreProvider`](../../crates/ragent-plugins/src/store_provider.rs) (spec `pluginstores`
  FR-013, FR-027). Entries that cannot be normalised are skipped and counted, never fatal.
- **A5 - Credentials are separate from configuration.** A connector's non-secret
  configuration (endpoint, scopes) lives in the connector store; secrets (tokens, client
  secrets, database passwords) live in the existing encrypted credential store and are
  referenced by name, never written into `ragent.json` or the connector manifest.
- **A6 - No new transport.** The connector system adds no protocol. Every connector
  resolves to `McpServerConfig` values the existing `McpClient` connects with (stdio, SSE,
  or HTTP). A catalogue entry that cannot be expressed as an MCP server is reported as an
  unsupported connector and skipped.

## Background - existing machinery to reuse

- **MCP client.** [`McpClient`](../../crates/ragent-agent/src/mcp/mod.rs) connects stdio,
  SSE, and HTTP servers, lists their tools, and invokes them; `MAX_CONCURRENT_MCP_CONNECTIONS`
  bounds fan-out. Tools register as `mcp_<server>_<tool>` and flow through the normal
  permission engine.
- **MCP enable ledger.** [`ragent_agent::mcp::enable_state`](../../crates/ragent-agent/src/mcp/enable_state.rs)
  persists a durable, global enabled/disabled choice per server id; absent means enabled.
- **MCP discovery.** [`ragent_agent::mcp::discovery`](../../crates/ragent-agent/src/mcp/discovery.rs)
  scans `PATH`, npm global packages, and MCP registry directories; its `DiscoveredMcpServer`
  carries the executable, args, and env a connector entry needs.
- **Plugin store + browser.** The `ragent-plugins` crate owns store paths, the scan/ledger
  pattern, the marketplace providers, the fetch budget, and the browse panel — the exact
  shapes the connector system mirrors.
- **Plugin MCP bridge.** [`scanned_plugin_mcp_servers`](../../crates/ragent-plugins/src/bridge.rs)
  already turns a manifest's `mcpServers` section into `(server_id, McpServerConfig)` pairs
  and prefixes the id with the owner; the connector bridge is the same shape with a
  connector owner.
- **Slash-command surface.** Commands are declared as `SlashCommandDef` in
  `crates/ragent-tui/src/app/state.rs` (`SLASH_COMMANDS`) and dispatched in
  `crates/ragent-tui/src/app/slash.rs`; `/plugins` is the precedent for a multi-subcommand
  family with help, validation, and usage text.

## Definitions

| Term                 | Meaning                                                                                                          |
| -------------------- | ---------------------------------------------------------------------------------------------------------------- |
| **Connector**        | A catalogue entry naming an integration, carrying a display name, category, tags, authentication shape, and one or more MCP server definitions. |
| **Connector manifest**| The on-disk descriptor that records an installed connector's servers and non-secret configuration.              |
| **Connector store**  | The directory tree where installed connectors live: project `.ragent/connectors/`, falling back to user-global `~/.config/ragent/connectors/`. |
| **Connector catalogue**| The HTTPS index the browser searches; a list of catalogue entries keyed by connector id.                       |
| **Catalog provider** | The strategy that normalises one catalogue document shape into the internal connector-entry model.              |
| **Bridged server**   | An MCP server contributed by a connector, registered under the id `<connector-id>.<server>`.                    |
| **Auth shape**       | How a connector is authenticated: none, environment variable, static token, OAuth (authorization-code), or username/password. |
| **Connector lifecycle**| discovered -> validated -> enabled -> connected -> disconnected; a connector in any failed state is `errored` with a recorded cause. |

## Requirements

### Ubiquitous requirements

**FR-001** — The system shall maintain a connector store under the project
`.ragent/connectors/` directory, falling back to the user-global
`~/.config/ragent/connectors/` directory, and shall scan both locations on session start to
discover installed connectors, with project-local connectors taking precedence on
connector-id collision.

**FR-002** — The system shall represent a connector as a normalised descriptor carrying an
id, display name, category, tags, provenance, an optional authentication shape, and one or
more MCP server definitions, and shall expose that descriptor to every command surface so
the `/connectors` family and the `ragent connectors` CLI print one wording.

**FR-003** — The system shall resolve every enabled connector's MCP server definitions into
`McpServerConfig` values and shall register each bridged server under the registry id
`<connector-id>.<server>`, so a connector's tools are the ordinary MCP tools of that server
and are subject to the normal permission engine (default action: `ask`).

**FR-004** — The system shall provide the `/connectors` slash-command family with the
subcommands `list`, `search`, `add`, `remove`, `enable`, `disable`, `connect`,
`disconnect`, `auth`, `test`, `stores`, and `help`, registered in the `SLASH_COMMANDS`
registry, reachable through the slash-command dispatch arm, listed in the autocomplete
menu, and documented in `/connectors help`.

**FR-005** — The system shall store a connector's secrets (tokens, client secrets,
passwords) in the existing encrypted credential store and shall reference them by name from
the connector manifest, and shall never write a secret value into `ragent.json` or into a
connector manifest.

**FR-006** — The system shall render every `/connectors` report with the attribution prefix
`From: /connectors <subcommand>` and shall contain no non-ASCII characters, matching the
`/plugins` family conventions.

**FR-007** — The system shall add an optional `connectors` configuration block to
`ragent-config`, with a master `enabled` switch (default `true`), catalogue endpoints, and
shared fetch budgets, loaded and merged with the same precedence as every other config
section (project overrides user-global).

### Event-driven requirements

**FR-008** — When a session starts, the system shall load each enabled connector by
resolving its server definitions, applying its enable ledger entry, and connecting each
bridged server through the existing `McpClient`, and shall record the connector's lifecycle
state as `connected` or `errored` with the cause.

**FR-009** — When `/connectors list` is invoked, the system shall print one row per
discovered connector showing id, name, category, state (`disabled`, `enabled`, `connected`,
`errored`), authentication state, and the count of bridged servers and their advertised
tools, shall print a summary line with totals, and shall print a notice for any connector
whose catalogue entry requested an unsupported capability.

**FR-010** — When `/connectors search <query>` is invoked, the system shall query the
catalogue and print matching entries with their id, name, category, tags, and authentication
requirement, and shall report an empty result set as a message rather than an error.

**FR-011** — When `/connectors add <id|source>` is invoked, the system shall install the
named catalogue connector (or the connector defined by a local directory, a local
`.zip`/`.tar.gz` package, or an `https://` URL) into the connector store, shall validate its
manifest and authentication shape, and shall record it as **disabled** so no server is
started until the user runs `/connectors enable`; no MCP connection is made during the
install.

**FR-012** — When `/connectors enable <id>` is invoked, the system shall mark the connector
enabled, shall immediately connect each bridged server in the current session, and shall
report the resulting state and the number of tools each server exposed.

**FR-013** — When `/connectors disable <id>` is invoked, the system shall mark the connector
disabled, shall disconnect each bridged server, shall deregister every tool those servers
contributed, and shall confirm how many servers and tools were disconnected.

**FR-014** — When `/connectors auth <id>` is invoked, the system shall, for a connector with
an OAuth shape, begin the authorization-code flow and store the resulting token under the
configured credential name; for a token or environment-variable shape, prompt for the value
and store it encrypted; and in all cases shall report the resulting authentication state
without echoing the secret.

**FR-015** — When `/connectors test <id>` is invoked, the system shall connect the
connector's servers in isolation, shall invoke one advertised tool once with schema-valid
sample arguments, shall report per-step `[ ok ]` / `[fail]` results including wall-clock
connect time, and shall then disconnect the isolated connection without touching the live
session.

**FR-016** — When a bridged MCP server fails to connect or returns an error from a tool
call, the system shall record the connector state as `errored` with the cause (connection
failure) or report the tool call as failed with the server's error message, and shall keep
ragent itself running and stable.

**FR-017** — When `/connectors help` is invoked, or `/connectors` is invoked with no
subcommand or an unrecognised one, the system shall display usage text documenting every
subcommand, its arguments, and the accepted `<source>` forms, and shall create or modify no
files.

### State-driven requirements

**FR-018** — While a connector is disabled, the system shall not start its servers, shall
not register their tools, and shall still list it in `/connectors list` with state
`disabled`.

**FR-019** — While a connector is enabled but its MCP server is not currently connected, the
system shall accept `/connectors connect <id>` to connect it without a session restart and
`/connectors disconnect <id>` to drop the connection while leaving the connector enabled.

**FR-020** — While a connector's server is connected, the system shall surface its tools to
the agent loop under the `mcp_<server>_<tool>` registry name, where `<server>` is the
bridged id `<connector-id>.<server>`, matching the existing MCP tool-registration path.

**FR-021** — While the `connectors.enabled` master switch is `false`, the system shall
perform no discovery, no catalogue fetch, and no connection, and shall make every
`/connectors` subcommand other than `help` report that the system is disabled.

**FR-022** — While a connector carries an authentication shape and no valid credential is
stored, the system shall report the connector as `needs auth` in `/connectors list` and
shall refuse to connect its servers until `/connectors auth <id>` succeeds.

### Optional requirements

**FR-023** — Where a catalogue entry declares an external credential requirement (an
environment variable name or an OAuth scope list), the system shall surface that
requirement in `/connectors add` and `/connectors list` output and shall record it in the
installed manifest so the requirement is visible without contacting the catalogue again.

**FR-024** — Where `connectors.stores.<name>.url` is configured, the system shall use that
endpoint in preference to the compiled default for that catalogue, and shall let each
catalogue keep its compiled default when only the other is overridden.

**FR-025** — Where a connector's catalogue entry cannot be expressed as one or more MCP
servers (an unknown transport, an empty command, a transport ragent does not speak), the
system shall record the entry's unexpressible capability as an unsupported-capability
label, shall skip the entry, and shall report the label in `/connectors list`.

**FR-026** — Where a connector's manifest declares more than one MCP server, the system
shall connect every declared server and shall report each server's state independently, so
one failing server does not hide the state of the others.

### Unwanted requirements

**FR-027** — If `/connectors add` is given a source that already exists under the same
connector id, then the system shall refuse the install and shall report the collision unless
`--force` is supplied.

**FR-028** — If `/connectors add` is given an `https` URL that is not `https`, or a URL
scheme other than `https`, then the system shall refuse the install and shall report the
rejected scheme.

**FR-029** — If a connector install source is an archive whose entries would escape the
connector store directory (a `..` path component or an absolute path), then the system shall
refuse the install, shall write no file outside the store, and shall report the rejected
entry.

**FR-030** — If `/connectors remove` is invoked for a connector that is currently enabled,
then the system shall refuse the removal, shall change no files, and shall instruct the user
to disable the connector first.

**FR-031** — If the catalog document fetched from an endpoint is malformed JSON, is not a
recognised catalogue shape, or exceeds `connectors.max_index_bytes`, then the system shall
abort the fetch, shall report the failure reason, and shall leave any previously cached
catalogue untouched.

**FR-032** — If a connector's secret is absent, expired, or rejected by the remote service
during a connection attempt, then the system shall record the connector state as
`errored` with an `auth failed` cause, shall surface the authentication guidance for that
connector's shape, and shall not retry the connection in a loop.

**FR-033** — If a connector's registered server id would collide with an existing MCP server
id in `ragent.json` or with another connector's bridged server, then the system shall reject
the later registration and shall report the collision rather than silently overwriting the
existing server.

## Configuration schema

The top-level `Config` struct gains an optional `connectors` object:

```jsonc
{
  "connectors": {
    "enabled": true,                       // master switch; default true
    "store_dir": null,                     // optional override of the connector store path
    "stores": {                            // catalogue endpoints
      "community": { "url": "https://example.org/connectors/index.json" },
      "timeout_ms": 10000,
      "max_index_bytes": 2097152,
      "cache_ttl_secs": 3600
    },
    "credentials": {                       // non-secret credential-name mapping
      "google-drive": { "token": "GDRIVE_TOKEN" }
    }
  }
}
```

- Loaded and merged with the same precedence as other config sections (project overrides
  user-global).
- `connectors.enabled: false` makes the entire subsystem inert: no discovery, no catalogue
  fetch, no connection, and `/connectors` subcommands other than `help` report that the
  system is disabled (FR-021).
- `connectors.stores.<name>.url` overrides the compiled default endpoint for that catalogue
  (FR-024); the shared budgets (`timeout_ms`, `max_index_bytes`, `cache_ttl_secs`) apply to
  every catalogue.

## Connector descriptor

```text
id            : string  (stable, unique; the catalogue key)
name          : string  (display name)
description   : string  (one line)
category      : string  (e.g. "productivity", "developer", "data")
tags          : [string]
source        : string  (catalogue origin or install source)
provenance    : enum    (catalogue | local | url)
auth          : enum    (none | env | token | oauth | password)
auth_scope    : [string]  (for oauth; empty otherwise)
credential    : string?   (credential-store name; never the value)
servers       : [ { id, transport, command|url, args, env, headers } ]
unsupported   : [string]  (recorded, not dropped - FR-025)
```

## Error handling

- Every connector-facing failure is converted into a report string or a Rust `Result`, never
  a panic, matching the plugin-system error policy (spec `plugins` FR-026).
- Malformed arguments render an `[err]` row and change no state.
- A failed connect records the cause on the connector and leaves the session running
  (FR-016); a failed authentication records `auth failed` and stops (FR-032).

## Security and privacy

- Secrets are stored encrypted and referenced by name; they never appear in manifests,
  `ragent.json`, or report output (FR-005, FR-014).
- Archive extraction is path-traversal-safe: no entry may write outside the connector store
  (FR-029).
- Non-`https` sources are refused (FR-028).
- Catalogue fetches are bounded by `timeout_ms` and `max_index_bytes` (FR-031).

## Performance

- A `/connectors list` with no flags is a store scan plus a ledger read; it performs no
  network access and no MCP connection.
- Catalogue fetches honour the shared timeout and byte cap, and a cached catalogue within
  `cache_ttl_secs` is served without a network round trip.
- Concurrent connector connections are bounded by the existing
  `MAX_CONCURRENT_MCP_CONNECTIONS` semaphore.

## Scope

In scope: the connector descriptor model, the connector store and ledger, the catalogue
providers and browser, the `/connectors` slash family, the `ragent connectors` CLI parity,
the session-start bridge into `McpClient`, and the `connectors` config block.

Out of scope: new MCP transports, an OAuth refresh-token daemon beyond the single
authorization-code exchange, non-MCP connector protocols, and any change to the MCP
transport itself.

## Acceptance criteria

1. `/connectors help`, a bare `/connectors`, and an unknown subcommand all print the same
   usage block and create no files.
2. `/connectors search` returns catalogue entries; an empty result set prints a message, not
   an error.
3. `/connectors add` installs a connector disabled; no MCP connection is made.
4. `/connectors enable` connects the servers and surfaces their tools under
   `mcp_<connector-id>.<server>_<tool>`.
5. `/connectors disable` disconnects and deregisters exactly the tools the connector
   contributed.
6. `/connectors auth` stores a secret encrypted and reports the resulting auth state
   without echoing it.
7. A disabled connector starts no server and registers no tool, yet still appears in
   `/connectors list` as `disabled`.
8. A catalogue entry that cannot be expressed as an MCP server is skipped and its
   unsupported label is reported.
9. An archive-escape, an id collision, a non-https URL, and a removal-while-enabled are all
   refused with a reason and no partial write.
10. `ragent connectors <sub>` prints the same wording as the TUI `/connectors` family.

## Open Questions

1. Should the community catalogue ship a compiled default endpoint, or default to the
   existing MCP discovery scan only? (Assumed: a compiled default endpoint, overridable.)
2. Should `/connectors auth` support device-code OAuth in addition to authorization-code?
   (Assumed: authorization-code first; device-code deferred.)
3. Should a connector be permitted to bundle skills and prompt commands, as a plugin does,
   or stay strictly MCP-server-only? (Assumed: MCP-server-only for v1.)

## Default Claude connector catalogue endpoint

This section resolves Open Question 1: ragent ships a compiled default endpoint for the
Claude connector catalogue, preinstalled and pointed at the standard Claude connector
catalog, so a new install can browse and add Claude connectors with no configuration. The
requirement mirrors the compiled-default discipline already proven for plugin stores
(`ragent_plugins::store_index::DEFAULT_CLAUDE_STORE_URL`).

**FR-034** — The system shall ship a compiled default endpoint for the Claude connector
catalogue as a single public constant, preinstalled with the binary, so that the Claude
catalogue is resolvable and browsable in every fresh install with no `connectors` block,
no `connectors.stores` entry, and no user configuration.

**FR-035** — While no non-empty `connectors.stores.claude.url` override is configured, the
system shall resolve the Claude catalogue endpoint to the compiled default and shall contact
the standard Claude connector catalog; a non-empty override shall win wholesale, and an
absent, empty, or whitespace-only override shall fall back to the compiled default with no
error.

**FR-036** — When `/connectors stores` is invoked, the system shall list the Claude
catalogue endpoint together with its provenance tag (`default` when the compiled default is
in effect, `config` when a configured override is in effect), so a user can see which
endpoint the browser will contact.

**FR-037** — If the compiled default endpoint or a configured override is not an absolute
`https` URL carrying a host, then the system shall refuse the fetch, shall report the failure
reason naming the offending scheme or malformed input, and shall treat no catalogue as
available for that endpoint rather than falling back to an unvalidated value.

**FR-038** — The system shall treat the compiled default Claude connector-catalogue endpoint
as the sole source of that literal, so that no other module, config default, or command
surface carries a second copy of the endpoint string.

## Category filtering

This section adds a category filter to the connector browser and the `list`/`search`
commands, with an `ALL` category that resets the filter to every connector.

**FR-039** — While a category filter is active in the connector browser or in
`/connectors list` / `/connectors search`, the system shall restrict the displayed
connectors to the single selected category, shall exclude connectors whose declared
category differs, and shall report the active category together with the match count.

**FR-040** — When the `ALL` category is selected, the system shall clear any active
category filter and display every connector regardless of category, and `ALL` shall be the
default filter whenever no specific category has been selected.

**FR-041** — When `/connectors list` or `/connectors search <query>` is invoked with a
`--category <name>` argument, the system shall display only connectors whose declared
category matches, shall report the active filter and the match count, and shall report a
category with no matching connectors as an empty result message rather than an error.

## Non-Functional Requirements

**NFR-001** — The compiled default Claude connector-catalogue endpoint shall be declared
exactly once, as a single public constant, and no other module shall carry a second copy of
that literal; a guard check shall fail when a duplicated default-endpoint literal is
introduced, matching the plugin-store default-endpoint discipline (spec `pluginstores`
NFR-001).

**NFR-002** — The default Claude connector-catalogue endpoint shall be resolved once per
session start and once per catalogue fetch with no rebuild required, so editing or clearing
`connectors.stores.claude.url` takes effect on the next launch rather than requiring a
recompile.

**NFR-003** — Resolving the default endpoint shall be a pure, offline operation that performs
no network access; the endpoint is contacted only when a catalogue fetch or
`/connectors stores --check` explicitly requests it.
