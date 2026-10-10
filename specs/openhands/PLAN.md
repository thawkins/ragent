# Implementation Plan: OpenHands Parity - Closing the Major Feature Gaps

**Spec:** [SPEC.md](SPEC.md) | **Test plan:** [TESTPLAN.md](TESTPLAN.md)

## Approach

The delta splits into seven workstreams that are largely independent and can proceed in
parallel once the first two land:

1. **Execution-backend abstraction** (`local` / `docker` / `podman` / `remote`) with a
   health-visible backend registry and a TUI switcher. This is the foundation: every other
   isolation-aware feature (automations, ACP agents) selects a backend from it.
2. **Interoperability surfaces**: an OpenAI-compatible inbound HTTP adapter over
   `SessionProcessor`, an OpenAPI 3.1 document generated from the live router, an HTTP MCP
   transport, and an optional ACP server for IDE clients.
3. **External-agent integration**: an ACP client that drives Claude Code / Codex / Gemini
   CLI (and custom ACP servers) as subprocesses over JSON-RPC on stdio.
4. **Automation service**: a webhook ingress, a scheduler, durable run history, and
   third-party dispatch, built on the existing cron tools and orchestrator.
5. **Skills and permissions**: AgentSkills (`.agents/skills/`) discovery, an LLM-based
   security analyzer, and `PreToolUse` / `PostToolUse` exit-code semantics.
6. **Optional capabilities**: an engine-gated headless browser/screenshot tool (enabled by
   default when a real engine is present, never advertised when no engine is), and UI
   internationalisation with an English default.
7. **Hardening and verification**: preserve the 7-layer bash validation and containment,
   verify no secret leakage, wire the manual test walk, and run the quality gates.

**Guiding constraints.** The agent loop, LLM providers, tool registry, memory, compaction,
teams, and swarms are unchanged; only the tool-execution leaf is dispatched through the
backend trait (A1). `local` is the default and a non-local failure never falls back to
`local` (A2); `podman` is the default container backend when no container runtime is
specified or detected (FR-026). The native REST+SSE API stays canonical and the
OpenAI-compatible surface is an authenticated adapter over it (A4). No new mandatory
runtime dependency is introduced (A10).

**Scope note.** The Scope section marks UI internationalisation as out of scope, while
FR-027 remains a stated (optional) requirement. T-014 / TC-015 therefore stay Low priority
and can be dropped without affecting the mandatory delta.

## Requirement coverage

| Requirement | Tasks |
| ----------- | ----- |
| FR-001 backends (local/docker/podman/remote) | T-001, T-002, T-003 |
| FR-002 security preserved | T-002, T-018 |
| FR-003 OpenAI surface | T-009, T-010 |
| FR-004 backend registry | T-003, T-004 |
| FR-005 AgentSkills packs | T-012, T-013 |
| FR-006 LLM security analyzer | T-015 |
| FR-007 OpenAPI doc | T-011 |
| FR-008 TUI backend surface | T-005 |
| FR-009 provision workspace | T-002, T-003 |
| FR-010 inject secrets | T-006 |
| FR-011 OpenAI non-stream | T-009 |
| FR-012 OpenAI stream | T-010 |
| FR-013 webhook ingress | T-016 |
| FR-014 scheduler | T-016 |
| FR-015 ACP client | T-007 |
| FR-016 hook exit codes | T-015 |
| FR-017 analyzer verdict published | T-015 |
| FR-018 run history | T-016 |
| FR-019 default local | T-001 |
| FR-020 container routes tools | T-002 |
| FR-021 remote drives API | T-003 |
| FR-022 ACP agent renders | T-007, T-008 |
| FR-023 conversation survives container | T-006 |
| FR-024 OpenAI auth | T-010, T-018 |
| FR-025 HTTP MCP | T-011 |
| FR-026 container runtime detection (docker/podman; podman default) | T-002 |
| FR-027 i18n | T-014 |
| FR-028 ACP server | T-008 |
| FR-029 browser engine-gated | T-017 |
| FR-030 remote registration | T-004 |
| FR-031 no fallback on failure | T-002, T-003 |
| FR-032 refuse unauth OpenAI | T-010, T-018 |
| FR-033 automations confined | T-016 |
| FR-034 remote mid-turn failure | T-003 |
| FR-035 no host/workdir or secret leak | T-002, T-018 |
| FR-036 ACP failure handling | T-007 |
| FR-037 no weakening | T-018 |
| FR-038 permissions on OpenAI surface | T-009, T-018 |
| Acceptance 1-14 | T-019 |

## Tasks

