# ragent-connectors

Connector system for ragent (spec `connectors`). A connector is a named,
user-facing integration (Google Drive, Slack, GitHub, ...) that reaches an
external system through one or more MCP servers. This crate is the
management and catalogue layer on top of ragent's MCP client: descriptors,
the on-disk store, catalogue providers, auth, lifecycle, and the MCP bridge.
It backs the `/connectors` command in `ragent-tui` and the root CLI.

## Workspace Dependencies

- ragent-config
- ragent-surface
- ragent-types

## External Dependencies

- async-trait, flate2, hex, reqwest (blocking), serde, serde_json, sha2, tar,
  thiserror, tokio, tracing, url, zip

Dev-dependencies: async-trait, flate2, serde_json, tar, tokio, zip.

## Features

None declared.

## Public API (crate root)

### Modules

- **add** — Install from a catalogue id or local/URL source: `add`, `classify_source`, `InstallSource`, `AddError`.
- **auth** — Auth shapes over the encrypted credential store: `AuthRequirement`, `AuthState`, `CredentialStore`, `store_secret`, `clear_secret`, `secret_value`, `resolve`.
- **bridge** — Connectors to MCP server configs: `resolve_servers`, `scanned_bridge`, `BridgePlan`, `BridgedServer`.
- **browse** — Catalogue browser category state: `CatalogueBrowser`, `CategoryFilter`, `build_categories`.
- **catalogue_enrich** — Best-effort manifest metadata enrichment for catalogue entries.
- **commands** — `/connectors` parse-and-run glue shared by TUI and CLI: `ConnectorCommand`, `parse_connector_command`, `run_connector_subcommand`, `run_connector_subcommand_async`.
- **descriptor** — Descriptor model and validation: `ConnectorDescriptor`, `ConnectorId`, `ConnectorServer`, `ConnectorAuthShape`.
- **entry** — Catalogue entry model: `ConnectorEntry`, `normalise_transport`.
- **error** — Contained error reporting: `ConnectorError`.
- **fetch** — Catalogue fetch, byte cap and cache: `fetch_catalogue`, `CatalogueFetcher`, `NetworkCatalogueFetcher`, `FixtureCatalogueFetcher`, `CatalogueLimits`.
- **harness** — `/connectors test` connect-and-invoke harness: `test_connector`, `run_harness`, `HarnessReport`.
- **help** — Usage text and subcommand list: `render_help`, `CONNECTOR_SUBCOMMANDS`, `CONNECTOR_FLAGS`.
- **lifecycle** — Session-start load, connect and disconnect: `ConnectorSession`, `ConnectorStatus`, `LifecycleError`.
- **management** — `/connectors list` and search rendering: `render_list`, `render_search`.
- **manifest** — Manifest read/write and install staging: `read_manifest`, `write_manifest`, `stage`, `install_descriptor`.
- **marketplace_ops** — `/connectors marketplace <store> add|remove`: `add_marketplace`, `remove_marketplace`, `resolve_source`.
- **provider** — Catalogue normalisation providers: `ConnectorProvider`, `ClaudeConnectorProvider`, `NativeConnectorProvider`, `parse_catalogue`.
- **remove** — Uninstall a disabled connector: `remove`, `RemoveOutcome`, `RemoveError`.
- **report** — Add/remove and disabled-subsystem report wording: `add_report`, `remove_report`, `disabled_subsystem_report`.
- **store** — Store paths, discovery scan and `_state.json` ledger: `StoreDirs`, `StoreLedger`, `scan`, `store_dirs`.
- **store_index** — Default catalogue endpoint and resolver: `CatalogueEndpoint`, `DEFAULT_CLAUDE_CATALOGUE_URL`.
- **store_ops** — Endpoint resolution and the `/connectors stores` report: `effective_endpoint`, `fetch_effective_catalogue`, `stores_report`, `probe_stores`.

The crate root re-exports the items named above (see `src/lib.rs`).

_Maintained alongside `crates/ragent-connectors/src/lib.rs` and `crates/ragent-connectors/Cargo.toml`; update when those change._
