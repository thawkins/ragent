---
status: draft
audit:
  - { time: 1791630619, from: "none", to: "draft", actor: "system" }
---
# Specification: OpenHands Parity - Closing the Major Feature Gaps Between ragent and OpenHands

## Overview

OpenHands (the "Agent Canvas" control centre plus the OpenHands Agent Server / Software
Agent SDK and the OpenHands CLI) is a self-hosted developer control centre for coding
agents. ragent is a single-binary, terminal-first coding agent. Both drive an agent
loop over a multi-provider LLM backend with a tool surface, an MCP client, a skills
system, a permission system, and a REST/SSE server. The two overlap heavily, but a
small set of **structural** capabilities exist in OpenHands that ragent does not have.

This specification records the **major feature differences** found by a deep inspection
of the OpenHands repositories (`OpenHands/OpenHands`, `OpenHands/software-agent-sdk`,
`OpenHands/OpenHands-CLI`, `OpenHands/automation`) and defines the **delta** - the
features ragent must add to reach functional parity. It is deliberately scoped to
capabilities that are architecturally distinct from what ragent already ships; it does
not restate parity that already exists (teams, swarms, memory, compaction, MCP stdio,
YAML skills, spec management, cron tools, multi-provider LLM support, TUI, REST/SSE).

The delta has six load-bearing themes:

1. **Sandboxed / pluggable execution backends** - OpenHands can run the agent inside a
   Docker container, on a remote VM, or on cloud infrastructure, selected per
   conversation; ragent always executes tools directly on the host.
2. **Multiple / remote agent backends** - OpenHands (Agent Canvas) connects to one or
   more Agent Servers and switches between them from one UI; ragent has only an
   in-process orchestrator and no remote backend registry.
3. **ACP external-agent integration** - OpenHands drives Claude Code, Codex, and Gemini
   CLI as ACP subprocesses over JSON-RPC on stdio; ragent has no Agent Client Protocol
   support.
4. **OpenAI-compatible inbound API** - the OpenHands Agent Server exposes a REST /
   WebSocket API with OpenAI-compatible endpoints; ragent's server speaks only its own
   bespoke REST+SSE dialect.
5. **Automation service** - OpenHands pairs the Agent Server with an Automation Server
   that runs agents on a schedule or from webhooks, integrating Slack, GitHub, Linear,
   and Notion, with run history; ragent has in-process cron tools only.
6. **AgentSkills directory convention and tooling** - OpenHands loads `.agents/skills/`
   `SKILL.md` packs and a public skill marketplace; ragent ships its own YAML skill
   packs and does not read the AgentSkills convention.

Supporting gaps that travel with the six themes are also specified: LLM-based security
analysis, an OpenAPI contract with a generated typed client, an opt-in headless browser
capability, an ACP server endpoint for IDE clients, HTTP MCP transport

### Why this exists

ragent competes on being a fast, dependency-free, terminal-native agent. That advantage
is real, but it comes with a hard ceiling: the agent runs with the full privileges of
the user on the machine it was launched from, it cannot be pointed at a remote or
sandboxed execution host, and it cannot be driven by the OpenAI-compatible clients and
IDE integrations that the wider ecosystem already speaks. A user who wants "run this
against my repo, but in a throwaway container" or "drive the agent from my existing
OpenAI tooling" or "run a nightly dependency-update job" cannot do any of those with
ragent today. Closing these gaps keeps ragent's single-binary deployability while
letting it plug into the same execution, automation, and interoperability surfaces that
OpenHands exposes.

## Feature difference analysis (ragent vs OpenHands)

