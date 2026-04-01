ragent — Product review

Summary

ragent is a terminal-first AI coding agent implemented in Rust. It is delivered as a single binary (workspace of Rust crates) that provides:

- A full-screen TUI (ratatui/crossterm) for interactive agent workflows
- An Axum-based HTTP server with REST + SSE to drive the agent programmatically
- A provider abstraction to support multiple LLM providers (Anthropic, OpenAI, Copilot, Ollama, generic OpenAI)
- A rich tool system (file ops, shell, grep, multiedit, office/PDF helpers, plan delegation, team messaging)
- Session persistence using SQLite and a session/agent processing loop
- Teams/sub-agents primitives (spawn/claim/complete tasks) and a task manager for background work
- Prompt optimization utilities (12+ frameworks) usable from TUI and HTTP

Core evidence: README.md (features list), Cargo.toml (workspace + deps), src/main.rs (CLI entrypoint: run/serve/session/auth/models)

High-level features and user flows

1. Interactive TUI flow
   - Launch: `ragent` or `ragent` with no subcommand opens the TUI (src/main.rs:331-371)
   - Provider setup: interactive provider/auth dialogs from TUI (docs + provider registry)
   - Slash commands: `/opt`, `/team`, `/agents`, `/agent` and other built-in slash commands (README features)
   - Streaming chat & tool execution with step-numbered tool logs (README lines ~22-24)

2. Headless run flow
   - `ragent run "<prompt>"` executes a single prompt against a resolved agent and prints a result (src/main.rs:373-392)

3. Server flow (programmatic)
   - `ragent serve --addr <addr>` starts an Axum HTTP server, exposes REST + SSE endpoints, and creates an AppState (src/main.rs:394-408; ragent-server crate)

4. Session management
   - `ragent session list|resume|export|import` — persistent conversation history in SQLite (src/main.rs:410-484; storage in ragent-core/storage)

5. Team / background agents
   - Team creation and teammate spawn flows accessible via TUI and programmatic APIs (team tools in core and team_* functions in tools)

Architecture, tech stack, modules, and boundaries

