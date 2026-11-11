# ragent-plugins

Plugin system for ragent (spec `plugins`). Discovers, loads and executes
third-party plugins written for OpenAI Codex and Claude Code / Claude Desktop
inside a sandboxed embedded JavaScript runtime (QuickJS via `rquickjs`),
presenting plugin code with the versioned `ragent` host API. It backs the
`/plugins` command and registers plugin tools and slash commands in the
session; used by `ragent-agent`, `ragent-tui` and the root CLI.

## Workspace Dependencies

- ragent-config
- ragent-surface
- ragent-tools-core
- ragent-types

## External Dependencies

- async-trait, anyhow, serde, serde_json, thiserror, tracing, zip, flate2, tar,
  reqwest (blocking), rquickjs, sha2, hex

Dev-dependencies: ragent-config, zip, flate2, tar, tempfile.

## Features

- `default` — empty.

## Public API (crate root)

### Modules

- **add** — `/plugins add` source handling: `add`, `parse_git_source`, `AddOutcome`, `AddError`.
- **bridge** — Plugin `skills`/`mcpServers`/`agents`/`hooks` bridges: `plugin_skill_dirs`, `plugin_mcp_servers`, `plugin_agent_files`, `scanned_plugin_hooks`.
- **command_adapter** — Plugin slash commands in the TUI: `PluginCommandAdapter`.
- **commands** — Store and marketplace parse/dispatch glue: `StoreCommand`, `MarketplaceCommand`, `run_store_command`, `run_marketplace_command`.
- **control** — `/plugins list|enable|disable`: `ControlCommand`, `parse_control_command`, `run_control_command`, `render_list`.
- **descriptor** — Descriptor model and dialect recognition: `PluginDescriptor`, `PluginDialect`, `detect_dialect`.
- **error** — Contained error reporting: `PluginError`, `IoError`.
- **harness** — `/plugins test` isolated harness: `test_plugin`, `run_test_command`, `HarnessReport`.
- **help** — `/plugins help` usage: `render_help`, `PLUGIN_SUBCOMMANDS`.
- **host_api** — Versioned `ragent` host-API bridge: `HostCalls`, `PermissionGate`.
- **lifecycle** — Enable/disable/load/unload lifecycle: `PluginManager`, `LoadedPlugin`, `LoadReport`.
- **manifest** — Per-dialect manifest parsing and host-API version check: `parse_manifest`, `parse_plugin_dir`, `ParsedManifest`, `check_api_version`, `HOST_API_VERSION`.
- **marketplace** — Marketplace-inline manifests for manifest-less plugin stubs.
- **marketplace_ops** — `/plugins marketplace <store> add|remove` source resolution and ledger.
- **remove** — `/plugins remove`: `remove`, `RemoveOutcome`, `RemoveError`.
- **report** — Add/remove report rendering: `add_report`, `remove_report`.
- **runtime** — Sandboxed JavaScript runtime: `RuntimePool`, `SandboxBudget`, `SandboxContext`.
- **session** — Session start: discover, load, register, shut down: `PluginSession`, `PluginSurface`.
- **store** — Store paths, discovery scan and state ledger: `StoreDirs`, `StoreLedger`, `scan`, `store_dirs`.
- **store_enrich** — Manifest metadata enrichment: `enrich_into`, `ManifestMeta`.
- **store_fetch** — Store-index fetch (HTTPS, timeout, byte cap): `fetch_index`, `StoreIndex`, `StoreError`.
- **store_index** — Store registry and entry model: `StoreEntry`, `StoreEndpoint`, `DEFAULT_CLAUDE_STORE_URL`, `DEFAULT_CODEX_STORE_URL`.
- **store_provider** — Per-store index providers: `StoreProvider`, `ClaudeStoreProvider`, `CodexStoreProvider`.
- **store_seam** — Injectable fetch seam: `StoreIndexFetcher`, `NetworkStoreFetcher`, `FixtureStoreFetcher`.
- **surface** — TUI/CLI `/plugins` front-end helpers: `run_plugin_subcommand`, `run_plugin_list_view`.
- **tool_adapter** — Plugin tools in the session registry: `PluginToolAdapter`, `plugin_tool_name`.

The crate root re-exports the items named above (see `src/lib.rs`).

_Maintained alongside `crates/ragent-plugins/src/lib.rs` and `crates/ragent-plugins/Cargo.toml`; update when those change._