| #  | Capability                       | OpenHands                                                         | ragent today                                          | Verdict                          |
| -- | -------------------------------- | ----------------------------------------------------------------- | ----------------------------------------------------- | -------------------------------- |
| 1  | Execution isolation              | Docker sandbox, remote runtime, cloud, per-conversation container | Direct host execution; 7-layer bash validation only   | **Missing - add**          |
| 2  | Remote / multiple agent backends | Agent Canvas connects to many Agent Servers, switches per backend | In-process orchestrator only; no remote registry      | **Missing - add**          |
| 3  | External agent protocol (ACP)    | Claude Code, Codex, Gemini CLI over ACP stdio JSON-RPC            | None (plugin dialects, not ACP)                       | **Missing - add**          |
| 4  | OpenAI-compatible inbound API    | REST/WebSocket + OpenAI-compatible endpoints                      | Bespoke REST+SSE only                                 | **Missing - add**          |
| 5  | Automation server                | Schedule + webhook runs, Slack/GitHub/Linear/Notion, run history  | Cron tools + MCP-notification triggers only           | **Missing - add**          |
| 6  | Skills convention                | `.agents/skills/` `SKILL.md` + public marketplace             | Own YAML packs + plugin skills                        | **Missing - add**          |
| 7  | LLM security analyzer            | `--llm-approve`, `SecurityAnalyzer` interface                 | Static permission rules + 7-layer bash                | **Missing - add**          |
| 8  | API contract / typed client      | OpenAPI contract + TypeScript client                              | Undocumented bespoke API                              | **Missing - add**          |
| 9  | Browser / screenshot             | Browser + editor (OpenVSCode) control                             | `browser` tool removed; `mf_screenshot` is a stub | **Missing - add (opt-in)** |
| 10 | IDE integration                  | `openhands acp` for Zed/VSCode/JetBrains                        | None (TUI only)                                       | **Missing - add**          |
| 11 | MCP HTTP transport               | stdio + HTTP                                                      | stdio only                                            | **Missing - add**          |
| 12 | i18n                             | Frontend translated to many locales                               | English only                                          | **Ignore**                 |
| 13 | Multi-provider LLM               | Yes                                                               | Yes                                                   | Parity - no change               |
| 14 | MCP stdio client                 | Yes                                                               | Yes                                                   | Parity - no change               |
| 15 | Memory / compaction              | Yes                                                               | Yes                                                   | Parity - no change               |
| 16 | Sub-agents / teams / swarms      | Yes                                                               | Yes (extensive)                                       | Parity - no change               |
| 17 | Skills (own format)              | Yes                                                               | Yes                                                   | Parity - no change               |
| 18 | Permission / approval modes      | Yes (`--yolo`, `--llm-approve`)                               | Yes (`--yes`/YOLO; static rules)                    | Partial - see#7                  |
| 19 | Headless one-shot mode           | `--headless -t`                                                 | `ragent run "<prompt>" --no-tui`                    | Parity - no change               |
| 20 | Session persistence / resume     | Yes (per-container state)                                         | Yes (SQLite)                                          | Parity - no change               |

Capabilities 13-20 are listed to show they are **not** part of this spec; the plan does
not touch them.

## Assumptions and interpretation (read before implementing)

- **A1 - Backend is a selection, not a rewrite.** The execution backend selects **where**
  a tool runs. The agent loop, LLM providers, tool registry, permission system, and
  event bus are unchanged; only the tool-execution leaf is dispatched through a backend
  trait. `local` is a thin adapter over today's behaviour.
- **A2 - `local` is the default and the fallback target only where explicitly allowed.**
  When no backend is configured the system resolves `local` (FR-019). A **failure** to
  provision a non-local backend does not fall back to `local` (FR-031).
- **A3 - Security layers are backend-independent.** The 7-layer bash validation, the
  file-path containment guard, the permission system, and the always-allowed codeindex
  hardwiring apply in full regardless of backend (FR-002, FR-037).
- **A4 - The native REST+SSE API stays canonical.** The OpenAI-compatible surface is an
  adapter over `SessionProcessor`; it does not replace or fork the native API, and it
  inherits the same bearer-token auth (FR-024, FR-038).
- **A5 - ACP is a client and a server, not a dialect.** ragent acts as an ACP **client**
  (driving Claude Code / Codex / Gemini) and optionally as an ACP **server** (serving
  IDE clients). This is distinct from the existing Codex/Claude plugin-dialect support.
- **A6 - Automations reuse cron and orchestrator primitives.** The automation service
  builds on the existing in-process cron tool family and the orchestrator, adding a
  webhook ingress, a durable run history, and third-party dispatch; it does not replace
  the cron tools.
- **A7 - AgentSkills is additive and compatible.** `.agents/skills/` packs load alongside
  the existing YAML packs; a name clash resolves in favour of the existing pack, mirroring
  the plugin/agent clash rules.
- **A8 - Secrets reach sandboxes by reference.** Container and remote backends receive
  credentials resolved from the encrypted credential store at spawn time (env var or
  materialised file), never baked into an image or committed to a workspace (FR-010,
  FR-035).
- **A9 - The browser capability is opt-in and off by default.** It is only enabled where
  a real headless engine is present; the current always-erroring `mf_screenshot` stub is
  not that (FR-029).
- **A10 - No new mandatory runtime dependency.** Docker is required only when the `docker`
  backend is selected; ACP requires the external CLI only when an ACP agent is selected;
  the default build keeps zero runtime dependencies.

## Definitions

