---
status: draft
audit:
  - { time: 1791500000, from: "none", to: "draft", actor: "system" }
---
# Manual Test Plan: OpenHands Parity - Closing the Major Feature Gaps

**Spec:** [SPEC.md](SPEC.md) | **Plan:** [PLAN.md](PLAN.md)

This is a **manual** test plan. Every case is executed by a human against a built
`ragent` binary. Each case lists the requirements it exercises, its preconditions, the
literal keys to press and data to type, and the expected result. Keys are written as
`Enter`, `Tab`, `Up`, `Down`, `Esc`, `Ctrl+C`, and literal text in `backticks`. Where a
step opens a TUI panel or dialog, every key press and the exact field contents are given.

This document is executed entirely by hand and contains no automated test code.

The new surfaces follow ragent's existing conventions: slash commands open TUI panels,
and the matching CLI subcommand performs the same action headlessly. The cases below use
the documented command names (`/backend`, `/automation`, `/skill`, `/mcp`,
`/security-analyzer`) and the matching CLI subcommands.

## Prerequisites

1. **Build the binary.** Run `cargo build` (debug is sufficient; allow up to 1000
   seconds). Confirm `./target/debug/ragent --version` runs and prints a version.
2. **Configure a model.** Configure a provider (a cloud API key or a local Ollama model)
   and confirm a one-shot prompt responds: `./target/debug/ragent run "say hello" --no-tui`.
3. **Create a scratch root.** Create `~/scratch/openhands-tests/` and run every step from
   inside it. Create a marker file `~/scratch/openhands-tests/HOST_MARKER.txt` containing
   `HOST-ONLY` so the host-working-directory check (TC-003) is measurable.
4. **Container runtime (TC-003, TC-004).** For the container backend cases, have either
   Docker (Docker Desktop or Docker Engine) or Podman running and reachable (`docker info`
   or `podman info` succeeds). Note which runtime is present and use it as `<RUNTIME>`. If
   neither is available, run only the FR-026 half of TC-003.
5. **Second ragent instance (TC-005).** For the `remote` backend case, start a second
   ragent server on another port from a different scratch directory:
   `./target/debug/ragent serve --addr 127.0.0.1:3001`. Record its bearer token from the
   server's startup output. Record the address as `<REMOTE_ADDR>` and the token as
   `<REMOTE_TOKEN>`.
6. **ACP agent CLI (TC-006).** For ACP cases, install at least one of Claude Code, Codex,
   or Gemini CLI, and sign in once so its credential store is populated. If none is
   installed, install a trivial custom ACP echo server and use its command instead.
7. **Webhook target (TC-009).** Have a port on loopback free to receive a webhook (for
   example a shell `nc -l 8080`), or use `curl` to drive the webhook directly. Record the
   running server's base URL as `<BASE>`.
8. **Fixtures (TC-011).** Create `~/scratch/openhands-tests/.agents/skills/tiny/SKILL.md`
   with the contents given in TC-011 before running that case.
9. **Headless engine check (TC-014).** Confirm whether a headless browser engine is
   installed on the host (for example, whether `chromium` or `google-chrome` resolves on
   `PATH`). This determines the expected result of TC-014.

## Test Cases

### TC-001 - Default backend is `local` and the TUI reports it

**Requirement:** FR-001, FR-008, FR-019

**Preconditions:** Fresh scratch root with no `execution_backend` key in
`.ragent/ragent.json` (or no config file at all). No container runtime required.

**Automated coverage:** `crates/ragent-agent/tests/test_backend_registry.rs`
(`registry_always_includes_a_healthy_local_entry`, `registry_entries_are_ordered_local_first`)
covers the always-present, always-`ok` `local` entry and the active-backend
resolution (FR-004, FR-019).

**Steps:**
1. Launch the TUI: `./target/debug/ragent`.
2. Read the status bar at the bottom and the first screen after startup.
3. Type `/backend` and press `Enter` to open the backend panel.
4. Read the panel's active and registry rows.
5. Press `Esc` to close the panel.
6. Exit the TUI with `Ctrl+C`.

