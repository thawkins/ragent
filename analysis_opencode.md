# OpenCode Competitor Analysis

Repo inspected: ../opencode (anomalyco/opencode)

Summary
-------
OpenCode is an open-source AI coding agent focused on a TUI-first developer experience with a rich plugin architecture and provider-agnostic model integrations. It couples a lightweight client (TUI / desktop / web) with a server-side core and exposes well-defined extension points for plugins (server and TUI), tools, LSPs, and provider authentication.

Primary features
----------------
- Agent model: built-in agents ("build", "plan", and a general subagent) with different permission profiles (e.g., plan is read-only and asks before commands).
- Plugin architecture: first-class plugin package (packages/plugin) exposing server-side hooks and a TUI plugin API (slots, commands, dialogs, keybinds, theme, kv store, lifecycle). Plugins can be server-only or TUI-only.
- Hook system: extensive hook surface for plugins (chat.message, chat.params, chat.headers, tool.execute.before/after, command.execute.before, permission.ask, shell.env, experimental transforms, tool.definition, session compaction hooks, etc.).
- Provider-agnostic model support: integrations for many LLM providers, pluggable provider auth hooks (OAuth / API key flows) and ability to run local models via "OpenCode Zen" or other providers.
- LSP integration: out-of-the-box LSP support with automatic detection/activation of LSP servers for file types; LSP server spawning and lifecycle management; LSP debug utilities.
- Client/server architecture: server can run headless while multiple clients (TUI, desktop, web, mobile) connect; enables remote control and multiple frontends.
- TUI-first UX: rich terminal UI, keyboard-first navigation, slots for UI extension, built-in dialogs/prompts/toasts, theme and keybind customization.
- Tooling ecosystem: tool definitions and tool invocation flows (tools exposed to LLM), scripts, shell access via BunShell abstractions, and tools for repository tasks (GitHub triage/search, etc.).
- Plugin install & management: TUI commands to list, enable/disable, install, activate plugins, and metadata tracking for plugins.
- Desktop/packaged clients: Tauri and Electron-based desktop apps (installer artifacts and releases).
- Session & permission model: session scoping, permission requests/flows (asking the user for permission before executing potentially destructive actions), and question requests.
- Session compaction/summary customization: plugins can customize compaction prompts used to compress session history.

Architecture
------------
- Monorepo (TypeScript/Node/Bun) with clear separation: packages/opencode (CLI/server), packages/plugin, packages/console, packages/web, packages/ui, desktop packages, and SDKs.
- Server side: packages/opencode implements core services (LSP manager, tool registry, session management, effects system, config). It exposes an SDK used by plugins.
- Plugin model: plugins are modules with either a server 'server' function that returns Hooks or a TUI 'tui' function that receives a TuiPluginApi. Plugin metadata and lifecycle are managed by server and TUI.
- TUI layer: rich UI primitives and a slot system (register route, register commands, dialogs, KV store, event bus), enabling fully custom TUI extensions.
- LSP subsystem: dedicated LSP server definitions and a client manager that spawns per-root LSP servers and debounces diagnostics. LSP servers are configurable and discoverable by file extension.
- SDK layer: @opencode-ai/sdk provides typed client-side API for custom integrations and remote clients.

Extensibility points
--------------------
- Server plugins (PluginModule.server) returning Hooks with many lifecycle and event hooks (chat.*, tool.*, shell.env, permission.ask, auth, event).
- TUI plugins (PluginModule.tui) exposing UI registration: commands, routes, slots, dialogs, plugin installation management.
- ToolDefinition exports: plugins can add tools that the agent or LLM may call, and tool.execute.before/after hooks let plugins intercept/modify tool calls.
- Provider auth hooks (AuthHook) to add OAuth or API auth flows for providers.
- LSP config and server registry: add/modify LSP servers and behavior.
- Experimental hooks for session compaction and message transformations.

Notable implementation patterns
-------------------------------
- Strong typing via TypeScript and exported SDK types. Plugin APIs are typed and exported from packages/plugin.
- Clear separation of server- and client-side extension surfaces (server vs. tui modules in plugins).
- Hook-based event system — rather than templated callbacks, OpenCode provides many named hooks that plugins implement to alter behavior.
- Instance-scoped effects/effect system (packages/opencode contains an 'effects' pattern / InstanceState) to manage per-directory agent state.
- Auto-detection + opinionated defaults: automatic LSP detection and loading, defaults for agents and permissions.
- Packaging across multiple frontends: CLI/TUI, web, desktop, and VSCode SDKs — same core server.

