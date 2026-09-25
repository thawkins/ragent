//! Plugin system for ragent (spec `plugins`).
//!
//! Discovers, loads, and executes third-party plugins written for OpenAI Codex
//! and Claude Code / Claude Desktop inside a sandboxed embedded JavaScript
//! runtime, presenting plugin code with the versioned `ragent` host API.
//!
//! Module layout (one module per implementation task):
//!
//! | Module             | Task  | Responsibility                                        |
//! | ------------------ | ----- | ----------------------------------------------------- |
//! | [`descriptor`]     | T-002 | [`PluginDescriptor`] model + dialect recognition (FR-002, FR-025) |
//! | [`manifest`]       | T-003 | per-dialect parsing, normalisation, host-API version check (FR-002, FR-019, FR-025) |
//! | [`bridge`]         | T-020 | plugin `skills`/`mcpServers`/`agents`/`hooks` bridges (FR-029, FR-030, FR-032, FR-033) |
//! | [`store`]          | T-005 | plugin store paths, discovery scan, state ledger (FR-001, FR-023) |
//! | [`store_index`]    | T-002 | store registry + store-index entry model (FR-001, FR-024) |
//! | [`store_fetch`]    | T-003 | store-index fetch: HTTPS, timeout, byte cap, JSON parse (FR-003, FR-013, FR-016, FR-024, FR-025) |
//! | [`mod@add`]        | T-006 | `/plugins add` source handling (FR-007, FR-010)    |
//! | [`mod@remove`]     | T-011 | `/plugins remove` store operation (FR-010)         |
//! | [`mod@report`]     | T-011 | `/plugins add|remove` report rendering (FR-007, FR-010) |
//! | [`mod@commands`]   | T-011 | `/plugins add|remove` parse + dispatch glue (FR-007, FR-010) |
//! | [`mod@control`]    | T-013 | `/plugins list|enable|disable` (FR-009, FR-011, FR-012, FR-016, FR-022, FR-025) |
//! | [`runtime`]        | T-004/T-007 | sandboxed JavaScript runtime                          |
//! | [`host_api`]       | T-008 | versioned `ragent` host-API bridge                    |
//! | [`lifecycle`]      | T-009 | enable/disable/load/unload/errored lifecycle          |
//! | [`tool_adapter`]   | T-010 | plugin tools in the session registry                  |
//! | [`command_adapter`]| T-012 | plugin slash commands in the TUI command surface      |
//! | [`mod@session`]    | T-016 | session start: discover, load, register, shutdown (FR-008, FR-022) |
//! | [`mod@harness`]    | T-015 | `/plugins test` isolated harness (FR-013, FR-026)     |
//! | [`mod@help`]       | T-014 | `/plugins help` usage text + subcommand metadata (FR-006, FR-014) |
//! | [`error`]          | T-002 | contained error reporting (FR-026)                    |

pub mod add;
pub mod bridge;
pub mod command_adapter;
pub mod commands;
pub mod control;
pub mod descriptor;
pub mod error;
pub mod harness;
pub mod help;
pub mod host_api;
pub mod lifecycle;
pub mod manifest;
pub mod remove;
pub mod report;
pub mod runtime;
pub mod session;
pub mod store;
pub mod store_fetch;
pub mod store_index;
pub mod store_provider;
pub mod store_seam;
pub mod surface;
pub mod tool_adapter;