**Test data:** `backend` typed after the `/` trigger; no other input.

**Expected results:**
- Step 2 the status bar names `local` as the active execution backend (FR-008).
- Step 4 the `/backend` panel shows `local` as the active row and lists at least that one
  registered backend with health `ok` (FR-004, FR-019).
- Step 6 the process exits cleanly with no panic and no error in the log.

### TC-002 - `/backend` switcher lists backends and switches the active one

**Requirement:** FR-004, FR-008

**Preconditions:** `local` backend active; a container backend configured in
`.ragent/ragent.json` under a `backends` list (see TC-003 for the shape); `<RUNTIME>`
(Docker or Podman) running.

**Steps:**
1. Launch the TUI and type `/backend`; press `Enter`.
2. Read the list rows and each row's health column.
3. Press `Down` to move the cursor to the container row, then press `Enter` to make it
   active.
4. Read the panel's active row and the status bar.
5. Press `Esc`, type a trivial prompt `run echo backend-check`, and press `Enter`.
6. Wait for the tool result and read it.
7. Re-open `/backend`, press `Up` to select `local`, press `Enter`, `Esc`.

**Test data:** `backend`; then `Down`, `Enter`; prompt text `run echo backend-check`.

**Expected results:**
- Step 2 the panel lists `local` and the container backend, each with a health state
  (FR-004).
- Step 3 the container row becomes the active backend and its health is re-reported
  (FR-008).
- Step 4 the status bar names the container backend as active.
- Step 6 the shell command ran inside the container, not on the host (see TC-003 for the
  containment check).
- Step 7 `local` is active again.

### TC-003 - Container backend runs tools in the container and does not touch the host

**Requirement:** FR-001, FR-002, FR-009, FR-020, FR-026, FR-031, FR-035

**Preconditions:** `<RUNTIME>` running. `~/scratch/openhands-tests/HOST_MARKER.txt` exists
with contents `HOST-ONLY`. A container backend is configured, for example in
`.ragent/ragent.json`:

```jsonc
{
  "execution_backend": "podman",
  "backends": [
    { "id": "podman", "name": "Podman sandbox", "kind": "podman",
      "image": "ghcr.io/thawkins/ragent-sandbox:latest", "workspace": "." }
  ]
}
```

Substitute `"docker"` for the `kind` (and the matching top-level value) when only Docker
is present.

**Steps:**
1. Launch the TUI and type `/backend`; press `Enter`. Confirm the container backend is
   active.
2. Press `Esc`, type the prompt below, and press `Enter`:
   `run "pwd; ls -a; cat HOST_MARKER.txt || echo HOST_MARKER_NOT_VISIBLE"`.
3. Wait for the tool output and read it line by line.
4. Type the prompt `run "touch CONTAINER_MARKER.txt"` and press `Enter`.
5. Wait, then in a separate host terminal read
   `~/scratch/openhands-tests/` directory contents.
6. Edit the config to replace `image` with a bogus value
   `ghcr.io/thawkins/ragent-sandbox-does-not-exist:latest`, restart the TUI, select the
   container backend, and type `run "echo hello"`.
7. Read the result.

**Test data:** the two `run ...` prompts exactly as written; the bogus image string.

**Expected results:**
- Step 3 the working directory printed is the container workspace (a container path, for
  example `/projects`), and `HOST_MARKER_NOT_VISIBLE` is printed because the host working
  directory is not used directly for tool invocations (FR-020, FR-035).
- Step 5 `CONTAINER_MARKER.txt` is created inside the container's persisted workspace (the
  worktree is volume-mapped), and the host directory does not gain a new file directly
  (FR-009, FR-023).
- Step 7 the turn fails with the provisioning error and **the command is not executed on
  the host** (FR-031); no `hello` output appears and the error names the missing image.
- If no container runtime is present at all, `/backend` does not offer `docker`/`podman`
  as a selectable kind, and `podman` is the default container kind when one is chosen
  (FR-026).

