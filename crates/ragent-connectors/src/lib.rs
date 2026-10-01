//! Connector system for ragent (spec `connectors`).
//!
//! A *connector* is a named, user-facing integration (Google Drive, Slack,
//! GitHub, ...) that reaches an external system through one or more MCP
//! servers. The MCP transport is the one ragent already speaks
//! ([`ragent_agent::mcp::McpClient`](https://docs.rs/ragent-agent)); the
//! connector system is a management and catalogue layer on top of it, mirroring
//! the plugin system's structure so the two subsystems read the same way.
//!
//! Module layout (one module per implementation task):
//!
//! | Module         | Task | Responsibility                                                    |
//! | -------------- | ---- | ----------------------------------------------------------------- |
//! | [`mod@add`]        | T-009| install from a catalogue id or a local/URL source, with refusal guards (FR-011, FR-027, FR-028, FR-029) |
//! | [`auth`]       | T-007| auth-shape handling over the encrypted credential store (FR-005, FR-014, FR-022, FR-023, FR-032) |
//! | [`bridge`]     | T-006| connectors to `McpServerConfig` and registry ids (FR-003, FR-020, FR-026, FR-033) |
//! | [`browse`]     | T-020| category-filter state and category enumeration for the browser (FR-039, FR-040) |
//! | [`commands`]   | T-010| `/connectors` parse-and-run glue shared by the TUI and the CLI (FR-004, FR-006, FR-023) |
//! | [`descriptor`] | T-002| connector descriptor model, validation, collision predicates (FR-002, FR-025) |
//! | [`entry`]      | T-005| catalogue entry model and tolerant field reading (FR-002, FR-025) |
//! | [`error`]      | T-002| contained error reporting, never a panic (spec `plugins` FR-026)  |
//! | [`fetch`]      | T-005| catalogue fetch, byte cap, cache, and the injectable fetcher seam (FR-024, FR-031) |
//! | [`harness`]    | T-013| `/connectors test` isolated connect-and-invoke harness (FR-015, FR-016) |
//! | [`help`]       | T-010| `/connectors` usage text, attribution, and subcommand list (FR-004, FR-006, FR-017) |
//! | [`lifecycle`]  | T-008| session-start load, connect, and disconnect lifecycle (FR-008, FR-012, FR-013, FR-018, FR-019, FR-032, FR-033) |
//! | [`management`] | T-011| `/connectors list` and `/connectors search` report wording (FR-009, FR-010, FR-025, FR-039, FR-041) |
//! | [`manifest`]   | T-004| manifest read/write and install staging (FR-002, FR-011, FR-027) |
//! | [`provider`]   | T-005| catalogue normalisation providers and skip discipline (FR-025)   |
//! | [`mod@remove`]     | T-009| uninstall a disabled connector, refusing while enabled (FR-030)  |
//! | [`report`]     | T-010| add/remove report wording and the disabled-subsystem report (FR-006, FR-021, FR-023) |
//! | [`store`]      | T-003| connector store paths, discovery scan, `_state.json` ledger (FR-001) |
//! | [`store_index`]| T-016| compiled default catalogue endpoint and config-over-default resolver (FR-034, FR-035, FR-037, FR-038) |
//! | [`store_ops`]  | T-017/T-018 | wire endpoint resolution into the catalogue fetch (FR-035, FR-037, NFR-002) and the `/connectors stores` report: endpoint + provenance tag, `--check` probe (FR-036) |
//!
//! Later tasks add the shared `list`/`search` report wording (T-011) and the
//! `/connectors test` harness (T-013).

pub mod add;
pub mod auth;
pub mod bridge;
pub mod browse;
pub mod commands;
pub mod descriptor;
pub mod entry;
pub mod error;
pub mod fetch;
pub mod harness;
pub mod help;
pub mod lifecycle;
pub mod management;
pub mod manifest;
pub mod provider;
pub mod remove;
pub mod report;
pub mod store;
pub mod store_index;
pub mod store_ops;

