Competitor analysis: OpenCode

Repository: ../opencode

1) Project summary

OpenCode (anomalyco/opencode) is an open-source, terminal-first AI coding assistant and platform. It ships a CLI/TUI, desktop app, web frontend, SDKs, plugins, and a server/control-plane. The product focuses on developer ergonomics for in-repo, conversational code assistance: editing files, running shell commands, persistent shell sessions, LSP integration, and an extensible tool model (tools/plugins). It's provider-agnostic (supports many model providers) and includes enterprise features like MCP support and permissions/approval flows. The repo is TypeScript/Bun-based and MIT-licensed.

2) Detailed feature list

- Terminal-first TUI with streaming chat, structured diffs and interactive selectors (packages/console, packages/app)
- Headless CLI with subcommands and onboarding
- Two built-in agents: build (full-access) and plan (read-only) plus a general subagent
- Tool-based agent architecture: file edit/read, bash, grep, lsp, multiedit, apply_patch, webfetch, write, task, etc. (packages/opencode/src/tool + tool implementations)
- Persistent shell / PTY session management (pty folder, pty/index.ts)
- LSP integration and language clients/servers (packages/opencode/src/lsp)
- Provider abstraction: many model providers supported via @ai-sdk/* adapters (packages/opencode/src/provider)
- MCP (Model Context Protocol) support and MCP server approval UX (packages/opencode/src/mcp)
- Permissions and per-project trust model (permission folder + UI dialogs)
- Plugin architecture (packages/opencode/src/plugin + @opencode-ai/plugin workspace)
- Built-in ripgrep integration and repo search (file/ripgrep.ts + tool/grep)
- Session management, compaction, truncation and structured output tooling (session/*)
- Telemetry hooks, VCR-style replay tooling, and observability (services & util; present across codebase)
- Desktop apps (Tauri/Electron packaging), web landing pages, SDK (packages/sdk)
- Extensive test suite (packages/opencode/test/*)

3) Standout capabilities

- Terminal/TUI-first UX: full-screen interactive terminal UI, structured diffs, in-terminal onboarding and approval flows.
- Provider-agnostic model layer: pluggable adapters for many providers, including local/Zen models.
- Persistent shell / PTY reuse: enables stateful multi-step shell interactions.
- MCP support with server approval flow: enterprise control for external context servers.
- Rich session tooling: structured output enforcement, compaction, truncation layers and a runner abstraction for session concurrency.
- High test coverage with file fixtures and VCR-style replay for deterministic tests.

4) Architecture & key files

- packages/opencode/src/index.ts — library/entrypoint for opencode package
- packages/opencode/bin/opencode — CLI launcher
- packages/opencode/src/session — session lifecycle, prompt composition, compaction, llm integration
- packages/opencode/src/tool — registry and concrete tools for agent actions (apply_patch, edit, read, write, webfetch, etc.)
- packages/opencode/src/agent & acp — agent orchestration and permission/session control
- packages/opencode/src/pty — PTY/shell session management
- packages/opencode/src/lsp — language server integration
- packages/opencode/test — extensive tests across domains
- packages/console & packages/app — TUI and web frontends

5) Integration points

- Model providers via @ai-sdk modules and a provider transform layer (packages/opencode/src/provider/*)
- OS-level shell and PTY via bun-pty and cross-spawn (packages/opencode/src/pty and effect/cross-spawn-spawner)
- File system and ripgrep for fast repo search (file/ripgrep.ts)
- MCP SDK and approval UI for external context servers (@modelcontextprotocol/sdk usage)
- Plugins via @opencode-ai/plugin workspace packages
- Packaging targets (Tauri, Electron) for desktop apps

6) Strengths vs our product (ragent)

- Strong TUI experience and terminal ergonomics (structured diffs, interactive prompts) — ragent currently focuses on a Rust TUI but lacks some of the same polished interactive components.
- Provider-agnostic, with many provider adapters and early Zen offering — ragent can learn from their flexible provider layer.
- PersistentShell and PTY session reuse for stateful shell tasks — ragent's shell execution is currently ephemeral.
- MCP support and approval UX — ragent lacks explicit external context/MCP approval flows in product inventory.
- Robust test suite and VCR replay patterns for deterministic tests.

7) Weaknesses vs our product

- Codebase is large and heavily TypeScript/Bun-specific; ragent (Rust) may have advantages in smaller binary footprint and memory-safety.
- Shipping and maintaining many native/packaged bits (desktop, vendor patches) increases maintenance cost.
- Some runtime complexity (effect library, effect-based concurrency) has steep learning curve for new contributors.

8) Licensing

- MIT (packages/opencode/LICENSE) — permissive, adoptable for our needs.

9) Developer ergonomics, onboarding, CLI UX that stand out

- One-line install script and multiple OS package channels (curl install, brew, scoop, pacman, nix) in README
- In-CLI onboarding that opens browser-based OAuth flow with local callback server to mint CLI-scoped API keys (reduces friction for getting API keys)
- Agents separation (build vs plan) with read-only plan agent by default — encourages safe exploration
- Interactive permission/trust dialogs and per-project persisted grants
- Streaming in-terminal responses, structured diffs and approval flow before applying edits
- Bundled ripgrep and vendor patches to make search consistent across platforms

10) Adoptable features (with estimated complexity & priority)

- OAuth + CLI-scoped API-key minting (priority: High, complexity: Medium)
  - Benefit: huge onboarding friction reduction. Complexity: implement PKCE, local callback server, provider-side API calls, config storage.

- Persistent shell (priority: High, complexity: Medium)
  - Benefit: stateful workflows, multi-step shell tasks. Complexity: PTY management, streaming, timeouts.

- Per-project trust & permission dialogs (priority: High, complexity: Low–Medium)
  - Benefit: safer defaults and auditability. Complexity: store project config and integrate prompts.

- Vendored ripgrep / fast repo search (priority: Medium, complexity: Low)
  - Benefit: deterministic, fast search. Complexity: packaging or call system ripgrep.

- MCP server approval UX (priority: Medium, complexity: Medium)
  - Benefit: enterprise adoption controls. Complexity: UI + persistence + security review.

- VCR-style session recorder for tests (priority: Medium, complexity: Medium)
  - Benefit: deterministic integration tests and replay.

11) Risks & considerations

- Large TypeScript/Ecosystem: adopting their design/approach requires JS/TS ecosystem expertise (bun, effect, solid-js). Translating patterns to Rust (ragent) will need redesign and careful API mapping.
- External dependency surface: many provider adapters and patched dependencies increase security and maintenance burden.
- UX expectations: the polished TUI and onboarding are product-level efforts — copying features without the associated UX polish may underdeliver.

12) Tests and example projects

- Packages/opencode contains an extensive test suite under packages/opencode/test. Tests use bun test (Bun runtime). Recommended commands to reproduce locally:

  - Ensure Bun is installed (https://bun.sh)
  - From this repository run:

    cd ../opencode/packages/opencode
    bun install
    bun test --timeout 30000

  - Or run a specific test file, e.g. run session prompt tests:

    bun test test/session/prompt.test.ts --timeout 30000

- Notes: root repo includes scripts and a monorepo workspace; many tests mock providers and use fixtures. Running the full test suite may require native dependencies (bun-pty, tree-sitter) and patched modules; run in the environment described in their README (they provide nix/flake and packaging instructions).

13) File references & unique technique snippet

Notable files:
- packages/opencode/src/session/prompt.ts — session runner, prompt resolution, structured output enforcement
- packages/opencode/src/pty/index.ts — PTY/PTY session management
- packages/opencode/src/tool/* — tool implementations (apply_patch, edit, read, write)
- packages/opencode/src/mcp/* — MCP support and oauth flows
- packages/opencode/test/* — unit and integration tests

Example snippet (demonstrates runner map + Runner.make usage and session-scoped runner lifecycle management):

File: packages/opencode/src/session/prompt.ts
Lines: 101-130

--- begin snippet ---
 101        const cache = yield* InstanceState.make(
 102          Effect.fn("SessionPrompt.state")(function* () {
 103            const runners = new Map<string, Runner<MessageV2.WithParts>>()
 104            yield* Effect.addFinalizer(
 105              Effect.fnUntraced(function* () {
 106                yield* Effect.forEach(runners.values(), (r) => r.cancel, { concurrency: "unbounded", discard: true })
 107                runners.clear()
 108              }),
 109            )
 110            return { runners }
 111          }),
 112        )
 113  
 114        const getRunner = (runners: Map<string, Runner<MessageV2.WithParts>>, sessionID: SessionID) => {
 115          const existing = runners.get(sessionID)
 116          if (existing) return existing
 117          const runner = Runner.make<MessageV2.WithParts>(scope, {
 118            onIdle: Effect.gen(function* () {
 119              runners.delete(sessionID)
 120              yield* status.set(sessionID, { type: "idle" })
 121            }),
 122            onBusy: status.set(sessionID, { type: "busy" }),
 123            onInterrupt: lastAssistant(sessionID),
 124            busy: () => {
 125              throw new Session.BusyError(sessionID)
 126            },
 127          })
 128          runners.set(sessionID, runner)
 129          return runner
 130        }
--- end snippet ---

Why this matters: they maintain a session-scoped Runner pool that manages concurrency, lifecycle hooks (onIdle/onBusy/onInterrupt), and cancellation — a clean abstraction for long-running session execution and resource cleanup.

14) Suggested POCs (short)

POC 1: Persistent shell in ragent
- Implement a Rust equivalent of their Runner + PTY persistent shell (spawn PTY process, stream, exec API). Scope: create crates/ragent-core/tools/persistent_shell.rs and a CLI flag to enable session reuse. Time: 2-3 weeks.

POC 2: OAuth + CLI API-key minting flow
- Prototype PKCE-based OAuth with a local callback server and provider-side API-key creation. Scope: implement a small server in ragent-server and a CLI flow in ragent-tui. Time: 2-4 weeks.

POC 3: Structured diff + pre-apply approval UI
- Implement structured diff preview in the TUI and require explicit approval before applying file edits. Scope: implement diff rendering and confirm modal in ragent-tui; back-end apply_patch tool in ragent-core. Time: 1-2 weeks.

15) Final recommendations

- Adopt the persistent shell and session-runner abstractions early — they noticeably improve reliability of multi-step shell workflows and debugging.
- Implement a low-friction OAuth/key-minting onboarding to reduce setup friction for new users.
- Add per-project trust prompts and default read-only agent modes for safe exploration.
- Consider porting select tooling ideas (structured output enforcement, VCR-style replay for tests, ripgrep-based search) where they align with ragent's architecture.

End of OpenCode analysis.