**Automated coverage:** `crates/ragent-agent/tests/test_container_backend.rs` covers
runtime detection and the `podman` default (FR-026), the fail-without-fallback path
for a bad image (FR-031), the refusal of unadapted tools (FR-031), workspace path
confinement (FR-020, FR-035), and - when a runtime and image are present - the live
run-inside-the-sandbox cases. The 7-layer gate on the container path (FR-002, FR-037)
is additionally pinned hermetically (no runtime needed) by
`crates/ragent-agent/tests/test_hardening_invariants.rs`
(`sandbox_bash_is_gated_by_the_seven_layer_validator`), which proves each of four
validator layers rejects its command before any container is touched and that the
tool never executes on the host.

### TC-004 - Sandbox secret injection does not bake secrets into the workspace

**Requirement:** FR-010, FR-023, FR-035

**Preconditions:** `<RUNTIME>` running; a container backend configured with a
`credentials` list naming `EXAMPLE_API_KEY`; a credential stored
in the encrypted credential store under a name such as `EXAMPLE_API_KEY` with value
`sk-test-do-not-leak-0001`.

**Steps:**
1. Launch the TUI, select the container backend in `/backend`, `Esc`.
2. Type the prompt `run "env | grep -c EXAMPLE_API_KEY"` and press `Enter`.
3. Read the numeric result.
4. Type the prompt `run "grep -r sk-test-do-not-leak-0001 . || echo NOT_IN_WORKSPACE"` and
   press `Enter`.
5. Read the result.
6. Open `/config` in the TUI (or run `./target/debug/ragent config show`) and read the
   output.
7. Open `.ragent/ragent.json` in an editor and read it.
8. Type the prompt `run "touch PERSISTED.txt"`; stop and restart the TUI; select the
   container backend again; type `run "ls PERSISTED.txt"` and read the result.

**Test data:** the three `run ...` prompts; secret value `sk-test-do-not-leak-0001`.

**Expected results:**
- Step 3 the count is `1`: the secret was injected into the container as an environment
  variable (FR-010).
- Step 5 `NOT_IN_WORKSPACE` is printed: the secret is not written into any workspace file
  (FR-010, FR-035).
- Step 6 the `/config` output redacts the secret value (FR-035).
- Step 7 `ragent.json` contains no plaintext secret value (FR-035).
- Step 8 `PERSISTED.txt` is listed after the restart: the workspace volume survives
  container replacement (FR-023).

**Automated coverage:** `crates/ragent-agent/tests/test_sandbox_secrets.rs` covers the
resolution, missing-credential failure, redaction registration, stable-volume/
rotated-container naming, remote store-resolved bearer key, and (when a runtime and
image are present) the live env-var/not-in-workspace and workspace-survival cases.

### TC-005 - `remote` backend drives a second server and surfaces unreachability

**Requirement:** FR-003, FR-004, FR-021, FR-030, FR-034

**Preconditions:** The second ragent server is running at `<REMOTE_ADDR>` with token
`<REMOTE_TOKEN>` (Prerequisite 5). A `remote` backend is configured by URL and key.

**Automated coverage:** `crates/ragent-agent/tests/test_backend_registry.rs`
(`remote_health_requires_a_url_and_a_key`, `register_remote_adds_and_replaces_an_entry`,
`refresh_health_probes_the_remote_health_endpoint`) covers remote registration by URL
and key, the reachable/`unavailable` health states, and the live `/health` probe
(FR-004, FR-030).

**Steps:**
1. Launch the TUI, type `/backend`, press `Enter`. Confirm the `remote` backend row shows
   health `ok` (FR-030).
2. Move the cursor to it with `Down` and press `Enter` to make it active.
3. Press `Esc`, type `say hello from remote`, press `Enter`, and wait.
4. Read the streamed response in the message window and watch the status bar.
5. In the second terminal, stop the remote server (`Ctrl+C`).
6. Type `say hello again`, press `Enter`, and wait.
7. Read the result and the status bar.

**Test data:** prompts `say hello from remote` and `say hello again`.

**Expected results:**
- Step 1 the `remote` backend is registered and reachable from URL + key (FR-004, FR-030).
- Step 4 the reply arrives and the events are mirrored into the local TUI (FR-021).
- Step 7 the turn fails with a connection error, the failure is shown in the TUI, and no
  tool from that turn is executed locally (FR-034).