pub use add::{AddError, InstallSource, add, classify_source};
pub use auth::{
    AuthAction, AuthError, AuthRequirement, AuthState, CredentialStore, EnvSource,
    InMemoryCredentialStore, MapEnv, NullCredentialStore, ProcessEnv, clear_secret,
    record_auth_failure, resolve, secret_value, store_secret,
};
pub use bridge::{
    BridgePlan, BridgeRefusal, BridgedServer, RefusedServer, resolve_servers, scanned_bridge,
};
pub use browse::{
    ALL_CATEGORY, CatalogueBrowseStatus, CatalogueBrowser, CategoryFilter, CategoryFilterState,
    browse_install_report, build_categories, entry_matches,
};
pub use commands::{
    AuthOutcome, AuthOutcomeError, ConnectorArgError, ConnectorCommand, ConnectorCommandEnv,
    auth_report, connect_report, disable_report, disconnect_report, enable_report,
    fetch_catalogue_descriptors_network, fetch_catalogue_descriptors_with_skipped,
    is_known_subcommand, lifecycle_error_report, parse_connector_command, run_connector_subcommand,
    run_connector_subcommand_async, run_connector_subcommand_env,
    run_connector_subcommand_stores_check, search_error_report, store_and_config,
};
pub use descriptor::{
    ConnectorAuthShape, ConnectorDescriptor, ConnectorId, ConnectorProvenance, ConnectorServer,
    UNSUP_COMMAND, UNSUP_SERVER_ID, UNSUP_TRANSPORT, UNSUP_URL, bridged_server_id,
    connector_id_collides, server_id_collides,
};
pub use entry::{ConnectorEntry, ConnectorEntryError, normalise_transport};
pub use error::ConnectorError;
pub use fetch::{
    CACHE_DIR, CatalogueCache, CatalogueFetcher, CatalogueLimits, ConnectorCatalogue,
    FetchedCatalogue, FixtureCatalogueFetcher, NetworkCatalogueFetcher, default_fetcher,
    fetch_bytes, fetch_catalogue, now_unix_secs, read_capped,
};
pub use harness::{
    HarnessError, HarnessReport, HarnessStep, McpProbe, ProbeTool, StepOutcome, render_report,
    run_harness, sample_for_schema, test_connector,
};
pub use help::{
    CONNECTOR_FLAGS, CONNECTOR_SUBCOMMANDS, attribution, autocomplete_tokens, render_help,
    subcommand_of,
};
pub use lifecycle::{
    AlwaysEnabled, ConnectReport, ConnectorEnv, ConnectorLifecycleState, ConnectorSession,
    ConnectorStatus, DisableReport, LifecycleError, MapServerLedger, McpConnect,
    ServerEnableLedger, ServerReport, ServerState, StartReport,
};
pub use management::{
    CategoryError, ListInput, ListRow, ToolCount, category_header, render_list, render_search,
    resolve_filter,
};
pub use manifest::{
    MAX_EXTRACTED_BYTES, MAX_SOURCE_BYTES, StageError, StagedConnector, install_descriptor,
    read_manifest, stage, write_manifest,
};
pub use provider::{
    CatalogueError, ClaudeConnectorProvider, ConnectorProvider, NativeConnectorProvider,
    NormalisedEntries, ParsedCatalogue, convert_entries, parse_catalogue, provider_for,
};
pub use remove::{RemoveError, RemoveOutcome, remove};
pub use report::{
    add_error_report, add_report, disabled_subsystem_report, remove_error_report, remove_report,
};
pub use store::{
    ConnectorCounters, ConnectorState, MANIFEST_FILE, STATE_FILE, ScanFailure, ScannedConnector,
    StoreDirs, StoreLedger, descriptor_by_id, scan, scan_dirs, store_dirs, store_dirs_at,
};
// `ConnectorError` is re-exported from `error` above; keep that one canonical.
pub use store_index::{
    CatalogueCatalog, CatalogueEndpoint, CatalogueEndpointError, CatalogueKind,
    DEFAULT_CLAUDE_CATALOGUE_URL, EndpointSource,
};
pub use store_ops::{
    StoreEndpointRow, StoreProbe, effective_endpoint, fetch_effective_catalogue, probe_stores,
    render_stores_report, render_stores_report_with_probes, resolve_catalog, stores_attribution,
    stores_check_requested, stores_report, stores_report_with_check,
};
