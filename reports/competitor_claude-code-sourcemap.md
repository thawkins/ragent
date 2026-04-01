Competitor report: claude-code-sourcemap

Overview

- Repository inspected: ../claude-code-sourcemap
- Notes: This is a packaged fork of Anthropic's "Claude Code" research-preview (v0.2.8) with extracted source maps and a pre-built CLI bundle (cli.mjs). It is a Node/TypeScript React-Ink terminal application focused on an interactive coding assistant backed by Claude (Anthropic).
- Files consulted (exact paths):
  - ../claude-code-sourcemap/README.md
  - ../claude-code-sourcemap/LICENSE.md
  - ../claude-code-sourcemap/cli.mjs
  - ../claude-code-sourcemap/yoga.wasm
  - ../claude-code-sourcemap/src/entrypoints/cli.tsx
  - ../claude-code-sourcemap/src/services/claude.ts
  - ../claude-code-sourcemap/src/services/mcpClient.ts
  - ../claude-code-sourcemap/src/tools/* (many tool implementations)
  - ../claude-code-sourcemap/src/utils/ripgrep.ts
  - (Top-level src/ directory and vendor/ ripgrep and sdk folders)

Project summary and purpose

Claude Code is an agentic, terminal-first coding assistant that understands the user's codebase and executes developer workflows via natural language commands. It offers file-editing tools, shell execution, repository search, test/lint execution and git workflows, plus dialogs (onboarding, trust dialog), and integrates tightly with Anthropic's Claude API. It is presented as a research preview with telemetry, feedback collection, and feature flags for experimentation.

Complete feature list (implemented by the project)

Each feature name plus short description - distilled from the source tree and README:

- CLI (node) entrypoint: A command-line application (cli.mjs / src/entrypoints/cli.tsx) that launches a React Ink TUI or runs one-shot prompts.
- React Ink TUI: Full-screen terminal UI components (src/components and src/screens) including REPL, onboarding, logs, dialogs, prompts.
- Anthropic/Claude API integration: services/claude.ts handles building requests and streaming responses to/from the Claude models via the Anthropic SDK.
- Built-in Tools system: tools/* implements modular tools (BashTool, FileRead/Write/Edit, Glob, Grep, Notebook tools, MCPTool, etc.) that the agent can call.
- Persistent sessions and logging: message logs, message recovery, and session persistence (utils/log.js, utils/conversationRecovery, message files).
- Permissions and Trust flow: permission gating UI and logic (components/permissions and utils/permissions) to confirm file writes, shell commands, etc.
- Onboarding and Approvals: interactive onboarding, approve API key flows, trust dialog (components/Onboarding, ApproveApiKey, TrustDialog).
- Ripgrep integration / fast code search: vendor/ripgrep and utils/ripgrep.ts for fast repository search.
- Jupyter Notebook read/edit tools: NotebookReadTool and NotebookEditTool support .ipynb operations.
- File edit UI & structured diffs: FileEditTool includes snippet extraction and diff presentation (components/permissions/FileEditToolDiff.tsx)
- Bash execution with persistent shell: PersistentShell utility and BashTool for running shell commands with sandboxing and banned-command checks.
- MCP (Model Context Protocol) client/server support: services/mcpClient.ts and entrypoints/mcp.ts to integrate external tool servers.
- OAuth/API key flows: console-based OAuth and create/store API key helpers (services/oauth.ts).
- Binary & structured feedback: components/binary-feedback implements sampling & recording of binary feedback.
- Auto-update helper: utils/autoUpdater for checking and installing newer versions.
- Token/usage & cost tracking UI: cost-tracker.ts and cost dialogs.
- Sentry integration & telemetry gating: services/sentry.js plus bundled Sentry client in cli.mjs (error capture, source-map/debug-id handling).
- Statsig feature gates/experiments: services/statsig.ts integration for feature gating and experiments.
- Notebook/attachment support: tools for images and attachments in messages.
- Multiple tools for developer workflows: search (grep/glob), file read/write, patch/multiedit, webfetch/websearch helpers, sticker request tool (swag), think tool, memory tools.

Unique / standout features and UX elements

- Terminal-first UX using React Ink, with polished dialogs (onboarding, trust), an onboarding flow, and step-wise tool calls shown in the log panel.
- Permission & Trust Dialog system that forces the user to grant read/write privileges and review potentially risky operations before execution.
- Built-in sampling and structured binary feedback pipeline for product research and iterated model adjustments.
- Sentry integration with explicit handling for "sourcemap" images (the bundled cli.mjs contains Sentry code that can attach debug/sourcemap metadata to events).
- MCP (Model Context Protocol) support: ability to connect to external servers providing additional tools and context for the agent.
- Fast repository search using bundled ripgrep optimized for large codebases.
- Notebook-aware file tools that treat .ipynb cells specially.
- Rich, tool-oriented prompts (each tool has a prompt.ts that describes its capability to the model) — good for tool-use safety and predictable behavior.

Architecture and key implementation files/modules

Top-level structure
- cli.mjs (pre-built Node bundle) — shipped executable bundle you can run directly. Large built/minified file that contains dependencies and Sentry bundle.
- src/ (TypeScript sources) — full source with components, tools, services and entrypoints.
- vendor/ (ripgrep, sdk) — bundled native/specialized third-party tools and SDK shims.
- yoga.wasm — binary used by layout/UI (Ink yoga layout engine) or other UI layout tasks.

Key modules and how they implement features
- src/entrypoints/cli.tsx — main CLI boot sequence, TUI rendering, onboarding & permission flows, wiring of services.
- src/services/claude.ts — Anthropic client wrapper: verifyApiKey, getAnthropicClient, message conversions, top-level query streaming helpers (query(), querySonnet(), queryHaiku(), formatSystemPromptWithContext()). Implements LLM interactions and error handling.
- src/tools/* — Individual tool implementations. Examples:
  - tools/BashTool/* — execution and security constraints for shell commands.
  - tools/FileEditTool/* — helper to extract snippets, apply edits, and present diffs.
  - tools/GrepTool and tools/GlobTool — repository search capabilities.
  - tools/MCPTool/* — interact with external MCP servers.
- src/services/mcpClient.ts — MCP client management, server discovery, running remote commands and wrapping those tools into local tool interfaces.
- src/components/*, src/screens/* — UI components rendered by Ink: onboarding, REPL, logs, prompts, permission dialogs.
- src/utils/ripgrep.ts and vendor/ripgrep — fast code search integration used by GrepTool.
- src/services/sentry.ts + bundled Sentry code in cli.mjs — captureException, captureMessage, and the large Sentry bundle shows sourcemap/debug-id handling (the bundle contains applyDebugMeta/applyDebugIds logic). The built bundle includes Sentry client code that can attach debug_meta.images with type "sourcemap" which is how Sentry uses sourcemaps to map minified frames.

Integration points (APIs, CLIs, SDKs) and sourcemap-specific functionality

- CLI: The project is delivered as an npm package and a CLI entrypoint (cli.mjs). Start via `claude` binary installed globally or run node cli.mjs directly.
- Anthropic SDK: uses @anthropic-ai/sdk shims (see import line in src/entrypoints/cli.tsx and vendor/sdk/). services/claude.ts wraps the model calls. Requires an Anthropic API key (ANTHROPIC_API_KEY) or an OAuth flow to console.
- OAuth flows: services/oauth.ts provides console OAuth flows and createAndStoreApiKey helper.
- MCP protocol: services/mcpClient.ts, entrypoints/mcp.ts and tools/MCPTool allow connecting to remote MCP servers (extend toolset and offload heavy tasks).
- Ripgrep vendor: fast repo search via bundled ripgrep binary integration.
- Sentry: services/sentry.js plus bundled client code present in cli.mjs — Sentry is initialized early (cli.tsx) and the bundled Sentry client includes logic to attach debug_meta.images referencing sourcemaps (the built bundle contains logic to detect debug ids and attach images type "sourcemap"). That is likely why the fork was described as "with extracted source maps" — the build was produced with separated source maps so Sentry can map errors back to the TS sources.
- Node/Bundled dependencies: the project bundles many deps in cli.mjs, including large SDKs and Sentry. It also includes yoga.wasm for layout.

Strengths vs our product (ragent)

- UX polish and onboarding: Claude Code has a very polished Ink-based terminal UX, onboarding flows, trust dialog, and many small UX touches (Approve API key, stickers, trust) that are product-grade.
- Rich toolset and model-driven tools: many pre-built, well-documented tools (Bash, FileEdit, Notebook tools, MCP integration) with per-tool system prompts for safer tool usage.
- Fast code search integration: bundled ripgrep gives excellent repository search performance for large codebases.
- Experimentation infrastructure: Statsig, binary feedback sampling, and Sentry telemetry are integrated for experiments and product analytics.
- MCP support: external tool servers allow extending capabilities without changing the client.
- Notebook (.ipynb) awareness: native notebook read/write tools.

Weaknesses vs our product

- License and redistribution: Claude Code is governed by Anthropic's commercial terms (LICENSE.md), not an open-source permissive license. This restricts reuse and redistribution compared to ragent's MIT license.
- Single-provider bias: code is tightly coupled to Anthropic/Claude API — multi-provider support (OpenAI, Copilot, Bedrock etc.) is either not present or limited compared to ragent's multi-provider architecture.
- Monolithic JS bundle: large, pre-built bundle (cli.mjs) and many Node deps increases runtime attack surface, memory footprint, and complexity compared to ragent's single Rust static binary.
- Telemetry & privacy: the product collects usage/feedback (transcripts) and uses Statsig/Sentry — this might raise privacy/enterprise concerns; ragent's design may better match self-hosting or privacy-first use-cases.
- Less emphasis on concurrent background agents / team workflow primitives — ragent's team/agent spawning primitives and Rust-based background agent tooling differ (ragent provides task/teams APIs and background sub-agents design).

Missing features relative to our product and other competitors

- Multi-provider LLM orchestration out-of-the-box (OpenAI, Copilot, Vertex etc.). Claude Code centers on Anthropic.
- Rust static binary / single executable deployment — Claude Code requires Node + native binaries (ripgrep, wasm) and an npm-installed CLI.
- Built-in background/sub-agent new_task/cancel_task primitives as ragent's sub-agent APIs — while MCP exists, I did not find the same team/task claim/complete semantics.
- SQLite-backed session store exposed as a server API; while Claude Code stores logs/messages on disk, ragent's server + SQLite session API is more explicit.

Licensing and redistribution constraints

- License file: ../claude-code-sourcemap/LICENSE.md contains Anthropic commercial terms and © Anthropic PBC. It references Anthropic's Commercial Terms of Service, and explicitly states usage is subject to those terms.
- Implications:
  - Not an OSI-approved permissive license — reuse and redistribution will likely be restricted or forbidden for our open-source project.
  - Incorporating code, assets, or logic into an MIT-licensed project would require legal review and likely permission from Anthropic.
  - Product components (cli.mjs) may include proprietary SDK shims and telemetry obligations.

Adoptable features (for ragent): rationale, estimated complexity, priority, and brief implementation approach

We'll list candidate features we might adopt into ragent, each with notes and mapping to ragent modules to change.

1) Ripgrep-based fast repository search
- Rationale: ripgrep provides very fast source-level search and is highly valuable for large repos.
- Complexity: M (medium) — integration requires bundling or calling a ripgrep binary and a thin wrapper; Windows cross-platform differences to handle.
- Priority: 5
- Implementation approach: Add a new tool in ragent-core - tools/greph (or extend existing Grep tool). Implement a small wrapper crate (or spawn vendor/ripgrep) under crates/ragent-core/tools/ripgrep.rs (or call ripgrep binary). Data flow: TUI -> tool call -> wrapper spawns ripgrep and streams output -> normalize to tool result messages. Tests: unit test wrapper parsing and integration test on sample repo. Files to change: crates/ragent-core/src/tools.rs (add new tool), ragent-tui to add UI for results streaming.

2) Permission & Trust Dialog flow (grant read/write/per-tool gating)
- Rationale: Safer by default; prevents accidental destructive operations by the agent.
- Complexity: M (medium) — ragent already has a permission system but we can adopt the nuanced interactive trust dialog and per-tool diffs.
- Priority: 4
- Implementation approach: Reuse ragent's permission rules, add a TUI dialog in ragent-tui that appears the first time a project is opened; add a PermissionRequest UI component and a FileEditTool diff renderer. Files to change: ragent-tui/src/ui/dialogs.rs, ragent-core/src/permissions.rs. Tests: integration tests for permission prompts and auto-approve flag.

3) Per-tool system prompts (tool prompt descriptors)
- Rationale: Cleaner separation between tools and LLM system prompt; improves model grounding and safety.
- Complexity: S (small) — add metadata files for each tool describing its capabilities and use them during prompt construction.
- Priority: 3
- Implementation approach: Extend ragent-core Tool trait to include `prompt_description` and use it when building system messages. Files to change: crates/ragent-core/src/tools/mod.rs, crates/ragent-core/src/providers/prompt_builder.rs. Tests: unit tests asserting prompt includes tool descriptions.

4) Binary feedback sampling & Statsig-like experiment scaffolding
- Rationale: Useful product telemetry and experimentation to choose UX defaults and measure acceptance.
- Complexity: L (large) — requires infra (server side) or pluggable backend; but a local sampling/telemetry client can be implemented.
- Priority: 2
- Implementation approach: Add a feedback sampling module in ragent-core that logs opt-in feedback events to local storage and optionally to a configured telemetry endpoint. Files: crates/ragent-core/src/feedback.rs, ragent-server endpoints to expose sendFeedback. Tests: end-to-end test using local file sink.

5) MCP (Model Context Protocol) client integration
- Rationale: Offloading expensive tools to external servers allows extending capabilities without shipping heavy baggage.
- Complexity: M-L depending on protocol parity; ragent has a server architecture already so adding MCP client is feasible.
- Priority: 3
- Implementation approach: Implement an MCP client crate that can list remote tools, run remote commands, and wrap results as local tools. Files: crates/ragent-core/src/mcp_client.rs and ragent-server to publish connector endpoints. Tests: unit mock server and integration tests.

For each adoptable feature, exact modules to change were suggested above.

Non-obvious risks and suggested mitigations

- License/IP risk: Claude Code is covered by Anthropic commercial terms — do not copy source code, prompts verbatim, or assets without legal review. Mitigation: reimplement ideas in our codebase (as we would), do not reuse their source. Keep inspiration only in design and behavior.
- Telemetry/privacy: The competitor collects transcripts and uses Statsig and Sentry. If adopting feedback/telemetry, ensure opt-in, local-first storage, redact sensitive content, and provide clear retention policies. Use encryption-at-rest and per-organization control.
- Sentry / Sourcemap handling: If we integrate Sentry and upload sourcemaps, be careful to not leak proprietary source maps publicly; ensure debug id/sourcemap uploads are to a secure project and require authentication.
- Performance: Spawning ripgrep or external MCP servers may be heavy. Mitigation: stream outputs, cap concurrency, provide caches, and sandbox long-running commands.
- Security: Allowing remote MCP servers or shell execution opens attack surface. Mitigation: permission gating, sandboxing, rate-limiting, signature verification for remote tool responses.

POC experiments (3 minimal experiments to evaluate adoption)

1) Ripgrep integration POC (expected time: 1-2 days)
- Implement a small wrapper in ragent to call system ripgrep (spawn process) and stream results into the TUI. Cost: low (developer time). Success criteria: streaming results appear, search is significantly faster than naive file scanning on a medium repo.

2) Permission dialog & file-edit diff POC (expected time: 2-3 days)
- Implement a TUI dialog that asks for user confirmation before the first write and shows a unified diff preview for FileEdit tool results. Cost: medium. Success criteria: user flow integrates smoothly and prevents accidental writes in manual testing.

3) MCP connectivity POC (expected time: 3-5 days)
- Stand up a minimal MCP mock server (HTTP) that registers a single tool and returns results. Implement client code in ragent to list and call that remote tool. Cost: medium-high. Success criteria: ragent can call the remote tool and present the streamed output as a tool-use result.

Build/test steps to inspect runtime behavior of Claude Code clone

- Requirements: Node.js 18+, npm/yarn, environment variable ANTHROPIC_API_KEY (or perform OAuth with an Anthropic Console account).
- Commands to run the packaged CLI (pre-built):
  - node cli.mjs --help
  - ANTHROPIC_API_KEY="sk-..." node cli.mjs
- To inspect source and run from source:
  - npm install (or yarn)
  - npm run build (if build script exists) — large repo may not include straightforward build steps; the repo includes prebuilt cli.mjs.
- Files needed for runtime: vendor/ripgrep binary and yoga.wasm are included; ensure they are executable.

References and exact files read

- ../claude-code-sourcemap/README.md
- ../claude-code-sourcemap/LICENSE.md
- ../claude-code-sourcemap/cli.mjs
- ../claude-code-sourcemap/yoga.wasm
- ../claude-code-sourcemap/src/entrypoints/cli.tsx
- ../claude-code-sourcemap/src/services/claude.ts
- ../claude-code-sourcemap/src/services/mcpClient.ts
- ../claude-code-sourcemap/src/tools/* (directory listing)
- ../claude-code-sourcemap/src/utils/ripgrep.ts
- ../claude-code-sourcemap/src/components/Onboarding.tsx

Concluding notes

Claude Code is a highly polished, productized terminal coding assistant with deep Anthropic integration, a rich tool ecosystem, and experimentation tooling. Its IP and license mean we should not directly reuse source code, but many ideas (ripgrep-based search, permission dialogue, MCP pattern, per-tool prompts, notebook-aware file editing) are attractive and worth reimplementing in ragent on our own terms.


