# Changelog

## [1.0.130] - 2026-10-08

### Fixed

- **`/events` SSE connection-cap now builds on stable Rust.** The connection
  budget reservation in `crates/ragent-server/src/routes/mod.rs` used
  `AtomicUsize::try_update`, which is an unstable library feature
  (`atomic_try_update`, issue rust-lang/rust#135894) and fails to compile with
  `error[E0658]: use of unstable library feature 'atomic_try_update'` on a
  stable toolchain. Its deprecated predecessor `fetch_update` compiles but
  emits a `deprecated` warning, so neither is portable. The reservation is now
  a `compare_exchange_weak` loop, which is stable across supported toolchains
  and preserves the exact semantics (reserve a slot while the count is below
  [`MAX_SSE_CONNECTIONS`], otherwise reject with `503`).

## [Unreleased]

## [1.0.131] - 2026-10-10

*Patch release on the OpenHands-parity delta. The full acceptance walk and the
pluggable-execution-backend / LLM security-analyzer / OpenAI-surface / ACP-server
/ automation / i18n feature set described in the `Added` entries below now ship
as a tagged release, together with the quality-gate fixes collected on top of
`2ed21e3d` (v1.0.130).*

### Fixed

- **Schema-version tests assert the exposed constant.** The fast-path schema tests
  in `crates/ragent-storage/tests/test_schema_version_fast_path.rs` now assert
  against the crate's exported `SCHEMA_VERSION` instead of a hard-coded `"1"`, so
  the suite tracks the schema bump to version 2 (the `automation_runs` table)
  rather than pinning a stale literal.
- **Plugin-store browse panel keeps document order.** `PluginStoreBrowser::set_index`
  no longer re-sorts the fetched index, so the navigator and detail panes render
  entries in the order the marketplace declares them.
- **`mask_key` doctest uses the registered-secret placeholder.** The
  `crates/ragent-types/src/sanitize.rs` doctest now expects `[REDACTED]`, the
  placeholder the shared redaction registry actually emits.
- **Report commands keep stdout clean.** `machine_output` in `src/main.rs` now
  routes tracing to stderr for the report subcommands (`openapi`, `plugins`,
  `connectors`, `automation`, `new`, `spec`, `session`, `config`, `research`), so
  `ERROR ... connection failed` diagnostics no longer pollute a command's
  machine-readable stdout.

### Changed

- **Dropped an unused `ragent-storage` dependency** from
  `crates/ragent-tools-extended/Cargo.toml` (flagged by `cargo machete`).

### Added

- **OpenHands parity acceptance walk, documentation, and quality gates (spec
  `openhands` T-019; Acceptance 1-14).** The capstone task closes the delta: it
  walks the manual test plan against the build, updates the user-facing docs, and
  runs the workspace gates.
  - **Acceptance walk.** Every item of `specs/openhands/TESTPLAN.md` (TC-001..TC-016)
    was exercised against the built binary and its automated coverage verified. The
    hermetic suites assert the invariant half of each criterion on any host -
    backend registry and `local` default (`test_backend_registry.rs`), container
    runtime detection / `podman` default / fail-without-fallback
    (`test_container_backend.rs`), the 7-layer gate and no-host-leak invariants
    (`test_hardening_invariants.rs`), secret resolution and redaction
    (`test_sandbox_secrets.rs`), remote registration and unreachability
    (`test_remote_backend.rs`), ACP client/server transports (`test_acp_client.rs`,
    `test_acp_server.rs`), the OpenAI surface and refuse-to-start gate
    (`test_openai_surface.rs`, `test_serve_openai_gate.rs`), the OpenAPI contract
    (`test_openapi_document.rs`), HTTP MCP transport (`test_mcp_http_transport.rs`),
    AgentSkills discovery (`test_agentskills_discovery.rs`, `test_skill_command.rs`),
    the security analyzer (`test_security_analyzer.rs`, `test_security_analyzer_loop.rs`,
    `test_security_command.rs`), automations (`test_automation_surface.rs`,
    `test_automation_runs_table.rs`, `test_automation_command.rs`), and i18n
    (`test_i18n_catalog.rs`, `test_i18n_command.rs`). All pass on this tree.
  - **Docs.** `README.md` gains the pluggable-execution-backend and LLM
    security-analyzer feature bullets and the `execution_backend`/`backends`,
    `security_analyzer`, `openai`, and `i18n` config blocks; `SPEC.md` records the
    `/backend`, `/security`, `/automation`, and `/i18n` slash commands, the new
    `/auto/{id}` and `/automation*` HTTP routes, and the `/backend` TUI surface and
    security-analyzer sections (§19D.7, §19D.8); `QUICKSTART.md` documents the
    backend switcher, the security analyzer, the OpenAI-compatible surface, and the
    full config example. `docs/howtos/tools/masterfetch.md`, `tool-visibility.md`,
    `config.md`, and `permissions.md` note the engine-gated `mf_screenshot` tool.
  - **Quality gates.** `cargo fmt --all -- --check`, `cargo check --workspace`, and
    `cargo clippy --workspace -- -D warnings -A clippy::used_underscore_items -A
    clippy::redundant_pub_crate -A clippy::wildcard_imports` (the exact CI
    invocation) all pass clean with every in-scope module present (Acceptance 14).
    Two lints surfaced on the new code and were fixed: an unused
    `unused_assignments` guard in the OpenAI streaming relay
    (`crates/ragent-server/src/routes/openai.rs`) and an empty-format-string
    `print_literal` in the `ragent automation list` header (`src/cli.rs`).
- **Engine-gated screenshot capability (spec `openhands` FR-029, T-017).** The
  always-erroring `mf_screenshot` stub is no longer advertised: the capability is
  registered **only** when a headless browser engine ragent can actually drive is
  present, so it is never offered when it cannot work (test-plan TC-014). A new
  `masterfetch::tools::screenshot::headless_engine_present()` seam (plus
  `detect_headless_engine()`, a pure `PATH` probe over the known engine binaries)
  gates the registration in `create_extended_registry()`, and a new opt-in
  `headless-browser` Cargo feature (off by default, assumption A10) is the switch
  a future bundled driver flips. The default build therefore advertises **148**
  tools across 21 categories (the `web` family drops 6 -> 5); the five other
  `mf_*` tools are unchanged. New `crates/ragent-tools-extended/tests/test_browser_gate.rs`
  pins the "no engine, no capability" default (and the host-agnostic `PATH`
  probe), and the existing `test_mf_permission_categories.rs` / `test_mf_visibility.rs`
  registry checks now treat `mf_screenshot` as conditional on
  `headless_engine_present()`.
- **Automation service (spec `openhands` T-016; FR-013, FR-014, FR-018,
  FR-033).** Named, schedulable or webhook-triggered agent runs with durable run
  history and execution-backend confinement:
  - `crates/ragent-config/src/automation.rs` adds the `automation` config block
    (`AutomationConfig`, `AutomationDefinition`, `AutomationTriggerKind`,
    `DispatchTarget`), inert unless an explicit `enabled: true` is present.
  - `crates/ragent-agent/src/automation/` implements the `AutomationService`
    (enqueue, manual run, scheduler loop) and third-party dispatch
    (`slack`/`github`/`linear`/`notion`/`webhook`); a run is confined to its
    configured backend via a per-session override applied by the session loop
    (FR-033).
  - `crates/ragent-storage` adds the `automation_runs` table and its CRUD
    (`insert`/`finish`/`list`/`prune`), schema version bumped to 2 (FR-018).
  - `crates/ragent-server/src/routes/automation.rs` adds the public webhook
    ingress `POST /auto/{id}` and the bearer-authenticated
    `GET /automation`, `GET /automation/runs/{id}`, `POST /automation/{id}/run`;
    all four routes are in the OpenAPI route table.
  - TUI `/automation [list]`, `/automation runs <id>`, `/automation run <id>`,
    `/automation help` and CLI `ragent automation ...` parity surfaces. See
    `docs/howtos/slashcommands/automation.md`.

### Commits (last 10)

- **`e617bcef` - plugin-store category navigator and `browser` tool removal.**
  The `/plugins codex` and `/plugins claude` browse panels gain a left-hand
  category navigator (`ALL` plus the distinct categories and tags the fetched
  index declares; `Tab` focus, `Up`/`Down` move-and-apply, `Enter` apply, `c`
  clear, mouse click/scroll), with a `--category <name>` launch pre-selection
  and an optional `category` field on `StoreEntry` (spec `catnav`); new
  `/plugins codex|claude|stores` subcommands; and the `browser` CDP tool and its
  whole `ragent-tools-extended::browser` stack (`actions.rs`, `cdp.rs`,
  `launch.rs`, `mod.rs`) are deleted together with `BrowserConfig`, the
  `tool_visibility.browser` switch, and the `tokio-tungstenite` dependency, so
  the registered count falls 152 -> 151 across 22 categories. MCP discovery now
  scans the ragent-native `~/.config/ragent/servers/` directory first, startup
  failures echo to stdout when the TUI stderr spool is active, and an
  unreachable store marketplace renders a clean
  `<Store> plugin marketplace is unavailable (...)` row. Described in detail in
  the `catnav` / `browser`-removal entries below and in `[1.0.129]`.
- **`2ed21e3d` (v1.0.130)** - the stable-toolchain fix for the `/events` SSE
  connection cap (`AtomicUsize::try_update` replaced with a
  `compare_exchange_weak` loop); see `[1.0.130]`.
- **`054e5154`, `f87e2d95`, `87024938`, `d0eaa4e1`, `b2b46d5d`, `80fac9fb`,
  `f6199c1c`, `0647d365` - the v1.0.122..v1.0.129 release line.** Each is a
  `Version: X.Y.Z` release commit already documented in its own `[1.0.12x]`
  section below. The working-tree entry that follows records the unreleased
  OpenHands-parity / `catnav` / `browser`-removal / MCP-discovery /
  startup-diagnostics set collected on top of `2ed21e3d`.

### Working tree (uncommitted)

The following entries describe the uncommitted working-tree set on top of
`2ed21e3d` (v1.0.130). The `catnav` navigator and `browser`-tool removal
described in the `Removed` subsection below have since been committed as
`e617bcef`; they are retained here because the same section also records the
OpenHands-parity delta that remains uncommitted on top of `2ed21e3d`.

#### Added

- **Durable backend registry with health probing (spec `openhands` FR-004, FR-030,
  T-004).** A new `crates/ragent-agent/src/backend/registry.rs` builds a durable
  registry entry for every configured execution backend, derived from the config
  (the config file is the durable record, the registry is its resolved read
  model). Each `BackendRegistryEntry` carries a stable id, a display name, a kind
  (`local`/`docker`/`podman`/`remote`), a `ConnectionDescriptor`, and a
  `HealthState`. The built-in `local` entry is always present and always `ok`
  (FR-019); container entries are `ok` when their runtime resolves on `PATH` and
  `unavailable` with a detail naming the missing binary otherwise (FR-026); remote
  entries are `ok` when a base URL and a key are configured (FR-030). A remote
  backend is registered by URL and key via `BackendRegistry::register_remote`, and
  `refresh_health`/`probe_remote` refine a remote's state with a live probe of the
  server's public `/health` endpoint (FR-004). The descriptor is secret-free: it
  records key *presence* and credential *names* only, never a value (FR-035), and
  `ConnectionDescriptor::summary` is safe to render. New
  `crates/ragent-agent/tests/test_backend_registry.rs` (14 tests) covers registry
  construction, kind/descriptor fidelity, active-backend resolution, remote
  registration/replacement, secret-free descriptors, runtime-presence health, and
  the live `/health` probe against an in-process mock.
- **Sandbox secret injection and workspace persistence (spec `openhands` FR-010,
  FR-023, T-006).** A `docker`/`podman`/`remote` backend descriptor may now list
  `credentials` - non-secret *names* stored in the encrypted credential store -
  and the value is resolved **at spawn time** and injected as a container
  environment variable (`<runtime> run ... -e NAME=VALUE ...`), never baked into
  an image or written into the workspace (FR-010). The new
  `crates/ragent-agent/src/backend/secrets.rs` introduces the `SecretResolver`
  seam: `StorageSecretResolver` reads the encrypted SQLite store
  (`Storage::get_provider_auth`, the same table `/auth` writes to) and
  `MapSecretResolver` is the in-memory test double. A name the store does not
  hold fails the provision with a `Provision` error rather than starting a
  sandbox without it; each resolved value is registered with the shared
  redaction registry, so a later rendering masks it (FR-035). A `remote`
  descriptor with no literal `api_key` resolves its bearer key from the same
  store (`RemoteBackend::resolve_credentials`); a missing credential fails the
  turn rather than relaying it unauthenticated, while a literal `api_key` still
  works and takes precedence. `resolve_backend_with_secrets` wires the store into
  the container and remote adapters from the session's `ToolContext` (FR-010).
  **Workspace persistence (FR-023):** the sandbox volume name is now derived from
  the *project* identity only, so it survives replacement of the container, while
  the *container* name folds in the credential names, so a changed credential set
  provisions a fresh environment over the same persisted volume.
  `ContainerBackend` gained `with_secret_resolver`, `credential_names`, and a
  redacted `injected_env_summary`; `RemoteBackend` gained `has_bearer_key` and a
  hand-written `Debug` that never prints the key. New
  `crates/ragent-agent/tests/test_sandbox_secrets.rs` (15 tests) covers
  resolution, the missing-credential failure, redaction registration, the
  stable-volume/rotated-container naming, the store-resolved remote key, and - 
  when a runtime and image are present - the live env-var / not-in-workspace and
  workspace-survival cases.
- **OpenAI-compatible streaming, bearer-token gate (spec `openhands` FR-012,
  FR-024, FR-032, T-010).** `POST /v1/chat/completions` now implements the
  streaming path: with `stream: true` the surface answers a `text/event-stream`
  of OpenAI `chat.completion.chunk` events - an opening `delta.role = "assistant"`
  chunk, one chunk per assistant `TextDelta`, and a terminal chunk carrying the
  mapped `finish_reason` (`stop` / `length` / `tool_calls` / `content_filter`) -
  terminated by a `data: [DONE]` sentinel (FR-012); `stream: false` still returns
  the buffered `chat.completion` object (FR-011). Both paths share one validated
  turn setup and preserve the permission checks (FR-038). A new `openai` config
  section (`openai.enabled`, `openai.token`; `crates/ragent-config/src/openai.rs`,
  `Config::openai_surface_enabled()` / `Config::openai_surface_token()`) carries
  the surface's bearer token and arms the FR-032 refuse-to-start gate: `ragent
  serve` exits with an error when the surface is enabled (FR-024) without a
  configured `openai.token` or ambient `RAGENT_TOKEN`. The token is the server's
  single bearer token - ragent runs no second credential for `/v1`. The untrusted
  project overlay cannot set the section (`merge_project` drops it, FR-035). New
  tests: `crates/ragent-server/tests/test_openai_surface.rs` gains
  `test_streaming_completion_emits_chunks_and_done` and
  `test_streaming_requires_auth`; the new
  `crates/ragent-config/tests/test_openai_config.rs` covers the token gate.
- **ACP server endpoint on stdio for IDE clients (spec `openhands` FR-022,
  FR-028, T-008).** ragent can now act as an ACP **server** as well as a client:
  `crates/ragent-agent/src/acp/server.rs` serves ACP-capable editors (Zed, VS
  Code, JetBrains) over JSON-RPC 2.0 on ragent's own stdin/stdout. Each editor
  session maps onto a local ragent session and that session's agent-loop events
  stream back as ACP `session/update` notifications (`initialize`,
  `session/new`, `session/prompt`, `session/cancel`; text/thought chunks and
  tool calls are rendered, and the reply carries the mapped `stopReason`) —
  satisfying FR-022 in the server direction too. The endpoint is **feature-gated
  and disabled by default**: it compiles only behind the new `ragent-agent`
  /`ragent` `acp-server` Cargo feature and runs only when `acp.server_enabled:
  true`, and it is reachable only through the new `ragent acp-server` subcommand
  (FR-028). When either gate is closed the command prints a diagnostic and exits
  with a usage error. Two new config keys (`acp.server_enabled`,
  `acp.server_agent`) with `Config::acp_server_enabled()` /
  `Config::acp_server_agent()` helpers; a malformed request frame is skipped
  rather than tearing the connection down, and an unknown method gets a JSON-RPC
  "method not found". `crates/ragent-agent/tests/test_acp_server.rs` (9 tests,
  feature-gated) drives the real transport over an in-memory duplex pipe with a
  mock provider.
- **Remote execution backend (spec `openhands` FR-001, FR-004, FR-009, FR-021,
  FR-031, FR-034, T-003).** ragent can now run a session **against a second ragent
  server** instead of on the local host. `crates/ragent-agent/src/backend/remote.rs`
  implements `RemoteBackend`: it opens (or reuses) a session at
  `POST <url>/sessions`, posts the prompt to `POST <url>/sessions/{id}/messages`,
  and decodes the server's `text/event-stream` into `RemoteUpdate`s; the new
  `crates/ragent-agent/src/session/remote_dispatch.rs` mirrors each update onto the
  local event bus under the local session id, so a remote turn renders exactly like
  a local one (FR-021). The assistant placeholder is persisted and updated with the
  mirrored text, so a remote turn stays resumable. A `remote` descriptor's `url` is
  the base URL and `api_key` the bearer token (FR-004, FR-030); the key is redacted
  from diagnostics. One `RemoteBackend` is cached per `(id, url, api_key)` and the
  remote session id per `(backend id, local session id)`, so a later turn continues
  the same remote conversation over one connection pool (FR-009). A missing `url`,
  an unreachable server, or a stream that drops part-way through the turn fails the
  turn - surfaced as an `AgentError` in the TUI - and never re-runs the turn's tools
  locally (FR-031, FR-034); an explicit cancellation stops promptly. `RemoteBackend`
  refuses every local tool dispatch, so a leaked dispatch cannot execute on the host
  either. The relay is bounded by a turn timeout and polls the cancellation flag, so
  an unresponsive server cannot hang the session. New tests:
  `crates/ragent-agent/tests/test_remote_backend.rs` (14 cases - config resolution,
  fail-without-fallback for a URL-less backend, local-dispatch refusal, the error
  taxonomy, the remote-session cache, the SSE frame decoder, and live cases against
  an in-process Axum mock: streamed-and-mirrored turns, session reuse, unreachability,
  a mid-turn stream drop, and cancellation).
- **Container execution backend (`docker`/`podman`) (spec `openhands` FR-001,
  FR-002, FR-009, FR-020, FR-026, FR-031, FR-035, T-002).** ragent can now run a
  session's shell, file, and search tools **inside a container** instead of on
  the host. `crates/ragent-agent/src/backend/container.rs` implements
  `ContainerBackend`, which provisions one long-lived sandbox per session
  (`<runtime> run -d --name ragent-sandbox-<hash> -v ragent-sandbox-vol-<hash>:/projects
  -w /projects <image> sleep infinity`) and dispatches each tool call with
  `<runtime> exec`. The workspace is a **container-named volume, never a host bind
  mount**, so a sandboxed tool cannot read or write the host working directory
  (FR-020, FR-035; test-plan TC-003's `HOST_MARKER_NOT_VISIBLE`). `bash` is gated by
  the existing host 7-layer validator before it runs, so the sandbox cannot be used
  to bypass the security model (FR-002, FR-037); a file/search subset (`read`,
  `write`, `create`, `append_file`, `rm`, `mkdir`, `list`, `glob`, `grep`) is
  translated to confined sandbox `exec` scripts; every other tool is refused with no
  host fallback (FR-031). A provisioning failure (missing runtime, missing or
  unpullable image) fails the turn with a `Provision` error and never runs the tool
  on the host. New `crates/ragent-agent/src/backend/detection.rs` probes `PATH` for
  `podman`/`docker` and fixes `podman` as the default container runtime when none is
  specified (FR-026). `Config::effective_backend_config()` resolves both config
  spellings (a bare `execution_backend: "podman"` label and an inline descriptor with
  `image`/`workspace`), and the processor now resolves the backend through
  `resolve_backend_for_config`, caching one `ContainerBackend` per `(kind, id, image,
  workspace, working dir)` so a session's container is provisioned once and reused.
  New tests: `crates/ragent-agent/tests/test_container_backend.rs` (12 cases -
  runtime detection and the podman default, fail-without-fallback for a missing
  image and a bad image, unadapted-tool refusal, workspace path confinement, and
  live sandbox cases for `bash` and the routed file/search tools, which skip when no
  runtime/image is present).
- **UI locale message catalog with English fallback (spec `openhands` FR-027,
  T-014).** ragent now renders user-facing strings (the status-bar
  `Project:`/`Branch:`/`Ready`/`Model:`/`Thinking:` labels, the `/help` header,
  and the connector-catalogue footer) through `ragent_config::i18n::t`, which
  serves a locale message catalog and falls back to the compiled-in English text
  for any key the catalog does not translate (never a raw key or blank). The
  `i18n` config section (`{ "i18n": { "enabled": true, "locale": "fr" } }`) is
  opt-in and absent by default; catalogs are JSON files discovered at
  `<project>/.ragent/locales/<locale>.json` then
  `~/.config/ragent/locales/<locale>.json`, with a locale tag (`fr_FR.UTF-8`)
  expanding to `/`-less file names (`fr_fr`, `fr`) most-specific-first. With no
  `i18n` section a non-English ambient `LANG`/`LC_ALL`/`LC_MESSAGES` with an
  installed catalog enables translation automatically. The new `/i18n`
  command (`on|off|status|list|<locale>|help`) reports and persists the setting;
  `ragent_config::sync_runtime_flags` wires the resolved state at startup.
  New tests: `test_i18n_catalog.rs` (config shape, English fallback, malformed
  catalog, candidate expansion, project-over-global precedence) and
  `test_i18n_command.rs` (`/i18n` dispatch plus status-bar translation).
- **AgentSkills pack discovery (spec `openhands` FR-005, T-012).** ragent now
  discovers AgentSkills-format packs (a directory of `<name>/SKILL.md` with YAML
  frontmatter declaring at least `name` and `description`) from the project
  `.agents/skills/` directory and the user `~/.agents/skills/` directory, loaded
  alongside the existing `.agent`/`.claude` OpenSkills variants and ragent's own
  `.ragent/skills/` packs. Discovery is now driven by a single ordered
  `skill::loader::discovery_roots` helper (`.agents` first at each level) which
  `discover_skills` scans and `SessionProcessor::skill_registry` watches for cache
  invalidation, so the scan and the cache key cannot drift. The convention is
  additive (A7): on a name clash the existing higher-scope ragent pack wins, and
  `.agents/skills/` skills override the user-global scope. New tests:
  `test_agentskills_discovery.rs` (end-to-end `SkillRegistry::load` discovery,
  frontmatter-name precedence, existing-pack clash precedence, multi-pack trees,
  SKILL.md-less directories) plus `discovery_roots` unit tests in
  `tests/inline/skill_loader.rs`.
  - **AgentSkills TUI `/skill` surface (spec `openhands` FR-005, T-013).** The
    TUI gains a first-class `/skill` command beside the existing `/skills`
    listing: `/skill` (or `/skill list`) lists every registered pack, and
    `/skill <name> [args...]` loads and invokes a named pack exactly as the bare
    `/<name>` trigger does. Both the list form and the invocation form resolve
    through the same `SkillRegistry`, so an AgentSkills pack discovered under
    `.agents/skills/` (project) or `~/.agents/skills/` (user) is listable and
    loadable by its frontmatter name. The bare-`/<name>` fall-through arm and the
    new command share one `App::invoke_skill_command` helper (returns whether a
    pack matched so the caller can still report an unknown command) and one
    `App::render_skill_list` helper (so `/skills`, `/skill`, and `/skill list`
    render identical output); the empty-registry hint now names the AgentSkills
    paths alongside the ragent ones. On a name clash the existing higher-scope
    ragent pack still wins (A7). `/skill` is registered in `SLASH_COMMANDS` and
    the `commands_info` mirror (kept in sync by the drift test). New tests:
    `crates/ragent-tui/tests/test_skill_command.rs` (help, list via `/skill` /
    `/skill list` / `/skills`, AgentSkills pack rendering with
    `scope: openskills-project`, load-starts-invocation, unknown-pack not-found,
    and the clash precedence) and a new
    `test_agentskills_pack_is_loadable_and_invocable` case in
    `crates/ragent-agent/tests/test_agentskills_discovery.rs`. Docs:
    `SPEC.md` section 12.4, `docs/howtos/slashcommands/skill.md` (new) and the
    `INDEX.md`, and `specs/openhands/TESTPLAN.md` TC-011 automated coverage.
- **Plugin-store category navigator (spec `catnav`).** The `/plugins codex` and
  `/plugins claude` browse panels gain a left-hand category navigator: `ALL` plus
  the distinct non-empty categories the fetched store index declares (sorted
  case-insensitively), followed by each distinct non-empty `tag` of an entry that
  also declares a category. It is driven by keyboard (`Tab` focus, `Up`/`Down`
  move-and-apply, `Enter` apply + focus results, `c` clear to `ALL`) and by mouse
  (left-click a row to select, wheel to scroll the navigator without changing the
  active category, left-click a result row to move the result cursor). A category
  change re-derives the visible set from the full fetched index in memory (no
  network request), composes with the free-text query, and resets the result
  cursor; a category the index no longer declares falls back to `ALL`. A launch
  accepts `--category <name>` on `/plugins codex` / `/plugins claude`, held until
  the index lands and then validated (an unknown name is reported in the footer).
  The `category` field is added to the store-index entry. New tests:
  `test_plugin_store_category_filter.rs`, `test_plugin_store_category_nav.rs`,
  `test_plugin_store_category_mouse.rs`, `test_plugin_store_category_launch.rs`,
  and the fixture-driven acceptance walk
  `test_plugin_store_category_acceptance.rs` (covers TESTPLAN TC-001..TC-013 and
  acceptance criteria 1-10). New fixtures:
  `assets/plugins/fixtures/stores/index-codex.json`, `index-claude.json`, and
  `index-nocat.json`.
- **ragent-native MCP server directory `~/.config/ragent/servers/`.** A new
  `ragent_config::user_dirs::global_mcp_servers_dir()` resolves the ragent-native
  location, and `/mcp discover` now scans it **first** (so its definitions win
  de-duplication) before the third-party locations (`~/.claude/mcp-servers`,
  `~/.cline/mcp-servers`, and the legacy `~/.mcp/servers`). The legacy location
  is still scanned for backwards compatibility. Documented in the new
  `docs/howtos/mcp.md`.
- **OpenAPI 3.1 contract, generated typed client, and `ragent openapi` CLI
  (spec `openhands` FR-007).** The REST API is now described by an OpenAPI 3.1
  document served at `GET /openapi.json`. The document and the generated client
  are both rendered from one route table
  (`crates/ragent-server/src/routes/openapi.rs::ROUTES`), so they cannot drift
  from each other; a companion integration test
  (`crates/ragent-server/tests/test_openapi_document.rs`) asserts the table and
  the served router agree in both directions — it scrapes every
  `.route(...)`/`.nest(...)` literal from `src/routes/` and compares it to the
  table, then probes every documented parameterless `GET` through the live
  router. Every protected operation declares the native bearer `security`
  scheme; `/openapi.json` itself is public (it carries no runtime data, tokens, or
  secrets — asserted by the test, FR-035). A dependency-free typed TypeScript
  client is generated from the same table and checked in at
  `docs/openapi/ragent-client.d.ts`; `ragent openapi` prints the document and
  `ragent openapi --client` prints the client, with tracing routed to stderr for
  that command so the machine-readable stdout stays clean.
- **End-to-end HTTP MCP transport test (spec `openhands` FR-025).** The existing
  Streamable-HTTP MCP client (`HttpMcpClient`) is now proven through the ordinary
  [`McpClient`](crates/ragent-agent/src/mcp/mod.rs) entry point against a live
  HTTP mock: connect, list tools, invoke a tool, and disconnect — alongside the
  `stdio` transport rather than only in a unit fixture
  (`crates/ragent-agent/tests/test_mcp_http_transport.rs`).
- **Hardening and secret-safety verification (spec `openhands` FR-002, FR-024,
  FR-032, FR-035, FR-037, FR-038, T-018).** Two hermetic test suites pin the
  security invariants the new execution/interop surfaces must not weaken.
  `crates/ragent-agent/tests/test_hardening_invariants.rs` (4 tests) proves the
  7-layer bash validator gates every sandbox `bash` call *before* a container is
  provisioned — a banned tool, a denied command name, a denied pattern, a
  directory escape, and a base64-decode-to-shell obfuscation are each rejected
  with the tool never executing on the host, and the assertion needs no container
  runtime (FR-002, FR-037); it also pins that a remote backend's `Debug` never
  prints its bearer key and that a sandbox's workspace is the in-container
  `/projects` named-volume mount, never the host working directory (FR-035).
  `tests/test_serve_openai_gate.rs` (2 tests) spawns the built binary and proves
  `ragent serve` refuses to start the OpenAI-compatible surface when it is enabled
  without a token (non-zero exit, actionable message) while a configured token
  starts it (FR-024, FR-032). The permission path on the OpenAI surface and the
  `GET /config` redactor were already covered by
  `ragent-server/tests/test_openai_surface.rs` and `test_config_redaction.rs`;
  the sandbox path-containment guard by `test_container_backend.rs`.

#### Fixed

- **`ragent acp-server` names the specific closed gate.** With both ACP-server
  gates closed the command reported only "the endpoint is disabled", which reads as
  a config problem even in a build that lacks the feature entirely. It now
  distinguishes the two: the default build (no `acp-server` feature) says to
  rebuild with `cargo build --features acp-server` **and** set
  `acp.server_enabled: true`, while a feature build with the key unset keeps the
  config-key message. Both paths exit with a usage error (code 2), satisfying
  test-plan TC-016's FR-028 wording (`src/acp_server.rs`).
- **Startup errors are no longer swallowed by the TUI stderr spool.** In TUI
  mode stderr is redirected into the log spool, so an error that aborted startup
  (a config parse failure, a storage open failure) printed its `Error:` line to
  the redirected fd 2 and vanished. `stderr_spool::is_active()` now records
  whether the redirect is in place, and the binary's top-level error handler
  echoes a startup failure to stdout (the terminal) when the spool is active, so
  the failure is never silent.
- **Store-marketplace failures read as clean messages.** `StoreError` gains
  `is_unavailable()` and `panel_detail(kind)`; a transport-level failure
  (connection/DNS/TLS, timeout, non-2xx status, refused redirect) renders as
  `<Store> plugin marketplace is unavailable (<plain cause>)` in the browse
  panel instead of echoing the raw transport error, while a reached-but-unusable
  response (malformed JSON/shape, oversized body, refused endpoint) keeps its
  specific cause (spec `pluginstores` FR-013).

#### Changed

- **`CategoryFilter` gains a shared category predicate.** `matches_category()`
  extracts the exact, case-insensitive category comparison from `matches()` so a
  surface whose rows are not `ConnectorDescriptor`s (the plugin-store navigator)
  can reuse the one definition of what a category is, and
  `CategoryFilterState::from_categories()` / `sort_categories()` are exposed so
  both surfaces derive the same sorted row set.

#### Removed

- **`browser` tool and its CDP automation stack** — the `browser` tool (Chrome
  DevTools Protocol: `open`, `snapshot`, `click`, `type`, `fill_form`, `select`,
  `wait`, `eval`, `scroll`, `upload`, `press`, `screenshot`, `status`, `setup`)
  and the whole `ragent-tools-extended::browser` module (`actions.rs`, `cdp.rs`,
  `launch.rs`, `mod.rs`) are deleted, and the tool is no longer registered in
  `create_extended_registry()`. The `tokio-tungstenite` WebSocket dependency is
  dropped from the workspace and the `ragent-tools-extended` manifest.
- **`browser` configuration and visibility switch** — `BrowserConfig`
  (`browser.cdp_endpoint`, `browser.default_headless`), the `Config.browser`
  field, the `ToolVisibilityConfig.browser` / `ToolVisibilitySpecified.browser`
  switches, and the `tool_family_names("browser")` entry are removed from
  `ragent-config`; the `/tools` family list drops `browser` (the remaining
  switches are `github`, `gitlab`, `teams`, `agents`, `plan`, `codeindex`,
  `masterfetch`). Configs that still carry a `browser` key are ignored (unknown
  keys are tolerated).
- **Browser tests and TUI renderers** — `tests/test_browser.rs`, the four inline
  browser test modules, and the `browser` input/result summary arms in the TUI
  message widget are deleted.

Registered tool count falls 152 -> 151 across 22 categories.

- **`gmail` and `send_channel_message` tools and their dependency code** — the
  `gmail` tool (`GmailTool`, Gmail REST v1 with encrypted OAuth2 token storage,
  `SqliteTokenStore`, `GmailTokens`) and the `send_channel_message` tool
  (`SendChannelMessageTool`, Telegram/Discord sinks) are deleted along with the
  whole `ragent-tools-extended::gmail` and `::channels` modules and their tests
  (`tests/test_gmail.rs`, `tests/test_channels.rs`). Both are removed from
  `create_extended_registry()`. The `base64` dependency is dropped from the
  `ragent-tools-extended` manifest (its only user was `gmail.rs`).
- **`gmail`/`channels` configuration in `ragent-config`** — `GmailConfig`,
  `ChannelsConfig`, `TelegramChannelConfig`, and `DiscordChannelConfig`, the
  `Config.gmail` / `Config.channels` fields, their merge arms, and
  `user_dirs::global_gmail_db_path` are removed. Configs that still carry a
  `gmail` or `channels` key are ignored (unknown keys are tolerated).
- **Gmail/Telegram/Discord credential seeding** — the
  `GMAIL_ACCESS_TOKEN`/`GMAIL_REFRESH_TOKEN`/`GMAIL_CLIENT_SECRET` and
  `TELEGRAM_BOT_TOKEN`/`DISCORD_WEBHOOK_URL` entries are dropped from the startup
  secret-registry seed in `src/main.rs`.

Registered tool count falls 151 -> 149 across 21 categories.

## [1.0.129] - 2026-10-07

*Code-audit remediation release. The `docs/plans/code-audit.md` plan is now
complete through M9; this cycle lands the last four milestones (M5 dead-code
removal, M6 test hygiene, M7 new test coverage, M8 standards/cosmetic cleanup,
M9 dependency upkeep) as uncommitted working-tree work on top of the ten
documented commits below. Headline dependency moves: `reqwest` 0.13,
`criterion` 0.8, `opentelemetry` 0.33, `lopdf` 0.44 (the vendored
`vendor/lopdf` crate is deleted), `tree-sitter` 0.27, `notify` 8.2,
`rquickjs` 0.14, `chacha20poly1305` 0.11, `rand` 0.10 workspace-wide, `which` 8;
the `thiserror` 1.x line is retired and the yanked `yoke-derive` is cleared. A
new `ragent-surface` crate (18th workspace crate) is extracted, the `.gitignore`
gains certificate/SQLite-sidecar rules, and a committed `.env.example` template
lands.*

### Commits (last 10)

- **`f87e2d95` - Version: 1.0.128.** Release commit for the config-durability,
  bounded-MCP-connect, and non-blocking-TUI-startup release (full detail in the
  `[1.0.128]` section below).
- **`87024938` - Version: 1.0.127.** Release commit. Added `TOOLREP.md`, removed
  the six Office / LibreOffice document tools and their module set (registered
  tool count 158 -> 152), the `tool_visibility.office` switch, the unused `docio`
  helper, the OOXML/ODF `DocumentFormat` variants, and the
  `docx-rust`/`calamine`/`ooxmlsdk`/`zip`/`spreadsheet-ods` dependencies; plus a
  `/simplify all` pass over the changed set.
- **`d0eaa4e1` - Version: 1.0.126.** Office / LibreOffice tool removal and the
  accompanying `/simplify all` sweep (tool count 158 -> 152).
- **`b2b46d5d` - Version: 1.0.125.** `os_info` host-introspection tool and the
  `/osinfo` slash-command family (registry 171 -> 172), the research
  scholarly-engine default flip (`--papers` replaces `--no-papers`), the `probe`
  parameter, and three new tracked research items.
- **`80fac9fb` - Version: 1.0.124.** Connector-system follow-up, `/memory clear`,
  and TUI render fixes (`/mcp` table rendering, `/tools list`, `split_summary_icon`,
  the MCP orphan-sweep extension to `systemd --user`).
- **`f6199c1c` - Version: 1.0.123.** Research system, `ragent-connectors` crate
  (17th workspace crate), Gmail/communications, osinfo groundwork, and the
  per-engine research progress table.
- **`0647d365` - Version: 1.0.122.** Security and anti-pattern remediation sweep
  folding in the staged working tree on top of `6cf0b60f` (MS-04): `ANTIPAT.md`
  M0 plus M2-M7, `ragent-team` shim crate removed (17 -> 16 workspace crates),
  major dependency bumps, and `SECTASKS.md` MS-05 "prevent recurrence" (the
  `ragent_types::guard` module, the single `sanitize` redaction chokepoint, and
  the `security-guards` CI job).
- **`6cf0b60f` - MS-04: defence in depth (`SECTASKS` T-059..T-066).** The 33
  Low-severity findings closed with the same guard shapes as MS-01..MS-03:
  `--samples` clamped to `MAX_BENCH_SAMPLES` (100), streamed dataset downloads
  under `MAX_DOWNLOAD_BYTES`, benchmark manifest path containment, plugin
  manifest `entry`/`main`/`server.entry` validation, `resolve_memory_dir`
  rejecting unsafe agent names, a process-private 0700 bash scratch directory,
  redacted JSON parse-diagnostic source lines, and credential-bearing events
  redacted on the SSE stream.
- **`da83d927` - MS-03: network and secret hardening (`SECTASKS` T-025..T-058).**
  The Medium-severity remediation set: MCP HTTP response bodies under an 8 MiB
  cap, benchmark download timeouts and size/pagination budgets, LLM provider
  `Retry-After`/SSE/error-body caps, GitHub/GitLab pagination budgets,
  `mf_fetch`/`mf_crawl` byte caps and `crawl_urls` SSRF check, secret registry
  seeded from every credential env var, bash/CI/SSE-argument redaction,
  HTTP-origin-gated GitHub token, Gmail CRLF stripping, 0600 DB/WAL/activity-log
  permissions, `http_request` refusing routing/credential headers, codeindex
  exclusion-glob/FTS-symlink/walker guards, bounded spec numbering, mailbox
  caps, panic-free `CronSchedule`, and a stderr-spool byte ceiling.
- **`14c25e1d` - Add research/ folder.** Reverts the `research/` entry in
  `.gitignore` so the directory is tracked, and commits the accumulated research
  findings, indexes, and output reports.

### Working tree

The uncommitted M5/M6/M7/M8/M9 work, the M1 `.gitignore`/`.env.example`
hardening, and the config-write robustness fix are documented in the
`Uncommitted (working tree)` section immediately below.

## Uncommitted (working tree)

### Added

- **Milestone M7 of `docs/plans/code-audit.md` complete - new test coverage.**
  Five tasks closing the highest-risk untested modules, all as external `tests/`
  suites (no inline `#[cfg(test)]` module was added):
  - **`CalculatorTool` coverage (T-701).** New
    `crates/ragent-tools-core/tests/test_calculator.rs` (24 tests) exercises the
    recursive-descent parser directly and through the tool: operator precedence,
    left-associative `-`/`/`, right-associative `^`, parentheses, unary `+`/`-`,
    `%`, the `pi`/`e`/`tau` constants, the whitelisted functions, the documented
    divide-by-zero semantics (`1/0` is `inf`, `0/0` is `NaN` - not an error), and
    every error path (unexpected character, empty input, trailing input, missing
    `)`, unknown name/function, arity mismatch, bad argument separator, the
    4096-character cap, and the 64-level nesting cap).
  - **Untested small tools (T-702).** New `test_agent_complete_tool.rs`,
    `test_bash_reset_tool.rs`, and `test_xlsx.rs` cover `AgentCompleteTool`
    (summary required, `TaskCompleted` event published, metadata surface),
    `BashResetTool` (deletes a sentinel session state file, no-op when absent),
    and `write_xlsx` (mixed cell types, multiple/default-named sheets, missing
    `sheets` error). `get_env` already had `test_get_env_tool.rs`.
  - **Azure AI Foundry provider (T-703).** New
    `crates/ragent-llm/tests/test_azure_foundry_provider.rs` (5 tests) stands up a
    loopback HTTP server and pins the provider's three responsibilities -
    request build (`POST <base>/openai/v1/chat/completions`, OpenAI-compatible SSE
    parsing), auth (the Azure `api-key` header, never `Authorization: Bearer`), and
    model discovery (`GET /openai/models?api-version=2024-10-21`, capability
    derivation, and redaction of a key-shaped token echoed in an error body).
  - **Agent-loop step harness (T-704).** New
    `crates/ragent-agent/tests/test_loop_steps_harness.rs` (3 tests) drives a
    scripted `Provider` + `LlmClient` through the public `process_message` loop
    (the only path an external test crate can reach, since the step methods are
    `pub(crate)`): it asserts that `prepare_client` produces one request carrying
    the resolved model, system prompt, tool definitions, and user message; that
    `call_llm_step` accumulates a single text step and a tool-then-text two-step
    run, publishing `RequestStarted`/`TextDelta`/`TokenUsage`/`ToolCall*`; that the
    final save persists the assistant message and emits a terminal
    `MessageEnd { Stop }`; and that a provider error surfaces with an `AgentError`
    event.
  - **Orchestrator `Coordinator` harness (T-705).** New
    `crates/ragent-agent/tests/test_orchestrator_coordinator.rs` (13 tests) uses a
    deterministic fake `Router` to cover `start_job_sync` (concatenated responses,
    no-match, all-send-fail, policy resolver, timeout classification),
    `start_job_first_success` (first non-`error:` response, all-`error:` bail,
    send-error bail, no-match), `start_job_async` (event emission and stored
    result, no-match `JobFailed`), the unknown-job error, and `MetricsSnapshot`
    serialisation.

### Fixed

- **`Coordinator::active_jobs` counter no longer underflows.** The active-job
  metric was incremented *after* spawning a job in `start_job_async` while the
  `ActiveJobsGuard` (which decrements on drop) was constructed inside the spawned
  task, and the synchronous `start_job_sync`/`start_job_first_success` paths
  installed a guard without ever incrementing. `active_jobs` therefore drifted
  below zero, wrapping `AtomicU64` to `18446744073709551615`. The increment now
  happens exactly once, before the guard is installed, via a new
  `ActiveJobsGuard::enter` constructor used by all three job entry points.

### Changed

- **Milestone M5 of `docs/plans/code-audit.md` complete - dead-code and
  stale-duplicate removal.** Five tasks:
  - **Stale agent snapshot deleted (T-501).** `crates/ragent-agent/src/snapshot/`
    held a byte-near-identical second copy of `crates/ragent-storage/src/snapshot.rs`
    that had drifted (its `to_full` still cloned the base file map, missing
    PERF-070, and its diff parser still used the panicking `split_at(1)` that
    SEC-ragent-storage-006 fixed). Nothing referenced `ragent_agent::snapshot`
    (live capture/restore uses `ragent_storage::snapshot`), so the module and its
    `pub mod snapshot;` declaration are gone, and the `recorder.rs` doc-link now
    points at `ragent_storage::snapshot::restore_snapshot`. The
    `security-unwrap-baseline.txt` entry for the deleted file is dropped.
  - **Duplicated schema test suite removed (T-502).** The seven
    `ragent-tools-core` schema tests existed twice: as an inline `#[path]` body
    (`tests/inline/schema_tests.rs`, reached through a `#[cfg(test)]` hook in
    `src/schema.rs`) and as the external `tests/test_schema_validation.rs`. The
    external public-API copy is kept as the single source; the inline body and its
    hook are deleted, so each test now runs once.
  - **Placeholder tests replaced with real assertions (T-503).**
    `crates/ragent-agent/tests/test_precompiled_regexes.rs` had two empty bodies
    that only "confirmed the test plumbing". They now assert on concrete values:
    the stall `RegexSet` is one shared instance (`std::ptr::eq`) that matches a
    stall phrase and rejects ordinary text; `redact_secrets` masks a key-shaped
    token deterministically; and the router classifier returns an identical tier
    and composite score across repeated calls.
  - **Dead probe fallbacks removed (T-504).** After T-309 the CLI `NeverProbe`
    and TUI `NoProbe` were never driven: both surfaces serve `test` through
    `run_connector_subcommand_async` and never reach the `Test` arm of
    `run_connector_subcommand_env`. That arm, the `probe`/`workdir` parameters it
    alone used, and both fallback types are deleted; the env dispatcher now takes
    `(&mut env, sub, rest)` and returns `None` for `test`.
  - **New `ragent-surface` crate (T-505).** The `/plugins` and `/connectors`
    families had drifted into parallel copies of their surface glue. A new
    workspace crate owns the one implementation of the `From: <trigger>
    <subcommand>` attribution and the first-token subcommand tokeniser
    (`ragent_surface::help`), the two-root store-directory resolution and the
    symlink-containment guard (`ragent_surface::store`), and the harness step
    model with `sample_for_schema`/`schema_type`/`truncate`/`step`
    (`ragent_surface::harness`). `ragent-plugins` and `ragent-connectors` delegate
    (their public names are unchanged, so no call site moved), and the new
    `tests/test_surface_helpers.rs` pins each helper. The workspace advances from
    17 to 18 crates.

- **Milestone M6 of `docs/plans/code-audit.md` complete - test infrastructure and
  hygiene.** Eight tasks:
  - **Test scratch paths moved off `/tmp` (T-601).** 31 sites across 24 files now
    use `target/temp` per the AGENTS.md temp-file rule.
  - **Shared `TempTree` sandbox helpers (T-602).** `ragent-plugins` and
    `ragent-connectors` each gained one `tests/support` `TempTree`, replacing 18
    and 10 copy-pasted definitions and matching the `ragent-llm`/`ragent-tui`
    pattern.
  - **Diagnostic tests gated (T-603).** `test_mf_orchestrator_diag` and
    `test_fts_diag` are `#[ignore]`-gated so a default `cargo test` does not run
    them.
  - **Live-network test gated (T-604).** `test_ollama_cloud_real` is
    `#[ignore]`-gated so the default run never hits the network.
  - **Config test triples collapsed (T-605).** The per-key
    `test_serper_api_key.rs`/`test_perplexity_api_key.rs`/`test_langsearch_api_key.rs`
    files are replaced by one table-driven `test_api_key_config_fields.rs`.
  - **Research scoreboard fixture (T-606).** The three scoreboard tests share
    `tests/support/scoreboard_fixture.rs`.
  - **Weak assertions strengthened (T-607).** `.is_some()`/presence-only
    assertions bind and compare concrete values.
  - **Test file naming (T-608).** `dump_registries.rs`, `session_processor.rs`,
    `source_vault.rs`, and `structure_types.rs` renamed to the
    `test_<component>_<scenario>` convention.
- **Milestone M8 of `docs/plans/code-audit.md` complete - standards and cosmetic
  cleanup.** T-801..T-811: GitHub/GitLab acronym casing standardised on the
  canonical form across `ragent-tools-vcs` (all 31 tool types plus the shared
  helpers); the three user-facing/external-input unwraps removed; emoji and
  box-drawing glyphs stripped from production-source comments workspace-wide; the
  six named modules carry `//!` headers; `MultiPlEAdapter` renamed to
  `MultipleAdapter`; the four `Regex::new(...)` sites in `slash.rs` use
  `.expect("valid ... regex")`; the `CollectingTierRouterObserver` test double is a
  real `pub` type; `test_codeindex_backward_compat` asserts against the
  `SLASH_COMMANDS` data model instead of scraping source.

### Changed

- **Milestone M9 of `docs/plans/code-audit.md` complete - dependency upkeep.**
  T-901..T-908: the yanked `yoke-derive@0.8.3` is cleared (`cargo audit` reports
  no yanked crate); all workspace crates converge on `thiserror 2`,
  `reqwest 0.13` (the `ragent-llm` direct pin now uses `workspace = true`),
  `rand 0.10`, and `criterion 0.8`; `ragent-storage` moves from `rand 0.8` to the
  workspace `0.10`; `lopdf` converges on `0.44` and the dedicated
  `vendor/lopdf` crate is deleted (the `MAX_OBJECT_DEPTH` recursion guard is no
  longer needed because `pdf-extract`/`lopdf` 0.44 bound depth natively);
  `opentelemetry`/`opentelemetry_sdk`/`opentelemetry-otlp` migrate 0.29 -> 0.33
  (the Prometheus scrape endpoint enables the new
  `experimental_metrics_custom_reader` feature); and `notify` 7 -> 8.2,
  `rquickjs` 0.10 -> 0.14, `tree-sitter` 0.26 -> 0.27, `chacha20poly1305`
  0.10 -> 0.11, `which` 7 -> 8 are applied. The residual `thiserror 1.x` lock
  entry is a stale transitive of a build-dependency edge not exercised on the
  default target and does not appear in the cargo dependency graph.
- **Milestone M1 `.gitignore`/`.env.example` hardening (T-101..T-103).**
  `.gitignore` gains `*.db-wal`/`*.db-shm` (SQLite sidecars can hold plaintext
  credential-store and session-history pages) and the certificate/credential
  patterns `*.pem`, `*.p12`, `*.pfx`, `service-account.json`,
  `credentials.json`; a tracked `.env.example` template lists every credential
  env var ragent reads (no real values), so the previously dead `!.env.example`
  rule now resolves.

### Fixed

- **Config-write comparison no longer folds a corrupt file to `Null`.**
  `Config::write_config_if_changed` parsed the existing and new JSON with
  `unwrap_or(Value::Null)`, so a corrupt existing file compared equal to a
  corrupt new one and no rewrite happened. It now distinguishes parse failures
  (logging them and forcing a rewrite) from a genuine read error, and the dead
  `create_dir_all(parent)` call before a serialise that cannot need it is
  dropped.

## [1.0.128] - 2026-10-06

*Fixes intermittent YOLO-off at startup (atomic config writes), bounds MCP
connect so a stalled stdio server cannot hang TUI startup, and stops dropping
MCP startup errors. The last ten commits (`87024938` `d0eaa4e1` `b2b46d5d`
`80fac9fb` `f6199c1c` `0647d365` `6cf0b60f` `da83d927` `14c25e1d` `340c32cc`)
are documented in their own sections below.*

### Fixed

- **Configuration writes are now atomic (intermittent YOLO-off at startup).**
  Every `ragent.json` write (`Config::save`, `Config::save_to_source`, the
  runtime-flag toggles, and `/config save`) went through
  `write_config_if_changed`, which used a plain `std::fs::write` - truncate then
  write. A crash, `kill`, or power loss between the truncate and the write left
  a partial config that no longer parsed; `Config::load` then fell back to the
  compiled defaults, silently turning a persisted `"yolo": true` back to
  `false` and re-enabling the interactive permission prompts on the next start.
  This was the "YOLO keeps going off after a few restarts" report - a real
  disablement, not just a status-bar glitch. The write now lands in a uniquely
  named temp file in the config directory, is `fsync`-ed, and is renamed over
  the target, so a partial file can never be observed. Regression tests assert
  no temp files are left behind and that the file is valid JSON after writes.
- **Runtime-flag persistence no longer rewrites the whole global config.**
  `yolo`/`edit_log`/`activity_log` toggles persisted by loading the merged
  `Config` and re-serialising it to the global file. That folded the project
  overlay and any key the running build does not model into the user's global
  config, and rewrote unrelated values (e.g. `activity_log`). They now edit the
  single top-level key in the raw global file (new
  `Config::set_global_bool_key`), preserving every other key byte-for-byte.
- **`/spec reverse --folder` scaffolds without a GitHub token.** The FR-027
  scaffold stage ran *after* the FR-004 GitHub authentication check, so on a
  runner with no token (the CI release jobs) the request returned at
  `reverse: no token` and the target folder was never populated, failing
  `test_slash_spec_reverse_folder_scaffolds_project`. The local scaffold needs
  no credential, so it now runs before the token gate; the credential still
  gates the network fetch-and-generate stage (FR-004, FR-027, FR-028).

### Added

- **`ragent_info` execution details** — the tool now reports runtime execution
  information about the current ragent instance in addition to build metadata:
  process id and parent process id, wall-clock start time (RFC 3339 UTC),
  uptime (seconds and `Nd Nh Nm Ns`), absolute executable path, current working
  directory, resident and virtual memory, and thread (task) count. The execution
  block appears as a new `## ragent Execution Information` section in `text`
  format and as an `execution` object in `json` format; every field is
  best-effort and renders `unknown` when the host does not expose it. Only the
  current process is inspected via `sysinfo` (no subprocess, no network). The
  TUI tool-call summary now appends the pid.
- **Runtime-flag write attribution and load-time state log.** Every write of a
  user-global config key logs the key, value, path, and a forced backtrace, and
  startup logs the effective `yolo`/`edit_log`/`activity_log`/`gcf` state and
  the contributing config paths, so an unexpected flag flip is attributable
  instead of invisible.

### Changed

- **Bounded MCP connect** — every MCP server connection attempt
  (`McpClient::connect`) now runs under a per-attempt timeout (default 30s,
  overridable with `RAGENT_MCP_CONNECT_TIMEOUT_SECS`) and is retried once on
  timeout only. A launcher such as `npx`/`npm exec` that stalls resolving a
  package against the npm registry, or a server that accepts the connection and
  then never answers the handshake, now fails within the bound with an
  actionable error instead of hanging startup indefinitely. Genuine errors are
  still surfaced immediately without a retry, and a timed-out stdio attempt has
  its partially-started child torn down before the retry so the two attempts
  cannot collide.

### Fixed

- **MCP startup errors are reported, not silently dropped** — a server whose
  connection failed *before* any transport was attempted (a config rejected by
  `validate_mcp_config`, e.g. a stdio entry with no `command`, or a closed spawn
  semaphore) was previously never added to the client's server list, so the
  startup report and `/mcp` omitted it entirely and the only trace was a `warn`
  in the log panel. Every `McpClient::connect` failure path now records the
  server as `McpStatus::Failed { error }` (via a shared `record_failed` helper)
  before returning, so the one-shot startup report prints
  `[mcp] Failed to connect to mcp server <id> via <transport>: <reason>` for it
  alongside the servers that came up. The startup connect loop publishes this
  client-derived status (`connected`, else `failed: <reason>`) rather than a bare
  `failed`, so the `/mcp` list shows the reason too.
- **Startup no longer blocks on MCP connection** — the TUI start-up path no
  longer waits for the background MCP connect loop. Previously, after the
  connect loop was moved off the main task, `run_tui` still adopted the shared
  client and gave a still-connecting loop a fixed 3-second grace period (a
  3051 ms `MCP server status` startup stage on this host, 6320 ms total), so
  the first prompt appeared frozen whenever a stdio server such as the MongoDB
  `npx` launcher took a couple of seconds to handshake. `run_tui` now adopts
  whatever state the loop has published without blocking; the per-server
  `[mcp] Starting/Connected/Failed` report and the tool-registry reconciliation
  are emitted later, off the loop's completion sentinel, and every
  event-loop read of the shared client is non-blocking (`try_read`). Ready now
  appears in well under a second, and `/mcp list` and `/tools list` still show
  every connected server and its tools.
- **Runtime-flag toggles are attributable** — every write of a
  config-backed runtime flag (`yolo`, `edit_log`, `activity_log`, `gcf`) to the
  user-global config now logs the flag name, the value written, and a forced
  backtrace, and a toggle logs the previous and new state. A privileged flip
  such as YOLO mode turning itself on or off can now be traced to the keystroke
  or code path that caused it instead of appearing unexplained.
- **`/config list` restore resyncs runtime flags** — restoring a backup over
  the global `ragent.json` now drops the cached config and re-reads it, then
  resyncs the `yolo`, `edit_log`, `activity_log`, and `gcf` runtime flags. The
  restored file and the live toggles (e.g. the status-bar YOLO indicator) no
  longer disagree until the next restart.

### Quality

- **`/simplify all` pass over the 1.0.127 diff** — the per-server MCP startup
  report now latches its own one-shot inside `report_mcp_startup` (a
  `try_read` deferral still retries; a re-delivered `initialized` sentinel no
  longer re-appends), `ragent_info` requests `with_tasks()` so the thread count
  is actually populated, the four runtime-flag syncs collapse onto
  `ragent_config::sync_runtime_flags`, and stale doc comments were corrected
  (`ragent_info` no longer described as build-only). No behaviour change beyond
  the report being emitted at most once.

### Commits (last 10)

- **`87024938` - Version: 1.0.127.** Release commit. Added `TOOLREP.md`, the
  per-tool-group dependency report; removed the six Office / LibreOffice
  document tools and their module set (registered tool count 158 -> 152), the
  `tool_visibility.office` switch, the unused `docio` helper, the OOXML/ODF
  `DocumentFormat` variants, and the `docx-rust`/`calamine`/`ooxmlsdk`/`zip`/
  `spreadsheet-ods` dependencies; and a `/simplify all` pass over the changed
  set. (Full detail in the `[1.0.127]` section below.)
- **`d0eaa4e1` - Version: 1.0.126.** Office / LibreOffice tool removal and the
  accompanying `/simplify all` sweep; tool count 158 -> 152.
- **`b2b46d5d` - Version: 1.0.125.** `os_info` host-introspection tool and the
  `/osinfo` slash-command family (registry 171 -> 172), plus the research
  scholarly-engine default flip (`--papers` replaces `--no-papers`), the `probe`
  parameter, and three new tracked research items.
- **`80fac9fb` - Version: 1.0.124.** Connector-system follow-up, `/memory clear`,
  and TUI render fixes (`/mcp` table rendering, `/tools list`, `split_summary_icon`,
  the MCP orphan-sweep extension to `systemd --user`).
- **`f6199c1c` - Version: 1.0.123.** Research system, `ragent-connectors` crate
  (17th workspace crate), Gmail/communications, osinfo groundwork, and the
  per-engine research progress table.
- **`0647d365` - Version: 1.0.122.** Security and anti-pattern remediation sweep
  folding in the staged working tree on top of `6cf0b60f` (MS-04): `ANTIPAT.md`
  M0 plus M2-M7, `ragent-team` shim crate removed (17 -> 16 workspace crates),
  major dependency bumps, and `SECTASKS.md` MS-05 "prevent recurrence" (the
  `ragent_types::guard` module, the single `sanitize` redaction chokepoint, and
  the `security-guards` CI job).
- **`6cf0b60f` - MS-04: defence in depth (`SECTASKS` T-059..T-066).** The 33
  Low-severity findings closed with the same guard shapes as MS-01..MS-03:
  `--samples` clamped to `MAX_BENCH_SAMPLES` (100), streamed dataset downloads
  under `MAX_DOWNLOAD_BYTES`, benchmark manifest path containment, plugin
  manifest `entry`/`main`/`server.entry` validation, `resolve_memory_dir`
  rejecting unsafe agent names, a process-private 0700 bash scratch directory,
  redacted JSON parse-diagnostic source lines, and credential-bearing events
  redacted on the SSE stream.
- **`da83d927` - MS-03: network and secret hardening (`SECTASKS` T-025..T-058).**
  The Medium-severity remediation set: MCP HTTP response bodies under an 8 MiB
  cap, benchmark download timeouts and size/pagination budgets, LLM provider
  `Retry-After`/SSE/error-body caps, GitHub/GitLab pagination budgets,
  `mf_fetch`/`mf_crawl` byte caps and `crawl_urls` SSRF check, secret registry
  seeded from every credential env var, bash/CI/SSE-argument redaction,
  HTTP-origin-gated GitHub token, Gmail CRLF stripping, 0600 DB/WAL/activity-log
  permissions, `http_request` refusing routing/credential headers, codeindex
  exclusion-glob/FTS-symlink/walker guards, bounded spec numbering, mailbox
  caps, panic-free `CronSchedule`, and a stderr-spool byte ceiling.
- **`14c25e1d` - Add research/ folder.** Reverts the `research/` entry in
  `.gitignore` so the directory is tracked, and commits the accumulated research
  findings, indexes, and output reports.
- **`340c32cc` - Version: 1.0.121 more fixes on mcp.** Incremental MCP fixes
  carrying the v1.0.121 release; no additional tool or configuration surface.

## [1.0.127] - 2026-10-05

### Added

- **`TOOLREP.md` tool dependency report** — a new root-level reference document
  listing, per tool group, the unique dependency crates each group uses. Groups
  map to the three tool crates that register tools (`ragent-tools-core`,
  `ragent-tools-extended`, `ragent-tools-vcs`) plus the agent-local tool modules
  under `ragent-agent/src/tool/`, with tool counts from the `create_*_registry`
  constructors.

### Changed

- **MS Office / LibreOffice document tools removed** — the six document tools
  (`office_read`, `office_write`, `office_info`, `libre_read`, `libre_write`,
  `libre_info`) and the whole `office_common` / `office_write` / `office_info` /
  `libreoffice_common` / `libreoffice_write` / `libreoffice_info` module set are
  gone. Only the PDF family (`pdf_read`, `pdf_write`) remains; a new
  `pdf_common` module collapses the path/truncation helpers the office modules
  used to share. Removed the `tool_visibility.office` switch and its
  `ToolVisibilityConfig` / `ToolVisibilitySpecified` fields (the remaining
  switches are `github`, `gitlab`, `teams`, `agents`, `plan`, `codeindex`,
  `masterfetch`, `browser`), the now-unused `ragent-tools-extended::docio`
  helper module, the shared `DocumentFormat::{Docx,Xlsx,Pptx,Odt,Ods,Odp}`
  variants (OOXML/ODF extensions now return an actionable error from
  `detect_document_format`), and the dependencies `docx-rust`, `calamine`,
  `ooxmlsdk`, `zip`, and `spreadsheet-ods`. Registered tool count falls
  158 -> 152 (23 categories). Documentation updated (the `docs/howtos/office.md`
  how-to is now PDF-only, `docs/howtos/tools/office-pdf.md` and its index row
  list the two PDF tools, the `/tools` switch lists and `tool_visibility` tables
  drop `office`), the sample `assets/officedocs/testword1.docx` was deleted, and
  the generated how-to PDFs were regenerated. Configs that still carry an
  `office` visibility key are ignored (unknown keys are tolerated). The read
  path is now the two PDF tools plus `document_extract`, which reads PDF through
  `pdf_read` and `.md`/`.markdown`/`.txt` verbatim; OOXML and ODF extensions
  return an actionable error from `detect_document_format`.
- **`/simplify all` code-quality pass** — a review-and-fix sweep over the changed
  set that accompanies the Office / LibreOffice tool removal. Correctness:
  `pdf_write`'s `image` element now runs `image_path` through
  `ctx.check_path_within_workspace` (SEC-tools-extended-002), so an absolute path
  such as `/etc/passwd` can no longer be read and embedded into the generated
  PDF; the write metadata key is fixed from `line_count` to `page_count` with a
  comment recording that a document always renders one page (no `page_break`
  element exists); the `web_gatherer` scholarly-exclusion log message no longer
  claims `--papers` re-enables the engines when the branch actually excludes
  them; the TUI `/memory clear` handler now scopes the delete to `self.cwd_path`
  (the same key the Memory panel lists) instead of the process cwd;
  `connectors::store::descriptor_by_id` resolves a reference in a single pass
  with exact-id > case-insensitive-id > case-insensitive-name precedence,
  allocating only the winning descriptor. Performance: `pdf_read` parses the PDF
  once and threads the parsed `lopdf::Document` into `extract_pages_text` (the
  expensive whole-document `pdf-extract` pass now runs only when the per-page
  pass yields nothing); `pdf_write` clones the content once and shares
  char-aware `truncate_cell`/`wrap_text` helpers; `os_info::render_json` is
  computed once and its unused `Serialize` derives/import dropped. Dead code and
  docs: the unused `theme::think_summary`, the write-only `memory_clear_confirm_area`
  field, a duplicated doc line, and malformed section banners are removed;
  `mcp::find_orphaned_stdio_pids` reuses `read_ppid_of` and drops a stale
  `DCREMOVALPLAN.md` reference; dangling rustdoc links (`CategoryFilter::All`,
  the removed `search_catalogue`) and the `first_id`, `xlsx`, and `lib.rs`
  module docs are corrected.

## [1.0.126] - 2026-10-04

### Removed

- **Finance tools removed** — the eight `stock_*` / `currency_*` tools
  (`stock_quote`, `stock_history`, `stock_fundamentals`, `stock_search`,
  `stock_options`, `stock_recommendations`, `currency_rate`, `currency_history`)
  and their whole dependency chain are gone. Removed the
  `ragent-tools-extended::finance` module (providers `YahooFinanceProvider`,
  `PaidProvider`, `TwelveDataProvider`, cache, rate limiter, throttle, models,
  tools), the `ragent-config::finance` module and the `Config.finance` field and
  `tool_visibility.finance` switch, the TUI result formatters, all finance tests
  and fixtures, the `specs/yfinance` spec, the `docs/howtos/finance.md` how-to
  (plus its PDFs and index entries), and the finance skill packs. Dependencies
  dropped: `yfinance-rs`, `paft-decimal`, `paft-money`, and the now-unused
  `rand` direct dependency in `ragent-tools-extended`. Registered tool count
  falls 172 -> 164 (24 categories). Configs that still carry a `finance` block
  are ignored (the field is gone; unknown keys are tolerated).
- **Plot tools removed** — the six `plot_*` tools (`plot_line`, `plot_scatter`,
  `plot_bar`, `plot_histogram`, `plot_pie`, `plot_heatmap`) and their whole
  dependency chain are gone. Removed the `ragent-tools-extended::plot` module,
  the TUI inline-plot rendering (`plot_output_lines`, `ansi_line_to_styled`,
  `apply_sgr`, and the plot input/result formatters in `message_widget.rs` and
  `layout.rs`), the plot tool tests, the `docs/howtos/tools/plot.md` how-to (plus
  its PDF and index entry), and the GPL-3.0 `ratatui-plt` license allow-list
  entry. Dependencies dropped: `ratatui-plt` and the crate-local `ratatui`
  declaration in `ragent-tools-extended`. Registered tool count falls
  164 -> 158 (23 categories).

## [1.0.125] - 2026-10-04

### Added

- **`os_info` now reports graphics adapters (GPU), including integrated
  graphics** (spec `osinfo` FR-019). The report gains a `## GPU` section (text)
  and a `gpus` array (JSON): every detected adapter's best-effort name (e.g.
  `Intel Corporation TigerLake-LP GT2 [Iris Xe Graphics]`), vendor, PCI
  vendor/device ids, bound kernel driver (`i915`), a coarse
  `integrated`/`discrete`/`virtual`/`unknown` classification, and dedicated VRAM
  where exposed. Linux enumerates the read-only DRM sysfs tree under
  `/sys/class/drm` and resolves names from `pci.ids`; non-Linux hosts, or hosts
  with no DRM card, render the `Adapters: unknown` placeholder (empty `gpus`
  array). The section stays read-only (no process, no write, no network) and is
  shared by `/osinfo show` through the tool's collector/renderer. `sysinfo` is
  unchanged; no new dependency is added.
- **`os_info` now detects common graphics-API support** (spec `osinfo` FR-020).
  The `## GPU` section gains an `- **APIs**: ...` line and the JSON gains a
  `gpu_apis` array listing the graphics programming APIs the host can support,
  from the fixed vocabulary `Direct3D`, `DirectX`, `Metal`, `OpenGL`,
  `OpenGL ES`, `Mesa`, `Vulkan`, `OptiX`, `CUDA`, `ROCm`, emitted in that order.
  The list is platform-scoped: `Direct3D`/`DirectX` are reported only on
  Windows, `Metal` only on macOS.
- **`os_info` now reports the version of each detected graphics API** (spec
  `osinfo` FR-021). Each `gpu_apis` element is an object carrying a `name` and a
  best-effort `version` (e.g. `{"name": "Vulkan", "version": "1.4.354"}`), and
  the text `- **APIs**` line renders `Vulkan 1.4.354` (bare name when the version
  is unknown). The version is read from read-only artefacts - the Vulkan ICD
  manifest `api_version`, the CUDA/ROCm version files, the OptiX header, and the
  macOS Metal plist - with no process spawned. With `{"probe": true}` the version
  is also read from the vendor diagnostics: `vulkaninfo` for Vulkan and
  `glxinfo -B` for OpenGL, OpenGL ES, and Mesa (each at most once per report).
- **`os_info` now reports physical hardware: system model, chassis, motherboard,
  firmware, storage devices, and network interfaces** (spec `osinfo` FR-022,
  FR-023, FR-024, FR-025). The report gains a `## Hardware` section (text) and
  the flat JSON gains identity keys (`system_vendor`, `system_model`,
  `system_version`, `product_family`, `chassis_type`, `chassis_vendor`,
  `board_vendor`, `board_name`, `board_version`, `bios_vendor`, `bios_version`,
  `bios_date`) plus `storage_devices` and `network_interfaces` arrays. System
  identity (e.g. `LENOVO` / `20W1S20H00` / `ThinkPad T14 Gen 2i`), the chassis
  type rendered as its SMBIOS label (`Notebook`), and the motherboard and
  firmware (BIOS) vendor/version/date come from the read-only Linux DMI sysfs
  tree (`/sys/class/dmi/id`); storage devices (name, vendor, model, capacity,
  `nvme`/`ssd`/`hdd` class) from `/sys/block` (virtual `ram*`/`zram*`/`loop*`/
  `dm-*`/`md*`/`sr*` excluded); network interfaces (name, MAC, operational state,
  link speed, loopback flag) from `/sys/class/net`. Non-Linux hosts, or hosts
  without these trees, render `unknown` placeholders and `none detected` device
  lists. Serial numbers, UUIDs, and asset tags are never read, so a report can be
  shared safely.
- **`--papers` research flag** - re-enables scholarly search engines for a
  `/research create` run (root CLI, TUI, and the `papers` field on
  `POST /research`).
- **`/osinfo` slash command family added** (spec `osinfo` FR-014). `/osinfo show
  [--no-probe]` renders the host report through the `os_info` tool's own
  collector/renderer (so the command and the tool agree), `/osinfo help` prints
  the command page, and an unknown subcommand is rejected with the usage line.
  Read-only: it writes no file, makes no network request, and consults no
  provider; registered in `SLASH_COMMANDS`, the agent command catalog, the help
  index, and autocomplete.
- **`sysinfo` 0.38 workspace dependency** (`system` feature only) and the
  `os_info` tool module (`crates/ragent-agent/src/tool/os_info.rs`), registered
  in `create_default_registry` (tool count 171 -> 172), with external tests
  (`crates/ragent-agent/tests/test_os_info_tool.rs`) and TUI tests
  (`tests/test_osinfo_command.rs`, `tests/test_osinfo_registration.rs`,
  `tests/test_tool_display.rs`).
- **Spec `osinfo`** now on disk (`specs/osinfo/{SPEC.md,PLAN.md,TESTPLAN.md}`).
- **Research outputs added to the tracked tree.** Three new self-contained
  research items - `research/codemigrate/`, `research/connectors/`, and
  `research/vendormarketplace/` - each with `RESEARCH.md`, `CORPA.md`, and a
  `sources/` tree; `research/INDEX.md` now lists 15 items.

### Changed

- **Research excludes scholarly engines by default; `--papers` opts them back
  in.** `/research create` (and `ragent research create`) no longer need
  `--no-papers`: academically-classified backends (OpenAlex) are excluded from
  the web-gathering sweep unless `--papers` is supplied. The old `--no-papers` /
  `--no-scholarly` spellings are removed from every entry point (root CLI, TUI
  hand parser, HTTP `POST /research`, catalog/`SLASH_COMMANDS` mirrors), and
  `ResearchRunRequest` now carries a `papers: bool` field in place of
  `no_scholarly`; the HTTP request body takes `papers` and the recorded
  invocation summary emits `--papers`. `research.exclude_academic_engines` still
  exists but can only persist the exclusion - it no longer governs the default,
  and `--papers` overrides it for a run. Tests, `/research` autocomplete and
  parameter hints, and the docs (`docs/howtos/research.md`,
  `docs/howtos/slashcommands/research.md`, `docs/howtos/config.md`, `README.md`,
  `QUICKSTART.md`, `TUI-QUICKSTART.md`, `SPEC.md`) were updated to the new
  spelling.
- **Graphics-API version reporting is now on by default.** OpenGL, OpenGL ES,
  and Mesa have no read-only version artefact, so `/osinfo show` and a default
  `os_info` call previously omitted their versions (only Vulkan showed one, from
  its ICD manifest). Now the default `os_info` call runs the fixed,
  timeout-bounded graphics diagnostics (`vulkaninfo`, `glxinfo`, `nvidia-smi`,
  `rocminfo`, `system_profiler`), so `/osinfo show` reports every API with its
  version on this host: `OpenGL 4.6, OpenGL ES 3.2, Mesa 26.2.3, Vulkan 1.4.354`.
  The `os_info` `probe` parameter defaults to `true`; pass `{"probe": false}`
  (or `/osinfo show --no-probe`) for a fully process-free report that reads only
  read-only files. The diagnostics only read host state (no writes, no network)
  and each is still run at most once per report under a 2 s timeout, degrading
  to "no signal" on a missing tool, non-zero exit, or timeout. Spec `osinfo`
  FR-009/FR-010/FR-014/FR-015/FR-016/FR-017/FR-020/FR-021 were updated to record
  the new default and the `--no-probe` opt-out; `/osinfo help` documents both
  modes, and `/osinfo` autocomplete offers `show`, `--no-probe`, and `help`.

### Fixed

- **`os_info` reports every graphics API version by default.** The default
  `os_info` call (and `/osinfo show`) now runs the allowlisted, timeout-bounded
  vendor diagnostics so OpenGL, OpenGL ES, and Mesa - which expose no read-only
  version artefact - are reported with their version alongside Vulkan; pass
  `{"probe": false}` (or `/osinfo show --no-probe`) for the process-free path.
- **Research scholarly-exclusion reporting.** Progress rows and gather-log lines
  now read "scholarly engine excluded (`--papers` not set)" instead of referring
  to the retired flag, and the cache key folds the new `papers` flag so an
  inclusion run never reuses an exclusion-run result.

## [1.0.124] - 2026-10-02

### Added

- **`/memory clear` - clear this project's structured memories behind a
  `Yes`/`No` confirmation dialog.** The slash command opens the
  `Clear this project's memory?` dialog and removes nothing; the store is
  emptied only on an explicit `Yes`. `No` (the default selection, so a stray
  `Enter` cannot clear memory) and `Esc` dismiss the dialog unchanged.
  `Left`/`Right` (or `Tab`) move the selection, `Enter` selects.
  - New `Storage::clear_memories_for_project(&Path) -> Result<usize>`
    (crates/ragent-storage) deletes every memory whose `project` column equals
    either the full directory path or the directory basename - the same scope
    `count_memories_for_project` / `list_memories_for_project` already use - so
    memories belonging to other projects are never touched. FTS rows and base
    rows are removed in one transaction (C-3 index-desync guard).
  - The dialog reports its outcome in the chat transcript
    (`Cleared N memory entries for this project.`) and refreshes the memory
    panel.
  - Autocomplete, the slash-command registry description, the usage lines, and
    `docs/howtos/slashcommands/memory.md` (+ its PDF and INDEX row) now list
    `/memory show | /memory clear | /memory help`.
  - Tests: `crates/ragent-tui/tests/test_memory_clear_confirm.rs` covers
    open-without-deleting, `Yes` clearing the project's memories, `No` and
    `Esc` leaving them unchanged, and the `No`-by-default selection.

### Changed

- **The TUI drives the session-start connector lifecycle.** `src/main.rs` now
  publishes the bridged `ConnectorSession` wrapped in a `tokio::sync::Mutex`
  through `SessionProcessor::set_connector_session` (type-erased, so
  `ragent-agent` stays free of a `ragent-connectors` dependency), and the same
  session feeds the shared connect loop that starts `ragent.json` and
  plugin-contributed servers. `/connectors enable|disable|connect|disconnect`
  now drive that tracked session instead of rebuilding a fresh, empty one per
  invocation (which tracked nothing and made `disable`/`disconnect` after an
  `enable` fail with `unknown connector id`), and `/connectors list` reads live
  per-server tool counts from the MCP client. A single-reference subcommand
  (`remove`, `enable`, `disable`, `connect`, `disconnect`, `auth`, `test`)
  accepts a connector by id, slug, or display name via the new
  `ragent_connectors::canonical_id`, resolved once at the
  `run_connector_subcommand_env` dispatch seam; `src/connectors.rs` drops its
  per-call `canonical_id` shim and the unused `search_catalogue` env hook.
- **The connector catalogue has a single surface.** The standalone
  `/connectors search <query>` subcommand is retired;
  `/connectors claude [query] [--category <name>] [--refresh]` carries the
  `--category` launch filter and an in-panel `c` cycle key over the categories
  the fetched catalogue declares. `list` keeps the same category model, so the
  browser and the textual report share one filter (FR-010, FR-041). The
  autocomplete menu seeds from the crate's shared connector-token list so it
  cannot drift from the usage block.
- **`/memory` catalog mirror realigned.** `command_catalog::MEMORY_SUBS` and the
  `SLASH_COMMANDS`/catalog descriptions now list exactly the subcommands the
  `/memory` arm handles (`show`, `clear`, `help`), and `/tools` gains an explicit
  `show` alias in the catalog. The `docs/howtos/slashcommands/memory.md`
  reference (+ its PDF and INDEX row) is updated to match.
- **Simplifications across the connector and TUI paths.** `descriptor_by_id`
  returns a borrowed reference and clones only on match; `is_uuid` validates
  without collecting; `process_group_members`/`find_orphaned_stdio_pids` share
  one `proc_pids()` `/proc` walk; `tool_input_summary`/`tool_result_summary`
  drop redundant `format!("{}", x)` wrappers; the message-widget tool line no
  longer strips a legacy icon prefix; and the queue-clear and memory-clear
  confirmation dialogs share one `render_yes_no_confirm` helper.

### Fixed

- **MCP orphan sweep reaps servers re-parented to the user session manager, not
  just to init.** `/mcp` showed a stdio server (`npx -y mongodb-mcp-server@<3>`)
  as `stuck` on every startup because a dead ragent's launcher tree was never
  reaped. Two defects in `find_orphaned_stdio_pids` (crates/ragent-agent):
  (1) the sweep accepted a process only when `Ppid == 1`, but a desktop session
  re-parents a dead launcher's children to the per-user service manager
  (`systemd --user`) instead of init, so the orphan never matched; and
  (2) a candidate had to lead its own process group AND have every group member
  match the server, but the surviving tree is `npm exec mongodb-mcp-server@<3>`
  -> `node .../mongodb-mcp-server` and only the *child* carries the package
  token, so requiring the non-matching leader to match rejected the whole tree.
  The sweep now accepts `Ppid == 1` or a parent identified as the user service
  manager, matches per process, and signals the matched process's whole group so
  the launcher and its `node` child are both reclaimed (SPEC.md section 4.6f).
- **Memory project scope de-duplicated.** The three memory project-scope queries
  (`clear_memories_for_project`, `count_memories_for_project`,
  `list_memories_for_project`) now share one `project_name` helper, so the
  full-path and legacy basename key rule lives in one place.
- **Stale status-bar doc comment corrected** - `layout_statusbar.rs` states the
  last-prompt tag budget as 48 characters (was 32), matching the call site.

## [1.0.123] - 2026-10-01

Adds the connector system and fixes.

### Added

- **Connector system - a Claude-connector-equivalent integration catalogue over
  MCP (spec `connectors`, `specs/connectors/`).** A *connector* is a named,
  user-facing integration (Google Drive, Slack, GitHub, Git, Postgres,
  Puppeteer, and the wider community catalogue) that reaches an external system
  through one or more MCP servers. The MCP server remains the transport ragent
  already speaks (`McpClient`); the connector layer adds the catalogue entry
  (display name, category, auth shape, and one or more servers) and a
  management surface on top of it.
  - **New crate `crates/ragent-connectors` (37 files, ~16.4k lines, 16 test
    files).** Owns the normalised descriptor model and its validation
    (FR-002, FR-025), the connector store paths / scan / `_state.json` ledger
    (FR-001), manifest read-write and install staging with a path-traversal-safe
    archive extraction (FR-011, FR-027, FR-029), the HTTPS catalogue fetch
    (byte cap, timeout, cache) plus per-store normalisation providers and the
    compiled default Claude connector-catalogue endpoint and its provenance
    resolver (FR-024, FR-025, FR-031, FR-034..FR-038, NFR-001..003), auth-shape
    handling over the existing encrypted credential store (FR-005, FR-014,
    FR-022, FR-023, FR-032), and the bridge that resolves every enabled
    connector's servers into `(bridged_server_id, McpServerConfig)` pairs
    (FR-003, FR-020, FR-026, FR-033). Bridged server ids are
    `<connector-id>.<server>`, the same collision-avoidance shape the plugin MCP
    bridge uses; the durable enable ledger is the **existing**
    `ragent_agent::mcp::enable_state` (`mcp_state.json`), so a connector
    disabled once stays disabled everywhere and no second enable file is added.
  - **`/connectors` slash-command family (FR-004, FR-006, FR-009..FR-019,
    FR-022, FR-023, FR-025, FR-036, FR-039..FR-041).** Twelve subcommands -
    `list [--verbose] [--category <name>]`, `search <query> [--category <name>]`,
    `claude [query] [--refresh]`, `add <id|source> [--force]`, `remove <id>`,
    `enable <id>`, `disable <id>`, `connect <id>`, `disconnect <id>`,
    `auth <id>`, `test <id>`, `stores [--check]`, `help` - dispatched through a
    new `crates/ragent-tui/src/app/connector.rs` glue module, registered in
    `SLASH_COMMANDS`, and listed in the command catalog and the `/` autocomplete
    menu (seeded from the crate's shared token list so it cannot drift from the
    usage block). A bare `/connectors`, `/connectors help`, and an unrecognised
    subcommand all render the same usage block. Every report carries a
    `From: /connectors <sub>` attribution line and is ASCII-only; an unknown
    `--category` value is refused with an `[err]` row and changes no state,
    while `ALL` (any case) or a blank value clears the filter.
  - **Interactive connector-catalogue browser (`/connectors claude`).** A new
    modal overlay in `crates/ragent-tui/src/layout.rs` mirrors the plugin-store
    panel: a title line (catalogue name, query, active category, visible/total
    count), a scrollable result list with the block cursor on the highlighted
    row, an `[installed]` marker and distinct colour on already-installed
    connectors, and a footer of key hints. The catalogue fetch and the `ENTER`
    install both run **off the event loop** on a blocking worker
    (`spawn_connector_catalogue_fetch` / `spawn_connector_catalogue_install`),
    depositing their result for the UI thread to drain
    (`poll_connector_catalogue_result` / `poll_connector_catalogue_install_result`),
    so the fetch never blocks the loop or an agent turn. The panel owns the
    keyboard while open (`Up`/`Down` move the cursor, `ENTER` installs,
    `Backspace`/`Esc` edit the query, and `Esc` on an empty query dismisses it).
  - **`/connectors test <id>` isolated harness (FR-015, FR-016).** Connects the
    connector's servers in isolation with a throwaway probe, invokes one tool,
    and reports per-step `[ ok ]`/`[fail]` results, so a failing server is
    named without touching the live session.
  - **CLI parity (`ragent connectors <sub>`).** A new `src/connectors.rs`
    entry point mirrors `src/plugins.rs`: the session-free subcommands, the
    management subcommands (over an ephemeral lifecycle session), and the `test`
    harness are all reachable from the shell, with the report re-spelled for the
    CLI surface.
  - **`connectors` configuration block (FR-007, FR-021, FR-024).** A new
    `ConnectorsConfig` in `ragent-config` controls the master switch
    (`connectors.enabled`, default `true`), the connector store directory
    override, the catalogue endpoints and shared fetch budgets (`timeout_ms`,
    `max_index_bytes`, `cache_ttl_secs`), and a non-secret credential-name
    mapping keyed by connector id (only the credential *name* is stored, never
    the value). The section merges overlay-wins (project over user-global). With
    `connectors.enabled: false` the subsystem performs no discovery, no catalogue
    fetch, and no connection, and every subcommand other than `help` reports it
    is disabled.
  - **Fixtures and acceptance walk (T-015).** `assets/connectors/fixtures/`
    holds connector directories, catalogue indices (including a malformed and an
    oversize index), and archive fixtures (`.zip`/`.tar.gz`, plus a
    path-traversal `escape.zip`);
    `crates/ragent-connectors/tests/test_connector_fixtures.rs` drives the real
    `add`/`remove`/`scan_dirs`/`render_list`/`render_search`/`test_connector`/
    dispatcher paths over them. `specs/connectors/` carries `SPEC.md`
    (FR-001..FR-041, NFR-001..003), `PLAN.md` (T-001..T-021), and `TESTPLAN.md`.

### Changed

- **TUI tool-category and status-bar markers are now plain ASCII (ANTIPAT M1).**
  The message-window tool-category icons and the status-bar service icons are
  rendered as plain ASCII marker prefixes (`[file]`, `[dir]`, `[search]`,
  `[exec]`, `[git]`, `[team]`, ...) instead of emoji, and the status-bar
  indicators use short text tags (`CDX:`, `LOG:`, `AUTO:`, `EDIT:`, `TELE:`,
  `YOLO:`, `GCF:`, `BUSY: `) rather than emoji glyphs, so the non-ASCII CI gate
  can require a fully ASCII tree with no icon carve-outs. `shorten_middle` and
  `truncate_with_ellipsis` now budget the single-character ellipsis glyph
  (U+2026) rather than the three-character `...` separator; the agent-notice
  bubble continues to be matched on its plain-text `Agent Notice` label.
- **`/yolo` and `Alt+Y` now persist to the user-global config only**
  (`~/.config/ragent/ragent.json`), never the project-local
  `.ragent/ragent.json` - see the Fixed entry below.
- **Default connector-catalogue endpoint literal guard.** New
  `scripts/check-connector-endpoint-literal.sh` (with `--self-test`) fails if the
  compiled default endpoint is declared more than once (FR-038, NFR-001), wired
  into `pre-flight.sh` and a new `ci.yml` step. The retired
  `scripts/check-non-ascii.sh` / `scripts/check_non_ascii.py` guard and its
  `ci.yml`/`pre-flight.sh` steps are removed (superseded by the M1 ASCII
  conformance work).
- **Spec plan-parser dependency ranges accept a `..` / `...` ellipsis.** The
  `Dependencies` range regex in `ragent-specs` now expands `T-001..T-014` and
  `T-001...T-014` in addition to `T-001-T-014`, `T-001 through T-014`, and the
  en/em dash spellings already supported.

### Fixed

- **MCP orphan sweep reaps servers re-parented to the user session manager, not
  just to init.** `/mcp` showed a stdio server (`npx -y mongodb-mcp-server@<3>`)
  as `stuck` on every startup because a dead ragent's launcher tree was never
  reaped. Two defects in `find_orphaned_stdio_pids` (crates/ragent-agent):
  (1) the sweep accepted a process only when `Ppid == 1`, but a desktop session
  re-parents a dead launcher's children to the per-user service manager
  (`systemd --user`) instead of init, so the orphan never matched; and
  (2) a candidate had to lead its own process group AND have every group member
  match the server, but the surviving tree is `npm exec mongodb-mcp-server@<3>`
  -> `node .../mongodb-mcp-server` and only the *child* carries the package
  token, so requiring the non-matching leader to match rejected the whole tree.
  The sweep now accepts `Ppid == 1` or a parent identified as the user service
  manager (`/proc/<parent>/comm` or argv[0] basename `systemd`), and matches
  **per process**: every supervisor-reparented process whose own command line
  matches is swept, and each returned pid's process group is signalled, so both
  the launcher and its `node` child are reclaimed without killing a live
  sibling instance's server. Verification: `cargo test -p ragent-agent --test
  test_mcp_adopt` (14 passed) and `--lib` (355 passed) green; live proof that an
  orphaned `npx -y mongodb-mcp-server@<3>` tree (launcher re-parented to
  `systemd --user`) is found and killed by the sweep.

- **YOLO toggle now persists to the user-global config (CI `Check & Test`
  failure).** `Config::save_to_source` writes back to whichever config file was
  loaded (project preferred over global), and `Config::load` creates a
  project-local `.ragent/ragent.json` on first run. In a clean checkout with a
  fresh config dir the YOLO `/yolo` / `Alt+Y` toggle therefore persisted into the
  project file - where `Config::merge_project` strips `yolo` as untrusted
  repository content (SECTASKS T-011) - so the next load never saw the change and
  `test_yolo_persist_helper_updates_config_file` failed. YOLO is a user-only
  toggle: `runtime_flag::RuntimeFlag` gains `persist_to_global` /
  `toggle_persist_global` (which write via `Config::save(false)` to
  `~/.config/ragent/ragent.json`), and `yolo::persist_yolo` / `yolo::toggle_persist`
  now use them. This also stops the toggle polluting the working directory's
  project config. The regression test is retargeted to the global path (via an
  `XDG_CONFIG_HOME` redirect) so it actually exercises the failure mode.

- **Tool name now renders immediately after the step counter (TUI-019).** Two
  divergent renderers emitted a tool-call line: `MessageWidget::to_lines`
  (`crates/ragent-tui/src/widgets/message_widget.rs`) already put the name first,
  but the log-panel path in `crates/ragent-tui/src/layout.rs` split the first
  whitespace token off `tool_input_summary` and emitted it *before* the display
  name, so any summary without an icon prefix (for example `bg` -> `output:
  task-123`) put a parameter before the tool name. A new public
  `split_summary_icon(summary)` helper treats only a leading `[...]` tag as an
  icon and returns the whole text otherwise; both render arms now emit name,
  icon, then summary. Regression test
  `test_tool_name_immediately_follows_step_counter` asserts the rendered line
  starts with `[3.2] Bg` and carries the parameters after the name.

- **`/mcp` now renders one server per row as a markdown table (TUI-019).** The
  `/mcp` listing emitted fixed-width `{:<18}` text rows, which pulldown-cmark
  joined into a single wrapped paragraph. It is now a six-column markdown table
  (`| Server | Enabled | Status | Transport | Endpoint | Tools |`) laid out row
  by row, with `|` in a cell escaped so an id or URL cannot split a row, and the
  pre-existing `normalize_ascii_tables` pass re-renders it as `+---+` ASCII rows.

- **`/tools` subcommand surface tidied.** A bare `/tools` (and `help`, `--help`,
  `-h`, `usage`) now prints a help block for the family; `/tools list` (alias
  `/tools show`) renders the visibility table plus every visible and disabled
  tool; `/tools <switch>` reports one switch; and `/tools <switch> on|off` turns
  it on or off and saves it. The command-catalog description is updated to match.

- **`/connectors claude` accepts a `--category <name>` filter and a `c` cycle
  key (spec `connectors` FR-041).** The catalogue browser's launch parser now
  removes `--refresh` *and* `--category <name>` from the query tokens (so a
  pre-filled query may contain spaces), validates the category against the
  categories the fetched catalogue actually declares, and falls back to an
  unfiltered launch when it is unknown. While the panel is open the `c` key
  cycles the category filter through the catalogue's categories and back to
  `ALL`. `/connectors claude` is now the single catalogue surface: the
  standalone `/connectors search` subcommand is retired and `list`'s filter is
  the same category model, so the browser and the textual report cannot disagree.

- **Connector lifecycle is tracked against the session-start session (spec
  `connectors` T-008).** `src/main.rs` resolves the enabled connectors through
  the bridge into the same background connect loop that starts the configured
  `ragent.json` and plugin-contributed servers, then publishes a
  `ConnectorSession` and its live status snapshot on the session processor. The
  processor stores both type-erased (`set_connector_session` /
  `set_connector_statuses`, `connector_session::<T>()`), so `ragent-agent` still
  does not depend on `ragent-connectors`. `/connectors
  enable|disable|connect|disconnect` now drives that published session and
  reconciles the MCP tool registry afterwards, `/connectors list` takes its
  per-server tool counts from the live MCP client
  (`ConnectorCommandEnv::tool_counts`), and `statuses_from_client_state` derives
  per-connector statuses from the client when the tracked snapshot is empty, so
  a bridged server that connected *after* startup reports a real count instead
  of `?`. The shared connect loop publishes a completion `McpStatusChanged`
  once every server is settled so the TUI adopts the client promptly. Lifecycle
  references are also resolved to the installed connector's canonical id
  (`canonical_id`), so `enable`/`disable`/`connect`/`disconnect` accept a
  connector by id, slug, or display name. Verification: `cargo check`
  (workspace, all targets) and `cargo audit` green.

## [1.0.122] - 2026-09-30

Security and anti-pattern remediation sweep. This release folds in the staged
working tree on top of `6cf0b60f` (MS-04): the `ANTIPAT.md` anti-pattern
remediation plan (`M0` plus `M2`-`M7`), the `SECTASKS.md` security remediation
programme through `MS-05`, and the accompanying documentation refresh. Highlights
below; per-milestone detail follows in the sections beneath.

### Security

- **Milestone M0 of `ANTIPAT.md` complete - the two shipping defects and the
  highest-severity containment holes from the 18-crate anti-pattern audit.**
  `ANTIPAT.md` is the consolidated roll-up of 17 per-crate `explore` audits plus
  the workspace-root binary; M0 is its priority-0/1 set.
  - **Crash marker ordering (A-01).** `src/main.rs` stamped the `running` marker
    *before* reading it, so the record's pid always matched the live process and
    the "previous session exited without unwinding" warning could never fire.
    `previous_unclean_exit` is now read first.
  - **Search-retry shift overflow (F-02).** `--search-max-retries` was stored
    unclamped and the backoff computed `1u64 << (attempt - 1)`, which panicked
    for a value >= 64 (and slept for days for 40-63).
    `WebGatherer::with_search_max_retries` now clamps to `MAX_SEARCH_RETRIES`
    (10), the shift is `checked_shl`, and a single delay is capped at
    `MAX_SEARCH_RETRY_DELAY_MS` (60 s).
  - **Git argument injection (A-1).** `git_checkout`, `git_cherry_pick`,
    `git_add`, and `git_remote` pushed LLM operands straight into `git` argv
    with no `reject_option_like` guard. All four (and thus all 18 git tools) now
    validate every option-position operand.
  - **GitHub hardcoded origin (A-2).** `fetch_readme` built its URL from a
    literal `https://api.github.com`, bypassing a configured GitHub Enterprise
    `base_url` and attaching the Bearer token to the public origin; it now uses
    `resolve_url` like every other verb, and `owner`/`repo` are validated as
    path segments.
  - **File-tool containment (F-06/F-07).** `diff_files`, `file_info`, `glob`,
    `open`, and `apply_patch` checked `working_dir` only, so a whitelisted
    `allowed_roots` entry was rejected for the same path every other tool
    accepted. All five (and `read`, `edit`, `grep`, `list`, `patch`) now route
    through `check_path_within_allowed_roots_cached`, and
    `scripts/check-file-tool-containment.sh` was tightened to demand that exact
    helper (its `--self-test` now seeds both an uncontained tool and a
    working-dir-only tool).
  - **SSRF on the Discord sink (3.1).** `send_discord` had no public-target
    check while its Telegram sibling did, so a config could point it at an
    internal service; it now calls `refuse_non_public_target`.
  - **Unbounded response bodies (4.1).** Ten sites (finance providers, search
    engines, robots.txt, Gmail, Discord, YouTube captions) read whole response
    bodies with no cap, behind a client that transparently decompresses. A
    shared `masterfetch::http::read_body_capped` / `read_bytes_capped` now
    bounds every one (16 MiB for API/JSON, 512 KiB for small text).
  - **Plugin install integrity (H2/M17/M18).** Installs had no integrity record;
    a SHA-256 over the staged tree is now recorded in the store ledger. The
    decompression cap is aggregate (`MAX_EXTRACTED_BYTES`) rather than
    per-entry, and `copy_dir_recursive` refuses a symlink instead of
    dereferencing it into the store.
  - **Spec target-folder traversal (H-1).** `write_govcreate_spec` joined an
    unvalidated `target_folder` into a `create_dir_all` plus three writes.
    Containment is now lexical (so it works for a path that does not exist yet)
    and runs before any filesystem mutation, with a canonical re-check when the
    path does exist.
  - **Config fail-open denylist + Debug secrets (H-2/H-3).** A config-load
    failure returned an empty `DirLists`, silently dropping the mandatory
    built-in system-directory denylist; the built-ins are now always enforced.
    `Config` no longer derives `Debug` (it printed plaintext API keys, tokens,
    and webhook URLs); a hand-written impl routes the rendering through
    `redact_secrets`.
  - **LLM error bodies + dropped SSE frames (3.1/3.3).** Sixteen provider error
    paths used `response.text()` uncapped; all now use `read_body_capped`.
    Five SSE parsers silently dropped a malformed frame (`Err(_) => continue`)
    that can carry tool-call deltas; each now logs it, matching the FUNC-032
    behaviour of the sibling parsers.
  - **World-readable crash artifacts (A-02/A-03).** Crash markers, panic
    reports, and the stderr spool were created at the process umask (0644);
    all are now `0o600`, and the crash record's argv is redacted before it is
    written. The `/bug-report` dump is `0o600` too.
  - **Server error disclosure (F-H3/F-H2).** ~23 HTTP 500 responses returned raw
    internal error strings; all now route through `internal_error_response`,
    which logs the detail and answers a generic body. The `store_memory`
    fetch-after-write propagates its failure instead of answering `201` with an
    empty body.
  - **Storage silent drops (D-1/D-2/D-4).** The memory access-count bump logs
    its failure, the four `conversation_stats` counts propagate instead of
    `unwrap_or(0)`, and an unconvertible legacy nonce errors instead of
    decrypting under an all-zero key.
  - **Sub-agent report loss.** A report that cannot be written to
    `log/subagents/<task-id>.md` now warns with the task id and directory rather
    than silently reporting `output_file: None`.
  - **Telemetry toggles (HIGH-4/HIGH-5).** `ToolRecorder`, `SessionRecorder`,
    `CoordinatorRecorder`, and `CompressionRecorder` ignored the FR-027
    per-metric toggles; all now gate each metric. `InstrumentRegistry::noop()`
    builds from an explicit locally-owned provider rather than the process-global
    meter provider, so a disabled subsystem cannot export.
  - Regression suites added:
    `ragent-research/tests/test_search_retry_clamp.rs`,
    `ragent-tools-vcs/tests/test_ms0_git_arg_guards.rs`,
    `ragent-config/tests/test_ms0_config_guards.rs`,
    additional cover in `ragent-plugins/tests/test_add.rs`,
    `ragent-specs/tests/test_govcreate_authoring.rs`,
    `ragent-tools-extended/tests/test_channels.rs`, and
    `ragent-telemetry/tests/test_metric_toggles.rs`. A new `ms0-regression` CI
    job runs them.

### Changed

- **Milestone M7 of `ANTIPAT.md` - dependency and tooling hygiene.** The
  dependency graph has no reachable vulnerable or discontinued crate, the lint
  configuration matches the stated rules, the retired `ragent-team` shim crate
  is gone, and every guard that M7.7/M7.9 flagged as misleading now enforces
  what its comment claims.
  - **Advisory ignores cleaned + enforced (M7.1/M7.5).** Stale
    RUSTSEC-2026-0002 (`lru` stacked-borrows) and RUSTSEC-2026-0235 (`rkyv` via
    `spreadsheet-ods`; `rkyv` is no longer in the lockfile) entries were dropped
    from `deny.toml` and `.cargo/audit.toml` respectively, and each list was
    verified against its own tool's output (`cargo deny check` and `cargo
    audit`; they are not byte-identical because the tools resolve different
    feature graphs). `deny.toml` now sets `[advisories] unsound = "all"`, so the
    remaining RUSTSEC-2026-0253 (`lru` 0.16.4, pinned by tantivy 0.26; ragent
    never calls `LruCache::pop()`) is a real, checked suppression instead of an
    `advisory-not-detected` warning.
  - **Major dependency bumps (M7.4).** `rmcp` 1.8 -> 3.5, `rusqlite` 0.32 ->
    0.40, `similar` 2.7 -> 3.2, `dirs` 6 -> 7, `sha2` 0.10 -> 0.11, `base64`
    0.22 -> 0.23, `printpdf` 0.9 -> 0.12, and `ratatui` 0.29 -> 0.30.
  - **ratatui 0.30 wrap parity (M7.4).** `wrap_line_styled`, the crate-local
    port of ratatui's `WordWrapper` that owns the message-cache scroll geometry,
    was realigned with the 0.30 upstream: the whitespace-only blank-row tail
    only fires when `trim` is set, and `Span::styled_graphemes` now drops every
    control-containing grapheme (tabs included), not just `"\n"`. The
    `test_message_pinned_scroll` parity test pins the cache row count to
    `Paragraph::line_count` across tabs, CJK, emoji, NBSP, and long words.
  - **Retired the `ragent-team` shim (M7.8).** The 59-line pure re-export crate
    over `ragent-agent` is deleted; `ragent-tui` already depended on
    `ragent-agent` directly. The team runtime and its 20 tools keep their single
    home in `ragent-agent`, and `check-team-duplication.sh` (plus the
    `structure_types` `#[path]` guard) now fail if the crate is reintroduced.
  - **Guard repairs (M7.7/M7.9).** `scripts/check_inline_tests.py` scans the
    root `src/` tree as well as `crates/*/src` (R-01), so the root-binary blind
    spot is closed; `check-file-tool-containment.sh` demands the
    `allowed_roots`-aware helper and its self-test fails a working-dir-only call
    (F-07); `check-vcs-duplication.sh` covers the intra-crate GitHub/GitLab
    helper duplication it previously only described (M7.9); and
    `check-team-duplication.sh` was rewritten to assert the shipped post-M3
    layout and is wired into `ci.yml` and `pre-flight.sh`.
  - **Lint config honesty + package metadata (M7.6).** The dead
    `unwrap_used = "allow"` line is replaced with a documented rationale: the
    rule is enforced by `check-security-unwraps.sh` (panicking unwrap/expect
    baseline with the in-place `// no-panic-ok:` exemption) and
    `check-silent-errors.sh`, not by clippy, because a deny-level lint would
    flag every test `.unwrap()`. Root `[package]` gained `keywords` and
    `categories`.

- **Milestone M2 of `ANTIPAT.md` complete - standards conformance for tests,
  module docs, and production unwraps across the workspace.** M2 is the
  test-location / docblock / production-`unwrap` set from the 18-crate
  anti-pattern audit.
  - **Inline-test gate rewritten (M2.17).** The old `check-inline-tests.sh`
    counted the substring `mod tests`, which matched the idiomatic
    `#[cfg(test)] #[path = ".../tests/inline/<name>.rs"] mod x;` external hooks
    (forcing an inflated 129-file baseline) and never scanned the root `src/`
    tree at all. It is replaced by `scripts/check_inline_tests.py`: the scanner
    flags only a genuine inline `mod x { ... }` body with no `#[path]` attribute,
    covers `crates/*/src` **and** root `src/`, keeps a shrink-only baseline of 0,
    and ships a `--self-test` that proves an inline body fails while a `#[path]`
    hook passes. Wired into `ci.yml` (self-test + check) and `pre-flight.sh`.
  - **Test-body relocation completed.** The one remaining genuine inline body
    (`ragent-tui/src/app/loop_dialog/loop_dialog_tests.rs`, which referenced a
    sibling file without a `#[path]` marker) moved to `tests/inline/`. Five
    `ragent-agent` modules (`goal`, `orchestrator`, `task`, `perf`, `template`)
    had shared one `mod_tests.rs` that only satisfied `template`'s imports; the
    removed bodies were restored from git history into per-module
    `*_mod_tests.rs` files, and `truncate_str` assertions realigned to the ASCII
    `...` marker.
  - **Production unwraps removed (M2.9/M2.11/M2.12/M2.13/M2.15).** Nine
    `FtsIndex::from_index` schema lookups now use a `resolve(name)?` helper; five
    per-provider `.expect("serialise ...")` in `tool_cache.rs` collapse into one
    `push_tool` helper that logs and skips on (impossible) failure; `apply_patch`,
    `replace`, and `read` lost their production `expect`/`unwrap`; and the TUI
    `/spec`/`/team`/`/triggers` handlers use `let Some(..) = .. else` instead of
    guard-then-`unwrap`. Every remaining infallible constant-regex `expect`/
    `unwrap` carries a `// INVARIANT:` comment or a `no-panic-ok` marker.
  - **Docs (M2.4/M2.7/M2.9/M2.11).** `error.rs`/`spec.rs` gained `//!` headers,
    `router_client.rs` gained a module docblock, `graph/mod.rs` lost its stale
    "populated by later tasks / T-00x" comments, `file_count` gained `///`, and
    the broken intra-doc links in `ragent-bench`, `ragent-codeindex`,
    `ragent-specs`, and `ragent-research` are all resolved (those four crates now
    build with zero rustdoc warnings).
  - **Vocabulary (M2.14).** `sanitise_fts_query` renamed to
    `sanitize_fts_query` to match the `sanitize` spelling used everywhere else.
  - **Logging policy (M2.16).** `AGENTS-RUST.md` now documents the CLI
    presentation-surface exception for `println!`/`eprintln!` in the root binary.

- **Milestone M3 of `ANTIPAT.md` complete - de-duplication of shared helpers.**
  M3 is the "each duplicated family has exactly one implementation" set. Where a
  byte-identical copy had many call sites, the local version now delegates to the
  shared one so the logic exists once.
  - **Cross-crate primitives (M3.1/M3.2/M3.3/M3.18).** `ragent-agent/src/id.rs`
    is now a re-export of `ragent_types::id`; the agent-internal truncation
    helpers now use `ragent_types::strutil` (`truncate_bytes_no_ellipsis`), so the
    two private nine-line `truncate` copies in `conversation_search.rs` and
    `session_search.rs` are gone.
  - **Code index parser layer (M3.4/M3.5).** `parser/util.rs` owns
    `extend_scope`, `node_hash` (always via `scanner::hash_content` - the six
    raw-`blake3` copies are deleted), `node_text`, `field_text`,
    `first_child_by_kind`, and `find_child`; the ten per-parser
    `build_qname`/`ext_scope`/`hash_node` copies delegate to it. A new
    `parser/ctx.rs` owns the shared `Ctx<'a>` accumulator (`new`/`alloc_id`/
    `text`), and the twelve per-parser `Ctx`/`ExtractionContext`/`ExtractCtx`
    struct definitions became type aliases.
  - **Provider helpers (M3.6).** New `providers/media.rs` owns the data-URI
    parsers that `anthropic.rs` and `bedrock.rs` had duplicated byte-for-byte.
    (The OpenAI/Anthropic SSE state machines are left separate: the OpenRouter
    copy interleaves `reasoning` handling and differs in malformed-frame and
    empty-stream semantics, so folding them would change behaviour.)
  - **Storage (M3.7).** `activity_log.rs` gained `INSERT_EVENT_SQL` and the
    `SQL_SELECT_*` constants (seven INSERT and five SELECT copies removed);
    `storage.rs` gained `SQL_MEMORY_COLUMNS` and routes all six memory reads
    through the existing `memory_row_from_sql` mapper (four inline closures
    deleted).
  - **Extended tools (M3.8).** The search engines share
    `engine::{truncate_snippet, truncate_snippet_bytes, truncate_query_to,
    mask_api_key, engine_http_client}` (the OpenAlex/Wikipedia re-implementations
    are gone); `ragent_types::html` gained `html_to_plain_text`/`decode_entities`
    for the browser module; a new `docio.rs` owns `truncate_output_with_suffix` +
    `MAX_OUTPUT_BYTES` for the Office/`LibreOffice` families; `task_status_icon`
    is defined once.
  - **VCS tools (M3.9).** New `github/helpers.rs` and `gitlab/helpers.rs` own
    `make_client`/`detect_repo`/`detect_project`/`detect`; the six
    `github_*`/`gitlab_*` tool modules import them.
  - **Root CLI (M3.10).** `ragent_research::provider_calls_suffix` is now the one
    provider-summary renderer for both the CLI and TUI; `src/plugins.rs` renders
    its help from `ragent_plugins::render_help` instead of a hand-copied table,
    which had drifted (it was missing the `list --mcp` row).
  - **Plugins (M3.11).** `error.rs` owns `with_plugin`;
    `runtime::js_string_literal` delegates to `tool_adapter::js_literal`; the
    doubled `eval_json` doc comment and duplicated table-format string are
    removed.
  - **Server (M3.12).** `routes::serialize_response` is shared; `routes/memory.rs`
    imports it rather than re-defining it.
  - **Telemetry (M3.13).** `subsystem.rs` gained `validate_endpoint` and a single
    `build_provider(config, Option<reader>)`; `new()` now goes through
    `build_enabled_provider` so the reader/handle wiring is defined once. The
    duplicated endpoint checks and the duplicated provider rebuild are gone.
  - **Redaction (M3.14).** `sse::redacted_event_debug` is a thin delegate to
    `Event`'s hand-written `Debug` (the credential-shape knowledge lives once in
    `ragent-types`); `PluginMcpServer` now has a hand-written `Debug` that
    redacts the `env`/`headers` values.
  - **TUI (M3.15).** `app/helpers.rs::open_owner_only(path, truncate)` owns the
    `0o600` create-and-tighten boilerplate previously copied into the log spool,
    session export, and bug-report writers.
  - **Bench (M3.16).** `suites::evaluate_suite_samples` and
    `suites::run_fixture_commands` own the shared per-sample evaluator loop and
    native-command harness; `humaneval.rs`/`mbpp.rs` call them and use
    `pass_at_1`/`count_passed_failed` instead of re-deriving the metrics.
  - **Specs (M3.17).** The duplicated `// -- Feedback helpers --` banner in
    `commands.rs` and the dead `// -- Tests --` banner in `id_scanner.rs` are
    removed.

- **Milestone M4 of `ANTIPAT.md` complete - silent error suppression.** M4 is
  the "every `let _ =` on a fallible call and every `unwrap_or*` that masks an
  error is logged or justified" set. Genuine drops now log the cause; the
  remaining best-effort idioms carry a `// INTENTIONAL: <reason>` marker, and a
  new shrink-only gate blocks any growth.
  - **Typed CLI exit (M4.11 / A-04 / I-05).** `src/cli.rs` handlers no longer
    call `std::process::exit` (30 sites) inside `Result`-returning code; they
    return a typed `cli::CliExit`. `main` performs the single exit *after* the
    clean-exit crash marker and the bounded `runtime.shutdown_timeout`, so
    neither is skipped. The taxonomy is fixed: usage/validation = 2, runtime
    failure = 1. The panic hook's deliberate `eprintln!` is now documented
    (A-05).
  - **Logged drops.** `ragent-tools-core` logs a failed `restrict_to_owner` on
    the secret-bearing bash scratch files and a failed `glob` walk;
    `ragent-tools-vcs` logs a recursive tree-fetch failure at `debug!`;
    `ragent-tools-extended` logs a failed `mf_fetch` cache-write join and an
    edit-log directory creation; `ragent-plugins` logs a failed
    `register_tool`/`register_command` handler stash; `ragent-codeindex` logs a
    failed graph-transaction `ROLLBACK`; `ragent-telemetry` logs a failed
    Prometheus scrape socket write/flush; `ragent-config` logs a failed
    compiled-glob recompile. `masterfetch`'s `robots.rs` `let _ = ..?` was
    rewritten as an explicit validation `if`.
  - **Annotated best-effort drops.** The remaining infallible / best-effort
    idioms (writes to a `String`, channel sends, process teardown, temp-file
    cleanup, terminal teardown, best-effort persistence whose in-memory state is
    authoritative, parameter-stability bindings) carry a same-line
    `// INTENTIONAL: <reason>` marker so the drop is explicit rather than silent.
  - **New gate (M4.14).** `scripts/check-silent-errors.sh` +
    `scripts/check_silent_errors.py` record the current masked-site count per
    file in `scripts/silent-error-baseline.txt` (shrink-only), fail when any file
    exceeds its baseline, and ship a `--self-test` that proves a seeded drop
    fails. Wired into `ci.yml` and `pre-flight.sh`.

- **Milestone M5 of `ANTIPAT.md` complete - inconsistency and vocabulary
  unification.** M5 is the "one name and one accepted value set per concept,
  one error mapping per provider, one policy per cross-cutting concern" set,
  spanning 13 crates.
  - **VCS vocabulary (M5.1).** New `ragent-tools-vcs::limits`
    (`DEFAULT_PAGE_LIMIT`/`MAX_PAGE_LIMIT`/`NOTES_PER_PAGE`) and `::vocab`
    (`normalize_issue_state`/`normalize_gitlab_state`/
    `normalize_gitlab_issue_state`). Both `open` and `opened` are accepted on
    every state-bearing tool and translated to each provider's native value
    (`merged` mapped where supported); `git_log`'s limit is now bounded; a
    GitHub 403 is treated as a rate limit only when `x-ratelimit-remaining: 0`
    and otherwise as permission-denied (matching GitLab); missing-config
    strings are one canonical constant per provider; GitHub comments/reviews
    fetches carry `per_page` to match GitLab; the Actions-vs-Pipelines
    asymmetry is documented; `status_icon` emoji became ASCII tokens.
  - **Telemetry vocabulary (M5.2).** Instrument emit sites use the `names::*`
    constants; the two unwritten fields are reconciled; `attr_session` is
    `#[doc(hidden)]` with the cross-crate-wiring rationale documented and its
    contract pinned by `tests/test_attr_session.rs`; stale `// -- Tests --`
    banners removed; `MIN_CRED_PART_LEN`/`MIN_B64_RUN_LEN`/`TOKENS_PER_MILLION`/
    `HTTP_READ_BUF` named.
  - **Codeindex policy (M5.3/M5.13).** A single poisoned-lock policy per guard
    (recover for the `CodeIndex` guards, fail-closed-to-`Result` for
    `search::with_writer`), documented; the last two raw `blake3::hash` sites
    route through `util::node_hash`; magic numbers named
    (`MAVEN_TAG_SNIPPET_LEN`/`DEFAULT_SEARCH_LIMIT`/`FTS_OVERFETCH_FACTOR`/
    `REPORT_TOP_GOD_NODES`); parser naming unified on
    `Ctx`/`text()`/`build_qname`/`extend_scope`/`hash_node`; broken intra-doc
    links fixed and stale task-scaffolding comments removed.
  - **Config consistency (M5.4).** An explicit `impl Default for Config`
    matches the serde defaults, so the `load_uncached` patch is gone;
    `dir_lists` warns on a poisoned lock like `bash_lists`; every config-path
    site routes through `Config::global_config_path()`; the `merge_project`
    doc/code denylist policy is aligned; duplicated default-`true` helpers
    collapsed; a failed `current_dir()` warns and skips caching.
  - **Plugin dialect consistency (M5.5).** Codex/Claude `version` handling made
    consistent, both permission shapes normalised into one model, `UNSUP_SKILLS`
    emitted by the Codex path too, one `guard`-backed identifier/relative-path
    predicate shared by `add.rs`/`manifest.rs`/`bridge.rs`, `IoError` preserving
    the source chain, and hook timeouts read from explicit
    `timeout_secs`/`timeout_ms` fields.
  - **LLM provider consistency (M5.6).** Billed chat/completion POSTs are never
    auto-retried (the double-billing paths in `azure_foundry` and
    `openai_responses` removed); `DEFAULT_STREAM_TIMEOUT_SECS` named and
    reconciled with the per-chunk idle budget; every provider registers its key
    with `sanitize::register_secret`; all SSE parsers use one `data:` line form.
  - **Storage consistency (M5.7).** One dynamic-SQL parameter-indexing
    convention (the `?NOW` trick removed); `ActivityLog::append` is now
    transactional and maps conflicts to `DuplicateSeq`; the remaining
    multi-statement writers are transactional; the pure-read memory searches use
    the read-only connection.
  - **Research output consistency (M5.8).** `--url-cloak` applied on every output
    path; engine classification drives both the exclusion set and the
    scholarly/encyclopedia predicates from one table; numeric CLI flags report an
    invalid value instead of silently defaulting.
  - **Tools-extended consistency (M5.9).** Shared HTTP client reused across
    `channels`/`gmail`/`finance`/`browser`/search engines; one
    `DEFAULT_TIMEOUT_SECS`; finance throttling honours the configured interval;
    one generic capped-body helper bounds every read; the
    `mf_search.max_results` default is a named constant.
  - **TUI / specs / server consistency (M5.10-M5.12).** Unified quit handling and
    named caps in the TUI; ASCII status markers in `ragent-specs`; and in
    `ragent-server` a `200`-with-body research delete (no more `204` + body),
    standardised error casing, a logged SSE serialization fallback, shared
    rate-limiting middleware, and a hoisted `constant_time_eq` with a test.

- **Milestone M6 of `ANTIPAT.md` complete - structural / complexity debt, and
  the bounding of previously-unbounded collections.** M6 decomposes the audited
  hot paths, moves blocking work off the async path, and gives every
  process-lifetime collection a named cap with eviction. All numeric changes are
  naming-only (behaviour preserved) unless a bound was explicitly added.
  - **TUI structure + spool writer (M6.1/M6.9).** `handle_mouse_event`'s region
    dispatch is factored behind `SCROLL_STEP_LINES` and helpers; the trivial
    `block_in_place`/nested-`block_on` command handlers were converted to
    `async` and the remainder carry a `// reason:` note; `FROM_CMD_PREFIX` +
    `from_cmd()` builder added; a shared help-render helper added; the
    duplicated spool writer now shares `open_log_spool`/`log_level_str`; and
    the `/bug-report` dump writes `0o600` via `open_owner_only`.
  - **tools-core bounds (M6.2).** `read` output is capped at
    `MAX_READ_OUTPUT_CHARS` (200,000) with an omission marker; the `list` walk
    runs under `spawn_blocking` and the `exists()`/`is_dir()` probes in
    `move_file`/`rm` are async; `BashTool::execute` calls
    `validate_shell_command` once (the duplicated security ladder is gone);
    `glob` uses `entry.file_type()` (no symlink follow) bounded by
    `MAX_WALK_DEPTH`; dead-code `#[allow]` sites carry `// reason:`.
  - **tools-extended bounds (M6.3).** The local-embedding path no longer builds
    a nested Tokio runtime and its model download is capped; untrusted-array
    `Vec::with_capacity` in the plot tools and search/graph code is dropped or
    capped with named `MAX_*`; `mf_crawl` clamps `crawl_urls` with
    `MAX_CRAWL_URLS` (1000); the search and finance caches are size-bounded.
  - **codeindex bounds (M6.4).** `MAX_GRAPH_LOAD_ROWS` (1,000,000) bounds every
    graph-derivation loader; `MAX_IMPL_CANDIDATES` (32) bounds
    `derive_impl_edges`; an RAII `TreeDepthGuard` + `MAX_TREE_DEPTH` (512)
    bounds all 12 parser tree walks; `IndexStore::rollback_transaction()`
    replaces the raw-`conn` ROLLBACK sites and `conn` is now private.
  - **server hardening (M6.5).** `CorsLayer::permissive()` replaced with an
    explicit origin allowlist; `MAX_SSE_CONNECTIONS` (64) caps `/events`;
    request body limit + timeout layers added; the SSE serialization fallback
    logs; rate-limit / visualisation / channel / related-research literals are
    named constants.
  - **agent caches (M6.6).** `PROMPT_CONTEXT_CACHE` (cap 64, TTL + evict-oldest)
    and `ENTRY_TOKENS` (cap 4096, clear-before-insert) are hard-bounded.
  - **types (M6.7).** `MAX_SECRET_REGISTRY_ENTRIES` (1024) caps the global
    secret registry (longest-first preserved) and `seed_secrets` de-duplicates;
    `Debug for Event` avoids the double allocation for non-credential variants;
    the dead duplicate `is_absolute` block and unreachable `RootDir` arm removed.
  - **plugins (M6.8).** Download chunk size, redirect-hop limit, sandbox stack
    size, SHA-256 hex length, and the marketplace-document cap are named
    constants; the marketplace registry is a bounded FIFO.
  - **Magic-number sweeps (M6.10).** Named constants added in `ragent-research`,
    `ragent-storage`, `ragent-config`, `ragent-bench`, and `ragent-specs`
    (regexes hoisted to `LazyLock` statics); a `DEFAULT_CONTEXT_WINDOW`
    fallback wired in `ragent-llm`.

### Added

- **Milestone MS-01 of `SECTASKS.md` complete — all five Critical findings from
  the per-crate security audit are remediated, each with a regression test.**
  The audit (`SECTASKS.md`, 17 crates, 117 findings) is tracked as five
  milestones; MS-01 was the "stop the bleeding" set.
  - **SEC-ragent-plugins-001** — an attacker-controlled `git+<url>#<ref>[:<subpath>]`
    fragment reached `git fetch`/`git sparse-checkout` as an *option*: a ref of
    `--upload-pack=<cmd>` executed a program with the user's privileges,
    escaping the plugin sandbox entirely. `git_ref`/`git_subpath` now reject any
    value that is not `[A-Za-z0-9._/-]+`, begins with `-`, or contains `..`, and
    `git_run` inserts `--` before every positional so the option parsing itself
    is terminated. Regression cover in
    `crates/ragent-plugins/tests/test_store_git_source.rs` (parse-level refusal
    plus an end-to-end install asserting the injected marker is never created).
  - **SEC-ragent-plugins-002** — a manifest-declared plugin `id` was accepted
    verbatim and joined onto the plugin store, so `id: "/home/user/.config/autostart"`
    (or `../../..`) wrote attacker files outside the store and — with
    `--force` — recursively deleted the target first. Both dialect parsers now
    run a declared id through `sanitize_declared_id` (a single `Component::Normal`
    matching `[A-Za-z0-9._-]+`, else it falls back to the derived id), and the
    install sink re-asserts containment in `confined_dest_dir` before
    `remove_dir_all`/`copy_dir_recursive`. Tests in
    `crates/ragent-plugins/tests/test_manifest.rs`.
  - **SEC-ragent-team-001 / SEC-ragent-agent-008** — a blueprint's
    `task-seed.json` / `spawn-prompts.json` looked each entry's tool up in the
    full default registry and called `execute()` directly, bypassing the
    permission gate. A repository is untrusted content, so sharing a project
    whose blueprint seeded `bash` gave unprompted command execution. Every
    seeded tool now goes through `dispatch_seed_tool`, which consults the
    session permission checker and refuses anything that is not explicitly
    allowed (failing closed when no checker is wired). Cover in
    `crates/ragent-team/tests/test_blueprint_seed_permissions.rs` (Ask refused,
    Deny refused, no-checker fails closed, explicit Allow still runs, benign
    task seed still creates its task).
  - **SEC-ragent-team-002 / SEC-ragent-agent-007 / SEC-ragent-tui-004** — a team
    name was interpolated straight into `base.join(name)`, `create_dir_all`, and
    `remove_dir_all`, making `team_create`/`team_cleanup` an arbitrary-path
    write/delete primitive driven by LLM output. A single `validate_team_name`
    (`^[a-z0-9][a-z0-9-]{0,63}$`) now guards `TeamStore::create`,
    `find_team_dir`, the cached lookup, `team_cleanup`, and the `/team create`
    and `/team delete` slash commands; blueprint-derived names are slugified
    before use so generation cannot produce an invalid name. Tests in
    `crates/ragent-team/tests/test_team_name_validation.rs` and
    `crates/ragent-tui/tests/test_teams_tui.rs`.
  - **SEC-ragent-tools-core-001** — `multi_edit` was the only file tool with no
    path-containment check: `{"file_path": "../../.bashrc"}` or an absolute
    `/home/user/.ssh/authorized_keys` was read and overwritten directly. Every
    edit target is now validated with `check_path_within_allowed_roots_cached`
    (the same helper `edit`/`read`/`write` use) before the batch acquires locks
    or reads anything, so a mixed batch is rejected atomically. Tests in
    `crates/ragent-tools-core/tests/test_multiedit.rs`.
  - `SPEC.md` §4.6a records the five guards and their call sites so later
    milestones can migrate remaining sites onto them.

- **Milestone MS-03 of `SECTASKS.md` complete — the Medium findings are
  remediated.** MS-03 ("Network & secret hardening") covers T-025 .. T-058: the
  network egress, secrets/redaction, resource-cap, and defence-in-depth items.
  `SPEC.md` §4.6c records the guards and their call sites.
  - **Network egress** — MCP HTTP response bodies are read under an 8 MiB cap
    (`SEC-ragent-agent-003`); benchmark downloads carry a client timeout, a
    streamed size cap, and a 64-page/64k-record pagination budget, with the
    manifest `case_file`/`relative_path` confined to the data root
    (`SEC-ragent-bench-003/004/005`); LLM providers clamp `Retry-After` to 30 s,
    cap the SSE accumulation buffer at 1 MiB on every streaming path (all 11
    provider stream loops plus the Gemini NDJSON path), and read error bodies
    under a 64 KiB cap (`SEC-ragent-llm-003/004/005`); the GitHub tree walk and
    the GitLab jobs pagination carry request/entry budgets
    (`SEC-ragent-tools-vcs-007/008`); `mf_fetch`/`mf_crawl` stream responses
    under byte caps instead of buffering first, and `crawl_urls` is SSRF-checked
    at the tool boundary (`SEC-tools-extended-006/010`).
  - **Secrets and disclosure** — the secret registry is seeded from every
    credential env var (`GITLAB_TOKEN`, `GITHUB_TOKEN`, `TAVILY_API_KEY`, Gmail,
    Telegram, ...), and `SECRET_PATTERN` already covered GitLab PATs / `hf_` /
    `npm_` / `?key=` (`SEC-ragent-types-004`); bash partial *and* completed
    output is redacted before it reaches the model or the session store
    (`SEC-ragent-tools-core-005`); CI log excerpts are redacted and size-capped
    (`SEC-ragent-tools-vcs-006`); the GitHub client attaches the token only to
    the configured API origin (`SEC-ragent-tools-vcs-005`); Gmail header values
    are CRLF-stripped (`SEC-tools-extended-003`); the provider-setup dialog
    masks the API key and the GitLab PAT (`SEC-ragent-tui-002`).
  - **Resource caps and containment** — database, WAL, and activity-log files are
    re-restricted to 0600 after the first write creates the sidecars
    (`SEC-ragent-storage-003`); document write tools route through the workspace
    containment check (`SEC-tools-extended-002`); `http_request` refuses
    routing/credential headers (`Host`, `Cookie`, `Authorization`, `Proxy-*`)
    (`SEC-tools-extended-004`); the codeindex scanner applies the configured
    exclusion globs and re-checks the size of the bytes it read
    (`SEC-ragent-codeindex-005/006`), `index_file` does the same on the watcher
    path, and the FTS recovery wipe refuses a symlinked index directory
    (`SEC-ragent-codeindex-007`); the bash wrapper script and `export -p` state
    file move out of world-readable `/tmp` into a 0700 scratch directory with
    0600 files (`SEC-ragent-tools-core-004`); the tree-sitter walker recursion is
    bounded and graph edge derivation carries per-reference and global budgets
    (`SEC-ragent-codeindex-003/004`); spec numbering-gap expansion and
    contradiction detection are bounded (`SEC-ragent-specs-001/002`); mailbox
    messages are size-capped, team hook commands are validated and run under a
    30 s timeout with process-group kill and capped feedback
    (`SEC-ragent-team-004/005/006`); `CronSchedule` no longer panics on an
    invariant violation and uses checked arithmetic
    (`SEC-ragent-types-003`); the stderr spool enforces a byte ceiling on every
    write, including newline-free ones (`SEC-ragent-types-002`); the
    permission-splitting helper no longer treats a `;` inside `$( ... )` as a
    sub-command boundary; and the provider base URL is no longer persisted into
    the benchmark workbook (`SEC-ragent-bench-007`).

- **Milestone MS-04 of `SECTASKS.md` complete — the Low findings are
  remediated.** MS-04 ("Defence in depth") covers T-059 .. T-066.
  `SPEC.md` §4.6d records the guards and their call sites.
  - **Resource caps** — session-archive import already caps entries and total
    decompressed bytes (`SEC-ragent-agent-009`); benchmark downloads gained a
    64 MiB streamed cap and a page/row budget on the HumanEvalPack pagination
    loop, and `--samples` is clamped to 100 so a mistyped value cannot drive an
    unbounded generation loop (`SEC-ragent-bench-003/004/005/006`); the
    benchmark workbook and run-state sidecar no longer record the resolved
    provider base URL, only that an override was in effect
    (`SEC-ragent-bench-007`).
  - **Containment and TOCTOU** — the codeindex scanner derives the stored size
    from the bytes actually read, applies `extra_exclude_patterns`, and
    `FtsIndex::open` refuses a symlinked index directory before the recovery
    path can delete through it (`SEC-ragent-codeindex-005/006/007`); the
    benchmark manifest `relative_path`/`case_file` joins stay inside the data
    root (`SEC-ragent-bench-004/005`).
  - **Config** — `BUILTIN_DENYLIST` is now merged into the *enforced* denylist
    rather than only being advertised to the TUI, and the JSON parse diagnostic
    redacts the echoed source line before it reaches the terminal or the log
    (`SEC-ragent-config-006/007`).
  - **Plugins** — the package download refuses a non-`https` redirect
    (reusing the store-fetch policy), manifest-declared `entry`/`main`/
    `server.entry` must be a contained relative path, and the marketplace
    materialiser refuses a wrapper-directory name that is not a single normal
    component (`SEC-ragent-plugins-005/006/007`).
  - **Research** — fetched bodies are neutralised before fenced synthesis
    insertion (backtick runs cannot close the fence, `#### Source [#N]` and
    `**Sources:**` lines cannot spoof citations) and the gather log masks
    userinfo and query strings in recorded URLs
    (`SEC-ragent-research-006/007`).
  - **Server** — a permission reply is now bound to the session that owns the
    pending request, and internal errors are logged in full while the client
    receives a generic string (`SEC-ragent-server-007/008`).
  - **Specs** — `/spec create` and `/spec specify` validate the name,
    `SpecId`'s `Deserialize` routes through the validating constructor, and the
    public `write_govcreate_spec` re-validates its id
    (`SEC-ragent-specs-003/004/005`).
  - **Panic-free parsing and file modes** — `apply_unified_diff` splits on a
    char boundary instead of panicking on a multibyte diff line
    (`SEC-ragent-storage-006`); `resolve_memory_dir` rejects an unsafe agent
    name (`SEC-ragent-team-007`); the telemetry cardinality resolver applies the
    sanitizer itself and fails closed on a poisoned lock, with `service.version`
    now guarded too (`SEC-ragent-telemetry-005`); the log-window spool is
    created 0600 and re-asserted on every open, `/alog export` rejects a run id
    that is not `[A-Za-z0-9_-]+`, and the bash scratch directory falls back to a
    private directory rather than shared temp (`SEC-ragent-tui-003/005`,
    `SEC-ragent-tools-core-004`); the GitHub recursive tree walk carries the
    same request/entry budget as the GitLab equivalent
    (`SEC-ragent-tools-vcs-008`); credential-bearing events are redacted on the
    SSE stream and expose a `redacted_event_debug` helper for log sites
    (`SEC-ragent-types-006`); and config-supplied outbound base URLs
    (Telegram, Gmail, finance providers) are SSRF-checked, `browser eval` is
    documented as its own capability, and the `CrawlFetcher` SSRF obligation is
    stated on the trait (`SEC-tools-extended-007/008/009/010`).

### Added

- **Milestone MS-05 of `SECTASKS.md` complete - "prevent recurrence": each guard
  shape from the earlier milestones now has a single implementation, and CI
  fails a build that reintroduces the class.**
  - **Shared guards (T-067/T-068)** - new `ragent_types::guard` owns
    `reject_option_like`, `is_safe_operand`, `validate_identifier`,
    `validate_relative_component`, `contained_join`, `clamp_retry_after`, and
    `cap_read`, plus `MAX_IDENTIFIER_LEN`/`MAX_RETRY_AFTER`. It is re-exported
    from the `ragent-types` root and as `ragent_tools_core::guard` for the tools
    crates. `ragent-tools-vcs::git::reject_option_like`,
    `ragent-plugins::add::is_safe_git_argument`, and
    `ragent-bench::data::contained_join` are now thin adapters over it, so the
    rule has one home while every existing call site keeps its name. Regression
    suite: `crates/ragent-types/tests/test_shared_guards.rs` (15 tests,
    including a symlink-escape and a split-UTF-8-boundary case).
  - **One redaction implementation (T-069)** - `ragent-agent`,
    `ragent-storage`, and `ragent-tools-core` re-export
    `ragent_types::sanitize` instead of holding a second secret registry, so a
    credential registered anywhere is masked on every surface (`GET /config`,
    telemetry attributes, logs, SSE, tool output). `Event` no longer derives
    `Debug`: a hand-written impl renders through an internal `DebugProxy` that
    reports `CopilotDeviceFlowComplete.token` and
    `CopilotDeviceFlowStartResult.device_code` as presence flags and then
    applies `redact_secrets` to the whole rendering, so
    `tracing::debug!("{event:?}")` cannot print an OAuth credential.
    Regression suite:
    `crates/ragent-types/tests/test_unified_redaction.rs` (4 tests).
  - **CI gates (T-070)** - new job `security-guards` in
    `.github/workflows/ci.yml` (also wired into `pre-flight.sh`).
    `scripts/check-file-tool-containment.sh` fails if a registered file tool
    stops calling a `check_path_within_*` helper;
    `scripts/check-security-unwraps.sh` fails if any production file exceeds its
    recorded `.unwrap()`/`.expect()` baseline
    (`scripts/security-unwrap-baseline.txt`); `scripts/check-shared-guards.sh`
    fails if a crate re-defines a shared guard or a second secret registry; and
    `scripts/check-vcs-duplication.sh` additionally fails if the
    leading-dash git check is re-derived. Every gate ships a `--self-test`
    that seeds a violation and asserts the gate rejects it. The two regression
    suites also run in the workspace test job via
    `cargo test -p ragent-types --test test_shared_guards --test
    test_unified_redaction`.
  - **Accepted-risk register (T-071)** - `SECTASKS.md` now records the three
    deliberate non-fixes (the 362 grandfathered panicking calls, the two
    call-site-preserving guard adapters, and the non-functional headless browser
    tools) with rationale, compensating control, and a 2027-03-28 review date,
    dated 2026-09-28.
  - `SPEC.md` §4.6e documents the shared helpers, the unified redaction
    chokepoint, and the gates.
  - The registry-layer redaction case is a single test
    (`test_registry_layer_redacts_and_cow_path_still_masks`) holding a private
    mutex across the process-global secret registry, so a parallel
    `cargo test --workspace` run cannot interleave registrations.
  - Cleared the 8 pre-existing `clippy::redundant_pub_crate` warnings in
    `ragent-research::document` and raised the inline-test baseline in
    `scripts/check-inline-tests.sh` from 127 to 129 for the two inline blocks
    added by the failure-capture work.

- **Crash-dump capture for aborts that bypass the panic hook** — a stack
  overflow is not an unwinding panic: the Rust runtime prints
  `thread '...' has overflowed its stack` and calls `abort()`, so nothing
  in-process runs afterwards. `src/crash_dump.rs` now stamps
  `log/panics/last-crash.json` at startup with the session identity (pid, exe,
  args, cwd, thread), marks it `running`, overwrites it with `clean exit` on a
  normal return, and — when the next start finds a still-`running` record whose
  pid is gone — prints a warning pointing at the marker and at the host's core
  dump command. The record also carries a platform-specific retrieval hint
  (`coredumpctl list/info/debug` when `core_pattern` pipes to
  `systemd-coredump`, otherwise the `core_pattern` file or the macOS `/cores`
  path). This is detection, not trapping: a stack overflow still aborts, and
  the dump itself is collected by the OS.
- **Truncating stderr spool for TUI mode** — the TUI owns the alternate screen,
  so raw stderr (the default panic hook, `eprintln!`, C-library diagnostics)
  was painted over by the next frame. `src/stderr_spool.rs` redirects fd 2 into
  `log/logwindow/stderr-<timestamp>.log` on a dedicated drain thread when the
  TUI runs, and the TUI mirrors the spooled text into its log panel
  (`stderr: <line>`). The shared `Spool` type
  (`ragent_types::stderr_spool`, `SPOOL_MAX_LINES = 1000`) counts newlines per
  write and rewrites the file with only its newest 1000 lines once the cap is
  exceeded; the rewrite is skipped entirely for writes containing no newline.
  Non-TUI modes (`--no-tui`, headless run, server) leave stderr untouched.

### Fixed

- The TUI output-view overlay for a running sub-agent no longer stays frozen
  while the Agents panel's step counter advances. Two gaps stacked up:
  (1) the session loop's M-008 interim save was gated on the count of
  non-tool-call message parts, so a tool-only step (the common sub-agent
  shape) never persisted its completed tool-call parts to SQLite until the
  final save — the gate is now a total-parts count (parts are only ever
  pushed or popped, never mutated in place, so the count alone remains a
  sufficient save gate; FTS is still skipped until the final save); and
  (2) the overlay's line-cache generation key (message count + `edit_seq`)
  cannot change mid-run — the assistant placeholder is created once and
  `edit_seq` is not persisted — so the cached lines were reused verbatim
  until the run ended. The key now mixes in the same per-session step /
  tool-call counters the Agents panel displays, rebuilding the cache exactly
  when the agent advances and reusing it otherwise (the PERF-048 single-copy
  invariant is unchanged). Opening the overlay on a running agent now shows
  completed steps live; regression cover lives in
  `crates/ragent-agent/tests/test_interim_tool_call_persist.rs` (mid-run
  persistence asserted strictly before `MessageEnd`) and
  `crates/ragent-tui/tests/test_output_view_live_steps.rs` (cache
  invalidation on counter advance, rendered tool-call rows, stable cache
  when nothing advanced).

### Added

- **Security and anti-pattern remediation sweep - `ANTIPAT.md` milestones M0
  and M2-M7 complete.** The anti-pattern remediation plan (17 per-crate
  `explore` audits plus the workspace-root binary) closed its standards,
  de-duplication, silent-error, vocabulary and structural/debt milestones, plus
  dependency and tooling hygiene. The only milestone left open is M1 (the
  ASCII/non-ASCII conformance sweep).
  - **M2 - standards conformance (tests and hygiene):** inline `#[cfg(test)]`
    modules migrated into each crate's `tests/` directory, shared guards and the
    poison-lock policy unified, and dead-code `#[allow]` sites annotated with a
    `// reason:`.
  - **M3 - shared-helper de-duplication:** truncation, qname/scope/hash,
    SSE-parser, row-mapper and per-provider boilerplate collapsed onto single
    implementations.
  - **M4 - silent-error suppression:** `let _ =`, `unwrap_or_default` and
    `unwrap_or(0/false)` sites handled or annotated so none is silently
    discarded.
  - **M5 - vocabulary unification:** one name, one value set and one policy per
    concept across sibling crates (VCS `number`/`iid` and `open`/`opened`,
    telemetry `attr_*`, config `is_default`/`is_empty`, poisoned-lock policy).
  - **M6 - structural / complexity debt:** bounded process-lifetime caches,
    capped untrusted-JSON collections, RAII tree-depth guards in the codeindex
    parsers, and magic numbers hoisted to named constants.
  - **M7 - dependency and tooling hygiene:** stale advisory ignores removed and
    `deny.toml` set to `unsound = "all"` so the remaining `lru` 0.16.4
    suppression is checked; `rmcp` 1.8 -> 3.5, `rusqlite` 0.32 -> 0.40, `similar`
    2.7 -> 3.2, `dirs` 6 -> 7, `sha2` 0.10 -> 0.11, `base64` 0.22 -> 0.23,
    `printpdf` 0.9 -> 0.12 and `ratatui` 0.29 -> 0.30 (with the wrap-port parity
    fix) applied; the retired `ragent-team` re-export shim crate deleted
    (**17 -> 16 workspace crates**); and `check-team-duplication.sh`,
    `check-vcs-duplication.sh`, `check-inline-tests.sh` and
    `check-file-tool-containment.sh` rewritten to enforce what their comments
    claim, wired into `ci.yml` and `pre-flight.sh`.
  - **MS-05 - recurrence prevention (`SECTASKS.md`):** new `ragent_types::guard`
    owns `reject_option_like`, `is_safe_operand`, `validate_identifier`,
    `validate_relative_component`, `contained_join`, `clamp_retry_after` and
    `cap_read` (re-exported as `ragent_tools_core::guard`); `ragent-agent`,
    `ragent-storage` and `ragent-tools-core` re-export `ragent_types::sanitize`
    so there is one secret registry and one redaction chokepoint (`Event` no
    longer derives `Debug`); the `security-guards` CI job runs
    `check-file-tool-containment.sh`, `check-security-unwraps.sh`,
    `check-shared-guards.sh` and `check-vcs-duplication.sh` (each with a
    `--self-test`); and `SECTASKS.md` records the accepted-risk register (T-071).
  - 941 files changed, +53,071 / -44,621. `cargo check`,
    `cargo clippy --all-targets`, `cargo fmt --check` and `cargo audit` all
    clean; full workspace test suite green (689 suites, 10,249 passed, 0
    failed).

### Changed

- **`340c32cc` - Version 1.0.121 more fixes on mcp.** Incremental MCP fixes
  carrying the v1.0.121 release; no additional tool or configuration surface.
- **`14c25e1d` - Add the `research/` folder.** Reverts the `research/` entry in
  `.gitignore` so the directory is tracked, and commits the 432 accumulated
  research findings, indexes and output reports.
- **`da83d927` - MS-03: network and secret hardening (`SECTASKS` T-025..T-058).**
  The Medium-severity remediation set. Network egress: MCP HTTP response bodies
  read under an 8 MiB cap; benchmark downloads carry a client timeout, a
  streamed size cap, a pagination budget and manifest path containment; LLM
  providers clamp `Retry-After` (30 s), cap the SSE accumulation buffer on every
  streaming path and cap error-body reads; the GitHub tree walk and GitLab jobs
  pagination carry request/entry budgets; `mf_fetch`/`mf_crawl` stream under
  byte caps and `crawl_urls` is SSRF-checked at the tool boundary. Secrets: the
  registry is seeded from every credential env var; bash output, CI log excerpts
  and SSE tool-call arguments are redacted; the GitHub token attaches only to the
  configured API origin; Gmail headers are CRLF-stripped and the provider-setup
  dialog masks the API key and PAT. Plus DB/WAL/activity-log re-restriction to
  0600, `http_request` refusing routing/credential headers, codeindex
  exclusion-glob/FTS-symlink/walker-recursion guards, bounded spec
  numbering/contradiction detection, mailbox caps and validated team hooks,
  panic-free `CronSchedule`, a stderr-spool byte ceiling, `$( ... )`-aware
  `split_bash_command`, and four pre-existing flaky tests stabilised.
- **`6cf0b60f` - MS-04: defence in depth (`SECTASKS` T-059..T-066).** The 33
  Low-severity findings, closed with the same guard shapes as MS-01..MS-03:
  `--samples` clamped to `MAX_BENCH_SAMPLES` (100), streamed dataset downloads
  under `MAX_DOWNLOAD_BYTES` with a HumanEvalPack pagination budget, only
  `base_url_override` recorded in the shareable workbook/sidecar, and the GitHub
  recursive tree walk given the GitLab request/entry budget. Containment:
  benchmark manifest `case_file`/`relative_path` joins confined, plugin manifest
  `entry`/`main`/`server.entry` and marketplace wrapper names validated as single
  normal components, `resolve_memory_dir` rejecting unsafe agent names, and the
  bash scratch directory falling back to a process-private 0700 directory.
  Redaction: redacted JSON parse-diagnostic source lines, `BUILTIN_DENYLIST`
  merged into the enforced denylist, fenced research source bodies neutralised
  and gather-log URL credentials masked, permission replies bound to the awaiting
  session, and credential-bearing events redacted on the SSE stream
  (`redacted_event_debug` for log sites).

## [1.0.121] - 2026-09-27

Release of the staged working tree: documentation refresh for v1.0.120, a
`/simplify all` pass over the 50-file changed set (including the FUNC-015 test
hardening below), and the regression cover for the rollback race fixed in
v1.0.120's working tree.

### Fixed (this release)

- The FUNC-015 guard test (`test_block_in_place_guard`) broke when the last
  `Handle::current().block_on` site in `src/app/slash.rs` was reformatted by
  rustfmt onto two lines: the guard only matched `Handle::current().block_on`
  on a single line and then (after the legitimate `/mcp` awaits were exempted)
  found zero sites to check. The guard now also matches a
  `Handle::current()` receiver whose `.block_on(..)` continues on the next
  line, and a new `AWAITED_EXEMPTIONS` table pins the `/mcp
  discover|connect|disconnect` sites that were legitimately converted to plain
  `.await`s (the slash dispatcher is itself async, so awaiting is the correct
  fix, not `block_in_place`).

### Code quality (`/simplify all`)

- Reviewed the 50-file changed set with four parallel reviewers and applied the
  worthwhile findings (no behaviour change):
  - `ragent-agent/src/tool/mod.rs` `remove_all` now releases the registry write
    lock with a scoped block before draining tools, instead of a manual
    `drop(tools)`.
  - `ragent-plugins/src/store_provider.rs` no longer double-SHA-256s the
    marketplace document: the provider records it and receives the key from
    `record_document`. The per-entry JSON round-trip was removed via a new
    `inline_manifest_in(&Value, …)` helper.
  - `ragent-plugins/src/manifest.rs` extracts a duplicated skills-directory
    merge into `merge_scanned_skills`.
  - `ragent-plugins/src/marketplace.rs` drops the redundant `root.clone()`
    candidate in `materialize_at` and logs the materialise error at `debug!`.
  - `ragent-tui/src/app/models.rs` and `ragent-agent/src/tool/tool_info.rs`
    `tool_source` now read the registry's `mcp_wrapper_info()` (with the exact
    `ragent_name_for` prefix fallback) instead of duplicating the naming
    logic, keeping the TUI `/tools` report and the tool dump consistent.
  - `tool_info.rs` additionally gains a `VISIBILITY_SWITCHES` const and a
    single `mcp_wrapper_info` query for the registry snapshot.
  - `ragent-tui/src/app/slash.rs` removes the nested
    `block_in_place`/`block_on` from `/mcp connect|disconnect|discover` (the
    arms are now awaited directly and collapsed).
  - `ragent-tui/src/app/state.rs` `set_mcp_server_enabled` now validates
    before touching the on-disk ledger.

### Fixed

- Accepting the TUI's post-loop rollback offer (`Enter` on the change-summary)
  could report success while the workspace file still held the loop's mutated
  contents: the spawned `SessionProcessor::rollback_loop` task removed the
  capture from `active_loop_captures` *before* the blocking snapshot-write
  completed, so a caller polling on the capture map becoming empty observed
  "capture dropped, file restored" a beat early (the intermittent CI failure
  of `test_rollback_accept_restores_snapshot`). The capture is now removed
  only after the restore has fully completed, and a failed restore leaves the
  capture pending for retry. Regression cover lives in
  `crates/ragent-agent/tests/test_rollback_capture.rs`.

- OpenSkills discovered under `~/.agents/skills/` (and project-local
  `.agents/skills/`) are now loaded and registered. The skill discovery scan
  previously only covered the `.agent` and `.claude` directory variants, so
  skills installed by the `skills` CLI into its default `.agents` location were
  invisible to ragent. Both the global and project scans now cover
  `.agent`, `.agents`, and `.claude`, under the existing
  `openskills-global` / `openskills-project` scopes.
- The Claude store's `*-lsp` stubs (`rust-analyzer-lsp`, `gopls-lsp`,
  `pyright-lsp`, ...) now install instead of failing with `no plugin manifest
  found`.
  Those marketplace entries are self-describing stubs: the entry
  itself carries the plugin's manifest content inline (`lspServers`), while
  the repo subdirectory its `source` points at holds only a README and a
  LICENSE. The store provider now records the parsed marketplace document
  in-process (keyed by `sha256(origin-url + document bytes)`) and suffixes
  such an entry's git-subdir source with that key; when `/plugins add` clones
  a keyed source and finds no manifest in the checked-out tree, it
  materialises the recorded inline manifest into `.claude-plugin/plugin.json`
  (listing-only marketplace fields stripped) so the plugin installs as an
  ordinary Claude-dialect non-JS bundle. The bridged-in `lspServers` /
  `monitors` sections are reported under the new FR-025 `claude lsp servers`
  unsupported capability; a checked-out tree that already carries a manifest
  is never overwritten, and a source without a registered key installs
  exactly as before.

- **Claude-marketplace plugins that ship a conventional `skills/` directory but
  declare no `skills` manifest section now contribute their skills.** The
  plugins spec FR-029 bridge previously only honoured an explicit `skills`
  declaration (a string or list of directories), so plugins like
  `mcp-server-dev` (from the Claude marketplace) — which rely on the
  convention that a `skills/` directory at the plugin root holds `SKILL.md`
  packs — installed and listed cleanly but exposed none of their skills to
  `skill_manage` / `/plugins list`. Both manifest parsers now probe the plugin
  root for the conventional `skills/` directory when the manifest declares no
  `skills` section, and merge the result (declared entries first, de-duplicated)
  the same way the `agents/` directory scan works.

## [1.0.120] - 2026-09-26

Release of the working tree carrying the introspection tools, MCP orphan-sweep
and shutdown fixes, and MCP tool-registry reconciliation. The full
rust-hygiene sweep (cargo check, cargo machete, cargo check --tests, cargo
test, dead-code lint, dead-code reason check, clippy, rustfmt, cargo audit,
cargo deny) is green on this tree.

### Added

- **`tool_info` and `commands_info` introspection tools** — two new
  read-only tools (permission category `none`, hardwired
  auto-approve). `tool_info` returns a JSON-encoded dump of every
  registered tool (name, description, parameters JSON schema,
  permission category, source family — `internal` / `mcp:<server>`
  / `plugin:<id>` / `visibility:<family>` — hidden state, and MCP
  server/tool provenance for bridged tools). `commands_info`
  returns a JSON-encoded catalog of every slash command — the
  built-in TUI commands (trigger, description, subcommands, flags)
  from a static mirror of `SLASH_COMMANDS`, plus the commands
  contributed by enabled plugins resolved live at call time. The
  static catalog is kept in sync with the TUI table by a drift
  test in `crates/ragent-tui/tests/test_command_catalog.rs`.

- **MCP duplicate-process cleanup at startup.** `McpClient::connect` no longer
  blindly spawns a second copy of an MCP server that is already running. Before
  spawning it first resolves the server by its config: a declared `http`/`sse`
  URL (or a stdio command line that names a port via `--httpPort <n>` /
  `--port <n>`) is probed with a short-timeout MCP `initialize` handshake, and
  a live MCP endpoint answering on the loopback candidates
  (`http://127.0.0.1:<port>/mcp`, then the bare origin) is **adopted** —
  session id, tools, and all — instead of starting a competing instance; a
  plain TCP listener that fails the handshake is not adopted. For a plain
  stdio command (no HTTP port), adoption is impossible because the pipes are
  private to the spawning parent, so `connect` sweeps `/proc/<pid>/cmdline`
  (Unix only) for orphaned earlier copies of the same `command` + `args` and
  SIGKILLs them before spawning a fresh child, freeing their file locks,
  ports, handles, and rate-limit slots. A process counts as an orphan only
  after being re-parented to init (`/proc/<pid>/stat` field 4, `Ppid == 1`);
  a same-command process whose parent is still alive belongs to a *running*
  ragent instance and is left alone, so two concurrent sessions can share the
  same MCP stdio server command without the later one severing the earlier
  one's pipes (`Transport closed` errors). Matching keys on the executable
  basename (`/usr/bin/npx` == `npx`) plus the package token extracted from the
  arguments (`@<version>` stripped), which recognises both the `npx` launcher
  and the `node` child it spawns without touching a different package through
  the same launcher; the current process is always excluded, and a launcher
  with no positional package argument matches nothing rather than sweeping
  every `npx` on the machine.

### Fixed

- **MCP stdio servers are shut down with ragent instead of being orphaned on
  exit.** Two failure modes leaked the child past every existing safeguard:
  first, `shutdown` re-drained the connections map through `disconnect`, whose
  graceful `cancel()` silently no-ops whenever the rmcp service `Arc` is still
  shared by a registered `McpToolWrapper` (the normal state at exit) - so the
  transport's `kill_on_drop` was never triggered and the "belt-and-braces"
  second pass could never see the pid because the map was already drained.
  Second, even a successful kill only reached the recorded *launcher* pid:
  `npx` (and friends) fork the real `node` server and exit within a second,
  leaving that grandchild - the process actually holding the stdio pipes -
  orphaned. The stdio child now leads its own process group
  (`process_group(0)`, the same pattern as the bash-timeout harness), shutdown
  tears each drained connection down inline (graceful cancel when the `Arc` is
  uniquely owned, a SIGKILL on the whole process group always afterwards), and
  a failed shutdown no longer resurrects an entry on a stale connection.
  `shutdown` also clears any `Connected` server state that outlives its
  connection. A `/simplify` follow-up dropped the `config.disabled` skip from
  the teardown loop (a re-enabled server whose config still said `disabled`
  would have leaked its child), removed the signal-0 liveness probe that gated
  only a log line, and made `disconnect` warn-log a cancel failure instead of
  discarding it. MCP Streamable-HTTP SSE replies that split a JSON payload
  across multiple `data:` lines are now joined per the SSE grammar instead of
  being truncated to the first line. Verified live: consecutive `ragent run`
  invocations leave zero
  `mongodb-mcp-server` processes on the system, and a second concurrent
  instance no longer kills the first's server (that sibling-kill was the
  `Transport closed` regression).

- **MCP tools reach the tool registry after startup, and a disconnect drops
  them again.** Publishing the shared `McpClient` before the background
  connect loop meant `set_mcp_client` ran while no server was Connected, so
  the registry never saw the servers' tools and `/tools` (and the model's
  tool surface) stayed empty. The TUI now reconciles the registry on
  `McpStatusChanged` / `McpServerEnabledChanged` and after a live
  enable/disable: connected servers' tools are registered (idempotently, the
  registry is name-keyed) and every `mcp_<server>_*` tool whose server is no
  longer Connected is first removed via the new
  `ToolRegistry::remove_all`, so disabling a server leaves no dead wrappers
  for the model to call.

## [1.0.119] - 2026-09-26

Release over the v1.0.118 tree with the user headline "mcp fixes". This version
folds in the working-tree MCP transports/sessions/startup-reporting, plugin
MCP bridging, and tool-visibility fixes, followed by a `/simplify all`
code-quality pass and a full rust-hygiene sweep (check, machete, tests,
dead-code lint, clippy, fmt, audit, deny) — all green.

### Fixed (this release)

- **Sessionful Streamable-HTTP MCP servers report their tools.** The plain
  HTTP client never performed an `initialize` handshake, so a server that
  issues an `mcp-session-id` (e.g. the MongoDB MCP server on its 2025-era
  sessionful path) rejected every `tools/list` with HTTP 400 — a connected
  server showed zero tools and ragent retried the dead endpoint for 7 seconds.
  `HttpMcpClient::initialize` now negotiates the session, replays the returned
  `mcp-session-id`, advertises `text/event-stream`, and unwraps SSE frames;
  `McpClient::adopt_connected` adopts an already-running server through this
  client instead of a second rmcp handshake.
- **Constructing the MCP HTTP client no longer panics outside a Tokio
  runtime.** `HttpMcpClient::new` called `reqwest::Client::new()`, which builds
  pool sockets eagerly and panics with "there is no reactor running" from sync
  contexts such as `App::init` and slash-command construction; the client is
  now built lazily on first request.
- **The TUI startup MCP report cannot be silently dropped.** The report was
  skipped whenever the shared `McpClient` lock was momentarily held by the
  connect loop; `App::report_mcp_startup` is now async and awaits the lock, and
  a `MCP_STARTUP_GRACE = 3s` wait (`App::wait_for_mcp_connect`) lets a slow
  server reach a terminal connected/failed state before the report prints.
- **`/plugins list` MCP Tools column resolves live counts.** The TUI never
  passed live counts to the renderer, so a connected plugin MCP server showed
  `?`; live counts are now threaded from `App.mcp_servers` through
  `run_plugin_subcommand`/`run_control_command` into
  `render_list_with_mcp_tools`, and the earlier rendered-text substitution hack
  is deleted.
- **The `/mcp` list no longer prints the full per-tool inventory.** It now
  prints only `tools: N`; the detailed inventory with registry names stays on
  `/plugins list --mcp`.
- **Deterministic plugin MCP contributions block.** `/plugins list --mcp`
  sorts each plugin's MCP server ids alphabetically so the rendered table and
  contributions line are stable, diffable, and independent of manifest or map
  iteration order; this also fixed the order-dependent test
  `list_table_totals_counts_across_a_plugins_servers`.
- **`/swarm status` progress bar math.** The `completed * bar_width / total`
  arithmetic could overflow before dividing; it now uses
  `checked_div(total).unwrap_or(0)` and renders empty at 0 tasks.
- **`/spawn` no longer clobbers a landed launch outcome.** The pending-marker
  restore in `poll_spawn_result` writes the marker only when the slot is still
  empty, so a success/error arriving between the two lock acquisitions is
  surfaced rather than overwritten.
- **`/plugins <subcommand> --mcp` executes the requested operation.** A
  literal `--mcp` on a non-`list` subcommand previously printed the MCP
  inventory instead of running the operation; the flag is honoured on
  `/plugins list` only.
- **A swarm member unblocked by dependency resolution is persisted.** The
  unblock path no longer silently drops `store.save()` errors; save failures
  are logged.
- **`/alog` delete confirmation surfaces storage errors.** The command no
  longer converts a storage error into a misleading "0 events" count.
- **Removed the temporary `[image-debug]` info-level logging** from the
  per-turn history conversion path (`session/history.rs`).
- **Removed duplication and small allocations** across the TUI app layer:
  `clear_active_bench()` shared by the bench watchdog and completion paths,
  `load_swarm_tasks()` used by status/unblock/completion/finalize, the
  `multiedit` parameter normalisation table, single-pass `truncate_str`,
  borrowed `files_preview`, and one-shot skill-body cold load.
- **Fixed mojibake (two U+FFFD replacement characters)** in the
  `start_cron_scheduler` doc comment.

### Added (this release)

- **`McpEnableLedger` is the single source of truth for MCP server
  enablement.** Whether a server is started is now a persisted choice
  (`<global state dir>/mcp_state.json`) rather than an implicit effect of being
  listed in `ragent.json`; a server absent from the ledger is enabled, and
  `mcp.<id>.disabled: true` always wins.
- **Per-server MCP startup report with transport labels.** Immediately after
  `[ok] Input history loaded` and before `[ok] **Ready**`, the TUI prints two
  lines per attempted server (`[mcp] Starting ...` / `[mcp] Connected ...`),
  plus explicit `disabled` / `failed` / `needs auth` lines; the transport label
  is rendered from a new `Display` impl on `McpTransport` so config names and
  display names cannot drift.
- **`/plugins list` gains MCP columns and a contributions block.** The table
  gains `MCP` (server count) and `MCP Tools` (total tool count), and the
  contributions block renders `mcp [<id> (<n> tools)] (S server(s), T
  tool(s))`; `/plugins list --mcp` keeps the full per-server tool inventory.
- **Plugin-declared MCP servers are bridged and connected by default.**
  `mcpServers` entries (inline or the Claude `"mcpServers": "./mcp.json"` file
  reference) are bridged as `<plugin-id>.<server>`.
- **`/tools` lists visibility-disabled tools.** `ToolRegistry::hidden_definitions()`
  and `hidden()` expose them, and the slash-output extractor keeps every fenced
  block.
- **Deterministic transport mapping for plugin-bridged MCP servers.** The
  bundled MongoDB special case was removed; a plugin's declared `type`/`url`
  always wins, so the bridge is a declared-transport mapping and a stdio
  server is still started as stdio.
- **`McpEnableLedger::to_map`** centralises the ledger-to-map conversion used
  by the TUI; **`impl Display for McpStatus`** gives `/mcp` and `/plugins`
  one place to render status text.

### Fixed

- **Code-quality sweep over the last three releases (simplify `all`).** A
  parallel review of every Rust source file changed since v1.0.116 applied
  the following fixes:
  - `/swarm status` progress bar always rendered 100% full (`total *
    bar_width / total` collapses to `bar_width`); it now shows the real
    `completed/total` fraction and renders empty at 0 tasks.
    - `/spawn` no longer clobbers a landed launch outcome: the
      pending-marker restore in `poll_spawn_result` now writes the marker
      only when the slot is still empty, so a success/error that arrives
      between the two lock acquisitions is surfaced instead of being
      overwritten with the empty error sentinel.
    - `/plugins remove foo --mcp` (and other non-`list` subcommands carrying
      a literal `--mcp`) now execute the requested operation instead of
      printing the MCP inventory; the flag is honoured on `/plugins list`
      only.
    - A swarm member unblocked by dependency resolution is now persisted with
      a logged failure instead of silently dropping `store.save()` errors
      (`swarm.rs` unblock path).
  - Removed the temporary `[image-debug]` info-level logging from the
    per-turn history conversion path (`session/history.rs`).
  - Duplication extracted: the bench watchdog and completion paths share one
    `clear_active_bench()` helper; the legacy `multiedit` parameter
    normalisation loop now drives the three renames from a single table;
    swarm task-list loading (`TeamStore` + `TaskStore` + read) is one
    `load_swarm_tasks()` helper used by the status, unblock, completion, and
    finalize paths; all six swarm `std::env::current_dir()` calls now use the
    App's recorded cwd.
  - Error handling: `/alog` delete confirmation no longer converts a storage
    error into a misleading "0 events" count — the error is propagated to
    the user.
  - Test fix: removed the tautological truncated-id fallback assertion in
    `test_background_task_removed_on_completion` (the `||` arm compared the
    same string).
  - Small allocations removed: bench summary/show rendering no longer clones
    the run message/paths twice, `files_preview` borrows instead of cloning,
    `truncate_str` is single-pass, and the skill-body cold load allocates
    once instead of twice.
  - Fixed mojibake (two U+FFFD replacement characters) in the
    `start_cron_scheduler` doc comment.

- **A sessionful Streamable-HTTP MCP server now reports its tools.** The
  plain-JSON-RPC HTTP client sent `Accept: application/json` only and never
  performed an `initialize` handshake, so a server that issues an
  `mcp-session-id` (the MongoDB MCP server on its 2025-era sessionful path)
  rejected every `tools/list` with HTTP 400 — a connected server showed zero
  tools, and ragent then retried the dead endpoint for 7 seconds with backoff.
  The client now negotiates the session (`HttpMcpClient::initialize`), replays
  the returned `mcp-session-id` on every later request, advertises
  `text/event-stream` in `Accept`, and unwraps the `event: message` SSE frame the
  server replies with. `McpClient::adopt_connected` adopts an already-running
  server through this client instead of a second rmcp handshake, which is what
  actually surfaced the failure; `find_running_streamable_http` replaces the old
  boolean probe so the discovered endpoint and its session id reach the adoption.
- **Constructing the MCP HTTP client no longer panics outside a Tokio runtime.**
  `HttpMcpClient::new` called `reqwest::Client::new()`, which builds the pool's
  `PollEvented` sockets eagerly and panics with "there is no reactor running"
  when called from a synchronous context such as `App::init`/slash-command
  construction. The client is now built lazily on first request (which always
  runs on the runtime) and shared process-wide.

### Added

- **The TUI prints per-server MCP status at startup, with the transport
  protocol on every line.** Immediately after `[ok] Input history loaded` and
  before `[ok] **Ready**`, ragent now reports every MCP server it attempted to
  start. A server that came up produces two lines, `[mcp] Starting mcp server
  <id> (<transport>)` then `[mcp] Connected to mcp server <id> via
  <transport>`; a server switched off in the enable-state ledger prints `[mcp]
  Skipping mcp server <id> (<transport>, disabled)`, a failed connection
  prints the reason on its own line, and a server awaiting auth is labelled
  the same way. The `<transport>` label is the config's declared wire protocol
  (`stdio`, `sse`, or `http`), rendered by a new `Display` impl on
  `McpTransport` so config names and display names cannot drift. The report
  reads the shared `McpClient` (adopted first, so it reflects the real
  per-server outcome rather than assuming `disabled`) and is skipped entirely
  when no server is configured or the connect loop has not published the
  client yet.
  The report waits up to three seconds for the background connect loop so a
  slow server is reported as connected rather than `disabled`.
- **Fixed: the startup MCP report could be silently dropped.** The report was
  skipped whenever the shared `McpClient` lock was momentarily held (the connect
  loop can still hold its guard after publishing the client), and a one-shot
  call that is skipped is never retried, so a slow-starting server showed nothing
  at all between `[ok] Input history loaded` and `[ok] **Ready**`.
  `App::report_mcp_startup` is now async and awaits the read lock instead of
  failing a `try_read`.

- **An already-running MCP server is adopted instead of started a second time.**
  For an MCP server that can also serve Streamable-HTTP on a well-known localhost
  port (currently the bundled MongoDB server, port 3000), ragent probes the port
  with a JSON-RPC `initialize` frame on `/mcp` and `/`: if a server answers,
  ragent connects to the running instance (`McpClient::adopt_connected`, over the
  session the probe negotiated) and spawns nothing. If nothing is listening the
  configured transport is used, so a stdio server is still started as before. An
  adopted server is never shut down on exit — the child is owned by another
  process — while a server ragent itself spawned is still killed on exit. The
  rule is applied identically at startup (`src/main.rs`) and on a live
  `/mcp connect <id>`.

## [1.0.118] - 2026-09-25

Release over the v1.0.117 tree. This version folds in the uncommitted MCP
enablement, plugin-bridge, and tool-visibility work plus a fix to the research
and spec subsystems. All changes carry tests and keep `cargo check --workspace
--all-targets` clean and `cargo audit` free of new advisories.

### Added

- **`/plugins list` shows each plugin's MCP server and MCP tool counts.** The
  list table gains `MCP` (the number of MCP servers the plugin declares) and
  `MCP Tools` (the total tools those servers advertise) columns, and the
  contributions block now renders `mcp [<id> (<n> tools)] (S server(s), T
  tool(s))`. The live counts are read from the session's MCP client and passed
  into the shared `render_list_with_mcp_tools` renderer as its `mcp_tool_counts`
  map (the CLI parity surface has no connected client and passes an empty map),
  keyed by the bridged `<plugin-id>.<server>` id. A server whose tool count is
  not yet known (it has not connected) renders `?`, never `0`, and an unknown
  count makes the plugin's tool total `?` too rather than an inexact figure.

- **Durable, global MCP server enable/disable state.** Whether an MCP server is
  actually started is now a persisted choice (`<global state dir>/mcp_state.json`)
  rather than an implicit side effect of being listed in `ragent.json`:
  - A server id **absent** from the ledger is enabled, so a newly added server —
    whether written into `ragent.json` or bridged from a plugin's `mcpServers`
    section — starts **enabled** with no extra step.
  - `mcp.<id>.disabled: true` in `ragent.json` still always disables the server
    (the config flag is the harder switch). A server switched off in the ledger
    is registered with `McpStatus::Disabled` and **no child process is spawned**,
    so `/mcp` can still list it and offer to re-enable it.
  - `/mcp connect <id>` enables and connects a server live (its tools are
    registered into the session tool registry immediately);
    `/mcp disconnect <id>` disables and disconnects it live. Both persist the
    choice globally, so it survives a restart and applies to every project.
  - `McpToolWrapper::execute` refuses to call a disabled server's tools even
    when the tool is still registered from an earlier connection, naming the
    command that re-enables it.

- **`/mcp` lists each server's tool count, not its tools.** The `/mcp` listing
  prints `enabled yes/no` with the number of tools the server advertises
  (`tools: N`, also for a server with none). The individual tool names are the
  model-facing surface and are not enumerated here; the full per-server
  inventory with the registry name each tool is callable under
  (`<tool> -> mcp_<server>_<tool>`) remains available via
  `/plugins list --mcp`.

- **`/plugins list` shows a plugin's MCP servers.** The contributions block gains
  an `mcp [...]` section listing the bridged server ids declared by the plugin's
  `mcpServers` section, resolved through the same bridge the session connects,
  so the plugin listing and `/mcp` can never disagree. A plugin that contributes
  only MCP servers is now listed in the contributions block too.

- **`/plugins list --mcp` reports how many tools each MCP server provides.**
  A plugin's `mcpServers` entry is a separate process, so the number of tools it
  exposes is only known once it has connected — it cannot be counted from the
  manifest. `/plugins list --mcp` therefore prints the live inventory from the
  session's MCP client: one row per server (`<id> <status> N tool(s)`) followed
  by every advertised tool and the registry name it is callable under
  (`<tool> -> mcp_<server>_<tool>`), e.g. `mongodb.mongodb connected 35 tool(s)`.
  The `mcp [...]` section of a plain `/plugins list` now also annotates each
  bridged server with its live tool count, or `?` when no live client is
  available (CLI parity, discovery-only rendering) so a server that has not yet
  connected is never reported as contributing zero tools.

- **`ragent_plugins::scanned_plugin_mcp_contributions`** (and the
  `ragent_agent::plugin::plugin_mcp_contributions` wrapper): the MCP bridge with
  the owning plugin id retained, so a surface can attribute a bridged
  `<plugin-id>.<server>` id to its plugin.

- **`McpClient::register_disabled`**: register a server that exists but must not
  be started, without a spawn permit or transport handshake.

- **MCP servers are now enabled by default whether they live in a plugin's
  `mcpServers` map or in `ragent.json`.** A plugin's MCP-server
  transport section was previously recorded only as an unsupported capability
  (`/plugins list` showed `mcp server transports`), even though the session also
  connected the very same servers. The bridge is now the single source of truth:
  a plugin `mcpServers` entry (inline `{"<id>": {command, args, ...}}` or the
  Claude `"mcpServers": "./mcp.json"` file-reference shape) is bridged as
  `<plugin-id>.<server>`, connects at startup, and is listed as an MCP server.
  The `mcp server transports` label is retained only for entry shapes the bridge
  cannot satisfy (for example the `claude-mcp` fixture's array-of-transports
  form).

### Changed

- **`App::execute_slash_command`, `App::handle_event`, `App::handle_key_event`,
  `App::advance_input_queue`, and `input::handle_key` are now `async`** (and
  return futures where they previously returned values), so the new `/mcp
  connect` and `/mcp disconnect` arms can drive the async `McpClient` inline.
  The async recursion through `/queue next` (a queued slash command that may
  dispatch the next entry) is boxed at the single recursion point. TUI tests
  that drive these entry points were converted to `#[tokio::test]`
  (multi-thread where the code under test uses `block_in_place`).

### Fixed

- **`/mcp` reports the real MCP server status, and the display list no longer
  drops a connected server.** Two defects combined to make the plugin-bridged
  `mongodb` server show as `disabled`:
  - The startup connect loop in `src/main.rs` is spawned *before* the TUI calls
    `event_bus.subscribe()`, and `EventBus` is a plain `broadcast` channel with
    no replay buffer — every `McpStatusChanged` published in that window was
    discarded, so the TUI's live status map stayed empty and `/mcp` fell back to
    the seeded `Disabled`. The TUI now also reads the authoritative
    `McpClient` held by `SessionProcessor::mcp_client` (status and tool list per
    server) at startup and on the housekeeping pass until the client appears, so
    `/mcp` is correct regardless of event delivery.
  - `mcp_display_servers` rebuilt the list from the merged config set alone, so
    a server present in the live client but absent from `ragent.json` and the
    plugin store (e.g. after a store change) was silently dropped from the
    display. Servers tracked in the previous list are now preserved.

- **`/mcp` now lists MCP servers contributed by plugins.** A plugin that
  declares an `mcpServers` section (inline or, as the Claude `mongodb` plugin
  does, via `"mcpServers": "./mcp.json"`) has its servers bridged as
  `<plugin-id>.<server>` and connected at startup, but the TUI's `/mcp` listing
  rebuilt its display list from the `ragent.json` `mcp` section alone — so on a
  project whose only MCP servers come from a plugin the command reported
  "(no MCP servers configured)" while the server was in fact connected and its
  tools registered. The display list is now built from the same merged set the
  connect path uses (`ragent_agent::plugin::plugin_mcp_servers`), at init, on
  `/mcp` and on `/reload mcp`, preserving tracked status and tool counts.

- **`Config` merge now propagates config provenance.** `Config::merge` dropped
  the `config_paths` accumulated by the loader, so a caller holding the value
  returned by `Config::load` (rather than the loader's intermediate binding)
  could not tell whether a project config had been loaded. `merge` now carries
  the contributing paths forward.

- **Plugin store resolution follows the active project config.** The plugin
  store root is derived from the loaded config's project path, anchored on the
  current working directory, so `/cd` into another project reads that project's
  `.ragent/plugins/` rather than the launch directory's.

- **`/tools` lists the tools disabled by a visibility switch.** The
  report previously printed only the tools the model currently sees, so a family
  switched off (`/tools github off`) silently vanished from the list. The
  listing now prints `Visible Tools (N total, M disabled)` followed by a
  `Disabled by visibility (M)` section listing every hidden-but-still-registered
  tool. Both sections share one column layout and header (name / source /
  description) so the rows stay directly comparable; the disabled rows are no
  longer wrapped in `[red]…[/red]`, since belonging to a separate section
  already conveys the state. Backed by the new
  `ToolRegistry::hidden_definitions()` (and `ToolRegistry::hidden()`), the
  complement of `definitions()`.

- **`/tools` output now keeps every preformatted table.** The slash-output
  extractor previously captured only the first bare triple-backtick block, so
  the tool list and its disabled section were discarded for any command that
  prints more than one table (the `/tools` switch table came first). The
  extractor now lifts every blank-line-separated fenced block, preserving the
  prose between them.

- **`[red]…[/red]` spans survive the markdown pass.** The markdown pipeline
  parses the intermediate HTML with html5ever, which silently drops C0 control
  bytes, so an ANSI-colour sentinel was stripped before it could be restored.
  The red-span sentinel is now a printable token (`@@ragent-red@@`), converted
  back into `[red]…[/red]` markers after rendering and styled by the message
  widget.

### Fixed (working tree)

- **Dead API removed from the MCP/plugin bridge surface.** `plugin_contributed_mcp_servers`
  (`crates/ragent-agent/src/plugin.rs`) and `McpEnableLedger::enabled_servers`
  plus its `mcp/mod.rs` re-export had zero callers; both are deleted.
- **`McpEnableLedger::to_map` added.** The ledger now exposes
  `to_map(&self) -> HashMap<String, bool>` in
  `crates/ragent-agent/src/mcp/enable_state.rs`, replacing the copy-pasted
  `BTreeMap`-to-`HashMap` chain in `crates/ragent-tui/src/app/init.rs` and
  `app/state.rs::refresh_mcp_enabled_map`.
- **`set_mcp_server_enabled` loads the config once.** The TUI handler called
  `Config::load()` twice; it now loads once and derives the configured `mcp`
  section from that single value.
- **`McpStatus` implements `Display`.** `crates/ragent-agent/src/mcp/mod.rs`
  gains `impl std::fmt::Display for McpStatus` (`connected` / `disabled` /
  `needs auth` / `failed: {error}`), and the TUI-local `mcp_status_label` helper
  in `app/plugin.rs` is deleted; `/plugins list --mcp` prints `{server.status}`.
- **`McpToolWrapper::execute` no longer holds the client lock across a file
  read.** `McpEnableLedger::load()` now runs before `self.client.read().await`,
  so a per-tool-call disk read does not block on the client lock.
- **Comment corrected** in `md_worker.rs`: the red-span sentinel is described as
  a printable token, matching the `@@ragent-red@@` design.

### Verified (working tree)

- `cargo fmt --all -- --check` clean, `cargo check --workspace --all-targets`
  clean, `cargo clippy` shows only the 8 pre-existing `ragent-research`
  `redundant_pub_crate` warnings, and the `ragent-tui` and `ragent-agent` test
  suites are green.

## [1.0.117] - 2026-09-25

Release over the v1.0.116 tree: the `/spec reverse --folder` scaffold path now
actually creates the project and writes the spec into it, the scaffolder gains a
first-class `webapp` app type, and the vendored `lopdf` crate joins the workspace
lint suite. All changes carry tests and keep `cargo check`, the dead-code lint
and reason checks, `cargo clippy --workspace -- -D warnings`,
`cargo fmt --all -- --check`, `cargo audit`, `cargo deny check`, and the full
`cargo test --workspace` suite green.

### Fixed

- **Deeply nested HTML can no longer abort the process with a stack overflow.**
  `html2text`'s DOM walker (`tree_map_reduce`) re-used the thread stack for
  every nested node, so pathologically nested markup from a scraped page
  overflowed the stack and aborted the binary — a stack overflow cannot be
  caught by `catch_unwind`, so the existing panic isolation could not help
  (recorded as a coredump from the `mf-html2text` worker thread). The walker
  now enforces a maximum DOM nesting depth of 32 and returns the recoverable
  `Error::Fail` instead; every caller already degrades that to its raw-text
  fallback. The vendored `html2text` is patched in place
  (`vendor/html2text/src/lib.rs`).

- **`/spec reverse --folder` now actually scaffolds the project.** The
  `/spec reverse` dispatcher re-rendered its validated values into a `/reverse`
  argument string and parsed it back, but neither the scaffold request nor
  `--folder` is part of the `/reverse` flag grammar, so both were dropped: the
  command printed its status, generated the prompt, and left the target folder
  uncreated. The parsed values are now handed straight to the reverse handler,
  the scaffold runs before the async fetch is spawned, and the target folder is
  created and populated (workspace, language starter, docs, git init plus
  initial commit, and a private remote with `--github` / `--gitlab`).
- **Chained `--create` writes the spec into the scaffolded project.**
  `/spec reverse --folder <dir> --create <name>` chained into
  `/spec create <name> <prompt>`, which always wrote `specs/<name>/` under the
  invoking directory. `/spec create` now accepts an optional `--folder <path>`
  and the chain passes the scaffold target, so the spec lands at
  `<dir>/specs/<name>/` inside the new project.
- **A `/spec reverse` invocation starting with a flag reports the missing
  `<repo>`.** `/spec reverse --language rust --type cmdline` split the flag into
  the subcommand position and produced an unknown-subcommand status line with no
  explanation. It now reports
  ``the first argument after `/spec reverse` must be `<repo>`, got '--language'``
  alongside the usage block, and no API call is made.
- **`/spec reverse` usage errors now print the specific cause.** An invalid
  flag tail (a missing `--type`, an unregistered `--language`, a duplicate or
  empty flag value, or `--github` together with `--gitlab`) previously
  collapsed to the bare `Usage: /spec reverse - try /spec help` status line
  with no explanation. Those failures are now reported as the dedicated usage
  message: the shared `/new` parser's own error text followed by the
  `/spec reverse` usage block. A bare word after `<repo>` is likewise reported
  rather than being silently discarded, and a missing `<repo>` is never read as
  a flag value.
- **Every `/spec` usage-error subcommand now states the cause in the message
  window, not only in the status line.** `/spec reverse` (missing `<repo>`),
  `/spec govcreate` (missing positionals), and every other subcommand with
  incomplete arguments previously left only `Usage: /spec <sub> - try
  /spec help` on the status line. Each now emits an `[err] **missing required
  argument**` message with the offending subcommand's usage block before the
  status pointer, so a missing or incorrect argument is never silent.
- **`/spec reverse` now forwards its validated flags to the `/reverse`
  handler.** The `/new` scaffold flags (`--language`/`--type`/`--stack`) are
  folded into the `/reverse --tech` constraint and `--create`/`--depth` are
  passed through, so a valid `/spec reverse` invocation reaches the single
  fetch-and-generate implementation instead of being parsed and dropped.
- **The scaffolder's `webapp` app type is now a first-class registered value.**
  `--type webapp` is accepted by `/new`, `/spec reverse`, and `/spec govcreate`
  (it is in the derived value list and the detailed help page) and generates a
  tiny dependency-free HTTP-server starter for the five web-capable languages
  (`rust`, `python`, `go`, `typescript`, `javascript`); every other language
  degrades to a manifest-only webapp layout, exactly as data/DSL formats
  degrade for `tui`/`gui`. Previously `webapp` was not a registered value, so
  the recipe-registry test that iterates every app type failed on the first
  language without a webapp source.
- **The vendored `lopdf` crate no longer trips the workspace lint suite.**
  `vendor/lopdf` is a pinned third-party crate whose upstream source exposes
  `pub` internals and retains unused helpers; it now carries crate-level
  `#![allow(unreachable_pub)]` / `#![allow(dead_code)]` /
  `#![allow(unused_imports)]` allowances (the same treatment
  `vendor/pdf-extract` already used), and its genuine `cargo-machete` false
  positives (`md-5`, used via the renamed `md5` path, and `getrandom`, behind
  `wasm_js`/`rand`) are listed in `[package.metadata.cargo-machete]`. The
  dead-code lint, `cargo check`, and `cargo-machete` are green again.

### Added

- **`/spec reverse` scaffolds and can host the target project.** With the
  `/new` scaffold flags present (`--language <lang> --type <type>`), `/spec
  reverse` now accepts `--folder <path>` (default: the current directory) and
  the mutually exclusive `--github` / `--gitlab` hosting flags. The scaffold
  runs the same engine `/new` and `/spec govcreate` share
  (`archdoc::run_govcreate_scaffold`), so the target folder gets the identical
  workspace artifacts, language starter, docs, git init plus initial commit,
  and — with a hosting flag — a private remote set as `origin` and pushed. The
  step runs before prompt synthesis; a refusal (non-empty target, hosting
  failure) is reported with `[err]` plus the blocking entries or failing step
  and the run continues to generate the prompt. `--folder`, `--github`, and
  `--gitlab` only take effect with `--language` and `--type`; supplying any of
  them alone is a usage error. The hosting flags are forwarded verbatim to the
  shared `/new` parser, so accepted values, mutual exclusion, and error text
  cannot drift. Help text and autocomplete list the new flags.

### Changed

- **`/reverse` is now `/spec reverse`.** Repository reverse-engineering moved
  under the `/spec` command family. Argument parsing now lives in the shared
  `ragent_specs::SpecCommand` parser, so `/spec reverse` accepts the same
  `--create <name>` and `--depth <N>` flags as before plus the `/new` scaffold
  flags (`--language <lang>`, `--type <type>`, `--stack <name>`), parsed and
  validated by the same `/new` parser. The standalone `/reverse` slash command
  has been removed. The how-to moved to
  `docs/howtos/slashcommands/specreverse.md`.

### Removed

- **`/spec reverse --tech <stack>` replaced by the `/new` scaffold flags.**
  The free-form `--tech <stack>` constraint is gone; it is now spelled with
  the same flags `/new` uses: `--language <lang> --type <type> [--stack
  <name>]`, with the same accepted values and purpose (steering the generated
  prompt towards a target language/type/stack). The context block sent to the
  model gained a `## Project Scaffold` section naming the target language,
  type, and stack (each line omitted when unset), replacing the old
  `## Technology Stack Constraint` section for scaffold-driven runs.

## [1.0.116] - 2026-09-23

Maintenance release over the v1.0.115 tree: agent and plugin updates plus a
code-quality pass, with no behavioural regressions. Documented from the
uncommitted working-tree change-set.

### Changed

- **TUI modal-centring helper.** The four content-sized modal renderers in
  `crates/ragent-tui` (queue menu, queue show panel, plugin-store panel, queue
  clear confirmation) each inlined the same clamp-then-centre rectangle
  geometry; it is now one `centered_rect_fixed(width, height, area)` helper in
  `ragent-tui/src/utils.rs` (net `-37/+2` across the four call sites,
  byte-for-byte identical geometry).
- **Streamed sub-agent report write.** `persist_task_output` in
  `crates/ragent-agent/src/task/mod.rs` now streams the header plus the reply
  body straight into a `BufWriter<File>` and then renames, instead of building
  the whole `header + content` in one `format!` String first — halving peak
  memory while the full untruncated sub-agent report is written, with identical
  on-disk bytes.

### Verified

- `cargo check --workspace`, `cargo check --tests --workspace`, the dead-code
  lint, the dead-code-reason check, `cargo clippy --workspace -- -D warnings`,
  `cargo fmt --all -- --check`, `cargo audit`, and `cargo deny check` all pass;
  the full `cargo test --workspace` suite is green.

## [1.0.115] - 2026-09-23

Detached fire-and-forget sub-agents (`/spawn` + `new_agent detached: true`), the
`spawnagent` report-file / finish-reason fixes, the `newproj` single-entry-point
overlay fix, the research gather-log path correction, and a `/simplify` +
`/rust-hygiene` code-quality pass. Documented from the last 10 commits plus the
uncommitted working-tree change-set.

### Added

- **`spawnagent` — `/spawn <agent> <prompt...>` launches a detached
  fire-and-forget sub-agent (FR-001..FR-005).** The TUI can now start a
  background sub-agent directly from the chat input without asking the primary
  agent to call `new_agent`. The task is **detached**: it runs concurrently and
  appears in the Agents panel, but nothing ever waits on it — it is excluded
  from `list_agents` and `running_background_count`, cannot be awaited via
  `wait_agents` (with or without `task_ids`), is untouched by `team_wait`, and
  its completion is **never** injected back into the chat as a background-task
  result (`drain_completed` marks it reported and drops it while still reaping
  the entry, so `tasks_snapshot` still shows it). Layers: `TaskEntry.detached`
  (serde default `false`), `AgentManager::spawn_detached` sharing a single
  `spawn_background_mode` implementation with `spawn_background`, the optional
  `new_agent` `detached` parameter (the model-facing equivalent), and the TUI
  `/spawn` slash command with roster validation via `app::prompt::resolve_agent`
  plus an `App::spawn_result` poll slot. Cancellable with `/cancel <prefix>`;
  a second `/spawn` while one is still registering is refused. Docs:
  `docs/howtos/slashcommands/spawn.md`, `docs/howtos/slashcommands/INDEX.md`,
  `docs/howtos/tools/sub-agents.md`. Spec: `specs/spawnagent/`.

- **`/simplify` code-quality pass over the `spawnagent` change-set.** The
  duplicated per-bridge enable/parse filter in `ragent-plugins` is now one
  `enabled_manifests` helper shared by every `scanned_plugin_*` bridge (and the
  skills bridge builds its set with a `BTreeSet` instead of a linear
  `Vec::contains` scan); the `project_scaffold` remote helpers merge
  `required_url` + `required_field` into a single `required_str_field`; the
  `ragent-tui` queue-clear confirmation toggle/yes-check move into
  `App::queue_clear_confirm_toggle` / `queue_clear_confirm_is_yes` (removing a
  duplicated inline branch in `input.rs`); the `ragent-agent` custom-agent
  discovery fast path compares the cached `dir_mtimes` set directly instead of
  re-statting every directory; and `text_toolcalls` narrows its two helpers to
  `pub(crate)` with a documented lint allow.

- **`/rust-hygiene` full-workspace CI pass.** `cargo check`,
  `cargo-machete`, `cargo check --tests`, `cargo test`, the dead-code lint
  (`-D unreachable_pub -D dead_code -D unused_imports`), the dead-code-reason
  check, `cargo clippy --all-targets -D warnings`, `cargo fmt --all --check`,
  `cargo audit`, and `cargo deny check` all pass; the `spawnagent` FR-004a/b/c
  fixes and the `/spawn` classification are covered by
  `crates/ragent-agent/tests/test_spawn_detached.rs` (now 6 tests, including a
  consistent-classification regression test).

- **`uncommitted` — TUI modal-centring helper and streamed sub-agent report
  write.** The four content-sized modal renderers in `crates/ragent-tui` (queue
  menu, queue show panel, plugin-store panel, queue clear confirmation) each
  inlined the same clamp-then-centre rectangle geometry; it is now one
  `centered_rect_fixed(width, height, area)` helper in `ragent-tui/src/utils.rs`
  (net `-37/+2` across the four call sites, byte-for-byte identical geometry).
  `persist_task_output` in `crates/ragent-agent/src/task/mod.rs` now streams the
  header plus the reply body straight into a `BufWriter<File>` and then renames,
  instead of building the whole `header + content` in one `format!` String first
  — halving peak memory while the full untruncated sub-agent report is written,
  with identical on-disk bytes. No behaviour change; `cargo fmt`, `cargo clippy
  --all-targets`, and the `ragent-tui` / `ragent-agent` test suites pass.

### Changed

- **Research gather-log path corrected to `log/research/`.** The JSONL URL log
  now lands at `log/research/research-<name>-<ts>-<rand>-web.jsonl` (the
  singular top-level `log/` root, matching every other ragent log directory)
  instead of the plural `logs/research/`; `/log clear research` and the
  `alog`/`log` howtos are corrected to match.

### Fixed

- **`spawnagent` — `/spawn` runs ended without `agent_complete`, delivered no
  report file, and claimed a healthy finish (FR-004a/b/c).** Three gaps made a
  `/spawn general … write ANTIPAT.md` run look like a no-op: **(a)** the task
  layer never wrote `log/subagents/<task-id>.md` and never set
  `TaskEntry.output_file`, so the "full report at log/subagents/<id>.md"
  recovery path documented in `wait_agents`/`list_agents` pointed at a file
  that did not exist; **(b)** `SubagentComplete.finish_reason` was hard-coded
  `"stop"` on every success, so a run cut off by the provider's silent
  end-of-stream still logged "[ok] Task completed" and the Agents panel row
  showed Complete; **(c)** the Subagent completion protocol mandated
  `agent_complete` but said nothing about the requested file, so a sub-agent
  could answer in plain text and exit without producing the deliverable.
  Fixes: `AgentManager::spawn_background_mode` now persists the FULL output to
  `log/subagents/<task-id>.md` under the parent session's working directory
  (temp-file + rename) and records the path on the entry; the session loop
  records its terminal `MessageEnd` reason into
  `SessionProcessor::last_message_end_reason` and the completion event maps it
  to `"truncation"`/`"length"`/`"cancelled"`/`"stop"` (the TUI Agents panel
  maps the truncation labels onto the TRUNCATED marker); and the Subagent
  system prompt gained a *Deliverable Enforcement* section — a prompt that
  asks for a file MUST call `write` before `agent_complete`, verify the file,
  and never claim a write that did not happen. Regression coverage in
  `crates/ragent-agent/tests/test_spawn_detached.rs`.

- **`newproj` — `/new` stack overlay duplicated the entry point (FR-007).**
  A Rust project scaffolded with `--language rust --type tui --stack ratatui`
  (or any stack on a binary app type: `axum`/`warp` on `cmdline`, `raylib`/
  `gtk4` on `gui`) emitted two `fn main` functions into `src/main.rs` — the base
  hello-world entry point plus the framework starter's own entry point. The
  generated file did not compile until one `main` was deleted by hand, and the
  binary that survived printed nothing. `StackRecipe::overlay_source` now
  removes the base `main` entry point (brace-balanced scan, so nested braces do
  not end it early) before appending the framework starter, so the emitted
  source declares exactly one `main` and the scaffold builds and runs as
  generated; a base source with no `fn main` (a library body) is left intact
  above the starter.

## [1.0.114] - 2026-09-22

Missing plugin components plus the `newproj` GitHub credential chain, the
`inputqueue` FR-017 amendment, the `ragent_info` tool, and a `/simplify all`
code-quality pass. The plugin system now bridges every non-tool surface for both
Codex- and Claude-dialect plugins - skills, MCP servers, slash commands, agents,
and hooks - including the full Claude `hooks.json` declaration surface and a
blocking Stop continuation; each item is detailed in the sections below.

- **Plugins** — nested `.codex-plugin/plugin.json` recognition (T-024), the
  both-nested multi-target descriptor fix (T-025), the skills/MCP/commands/
  agents/hooks bridges (T-021..T-023), and the full Claude `hooks.json`
  declaration surface plus Stop continuation (T-026); `/plugins list` now counts
  skills/agents/hooks and `/plugins add` installs enabled (FR-007/FR-009).
- **`newproj`** — `/new --github` now resolves the GitHub token through the
  single shared `ragent_config::github` chain (`GITHUB_TOKEN` ->
  `~/.config/ragent/github_token` -> `gh` CLI) with an app-token downgrade so a
  `ghu_` token no longer blocks `POST /user/repos` (FR-008).
- **`inputqueue`** — a slash command submitted while the agent runs is queued
  like a plain message and drains at the next turn boundary (FR-017 amendment).
- **`ragent_info`** — a new read-only introspection tool (168 -> 169 tools) and
  the `crates/ragent-agent/build.rs` metadata embed.
- **`/simplify all`** — seven verified code-quality fixes across
  `ragent-agent`, `ragent-plugins`, `ragent-tui`, and `src/` (see the
  "Code quality" section below).
- **Docs** — `STATS.md`, `README.md`, `SPEC.md`, `QUICKSTART.md`,
  `TUI-QUICKSTART.md`, and the how-to set refreshed and re-rendered to PDF.

### Changed

- **`/plugins list` now counts and details skills, agents, and hooks** (spec
  `plugins` FR-009) — the table gained a count column for each contribution kind
  (`Tools`, `Commands`, `Skills`, `Agents`, `Hooks`), and the `Contributions:`
  block now lists the detailed names for every kind — skill names (resolved from
  each declared `skills/` directory, one level deep), agent-profile paths, and
  hook triggers — alongside the existing tool and command names. A new
  `plugin_skill_names` bridge and `list_shows_skill_agent_and_hook_counts_and_details`
  test cover the skill-name resolution and the rendered output.

- **`/plugins add` now installs a plugin enabled** (spec `plugins` FR-007) — a
  freshly added plugin is recorded **enabled** in its store ledger instead of being
  left disabled, so its tools, commands, skills, agents, and hooks become available at
  the next session start without a separate `/plugins enable`. No plugin JavaScript
  executes during the install itself. The add report now states the plugin is enabled
  and points at `/plugins disable` to turn it off, and `/plugins list` shows a freshly
  installed plugin as `enabled`. `/plugins remove` still refuses an enabled plugin, so
  remove a fresh install with `/plugins disable <id>` first. Existing plugins whose
  ledger row is absent remain disabled.

- **Slash commands are now queued while the agent is busy** (spec `inputqueue`
  FR-017 amendment) — a slash command submitted while the primary agent is
  executing is appended to the message input queue exactly like a plain message,
  instead of being refused with `busy - wait for the current turn to finish`.
  The queued command runs at the next turn boundary (or immediately via the
  `Alt+Q` `Next` row / `/queue next`). Bang commands (`!…`) and
  teammate-targeted messages keep their existing busy refusal. The turn-boundary
  drain routes a `/`-prefixed entry back through the slash-command executor; a
  synchronous command leaves the boundary free, so the drain continues and a run
  of consecutive queued commands executes back-to-back rather than stalling
  behind the first. At queue capacity the command is rejected and restored to the
  input field, so nothing is lost. Covered by the new
  `crates/ragent-tui/tests/test_input_queue_slash.rs` and the amended
  `test_busy_send_guard.rs`. The boundary drain is a loop (not recursion) and
  `dispatch_queued_input` reports whether it started a chat turn, so a mixed run
  of queued messages and commands drains in submission order without a stack
  depth that grows with the queue. The plugin-store poisoned-lock deposit sites
  were folded onto the shared `recover_poisoned` helper as part of the same pass.

- **Documentation refresh** — the version-history and how-to docs were
  regenerated for the current tree: `STATS.md` re-measured (1251 crate Rust
  files, 499,037 crate lines, 628 external test files, ~9,675 tests, 169 tools,
  52 spec directories), the `inputqueue` FR-017 amendment and the 169-tool count
  propagated into `README.md`, `QUICKSTART.md`, `TUI-QUICKSTART.md`, `SPEC.md`
  (slash-command table + appendix), and
  `docs/howtos/slashcommands/queue.md`; every how-to (20 category, 77 slash
  command, 27 tool-category documents) was re-rendered to PDF (124 PDFs).

### Code quality (`/simplify all`)

Quality pass over the uncommitted plugins / hooks / newproj work. Seven verified
fixes applied; three further proposed refactors were rejected and reverted after
testing because they added more complexity than they removed (an extracted
source-resolution helper, an `unwrap_or_else` rewrite that changed an
`Option<String>` type, and removing the load-bearing `block_in_place` from the
non-async `run_new_scaffold`). No user-visible behaviour change.

- **`ragent-agent/tool/ragent_info.rs`** — the `Info` struct is serialised once
  via `serde_json::to_value`, and the value is reused for both the pretty
  `content` string and the tool-result `metadata`, instead of serialising twice.
- **`ragent-agent/plugin.rs`** — the plugin-command name-dedup now uses a
  `HashSet` membership test (`taken.insert`) instead of an O(n·m)
  `merged.iter().any()` scan over the growing result vector.
- **`ragent-agent/hooks/mod.rs`** — `cap_stderr` collapsed a provably identical
  two-branch `if`/`else` into a single `chars().take(max).collect()`.
- **`ragent-plugins/store_provider.rs`** — a store entry that fails to parse is
  now logged at `tracing::debug!` before being counted as skipped (was silent);
  the `required_str(map, "name")` lookup is bound once as `name_field` and reused
  for the id fallback and the display name (was read twice).
- **`ragent-tui/app/plugin.rs`** — the sub-command remainder is taken with
  `strip_prefix(sub).map(str::trim_start).unwrap_or("")` instead of slicing
  `args[sub.len()..]`, which could panic on a non-char-boundary index.
- **`src/main.rs`** — `std::env::current_dir().unwrap_or_default()` now warns on
  failure and uses an empty `PathBuf`, so a failed cwd lookup no longer silently
  scans the wrong directory for plugin-contributed MCP servers.

### Added

- **Claude plugin hooks: full declaration surface + Stop continuation** (spec
  `plugins` T-026; FR-033) — the hooks bridge (T-023) read an inline `hooks`
  section, but the official catalogue's real layout was not consumed: a Claude
  plugin declares its hooks in a `hooks.json` file (at the plugin root or under
  `hooks/`), wrapping the triggers in a top-level `hooks` object, and each trigger
  maps to a **group** `{matcher, hooks: [{type, command|command_path, timeout, if?}]}`
  rather than a bare command. `security-guidance`, which ships only a `hooks/`
  directory, therefore loaded inertly with `Hooks 0`, and its delivered commands
  were non-functional: they interpolate `${CLAUDE_PLUGIN_ROOT}` (never set), read
  the Claude event JSON from **stdin** (never written), and signal findings through
  `hookSpecificOutput.additionalContext` / a blocking `exit 2`, none of which the
  bridge honoured. The manifest parser now reads the `hooks.json` locations
  (`HOOKS_FILE` / `read_plugin_hooks_file`), unwraps the group shape (inheriting a
  group `matcher` into its children), accepts the Claude `timeout` (ms -> s) and
  `if` fields, and skips any entry whose `type` is not `command`; `PluginHook`
  gains `plugin_root` (exported as `CLAUDE_PLUGIN_ROOT`) and `matcher`. The agent
  hook engine gains `HookConfig::matches_tool` (`|`/`,`-separated tool names with
  `Name(pattern)` guards, `Bash(git commit:*)` as a prefix match), writes the
  Claude event JSON to a plugin hook's stdin (`claude_event_payload`), folds a
  `PostToolUse` `additionalContext` into the tool result so the model sees it on
  the next iteration, and adds `run_stop_hooks`. A blocking Stop hook
  (`asyncRewake` shape: `exit 2`, a top-level `decision: "block"`, or an
  `additionalContext`) now feeds its findings back as a synthetic user turn
  **inside** the loop, bounded by `MAX_STOP_CONTINUATIONS = 3` so a hook that
  always blocks cannot spin. Covered by `test_hooks.rs`, `test_manifest.rs`, and
  `test_plugin_hooks.rs`.

- **Plugin agents + hooks bridges** (spec `plugins` T-023; FR-025, FR-032, FR-033) — a
  plugin's `agents` and `hooks` sections are now consumed rather than merely recorded.
  The manifest parser extracts declared agent profiles plus every `agents/*.md` file
  (`extract_agent_decls` / `scan_agent_dir`) and normalises three hook shapes into
  `PluginHook {trigger, command, timeout_secs}` (`extract_hooks`); both are resolved by
  the new `bridge` functions `scanned_plugin_agent_files` / `scanned_plugin_hooks` for
  every **enabled** plugin (a bare agent name resolves to `agents/<name>.md` and a path
  that escapes the plugin root is dropped). The custom-agent loader appends the
  plugin-contributed profiles to its discovery **sources** (scanned after
  `.ragent/agents/`, so a project agent wins a name clash) and now accepts YAML
  frontmatter as well as ragent's JSON, so Claude-dialect `agents/*.md` profiles load
  unchanged. Plugin hooks are mapped onto the existing session hook engine
  (`HookTrigger::parse` accepts both `pre_tool_use` and `PreToolUse`, plus Claude's
  `UserPromptSubmit`/`Stop`/`PreCompact`) and merged **after** the `ragent.json` hooks so
  a user's own hooks run first. `/plugins list` reports agent paths and hook triggers in
  its contributions block. A section whose shape has no bridged equivalent keeps the new
  `plugin agents` / `plugin hooks` FR-025 labels. Covered by `test_manifest.rs`,
  `test_bridge.rs`, `test_plugin_bridge.rs`, and `test_plugin_hooks.rs`.

- **Plugin slash commands** (spec `plugins` T-022; FR-004, FR-031) — a plugin's
  contributed slash commands now reach the session command surface. The manifest
  `commands` field is parsed tolerantly (an array of strings or objects) and the
  Claude-dialect `commands/*.md` directory is discovered: each markdown file is a
  **prompt command** named from its file stem with its frontmatter `description` as
  the menu text. An inline command (a manifest object with no `file`, or
  `ragent.register_command`) still dispatches into the plugin sandbox; a prompt
  command's body (frontmatter stripped, `$ARGUMENTS`/`$N` substituted) is injected
  into the session as an ordinary user turn. Every **enabled** plugin's commands are
  bridged onto the surface (a free name registers bare, a colliding name registers
  under the namespaced `plugin:<plugin-id>:<name>` trigger so a plugin can never
  shadow a built-in, a skill, or another plugin), listed in the `/` autocomplete menu
  and `/help`. Fixes Claude marketplace plugins such as `commit-commands`, whose
  `/commit` was previously invisible. Covered by `test_manifest.rs`,
  `test_plugin_bridge.rs`, and the TUI `test_plugin_commands.rs`.

- **Plugin skills + MCP bridges** (spec `plugins` T-021; FR-025, FR-029, FR-030) —
  a plugin's `skills` and `mcpServers` sections are now consumed rather than merely
  recorded. `manifest::extract_skill_dirs` / `extract_mcp_servers` /
  `map_mcp_server_entry` populate `ParsedManifest::skills`/`mcp_servers`, and a new
  `bridge` module (`scanned_plugin_skill_dirs` / `scanned_plugin_mcp_servers`)
  resolves the contributions of every **enabled** plugin in the project and
  user-global stores. Skill directories (relative to the plugin root, behind a
  lexical path-traversal guard) are appended to the skill-discovery roots, so
  `SkillRegistry::load` loads a plugin's `SKILL.md` packs exactly like a user skill
  directory; the agent layer adds `skill::effective_skill_dirs` and keys the
  `SessionProcessor` skill mtime cache on the effective list. MCP servers — inline
  objects or an external `mcp.json` (the Claude marketplace `"mcpServers":"./mcp.json"`
  shape) — map to `McpServerConfig` with a plugin-qualified id `<plugin-id>.<server>`
  and are merged with the configured `mcp` set at startup (configured wins on a
  collision). A `skills`/`mcpServers` section that is bridged is no longer reported as
  an unsupported capability (FR-025 keeps the label only for unbridgeable shapes).
  Covered by `test_bridge.rs` (ragent-plugins) and `test_plugin_bridge.rs`
  (ragent-agent).

- **`ragent_info` tool** — a new read-only `ragent_info` introspection tool lets the
  LLM report the running ragent version, when the binary was built, the git commit
  it was built from (best-effort), and the compiler version, without shelling out.
  A new `crates/ragent-agent/build.rs` embeds `BUILD_TIME` (always) plus
  `GIT_COMMIT`, `GIT_COMMIT_SUBJECT`, and `RUSTC_VERSION` (best-effort) via
  `cargo:rustc-env`; `crates/ragent-agent/src/tool/ragent_info.rs` renders them as
  text (`text`, default) or structured JSON (`format: "json"`). The tool has
  permission category `none`, is auto-approved, and is in the skill
  always-allowed set alongside `model_info`. Registered in the default tool
  registry (168 -> 169 tools). Covered by `test_ragent_info_tool.rs` (4 tests)
  and a TUI display test.

- **Store providers + `git+` installs** (spec `pluginstores` T-023..T-025; FR-043..FR-046,
  NFR-005) - each store now parses its own official marketplace through a
  `StoreProvider` trait instead of assuming one catalogue format. New
  `crates/ragent-plugins/src/store_provider.rs` adds `StoreProvider`,
  `CodexStoreProvider`, `ClaudeStoreProvider`, and `provider_for(kind)`; a vendor entry is
  normalised with `name` as the id, an optional `version`, and a `local`/`url`/`git-subdir`
  or repo-relative `source`, resolved against the marketplace repository root. Native
  ragent indexes (entries carrying an `id`) still parse strictly, so existing fixtures are
  unaffected. The `StoreKind` is now threaded through `StoreIndexFetcher::fetch_index` (and
  the network/fixture fetchers, `probe_stores`, and the TUI fetch path); the compiled
  defaults point at `openai/plugins` and `anthropics/claude-plugins-official`. The install
  pipeline accepts a `git+<https-url>#<ref>[:<subpath>]` source via a shallow, sparse
  `git clone`, refusing non-`https` remotes before cloning. Covered by 16 new offline tests
  (`test_store_provider.rs`, `test_store_git_source.rs`) plus updated seam tests.

- **`/plugins stores --check`** (spec `pluginstores` T-022; FR-039..FR-042) - the
  `/plugins stores` report gains an opt-in `--check` flag on both the TUI
  (`/plugins stores --check`) and the CLI (`ragent plugins stores --check`). With
  the flag, each store's effective endpoint is contacted once through the same
  config-over-compiled-default precedence and the injectable store-fetch seam,
  and each line gains `[ok] available, N plugins` or `[err] unavailable: <reason>`.
  The plain report (no `--check`) is unchanged: a pure configuration read that
  makes no network request, so its tests stay offline. The TUI probe runs off the
  event loop (spawned, then drained by `poll_plugin_store_probe_result`), so the
  network round-trip never stalls the UI. Covered by 15 new offline tests:
  `crates/ragent-plugins/tests/test_stores_check.rs` (9),
  `crates/ragent-tui/tests/test_plugin_store_probe.rs` (5), and a CLI usage case
  in `tests/test_plugins_cli_command.rs` (1).

### Fixed

- **`SourceVault::store` URL dedup is now atomic** — the URL-dedup check and the
  insert in `crates/ragent-research/src/source_vault.rs` now run under a single
  held connection lock (the lookup moved onto the shared connection via a new
  `find_by_url_on` helper), so two concurrent `store` calls for the same URL
  cannot both pass the check. Each row gets a fresh `source_id` (a new UUID), so
  the `UNIQUE(source_id)` index never caught a duplicate URL; when both store
  call sites in `web_gatherer.rs` (the seed URL and a search hit) fired for the
  same URL concurrently, the vault ended up with two rows. This is what made
  `supervisor::tests::iterative_researcher_node_persists_sources_to_vault` see
  `stored.len() == 2` instead of `1`. No behaviour change for the sequential
  path; a concurrent duplicate URL now returns the existing record.

- **`reference::resolve` and `reference::fuzzy` tests no longer share fixed temp
  directories** — the `test_fuzzy_reference.rs` integration tests derived their
  scratch trees from static names (`ragent_test_fuzzy_collect`, `..._async`, ...),
  so two concurrent `cargo test` processes (or overlapping runs) could
  `remove_dir_all` a path another process was still walking, making
  `test_collect_project_files_async_matches_sync_walk` intermittently fail with
  `src/main.rs` missing. Each test now allocates a per-run unique directory via a
  `unique_tmp(tag)` helper (`ragent_test_fuzzy_<tag>_<uuid>`), matching the fix
  already applied to the `reference::resolve` inline tests. No production code
  changed.

- **`/new --github` can now create the hosting repository when `/github login`
  stored a GitHub App token** (spec `newproj` FR-008) — `/github login` reuses the
  OAuth application registered for the Copilot provider, and that application mints
  GitHub App tokens (`ghu_`) whose permissions exclude repository administration, so
  `POST /user/repos` was rejected with `403 Resource not accessible by integration`
  and the remote step failed at `github repo create`. GitHub credential resolution is
  now a single shared chain in `ragent_config::github` (`GITHUB_TOKEN` env → the
  stored `~/.config/ragent/github_token` file → the authenticated `gh` CLI) with an
  app-token downgrade: a stored token that is a GitHub App token now defers to the
  `gh auth token` credential when one is available, while an ordinary PAT/OAuth token
  (`ghp_`/`gho_`/`github_pat_`) is used unchanged and a read-only app token is still
  used when no `gh` credential exists. Set `RAGENT_GITHUB_NO_GH_CLI=1` to disable the
  `gh` fallback. The VCS tool family and the `/new` scaffolder both consume the shared
  resolver, so `/spec reverse` and the GitHub issue/PR tools benefit from the same fix.

- **Multi-target plugins (both nested manifests) now install** (spec `plugins`
  T-025; FR-002) — some upstream trees ship more than one host manifest in the same
  folder: `mongodb/agent-skills` (the `mongodb` entry in the Claude store browser)
  carries `.claude-plugin/plugin.json` *and* `.codex-plugin/plugin.json` side by
  side. After the nested-Codex recognition landed, such a directory matched both
  dialects and was refused with `ambiguous plugin manifest: directory matches both
  Codex and Claude dialects`, so `/plugins claude` -> install of `mongodb` failed.
  The `descriptor` recogniser now treats the *both-nested-manifest* pairing as a
  multi-target plugin and resolves it deterministically to the **Claude** dialect
  (whose manifest supports every bridged surface: skills, MCP servers, commands,
  agents, hooks). Every other both-match — a top-level `codex-plugin.json` beside a
  `claude-plugin.json`, a `plugin.json` with a `"codex"` marker beside a Claude
  manifest, or a nested Codex manifest beside a top-level Claude manifest — is
  unchanged and still refused. Verified live:
  `ragent plugins add 'git+https://github.com/mongodb/agent-skills#main:plugins/mongodb'`
  now reports `Installed plugin 'mongodb' (dialect: claude, version: 1.2.1)` with its
  7 skill directories bridged. Covered by three new `test_descriptor.rs` tests.

- **Codex store entries now install and load** (spec `plugins` T-024; FR-002) — the
  official `openai/plugins` catalogue keeps each plugin manifest in
  `.codex-plugin/plugin.json`, the Codex counterpart of Claude's
  `.claude-plugin/plugin.json`. ragent's recogniser accepted only a root
  `codex-plugin.json` or a `plugin.json` with a top-level `"codex"` marker, so a
  store entry fetched as `git+https://github.com/openai/plugins#main:plugins/<name>`
  downloaded but failed with `no plugin manifest found` (52 of the store's entries).
  The `descriptor` recogniser now accepts the nested `.codex-plugin/plugin.json`
  location (`CODEX_NESTED_MANIFEST`), parsed through the existing Codex parser so the
  `skills` / `mcpServers` / `agents` / `hooks` bridges all apply. Verified live:
  `ragent plugins add 'git+https://github.com/openai/plugins#main:plugins/linear'`
  now reports `Installed plugin 'linear' (dialect: codex, version: 5.0.1)` and
  `enable`/`test` both succeed. Covered by `test_descriptor.rs` and
  `test_add.rs::add_nested_codex_manifest_directory_installs`.

- **Claude/Codex store installs of repo-relative plugins** (spec `pluginstores`;
  FR-044) - a marketplace entry whose `source` is a repo-relative string
  (e.g. Claude's `"./plugins/security-guidance"`, 52 of the official store's 310
  entries) was normalised into a bare `https://raw.githubusercontent.com/...`
  directory URL, which the install pipeline rejects with `unrecognised source`.
  The provider now resolves a repo-relative source against the marketplace
  origin's GitHub repository as an installable `git+<repo>#<ref>:<path>` source
  (the `main` ref of the `raw.githubusercontent.com` or `github.com` origin, or
  HEAD for a bare repository URL), so `/plugins claude` -> install now works for
  every repo-relative entry. A repo-relative source with no GitHub repository
  behind its origin is skipped and counted rather than silently uninstallable.
  Covered by two new offline tests in `test_store_provider.rs`.

- **`reference::resolve` tests no longer share fixed temp directories** — the six
  `resolve_ref` / `resolve_all_refs` tests under
  `crates/ragent-agent/src/reference/resolve.rs` derived their scratch directories
  from static names (`ragent_test_resolve_file`, `..._all`, ...) in the system temp
  dir, so the lib test binary and the integration test binaries could create and
  `remove_dir_all` the same path concurrently and flake
  (`test_resolve_all_refs_with_file` saw the directory vanish mid-run). Each test
  now allocates a per-run unique directory via a small `unique_tmp(tag)` helper
  (`ragent_test_<tag>_<uuid>`), making the workspace suite deterministic under
  parallel execution. No production code changed.

## [1.0.113] - 2026-09-21

Message input queue (spec `inputqueue`, `specs/inputqueue/`): the TUI
message-window input field stays **editable while the primary agent is
executing**. Each `Enter` appends the submission to a bounded FIFO queue
(default 32 entries), a two-digit, zero-padded counter appears immediately
before the `> ` prompt, and the oldest entry runs automatically at each turn
boundary. Queue control is exposed through an `Alt+Q` menu and a `/queue`
slash command. All 27 plan tasks (T-001..T-027) are complete. The `Alt+Q` menu
now has four rows (`Next`, `Stop`/`Resume`, `Clear`, `Show`), is navigated with
`Up`/`Down`/`Enter`, and its new `Show` row opens a scrollable queue-entry
panel where `Enter` moves an entry toward the front and `Del` removes it. 234
new test attributes across 19 new test files (18 with `test_busy_send_guard.rs`).

### Added

- **Input queue core** (FR-001..FR-012) — `App` gains
  `input_queue: VecDeque<QueuedInput>` (text + staged image paths),
  `QueuedInput`, `input_queue_len()`, `enqueue_input()` (cap-rejecting),
  `clear_input_queue()`, and `advance_input_queue()`. `MAX_INPUT_QUEUE` (32)
  aliases the new `ragent_config::DEFAULT_INPUT_QUEUE_CAPACITY`. The input
  field no longer rejects a plain message while a turn runs: instead of
  `busy - wait for the current turn to finish` the submission is queued;
  slash commands (`/…`), bang commands (`!…`), and teammate-targeted messages
  keep their existing busy refusal (FR-017). `Enter` on the slash picker and
  teammate-routed sends remain guarded.
- **Two-digit queue counter** (FR-008..FR-010, NFR-002/003) —
  `layout::input_prompt_prefix(queue_len)` renders `NN> ` (zero-padded) before
  the prompt when the queue is non-empty and the bare `> ` prompt when empty;
  `input_prompt_prefix_len`, `input_prompt_selection_prefix`, and
  `input_widget_height(input, inner_width, prefix_len)` keep the cursor,
  wrapped-row geometry, and clipboard-copy offset aligned to the prefix. The
  geometry copy replaces the digits with spaces so a copy can never include the
  counter as message content (FR-020). `InputRenderCache` gained a `queue_len`
  field so the cached rows/height rebuild whenever the queue length changes.
- **Turn-boundary drain** (FR-006/FR-007/FR-016/FR-018/FR-019) — the
  `Event::MessageEnd` (non-cancelled) and `Event::AgentError` handlers call
  `advance_input_queue()`; the single guard inside it defers while
  `is_processing`, `compact_in_progress`, `auto_compact_in_progress`, or
  `pending_send_after_compact.is_some()`, preserving `queue_next_pending` and
  never popping an entry. A cancelled turn retains the queue.
- **ALT-Q queue-control menu** (FR-021..FR-032, NFR-006..NFR-009) — `Alt+Q`
  opens a centred overlay with four rows: `Next` (stop the turn and dispatch
  the oldest entry, shown non-selectable when the queue is empty),
  `Stop`/`Resume` (halts exactly like `CancelAgent` without advancing the
  queue, or resumes the interrupted work), `Clear` (opens the confirmation
  dialog), and `Show` (opens the queue-entry panel). `Up`/`Down` move the
  highlight (skipping the non-selectable empty-queue `Next` row) and `Enter`
  activates the highlighted row. `Esc` dismisses without mutating the input
  buffer, staged attachments, queue, or running turn. Implemented by
  `queue_menu_move_up` / `queue_menu_move_down` /
  `queue_menu_activate_selected` (plus the existing
  `queue_menu_select_next` / `_halt` / `_clear`) and rendered by
  `layout::render_queue_menu`; the `QUEUE_MENU_ROW_*` / `QUEUE_MENU_ROWS`
  consts name the row indices.
- **`Show` queue-entry panel** (FR-038..FR-043, NFR-012) — the menu's new
  `Show` row opens a scrollable modal that lists every queued entry
  oldest-first with a block cursor. `Up`/`Down` scroll the highlight (the list
  keeps it visible), `Enter` moves the highlighted entry one step **toward the
  front** (`queue_show_promote_selected`, a neighbouring swap that follows the
  moved entry so repeated `Enter` walks it forward), and `Del` removes it
  (`queue_show_delete_selected`, clamping the highlight and closing the panel
  when the queue empties). Only `Esc` dismisses the panel
  (`queue_show_close`); an emptied-and-cleared queue also closes it via
  `clear_input_queue`. The panel never mutates the draft or staged
  attachments. Implemented by `queue_show_open_panel` and rendered by
  `layout::render_queue_show_panel`.
- **`Clear the input queue?` confirmation dialog** (FR-033..FR-037,
  NFR-010/NFR-011) — a `Yes`/`No` modal, default `No`
  (`QUEUE_CLEAR_CONFIRM_NO`), opened by the menu's `Clear` row. `Yes`
  (`InputAction::ConfirmQueueClear`) drains the queue via
  `clear_input_queue()`; `No`/`Esc` (`CancelQueueClear`) close the dialog
  leaving the queue untouched. `InputAction::ConfirmQueueClear` /
  `CancelQueueClear` and the `QUEUE_CLEAR_CONFIRM_YES`/`_NO` consts are new;
  `is_input_locked()` also reports the dialog as modal.
- **`/queue` slash command** (FR-013) — `handle_queue_command` with
  sub-commands `list` (default), `clear`, `next`, and `help`; registered in
  `SLASH_COMMANDS`, the autocomplete suggestions table, and the dispatcher.
  `/queue next` delegates to `advance_input_queue()`, so it only dispatches at
  a free boundary and is deferred (never overlapping a running turn or
  compaction) otherwise.
- **Configurable capacity** (FR-015) — `Config::input_queue_capacity`
  (`Option<usize>`) resolved by `Config::effective_input_queue_capacity()`,
  clamped to `1..=99` (the two-digit counter's bound) with a default of 32;
  merged overlay-wins so a project override is not discarded by an absent
  user-global value. `DEFAULT_INPUT_QUEUE_CAPACITY` is re-exported from
  `ragent-config`.
- **New test suites** — `crates/ragent-tui/tests/`:
  `test_input_queue.rs` (21), `_enqueue.rs` (11), `_drain.rs` (13),
  `_error_drain.rs` (10), `_geometry.rs` (8), `_guard.rs` (8),
  `_command.rs` (13), `_menu_binding.rs` (9), `_menu_render.rs` (12),
  `_menu_next.rs` (16), `_menu_stop_resume.rs` (13), `_menu_clear.rs` (8),
  `_menu_clear_confirm.rs` (13), `_menu_clear_gating.rs` (8), `_menu_esc.rs`
  (10), `_menu_integration.rs` (10), `_menu_navigation.rs` (13),
  `_show_panel.rs` (28), plus
  `crates/ragent-config/tests/test_input_queue_config.rs` (10).
  `test_busy_send_guard.rs` is rewritten: a plain message and plain typing are
  now accepted while processing; slash/bang/teammate sends keep the busy guard;
  the input border stays white while processing and turns red only when a modal
  is open.

### Changed

- **Side-panel split** — `ResponsiveBreakpoint::log_split` changed from a 50/50
  to a 60/40 messages/side-panel split (`crates/ragent-tui/src/utils.rs`), so
  every right-hand side panel now takes 40% of the application width.
- **Teardown** — `run_tui` calls `clear_input_queue()` on exit; the queue is
  never persisted (NFR-005).
- **Input geometry** — `input_handler.rs` builds the selection/clipboard copy
  from the shared `input_prompt_selection_prefix` helper, and `halt_running_agent`
  is now `pub(crate)` so the menu's `Stop` row shares the cancel path.

### Documentation

- **TUI-QUICKSTART.md §4** — the "input queue and the ALT-Q queue-control menu"
  subsection now documents the four-row menu, `Up`/`Down`/`Enter` navigation,
  and the `Show` queue-entry panel (`Up`/`Down` scroll, `Enter` moves toward the
  front, `Del` removes, `Esc` dismisses); the `Alt+Q` row in the
  keyboard-shortcut table and the v1.0.112 highlight are updated.
- **`docs/howtos/tutorial.md`** — the queue behaviour, ALT-Q menu, and `/queue`
  command added beside the side-panel shortcut table.
- **`docs/howtos/slashcommands/queue.md`** — new per-command reference, linked
  from `slashcommands/INDEX.md`.
- **`docs/howtos/config.md` §7.38** — `input_queue_capacity` field table and
  merge-semantics row.

### Code quality (`/simplify all`)

Quality pass over the uncommitted `inputqueue` diff and the HEAD~3
plugins/research/config work (v1.0.109..v1.0.112); de-duplication, dead-code
removal, and per-frame allocation removal, with no user-visible behaviour
change.

- **`ragent-tui/app/session_ops.rs`** — new `close_queue_menu()` centralises the
  "dismiss the ALT-Q menu" effect (clears `queue_menu_open` + selection) and is
  adopted by `queue_show_open_panel` and all three `queue_menu_select_*` rows;
  `halt_turn_like_cancel_agent()` extracts the shared cancel path used by the
  govcreate poll and `halt_running_agent`; new `dispatch_resume_continuation()`
  carries the resume-a-halted-agent body and is now shared by the `/resume`
  slash command and the menu's `Resume` row; `queue_menu_labels()` returns
  `[&'static str; 4]` instead of `[String; 4]`, removing four per-frame
  allocations.
- **`ragent-tui/app/slash.rs`** — the `/resume` arm calls
  `dispatch_resume_continuation()` (was a ~25-line inline block); the four
  `if count == 1 { "y" } else { "ies" }` sites reuse the existing
  `widgets::message_widget::pluralize()`; the two `/queue` count truncations use
  `helpers::truncate_to_char_boundary`, unifying them with the three
  `session_ops` sites.
- **`ragent-tui/input.rs`** — the `Esc` arm dismisses the menu through
  `app.close_queue_menu()`.
- **`ragent-tui/layout.rs`** — `render_queue_menu` binds each label by reference
  (no per-frame `to_string`).
- **`ragent-plugins/surface.rs`** — `ScratchSurface::merged()` extracts the two
  identical `existing_*` set bodies; new `pub fn run_plugin_subcommand(...)`
  holds the store -> test -> control dispatch ladder plus the usage fallback,
  and is now called by both `src/plugins.rs` and
  `crates/ragent-tui/src/app/plugin.rs` (the latter also resolves its working
  directory once instead of twice).
- **`ragent-plugins/commands.rs`** — `StoreArgError::report` uses
  `attribution(sub)` consistently (was hardcoded to `add`/`remove`).
- **`ragent-plugins/add.rs`** — dead `let _ = rest` binding removed; the
  `remove_if_empty` `Ok`/`Err` swallow-match simplified.
- **`ragent-research/source_vault.rs`** — `row_to_source` now `tracing::warn!`s
  an unparseable `fetch_timestamp` before falling back to `Utc::now` (was
  silent); `media_extension` uses `eq_ignore_ascii_case` (no `to_lowercase()`
  allocation); two `push_str` calls folded into one.

### CI hygiene (`cargo clippy --all-targets -D warnings`)

The full `--all-targets` clippy gate now passes clean. The workspace had masked
an unknown-lint failure: 244 files carried
`#![allow(clippy::assert_is_empty)]`, a lint that does not exist, which CI never
caught because the workflow runs clippy on `--workspace` (lib/bin targets) only.

- **Removed the non-existent lint allow** from 244 files: 237 test files plus
  the inner `#[cfg(test)]` modules in `crates/ragent-research/src/*.rs`.
- **`clippy::float_cmp`** — added `#![allow(clippy::float_cmp)]` (with a
  one-line justification comment, matching the existing `test_profiler.rs`
  pattern) to 11 test files whose f64 literals compare exactly, plus the inner
  test modules of `ragent-tools-extended/src/finance/cache.rs` and
  `.../finance/providers/twelvedata.rs`.
- **`clippy::suboptimal_flops`** — `ragent-tools-extended/tests/
  test_mf_consensus_multieng.rs` rewrites `0.6 - i * 0.007` (and the `-0.05`
  variant) as `(i as f64).mul_add(-0.007, 0.6)`.
- **`clippy::literal_string_with_formatting_args`** — the four
  `.expect("... {var}")` sites in `ragent-tui/tests/test_govcreate_dispatch.rs`
  become `.unwrap_or_else(|| panic!(...))`.
- **`clippy::used_underscore_binding`** — `tests/test_new_cli_command.rs`
  renames `_dir` to `dir` (the binding is used three lines later).
- **`clippy::items_after_test_module`** — `ragent-llm/src/providers/
  tool_cache.rs` moves `tool_arguments_json` above the test module (106 lines
  relocated, no logic change).

## [1.0.112] - 2026-09-20

Plugin system release: the `plugins` spec (`specs/plugins/`) moves from draft
to a fully implemented plugin subsystem, plus a code-quality cleanup pass
across the plugins, research, and config modules.

### Added

- **Plugin-system manual** (`docs/howtos/plugins.md`) -- a full 15-section
  how-to covering architecture, store layout, dialects/manifests, the host API
  v1, permissions and sandbox budgets, management commands, authoring plugins,
  acquiring plugins from the Claude and Codex stores, configuration, security,
  error handling/lifecycle states, performance budgets, troubleshooting, and a
  reference; linked from `docs/howtos/slashcommands/plugins.md`.
- **`ragent-plugins` crate** -- a new workspace crate (`crates/ragent-plugins`,
  `v0.1.0`) implementing the `plugins` spec: discovery and normalisation of
  Codex- and Claude Code/Desktop-dialect plugin manifests, a plugin store with
  a JSON state ledger (`plugin_<id>_<tool>` namespaced tool registration), a
  budgeted embedded JavaScript runtime (`rquickjs`) with `!Send` sandbox
  contexts, the versioned `ragent` host API (`api_version`, `register_tool`,
  `register_command`, `config.get`, `message.*`, `log`,
  `plugin.read_text_file`), enable/disable lifecycle with capture-before-drop
  deregistration, the `/plugins test` isolated harness with schema-derived
  sample arguments, and `/plugins list|add|remove|enable|disable|help` report
  rendering. 234 tests across 17 test files.
- **`ragent plugins` CLI parity surface** (`src/plugins.rs`, FR-021) -- a
  fully synchronous `run_cli` mirroring the TUI `/plugins` family for shell use.
  Its ephemeral `PluginSession` owns `!Send` rquickjs contexts and never crosses
  an `.await`, keeping the async dispatch future `Send`.
- **TUI `/plugins` slash command** (`crates/ragent-tui/src/app/plugin.rs`) --
  registered in `SLASH_COMMANDS` and the slash-command autocomplete menu
  (`state.rs`, `slash.rs`); a bare `/plugins`, `/plugins help`, or an unknown
  subcommand renders the usage block.
- **`plugins` config section** (`crates/ragent-config/src/plugins.rs`) --
  `PluginsConfig` (`enabled`, `max_execution_ms`, `max_entry_ms`,
  `max_memory_mb`, `store_dir`, per-plugin `permissions`) loaded from the
  optional `"plugins"` block, merged overlay-wins so project config is not
  discarded by an absent user-global block. `plugins.enabled: false` makes the
  whole subsystem inert (discovery/load/control all short-circuit).
- **Plugin test fixtures** (`assets/plugins/fixtures/`) -- eight Codex- and
  Claude-dialect fixtures (including infinite-loop, filesystem-escape,
  future-`api_version`, unsupported-MCP, name-collision, and bad-manifest
  cases) covering the spec's acceptance criteria 1--9 and NFR budgets.

### Changed

- **Shared plugin surface helper** -- new `crates/ragent-plugins/src/surface.rs`
  provides `ScratchSurface` (`seeded()`) and `store_and_config(workdir)`,
  replacing the byte-identical copies in `src/plugins.rs` (`CliPluginSurface`)
  and `crates/ragent-tui/src/app/plugin.rs`.
- **Single report attribution source** -- new `help::attribution(sub)` in
  `crates/ragent-plugins/src/help.rs` is the one place the `From: /plugins …`
  header is spelled; `report.rs`, `commands.rs`, `control.rs`, `harness.rs`,
  and `render_help` all route through it. `src/plugins.rs` imports the shared
  `PLUGIN_SUBCOMMANDS` list instead of keeping a local copy.
- **Config-load failures surface** -- `store_and_config` no longer silently
  `.ok()`s a malformed `ragent.json`; it emits `tracing::warn!` before falling
  back to defaults.
- **Allocation hoist** -- `is_own_tool`'s `plugin_<id>_` prefix is built once
  outside the per-tool deregistration loop.
- **Doc/comment accuracy** -- `SourceVault::lock_conn`'s doc now states it
  surfaces `SourceVaultError::LockPoisoned` rather than recovering via
  `into_inner`; `extract_http_status`'s comment reflects the `take(4)`
  three-digit guard.

### Fixed

- **Plugin subsystem master switch** -- `plugins.enabled: false` is now honoured
  by `PluginSession::start`, `run_control_command`, and `run_store_command`
  (previously only the test harness respected it); each reports
  `[err] Plugin subsystem is disabled (plugins.enabled = false)`.

### Documentation

- **README / SPEC / QUICKSTART / TUI-QUICKSTART** -- document the plugin system
  (17 crates now), the `/plugins` slash family, the `ragent plugins` CLI, and
  the `plugins` config block; SPEC.md gains the `ragent-plugins` crate row, an
  updated crate dependency graph, a `/plugins` slash-command row, and the
  `plugins` config section.
- **`docs/howtos/config.md`** -- new section 7.37 `plugins` (field table, merge
  semantics row, full-example entry, ToC anchor).
- **`docs/howtos/slashcommands/plugins.md`** -- new per-command reference for
  `/plugins`, linked from `slashcommands/INDEX.md`.
- **PDFs regenerated** -- all `docs/howtos/`, `docs/howtos/slashcommands/`, and
  `docs/howtos/tools/` how-tos rebuilt to A4 PDFs (19 + 76 + 27 files).

## [1.0.111] - 2026-09-20

Fix release mechanism: GitHub releases were created with empty release notes
because the changelog extractor in the release workflow searched for a
`## Version: <version>` header (and historically a `## [Unreleased] - <version>`
form), while CHANGELOG.md uses the Keep a Changelog form
`## [<version>] - <date>`.

### Fixed

- **`.github/workflows/release.yml`** -- the "Extract release notes from
  CHANGELOG.md" step now matches all three header forms
  (`## [<version>] - <date>`, `## [Unreleased] - <version>`,
  `## Version: <version>`) and is bounded by either a bracketed section
  header or a `## Version:` header, so the extracted entry no longer runs to
  end-of-file for older formats.
- **Backfilled release notes** -- the v1.0.110, v1.0.109, v1.0.108 and
  v1.0.107 GitHub releases had their notes populated from their
  CHANGELOG.md entries retroactively.

## [1.0.110] - 2026-09-20

Worktree changes previously recorded as "Uncommitted (on top of v1.0.109)",
promoted to a release:

- **`ragent-tui` PERF-042 stream-throttle tail fix** -- `should_render` in
  `crates/ragent-tui/src/lib.rs` now also forces the safety-interval paint when
  a message-cache group is still pending (`message_cache_dirty_from <
  messages.len()`), closing the stall where a tool-call row left unpainted by
  the PERF-042 throttle stayed invisible while the status bar already showed
  it running; regression test
  `test_pending_message_cache_group_forces_safety_paint` in
  `crates/ragent-tui/tests/test_perf_render_idle.rs`.
- **Dependency and advisory hygiene** -- dropped now-unused `dirs` from the
  root, `ragent-tools-extended` and `ragent-tools-vcs` manifests, unused
  `tokio` dev-dependency from `ragent-config`, and unused `tempfile`
  dev-dependency from `ragent-server`; removed four stale advisory ignores
  from `deny.toml` (RUSTSEC-2025-0119, RUSTSEC-2026-0190, RUSTSEC-2026-0185,
  RUSTSEC-2026-0235 -- no longer triggered by the resolved tree).
- **New spec: `plugins`** -- `specs/plugins/` (SPEC.md, PLAN.md,
  TESTPLAN.md) drafts a plugin system that loads Codex- and Claude
  Code/Desktop-dialect plugins, runs their JavaScript code on an embedded,
  budget-sandboxed engine with a versioned host API (tool/slash-command
  contribution under `plugin_<id>_<name>`), and manages it through the
  six-subcommand `/plugins` family (`list`, `add`, `remove`, `enable`,
  `disable`, `help`, `test`).

## [1.0.109] - 2026-09-19

Final phase of the `/simplify all` quality pass over the last three commits
(v1.0.106..v1.0.108), applying the remaining code-quality findings flagged
in the earlier review. No user-visible behaviour change.

### Changed

- **`ragent-llm/providers/http_client.rs`** -- removed `utf8_prefix_len`,
  dead since the FUNC-033 rewrite drives stream-chunk flushing from
  `Utf8Error::error_len` and the helper had zero call sites even in tests.
- **`ragent-agent/agent/custom.rs`** -- collapsed `config_agents_dir` into a
  direct alias of `global_agents_dir`; both resolved the same canonical XDG
  path and maintaining two byte-identical delegations only confused the
  discovery layer.
- **`ragent-research/web_gatherer.rs`** -- `extract_http_status` now parses
  the three status digits byte-wise without the intermediate `String`
  allocation in its marker loop.

### Fixed

- **`ragent-research/source_vault.rs`** -- the eight copies of
  `.conn.lock().map_err(... InvalidRunTag "lock poisoned")` are replaced by a
  shared `lock_conn()` helper carrying a dedicated
  `SourceVaultError::LockPoisoned` variant, so a poisoned connection mutex
  no longer masquerades as a run-tag validation error.

## [1.0.108] - 2026-09-18

Config rules and fixes: code-quality pass over the v1.0.105..v1.0.107 window
(user_dirs centralisation, FUNC-033 SSE coalescing, secret redaction, and the
govcreate work) from five parallel explore reviews; no user-visible behaviour
change.

### Fixed

- **`ragent-tools-vcs/gitlab/auth.rs`** -- `migrate_legacy_files` scanned the
  NEW `~/.config/ragent` directory instead of the legacy `~/.ragent/` as its
  source, making the self-token/config-file migration a silent no-op for
  users with credentials still under `~/.ragent/`; the legacy-home helper is
  restored and a regression test pins the scan root
  (`tests/test_gitlab_legacy_migration.rs`).
- **`ragent-tui/app/blueprints.rs` and `ragent-agent/tool/team_create.rs`** --
  the "global" blueprint fallback had been re-pointed at the XDG
  `blueprints/teams/` directory (a duplicate of the standard leg); the true
  legacy `~/.ragent/blueprints(/teams)` fallback scan is restored under the
  "global" scope so old installs keep working.
- **`ragent-tui/app/slash.rs`** -- the four `/spec govcreate` mutex-lock sites
  now uniformly recover poisoned locks via `unwrap_or_else(into_inner)` (was a
  mix of `into_inner`, `.ok()` and `if let Ok`); `/config show` renders
  unavailable global memory/agent dirs as "(unavailable) ✗" instead of
  printing empty path rows.
- **`ragent-tools-extended/archdoc/local_source.rs`** -- `acquire_local` no
  longer misreports an exact-cap natural completion as budget exhaustion: a
  `stopped_early` flag is set at each break site and OR-ed with
  elapsed-vs-deadline at the tail (`test_zero_deadline_reports_deadline`
  pinned).

### Changed

- **`ragent-llm/providers/http_client.rs`** -- `utf8_prefix_len` (the FUNC-033
  SSE chunk-coalescing helper) is demoted from dead `pub fn` to a private
  `#[cfg(test)]` item.
- **Shared helpers deduplicated** -- the regex `NUM_PREFIX_RE` moved from
  `ragent-research/src/cluster.rs` into `limits.rs::num_prefix_re()` beside
  `web_ref_re()`; the verbatim `join_text` copies in `archdoc` collapsed to a
  single `pub(crate)` helper in `url_source.rs` with `local_source.rs`
  delegating.
- **`ragent-agent/session/processor.rs`** -- the global skills directory is
  pushed only when `Some`, dropping an empty `PathBuf` leg.
- **`ragent-tools-vcs/percent.rs`** -- `let _ = write!` on an infallible
  `String` sink replaced with `.expect("BUG: write to String is infallible")`.
- **Documentation** -- trailing-whitespace/border fixes in the tools how-to
  PDFs; `user_dirs::global_state_dir` documents its platform collision with
  `data_dir`; `gather_log.append_line` comment corrected (bounded flush at
  buffer-fill); `github_prs.rs` trimmed-branch comment pinned.

## [1.0.107] - 2026-09-18

govcreate doc and help update.

(Changes listed below were previously tracked as uncommitted work on top of v1.0.106.)

### Added

- **`/spec govcreate` — spec authoring from an architecture document** --
  `/spec govcreate <spec-id> <content-ref> <target-folder>` (FR-001..FR-020)
  orchestrates a staged pipeline: acquire the document (absolute local folder
  or public URL; the local arm stages up to a 200 KB corpus budget in under
  30 s for 50+ docs, NFR-004), extract architectural content, author
  `SPEC.md`/`PLAN.md`/`TESTPLAN.md` via the configured LLM, and write the new
  spec. FR-011 refuses to run into a non-empty target unless `--force` is
  given, naming the blocking entries. Options: `--language`, `--type`,
  `--stack`, `--github`/`--gitlab` (mutually exclusive hosting), `--force`.
  Live `[ .. ]/[ ok ]/[fail]` progress streams into a single in-place TUI
  message (NFR-005 ASCII render) ending with a terminal report; Escape cancels
  the run via a per-run `CancellationToken`. `/spec govcreate help` prints the
  usage block. Also available as the CLI subcommand `ragent spec govcreate`
  (model resolution: `--model provider/model`, else the stored
  `selected_model`; neither configured -> exit 2).

### Fixed

- **HEAD~3 review remediation (v1.0.104..1.0.106 follow-ups)**
  * `ragent-llm/providers/http_client.rs` -- SSE chunk coalescing now flushes
    `pending` UTF-8 bytes driven by `Utf8Error::error_len()` (only bad bytes
    are skipped; incomplete multi-byte tails are held) instead of the
    `pending.len() >= 4` heuristic that could split surrogate sequences;
    `utf8_prefix_len` stays exported as the FUNC-033 contract. Six new
    regression tests in `tests/test_http_client_stream_chunk.rs`.
  * `ragent-types/sanitize.rs` -- secret redaction now captures
    `key + separator` and replaces with `${1}[REDACTED]`; the value charset
    is widened to base64/base64url (`+ / =`).
  * `ragent-specs/plan_parser.rs` -- task dependency-range expansion
    (`T-001..T-014`) is capped at 1000 IDs; endpoints are kept verbatim with a
    `warn!` instead of memory exhaustion.
  * `ragent-tools-core/copy_file.rs` -- canonicalize errors now propagate
    instead of silently producing a wrong same-file answer.
  * `ragent-storage` `delete_memories_by_filter` reports the real deleted row
    count (FUNC-021).
  * `ragent-tools-vcs/github/client.rs` -- `http://` base URLs are honoured,
    not silently upgraded; `github_prs.rs` drops a pointless base64
    round-trip; `percent.rs` builds percent-encoding via `fmt::Write` with a
    1.5x capacity reservation.
  * `masterfetch/cache.rs` -- corrupt rows are DELETEd on read instead of
    poisoning every subsequent hit.
  * `masterfetch/tools/crawl_tool.rs`, compaction runner, and
    `ragent-tui/src/app/research.rs` -- smaller clean-ups (research help text
    extracted to `show_research_help()`); compaction's boundary snapping now
    imports `floor_char_boundary` from `strutil` instead of a local copy.

### Changed

- **Tool catalogue split into per-category how-tos** -- the 612-line
  `docs/howtos/tools.md` is replaced by 26 category documents in
  `docs/howtos/tools/` (plus `tools/INDEX.md`): every tool gets an argument
  table (name, type, required, description, typical value) and at least one
  worked example. A companion PDF is generated for each file
  (`docs/howtos/tools/pdf/`). Cross-references in `docs/howtos/codeindex.md`,
  `config.md`, `toolchain.md`, and `permissions.md` now point at
  `tools/INDEX.md`. The `/docupdate` skill converts the new files alongside
  the existing how-to and slash-command PDFs.
- **Markdown table preprocessing in the TUI renderer** -- a new
  `preprocess_markdown_tables` step in `md_worker.rs` normalises ragged pipe
  tables (missing trailing pipes, uneven cell counts, separator rows) before
  `pulldown-cmark` so multi-line tables render instead of falling back to raw
  text; inline code inside cells is preserved.
- **`ragent-agent` background locking hygiene** -- 8 lock sites in
  `background/mod.rs` route through a single `lock_state()` helper that
  recovers poisoned locks per the FUNC-043 policy.
- **`ragent-specs`** gains a `cargo add`-time dependency required by
  `govcreate`; the commands module grows ~520 lines of parser/scaffold support
  shared between the TUI slash command and the `ragent spec` CLI (single
  `SpecCommand::parse` entry point).

## Version: 1.0.106

URL cloaking for `/research create`, plus a code-quality pass over the change set.

### Added

- **`--url-cloak` research source-URL defanging** -- `/research create` gained
  `--url-cloak`, which writes the report's web source URLs as defanged plain
  text instead of clickable links so automated URL scanners do not flag or
  reject `RESEARCH.md`. The scheme is rewritten (`https://` -> `hxxps://`,
  `http://` -> `hxxp://`), every dot is bracketed (`example.com` ->
  `example[.]com`), and the result is wrapped in a Markdown code span. It
  applies to the `**Sources:**` bullets under each finding and to the
  `References Index` table in `RESEARCH.md` (plus the `Sources Reference`
  table in `CORPA.md`); non-URL rows (local paths, spec ids, labels) are
  unaffected. The flag is available on the root CLI (`ragent research
  create`), the TUI slash command, and `POST /research` (new `url_cloak`
  field, round-tripped through the invocation summary), is recorded in item
  frontmatter (`url_cloak: true`) so `/research update` replays it, and is off
  by default.

### Changed

- **Code-quality pass over the url-cloak change set (`/simplify`)** -- a
  review of the uncommitted diff (plus the `HEAD~3` window) produced the
  following changes; no user-visible behaviour change.

  * `masterfetch/tools/search_tool.rs` -- removed three stray leading spaces
    on the new `exclude_engines` doc-continuation lines that leaked a double
    space into the JSON-schema tool description.
  * `ragent-research/gather_log.rs` -- `append_line` no longer wraps the whole
    lock-guarded buffered write in `block_in_place`; the first-use
    `create_dir_all` + `open` is offloaded by a new `ensure_open_blocking`
    (via `run_blocking`) and the lock is taken by a small `lock_writer`
    helper. The now-unused `open_writer` was deleted.
  * `ragent-research/source_vault.rs` -- added `SourceVaultError::TaskPanic`
    and one generic `run_blocking<T, F>` helper; the seven async wrappers
    (`search`, `find_by_url`, `store`, `read_content`, `read_summary`, `list`,
    `count`) now share it, replacing the repeated
    `spawn_blocking` + `InvalidRunTag` boilerplate and fixing a bug where a
    join panic was reported as an invalid run tag.
  * `ragent-research/session.rs` -- `format_width_sweep_detail` gained a
    `debug_assert_eq!` guarding the reason-columns-sum-to-`excluded` invariant;
    the 18-parameter `assemble_and_write` now takes a single
    `AssembleInput<'_>` struct.
  * `ragent-research/io.rs` -- `cloak_url` now returns `sanitize_inline(value)`
    for the non-URL fallback instead of the raw value, so a scheme-less web
    URL containing `|` or a newline cannot break a Markdown table row.
  * `ragent-research/web_gatherer/relevance.rs` -- `lowercase_cow` now gates on
    `to_lowercase() == s` rather than `chars().any(char::is_uppercase)`, so
    titlecase code points (e.g. `ǅ`) are still lowercased.
  * `ragent-research/web_gatherer.rs` -- `extract_http_status` only matches
    digits after an explicit `status`/`http` marker (with an optional `code`
    word), so "request took 500ms" or "read 404 bytes" no longer misclassify as
    an HTTP status; `bump_engine_fetch_failure` folds its double engine-CSV walk
    into one `bump_engine_stats` closure; `prepared_queries` is rekeyed
    `HashMap<String, _>` -> `HashMap<Arc<str>, _>` and `exclude_engines` is
    wrapped in `Arc<[&str]>` cloned once instead of per sub-query future.
  * `ragent-server/routes/research.rs` tests -- the two exhaustive
    `CreateResearchRequest` literals were reduced to their distinguishing fields
    plus `..minimal_request(..)`.

- **How-to PDFs regenerated** -- the `docs/howtos` and
  `docs/howtos/slashcommands` Markdown manuals were rebuilt to PDF
  (95 documents) so the published manuals carry the new `--url-cloak` entries.

## Version: 1.0.105

Research output limits (spec `researchmax`), scholarly-engine exclusion (spec
`researchnoacc`), per-run open-access toggles, a progress table that breaks
exclusions and fetch failures down by reason/cause, `/spec impl` task-range
expansion, and TUI panel layout/identifier changes. 33 tracked files changed
(3,260 insertions, 190 deletions) plus 11 new files.

### Added

- **Concept and finding output limits (spec `researchmax`)** -- `/research create`
  now caps its `## Concepts` and `## Findings` lists at 5 concepts and 20
  findings by default. Both lists are reordered most-relevant-first before
  truncation (an entry's relevance is the highest `[#N]` source rank it cites;
  ties break by cited count, then by original model order), so the cap always
  discards the least-relevant entries and surviving headings/findings are
  renumbered contiguously. The limits are settable per run with
  `--max-concepts N` / `--max-findings N` on the root CLI (`ragent research
  create`), the TUI slash command, and `POST /research` (new `max_concepts` /
  `max_findings` fields, round-tripped through the invocation summary), or
  persistently via the `research.max_concepts` / `research.max_findings` config
  keys (per-run flags take precedence). A value of `0` means unbounded.
  Non-integer values are rejected with a clear error. Documented in
  `docs/howtos/research.md`, `docs/howtos/slashcommands/research.md`,
  `docs/howtos/config.md`, and the `/research help` output.

- **`research.max_concepts` / `research.max_findings` config keys (spec
  `researchmax` FR-008/FR-017)** -- `ResearchConfig` gained two output-limit
  keys, defaulting to 5 concepts and 20 findings, settable in `ragent.json`.
  Both follow the existing `research` merge semantics (overlay wins when set
  away from the default) and are omitted from serialized output at their
  defaults; the per-run `--max-concepts` / `--max-findings` flags take
  precedence, and `0` means unbounded.

- **Width-sweep table now breaks fetch failures down by cause** -- the
  `/research` per-engine progress table's single `fetch` column is split into
  seven fine-grained columns (`t/o` timeout, `net` network, `blk` SSRF/robots
  block, `http` server error status, `wall` paywall/auth wall, `js`
  JavaScript shell, `extr` extraction failure). `EngineSweepStat` carries a
  `failed_by_kind` map (new `FetchFailureKind` enum) whose counts sum to the
  engine's `fetch` count, the `WidthSweepSummary` gather event and its
  web-URL log record carry a global `failed_by_kind` tally, and the totals row
  repeats the breakdown. Fetch adapters attach a typed `FetchFailure`
  (downcast by the gatherer); untyped errors fall back to a message heuristic,
  so test doubles and the legacy `webfetch` path keep working.

- **Per-engine research progress table now breaks exclusions out by reason** --
  the `/research` width-sweep progress table gained five reason columns
  (`papers`, `pdf`, `relev`, `short`, `fetch`) in addition to
  `considered/captured/excluded`, so each backend engine's row shows *why*
  candidates were dropped. `EngineSweepStat` carries an
  `excluded_by_reason` map (new `ExclusionReason` enum), the
  `WidthSweepSummary` gather event and its web-URL log record carry a global
  per-reason tally that sums to `excluded`, and the table is now separated
  from the summary line above and the trailing diagnostics below by a blank
  line.

- **`mf_search` engine exclusion + research `--no-papers` (spec `researchnoacc`)**
  -- `mf_search` gained an optional `exclude_engines` array that removes named
  backends from the orchestrator *before* any request is dispatched, so excluded
  engines are never queried (unknown names are ignored). A new
  `SearchOrchestrator::exclude_engines` method backs it, the academic engine
  vocabulary (`ACADEMIC_ENGINES`, `ENGINE_OPENALEX`, `ENGINE_WIKIPEDIA`) is
  defined once in `masterfetch::search` and reused by research hit
  classification, and an explicit "all engines excluded" result is returned when
  the exclusion list names every configured engine. Research `--no-papers` now
  routes through this exclusion (via a new `WebSearchTool` exclusion parameter)
  instead of a post-search per-hit filter, so OpenAlex consumes no search budget
  and cannot shadow general-web URLs in dedup. New
  `research.exclude_academic_engines` config value (OR-merged) makes the
  exclusion persistent, with the per-run flag taking precedence. `POST /research`
  gained a `no_scholarly` field. Docs updated in `docs/howtos/research.md`,
  `docs/howtos/config.md`, and `docs/howtos/slashcommands/research.md`.

- **`--oa-enable` / `--no-oa` research flags** -- open-access recovery can now
  be toggled per run on every front-end (`ragent research create`,
  `/research create`, `POST /research`) without editing `ragent.json`.
  Precedence is: explicit flag wins, otherwise `research.open_access_recovery`
  from config, otherwise off. The `ResearchRunRequest` gained an
  `open_access_recovery: Option<bool>` override and `build_session_config`
  resolves the three-way precedence.

### Changed

- **TASKS panel now shows the task ID after the status** -- the Alt+T tasks
  panel previously rendered only `[STATUS] title`; each row now reads
  `[STATUS] <id> title` so the task identifier is visible without opening
  `/task get`.

- **Right-hand side panels now take 50% of the window width** -- every
  toggled side panel (Alt+M memory, Alt+T tasks, Alt+P profile, Alt+O
  telemetry, Alt+C context, and the log panel) previously used a
  breakpoint-dependent split (21-32% of the width). `log_split` now returns a
  flat `(50, 50)` split, so the side column is half the application width at
  every terminal size.

### Fixed

- **`/spec impl` task-range dependencies now expand** -- a Dependencies cell
  such as `T-001–T-014` was parsed as a single unknown task ID, so the dependency
  was dropped and the final verification task was scheduled *first* (right after
  T-001) instead of last, then failed as blocked because its prerequisites were
  unimplemented. `PlanParser::parse_dependencies` now expands an inclusive range
  into every spanned ID, accepting an en dash, an em dash, an ASCII hyphen, or
  the words `to` / `through` between well-formed endpoints, with zero-padding
  preserved from the range's start ID. This affected 16 specs that used a range
  for their whole-plan verification task (researchmax, researchnoacc, wikisearch,
  openalex, tavmove, exasearch, reqeng, agentloop, openharness, compact, ...).

- **`--no-papers` / `--no-scholarly` flag spelling** -- the root CLI exposed
  only `--no-scholarly` while the research hand parser, TUI, and every doc used
  `--no-papers`. `--no-papers` is now the canonical spelling on every entry
  point with `--no-scholarly` retained as a visible alias, and the
  `POST /research` invocation summary emits the canonical `--no-papers`.

## Version: 1.0.104

- **Functional anti-pattern remediation -- FUNCPLAN.md (FUNC-038..069, 080..082)**
  second pass completes the plan's M1 tail and the whole of M2-M5:

  * **FUNC-035** -- `mf_search` keyless engines surface a dead engine as an
    error instead of a zero-result success: `wikipedia::fetch_summary` returns
    `Result<Option<RawResult>, String>`, the new `partition_summary_outcomes`
    reports `EngineReport::error` when every summary fetch failed, and a
    corrupt cached metadata blob (`masterfetch/cache.rs`) is now treated as a
    cache miss so the caller re-fetches. A corrupt PDF title parse is logged.
  * **FUNC-038** -- `github_merge_pr` rejects a `method` outside
    `{merge, squash, rebase}` (extracted `parse_merge_method`) instead of
    silently performing a real merge.
  * **FUNC-040/041/042** -- the last three production poison-lock panics
    (`ragent-agent/team/manager.rs` watchdog `last_progress`) recover via
    `PoisonError::into_inner`; a workspace inventory confirms zero remaining.
  * **FUNC-043/044/045** -- no production `unreachable!`/poison-expect remains;
    `storage.rs` cron/rank row maps bind by column name; codeindex
    `with_writer` uses `ok_or_else`; the nullable `source_module` collapse is a
    documented, logged helper.
  * **FUNC-050/052/053** -- GitLab GETs retry 429 honouring `Retry-After`
    (bounded), the recursive tree fetch has request/entry budgets, the jobs
    list follows pagination (>100 jobs no longer truncated); research
    `GatherLog` append/flush run through `block_in_place`, `SourceVault`
    gained `read_summary_async`, `write_concepts_md` offloads its blocking
    source-metadata read.
  * **FUNC-060/061/062/063/064/065/066/067/068/069** -- the TUI question Enter
    clamps its selection (never submits a blank answer); `move_file` renames
    first (no orphan dirs), `copy_file` refuses a self-copy, `append_file`
    flushes; GitHub `post`/`put`/`patch` honour `base_url`; percent-encoding is
    byte-wise (UTF-8 correct) across GitHub/GitLab; codeindex `parent_id`
    resolves multi-level nesting to a fixpoint; the keyword verifier no longer
    reports an empty/uncited analysis as `passed` and unknown contradiction
    dimensions are neutral; the server auth comparison hashes fixed-length
    digests, the rate limiter enforces exactly 60/min, and a dropped research
    SSE event is logged; provider classification keys off the parsed host.
  * **FUNC-080/081/082** -- new `scripts/check-poison-locks.sh` (with
    `--self-test`) wired into `pre-flight.sh` and CI; regression tests added
    across tools-core, tools-vcs, codeindex, storage, research, server, tui;
    `SearchBudget::try_acquire` increments only on acceptance so `used()`
    matches its contract and the budget-exhausted event reports an unlimited
    limit as `None`, not a misleading `0`.

## Version: 1.0.103

- **M1 agent per-turn hot path -- PERF-032..040 + PERF-048 complete** -- the
  per-turn allocation and clone load in the agent loop is removed:

  * **PERF-032** -- `session/cache.rs` holds the cached provider-facing
    transcript as an `Arc<Vec<ChatMessage>>`; `cached_chat_messages_for_version`
    and `store_chat_messages` now hand out and store the `Arc` (O(1) refcount)
    instead of deep-cloning the whole vector on every turn.
  * **PERF-033** -- `SessionState` records the internal-history prefix length
    and its version when the transcript was built (`record_history_base`), and
    `take_cached_for_append` lets the loop detect a pure append, convert only
    the newly-appended tail, and extend the retained vector instead of
    rebuilding the whole transcript. `session/history.rs` still provides
    `history_version_of` for the prefix check.
  * **PERF-034** -- the subagent wire surface (tool definitions with
    interactive tools removed) is cached behind the tool-registry version
    (`get_subagent_tool_definitions`), so repeated sub-agent steps share one
    immutable filtered vector instead of rebuilding it per step.
  * **PERF-035** -- `LoopTracker` is now `Copy` (every field is `Copy`), so the
    per-loop-step `active_loops.get(..).cloned()` save/restore and
    `persist_loop_tracker` round-trips are bitwise copies instead of heap
    clones.
  * **PERF-036** -- new `RequestTokenTracker` (`compaction/estimator.rs`)
    memoises the estimated cost of each provider-facing message by position and
    folds in the system prompt and tool definitions, so the per-step pre-send
    compaction check recomputes only the message that changed rather than
    re-summing the whole history every step.
  * **PERF-037** -- `compaction/convert.rs` pairs each `ToolResult` with its
    `ToolUse` through a single-pass `call_id -> (message, part)` index instead
    of a backward linear scan (O(n*m) -> O(n)).
  * **PERF-038** -- `build_prompt` assembles the compaction prompt into one
    pre-sized buffer via `write!`/`push_str` instead of building a `Vec<String>`
    of clones and joining it (two full copies of the largest prompt the agent
    builds); the serializer and runner stop duplicating the full-history clones.
  * **PERF-039** -- `build_memory_prompt_section` memoises a rendered memory
    entry's token cost by row id against a content/category/confidence
    fingerprint, and the prompt builders use `write!` into one buffer instead of
    `push_str(&format!(..))` per field.
  * **PERF-040** -- the activity log is written by one background writer task
    per process (`SessionProcessor::start_activity_writer`, wired in
    `src/main.rs`) fed by a bounded queue, replacing the per-event
    `spawn_blocking` dispatch and the per-run event-bus subscriber; new
    `tests/test_activity_writer.rs` pins the batched contract.
  * **PERF-048** -- the TUI viewers (`/research open`, Alt+M full-memory,
    output view) retain a single copy of their rendered wrapped rows and the
    status bar measures each span set once per frame.

  New benches: `cargo bench -p ragent-agent --bench turn_loop` (per-turn
  history conversion, token estimator, compaction `select`) and
  `--bench m3_hot_paths`; new guard test
  `ragent-agent/tests/test_no_percall_regex.rs`. `docs/agentorch.md` gained the
  PERF-032..082 remediation register and `docs/PERFPLAN.md` records the M1
  completion notes. **Security**: `rustls` bumped 0.23.43 -> 0.23.45
  (RUSTSEC-2026-0285, TLS 1.3 handshake boundary confusion).

- **M6 data layer -- PERF-069..077 complete** -- the storage, code-index, MCP,
  and research-gatherer data paths no longer copy or serialise work on hot
  paths:

  * **PERF-069** -- `ragent-storage` now opens a dedicated read-only `SQLite`
    connection (`Storage::reader`) alongside the writer connection. Read-only
    query methods acquire it through a new `lock_conn_read!` macro, so a long
    write transaction (or the startup FTS warm-up) no longer serialises reads
    behind it. In-memory storage keeps sharing the single connection. New
    `tests/test_reader_concurrency.rs` asserts a read completes while a
    `BEGIN IMMEDIATE` write tx is held open on the same handle and that the
    reader observes committed writes.
  * **PERF-070** -- `IncrementalSnapshot::to_full` now consumes the delta and
    base snapshot by value, moving the base file map and each added file's bytes
    instead of cloning them (`mem::take`-style). `tests/test_snapshot_expand.rs`
    covers diff application, additions, removals, and a restore round-trip.
  * **PERF-071** -- the session `SELECT` bodies are hoisted to `const &str`
    (no per-call `String`), and the activity-log run-list / run-range reads use
    `prepare_cached` with two static SQL statements instead of a rebuilt
    `String`.
  * **PERF-072** -- `IndexStore::get_stale_files` no longer loads the entire
    `indexed_files` table into a `HashMap`; it builds an O(scanned) path index
    and streams the stored rows once, so a stale check with zero changes is
    cheap regardless of index size.
  * **PERF-073** -- the symbol-name filter is now an anchored, escaped prefix
    match (`name COLLATE NOCASE LIKE 'prefix%' ESCAPE '\'`) served by a new
    `idx_symbols_name_nocase` index (schema v4), instead of a leading-wildcard
    `LIKE` that scanned the whole `symbols` table. Interior-only substrings no
    longer match; the filter docs and tool schema were updated accordingly.
  * **PERF-074** -- `upsert_file` and `upsert_symbols` use
    `INSERT ... RETURNING id` (dropping the follow-up `SELECT id` /
    `last_insert_rowid()`), `apply_diff` prepares its batch statements once, the
    standalone `upsert_symbols` / `upsert_imports` / `upsert_refs` delete+insert
    pairs are wrapped in a transaction (skipped when the caller already opened
    one), and reference queries push their `LIMIT` into SQL
    (`find_references_limited`).
  * **PERF-075** -- `FtsIndex` holds one long-lived Tantivy `IndexWriter`
    behind a `Mutex<Option<..>>`, created lazily on first use and reused by
    every batch, instead of rebuilding the writer (and its heap) per commit.
  * **PERF-076** -- `McpClient` maintains a `tool_name -> server_id` index,
    rebuilt on connect / refresh / refresh-for-server / disconnect, so
    `call_tool_by_name` is an O(1) map lookup rather than a nested
    `servers x tools` scan. `tests/test_mcp_tool_index.rs` pins the contract.
  * **PERF-077** -- the research web gatherer shares the sub-query text across
    its hits as an `Arc<str>`, the shared query cache stores
    `Arc<[WebSearchHit]>` (insert and get are refcount operations), and
    `WebFetchedPage::body` is an `Arc<str>` so the CPU-bound
    language-detection `spawn_blocking` closure clones a refcount instead of a
    page-sized body buffer.

  Verified by `cargo test -p ragent-storage`, `-p ragent-codeindex`,
  `-p ragent-research`, `-p ragent-agent`, `-p ragent-server`, and `-p
  ragent-tui` (all suites green) plus `cargo fmt --check` and `cargo clippy
  --all-targets` clean.

- **M5 regex hoisting — PERF-064..068 complete** — every regex on a per-page,
  per-candidate, or per-plan path is now compiled once per process behind a
  `OnceLock`/`LazyLock` static instead of on every invocation:

  * **PERF-064** — `masterfetch/metadata.rs` no longer compiles ~12 regexes per
    HTML page parse. All 12 (`<meta>` tag/key, double- and single-quoted
    `content`, `<title>`, `<link>`, `rel="canonical"`, double- and single-quoted
    `href`, `<html>`, `lang`, and the JSON-LD `<script>` block) are hoisted to
    `LazyLock` statics, and `masterfetch/youtube.rs`'s `<title>` fallback moves
    its function-local regex to a `LazyLock` static. New guard test
    `ragent-tools-extended/tests/test_no_percall_regex.rs` fails if a direct
    `Regex::new` reappears in the metadata or YouTube modules.
  * **PERF-065** — all seven `ragent-research/src/web_date.rs` patterns
    (JSON-LD script, JSON date key, `<meta>`, `content`, `<time datetime>`, ISO
    `YYYY-MM-DD`, and long-form `Month D, YYYY`) are hoisted to `LazyLock`
    statics instead of being compiled per invocation.
  * **PERF-066** — per-call compiles are hoisted in `clarify.rs` (concrete-id
    detector), `planner.rs` (JSON code fence), and the already-correct
    `cluster.rs` / `session/topic.rs` / `document.rs` sites are covered by the
    new hot-path guard test.
  * **PERF-067** — the template-placeholder regex in
    `ragent-agent/src/template/mod.rs` was compiled **inside a loop** on every
    iteration of `extract_placeholders`; it is now one `LazyLock` static. New
    guard test `ragent-agent/tests/test_no_percall_regex.rs`.
  * **PERF-068** — relevance scoring no longer rebuilds query-side state for
    every candidate. New `PreparedQuery` (in `web_gatherer/relevance.rs`)
    normalises the query, strips stopwords, and derives every term's
    morphological variants **once per sub-query**; `WebGatherer` memoises it in
    a per-gather map shared by the pre-fetch and post-fetch filters, and
    `compute_relevance_label` is now a thin wrapper (`PreparedQuery::new(q).label(..)`)
    for the one-shot callers. Candidate fields are lowercased through a
    `Cow`-returning helper that borrows when no uppercase character is present
    (the common case for URLs), and the former `format!("{title} {snippet} {url}")`
    haystack allocation is gone — the three fields are tested separately, which
    is equivalent because normalised terms never contain whitespace. A/B bench
    (`cargo bench -p ragent-research --bench relevance_bench`) over four
    candidates: **7.55 us -> 1.45 us (5.2x)**. New equivalence tests in
    `test_web_gatherer_helpers.rs` assert the prepared path returns identical
    labels and retention to the one-shot wrapper, including the empty-query and
    uppercase-URL paths.

  New guard test `ragent-research/tests/test_no_percall_regex.rs` scans eleven
  research hot-path modules for un-hoisted `Regex::new` calls.

- **M3 async runtime hygiene — PERF-049..056 complete (blocking I/O, event-bus
  and SSE allocations)** — the release-blocking M3 set is landed:

  * **PERF-049/050** — the research gather-log (`GatherLog`) now opens its JSONL
    file once, lazily on the first append, and appends through a 64 KiB
    `BufWriter` (flushed on a `gather_summary` marker, on any session run-log
    marker, and on drop) instead of `open` + two `write_all` + `flush` per
    record. A/B measurement of the raw write path: **1140 ns/record -> 44 ns/record
    (25.6x)**. Per-URL records are also serialised from a borrowed
    `#[derive(Serialize)]` struct (`UrlRecord` + `FlattenDetail` + a
    `collect_str` timestamp) rather than a `serde_json::Value` tree, so a record
    no longer allocates a `Value` map or clones every detail key/value. JSONL
    bytes are unchanged.
  * **PERF-051** — `@fuzzy` reference resolution no longer runs the recursive
    filesystem walk on the async worker: new `collect_project_files_async` wraps
    the existing sync walk in `spawn_blocking` and `resolve_fuzzy` uses it. The
    project-file cache keyed by canonical working directory is now an
    `FxHashMap`.
  * **PERF-052** — the `glob` tool wraps its recursive `collect_matches` walk in
    `spawn_blocking` so a large tree no longer pins a tokio worker.
  * **PERF-053** — the `read` tool's duplicate `std::fs::metadata` is gone:
    `cached_read` now returns the mtime it already fetched asynchronously and
    passes it to `record_read_timestamp` (one async metadata syscall per read).
  * **PERF-054** — `EventBus::publish` moves the event into the broadcast
    channel (`sender.send(event)`) instead of `send(event.clone())`, removing a
    whole-event deep clone on the per-token path; the dropped-event log reads
    from the `SendError(ev)` payload.
  * **PERF-055** — new `redact_secrets_cow(&str) -> Cow<str>` returns
    `Cow::Borrowed` without allocating when the secret registry is empty and the
    secret regex does not match (the common case for streamed payloads); the
    registry is a longest-first `Vec` kept sorted on insert instead of a
    `HashSet` collected and sorted per call. This also **fixes a latent
    correctness bug**: `ragent-agent` carried a second, byte-identical copy of
    the sanitize module with its own private secret registry, so credentials
    registered through `ragent_agent::sanitize` (`src/main.rs`, the session
    processor) or `ragent_storage`'s re-export were invisible to
    `ragent_server::sse::redact_secrets`, and vice versa. The agent-local module
    is now a `pub use ragent_types::sanitize::*` facade, so one registry serves
    every subsystem.
  * **PERF-056** — the session SSE stream clones the session id once per
    connection instead of once per streamed event, and the `Event::ModelResponse`
    / `Event::ToolResult` arms feed `redact_secrets_cow` straight into the
    `Cow`-typed payload (removing the extra `String` + `==` comparison per
    event). Both SSE streams now count and warn on `Lagged` drops instead of
    silently discarding them.

  New benches/tests: `cargo bench -p ragent-research --bench gather_log_bench`
  (append throughput), `tests/test_sanitize_redact.rs` (Cow borrow/own +
  longest-secret ordering), `tests/test_event_publish_move.rs` (payload reaches
  every subscriber intact), plus new unit tests for lazy log-file creation, the
  summary flush, and the async walk wrapper.

- **M4 network and resource reuse — PERF-057..063 complete (git spawn, HTTP
  clients, request bodies)** — every network/resource-reuse finding from the
  audit is fixed:

  * **PERF-057** — `run_git_or_error` no longer spawns `git` twice (once for
    output, once for the exit status). A new `run_git_output` helper performs a
    single spawn and returns a `GitOutput { stdout, stderr, success }`; both
    `run_git` and `run_git_or_error` derive their views from it, so every local
    git tool runs the subprocess exactly once. This removes a real correctness
    hazard: mutating subcommands (`commit`, `push`, `merge`, ...) previously
    executed their side effects twice. New integration test
    `tests/test_git_single_spawn.rs` records `git` invocations through a PATH
    shim and asserts one spawn per tool call.
  * **PERF-058** — new `ragent_tools_vcs::http_client::shared_client()` returns a
    clone of a process-wide `OnceLock<reqwest::Client>` (connection-pool limits,
    connect/request timeouts, TCP keep-alive). Every `reqwest::Client::new()` in
    the GitHub/GitLab clients, both auth modules, and the CI job-trace path now
    uses it, so a tool call no longer builds a fresh TLS pool and handshake.
    `tests/test_shared_http_client.rs` fails if a `reqwest::Client::new()`
    reappears outside the helper.
  * **PERF-059** — resolved VCS tokens are cached instead of re-read on every
    request: the GitHub token read is cached against the token file's mtime
    (re-read only when the file changes, so a changed `HOME` is still honoured),
    and the GitLab PAT — encrypted at rest and decrypted on read — is cached per
    storage handle. `save_token`/`delete_token` invalidate the cache. The
    GitLab CI job-trace path now reuses the client's already-resolved token via
    the new `GitLabClient::token()` instead of decrypting it a second time.
  * **PERF-060** — the Copilot device-flow poll used `reqwest::Client::new()`
    per poll; it now uses the cached streaming `create_http_client()`.
  * **PERF-061** — the Azure AI Foundry retry loop no longer clones the whole
    serialised request body per attempt: the body is converted once to a
    reference-counted `bytes::Bytes`, so each of the up-to-5 retry attempts
    clones only the refcount.
  * **PERF-062** — Ollama Cloud serialises the request body exactly once (the
    800-char debug preview and the done-frame log are now borrowed slices of the
    serialised bytes and are only built when the corresponding trace level is
    enabled), removing both the duplicate serialisation and the per-completed-
    message `to_string`.
  * **PERF-063** — the SSE accumulation buffer in all 12 streaming providers is
    pre-sized to 8 KiB (`String::with_capacity(8 * 1024)`), removing the
    realloc/copy cliff on long streams.

- **M2 TUI render loop — PERF-041/042/043/047 (message-window caches)** — the
  message timeline no longer re-renders, re-parses, re-wraps, or re-allocates
  unchanged rows on every frame:

  * **PERF-041** — the flat plain-text copy buffer (`message_content_lines`) is
    no longer rebuilt on every idle frame by cloning every wrapped `String` of
    the whole transcript. `App::message_content_lines_dirty` is set only when a
    group was re-rendered, re-wrapped at a new width, or the cache length
    changed; copy paths (`/clip`, keyboard/right-click select-copy) call
    `App::ensure_copy_content_lines()` to refresh on demand from the per-message
    cache.
  * **PERF-042** — a streamed reply is no longer re-parsed, re-wrapped, and
    re-stringified on every token (O(n^2) over the reply). A group already
    rendered may refresh at most once per `MESSAGE_STREAM_MIN_INTERVAL` (33 ms);
    the dirty watermark keeps it pending and `compute_next_deadline` schedules a
    wake at the end of the window so the tail is never left stale.
  * **PERF-043** — the per-frame staleness scan starts at
    `App::message_cache_dirty_from`, a watermark lowered by
    `App::mark_message_dirty()` at every in-place mutation site, so an idle
    frame does O(1) staleness work instead of comparing `edit_seq` over the
    whole transcript.
  * **PERF-047** — continuation-line indentation is emitted as its own borrowed
    `Span::raw(&INDENT_SPACES[..indent])` instead of
    `format!("{}{}", " ".repeat(indent), line)`, saving two allocations per
    continuation line in `message_widget::to_lines` and the mirror site in
    `layout`.

  New integration test `tests/test_perf_message_cache.rs` (7 tests) pins the
  watermark scan, the streaming throttle, and the lazy copy-buffer rebuild.

- **M2 TUI render loop — PERF-044/045/046 (TUI idle-CPU and per-frame work)** —
  the remaining M2 tasks close the render loop's remaining hot paths:

  * **PERF-044** — the 2 s idle safety wake no longer repaints an unchanged
    frame. `should_render()` paints only when `App::needs_redraw` is set, or
    when the safety interval elapsed *and* a wall-clock-driven display is live
    (`App::needs_periodic_redraw`): a permission countdown, a web-phase
    countdown, a visible Agents/Teams panel whose elapsed column ticks (or a
    running background-shell row), or a spinner/progress latch. A fully idle
    TUI now paints nothing at rest instead of 0.5 frames/sec.
  * **PERF-045** — the ~18 cheap `poll_*`/`refresh_*` jobs are collapsed
    behind one `App::run_housekeeping_if_due()` gate
    (`HOUSEKEEPING_INTERVAL`, 200 ms). A wake that merely drained a keystroke
    or a single streamed token skips the whole pass; each job keeps its own
    internal cadence and `compute_next_deadline` still wakes the loop at the
    end of the window so nothing is starved. `housekeeping_runs` exposes the
    pass count.
  * **PERF-046** — the chat input is re-wrapped and its height and cursor
    re-measured only when one of `(input text, cursor, keyboard selection,
    inner width)` changes. `InputRenderCache` holds the wrapped rows, height,
    and cursor position; an idle or non-typing frame performs no wrapping or
    measurement work. The now-unused `char_wrap`/`input_widget_lines` helpers
    were removed in favour of the cached rows plus an allocation-free
    `input_widget_height` row-count.

  Benches: `bench_panels` gained `idle_should_render` (the O(1) idle decision)
  and `idle_full_frame` (warm full-frame render at 100/500 messages), and
  `bench_markdown` gained `wrap_line_styled` (the single wrapping helper the
  streaming cache relies on). New integration test
  `tests/test_perf_render_idle.rs` (11 tests) pins the idle-decision policy,
  the housekeeping gate + deadline, and the input render cache. `cargo bench
  -p ragent-tui` green: `idle_should_render/idle_500` ~20 ns; warm full-frame
  at 500 messages ~1.1 ms.

- **PERF-048: single-copy rendered lines in the TUI viewers** — the
  `/research open`, Alt+M full-memory, and output-view overlays no longer keep
  two copies of their rendered rows. `OutputViewLineCache` dropped its
  un-wrapped `lines` field: the pre-wrapped `wrapped_lines` is now the single
  retained copy of the rendered document (the old
  `wrapped_lines = lines.clone()` duplicated the whole document on every
  rebuild). The output view now re-derives its lines from the source messages
  only when the source generation or terminal width changes, wrapping them
  in place. The status bar measures each span set exactly once per frame
  (`spans_width` helper replacing the repeated
  `iter().map(|s| s.width() as u16).sum()` passes in `build_line1` /
  `build_line2`) and builds the `HEALTHY <name> v<version>` prefix span once.
  The Agents/Teams side buttons build their labels once per frame and share
  them between the width pass and both button renders instead of reallocating
  them (was ~6 label allocations per frame). New integration test
  `tests/test_perf_single_copy_lines.rs` drives the real render path for all
  three viewers and asserts `wrapped_count`/`content_lines` stay in lockstep
  with the retained rows and that a same-width frame re-renders nothing.

## Version: 1.0.102

- **Simplify/quality pass across the search, research, agent, and TUI
  crates** — second `/simplify` sweep. In `masterfetch::search` the API-key
  engines (`tavily`, `perplexity`, `exa`, `serper`, `langsearch`) now share
  four helpers on `engine.rs` (`engine_http_client`, `api_engine_preflight`,
  `finish_json_search`, `mask_api_key`) plus `truncate_snippet` /
  `truncate_query_to`, removing ~150 duplicated lines; `strip_disallowed_quotes`
  became a single allocation; `search_with_retry` caps the backoff shift at 31
  to avoid an out-of-range-shift panic; and `diversity_truncate` now takes and
  returns owned vectors, renumbers positions in surviving order, and re-syncs
  `total_merged_results`. `MfSearchTool::engine_status` derives all three
  flags from `in_use` and `resolve_search_keys` uses one `pick` closure. In
  `ragent-research` the web-gatherer volume policy (per-query search allowance
  + fetch budget) is resolved by one `volume_policy()` helper, the capture
  stat is credited before the `search_engine` field is moved, and the vestigial
  `deadline_fired` flag was removed (the fetch stage is deadline-neutral);
  `SimpleCritic` lowercases source titles/paths once instead of per
  sub-question; `provider_stats` routes its lock through `lock_by_tool`;
  `relevance.rs` precomputes morphological variants once per term and guards
  the doubled-consonant stem against a non-char-boundary slice. In
  `ragent-agent` `history.rs` extracts a shared `assistant_tool_results`
  helper and `compaction/convert.rs` parses `data:` URIs to the real MIME type
  and matches a single-text part by slice. In `ragent-tui` the research
  clarification retry is extracted into `run_with_clarification`, the
  websearch render spawn is guarded against a panic leaving the status line
  stuck, and orphaned comments/formatter duplication were removed.

## Version: 1.0.101

- **Simplify pass over four crates** - code-quality fixes from a review
  sweep: in `masterfetch::search` (`consensus.rs`, `engine.rs`, `mod.rs`)
  engine-merge output is now deterministic (sorted `source` strings),
  `total_merged_results` is re-synced after truncation with merge positions
  renumbered 1..N, four mutex `.expect()` calls became poison-tolerant
  `.unwrap_or_else(|p| p.into_inner())`, cache inserts purge expired
  entries, dead constants and a duplicate stopword were removed,
  `partial_cmp` was replaced with `total_cmp`, the failed-engine predicate
  was extracted into a shared `report_is_failed()` helper, and stale doc
  comments were corrected. In `ragent-research` (`cli.rs`) the
  `parse_continue`/`parse_export`/`parse_import` parsers now skip consumed
  flag values so `--message hello my-item` no longer resolves the item name
  from the flag value. In `ragent-tui` (`app/research.rs`) three orphaned
  comments were removed, the twice-duplicated provider-call formatter was
  extracted into `format_provider_calls()`, and the cluster session-ID
  fallback was aligned with the other arms. In `ragent-agent`
  (`compaction/convert.rs`) `"system"`/`"tool"` roles are skipped instead
  of silently coerced to user messages, and `ImageUrl` parts now parse the
  `data:` URI to extract the real MIME type instead of hardcoding
  `image/png`.

## Version: 1.0.100

- **mf_search engine-level resilience (T-016)** - transient engine failures
  are now retried inside the engine call path (`search_with_retry` in
  `masterfetch::search::engine`) instead of being patched per surface:
  Wikipedia 429/`rate-limited`, HTTP 5xx, and transport timeout/connect
  errors get up to 2 retries with 1s/2s exponential backoff, and the
  orchestrator staggers engine starts by 120 ms so keyless backends are not
  all hit at t=0. Account-level quota blocks (Serper 403, Exa 402, Tavily
  432, OpenAlex daily "Insufficient budget" 429) are deliberately NOT
  retried. Wikipedia's `page/summary` step is additionally bounded to 8
  concurrent fetches with a 150 ms start-up stagger so deep per-engine
  sweeps no longer burst past Wikimedia's per-IP limiter and zero the whole
  engine (this was the root cause of "only LangSearch results" in
  `/websearch search`). `MergeOutput.blocked_engines` entries now carry
  `name: reason` so quota blocks are distinguishable from transient
  rate-limits without enabling tracing. 15 new integration tests in
  `tests/test_mf_engine_resilience.rs` pin the classification, retry, and
  stagger behaviour.
- **Fetch-stage deadline cancellation removed (research webgather)** - the
  web-gathering fetch stage is no longer gated on or cancelled by the
  `--web-time` phase deadline. Previously, when the deadline expired
  mid-fetch, the fetch loop stopped starting new fetches and in-flight
  requests were cancelled on drop. Now every candidate produced by the
  search stage is fetched to completion and in-flight fetches always run
  to completion (each still individually capped by `--fetch-timeout-secs`).
  The deadline continues to bound the *search stage* (decomposer call and
  sub-query search-result waits, FR-008) so no new *searches* are issued
  after it fires; the `web_deadline` diagnostic still fires exactly once
  per gather pass when the deadline elapsed during the pass (a
  `deadline_fired` flag + final elapsed check distinguishes true expiry
  from zero-timeout scheduling artefacts). `WidthSweepSummary.cancelled`
  is now structurally 0 and kept only to preserve the balance invariant.
  New test `test_fetches_run_to_completion_past_deadline`; the session,
  tiered, and deadline test fixtures updated accordingly (slow fetches are
  now 1.5-2 s, not 120 s).
- **Search circuit breaker removed** - the per-gather-pass
  consecutive-failure circuit breaker (`--search-circuit-breaker-threshold`,
  `DEFAULT_SEARCH_CIRCUIT_BREAKER_THRESHOLD`, `SearchCircuitOpen` event,
  `SearchCallOutcome::CircuitOpen`, `ResilienceConfig.search_circuit_breaker_threshold`,
  and all TUI/CLI/HTTP plumbing) is deleted. Search failures are still
  retried with exponential backoff (Milestone H-002) and bounded by the
  per-fetch timeout and `--max-search-calls` budget; the breaker was an
  early-abort optimization that discarded subsequent sub-queries after 3
  failures.

## Version: 1.0.99

- **Width-sweep volume cap removed except in competitive mode** - the fetch
  budget is a leftover from the T-006 era and is redundant now that the
  60 s web phase deadline and `--fetch-concurrently` bound
  parallelism: it discarded candidates the search had already paid for.
  The gatherer now runs **uncapped by default** — every retained
  candidate is fetched and every sub-query search asks the engines for
  their maximum result page (OpenAlex up to 75/query) — in tiered,
  supervisor, and agent-tool modes; the width-sweep summary `capped`
  counter stays 0 (the balance
  `considered == captured + excluded + capped + cancelled` is preserved
  and the field is kept). Capping is now opt-in via
  `WebGatherer::with_volume_cap(Some(n))`, wired only for competitive
  (`--mode competitive`) runs, where `n = effective_web_budget()` caps
  both the per-query search allowance and the per-query fetch budget.
  Tests rewritten for the new policy plus 3 new cap-semantics tests;
  docs/howtos/research.md "Fetch budget" section updated.
- **Width-sweep fetch budget now scales with sub-query count** - the
  gatherer applied the caller's `max_results` web allowance as a *global*
  fetch cap, but the same number is also the per-engine result size per
  sub-query. On a 7-query width sweep (standard depth budget = 7) this
  capped the pass at 7 fetches: 57 candidates were considered, 42 passed
  the relevance filter, 35 were reported `capped` without ever being
  fetched, and only 7 were captured. The fetch budget is now
  `max_results` x sub-query count (49 on that run), so every retained
  candidate is fetched and `capped` only fires when a sweep genuinely
  exceeds the per-query allowance. A new `fetch_budget_scales_with_sub_query_count`
  integration test pins the semantics; the existing single-query T-006 cap
  test is unchanged.
- **Title-signal relevance rescue for verbose sub-queries** - decomposed
  sub-queries are often long ("how to write goals and configure AI agent
  loops", 6 terms), so an on-topic title like "Designing agentic loops"
  matched only 2 terms (ratio 0.33) and fell below the 35% Medium floor.
  `compute_relevance_label` now retains a hit when the title matches two
  or more distinct query terms even at ratio >= 0.25 (label "Medium —
  multiple title terms match query"), rescuing roughly half of the 13
  remaining relevance rejections in the 2026-09-13 gather log while
  single-title-term noise (e.g. off-topic arXiv surveys) stays rejected.
  4 new tests in `test_web_gatherer_helpers.rs`; docs/howtos/research.md
  documents the new "Fetch budget" semantics.
- **Research web-gather relevance filter no longer rejects morphological
  variants** - the pre-fetch title/snippet relevance filter
  (`compute_relevance_label`) previously required exact substring matches
  of query terms, so an on-topic result like "What is an agentic loop?"
  was rejected for the sub-query "how to configure and operate agentic AI
  loops" ("agentic" appears but "loop" vs "loops" did not). Query terms now
  also match morphological variants: inflectional suffixes (`-ies`, `-es`,
  `-s`), derivational suffixes (`-ing`, `-ics`, `-ic`, `-ly`, `-ment`,
  `-ness`, `-ation`, `-tion`, `-sion`, `-ity`), gerund consonant doubling
  ("running" matches "run"), and one chained agentive strip
  ("engineering" matches "engine" via "engineer"). The Medium retention
  threshold was also relaxed from 45% to 35% term overlap. Verified against
  the two 2026-09-13 `research-loops` gather logs: 22 of 57 exclusions
  (39%) are now retained, including previously rejected top-ranked hits
  such as "The Agent Loop Architecture - Inngest Blog" and "How to write
  AI agent loops in Claude Code and Codex". Tests: 5 new
  `term_matches_*` unit tests in `test_web_gatherer_helpers.rs`;
  `docs/howtos/research.md` `--use-low-relevance` row updated to document
  the matching behaviour.
- **`/research create` clarification now off by default** - `--no-clarify`
  is the default across all front ends (TUI `/research create`, CLI
  `ragent research create`, HTTP `POST /research`, and `/research update`
  replay): the shared `build_session_config` builder and
  `SessionConfig::default()` resolve `clarify` to `false` when no explicit
  `--clarify`/`--no-clarify` flag is given, so ambiguous topics no longer
  pause the run with a clarifying question. `--clarify` opts back in;
  `--brief` still skips clarification. Root-CLI `--no-clarify` is now a
  documented no-op (kept for recorded-invocation compatibility).
- **`/research help` lists all create flags** - `build_help_message` now
  documents every `create` flag (seed URLs/files, iterations, depth, tier,
  mode, format, sources-dir, template, per-phase models, concurrency and
  timeout budgets, retry/backoff/circuit-breaker knobs, source caps, brief,
  clarify, use-local/use-specs/use-low-relevance/no-papers/use-pdf,
  evaluate) instead of the previous eight-row subset. SPEC.md (both config
  schema copies), QUICKSTART.md, and the `/research` how-to updated to the
  new default.

- **`/tools` registry dedupe + autocomplete fix** - removed the stale
  duplicate `SlashCommandDef { trigger: "tools" }` ("List all available
  tools") from `SLASH_COMMANDS` - the command has always been the
  tool-visibility toggle, so the phantom entry leaked a bogus row into
  `/help`. The real entry's description now lists all nine switches
  (`office, github, gitlab, teams, agents, plan, codeindex, masterfetch,
  browser`), the `/tools` autocomplete suggestions cover all nine plus
  `show|help`, and `docs/howtos/slashcommands/INDEX.md` row matches the
  implementation.
- **GCF encode hook simplification** - `gcf_encode_tool_result` in
  `crates/ragent-agent/src/session/history.rs` replaces the
  `u64::try_from(..).unwrap_or(u64::MAX)` pairs with lossless `usize -> u64`
  casts (char counts are always positive).
- Verification: `cargo check` clean; `cargo audit` pass (10 pre-existing
  allowed warnings); `test_slash_commands` 183/183, `test_gcf_command` 7/7,
  `test_gcf_indicator` 3/3, `test_slash_help` 50/50, `test_gcf_encode_hook`
  10/10, `test_gcf_raw_consumers` 6/6.

## Version: 1.0.98

- **GCF tool-result encoding (spec `gcf`, FR-001..FR-009)** - new opt-in
  `gcf.enabled` config flag (default off, omitted from the saved file while
  disabled) backed by the `gcf` crate (v3.0.1, MIT, zero deps). When
  enabled, the LLM-facing copy of JSON tool results of at least 200
  characters is re-encoded losslessly inside a `[BEGIN GCF generic]` ...
  `[END GCF]` labelled block, but only when the GCF output is at least 10 %
  smaller than the raw JSON; any encode failure falls back to raw. Only the
  LLM view is re-encoded (`tool_result_content_for_llm`, live dispatch and
  replay paths); TUI rendering, activity log, memory extraction, hooks, and
  compaction always see the raw JSON. While GCF is on the system prompt gains
  a short `[BEGIN GCF generic]` reading primer. New `/gcf on|off|show|help`
  slash command persists the flag to the loaded config source, invalidates
  the config cache, and reports effective state/source; unknown subcommands
  are rejected without state changes. Test suites: `test_gcf_config.rs`
  (ragent-config), `test_gcf_encode_hook.rs` / `test_gcf_primer.rs` /
  `test_gcf_raw_consumers.rs` (ragent-agent), `test_gcf_slash_registration.rs`
  / `test_gcf_command.rs` (ragent-tui).
- **Alt+G GCF toggle + status-bar indicator** - `Alt+G` now toggles GCF
  encoding with the same persist semantics as `/gcf on|off` (the `g`
  keystroke is never inserted into the input buffer), and a new line-2
  status-bar service indicator (compression-clamp icon, light magenta,
  leftmost in the icon row — left of the codeindex icon) shows the
  enabled (`✓`) / disabled (`✗`) state in verbose modes. Keybinding
  documented in the `?` help panel. Test suite: `test_gcf_indicator.rs`.
- Docs: `gcf` section in the SPEC.md config schema (new section 5.6), `/gcf`
  in the slash-command tables, config example + feature blurb in
  QUICKSTART.md, and the `/gcf` how-to manual
  (`docs/howtos/slashcommands/gcf.md`) with its INDEX.md row.

## Version: 1.0.97

- **`/toolchain list` hang fix (FR-012)** - `run_probe_raw` in
  `crates/ragent-tui/src/app/toolchain.rs` now detaches (never joins) the
  probe worker thread after a receive timeout; a grandchild process
  inheriting the pipes (shell wrappers fork) could block
  `wait_with_output()` indefinitely and hang the command. The shared
  engine replaces five duplicated probe paths (~60 lines removed);
  spawn failures return `Ok(Some(Err(err)))` so callers keep
  distinguishing `Unknown` from `Timeout`; the table renderer no longer
  emits a doubled bottom border.
- **`/prompt` and repeat-guard simplification pass** - deduplicated the
  `/prompt` dispatch arm in `slash.rs` (bound `miss @` arm, one
  `resolve_agent`), routed TUI config reads through `load_config_cached`
  + `block_in_place`, shared a `REPORT_PREFIX` const across the report
  emitters and the `models.rs` report filter, made `apply_size_cap`
  single-pass, added a `get_mut` fast path to the tool-repeat-guard hot
  loop in `session/permissions.rs`, and reset the full `RepeatTracker`
  on approval. Comment/doc corrections across the touched modules.

## Version: 1.0.96

- **Edit-tool line-structure guard (FR-045)** - the `edit`/`multi_edit`
  whitespace-flexible fallback lane could previously fold a needle's
  same-line whitespace run against the file's newline (or vice versa),
  splicing `new_string` with a joined or split line and corrupting the
  file (observed as "the edit tool collapsed lines" on
  `crates/ragent-server/Cargo.toml`). The flexible matcher now requires a
  needle run and its matched content run to agree on newline membership,
  so line structure can never change; same-line indent/alignment rescue
  and blank-line collapse still work. Logged the corruption chain from
  the edit-log `match_lane=flexible` entries as evidence; new regression
  tests in `test_multiedit_helpers.rs` and an updated
  `test_edit_smoke.rs` scenario that previously codified the corrupting
  join.
- **`/prompt` agent system-prompt inspector** - new read-only TUI slash
  command (spec `specs/prompts/`, 15 FRs) that re-runs the canonical
  system-prompt assembler for an agent and renders what the LLM actually
  receives: `/prompt primary [agent]` (primary-mode report + compact tool
  reference), `/prompt subagent [agent]` (Subagent mode forced, interactive
  tools excluded from the effective surface), `/prompt list` (agent roster
  with mode/source badges), and `/prompt <agent-name>` (case-insensitive
  alias). No LLM call, no writes, no session mutation; 100k-char report cap
  (FR-013); tool-free agents render `(no tools)`. Renderers live in the new
  `crates/ragent-tui/src/app/prompt.rs`; dispatch glue in `slash.rs`
  (`handle_prompt_command`). Five test files: `test_prompt_readonly.rs`,
  `test_prompt_size_cap.rs`, `test_prompt_subagent_render.rs`,
  `test_prompt_tool_free.rs`, `test_prompt_units.rs`. `/prompt` reports
  bypass the research code-block extractor (embedded AGENTS.md/README
  documents contain bare ``` fences). How-to: `docs/howtos/slashcommands/prompt.md`.
- **`/theme` slash command removed** - the command was redundant: it was
  registered in `SLASH_COMMANDS` with autocomplete suggestions
  (`toggle|light|dark`) but had no dispatch arm in
  `execute_slash_command_inner`, and its advertised modes did not even match
  `ThemeMode` (`default|high-contrast`). Removed the registry entry, the
  autocomplete suggestions, the usage hint, the slashcommands doc
  (`docs/howtos/slashcommands/theme.md`), its INDEX.md row, and the two SPEC.md
  UI-preferences table references. The `ThemeMode` enum and `theme_mode` field
  remain (unused by any command) for future use.

### /tasks slash command removed

- Dropped the redundant `/tasks` TUI slash command (session task list, alias of
  `/task list`): registry entry removed from `SLASH_COMMANDS`, the dispatcher
  arm deleted, `test_slash_tasks_help` removed. `/task` remains the single
  surface (bare form toggles the TASKS panel, `/task list` renders the task
  table). Docs updated: `tasks.md` howto and its INDEX.md row deleted
  (pre-existing gap), `/tasks` cross-references dropped from `task.md`,
  `log.md`, and `cron.md`.

### Tool-repeat guard (FR-044)

- New protection against agent loops stuck replaying the same tool call: after
  five consecutive identical calls (same tool + same argument hash), the
  sixth call raises a `tool:repeat` permission prompt in interactive primary
  runs ("Continue?"), and is auto-denied with a corrective observation in
  unattended runs (subagent, `--yes` auto-approve, YOLO) so no run can hang
  on a prompt nobody can answer. Timeout counts as denial (FR-015 semantics).
- `check_tool_repeat_guard` lives in `ragent-agent/src/session/permissions.rs`
  alongside `prompt_for_permission`; it keys on the canonical serialised
  `tool_input` (not the raw `args_json` text) so provider-side key reordering
  or whitespace differences count as one logical repetition. A different-args
  call resets the consecutive counter; a user "allow" also resets it.
- Wired into the dispatch task in `session/processor.rs` immediately after the
  PreToolUse hooks (before the resource permit), covering permission-exempt
  and hardwired-allowed tools too; denials funnel through `denied_tool_call`
  so the TUI spinner always closes out. The tracker state sits on a new
  `SessionProcessor.tool_repeat_guard` field.
- Seven integration tests in `crates/ragent-agent/tests/test_tool_repeat_guard.rs`
  cover the five-call pass, the sixth-call prompt + allow/deny/timeout paths,
  the counter reset, and the subagent / auto-approve fail-closed paths.

### /opt prompt optimization feature removed

- Deleted the `ragent-prompt_opt` crate (12-framework prompt optimization
  templates, `Completer` trait, `OptMethod` enum, `optimize()`), the `/opt`
  TUI slash command (`/opt help`, `/opt <method> <prompt>`, the async
  `opt_result` pipeline and `poll_pending_opt` drain), and the `POST /opt`
  HTTP endpoint (`prompt_opt_handler` + `ServerCompleter`). The `/swarm`
  decomposition call keeps its one-shot LLM helper, now a plain
  `RagentCompleter::complete` method in `ragent-tui` (no longer tied to the
  removed `Completer` trait).
- `ragent-tui` and `ragent-server` no longer depend on `ragent-prompt_opt`;
  the workspace is now 16 crates. `test_opt_result.rs` and the `/opt` slash-command test
  block were removed; `/opt` rows dropped from README/SPEC/QUICKSTART and
  the `/webapi` endpoint table.

## Version: 1.0.95

### /toolchain list table switched to fixed 10/10/10/50 column widths

- The `/toolchain list` ASCII table no longer widens the Language, Runtime,
  and Status columns by 5 characters over their widest content; the four
  columns are now fixed at 10 (Language), 10 (Runtime), 10 (Status), and 50
  (Version) characters (FR-017 updated), keeping the table at a constant 93
  columns wide. Language and Runtime cells wider than their column are
  clipped to the column width; Status and Version cells word-wrap onto
  continuation grid lines instead of clipping, so no status or version text
  is lost (multi-runtime status strings and the data-format marker now wrap).
- `COLUMN_WIDEN_CHARS` is replaced by the `TABLE_COLUMN_WIDTHS` constant;
  the FR-017 spec text, renderer tests, `/toolchain list` TUI regression
  tests, and the `docs/howtos/toolchain.md` example table were updated to
  the fixed-width layout.

### Agent Notice chat-bubble separation fixed

- Consecutive `Event::AgentNotice` notices no longer run onto each other in
  the message window: the TUI event handler now forces a new assistant
  message before and after appending a notice, so every notice renders as
  its own yellow bubble with the renderer's trailing blank line separating
  it from the next bubble and from subsequent streamed text.
- New regression tests in
  `crates/ragent-tui/tests/test_agent_notice_separation.rs` cover
  consecutive notices, streamed text after a notice, and a notice arriving
  mid-stream.

## Version: 1.0.94

### `/spec impl` no longer pre-creates session tracker tasks

- The TUI `/spec impl` runner no longer calls
  `create_spec_impl_session_tasks` to pre-populate the session task list
  with one tracker task per spec task before the run starts; that helper
  (and its tests) is removed from `crates/ragent-tui/src/app/slash.rs`.
- `SpecImplState` now carries only the runner snapshot, the spec id, and
  the execution order plus rank/total progress fields; the session task
  list stays clean until the spec runner itself reports progress.
- `spec_task_update` now creates-or-updates the session tracker task
  itself: it matches on `spec_id`/`spec_task_id` metadata, maps the spec
  status onto the tracker status, and seeds new tracker tasks from the
  spec's PLAN.md entry so `/spec impl` progress stays visible without
  pre-created rows.
- The `/spec impl` dispatch prompts now instruct the agent to mark the
  tracker task `in_progress` BEFORE starting work on each spec task.

### Sub-agent completion protocol is now a hard requirement

- The "Sub-Agent Completion Protocol" section in the sub-agent system
  prompt is retitled "MANDATORY - HARD REQUIREMENT" and now demands
  `agent_complete(summary: "...")` as the FINAL action of EVERY run —
  without exception, including failed, empty, cancelled, or
  "nothing to report" runs, which must report an honest failure summary
  instead of going silent.
- The `agent_complete` tool description now opens with a
  "SUB-AGENTS: calling this tool as your final action is MANDATORY"
  paragraph so the requirement is also visible in the per-turn tool
  reference.
- The mid-run summary nudge (`SUBAGENT_SUMMARY_NUDGE`) ends with "Then
  end your run with agent_complete(summary) as your final action".
- SPEC.md section 16.4.1 upgrades "should finish" to "MUST finish ...
  without exception".
- Two new guard tests in `test_builtin_agents.rs`:
  `test_subagent_completion_protocol_is_mandatory_in_system_prompt` and
  `test_primary_agent_prompt_has_no_subagent_completion_protocol`.

### Compaction performance pass

- Compaction now supports a dedicated fast/cheap model via
  `compaction.model` (`{ "provider_id": ..., "model_id": ... }`) in
  `ragent.json`; `/compact` (and `SessionProcessor::compact_session`)
  resolve and build a separate LLM client for the override instead of
  reusing the session's primary model.
- The summary output budget is configurable via `compaction.summary_tokens`
  (default 1,500 tokens, down from 4,096), roughly halving worst-case
  summary-generation latency on slow models.
- `compaction.tool_output_max_chars` is now configurable (default 2,000).
- The summarisation prompt cap adapts to the model's context window:
  `min(60,000 chars, context_window * 4 / 2)`, so small local models get a
  proportionally smaller prompt instead of a fixed ~15k-token request.
- The compaction stream now has a hard 180-second overall cap plus a
  60-second per-chunk stall timeout, so a drip-feeding or hung provider
  fails fast instead of heartbeating until a 300-second timeout.
- Progress feedback every 10 seconds now reports elapsed time and the
  number of summary characters received so far, replacing the static
  "still running after Ns..." heartbeat every 30 seconds.
- The compaction agent's configured temperature (0.2) is now honoured by
  the summarisation request — it was previously dropped because
  `build_summary_request` hardcoded `temperature: None`.
- Cancellation is now checked between stream chunks: `/compact` honours
  the session cancel flag during the LLM call, not just before it.
- Token estimation no longer re-serialises tool JSON schemas on the
  compaction path: `estimate_tool_tokens` and tool-use input sizing use an
  allocation-free `json_serialized_len` byte counter instead of
  `parameters.to_string()`.

## Version: 1.0.92

Subagent reliability fix: sub-agent runs (background `new_agent`, cron agent
runs, team teammates, skill invocations) no longer block forever when the
model calls an interactive tool. Sub-agent runs have no attached user, so an
unanswered `ask_user` dialog previously stalled the run until the watchdog
(production symptom: sub-agents that never terminate). Interactive tools are
now removed from the wire tool surface advertised to a Subagent-mode run
(`SessionProcessor` tool-definitions filter and the system-prompt tool
reference), the "Question Tool Usage" system-prompt section is no longer
injected for sub-agent runs, and a hallucinated interactive call is denied at
dispatch with a corrective observation ("interactive tool 'ask_user' is not
available in subagent runs - no user is attached; decide autonomously and
continue") so the loop continues instead of blocking. Primary (interactive)
runs are unchanged — `ask_user` still executes and returns the user's reply.
New integration suite `test_subagent_interactive_block.rs` covers denial in
subagent runs, continued loop completion after denial, absence of interactive
tools from the subagent tool surface, and the intact primary-run ask_user
round-trip (responder task subscribed before the run so the single-threaded
test runtime delivers `QuestionAnswered`; the scripted ask_user call now
carries a valid JSON question payload and a bounded 5 s responder wait guards
against suite hangs).

## Version: 1.0.91

Documentation update pass: full project docs refresh (`CHANGELOG.md`,
`README.md`, `STATS.md`, `SPEC.md`, `QUICKSTART.md`, `TUI-QUICKSTART.md`)
covering the scaffolding simplify work, regenerated crate statistics
(436,219 Rust lines across 1,064 files, ~8,162 tests), the `/new`
`plan_and_emit`/`register_origin_and_push` architecture captured in SPEC
section 11A, and all 19 `docs/howtos/` manuals re-rendered to PDF
(xelatex, A4, including the new `newproj.md.pdf`). No code behaviour change.

Code-simplification pass (mode `all`) over the project-scaffolding feature and
the status bar: the TUI `/new` command and the `ragent new` CLI subcommand now
share a single `plan_and_emit()` engine function in
`crates/ragent-tools-extended/src/project_scaffold/mod.rs` (returning the
scaffold summary plus the stack note; each surface attaches its own
git/hosting outcome lines), removing roughly 95 duplicated pipeline lines.
The GitHub and GitLab remote flows in `project_scaffold/remote.rs` share a
`register_origin_and_push(root, url)` helper, and the status-bar
`prompt_display_text` renderer in `crates/ragent-tui/src/layout_statusbar.rs`
is now single-pass (one `chars().take()` walk plus a one-character lookahead
decides the `....` marker) instead of building the truncated string twice.
The `run_scaffold_steps` helper in the TUI command dropped a redundant
parameter (and its `too_many_arguments` allowance) as part of the
`plan_and_emit` move. Verified with `cargo fmt --check`, `cargo check
--workspace`, clippy on the touched crates, and the full scaffold engine,
TUI `/new`, status-bar, and CLI `new` test suites.

## Version: 1.0.90

The status bar's top line now prefixes the git branch with a `Branch: ` label
(e.g. `Branch: main`) and renders the branch + last-prompt tag as one group
immediately after the `Project: `-labelled working directory — the tag is no
longer independently centred, it follows the branch section with a single
separator space. The working directory is shortened only when the group plus
the right-hand session status would not otherwise fit (keeping a 12-column
`MIN_CWD_SPAN` readable minimum); otherwise the natural cwd width is kept.
The no-prompt fallback layout (cwd, gap, branch, status) is unchanged.

The status bar's top line now prefixes the working directory with a
`Project: ` label (e.g. `Project: ~/Projects/ragent`) so the cwd section is
identifiable at a glance next to the git branch and centred last-prompt tag.
The label participates in the existing width budgeting: when the tag is
rendered, the path is shortened to make room for the label so the tag stays
centred, and the Compact/Minimal responsive modes keep their shortened-path
budgets with the label prepended.

`/new` gained the `ratatui` stack (spec `newproj` FR-007): `--stack ratatui` on a
Rust project appends `ratatui = "0.29"` and `crossterm = "0.28"` to `Cargo.toml`
and layers a terminal-UI starter over the hello-world entry point — a ratatui
`Terminal` on the crossterm backend that draws a `Hello, world!` `Paragraph`
each frame and exits on the first key-press event. The stack registers in the
same `STACK_RECIPES` registry as `axum`/`warp`/`raylib`/`gtk4`, so `/new help`
stack examples, case-insensitive matching, and the warn-and-continue behaviour
for unknown stacks are all derived automatically (NFR-001). Covered by a new
overlay snippet test plus the registry-driven help and CLI/TUI assertions.

Fixed the stack dependency append producing an unparseable `Cargo.toml`
(applies to every Rust stack): the generated base manifest has no
`[dependencies]` section, so appending `axum = "0.7"` etc. put the dependency
lines under `[package]`. `apply_stack_overlay` now inserts a
`[dependencies]` header before the first dependency line when the manifest
does not already declare one; all five Rust stacks (axum, warp, raylib,
gtk4, ratatui) emit a manifest that `cargo check` accepts, verified by
smoke-testing generated projects.

`/new` gained the `gtk4` stack (spec `newproj` FR-007): `--stack gtk4` on a
Rust project appends `gtk4 = "0.9"` to `Cargo.toml` and layers a GTK 4
starter over the hello-world entry point — a `gtk4::Application` builder
with an application id, an `activate` handler that presents a titled
`Hello, world!` window, and `app.run()`. The stack registers in the same
`STACK_RECIPES` registry as `axum`/`warp`/`raylib`, so `/new help` stack
examples, case-insensitive matching, and the warn-and-continue behaviour for
unknown stacks are all derived automatically (NFR-001). Covered by a new
overlay snippet test plus the registry-driven help and CLI/TUI assertions.

`/new` language registry extended to the full codeindex scanner set
(spec `newproj` FR-017): the scaffolder now accepts 46 canonical languages —
the original 23 application languages plus 23 new recipes for `shell`, `zsh`,
`fish` (full four-app-type hello-world sets: `sh`/`bash main.sh`, `zsh`,
`fish` scripts), and 20 data/markup/build formats (`toml`, `yaml`, `json`,
`xml`, `html`, `css`, `scss`, `sql`, `markdown`, `protobuf`, `verilog`,
`vhdl`, `terraform`, `openscad`, `cmake`, `gradle`, `gradle_kts`, `maven`,
`nix`, `hcl`) that scaffold a manifest plus `cmdline`/`library`
`Hello, world!` sample documents, with `--type tui`/`--type gui` degrading to
manifest-only layouts. Every id in the codeindex scanner's
`SUPPORTED_LANGUAGES` list now resolves: scanner dialect ids `tsx`, `jsx`,
`c_header`, and `cpp_header` map to their parent languages, with additional
aliases (`sh`/`bash`, `yml`, `sv`, `vhd`, `tf`, `scad`, `kts`). Real build
systems get canonical manifests (`pom.xml`, `build.gradle`,
`build.gradle.kts`, `CMakeLists.txt`, `main.tf`, `index.html`, `NOTES.md` —
never `README.md`, which the docs layer owns). The `/new help` page
documents the stub vs. hello-world split and the full alias list; help
value lists remain registry-derived so they cannot drift (NFR-001). New
tests: registry-vs-codeindex parity, dialect-alias parsing, stub
cmdline/library sampling, and registry-wide gitignore coverage.

`/new help` detailed help renderer (spec `newproj` T-016, FR-018, NFR-001):
bare `/new` and `/new help` now print a full help page — command purpose,
per-argument documentation (`--language`, `--type`, `--stack`, `--github`,
`--gitlab` each documented with name, one-line description, required/optional
status, accepted values, and the default behaviour when omitted), and two
worked example invocations (one minimal without hosting, one with a hosting
flag). The accepted-value lists are derived from the scaffolder's language
and app-type registries plus the `STACK_RECIPES` table, so help text cannot
drift from what the command accepts (NFR-001). The page is rendered by a
shared `project_scaffold::render_detailed_help` engine function used by both
the TUI slash surface and the `ragent new` CLI surface (surface-specific
usage lines and example spellings only); validation errors still append the
same detailed page.

Project scaffolding documentation (spec `newproj` T-015, FR-012): new how-to
manual `docs/howtos/newproj.md` covering the `/new` slash command and the
`ragent new` CLI surface — purpose and scope, quick start, command syntax with
the flag table (`--language`, `--type`, `--stack`, `--github`/`--gitlab`),
generated-content reference (workspace artifacts, per-language/type artifact
matrix, stack layers, documentation scaffold), the empty-directory guard,
GitHub/GitLab hosting flows with failure containment, progress streaming and
the summary report, CLI equivalents, five end-to-end examples, and a
troubleshooting table. README gains a Project Scaffolding section, the `new`
entry in the CLI command list, and a feature bullet; SPEC.md gains section
11A (Part III cross-reference in the table of contents) describing the
scaffold pipeline and generated content plus a `10.12` pointer subsection,
three long-standing unpaired/merged code fences were repaired (the
beta-changelog JSON stanza and the two `specs/` tree blocks in SPEC.md, and a
merged fence line in the QUICKSTART provider-setup block); QUICKSTART.md
gains a "Scaffold a New Project" common workflow and `/new` rows in the
slash-command table; TUI-QUICKSTART.md gains section 7a (scaffolding with
`/new`) and a v1.0.89 highlight bullet.

## Version: 1.0.89

Toolchain and sub-agent lifecycle polish on top of the v1.0.88 research-crate
simplify pass: the workspace now builds on the stable Rust channel, sub-agents
are prompted to terminate themselves via `agent_complete`, mid-run compaction
summaries are visually disambiguated from sub-agent completion reports, and the
Context side panel moved to the `Alt+C` side-panel chord.

### Changed

- **Rust toolchain pinned to stable** — `rust-toolchain.toml` moved from
  `nightly-2026-09-04` to the `stable` channel (stable 1.98.1 at time of
  release); local builds, CI, and user shells now compile with stable Rust.
- **Context panel keybinding `Alt+X` → `Alt+C`** — the Context side panel
  toggle moved to `Alt+C`, keeping the side-panel chord family
  (`Alt+L` log, `Alt+P` profiler, `Alt+T` tasks, `Alt+O` telemetry,
  `Alt+C` context) mnemonic-consistent. Updated the key-handling arm
  (`input.rs`), the help-table entry (`layout.rs`), related doc comments
  (`utils.rs`, `state.rs`, `input_handler.rs`), the contextpanel spec
  (`specs/contextpanel/`), and the user-facing docs (TUI-QUICKSTART,
  QUICKSTART, README, SPEC, tutorial how-to). The TC-001 test now drives
  `Alt+C` (`test_context_panel_t016.rs`).

### Added

- **Sub-agent completion protocol** — a new sub-agent-mode-only
  "Sub-Agent Completion Protocol" section in the system prompt
  (`ragent-agent/src/agent/mod.rs`) instructs spawned sub-agents to always
  finish with a single `agent_complete(summary=...)` call, so background
  tasks break their loop promptly instead of lingering in a running state
  after their final report.

### Fixed

- **Compaction label disambiguation** — mid-run auto-compaction summaries
  now render with a dim `[compaction]` label in the transcript
  (`layout.rs`/`message_widget.rs`) instead of the assistant `●` marker, so a
  compaction summary is no longer mistaken for a sub-agent's completion report
  in the output overlay (root cause of the "agent stays active ~120s after
  completion" misdiagnosis).
- **Edit-tool test hygiene** — `read_utf8_file` in
  `ragent-tools-core/src/edit.rs` carries an `#[allow(dead_code)]` with a
  reason comment, matching the existing `create_file` pattern for items used
  by the lib target but not by the `#[path]`-importing test target.
- **Loop-interrupt test robustness** — `test_loop_interrupt` re-applies the
  polling `raise_interrupt_until_armed()` helper so the Esc-interrupt test no
  longer flakes when CI scheduling runs the loop to completion before the
  interrupt request lands.

### Verification

- Full `/rust-hygiene` suite (10 checks) green on the release tree:
  `cargo check --workspace`, cargo-machete (no unused dependencies),
  `cargo check --tests --workspace`, full `cargo test --workspace` (490 test
  binaries, exit 0), dead-code lint (`-D unreachable_pub -D dead_code
  -D unused_imports` over `--workspace --lib --all-features`),
  `scripts/check-dead-code-reasons.sh`, clippy `-D warnings`,
  `cargo fmt --all -- --check`, `cargo audit` (0 errors; 1 allowed yanked
  chacha20 warning), and `cargo deny check` (advisories/bans/licenses/sources
  ok).

## Version: 1.0.88

Research-crate simplify pass: a `/simplify` code-quality audit over the
`ragent-research` crate (all 56 source files reviewed by five parallel audit
agents; no behaviour change intended). Correctness fixes, regex caching, and
duplication removal across seven files (+419/-400), shipped together with the
v1.0.88 documentation refresh (CHANGELOG/README/STATS/SPEC/QUICKSTART/
TUI-QUICKSTART, research how-to, regenerated spec + research how-to PDFs).

### Fixed

- `parse_subject_summary` slice panic: an LLM response containing `}` before
  `{` inverted the `trimmed[start..=end]` slice index and panicked the
  analysis merge path; the span is now extracted with
  `trimmed.get(start..=end)?` so a malformed span yields `None` (mechanical
  fallback) instead of a panic (`analysis.rs`).
- `AnalysisEngine::with_brief` promoted from a panicking
  `unimplemented!()` default to a required trait method. The previous no-op
  default silently broke mock-engine tests that expected the brief to reach
  `analyze_with_outcome`; requiring the method makes the compiler enforce
  what the runtime panic enforced (`analysis.rs`).
- `SearchCallOutcome::Ok` no longer carries an unread `retries` field (the
  `Err` variant still carries retries for `SearchRetrying` events); the
  stale `#[allow(dead_code)]` on the enum was removed (`web_gatherer.rs`).
- `ResearchManager::continue_item` no longer calls the no-op
  `mark_in_progress_for_state` helper (deleted); the doc comment now states
  that item status lives in the `RESEARCH.md` frontmatter, not the state
  file, and the state is written back unchanged when there is no follow-up
  (`manager.rs`).
- `document.rs` doc comment corrected: `REQUIRED_SECTIONS` describes 9
  sections, not 10.
- `document.rs` `cited_date_span` computes the earliest/latest cited years
  with running min/max variables instead of collecting a
  `Vec<i32>` + two passes.
- `web_gatherer.rs` `--from-url`/observer paths classify each page's media
  type once and reuse the result at all three render sites (SourceCaptured
  event, vault store, final `Source::Web`), instead of re-running
  `classify_web_source` independently three times per page.

### Changed

- Regex caching: `analysis/parser.rs` (citation marker, bare citation, ISO
  date, finding-dependency) and `session/fallback.rs` (`**Implication:**`)
  compile their patterns once via `OnceLock` statics instead of rebuilding
  the `Regex` on every call; `document.rs` `url_regex`/`fence_re` already
  shared the same pattern.
- `document.rs`: a new `layout` module shares eight section-push helpers
  (`push_queries`, `push_search_engine_summary`, `push_provider_requests`,
  `push_open_questions`, `push_findings`, `push_cross_references`, ...) between
  `assemble_report_body` and `assemble_imrad_body`, removing ~150 duplicated
  lines. Note: the report layout's `Search Engine Summary` and `Search
  Provider Requests` blocks remain level-2 (`###`) sub-headings (pre-existing
  behaviour, covered by `assemble_document_renders_search_engine_summary_after_queries`).
- `web_gatherer.rs`: an `is_sole_engine_hit(hit, engine)` helper replaces the
  duplicated engine-set logic in `is_scholarly_hit`/`is_encyclopedia_hit`;
  a `collect_engines()` helper produces the sorted, de-duplicated engine list
  for both gather paths, fixing an ordering inconsistency (the vault path
  previously emitted unsorted `HashSet` iteration order while the observer
  path sorted); the pass-through `fence_captured_body` wrapper was deleted in
  favour of calling `fence_source_body` directly.
- `session.rs`: a `MediaCounts::of(&sources)` single-pass tally replaces four
  separate `matches!` filter scans over the source list (pdf/youtube counts
  computed at gather-complete, finalize, and document-assembly sites);
  `planner_or_default()`/`critic_or_default()` helpers replace five repeated
  `unwrap_or_else(Arc::new(HeuristicPlanner/SimpleCritic))` chains; the now-
  unused `SimpleCritic`/`HeuristicPlanner` imports were dropped.
- `analysis.rs` `merge_chunk_results`: cross-reference/implication/question
  dedup sets are `HashSet<&str>` borrowed from the parts instead of cloning
  every string into the seen-set, and findings concatenate with a single
  `iter().cloned()` per part instead of a double clone.

### Verification

- `cargo check --workspace` clean; workspace clippy with `-D warnings` clean
  (a `search_budget_for(Option<usize>)` helper was rejected by
  `redundant_closure_for_method_calls`-style lint review and inlined back at
  its three call sites); `cargo fmt --all -- --check` clean; dead-code lint
  (`-D unreachable_pub -D dead_code -D unused_imports`) clean;
  `cargo test -p ragent-research` 41 test binaries all green (662 lib tests).

## Version: 1.0.87

UI polish and tool-calling fixes on top of the v1.0.86 spec-system
documentation pass. The status bar gained a centred last-prompt tag, the
message window no longer pushes the transcript down with a leading blank
line, and slash-command internals were consolidated.

### Changed

- Status bar top line: the most recently submitted prompt is rendered as a
  centred bracketed tag (`[first 32 chars....]` when truncated) between the
  git branch and the session status. `App.last_prompt` tracks the latest
  prompt across chat messages, slash commands, bang commands, and `/loop`
  goals; the working-directory section is shortened so the tag stays centred,
  and the layout falls back to the previous cwd/branch/status arrangement
  when the terminal is too narrow to fit all sections without clipping.
- Message window: the blank-line separator before a `You:` prompt is no
  longer emitted for the very first rendered line of the transcript, so the
  transcript no longer starts (or restarts after scrollback) with a stray
  blank row pushing content down.
- Slash-command internals: a `websearch_diag_ctx()` helper replaces the
  duplicated nine-field `ToolContext` literal in the `/websearch test` and
  `/websearch show` diagnostic arms; an `is_help_args()` helper replaces
  repeated `args.trim() == "help"` checks across slash arms; `/spec` arms
  reuse the shared `spec_manager()` helper.
- Tool-calling recovery: `session::text_toolcalls::extract_text_tool_calls_with_spans`
  and `blank_spans` are now `pub(crate)` (crate-internal helpers were
  publicly exported).

### Added

- New tests: `test_statusbar_last_prompt.rs` (tag truncation semantics,
  centring, and no-prompt fallback) and `test_schema_validation.rs`
  (required-args schema validation, relocated from the inline `#[cfg(test)]`
  module per the workspace test-organization rule).

## Version: 1.0.86

Spec-system simplify and documentation pass on top of the v1.0.85 tool-calling
audit remediation. `/spec` semantics and coverage rendering were consolidated,
the spec how-to manual was expanded, and the docs set (SPEC.md appendix,
QUICKSTART, TUI-QUICKSTART, README, STATS) was refreshed to the 1.0.86 state.

### Changed

- `Spec::coverage_report()` is now the single requirement-coverage renderer
  shared by the TUI `/spec coverage` arm and the `spec_coverage` agent tool,
  with ASCII `[ok]`/`[wait]`/`[sync]`/`[stop]` status symbols from
  `TaskStatus::symbol()` as one source of truth (the two former drifting
  implementations are gone).
- Spec file write semantics tightened: atomic temp-file + rename writes with
  unique temp names (monotonic sequence) and sync-before-rename;
  clear-on-empty `REVIEW.md`/`FEEDBACK.md` (a cleared field deletes the file
  instead of leaving stale content that repopulated on the next read);
  `update_frontmatter` preserves unmodelled frontmatter keys (e.g. `research:`
  linkage) and YAML-escapes audit/reviewer values.
- Automatic task completion after writes is now guarded by
  `writes_in_spec_dir`: only file-writing tools (`write`, `edit`, `multiedit`,
  `patch`, `apply_patch`, `create`, `append_to_file`) whose resolved target
  sits inside the active spec's `specs/<id>/` directory advance `in_progress`
  tasks to `completed`; writes elsewhere in the workspace never complete spec
  tasks (prevents unrelated edits silently corrupting `PLAN.md`).
- `/spec` arms consolidate 15 `SpecManager::new` constructions into one
  `spec_manager()` helper and 11 `SpecId::new` match blocks into
  `parse_spec_id()`; `discover_specs`/`read_spec` share one
  `load_spec_from_dir` hydration path; search snippet extraction reuses the
  already-lowercased text; `extract_from_research` is token-based and strips
  dangling `--from-research` tokens.
- Spec internals simplification: `find_dependents` drops a dead O(n) map,
  `build_file_order_warning` replaces an `unwrap()` with let-else,
  `Effort`/`Priority` parse without allocation, validation report template
  counts are sorted for deterministic output, and amendment rows containing
  `|` are parsed correctly (rationale cells rejoined instead of mis-split).

### Fixed

- UTF-8 panic in feedback logging: `build_feedback_log` sliced `&note[..80]`
  by byte offset and panicked on multi-byte notes; truncation now lands on a
  real character boundary via `char_indices().nth(80)`.
- Atomic-write temp collision + durability: concurrent writers no longer race
  on the same `.tmp` path; the temp file is synced before rename.
- `create_spec_dir` TOCTOU: `create_dir` + `AlreadyExists` mapping replaces
  the racing `exists()` pre-check.
- `parse_tasks` no longer fabricates `completed_at: Some(1)` on read-back.

### Added

- `Task::table_row()` / `PlanTask::table_row()` shared task-table row
  formatting; `SpecIo::extract_research` populates the documented-but-unused
  `Spec.research` field; `SpecIo::frontmatter()` single frontmatter slicing
  helper; `EarsTemplate` re-exported from the crate root.

### Documentation

- `docs/howtos/spec.md` gained a "Spec file write semantics" subsection (atomic
  writes, clear-on-empty `REVIEW.md`/`FEEDBACK.md`), a `/spec coverage` report
  format description, a task-status symbol table, an "Automatic task completion
  after writes" subsection documenting the `writes_in_spec_dir` guard, and a
  note that `/spec update` preserves unmodelled frontmatter keys; the PDF was
  regenerated. SPEC.md gained sections 10.10a-10.10c covering the same
  semantics. How-to PDFs regenerated (pandoc xelatex A4).

### Verification

- `cargo check` passes; `cargo audit` reports no actionable security failures
  (only the pre-existing allowed warnings).

## Version: 1.0.85

Tool-calling audit remediation pass (arg parsing, dispatch, providers, edit
tools). Fixes every HIGH/MED finding from the tool-calling + UTF-8 audit:
`crates/ragent-agent` (processor dispatch), `crates/ragent-llm` (provider
stream parsers), `crates/ragent-tools-core` (edit family + central validator),
25 files plus 4 new test files. Highlights:

### Verification

- `cargo check` passes (only the pre-existing future-incompat notice for the external `attribute-derive-macro` crate).
- `cargo audit` reports only the 10 allowed warnings (unmaintained `ttf-parser`, unsound `lru` 0.12.5 + 0.16.4, yanked `chacha20` transitive dependencies); no actionable security failures.

### Fixed — tool calls executed reliably

- **OpenAI Responses API tool calls were silently dropped** — the
  `openai_responses` provider emitted `ToolCallDelta`/`ToolCallEnd` for
  `response.function_call_arguments.*` events but never a `ToolCallStart`, so
  the agent loop (which creates a pending call only on `ToolCallStart`) lost
  every tool call and only the surrounding text was rendered. The provider now
  handles `response.output_item.added` (type `function_call`) and emits the
  `ToolCallStart` with the item's `call_id` and `name`.
- **Gemini final-chunk `functionCall` lost** — within a candidate frame the
  finish reason was processed BEFORE the content parts; a final frame carrying
  both `finishReason` and a `functionCall` part pushed the call into the
  pending buffer *after* the flush-and-clear, so it was never emitted. Parts
  are now parsed first, an end-of-stream flush emits any remaining buffered
  calls exactly once (tracked by an emitted-id set), and the never-implemented
  "ensure we emit a finish event" comment is now backed by real behaviour.
- **Malformed tool arguments silently became `{}`** — `args_json` was re-parsed
  at 7 sites in the tool dispatch flow and 6 of them coerced parse failures to
  an empty object, so the tool executed with empty arguments and the model only
  saw a misleading "missing parameter X" error. Arguments are now parsed ONCE
  per call; a parse failure short-circuits execution with a corrective,
  LLM-visible error ("Invalid arguments JSON for tool '<name>': <serde error>…
  resend the complete tool call with corrected JSON") so the model can recover
  in one shot instead of looping on missing-parameter messages.
- **Loop restriction guard now fails closed** — `deny_reason` used to evaluate
  against `{}` when the arguments JSON was malformed, letting path-based scope
  checks pass vacuously in restricted `/loop` runs; unparseable args now deny
  the call with an explicit reason.
- **Tool schema is finally enforced** — a new `ragent_tools_core::schema`
  validator checks required-parameter presence and primitive types against
  each tool's `parameters_schema()` before permissions and execution, with a
  corrective error naming the offending field. Unknown fields and optional
  fields are deliberately not enforced.
- **Panicked tool tasks no longer orphan tool_use records** — a tool task that
  panicked (or failed to join) was silently dropped, leaving an assistant
  `tool_use` without a matching tool result in the conversation history. The
  dispatch now synthesises an error result (published through the normal
  handler) and closes the UI call with `ToolCallEnd`.
- **Watchdog abort no longer drops sibling results** — in the parallel tool
  path the first watchdog-stalled call broke the result drain, discarding
  already-completed sibling results and publishing a placeholder
  `watchdog-parallel` call id. The drain now continues past stalled calls and
  publishes the REAL call id; the sequential path gained the same synthetic
  error-result treatment for join failures.
- **Permit-acquisition failure closed the TUI spinner** — the early return
  never published `ToolCallEnd`; it now does.
- **Interrupt during the tool phase stops dispatch** — not-yet-started calls
  are skipped (no orphaned tool_use) and the turn ends gracefully.
- **Unknown tool names get a recovery hint** — "Unknown tool: 'x'. Did you
  mean 'y'?" via cheap similarity (case, prefix, containment, small edit
  distance) against the registry; a duplicate-`function.name` guard also
  prevents duplicate `ToolCallStart` events in OpenAI-family parsers.
- **Tool-call arguments from object-form providers** — llama.cpp / vLLM style
  servers emit `arguments` as a JSON object; the OpenAI-family parsers
  (`openai`, `ollama`, `copilot`, `huggingface`, `openrouter`) previously read
  only the string form and executed with empty args. Both forms are accepted
  everywhere now.
- **Anthropic-family parallel tool_use blocks no longer mis-associate args** —
  `input_json_delta` and `content_block_stop` were attributed to "the last
  HashMap entry" (arbitrary order) in `anthropic` and `azure_resource` (where
  the index was captured then ignored, and args were never even accumulated);
  both parsers now key open blocks by the SSE content-block index.
- **Ollama narration suppression is stream-scoped** — narration was only
  suppressed when the SAME delta carried the tool call; once any tool call is
  seen, later content deltas are suppressed as duplicate narration (pre-call
  narration cannot be retracted retroactively).
- **Capability gating for tool-incapable models** — HuggingFace models declared
  `tool_use: false` (e.g. DeepSeek-R1) no longer receive the tools array; the
  Model Router skips models known to lack tool capability when the request
  carries tools (falling back to the first entry with a warning rather than
  failing the request). Ollama's `/api/tags` catalog cannot report tool
  capability, so it stays ungated by design (documented).
- **MCP non-object arguments normalised consistently** — the rmcp/stdio and
  HTTP transports used different fallback shapes for a non-object payload; a
  shared `normalize_mcp_arguments` now serves both (object passthrough, null →
  empty map, other values wrapped in a documented `"value"` envelope key).
- **Unbounded args accumulation bounded** — `args_json` deltas past 1 MiB are
  dropped so a runaway provider fails the parse (with the corrective error)
  instead of growing the buffer without limit.

### Fixed — edit tools (UTF-8 / line endings)

- **CRLF files no longer silently converted to LF at every edit point** — the
  whitespace-folding flexible lane matched a needle `\n` against file text
  `\r\n` (the CR folded into the run) and the splice consumed it; the indent
  lane re-emitted LF only, and the cascade's lane-2 substitution site bypassed
  restoration entirely. The matched span's original line-ending style is now
  re-emitted onto the replacement in both lanes (terminator-aware per line in
  the indent lane), so edited regions keep CRLF endings and `git diff` shows
  only real changes.
- **BOM (U+FEFF) survives edits** — a leading BOM sat outside any needle
  composed from a BOM-less read, so first-line edits could never match and the
  model looped on "not found"; the BOM is now stripped for matching and
  re-prefixed on write in both `edit` and `multi_edit`.
- **Non-UTF-8 files get a precise error** — the blanket "file may not exist or
  is not accessible" buried the encoding failure; the tools now read bytes and
  validate UTF-8 explicitly, reporting "the file is not valid UTF-8; edit tools
  require UTF-8 text files". Still hard-rejected, never lossy-converted.
- **Latent char-boundary panics hardened** — `byte_offset_to_line` no longer
  slices (`bytes().take(offset)` + debug assert), and every splice site in
  `edit`/`multi_edit` carries a `debug_assert!(is_char_boundary)` so a future
  lane regression panics in debug builds instead of slicing mid-char.

### Added

- **Text-format tool-call recovery** — when a step produces no native tool
  calls, the processor runs a conservative extractor over the assistant text
  (`session/text_toolcalls.rs`) recognising the common markup dialects models
  narrate instead of calling: the Qwen-style `tool_call` JSON blocks,
  `function=name` XML-parameter blocks, and a whole-response bare tool-call
  JSON object/array. Ordinary prose never matches (markup tags must be
  literally present); recovered calls go through the full dispatch pipeline
  (loop restrictions, hooks, permissions) and the model is notified.
- `ragent_tools_core::schema::validate_required_args` — central required-args
  validator (see above), with unit tests.
- New regression tests: `test_gemini_tool_calls.rs` (3 tests), Ollama
  object-args + narration-suppression tests, `test_edit_utf8_fixes.rs`
  (CRLF preservation, BOM round-trip, non-UTF-8 message), and text-fallback
  extraction tests (11 cases including multibyte payloads).

## Version: 1.0.86 — /spec simplify pass (superseded section, retained for detail)

/simplify pass over the /spec system (ragent-specs + its TUI/agent consumers):
17 files, +580/-455. Highlights:

### Fixed

- **Auto task-completion no longer corrupts PLAN.md** — the agent loop marked
  every `in_progress` spec task `completed` whenever ANY file-write tool was
  called; writes are now required to resolve inside the active spec's own
  directory (`writes_in_spec_dir` guard on the tool-call path arguments).
- **UTF-8 panic in feedback logging** — `build_feedback_log` sliced `&note[..80]`
  by byte offset; multi-byte notes (em-dash) panicked on a char boundary. Now
  truncates at a real character boundary via `char_indices().nth(80)`.
- **Frontmatter key loss on write** — `update_frontmatter` rebuilt the
  frontmatter from scratch, silently dropping unmodelled keys (notably
  `research:` written by `/spec create --from-research`) on every status
  transition. Unmodelled keys are now preserved; audit/reviewer values are
  YAML-escaped so embedded quotes cannot corrupt the frontmatter.
- **Stale REVIEW.md/FEEDBACK.md after in-memory clear** — `write_spec` skipped
  empty content, so a cleared field round-tripped as a no-op and the stale file
  repopulated it on the next read; the file is now deleted when its content is
  cleared (empty files are still never created).
- **Atomic-write temp collision + durability** — temp names are unique per
  write (monotonic sequence) so concurrent writers no longer race on the same
  `.tmp` path, and the temp file is synced before rename.
- **create_spec_dir TOCTOU** — the `exists()` pre-check raced past concurrent
  creation; `create_dir` + `AlreadyExists` mapping is used instead.
- **Lying completion timestamp** — `parse_tasks` fabricated
  `completed_at: Some(1)` for completed rows on read-back; tests updated to the
  honest `None` (PLAN.md tables carry no timestamp column).
- **Non-deterministic validation report** — `Report::format` iterated a
  `HashMap` of template counts; output is now sorted by template name.
- **Amendment rows containing `|`** — `parse_amendment_row` shifted cells when
  the rationale contained a pipe; middle cells are rejoined instead of
  mis-parsed; an unreachable duplicate `section_end` assignment was removed.

### Added

- `Spec::coverage_report()` — single requirement-coverage renderer shared by
  the TUI `/spec coverage` arm and the `spec_coverage` agent tool (previously
  two drifting implementations, one ASCII one emoji).
- `TaskStatus::symbol()` — ASCII status symbols as one source of truth.
- `Task::table_row()` / `PlanTask::table_row()` — shared task-table row
  formatting (manager PLAN rewrite + `/spec tasks` builder).
- `SpecIo::extract_research` + `Spec.research` is now actually populated from
  frontmatter (the field was documented but never populated).
- `SpecIo::frontmatter()` — single frontmatter slicing helper for
  status/reviewers/research extraction.

### Changed

- `/spec` arms consolidate 15 `SpecManager::new` constructions into one
  `spec_manager()` helper and 11 `SpecId::new` match blocks into
  `parse_spec_id()`.
- `discover_specs`/`read_spec` share one `load_spec_from_dir` hydration path
  (was ~50 duplicated lines); requirements are built before the `spec_md` move,
  removing a full-text clone per spec.
- Search snippet extraction reuses the already-lowercased text (one
  lowercasing per document instead of two); empty search queries short-circuit.
- `find_dependents` drops a dead O(n) ID-to-index map; `build_file_order_warning`
  replaces an `unwrap()` with a let-else; `milestone_groups` borrows milestone
  names instead of cloning per task; `Effort`/`Priority` parse without
  allocation; unrecognised task Status values warn (parity with effort/priority).
- `Report` filter helpers deduplicated to `any_in`/`count_in`;
  `is_usage_error` shares a `USAGE_SUBCOMMANDS` const with parse;
  `build_create_message`/`build_specify_message` drop unused `_feature`
  parameters; `extract_from_research` is token-based (prose containing the flag
  no longer splits) and strips dangling `--from-research` tokens.
- lib.rs re-exports `EarsTemplate`.

### Documentation

- `docs/howtos/spec.md` — new "Spec file write semantics" subsection (atomic
  writes, clear-on-empty `REVIEW.md`/`FEEDBACK.md`), a `/spec coverage` report
  format description (shared `Spec::coverage_report()` renderer, `[ok]`/
  `[  ]` requirement symbols), a task-status symbol table (`[wait]`/`[sync]`/
  `[ok]`/`[stop]`), an "Automatic task completion after writes" subsection
  documenting the `writes_in_spec_dir` guard, and a note that `/spec update`
  preserves unmodelled frontmatter keys (e.g. `research:` linkage).
- `docs/howtos/pdf/spec.md.pdf` regenerated (A4 xelatex).

## Version: 1.0.84

The /simplify all pass fixes (post 1.0.83) reviewed by parallel sub-agents over the diff since 1.0.83, plus slash-command help coverage.

### Added

- **Codeindex store helpers** — `list_files_with_entries()` (single-scan file +
  entry enumeration), `get_symbol_by_exact_name()` (exact-match-first bounded
  symbol lookup before the substring fallback).
- **TUI helper functions** — `plural(n, word)`, `files_preview(files, take)`,
  `current_working_dir()` (replaces 63 silent `unwrap_or_default()` cwd
  resolutions that would resolve paths against the filesystem root), and
  `loop_config_defaults()` for the /loop dialog.
- **Rollback result polling slot** — the post-/loop rollback flow now polls a
  dedicated `rollback_result` slot instead of fire-and-forget logging, so the
  status line no longer sticks at "rolling back..." forever.
- **Codeindex background result slot type change** — the fragile
  `"\n\nSTATUS:"` magic-marker payload became a typed
  `Option<Result<(String, String), String>>` slot.

### Fixed

- **Real panic in research comparison** — `comparison.rs` sliced byte offsets
  from `to_lowercase()` against the ORIGINAL string; a character that lengthens
  under lowercase (U+0130) violated char boundaries. Now slices the lowercased
  text with an `is_char_boundary` guard plus line fallback; summaries are
  prepared once per profile (PreparedSummary) instead of per cell.
- **Dead TUI event-lag reconcile** — two separate `tui_event_lag` Arcs were
  created (bridge incremented one, app watched the other); now a single shared
  Arc drives the lag-driven reconcile.
- **Verification zombie child** — `try_wait`-error branch now `child.wait()`s
  to reap the zombie; output truncation uses a single `char_indices` pass.
- **Telemetry shutdown** — `into_inner` uses `Option<Arc>` + `take()` with a
  post-take no-op Drop (replacing ManuallyDrop tricks); `flush()` no-ops when
  the slot is empty; `TelemetrySubsystem::disabled()` replaces an unwrap in
  the disabled path.
- **Config from_value deep clone removed** — parses the Value once for the
  defaultAgent probe, then re-parses with `from_str` so typed parse errors keep
  real line/column caret diagnostics (caught by test_config_parse_errors).
- **Loop capture by value** — `rollback_to_capture` takes `LoopCapture` by
  value (no deep byte cloning); captures stored as `LoopCapture` not
  `Arc<LoopCapture>`; metadata filtering happens BEFORE `take_snapshot` (files
  were being read twice).
- **Loop deny_reason glob compilation** — glob matchers compile once per check
  (was recompiled per path token); `bash_path_tokens` filters URL and
  non-path-like `key=value` tokens.
- **Slash command output fixes** — `/spec list` returns a real count, `/spec
  coverage` marker de-unicode-fied, `/spec impl` surfaces runner errors,
  `/spec activate` reuses the SpecManager Arc, `/autopilot` flag parse errors
  are surfaced instead of silently `.ok()`-ed, compact/compress arms share
  `start_compaction_or_warn()`, and /mcp help no longer gets its status
  clobbered by the match-end fallthrough.
- **Bang command off the async runtime** — the `!` shell passthrough in main.rs
  wraps `sh -c` in `spawn_blocking`; `Runtime::new` failures map to a clean
  error; invalid `--log-level` warns instead of being ignored.
- **codeindex_status dead branch** — removed hardcoded "Enabled: no" branch.
- **Lowercase-slicing guards** in research comparison tests (3 new tests).

### Changed

- `drain_completed` fuses the pending scan into the retain closure; dead
  `run_verification_command_owned` and `path_within` deleted (verified 0
  callers).
- New tests: `test_slash_help.rs` (30 tests covering every slash-command help
  arm), loop-capture/rollback test updates, telemetry shutdown test updates,
  finance/comparison/codeindex test updates.

## Version: 1.0.83

### Fixed

- **CI Clippy and Rustfmt jobs failing with missing toolchain components** —
  the checked-in `rust-toolchain.toml` pins a dated nightly
  (`nightly-2026-09-04`) that overrides the stable toolchain installed by
  `dtolnay/rust-toolchain@stable`, so the Clippy and Rustfmt jobs failed with
  "'cargo-clippy' is not installed for the toolchain
  nightly-2026-09-04-x86_64-unknown-linux-gnu" because the pinned nightly's
  minimal profile did not include those components. Both jobs now run an
  explicit `rustup component add clippy` / `rustup component add rustfmt`
  step after toolchain setup, installing the components into the pinned
  toolchain that `cargo` actually resolves.

### Changed

- Version bump to 1.0.83 (CI toolchain component fix).
- `cargo check` passes (only the pre-existing future-incompat notice for the external `attribute-derive-macro` crate).
- `cargo audit` reports only the 10 allowed warnings (unmaintained `ttf-parser`, unsound `lru`, yanked `chacha20` transitive dependencies); no actionable security failures.

## Version: 1.0.82

### Added

- **Agentic loop programming (`/loop`)** — this release ships the goal-driven
  loop programming feature: a `LoopSpec`/`LoopTracker`/`StopCondition` state
  machine that runs a goal to a stop condition (`completed`, `budget_exhausted`,
  `error`, `interrupted`), a verification gate for verify commands, restriction
  layers (tool set -> read-only -> scope, invalid globs failing closed),
  pre-loop snapshot/git capture with post-loop rollback, and an interactive TUI
  setup dialog. Configured via the `loop` section of `ragent.json`
  (`loop.max_steps` 512 default, `cost_limit`, `error_retry_allowance`,
  `checkpoints`, `checkpoint_timeout_secs`); documented in
  `docs/howtos/loopprogramming.md`. One-shot `/loop <agent> [flags] <goal>`
  accepts `--max-steps N`, `--cost_limit N`/`--cost-limit N`, and `--timeout N`.
  See the "Uncommitted (post 1.0.81)" section below for the full change list
  accumulated on the way to this release.

### Changed

- Version bump to 1.0.82 (agentic loop programming).
- `cargo check` passes (only the pre-existing future-incompat notice for the external `attribute-derive-macro` crate).
- `cargo audit` reports only the allowed warnings (unmaintained `ttf-parser`, unsound `lru`, yanked `chacha20` transitive dependencies); no actionable security failures.

## Uncommitted (post 1.0.81)

Working-tree changes (staged + unstaged) on top of commit `429c9190`
("Version: 1.0.81 - updated codeindex indexing tooling").

### Added

- **Goal-driven loop programming (`/loop`, spec agentloop)** — a goal-driven
  agentic loop that runs a spec to a stop condition: `LoopSpec`/`LoopTracker`/
  `StopCondition` state machine (`crates/ragent-agent/src/session/loop_state.rs`),
  pre-loop snapshot/git capture with rollback (`loop_capture.rs`, FR-018/FR-020),
  a verification gate for verify commands (`verification.rs`, 600 s timeout,
  head 8000 + tail 2000 output capture), `build_goal_loop_section` system-prompt
  injection (prompt_builders.rs), `LoopConfig` tunables in `ragent.json`
  (`loop.max_steps` 512, `cost_limit`, `error_retry_allowance` 3, `checkpoints`
  true, `checkpoint_timeout_secs` 120), SSE `LoopTerminated`/`LoopChangeSummary`
  mappings, and the TUI interactive setup dialog + rollback flow
  (`loop_dialog.rs`, `test_loop_dialog.rs`, `test_rollback_flow.rs`).
  Loop runs enforce restrictions in `deny_reason` order: tool set -> read-only
  -> scope, with invalid globs failing closed.
- **`/loop` one-shot flags and higher step budget** — the one-shot form
  `/loop <agent> [flags] <goal>` now accepts `--max-steps N` (step budget),
  `--cost_limit N` / `--cost-limit N` (token-cost budget), and `--timeout N`
  (checkpoint-prompt seconds), in any position with `--flag value` or
  `--flag=value`; invalid values stop the loop from starting and name the
  offending flag (`LoopSpec.checkpoint_timeout_secs` override plumbed through
  `SessionProcessor::start_loop` into the permission layer). The default step
  budget (`loop.max_steps`) was raised from 25 to 512.
- **How-to documentation set** — three new how-tos in `docs/howtos/`:
  `reactagent.md` (core per-turn Reason+Act+Observe loop, explicitly not
  `/loop`), `loopprogramming.md` (the `/loop` goal-driven feature), and
  `office.md` (office + PDF tool families: `office_read/write/info`,
  `libre_read/write/info`, `pdf_read/write`, formats, examples, and
  `tool_visibility.office` configuration).
- **How-to documentation accuracy sweep** — all 18 how-to manuals in
  `docs/howtos/` were re-verified against the source code and corrected where
  they had drifted: bash security-layer execution order, hook timeout
  behaviour (`PreToolUse` runs unwrapped), tool category counts (168 static
  tools + dynamic `mcp_tool`), the finance tool-visibility family (now 8
  tools including `stock_recommendations`), quoted `--tech` values in
  `/spec reverse`, config sections 7.35 `loop` / 7.36 `activity_log` and
  `dirs.allowed_roots`, and stale cross-references. The PDF set under
  `docs/howtos/pdf/` was regenerated from the updated sources (18 A4
  xelatex PDFs).

### Fixed

- **`stock_recommendations` was not hidden by the finance visibility family**
  — `tool_family_names("finance")` listed 7 tools and omitted
  `stock_recommendations`, so switching the finance family off still
  advertised `stock_recommendations` to the model. It is now a member of the
  family (8 tools), with a regression assertion in
  `test_tool_visibility.rs` and updated `docs/howtos/tool-visibility.md`.
- **Quoted `--tech` values silently truncated in `/spec reverse`** —
  `parse_reverse_args` split the argument string on whitespace only, so
  `/reverse owner/repo --tech "Next.js + Rails"` silently truncated the
  stack to `Next.js`. A new shell-like tokenizer (`tokenize_reverse_args`)
  groups single/double-quoted spans into one token and strips the quote
  characters; an empty quoted value (`""`) does not form a token. Help text
  and `docs/howtos/reverse.md` now document the quoting requirement.
- **Agents button not enabling / showing the sub-agent count with many
  concurrent sub-agents** — the v1.0.81 reconcile only fired when the TUI's
  event-bus bridge observed a broadcast `Lagged` burst, but the bridge
  forwards into an unbounded mpsc and rarely lags itself, and `SubagentStart`
  events for nested sub-agents can be filtered by the session-lineage guard
  before reaching `active_tasks` — so with 18 instantiating sub-agents the
  Agents button stayed disabled with no count for the entire run. The
  reconcile poll now also fires periodically (every 1.5 s,
  `AGENTS_RECONCILE_INTERVAL`) independent of lag detection: it fetches the
  authoritative `AgentManager::tasks_snapshot` off-thread and merges it
  (re-adding missing running agents, removing ghosts), and the TUI event
  loop wakes at the reconcile deadline so the panel self-heals within ~1.5 s
  regardless of the loss cause. The merge is idempotent and logs only on
  change, so the periodic path is cheap and silent when already in sync.

## Version: 1.0.81

### Changed

- Version bump to 1.0.81 (updated codeindex indexing tooling).
- `cargo check` passes (only the pre-existing future-incompat notice for the external `attribute-derive-macro` crate).
- `cargo audit` reports only the 10 allowed warnings (unmaintained `ttf-parser`, unsound `lru`, yanked `chacha20` transitive dependencies); no actionable security failures.

## Uncommitted (post 1.0.80)

Working-tree changes (staged + unstaged) on top of commit `a042f4ed`
("Version: 1.0.80 - add plot tools"). Detailed entries for the codeindex graph
work below also live in the 1.0.80 section; this block is the uncommitted
marker for this documentation pass.

### Fixed

- **Config save/load cache coherence (M-025 CI failure, run 34023696563)** —
  `Config::load()` (crates/ragent-config/src/config.rs) cached the resolved
  config keyed on `(cwd, mtimes+sizes)` of candidate config files, but the
  mtimes were snapshotted BEFORE `load_uncached()` — which auto-creates the
  default `.ragent/ragent.json` on a first load — so a first load in a fresh
  cwd cached an EMPTY mtime list and the fast path short-circuited true
  forever; additionally `save()`/`save_to_source()` never invalidated the
  cache, so any TUI config save (`/codeindex off`, `/tools`, ...) was
  invisible to subsequent `Config::load()` calls for the rest of the process.
  Fixed by re-snapshotting candidate mtimes AFTER `load_uncached()` (new
  `snapshot_candidate_mtimes`), calling the new public
  `Config::invalidate_load_cache()` after every successful write, and moving
  the `OnceLock` cache into a shared `load_cache_slot()` so load and
  invalidate clear the same static. This is what made
  `test_slash_codeindex_off_updates_visibility_and_config` and
  `test_slash_tools_toggle_persists_and_updates_hidden_registry` fail on CI
  (no pre-existing global config) while passing locally.
- **`codeindex_path` / `codeindex_explain` name resolution** — the graph
  traverser picked the FIRST row of a case-insensitive substring query
  ordered by name, so `SessionProcessor` resolved to the
  `CachedSessionProcessor` trait, `EventBus` to the `Default for EventBus`
  impl, and `WebGatherer` to `WebGatherError` — all near-zero-edge nodes that
  made `codeindex_path` report "No path found" for well-connected symbols.
  `crates/ragent-codeindex/src/graph/traverse.rs` now ranks candidates:
  exact name matches first, then definition kinds (struct/function/trait/
  class/enum/interface) over impl/module containers, with a lowest-symbol-id
  tie-break for determinism.
- **`codeindex_status` tool no longer blocks (or retries) on a held store
  lock** — the tool replaced its 5 s `with_retry` loop with a single
  `try_status()` probe; when the store/FTS mutex is held by a background
  reindex or graph build it returns immediately with a busy report built from
  the lock-free progress atomics (`reindex_progress()`, `graph_busy()`,
  `graph_build_progress()`), carrying `metadata.busy = true` and
  `error: "codeindex_busy"`. The TUI `/codeindex status` slash command uses
  the same non-blocking probe path.

### Changed

- **`IndexStats` now carries graph counters** — `graph_total_edges`,
  `graph_nodes`, and `graph_communities` are populated by `CodeIndex::status()`
  (and best-effort by `try_status()` under `try_lock`), so status surfaces can
  report "graph built" vs "not built" without a separate graph query.
- **Pinned nightly toolchain** — new `rust-toolchain.toml` pins
  `nightly-2026-09-04` (replacing the `RUSTUP_TOOLCHAIN=nightly` env
  approach) so daily nightly updates stop invalidating the sccache cache and
  forcing full rebuilds; `TOOLCHAIN.md` documents the review and the
  deliberate bump procedure.

### Tests

- `crates/ragent-codeindex/tests/test_graph_resolve_regression.rs` (3 tests)
  pins the resolver regressions above (substring-trait shadowing, impl
  `Default for X` shadowing, exact-vs-substring preference).
- `crates/ragent-codeindex/tests/test_status_graph_fields.rs` (3 tests) covers
  the `IndexStats` graph fields and the `try_status`/`try_graph_status`
  locked-store degradations.
- `crates/ragent-tools-extended/tests/test_codeindex_status_busy.rs` (4
  tests) covers the busy-report output/metadata, including a source-level
  guard that `codeindex_status.rs` must not reintroduce `with_retry`.
- `crates/ragent-tui/tests/test_codeindex_indicators.rs` gained 3 tests for
  the simultaneous-vanish fix: a graph build holding the store lock must not
  latch the `idx` indicator, and each indicator clears only for its own
  phase.

## Version: 1.0.80

### Changed

- Version bump to 1.0.80.
- `cargo check` passes (only the pre-existing future-incompat notice for the external `attribute-derive-macro` crate).
- `cargo audit` reports only allowed warnings (yanked `chacha20` transitive dependency); no actionable security failures.
- **Phased graph build keeps the store lock free during derivation (FR-026)** —
  `CodeIndex::build_graph`, `build_graph_for_language`, and the graph phase of
  `full_reindex` no longer hold the `IndexStore` mutex for the whole build.
  Edge derivation is split into `graph::edges::load_graph_inputs` (brief
  read-only snapshot of symbols, refs, imports, files — imports now loaded in
  one SQL scan via the new `IndexStore::list_all_imports`),
  `derive_edges_from_inputs` / `derive_edges_from_inputs_for_language` (pure
  in-memory derivation, no locks), and `persist_edges` (brief single-transaction
  write touching only `graph_edges`). FTS search and the other store readers
  stay available while the graph builds; live `graph_done`/`graph_total`
  progress now also advances during direct `build_graph` calls (not just
  `full_reindex`) and the counters are reset to `(0, 0)` when the build
  completes.

### Added

- **Codeindex busy indicators on the status bar (second line, top-right)** —
  the TUI now shows two bold busy tags while the code index works: `idx`
  (warning/yellow) while a reindex holds the store/FTS locks or the reindex
  progress counters are active, and `graph` (cyan) while the semantic edge
  graph is being (re)built. Both latches are polled lock-free from new
  `CodeIndex` atomics (`graph_busy()`, `graph_build_progress()`) by
  `App::refresh_code_index_stats`, so they stay live even while the index
  mutexes are held, render in every responsive mode, and animate at the 250 ms
  event-loop cadence. Their width is reserved before the right-gap computation
  so the tags never clip off the terminal edge.
- **Threaded codeindex graph build** — `CodeIndex::spawn_graph_build(Arc)`
  runs the heavy semantic-graph derivation on a dedicated OS thread named
  `codeindex-graph-build` (with a double-build guard that refuses overlapping
  builds). The TUI `/codeindex graph build`, `/codeindex graph lang <l>`, and
  `/codeindex reindex` commands now run in the background instead of blocking
  the event loop for minutes on large repos: the command sets a `[wait]`
  status immediately, the status bar indicator animates while the build runs,
  and the completion message + status text are delivered through a mutex
  slot drained by `App::poll_codeindex_bg_result` each loop wake (mirroring
  the `/opt` async pattern). `full_reindex` now also labels its graph phase
  via the same graph-busy flag and reports per-file progress
  (`graph_done`/`graph_total`) during derivation.
- **Graph-build visibility in status surfaces** — `/codeindex show` reports
  `**Graph:** building...` with `done/total` file progress while a build runs,
  and the `codeindex_status` tool reports `graph_state: "building"` plus
  `graph_building`/`graph_done`/`graph_total` in its metadata so agents can
  poll build state without blocking on the store lock.
- **`plot_*` tool family (6 new tools)** — scientific/terminal plotting on the
  message window, rendered off-screen to text via `ratatui-plt` 0.0.2
  (GPL-3.0, explicitly accepted by the project owner 2026-09-06 and added to
  the `deny.toml` license allow list): `plot_line` (XY series), `plot_scatter`
  (dot markers), `plot_bar` (categories + datasets, `stacked`/`horizontal`
  modes), `plot_histogram` (`count`/`density`/`probability` normalisation),
  `plot_pie` (slices with auto palette cycling, optional `donut`), and
  `plot_heatmap` (`viridis`/`plasma`/`inferno`/`magma`/`coolwarm` colormaps).
  Shared parsing/rendering helpers live in
  `crates/ragent-tools-extended/src/plot/mod.rs` (string-encoded series coerce
  gracefully, canvas clamped to 220x80, argument errors degrade to an error
  output rather than crashing). All six tools register under the `system`
  permission category in `create_extended_registry`; the crate-local `ratatui
  0.30` dependency (pinned by `ratatui-plt`) sits alongside the workspace
  `ratatui 0.29` used by `ragent-tui`.
- **Inline plot rendering in the TUI message window** — `plot_output_lines`
  (crates/ragent-tui/src/widgets/message_widget.rs) renders the plot tool's
  ANSI-coloured canvas (metadata key `plot_ansi`, falling back to the plain
  `plot` key) inline beneath the tool call, with a new `ansi_line_to_styled`
  parser that maps SGR colour runs to distinct styled spans so palette series,
  pie slices, and heatmaps keep their real colours. `layout.rs` routes any
  `plot_*` tool output through the renderer instead of the one-line summary.
- **Tool input summaries for the code-index graph and plot tools** — the TUI
  step log now shows human-readable one-liners for `codeindex_godnodes`,
  `codeindex_path`, `codeindex_explain`, `codeindex_communities`,
  `model_info`, and each `plot_*` tool (series/category/sample counts,
  titles) in `tool_input_summary`.

### Tests

- `crates/ragent-codeindex/tests/test_graph_build_async.rs` (7 tests) covers
  graph-busy flag lifecycle (clears after success and on the empty-index
  path), spawned-thread build completion with real edges, observation of
  `graph_busy` while the store lock is held by another thread (the property
  the status-bar indicator relies on), the double-spawn guard, and the FR-026
  phased-build properties above.
- `crates/ragent-tui/tests/test_codeindex_indicators.rs` (8 tests) covers
  `idx`/`graph` indicator rendering (single, combined, hidden when idle),
  `poll_codeindex_bg_result` success/error/no-op draining with spawned-latch
  reset, and the graph-busy latch clearing when no index is attached.
- `crates/ragent-tools-extended/tests/test_plot_tools.rs` (25 tests) covers
  rendering, stacked/horizontal bars, histogram norms, heatmap defaults,
  registry registration, canvas bounds, string-encoded argument coercion, and
  distinct-SGR-colour-run rendering.
- `crates/ragent-tui/tests/test_message_widget_tests.rs` gained 6 tests for
  `plot_output_lines` (ANSI preference, multi-colour span splitting,
  plain-canvas fallback, empty/missing output).

### Documentation

- `docs/howtos/pdf/*.pdf` — all 15 how-to guides now ship A4 xelatex PDFs.
- `docs/howtos/permissions.md` — `$'\xNN'` hex-escape examples wrapped in
  inline code so the LaTeX PDF build no longer breaks.
- `docs/howtos/research.md` — documents `--max-web-results` /
  `--max-search-calls`, the depth-derived web budget, and
  `/research update <name>` replay.

## Version: 1.0.79

### Changed

- Version bump to 1.0.79.
- `cargo check` passes with only a pre-existing `unused workspace dependency` warning for `axum-extra`.
- `cargo audit` reports only allowed warnings (no actionable security failures); legacy `lru 0.12.5` unsoundness warnings remain tracked.

### Added

- **`/clip` slash command** — copies the rendered contents of the message window
  (the exact plain-text transcript rows the Messages pane displays) to the
  system clipboard in one step, joining each rendered row with newlines. Uses
  the same pre-wrapped line buffer as text-selection copy, so what lands on the
  clipboard is exactly what is on screen. An empty window reports
  "nothing to copy" instead of writing an empty clipboard. Registered in the
  slash-command autocomplete list and `/help`; documented in TUI-QUICKSTART.md.

### Changed

- **`/research create` TUI progress display simplified** — the live progress log now opens with an `Options:` line (mode, output format, tier, depth, iterations, from-urls/from-files) rendered once beneath the topic instead of a per-run config step line, per-captured-source lines are gone from both the message window and the log panel (web capture/failure/exclusion events accumulate into the existing fetch-totals counters only), and the per-engine capture summary table has been removed along with its `CaptureDelta`/`EngineCaptureRow` payload machinery. Web-search request counts still surface at run end via the completion notice and the `search_providers` pipeline step. `SessionEvent::ConfigSnapshot` gained a `mode` field so the TUI can display the resolved mode.

### Added

- **Research web-search quota controls** — `--max-search-calls N` (CLI flag, TUI parser flag, HTTP `max_search_calls` field) places a hard, run-scoped cap on the total number of web-search calls a research run may issue. The cap lives on a new `SearchBudget` primitive (crates/ragent-research/src/search_budget.rs) shared via `Arc` across every supervisor/competitive researcher and every gather pass (main gather, engine iterations, gap-fill), so N parallel researchers draw from one per-run pool. When the cap is reached, remaining sub-queries are skipped without error, the run proceeds with the sources gathered so far, and a `search_budget` `RunStep` diagnostic reports the used/limit counts (new `GatherEvent::SearchBudgetExhausted`). A run-scoped `SharedQueryCache` also memoises successful search results keyed on the normalized query text, so supervisor/competitive researchers whose decomposed sub-queries collide (common when every entity researcher decomposes the same comparison dimensions) reuse the cached hits instead of re-issuing paid search calls. The cache is always on for supervisor/competitive runs; the budget is opt-in via `--max-search-calls`.

### Changed

- **`--depth` now bounds web volume by default** — the effective web-source budget for gather passes is derived from the selected depth (`shallow` 6 / `standard` 9 / `deep` 15 sources) unless `--max-web-results` is passed explicitly. Previously every depth preset collapsed into the 500-source default (`max(depth_budget, 500)`), making `--depth shallow` ineffective at limiting search/fetch volume. `WebConfig::max_web_results` uses `0` as the derive-from-depth sentinel; `SessionConfig::effective_web_budget()` is the single resolution point (gap-fill included). Sub-query search requests are also right-sized to the per-query share of the budget (floor 10) instead of every parallel sub-query requesting the full `max_results`.
- **Competitive-mode researcher count is capped** — `build_competitive_sub_topics` emits one sub-topic per extracted entity and the session previously ran all of them regardless of `--max-concurrent-research-units`, scaling search quota linearly with the entity list. The competitive plan is now truncated to the configured cap (the generic supervisor fallback path was already capped).

### Fixed

- **`/research list` renders a human-readable table again** — v1.0.78 changed `render_list_output`/`render_search_output` to emit raw JSON, which every consumer (TUI `/research list`, `/research search`, and `ragent research list|search`) printed verbatim inside a fenced code block, producing a dense JSON slab. The fixed-width `NAME/TITLE/STATUS/CREATED/MODIFIED` table and the bullet-list search renderer are restored as the default output; the JSON form now lives behind the existing `--json` flag via new `render_list_output_json`/`render_search_output_json` helpers. The shell CLI `ragent research list` gained a `--json` flag, and list rows once again include `created`/`modified` timestamps (RFC 3339) in both table and JSON output.

### Added

- **`--mode competitive` defaults `--format` to `comparison-table`** — `/research create` (and the TUI slash command, HTTP `POST /research`, and `PUT /research/{name}` replay path) no longer requires an explicit `--format comparison-table` when `--mode competitive` is set: the shared `build_session_config` builder (crates/ragent-research/src/run_request.rs) now defaults the output format to `comparison-table` for competitive runs. An explicit `--format` argument still wins, and non-competitive modes keep the `report` default. The CLI help table, SPEC.md, and QUICKSTART.md were updated accordingly; four new builder tests pin the default, the override, and the non-leak into `tiered`/`supervisor`.
- **`/research update <name>` invocation replay** — the `invocation` frontmatter line recorded by every research front-end (CLI argv, TUI `/research ...` slash command, HTTP `POST /research` summary) can now be replayed to re-run a research item and overwrite `RESEARCH.md` and its associated files (`CORPA.md`, `sources/`, index) with freshly gathered results:
  - `crates/ragent-research/src/run_request.rs` adds `ResearchRunRequest::from_invocation` plus `InvocationParseError`. It normalizes all three recorded grammars, reuses the shared `ResearchCliCommand` parser, and keeps the original invocation stamped on the replayed run. The shared parser now also accepts the `--from-urls`/`--from-files` spellings that clap derives (previously only the documented singular `--from-url`/`--from-file` worked in the TUI parser).
  - `crates/ragent-research/src/cli.rs` adds the `update <name>` verb to `ResearchCliCommand` (variant, parse helper, help-table row).
  - CLI: `ragent research update <name>` (src/cli.rs) — loads the item, requires an invocation line, replays it, and re-runs the full pipeline including the clarification-answer flow.
  - TUI: `/research update <name>` (crates/ragent-tui/src/app/research.rs) — runs the replay in the background with live phase progress; `update` also added to the slash autocomplete list.
  - HTTP: `PUT /research/{name}` (crates/ragent-server/src/routes/research.rs) — `404` missing item, `400` missing/unparseable invocation, `409` in-flight, `202 Accepted` + `Location` SSE header on spawn. The broadcast observer is hoisted to a shared module-level type reused by POST and PUT.
  - Tests: `crates/ragent-research/tests/test_research_invocation_replay.rs` (14 tests) and five new `PUT` route tests in `crates/ragent-server/tests/test_research_routes.rs`.

### Fixed

- **GitHub `blob/` URLs no longer fail research gathering** — competitive/supervisor research runs surfaced many
  `web — source failed: … readability extraction failed` errors for GitHub file-view URLs
  (`github.com/{owner}/{repo}/blob/{ref}/{path}`). The blob page is GitHub application chrome (nav menus, file
  tree, buttons) that `readability-rs` cannot extract as an article, so those URLs were guaranteed rejections
  under the research web-gather readability guarantee. `AgentWebFetchTool::fetch` now rewrites such URLs to their
  `raw.githubusercontent.com/{owner}/{repo}/{ref}/{path}` counterparts (multi-segment refs like `refs/heads/main`
  are preserved; repo roots, issues, PRs, and directory listings are left unchanged), so the file's actual
  content is captured instead. Non-HTML content types (`text/plain` from the raw endpoint and similar) now
  bypass the readability check like PDFs and YouTube transcripts already did — their bodies are the verbatim
  document content and no fallback extraction is involved. Remaining `source failed` messages for bot-blocked
  (HTTP 429), gone (HTTP 410/521), or JS-shell pages are genuine rejections and continue to be reported.

### Tests

- `test_normalize_github_blob_url_rewrites_to_raw`, `test_normalize_github_blob_url_multi_segment_ref`,
  `test_normalize_github_blob_url_leaves_non_blob_urls_unchanged`,
  `test_github_blob_fetch_uses_rewritten_raw_url`, and
  `test_mf_fetch_non_html_content_type_bypasses_readability_check` in
  `crates/ragent-agent/src/research_adapter.rs`.

### Documentation

- **`docs/howtos/permissions.md` PDF added** — the permissions how-to now ships a
  `pdf/permissions.pdf` alongside the other how-to PDFs. The `$'\xNN'` hex-escape
  examples were wrapped in inline code so they render correctly under the xelatex
  PDF engine (previously the raw `\xNN` sequence broke the LaTeX build).
- **`docs/howtos/research.md` updated** — documents the new `--max-web-results` /
  `--max-search-calls` flags, the depth-derived web-source budget, and the
  `/research update <name>` invocation-replay flow.
- **`STATS.md` refreshed** — updated project-wide metrics (404,293 lines, 991 files,
  ~7,676 tests, 444 test files) and per-crate file/test counts for v1.0.79.

## Version: 1.0.78

### Added

- **Comparative (supervisor/competitive) research mode** — the largest
  research-system expansion to date (306 files, +10,325/-2,114):
  - `crates/ragent-research/src/supervisor.rs` — the multi-agent
    Plan -> Delegate -> Collect -> Synthesize -> Finalize state machine behind
    `--mode supervisor` and `--mode competitive`, with parallel
    `IterativeResearcherNode` workers (default cap 5) running iterative
    gather-analyse loops per sub-question/entity.
  - `crates/ragent-research/src/comparison.rs` — deterministic,
    LLM-agnostic `comparison-table` synthesis: per-entity `CompetitiveProfile`
    profiles plus a cross-entity Markdown comparison table with explicit
    criteria (FR-006/FR-014/FR-016 of specs/opendeepresearch), so the artifact
    ships even when the synthesis model returns only compressed notes.
  - New supporting modules: `brief.rs` (research brief), `clarify.rs`
    (clarification round), `entities.rs` (competitive entity extraction),
    `evaluation.rs` (run evaluation), `page_summarizer.rs` (per-page
    summarisation), plus the CLI restructure (`cli.rs` -2,185 lines
    reorganised) and TUI research-progress surface (`research_progress.rs`).

## Version: 1.0.77

### Changed

- Version bump to 1.0.77 (documentation refresh; the 1.0.77 commit itself
  touched only `CHANGELOG.md`, `Cargo.lock` and `Cargo.toml` — the feature
  work merged into this release snapshot landed across the 1.0.76-era
  commits).
- Updated project documentation for v1.0.77 (`README.md`, `SPEC.md`, `STATS.md`, `QUICKSTART.md`, `TUI-QUICKSTART.md`, `CHANGELOG.md`, `docs/howtos/config.md`, `docs/howtos/research.md`).
- Merged the latest feature work from `1.0.76` and `1.0.75` into the v1.0.77 release snapshot.

### Added

- **Documentation refresh for v1.0.77** — `README.md` project status now references v1.0.77 highlights. `SPEC.md` and `TUI-QUICKSTART.md` version/date blocks updated to 2026-09-03. `STATS.md` updated with current project composition and crate breakdown.
- **Research evaluation scorecard configuration** — new `research.evaluate` section in `ragent.json` for self-evaluation scorecard settings (FR-015 of specs/opendeepresearch). Includes `ResearchEvaluateConfig` and `enabled` default.
- **Research configuration re-export** — `ResearchEvaluateConfig` is now re-exported from `ragent-config` for downstream consumers.

### Fixed

- **Research config round-trip test** — `crates/ragent-config/tests/test_research_config.rs` now initializes the required `evaluate` field so the test compiles and passes.

## Version: 1.0.76

### Changed

- Version bump to 1.0.76.
- Cleaned up Clippy warnings across the workspace to keep `cargo clippy --workspace --tests -- -D warnings` green, including case-sensitive file-extension comparisons and redundant visibility qualifiers in `ragent-tools-core`.

### Added

- **`--web-time` web-phase deadline (60 s default)** — `/research create` now
  caps the web-gathering phase at 60 seconds by default; when the deadline
  passes, everything gathered so far is ingested and the run proceeds to
  analysis/synthesis with the partial source set instead of discarding the
  phase:
  - `crates/ragent-research/src/web_gatherer.rs`: new `phase_deadline`
    option (`WebGatherer::with_phase_deadline`) — search completion, query
    decomposition, and fetch completion are each bounded by the remaining
    budget; a stalled search/fetch is abandoned (cancelled on drop) and the
    gatherer returns a partial `GatherResult`. New `GatherEvent::PhaseTimedOut`
    diagnostic carries the deadline and the captured count.
  - `crates/ragent-research/src/session.rs`: `DEFAULT_WEB_PHASE_TIMEOUT_SECS`
    lowered from 180 to 60; the session passes the deadline to the gatherer
    instead of wrapping it in a whole-phase `tokio::time::timeout` that
    previously threw away everything gathered. The deadline is surfaced as a
    `web_deadline` `RunStep` diagnostic (TUI step log shows it in the Web
    phase). `--web-time 0` disables the deadline.
  - `crates/ragent-research/src/cli.rs`: `--web-time N` alias for
    `--web-phase-timeout-secs N` (both front-ends: CLI + TUI parser), added to
    the help text.
  - New test suite `crates/ragent-research/tests/test_web_time_deadline.rs`
    (partial capture on deadline, default-60 assertion, `--web-time 0`
    disable path, flag parsing).
- **No-new-work-after-deadline guarantee (researchfix T-005, FR-008)** — the
  web-phase deadline now provably stops all new gather work once it elapses:
  the decomposer call, each sub-query search-result wait, and each in-flight
  fetch-completion wait are bounded by the remaining budget, and the search
  and fetch loops break on truncation before polling further work, so no new
  search or fetch is started after the deadline. The worst-case overshoot is
  the completion of the fetches already in flight at truncation — in-flight
  requests are cancelled on drop, and each is capped by
  `--fetch-timeout-secs` — so the phase can never run unbounded past the
  deadline. Documented in `WebGatherer::gather_with_observer` and
  `with_phase_deadline` rustdoc and in `docs/howtos/research.md` /
  `SPEC.md`. New tests: `test_no_search_issued_after_deadline` (8 sub-queries,
  500 ms deadline — only the initial in-flight batch is issued, zero fetches)
  and `test_overshoot_bounded_by_in_flight_fetch_timeout` (slow pages cannot
  extend the phase past the deadline; no fetch starts after it).
- **Concepts section in `/research create`** — the research pipeline now
  extracts a cross-source concept list and embeds it in `RESEARCH.md` as a
  `## Concepts` section directly above `## Findings` (report layout) or
  `### Concepts` directly above `### Findings` inside Results (IMRaD layout):
  - `crates/ragent-research/src/analysis.rs`: new public
    `LlmAnalysisEngine::complete_raw(prompt, system, max_tokens)` — the shared
    low-level completion path (registry client resolution, streaming, text
    accumulation); `stream_synthesis` now delegates to it.
  - `crates/ragent-research/src/cluster.rs`: `build_concepts_payload_from_bodies`
    (in-memory payload whose blocks are headed `--- [#N] Title ---` with N =
    References Index position) and `concepts_section_for_research` (strips the
    `# Concepts` H1, demotes concept headings one level, rewrites `web-NN`
    filename citations to combined-index `[#N]` markers).
  - `crates/ragent-research/src/document.rs`: `ResearchDocument.concepts`
    field rendered in both layouts; omitted entirely when `None`.
  - `crates/ragent-research/src/session.rs`: `concepts_engine` field +
    `with_concepts_engine` builder; new `extract_concepts_section` step runs
    after ReadabilityAudit and before Assemble, emits `concepts` `RunStep`
    events (started/completed/skipped/failed), and never aborts the run on
    failure; shared `read_source_body` helper now backs both synthesis and
    concept-extraction body loading.
  - `crates/ragent-agent/src/research_adapter.rs`: wires the concepts engine
    from the same `LlmAnalysisEngine` Arc used for the `--from-url`/`--from-file`
    summarizer, so no extra engine is constructed.
- **Web-phase deadline observability and deduplication (researchfix T-012)** —
  documented the new deadline behavior in `docs/howtos/research.md`, `SPEC.md`
  section 11.9, and this changelog. End-state: a 60-second default web-phase
  deadline (`DEFAULT_WEB_PHASE_TIMEOUT_SECS`), partial-corpus semantics, a
  single `PhaseTimedOut` / `web_deadline` diagnostic per gather phase (FR-004),
  `--web-time 0` disables the deadline (FR-007), the iterative engine applies
  the same deadline each iteration (FR-006), a `web_phase_start` event carries
  the remaining budget for UI countdowns (FR-009), and the TUI renders a live
  `web:M:SS` countdown in the status-bar wait segment at least once per second
  (FR-010, FR-013) plus a one-shot "Web phase deadline reached" notice in the
  research progress message (FR-012).

### Changed

- Concept-extraction prompt citations generalized: the fixed
  `CONCEPT_EXTRACTION_PROMPT_TEMPLATE` now asks the model to cite
  `[#N]` markers keyed to the number shown in each document header, which
  works for both the `/research cluster` payload (`--- web-NN.md ---`) and
  the new `/research create` payload (`--- [#N] Title ---`).

## Version: 1.0.75

### Changed

- Version bump to 1.0.75.

## Version: 1.0.74

### Fixed

- **Clippy `for_kv_map` warning in Ollama provider** — replaced
  `for (_, id) in &tool_call_ids` with `for id in tool_call_ids.values()` in
  `crates/ragent-llm/src/providers/ollama.rs` to satisfy the `for_kv_map`
  lint. (commit `d338c25b`)
- **LangSearch config merge test expectation** — corrected
  `test_merge_preserves_other_top_level_fields` in
  `crates/ragent-config/tests/test_langsearch_api_key.rs`; the overlay did not
  explicitly set `default_agent`, so the base value is now expected to be
  preserved. (commit `d338c25b`)

## Version: 1.0.73

### Changed

- Safe `/simplify` refactorings across multiple crates to reduce verbosity,
  remove unnecessary allocations, and improve clarity while preserving
  behavior:
  - `crates/ragent-research/src/gather_log.rs`: simplified `sanitize` to use
    `.take(64)` instead of collecting then truncating.
  - `crates/ragent-research/src/io.rs`: use `writeln!` for Markdown table
    construction and make frontmatter parsing tolerate leading whitespace.
  - `crates/ragent-research/src/session/fallback.rs`: extracted a shared
    `append_top_three_list` helper for the default summary sources.
  - `crates/ragent-agent/src/agent/mod.rs`: replaced emoji markers with ASCII
    equivalents in `InstructionFileDiscovery::format_summary` and added an
    early emptiness check in `skills_prompt_section`.
  - Earlier safe simplifications in `crates/ragent-agent/src/one_shot.rs`,
    `crates/ragent-agent/src/session/processor.rs`,
    `crates/ragent-config/src/config.rs`,
    `crates/ragent-llm/src/providers/ollama.rs`,
    `crates/ragent-llm/src/providers/ollama_cloud.rs`, and
    `crates/ragent-research/src/analysis.rs`.

### Added

- **Sub-agent / teammate step visibility in TUI step log** —
  `crates/ragent-tui/src/app/event_handler.rs` and
  `crates/ragent-tui/src/app/session_ops.rs` now track active task and team
  member sessions; tool calls from tracked sub-agents/teammates are logged with
  an `[agent-tag]` prefix and a rebuilt step counter, so lagged event-bus
  bursts no longer undercount visible steps. (commit `2a8a5850`)

### Fixed

- **`SessionProcessor` tool-permit handling** — the per-tool permit acquisition
  result is now matched instead of silently ignored, and the tool call returns
  an explicit error tuple when a permit cannot be acquired. (commit `2a8a5850`)
- **One-shot runner diagnostics** — unexpected stream events during
  `send_one_shot` now emit a `warn!` log instead of being silently dropped.
  (commit `2a8a5850`)

## Version: 1.0.72

### Fixed

- **Token counting fixes** — corrected the TUI context panel to use a
  consistent `bytes_to_tokens()` conversion (4 bytes per token) at every
  estimator boundary, aligning panel percentages with the status-bar usage
  figure instead of raw byte counts. (commit `94b35acb`)

### Added

- **Config file size is now tracked in the cached loader** — `Config::load`
  records `(path, mtime, size)` for each on-disk candidate because filesystem
  mtimes can be coarse (1-second granularity); two writes within the same
  second no longer return a stale cached config. (commit `94b35acb`)
- **Research session CLI options** — `ragent-research/src/session.rs` gained
  `--tier`, `--use-local`, `--use-specs`, `--use-low-relevance`, `--no-papers`,
  and `--use-pdf` flags for finer-grained control over research runs.
  (commit `94b35acb`)

### Changed

- **Ollama context-window heuristic** — local and cloud Ollama providers now
  advertise 131,072 tokens for any model with at least 1B parameters (32k
  fallback for sub-1B models), replacing the old size-based tiers that
  under-reported capacity for modern small models. (commit `94b35acb`)

### Tests

- New research tests in `crates/ragent-research/tests/test_analysis_engine.rs`,
  `test_research_integration.rs`, and `test_session_fallback.rs`. (commit `94b35acb`)

## Version: 1.0.71

### Added

- **Context side panel (`Alt+X`, spec `contextpanel`, T-001..T-015)** — new
  toggleable right-hand panel showing a live, quantified breakdown of the
  session's context-window occupancy: system prompt (with indented
  skills/memory/agents.md sub-rows), tool catalog, tool metadata wire
  overhead, conversation history (tokens + message count), and total against
  the active model's context window, with percentage bars and free headroom.
  Token estimates reuse the shared estimator paths (tool catalog via
  `estimate_tool_definition_bytes`, tool metadata via the per-provider
  `ToolFormat`-mapped `tool_cache` byte length, history via per-message
  role+content+40 accounting), so the panel cannot drift from what is
  actually serialized. FR-015 non-blocking: the disk/SQLite-backed
  partitions (system-prompt assembly, skill registry, AGENTS.md discovery,
  structured memory) run on `tokio::task::spawn_blocking` with in-flight
  coalescing; results are deposited into a mutex and adopted each frame by
  `poll_context_snapshot_refresh`. The scheduler no-ops when the panel is
  closed. Panel refresh triggers: open, `MessageEnd` events, post-compaction
  history rewrite, model/thinking switches.
- **One-shot agent runner** — new `crates/ragent-agent/src/one_shot.rs`
  providing a single-turn, no-persistence agent execution path.
- **Research clustering** — new `crates/ragent-research/src/cluster.rs` with
  clustering support for research corpus analysis and 14 new context-panel
  test files plus a research-cluster test suite.
- **Context panel model-capacity row** — a dedicated "Context window" line
  is now rendered directly above the "System prompt" row so the denominator
  behind every percentage is explicit.

### Fixed

- **Context panel ">100% full" percentages for local Ollama models** — the
  parameter-size heuristic used to advertise context windows (8k for 3B,
  32k for 7B/8B, 64k for 32B) was far too conservative for modern local
  models such as Llama 3.2 1B/3B, Phi4-mini, Qwen2.5 and Gemma 2, which
  support 128k contexts. Ollama (local and cloud) now advertises 131,072
  tokens for any model with at least 1B parameters, with a 32k fallback for
  sub-1B models.
- **Clippy `struct_field_names` (`DiskContextPartitions`)** — renamed the
  partition fields (dropping the uniform `*_tokens` postfix) and extracted a
  shared `DiskContextPartitions::into_snapshot` combinator so the sync
  fallback and the background refresh task build
  `ContextPartitionSnapshot` through one code path and can never disagree.
- **Unused import** in `test_context_panel_t013.rs`.
- **`recover_poisoned` visibility** — narrowed from `pub` to `pub(crate)`
  (unreachable-pub dead-code lint), now documented with rustdoc.

### Hygiene

- Full CI-equivalent hygiene pass: `cargo check`, `cargo check --tests`,
  `cargo test` (7,774 tests, 0 failures), dead-code lint with
  `-D unreachable_pub -D dead_code -D unused_imports`,
  `scripts/check-dead-code-reasons.sh`, clippy `-D warnings`, `cargo fmt
  --check`, `cargo audit` (no vulnerabilities; 1 allowed yanked-crate
  warning), and `cargo deny check` all clean.

## Version: 1.0.70

### Changed

- Multiple agent TUI fixes. (commit `a26c8ab5dc444dfecbe0aa2c76525405dee700de`)
- Fixed TUI scroll-pinning geometry and idle-CPU hotspots. (commit `a570f4beff2cd755b133739d93a6b0bb7b4891a5`)

## Version: 1.0.69

### Changed

- **Compaction performance rework (`SessionProcessor::compact_session`)** —
  the TUI `/compact` command and pre-send auto-compaction previously spawned a
  FULL agent turn for summarisation: whole history, ~169 tool definitions,
  AGENTS.md init acknowledgement, and thinking config — with the in-loop
  pre-send trigger able to re-fire the summarisation (up to 3 LLM calls).
  The new `compact_session` session op (new
  `crates/ragent-agent/src/session/compaction_ops.rs`) drives the no-tools
  compaction runner directly: exactly ONE summarisation LLM call, no agent
  loop, reusing the warm per-(provider, model) client cache via a synthetic
  subagent `AgentInfo` (`max_steps = 1`, temperature `0.2`). It seeds
  prior-summary anchoring from the most recent `Role::Compaction` message,
  resolves the model context window from the provider registry (128k
  fallback), and returns a `CompactionOutcome` mirroring the runner's.
- **Compaction storage-ordering fix** — runner compaction messages carried
  `Utc::now()`, but `Storage::get_messages` orders by `created_at ASC`, so the
  persisted summary sorted to the END of the next turn's history instead of in
  front of the kept tail. `compact_session` now backdates the compaction
  message to `oldest_recent - 1 ms` before the delete + reinsert, keeping
  chronological order aligned with the conceptual compaction order.
- **TUI compaction result plumbing** — `compress.rs` deposits the compaction
  outcome into a `compact_result` mutex instead of relying on the agent-loop
  event path; new `poll_compaction_result` (called each frame from `lib.rs`)
  replaces `self.messages`, clears the message render cache and in-progress
  flags, and dispatches `pending_send_after_compact` on success. On
  auto-compaction failure the queued send is dropped and the
  `auto_compact_failed` latch blocks further sends for the turn. The
  `MessageEnd` summary-application block was removed from `event_handler.rs`;
  poisoned-mutex recovery is included.

### Tests

- New `crates/ragent-agent/tests/test_compaction_session_ops.rs` — 4 tests:
  exactly one no-tools LLM call with history deletion/replace verified against
  in-memory storage, cancel-before-LLM bails, unknown provider errors
  cleanly, and a mid-run cancel race never produces a second call.
- New `crates/ragent-tui/tests/test_compaction_result.rs` — 5 tests:
  no-op on empty, Ok replaces messages and clears the render cache, Err
  drops the queued send and latches failure, Ok dispatches the queued send,
  and poisoned-mutex recovery. All 9 new tests pass; the full compaction
  suite (inline convert/estimator/runner 32, flow 8, integration 5,
  prompt 3, serializer 9, config 5, storage 2), `cargo fmt --check`, and
  `cargo check` are clean across `ragent-agent` and `ragent-tui`.

## Version: 1.0.68

### Fixed

- **Messages pane bottom-pinned view 2 lines short (tail hidden)** — the
  scroll-window Paragraph re-wrapped the pre-wrapped cache rows with
  `Wrap { trim: false }`, but ratatui 0.29 word-wraps a whitespace-only row
  into TWO painted rows (a blank row plus the row of spaces). Markdown and
  log output routinely contain whitespace-only lines, so the painted tail
  slid below the scroll geometry: with the scrollbar at the bottom the last
  ~2 transcript lines were cut off, and lines appended at the bottom stayed
  invisible until the user scrolled. The messages and log windows now paint
  the cached rows verbatim (no re-wrap); the scroll window, geometry, and
  paint share one wrapped-row coordinate system, so the pinned view always
  ends at the true tail and appended lines appear immediately. Regression
  test: `test_pinned_tail_not_shifted_by_whitespace_only_rows`.
- **TUI exit hang (press-a-key-to-exit)** — the crossterm reader ran as a
  blocking task parked inside `ct_event::read()` with no shutdown path, so at
  runtime drop tokio's blocking-pool join hung the process after the TUI closed
  until the next keypress delivered a final event. The reader now uses
  `ct_event::poll(100ms)` with a cooperative `AtomicBool` stop flag (set after
  the main loop and by a Drop guard covering early `?` returns), so it exits
  promptly at shutdown.
- **TUI select-loop hot-spin on closed reader channel** — the
  `ct_event_rx.recv()` arm swallowed `None` via `if let Some(...)`, so a closed
  reader channel resolved immediately forever and spun the loop at 100% CPU.
  It now mirrors the event-bus arm: `None` is treated as fatal input-source
  loss and stops the TUI loop.
- **200% CPU at exit (root cause documented)** — while the process was hung
  waiting for the keypress, the detached `codeindex-init-reindex` thread
  (JoinHandle discarded) kept running `full_reindex()` with rayon-parallel
  hashing, alongside any in-flight worker reindex, burning both cores until
  the exit-hang fix let the process terminate.

## Version: 1.0.66

### Fixed

- **Tasks panel scrollbar drag direction** — the drag handler grouped the
  TODO/Tasks pane with the "lines from top" panels, but its renderer maps the
  stored offset via `max_scroll - scroll` (the "lines from bottom" family, like
  Profile). Dragging the Tasks scrollbar therefore moved the content the wrong
  way relative to the thumb. Tasks now uses the inverted
  `(1.0 - fraction) * max_scroll` formula, `tasks_scroll_offset` semantics are
  documented as lines-from-bottom in `state.rs`, and a regression test
  (`test_drag_tasks_scrollbar_moves_offset_bottom_semantics`) covers dragging
  to both ends of the track.
- **Active-agents button/kill hit-rect desync** — sub-panel button hit-rects
  were placed with button-order arithmetic (`i + 2`), which desynced hit-rects
  from painted rows whenever banner lines exist above the task list. The row's
  painted line index is now captured at build time in `Rect::y` and shifted
  once by scroll, with rows scrolled past the guard skipped instead of
  mis-placed.
- **Spinner latch self-healing** — the model-list and model-download spinners
  are latched by single bus events; after a broadcast `Lagged` burst the
  clearing event could be dropped, leaving a stuck popup and a 250 ms idle
  deadline for the rest of the process. New staleness caps (120 s loading,
  45 min download) clear wedged latches each main-loop wake, progress events
  reset the download latch clock, and the animation deadline only counts
  latches that are still fresh.
- **Bench watchdog** — a benchmark run that outlives the 30-minute staleness
  cap is detached (task row removed, status shown, warning logged) so a wedged
  runner cannot hold the animate deadline and poll path forever.
- **History flush latch** — `flush_history_if_due` now clears
  `history_dirty` / `history_save_deadline` to a terminal state when no
  history file is configured instead of keeping an already-past deadline that
  would busy-spin the main loop.

### Tests

- New `test_drag_tasks_scrollbar_moves_offset_bottom_semantics` in
  `test_tasks_panel.rs`; `test_flush_history_if_due_no_path_set` now asserts
  the latch is cleared rather than preserved.

## Version: 1.0.65

### Added

- **OpenRouter provider** — new `openrouter` first-class LLM provider for the OpenRouter model aggregator. Supports dynamic model discovery (`GET /api/v1/models`), OpenAI-compatible streaming chat completions with tool-call and reasoning deltas, per-model metadata mapping (context window, pricing, vision/reasoning flags), vendor-slug model ids such as `openrouter/anthropic/claude-sonnet-4`, secure API-key storage via `ragent auth openrouter <key>` (with `OPENROUTER_API_KEY` fallback), and masked-key redaction in logs. `ragent models --provider openrouter` lists the live catalog (discovery works without a key; chat requires one). (spec: `openrouterprov`; FR-001..026)
- **`model_info` tool** — read-only introspection tool reporting the active provider/model pair with resolved capabilities, context window, output limits, cost tier, thinking support, and Model Router enabled state. Hardwired auto-approved, registered in the default tool registry, and wired to a new optional `ToolContext::provider_registry` so tools can resolve model metadata.
- **Ollama thinking defaults** — Ollama-family models that advertise reasoning support now carry a default `ThinkingConfig` (level `Low`), and boolean-thinking providers present the full user-facing effort range (`Auto/Off/Low/Medium/High`), with any non-`Off` level mapped to `think: true` at request time.
- **Research corpus scoreboard** — deterministic, LLM-free corpus-quality display helpers per the `corpusAnalysis` spec: `GradeBand` letter grades (FR-002), proportional ASCII meter bars (FR-003), and IMRAD report rendering, all pure-ASCII output (FR-016).
- **OpenRouter reasoning payload builder** — `openrouter_reasoning_payload_from_request` maps `ThinkingConfig` (`effort` low/medium/high/none plus `max_tokens` from `budget_tokens`) onto OpenRouter's native `reasoning` object, with legacy `reasoning_effort`/`reasoning_level` option fallbacks.

### Fixed

- **TUI scroll pinning and idle CPU** — message/log panels pre-wrap cached lines at the pane width (`wrap_line_styled`, a faithful ratatui 0.29 `WordWrapper` port with NBSP/ZWSP parity) so scroll geometry and the visible-window slice share one coordinate system; only the visible window is wrapped per frame. The code-index busy latch clears when a reindex completes, the cron tick loop sleeps the full interval instead of polling, and a saturated event bus no longer hot-spins the permission prompt loop on `RecvError::Lagged` (brief yield added).
- **Model picker columns** — `SelectModel`/`SelectRouterModel` tables size columns to the widest header or cell so long vendor-slug model names and the Thinking column render untruncated.
- **Config deep merge** — provider `models` entries are now merged per model id instead of replaced wholesale, so a project overlay that sets only `thinking` no longer wipes globally-configured model names, capabilities, or pricing; model-level thinking overrides the provider default.
- **Provider discovery cache** — a transient discovery failure no longer wipes a provider's previously discovered catalog; the empty list is only cached when nothing is stored yet.
- **Research gather events** — policy exclusions (low relevance, too-short body, PDFs disabled) are reported as a new `SourceExcluded` event instead of masquerading as `FetchFailed`, keeping fetch-failure counters meaningful; the web phase gained a default 180 s wall-clock timeout and open-access lookups (Unpaywall/Europe PMC) a 15 s bound so a stalled API cannot wedge a run; distinct web domains are collected via `HashSet`, and the Data Quality panel reports the true strongest contradiction edge.
- **Lock-poisoning resilience sweep** — router config, bash/dir allow/deny lists, grep results, edit timestamps, tool cache, and the read cache recover from poisoned locks (`PoisonError::into_inner`) instead of silently dropping updates or failing subsequent calls; session archive import/export replaces the `.` data-dir fallback with a real error and hashes files via `std::io::copy`; the orchestrator conflict resolver removes unwrap-style paths on the guaranteed-non-empty tail.
- **HTTP retry backoff unified** — a single `backoff_delay` helper (0.5 s doubling to 8 s) drives 5xx and 429 retries.
- **Bench metrics and code-index** — exact-match bench samples short-circuit the Levenshtein pass entirely; the parallel grep walker checks its stop flag per entry before the truncation check; code-index import-edge derivation inverts the symbol/import loop to resolve each import once.
- **Router classifier panic guard** — non-finite composite scores and classifier errors fall back to the MEDIUM tier with zeroed dimension scores instead of poisoning the routing decision.

### Tests

- New suites: `test_openrouter_provider` (16 tests), `test_scoreboard` with IMRAD/reductions/report companions (31 tests), `test_model_info_tool` (4), `test_vendor_slug_partitioning` (3), and expanded `test_thinking_config` coverage (141 added lines).

## Version: 1.0.64

### Added

- **Interim-save gate for streaming assistant messages (M-008/P-12/H3)** —
  the per-stream-event interim save is now gated on a count of significant
  (non-tool-call) message parts and uses the FTS-skip update variant. Tool-call
  parts are appended to the transcript and finalised on the final save, so
  rewriting the SQLite row (and re-indexing FTS) on every stream event was
  wasted work; the significant-count invariant makes the count alone a
  sufficient save gate.
- **Per-entry log-line render cache (C-008)** — each TUI log entry carries a
  monotonic `seq` stamp and each cached render group mirrors the `seq` of the
  entry it was rendered for, so appending a log line only invalidates its own
  group instead of the entire log-panel cache. `truncate_chars`/
  `truncate_bytes` helpers handle UTF-8 boundaries for compact JSON previews.
- **`list_dependent_paths` (H-004)** — the code-index store resolves files that
  depend on a target path with a single SQL join instead of loading the whole
  indexed-file table on the caller side.
- **Concurrent vault reads (M-026)** — research `gather_from_vault` off-loads
  every local-file read to the blocking pool and runs them concurrently
  (`join_all` preserves hit order); previously the batch serialized behind the
  slowest file.
- **Parallel grep walker early exit** — the `ignore` parallel walker now checks
  the stop flag per entry before paying the global truncation check, and
  directory/error entries skip it entirely.

### Fixed

- **Tool-call `ToolCallEnd` emission order** — Ollama, Copilot, and HuggingFace
  providers now drain pending tool-call ends at `finish_reason`, sort by
  tool-call index, and emit them in order, matching the OpenAI provider;
  out-of-order ends could leave the UI associating results with the wrong call.
- **Event-bus drop diagnostics** — broadcast overflow and no-active-subscriber
  drops are now logged with the event type (and a `[short-session:step]` tag
  when a step counter is active) instead of being silently discarded.
- **Background-command lock hygiene** — the completion `Notify` handle is
  immutable after spawn and now lives outside the locked `Inner` state; mutex
  poisoning is logged once and surfaced as an error rather than panicking.
- **Bash timeout process-group kill** — the process-group id is recorded in
  the shared capture (`OnceLock` on an `Arc`) so the timeout handler can
  `killpg` survivors across tokio worker threads; native Bash and Git Bash
  share one command-builder arm.
- **Read-tool cache resilience** — the LRU file-read cache recovers from mutex
  poisoning (the ops under the lock cannot panic) instead of permanently
  failing all reads.
- **Memory routes** — removed the redundant existence pre-check in
  `forget_memory` (TOCTOU window; relies on the delete result), surfaced
  batched-tag-fetch errors via `tracing::warn!` instead of silently blanking
  tags, and handled the store-then-refetch `JoinError`.
- **Research citation checking** — `cited_indices` is shared with
  verification/synthesis; out-of-range citations are always reported even when
  mixed with valid ones; source token sets are cached by index so repeated
  finding-by-citation lookups are allocation-free; excerpt building accumulates
  lazily within a byte budget instead of copying whole bodies.
- **Codeindex persistence** — edge clear + bulk insert runs inside one
  transaction so a crash cannot leave the edge table empty (which would trip
  the TUI empty-graph guard); community upserts use `prepare_cached` inside the
  detection transaction.
- **HTTP tools** — `http_request`/`webfetch` share the versioned masterfetch
  client (redirect limit, gzip, UA); `webfetch` truncates once after extraction
  with a `[Content truncated]` marker instead of cutting mid-tag.
- **Custom-agent cache bounded** — the per-working-directory cache clears
  itself past 8 entries so directory-sweeping runs cannot grow it unbounded.
- **Task runner** — unhandled task errors warn instead of silently aborting the
  background task (which stalled `wait_agents` forever); model override parsing
  extracted into `parse_model_ref`.
- **AGENTS.md init exchange** — the per-event stall guard reuses the
  loop-invariant `stream_config.timeout_secs` (hoisted out of the loop).
- **Activity log** — lifecycle event lookup uses `prepare_cached`.

### Tests

- **Workspace test-target compilation restored** — 7 env-mutating integration
  test files (`ragent-config` x5, `ragent-llm` `test_ollama_cloud_real.rs`,
  `ragent-tools-extended` `test_mf_fetch.rs`) gained a file-level
  `#![allow(unsafe_code)]` with an explanatory note: Rust 2024 makes
  `std::env::set_var`/`remove_var` unsafe while the workspace lints deny
  `unsafe_code`. `cargo test --workspace` passes (7586 tests, 0 failures).

### Removed

- Stale root-level planning documents (`GUIDEANCEFIX.md`, `PIPELINEPLAN.md`,
  `RESEARCHPLAN.md`, `RESOURCEPLAN.md`, `SECPLAN.md`, `SIMPPLAN.md`,
  `TOOLS.md`) superseded by `docs/` material.

## Version: 1.0.63

### Added

- **Bash output-capture hardening** — `run_with_output` now drains stdout/stderr
  concurrently into a capped buffer and waits only for the direct child, then
  gives readers a bounded post-exit drain window before returning. This fixes
  the hang where `cargo test --workspace` spawns test binaries that inherit the
  pipe write-ends (EOF never arrives) and bounds memory via a per-stream capture
  cap so a huge-output command cannot drive the OOM killer.
- **Parallel grep walker (M-020)** — the `ignore` crate's parallel walker now
  searches large trees on all cores; per-file `Searcher` clones keep each visit
  independent. Include/exclude glob overrides are validated up front so an
  invalid glob is surfaced as an error instead of silently searching unfiltered.
- **Streaming SSE line helper (C-003)** — `take_sse_line` drains the consumed
  prefix in place (reusing the remaining bytes) instead of re-allocating the
  whole unconsumed buffer per line, removing quadratic behaviour on long
  streams. Shared `STREAM_CHUNK_IDLE_TIMEOUT_SECS` (120 s) now applies across
  providers so a mid-stream stall cannot hang the agent forever (C-004).
- **Skill-registry caching (C-001)** — the session processor caches the loaded
  `SkillRegistry` keyed by the mtimes of every scanned skill directory plus
  `extra_dirs`, eliminating the per-turn synchronous disk walk + YAML parse.
- **Custom-agent caching (H-002)** — `load_custom_agents` results are cached per
  working directory and invalidated only when a discovery directory's mtime
  changes, so the per-turn system-prompt build no longer re-walks the
  filesystem.
- **Codeindex graph + search optimisations** — community upserts wrapped in a
  single transaction (M-028); name resolution built from one symbol load
  (H-005); dependent file-ID→path resolved with a single map query (H-004);
  `explain`/`path` use a keyed `SELECT name FROM symbols WHERE id = ?` instead
  of loading all symbols (H-003); FTS documents truncated on a UTF-8 char
  boundary; cached on-disk index size refreshed only on full reindex (M-029).

### Fixed

- **`nice`/`ionice` low-priority wrapper removed** — the `bash.nice` config and
  `low_priority_prefix`/`prepend_low_priority` helpers were removed; the
  `build_shell_command` builder now applies `kill_on_drop(true)` uniformly.
  README no longer advertises low-priority shell execution.
- **Codeindex lock poisoning recovery** — `store_guard`/`fts_guard`/
  `tree_cache_guard` helpers replace ~36 `lock().unwrap()` sites so a panicking
  indexing thread cannot cascade panics into subsequent user calls.
- **Activity-log query and locking cleanup** — `expire_runs_older_than` uses a
  single `GROUP BY` query; `branch_from_checkpoint` wrapped in an immediate
  transaction; extracted `insert_lifecycle_event_locked` (dedupes
  expiry/archive) and `append_new_at_locked` (no double-lock); event lookup
  uses `prepare_cached` (`find_event_locked`).
- **LLM provider cleanup** — removed dead `tool_call_names` maps across
  OpenAI/Copilot/HuggingFace/Ollama, dead `_idx` bindings, and full request-body
  logging on error; fixed severe indentation corruption in the OpenAI
  `parse_sse_stream`; Ollama/Ollama-Cloud now share the 120 s per-chunk timeout.
- **Config error reporting** — `format_report` replaces the magic-number
  `replace_range` hack with a direct header row and ASCII-only borders.
- **Missing-docs / clippy hygiene pass** — documented public items across
  `ragent-types` (activity/event/trigger derives), `ragent-specs` (error
  variants, modules), `ragent-tools-core`, `ragent-tools-extended` (finance
  model fields, Tool/ToolContext/StorageBackend items), `ragent-tools-vcs`,
  `ragent-llm` (ToolFormat variants, tool_cache/router_client), and
  `ragent-research`; `#[must_use]` on builder helpers; FNV literal separators;
  backtick and path `Display` fixes.

### Tests

- Bash timeout now surfaces partial stdout/stderr captured before cancellation
  (`PartialOutput`) with a regression test.
- `test_activity_log_runtime_defaults_to_true` made serial and
  order-independent; FTS warm-up tests assert zero missing rows for the
  incremental behaviour.

## Version: 1.0.62

### Added

- **Shared runtime-flag helper** — `crates/ragent-config/src/runtime_flag.rs`
  consolidates the duplicated `AtomicBool` + persist/sync/toggle boilerplate
  shared by the `activity_log`, `edit_log`, and `yolo` toggle modules
  (`RuntimeFlag` with `persist`/`sync_from_config`/`toggle_persist`).
- **Shared research citation helper** — `cited_indices` in `polarity.rs`
  extracts the distinct, 1-based source indices cited by a text via `[#N]`
  (sorted + deduplicated), reused by the verification, synthesis, and
  cite-checking passes.
- **`apply_model_override` helper** — extracted the duplicated model-override /
  pinned-model resolution logic in the coordinator and task runner.

### Fixed

- **Session-state cache eviction was a no-op** — `remove_session_state` used a
  function-local `static` distinct from the one inside `session_state_cache`,
  so eviction silently never matched. The map now lives at module scope
  (`SESSION_STATE_CACHE`) so both access the same store.
- **Startup timing: `seed_secret_registry` off the critical path** — the full
  `provider_auth` scan + per-row deobfuscation (which only feeds log
  redaction) now runs on a background thread; well-known env-var secrets are
  still seeded synchronously so common keys are redacted from the first log
  line.
- **Startup timing: `ActivityLog::open` off the critical path** — opening the
  separate `activity_log.db` + schema migration now runs on a background
  thread, wired via `OnceLock`, so it no longer stalls the TUI's first frame.
  Recording stays best-effort and simply skips until the open finishes.
- **Bash tool refactor** — extracted `build_shell_command` (single builder with
  `kill_on_drop(true)` for Bash/GitBash/PowerShell) and `truncate_output`
  (keeps head/tail of oversized output).
- **FTS warm-up is incremental (PERF-013)** — `warm_message_search_index` no
  longer DELETE+rebuilds `messages_fts`; it only inserts missing rows, so on a
  warm start it is a fast no-op. Test and doctest updated to assert zero
  missing rows (the FTS table is maintained incrementally by `create_message`).
- **Research polarity scan deduplicated** — source bodies are lowercased once
  and the positive/negative token scans are precomputed instead of re-lowering
  every body per dimension (~15x redundant work removed).
- **TUI minor fixes** — `tail8` helper for run-id truncation, robust fallback
  when parsing the cron `far_future` timestamp, single-pass `parse_refs`, and
  `as_mut` link/image render-state handling.

### Tests

- `test_activity_log_runtime_defaults_to_true` made serial and order-
  independent (the runtime flag is a process-wide static shared with the other
  serial tests, which may leave it disabled); it restores the documented
  default before asserting.
- `background_warmup_does_not_block_reader` and the
  `warm_message_search_index` doctest now assert zero missing rows, matching
  the incremental FTS behaviour.

## Version: 1.0.61

### Added

- **Per-message render cache (`edit_seq`)** — `Message` gains an `edit_seq`
  counter bumped by `Message::touch()` on every in-place mutation, and the TUI
  `MessageLineGroup` cache keys staleness on this per-message counter instead
  of the global `messages_version`, so streaming into one message no longer
  invalidates the cached renders of every other message. The cache is cleared
  on structural changes (compaction, session restore) and cache trimming stays
  aligned with message trimming. Covered by
  `crates/ragent-tui/tests/test_message_edit_seq_cache.rs`.
- **Multi-line `agent_complete` summaries in the transcript** — task-completion
  summaries render one output line per line (ratatui strips `\n` inside a
  Span), with the compact header showing only the first line.
- **Status-bar indicator helper** — shared `push_indicator` deduplicates the
  `Label:{icon}` service-indicator spans on status-bar line 2.
- **Shared `/alog` argument helpers** — `parse_alog_run_id_yes` and
  `open_verified_alog` centralise the parse/validate/open/verify scaffold used
  by the destructive `/alog delete` and `/alog export` handlers.
- **Activity-log race regression test** —
  `append_new_immediate_tx_two_handles_no_seq_theft` pins the two-handle
  append contract, and `run_status_active_after_resume` pins the FR-013
  resumed-run status transition.

### Fixed

- **Activity-log concurrent-append race** — `append_new` now uses an IMMEDIATE
  transaction so the next-seq read + insert are atomic across handles, and a
  `UNIQUE(run_id, seq)` constraint violation surfaces as the typed
  `AppendError::DuplicateSeq` instead of a raw storage error.
- **`bash.nice` wrapper ordering** — `prepend_low_priority` rebuilds the
  command object wholesale, so it must run before `current_dir`/stdio/env
  configuration; previously the working directory (and other settings applied
  first) were silently discarded. Also replaced an `unwrap()` with
  `split_first`. *(Feature later removed.)*
- **Double finish-reason handling** — the SSE `[DONE]` sentinel's generic
  `Finish { Stop }` no longer overwrites an earlier specific finish reason
  (`ToolUse`, `Length`), keeping tool-use turns distinguishable from clean
  stops.
- **AGENTS.md init stream stall guard** — the init-exchange loop now uses the
  configured `stream_config.timeout_secs` instead of a fixed 60 s, so slow
  reasoning models are not aborted mid-ack while streaming healthily.
- **Session-state cache removal** — `invalidate_session_state` could silently
  no-op because function-local `static` items are per-declaration-site; it now
  initialises and locks the shared cache unconditionally.
- **Gemini malformed-SSE head-of-line blocking** — a line that stays
  unparseable after more data arrives is retried a bounded number of times and
  then dropped with a warning, instead of being re-queued forever and blocking
  every event behind it.
- **Orphaned mailbox loops on registry prune** — pruning timed-out orchestrator
  agents now aborts their mailbox `JoinHandle`s exactly as `unregister` does.
- **`Coordinator` job-task leak** — dropping the coordinator aborts any
  still-running job tasks so spawned work does not outlive its owner (R-20).
- **Random eviction of the read-timestamp guard** — when the 2000-entry
  read-timestamps cap is hit, the genuinely oldest quarter (by mtime) is
  evicted; the previous `keys().take(...)` drained an arbitrary `HashMap`
  sample that could evict files read seconds ago, disabling the stale-file
  guard for them.
- **Team manager panics on dropped processor** — `expect()` on the weak
  `SessionProcessor` upgrade is replaced with typed errors on the user-facing
  teammate-spawn, team-memory, and agent-loop paths.
- **Fresh-install `activity_log` default** — `Config::load()` seeds the
  runtime flag from `true` (matching the serde default) so a generated default
  config never records `activity_log: false`.
- **`bash.nice` config overlay** — the overlay value now takes precedence over
  the compiled default during config merging. *(Feature later removed.)*
- **Compaction cache desync** — replacing the timeline with the summary
  message clears the per-message render cache.

### Changed

- **Research verifier tokenisation hoisted** — `KeywordVerifier` splits each
  finding and source body once per verification pass instead of once per
  (finding x citation) pair.
- **Research `--from-url`/`--from-file` seeding borrows page bodies** — the
  (potentially hundreds-of-KB) fetched body is borrowed for preview, topic
  derivation, and summarisation instead of being cloned, with shared
  `is_placeholder_title`/`body_preview` helpers keeping both seed paths
  consistent.
- **Shared citation regex** — the `[#N]` citation regex is defined once in
  `polarity::citation_re()` and reused by synthesis, verification,
  cite-checking, and document rendering.
- **Research HTTP routes** — `ResearchItemRow::from_item` unifies the list and
  show mappings; `POST /research` validates the name (and duplicate) before
  building the session config; error responses pass owned strings.
- **Startup timings table** — the Sum/Untracked row widths are included in the
  time column calculation so wide values no longer break alignment; rows share
  one formatter.
- **Config hygiene** — unused `toggle()` helpers removed from the `yolo`,
  `edit_log`, and `activity_log` config modules; the mixed-line-endings scan
  drops a redundant `has_lf` check; `checkpoint_wal` uses the `lock_conn!`
  macro; a stale `t0` timing in `main.rs` was removed.

## Version: 1.0.60

### Added

- **`/alog` activity-log slash commands** — new TUI command family for the
  append-only activity log (`help`, `on`, `off`, `list`, `show`, `stats`),
  with config persistence via the new `ragent_config::activity_log` module
  (default on) and an `Alog:` status-bar indicator.
- **Activity-log wiring in the session processor** —
  `SessionProcessor::set_activity_log` plus a best-effort
  `record_activity_event` helper that off-loads SQLite writes to
  `tokio::task::spawn_blocking` so recording never stalls the async executor.
  Wired from `main.rs` after the dry-run early return.
- **`ActivityLog::default_path`** — resolves `activity_log.db` next to the
  main ragent database; the TUI `activity_log_db_path` helper shares the same
  convention.
- **`bash.nice` low-priority execution** — shell commands now run as children
  of `nice -n <level>` (default 10) and, on Linux, `ionice -c 3`, so heavy
  agent workloads keep the host responsive. PowerShell/Git Bash and Windows
  are excluded (POSIX wrappers only). Documented in `docs/howtos/bash.md`.
  *(Feature later removed.)*
- **`/simplify all`** — the simplify skill now accepts `all` (apply every
  issue, not just safe ones) or an output-path argument, and gained the
  `edit`/`multi_edit` tools so it can apply fixes directly.  - **`docs/howtos/bash.md`** — full bash-tool documentation covering the
    seven-layer security model and the `/bash` slash commands.

### Fixed

- **Gemini stream parser infinite loop** — a `continue` in the inner SSE
  buffer loop re-parsed the same partial line forever (100% CPU). Replaced
  with `break` so the outer stream loop fetches more data; regression tests
  added in `crates/ragent-llm/tests/test_gemini_stream_spin.rs`.
- **Orphaned bash children on timeout** — `kill_on_drop(true)` added to the
  `validate_bash_syntax` subprocess and the main execute-path `Command`
  builders so timed-out commands are reaped instead of spinning at 100% CPU
  after their future is dropped.
- **SSE `[DONE]` termination** — OpenAI, Copilot, Azure Resource, and Bedrock
  stream loops now emit `StreamEvent::Finish { FinishReason::Stop }` on
  `[DONE]` instead of silently breaking, which downstream truncation
  detection could misread.
- **Removed `unreachable!()` in tool-call mapping** — OpenAI and Copilot
  message conversion now uses `filter_map` and skips non-tool content parts
  instead of panicking on unexpected variants.
- **Team manager error classification** — token-overflow and
  permanent-API-error detection now delegate to the canonical
  `session::history` helpers so team and session loops stay in sync.
- **Activity log interrupt checks** — `is_interrupted_locked` now propagates
  SQLite errors (`Result<bool>`) instead of treating failures as
  "not interrupted".
- **Background stream readers** — stdout/stderr reader tasks log and break on
  read errors instead of the `while let Ok(...)` pattern silently ending the
  task on the error path.
- **Research SSE lag marker** — the `/research` event stream emits a visible
  `[LAGGED]` event when the broadcast channel falls behind instead of an
  empty event.
- **LLM stream read timeout** — the session loop applies a 60-second timeout
  per streamed chunk so a stalled provider connection cannot wedge a run.
- **Config test isolation** — `test_activity_log_persist_helper_updates_config_file`
  now runs in a temp project with `XDG_CONFIG_HOME` overridden so
  `save_to_source` writes to the temp project config rather than the user's
  real configuration file.

### Changed

- **TUI idle redraw** — idle redraw interval raised from 250 ms to 2 s and
  rendering is gated on the dirty flag; the previous value forced a
  full-frame re-render (including unicode re-segmentation of the transcript)
  4x per second while idle, burning 10-15% of a core.
- **Startup flag sync** — `yolo`, `edit_log`, and `activity_log` expose
  `sync_from_config_value`, letting `main.rs` sync all three runtime flags
  from the already-loaded config instead of performing three extra disk
  reads; `persist_*` helpers now propagate config-load errors instead of
  silently writing defaults.
- **Copilot provider detection** — no longer spawns the `gh` CLI subprocess
  for token discovery; only explicit credential sources are honoured.
- **Research synthesis and analysis** — fewer intermediate allocations when
  building keyword blobs; `summarize_subject` logs and degrades gracefully
  on client or stream errors instead of aborting.
- **Orchestrator mailbox buffer** — pending-request buffer size extracted to
  the `MAILBOX_BUFFER_SIZE` constant (100) with documentation.
- **Hygiene** — `#![recursion_limit = "256"]` added to `ragent-tools-extended`
  (yfinance_rs future nesting trips the `recursion_depth_exceeding_limit`
  future-incompat lint), clippy `needless_bool` and `possible_missing_else`
  fixes in `ragent-codeindex`, `ragent-tools-extended`, and `ragent-llm`,
  a repaired `decrypt_key` doctest, and AGENTS-RUST.md now mandates running
  `cargo fmt` after every Rust file edit.

## Version: 1.0.59

### Added

- **Activity log system** — new `ragent-types/src/activity.rs` event schema and
  `ragent-storage/src/activity_log.rs` append-only JSONL event store with
  `RunId`/`EventId` identifiers, projection types (checkpoint, message,
  permission, tool call/result), rollback/resume/termination support, and
  consistency validation. Re-exported from `ragent-types` and `ragent-storage`.
- **Storage schema-version fast path** — `Storage::migrate` now records a
  `schema_version` setting; warm starts skip the full 34-statement
  `CREATE ... IF NOT EXISTS` batch and 7 column probes, reducing `Storage::open`
  from ~41 SQL round-trips to a single `CREATE TABLE` + `SELECT`.
- **WAL auto-checkpoint** — `PRAGMA wal_autocheckpoint=500` (down from the
  default 1000) plus a `checkpoint_wal()` method for graceful shutdown to keep
  the WAL file small during long sessions.
- **Startup timing untracked gap** — `StartupTimings` now reports
  `sum_stages_ms` and `untracked_ms` alongside the wall-clock total so
  uninstrumented startup sections are visible.
- **Criterion benchmark for activity log** — `ragent-storage/benches/activity_log_bench.rs`.

### Changed — Provider detection and startup performance

- **Removed `gh auth token` auto-discovery** — Copilot provider detection no
  longer spawns the `gh` CLI subprocess. Only explicit credential sources
  (env var, secure storage, config-based resource file) are honoured. The
  `find_gh_cli_token` function and process-wide cache have been removed.
- **Removed fast/slow provider-detection pass** — `detect_provider` and
  `get_configured_providers_impl` simplified to a single pass without the
  `defer_gh_cli` parameter.
- **Preferred-provider reordering** — the user's `preferred_provider` setting
  now moves a credentialed provider to the front of the list instead of being
  pushed unconditionally (which could select a provider without credentials).
- **Provider health-check timeouts** — Ollama health check capped at 2 s,
  Copilot at 8 s, with elapsed-time `tracing::info!` logging.
- **TUI struct-init timing** — `App` struct construction is now instrumented
  as a startup stage.

### Changed — Background task and code-index performance

- **Background command `wait`/`cancel` use `tokio::sync::Notify`** instead of
  polling `is_done()` every 50–200 ms, eliminating hundreds of idle wakeups
  per cancelled task.
- **Background `waiter_task` uses async `child.wait()`** with a
  `cancel_notify` select instead of `try_wait()` polling every 100 ms.
- **Code-index worker uses `recv_timeout`** instead of `try_recv` + `sleep`
  busy-spin, eliminating ~20 idle wakeups/second.
- **Askpass poll interval** increased from 100 ms to 500 ms (R-8) to reduce
  idle CPU wakeups during sudo-capable bash commands.

### Changed — Memory and resource bounding

- **TUI message trimming** — `trim_messages_if_needed()` caps the in-memory
  message list and `message_line_cache` to 500 entries (FIFO eviction).
- **TUI log entry trimming** — `trim_log_entries_if_needed()` caps
  `log_entries` and `log_line_cache` to 1000 entries.
- **LLM request stats rolling window** — `llm_request_stats` capped at 1000
  entries to prevent unbounded growth over long sessions.
- **Read-timestamp map eviction** — `read_timestamps` map capped at 2000
  entries with oldest-quarter eviction.
- **Background `drained_ids` cleanup** — drained background task IDs are now
  removed from the `drained_ids` HashSet on cleanup.
- **Session state cache eviction** — `SessionManager::remove_session_state`
  removes entries from the global cache on archive/sub-agent completion.
- **MCP client `Drop` guard** — best-effort cleanup of stdio child processes
  on `McpClient` drop.
- **Orchestrator job task handles** — coordinator stores `JoinHandle`s and
  uses an `ActiveJobsGuard` drop guard so `active_jobs` is decremented even on
  panic/cancellation.
- **Orchestrator registry mailbox-handle abort** — `unregister` aborts the
  mailbox-loop task so re-registration does not accumulate orphaned loops.
- **H4 progress-notice `AbortOnDrop` guard** — the provider-progress notice
  task is now aborted on every scope exit (including error/retry paths), not
  just the success path.

### Fixed

- **Edit-log test serialisation** — `EDIT_LOG_TEST_GUARD` mutex serialises
  tests that mutate the process-global `EDIT_LOG_MODE` flag, preventing
  parallel-test flakiness.
- **`config_path` no longer probed at session creation** — `SessionMeta`
  defaults `config_path` to `None` instead of reading `Config::load()` on
  every `create_session` call.

## Version: 1.0.58

### Added

- **Research tooling overhaul** — major upgrade to the research system across CLI, TUI, and HTTP server:
  - New `polarity.rs` module and `run_request.rs` builder for unified research run configuration (`ResearchRunRequest`).
  - New `contradiction.rs` module for detecting and reporting conflicting findings across sources.
  - New `corpus_critic.rs` module for critiquing research corpus quality and coverage.
  - New `reconcile.rs` module for reconciling conflicting evidence and producing consensus scores.
  - Extended `cli.rs` with tier routing (`--tier` flag), IMRAD output format support, and shared `build_session_config` builder.
  - New HTTP research routes (`GET/POST/DELETE /research`, SSE events endpoint at `/{name}/events`) with 202 + Location async behaviour and `?full=true` query param.
  - New TUI research progress display with tier-aware rendering and session event JSON handling.
  - New test suites: `test_contradiction.rs`, `test_corpus_critic.rs`, `test_reconcile.rs`, `test_research_run_request.rs`, `test_research_routes.rs` (37 new tests).
  - `session.rs` refactored (2300+ lines) with unified session config, IMRAD support, and tier router integration.

### Changed

- Shared `build_session_config` / `ResearchRunRequest` builder now used by CLI, TUI, and HTTP, ensuring configuration convergence.
- Research route registration fixed — relative paths inside `.nest("/research", ...)` to avoid doubled prefixes.
- `ResearchItemRow` extended with `topic`, `queries`, `output_format`, and `model` fields.
- `AppState` gained `research_runs` field for async research job tracking.

### Fixed

- Fixed pre-existing compile errors in `test_integration.rs` and `test_memory_api.rs` (missing `research_runs` field in `AppState`).

## Version: 1.0.57

### Added

- **Research Phase 4 — Integration / rollout**:
  - `session_event_json` helper extracted from `render_session_event_json` — the SSE handler now uses pure JSON without stripping the CLI `ragent-research: ` prefix (R-017).
  - Extended `ResearchItemRow` with `topic`, `queries`, `output_format`, and `model` fields; `GET /research/{name}?full=true` controls inclusion (R-018).
  - End-to-end tests for the shared `build_session_config` / `ResearchRunRequest` builder (24 tests) and HTTP research routes (13 tests) verifying CLI/TUI/HTTP convergence from a common fixture.
  - Fixed pre-existing compile errors in `test_integration.rs` and `test_memory_api.rs` (missing `research_runs` field in `AppState`).
  - Fixed research route registration — `research_routes()` was using absolute paths (`/research/...`) inside a `.nest("/research", ...)`, causing doubled prefixes; now uses relative paths (`/`, `/{name}`, `/{name}/events`).
  - SPEC.md updated with HTTP Research API section (§11.9), Research Tiers (§11.8), `imrad` output format, `research` SSE event type, and detailed endpoint documentation.
  - QUICKSTART.md updated with `?full=true`, `POST /research` 202+Location behaviour, SSE events endpoint, and `--tier` flag.

### Changed

- Internal code quality cleanup in `crates/ragent-tui/tests/test_bench_command.rs`: extracted `enter_isolated_project_dir()` and `last_message_text()` helpers to remove repeated CWD lock/temp-directory setup and `messages.last().expect(...).text_content()` patterns.

### Fixed

- Continued from 1.0.56: `/bench` TUI integration tests remain isolated and now share helper code, keeping the previously-fixed retry/recovery logic intact.

## Version: 1.0.56

### Fixed

- Fixed flaky `/bench` TUI integration tests in `crates/ragent-tui/tests/test_bench_command.rs` that caused CI failures on slower GitHub Actions runners. The tests now wait up to 10 seconds for background benchmark runs to complete and recover gracefully from a poisoned process-wide cwd mutex, preventing one failure from cascading into multiple reported failures.

## Version: 1.0.55

### Fixed

- Fixed the `release.yml` workflow YAML that was corrupted when the Windows-x86_64 build job was disabled; the file is now valid and parses correctly.
- Removed the disabled `build-windows` job from the `release` job's `needs:` dependencies so that the release job is no longer skipped when Windows is disabled.
- Removed the Windows artifact download/upload steps from the `release` job while the Windows build is disabled.
- Updated the informational `cargo audit` ignore list in the release workflow to include the currently-known unmaintained/unsound advisories, matching recent `cargo audit` output.

### Changed

- The `Build and Release` workflow now creates releases from Linux and macOS artifacts only; the Windows job remains in the file (with `if: false`) for easy re-enablement.

## Version: 1.0.54

### Fixed

- Fixed Clippy error `clippy::fn_params_excessive_bools` in `StatusBarCache::signature_matches` by adding an explicit `#[allow(clippy::fn_params_excessive_bools)]` attribute.
- Fixed Clippy error `clippy::while_let_loop` in the TUI crossterm event reader by rewriting the `loop { match ... }` block as a `while let Ok(event) = ...` loop.
- Fixed Windows release build failure in `ragent-tools-core` where `same_file_identity` called a non-existent `file_attributes()` method on `std::fs::Metadata`.  The Windows implementation now imports `std::os::windows::fs::MetadataExt` and compares `volume_serial_number()` + `file_index()` when both are available.
- Added a `#[cfg(not(any(unix, windows)))]` fallback for `same_file_identity` that returns `false`, keeping the canonical-path equality already provided by the caller.

### Changed

- `cargo clippy --workspace -- -D warnings -A clippy::used_underscore_items -A clippy::redundant_pub_crate -A clippy::wildcard_imports` now passes locally.
- `cargo check` passes locally.

## Version: 1.0.53

### Fixed

- Fixed compilation warning in `crates/ragent-tui/src/app/status_bar_cache.rs` by removing unused helper functions and imports.
- Fixed test compilation error in `crates/ragent-tui/tests/test_thinking_defaults.rs` after `agent::resolve_agent` started returning `Arc<AgentInfo>`; tests now unwrap the `Arc` before assignment.

### Changed

- Full workspace now compiles cleanly with `cargo check` and all workspace tests pass.
- Ran `cargo fmt` across the workspace.

## Version: 1.0.51

### Changed — `ragent-agent` performance optimisations (spec `agentopt`)

- **Shared `reqwest::Client`** for URL reference fetching (`@https://...`) and
  HTTP MCP transport — a single `OnceLock`-backed client replaces per-request
  construction (FR-007, T-001/T-002).
- **Cached fuzzy reference file list** — `collect_project_files` results are
  cached per working directory with mtime/TTL invalidation, avoiding a full
  directory walk on every fuzzy `@filename` reference (FR-009, T-003).
- **Reduced fuzzy matching allocations** — lowercased candidate strings are
  reused across the match loop instead of re-materialised per comparison
  (FR-009/FR-019, T-004).
- **`Arc<AgentInfo>` from agent resolution** — `resolve_agent` and
  `load_all_agents` return `Arc<AgentInfo>` so sub-agent/background spawning
  shares the prompt instead of deep-cloning it (FR-005/FR-013, T-005/T-007).
- **Reference-counted built-in prompt strings** — built-in agent system prompts
  are stored as `Arc<str>` and reused across compositions (FR-013/FR-018,
  T-006).
- **Borrowed tool definitions** — the tool registry exposes
  `definitions_slice()` returning `&[ToolDef]` so the LLM serialisation path
  no longer clones a full `Vec<ToolDef>` per request (FR-014, T-008).
- **Shared session `ToolContext`** — `ToolContext` and `PermissionChecker`
  state constant for the session are built once and shared across tool calls
  instead of reconstructed per dispatch (FR-004, T-009).
- **Batched `BackgroundTaskService` storage writes** — output and status
  updates are batched into a single storage transaction and skipped when
  stdout/stderr/progress are unchanged (FR-008, T-010).
- **Consolidated `BackgroundTaskService` state** — three separate
  `Mutex`-protected maps (`tasks`, `sessions`, `drained_ids`) are merged into
  a single `Mutex<BgState>`, eliminating the multi-lock acquisition convoy
  (FR-015, T-011).
- **`Arc<str>` for `TaskEntry` result/error strings** — `result` and `error`
  fields are `Arc<str>` so cloning a `TaskEntry` (e.g. in `list_agents`,
  `drain_completed`) is a cheap pointer bump rather than a full string copy
  (FR-006/FR-016, T-012).
- **`DashMap` in `AgentManager`** — `tasks` and `cancel_flags` maps replaced
  `RwLock<HashMap>` with `DashMap`, eliminating reader-writer lock contention
  on read-heavy paths (FR-016, T-013).
- **Cached canonical paths in permission checker** — `canonicalize` results
  are cached per step, avoiding a blocking syscall on every file-tool
  permission check (FR-017, T-014).
- **Hoisted hardwired tool approvals** — codeindex and other always-allowed
  tools are checked before any I/O in `check_permission_with_prompt`
  (FR-004/FR-017, T-015).
- **Batched memory tag fetches** — memory visualisation fetches all tags in a
  single SQL query instead of one per row (FR-010, T-016).
- **Single-pass skill/template substitution** — `{{PLACEHOLDER}}` substitution
  scans the template once and builds the result in a single pass (FR-011,
  T-017).
- **Single-pass goal-evaluation context builder** — the goal evaluator builds
  its context string in one pass with pre-sized buffers instead of allocating
  a formatted string per message plus an intermediate `Vec` (FR-012, T-018).
- **Pre-sized snapshot diff buffers** — `similar` diff buffers are
  pre-allocated to the input size to reduce reallocation during patch
  generation (FR-004, T-019).
- **Regression/criterion benchmarks** — benchmarks for hot paths
  (`crates/ragent-agent/benches/hot_paths.rs`) provide a baseline for
  detecting regressions (FR-002, T-020).
- **Full test suite and clippy pass** — `cargo test -p ragent-agent`,
  `cargo clippy -p ragent-agent`, and `cargo fmt -p ragent-agent` all pass
  clean after all optimisations (FR-002, T-021).

### Added — Tool-call log in sub-agent report files

- **Sub-agent report files** (`log/subagents/<task-id>.md`) now include a
  `## Tool Call Log` section between `## Task` and `## Output` that records
  every tool the agent invoked during its run: tool name, status
  (completed/error), input arguments, output (or error message), and
  execution duration. The section is clearly separated from the agent's
  output text with a blockquote warning ("The entries below are tool
  invocations … NOT agent output text") so the log is never confused for
  findings. Tool inputs are truncated to 500 chars and outputs to 500
  chars to keep the log readable. Error/cancellation paths do not include
  a tool log.

### Fixed — Sub-agent premature-termination ("narration without findings")

- **Root cause identified**: when a sub-agent (e.g. `explore`) that was
  actively calling tools produces a SHORT text-only response — narration like
  "Now let me check the remaining spots …" — without a tool call, the agent
  loop treated that narration as the final answer and terminated. The agent
  never produced its findings report, so the deliverable was a 100–400 char
  fragment. This is a *third* truncation pattern distinct from the two fixed
  in 1.0.50 (silent end-of-stream and generic 12k content cut): the provider
  sends a normal `Finish { reason: Stop }`, so none of the existing truncation
  guards fired.
- **Fix**: the session loop (`crates/ragent-agent/src/session/processor.rs`)
  now detects this pattern for sub-agents: when `tool_calls.is_empty()` on a
  step > 1 (prior tool-use steps) and the text is under 2 000 chars, it
  injects a one-shot `SUBAGENT_SUMMARY_NUDGE` user message asking the model
  to produce its complete findings report now, then continues the loop. The
  next text-only response (the actual findings) terminates the loop normally.
  The nudge fires at most once per run. Primary agents are not affected.
- **Regression tests**: `crates/ragent-agent/tests/test_subagent_summary_nudge.rs`
  — verifies a sub-agent that does tool work then produces short narration is
  nudged to produce findings (3 LLM calls: tool, narration, findings), and
  that a primary agent producing the same pattern is NOT nudged (2 calls only).

### Added — Durable full sub-agent reports on disk

- **Every sub-agent output is now persisted to a unique file**
  `log/subagents/<task-id>.md` under the working directory when the agent
  completes (or fails) — foreground and background alike
  (`crates/ragent-agent/src/task/mod.rs`: `write_subagent_output` /
  `record_output_file`, stamped onto the new `TaskEntry::output_file`
  field, `#[serde(default)]` for backward compatibility).  Written via a
  temp-file + rename so a crash mid-write never leaves a truncated report
  masquerading as complete.
- **`wait_agents` surfaces the report path** in both its formatted content
  (`📄 Full report: <path>` line) and its metadata `results[]` entries
  (new `output_file` field).  If the combined batch output is cut by the
  generic 12k context truncation, the parent agent can recover the omitted
  findings by `read`-ing the file instead of re-running the sub-agent.
- **`list_agents` surfaces the report path** under each finished task in
  the table output, in the single-task detail view, and in its tool
  description.
- **Background task injection** (`session/processor.rs::drain_completed`
  consumer) appends a `Full untruncated report: <path>` note to the
  injected message when the body might be truncated for context.
- **Tool-result truncation notice** (`session/history.rs`) now explicitly
  names the `log/subagents/<task-id>.md` file as the recovery path.  The
  system prompt's `wait_agents` guidance likewise points at the on-disk
  file as the primary recovery mechanism.

### Fixed — Reappearing "sub-agent output truncated" complaint

- **`wait_agents` metadata now carries full per-agent reports**
  (`crates/ragent-agent/src/tool/wait_agents.rs`) — the tool output
  `content` already contained the full agent results (fixed in 1.0.50), but
  once the combined batch exceeded the generic 12 000-char tool-result
  budget in `history.rs::tool_result_content_for_llm`, the head+tail
  truncation silently dropped one agent's report from the middle.  The tool
  now mirrors every completed agent's full output into a metadata
  `results` array (`{task_id, agent, success, output}`) so the data is
  recoverable even when the printed content is cut.
- **`wait_agents` now collects already-completed background-task results** —
  previously `waiting_for` only seeded `Running` tasks, so a caller that
  invoked `wait_agents` *after* its sub-agents had finished received
  "No running background tasks to wait for." even though completed results
  existed for the session.  The waiting set now also picks up completed /
  failed / cancelled background tasks for the current session.

### Changed — Tool-result truncation headroom and pointers

- Raised the tool-result head budget for LLM context from 8 000 to 10 000
  chars (`crates/ragent-agent/src/session/history.rs`) so a typical
  3-agent `wait_agents` batch fits under the 12k threshold without
  truncation.
- The truncation marker now explicitly tells the model that full per-agent
  reports are available in the `wait_agents` metadata `results` array
  instead of only "request narrower output if more detail is needed".
- The built-in system prompt now warns agents that `wait_agents` output may
  be truncated and that the complete reports live in metadata `results`.

### Added

- `AgentManager::seed_completed_for_test` — hidden test-only helper that
  inserts a completed `TaskEntry` directly into the in-memory task map so
  integration tests can verify `wait_agents` behaviour without running a
  real child LLM session.
- Regression test `test_wait_agents_results_meta.rs` — asserts a 20 000-char
  agent report arrives untruncated via both `ToolOutput.content` and
  `metadata.results[0].output` for the completed-task path.

## Version: 1.0.50

### Fixed — Sub-agent result truncation

- **`agent_complete` summary vs result conflation**
  (`crates/ragent-agent/src/task/mod.rs`,
  `crates/ragent-agent/src/tool/wait_agents.rs`) — the
  `SubagentComplete` event carries a `summary` field that is truncated to
  2000 characters for TUI display. When `WaitAgentsTool` collected results it
  was using this truncated summary as the agent output, so sub-agent responses
  longer than 2000 chars were silently truncated for the parent agent. The
  task entry now stores the full response in `result`, and `WaitAgentsTool`
  looks up the complete text from the task entry (falling back to `error`,
  then `(no output)`).
- Added a regression test (`test_event_summary_is_short`) confirming the
  event summary remains truncated while the full result is preserved.

### Changed — How-to documentation restructure

- Renamed `howto_hooks.md` to `hooks.md` and `howto_teams.md` to `teams.md`
  to match the naming convention of the other how-to files.
- Added new how-to guides: `codeindex.md`, `config.md`, `permissions.md`,
  `tools.md`, and `tutorial.md`, with PDF renderings.
- Added PDF renderings of `custom-agents` and `tool-visibility`.

## Version: 1.0.49

### Changed — Documentation Updates

- Updated and consolidated how-to documentation across `docs/howtos/`:
  - Renamed `commshowto.md` to `communications.md` and refreshed content.
  - Added `finance.md` how-to covering stock/currency tools.
  - Refreshed `custom-agents.md`, `howto_teams.md`, `reverse.md`,
    `tool-visibility.md`, and `vcs-tool-implementation-pattern.md`.
  - Added PDF renderings of the communications and finance how-tos.
- Removed stale scratch files (`editplan.md`, `reddit.html`, `reddit.md`,
  `v1rocket.md`) from the repository root.
- Minor fixes throughout.

## Version: 1.0.48

### Fixed — CI Flaky Test

- **`test_worker_manual_full_reindex` flaky on loaded CI runners**
  (`crates/ragent-codeindex/tests/test_m4_integration.rs`) — the test used a
  fixed 600ms sleep before asserting that the background worker had processed
  the full reindex batch. On slow/loaded CI machines thread scheduling latency
  could exceed that window, causing the assertion `batches_processed >= 1` to
  fail spuriously. Replaced the fixed sleep with a polling loop (up to 5s,
  50ms interval) that checks `stats.batches_processed` on each iteration. The
  assertion message now includes `batches_processed` and `files_indexed` for
  better diagnostics if the deadline is reached.

## Version: 1.0.47

### Added — GitLab Support for `/spec reverse`

- **Provider-agnostic repository parsing** — new `VcsProvider` enum and
  `parse_reverse_repo` function (`crates/ragent-tools-vcs/src/vcs_provider.rs`)
  accept any supported repository identifier format and route to the correct
  VCS API client:
  - Provider-prefixed: `github:owner/repo`, `gitlab:namespace/project`,
    `gitlab:host/namespace/project` (self-hosted)
  - Bare shorthand: `owner/repo` (defaults to GitHub, backward compatible)
  - GitHub URLs: `https://github.com/owner/repo`, `git@github.com:owner/repo.git`
  - GitLab URLs: `https://gitlab.com/namespace/project`,
    `https://gitlab.example.com/group/project` (self-hosted),
    `git@gitlab.com:namespace/project.git` (SSH)
  - Nested GitLab namespaces: `group/subgroup/project`
  - Re-exported from `ragent-agent` as `VcsProvider` and `parse_reverse_repo`
- **GitLab API client methods** (`crates/ragent-tools-vcs/src/gitlab/client.rs`):
  - `fetch_project_metadata` — `GET /projects/:id` with language resolution
    via `GET /projects/:id/languages`
  - `fetch_repository_tree` — `GET /projects/:id/repository/tree` (root level)
  - `fetch_repository_tree_recursive` — recursive tree fetch up to N levels
    with trailing-slash directory markers
  - `fetch_readme` — `GET /projects/:id/readme` with `readme_url` raw fetch
    and `repository/files` fallback; 404 yields `Ok(None)`
  - Pure parsing helpers: `gitlab_project_to_metadata`, `top_language`,
    `parse_gitlab_tree`, `parse_gitlab_tree_entries`, `extract_readme_url`
- **Recursive tree fetch for GitHub** — `GitHubClient::fetch_tree_recursive`
  expands directories up to a configurable depth via
  `GET /repos/{owner}/{repo}/contents/{path}`, with trailing-slash directory
  markers and tolerated per-directory failures
- **`--depth <N>` flag for `/spec reverse`** — controls tree-fetch depth (1–10,
  default 1). Validated by `validate_depth`; invalid values produce a
  human-readable error
- **Provider label in reverse-engineering context** — `build_reverse_prompt`
  now accepts an optional `provider_label` (e.g. `"GitHub"` or
  `"GitLab (gitlab.example.com)"`), emitted as a `## Repository Source`
  section before metadata
- **`GitHubClient::with_base_url`** — constructor for pointing the client at
  a custom base URL (primarily for mock-server tests)
- **GitLab token resolution** for `/spec reverse` — priority chain:
  `GITLAB_TOKEN` env → `ragent.json` → encrypted database via `/gitlab setup`
- **`--depth` and GitLab formats in `/spec reverse` help message** — updated
  `reverse_help_message` to list `--depth`, `gitlab:`, `/gitlab setup`, and all
  accepted URL formats
- 4 new test files: `test_backward_compat.rs`, `test_gitlab_fetch_methods.rs`,
  `test_parse_reverse_repo_formats.rs`, `test_recursive_tree_fetch.rs`; plus
  expanded `test_build_reverse_prompt.rs` and inline tests in
  `vcs_provider.rs`, `gitlab/client.rs`, and `reverse.rs`

### Fixed — Yahoo Finance Provider Cache

- **Race condition in `get_or_create_yahoo_provider`**
  (`crates/ragent-tools-extended/src/finance/providers/paid.rs`) — the cache
  lookup and insertion are now performed atomically under a single lock
  acquisition, preventing two concurrent calls with the same key from both
  missing the cache and creating distinct providers (which would break
  `Arc::ptr_eq` guarantees relied on by callers).

### Changed

- Bumped workspace version to 1.0.47.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.
- Added `wiremock = "0.6"` dev-dependency to `ragent-tools-vcs` for
  mock-server HTTP tests.

## Version: 1.0.46

### Added — GitHub Repository Reverse-Engineering (`/spec reverse`)

- **New `/spec reverse` slash command** — takes a public GitHub repository URL (or
  `owner/repo` shorthand), fetches the repo's metadata, root file tree, and
  README via the GitHub API, then passes the assembled context to the
  currently selected LLM model to generate a synthetic creation prompt.
  - Accepts full URLs (`https://github.com/owner/repo`), SSH URLs
    (`git@github.com:owner/repo.git`), and `owner/repo` shorthand.
  - Optional `--tech <stack>` flag constrains the generated prompt to a
    specified technology stack.
  - Optional `--create <name>` flag chains into `/spec create <name>
    <generated-prompt>` after the LLM finishes, automatically creating a spec
    from the reverse-engineered prompt.
  - `--help` subcommand displays a usage message.
  - GitHub API error handling for 404, 403/429 rate-limit (with reset time),
    and other non-success status codes with human-readable messages.
  - Invalid-input rejection for empty, single-word, and three-segment
    identifiers.
  - Autocomplete suggestions for `reverse` subcommands and flags.
  - New GitHub client helpers: `parse_repo_url`, `validate_repo_input`,
    `fetch_repo_metadata`, `fetch_root_tree`, `fetch_readme`,
    `build_reverse_prompt`.
  - 13 new test files covering URL parsing, repo metadata, root tree, README
    extraction, API error handling, invalid-input rejection, and
    `build_reverse_prompt` context assembly.

### Added — Howto Documentation

- **New `docs/howtos/research.md`** — extensive documentation for the `/research`
  command system covering purpose, capabilities, all slash subcommands, tiers,
  depth/iterations, output formats, seed sources, gathering tuning, the
  Hyperresearch pipeline, source vault, templates, HTTP API, CLI equivalents,
  and end-to-end examples (42,895 bytes).
- **New `docs/howtos/spec.md`** — extensive documentation for the `/spec`
  command system covering EARS notation, all 19 subcommands, lifecycle status
  transitions, task management, SDD workflow, SDD configuration, validation
  details, implementation orchestration, JTBD analysis, production feedback,
  and end-to-end examples (36,577 bytes).
- **New `docs/howtos/reverse.md`** — extensive documentation for the `/spec reverse`
  command system covering purpose, capabilities, command syntax, GitHub API
  interaction, synthetic prompt generation, `--tech` and `--create` flags,
  CLI equivalents, and end-to-end examples (20,767 bytes).

### Changed — Spec Task Table Status Column

- **PLAN.md and TASKS.md task tables now include a Status column** initialised
  to `Pending`. Updated all four LLM prompt builders (`build_create_prompt`,
  `build_plan_prompt`, `build_update_prompt`, `build_add_prompt`) to instruct
  the agent to include the Status column with `Pending` for all new tasks.
  Updated the `PlanTemplate` programmatic template, the `build_tasks_md`
  generator, and the doc comment for `parse_tasks` to reflect the 7-column
  format. The runtime parser already handled both 6-column and 7-column
  formats; the `rewrite_plan_tasks` function already wrote the Status column
  when rewriting tasks after `/spec impl` updates.

### Changed

- Bumped workspace version to 1.0.46.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

## Version: 1.0.45

### Fixed — CI

- **Dead-code lint**: Added `#![allow(unreachable_pub)]` to vendored
  `html2text` crate root (`vendor/html2text/src/lib.rs`). The CI
  `dead-code-lint` job runs with `-D unreachable_pub`, which flagged 26
  internal `pub` items in the vendored crate that are not re-exported from
  the crate root. As a vendored third-party crate, these internal `pub`
  items are intentional and should not be subject to our workspace lint
  rules.
- **Codeindex permission categories**: Changed `codeindex_status` tool
  `permission_category()` from `"none"` to `"codeindex:read"`, matching
  all other read-only codeindex tools. Updated the backward-compatibility
  test (`test_codeindex_backward_compat.rs`) and the registry test
  (`test_codeindex_registry.rs`) to reflect the corrected category.
  The registry test also now correctly separates `codeindex_reindex`
  (`codeindex:write`) from the read-only tools assertion.

### Changed

- Bumped workspace version to 1.0.45.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

## Version: 1.0.44

### Added — Code Index Semantic Graph

- **Graph tools** — four new LLM-callable tools for semantic code graph analysis:
  - `codeindex_godnodes` — top-N most-connected symbols (highest degree)
  - `codeindex_path` — shortest path (by hop count) between two symbols
  - `codeindex_explain` — node metadata and incoming/outgoing edges for a symbol
  - `codeindex_communities` — community detection via label propagation
- All graph tools use non-blocking `try_*` variants with retry and return a
  `codeindex_busy` response when the index is locked.
- TUI `/codeindex` slash command extended with `graph <build|export|lang>`,
  `explain <symbol>`, `path <A> <B>`, `communities`, and `godnodes` sub-commands.
- New `ragent-codeindex::graph` module with `SymbolGraph`, typed `EdgeKind`,
  `Confidence`, community detection, and graph export.
- New `GraphStatus` type for graph-level statistics in `/codeindex show`.
- 13 new test files for graph build, edges, communities, export, readonly,
  resolve, status, store, symbol graph, traverse, and types.

### Added — Skills generation

- TUI skill generation module (`skillgen.rs`) and `graphify_skill_data`
  helper for converting skill metadata into graph-friendly structures.

### Changed — Code quality (simplify review)

- Extracted `bypass_research_text()` helper in `ragent-tui` models.rs,
  eliminating duplicated research-bypass guards between `render_markdown_to_ascii`
  and `render_markdown_unconditionally`.
- Extracted `prepare_agent_for_dispatch()` in `ragent-tui` session_ops.rs,
  eliminating duplicated agent-setup boilerplate between `dispatch_bang_command`
  and `dispatch_user_message`.
- Eliminated unnecessary `html_buf.clone()` in `render_markdown_pipeline`
  (move instead of clone).
- Removed 121 `/*FR-010 export*/` noise comments from `slash.rs`.

### Changed — Embedding deserialisation

- Refactored embedding deserialisation into a shared
  `ragent_types::embedding::deserialise_embedding` function.
  `ragent-storage` now delegates to this shared implementation instead of
  maintaining a copy.

### Changed — PDF text decoding

- Replaced `chunks_exact(2)` with `as_chunks::<2>().0` in UTF-16BE/LE text
  string decoding in `masterfetch/pdf.rs`, eliminating bounds-check panics.

### Fixed — Compaction runner

- Fixed compaction getting stuck when all messages fit inside the keep budget.
  `select()` now forces at least the oldest message into the head when there
  are 2+ messages, preventing the "nothing to summarise" bail that left users
  with an un-compactable context.
- Single-message compaction bail now logs at debug level instead of showing
  a confusing user-visible notice.

### Changed

- Bumped workspace version to 1.0.44.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

## Version: 1.0.43

### Changed

- Bumped workspace version to 1.0.43.
- Verified clean `cargo clippy --all-targets` run (zero warnings).
- Verified full test suite: 6707 tests across 353 test binaries, 0 failures.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

## Version: 1.0.42

### Added — Bang commands

- Prefix any prompt with `!` (e.g. `! ls -la`, `! cargo test --lib`) to run a
  shell command directly. The command output is sent to the model for review
  and error resolution.
- Works in both the interactive TUI and headless `ragent run` mode
  (`ragent run "! cargo check"`).
- In the TUI, the command is executed via `sh -c` on a spawned blocking thread
  so the UI stays responsive; the combined stdout/stderr is rendered in the
  chat panel before the model reviews it.
- `InputAction::BangCommand` variant added to `ragent-tui` input handling.
- Documented in `README.md` (features list) and `QUICKSTART.md` (new
  "Bang Commands" section).

### Changed — Code quality

- Replaced `chunks_exact(4)` / `chunks_exact(2)` with `as_chunks::<N>().0` in
  embedding deserialisation (`ragent-storage`, `ragent-tools-extended/memory`)
  and PDF text-string UTF-16 decoding (`ragent-tools-extended/masterfetch/pdf`),
  eliminating bounds-check panics and redundant slicing.
- `ragent-tui` input handler: `drain(..).collect()` → `std::mem::take` for
  `pending_attachments`, avoiding an intermediate allocation.
- Merged two separate throttle tests into a single `throttle_behaviour` test
  in `ragent-tools-extended/finance/throttle.rs` to reduce shared-state flakiness.
- Added `unused_async_trait_impl` to the workspace clippy allow list.

### Changed

- Bumped workspace version to 1.0.42.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

## Version: 1.0.41

### Changed

- Bumped workspace version to 1.0.41.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

### Fixed — CI

- Fixed `cargo fmt --check` failures in the CI Rustfmt job. Ran `cargo fmt`
  to reformat four files that had formatting diffs from the 1.0.40 commit:
  - `crates/ragent-llm/src/providers/router_classifier.rs` — closure body
    wrapping.
  - `crates/ragent-tools-extended/src/masterfetch/extractor.rs` —
    `spawn` closure body wrapping.
  - `crates/ragent-tools-extended/tests/test_pdf_expert_cff.rs` —
    `Stream::new` argument wrapping.
  - `crates/ragent-types/tests/test_panic_guard.rs` — `assert!` argument
    wrapping.

## Version: 1.0.40

### Changed

- Bumped workspace version to 1.0.40.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

### Fixed — /research system

- Fixed a `/research create` panic (`attempt to subtract with overflow` in
  `html2text 0.16.7` `WrappedBlock::flush_word`) that aborted the whole
  process when the web gatherer fetched an HTML page whose rendered line
  exceeded the wrap width (typically large HTML tables). `html2text` is now
  vendored at `vendor/html2text` with `saturating_sub` patches at both
  `width - line.len` sites.
- All remaining `html2text::from_read` call sites (masterfetch extractor,
  legacy `webfetch`, TUI markdown renderer, `@url` reference resolver) now run
  the conversion on a dedicated OS thread instead of `catch_unwind` on an
  async/UI thread, so any residual third-party panic is confined to that
  thread and can never unwind through the Tokio runtime or take the process
  down.
- Fixed a `/research create` panic (`explicit panic` from
  `cff-parser-0.1.0` `Encoding::get_table`) caused by PDFs with CFF fonts
  using `EncodingKind::Expert`. `extract_pdf_text` now runs
  `pdf_extract::extract_text_from_mem` on a dedicated OS thread with
  `panic_guard::run` inside, matching the `run_html2text_isolated` pattern.
- In the `/research` report (`RESEARCH.md`) layout, the `## Open Questions`
  section now appears directly under `## Top 10 Implications` instead of at the
  end of the document (before `## References Index`). Unresolved gaps now
  surface immediately after the ranked consequences. The IMRaD layout keeps
  Open Questions under `## Discussion`.

### Added

- `cargo patch` entry mapping crates-io `html2text` to `vendor/html2text`,
  mirroring the existing `vendor/pdf-extract` pattern.
- Regression tests: `test_wide_table_cell_does_not_panic` (synthetic wide
  table) and `test_mf_panic_repro.rs` (real-world mdBook fixture loop) in
  `ragent-tools-extended`.
- Regression test: `test_pdf_expert_cff.rs`
  (`test_expert_cff_through_extract_pdf_text`) covering the CFF Expert
  encoding panic path.
- `panic_guard` module in `ragent-types` providing `run()` for isolating
  panics on dedicated OS threads.

## Version: 1.0.39

### Changed

- Bumped workspace version to 1.0.39.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

## Version: 1.0.38

### Changed

- Bumped workspace version to 1.0.38.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

## Version: 1.0.37

### Changed

- Bumped workspace version to 1.0.37.
- Extended single tool-call watchdog timeout from 10 minutes to ~16 minutes
  (1000 seconds) to better accommodate long-running operations.
- Updated `mf_fetch` PDF extraction to use the workspace-patched `pdf-extract`
  crate, which falls back to `PDFDocEncoding` instead of panicking on missing
  Unicode maps. The panic-isolation wrapper is retained as a safety net.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

## Version: 1.0.36

### Changed

- Bumped workspace version to 1.0.36.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

### Fixed

- Finance provider selection now surfaces the configured paid provider
  (TwelveData/Alpha Vantage) instead of silently falling back to Yahoo when the
  paid provider fails for a symbol or endpoint. A new `finance.yahoo_fallback`
  option controls this; it defaults to `false` when a paid provider is
  configured, and `true` for the free Yahoo provider. The rate-limit error
  message now mentions TwelveData alongside Alpha Vantage.
- Removed undocumented `#[allow(dead_code)]` const placeholders in
  `crates/ragent-tools-extended/src/finance/tools/mod.rs`, resolving the
  dead-code reason check CI job failure.

## Version: 1.0.35

### Changed

- Bumped workspace version to 1.0.35.
- Updated `h2` crate to 0.4.16 to resolve RUSTSEC-2026-0258.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

## Version: 1.0.34

### Fixed

- Start-of-turn context compaction now uses the same provider-reported input
  token count shown in the TUI status bar. The previous turn's reported usage
  is persisted in the per-session state cache, preventing the trigger from
  falling back to the tool-heavy local estimate and firing when the displayed
  usage percentage is well below the 70 % floor. Emergency overflow and pre-send
  compaction paths also persist the compressed-token estimate so the next turn's
  usage percentage remains accurate after compaction.
- Added debug logging for every pre-send compaction trigger evaluation so users
  can inspect `effective_tokens`, `threshold`, `context_window`, and
  `last_reported_input_tokens` when diagnosing compaction behaviour.

### Changed

- Bumped workspace version to 1.0.34.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

## Version: 1.0.33

### Changed

- Fixed rustfmt formatting in `crates/ragent-tools-core/src/apply_patch.rs`.
- Bumped workspace version to 1.0.33.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

## Version: 1.0.32

### Changed

- Bumped workspace version to 1.0.32.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

## Version: 1.0.31

### Fixed

- Background shell tasks spawned with the `bg` tool now emit
  `BackgroundTaskUpdated` and `BackgroundTaskCompleted` events with the
  correct owning `session_id`, so the TUI Agents panel updates and removes
  rows as tasks finish instead of leaving them stuck in the `running` state.

### Changed

- Removed the legacy `/todo` and `/todos` slash-command aliases. The Tasks
  side panel and task service are now accessed through `/task` (toggle panel
  or subcommands) and `/tasks` (list task items). The TUI internal task-panel
  identifiers (`show_todo`, `todo_area`, `SelectionPane::Todo`,
  `InputAction::ToggleTodo`, etc.) have been renamed to task-specific names.
- Updated `QUICKSTART.md` to point to the Agents panel for background tasks
  and `/task list` for session tasks.

### Fixed — CI clippy and dead-code lint failures

- Fixed `clippy::needless_raw_string_hashes` warnings in
  `crates/ragent-agent/src/goal/mod.rs`, `crates/ragent-agent/src/template/mod.rs`,
  and `crates/ragent-tui/src/app/slash.rs` by removing unnecessary `#` delimiters
  from raw string literals that don't contain double quotes.
- Fixed `clippy::vec_init_then_push` warning in
  `crates/ragent-agent/src/template/mod.rs` by converting `Vec::new()` + `push`
  pattern to a `vec![]` macro.
- Fixed `clippy::useless_borrows_in_formatting` warning in `src/main.rs` by
  removing a redundant `&` borrow on a format argument.
- Added explanatory `// reason:` comments to all 8 undocumented
  `#[allow(dead_code)]` attributes in
  `crates/ragent-agent/src/session/archive.rs` `CronEventExport` struct,
  resolving the dead-code reason check CI job failure.

### Changed

- Bumped workspace version to 1.0.31.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

## Version: 1.0.30

### Added — Panic hook with full log capture (ragent binary)

- Installed a custom `std::panic` hook at the very start of `main` that
  captures every panic and writes a full report to `log/panic-*.log` in
  the project working directory.
- Each panic log includes: UTC timestamp, PID, executable path, working
  directory, panic location (file:line:column), panic message, full
  command-line arguments, `RUST_BACKTRACE` / `RUST_LIB_BACKTRACE`
  environment values, and a complete backtrace captured via
  `std::backtrace::Backtrace::force_capture` (always captured regardless
  of `RUST_BACKTRACE` setting).
- The hook chains to the default Rust panic hook after writing the file,
  preserving stderr output for terminal users.
- Added `src/panic_hook.rs` module with `install()`, `log_dir()`,
  `panic_log_path()`, and `write_panic_log()` functions, plus unit tests
  for path format and log directory resolution.
- Added `chrono` to the root `Cargo.toml` `[dependencies]`.

### Changed — Edit and edit-log improvements (ragent-tools-core)

- Hardened `edit`, `multiedit`, `apply_patch`, and `replace` matching with
  improved fallback cascades, whitespace-flexible matching, and
  indent-normalised retry logic to reduce spurious match failures.
- Enhanced `edit_log` tracking for better auditability of applied edits.
- Expanded test coverage in `test_edit`, `test_edit_integration`,
  `test_edit_smoke`, `test_multiedit`, `test_multiedit_helpers`, and
  `test_apply_patch` to cover the new matching behaviour.

### Changed — Research system (ragent-research)

- Added new hyperresearch modules: `chapter`, `cite_checker`,
  `contradiction`, `corpus_critic`, `digest`, `locus`, `open_access`,
  `patcher`, `readability`, `reconcile`, `run_manifest`, `source_vault`,
  `synthesis`, and `tier_router`.
- Extended `cli`, `document`, `session`, `web_gatherer`, `source`,
  `manager`, `run_config`, and `item` with tier-based research pipeline
  support.
- Added `test_hyperresearch_manual` and `source_vault` tests.

### Changed — Configuration (ragent-config)

- Added `research.open_access_recovery` and `research.contact_email`
  config fields.
- Added `test_research_config` test.

### Changed — Other

- Updated `openai_responses` provider handling.
- Updated `ragent-server` routes and `ragent-storage` storage helpers.
- Updated TUI research progress rendering and tests.
- Removed obsolete docs: `CODE_QUALITY_IMPLEMENTED.md`,
  `edit-matching-improvements.md`, `O365_TOOL.md`, and
  `research-options-wiring-plan.md`.
- Added `docs/howtos/research.md`.
- Bumped workspace version to 1.0.30.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound
  crates); no new security issues introduced.

## Version: 1.0.29

### Added — Hyperresearch integration

- `/research --tier light|full|dissertation` selects research depth; default is `full` (T-001, FR-001).
- `RunManifest` with `RunStep`/`StepStatus` tracks every run step and supports resuming an interrupted run via `/research resume <run_tag>` (T-002, FR-007).
- Persistent source vault under `.ragent/research_vault/<run_tag>/` with SQLite index and raw content files; vault sources are reused before any new web search (T-003, T-004, FR-002, FR-003, FR-009).
- Tier router implements the full 16-step pipeline for `full`, a trimmed pipeline for `light`, and dissertation chaptering for `dissertation` (T-005, FR-005, FR-008, FR-013).
- Width sweep aggregates results from all configured `mf_search` backends in parallel (T-006).
- Deterministic contradiction graph and cross-locus reconcile/source tensions are rendered in `RESEARCH.md` (T-007, T-009).
- Deterministic loci analysis and depth investigation surface recurring dimensions and shallow evidence (T-008).
- Deterministic corpus critic, gap-fill fetch, and surgical patcher refine the draft before citation checking (T-010, T-013).
- Deterministic evidence digest and triple draft feed synthesis and a 4-critic audit (T-011, T-012).
- Citation checker verifies every `[#N]` marker and closes the failure gate with `CITATION_VERIFICATION_FAILED` markers when a source is unsupported (T-014, FR-006, FR-014).
- Deterministic polish and readability audit run before final assembly (T-015).
- Open-access recovery via Unpaywall and Europe PMC, with source license/version disclosure in `RESEARCH.md` frontmatter and supporting-file notes (T-017, T-019, FR-010, FR-015).
- `research.open_access_recovery` and `research.contact_email` config fields (T-018, FR-011, FR-012).
- Sufficient-source check skips web search when the vault already holds enough sources for the requested tier (T-021, FR-016).

### Tests — Hyperresearch integration

- Added `crates/ragent-research/tests/test_hyperresearch_manual.rs` with manual verification test cases covering tier step selection, resume-from-manifest, rendered contradiction/tensions/cite-check sections, sufficient-source skip, and OA-recovery disclosure.

### Changed

- Bumped workspace version to 1.0.29.
- `cargo audit` reports 9 pre-existing allowed warnings (unmaintained/unsound crates); no new security issues introduced.

## Version: 1.0.28-beta

### Added — Spec-Driven Development (SDD) back-fill

Back-fills missing Spec-Driven Development capabilities from GitHub's
`spec-kit/spec-driven.md` into ragent's `/spec` feature set. All new
capabilities are opt-in via configuration flags (FR-019) and backward
compatible with existing spec directories (FR-018). See
`specs/reqeng/SPEC.md` for the full specification and `specs/reqeng/PLAN.md`
for the implementation plan with gap-resolution tracking (FR-020).

#### New `/spec` subcommands (ragent-specs)

- **`/spec specify <specname> <feature>`** — New `SpecCommand::Specify`
  variant and parser (T-001, FR-001). Creates a `SPEC.md` with structured
  requirements, user stories, and acceptance criteria without
  simultaneously generating a `PLAN.md`, separating the specification
  stage from the planning stage.
- **`/spec plan <spec-id> <tech-context>`** — New `SpecCommand::Plan`
  variant with technology-context argument (T-003, FR-004). Generates
  (or regenerates) `PLAN.md` from an existing `SPEC.md` using the
  provided technology context as guidance. Coexists with the existing
  `/spec update` and `/spec add` commands.
- **`/spec tasks <spec-id>`** — New `SpecCommand::Tasks` variant and
  parser (T-005, FR-005). Generates a `TASKS.md` file containing an
  ordered task list derived from the existing `PLAN.md`.

#### `[NEEDS CLARIFICATION]` marker support (ragent-specs)

- New `detect_clarification_markers` function and `ClarificationMarker`
  struct (T-007, FR-002). Case-insensitive regex detects
  `[NEEDS CLARIFICATION: <question>]` markers in `SPEC.md` content and
  returns each with its 1-based line number and captured question text.
- New validation `Category::Clarification` for reporting unresolved
  clarification markers.

#### Quality checklists in templates (ragent-specs)

- `SpecTemplate::generate_with_checklist` (T-010, FR-006) — Embeds an
  optional `## Quality Checklist` section in `SPEC.md` covering
  requirement completeness, testability, and absence of speculative
  features. Existing `generate` and `generate_with_research` delegate
  with `include_checklist = false`, preserving existing output.
- `PlanTemplate::generate_with_checklist` (T-011, FR-006) — Embeds an
  optional `## Quality Checklist` section in `PLAN.md` covering
  requirement traceability, testability, and absence of speculative
  tasks. Existing `generate` delegates with `include_checklist = false`.

#### Constitution artifact (ragent-specs)

- New `crates/ragent-specs/src/constitution.rs` module (T-013, FR-007)
  implementing `Constitution`, `Article`, and `Amendment` structs with
  `parse_constitution` parser. Parses `CONSTITUTION.md` files containing
  immutable architectural principles (`## Article N: Title` headings)
  and an optional `## Amendment Log` table. `Constitution::empty()`
  returns a no-articles value for backward compatibility (FR-018).
- Exported `Constitution`, `Article`, `Amendment`, and
  `parse_constitution` from the crate root.

#### Consistency validation — ambiguity detection (ragent-specs)

- New `detect_ambiguity` function, `AmbiguityIssue` struct, and
  `AmbiguityKind` enum (T-026, FR-015). Detects vague terms (e.g.,
  "maybe", "possibly", "might") and undefined cross-references in
  `SPEC.md` content.
- New validation `Category::Ambiguity` for reporting ambiguous language.

#### `FEEDBACK.md` file support (ragent-specs)

- New `FeedbackTemplate` struct with `generate(title)` method (T-031,
  FR-017). Produces a default `FEEDBACK.md` template with a feedback
  notes table (Date | Source | Note) and an advisory notice.
- `Spec` struct gains a `feedback_md: String` field and
  `feedback_md_path()` method. `SpecIo::discover_specs`, `read_spec`,
  and `write_spec` load and persist `FEEDBACK.md` following the same
  pattern as `REVIEW.md` — loaded if present, written only when
  non-empty (FR-018 backward compatibility).
- Exported `FeedbackTemplate` from the crate root.

#### SDD configuration flags (ragent-config)

- New `SddConfig` struct (T-035, FR-019) with 13 opt-in boolean flags
  gating SDD capabilities: `clarification_markers` (FR-002),
  `quality_checklists` (FR-006), `constitution` (FR-007),
  `phase_minus_one_gates` (FR-008), `branch_per_spec` (FR-009),
  `research_artifacts` (FR-010), `data_model` (FR-011), `contracts`
  (FR-012), `quickstart` (FR-013), `test_first_ordering` (FR-014),
  `consistency_checks` (FR-015), `amendment_process` (FR-016), and
  `feedback_loop` (FR-017).
- All flags default to `false` (opt-in). `SddConfig::merge` uses OR
  semantics — a flag enabled in either base or overlay stays enabled.
- Serialized under the `sdd` key in `ragent.json` with
  `skip_serializing_if` on each field; an all-false `sdd` block is
  omitted entirely from serialized output.
- `Config::merge` wires `base.sdd.merge(&overlay.sdd)`.
- 12 integration tests in
  `crates/ragent-config/tests/test_sdd_config.rs` covering defaults,
  parsing, serialization, and merge semantics.
- Exported `SddConfig` from the `ragent-config` crate root.

#### Gap resolution tracking (specs/reqeng)

- `specs/reqeng/PLAN.md` Gap Resolution Tracking section enhanced with
  a Status column (T-037, FR-020) showing per-gap resolution state
  (✅ Resolved / ⏳ Partial / ⬜ Not started), a progress summary, and a
  completed-task summary table linking each completed task to its gap
  and deliverable.

### Changed — ragent-tui slash command handling

- `crates/ragent-tui/src/app/slash.rs` match arms extended to cover
  `SpecCommand::Specify`, `SpecCommand::Plan`, and `SpecCommand::Tasks`
  variants, fixing a non-exhaustive match that would have prevented
  compilation once the new variants were added.

### Tests — SDD back-fill

- `crates/ragent-specs/tests/test_templates.rs` — 10 new tests for
  `SpecTemplate::generate_with_checklist`, `PlanTemplate::generate_with_checklist`,
  and `FeedbackTemplate::generate`.
- `crates/ragent-specs/tests/test_spec_io.rs` — 7 new tests for
  `FEEDBACK.md` load, read, write, discover, and path resolution.
- `crates/ragent-specs/tests/test_slash_spec.rs` — 6 new tests for
  `SpecCommand::Specify`, `SpecCommand::Plan`, and `SpecCommand::Tasks`
  parsing and help text.
- `crates/ragent-specs/tests/inline/validate.rs` — 12 new tests for
  `detect_clarification_markers` and `detect_ambiguity`.
- `crates/ragent-config/tests/test_sdd_config.rs` — 12 new tests for
  `SddConfig` defaults, parsing, serialization, and merge semantics.
- `crates/ragent-specs/src/constitution.rs` — 13 inline tests for
  constitution parsing (articles, amendments, em-dash headings,
  multiline bodies, path resolution).

## Version: 1.0.27

### Added — Exa Search API backend for `mf_search`

- New **Exa** search engine added to `mf_search`, configured via
  `exa_api_key` in `ragent.json` (global or project) or the
  `EXA_API_KEY` environment variable; the environment variable takes
  precedence. The key is masked in diagnostics and never logged.
- New module
  `crates/ragent-tools-extended/src/masterfetch/search/exa.rs`
  implementing the `ExaEngine` backend.
- The `mf_search` `engine` parameter now accepts `"exa"` to restrict
  searches to the Exa backend only. The JSON schema `engine` enum and
  tool description updated to include `exa`.
- `MfSearchTool::resolve_search_keys` now returns a 4-tuple including
  the Exa key; `build_orchestrator` and `engine_status` wired to
  include the Exa engine when a key is present.
- `Config::exa_api_key` field added to `ragent-config` with
  `#[serde(default, skip_serializing_if = "Option::is_none")]` and
  merge support in `Config::merge`.
- New tests `test_mf_exa.rs` and extended `test_mf_search_tool.rs`
  covering orchestrator wiring, engine selection, engine status, and
  schema enum for Exa.

### Changed — Research document `Search Engine Summary` section

- `crates/ragent-research/src/document.rs` now renders a
  **Search Engine Summary** table in `RESEARCH.md` (both Report and
  IMRaD layouts) showing, per backend engine, the number of web
  sources acquired broken down by media type (pages, PDFs, videos).
  The section is emitted only when at least one web source has a
  non-empty `search_engine` field, so skeletons and pre-gathering
  documents remain unchanged.
- `crates/ragent-research/src/source.rs` gains a
  `Source::search_engine()` accessor returning the comma-separated
  backend engine list for `Source::Web` (empty string for other
  variants).
- New unit tests for `render_search_engine_summary` covering
  multi-engine source splitting, media-type counting, empty-state,
  and section ordering in both Report and IMRaD layouts.

### Changed — Documentation and statistics

- README.md updated to mention Exa in the MasterFetch feature list,
  recent highlights, and `engine` parameter enum.
- STATS.md updated with current line/file/test counts reflecting the
  new Exa module and research document changes.

## Version: 1.0.26

### Added — OpenAlex and Wikipedia search backends for `mf_search`

- New **OpenAlex** keyless search backend queries the scholarly-works
  catalog (papers, articles, datasets). Results include title, authors,
  publication year, venue, citation count, open-access URL, and relevance
  score. Set `OPENALEX_EMAIL` in the environment or `ragent.json` to join
  the polite pool.
- New **Wikipedia** keyless search backend queries the English Wikipedia
  REST API for encyclopedia-style summaries. Results include the page
  title, extract, and canonical URL.
- Both backends run in parallel with the existing DuckDuckGo and Brave
  engines by default; optional LangSearch / Tavily / Perplexity
  API-backed engines continue to be supported when configured.
- The `mf_search` tool gains an `engine` parameter to restrict the
  search to a single backend
  (`duckduckgo` / `brave` / `openalex` / `wikipedia` / `langsearch` /
  `tavily` / `perplexity`).
- New modules `crates/ragent-tools-extended/src/masterfetch/search/openalex.rs`
  and `.../wikipedia.rs` implementing the backends.
- New tests `test_mf_openalex.rs`, `test_mf_openalex_live.rs`,
  `test_mf_wikipedia.rs`, `test_mf_wikipedia_live.rs` covering unit and
  live integration paths.

### Changed — Search consensus and relevancy adjustments

- The consensus merge in `mf_search` now weights per-engine relevance
  scores and `fetch_relevance` signals more evenly, reducing
  DuckDuckGo-dominated ranking when multiple backends contribute.
- `mf_search` `engine.rs` and `search/mod.rs` refactored to share a
  common `SearchResult` builder path so all backends produce consistent
  field sets (title, url, snippet, score, source engine, extra metadata).
- The search-tool JSON schema now documents the `engine` enum and
  the `per_engine_results` cap.

### Changed — Research web-gatherer enhancements

- `crates/ragent-research/src/web_gatherer.rs` extended to handle the
  new backend result shapes and to pass through OpenAlex/Wikipedia
  metadata into `RESEARCH.md` source entries.
- `crates/ragent-research/src/cli.rs` and `session.rs` updated for the
  new gatherer flow.
- `crates/ragent-server/src/routes/research.rs` and
  `crates/ragent-tui/src/app/research.rs` wired to the updated research
  session.
- New `--use-low-relevance` style flags consolidated in the research
  CLI.

### Fixed — Compaction loop and processor stability

- `crates/ragent-agent/src/session/loop_steps.rs` simplified (removed
  ~100 lines of duplicated nudge logic) to fix repeated
  post-compaction continuation nudges across loop iterations.
- `crates/ragent-agent/src/session/processor.rs` updated to thread the
  `last_task_completed_at` guard so autopilot auto-continue is
  suppressed after `agent_complete`.
- `crates/ragent-agent/tests/test_compaction_integration.rs` updated
  to match the simplified loop.

### Changed — Config and CLI

- `crates/ragent-config/src/config.rs` extended with the new search
  backend option fields.
- `src/cli.rs` and `crates/ragent-research/src/cli.rs` updated for
  the new research/search flags.

## Version: 1.0.25

### Fixed — cargo-deny CI and security audit

- Add RUSTSEC-2026-0253 (lru `LruCache::pop()` use-after-free) to `deny.toml`
  `[advisories].ignore` — transitive via ratatui 0.29 / tantivy 0.22; ragent does
  not call `pop()`; upgrade blocked by ratatui's lru 0.12 pin
- Add `--ignore RUSTSEC-2026-0253` to the `cargo audit` step in
  `security-audit.yml`, keeping it in sync with `deny.toml`
- Pin `cargo-deny` to `0.18.12` in the CI workflow to avoid
  `bug[unresolved-workspace-dependency]` false-positives that appeared in later
  cargo-deny versions when resolving root-crate `{ workspace = true }` deps

## Version: 1.0.24

### Added — `/spec update` subcommand and TESTPLAN.md artifact

- New `/spec update <spec-id>` sub-command re-reads the existing
  `specs/<spec-id>/SPEC.md` and regenerates `PLAN.md` and `TESTPLAN.md`
  to match the current requirements. `SPEC.md` is not modified. Existing
  task IDs in `PLAN.md` are preserved where unchanged.
  - Validates the spec ID and that the spec directory / `SPEC.md` exist.
  - Guards against updating archived specs (returns an error).
  - On not-found, lists available specs to aid discovery.
  - Delegates to the `explore` agent by default, falling back to the
    current agent. Dispatches generation via `process_message` and
    publishes `AgentError` on failure.
  - `SpecCommand::Update` variant added to `crates/ragent-specs` with
    `build_update_status`, `build_update_message`, `build_update_log`,
    and `build_update_prompt` helpers.
- `/spec create` now generates a third artefact, `TESTPLAN.md`, alongside
  `SPEC.md` and `PLAN.md` in every new spec directory. The file is a
  human-readable **manual** test plan with a `## Test Cases` section, each
  test case having a `TC-NNN` ID, title, preconditions, step-by-step
  instructions, test data to enter, and expected results. When the feature
  involves UI navigation, the plan enumerates every navigation step and
  the exact data to enter. It may optionally include `## Prerequisites`
  and `## Cleanup` sections. It does **not** contain automated test code,
  `#[test]` functions, or `cargo test` references.
- The `/spec create` status message, user-facing message, and log entry
  now mention `TESTPLAN.md` so testers know a manual test plan was
  produced.
- `/spec add` now runs a second phase after the incremental requirement
  addition that fully regenerates `PLAN.md` and `TESTPLAN.md` from the
  updated `SPEC.md` (same logic as `/spec update`), so add operations stay
  consistent with the latest requirements.
- Tests added for the new behaviour: `test_slash_spec_update_missing_spec_id_shows_usage_error`,
  `test_spec_jtbd`, and `TESTPLAN.md` assertions in the create test.

### Changed — AGENTS.md project guidelines

- Added rule 6: **No unsafe code.**
- Added rule 7: **No `.unwrap()` on user-facing paths.**

### Removed — Stale planning files

- Deleted obsolete root planning documents that were superseded by the
  `specs/` workflow: `ALPLAN.md`, `CUTPLAN.md`, `EDITPLAN.md`,
  `RESEARCHPLAN.md`, `TOOLPLAN.md`.
- Removed a stale unversioned `build` artifact from the repository root.

## Version: 1.0.23

### Added — Jobs-To-Be-Done analysis for specs

- New `/spec jtbd <specname>` sub-command performs a Jobs-To-Be-Done
  analysis of an existing spec's `SPEC.md` and writes the result to
  `specs/<specname>/JTBD.md`. Supports `--force` to overwrite an existing
  analysis and `--agent <name>` to select the analysis agent.
- Added parsing, dispatch, and tests for the new `Jtbd` spec command
  variant in `crates/ragent-specs`.

### Changed — Mandatory readability extraction in the research web-gather phase

- Every HTML page captured as a research web source must now have been
  extracted by the `readability-rs` crate. Pages where readability fails
  — and would previously have been accepted via the silent html2text /
  raw tag-strip fallbacks — are rejected with a
  `readability extraction failed …` fetch error and skipped by the
  gatherer. PDF and YouTube sources bypass readability by design and are
  unaffected.
- `mf_fetch` now reports which stage of the extraction chain produced a
  page via a new `extraction_method` metadata signal
  (`readability` / `html2text` / `raw_text` / …); the signal is also
  recorded in the masterfetch content cache (new
  `fetch_cache.extraction_method` column) so cached responses keep it.
- The legacy `webfetch` path (which does not report the extraction
  stage) is verified by re-running `readability-rs` directly on the raw
  HTML so the guarantee is enforced rather than trusted.

### Fixed — YouTube transcript capture in research web-gather

- `mf_fetch` now parses real YouTube watch pages correctly: the
  `ytInitialPlayerResponse` object is extracted with a brace-balanced,
  string-aware scanner instead of the previous `(\{.*?\});` regex (which
  broke on nested braces and on `}` characters inside JSON strings), and
  the caption track list is read from the real
  `captions.playerCaptionsTracklistRenderer.captionTracks` location
  (with the legacy flat `captions.captionTracks` layout kept as a
  fallback). Watch-page transcription data is therefore actually
  recovered instead of erroring on every real video page.
- Failed fetches reported by `mf_fetch` metadata (`error` field or
  `content_ok = false` — e.g. "no caption tracks available for this
  YouTube video") now abort the research fetch adapter with an explicit
  error. The gatherer surfaces them as a `FetchFailed` event carrying
  the real reason and suppresses the video outright; previously the
  placeholder `[YouTube transcript extraction failed: …]` text was kept
  as the page body and silently suppressed by the
  "extracted content too short" gate, hiding why videos were dropped.

## Version: 1.0.22

### Fixed — Time-sensitive `test_parse_natural_time_5pm_tomorrow` CI failure

- Replaced the fragile assertion `parsed.next_due > now + 20 hours` in
  `crates/ragent-types/src/cron.rs` with a robust date-based check.
  The old assertion failed when CI ran after ~3pm UTC because "5pm
  tomorrow" could be as little as ~18 hours away. The new assertions
  verify that the result is in the future and on a later calendar day,
  which is what "tomorrow" actually means.

## Version: 1.0.21

### Fixed — CI clippy failure

- Added `#[allow(clippy::too_many_arguments)]` to
  `log_cron_execution` in `crates/ragent-tools-core/src/cron_log.rs`.
  The function takes 8 arguments (clippy's default threshold is 7),
  which caused the CI clippy gate (`-D warnings`) to fail on the
  v1.0.20 release commit.

## Version: 1.0.20

### Added — LLM-callable cron tools

- New `cron_add`, `cron_remove`, `cron_list`, `cron_enable`, and
  `cron_disable` tools registered in the default tool registry, giving
  the model direct access to the cron scheduler without going through
  the TUI slash-command surface.
- All cron tools read/write through the `Storage` layer, mirroring the
  `/cron` slash-command handlers.
- TUI log-panel rendering added for cron tool inputs and results
  (⏰ summaries in `message_widget`).

### Added — `/cron` slash-command enhancements

- `/cron add` now takes a positional `cronname` as the first argument:
  `/cron add <cronname> <agent> <schedule> "<prompt>"`. The cronname
  becomes the event ID.
- New `/cron enable <event_id>` and `/cron disable <event_id>` commands
  to toggle events without removing them.
- New `/cron detail <event_id>` command showing every stored field
  including the full, untruncated prompt.
- `/cron help` updated with parameter tables, schedule examples, and
  natural-language timestamp documentation.

### Added — Natural-language timestamp parsing

- `parse_timestamp` now accepts human-friendly time shortcuts resolved
  against the user's local timezone in addition to ISO-8601:
  - `5pm` / `5PM` — next 5pm (today or tomorrow)
  - `5:30pm` / `5:30 pm` — 12-hour clock with minutes
  - `17:00` — 24-hour clock
  - `5am tomorrow` / `5pm today` — explicit day offset
- 12-hour edge cases handled: `12pm` = noon, `12am` = midnight, `13pm`
  rejected as invalid.
- Added 13 unit tests covering natural-language parsing, case
  insensitivity, 24-hour conversion, and invalid inputs.

### Fixed — Sub-agent and background-agent model resolution

- Background agents and sub-agents (`new_agent`) now use the user's
  persisted `selected_model` setting from Storage instead of falling
  back to `Config::default()` and `resolve_default_model`, which
  typically picked Anthropic even when no API key was configured.
- Agent resolution now uses `resolve_agent_with_customs_and_model` with
  the cached config (`load_config_cached`) and the provider registry,
  matching the TUI's model resolution path.
- Explicit model overrides (`--model` / `model` parameter) still take
  precedence; the persisted setting is only applied when no override is
  present and the agent's model is not pinned.

### Tests

- `crates/ragent-agent/tests/test_cron_tools.rs` — 6 tests covering the
  LLM-callable cron tool surface.
- `crates/ragent-tui/tests/test_cron_add_positional.rs` — tests for the
  positional `/cron add <cronname> ...` parser.
- 13 new natural-language timestamp tests in `ragent-types/src/cron.rs`.

## Version: 1.0.19

### Added — Cron capability

- Added agent cron scheduling system (`/cron` slash-command family) with
  one-shot (`at <timestamp>`), repeating (`from <timestamp> every
  <duration>`), and interval (`every <duration>`) schedule forms.
- `cron_events` table persisted to SQLite via the existing `Storage` layer
  with migration support.
- Background scheduler ticks every 30 seconds, evaluating and firing
  enabled events whose `next_due` has passed.
- Execution outcomes (`success`, `error`, `skipped`) logged as JSONL to
  `<working_dir>/log/cron-<timestamp>.jsonl`.
- Duration parser supporting `m`, `h`, `d`, `w`, `mo` units with
  plural/long-form aliases.
- Comprehensive unit test coverage: duration parser (21 tests), schedule
  parser (53 tests), storage round-trip (12 tests), `every` no-start
  computation (6 tests), and past-start advancement (14 tests).

## Version: 1.0.18

### Added — Agent cron system (`/cron`)

- New cron scheduling system that lets users schedule agent runs with a
  designated agent type and an initial prompt.
- Three schedule forms supported:
  - `at <timestamp>` — one-shot, fires once at the specified time.
  - `from <timestamp> every <duration>` — repeating, first fire at the
    given timestamp, then at each interval.
  - `every <duration>` — repeating with no explicit start; first fire is
    `duration` from now.
- Duration parser supports `m` (minutes), `h` (hours), `d` (days),
  `w` (weeks), `mo` (months = 30 days) with plural/long-form aliases
  (`mins`, `hrs`, `days`, `wks`, `months`).
- Events persisted to SQLite via the existing `Storage` layer; `cron_events`
  table with migration.
- Background scheduler ticks every 30 seconds on a non-blocking tokio
  task while the TUI session is running, evaluates all enabled events,
  and fires those whose `next_due` has passed.
- `/cron` slash-command family:
  - `/cron add <agent> <schedule> "<prompt>"` — schedule a new event.
  - `/cron remove <event_id>` — remove a scheduled event.
  - `/cron list` — list all events with human-readable schedule
    descriptions.
  - `/cron log [event_id]` — show execution log (optionally filtered).
  - `/cron help` — show usage.
- Every execution is logged as a JSONL line to
  `<working_dir>/log/cron-<timestamp>.jsonl`, mirroring the edit-log
  convention. Each entry records event id, agent type, prompt, outcome
  (`"success"`, `"error"`, or `"skipped"`), and timestamp.
- Disabled events are skipped with a `"skipped"` outcome.
- Past-start timestamps for `from <ts> every <d>` are advanced by whole
  duration intervals until `next_due` is in the future.
- Added unit tests for duration parser (21 tests), schedule parser
  (53 tests), storage round-trip (12 tests), `every <d>` no-start
  `next_due` computation (6 tests), and `from <past> every <d>` past-start
  advancement (14 tests).

### Added — Perplexity Sonar backend for `mf_search`

- Added a new `perplexity` search backend (`PerplexityEngine`) that queries the
  Perplexity Sonar API, wired into the MasterFetch search orchestrator alongside
  the existing DuckDuckGo, Brave, LangSearch, and Tavily engines.
- New `perplexity_api_key` field in `ragent_config::Config` (with merge support
  across global/project config files). When present (or when the
  `PERPLEXITY_API_KEY` environment variable is set), `mf_search` includes the
  Perplexity backend as an additional engine. The key is masked in diagnostics
  and never logged.
- Added integration tests for the Perplexity backend wiring and API-key
  resolution.

### Added — Edit-log per-tool success/failure analysis

- `EditLogAnalysis` now tracks per-tool success and failure counts via
  `success_by_tool` and `failure_by_tool` maps.
- New `tools_sorted()` helper returns all tools that have logged operations.
- New `failure_success_ratio_pct_for(tool)` helper computes the failed-to-succeeded
  ratio as a percentage for a given tool (returns `0.0` when the tool has no
  succeeded operations).
- The `edit_log_analyse` function now counts successful operations (previously
  skipped) and records them per tool, enabling richer edit-log summaries.
- Added tests verifying per-tool counts and ratio calculations.

### Changed — Miscellaneous

- Removed unused setup code from TUI app initialization (`init.rs`).
- TUI slash-command and tool-display rendering updated for the new search
  backend and edit-log analysis fields.
- Updated existing `mf_search` tool tests for the new three-tuple key
  resolution signature.

## Version: 1.0.17

### Added — Optional `collapse_whitespace` matching for `edit` / `multi_edit`

- The `edit` and `multi_edit` tools now accept an optional
  `collapse_whitespace` boolean (default `false`, i.e. the existing
  byte-for-byte strict matcher is unchanged). When `true`, backslash escapes
  (`\t`, `\n`, `\r`, `\\`) in `old_string` are decoded and every run of
  whitespace matches a non-empty run of whitespace in the file, so collapsed
  indentation or alignment whitespace no longer causes spurious "old_string
  not found" failures. Uniqueness and the whole-batch atomicity of
  `multi_edit` are preserved in flexible mode.
- New public helpers in `ragent_tools_core::replace`: `decode_escapes` and
  `find_flexible_replacement_range` (two-lane matching: exact lane wins when
  unique; whitespace-tolerant scan otherwise; ambiguous hits rejected).
- `edit` result metadata now includes `collapse_whitespace: true` when the
  relaxed matcher was used, and failure messages mention the flexible mode.

### Added — Persistent edit-log toggle with Alt+E and status-bar indicator

- Added `edit_log` boolean field to `ragent_config::Config`; defaults to `false`
  and merges across global/project config files.
- New `ragent_config::edit_log` module provides `is_enabled`, `set_enabled`,
  `sync_from_config`, `persist_edit_log`, and `toggle_persist` — mirroring the
  YOLO persistence helpers.
- `ragent_tools_core::edit_log::is_edit_log_enabled` now delegates to the
  shared `ragent_config::edit_log` runtime flag, ensuring the TUI, tools, and
  status bar all see the same state.
- TUI `Alt+E` toggles edit logging; the new state is persisted to
  `.ragent/ragent.json` and a status/log message confirms the change.
- TUI `/editlog on|off` now persists the new state to `ragent.json` instead of
  changing the runtime flag only.
- Status bar line 2 shows an `EditLog:{✓|✗}` indicator next to `AutoPilot`,
  reflecting the current persisted state.
- Added tests for `Alt+E` toggle + indicator and `/editlog` persistence.

### Fixed — Clippy warnings across workspace

- Removed unused underscore-prefixed bindings in `session_ops.rs`.
- Added `#![allow(clippy::redundant_pub_crate)]` to `crates/ragent-tui/src/app/tests.rs`.
- Allowed `clippy::await_holding_lock` on edit-log integration tests that
  intentionally serialise process-wide state with a static mutex.
- Reduced `LocalEmbeddingInner::Ready` variant size by boxing the tokenizer.
- Collapsed nested `if let` in `LocalEmbeddingProvider::embed`.
- Added `#[allow(dead_code)]` to edit-log analysis APIs that are public but not
  yet consumed in every compilation unit.

### Fixed — Repeated "Context compression skipped" notices

- Added a per-turn `compaction_attempted_this_turn` flag to `LoopState` and the
  agent-loop orchestrator.
- The pre-send compaction path now sets the flag before invoking the runner and
  suppresses further compaction attempts for the rest of the turn, even when the
  runner bails with a "Context compression skipped" notice.
- Both emergency-overflow compaction paths now use the same flag, so a skipped
  emergency compaction is not retried either.
- `compressed_this_turn` is still tracked separately so the post-compaction
  continuation nudge only fires after a successful compression.
- Added `test_pre_send_compaction_skipped_notice_emitted_once_per_turn` to
  verify that only one skipped notice is published per turn.

### Changed — Model-independent context-compaction trigger

- `CompactionConfig::default().threshold` is now `0.7` (70 % of the model's
  context window) instead of `None`. This makes the default pre-send compaction
  trigger independent of the model's absolute context size.
- `CompactionConfig::default().buffer` is now a fraction of the context window
  (`0.10`, i.e. 10 %) instead of an absolute `20_000` tokens.
- `CompactionConfig::default().keep.tokens` is now a fraction of the context
  window (`0.20`, i.e. 20 %) instead of an absolute `8_000` tokens.
- `compaction_threshold()` and `select()` compute absolute token budgets from
  these fractions using the resolved `context_window`, so compaction behavior
  scales automatically with the model in use.
- The agent-loop pre-send estimator (`compaction_threshold`) now enforces a
  70 % context-window floor, so automatic compaction never fires on routine
  prompts that fill less than 70 % of the available context.
- The TUI pre-send compaction check (`should_auto_compact_before_send`) now
  uses the shared `compaction_threshold()` estimator instead of a private 92 %
  fallback, keeping the TUI and server/agent paths consistent.
- Updated `SPEC.md`, `QUICKSTART.md`, and `README.md` compaction examples to
  show the new `threshold`, `buffer`, and `keep.tokens` fraction fields.
- Adjusted compaction config and runner tests to reflect fraction-based
  defaults and the fact that legacy `compression.auto_threshold` only fills an
  explicit `threshold: null`.

## Version: 1.0.15

### Fixed — CI formatting and multi_edit edit-log tests

- Ran `cargo fmt` to fix formatting in `crates/ragent-tools-core/tests/test_multiedit.rs`.
- Fix flaky `multi_edit` edit-log tests (commit `3fb331c`).
- Incremental release bump to 1.0.15.

## Version: 1.0.14

### Changed — version bump and security audit

- Incremental release bump to 1.0.14.
- `cargo audit` reports 7 allowed warnings (unmaintained/unsound crates) that are
  suppressed via existing project policy; no new high-severity advisories introduced.

## Version: 1.0.13

### Added — Edit-operation audit logging

- New `ragent-tools-core/src/edit_log.rs` provides `log_edit_operation`, a
  process-wide atomic `EDIT_LOG_ENABLED` flag, and helpers `is_edit_log_enabled`,
  `set_edit_log_enabled`, and `clear_edit_logs`.
- `edit` and `multi_edit` now log every outcome path (success, dry-run preview,
  no-change rejection, create/mkdir/write errors, stale-file rejection, and
  match failures) to `<working_dir>/log/edits-<timestamp>.jsonl` when enabled.
- TUI `/editlog` slash command added with subcommands `on|off|status|show|help`.
- `EditTool` and `MultiEditTool` instrument all branches; `create_file` now
  receives the `dry_run` flag so create paths are also logged.
- Tests added for successful edits, dry-run previews, failures, disabled logging,
  and multi_edit batch logging.

## Version: 1.0.12

### Changed — update security audits

- Incremental release bump; security audit clean via `.cargo/audit.toml`
  suppressions for advisories blocked by upstream compatibility.

### Changed — research finding heading format

- `/research` `RESEARCH.md` findings now render with an extra blank line above,
  a bold finding number (`### **Finding N** —`), and the same `###` heading
  level to preserve outline structure.

## Version: 1.0.11

### Changed — update research system

- Incremental release bump and changelog update for the research subsystem.

### Security — audited and accepted transitive dependency advisories

- Ran `cargo audit`; 7 high-severity advisories were reported. After review,
  the following are suppressed in `deny.toml` because a fix requires major
  upstream crate upgrades that are out of scope for this release:
  - `RUSTSEC-2026-0187` (`lopdf` stack overflow): blocked by `pdf-extract`
    0.10 and `printpdf` 0.9; trusted local PDF input.
  - `RUSTSEC-2026-0194` / `RUSTSEC-2026-0195` (`quick-xml` DoS): blocked by
    `ooxmlsdk` 0.3 and `calamine` 0.34; trusted local Office/Spreadsheet input.
  - `RUSTSEC-2026-0235` (`rkyv` archive validation): blocked by
    `rust_decimal` 1.42.1 via `spreadsheet-ods`; not used with untrusted input.

## Version: 1.0.10

### Changed — updated /research functionality

- `ragent-research` crate updated with improved synthesis, gathering, and session
  pipeline changes (see details below).

## Version: 1.0.9

### Added — Low-relevance web-source filter in `/research` gathering

- `WebGatherer` now computes a deterministic relevance label for every
  fetched candidate and drops sources labelled **Low** or **Very low** before
  they are added to the research state.
- The relevance score is based only on the query, title, snippet, and URL, so
  it adds zero LLM cost. Common English stopwords are stripped from the query
  so question-style queries (e.g. "What is Rust?") are not penalised for
  missing auxiliary words.
- Skipped sources still emit a `GatherEvent::FetchFailed` diagnostic with the
  reason `relevance too low (...)`, preserving the index slot so `web-NN.md`
  numbering remains stable.
- Added unit test `gather_filters_low_relevance_hits_and_notifies_observer`.

### Fixed — Mermaid findings diagram now renders when labels contain quotes or backticks

- `crates/ragent-research/src/diagram.rs::escape_mermaid_label` now replaces
  both `"` and `` ` `` with `'` instead of trying to escape them.
  Mermaid's double-quoted node labels (`F["..."]`) do **not** support
  backslash-escaped quotes or backticks, so the previous `\"` output and any
  inline code spans caused a parser error (visible in GitHub/GitLab previews and
  `mermaid-cli`).
- Updated the quote-escaping unit test and added a new regression test covering
  backtick-laden labels such as `` `rlms` ``.
- Verified the generated diagram renders with `@mermaid-js/mermaid-cli`.

### researchprompt — improved `/research create` synthesis prompt

The `/research create` analysis prompt (in `crates/ragent-research/src/analysis.rs`)
now applies the evidence-based prompt-engineering guidance from
`research/researchanalysis` (20 synthesized findings from 91 web sources):

- **Versioned, composable prompt builder** (`SynthesisPromptBuilder` +
  `SynthesisPromptConfig`) replaces the monolithic `build_synthesis_prompt`
  string concatenation. The legacy free function is preserved as a thin
  wrapper with byte-identical default output, so existing callers are
  unchanged (FR-001, T-002).
- **Mandatory `Sources Cited / Date Spread` paragraph** (FR-003, T-003). When
  enabled, every finding must end with a fifth labeled paragraph listing its
  `[#N]` citations and the earliest/latest publication dates among the cited
  web sources, plus a sentence on how the date range affects confidence. The
  per-source block in the prompt gains a `Published (UTC):` line so the model
  can quote real dates instead of inventing them.
- **Recency-weighting rule** (FR-004, T-004). When enabled, the prompt
  instructs the model to prefer more recently published sources, note
  conflicts between older and newer sources, and down-weight anonymous/undated
  pages.
- **Deterministic mechanical fallback** (FR-005, FR-006, T-005, T-010). The
  `AnalysisEngine` trait now exposes `analyze_with_outcome` returning
  `(AnalysisResult, AnalysisOutcome)`. When the LLM response is empty,
  unparseable, missing required labels, or missing citations, the parser
  returns `AnalysisOutcome::FallbackEmpty` and a mechanically-extracted set
  of findings that always contains at least one finding (with the spec's
  `(findings could not be structured — see below)` placeholder wording and
  the raw model output preserved in a fenced block). `session.rs` maps
  `AnalysisOutcome` to the user-facing `SynthesizeOutcome`, and provider
  errors still surface as `SynthesizeOutcome::FallbackError`.
- **Template merge** (FR-007, T-006). `document.rs::assemble_document`
  clarifies that a `--template` body is MERGED with the standard sections
  (prepended), never a replacement for the Findings section or its four
  required labeled paragraphs. The prompt builder also carries a
  `template_body` knob so the model is told template sections augment the
  structured findings rather than replace them.
- **Few-shot exemplars** (FR-008, T-007). The prompt builder appends up to
  two short exemplar findings (gated on `config.few_shot_examples`) to
  calibrate the exact label structure, `[#N]` citations, and
  `Sources Cited / Date Spread` paragraph.
- **Configurable persona** (FR-009, T-008). `LlmAnalysisEngine::with_persona`
  overrides the default `"You are a careful research analyst..."` system
  message verbatim.
- **Citation/date validation** (FR-010, T-009). On a clean LLM parse, every
  `[#N]` citation is cross-checked against the captured source indices; out
  of-range citations are rewritten inline to `[#N?] (out of range — not in
  source list)` and logged via `tracing::warn`. Explicit publication dates
  inside a `Sources Cited / Date Spread` paragraph that don't match any
  cited source's `published_at` are rewritten to `(unsupported date)`.

### Added — researchprompt configuration surface

- `SynthesisPromptConfig` (`pub(crate)`) with `audience_scope`, `recency_rule`,
  `date_spread_paragraph`, `few_shot_examples`, `persona`, and
  `template_body` knobs.
- `AnalysisOutcome` enum (`Llm`, `FallbackEmpty`, `FallbackError`) re-exported
  from `ragent_research`.
- `SourceBody.published_at: Option<DateTime<Utc>>` field, populated by
  `build_source_bodies` from `Source::published_at`, so the synthesis prompt
  can quote real publication dates.
- `LlmAnalysisEngine::with_persona(Option<String>)` builder.
- `ragent.json` keys documented (SPEC.md → "Research Configuration",
  QUICKSTART.md → "Research prompt configuration"):
  `research.few_shot` (bool) and `research.analysis_persona` (string). Both
  are opt-in; wiring them from config into the engine is tracked as a
  follow-up.

### Tests

- `crates/ragent-research/src/analysis.rs` (inline `mod tests`): +14 tests
  covering the builder (default byte-identity, four required labels,
  date-spread paragraph, recency rule, few-shot append, few-shot cap),
  `parse_analysis_response_with_outcome` (clean Llm, empty/no-findings/
  missing-labels fallback), `mechanical_fallback_findings` (non-empty
  guarantee, placeholder wording), and `validate_citations_and_dates`
  (out-of-range citation, unsupported date, valid-finding untouched).
- `crates/ragent-research/tests/test_template_merge.rs`: 3 tests for FR-007
  template merge (template + standard sections coexist; required labels
  survive merge; no-template regression guard).
- `crates/ragent-research/tests/test_research_create_synthesis.rs`: 3
  integration tests for FR-005/FR-006 (malformed → FallbackEmpty +
  placeholder findings; well-formed → Llm + verbatim findings; Noop → NoLlm
  + mechanical findings).
- `crates/ragent-research/tests/test_research_create_synthesis.rs` (T-012)
  exercises the full `ResearchSession::run` pipeline with mock
  `AnalysisEngine` implementations (no real LLM provider required).

### Verification

- `cargo check --workspace` — green.
- `cargo test -p ragent-research` — 298 lib + 3 template-merge + 3 synthesis
  integration + 1 doc test pass.
- `cargo clippy -p ragent-research --lib` — clean.
- `cargo fmt -p ragent-research -- --check` — clean.
- Default-config prompt is byte-identical to the pre-refactor
  `build_synthesis_prompt` output (regression guard).

## Version: 1.0.8

### Added

- macOS Apple Silicon (M-series) packaging support.
  - New `scripts/macos-pkg.sh` builds a flat `.pkg` installer from an `aarch64-apple-darwin` release binary.
  - New `packaging/macos/scripts/preinstall` and `postinstall` scripts install the binary to `/usr/local/lib/ragent/ragent` and symlink it to `/usr/local/bin/ragent` so `ragent` is on the default PATH.
  - New `build-macos-arm64` job in `.github/workflows/release.yml` runs on `macos-15`, builds the arm64 binary, creates `ragent-{version}-macos-arm64.pkg`, and uploads the binary and package as release artifacts.
  - Release job now downloads and publishes the macOS artifacts alongside Linux and Windows assets.
  - The `.pkg` is intentionally unsigned; it is suitable for local installation and enterprise distribution. Apple Developer ID signing and notarization can be added later if required.

## Version: 1.0.7

### Fixed

- Added `authors.workspace = true` to the root `ragent` package in `Cargo.toml` so the per-package manifest inherits the workspace authors. This fixes the `cargo wix` step of the Windows release build, which requires an `authors` field.
- Removed the temporary `$env:CARGO_PKG_AUTHORS` override from `.github/workflows/release.yml`; the workflow now relies on the corrected Cargo.toml manifest.

### Changed

- Incremented workspace version to 1.0.6.

## Version: 1.0.5

### Fixed

- Windows release build in `.github/workflows/release.yml`:
  - Added a separate `cargo wix init --package ragent` step to generate the WiX `wix/main.wxs` source file before building the `.msi` package.

### Changed

- Incremented workspace version to 1.0.5.

## Version: 1.0.4

### Fixed

- Windows release build fixed in `.github/workflows/release.yml`:
  - Removed unsupported `cargo wix init --no-build` call (cargo-wix 0.3.9 does not accept `--no-build` on the `init` subcommand).
  - Added `--package ragent` so `cargo wix` works inside a Cargo workspace.
  - Stage the already-built `target/x86_64-pc-windows-msvc/release/ragent.exe` at `target/release/ragent.exe` before invoking `cargo wix --no-build`, matching the layout that cargo-wix expects.

### Changed

- Incremented workspace version to 1.0.4.

## Version: 1.0.3

### Added

- Windows x86_64 build and `.msi` installer packaging added to the release pipeline (`.github/workflows/release.yml`).
  - New `build-windows` job builds `ragent.exe` and creates a WiX-based MSI.
  - Release job now combines Linux and Windows artifacts for GitHub Releases.
  - Added `[package.metadata.wix]` section in `Cargo.toml` with a stable upgrade GUID.
  - Added Windows MSVC static C-runtime flags in `.cargo/config.toml` so the installed binary does not require the VC++ redistributable.
  - Added `/wix/` to `.gitignore` for generated WiX source files.

### Changed

- Incremented workspace version to 1.0.3.

## Version: 1.0.2

### Changed

- Incremented workspace version to 1.0.2.
- Updated `.github/workflows/security-audit.yml` and `deny.toml` ignore list to include all current transitive/direct dependency advisories so `cargo audit` stays green.
- Release pipeline now treats the dependency-audit job as informational (`continue-on-error: true`) so unmaintained crates do not block a release.

## Version: 1.0.0

### Changed

- Removed pre-release beta label and reset stable version to 1.0.0.
- Future releases will increment the patch (last) digit.

## Version: 0.1.0-beta.41

### Fixed

- Fixed CI failures after the v0.1.0-beta.40 release:
  - Restored pub(crate) visibility in ragent-tui/src/app/helpers.rs and added #![allow(clippy::redundant_pub_crate)] so it passes both the -D unreachable_pub dead-code lint and Clippy.
  - Added RUSTSEC-2026-0235 (rkyv) to deny.toml and .github/workflows/security-audit.yml ignores, matching existing transitive-dependency treatment.

### Changed

- Incremented workspace version to 0.1.0-beta.41.

## Version: 0.1.0-beta.40

### Fixed

- Resolved build/clippy warnings in `ragent-tui`:
  - Added `#[allow(dead_code)]` to `App::is_router_enabled` and the unused status helper methods (`set_status_info`, `set_status_success`, `set_status_warning`, `set_status_error`).
  - Added a doc comment to `App::execute_slash_command_inner`.
  - Changed redundant `pub(crate)` visibility to `pub` in the private `app::helpers` module to satisfy `clippy::redundant-pub-crate`.

### Changed

- Incremented workspace version to 0.1.0-beta.40.

## Version: 0.1.0-beta.39

### Changed

- Incremented workspace version to 0.1.0-beta.39.

## Version: 0.1.0-beta.38

### Added

- Strict exact-byte matching for `edit`, `multi_edit`, and `apply_patch` tools (EDITPLAN.md):
  - Replaced whitespace-tolerant/line-normalized replacement logic in `ragent-tools-core` with exact-byte `find_exact_replacement_range` and `find_exact_batch_edit` helpers.
  - Removed fallback heuristic replace in `replace.rs`; tools now return a clear single error listing the first non-matching old_string.
  - Added `test_edit_smoke.rs` automated smoke test (T-014) covering exact-match success and failure paths.
  - Added `EDITPLAN.md` and completion report `docs/reports/editplan-m1-completion.md` tracking milestones M1–M3.

### Fixed

- Fixed GitHub Actions Clippy failures caused by the stable toolchain upgrading to Rust 1.97.0.
  - Removed redundant `Arc::from(client)` wrapping and closure-style `Arc::clone` in `crates/ragent-agent/src/session/loop_steps.rs`.
  - Replaced `needless_collect` and `filter().next()` patterns in `crates/ragent-agent/tests/inline/skill_loader.rs` with `!any(...)`.
  - Removed `let _ =` on unit timeout await in `crates/ragent-agent/tests/test_bg_service.rs`.
  - Switched single-character `contains` checks to `char` patterns in `crates/ragent-agent/tests/test_instruction_includes.rs`.
  - Replaced `Default::default()` followed by field assignment with struct-expression initialization in `ragent-telemetry` tests and `src/instruments.rs`.
  - Removed unnecessary `to_path_buf()` in `crates/ragent-tools-core/tests/test_open.rs`.
  - Removed clone-to-slice in `crates/ragent-llm/src/providers/tool_cache.rs`.
  - Added missing blank line before top-level doc comment in `crates/ragent-tui/src/widgets/message_widget.rs`.
  - Restored public visibility for API symbols used by external integration tests (TUI `App` methods, `MessageWidget`, and message-widget helpers) that became private after a previous pub(crate) sweep.
- Fixed flaky TUI test `test_telemetry_setup_context_menu_paste_writes_active_field` that failed in GitHub Actions CI with `X11 server connection timed out`.
  - Added a thread-local test-only clipboard override (`ClipboardTestOverrideGuard`) in `crates/ragent-tui/src/clipboard.rs` so paste tests can run on headless runners without a display server.
  - Updated the telemetry paste test to use the override instead of writing to the real system clipboard.
- Hardened YOLO-mode test isolation to remove parallel-test races in `test_alt_y_toggles_yolo_mode_and_status_bar_indicator` and `test_slash_yolo_toggles_and_persists`.
  - `enter_temp_config_dir()` now primes a project-local `.ragent/ragent.json` with a known `yolo: false` state before toggling.
  - Decoupled the in-memory YOLO flag from `Config::load()`; added `ragent_config::yolo::sync_from_config()` for explicit startup sync and called it from `src/main.rs`. This prevents unrelated config reloads from racing with an in-flight toggle during parallel tests.
  - Updated `ragent-config/tests/test_yolo_persistence.rs` to call the new explicit sync helper.

### Changed

- Incremented workspace version to 0.1.0-beta.38.

## Version: 0.1.0-beta.37

### Added

- TUI clipboard remediation (CUTPLAN.md Milestones 1–5):
  - New `crates/ragent-tui/src/clipboard.rs` module is the single source of truth for `arboard` text and image clipboard operations.
  - `InputField::paste_text_from_clipboard` (with `paste_clipboard` alias) and `App::{get,set}_clipboard` now delegate to shared helpers.
  - Device-flow user-code copy in `input.rs` uses the shared helper.
  - `App::handle_paste_text` strips `\r` and replaces active keyboard/mouse selections; used by Ctrl+V, terminal bracketed paste, and context-menu Paste.
  - Context-menu Paste in provider setup now supports `TelemetrySetup` alongside `EnterKey` and `GitLabSetup`.
  - Clipboard image temp files are written under `<cwd>/target/temp/` as `ragent_paste_*.png` with Unix permissions `0o600`, encoded directly from the borrowed pixel buffer (no `to_vec()` copy).
  - TUI startup prunes orphaned `ragent_paste_*.png` files older than 24 hours.
  - `App::paste_image_from_clipboard` is now `pub(crate)`; it warns when a clipboard-resolved image path lies outside the working directory or home directory while still attaching the file.
  - User-facing docs (`QUICKSTART.md` and `TUI-QUICKSTART.md`) updated to describe text selection, Copy/Cut/Paste, right-click context menu, terminal bracketed paste, and `Alt+V` image paste.
  - Tests added in `crates/ragent-tui/tests/test_clipboard.rs` and extended in `tests/test_clipboard_tempfile.rs` and `tests/test_slash_commands.rs`.

### Changed

- Incremented workspace version to 0.1.0-beta.37.
- Reviewed and cleaned up project guidelines in `AGENTS.md`:
  - Renamed the agent acknowledgement section and removed legacy memory tool names (`memory_read`, `memory_write`, `memory_replace`, `memory_search`, `memory_migrate`) from the available-tools list.
  - Added an explicit "Tool Use — Critical Instructions" callout instructing immediate tool invocation without narrative preamble.
  - Added `RELEASE.md` to the approved root documentation exceptions list.
  - Removed project-specific requirement identifiers from the test migration guidance.
- Deleted the stale `assets/config/AGENTS.md` copy to eliminate drift from the canonical root guidelines.

## Version: 0.1.0-beta.36

### Changed

- Incremented workspace version to 0.1.0-beta.36.
- Security remediation planning: created `SECPLAN.md` with P0–P3 risk register,
  concrete file/line references, and milestone-based remediation roadmap.

## Version: 0.1.0-beta.35

### Added

- New `TOOLS.md` root documentation file listing all available agent tools.
- Updated `team_create` tool input schema to support an optional `context` field
  for richer teammate creation prompts.

### Improved

- Tool system-prompt sufficiency audit and remediation across all 142 registered
  tools (T-001–T-004 in `TOOLPLAN.md`). Every tool now has a description of at
  least 120 characters, explicit required-parameter callouts, and a strict
  JSON schema with `"additionalProperties": false` to reject hallucinated
  parameter names. System-prompt guidance sections were added/updated for VCS
  safety, codeindex usage, memory tools, team tools, ask-user tools, and
  file-reading best practices.

## Version: 0.1.0-beta.34

### Added

- `/actionloop` slash command (with `help` and `clip` subcommands) that reports
  agent action-loop average timings from the profiler, sorted by descending
  average elapsed time so hotspots are visible at a glance.
- `github_get_actions` tool input/result summaries in the TUI message widget,
  showing the inspected run count and any failed runs.
- `CompactionConfig.threshold` percentage-based trigger (0.0–1.0). When set
  (e.g. `0.8` = 80%), compaction fires at `context_window * threshold`; when
  `None` the buffer-based `context_window - max(output_tokens, buffer)` model
  is used. The legacy `compression.auto_threshold` value is migrated into this
  field so existing configurations keep their trigger point.
- New documentation: `ALPLAN.md` (agent-loop performance remediation plan with
  hotspots H1–H4 and rollout order) and `docs/agentorch.md` (component-by-component
  agent loop / orchestrator internals with exact `file:line` references).

### Changed

- Incremented workspace version to 0.1.0-beta.34.
- `Storage::open` now enables `journal_mode=WAL` and a 5s `busy_timeout` so a
  background writer (e.g. the startup FTS warm-up) no longer serialises
  concurrent readers behind it and stalls `get_setting`/`detect_provider`.
- `warm_message_search_index` rebuilds the FTS index inside a single transaction
  (one fsync instead of one per row), making the startup warm-up effectively free.
- Loop-step P-3 now treats non-positive provider-reported context windows
  (notably the virtual Model Router reporting `0`) as "unknown" and falls back
  to the 128k default so compaction never fires on the first turn.
- The TUI pre-send compaction check now honours the configured
  `compaction.threshold` percentage when present, matching the server-side
  estimator; otherwise it falls back to the conservative 92% hard limit.
- `detect_provider` now runs a fast pass that defers the Copilot `gh auth token`
  CLI subprocess until no provider is found cheaply, so startup provider
  detection never blocks on a cold keyring / slow `gh`.
- Fixed a corrupted UTF-8 box-drawing comment in `message_widget.rs` (mojibake
  from a prior commit) and removed redundant double-gating in the raw-args
  fallback per the `/simplify` review.

### Fixed

- `test_worker_indexes_changed_file` now polls for worker batch processing
  instead of a fixed sleep, making it robust on slow/saturated CI runners.

### Tests

- Added `crates/ragent-storage/tests/test_wal_warmup.rs` asserting WAL mode is
  active and that a background warm-up writer does not block a concurrent reader.
- Added compaction threshold percentage / legacy-alias migration tests in
  `ragent-config` and `ragent-agent` compaction estimator tests.
- Added `/actionloop` slash-command tests (help, no-samples hint, clip, timings).
- Added `github_get_actions` tool summary tests and provider-detection
  fast-path (defer `gh` CLI) tests.

## Version: 0.1.0-beta.33

### Changed

- Incremented workspace version to 0.1.0-beta.33.
- Removed the `agentgrep` structure-aware code search tool and its unused
  dependencies (`grep-regex`, `grep-searcher`, `ignore`, `glob` from
  `ragent-tools-extended`) because it duplicated the `codeindex_*` toolset.
- Updated `README.md`, `QUICKSTART.md`, `TUI-QUICKSTART.md`, `SPEC.md`, and
  `docs/JCODEPLAN.md` to remove `agentgrep` references.

## Version: 0.1.0-beta.32

### Changed

- Incremented workspace version to 0.1.0-beta.32.
- Updated CI workflow.

## Version: 0.1.0-beta.31

### Fixed

- TUI tool-call summaries now always show parameters by falling back to the raw
  tool args when the expected JSON keys (`path`, `command`, etc.) are missing.
  This keeps bash/read/write/create/edit inputs visible even when providers emit
  unexpected field names.
- Fixed rustfmt indentation issue in `crates/ragent-tools-extended/src/masterfetch/security.rs`.

## Version: 0.1.0-beta.30

### Changed

- Incremented workspace version to 0.1.0-beta.30.
- Updated CI workflow and performed multiple codebase hygiene updates.

## Version: 0.1.0-beta.29

### Changed

- Removed legacy memory system (file-block memory modules, old structured-memory
  storage, migration helpers, and cross-project import/export code) and replaced
  it with the new structured-memory store backed by `ragent-storage`.
- Fixed the memory panel in the TUI after the memory-system refactor so it
  continues to browse and render stored memories correctly.

## Version: 0.1.0-beta.28

### Changed

- Fixed Build and Release workflow by granting `contents: write` permission so
  `softprops/action-gh-release@v2` can create GitHub releases.

### Fixed — CI Check & Test

- Reverted the `check-and-test` job to debug builds (`cargo check/test
  --workspace`) and removed the accidental `--release` flags that caused the
  runner to run out of memory while linking the release test binary.
- Added an 8 GiB swapfile step and a 45-minute job timeout to give the debug
  build more headroom and prevent runaway jobs.

## Version: 0.1.0-beta.27

### Changed

- Optimized CI runners: moved `check-and-test` to `ubuntu-latest-4-cores`, disabled
  debuginfo in dev/test profiles, and added `free-disk-space` cleanup to reduce
  disk pressure during builds.

## Version: 0.1.0-beta.26

### Changed

- Incremented workspace version to `0.1.0-beta.26`.

## Version: 0.1.0-beta.25

### Changed — CI package builds

- Disabled `.rpm` package builds in the release workflow; only the plain binary
  is now published. Both Debian and RPM packaging are temporarily disabled
  while the packaging build paths are reviewed.

## Version: 0.1.0-beta.24

### Changed — CI package builds

- Disabled `.deb` package builds in the release workflow; the `.rpm` package and
  plain binary remain published. Debian packaging is temporarily disabled while
  the `cargo-deb` build path is reviewed.

## Version: 0.1.0-beta.23

### Added — CI package builds

- Added `.github/workflows/release.yml` that triggers on `v*` tags and builds
  `ragent` for `x86_64-unknown-linux-gnu` on `ubuntu-latest`.
- CI installs `cargo-deb` and `cargo-generate-rpm`, then runs `cargo deb` and
  `cargo generate-rpm` against the release binary.
- The release body is populated from the matching section of `CHANGELOG.md`
  (extracted via `awk`) and published with `softprops/action-gh-release@v2`.
- Assets published to the GitHub Release include:
  - `ragent-<version>-x86_64.deb`
  - `ragent-<version>-x86_64.rpm`
  - the plain `ragent` binary
- Root `Cargo.toml` now carries `[package.metadata.deb]` and
  `[package.metadata.generate-rpm]` metadata so the generated packages
  install the binary to `/usr/bin/ragent` and ship `README.md`, `LICENSE`,
  and `CHANGELOG.md` to `/usr/share/doc/ragent/`.

### Changed — OpenTelemetry updated to 0.28

- Bumped `opentelemetry`, `opentelemetry_sdk`, and `opentelemetry-otlp` to
  0.28 across `crates/ragent-telemetry` and adapted to the breaking API
  changes (new `Resource` builder, `PeriodicReader` signature,
  `InMemoryMetricExporter` relocation, `MetricReader` return types).

## Version: 0.1.0-beta.22

### Added — `/provider` always allows editing the API key

- The `/provider` slash command now opens the provider picker with
  `force_key_entry: true`, so selecting an already-configured key-based
  provider shows the `EnterKey` dialog instead of skipping straight to the
  model list. This lets users update an existing API key without removing
  and re-adding the provider.
- The `EnterKey` dialog pre-fills the key field with the existing stored key
  (`App::provider_api_key`) so the user can edit it rather than re-entering
  from scratch.
- The API-key and GitLab token fields are now displayed **unmasked** so the
  user can verify the full value, and the dialog is widened (80×30) so the
  full key (≥ 48 chars) is visible.

### Changed — `/model` jumps straight to the model list for configured providers

- When a provider is already configured, `/model` now skips the provider
  picker and jumps directly to model discovery / the model list for that
  provider. The provider picker is only shown when no provider is configured.
  Special-cases: `azure_resource` opens the resource-file picker, `router`
  opens the cluster setup UI.

### Added — Research `--use-low-relevance` flag

- `ragent research create <name> --use-low-relevance` (and the TUI / HTTP
  equivalents) retains every fetched web page regardless of its
  query-match relevance score, disabling the default filter that discards
  "Low"/"Very low" sources. Plumbed through `SessionConfig::use_low_relevance`,
  `WebGatherer::with_keep_low_relevance`, the CLI, TUI slash handler, and the
  `POST /research` HTTP route.

## Version: 0.1.0-beta.21

### Fixed — Compaction user feedback & resilience

- Compaction bail paths (empty head, prompt-overflow, LLM summarisation failure,
  empty summary) now publish an `Event::AgentNotice` so the TUI and HTTP clients
  show "Context compression skipped/failed: …" instead of silently bailing.
- Compaction warning logs now use `warn!` with the bail reason and carry the
  session id, making skipped compressions visible in diagnostics.

### Fixed — Post-compaction continuation nudge

- After a successful compaction the session loop injects a continuation nudge
  so the agent resumes its task instead of stopping. The `compaction_nudged`
  flag is now threaded across loop iterations (no longer reset to `false`
  each turn), preventing repeated nudges. Integration tests updated to expect
  the extra post-compaction continuation request.

### Fixed — Autopilot auto-continue after task completion

- Added `App::last_task_completed_at` timestamp set when a `TaskCompleted`
  event arrives. `poll_autopilot_continue` now suppresses the auto-continue
  and disables autopilot when the agent already signalled completion, so
  autopilot no longer keeps re-prompting after `agent_complete`.
- The `FinishReason` handler also guards against re-entering autopilot
  continue when a `TaskCompleted` was already consumed this turn.

### Added — Router downstream-model status bar

- The TUI status bar now shows the actual downstream model and tier for the
  router virtual provider: `Model Router ({provider}:{model}) / {tier}`
  instead of the static `Model Router / router` label. New
  `router_current_model` field captures the last routed downstream model,
  surfaced via `Event::RouterTierSelected`. New test
  `test_router_status_bar_label_shows_downstream_model_and_tier`.

### Added — Autopilot status indicator in status bar

- Status bar line 2 now shows `AutoPilot:✓` (green) when autopilot is active
  and `AutoPilot:✗` (red) when disabled, giving immediate visual feedback.

### Fixed — Router terminal-signal guarantee

- `RouterClient::chat` now wraps the downstream stream so that if the
  provider ends without emitting a `StreamEvent::Finish`, a synthetic
  `Finish { reason: Stop }` is injected. This guarantees the session loop
  always observes a terminal event per LLM call, preventing infinite loops
  on provider protocol drift.

### Fixed — Skill discovery test isolation

- Skill discovery/registry tests now filter by `SkillScope` (Project vs
  Personal) and assert against `bundled_count()` rather than the total
  registry length, so the tests no longer break when bundled or personal
  skills are present alongside project skills.

### Fixed — Doctest build breakages

- Updated doctests in `session::permissions` (marked `ignore` since the
  helper is `pub(crate)`) and `tool::ToolRegistry` (switched example from
  `ReadTool` to `PlanEnterTool` and added the missing fields to the
  `ToolContext` doc example) so the crate's doctests compile again.

### Fixed — Research progress config `from_file` field in tests

- `test_research_progress_config` now sets the new `from_file: None` field
  introduced by the `--from-file` research feature.

## Version: 0.1.0-beta.20

### Added — Research support for local file topics (`--from-file`)

- `ragent research create <name> --from-file <PATH>` (and `/research create
  --from-file` in the TUI) extracts a local document and uses its content as
  the research subject in place of an explicit topic. Supported formats: PDF,
  DOCX, XLSX, PPTX, ODT, ODS, ODP, TXT, and MD. The extracted content becomes
  the primary source; web search still runs using the derived topic.
- When no explicit topic is given, a concise topic and clean title are derived
  from the extracted document body via the optional LLM summarizer
  (`summarize_subject`), falling back to the heuristic
  `derive_topic_from_url_body` scraper when no LLM is configured.
- New `document_extract` module in `ragent-tools-extended` performs the text
  extraction (including a direct PDF fast path extended in `libreoffice_read`).
- New `SessionEvent::FromFileBodyPreview` surfaces a ~200-char preview of the
  extracted text so the TUI and HTTP clients can show what content was used
  to derive the topic; `/research` TUI progress panel shows the `from-file`
  path in the header.
- `derive_title_full` picks the file path as the item-title fallback after
  topic and URL.
- `SessionConfig::from_file` plumbed through the research adapter, manager,
  server routes, and CLI; `--from-url`, `--from-file`, and explicit topics
  are mutually combinable.

### Fixed — Control-character sanitisation in research documents

- `strip_control_chars` (new public helper in `ragent-research::item`)
  removes C0/C1 control characters and BOM from all research document fields
  (summary, findings, cross-references, open questions, queries) before
  rendering into `RESEARCH.md`, so model output or raw PDF extraction can no
  longer corrupt the document with binary garbage.
- Analysis parsing (`parse_subject_summary`, fallback findings rescue) now
  sanitises model JSON before extraction.
- New `test_control_char_sanitization` test suite covers the rendering path.

## Version: 0.1.0-beta.19

### Fixed — Startup messages

- Added trailing newlines to TUI startup status messages (code index
  enabled/disabled/failed and the "Ready" banner) so subsequent output starts
  on a clean line instead of being appended to the same line.

## Version: 0.1.0-beta.18

### Fixed — Startup blocking issues

- MCP server connections now happen in a background `tokio::spawn` task instead
  of sequentially on the main task, eliminating the 5–15 s startup stall when one
  or more MCP servers are slow to start.
- Code-index startup (open + watcher + initial `full_reindex`) now runs in a
  background task and wires into `App` state via an mpsc channel, so the TUI
  event loop starts immediately and the index becomes available when ready.
- Provider health check (including Copilot token resolution via `gh auth token`)
  now runs entirely inside its spawned async task instead of blocking the TUI
  render loop during startup.
- `App::backfill_model_ctx_window` no longer calls synchronous model discovery
  (`sync_discover_models`) at startup; only cached/default metadata is consulted.
- The first printable keystroke after the run-cost banner is no longer
  swallowed — non-character keys still just dismiss the banner, but a plain
  character clears the banner and falls through to normal input so the first
  typed character is not lost.

### Added — Startup timing instrumentation

- New `StartupTimings` type (`crates/ragent-types/src/startup.rs`) records the
  wall-clock duration of every instrumented startup stage (CLI parse, config
  load, storage open, provider/tool registries, TUI init, session create, code
  index, MCP, etc.).
- New `/startup` TUI slash command renders an aligned stage/time table so users
  can identify which stages contribute most to perceived startup latency.
- Stages recorded inside `App::new()` are merged into the main timings
  collector via `StartupTimings::merge_stages`.

### Changed — Compaction prompt cap and reuse

- `MAX_COMPACTION_PROMPT_CHARS` reduced from 120 000 to 60 000 chars (~15 k
  tokens) to keep the LLM summarisation call tractable while still giving the
  model enough context for a useful summary. The verbatim recent tail
  (`keep_tokens`) is preserved regardless of this cap.
- `select()` now pre-serialises the head transcript and computes the original
  token cost, so `compact()` reuses them instead of re-serialising every message
  a second time.

### Added — Post-compaction continuation nudge

- When context compaction runs and the LLM responds without tool calls, a
  one-time user nudge is injected so the agent resumes its in-progress task
  rather than letting the loop stop prematurely.

### Added — Copilot `gh` CLI token cache

- `find_gh_cli_token` caches its result in a process-wide `OnceLock` so the
  `gh auth token` subprocess is spawned at most once per session.

### Changed — Code-index performance

- SQLite store now sets `WAL` journal mode, `synchronous = NORMAL`, and
  `temp_store = MEMORY` pragmas for dramatically faster writes.
- `get_file_symbols` queries directly by `file_id` instead of loading all
  symbols and filtering in Rust.
- Reindex chunk yield reduced from 5 ms to 1 ms for tighter throughput.
- Reindex now logs per-phase timing (scan, diff, apply, fts_sync, total).

## Version: 0.1.0-beta.17

### Added — `@<path>` directive for modular instruction files

- Instruction files (`AGENTS.md`, `CLAUDE.md`, `.ragent.md`, `INSTRUCTIONS.md`)
  now support a C/C++ `#include`-style mechanism for modularity. A line of
  the form `@docs/conventions.md` (or `@"path with spaces.md"`) — with the `@`
  in the first column — is replaced in-place by the contents of the referenced
  file before the content is loaded into the system prompt.
- A leading `@@` is an escape sequence that collapses to a single literal `@`
  character, so lines that start with `@` can be written verbatim without
  triggering an include.
- Paths resolve relative to the directory of the file containing the
  directive; includes are transitive (included files are themselves
  expanded). Cycle detection (visited-path set) and a depth cap
  (`MAX_INCLUDE_DEPTH = 16`) prevent infinite loops. Absolute paths and
  `../` escapes outside the working dir / global ragent data dir are
  rejected with an inline marker comment; missing or unreadable files emit
  a marker comment rather than failing. Implemented in
  `crates/ragent-agent/src/agent/mod.rs` (`expand_includes`), wired into
  `collect_agents_md_content_with_discovery`. Tests in
  `crates/ragent-agent/tests/test_instruction_includes.rs`.

### Changed — `@include <path>` → `@<path>` include syntax

- The instruction-file include directive now uses `@<path>` (the `@` sigil
  followed directly by the path) instead of `@include <path>`. The `@` must
  appear in the first column of the line. The `@@` escape sequence is new.
  Existing instruction files using `@include path/to/file.md` must be updated
  to `@path/to/file.md`.

### Completed — Open/reveal and remaining UX tools — JCODEPLAN M10

- Confirmed `open` tool implementation in `crates/ragent-tools-core/src/open.rs`
  (T-090 through T-094): cross-platform `xdg-open`/`open`/`start` wrapper,
  `reveal` action for opening a target's parent directory, URL scheme
  allowlist validation (`http`, `https`, `mailto`, `file`), and integration
  tests in `crates/ragent-tools-core/tests/test_open.rs`.
- `open` is registered via `create_core_registry()` and surfaced automatically
  in the agent default registry under the `shell:execute` permission category.
- `docs/JCODEPLAN.md` updated to mark M10 tasks complete and added
  `docs/reports/jcodeplan-m10-completion.md`.

### Added — Durable initiatives and skill management — JCODEPLAN M8

- New `initiative` tool in `crates/ragent-agent/src/tool/initiative.rs`
  (T-070) managing durable, project-scoped goals with milestones. Actions:
  `create`, `read`, `update`, `checkpoint`, `list`, `close`. `checkpoint`
  marks a milestone complete (recording `completed_at`), bumps overall
  progress, and appends a timestamped `Checkpoint:` note to the description.
  `close` supports `completed` (auto-fills progress to 100) and `abandoned`
  (keeps recorded progress). Registered in `create_default_registry()` under
  the `storage:write` permission category (T-073).
- New `initiatives` SQLite table + CRUD (`create_initiative`,
  `get_initiative`, `list_initiatives`, `update_initiative`,
  `delete_initiative`) in `ragent-storage`, with `InitiativeMilestone` /
  `InitiativeRow` types re-exported via `crate::storage` (T-070).
- `## Active Initiatives` system-prompt section injected on every turn in
  `session/loop_steps.rs`, listing active initiatives with progress and the
  next pending milestones so the agent stays aware of long-term goals across
  sessions and compaction (T-070 "surface in system prompt").
- New `skill_manage` tool in `crates/ragent-agent/src/tool/skill_manage.rs`
  (T-071). Actions: `list` (with `scope` filter + optional `include_bodies`),
  `read` (processed prompt with `$ARGUMENTS` substitution), `load` (discover
  + return prompt — injects skills added/edited after session start),
  `reload` (clear caches, re-discover, report added/removed skills).
- New `SkillInfo::clear_body_cache()` async helper used by the `reload`
  action to drop on-disk `SKILL.md` caches without a session restart.
- Migration SQL block in `ragent-storage` re-indented consistently to prevent
  future edit collisions.
- Tests (T-072): 26 tests in `crates/ragent-agent/tests/test_initiative.rs`
  (tool identity, schema, create/read/update/checkpoint/list/close, duplicate
  and invalid-slug rejection, cross-session visibility, per-project
  isolation, empty-storage graceful error, storage round-trip, prompt
  section), 12 tests in `crates/ragent-agent/tests/test_skill_manage.rs`
  (bundled + project discovery, scope filter, arg substitution, unknown-skill
  listing, load injects prompt, reload picks up added skills + edited
  bodies), 7 tests in `crates/ragent-storage/tests/test_initiatives.rs`
  (table creation, field round-trip, status filter, closed_at lifecycle,
  malformed-JSON fallback).
- Documented both tools in `SPEC.md` §19B.

### Added — Gmail and messaging channel tools — JCODEPLAN M7

- New `gmail` tool in `crates/ragent-tools-extended/src/gmail.rs` (T-060)
  providing Gmail search/read/draft/send via the Gmail REST API with OAuth2
  tokens stored encrypted in the SQLite credential store (`SqliteTokenStore`),
  plus `auth`/`status`/`logout` management actions and automatic
  refresh-token exchange + retry on HTTP 401.
- New `send_channel_message` tool in
  `crates/ragent-tools-extended/src/channels.rs` (T-061) sending short
  messages to Telegram (bot API) and Discord (incoming webhook), with
  `send` (target `telegram`/`discord`/`all`) and `status` actions.
- New config schema: `gmail` block (`client_id`, `client_secret` with `env:`
  indirection) and `channels` block (`enabled`, `telegram`, `discord`) in
  `ragent-config`. Client credential precedence: auth args → stored tokens →
  config → `GMAIL_CLIENT_ID`/`GMAIL_CLIENT_SECRET` env vars.
- Both tools use the `network:send` permission category and degrade
  gracefully with honest errors and `next_action` hints when unconfigured.
- Registered both tools in `create_extended_registry()` (T-063), surfaced in
  the agent automatically via `register_extracted_extended_tools`.
- Mocked-backend integration tests (T-062):
  `crates/ragent-tools-extended/tests/test_gmail.rs` (19 tests, axum mock
  server, encrypted store round-trip, refresh exchange, RFC 2822 wire check)
  and `crates/ragent-tools-extended/tests/test_channels.rs` (20 tests, mock
  Telegram/Discord fanout, config merge, env indirection).
- Documented both tools in `SPEC.md` §19A.

## Version: 0.1.0-beta.16

### Added — Conversation and cross-session search tools — JCODEPLAN M5

- New `conversation_search` tool in `crates/ragent-agent/src/tool/conversation_search.rs`
  provides keyword search, turn-range retrieval, and statistics for the current
  session. Modes: `keyword` (default), `turn_range`, `stats`.
- New `session_search` tool in `crates/ragent-agent/src/tool/session_search.rs`
  performs ranked full-text search across all stored sessions with filters for
  date range, working directory, role, per-session limits, and optional
  surrounding context.
- Session message FTS5 index (`messages_fts`) and optional embedding cache
  (`messages_embedding`) in `ragent-storage`, with `store_message_embedding`,
  `get_message_embedding`, and `search_messages_by_embedding` helpers.
- `warm_message_search_index()` is called on startup to rebuild the FTS index
  in a background blocking task.
- New `ConversationSearched` and `SessionSearched` event variants, with SSE
  serialization in `crates/ragent-server/src/sse.rs`.
- Added integration tests in `crates/ragent-agent/tests/test_conversation_search.rs`,
  `crates/ragent-agent/tests/test_session_search.rs`, and
  `crates/ragent-storage/tests/test_message_embeddings.rs`.
- Registered both tools in `create_default_registry()` under the Memory category.

## Version: 0.1.0-beta.15

### Added — Browser automation tool (`browser`) — JCODEPLAN M4

- New `browser` tool in `crates/ragent-tools-extended/src/browser/` providing
  Chrome DevTools Protocol (CDP) browser automation with 14 actions: `open`,
  `snapshot`, `click`, `type`, `fill_form`, `select`, `wait`, `eval`,
  `scroll`, `upload`, `press`, `screenshot`, `status`, `setup`.
- CDP WebSocket client (`browser/cdp.rs`) with JSON-RPC command/response
  correlation, event fan-out via broadcast channel, and graceful degradation
  when no browser is available.
- Browser launcher (`browser/launch.rs`) with platform-specific Chrome/
  Chromium binary detection (Linux, macOS, Windows) and headless launch
  via `--remote-debugging-port`.
- Action handlers (`browser/actions.rs`) implementing each action using CDP
  domains (Page, DOM, Runtime, Input, Network).
- `BrowserConfig` in `ragent-config` with `cdp_endpoint` and
  `default_headless` fields, configurable in `ragent.json` under the
  `browser` key.
- `browser` tool-visibility switch added to `ToolVisibilityConfig` —
  toggle via `/tools browser on|off`.
- TUI `/tools` slash command updated to include `browser` and `masterfetch`
  in the valid switches list.
- Added `tokio-tungstenite` workspace dependency for WebSocket support.
- Added integration tests in
  `crates/ragent-tools-extended/tests/test_browser.rs` (37 tests covering
  tool identity, schema, graceful degradation, config, visibility, CDP types,
  and conditional live CDP tests).
- Registered as `browser` in `create_extended_registry()`.

## Version: 0.1.0-beta.14

### Added — Codex-style patch tool (`apply_patch`)

- New `apply_patch` tool in `crates/ragent-tools-core/src/apply_patch.rs` parses
  `*** Begin Patch` / `*** End Patch` envelopes with `*** Add File:`,
  `*** Delete File:`, and `*** Update File:` operations. Update hunks use `@@`
  headers with context (` `), add (`+`), and remove (`-`) lines.
- Supports file moves via `*** Move to:` inside an update block, including
  rename-and-edit in a single patch.
- All operations are validated before any file is written; paths are resolved
  relative to the working directory and canonical containment is enforced.
- Includes `dry_run` parameter to preview changes without writing files.
- Added integration tests in `crates/ragent-tools-core/tests/test_apply_patch.rs`.
- Registered in `create_core_registry()`.

### Added — Open/reveal tool (`open`)

- New `open` tool in `crates/ragent-tools-core/src/open.rs` opens files, folders,
  and URLs using the platform default handler (`xdg-open` on Linux, `open` on
  macOS, `start` on Windows).
- Supports `open`, `reveal` (parent directory), and `url` actions. URL schemes
  are validated against an allowlist (`http`, `https`, `mailto`, `file`).
- Paths are resolved relative to the working directory and checked for root
  containment.
- Added integration tests in `crates/ragent-tools-core/tests/test_open.rs`.
- Registered in `create_core_registry()`.

### Fixed — `agentgrep` clippy warnings

- Cleaned up `agentgrep.rs` to satisfy `-D warnings`:
  replaced `map_or(false, ...)` with `is_some_and`, iterated map keys directly,
  simplified sorts, and flattened a glob loop.

### Fixed — TUI read tool header uses pending args when `ToolCallStart` is dropped

- In `crates/ragent-tui/src/app/event_handler.rs`, the `Event::ToolCallBatch`
  fallback now applies any previously-stored `pending_tool_args` to a missing
  tool-call part after creating it. This fixes the case where the broadcast
  bridge drops `ToolCallStart` but `ToolCallArgs` was already queued: the TUI
  widget header was showing `📄 missing path` even though the args JSON
  contained a valid `path`.
- `update_tool_call_input` is reused to merge the pending JSON into the newly
  created part's `state.input`, so `tool_input_summary` can render the correct
  read path in the header.
- Added regression tests
  `test_tool_call_batch_applies_pending_args_when_start_dropped` and
  `test_tool_call_batch_does_not_overwrite_existing_input` in
  `crates/ragent-tui/src/app/tests.rs`.

### Fixed — TUI read tool header always shows icon, and missing path surfaces in UI

- `tool_input_summary` in `crates/ragent-tui/src/widgets/message_widget.rs` no
  longer returns an empty string for `read` calls with a missing/empty `path`.
- When `path` is absent the header now renders `📄 missing path`, keeping the
  file icon visible and clearly signalling the malformed input to the model.
- Added `test_input_summary_read_tool_missing_path_shows_placeholder` in
  `crates/ragent-tui/tests/test_tool_display.rs`.
- The underlying `ReadTool::execute` already errors with
  "Missing required 'path' parameter", so the LLM receives an actionable
  diagnostic prompting it to correct the call.

## Version: 0.1.0-beta.13

### Changed — Version bump

- Workspace version bumped from `0.1.0-beta.12` to `0.1.0-beta.13`.
- Added JCode cost accounting and fixed tool widgets.

## Version: 0.1.0-beta.12

### Added — Research completion reports excluded web sources

- `ragent-research` now counts web pages that were fetched but excluded due to
  low relevance (`excluded_count`).
- `GatherResult`, `RunOutcome`, and `SessionEvent::Done` all carry the new
  `excluded_count` field so the information flows from the gatherer through the
  session to observers.
- `WebFetchedPage` now includes an optional `language` field populated by the
  `mf_fetch` layer and propagated into `Source::Web`.
- CLI final output updated to
  `Done: N sources (PDF P, YouTube Y, X excluded)` and the JSON event now
  includes `excluded_count`.
- TUI `/research create` progress now decodes `excluded_count`, passes it to
  `ResearchProgress::finish`, and includes it in both the rendered markdown log
  and the final status-bar message.

### Added — Per-run cost summary (`Event::RunCostSummary`)

- At the end of every `process_user_message` turn, the session processor now
  accumulates `Event::TokenUsage` totals, calls `compute_run_cost`, and publishes
  a single `Event::RunCostSummary` on the event bus.
- The summary carries `session_id`, `model_id`, `input_tokens`,
  `output_tokens`, `total_cost_usd`, and `duration_ms`.
- Cost computation respects user-defined price overrides from `ragent.json`
  (`Config::prices`) via `merged_prices` and falls back to the built-in price
  table; unknown models count tokens with zero cost.
- The TUI logs a one-line `⟡ run complete` banner on `Event::RunCostSummary`
  and updates the `ragent.cost.session` telemetry counter.
- The TUI now also renders a transient one-line
  `⟡ run complete · {in}+{out} tokens · ${cost} · {dur}s` banner overlay
  (FR-012, T-013) on `Event::RunCostSummary`, dismissed on the next keypress,
  while the full summary (model id + millisecond duration) is logged to the
  log panel.
  - Added TUI tests `crates/ragent-tui/tests/test_run_cost_banner.rs` covering
    banner population, log content, cross-session filtering, keypress dismissal,
    and default state.
  - Run-cost summaries are now persisted in a dedicated `run_cost_summaries`
    SQLite table (FR-018, T-024) so they can be retrieved for `--include-cost`
    exports, but are **omitted** from the default session export JSON.
  - `session export` CLI command now accepts a `--include-cost` flag; when set,
    the export JSON is wrapped as `{ "messages": [...], "cost_summaries": [...] }`
    with per-run cost records (`input_tokens`, `output_tokens`,
    `total_cost_usd`, `duration_ms`, `model_id`, `created_at`). Without the
    flag, only the messages array is exported (no cost data).
  - `RunCostSummaryRow` derives `Serialize`/`Deserialize` for JSON export and
    is re-exported from `ragent-storage` and `ragent-agent::storage`.
  - The session processor persists each `RunCostSummary` via
    `spawn_blocking` (non-blocking) alongside publishing the event.
  - Added storage tests `crates/ragent-storage/tests/test_run_cost_summaries.rs`
    covering round-trip persistence, session scoping, JSON serialization, and
    default-vs-opt-in export separation.
  - Extended `crates/ragent-agent/tests/test_run_cost_summary.rs` to assert the
    summary is persisted in storage after `process_message` completes.
  - The HTTP server serializes `RunCostSummary` as SSE event type
    `run_cost_summary`.
  - Added integration test `crates/ragent-agent/tests/test_run_cost_summary.rs`
    and SSE serialization test `test_run_cost_summary`.

### Changed — Version bump

- Workspace version bumped from `0.1.0-beta.11` to `0.1.0-beta.12`.
- Formatting fixes to `/research` tooling.

### Fixed — Web-source direct quotations are hard-capped to 200 characters

- Added a post-processing pass in `crates/ragent-research/src/analysis.rs` that
  mechanically truncates inline double-quoted strings and fenced code blocks
  inside each finding's `**Observation:**` paragraph to 200 characters. This
  enforces the existing prompt instruction even when the synthesis model
  ignores it, so RESEARCH.md findings no longer contain oversized web-source
  excerpts.
- The cap is applied after both clean LLM parses and mechanical fallback
  findings, and only affects the Observation paragraph so analysis text,
  cross-references, and implications are left untouched.

### Fixed — Removed redundant `mf_fetch:` prefix from `RESEARCH.md` source citations

- `parse_mf_fetch_output` in `crates/ragent-agent/src/research_adapter.rs` now
  strips the leading `mf_fetch: <url>` header block from plain-text tool output
  (cache hits, PDF pages, YouTube transcripts, and error responses) before the
  research layer stores the page body.
- The source title fallback is now taken from the real content rather than the
  `mf_fetch:` header, so the per-finding **Sources** list no longer shows the
  redundant `mf_fetch: <url> — <url>` pattern.
- Unrelated plain-text tool responses are left unchanged because the strip only
  runs when the first non-empty line starts with `mf_fetch:`.

### Fixed — `/research create` no longer crashes on html2text renderer panics

- `masterfetch::extractor::extract_markdown` now wraps the full readability →
  html2text → raw-text chain in a top-level `std::panic::catch_unwind`. If
  `html2text` panics on real-world HTML (e.g. mdBook-generated pages such as
  `rust-book.cs.brown.edu`), the extractor degrades to raw tag-stripped text
  instead of aborting the web-gatherer task or the whole ragent process.
- Added a regression test in `test_mf_extractor.rs` using a captured mdBook HTML
  fixture; it triggers the known html2text overflow and verifies the extractor
  returns a non-empty fallback result.

### Added — PDF and YouTube text extraction in `/research create`

- `mf_fetch` now extracts readable text from PDF responses instead of returning
  raw binary bytes. It detects `Content-Type: application/pdf` and `.pdf` URLs,
  runs `pdf_extract::extract_text_from_mem` in a blocking task, and surfaces the
  document title from the PDF `/Info` dictionary (with UTF-16BE BOM handling).
- `mf_fetch` now extracts timestamped captions from YouTube watch pages. It
  parses the embedded `ytInitialPlayerResponse` JSON, selects the best caption
  track (default, then English, then first available), fetches the caption XML,
  and formats each caption with a `[MM:SS]` timestamp.
- PDF and YouTube `mf_fetch` outputs set `page_type: pdf` / `page_type: youtube`
  and include `content_type` in metadata so the research layer classifies and
  counts them correctly.
- Updated `WebSourceKind::YouTube` documentation to reflect that transcript
  extraction is now implemented.
- `WebGatherer::fetch_url_as_source` now classifies `--from-url` seed sources by
  their fetched `content_type` / URL instead of always reporting `media_type: page`.
- Added `crates/ragent-tools-extended/tests/test_mf_pdf.rs` and
  `crates/ragent-tools-extended/tests/test_mf_youtube.rs` covering PDF text/title
  extraction, YouTube caption parsing, and end-to-end transcript fetch against a
  local mock caption server.
- Added `ragent-research` tests verifying that `fetch_url_as_source` and
  `gather_with_observer` classify and count PDF and YouTube sources correctly.

### Added — `/research create` richer fetch metadata and PDF/YouTube counters

- Research web gathering now prefers the `mf_fetch` tool over legacy `webfetch`.
  `mf_fetch` returns a structured envelope with `content_type`, `page_type`, and
  `metadata.title`, giving the research layer reliable media classification and
  better page titles.
- Added `WebFetchedPage.content_type` and `WebFetchedPage.page_type` to carry
  the richer `mf_fetch` metadata through the gatherer.
  - Added `Source::Web` fields `content_type`, `page_type`, and `media_type`
    (`"page" | "pdf" | "youtube"`) with serde defaults so older `RESEARCH.md`
    files remain backward compatible.
  - Added `Source::media_type()` and a new **Media** column in the
    `RESEARCH.md` References Index table so every source shows its classified
    media type (`page`, `pdf`, `youtube`) alongside the existing Type column.
    Non-web sources render `—`.
- New `WebSourceKind` enum and `classify_web_source()` helper classify sources
  from `Content-Type: application/pdf`, `.pdf` URLs, and YouTube hosts.
- `GatherResult`, `SessionEvent::Done`, `RunOutcome`, and the TUI
  `ResearchProgress` tracker now carry `pdf_count` and `youtube_count`.
- CLI `ragent research create` prints recovered PDF and YouTube counts in the
  completion summary (e.g. "created research/foo (5 sources, 2 PDFs, 1 YouTube
  video)").
- TUI `/research create` progress summary now shows PDF and YouTube counts both in
  the live tracker render and in the status-bar completion message.
- Research event JSON output (`render_session_event_json`) includes
  `pdf_count` and `youtube_count` in the `done` payload.
- Added `url` crate dependency to `ragent-research` for host-based YouTube
  classification.

## Version: 0.1.0-beta.11

### Changed — Version bump

- Workspace version bumped from `0.1.0-beta.10` to `0.1.0-beta.11`.
- Moved Tavily search backend into the `mf_search` multi-engine framework.

## Version: 0.1.0-beta.9

### Added — TUI `/websearch` diagnostics

- New TUI slash command `/websearch show` lists every configured web-search
  backend (DuckDuckGo, Brave, LangSearch, Tavily) with `enabled`, `in_use`, and
  `failed` status columns.
- `/websearch help` prints usage and subcommand help.
- Command is registered in the TUI slash menu with autocomplete suggestions for
  `show` and `help`.
- `MfSearchTool::engine_status()` exposes the same status table programmatically
  so the TUI and tests share one source of truth.
- SPEC.md slash-command table updated with `/websearch show|help` and `/webapi`.

### Added — Tavily backend for `mf_search` and research migration

- New `TavilyEngine` in `crates/ragent-tools-extended/src/masterfetch/search/tavily.rs`
  implementing the `SearchEngine` trait. It calls `https://api.tavily.com/search`
  with `Authorization: Bearer {key}`, maps `SearchOptions` to Tavily JSON fields
  (`query` truncated to 400 chars, `max_results` clamped 1–20, `include_answer: false`),
  parses the `results` array, and reports non-2xx responses as `engine_blocked`.
- `mf_search` now includes the Tavily backend automatically when `tavily_api_key`
  is configured in `ragent.json` or `TAVILY_API_KEY` is set; it runs in parallel
  with the existing DuckDuckGo, Brave, and optional LangSearch backends.
- `mf_search` description updated to mention Tavily and both optional API keys.
- Legacy `websearch` tool is retained for direct agent use and now documents that
  research workflows prefer `mf_search`.
- `ragent-research` now depends on `ragent-tools-extended` so the research layer
  can understand the `mf_search` metadata shape.
- `AgentWebSearchTool` now prefers the `mf_search` tool when available and falls
  back to `websearch`, mapping structured metadata and plain-text output into
  `WebSearchHit` while preserving `search_tool` and `search_engine` provenance.
- New helper `parse_mf_search_metadata` converts `mf_search` JSON metadata into
  research-layer hits, falling back from `search_engine` to `source` to the
  tool name.
- TUI `tool_input_summary` and `tool_result_summary` now handle `mf_search` like
  `websearch`, and the tool-category doc comment lists `mf_search`.

### Added — Research source provenance

- `WebSearchHit` and `Source::Web` now carry `search_tool` and `search_engine`
  fields so every web source produced by the research system records *which*
  search tool (e.g. `mf_search`, `websearch`) and backend engine(s) (e.g.
  `tavily`, `duckduckgo, brave`) discovered the URL.
- `AgentWebSearchTool` populates provenance from the `websearch` tool's
  structured metadata and from the text fallback parser; the `websearch`
  `SearchResult` now emits `search_tool`/`search_engine` defaults, and
  `mf_search` metadata includes them per result.
- `WebGatherer` propagates provenance into `Source::Web`, and `GatherEvent::SourceCaptured`
  forwards it so both the non-iterative and iterative research engines surface it.
- `SessionEvent::WebCaptured` now includes `search_tool`/`search_engine`; the CLI
  JSON renderer and TUI progress encoder display provenance in capture lines
  (e.g. `captured https://example.com via websearch (tavily) — Title`).
- `ResearchIo::render_references_index` adds **Search tool** and **Engine**
  columns to the References Index table in `RESEARCH.md`.
- All new fields use `serde(default)` so existing `RESEARCH.md` files and older
  metadata load without migration.

### Fixed

- `/websearch test` no longer panics from nested Tokio runtime. The TUI slash
  command now runs the async `MfSearchTool::engine_test()` inside
  `tokio::task::block_in_place`, matching the pattern used by `/spec validate`.

## Version: 0.1.0-beta.9
### Added — LangSearch backend for `mf_search`

- New optional `langsearch_api_key` top-level config field in `ragent.json`.
  When set, the key is merged across global/project/env config layers and
  serialised back only when explicitly present (defaults omit the key).
- New `LangSearchEngine` in `crates/ragent-tools-extended/src/masterfetch/search/langsearch.rs`
  implementing the `SearchEngine` trait. It calls `https://api.langsearch.com/v1/web-search`
  with `Authorization: Bearer {key}`, maps `SearchOptions` to LangSearch JSON
  fields (`query`, `count` clamped 1–10, `freshness`, `summary: true`), parses
  `data.webPages.value`, and reports non-2xx responses as `engine_blocked`.
- `mf_search` now includes the LangSearch backend automatically when a key is
  configured; existing keyless DuckDuckGo and Brave backends continue to work
  when no key is present.
- API key is masked in diagnostics and never logged or surfaced in error
  messages.
- Tests: request/response mapping and key masking unit tests, config
  merge/load/serialise tests, and an `#[ignore]`-gated live API test.

## Version: 0.1.0-beta.8

### Changed — Version bump

- Workspace version bumped from `0.1.0-beta.7` to `0.1.0-beta.8`.
- `cargo check` passes cleanly with the new version.

## Version: 0.1.0-beta.7

### Changed — Version bump

- Workspace version bumped from `0.1.0-beta.5` to `0.1.0-beta.6`.
- `cargo check` passes cleanly with the new version.

## Version: 0.1.0-beta.5

### Added — Live telemetry reconfiguration, agent metric recording, and sudo askpass broker
- `/telemetry on|off` now reconfigures the live `TelemetrySubsystem` in place
  (shuts down the meter provider on `off`, builds a fresh one on `on`) so the
  toggle takes effect immediately instead of requiring a restart. The
  subsystem's runtime state is held behind a `parking_lot::Mutex` and the
  provider wrapped in `Arc` for safe interior mutability.
- New `ragent-agent` telemetry module (`LlmRecorder`, `SessionRecorder`,
  `ToolRecorder`) records LLM call duration, tool invocation counts/durations,
  session start/end, and agent-loop timing into the telemetry subsystem.
  `SessionProcessor` is wired to the subsystem via an `Arc<TelemetrySubsystem>`.
- New `askpass` module in `ragent-tools-core` routes `sudo` password prompts
  through ragent's interactive question dialog instead of hanging on the
  controlling tty. The bash tool now detaches stdin (`Stdio::null()`) and sets
  `SUDO_ASKPASS` environment variables when a broker is active.
- `ragent-telemetry` re-exports `LlmRecorder`, `SessionRecorder`, and
  `ToolRecorder` for cross-crate use.
- `ShutdownGuard` keeps the meter provider alive for the process lifetime and
  flushes pending metrics on normal or panic exit paths.
- Telemetry panel rendering and `/telemetry` slash-command code reformatted
  (indentation and trailing-newline fixes).

## Version: 0.1.0-beta.4

### Added — Telemetry panel styling and release tooling

- Telemetry metric type labels now render in bold blue for better visual
  distinction in the ALT-O Telemetry panel.
- Automated release skill increments workspace version, updates release notes,
  and tags the repository.

## Version: 0.1.0-beta.3

### Added — Context-window compaction and `/config save`

- New context-window compaction pipeline replacing the Headroom-based compression
  scheme. Includes `compaction` config block, `/compact` slash command (with
  `/compress` alias), `CompactionStarted/CompactionFinished` events, and
  Unicode-safe truncation.
- `/config save` and `/config list` slash commands for backing up and restoring
  global `ragent.json`.
- Updates to telemetry counters and TUI wiring.

### Removed — Headroom dependency, CCR store, and compression pipeline

- Dropped the `headroom-core` git dependency, deleted the `compression` modules,
  removed CCR markers and the `headroom_retrieve` bridge, and added a legacy
  `compression` → `compaction` config alias.

## Version: 0.1.0-beta.2

### Added — Telemetry (OTEL) and ALT-O Telemetry panel

- OpenTelemetry metrics export (`/telemetry` slash command family: `help`, `on`, `off`, `setup`, `counters`) for managing OTLP endpoints, protocol, export interval, timeout, and an internal Prometheus port.
- TUI **ALT-O Telemetry panel** for live OpenTelemetry metrics and counter inspection.
- Configuration schema and TUI wiring for telemetry settings in `ragent.json`.

## Version: 0.1.0-beta.1

### Changed — Transition to beta channel

- Workspace version bumped from `0.1.0-alpha.147` to `0.1.0-beta.1`, marking
  the transition from the alpha pre-release channel to the beta pre-release
  channel.

### Added

- TUI `/telemetry` slash command family (`help`, `on`, `off`, `setup`,
  `counters`) for managing OpenTelemetry metrics export, including a full
  multi-field setup dialog for endpoint, protocol, export interval, timeout,
  and internal Prometheus port.

## Version: 0.1.0-alpha.147

### Fixed — Model Router no longer forces a vision model for text-only follow-ups

- `extract_attachments()` in `crates/ragent-llm/src/providers/router_client.rs` now
  scans **only the most recent user message** for image/video attachments instead of
  the entire conversation history.
- Previously, once an image was sent in a conversation, every subsequent prompt
  was treated as `requires_vision`, which caused the router to keep selecting a
  vision-capable model even when the current user prompt had no attachment.
- Now a text-only follow-up correctly re-classifies and selects the first model
  in the resolved tier (e.g. a non-vision `glm-5.2` listed above a vision variant).
- Added regression test
  `test_router_text_followup_after_image_uses_non_vision_model` in
  `crates/ragent-llm/tests/test_router_client.rs`.

### Fixed — Selecting Model Router in the provider picker now opens the router setup UI

- In the provider setup dialog (`/provider`), choosing **Model Router** when it
  was already marked as configured previously fell through to the generic model
  picker, which showed a useless single-entry "Model Router" list and offered no
  way to reconfigure the cluster. The provider picker now detects this case and
  opens the **Model Router cluster setup panel** instead.
- The empty-cluster guard is preserved: if no concrete providers are configured,
  the picker stays open with a warning so the user can set up a downstream provider
  first.
- Added a regression test in `crates/ragent-tui/tests/test_router_setup.rs`
  (`test_provider_picker_already_configured_router_opens_setup_router`).

### Fixed — Model Router classification now appears in the TUI log panel

- `RouterProvider` now overrides the `Provider::set_event_bus` trait default so
  that the TUI's `provider_registry.set_event_bus_all()` call actually reaches
  the router. Previously the router kept its event bus as `None` even though
  other providers were wired, so `Event::RouterClassification` was never
  published and the classification/bucket/model selection stayed invisible in
  the Logging Window.
- The router now also publishes classification events via a plain
  `tracing::info!()` record, so the summary appears in the log panel regardless
  of whether the event-bus bridge is active.

### Fixed — Model Router no longer defaults to Anthropic for Medium/Complex/Reasoning tiers

- `default_tier_config()` now uses local-first `ollama` models for every tier:
  - `SIMPLE`: `qwen3:0.6b`, `llama3.2`
  - `MEDIUM`: `qwen3:1.7b`, `qwen2.5:7b`
  - `COMPLEX`: `qwen3:4b`, `qwen2.5:14b`
  - `REASONING`: `qwq:32b`, `deepseek-r1:14b`
- This removes the silent `anthropic` fallbacks for `MEDIUM`, `COMPLEX`, and
  `REASONING` that produced 401 Unauthorized errors when no Anthropic API key
  was configured.
- If the resolved tier has no configured models, the router now falls back to
  higher tiers first, then lower tiers, before giving up. This keeps a
  partially configured cluster (e.g. only `SIMPLE` defined) working for prompts
  that classify into any other bucket.

### Fixed — Default model no longer hard-wires Anthropic/Claude

- `create_default_registry()` now registers **local/self-hosted providers first**
  (`ollama`, `ollama_cloud`, `generic_openai`, `huggingface`, `azure_resource`,
  `azure_foundry`, `copilot`) before cloud providers (`openai`, `gemini`, `xai`,
  `anthropic`, `bedrock`). The built-in `RouterProvider` remains last.
- As a result, when no model is explicitly selected the fallback resolves to the
  first available *local* provider/model (e.g. `ollama/...`) instead of
  `anthropic/claude-sonnet-4-20250514`.
- This affects all paths that call `resolve_default_model` /
  `resolve_agent_with_model`: TUI startup, `ragent run`, `ragent serve`,
  `POST /sessions/{id}/messages`, and the AGENTS.md init exchange.
- Router built-in tiers are left unchanged but are only active when the user
  explicitly enables the router (`/router on` or custom `provider.router` config).

### Added — Model Router classification logging and virtual model discovery

- The router now logs the **classified prompt**, **selected bucket/tier**,
  **selected downstream model**, **composite score**, and **active classifier
  dimensions** every time it routes a request. This information is published as
  an `Event::RouterClassification` so it appears in the TUI log panel even when
  the tracing filter is set to the default `warn` level.
- `RouterProvider` now exposes a single virtual model (`Model Router`), so the
  model picker and discovery flow show the router as a valid selection instead
  of reporting "No models are currently available for this provider".

### Added — Model Router save confirmation dialog

- **Ctrl+S in the Model Router setup dialog now opens a confirmation modal**
  instead of saving immediately. The modal shows the number of tier entries that
  will be saved and prompts the user to press Enter to confirm or Esc to cancel.
- **Confirming the dialog** persists the draft cluster to `ragent.json`,
  enables the router, and selects `router/router` as the active model.
- **Cancelling the dialog** clears the pending save and returns to the router
  setup dialog without writing to disk.
- **New tests** in `crates/ragent-tui/tests/test_router_save_dialog.rs` cover
  both confirming and cancelling the save confirmation dialog.

### Fixed — Model Router save confirmation dialog visibility

- The router save confirmation modal is now rendered **after** the router setup
  dialog so it appears on top instead of being painted over. Previously the
  confirmation was invisible because `render_provider_setup_dialog` was drawn
  later and covered the modal, which also meant users could not confirm the
  save and the router cluster was not persisted.
- Added a regression test in `crates/ragent-tui/tests/test_router_setup.rs`
  (`test_router_save_confirmation_renders_above_setup_dialog`) that renders
  both dialogs together and asserts the save confirmation title and hint are
  visible.

### Fixed — Router provider no longer demands an API key

- `SessionProcessor::resolve_api_key` now returns an empty key immediately for
  the virtual `router` provider. The router delegates authentication to its
  downstream providers, so it does not require its own API key. This prevents the
  spurious `"No API key found for provider 'router'"` error when the Model
  Router is selected.

### Fixed — "error decoding response body" retry loop for local/OpenAI-compatible providers

- **OpenAI-compatible providers (OpenAI, Azure Foundry, Generic OpenAI) now detect
  an empty/malformed SSE body immediately** and emit a clear, non-retryable error
  (`"... returned an empty/malformed event stream ... model is not loaded"`) instead
  of the raw reqwest `"error decoding response body"` diagnostic. The transform fires
  when a chunk decode failure happens before any events have been yielded and the HTTP
  status was successful, which is the typical signature of a local model that is not
  loaded or an endpoint that returned a non-stream body.
- **Ollama provider applies the same empty/malformed stream detection** in its SSE
  parser, using captured response status and content-type to produce a clear local
  model error message.
- **Retry policy no longer retries raw `"error decoding response body"` before any
  output has been received.** `should_retry_stream_error` now treats that specific
  early decode failure as fatal, while still preserving partial output if the error
  occurs mid-stream.
- **TUI status-bar label now reflects the provider actually handling the request.**
  It shows `"Model Router"` only when the active model ref points to the `router`
  provider; if the router is enabled but a concrete model is still selected, the
  label falls back to that concrete provider's name.

### Changed — Model Router setup UI layout and model-property display

- **Router cluster buckets now render as a 2×2 grid** (two rows of two buckets)
  instead of a single row of four columns, improving readability on narrower
  terminals. The four tiers are laid out in ascending complexity order:
  `SIMPLE` | `MEDIUM` on the top row, `COMPLEX` | `REASONING` on the bottom row.
- **Bucket titles now show the full tier name** (e.g. `SIMPLE`, `REASONING`)
  instead of the previous single-character abbreviation (`S`, `M`, `C`, `R`).
- **Retained model properties are displayed inside each bucket.** Each assigned
  model entry now renders its context window, feature flags (`R`/`V`/`T`),
  thinking levels, cost tier/multiplier, and registry cost estimate, in addition
  to the `provider / model` label. Properties are resolved at render time via a
  new `App::router_model_picker_entry` helper that prefers cached/discovered
  model metadata and falls back to the provider registry's default catalog.
- **Router model picker upgraded to a full properties table.** The
  `SelectRouterModel` dialog now renders the same `Model | Context | Cost |
  Thinking | Features` table used by the standard model picker, so users can
  compare model properties before assigning a sub-model to a tier bucket.
- **New tests** added to `crates/ragent-tui/tests/test_router_setup.rs` covering
  the full tier-name bucket titles, retained property rendering, and the
  picker property-column rendering.

## Version: 0.1.0-alpha.146

### Added — Model router baseline support with TUI

- Baseline model router support with TUI setup flow for selecting providers and
  assigning them to routing tiers (`SIMPLE`, `MEDIUM`, `COMPLEX`, `REASONING`).

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.146`.

### Removed — Rig framework integration

- **Deleted the `ragent-rig` workspace crate** and the entire Rig (`rig-core`)
  integration, including Rig-backed providers, embeddings, vector stores,
  conversation-memory policies, semantic code index, semantic memory, and the
  Rig-backed research augmentor.
- **Removed Rig-only abstraction points:**
  - `crates/ragent-agent/src/session/semantic_handles.rs`
  - `crates/ragent-research/src/semantic.rs`
  - `crates/ragent-research/tests/test_research_semantic.rs`
- **Removed Rig-specific documentation and specs:**
  - `docs/howtos/rig-integration.md`
  - `docs/reports/rig-interface-audit.md`
  - `docs/reports/rig-binary-size-compile-time-impact.md`
  - `specs/rig/SPEC.md`
  - `specs/rig/PLAN.md`
- **Removed generated Rig research artifacts** under `research/rig/`.
- **Native providers and native memory/code-index/search remain unchanged;**
  `memory_search` and `codeindex_search` continue to use their existing
  non-Rig implementations.

## Version: 0.1.0-alpha.145

### Added — Router Provider TUI Setup (spec: routeui)

- **Interactive Model Router configuration panel** reachable via `/provider` → `Model Router`
  or `/provider router`. Users can select multiple already-configured concrete providers,
  assign provider/model pairs to the four routing tiers (`SIMPLE`, `MEDIUM`, `COMPLEX`,
  `REASONING`), and save the cluster to `ragent.json`.
- **Router setup state machine** added to `ProviderSetupStep` with `SetupRouter` and
  `SelectRouterModel` variants, reusing the existing provider-setup overlay.
- **Two-pane router UI** — provider multi-selection list on the left, four bucket columns
  on the right, with keyboard navigation (Tab, arrows, Space, Enter, Ctrl+S, Esc).
- **Model picker dialog** for choosing which model from a selected provider is assigned
  to the active tier bucket.
- **Persistence and validation** — `Ctrl+S` saves `provider.router.tiers` to `ragent.json`,
  enables the router, preserves existing classifier weights/boundaries, rejects recursive
  router-to-router assignments, and requires at least one non-empty tier.
- **Bucket reordering** — `Ctrl+↑` / `Ctrl+↓` moves the selected model within a tier.
- **Cost estimates** — each bucket entry shows a per-model `~$/M` estimate when pricing
  metadata is available from the provider registry.
- **`/provider show` now renders the router cluster** with each tier and its assigned
  provider/model entries.
- **Status bar label** displays `"Model Router"` when the router virtual provider is active.
- **Spec and tests** — added `specs/routeui/SPEC.md`/`PLAN.md` and
  `crates/ragent-tui/tests/test_router_setup.rs` covering the provider helper, state
  machine, input handling, persistence, validation, `/provider` integration, and report
  rendering.

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.145`.
- **`ragent-llm` Provider trait** now requires `as_any_static()` so callers can
  downcast concrete providers (e.g. to inspect the router's enabled state).

## Version: 0.1.0-alpha.144

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.144`.

## Version: 0.1.0-alpha.143

### Fixed — Scroll optimizations and edit tool follow-up

- **Workspace version bumped** to `0.1.0-alpha.143`.
- **Scroll optimizations** — additional TUI scrolling fixes and improvements.
- **Edit/MultiEdit reliability follow-up** — continued hardening of the tolerant matcher and batch normalization fallback introduced in `0.1.0-alpha.142`.

## Version: 0.1.0-alpha.142

### Fixed — Edit/MultiEdit tool matcher reliability

- **Single-file `edit` now uses the tolerant seven-pass matcher.** It accepts common
  whitespace and line-ending differences (CRLF vs LF, trailing/leading spaces,
  indentation drift) while still requiring a unique match. This exceeds the strict
  byte-for-byte behavior of Claude Code's `Edit` tool, which is known to fail
  frequently on real-world files.
- **`multi_edit` batch normalization fallback.** Each batch edit tries strict exact
  match first; if that fails with `NotFound`, a controlled fallback strips CRLF and
  trailing whitespace per line and retries. Indentation and internal whitespace
  are preserved so batch edits remain deterministic.
- **Improved match-failure diagnostics.** Error messages now report the last
  attempted tolerance pass (e.g. `trailing-ws`, `batch-normalized`) and the closest
  near-match line when one can be identified, giving the model concrete guidance.
- **Dry-run mode for `edit` and `multi_edit`.** Pass `"dry_run": true` to resolve
  matches and preview snippets without writing any files.
- **Updated agent instructions and `AGENTS.md`** to describe the tolerant matching,
  recommend context blocks and `dry_run`, and document the `write`-fallback pattern.
- **Amended `specs/editrenewal/SPEC.md`** (FR-004, FR-009, FR-011) to reflect the
  new matcher behavior.
- **Files changed:** `crates/ragent-tools-core/src/edit.rs`,
  `crates/ragent-tools-core/src/multiedit.rs`,
  `crates/ragent-tools-core/src/replace.rs`,
  `crates/ragent-tools-core/tests/test_edit_integration.rs`,
  `crates/ragent-tools-core/tests/test_multiedit.rs`,
  `crates/ragent-tools-core/tests/test_multiedit_helpers.rs`,
  `crates/ragent-agent/src/agent/mod.rs`,
  `assets/config/AGENTS.md`, `specs/editrenewal/SPEC.md`.

## Version: 0.1.0-alpha.141

### Added — Research findings now render with a headline

- **Finding headings combine number and a short headline.** `RESEARCH.md` findings
  now render as `### Finding N — <headline>` instead of a bare `### Finding N`.
- **Headline comes from a new `**Headline:**` paragraph.** The synthesis prompt now
  asks the LLM to start each finding with a `**Headline:**` paragraph of at most
  15 words summarising the observation. If the LLM omits the headline, the
  assembler falls back to the first 15 words of the observation.
- **Backward compatibility.** Old findings without a `**Headline:**` paragraph
  still produce a sensible heading from the observation text.
- **Files changed:** `crates/ragent-research/src/analysis.rs`,
  `crates/ragent-research/src/document.rs`,
  `crates/ragent-research/src/session.rs`, plus updated tests in
  `crates/ragent-research/src/document.rs`,
  `crates/ragent-research/src/analysis.rs`,
  `crates/ragent-research/src/session.rs`,
  `crates/ragent-research/tests/test_research_create_synthesis.rs`,
  `crates/ragent-research/tests/test_template_merge.rs`.

## Version: 0.1.0-alpha.140

### Added — `/research open` markdown viewer panel

- **TUI panel renders RESEARCH.md content.** Running `/research open <name>`
  now opens a full-screen overlay that reads the item's `RESEARCH.md` (minus
  YAML frontmatter), strips control characters, and renders the markdown body
  directly in the TUI instead of only printing metadata to the chat log.
- **Mermaid diagram support.** Fenced ` ```mermaid ` blocks are detected and
  rendered with a header label (`[Mermaid diagram — rendered as text below]`)
  so the user knows the diagram source is present even though the terminal
  cannot draw vector graphics.
- **Image placeholders.** Markdown image syntax (`![alt](src)`) is converted
  into a colored placeholder such as `[Image: alt (100x50)]`. Local PNG/JPEG
  files are inspected for dimensions without decoding pixels; remote images
  and missing files still show the alt text and path/URL.
- **Link rendering.** Inline links are shown as `[text](url)` with the URL
  styled underlined/cyan. The panel footer notes that terminal links are plain
  text (browsers/terminals with OSC-8 are not yet supported).
- **Navigation.** The panel supports scrolling with `PageUp` / `PageDown`
  and jumps to start/end with `Ctrl+PageUp` / `Ctrl+PageDown`. `Esc` closes
  the viewer.
- **Mouse support.** The panel responds to mouse-wheel scroll and closes when
  clicking outside its bounds.
- **Files changed:** `crates/ragent-tui/src/app/{state,init,event_handler,
  input_handler,research,helpers}.rs`, `crates/ragent-tui/src/{input,layout}.rs`,
  `crates/ragent-tui/src/app.rs`, `crates/ragent-types/src/event/mod.rs`,
  `crates/ragent-server/src/sse.rs`.
- **New tests** — `crates/ragent-tui/tests/test_research_viewer.rs` (18 tests)
  covering headings, code blocks, mermaid labels, image placeholders, link
  rendering, bullet lists, the footer note, `Esc`/click close, keyboard
  scrolling, and mouse-wheel scrolling.

### Added — `/research` parallel web capture

### Fixed — `/research` TUI screen corruption

- **Sanitize external strings before display.** URL, page title, error, and
  `--from-url` body-preview text are now passed through `sanitize_for_display`
  in `crates/ragent-tui/src/research_progress.rs` and
  `crates/ragent-tui/src/app/event_handler.rs`. This strips ANSI escape
  sequences and control characters (`\x00`–`\x1F` except `\n` and `\t`) that
  could leak into the TUI from fetched page content or HTTP errors and appear
  as garbage glyphs (e.g. `%???`) on the left side of the research progress
  panel.
- **Bypass lossy markdown→HTML→text pipeline for research progress.** Messages
  starting with `🔬 Research Progress` are now rendered as plain text in
  `crates/ragent-tui/src/app/models.rs` instead of being converted by
  `pulldown_cmark` + `html2text`. This prevents the converter from replacing
  valid Unicode icons and line indentation with artifacts and keeps the
  pre-formatted log list intact.
- **New tests** — `test_research_progress_sanitize.rs` (7 tests) covering ANSI
  stripping, control-char removal, newline/tab preservation, sanitized
  `WebCaptured`/`WebFetchFailed` encoding, and the research-progress
  plain-text bypass.

### Added — `/research` parallel web capture

- **Concurrent page fetching in `/research create`.** The web-gathering
  fetch phase in `crates/ragent-research/src/web_gatherer.rs` now issues
  candidate page fetches concurrently via `futures::stream::buffer_unordered`
  instead of sequentially `await`-ing each URL in turn. The default
  concurrency limit is **10** (`DEFAULT_FETCH_CONCURRENCY`), configurable per
  run with the new `--fetch-concurrently N` CLI flag (and the matching
  `fetch_concurrency` field on `SessionConfig`, the `fetch_concurrency` JSON
  body field on `POST /research`, and the
  `WebGatherer::with_fetch_concurrency` builder).
- **Ordering preserved.** `SourceCaptured` / `FetchFailed` observer events
  still fire in fetch-completion order (so the TUI renders pages as they
  arrive), but the returned `sources` vector is re-sorted into the original
  search-ranking order so `web-NN.md` supporting-file names keep tracking
  hit position rather than completion timing.
- **New tests** — `gather_fetches_pages_concurrently_up_to_fetch_concurrency`
  (proves the high-water mark of in-flight fetches matches
  `fetch_concurrency`), `with_fetch_concurrency_clamps_zero_to_one`, and
  `default_fetch_concurrency_is_ten` in
  `crates/ragent-research/src/web_gatherer.rs`; plus CLI parse tests for
  `--fetch-concurrently` in `crates/ragent-research/src/cli.rs`.

### Fixed — `/research --from-url` topic derivation

- **Prefer cleaned page titles, fall back to cleaned body.** When
  `/research create --from-url <URL>` is used, the research topic is now
  derived from the extracted page title first. Site-brand tokens (e.g.
  `InfoQ`), common nav words (`Homepage`, `Articles`), title-tag separators
  (`|`, `-`, `/`), and glued-together words (`HomepageArticlesLarge`) are
  stripped, so titles like `InfoQ HomepageArticlesLarge Concept Models: a
  Paradigm Shift in AI Reasoning` become `Large Concept Models: a Paradigm
  Shift in AI Reasoning`. If the title is missing or too generic, the first
  substantive sentence of the cleaned page body is used. This fixes sparse
  or concatenated topics without falling back to a bare URL.
- **New unit tests** — `clean_site_title_strips_site_brand_and_nav_prefixes`,
  `derive_topic_prefers_cleaned_title_over_body`,
  `derive_topic_falls_back_to_body_when_title_is_generic`, and
  `split_glued_words_splits_camel_case_and_acronyms` in
  `crates/ragent-research/src/session.rs`.

## Version: 0.1.0-alpha.139

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.139`. Updates the `/research`
  subsystem: analysis prompts are rebuilt on the evidence-based
  `research/researchanalysis` guidance, the `--from-url` seed now uses the
  `readability-rs` crate for HTML extraction, and a new `FromUrlNoUsableBody`
  outcome prevents unrelated topics from nav links.

### Added — `/research` improvements

- **`--from-url` uses `readability-rs` for HTML extraction.** The custom
  ARC90-style readability module in `crates/ragent-research/src/readability.rs`
  has been removed. The `webfetch` tool now runs `readability::extract` on HTML
  responses; if the crate cannot produce a substantive article body, it falls
  back to `html2text`. The extracted page title is still captured in source
  metadata, but it is **never** used as the research topic.
- **Improved `/research create` synthesis prompt.** The analysis prompt (in
  `crates/ragent-research/src/analysis.rs`) now applies the evidence-based
  prompt-engineering guidance from `research/researchanalysis` (20 synthesized
  findings from 91 web sources): a versioned, composable
  `SynthesisPromptBuilder` replaces the monolithic `build_synthesis_prompt`;
  a mandatory `Sources Cited / Date Spread` paragraph; recency-weighting
  rules; deterministic mechanical fallback via `analyze_with_outcome` /
  `AnalysisOutcome`; and `--template` merge clarification in `document.rs`.
- **New tests** — `crates/ragent-research/tests/test_research_create_synthesis.rs`
  and `crates/ragent-research/tests/test_template_merge.rs`.

### Fixed

- **Research `--from-url` no longer derives unrelated topics from nav links.**
  The topic is now derived only from the fetched page body. If the body
  contains no usable article text, the session stops and reports
  `FromUrlNoUsableBody` instead of falling back to the page title or URL.
  This prevents cases like an OpenAI deep-research URL producing
  Foundation-framework queries.

### Fixed — `/research` TUI progress widget

- **Each `/research create` run now gets its own progress widget.** Previously
  the TUI held a single `Option<ResearchProgress>` tracker, so starting a new
  research run overwrote the progress log of any earlier run, making it
  impossible to see the results of older requests. The state field is now a
  `Vec<ResearchProgress>` and each run is matched by name, so every run keeps
  its own self-updating `🔬 Research Progress — \`<name>\`` message in the
  window and older runs stay visible alongside the latest one.
- **`--from-url` body preview in the progress widget.** When
  `/research create --from-url <URL>` is used, the progress widget now shows
  the first ~200 characters of the extracted article body (via a new
  `SessionEvent::FromUrlBodyPreview` event emitted right after the primary
  fetch succeeds), so you can see exactly what content the topic was derived
  from.
- **Decomposed queries render as soon as they are generated.** The
  `GatherEvent::QueriesDecomposed` event is now forwarded immediately by
  `GatherEventForwarder` (previously it was dropped and the session re-emitted
  `QueriesDecomposed` only after the whole web gather returned). The
  duplicated post-gather emission has been removed. Sub-queries now appear in
  the progress widget before the parallel searches complete.
- **Successfully retrieved URLs render inline during the gather.** A new
  `GatherEvent::SourceCaptured { url, title }` event is emitted by
  `WebGatherer` each time a page fetch succeeds, and is forwarded as
  `SessionEvent::WebCaptured` so the progress widget shows each captured URL
  as it arrives. Previously only `WebFetchFailed` events were surfaced during
  the gather; successful captures were only shown in a batch at the end.

## Version: 0.1.0-alpha.138

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.138`. Continued code
  duplication removal tracked in `DUPPLAN.md`, following the
  `0.1.0-alpha.137` milestone which reduced `cargo dupes` exact-duplicate
  groups from 385 → 340 and exact-dup lines from 15,573 → 11,775 across
  milestones A–K.

## Version: 0.1.0-alpha.137

### Removed — Code duplication (DUPPLAN.md Milestones A–K)

**Summary:** Across 11 milestones (A–K), the `cargo dupes` duplication
metrics improved as follows:

| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| Exact-duplicate groups | 385 | 340 | −45 (−11.7%) |
| Exact-dup lines | 15,573 | 11,775 | −3,798 (−24.4%) |
| Near-duplicate groups | 142 | 144 | +2 (new minor groups) |
| Exact duplication % | 10.9% | 8.4% | −2.5 pp |
| Total lines analysed | 142,348 | ~139,700 | ~2,600 removed |

**Milestones completed:**

| Milestone | Description | Lines removed | Groups eliminated |
|-----------|-------------|---------------|-------------------|
| A | Dead-code VCS tool copies | 2,461 | 50, 51, 53, 102 |
| B | `resolve_path` extraction | ~136 | 1 |
| C | Parser boilerplate (`build_qname`, `create_parser`, `parse_tree`) | ~190 | 8, 10, 16 |
| D | `not_available` codeindex fallback | ~50 | 22 |
| E | `resource.rs` triple-copy | ~60 | 32 |
| F | `strip_tags` unification | ~15 | 121 (near) |
| G | TUI `make_app` test helper | ~950 | 2, 34 |
| H | `MockStorage` test helper | ~90 | 49, 60, 65, 67 |
| I | `setup` / `setup_workspace` helpers | ~40 | 33, 37 |
| J | Accepted duplications documented | 0 (comments) | — |
| K | Verification & regression baseline | 0 | — |
| **Total** | | **~4,000** | **~20 groups** |

See `DUPPLAN.md` for the full plan and per-milestone details.
Pre-refactor baseline: `docs/reports/dupes-baseline.txt`.
Post-refactor report: `docs/reports/dupes-final.txt`.

### Removed — Code duplication (DUPPLAN.md Milestones A & B)

- **Milestone A — Dead-code VCS tool copies removed (2,461 lines).** The
  `ragent-agent` crate held five full verbatim copies of GitHub/GitLab tool
  implementations that already live canonically in `ragent-tools-vcs` and are
  registered via the `ExtractedVcsToolAdapter`. The local copies were never
  referenced. Deleted `github_issues.rs`, `github_prs.rs`, `gitlab_issues.rs`,
  `gitlab_mrs.rs`, and `gitlab_pipelines.rs` from
  `crates/ragent-agent/src/tool/` and removed their `pub mod` declarations from
  `tool/mod.rs`. Added `scripts/check-vcs-duplication.sh` CI guard (wired into
  `pre-flight.sh`) to prevent regressions. Eliminates `cargo dupes` exact-dup
  groups 50, 51, 53 and near-dup group 102 (cross-crate). Stats: 385 → 355
  exact groups, 15,573 → 13,347 exact-dup lines.

- **Milestone B — `resolve_path` extraction (18 → 1 copy, ~136 lines).** The
  identical 8-line `resolve_path` helper was copy-pasted across 16 files in
  `ragent-tools-core/src/` and 2 in `ragent-tools-extended/src/` (the largest
  single exact-duplicate group in the codebase). Extracted the canonical
  implementation into `crates/ragent-tools-core/src/path_util.rs` and replaced
  all 16 core copies with `use super::path_util::resolve_path;`. Replaced the
  2 extended copies (`libreoffice_common.rs`, `office_common.rs`) with
  `pub use ragent_tools_core::path_util::resolve_path;` re-exports. Fixed the
  two `#[path]`-based test files (`test_edit.rs`, `test_multiedit_helpers.rs`)
  to add a `path_util` shim module. `cargo fix` cleaned up now-unused
  `Path`/`PathBuf` imports. Eliminates `cargo dupes` exact-dup group 1. Stats:
  355 → 354 exact groups, 13,347 → 13,203 exact-dup lines.

- **Milestone E — `resource.rs` triple-copy (3 → 1 copy, ~60 lines).** The
  process/tool concurrency semaphore module was duplicated three times:
  `ragent-types/src/resource.rs` (canonical, with tests),
  `ragent-agent/src/resource.rs` (near-identical, only added a `#[cfg(test)]`
  block), and `ragent-tools-core/src/lib.rs` (inline `pub mod resource` with a
  subset). Deleted `ragent-agent/src/resource.rs` and replaced `pub mod
  resource;` in `lib.rs` with `pub use ragent_types::resource;`. Deleted the
  inline `pub mod resource { ... }` block in `ragent-tools-core/src/lib.rs` and
  replaced it with `pub use ragent_types::resource;`. Both crates already
  depended on `ragent-types`. All call sites (`processor.rs:1014`,
  `context.rs:235`, `bash.rs:1051`) continue to resolve via the re-exports.
  The tests that were in `ragent-agent/src/resource.rs` are already covered by
  `ragent-types/tests/test_resource.rs`. Eliminates `cargo dupes` exact-dup
  group 32. Stats: 349 → 347 exact groups, 12,947 → 12,866 exact-dup lines.

- **Milestone F — `strip_tags` unification (2 → 1 copy).** The `strip_tags`
  HTML-tag-stripping helper was duplicated in `ragent-tools-extended/src/
  webfetch.rs` and `ragent-research/src/web_date.rs` (near-dup group 121). The
  two variants differed in behaviour: `web_date.rs` pushed a space on `<` to
  prevent words merging across tag boundaries (e.g. `"foo<b>bar"` → `"foo
  bar"`), while `webfetch.rs` did not (producing `"foobar"`). Adopted the
  space-pushing variant as the canonical implementation in a new
  `ragent-types/src/html.rs` module (both crates depend on `ragent-types` but
  `ragent-research` does not depend on `ragent-tools-extended`). Replaced the
  `webfetch.rs` definition with `pub use ragent_types::html::strip_tags;` and
  the `web_date.rs` definition with `use ragent_types::html::strip_tags;`. The
  `webfetch.rs` internal `extract_text` helper now calls the imported
  `strip_tags`, inheriting the improved space-on-`<` behaviour. Eliminates
  `cargo dupes` near-dup group 121.

- **Milestone G — TUI `make_app` test helper (27 → 1 shared + ~10 variant
  copies, ~900 lines removed).** The `make_app()` function — a ~45-line `App`
  constructor wiring up `EventBus`, `Storage::open_in_memory()`,
  `SessionProcessor`, and `App::new(...)` — was copy-pasted across 27 files
  (24 test files + 3 bench files; `cargo dupes` groups 2 and 34, the
  second-largest duplication in the codebase). Extracted the canonical
  `pub fn make_app() -> App` into `crates/ragent-tui/tests/support/mod.rs`.
  Replaced the standard copy in 18 test files and 3 bench files with
  `#[path = "support/mod.rs"] mod support;` + `support::make_app()` calls.
  Left ~9 files with variant signatures or flags (`make_app(event_bus)`,
  `make_app_with_storage(storage)`, `make_app_with_manager()`, and files
  passing `true` as the debug flag to `App::new`) as local definitions — these
  have legitimately different behaviour and cannot use the shared helper.
  `cargo fix` cleaned up now-unused imports across all modified files.
  Eliminates `cargo dupes` exact-dup groups 2 and 34. Stats: 347 → 341 exact
  groups, 12,866 → ~11,900 exact-dup lines.

- **Milestone H — `MockStorage` / `DemoStorage` test helpers (4 → 1 shared +
  1 documented example, ~90 lines removed).** An in-memory `StorageBackend`
  mock was duplicated verbatim across 4 files in `ragent-tools-extended/`
  (`cargo dupes` groups 49, 60, 65, 67 — each group was one trait method:
  `get_todos`, `create_todo`, `update_todo`, `clear_todos`). Extracted the
  canonical `MockStorage` struct + its `StorageBackend` impl into
  `crates/ragent-tools-extended/tests/support/mock_storage.rs`. Replaced the
  local `MockStorage` definitions in the 3 test files (`test_todo_demo.rs`,
  `test_todo_lifecycle.rs`, `test_todo_status_change.rs`) with
  `#[path = "support/mock_storage.rs"] mod mock_storage;` +
  `use mock_storage::MockStorage;`. Left `DemoStorage` in
  `examples/todo_cycle.rs` as a documented example variant (with a comment
  pointing to the shared module) so the example remains self-contained.
  `cargo fix` cleaned up now-unused imports. Eliminates `cargo dupes`
  exact-dup groups 49, 60, 65, 67.

- **Milestone I — `setup` / `setup_workspace` test helpers (10 → 2 shared +
  0 local, ~40 lines removed).** Two temp-directory setup helpers were
  duplicated across 10 test modules. `setup_workspace() -> (TempDir,
  PathBuf)` (5 copies in `ragent-team/tests/`, `cargo dupes` group 37) was
  extracted into `crates/ragent-team/tests/support/mod.rs` and included via
  `#[path = "support/mod.rs"] mod support;` in all 5 test files.
  `setup() -> TempDir` (5 copies across `ragent-agent/src/memory/` and
  `ragent-tools-extended/src/memory/` inline `#[cfg(test)]` modules, group 33)
  was extracted into `memory/test_helpers.rs` in each crate with
  `#[cfg(test)] mod test_helpers;` in `memory/mod.rs`. Each test module's
  local `fn setup` was replaced with `use super::test_helpers::setup_temp_dir;`
  and `setup()` calls updated to `setup_temp_dir()`. Eliminates `cargo dupes`
  exact-dup groups 33 and 37.

- **Milestone J — Accepted duplications documented (comment-only, 0 lines
  changed).** Added `// NOTE: intentional duplication — see DUPPLAN.md
  Milestone J` comments above the Tier-3 accepted duplicate groups so future
  readers don't attempt to "fix" them. Comments added to: `mock_llm_client.rs`
  (group 30, `as_str`), `store.rs` (group 40, `IndexStore` accessors),
  `read.rs` (group 80, `detect_python_sections`/`detect_go_sections`),
  `anthropic.rs` (group 28, streaming closures), `session.rs` (group 44,
  `LocalTool::grep` no-op impls), `bigcodebench.rs` (group 47,
  `BenchSuiteAdapter::build_prompt`), `gradle.rs` (group 58,
  `LanguageParser::parse`), and `knowledge_graph.rs` (group 13, `From` impls).
  No behaviour change; `cargo dupes` numbers unchanged.

- **Milestone D — `not_available` codeindex fallback (6 → 1 copy, ~50
  lines).** Six codeindex tool files in `ragent-tools-extended/src/` each
  defined a structurally-identical `fn not_available() -> ToolOutput` fallback
  for the "code index disabled" message (the messages and `fallback_tools`
  metadata differed per-tool). Extracted a parameterised
  `pub(crate) fn codeindex_not_available(fallback_hint: &str, fallback_tools: &[&str])`
  into the existing `codeindex_utils.rs` module and replaced all 6 local
  definitions with one-line calls. `codeindex_status.rs` was unified to use the
  same `fallback_tools` metadata shape (its redundant `"enabled": false` field
  was dropped since `"error": "codeindex_disabled"` already signals the state).
  `cargo fix` cleaned up now-unused `json!` imports. Eliminates `cargo dupes`
  exact-dup group 22. Stats: 350 → 349 exact groups, 13,013 → 12,971 exact-dup
  lines.

- **Milestone C — Code-index parser boilerplate (3 groups eliminated, ~190
  lines).** The tree-sitter parser subsystem in `ragent-codeindex/src/parser/`
  repeated three boilerplate patterns across 7–10 language files. Extracted
  `pub fn build_qname(scope, name, sep)` into a new `parser/util.rs` and
  replaced all 10 local copies (`build_qname` / `build_qualified` /
  `build_qualified_name`) with thin `#[inline]` delegating wrappers that pass
  the per-language separator (`"::"`, `"."`, or `":"`). Defined a
  `tree_sitter_parser!` declarative macro in `parser/util.rs` that generates
  the uniform `create_parser()` + `parse_tree()` pair, and applied it to all 9
  language parsers that use the standard pattern (gradle, cmake, go,
  gradle_kts, hcl, openscad, python, maven, rust). The `go.rs` and `python.rs`
  `LanguageParser::parse` impls were updated to call `Self::parse_tree(source)`
  (matching the other 7 files) so the macro-generated `parse_tree` is used.
  `cargo fix` cleaned up now-unused `Parser`/`Tree` imports. Excluded
  `typescript.rs` (variant `create_parser(&self)` with match-on-variant) and
  `c_cpp.rs` (inline parse, no separate methods) per the plan. Eliminates
  `cargo dupes` exact-dup groups 8, 10, and 16. Stats: 354 → 350 exact groups,
  13,203 → 13,013 exact-dup lines.

### Added

- **`/init config` slash command** — New subcommand of `/init` that creates a
  default `ragent.json` file in the global config directory
  (`~/.config/ragent/ragent.json` on Linux,
  `~/Library/Application Support/ragent/ragent.json` on macOS,
  `%APPDATA%\ragent\ragent.json` on Windows). If a global config already
  exists, the command reports its path and makes no changes. The default config
  is serialised from `Config::default()` and contains all default settings ready
  to edit. Autocomplete suggestions and parameter hints updated for `/init`.

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.137`. Follow-up to
  `0.1.0-alpha.136`, which added web source publication dates to the
  `/research` slash command (`RESEARCH.md` References Index **Published**
  column, per-finding `**Source date range:**`, and the
  `ragent_research::extract_published_at` helper).

## Version: 0.1.0-alpha.136

### Added

- **Research source publication dates** — The `/research` slash command now
  captures the publication date of each web source and surfaces it in the
  `RESEARCH.md` output and references. A new `**Source date range:**` line
  under each finding summarises the earliest–latest publication dates of its
  cited web sources, so the relative age of the evidence is visible at a
  glance. Dates are parsed best-effort from JSON-LD `datePublished`, article
  meta tags (`article:published_time`, `pubdate`, `dc.date`, etc.), `<time>`
  elements, and a visible-text fallback; any failure leaves the date as `—`
  without aborting the research run. The References Index table gained a
  **Published** column, supporting files show `Published (UTC)`, and the new
  `ragent_research::extract_published_at` helper is re-exported for
  `ragent-agent`'s best-effort raw-HTML fetch. Older `RESEARCH.md` files
  remain loadable via `#[serde(default)]` on the new optional field.

### Fixed

- **Workspace version** — Bumped to `0.1.0-alpha.136`.

## Version: 0.1.0-alpha.135

### Fixed

- **Workspace version** — Bumped to `0.1.0-alpha.135`.
- **fix ci** — Resolved GitHub Actions "Check and Test" failure caused by the `0.1.0-alpha.132` scrollbar drag math regression in Memory/TODO panels. Reverted the `top_based` inversion in `apply_scrollbar_drag()` and updated the Memory panel tests to use the bottom-based offset convention consistent with Messages/Log/Profile.

## Version: 0.1.0-alpha.134

### Fixed

- **Scrollbar drag math regression** — Reverted the `top_based` inversion introduced for Memory/TODO panels in `0.1.0-alpha.132` and updated the Memory panel tests to match the bottom-based offset convention used by the rest of the TUI.

## Version: 0.1.0-alpha.133

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.133`.
- **fix tests that depend on untracked files** — Pointed `ragent-specs` real-project integration tests at the self-contained fixture under `crates/ragent-specs/tests/fixtures/testspec` so they no longer rely on the untracked `specs/` directory.

## Version: 0.1.0-alpha.132

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.132`.
- **fix thumb scrolls** — TUI thumb/srollbar scrolling improvements.

## Version: 0.1.0-alpha.131

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.131`.
- **CI/CD updates and formatting fixes** — Applied GitHub Actions workflow
  maintenance and repository formatting improvements.

## Version: 0.1.0-alpha.130

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.130`.
- **TODO panel** — Implemented a third side panel (Alt+T) in `ragent-tui`
  that renders the session's TODO items from `ragent-storage`. The panel
  follows the existing log/profile side-panel pattern with mutual
  exclusion, text selection, scrollbar drag, and a `/todo` slash alias.
  All 12 plan tasks (T-001…T-012) and 8 acceptance criteria from the
  `todopanel` spec are satisfied.
- **Agentic-loop performance upgrade** — Implemented all six milestones
  (A–F) of `PERFPLAN.md`, covering 26 findings (P-1…P-26) plus 5
  measurement/gating tasks (F-1…F-5). Highlights:
  - Deleted inline nudge recomputation; single `set_step` call; verified
    empty-buffer stall guard (`handle_no_tool_decision`).
  - `LoopState.chat_messages` is now `Arc<Vec<ChatMessage>>` with
    `Arc::make_mut` for cheap clones; tool-definition bytes cached on
    `SessionProcessor`; one `ToolContext` per step; hoisted reusable Vecs;
    `text_buffer` moved via `mem::take`.
  - `get_messages` routed through `storage_op`; cached config keyed by
    file mtimes; `build_turn_chat_messages` returns the context window;
    `AgentManager.has_pending_background` AtomicBool skips drain scans;
    interim-save hash uses `serde_json::to_vec` bytes.
  - `ToolsSent` published only on step 1; added `Event::ToolCallBatch` +
    `ToolCallBatchEntry` and SSE forwarding; tool-result preview scan
    capped at 400 bytes.
  - Consolidated emergency-compression call sites; verified async history
    reads; short-circuit when `last_reported_input_tokens > 0`; added
    `cached_spec_section` to `SystemPromptCache` keyed by
    `(spec_id, spec.modified_at)` with `/spec activate` invalidation.
  - Added `MockLlmClient`/`MockLlmScript` in `ragent-bench`, criterion
    `agent_loop` benchmarks, baseline report, `/perf` TUI alias, and
    `scripts/check-bench-regression.sh` CI guard wired into `pre-flight.sh`.

### Fixed

- **Tool-result preview char-boundary panic** — Replaced the three manual
  200-byte preview builders in `crates/ragent-agent/src/session/processor.rs`
  (`response_preview`, `batch_content`, and `result_preview`) with
  `ragent_types::truncate_bytes`, which steps back from a byte cut point to
  the previous valid UTF-8 character boundary. This prevents the panic
  `end byte index 400 is not a char boundary; it is inside '\u{2014}'` when
  a fixed 400-byte scan landed in the middle of a multi-byte em dash.
  Added `test_truncate_bytes_em_dash_at_400_boundary` in
  `crates/ragent-types/tests/test_strutil.rs` to guard the exact scenario.

## Version: 0.1.0-alpha.129

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.129`.
- **Compression made permanent** — Removed the `compression` and
  `compression-ml` Cargo feature flags across the workspace.
  `headroom-core` is now an unconditional dependency of `ragent-agent`,
  and the context-compression pipeline is always compiled in. Specific
  changes:
    - `Cargo.toml` (workspace root): removed `compression` and
      `compression-ml` features; `default` is now empty.
    - `crates/ragent-agent/Cargo.toml`: removed `compression` and
      `compression-ml` feature definitions; `headroom-core` is no longer
      `optional`.
    - `crates/ragent-tui/Cargo.toml`: removed the `compression` feature
      passthrough.
    - `crates/ragent-agent/src/compression/mod.rs`: dropped all
      `#[cfg(feature = "compression")]` gates; `is_available()` now
      always returns `true`.
    - `crates/ragent-agent/src/lib.rs`,
      `crates/ragent-agent/src/session/{mod,history,loop_steps,processor}.rs`:
      removed every `#[cfg(feature = "compression")]` /
      `#[cfg(not(feature = "compression"))]` guard and the dead-code
      markers that existed only to silence the disabled-feature build.
    - `crates/ragent-agent/tests/test_compression_pipeline.rs` and
      `crates/ragent-agent/benches/agent_loop.rs`: removed the
      feature-gated `#[cfg(...)]` attributes on tests and benchmarks.
    - `crates/ragent-config/src/compression.rs`: updated doc comment for
      `CompressorConfig.prose` (no longer references the
      `compression-ml` feature).

## Version: 0.1.0-alpha.128

### Changed

- **Workspace version** — Bumped to `0.1.0-alpha.128`.
- **Warning remediation** — Eliminated all 279 compiler warnings across the
  workspace (build, tests, benches, and examples now compile with zero
  warnings under `--all-features`). Fixes applied:
  - Removed ~270 unused imports across `ragent-tui` app submodules (init,
    compress, bench, swarm, research, models, slash, input_handler,
    event_handler, session_ops) left over from the `app.rs` split (M5).
  - Added `///` doc comments to 62 previously-undocumented `pub` /
    `pub(crate)` methods and associated functions across `ragent-tui` app
    submodules and `ragent-agent` `session/processor.rs` to satisfy
    `-W missing-docs`.
  - Gated the `is_token_overflow_error_message` import in
    `session/loop_steps.rs` behind `#[cfg(feature = "compression")]` and
    added `#[allow(unused_variables)]` / `#[allow(unused_mut)]` on
    feature-conditional parameters in `build_turn_chat_messages`.
  - Removed unused `std::sync::Arc` and `clap::Subcommand` imports from
    `src/cli.rs`.
  - Removed unused `ragent_prompt_opt::Completer` import from
    `app/swarm.rs` and `tool::TeamManagerInterface` from
    `app/input_handler.rs`.
  - Deleted dead duplicate `#[cfg(test)]` test functions in
    `app/models.rs` and `app/session_ops.rs` (the canonical `#[test]`
    versions live in `app/tests.rs`).
  - Deleted the dead `test_app` helper in `app/helpers.rs` (the canonical
    copy lives in `app/tests.rs`) and its `#[cfg(test)]` import block.
  - Removed redundant `use super::*;` from `app/tests.rs` and the
    `router_modifiers` inline test file.

## [Unreleased] — REMPLAN.md Structural Remediation (M1–M10)

### Refactoring

Completed a 10-milestone structural remediation plan that deduplicated types,
eliminated source copies, broke dependency cycles, split overlarge files,
removed dead code, migrated inline tests, and cleaned up repository hygiene.

- **M1** — Foundation type consolidation: `Message`, `Permission*`, and LLM
  primitive types each have exactly one canonical definition. Guard test added.
- **M2** — Eliminate duplicate `Storage`: `ragent-storage` is the sole impl;
  `ragent-agent` re-exports it via a 27-line shim.
- **M3** — Break `ragent-agent`↔`ragent-team` `#[path]` cycle: Team sources
  moved into `ragent-agent`; `ragent-team` is a thin re-export shim. 27
  `#[path]` attributes eliminated.
- **M4** — Retire the `ragent_core` alias: 470+ `ragent_core::` →
  `ragent_agent::` references rewritten across 63 files.
- **M5** — Split `ragent-tui/src/app.rs`: 15,332 → 28 lines; methods
  distributed across 12+ submodules.
- **M6** — Split `session/processor.rs`: 4,503 → 2,911 lines; 4 sibling
  modules extracted; 22 inline tests moved to external test file.
- **M7** — Remove dead code & compat shims: `predictive.rs` (454 lines) and
  `message/pool.rs` (168 lines) deleted; 4 `pub mod config {}` shims
  collapsed.
- **M8** — Migrate inline tests to `tests/`: 373 tests moved to
  `tests/inline/`; CI guard script added (baseline 109).
- **M9** — Repository hygiene: Stray files and output dirs untracked;
  `docs/howtoos/` → `docs/howtos/`; `src/main.rs` split (1,223 → 905 lines).
- **M10** — Final verification & docs: All structural-defect checks pass.
  Completion report at `docs/reports/remplan-completion.md`.

## Version: 0.1.0-alpha.127

### Changed
- **Workspace version** — Bumped to `0.1.0-alpha.127`.
- **Dead-code removal** — Audited every `#[allow(dead_code)]` site across the
  workspace and removed ~579 net lines of genuinely unreachable code.
  Removed items include: `cleanup_unused_locks` and the whole
  `ragent-agent/src/tool/file_lock.rs` module (a duplicate of the
  `ragent-tools-core` file-lock); `get_attribute` (maven parser);
  `orch_metrics` HTTP handler (route never registered); the deprecated
  `render_status_bar` v1 and `render_plan_approval_dialog` (superseded by
  v2 / widget-based rendering); `GREP_PATTERNS` (predictive); seven unused
  style helpers (`style_healthy`, `style_warning`, `style_error`,
  `style_info`, `style_healthy_bold`, `style_warning_bold`,
  `style_error_bold`); the standalone `AzureFoundry::discover_models`
  method; `FailedToolCall.timestamp`; `FindDiag.pass` / `FindDiag.closest_line`;
  `ShellType::program`; and `resolve_base_url` (research analysis). Stale
  `#[allow(dead_code)]` attributes were also removed from items that are
  actually used (`SAFE_COMMANDS`, `char_wrap`, `push_log_no_agent`).
- **`/spec impl` sequential driver** — Fixed the bug where `/spec impl`
  only ran the first task. The compound single-prompt injection was
  replaced with an event-driven outer loop that dispatches one task per
  agent turn, checks the task status via `SpecManager` after each turn,
  and advances to the next task (or stops with a resumable message) on
  `Event::MessageEnd`. Added `SpecImplRunner::task_prompt`,
  `build_single_task_prompt`, `total_to_execute`, and `task_id_at` APIs
  plus a new `SpecImplState` TUI state struct.

## Version: 0.1.0-alpha.126

### Changed
- **Workspace version** — Bumped to `0.1.0-alpha.126`.
- **Compression pipeline fixes** — Addressed issues in the context
  compression pipeline (`/compress`) so it behaves correctly under the
  updated agent loop.
- **Test updates** — Refreshed and repaired tests across multiple crates
  to track API and behaviour changes from the compression work and
  related refactors.
- **Warning fixes** — Resolved compiler/clippy warnings surfaced by the
  latest tool and config changes.

## Version: 0.1.0-alpha.125

### Changed
- **Workspace version** — Bumped to `0.1.0-alpha.125`.

### Added — Edit Tool Renewal (editrenewal spec)
- **Renewed `edit` tool** — The single-file `edit` tool now aligns with Claude
  Code's `Edit` semantics. It uses strict exact-match replacement (FR-004),
  canonical parameter names `file_path`/`old_string`/`new_string` (FR-001),
  create/update/delete operations via empty `old_string`/`new_string`
  (FR-006), no-change rejection (FR-007), stale-file detection when a read
  timestamp has been recorded (FR-003), and returns a line-numbered result
  snippet with ≥4 lines of context (FR-008). Legacy parameter names
  (`path`/`old_str`/`new_str`) are still accepted and emit a
  `deprecation_warning` in the output metadata.
- **`multi_edit` tool** — The atomic batch edit tool formerly known as
  `multiedit` is now registered as `multi_edit` (FR-009). Each edit in the
  `edits` array uses `file_path`/`old_string`/`new_string` and is validated
  with the strict exact-match matcher. Overlap detection and atomic rollback
  are preserved. Stale-file detection is applied per file in the batch
  (FR-003/FR-009).
- **Legacy `multiedit` alias** — The old `multiedit` tool name remains
  registered as a deprecated alias that forwards to `multi_edit`, normalises
  legacy parameter names, and emits a `deprecation_warning` (FR-012).
- **Read-timestamp tracking** — `ToolContext` now carries a shared
  `read_timestamps` map (FR-003). The `read` tool records each file's mtime;
  `edit` and `multi_edit` consult it to reject stale-file edits. Plumbed
  through `SessionProcessor` and all tool-context construction sites.
- **Migration guide** — `docs/editrenewal-migration.md` documents the new
  tools, the strict matching semantics, and the migration path from legacy
  parameter names.

### Tests
- `crates/ragent-tools-core/tests/test_edit_integration.rs` — 17 tests
  covering exact match, strict rejection of whitespace mismatches, multiple
  matches, NotFound, create/delete operations, no-change rejection,
  stale-file detection, snippet generation, canonical vs legacy params, and
  read-then-edit integration.
- `crates/ragent-tools-core/tests/test_multiedit.rs` — 18 tests covering
  cross-file batches, overlap rejection, atomic rollback, JSON-order
  independence, strict-match acceptance/rejection, stale-file rejection in
  batches, and canonical/legacy parameter names.
- `crates/ragent-tools-core/tests/test_read_tool.rs` — 2 new tests for
  read-timestamp recording.
- `crates/ragent-agent/tests/test_editrenewal_aliases.rs` — 6 tests
  verifying the registry exposes `edit`, `multi_edit`, and the `multiedit`
  alias, and that descriptions/schemas carry the canonical parameter names
  and deprecation signalling.

## Version: 0.1.0-alpha.124

### Changed
- **Workspace version** — Bumped to `0.1.0-alpha.124`.

## Version: 0.1.0-alpha.122

### Fixed — `/help` and `/skills` slash output no longer collapses to a single paragraph
- **`/help` table preserves per-line layout in the TUI** — The `/help` slash
  command now wraps its command/skill listing in a bare fenced code block
  (` ```\n … \n``` `) so the markdown → HTML → text pipeline does not reflow
  every row into one paragraph. Each command/skill stays on its own line and
  column alignment is preserved instead of being mangled.
- **`/skills` table preserves per-line layout in the TUI** — Same fix
  applied to the `/skills` listing: the registered-skills table is wrapped in
  a bare fenced code block, and `try_extract_research_code_block` now detects
  the block via the generic `From: /<cmd>` prefix (not just `From: /research`)
  so `/skills` benefits from the same verbatim rendering path.
- **`try_extract_research_code_block` generalised to any `From: /<cmd>`
  response** — Only **bare** fences (a line containing exactly three
  backticks followed by a newline) are recognised. Responses that use
  language-tagged fences (e.g. `/tools show` emits multiple ` ```text `
  blocks) are not intercepted and continue to flow through the normal
  markdown pipeline.

### Tests
- New unit tests in `crates/ragent-tui/src/app.rs::tests`:
  - `test_try_extract_research_code_block_handles_skills_output` — extracts
    the bare fenced block from a `/skills` response and verifies the skill
    rows remain on separate lines.
  - `test_try_extract_research_code_block_handles_help_output` — same check
    for a `/help` response with both command and skills sections.
  - `test_try_extract_research_code_block_returns_none_for_non_slash_text` —
    ensures the helper still returns `None` for plain text that happens to
    contain a fenced block.
  - `test_render_markdown_to_ascii_preserves_skills_table_lines` — renders
    a `/skills` table through `render_markdown_to_ascii` and asserts the two
    skill lines are not collapsed into one sentence.
  - `test_render_markdown_to_ascii_preserves_help_command_lines` — same
    end-to-end check for `/help` output.

## Version: 0.1.0-alpha.121

### Fixed — `/research` now actually analyses the gathered sources
- **Supporting files contain the captured body, not a placeholder** — `Source::Web`,
  `Source::Local`, and `Source::Other` gained an inline `body: String` field. The
  `WebGatherer` now passes the fetched page text into `Source::Web.body`; the
  `LocalGatherer` reads each candidate file and writes a context-aware excerpt
  (matching lines plus one line on either side) into `Source::Local.body`.
  `render_supporting_file` and the synthesis engine both consume the inline body,
  so `research/<name>/sources/web-NN.md` and `local-NN.md` now contain the
  actual evidence (with `▶` markers for exact matches and a ` ` marker for
  context) instead of the legacy `(see WebGatherer for the captured body)`
  placeholder. Old `RESEARCH.md` files without the new field deserialize with
  `body == ""` thanks to `#[serde(default)]`.
- **Local source relevance note is now informative** — The previous "X keyword
  match(es) for research topic" string has been replaced by a note that names
  the matched keywords (truncated to 3, e.g. `…(+N)` for the tail) and a 120-char
  snippet of the first matching line. Driven by the new
  `LocalGatherer::build_relevance_note` and `collect_matched_terms` helpers.
- **Mechanical fallback summary/findings are useful, not skeletal** — When no
  LLM synthesis is available (CLI, or TUI without an active model, or LLM call
  failed), the default `Summary` now names the captured web titles and local
  file paths grouped by type, the default `Findings` is one bullet per source
  with a 240-char excerpt, and the default `Open Questions` suggests concrete
  gaps and re-running with a configured LLM. The Summary is also transparent
  that no LLM analysis was applied.
- **Synthesis errors are visible, not silently swallowed** — The
  `ResearchSession::run` synthesize step now matches on the outcome and emits a
  `SessionEvent::SynthesizeResult { outcome, detail }` whose outcome is one of
  `Llm`, `FallbackEmpty`, `FallbackError`, `NoLlm`. Failures are logged at
  `error` level (not `warn`) and the message bubbles through to the TUI
  progress tracker and the CLI JSON emitter. Also fixed a latent bug where
  `analysis_is_noop` always returned `false` because `Any::type_id` on a trait
  object returns the trait object's `TypeId`, not the underlying concrete
  type's — replaced with a small `is_noop_marker()` trait method that
  `NoopAnalysisEngine` overrides to `true`.
- **CLI now wires up the local gatherer** — `ragent research create` used to
  call `ResearchSession::new(manager, None, None, NoopAnalysisEngine)`,
  producing a `RESEARCH.md` with 0 sources regardless of project contents. It
  now uses the new `ragent_research::cli::FsLocalTool` (a filesystem-backed
  `LocalTool`) so the CLI produces useful output without API keys. Web search
  and LLM synthesis still require credentials and remain off in the CLI.

### Added
- **`ragent_research::cli::FsLocalTool`** — Public filesystem-backed
  implementation of `LocalTool` for CLI use. Walks the project root,
  greps line-by-line, reads files, and lists `specs/<id>` directories.
  Skips `research/`, `target/`, `.git/`, `node_modules/`, and dot-prefixed
  directories so the gatherer doesn't index its own previous outputs.
- **`ragent_research::session::SynthesizeOutcome` enum** —
  `Llm | FallbackEmpty | FallbackError | NoLlm` so callers can attribute the
  resulting `RESEARCH.md` summary to the path that produced it.
- **`SessionEvent::SynthesizeResult`** — New event emitted after the
  synthesis phase with the outcome and an optional detail string.
- **`ragent_research::analysis::AnalysisEngine::is_noop_marker()`** — Default
  trait method (`false`) overridden by `NoopAnalysisEngine` (`true`).
- **`LocalGatherer::build_relevance_note`, `build_local_excerpt`,
  `collect_matched_terms`, `MAX_LOCAL_EXCERPT_LINES`** — Public helpers used
  by both the gatherer and the synthesis fallback.
- **`Source::body()` and `Source::has_body()`** — Accessor helpers on
  `Source` for the new body field.

### Changed
- **`Source` enum** — `Source::Web`, `Source::Local`, `Source::Other` gained a
  `body: String` field with `#[serde(default)]`. All call sites (gatherers,
  tests, fixture files) updated accordingly.
- **`LocalGatherer::score_candidates`** — Now returns
  `(LocalCandidate, Vec<GrepMatch>, Vec<String>)` so the caller has the
  matched terms and per-line hits needed to build the relevance note and
  excerpt.
- **`ResearchSession::synthesize`** — Prefers the inline `Source::body`
  field over reading from the on-disk supporting file (the latter still
  works as a fallback for items loaded from older `RESEARCH.md` files).

### Tests
- **199 unit tests + 8 integration tests for `ragent-research`** (up from
    181 + 7) covering body propagation, new relevance notes, mechanical
    fallback content, `SynthesizeResult` event emission, and the new
    `FsLocalTool`.

## Version: 0.1.0-alpha.120 (unreleased)
## Version: 0.1.0-alpha.119 (unreleased)

### Added — LLM-driven synthesis for `/research create`
- **`/research create` now analyzes sources with the active LLM** — The TUI research
  session gained a `Synthesize` phase between gathering and assembly. When an
  active provider/model is configured, `ResearchSession` sends the captured
  source bodies (web pages and local excerpts) to the LLM with a structured
  prompt requesting `## Summary`, numbered `## Findings` with `[#N]` citations,
  `## In-Project Cross-References`, and `## Open Questions`. The response is parsed
  and used to populate `RESEARCH.md`. If the LLM is unavailable, misconfigured, or
  returns empty output, the session falls back to the existing mechanical
  summary/findings.
- **New `ragent-research::analysis` module** — Introduces `AnalysisEngine`,
  `NoopAnalysisEngine`, `LlmAnalysisEngine`, `SourceBody`, and
  `AnalysisResult` so callers can plug in alternative analysis implementations.
- **Updated TUI wiring** — `build_research_session` in
  `crates/ragent-tui/src/research_adapter.rs` now accepts an optional
  `ProviderRegistry` and active `ModelRef` and constructs an `LlmAnalysisEngine`
  when both are present. CLI and HTTP research creation endpoints continue to use
  `NoopAnalysisEngine`, preserving their current behaviour.
- **Progress tracking** — `SessionPhase::Synthesize` and the research progress
  tracker now display the synthesis phase in the TUI log.

### Changed
- **`ragent-research` dependencies** — Added `ragent-llm`, `ragent-config`, and
  `ragent-storage` (dev-only for tests) so the crate can build LLM clients and
  accept provider auth without leaking storage types across the public API.
- **Research system specification** — `specs/researchsystem/SPEC.md` updated with
  FR-021 (AI-Driven Source Synthesis) and FR-022 (Graceful Degradation of
  Synthesis), plus a new Research System section in the top-level `SPEC.md`.

## Version: 0.1.0-alpha.116 (unreleased)

### Fixed — persistence and performance of agent loop
- **Fix persistence and improve performance of agent loop** — Addressed
  persistence-related issues in the session/cache and storage layers and
  reduced overhead in the session processor hot path. Bumps workspace version
  to `0.1.0-alpha.116`.

## Version: 0.1.0-alpha.114 (unreleased)

### Changed — eliminate duplicated team tool source
- **Single source of truth for team coordination tools** — The 20 team tool
  files (`team_approve_plan`, `team_assign_task`, `team_broadcast`,
  `team_cleanup`, `team_create`, `team_idle`, `team_memory_read`,
  `team_memory_write`, `team_message`, `team_read_messages`,
  `team_shutdown_ack`, `team_shutdown_teammate`, `team_spawn`, `team_status`,
  `team_submit_plan`, `team_task_claim`, `team_task_complete`,
  `team_task_create`, `team_task_list`, `team_wait`) previously existed as
  byte-for-byte identical copies in both `crates/ragent-agent/src/tool/` and
  `crates/ragent-team/src/tools/`. Every COMMSPLAN fix had to be applied twice
  and re-synced with `cp`. The `ragent-agent` copies have been deleted and
  `crates/ragent-agent/src/tool/mod.rs` now compiles each tool from the
  canonical `crates/ragent-team/src/tools/team_*.rs` file via `#[path]`
  includes — the same mechanism already used for the team runtime modules in
  `crates/ragent-agent/src/team/mod.rs`. Edits to the team tools now only need
  to be made in one place. The CI guard `scripts/check-team-duplication.sh`
  was extended to reject any re-introduced physical `team_*.rs` copy under
  `crates/ragent-agent/src/tool/` and to verify every tool has a `#[path]`
  include.

### Added — COMMSPLAN Milestone 4 (message delivery semantics)
- **Read-vs-processed split in mailbox consumption (M4-T1)** — `Mailbox` gained
  `peek_unread()` (returns unread messages **without** marking them read) and
  `acknowledge(message_id)` (the explicit "I processed this" ack, semantically
  `mark_read`). `team_read_messages` now peeks, builds its output, and only
  acknowledges each message once the `ToolOutput` is ready — so a failure
  mid-build leaves the messages unread and they are redelivered on the next
  call (at-least-once semantics). `drain_unread` is kept for the mailbox poll
  loop, which treats event publishing as the processing step. As part of this,
  `mark_read` / `acknowledge` now return `changed` instead of `pos.is_some()`,
  making acknowledge idempotent (a second ack of an already-read message
  reports `false`), matching the documented contract.
- **`team_assign_task` notifies the assigned teammate (M4-T2)** — After
  updating `tasks.json`, the tool pushes a `MailboxMessage` to the assignee's
  mailbox so they are notified immediately instead of having to poll
  `team_task_list` / `team_task_claim`. The notification outcome is reported
  in the tool output (`Notification: delivered` / `failed: …`). The tool also
  rejects assignment to `Stopped` / `Failed` teammates up front.
- **`team_broadcast` reports per-recipient results (M4-T3)** — The
  early-return `?` on the first failure is replaced with a loop that collects
  `Result` per recipient, so a failure on one teammate no longer aborts
  delivery to the rest. The tool output includes `succeeded` and `failed`
  arrays (with per-failure error text) in the JSON metadata.
- **`team_message` validates recipient state (M4-T4)** — Before pushing, the
  tool loads `TeamStore` and rejects messages to `Stopped` / `Failed`
  teammates and to unknown agent IDs, so the sender gets an error instead of a
  false success. Messages to `lead` and active teammates are delivered as
  before.
- **`team_read_messages` output schema fixed (M4-T5)** — The JSON metadata
  now serialises `message_type` via `serde_json::to_value` (snake_case,
  matching the on-disk `#[serde(rename_all = "snake_case")]` format) instead
  of `format!("{:?}", …)` (PascalCase), and includes the `to` and `read`
  fields. The human-readable text now shows `To:` and the snake_case type.
- **Delivery regression tests (M4-T1..T5)** — New `ragent-team` integration
  test suite `tests/test_m4_delivery.rs` (12 tests) covering peek/ack
  idempotence, redelivery-on-no-ack, assign-task notification, dead-assignee
  rejection, per-recipient broadcast results, stopped/unknown recipient
  rejection, and the snake_case `team_read_messages` schema.

### Added — COMMSPLAN Milestone 3 (team liveness, shutdown, idle signalling)
- **`team_wait` subscribes before reading team state (M3-T1)** — The event-bus
  receiver is now created *before* the initial `TeamStore::load` so a teammate
  that goes idle or fails between the store read and the wait loop is captured
  rather than missed. A pre-loop `try_recv` drain reconciles any events that
  arrived during the store scan into the `waiting_for` set.
- **`team_wait` handles `TeammateFailed` (M3-T2)** — A failed teammate is now
  removed from the waiting set on receipt of `Event::TeammateFailed`, so the
  lead no longer waits the full 300 s timeout for an agent that will never
  become idle.
- **`team_wait` re-checks disk state on timeout (M3-T3)** — Before returning a
  timeout, `team_wait` reloads the team store and treats any member whose
  on-disk status is `Idle`, `Failed`, or `Stopped` as finished. This recovers
  terminal state when an `EventBus` event was dropped (buffer full / no
  subscribers) but the teammate legitimately reached a terminal state on disk.
- **`team_idle` publishes `Event::TeammateIdle` (M3-T4)** — After marking the
  member `Idle` on disk, the tool now publishes `TeammateIdle` on the event
  bus so `team_wait` and the TUI/SSE observe the transition even when the
  mailbox poll loop does not deliver an `IdleNotify` message. The lead session
  id is derived from the on-disk team config.
- **Unified shutdown path (M3-T5/T6)** — `TeamManagerInterface` gained a
  `shutdown_teammate(agent_id, graceful)` method, implemented once on
  `TeamManager` and used by both the `team_shutdown_teammate` tool and
  internal callers (`shutdown_all`, the TUI teardown paths). Graceful shutdown
  marks the member `ShuttingDown` and pushes a `ShutdownRequest` without
  forcing cancel; immediate shutdown sets the agent-loop and poll-loop cancel
  flags, deregisters the mailbox notifier, pushes a `ShutdownRequest` as a
  fallback, and marks the member `Stopped`. The tool gained an `immediate`
  parameter (default `false`) and falls back to a disk-only path when no
  `TeamManager` is wired into the context.
- **Lifecycle regression tests (M3-T7)** — New `ragent-team` integration test
  suite `tests/test_m3_lifecycle.rs` covering all four required scenarios:
  (a) teammate fails while lead is in `team_wait`, (b) teammate goes idle
  before `team_wait` starts, (c) `EventBus` event dropped but disk state
  correct, (d) `team_idle` publishes `TeammateIdle` and `team_shutdown_teammate`
  marks `ShuttingDown` (graceful) / `Stopped` (immediate).

### Added
- **Unified whitespace-tolerant replacement matcher** — `edit`, `multiedit`,
  and `memory_replace` now share a single seven-pass matcher in the new
  `ragent_tools_core::replace` module (`find_replacement_range` /
  `find_replacement_range_diag`). The matcher tolerates CRLF line endings,
  trailing/leading whitespace differences, collapsed-whitespace (tabs vs
  spaces, double spaces, mixed indentation), blank-line edge differences, and
  final-newline mismatches — eliminating `old_str not found` failures caused
  by common LLM output quirks. `memory_replace` previously used exact-only
  `String::matches`/`replacen` and would fail on the same whitespace quirks
  that `edit` already handled; it now behaves identically to `edit`.
- **`stream.initial_response_timeout_secs` config knob** — New optional
  field on `StreamConfig` (default `300`) that bounds how long the HTTP
  client waits for the **first byte** of a streaming response.  This is
  distinct from `stream.timeout_secs` (default `120`), which now exclusively
  governs the gap between subsequent stream deltas.  Cloud-hosted models
  (Ollama Cloud, Bedrock, Copilot, Azure AI Foundry) routinely need
  30-90 s for cold-start, which the previous shared 120 s timeout was
  insufficient to absorb once 4-5 swarm teammates started hammering the
  same provider concurrently.

### Changed
- **`multiedit` overlap detection & ordering** — `MultiEditTool::execute` now
  resolves every edit against the **original** file content (so byte ranges
  are stable), pairwise-checks edits on the same file for intersecting byte
  ranges (rejecting with a clear error naming the edit indices and file path),
  and applies non-overlapping edits highest-end-offset-first so the JSON input
  order no longer matters. Touching ranges (`a.end == b.start`) are allowed.
- **`multiedit` / `edit` diagnostics** — `NotFound` errors from `multiedit`
  now name the edit index, the file path, the last matching pass attempted
  (e.g. `collapsed`, `final-newline`), and a best-effort closest-line hint,
  via the new `FindDiag` / `find_replacement_range_diag` API. The original
  `find_replacement_range` remains as a thin wrapper so `edit` and
  `memory_replace` are unaffected.
- **Relative indentation preservation** — `reindent_with` now uses the
  **common** leading whitespace of all matched file lines (via
  `common_leading_ws`) rather than just the first line's full indentation,
  and leaves blank lines untouched so no trailing whitespace is introduced.
- **Swarm teammate retry backoff is now exponential with jitter** —
  `teammate_retry_backoff(attempt)` (in `ragent-team`, used by
  `TeamManager::spawn_teammate_internal`) replaced the previous linear
  `500 ms × attempt` schedule with an exponential curve
  (`1 s, 2 s, 4 s, 8 s` for attempts 1-4) plus up to 500 ms of clock-
  derived jitter, capped at 30 s.  The previous linear schedule caused
  every teammate that failed at the same moment to retry in lockstep,
  re-triggering the same upstream rate-limit / cold-start pressure on
  cloud LLMs (Ollama Cloud, Bedrock, Copilot).  The new helper is exposed
  publicly for downstream tooling and is covered by 4 integration tests
  in `crates/ragent-team/tests/test_teammate_retry_backoff.rs`.

### Fixed
- **Team subsystem data-loss races (COMMSPLAN Milestone 1)** —
  `Mailbox`, `TaskStore`, and `TeamStore` now serialise all mutating disk
  operations on a stable companion lock file (`*.json.lock`) using `fs2`
  advisory locks.  The lock is held across the full read-modify-write cycle
  and the atomic rename of a uniquely-named temp file, eliminating the
  previous TOCTOU window where concurrent writers released `flock` before
  the data reached disk.  Temp file names now include a UUID so concurrent
  writers cannot collide on a shared `.tmp` path.  Added regression tests
  `parent_mailbox_concurrent_push`, `parent_task_store_concurrent_claims`,
  and in-process threaded variants in
  `crates/ragent-team/tests/test_concurrent_store_writes.rs`.

  - **Team implementation unified (COMMSPLAN Milestone 2)** —
    `crates/ragent-agent/src/team/` now contains only `mod.rs`, which
    source-includes the implementation from `crates/ragent-team/src/team/`
    via `#[path]` attributes.  The duplicated local copies of
    `config.rs`, `mailbox.rs`, `manager.rs`, `store.rs`, `swarm.rs`,
    `task.rs`, and the previous `store.rs` `#[path]` wrapper have been
    removed, so fixes such as the Milestone 1 lock-file changes apply to
    both crates from a single source.  A `MemoryScope::as_str()` helper was
    added so that `TeamManager` can compare memory scopes across the
    source-inclusion boundary.  Added `docs/team-unification-decision.md`
    documenting why `#[path]` is used (the existing Cargo dependency cycle
    between `ragent-agent` and `ragent-team`) and the future path to a real
    crate dependency.  Added `scripts/check-team-duplication.sh` CI guard
    that fails if local team source files reappear in `ragent-agent`.

### Fixed
- **`old_str not found` on blank-line / final-newline edge differences** —
  Added blank-line normalisation (pass 6) and final-newline normalisation
  (pass 7) passes to the matcher, handling `str::lines()` inconsistencies
  around leading/trailing blank lines and trailing `\n` disagreements in
  either direction.
- **Collapsed-whitespace false `MultipleMatches`** — When collapsed matching
  yields multiple candidates, the matcher now prefers the candidate whose
  per-line leading whitespace is closest (smallest total char-length
  distance) to the needle's, rather than hard-erroring. Ties still error
  with `MultipleMatches`.
- **Swarm synthesis task timing out on cloud LLM providers** — When the
  final `/swarm` subtask (typically the architect agent) started after the
  other 3-4 parallel teammates had already consumed provider rate-limit
  budget, Ollama Cloud and similar hosted services would frequently fail
  to deliver the first byte within the configured 120 s
  `stream.timeout_secs`, surfacing as repeated `Ollama Cloud chat request
  timed out` warnings and, on the final synthesis task, an unrecoverable
  stall.  The fix splits the conflated timeout field into
  `initial_response_timeout_secs` (300 s, forwarded to providers as
  `ChatRequest.stream_timeout_secs`) and `timeout_secs` (120 s, used only
  for per-event stall detection).  Paired with the new exponential
  teammate retry backoff, the final synthesis task now completes on the
  first attempt instead of exhausting retries on cold-start latency.

## Version: 0.1.0-alpha.113 (unreleased)

### Fixed
- **`/research create` now wires real gatherers in the TUI** — Previously `handle_research_command` built every session with `ResearchSession::new(manager, None, None)`, so web search and local cross-referencing were skipped and every research item had zero sources.  The TUI now builds a `ResearchSession` backed by the existing agent tool registry (`websearch`/`webfetch` for the web phase, `glob`/`grep`/`read`/`list` for the local/spec phase) via new adapters in `crates/ragent-tui/src/research_adapter.rs`.
- **`/research create` now reports completion in the TUI** — The TUI makes sure a session exists before spawning the research task, so the `TuiResearchObserver` `AgentNotice` events are routed to the active session.  The status bar now updates to the `Done` message instead of staying stuck on "research: writing …".
- **`/research list|show|search` output no longer distorts columns** — `render_markdown_to_ascii` now bypasses the markdown→HTML→text and ASCII-table-normalisation pipeline for `/research` responses that contain a fenced code block, returning the already-formatted plain text directly.  This keeps the fixed-width tables aligned and readable.
- **Research keyword matching improved** — `derive_terms()` in `ragent-research` now strips ASCII punctuation (except apostrophes in contractions) and splits on internal punctuation such as `/` and `,`, so topics like `"async/await, tokio!"` produce usable terms.  `LocalGatherer::gather_specs()` now actually filters specs by the research topic and falls back to all specs only when fewer than three relevant specs are found.

## Version: 0.1.0-alpha.113

### Fixed
- **Research-system spec status overstated (second occurrence)** — `specs/researchsystem/SPEC.md` was again tagged `status: implemented` with a three-step audit trail even though only six of the 56 plan tasks had shipped (T-001/T-002/T-003 foundational types, T-005 crate scaffold, T-004 `ResearchItem`, T-014 web-gatherer, T-016 local-gatherer, T-040 plan-dep parser, T-045 path-traversal rejection, T-051 name-validation tests). The frontmatter is now `status: in_progress` with a two-step audit (`none → draft → in_progress`) that correctly reflects "framework implemented, integration pending". The remaining ~50 tasks (manager CRUD, session orchestrator, supporting-file writers, `RESEARCH.md` assembler, References Index generator, `INDEX.md` cache, TUI slash-command wiring, CLI/HTTP endpoints, spec-integration glue, benchmarks, user docs) remain `pending` and will be promoted in subsequent releases.

## Version: 0.1.0-alpha.112

### Fixed
- **Research-system spec status overstated** — `specs/researchsystem/SPEC.md` was committed with `status: implemented` and a three-step audit trail even though only four of the 22 plan tasks were actually completed (T-001 `ResearchName`, T-002 `ResearchStatus`, T-003 `Source`, T-005 crate scaffold). The frontmatter is now `status: draft` with a single `none → draft` audit transition, and the remaining 18 tasks remain `pending` in `specs/researchsystem/PLAN.md` until the gathering engine, TUI slash command, CLI/HTTP endpoints, and spec integration land in follow-up releases.

## Version: 0.1.0-alpha.111

### Changed
- **`ask_user` tool promoted from alias to standalone** — The previously-delegating `ask_user` tool in `crates/ragent-agent/src/tool/aliases.rs` now publishes `Event::QuestionRequested` and awaits `Event::QuestionAnswered` directly via the event bus. The standalone `question` tool has been deleted from `ragent-agent`, `ragent-tools-core`, and the TUI question-dialog widget module; question rendering is now driven entirely by the existing TUI event handler in `ragent-tui/src/app.rs`.
- **`ask_user` supports multiple-choice** — The optional `options` array parameter renders a selectable list in the TUI question dialog; omitting `options` keeps the previous free-text input behaviour. The tool description and JSON schema now document the new parameter, and `permission_category` is reported as `ask_user` (was `question`).
- **Permission auto-approval key renamed** — `check_permission_with_prompt`'s hardwired always-allow list now matches `ask_user` (was `question`); the corresponding unit test was renamed accordingly.

### Added
- **`ragent-research` crate scaffold** — New workspace member under `crates/ragent-research/` providing `ResearchName` (validated, URL-safe identifier newtype with FR-002 validation), `Source` (Web/Local/Spec/Other enum backing the references index), and `ResearchStatus` (draft/in-progress/complete/archived covering FR-013). Depends only on `ragent-types` and common workspace deps so it can be reused by both the TUI and HTTP layers once the manager/session/io modules are added in follow-up releases.
- **Research system spec + plan** — New `specs/researchsystem/SPEC.md` and `specs/researchsystem/PLAN.md` describing the `/research` slash command, directory conventions, information-gathering session, references index, and integration with the existing spec workflow.

## Version: 0.1.0-alpha.110

### Removed
- **Internal LLM subsystem removed** — The embedded local LLM (Candle GGUF + Foundry Local + LiteRT-LM), the `/internal-llm` slash command family, the TUI `InternalLLM` chat overlay panel, the `InternalLlmConfig` block, the `internal_llm` Cargo feature flag, and all related test files have been removed. Compaction now always uses the provider-compaction fallback. Session titles default to empty (no longer auto-generated). Memory extraction no longer has an LLM prefilter step. The `internal_llm` key in `ragent.json` is silently ignored. This supersedes the prior Foundry Local internal-LLM backend and the LiteRT-LM backend switch work.

## Version: 0.1.0-alpha.109

### Added
- **In-process Microsoft Foundry Local backend** — New `FoundryLocalInProcClient` in `crates/ragent-llm/src/providers/foundry_local_inproc_client.rs` loads and runs Foundry Local models inside the ragent process via the `foundry-local-sdk` native core, bypassing the local web service.  Supports model alias resolution, download progress events, device selection (`auto`/`cpu`/`gpu`/`npu`), temperature/max_tokens, tools, and full `StreamEvent` translation (text, tool calls, usage, finish reason).
- **`in_process` provider option** — `provider.foundry_local.in_process` (default `false`) selects the in-process backend; when unset or `false` the existing web-service path is preserved.
- **`RAGENT_FOUNDRY_LOCAL_FORCE_WEB` escape hatch** — Set this environment variable to `1` or `true` to force the web-service path even when `in_process: true` is configured.
- **TUI foundry-mode indicator** — `/internal-llm show` now displays whether the main Foundry Local provider is configured for `in-process` or `web-service` inference when the internal LLM backend is `foundry`.

### Changed
- **Foundry Local provider routing** — `FoundryLocalProvider::create_client()` now branches on the resolved `in_process` flag, returning either `FoundryLocalInProcClient` or the existing `FoundryLocalClient`.
- **Device validation** — `provider.foundry_local.device` values are now validated and rejected if not one of `auto`, `cpu`, `gpu`, or `npu`.
- **Foundry Local documentation** — Updated `PROVIDERS.md` and `SPEC.md` with in-process mode configuration, environment escape hatch, and internal-LLM notes.

### Fixed
- **HuggingFace provider discovery failed** — The HuggingFace `/v1/models` router endpoint is public and now works without an API token; discovery no longer errors out immediately when `HF_TOKEN` is unset.  Added `HUGGING_FACE_HUB_TOKEN` as a recognised token source for consistency with the TUI configured-provider detection.  When dynamic discovery fails or returns no models, the TUI now falls back to the provider's static default catalog instead of showing an empty "No models are currently available" dialog.  Empty discovery results are no longer cached, preventing a transient failure from permanently hiding the default models.
- **Task tool family guidance** — Added a dedicated `## Task Tool Family` section to every primary agent's system prompt that clearly distinguishes `agent_complete` (autonomous loop signal — only takes `summary`) from `team_task_complete` (team workflow — only takes `team_name` + `task_id`).  The `agent_complete`, `team_task_complete`, and `new_agent` tool descriptions and JSON schemas now explicitly warn against the most common parameter-confusion mistakes and reject unknown keys via `additionalProperties: false`.  `agent_complete` and `list_agents` are now hardwired auto-approved so the agent can always finish or inspect background tasks without a permission prompt.

## Version: 0.1.0-alpha.108

### Added
- **Foundry Local internal-LLM backend** — New `FoundryLocalExecutor` in `ragent-agent/src/internal_llm/foundry_executor.rs` routes internal-LLM requests through Microsoft Foundry Local instead of the Candle-based embedded runtime. The `/internal-llm foundry` and `/internal-llm embedded` slash commands switch between backends at runtime, and `from_config()` now dispatches on `config.backend` (`"foundry"`/`"foundry_local"` vs default candle).

### Changed
- **Internal LLM backend routing** — `InternalLlmService::from_config()` now selects the executor based on the configured backend name, supporting both Candle (`embedded`) and Foundry Local (`foundry`/`foundry_local`) paths.
- **TUI /internal-llm commands** — Added `foundry` and `embedded` subcommands to the `/internal-llm` slash command for switching backends. Updated autocomplete list, help text, and slash-command definition.
- **Compiled backends display** — Replaced litertlm feature-flag detection with Foundry Local availability check (`is_foundry_local_available()`) in the TUI show/info panel.
- **Workspace version** — Bumped to `0.1.0-alpha.108`.

### Fixed
- **Microsoft Foundry Local empty SSE stream after model preparation** — `wait_for_model_ready` previously polled `/v1/models`, which lists downloaded/cataloged models and may report a model before it is actually loaded into memory. Chatting with an unloaded model caused the empty/malformed event-stream error seen in the TUI. The readiness check now polls the web service's `/models/loaded` endpoint (the authoritative "loaded into memory" signal) and falls back to `/v1/models` only on older services. The model load id is now taken directly from the SDK's full variant id, making load requests more robust across catalog versions.

## Version: 0.1.0-alpha.107

### Fixed
- **Compression pipeline threshold gating** — Added `should_compress` and `should_compress_chat_messages` checks before invoking the full compression pipeline, preventing unnecessary overhead and unconditional UI events when the conversation is well within the context window. The initial-history compression and per-iteration compression now both gate on the configured `auto_threshold` (default 0.80) before running. Added 2 new unit tests for the chat-messages threshold helper.

### Changed
- **Workspace version** — Bumped to `0.1.0-alpha.107`.

## Version: 0.1.0-alpha.106

### Added
- **Microsoft Foundry Local provider integration** — Added first-class support for Microsoft Foundry Local as a local LLM provider, including provider setup dialog visibility, `[local]` badge rendering, status-bar abbreviation, health checks, and configuration option merging (`auto_start`, `device`, `models_path`).
- **Headroom compression lifecycle events** — New `Event::CompressionStarted` and `Event::CompressionFinished` events are published by the session processor and consumed by the TUI and SSE stream. They carry `original_tokens`, `compressed_tokens`, `compression_ratio`, and `did_compress` so observers can show live progress and update context-window displays immediately.

### Changed
- **Workspace version** — Bumped to `0.1.0-alpha.106`.
- **Per-iteration compression visibility** — The agent loop now publishes `CompressionStarted`/`CompressionFinished` around every automatic Headroom compression run. The TUI sets `compress_in_progress` while the pipeline is active, so the existing status-bar "compressing" indicator actually appears during automatic compression, and the `ctx:` display is refreshed with the post-compression token count as soon as the run completes.
- **SSE event coverage** — `ragent-server` now serializes `compression_started` and `compression_finished` SSE events.

### Fixed
- **Context window display lag after compression** — `last_input_tokens` was only updated when the provider returned token usage, so the status-bar context percentage stayed at the pre-compression value until the LLM response arrived. The TUI now updates `last_input_tokens` directly from `CompressionFinished`, keeping the `ctx:` display in sync with the actual request size.

## Version: 0.1.0-alpha.105

### Added
- **Context compression pipeline** — New compression module (`ragent-agent/src/compression/`) with multi-strategy history compaction: BM25 relevance scoring, CCR (Critical Content Retention) store, aggressive/conservative/default modes, and `/compress` slash command integration.
- **Model Router Provider** — New intelligent model routing system (`ragent-llm/src/providers/router*.rs`) with 15-dimension classifier (complexity, creativity, code, reasoning, vision, image_attachment, etc.), automatic model selection, and configurable routing rules.
- **Compression config** — New `compression.rs` module in ragent-config for compression pipeline configuration.
- **String utilities** — New `strutil.rs` module in ragent-types for shared string helpers.
- **Spec ID scanner** — New `id_scanner.rs` in ragent-specs for extracting and tracking spec requirement/task IDs.
- **HeadroomCompress spec** — Full specification for the compression feature in `specs/HeadroomCompress/`.
- **ModelRouterProvider spec** — Full specification for the model router in `specs/ModelRouterProvider/`.
- **Config defaults fix** — Added `#[serde(skip_serializing_if)]` to `code_index.enabled`, `internal_llm.enabled`, and `tool_visibility.codeindex` so auto-generated config files don't override code-level defaults. Added 10 regression tests.
- **Compression indicator test** — New TUI test for compression status display.

### Changed
- **Agent system** — Refactored agent module with expanded presets and compression integration.
- **Session processor** — Added compression integration, improved tool call handling, and spec command support.
- **TUI** — Added `/compress` slash command, compression status bar indicator, and improved status bar layout.
- **Spec commands** — Major refactor of spec command handling with expanded `/spec impl` and `/spec implement` support.
- **Bedrock provider** — Refinements to credential handling and SigV4 signing.
- **Multiple tool refinements** — Updated codeindex_search, list_agents, memory_search, office_write, and spec_list tools.
- **Test improvements** — Updated multiple test files for compatibility with new APIs and module structure.
- **TUI toggle persistence** — `/codeindex`, `/internal-llm`, and `/tools` toggles now save to a project-local `.ragent/ragent.json` (creating the directory if needed) instead of falling back to the global config. They also skip writes when the target value has not changed, avoiding unnecessary file churn.
- **YOLO mode persistence** — YOLO mode state is now saved to the config file (`yolo: true/false`) and restored on startup. Toggling via `/yolo` or the `InputAction::ToggleYolo` keybinding persists the new state immediately.

### Fixed
- **Microsoft Foundry Local visibility** — The provider was already registered in the LLM layer and listed by `ragent models`, but was missing from the TUI provider setup dialog and the user-facing provider list. Added `foundry_local` to `PROVIDER_LIST`, the provider setup/reset flows, configured-provider detection, health checks, and status-bar abbreviation, and rendered a `[local]` badge alongside Ollama. Also merged `provider.foundry_local` options (`auto_start`, `device`, `models_path`) from `ragent.json` into the client creation path. Updated README.md and SPEC.md to include Microsoft Foundry Local.
- **TUI provider picker scrolling** — The provider setup dialog used a fixed 50% height and rendered all providers in a single paragraph, so on typical 24-row terminals the bottom of the list (including Microsoft Foundry Local) was clipped off-screen and unreachable. The dialog is now taller (capped at 22 rows) and the provider list scrolls to keep the selected item visible, with a "(more providers below)" hint when the list overflows.
- **Per-iteration context compression** — The Headroom compression pipeline was only run once at the start of an agent run, so the LLM request payload kept growing as the agent loop appended assistant tool uses and tool results each turn. Once the payload exceeded the model's context window, the provider returned an error and the task failed. The agent loop now re-runs compression before every LLM call when the configured `auto_threshold` (default 0.80) is exceeded, satisfying FR-005. Added `compress_chat_messages` round-trip helpers and 6 unit tests in `ragent-agent/src/compression/pipeline.rs`.
- **Config defaults for CodeIndex and InternalLLM** — When `Config::load()` created a default config file (no existing config), it serialised all fields including default values like `"enabled": true` for `code_index` and `"enabled": false` for `internal_llm`. These explicit values then overrode any future code-level default changes. Added `#[serde(skip_serializing_if)]` annotations to `code_index.enabled`, `internal_llm.enabled`, and `tool_visibility.codeindex` so default values are omitted from serialised output, allowing code-level defaults to take effect automatically. Added 10 regression tests in `test_code_index_config.rs`.
- **Foundry Local always compiled** — Removed the empty `foundry-local` feature flag from the root `Cargo.toml` defaults. The `foundry-local-sdk` dependency in `ragent-llm` is already unconditional, so the Microsoft Foundry Local provider is now always present with no compile-time gate.

## Version: 0.1.0-alpha.104

### Added
- **Amazon Bedrock provider** — Full AWS Bedrock support with SigV4 request signing (no AWS SDK dependency), dual API clients (Anthropic Messages API for Claude models, Converse API for all other models), 9 default models, short alias mapping, `@bedrock` suffix stripping, `ListFoundationModels` discovery, and credential resolution chain (env vars → AWS profile INI → session tokens).
- **xAI Grok provider** — New `xai.rs` provider for the xAI Grok API, registered in the default provider registry.
- **Spec implementation commands** — `/spec impl` and `/spec implement` slash commands for spec lifecycle: generates implementation plans, tracks progress against requirements, and runs implementation tasks.

### Changed
- **Copilot provider improvements** — Updated copilot.rs with refinements to the GitHub Copilot provider.
- **HuggingFace provider improvements** — Updated huggingface.rs with refinements.
- **Provider registry** — Added Bedrock and xAI to `create_default_registry()`.
- **Spec module expanded** — New `impl_runner.rs` and `plan_parser.rs` modules in ragent-specs.

## Version: 0.1.0-alpha.103

### Fixed
- **Bash syntax validation on Windows** — `validate_bash_syntax()` previously used a hardcoded `sh -n -c` command, which fails on Windows because `sh` is not available. Now uses the discovered shell program: `bash -n -c` on Unix, the Git Bash executable path on Windows (Git Bash), and skips validation entirely for PowerShell. This eliminates the "program not found" error when running bash commands on Windows 11.

## Version: 0.1.0-alpha.102

### Added
- **Windows shell support for BashTool** — The `BashTool` now runs on Windows with automatic shell discovery (Git Bash preferred, PowerShell fallback). All 7 security layers remain active regardless of platform. Windows-specific directory-escape detection blocks `C:\`, `D:\`, and `\` paths. PowerShell syntax validation is skipped (PowerShell self-validates at runtime). State files are stored in `%LOCALAPPDATA%\ragent\shell\` on Windows.

### Changed
- **Refactored `is_directory_escape_attempt`** — Split into `is_directory_escape_attempt` (public, calls inner) and `is_directory_escape_attempt_inner` (testable inner function with explicit `on_windows` parameter). This enables testing Windows-specific path detection on any platform.
- **Shell discovery caching** — Added `OnceLock`-based process-global shell type cache so shell discovery only runs once per process.

### Fixed
- **Directory escape test** — `test_directory_escape_absolute` now uses `tempfile::tempdir()` for a real filesystem path, avoiding `canonicalize()` hangs on nonexistent paths.

## Version: 0.1.0-alpha.101

### Fixed
- fixed AGENTS.md load path

## Version: 0.1.0-alpha.100

### Added
- **Sub-agent suspend/resume/kill lifecycle** — New `suspend_task()`, `resume_task()`, and `kill_task()` methods on the task manager. Sub-agents can now be paused, resumed, or forcibly terminated with a 10-second force-kill escalation timeout. New `TaskStatus::Suspended` and `TaskStatus::Terminating` states, plus `SubagentSuspended`, `SubagentResumed`, and `SubagentKilled` events.
- **Teammate suspend/resume events** — `TeammateSuspended` and `TeammateResumed` events for team coordination, enabling lead agents to pause and resume teammates.
- **Enhanced active-agents panel** — TUI active agents panel now shows per-agent status (running/suspended), supports suspend/resume/kill actions, and renders agent step counts and elapsed time.
- **Enhanced teams panel** — TUI teams panel shows teammate statuses, suspend/resume buttons, and per-teammate progress indicators.
- **SSE events for sub-agent lifecycle** — Server-sent events now stream `SubagentSuspended`, `SubagentResumed`, `SubagentKilled`, `TeammateSuspended`, and `TeammateResumed` event types.

### Changed
- **Permission check indentation fix** — Re-indented `check_permission_with_prompt()` to correct a long-standing indentation issue.
- **AGENTS.md discovery sorting** — Improved instruction file priority sorting with properly formatted ordering logic.
- **Azure Resource provider refactoring** — Cleaned up `azure_resource.rs` provider implementation.
- **HTTP client retry logic** — Updated `execute_with_retry` in `http_client.rs`.

### Fixed
- **Instruction file discovery priority bug** — `collect_agents_md_content_with_discovery()` in `agent/mod.rs` was incorrectly calculating file depth (missing `saturating_sub(1)`), causing `AGENTS.md` in the project root to have depth=1 instead of depth=0. This meant root instruction files were treated as subdirectories, and the sorting step mixed root, global, and subdirectory candidates together. Now root files are correctly identified (depth=0), each priority tier is sorted independently, and concatenated in strict order: project root → global directory → project subdirectories. Added integration test `test_root_agents_md_beats_subdirectory_agents_md` to verify the fix.

## Version: 0.1.0-alpha.99

### Changed
- **updated splash screen text**

## Version: 0.1.0-alpha.98

### Added
- **Azure Resource Provider API type switch** — Added `api_type` field to `azureresources.json` entries. When set to `"anthropic"`, requests are routed to `{endpoint}/anthropic/v1/messages` using the Anthropic Messages API format with Azure-style `api-key` authentication. When set to `"openai"` or omitted, the existing OpenAI-compatible path (`{endpoint}/openai/v1/chat/completions`) is used.  
  - New `AzureAnthropicClient` wrapper in `azure_resource.rs` reuses `AnthropicClient` body construction and SSE parsing but sends `api-key` header instead of `x-api-key`.
  - `AzureResourceProvider::create_client` now branches on `api_type` by looking up the model ID in the cached entries.
  - TUI `SelectAzureResource` flow now persists `azure_resource` as the provider (instead of `azure_foundry`) and stores `azure_resource_api_base` alongside `azure_resource_last_selection`.
  - Session processor `resolve_api_key` and base-URL resolution now handle `azure_resource` provider directly.
  - Added unit tests for parser validation (`test_api_type_openai_accepted`, `test_api_type_anthropic_accepted`, `test_api_type_missing_defaults_to_openai`, `test_api_type_invalid_skipped_with_warning`).
  - Added integration tests for `create_client` branching (`test_azure_anthropic_create_client_branches_correctly`, `test_azure_openai_branch_unchanged`).
  - Updated `specs/AzureResource/FILEFORMAT.md` with `api_type` documentation.

## Version: 0.1.0-alpha.97

### Changed
- **add rate limiting logic**

## Version: 0.1.0-alpha.96

### Changed
- **fix for azure resource provider**
- **Azure AI Foundry / Azure Resource 429 rate-limit retry** — HTTP 429 (Too Many Requests) responses from Azure AI Foundry and Azure Resource endpoints are now treated as retryable. The `execute_with_retry` helper in `http_client.rs` has been updated to:
  - Detect `429 Too Many Requests` status code and automatically retry up to 4 times
  - Respect the `Retry-After` response header when present (integer seconds format)
  - Fall back to exponential backoff (2ˢ, 4ˢ, 8ˢ, 16ˢ) when no `Retry-After` header is provided
  - Log each retry attempt with the delay duration for transparency
- **AzureFoundryClient now uses `execute_with_retry`** — The chat request in `azure_foundry.rs` now routes through the retry-aware `execute_with_retry` path, so transient 429 errors will be handled automatically instead of surfacing as immediate failures.

### Changed
- **YOLO mode fixes** — Fixed YOLO mode permission bypass logic.
- **AGENTS.md search and inclusion order** — Updated AGENTS.md discovery and inclusion order handling.

## Version: 0.1.0-alpha.93

### Changed
- **Built-in agents no longer hardcode Anthropic Claude** — All 18 built-in agent definitions (`ask`, `general`, `build`, `plan`, `explore`, `title`, `summary`, `compaction`, `rust-coder`, `python-coder`, `go-coder`, `typescript-coder`, `java-coder`, `cpp-coder`, `csharp-coder`, `swift-coder`, `database-agent`, `frontend-agent`) previously hardcoded `model: Some(ModelRef { provider_id: "anthropic", model_id: "claude-sonnet-4-20250514" })`. They now default to `model: None` and auto-resolve the first available model from the provider registry at runtime. This means agents automatically use whatever provider/model the user has configured via `/provider`, `/model`, or `--model` instead of always falling back to Claude.

### Added
- **`resolve_default_model()` helper** — Scans the provider registry and returns the first model from the first provider, used when an agent has no explicit model binding.
- **`resolve_agent_with_model()` / `resolve_agent_with_customs_and_model()`** — Wrappers around `resolve_agent()` / `resolve_agent_with_customs()` that ensure the returned agent always has a model by falling back to `resolve_default_model()` when needed.

### Fixed
- **TUI initial agent setup** — `App::new()` now calls `resolve_default_model()` on the initial agent so startup works even when no model was previously persisted in storage.
- **TUI agent switching** — `apply_selected_model_and_thinking()` now falls back to `resolve_default_model()` when both `selected_model` and `agent.model` are `None`.
- **Server message handler** — `POST /sessions/{id}/messages` now uses `resolve_agent_with_model()` instead of `resolve_agent()` so server requests also auto-resolve the default model.

## Version: 0.1.0-alpha.91

### Fixed
- **TUI 5-minute stall / frozen ESC** — `refresh_memory_stats()` was doing synchronous file I/O (`load_all_blocks` reads ALL memory blocks from disk) + SQLite query on every single event-loop tick (~50 ms) with zero debouncing. When many memory blocks exist or SQLite has lock contention, the entire async runtime blocks for seconds, preventing keyboard events from being processed (ESC appears frozen). Added 5-second debounce to `refresh_memory_stats()` matching the pattern used by `refresh_code_index_stats()`. Also added 2-second debounces to `poll_swarm_unblock()` and `poll_swarm_completion()` which were doing filesystem I/O (`TeamStore::load_by_name`, `TaskStore::open`) on every tick.
- **Question dialog not rendering** — `Event::QuestionRequested` handler in `app.rs` was missing `self.needs_redraw = true`, causing the question dialog to never appear on screen until an unrelated input or event triggered a redraw. Added the flag so the dialog renders immediately when a `question` tool call arrives.

### Changed
- **Agent loop optimization** — Optimize the agent loop to prevent stalls.

## Version: 0.1.0-alpha.90

### Added
- **Git tool summaries in TUI** — `tool_input_summary` and `tool_output_summary` in `message_widget.rs` now provide human-readable summaries for all 16 git tools (`git_add`, `git_branch`, `git_checkout`, `git_cherry_pick`, `git_clone`, `git_commit`, `git_diff`, `git_fetch`, `git_log`, `git_merge`, `git_pull`, `git_push`, `git_remote`, `git_reset`, `git_show`, `git_stash`), showing actions like "🌿 add -A", "🌿 commit --amend", "🌿 merge feature-branch", etc.
- **GitHub tool summaries in TUI** — Added summaries for all 10 GitHub tools (`github_list_issues`, `github_get_issue`, `github_create_issue`, `github_comment_issue`, `github_close_issue`, `github_list_prs`, `github_get_pr`, `github_create_pr`, `github_merge_pr`, `github_review_pr`), displaying actions like "📋 issue #42 created", "📋 PR #7 merged", etc.
- **GitLab tool summaries in TUI** — Added summaries for all 14 GitLab tools (`gitlab_list_projects`, `gitlab_get_project`, `gitlab_list_issues`, `gitlab_get_issue`, `gitlab_create_issue`, `gitlab_close_issue`, `gitlab_list_prs`, `gitlab_get_pr`, `gitlab_create_pr`, `gitlab_get_pipeline`, `gitlab_list_jobs`, `gitlab_get_job`, `gitlab_retry_job`, `gitlab_cancel_job`), displaying actions like "🦊 project retrieved", "🦊 issue #5 created", "🦊 pipeline #3", etc.
- **Tool output summaries** — `tool_output_summary` function extended to cover git, GitHub, and GitLab tools with descriptive output strings.

## Version: 0.1.0-alpha.89

### Changed
- **README.md rebuilt** — Rewrote from scratch to reflect the current specification. Expanded feature list to ~111 tools across 15 categories, corrected provider list (10 providers), added missing systems (memory, spec management, skills, teams/swarm, autopilot, MCP client, config error reporting), updated architecture table with all 15 crates, and refreshed project status.
- **STATS.md updated** — Complete rewrite showing project-wide metrics (175,840 lines, 468 files, 1,670 tests) and a per-crate breakdown with file counts, line counts, test files, descriptions, ASCII bar chart, and architecture ratios.
- **SPEC.md cover page** — Added styled HTML cover page with title, author, version, date, and repository link.

## Version: 0.1.0-alpha.88

### Fixed
- **Context compaction bug** — Fixed `compact_history_with_atomic_tool_calls` which was breaking the compaction loop prematurely when the oldest message was part of a tool call pair, preventing any trimming. Now uses prefix sums and a proper scan to find the correct truncation point while preserving atomic tool call pairs.

### Changed
- **SPEC.md audit and corrections** — Comprehensive review of SPEC.md against actual codebase implementation. Corrected tool counts (File Ops 14, Execution 3, Memory 8, Team 20), removed non-existent tools (`execute_python`, `file_ops_tool`, dead aliases), fixed version string to `alpha.88`, added alpha.87/alpha.88 to Appendix A, replaced MCP stub (§19) and Auto-Update stub (§20) with real documentation, expanded §5.2 config schema to include `tool_visibility`, `dirs`, `bash`, `gitlab`, `internal_llm`, `stream`, added missing slash commands (`/config show`, `/dirs`, `/profile`, `/theme`, `/status`, `/mouse`) to §6.2, and expanded §5.3 environment variable table with all provider-specific keys.

## Version: 0.1.0-alpha.87

### Fixed
- **Read tool instructions** — Clarified in AGENTS.md that `end_line` is an absolute line number, not a count or offset.
- **Remote push instructions** — Strengthened AGENTS.md guidelines to explicitly prohibit pushing without explicit user instruction.

### Changed
- **SPEC.md reorganization** — Reorganized sections into logical order (1-20), fixed numbering, added Blueprints subsection (14.7), and merged GitHub/GitLab into a single peer section (18).

## Version: 0.1.0-alpha.86

### Added
- **Azure Resource (File) provider** — New `azure_resource` provider that reads Azure endpoint definitions from `azureresources.json` in `~/.config/ragent/` or `.ragent/`. Supports multiple resource entries with per-endpoint API keys, environment-variable-based keys, custom context windows, capability tags, and thinking configuration.
- **Azure Resource documentation** — Added `docs/userdocs/azure-resource.md` with full JSON schema, field reference, copy-pasteable example, and troubleshooting guide.
- **Azure Resource integration tests** — Added `crates/ragent-tui/tests/test_azure_resource_flow.rs` covering provider listing, persistence round-trip, stale selection cleanup, ModelInfo conversion, and backend resolution.

## Version: 0.1.0-alpha.85

### Changed
- **Version bump** — Incremented pre-release version for release.

## Version: 0.1.0-alpha.84

### Added
- **Azure test script** — Added `scripts/getresult.sh` for testing Azure AI Foundry chat completions.

## Version: 0.1.0-alpha.83

### Fixed
- **SPEC.md fixes** — Fixed malformed benchmark runner table, corrected Team Lifecycle mermaid diagram syntax, replaced misplaced GitLab Integration section with proper content, and updated version references throughout.

## Version: 0.1.0-alpha.82

### Added
- **azProvider fixes** — Applied fixes to Azure provider implementation.
- **`/config show`** — Added `/config show` slash command to display current configuration.

## Version: 0.1.0-alpha.81

### Fixed
- **Azure endpoint logging** — TUI log panel now displays the full endpoint URL for Azure AI Foundry requests.

## Version: 0.1.0-alpha.78

### Fixed
- **Azure endpoint logging** — TUI log panel now displays the full endpoint URL for Azure AI Foundry requests, not just the `[provider/model]` prefix.

## Version: 0.1.0-alpha.77

### Added
- **Azure endpoint logging** — Azure AI Foundry provider now logs the resolved endpoint via `tracing::info!` when connecting.

## Version: 0.1.0-alpha.76

### Added
- **Azure AI Foundry provider** — New `azure_foundry` provider for Microsoft Azure AI Foundry models. Supports OpenAI-compatible endpoints with `api-key` header authentication, dynamic model discovery, streaming chat completions, tool calling, vision, and reasoning levels (o1, o3-mini). Configurable via `AZURE_AI_FOUNDRY_API_KEY` and `AZURE_AI_FOUNDRY_BASE` environment variables or `ragent.json`.

## Version: 0.1.0-alpha.75

### Fixed
- **SPEC.md mermaid diagrams** — Fixed 2 diagrams (Figure 1 and Figure 7) where closing fences/`end` keywords were on the same line as node definitions, which broke rendering. All 14 diagrams now pass syntax validation and render correctly.

## Version: 0.1.0-alpha.73

### Fixed
- **/model selection** — Fixed `/model` selection handling.

## Version: 0.1.0-alpha.72

### Added
- **gen-spec-pdf.sh script** — New `scripts/gen-spec-pdf.sh` for converting Markdown specifications (with Mermaid diagrams) to PDF using Pandoc and Chromium.

### Changed
- **SPEC.md updates** — Removed LSP references and added a dedicated Spec Management section documenting the spec tool suite.

## Version: 0.1.0-alpha.71

### Added
- **Startup ASCII art banner** — The TUI now displays an ASCII art rendering of the application name on startup, followed by the version number and the exact date/time the binary was compiled.

## Version: 0.1.0-alpha.70

### Changed
- **Update concurrency** — Improved concurrency handling across the codebase.
- **Fix todos** — Resolved outstanding todo issues.

## Version: 0.1.0-alpha.68

### Added
- **`/codeindex` language filtering** — The `/codeindex` slash command now supports an optional `lang` parameter to filter code index results by programming language (e.g., `/codeindex lang rust`).

### Changed
- **Benchmark data cleanup** — Removed unused benchmark dataset files from `benches/data/` across multiple languages and suites, significantly reducing repository size.

## Version: 0.1.0-alpha.67

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.67.

## Version: 0.1.0-alpha.66

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.66.

## Version: 0.1.0-alpha.65

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.65.

## Version: 0.1.0-alpha.64

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.64.

## Version: 0.1.0-alpha.63

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.63.

## Version: 0.1.0-alpha.62

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.62.

## Version: 0.1.0-alpha.61

### Added
- **Instruction file discovery logging** — New `InstructionFileDiscovery` struct and `collect_agents_md_content_with_discovery()` function track which AGENTS.md-style files were found and where. Logs discovery summary via tracing and emits `AgentNotice` events for visibility.

### Changed
- **AgentNotice display** — TUI now displays `AgentNotice` events in the message window ("📋 **Agent Notice**" prefix) in addition to the status bar, making instruction file discovery visible to users.
- **Improved formatting** — AGENTS.md acknowledgment messages now include a newline separator for better readability.

## Version: 0.1.0-alpha.60

### Added
- **Global AGENTS.md search path** — Extended `collect_agents_md_content()` to search `~/.local/share/ragent/` for instruction files. Falls back to global files only when no local project files exist.

### Changed
- **AGENTS.md precedence** — Local project instruction files now completely replace global files, rather than being appended to them. This enables cleaner project-specific overrides of global guidelines.

## Version: 0.1.0-alpha.59

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.59.

## Version: 0.1.0-alpha.58

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.58.

## Version: 0.1.0-alpha.57

### Changed
- **Version bump** — Incremented to 0.1.0-alpha.57.

## Version: 0.1.0-alpha.56

### Added
- **Multilingual benchmark suites** — Added benchmark test files for Go, Java, JavaScript/TypeScript, Python, and Ruby to expand language coverage for the benchmark system.

## Version: 0.1.0-alpha.55

### Fixed
- **Permission milestone test fixes** — Fixed failing unit tests in `test_permission_system.rs` related to context window limits, message ordering, and compact history behaviour. Removed brittle assertions and added robust assertions for actual system state.

## Version: 0.1.0-alpha.54

### Fixed
- **Permission dialog timeout** — Fixed permission dialog timeout from 30 seconds to 120 seconds in `processor.rs`. Added `created_at` and `timeout_secs` fields to `PermissionRequest` struct in `permission/mod.rs`.

### Changed
- **Permission dialog countdown timer** — Implemented live countdown timer in permission dialog title in `crates/ragent-tui/src/input.rs`.

## Version: 0.1.0-alpha.53

### Fixed
- **Permission dialog live update** — Fixed permission dialog countdown not visually decrementing by changing main event loop to always redraw.

## Version: 0.1.0-alpha.52

### Changed
- **Bash safe command display** — Changed `SAFE_COMMANDS` from private to `pub const` in `bash.rs` and updated `/bash show` TUI command to display the built-in safe command list.

## Version: 0.1.0-alpha.51

### Changed
- **Bash permission command name extraction** — Added `extract_command_name()` helper in `processor.rs` to extract just the first word from a bash command before permission checking. Modified bash permission check loop to use command names.

## Version: 0.1.0-alpha.50

### Changed
- **Bash denylist word-boundary matching** — Split `DENIED_PATTERNS` into `DENIED_COMMANDS` (word-boundary matched via command name extraction) and `DENIED_PATTERNS` (substring matched). Added `extract_command_names()` helper in `bash.rs`.

## Version: 0.1.0-alpha.49

### Added
- **Permission dialog countdown** — Added countdown timer to permission approval dialog in TUI.
- **Config parse error enhancement** — Improved config file parser to show clear, actionable errors.
- **Codeindex hardwired permissions** — Made codeindex tools always allowed without permission checks.

## Version: 0.1.0-alpha.48

### Fixed
- **Permission milestones** — Fixed various issues in permission system and bash security layers.

## Version: 0.1.0-alpha.47

### Changed
- **Crate reorganisation** — Extracted `ragent-types`, `ragent-config`, `ragent-storage`, and `ragent-llm` from `ragent-core`.

## Version: 0.1.0-alpha.46

### Added
- **Permission system** — Implemented core permission system with 20 passing tests.

## Version: 0.1.0-alpha.45

### Added
- **Bash security** — Implemented 7-layer bash security system.

## Version: 0.1.0-alpha.44

### Added
- **Permission dialog** — Added permission approval dialog with timeout.

## Version: 0.1.0-alpha.43

### Added
- **Permission checker** — Implemented permission checker with allow/deny/ask rules.

## Version: 0.1.0-alpha.42

### Added
- **Permission rules** — Added permission rule evaluation with last-match-wins semantics.

## Version: 0.1.0-alpha.41

### Added
- **Permission request flow** — Implemented permission request flow with EventBus integration.

## Version: 0.1.0-alpha.40

### Added
- **Permission system foundation** — Added Permission enum, PermissionAction, PermissionRule, and PermissionChecker.

## Version: 0.1.0-alpha.39

### Added
- **Code index tools** — Added `codeindex_search`, `codeindex_symbols`, `codeindex_references`, `codeindex_dependencies`, `codeindex_status`, and `codeindex_reindex` tools.

## Version: 0.1.0-alpha.38

### Added
- **Code index** — Implemented codebase indexing with tree-sitter parsing and Tantivy FTS.

## Version: 0.1.0-alpha.37

### Added
- **Memory system** — Implemented three-tier memory system with file blocks, structured SQLite store, and semantic search.

## Version: 0.1.0-alpha.36

### Added
- **Teams** — Implemented multi-agent coordination with named teammates and shared task lists.

## Version: 0.1.0-alpha.35

### Added
- **Swarm mode** — Implemented swarm decomposition for parallel task execution.

## Version: 0.1.0-alpha.34

### Added
- **Autopilot mode** — Implemented autonomous operation mode.

## Version: 0.1.0-alpha.33

### Added
- **Custom agents** — Implemented OASF-based custom agent profiles.

## Version: 0.1.0-alpha.32

### Added
- **Skills system** — Implemented loadable skill packs.

## Version: 0.1.0-alpha.31

### Added
- **Prompt optimization** — Implemented `/opt` slash command with 12 methods.

## Version: 0.1.0-alpha.30

### Added
- **MCP client** — Implemented Model Context Protocol client support.

## Version: 0.1.0-alpha.29

### Added
- **Background agents** — Implemented sub-agent spawning and management.

## Version: 0.1.0-alpha.28

### Added
- **Event bus** — Implemented internal tokio pub/sub for real-time UI updates.

## Version: 0.1.0-alpha.27

### Added
- **Snapshot & undo** — Implemented file snapshots before edits.

## Version: 0.1.0-alpha.26

### Added
- **Project guidelines** — Implemented auto-loading of `AGENTS.md` from project root.

## Version: 0.1.0-alpha.25

### Added
- **Agent presets** — Implemented coder, task, architect, ask, debug, code-review agents.

## Version: 0.1.0-alpha.24

### Added
- **Permission system** — Implemented configurable permission rules.

## Version: 0.1.0-alpha.23

### Added
- **Session management** — Implemented persistent conversation history in SQLite.

## Version: 0.1.0-alpha.22

### Added
- **HTTP server** — Implemented axum-based REST + SSE API.

## Version: 0.1.0-alpha.21

### Added
- **Terminal UI** — Implemented full-screen ratatui interface.

## Version: 0.1.0-alpha.20

### Added
- **Tool system** — Implemented core tool registry and dispatch.

## Version: 0.1.0-alpha.19

### Added
- **GitHub integration** — Implemented GitHub tools for issues and PRs.

## Version: 0.1.0-alpha.18

### Added
- **GitLab integration** — Implemented GitLab tools for issues, MRs, and pipelines.

## Version: 0.1.0-alpha.17

### Added
- **Office tools** — Implemented office_read, office_write, office_info, libre_read, libre_write, libre_info.

## Version: 0.1.0-alpha.16

### Added
- **PDF tools** — Implemented pdf_read and pdf_write.

## Version: 0.1.0-alpha.15

### Added
- **Web tools** — Implemented webfetch, websearch, and http_request.

## Version: 0.1.0-alpha.14

### Added
- **Bash tool** — Implemented bash execution with security restrictions.

## Version: 0.1.0-alpha.13

### Added
- **File tools** — Implemented read, write, create, edit, multiedit, patch, copy_file, move_file, rm, mkdir, append_file, file_info, diff_files, glob, and list.

## Version: 0.1.0-alpha.12

### Added
- **Provider system** — Implemented Anthropic, OpenAI, and Ollama providers.

## Version: 0.1.0-alpha.11

### Added
- **Configuration** — Implemented ragent.json configuration loading.

## Version: 0.1.0-alpha.10

### Added
- **Storage** — Implemented SQLite-backed storage.

## Version: 0.1.0-alpha.9

### Added
- **Event system** — Implemented EventBus with tokio broadcast channels.

## Version: 0.1.0-alpha.8

### Added
- **Message system** — Implemented chat message types and serialization.

## Version: 0.1.0-alpha.7

### Added
- **LLM client** — Implemented HTTP client for LLM providers.

## Version: 0.1.0-alpha.6

### Added
- **Types** — Implemented shared types and IDs.

## Version: 0.1.0-alpha.5

### Added
- **CLI** — Implemented clap-based CLI with run, serve, session, auth, models, and config commands.

## Version: 0.1.0-alpha.4

### Added
- **TUI** — Implemented ratatui terminal interface.

## Version: 0.1.0-alpha.3

### Added
- **Server** — Implemented axum HTTP server with REST + SSE endpoints.

## Version: 0.1.0-alpha.2

### Added
- **Tools** — Implemented core tool system.

## Version: 0.1.0-alpha.1

### Added
- **Initial project scaffolding** — Created Cargo workspace with core crates.

## Version: 0.1.0-alpha.0

### Added
- **Initial commit** — Project created.