| ID | Title | Requirement | Effort | Priority | Status | Dependencies |
|----|-------|-------------|--------|----------|--------|--------------|
| T-001 | Define the execution-backend trait supporting `local`, `docker`, `podman`, and `remote`, and the `local` adapter; add the `execution_backend` config key defaulting to `local` | FR-001, FR-019 | M | Critical | completed | — |
| T-002 | Implement the container execution backend (`docker` and `podman`): container provisioning, workspace volume mount, tool dispatch into the container, container-runtime detection with `podman` as the default when none is specified, and fail-without-fallback on provisioning error | FR-001, FR-002, FR-009, FR-020, FR-026, FR-031, FR-035 | L | Critical | completed | T-001 |
| T-003 | Implement the `remote` backend: drive a ragent server over REST+SSE, mirror the remote event stream into the local event bus, and surface mid-turn unreachability without local re-execution | FR-001, FR-004, FR-009, FR-021, FR-031, FR-034 | L | Critical | completed | T-001 |
| T-004 | Build the durable backend registry (id, name, kind, connection descriptor, health) covering the `local`, `docker`, `podman`, and `remote` kinds, with remote registration by URL and key, plus health probing | FR-004, FR-030 | M | High | completed | T-003 |
| T-005 | TUI backend surface: show the active backend, show each registered backend's health (including `docker` and `podman` kinds), and add a command to switch the active backend | FR-008 | M | High | completed | T-004 |
| T-006 | Sandbox secret injection: resolve container/remote credentials from the encrypted store at spawn time and persist conversation workspace and history across container replacement | FR-010, FR-023 | M | High | completed | T-002 |
| T-007 | ACP client: spawn an ACP agent's command as a subprocess, relay turns over JSON-RPC on stdio, render streamed updates, and fail the turn on non-zero exit or malformed frames | FR-015, FR-022, FR-036 | L | High | completed | — |
| T-008 | ACP server endpoint on stdio for IDE clients (Zed, VS Code, JetBrains), feature-gated and disabled by default | FR-022, FR-028 | M | Medium | completed | T-007 |
| T-009 | OpenAI-compatible surface, non-streaming: `GET /v1/models` and `POST /v1/chat/completions` (stream:false) bridging to `SessionProcessor`, preserving permission checks | FR-003, FR-011, FR-038 | L | Critical | completed | — |
| T-010 | OpenAI-compatible streaming: `chat.completion.chunk` SSE with `data: [DONE]`, bearer-token auth, and refuse-to-start when enabled without a token | FR-012, FR-024, FR-032 | M | High | completed | T-009 |
| T-011 | OpenAPI 3.1 document generated from the live router plus a generated typed client, and HTTP MCP transport alongside stdio | FR-007, FR-025 | M | Medium | completed | — |
| T-012 | AgentSkills discovery: read `.agents/skills/` and `~/.agents/skills/`, parse `SKILL.md` YAML frontmatter (`name`, `description`), and register packs with existing-pack precedence on clash | FR-005 | M | High | completed | — |
| T-013 | AgentSkills tests and TUI `/skill` integration: list, load, and invoke an AgentSkills pack; verify a name clash prefers the existing pack | FR-005 | S | Medium | completed | T-012 |
| T-014 | Optional capabilities: locale message catalog with English fallback, and the i18n wiring for user-facing strings | FR-027 | M | Low | completed | — |
| T-015 | Security analyzer mode: LLM-based allow/ask/deny verdict with rationale published before execution, plus `PreToolUse` exit-code-2 block and exit-code-1 warn semantics | FR-006, FR-016, FR-017 | L | High | completed | — |
| T-016 | Automation service: webhook ingress, scheduler over existing cron primitives, third-party dispatch (Slack/GitHub/Linear/Notion), durable run history, and backend confinement | FR-013, FR-014, FR-018, FR-033 | L | High | completed | T-004 |
| T-017 | Engine-gated browser/screenshot tool: provide the capability when a real headless engine is present (enabled by default per FR-029), and remove or gate the always-erroring `mf_screenshot` stub so the capability is never advertised without an engine | FR-029 | M | Low | completed | — |
| T-018 | Hardening and secret-safety verification: confirm the 7-layer bash validation, containment, and permission system are intact across all new surfaces; verify no secret or host-workdir leakage | FR-002, FR-024, FR-032, FR-035, FR-037, FR-038 | M | Critical | completed | T-002, T-009, T-010 |
| T-019 | Manual acceptance walk of the test plan, update `README.md`/`SPEC.md`/`QUICKSTART.md`/`CHANGELOG.md`, and run the quality gates (`fmt`, `clippy`, `check --workspace`) | Acceptance 1-14 | M | High | completed | T-005, T-006, T-008, T-011, T-013, T-017, T-018 |
## Sequencing notes

- **Wave 1 (foundations, parallel):** T-001, T-007, T-009, T-011, T-012, T-014, T-015,
  T-017 have no dependencies and can start immediately.
- **Wave 2 (depend on Wave 1):** T-002, T-003, T-008, T-010, T-013.
- **Wave 3:** T-004, T-006, T-016 (registry, secret injection, automations).
- **Wave 4:** T-005 (TUI surface), T-018 (hardening), T-019 (acceptance and gates).

## Risks

| Risk | Mitigation |
| ---- | ---------- |
| Container provisioning is host-specific (Docker Desktop, rootless Podman, rootless Docker, WSL). | T-002 detects the runtime and offers `docker`/`podman` only when present; FR-026 keeps it optional and defaults to `podman`. |
| Remote-backend mid-turn failure could silently duplicate work. | FR-034 forbids local re-execution; T-003 surfaces the failure and fails the turn. |
| An OpenAI-compatible surface that bypasses permissions is a security regression. | FR-038 routes through the existing permission system; T-018 verifies it. |
| Secrets reaching containers via images or workspace files. | A8/FR-010 resolve secrets at spawn time by reference; FR-035 forbids leakage. |
| New surfaces weaken the bash/containment model. | A3/FR-002/FR-037 hold the layers constant; T-018 is a dedicated verification task. |
| Optional capabilities (browser, i18n) expand the default build. | A10 keeps them optional; FR-029 gates the browser on a present engine so it is never advertised when it cannot work. |