### TC-006 - ACP agent (external CLI) round-trip

**Requirement:** FR-015, FR-022, FR-036

**Preconditions:** At least one ACP CLI installed and signed in (Prerequisite 6); an ACP
agent selected, for example via Settings -> Agent (`/agent`) with the ACP preset.

**Steps:**
1. Launch the TUI, type `/agent`, press `Enter`.
2. In the picker move the cursor to the ACP agent row (for example `Claude Code`) with
   `Down` and press `Enter`.
3. Press `Esc`, type `what is 2 + 2? explain briefly`, press `Enter`, and wait.
4. Read the streamed response.
5. Now point the ACP command at a deliberately broken command (for example
   `/nonexistent/acp-agent`) in the ACP settings, press `Enter` on the settings save, then
   send `hello`.
6. Read the result.

**Test data:** prompt `what is 2 + 2? explain briefly`; broken command
`/nonexistent/acp-agent`; prompt `hello`.

**Expected results:**
- Step 2 the agent choice is saved and shown as active (FR-022).
- Step 4 the external agent's streamed updates render in the TUI and the turn was relayed
  over JSON-RPC on stdio (FR-015, FR-022).
- Step 6 the turn fails with a clear error, the process does not hang waiting for output,
  and the failure is reported rather than blocking (FR-036).

### TC-007 - OpenAI-compatible surface, non-streaming

**Requirement:** FR-003, FR-011, FR-024, FR-038

**Preconditions:** A ragent server running with the OpenAI-compatible surface enabled and
a bearer token set. Record the base URL as `<BASE>` and the token as `<TOKEN>`.

**Automated coverage:** `crates/ragent-server/tests/test_openai_surface.rs` drives the
live router in-process: `test_models_requires_auth` and
`test_chat_completions_requires_auth` cover the bearer gate (FR-024),
`test_non_streaming_completion_shape` the completion shape (FR-011), and
`test_tool_call_raises_permission_prompt_and_is_not_auto_approved` the
never-auto-approve rule (FR-038).

**Steps:**
1. Run `curl -s <BASE>/v1/models -H "Authorization: Bearer <TOKEN>"` and read the JSON.
2. Run the non-streaming completion request below and read the JSON.
3. Run the same request **without** the `Authorization` header and read the response code
   and body.
4. Run the request with a tool-inducing prompt that would normally require confirmation,
   and observe whether the permission prompt is raised.

```bash
curl -s <BASE>/v1/chat/completions -H "Authorization: Bearer <TOKEN>" \
  -H "Content-Type: application/json" -d '{
    "model": "<MODEL_ID>",
    "stream": false,
    "messages": [{"role": "user", "content": "say hello"}]
  }'
```

**Test data:** the JSON body above; prompt `say hello`; a tool-inducing prompt such as
`delete every file in the workspace`.

**Expected results:**
- Step 1 `/v1/models` returns a JSON list of model ids (FR-003).
- Step 2 the response is a well-formed OpenAI completion object
  (`id`, `object:"chat.completion"`, `choices[0].message.content`, `usage`) (FR-011).
- Step 3 the request is rejected with `401` and no completion is produced (FR-024).
- Step 4 the action still requires confirmation exactly as it would through the native
  API; the surface never auto-approves it (FR-038).

### TC-008 - OpenAI-compatible surface, streaming; and refuse-to-start without a token

**Requirement:** FR-012, FR-024, FR-032

**Preconditions:** As TC-007, with the surface enabled via
`"openai": { "enabled": true, "token": "<TOKEN>" }` (or the ambient
`RAGENT_TOKEN`). The server's bearer token is the configured token.