- Language & packaging: Rust 2024 edition, Cargo workspace with crates/*
  - Cargo.toml: workspace members (crates/ragent-core, ragent-server, ragent-tui, ragent-code, prompt_opt) (Cargo.toml: lines 1-4, 30-34)
- Crate responsibilities (README and repo layout):
  - crates/ragent-core: core types, config, providers, tools, session processor, storage, event bus (README: lines 211-217)
  - crates/ragent-server: axum HTTP routes, SSE streaming (README: 215-217)
  - crates/ragent-tui: ratatui-based terminal UI, tracing bridge for log panel (README: 215-217, src/main.rs:209-220)
  - crates/ragent-code, prompt_opt: utilities for code/tooling & prompt optimization
- Key runtime components and boundaries:
  - CLI entrypoint (binary) at src/main.rs wires together provider registry, tool registry, permission checker, session manager, and session processor (src/main.rs:17-26, 225-321)
  - EventBus (ragent-core) is the pub/sub for TUI, server and processor (src/main.rs:254-256)
  - SessionProcessor (ragent-core::session::processor) handles the agent loop → LLM calls → tool execution (src/main.rs:311-320)
  - Provider registry & tool registry: created at startup and passed to SessionProcessor (src/main.rs:257-260, 311-315)
  - PermissionChecker guards file writes and shell commands via configurable rules (src/main.rs:260)
  - TaskManager (ragent_core::task::TaskManager) implements background agents and is wired via OnceLock into SessionProcessor (src/main.rs:322-328)

Public APIs, CLI surface and extension points

- CLI commands (src/main.rs):
  - run (one-shot prompt) — src/main.rs:116-123, 373-392
  - serve (start HTTP server) — src/main.rs:121-126, 394-409
  - session (list/resume/export/import) — src/main.rs:129-175, 411-484
  - auth, models, config — src/main.rs:134-153, 485-515
- HTTP API and SSE (ragent-server crate):
  - AppState constructed in src/main.rs:399-406 and passed to ragent_server::start_server (src/main.rs:406-409). See crates/ragent-server/src/routes.rs for endpoints and handlers (not exhaustively listed here).
- Extension points and plugin hooks:
  - Provider registry (ragent_core::provider::create_default_registry) — add new LLM providers via trait impls (src/main.rs:257-259)
  - Tool registry (ragent_core::tool::create_default_registry) — register additional tools (src/main.rs:258-259)
  - Agent presets loaded from project `AGENTS.md` → custom agents support (README.md: lines 31-51, 135-151)
  - Sub-agent API: new_task / cancel_task / list_tasks — spawn/coordinate teammates (README.md: lines 20-21, team tools in ragent-core task manager)
  - Prompt optimization crate (prompt_opt) exposes HTTP endpoint POST /opt (README.md: lines 153-187)

Build, test, CI tooling and developer commands

- Local build & run commands (README.md and AGENTS.md):
  - cargo build (debug) — standard workspace build
  - cargo build --release — release binary (Cargo.toml [[bin]] points at src/main.rs)
  - cargo test — run all tests; crates contain unit and integration tests (tests/ directories under crates)
  - cargo check, cargo fmt, cargo clippy
- CI config present:
  - .github/workflows/ci.yml — workspace `cargo test --workspace` and other jobs (exists at repo root)
  - .github/workflows/ci_benchmarks.yml — benchmark CI
- Lint/format config: rustfmt.toml (root), workspace lints in Cargo.toml (lines 18-28)

Developer workflows and docs

- README.md contains Quick Start, Usage and Configuration (including ragent.json example) and describes custom agents, teams, and prompt optimization (README.md: lines 43-88, 112-133, 135-151, 153-187)
- AGENTS.md includes development conventions and agent guidelines. Docs in docs/ include how-tos, plans, userdocs and reports. Example code in examples/ and example agents in examples/agents/
- COMPLIANCE.md, SECURITY_FINDINGS.md, TEST_COVERAGE.md per-crate provide audit findings and recommended remediation (crates/*/COMPLIANCE.md and crates/*/SECURITY_FINDINGS.md)

Tests, examples and quality

- Tests are present across crates: many focused unit tests and integration tests exist (see crates/*/tests/). Example files: crates/ragent-core/tests/, crates/ragent-tui/tests/, crates/ragent-server/tests/.
- Benchmarks are present (criterion) under crates/ragent-tui/benches/ and others.
- Some per-crate compliance/test finding docs note coverage gaps and flaky tests (crates/ragent-server/COMPLIANCE.md, crates/ragent-tui/test_findings.md).
- A repo-level CI workflow exists (.github/workflows/ci.yml) which runs `cargo test --workspace`, so tests are executed on CI.

Current limitations, known bugs, TODOs and missing/incomplete features

- MCP (Model Context Protocol) support is a stub / in progress (README.md: line 33-34). Implementation incomplete.
- Secret masking: docs and security findings warn about potential accidental secret logging in the TUI log panel — recommended CI grep lint and unit tests are present in findings (crates/ragent-tui/security_findings.md).
- Server HTTP/SSE E2E tests coverage: ragent-server recommends adding more end-to-end tests for SSE and handlers (crates/ragent-server/COMPLIANCE.md). While in-process tests exist, full integration coverage is limited.
- Docker/containerization: some analysis docs previously noted missing top-level Dockerfile; repo does have .github workflows but lacks a recommended Dockerfile for server image (product_gaps.md and analysis_product.json).
- LSP & some advanced integrations: there are LSP manager placeholders using OnceLock (src/main.rs:317-319) but full LSP integration may be incomplete.
- Plugin/extension documentation: provider/tool registries exist but there is limited explicit public-facing documentation describing how to author and package plugins.

Prioritized gap list (short)

Note: each gap below shows suggested component(s) to change and an estimate in person-days.

1) CI: add full workspace CI with coverage reporting (HIGH)
   - Description: Add code coverage (grcov/tarpaulin), cargo-audit, and failing jobs for fmt/clippy to .github/workflows and require passing on PRs.
   - Why it matters: CI ensures regressions are caught and dependency vulnerabilities reported.
   - Complexity: M
   - Components: .github/workflows, CI scripts, repo root
   - Estimate: 2-4 person-days

2) Secret masking + CI linting (HIGH)
   - Description: Implement mask_secret helper usage audit, unit tests, and a CI grep-based lint job to detect accidental logging of keys/tokens.
   - Why: Prevents credential leakage in TUI logs and CI artifacts.
   - Complexity: S-M
   - Components: crates/ragent-tui, ragent-core sanitize helpers, CI workflows
   - Estimate: 1-3 person-days