Unique functionality / UX differentiators
---------------------------------------
- TUI-first, keyboard-driven UX with plugin slots: OpenCode treats the terminal experience as a first-class citizen and exposes slot-based UI extension points allowing plugins to register routes, commands, and dialogs directly into the TUI.
- Dual agent model (build vs plan): built-in 'plan' agent configured as read-only + asks for command execution, enabling safer code exploration workflows.
- Rich plugin hook surface affecting LLM input and execution flows (chat.params, chat.headers, experimental transforms), enabling plugins to programmatically influence prompts and tool metadata.
- Integrated LSP manager that automatically loads language servers appropriate for detected file types; enables accurate code navigation, diagnostics and symbol search integrated into the agent.
- Plugin install/activate/deactivate UX built-into the TUI, complete with metadata tracking and first/updated/same states.
- Provider-agnostic model layer and bundled model recommendations (OpenCode Zen), but no lock-in — easily switch providers or run local models.
- Client/server architecture enabling remote clients including mobile/desktop controlling a server-run agent.

Features OpenCode has that our product (ragent) lacks
----------------------------------------------------
Note: these are based on the inspected OpenCode repo and typical ragent scope. If a specific ragent feature exists, adjust priorities accordingly.

1) First-class plugin system with server & TUI plugin modules
- Rationale: OpenCode allows both server-side hooks and TUI-side UI plugins with typed APIs, enabling rich extensions.
- Estimated complexity to implement: High (design plugin API, sandboxing, plugin lifecycle, compatibility, loader/installer)
- Priority: High

2) Extensive hook surface for LLM/message/tool lifecycle (chat.params, tool.execute.before/after, chat.message transform, tool.definition hooks)
- Rationale: Plugins can control prompts/headers/tool params, enabling features like prompt tuning, telemetry, custom tool authorization, or request throttling.
- Complexity: Medium
- Priority: High

3) Out-of-the-box LSP management & auto-detection
- Rationale: Integrated code intelligence, diagnostics, workspace symbols, and server lifecycle make agent responses much more precise and navigable.
- Complexity: Medium-High
- Priority: High

4) TUI-first slot-based UI extensibility (commands, routes, dialogs, keybinds, toast)
- Rationale: Makes adding UX features and integrations quick; consistent cross-frontend extension model.
- Complexity: Medium
- Priority: Medium

5) Built-in agent modes (plan/read-only versus build/full-access)
- Rationale: Safer code exploration and a reproducible permissioned workflow; plan agent asks permission for destructive actions.
- Complexity: Low-Medium
- Priority: Medium

6) Plugin install/management UX integrated into TUI (install, activate, deactivate, list)
- Rationale: Low friction for users to extend the product and add tools.
- Complexity: Medium
- Priority: Medium

7) Provider auth hooks for OAuth/API providers (pluggable auth flows)
- Rationale: Easier provider onboarding and enterprise SSO integrations.
- Complexity: Medium
- Priority: Medium

8) Client/Server architecture with multi-client support (desktop, web, TUI)
- Rationale: Allows running a headless server and remote control from multiple frontends or mobile devices.
- Complexity: High
- Priority: Low-Medium

9) Session compaction customization hooks and experimental chat transforms
- Rationale: Plugins can implement domain knowledge compression, privacy redaction, or custom summarization.
- Complexity: Medium
- Priority: Low-Medium

Recommended features / improvements inspired by OpenCode
-------------------------------------------------------
The list below prioritizes high impact, reasonably scoped items first.

1) Implement a plugin host with a minimal hook surface (High priority)
- Feature: Server plugin system supporting: chat.message, tool.execute.before, permission.ask, and shell.env.
- Benefit: Enables third-party extensions and internal modularization (analytics, custom tools, auth connectors).
- Complexity: High — but scope can be limited to a small set of hooks to start.
- Implementation notes: In ragent (Rust) expose a Plugin trait and a plugin loader that runs plugins as subprocesses using a JSON-RPC protocol. Keep plugins permissioned and sandboxed. Alternatively, implement WASM-based plugins (wasmtime) for safer embedding. Define a stable typed wire-format (protobuf or JSON schema) and versioned hooks.