| Term                                | Meaning                                                                                                                            |
| ----------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| **Execution backend**         | The environment a tool invocation runs in:`local`, `docker`, or `remote`.                                                    |
| **Backend registry**          | The durable list of configured execution backends and remote agent servers, each with id, kind, connection descriptor, and health. |
| **Remote backend**            | A ragent server (usually on another host) addressed over its REST+SSE API.                                                         |
| **ACP**                       | Agent Client Protocol: JSON-RPC over stdio used to drive an external coding agent.                                                 |
| **ACP agent**                 | An external agent (Claude Code, Codex, Gemini CLI, or a custom ACP server) driven over ACP.                                        |
| **Automation**                | A named, schedulable or webhook-triggered agent run with a stored definition and run history.                                      |
| **Run history**               | The durable record of automation runs: trigger, start/end, status, backend, and output reference.                                  |
| **AgentSkills pack**          | A directory containing a`SKILL.md` with YAML frontmatter, loadable by name.                                                      |
| **Security analyzer mode**    | A permission mode that evaluates each proposed tool action (allow / ask / deny) with an LLM or policy engine before execution.     |
| **OpenAI-compatible surface** | The`POST /v1/chat/completions` and `GET /v1/models` endpoints that speak the OpenAI request/response dialect.                  |

## Background - existing ragent machinery to reuse

- **Tool execution** - `crates/ragent-tools-core` (`bash.rs` 7-layer validation,
  file tools) and `crates/ragent-tools-extended`; the new backend trait wraps the leaf
  dispatch of these tools.
- **Session loop** - `crates/ragent-agent/src/session/` (`processor.rs`,
  `loop_capture.rs`, `history.rs`) is the turn engine the OpenAI-compatible surface and
  the ACP client both bridge to.
- **HTTP server** - `crates/ragent-server/src/routes/mod.rs` (axum router, bearer-token
  `auth_middleware`, `AppState`, `/events` SSE stream, rate limiting, body/timeout
  limits) is where the OpenAI-compatible and OpenAPI surfaces attach.
- **Agent registry / orchestrator** - `crates/ragent-agent/src/orchestrator/`
  (`registry.rs`, `coordinator.rs`) and `crates/ragent-agent/src/background/` seed the
  backend registry and the automation dispatcher.
- **Cron / triggers** - `crates/ragent-agent/src/tool/cron.rs`,
  `crates/ragent-agent/src/trigger/` (`dynamic.rs`, `mcp_notification.rs`, `runtime.rs`)
  are the primitives the automation service extends.
- **Skills** - `crates/ragent-agent/src/skill/` (`loader.rs`, `bundled.rs`,
  `invoke.rs`, `context.rs`) is where AgentSkills discovery is added.
- **Permissions / hooks** - `crates/ragent-config/src/permission.rs` (`Permission`,
  `PermissionAction`, `PermissionRule`, `PermissionChecker`) and `Config.hooks` are the
  hooks the security analyzer and `PreToolUse` semantics extend.
- **Credentials** - `crates/ragent-storage` (encrypted credential store) is the source
  for sandbox secret injection.
- **Plugins / MCP** - `crates/ragent-plugins` and `crates/ragent-agent/src/mcp/` provide
  the manifest and transport patterns that HTTP MCP transport follows.

## Requirements

### Ubiquitous requirements

**FR-001** - The system shall support pluggable execution backends selected from at least
`local`, `docker`, `podman` and `remote`, where `local`executes tools on the host as today and`docker`/`remote` execute them in a container or on a remote server respectively.

**FR-002** - The system shall keep the 7-layer bash validation, the file-path containment
guard, the permission system, and the always-allowed codeindex hardwiring active for every
tool invocation regardless of the active execution backend.

**FR-003** - The system shall expose an OpenAI-compatible inbound HTTP surface comprising
`GET /v1/models` and `POST /v1/chat/completions` that bridges a request to the existing
`SessionProcessor`.

**FR-004** - The system shall expose each configured execution backend in a durable
backend registry entry carrying a stable id, a display name, a kind (`local`, `docker`, `podman` or `remote`), a connection descriptor, and a health state.

**FR-005** - The system shall discover and load AgentSkills-format packs from the project
directory `.agents/skills/` and the user directory `~/.agents/skills/`, where each pack is
a directory containing a `SKILL.md` with YAML frontmatter declaring at least `name` and
`description`.

**FR-006** - The system shall provide an LLM-based security-analyzer permission mode that
evaluates each proposed tool action and returns a verdict of allow, ask, or deny with a
rationale before the action is permitted or refused.