3) Server E2E and SSE integration tests (MEDIUM)
   - Description: Add end-to-end in-process router tests and SSE streaming tests (automated) and ensure CI runs them.
   - Why: Prevent regressions in server endpoints that frontends depend on.
   - Complexity: M
   - Components: crates/ragent-server/tests, .github/workflows
   - Estimate: 3-5 person-days

4) Docker image + release build (MEDIUM)
   - Description: Add Dockerfile(s) for server and TUI/headless modes and CI job to build and smoke-test images.
   - Why: Easier deployment and reproducible server environments.
   - Complexity: M
   - Components: repo root, ./docker, .github workflows
   - Estimate: 2-4 person-days

5) Plugin documentation & example authoring guide (MEDIUM)
   - Description: Add docs and examples describing how to add providers and tools (trait impls), and publishable plugin layout.
   - Why: Lowers barrier for third-party integrations and contributors.
   - Complexity: M
   - Components: docs/, crates/ragent-core provider/tool registry code
   - Estimate: 2-3 person-days

6) Autocompletion & CLI UX polish (LOW-MEDIUM)
   - Description: Improve CLI UX (shell completion, help text polish, interactive autocomplete for slash-commands and agent names).
   - Why: Developer ergonomics and adoption.
   - Complexity: S-M
   - Components: clap configuration in src/main.rs, ragent-tui slash command UI
   - Estimate: 2-4 person-days

7) Sourcemap / debugging support for code tools (LOW-MEDIUM)
   - Description: Add sourcemap parsing support for tools that run code or map transformations; integrate with code viewer.
   - Why: Better developer debugging of transformations; competitive feature parity.
   - Complexity: M
   - Components: crates/ragent-code, tools that execute/format code
   - Estimate: 3-6 person-days

8) Offline/latency optimizations & optional local LLM caching (MEDIUM-LARGE)
   - Description: Add configurable local cache or short-circuit for model responses, and improve offline resilience for tools.
   - Why: Lower latency,better reliability for local devs and CI.
   - Complexity: L
   - Components: ragent-core provider abstractions, storage, optional local LLM adapters
   - Estimate: 8-20 person-days

Tests & examples assessment

- Good unit test coverage in many crates (ragent-tui has focused tests for parsing/markdown and clipboard handling; ragent-core contains many reference and concurrency tests). See crates/ragent-tui/tests/* and crates/ragent-core/tests/*.
- Test docs indicate gaps around secret masking, CWD-dependent tests (detect_git_branch) and external dependency reliant tests (Ollama). See crates/ragent-tui/test_findings.md (lines referenced in repo search).
- repo-level CI exists and runs cargo test --workspace (.github/workflows/ci.yml) but coverage upload and advanced CI checks (cargo-audit, grcov) are not configured.

Security and licensing notes

- License: MIT (LICENSE at repo root)
- Security findings exist per crate (crates/ragent-tui/security_findings.md, crates/ragent-server/COMPLIANCE.md). Key actionable items: add secret masking enforcement, CI lint to detect logging of secrets, run cargo-audit in CI (crates/ragent-server/COMPLIANCE.md lines ~143-160).
- Rust lints: workspace lints configured in Cargo.toml (unwrap/expect use warnings; deny unsafe_code) (Cargo.toml: lines 18-28)

References (selected)

- README.md — product features, quick start, architecture and usage (README.md lines ~12-46, 211-241)
- Cargo.toml — workspace deps and crates (Cargo.toml lines 1-36, 41-89, 132-142)
- src/main.rs — CLI, startup wiring, TUI/server/run flows (src/main.rs:73-116, 189-223, 330-371, 394-409)
- crates/ragent-tui/test_findings.md — test gaps and secret logging concerns
- crates/ragent-server/COMPLIANCE.md — recommended server integration tests and CI items
- .github/workflows/ci.yml and ci_benchmarks.yml — CI workflows
- LICENSE — MIT

Concluding notes

ragent is a mature, feature-rich Rust workspace implementing a local-first agent with TUI and server surfaces. The architecture is modular and well separated (core/server/tui). Most of the remaining work is around hardening (CI, secret handling), packaging (Docker/CI images), and a few advanced features (MCP completion, LSP, offline/local caching). The repo includes many of the building blocks (provider/tool registries, task manager, event bus) that make adding features straightforward.

If you want, I can now produce a targeted plan (files to edit, specific PR checklist) for the top 3 gaps (CI coverage + cargo-audit, secret masking + CI lint, server E2E tests).