2) Add integrated LSP support and an LSP manager (High priority)
- Feature: Automatic detection of language servers, spawn/manage LSP per project root, expose diagnostics and symbols to agent, CLI commands to inspect LSP state.
- Benefit: Improved code understanding for LLM prompts and better navigation/diagnostics for users.
- Complexity: Medium-High
- Implementation notes: Use existing Rust LSP client crates (e.g., tower-lsp for server or languageserver-protocol crates) or run an LSP client process communicating with language servers over stdio. Maintain a registry of known servers and their file extension mapping and provide a configuration to add custom servers. Debounce diagnostics and surface via session API.

3) Expose a small TUI plugin primitives set (Medium priority)
- Feature: slot system to register commands, small UI components (dialogs, prompts, toast), keybinds, and routes.
- Benefit: Allows adding UX flows and integrations without changing core UI code.
- Complexity: Medium
- Implementation notes: If ragent has a tui crate, add a PluginApi struct and registration functions. Plugins register closures that the TUI invokes. Consider using dynamic loading via subprocess IPC if plugins are not Rust native.

4) Add built-in 'plan' (read-only) and 'build' (full-access) agent modes (Medium priority)
- Feature: Multiple agent modes with different permission policies; plan denies direct file edits and asks before shell commands.
- Benefit: Safer exploratory workflows and clearer separation of analysis vs editing actions.
- Complexity: Low-Medium
- Implementation notes: Implement agent profiles in session state and enforce permission checks in the command/tool execution path. Use a permission.ask hook to present approval flows to users.

5) Add tool / LLM parameter hooks (Medium priority)
- Feature: Allow middleware to inspect/modify LLM request parameters and tool call arguments (temperature, top_p, custom headers) via hooks.
- Benefit: Enables rate-limiting, provider-specific adjustments, and per-session tuning.
- Complexity: Medium
- Implementation notes: Add a chain of middleware functions invoked before sending an LLM request or executing a tool; make middleware installable by plugins.

6) Provider-agnostic model layer and pluggable auth (Medium priority)
- Feature: Abstract provider implementations (OpenAI, Anthropic, local) and expose pluggable auth handlers (OAuth + API key flows) for enterprise providers.
- Benefit: Lower lock-in and easier enterprise onboarding.
- Complexity: Medium
- Implementation notes: Provide a Provider trait, an auth manager that supports OAuth and API key flows, and a small SDK to enumerate providers and models.

7) Plugin install/manage CLI commands and basic UI (Medium-Low priority)
- Feature: Commands to add/remove/list/enable/disable plugins; track plugin metadata and fingerprint.
- Benefit: Improves extensibility adoption and developer experience.
- Complexity: Medium
- Implementation notes: Implement a plugin store folder (per-project + global), download/install via npm/URL/git, verify checksum, and provide a manifest for each plugin. For initial MVP, support local plugin directories.

8) Session compaction customization (Low-Medium priority)
- Feature: Allow plugins to add context or replace compaction prompts used when summarizing sessions.
- Benefit: Better domain-specific summaries and privacy filters.
- Complexity: Medium
- Implementation notes: Expose a hook before compaction that passes current session and collector to plugin to return additional context or replace prompt.

Implementation risks and considerations
-------------------------------------
- Security & sandboxing: plugin execution needs careful sandboxing, least-privilege defaults, and user consent flows for tool/shell access.
- Compatibility/versioning: plugin APIs must be versioned and stable; provide migration guidance.
- LSP server binaries: shipping automatic LSP download has platform/packaging implications and adds maintenance overhead.
- UX/consistency: TUI plugin primitives must be minimal and consistent to avoid fragmentation.

Files inspected / limitations
----------------------------
I inspected the following key paths in ../opencode (non-exhaustive):
- README.md (root)
- packages/plugin/src (index.ts, tui.ts)
- packages/plugin/package.json
- packages/opencode (LSP code: packages/opencode/src/lsp, client, server)
- multiple UI and i18n references to LSP, plugins, and TUI behavior via repo-wide grep

This analysis focused on the plugin surface, LSP subsystem, TUI extension points, and architecture. I did not perform a line-by-line audit of every package (monorepo is large). If you want a deeper targeted audit (security, performance, or plugin API details), I can run a follow-up pass that reads specific source files in full.

Conclusion
----------
OpenCode provides a mature, TUI-first AI coding experience centered on an extensible plugin architecture and integrated language tooling (LSP). The highest-impact ideas to evaluate for ragent are: (1) a plugin host with a narrow initial hook surface to enable extensibility, (2) integrated LSP management for accurate code intelligence, and (3) a small TUI plugin API to enable UX extensions. Implement these incrementally with attention to plugin sandboxing and API versioning.