pub use add::{AddError, AddOutcome, GitSource, MAX_ARCHIVE_BYTES, add, parse_git_source};
pub use bridge::{
    PluginMcpContribution, plugin_agent_files, plugin_mcp_servers, plugin_skill_dirs,
    plugin_skill_names, scanned_plugin_agent_files, scanned_plugin_commands, scanned_plugin_hooks,
    scanned_plugin_mcp_contributions, scanned_plugin_mcp_servers, scanned_plugin_skill_dirs,
    skills_of,
};
pub use command_adapter::{
    PluginCommandAdapter, dispatch_command_sandbox, substitute_command_args,
};
pub use commands::{
    StoreArgError, StoreCommand, StoreProbe, parse_store_command, probe_stores,
    render_stores_report, render_stores_report_with_probes, run_store_command,
    stores_check_requested,
};
pub use control::{
    ControlArgError, ControlCommand, disable_error_report, disable_report,
    disabled_subsystem_report, enable_error_report, enable_report, parse_control_command,
    render_list, render_list_with_mcp_tools, run_control_command,
};
pub use descriptor::{
    CLAUDE_MANIFEST_FILE, CLAUDE_NESTED_MANIFEST, CODEX_MANIFEST_FILE, CODEX_MARKER_FIELD,
    CODEX_NESTED_MANIFEST, DialectMatch, GENERIC_MANIFEST_FILE, PluginDescriptor, PluginDialect,
    detect_dialect, recognise_dialect,
};
pub use error::PluginError;
pub use harness::{
    HarnessReport, HarnessStep, StepOutcome, TestArgError, parse_test_command, render_report,
    run_test_command, sample_for_schema, test_plugin,
};
pub use help::{PLUGIN_SUBCOMMANDS, attribution, render_help, subcommand_of};
pub use host_api::{
    HostApiInstall, HostCalls, PermissionGate, PluginLogLine, PluginMessage,
    capability_permission_key, read_within,
};
pub use lifecycle::{
    DEFAULT_AUTO_UNLOAD_THRESHOLD, DisableReport, LoadReport, LoadedPlugin, PluginManager,
    UnloadedPlugin, build_gate,
};
pub use manifest::{
    AGENTS_DIR, COMMANDS_DIR, CommandSource, HOOKS_DIR, HOOKS_FILE, HOST_API_VERSION,
    ParsedManifest, PermissionRequest, PluginCommandDecl, PluginCommandDef, PluginHook,
    PluginMcpServer, PluginToolDecl, UNSUP_AGENTS, UNSUP_DESKTOP_MOUNTS, UNSUP_DESKTOP_WINDOW,
    UNSUP_EXEC, UNSUP_FS, UNSUP_HOOKS, UNSUP_MCP, UNSUP_SKILLS, V1_CAPABILITIES, VersionMismatch,
    check_api_version, derive_id, extract_agent_decls, extract_command_decls, extract_hooks,
    extract_mcp_servers, extract_skill_dirs, parse_claude_manifest, parse_codex_manifest,
    parse_manifest, parse_plugin_dir, read_plugin_hooks_file, scan_agent_dir, scan_command_dir,
};
pub use remove::{RemoveError, RemoveOutcome, remove};
pub use report::{add_error_report, add_report, remove_error_report, remove_report};
pub use runtime::{RuntimePool, SandboxBudget, SandboxContext};
pub use session::{
    DisableOutcome, EnableOutcome, PluginSession, PluginSurface, RegistrationOutcome,
};
pub use store::{
    LifecycleState, PluginState, STATE_FILE, ScanFailure, ScannedPlugin, StoreDirs, StoreLedger,
    TelemetryCounters, installed_ids, scan, scan_dirs, store_dirs, store_dirs_at,
};
pub use store_fetch::{
    FetchLimits, StoreError, StoreIndex, fetch_bytes, fetch_index, read_capped, store_label,
};
pub use store_index::{
    DEFAULT_CLAUDE_STORE_URL, DEFAULT_CODEX_STORE_URL, EndpointSource, StoreCatalog, StoreEndpoint,
    StoreEndpointError, StoreEntry, StoreEntryError, StoreKind,
};
pub use store_provider::{ClaudeStoreProvider, CodexStoreProvider, StoreProvider, provider_for};
pub use store_seam::{
    FixtureStoreFetcher, NetworkStoreFetcher, StoreIndexFetcher, default_fetcher,
};
pub use surface::{ScratchSurface, run_plugin_subcommand, store_and_config};
pub use tool_adapter::{PluginToolAdapter, dispatch_sandbox, plugin_tool_name};