**Automated coverage:** `crates/ragent-server/tests/test_openai_surface.rs`
(`test_streaming_completion_emits_chunks_and_done`, `test_streaming_requires_auth`)
drives the live router in-process; `crates/ragent-config/tests/test_openai_config.rs`
covers the token gate (`enabled_without_a_token_has_no_resolvable_token`,
`configured_token_is_resolved`, `blank_token_is_not_a_credential`,
`project_overlay_cannot_set_the_surface_token`). The serve-time refuse-to-start
itself (FR-032) is pinned by `tests/test_serve_openai_gate.rs`
(`serve_refuses_to_start_the_surface_when_enabled_without_a_token`): the built
binary is spawned with a trusted config enabling the surface and no token, and
must exit non-zero with an actionable message instead of serving.

**Steps:**
1. Run the streaming completion request below and watch the output arrive incrementally.
2. Read the streamed chunks.
3. Stop the server. Start it again with the OpenAI-compatible surface enabled but with the
   bearer token unset (remove the token config/env var).
4. Read the startup output.

```bash
curl -sN <BASE>/v1/chat/completions -H "Authorization: Bearer <TOKEN>" \
  -H "Content-Type: application/json" -d '{
    "model": "<MODEL_ID>",
    "stream": true,
    "messages": [{"role": "user", "content": "count from one to five"}]
  }'
```

**Test data:** the JSON body above; prompt `count from one to five`.

**Expected results:**
- Step 2 the response is `text/event-stream`; each event is a
  `data: {"object":"chat.completion.chunk", ...}` line, and the stream ends with
  `data: [DONE]` (FR-012).
- Step 4 the server refuses to start the OpenAI-compatible surface and reports why, rather
  than serving it unauthenticated (FR-032).

**Automated coverage:** `tests/test_serve_openai_gate.rs` covers the refuse-to-start
(FR-032) and the positive case (a configured token lets the server start).

### TC-009 - Webhook automation enqueues a run and records history

**Requirement:** FR-013, FR-018, FR-033

**Preconditions:** A ragent server running with the automation service enabled. An
automation named `on-issue` registered with a webhook trigger on path `/auto/on-issue`.
Selected backend: `local`.

**Steps:**
1. In the TUI, type `/automation`, press `Enter`, and read the automation list.
2. Confirm `on-issue` is listed with trigger `webhook`.
3. From a shell, POST a payload to the webhook:
   `curl -s -X POST <BASE>/auto/on-issue -H "Content-Type: application/json" -d '{"issue":"42","title":"fix login"}'`.
4. Read the webhook response code.
5. Re-open `/automation` in the TUI, select `on-issue`, press `Enter` on its `runs` row (or
   run `./target/debug/ragent automation runs on-issue`).
6. Read the newest run history record.

**Test data:** the JSON payload `{"issue":"42","title":"fix login"}`; automation name
`on-issue`.

**Expected results:**
- Step 4 the webhook is accepted (2xx) and enqueues a run with the payload as context
  (FR-013).
- Step 6 a run history record exists with automation id `on-issue`, trigger `webhook`,
  start and end times, outcome, backend `local`, and an output reference (FR-018).
- The run's tools executed in the configured `local` backend and nowhere else (FR-033).

### TC-010 - Scheduled automation fires and records history

**Requirement:** FR-014, FR-018

**Preconditions:** An automation named `nightly` registered with a schedule due within the
next few minutes (for example `every 2m`), backend `local`.

**Steps:**
1. Run `./target/debug/ragent automation list` and read the `next_due` column.
2. Wait until the next-due time passes.
3. Run `./target/debug/ragent automation runs nightly` and read the newest record.
4. Re-run step 3 after another interval and compare timestamps.

**Test data:** automation name `nightly`; schedule `every 2m`.

**Expected results:**
- Step 1 the scheduled automation appears with a concrete next-due time (FR-014).
- Step 3 a new run history record appears per fired interval, each with trigger `schedule`
  and a terminal outcome (FR-014, FR-018).

### TC-011 - AgentSkills pack discovery and invocation

**Requirement:** FR-005

**Preconditions:** `~/scratch/openhands-tests/.agents/skills/tiny/SKILL.md` exists with:

```markdown
---
name: tiny
description: A trivial pack used by the manual test plan.
---

# Tiny skill

When asked, reply with exactly: TINY-OK
```