**FR-007** - The system shall generate an OpenAPI 3.1 document that describes the server
REST API and is derived from the live router definition so the document cannot drift from
the served routes.

**FR-008** - The system shall surface, in the TUI, the active execution backend and the
health of every registered backend, and shall offer a command to switch the active backend.

### Event-driven requirements

**FR-009** - When a session starts against an execution backend other than `local`, the
system shall provision that backend's workspace and make the project available to it
before the first tool call is dispatched.

**FR-010** - When a container or remote backend is provisioned, the system shall resolve
the credentials it needs from the encrypted credential store and inject them as environment
variables or materialised files rather than baking them into the workspace.

**FR-011** - When the OpenAI-compatible surface receives a `POST /v1/chat/completions`
request whose body sets `stream` to `false`, the system shall run the turn to completion
and return an OpenAI-shaped JSON completion object.

**FR-012** - When the OpenAI-compatible surface receives a `POST /v1/chat/completions`
request whose body sets `stream` to `true`, the system shall stream `text/event-stream`
chunks in the OpenAI `chat.completion.chunk` shape and terminate the stream with a
`data: [DONE]` sentinel.

**FR-013** - When the automation webhook ingress receives a request that matches a
registered automation's trigger, the system shall enqueue an agent run for that automation
with the request payload supplied as prompt context.

**FR-014** - When the scheduler reaches a registered automation's due time, the system
shall enqueue that automation's agent run.

**FR-015** - When a turn is dispatched to an ACP agent, the system shall spawn the ACP
agent's command as a subprocess and relay the turn over JSON-RPC on the subprocess's
standard input and output.

**FR-016** - When a `PreToolUse` hook exits with code 2, the system shall abort the
proposed action and return the hook's stderr as the tool result so the model can correct
course; when the hook exits with code 1, the system shall warn and continue.

**FR-017** - When a tool action is proposed while security-analyzer mode is active, the
system shall publish the analyzer's verdict and rationale before the action is permitted or
refused.

**FR-018** - When an automation run reaches a terminal state, the system shall append a run
history record capturing the automation id, trigger kind, start and end times, outcome,
selected backend, and a reference to the run output.

### State-driven requirements

**FR-019** - While no execution backend is explicitly configured, the system shall resolve
the active backend to `local`.

**FR-020** - While the active execution backend is a container, the system shall route
every shell, file, and search tool invocation into that container's mounted workspace and
shall not read or write the host working directory for those invocations.

**FR-021** - While the active execution backend is `remote`, the system shall drive the
agent through the remote server's REST+SSE API and shall mirror the remote event stream
into the local TUI.

**FR-022** - While an ACP agent is the active agent kind, the system shall render that
session's streamed updates in the TUI and shall not route turns to a local ragent LLM
provider.

**FR-023** - While a container backend holds a conversation, the system shall persist that
conversation's workspace and history so both survive replacement of the container.

**FR-024** - While the OpenAI-compatible surface is enabled, the system shall require the
same bearer-token authentication as the native REST API for every request to it.

### Optional requirements

**FR-025** - Where an MCP server is configured with an `http` transport, the system shall
connect to it over HTTP in addition to the existing `stdio` transport.

**FR-026** - Where a container runtime is detected on the host, the system shall offer
`docker` or `podman` as a selectable execution backend, and shall not require a container runtime when
one is absent. The default container backend will be `podman` if none is specified or detected. 

**FR-027** - Where UI internationalisation is enabled with a configured locale, the system
shall render user-facing strings from that locale's message catalog and shall fall back to
English for any missing key. 

**FR-028** - Where the IDE-integration feature is enabled, the system shall expose an ACP
server endpoint on stdio that an ACP-capable editor client (Zed, VS Code, or JetBrains) can
attach to.

**FR-029** - Where the browser capability is enabled and a real headless engine is present,
the system shall provide a browser/screenshot tool backed by that engine; the capability
shall be enabled by default and shall never be advertised when no engine is present.

**FR-030** - Where a remote backend is configured with a base URL and an API key, the
system shall register it as a reachable backend in the backend registry.

### Unwanted requirements

**FR-031** - If a non-local execution backend fails to provision, then the system shall not
fall back to host execution; it shall fail the turn and surface the provisioning error.

**FR-032** - If the OpenAI-compatible surface is enabled without a configured authentication
token, then the system shall refuse to start that surface rather than serve it
unauthenticated.

**FR-033** - The system shall not execute any part of an automation run outside the
automation's configured execution backend.