**Automated coverage:** `crates/ragent-agent/tests/test_agentskills_discovery.rs`
(`test_registry_load_discovers_project_agents_skills_pack`,
`test_agentskills_pack_is_loadable_and_invocable`,
`test_registry_load_agents_skills_clash_prefers_existing_pack`) covers
discovery, load/invoke content, and the clash precedence. The TUI `/skill`
surface is covered by `crates/ragent-tui/tests/test_skill_command.rs`
(`test_skill_list_shows_agentskills_pack`, `test_skill_load_starts_invocation`,
`test_skill_clash_prefers_existing_pack`, `test_skill_unknown_pack_reports_not_found`).

**Steps:**
1. Launch the TUI and type `/skill`; press `Enter`.
2. Read the skill list.
3. Confirm the `tiny` pack with the description above is present.
4. Load it with `/skill tiny` (or the bare `/tiny` trigger); read the confirmation.
5. Type `use the tiny skill and answer`, press `Enter`, and read the reply.
6. Create a second pack `~/.agents/skills/tiny/SKILL.md` whose frontmatter `description`
   differs, restart, and re-open `/skill`.

**Test data:** skill name `tiny`; prompt `use the tiny skill and answer`.

**Expected results:**
- Step 3 `tiny` appears, discovered from the project `.agents/skills/` directory
  (`scope: openskills-project`), with the frontmatter `name` and `description` (FR-005).
- Step 4 loading starts the invocation and reports `invoking skill /tiny...` (FR-005).
- Step 5 the skill's instructions take effect and the reply is `TINY-OK` (FR-005).
- Step 6 the project pack wins the name clash and the user pack does not shadow it (A7).

### TC-012 - Security analyzer mode asks or denies with a rationale

**Requirement:** FR-006, FR-016, FR-017

**Preconditions:** Security-analyzer mode enabled (for example
`/security-analyzer on`, or the matching config key). A model configured for the analyzer.

**Steps:**
1. Launch the TUI and enable security-analyzer mode via `/security-analyzer` then `Enter`
   on the `on` row. Read the confirmation.
2. Press `Esc`, type `run "rm -rf /tmp/openhands-tests-delete-me"`, and press `Enter`.
3. Read the permission prompt and the analyzer rationale shown with it.
4. Choose `Deny` at the prompt.
5. Read the tool result.
6. Now add a `PreToolUse` hook that exits with code 2 for commands containing `curl`, and a
   second hook that exits with code 1 for commands containing `git`. Run
   `run "curl http://example.com"` and then `run "git status"`.
7. Read both results.

**Test data:** prompts as written; hook scripts exiting `2` and `1` respectively.

**Expected results:**
- Step 2 a permission request is raised and the analyzer's verdict and rationale are
  published before the action is permitted or refused (FR-006, FR-017).
- Step 4 the action is denied and the denial is reported to the model.
- Step 6 the `curl` action is aborted and the hook's stderr is returned as the tool result
  (exit code 2 blocks); the `git status` action warns and continues (exit code 1 warns)
  (FR-016).

### TC-013 - OpenAPI document and HTTP MCP transport

**Requirement:** FR-007, FR-025

**Preconditions:** A ragent server running; an MCP server configured with `http` transport.

**Steps:**
1. Run `curl -s <BASE>/openapi.json` and save it to a file.
2. Open the saved file and read `openapi` and the `paths` keys.
3. Compare the `paths` keys with the routes you exercised in TC-007/TC-009.
4. Type `/mcp` in the TUI, press `Enter`, and read the configured servers.
5. Connect the `http`-transport server, then list its tools.

**Test data:** none typed beyond `/mcp`.

**Expected results:**
- Step 2 the document declares `"openapi": "3.1.x"` and a `paths` object (FR-007).
- Step 3 every served route (including `/v1/chat/completions` and the automation webhook)
  appears in `paths`; no served route is missing (FR-007).
- Step 5 the `http` MCP server connects and exposes its tools, exactly as a `stdio` server
  would (FR-025).

### TC-014 - Browser capability is gated on a present engine

**Requirement:** FR-029

**Preconditions:** Note whether a headless engine is installed (Prerequisite 9).

**Steps:**
1. Launch the TUI and type `/tools`; press `Enter`.
2. Read the tool list and search for a browser or screenshot tool.
3. If a headless engine is **not** installed: confirm no browser/screenshot tool is listed,
   and read the result of any attempted call.
4. If a headless engine **is** installed: confirm a browser/screenshot tool is present,
   call it against `https://example.com`, and read the result.

**Test data:** none typed; a URL `https://example.com` only in the engine-present branch.

**Expected results:**
- Step 2/3 when no headless engine is present, **no** browser/screenshot tool is advertised
  (the always-erroring `mf_screenshot` stub is not listed), so the capability is never
  offered when it cannot work (FR-029).
- Step 4 when an engine is present the capability is enabled by default and the tool
  returns a screenshot or page content for the URL (FR-029).

### TC-015 - i18n catalog renders with English fallback

**Requirement:** FR-027

**Preconditions:** A locale message catalog installed (for example a `fr` catalog under the
ragent locale directory) with only a subset of keys translated. Optional: this case may be
skipped if FR-027 is dropped (it is out of scope per the SPEC Scope section).

**Steps:**
1. Launch the TUI with the locale set to the configured locale (for example
   `LANG=fr_FR.UTF-8 ./target/debug/ragent` or the matching config key).
2. Read the TUI status bar, help text, and a panel footer.
3. Find a string that has a translation in the catalog and one that does not.
4. Switch the locale back to English (`en`) and re-read the same strings.

**Test data:** none typed.

**Expected results:**
- Step 2 translated keys render from the locale catalog (FR-027).
- Step 3 the untranslated key falls back to English rather than showing a raw key or blank
  (FR-027).
- Step 4 all strings render in English.

### TC-016 - ACP server endpoint is feature-gated and off by default

**Requirement:** FR-022, FR-028

**Preconditions:** The default build (`./target/debug/ragent`); and, for the enabled branch,
a build with the feature: `cargo build --features acp-server` (the resulting binary replaces
`./target/debug/ragent`).

**Steps:**
1. With the default build and no `acp.server_enabled` key, run
   `./target/debug/ragent acp-server` and read the exit code and message.
2. Build with the feature (`cargo build --features acp-server`) but leave
   `acp.server_enabled` unset; run `./target/debug/ragent acp-server` again.
3. Enable the endpoint in `.ragent/ragent.json`
   (`"acp": { "server_enabled": true, "server_agent": "general" }`).
4. Send an `initialize` and a `session/new` frame on stdin and read the replies:
   `printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize"}' '{"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"'$PWD'"}}' | ./target/debug/ragent acp-server`
5. Read the first frame's `result.protocolVersion` and the second frame's
   `result.sessionId`.

**Test data:** the two JSON-RPC frames in Step 4.

**Expected results:**
- Step 1 the endpoint refuses to start and exits with a usage error (code 2), naming that the
  `acp-server` feature is required (FR-028).
- Step 2 the endpoint still refuses because `acp.server_enabled` is not set (FR-028).
- Step 4/5 `initialize` replies with `result.protocolVersion` `1` and `session/new` replies
  with a non-empty `result.sessionId`, so an ACP editor can attach (FR-028, FR-022).

## Cleanup

1. Exit every TUI session with `Ctrl+C`.
2. Stop the second ragent server and any webhook receiver (`nc`/`curl` listener).
3. Remove the test container if one was created:
   `podman rm -f ragent-sandbox-test` (or `docker rm -f ragent-sandbox-test`; adjust to the
   id actually launched).
4. Remove the scratch tree: delete `~/scratch/openhands-tests/`.
5. Revert any TUI/config changes made for the tests: restore `.ragent/ragent.json` to its
   pre-test contents and remove the `PreToolUse`/`PostToolUse` hook scripts added in
   TC-012.
6. Remove the temporary MCP `http` server entry added in TC-013 if it was not pre-existing.
7. If a locale catalog was installed only for TC-015, remove it.
8. Confirm the host working directory is unchanged by TC-003: `~/scratch/openhands-tests/`
   should not contain `CONTAINER_MARKER.txt` created directly on the host.