**FR-034** - If a remote backend becomes unreachable part-way through a turn, then the
system shall surface the failure in the TUI and shall not re-execute that turn's tools
locally.

**FR-035** - The system shall not expose the host working directory to a container
backend's tool invocations, and shall not emit secret values in the OpenAPI document, the
`/config` response, or any OpenAI-compatible response.

**FR-036** - If an ACP subprocess exits non-zero or emits malformed JSON-RPC, then the
system shall mark the turn failed and shall not block indefinitely waiting for further
output.

**FR-037** - The system shall not weaken the existing 7-layer bash validation, file-path
containment, or permission system when an execution backend, the OpenAI-compatible
surface, ACP support, or the automation service is added.

**FR-038** - The OpenAI-compatible surface shall not bypass the permission system; a tool
action that would require confirmation through the native API shall require it through the
OpenAI-compatible surface as well.

## Scope

**In scope**

- An execution-backend abstraction with `local`, `docker`, `podman`and `remote` implementations.
- A durable backend registry with health reporting and a TUI backend switcher.
- An ACP client (drive external agents) and an optional ACP server (serve IDE clients).



- An automation service: webhook ingress, scheduler, run history, and third-party
  dispatch.
- AgentSkills (`.agents/skills/`) discovery with YAML frontmatter.
- An LLM-based security-analyzer permission mode and `PreToolUse` exit-code semantics.
- OpenAPI 3.1 generation from the live router and a generated typed client.
- HTTP MCP transport.
- An opt-in, engine-gated headless browser/screenshot capability.

**Out of scope**

- Changing the LLM provider set, tool registry, memory, compaction, teams, or swarms.
- Multi-tenant hosting, per-user authentication, or role-based access control.
- Hosted cloud offerings or a hosted sandbox service.
- Replacing the native REST+SSE API or the TUI.
- Rewriting the existing bash security model; it is preserved, not replaced.
- Optional UI internationalisation with an English default.

## Acceptance criteria

1. `ragent` with no backend configured runs tools locally exactly as today, and the TUI
   shows `local` as the active backend.
2. Selecting the `docker` or `podman` backend provisions a container, runs a shell tool inside it, and
   the host working directory is unchanged by that tool. The conytainer will have the worktree mapped as volume. 
3. A failing container provision fails the turn with the provisioning error and does not
   execute the tool on the host.
4. A remote backend registered by URL and key appears in the backend registry as reachable
   and can be made active; a turn is driven over the remote API and events appear locally.
5. An ACP agent (Claude Code, Codex, or Gemini CLI) can be selected and a prompt round-trip
   renders in the TUI.
6. `curl` against `POST /v1/chat/completions` with `stream:false` returns a valid
   OpenAI-shaped completion; with `stream:true` it returns `chat.completion.chunk` SSE
   events ending in `data: [DONE]`.
7. The OpenAI-compatible surface, when enabled without a token, refuses to start.
8. A webhook POST to an automation endpoint enqueues a run and a run history record is
   written on completion; a scheduled automation also fires and records history.
9. A `.agents/skills/<name>/SKILL.md` pack is discovered and appears in the skill list.
10. In security-analyzer mode a risky action is asked or denied with a rationale before it
    executes.
11. `GET /openapi.json` returns an OpenAPI 3.1 document that includes every served route.
12. An `http` MCP server connects and exposes its tools.
13. The browser tool is absent from the tool list when no headless engine is present.
14. `cargo fmt --check`, `cargo clippy --workspace`, and `cargo check --workspace` pass
    with all in-scope modules present.

## Open Questions

1. **Backend default for existing users.** A1/A2 fix `local` as the default. If a reviewer
   prefers the backend key to be required when any non-local capability is configured, the
   change is confined to FR-001 and FR-019.
2. **ACP server vs. plugin dialect overlap.** ACP support (FR-015, FR-028) coexists with
   the existing Codex/Claude plugin dialect. If a reviewer wants one unified path, FR-015
   and FR-028 are the requirements to reconcile.
3. **Automation third-party breadth.** FR-013 names Slack, GitHub, Linear, and Notion as
   dispatch targets. If the integration matrix should start narrower (for example GitHub
   only), the change is confined to the automation-integration tasks.
4. **Browser engine dependency.** FR-029 keeps the browser capability optional to protect
   the zero-runtime-dependency default. If a reviewer accepts a bundled headless engine,
   the "off by default" clause is the requirement to revisit.
5. **i18n scope.** FR-027 covers "user-facing strings"; the exact surface (TUI only, or TUI
   plus CLI errors) is left to implementation and may be narrowed.
