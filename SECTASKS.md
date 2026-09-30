# SECTASKS.md — Security Remediation Plan

Consolidated remediation plan produced from a per-crate security audit of all 17
workspace crates. One `security-audit` agent audited each crate; findings were
consolidated here and grouped into milestones and tasks.

- **Audit date:** 2026-09-27
- **Crates audited:** 17 / 17 (`ragent-agent`, `ragent-bench`, `ragent-codeindex`,
  `ragent-config`, `ragent-llm`, `ragent-plugins`, `ragent-research`, `ragent-server`,
  `ragent-specs`, `ragent-storage`, `ragent-team`, `ragent-telemetry`,
  `ragent-tools-core`, `ragent-tools-extended`, `ragent-tools-vcs`, `ragent-tui`,
  `ragent-types`)
- **Raw per-crate reports:** `target/temp/secaudit/<crate>.md`
- **Full agent transcripts:** `log/subagents/<task-id>.md`

## Summary

| Severity | Count |
| -------- | ----- |
| Critical | 5     |
| High     | 25    |
| Medium   | 54    |
| Low      | 33    |
| **Total**| **117** |

### Cross-cutting themes

1. **Argument injection into `git`** — no VCS or plugin tool rejects a value
   beginning with `-` or inserts `--`; `--upload-pack`/`--receive-pack`/`--output`
   give command execution and arbitrary file write. (VCS-001/002, PLUGINS-001)
2. **Path containment is inconsistent** — a handful of tools (`multi_edit`,
   document write tools, `restore_snapshot`, `git_clone`, archive import) never call
   the existing `check_path_within_*` helpers that every sibling tool uses.
3. **Untrusted project config is treated as trusted** — a cloned repository's
   `.ragent/ragent.json` can enable YOLO mode, widen allow-lists, and redirect
   telemetry/LLM endpoints.
4. **Secrets leak through shared primitives** — the chokepoint sanitizer
   (`ragent-types/src/sanitize.rs`) misses GitLab PATs and env-sourced credentials,
   and there is no control-character/escape-sequence neutralisation anywhere, so
   secrets and terminal escapes reach logs, the SSE stream, and the TUI.
5. **Unvalidated LLM/tool input reaching a filesystem path or a spawn** — team
   names, spec names, run ids, template names, plugin manifest ids, and marketplace
   subpaths are all joined into paths without validation.
6. **Missing size / timeout / rate caps** — HTTP responses, SSE buffers, archive
   extraction, mailbox messages, benchmark downloads, and server endpoints all lack
   bounds.

### Milestones

| ID    | Milestone                              | Exit criteria                                                             | Tasks                     |
| ----- | -------------------------------------- | ------------------------------------------------------------------------- | ------------------------- |
| MS-01 | Stop the bleeding (Critical)           | All 5 critical findings fixed; regression tests added; no new criticals   | T-001 .. T-006            |
| MS-02 | Host & sandbox escape (High)           | All 25 high findings fixed or formally risk-accepted with rationale       | T-007 .. T-022            |
| MS-03 | Network & secret hardening (Medium)    | All 54 medium findings fixed or triaged                                   | T-023 .. T-046            |
| MS-04 | Defence in depth (Low)                 | All 33 low findings fixed or triaged                                      | T-059 .. T-066            |
| MS-05 | Prevent recurrence                     | Shared guards in place; CI gates enforce them                             | T-067 .. T-071            |

---

## Milestone MS-01 — Stop the bleeding (Critical)

**Exit criteria:** all 5 critical findings remediated, each with a regression test
that fails before the fix and passes after; `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets`, and `cargo test --workspace` all green.

### T-001 — Reject `-`-prefixed values passed to every `git` invocation
- **Findings:** SEC-ragent-plugins-001
- **Severity:** Critical
- **Crate:** `ragent-plugins`
- **Location:** `crates/ragent-plugins/src/add.rs:343-350` (ref), sink `add.rs:407-412` (`git_run`, `add.rs:430-449`)
- **Remediation:** Validate the `git+` ref against `^[A-Za-z0-9._/-]+$` and reject a
  leading `-`; pass `--` before positional arguments in `git_run`; add a unit test
  with the ref `--upload-pack=/bin/sh -c true`.
- **Verify:** `cargo test -p ragent-plugins marketplace` plus a new injection test.

### T-002 — Confine the plugin manifest `id` to the store root
- **Findings:** SEC-ragent-plugins-002
- **Severity:** Critical
- **Crate:** `ragent-plugins`
- **Location:** `crates/ragent-plugins/src/manifest.rs:1139-1143` / `1268-1272` (produce), `add.rs:203-216` (sink)
- **Remediation:** Require the id to be a single `Component::Normal` matching
  `^[A-Za-z0-9._-]+$` (no separators, no `..`, not absolute) in the manifest
  parser; assert the joined path is still inside the store before
  `remove_dir_all`/`copy_dir_recursive`.
- **Verify:** new test asserting `id: "../../x"` and `id: "/tmp/x"` are rejected.

### T-003 — Gate blueprint-declared tool execution behind the permission system
- **Findings:** SEC-ragent-team-001, SEC-ragent-agent-008
- **Severity:** Critical (team) / Medium (agent)
- **Crates:** `ragent-team` (runtime in `ragent-agent::team`), `ragent-agent`
- **Location:** `crates/ragent-agent/src/tool/team_create.rs:187-231`, `274-368`
- **Remediation:** Route every tool invoked from a blueprint seed file
  (`task-seed.json`, `spawn-prompts.json`) through the normal permission checker;
  treat blueprint content as untrusted project content, not operator intent.
- **Verify:** test that a blueprint invoking `bash`/`write` raises an `Ask` verdict.

### T-004 — Validate the team name before it reaches a path
- **Findings:** SEC-ragent-team-002, SEC-ragent-agent-007, SEC-ragent-tui-004
- **Severity:** Critical (team) / Medium (agent, tui)
- **Crates:** `ragent-team`, `ragent-agent`, `ragent-tui`
- **Location:** `crates/ragent-agent/src/tool/team_create.rs:73-77` → `team/store.rs:132-142`, `team/store.rs:80-95`, sink `tool/team_cleanup.rs:92`; `crates/ragent-tui/src/app/slash.rs:5131-5149`, `5394-5396`
- **Remediation:** One `validate_team_name` helper (`^[a-z0-9][a-z0-9-]{0,63}$`),
  called by `TeamStore::create`, `find_team_dir`, cleanup, and every slash command;
  reject anything else with a clear error.
- **Verify:** test asserting `../..` and `/etc` are rejected at all four entry points.

### T-005 — Add path containment to `multi_edit`
- **Findings:** SEC-ragent-tools-core-001
- **Severity:** Critical
- **Crate:** `ragent-tools-core`
- **Location:** `crates/ragent-tools-core/src/multiedit.rs` (no `check_path_within_*` call; contrast `read.rs:194-199`)
- **Remediation:** Call the same containment check every sibling file tool uses
  before reading or writing each edit target; reject absolute paths and `..` that
  resolve outside the workspace.
- **Verify:** test asserting `{"file_path":"../../.bashrc"}` is denied.

### T-006 — Close out MS-01
- **Deliverable:** re-run the four affected audits (`ragent-plugins`,
  `ragent-team`, `ragent-agent`, `ragent-tools-core`) and confirm the criticals are
  gone; record the result in `CHANGELOG.md` and `SPEC.md`.
- **Verify:** no `Severity: Critical` finding remains in a re-audit.
- **Status (complete):** re-audited the four affected crates; raw reports in
  `target/temp/secaudit/<crate>-reaudit.md`. Result: `CRITICAL_COUNT=0` for
  `ragent-agent`, `ragent-team`, and `ragent-tools-core`; `ragent-plugins`
  reports "both prior Criticals genuinely RESOLVED" (validators applied at
  parse **and** at the sinks). Recorded in `CHANGELOG.md` (`[Unreleased]` ->
  `Security`) and `SPEC.md` 4.6a.

---

## Milestone MS-02 — Host & sandbox escape (High)

**Exit criteria:** each high finding either fixed with a test or explicitly
risk-accepted in this file with a written rationale.

### T-007 — Stop hooks from silently auto-approving tools
- **Findings:** SEC-ragent-agent-001
- **Crate:** `ragent-agent`
- **Location:** `session/processor.rs:3076-3113` (consumer), `hooks/mod.rs:545-562` (producer)
- **Remediation:** Carry `hook_approved: bool` explicitly through
  `PreToolUseResult::Allow` so a hook approval is distinguishable from "no hook";
  a hook may satisfy an `Ask` but must never override an explicit `Deny`. Drop
  plugin-contributed `pre_tool_use` hooks from the merge (`merge_hook_configs`)
  unless declared in user-owned `ragent.json`.

### T-008 — Contain session archive import
- **Findings:** SEC-ragent-agent-002, SEC-ragent-agent-009
- **Crate:** `ragent-agent`
- **Location:** `session/archive.rs:444-469` (`import_session_archive`), `426-429`
- **Remediation:** Reject any archive entry whose path is absolute or escapes the
  session root; cap total decompressed bytes and entry count; do the same for the
  gzip path.

### T-009 — Allowlist the benchmark fixture program set
- **Findings:** SEC-ragent-bench-001, SEC-ragent-bench-002
- **Crate:** `ragent-bench`
- **Location:** `suites/mbpp.rs:294-313`, `suites/humaneval.rs:338-341`, `391-410`; entry `data.rs:627-646`
- **Remediation:** Allowlist the intended toolchain programs (`g++`, `go`, `javac`,
  `node`, `rustc`, ...), reject absolute paths and shell interpreters, and give the
  `python3` fallback path the same hard timeout as the native harness.

### T-010 — Contain codeindex watcher indexing
- **Findings:** SEC-ragent-codeindex-001, SEC-ragent-codeindex-002
- **Crate:** `ragent-codeindex`
- **Location:** `watcher.rs:80-82`, `lib.rs:834-850` (`index_file`), `lib.rs:849`; contrast `scanner.rs:116-122`
- **Remediation:** Apply the scanner's root-containment check and `max_file_size`
  cap on the incremental path too; do not follow symlinks out of the indexed root.

### T-011 — Stop untrusted project config widening privileges
- **Findings:** SEC-ragent-config-001, SEC-ragent-config-002, SEC-ragent-config-003
- **Crate:** `ragent-config`
- **Location:** `config.rs:1584-1591`, `2153-2156`, `2188-2190`
- **Remediation:** Treat the cwd/`.ragent/` config as untrusted: it may not enable
  YOLO mode, may not remove or weaken global deny rules (union deny, intersect
  allow), and may not extend `allowed_roots`/bash allowlists beyond the project
  root. Require privileged keys to come from the user-global config only.

### T-012 — Stop credential leakage from the Gemini provider
- **Findings:** SEC-ragent-llm-001, SEC-ragent-llm-007
- **Crate:** `ragent-llm`
- **Location:** `providers/gemini.rs:320-332`, `providers/http_client.rs:129-146`
- **Remediation:** Move the key out of the query string where the API allows it;
  redact the key from every error/URL string before logging or returning it;
  implement `Debug` by hand (or `#[derive]` excluding the field) on structs holding
  credential material.

### T-013 — Do not forward credentials across redirects
- **Findings:** SEC-ragent-llm-002
- **Crate:** `ragent-llm`
- **Location:** `providers/http_client.rs:129-146`
- **Remediation:** Pin the redirect policy for authenticated requests; drop
  non-standard auth headers when a redirect crosses to a different host, or refuse
  to follow the redirect.

### T-014 — Validate the research template name
- **Findings:** SEC-ragent-research-001, SEC-ragent-server-004
- **Crates:** `ragent-research`, `ragent-server`
- **Location:** `io.rs:110-114` (`template_path`), reached from `session.rs:3852-3856`, `2167-2168`; server entry `routes/research.rs:141`
- **Remediation:** Require a single `[A-Za-z0-9_-]` component (reuse the existing
  `validate_run_tag` shape) before building the path; return `None` + warn on
  rejection. Server-side: reject before dispatching.

### T-015 — Stop `GET /config` returning plaintext secrets
- **Findings:** SEC-ragent-server-001
- **Crate:** `ragent-server`
- **Location:** `routes/mod.rs:199-202` (`get_config`)
- **Remediation:** Redact every credential field before serialising (reuse the
  `ragent-types` secret redaction, extended per T-050); never return raw keys,
  PATs, bot tokens, or client secrets.

### T-016 — Contain research `from_files` reads
- **Findings:** SEC-ragent-server-002, SEC-ragent-research-005
- **Crates:** `ragent-server`, `ragent-research`
- **Location:** server entry `routes/research.rs:154-155`; `session.rs:3212-3225`
- **Remediation:** Canonicalise each entry and reject anything outside the project
  root (reuse `check_path_within_root`); reject `..` and absolute components.

### T-017 — Replace the credential XOR cipher
- **Findings:** SEC-ragent-storage-001, SEC-ragent-storage-002, SEC-ragent-storage-004
- **Crate:** `ragent-storage`
- **Location:** credential store module (key derivation from `$USER`+`$HOME` / hardcoded fallback)
- **Remediation:** Use an AEAD (e.g. ChaCha20-Poly1305) with a random per-install
  key from the OS keystore or a 0600 key file, a real KDF for any passphrase path,
  and a versioned format; migrate legacy rows on read and delete the hardcoded key
  path. Never downgrade a corrupt credential to an empty string on the main read
  path — surface an error.

### T-018 — Authenticate team mailbox senders
- **Findings:** SEC-ragent-team-003
- **Crate:** `ragent-team` (runtime in `ragent-agent::team`)
- **Location:** mailbox write/read path
- **Remediation:** Derive the sender from the authenticated agent identity held by
  the runtime, not from a message-supplied field; reject or clearly label a message
  whose claimed sender does not match.

### T-019 — Enable TLS for the OTLP gRPC exporter
- **Findings:** SEC-ragent-telemetry-001
- **Crate:** `ragent-telemetry`
- **Location:** telemetry exporter construction (`tonic/tls` feature not enabled)
- **Remediation:** Enable the TLS transport for the OTLP gRPC exporter; refuse a
  non-loopback `https://` endpoint that would be contacted in cleartext.

### T-020 — Stop untrusted config redirecting telemetry
- **Findings:** SEC-ragent-telemetry-002
- **Crate:** `ragent-telemetry` / `ragent-config`
- **Location:** telemetry enablement + endpoint resolution
- **Remediation:** Require telemetry enablement and endpoint override to come from
  the user-global config or an explicit CLI flag; ignore project-config telemetry
  keys.

### T-021 — Close the bash whitelist bypass and drop the Python calculator
- **Findings:** SEC-ragent-tools-core-002, SEC-ragent-tools-core-003
- **Crate:** `ragent-tools-core`
- **Location:** `bash.rs` safe-command whitelist; `calculator` tool
- **Remediation:** Do not auto-approve interpreter invocations (`python`, `perl`,
  `node`, `sh`, `awk`, ...) on the strength of a whitelist entry — they must go
  through the normal permission gate; ensure the banned-tool list is consulted
  before the whitelist, not after. Replace `calculator`'s `python3 -c` with a real
  arithmetic evaluator.

### T-022 — Guard the VCS `git` argument surface
- **Findings:** SEC-ragent-tools-vcs-001, SEC-ragent-tools-vcs-002, SEC-ragent-tools-vcs-003, SEC-ragent-tools-vcs-004
- **Crate:** `ragent-tools-vcs`
- **Location:** `git/git_clone.rs:69-89`, `87-91`; `git/git_fetch.rs:69-74`; `git/git_push.rs:69-73`; `git/git_pull.rs:60-64`; `git/git_log.rs:75-94`; `vcs_provider.rs:187-197`
- **Remediation:** Insert `--` before every positional argument; reject any value
  beginning with `-` for ref, branch, tag, remote, path, and directory parameters;
  confine the `git_clone` `directory` to the working dir; validate the
  `gitlab:<host>` host against an allowlist before attaching the token.

### T-023 — Neutralise terminal escapes at the chokepoint
- **Findings:** SEC-ragent-types-001 (High), SEC-ragent-tui panics/leaks
- **Crate:** `ragent-types` (plus callers)
- **Location:** `sanitize.rs`, `html.rs`, `strutil.rs`; sinks in `crates/ragent-tui/src/layout.rs:4807-4830` and the widget sinks listed under SEC-ragent-tui-001
- **Remediation:** Add control-character and CSI/OSC/DCS neutralisation to the
  shared sanitizer and apply it on every path that renders untrusted text to a
  terminal, a log, or a file.

### T-024 — Guard browser-tool navigation
- **Findings:** SEC-tools-extended-001
- **Crate:** `ragent-tools-extended`
- **Location:** browser tool `open` action
- **Remediation:** Reject non-`http(s)` schemes (`file://`, `chrome://`, `data:`)
  and apply the SSRF host check before navigating.

---

## Milestone MS-03 — Network & secret hardening (Medium)

**Exit criteria:** each medium finding fixed or triaged; `cargo test --workspace`
green.

### Network egress

| Task  | Finding(s)                                  | Crate / location                                                            | Remediation                                                                                 |
| ----- | ------------------------------------------- | --------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| T-025 | SEC-ragent-agent-003                        | `ragent-agent/src/mcp/http.rs:372-374`                                      | Cap MCP HTTP response body size.                                                            |
| T-026 | SEC-ragent-agent-006                        | `ragent-agent/src/updater/mod.rs:125-154`                                   | Verify a signature/checksum before replacing the running binary; refuse unverified downloads.|
| T-027 | SEC-ragent-bench-003, SEC-ragent-bench-004   | `ragent-bench/src/data.rs:859-875`, `1082-1105`, `631`                      | Add a download size cap and a pagination limit; contain the manifest `case_file` path.       |
| T-028 | SEC-ragent-llm-003, SEC-ragent-llm-004, SEC-ragent-llm-005 | `ragent-llm/src/providers/http_client.rs:327-334`, `103-108` | Clamp `Retry-After`; cap the SSE accumulation buffer; cap error/response body reads.        |
| T-029 | SEC-ragent-plugins-004                      | `ragent-plugins/src/store_provider.rs:439-473` → `add.rs:395-426`           | Allowlist the marketplace/git host set before cloning.                                       |
| T-030 | SEC-ragent-research-002, SEC-ragent-research-003, SEC-ragent-research-004 | `ragent-research/src/web_gatherer.rs:2625-2662`, `112-134`, `2069-2071` | Re-validate the post-redirect host; make `effective_web_budget()` a hard ceiling; clamp retries and use `checked_shl`. |
| T-031 | SEC-ragent-server-003, SEC-ragent-server-005, SEC-ragent-server-009 | `ragent-server/src/routes/mod.rs:146`, `358-388`             | Replace `CorsLayer::permissive()` with an explicit origin allowlist; add request/concurrency timeouts, endpoint rate limits, and an SSE client cap. |
| T-032 | SEC-ragent-telemetry-003                    | telemetry cardinality resolver                                              | Build the cardinality cache once per recorder, not per call, so the cap actually binds.      |
| T-033 | SEC-ragent-tools-extended-005, SEC-ragent-tools-extended-006, SEC-ragent-tools-extended-010 | masterfetch http/crawl modules                              | Re-validate each redirect hop; apply the size budget before buffering (streaming cap); SSRF-validate `crawl_urls`. |
| T-034 | SEC-ragent-tools-vcs-005, SEC-ragent-tools-vcs-007, SEC-ragent-tools-vcs-008 | `github/client.rs:140-149`, `gitlab/gitlab_pipelines.rs:280-296`, `github/client.rs:487-532` | Require a same-origin `download_url` before attaching the token; bound pagination loops and the recursive tree walk. |
| T-035 | SEC-ragent-types-003, SEC-ragent-types-005   | `ragent-types/src/cron.rs:165-215`, `339-415`, `849-863`; `event/mod.rs:1313-1321` | Replace invariant `expect`/`assert!` and unchecked multiplication with checked arithmetic returning `Result`; use char-boundary-safe slicing. |

### Secrets, redaction, and disclosure

| Task  | Finding(s)                                  | Crate / location                                                             | Remediation                                                                                 |
| ----- | ------------------------------------------- | ---------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| T-036 | SEC-ragent-agent-005                        | `ragent-agent/src/skill/context.rs:183-209`, `251-267`; `skill/invoke.rs:145-146` | Quote/validate skill command arguments so shell metacharacters cannot escape the allowlist.  |
| T-037 | SEC-ragent-bench-007                        | `ragent-bench/src/runner.rs:550-552`, `458-460`                              | Do not persist provider base URLs into the workbook or run-state sidecar.                    |
| T-038 | SEC-ragent-config-004, SEC-ragent-config-005 | `ragent-config/src/config.rs:1750-1753`; `permission.rs:249-256`            | Write config files 0600; fail closed (and warn loudly) when a deny-rule glob does not parse. |
| T-039 | SEC-ragent-server-006                       | `ragent-server/src/routes/mod.rs:533-557` (`events_stream`)                  | Scope `/events` to the caller's session; apply secret redaction to tool-call arguments.      |
| T-040 | SEC-ragent-specs-001, SEC-ragent-specs-002   | `ragent-specs/src/impl_runner.rs`, `validate.rs`                             | Bound numbering-gap expansion and contradiction detection; reject oversized SPEC.md.         |
| T-041 | SEC-ragent-storage-003                       | storage DB/WAL/activity-log creation                                         | Create the DB, WAL, and activity log 0600 (umask-independent).                               |
| T-042 | SEC-ragent-tools-core-005                    | `ragent-tools-core/src/bash.rs` timeout path                                 | Redact secrets from partial timeout output before it reaches the model or session store.     |
| T-043 | SEC-ragent-tools-vcs-006                     | `github/github_actions.rs:117-158`                                           | Redact secrets from CI log excerpts before injecting them into the model context.            |
| T-044 | SEC-ragent-tools-extended-003, SEC-ragent-tools-extended-007 | gmail tool                                                | Strip CRLF from header values; never echo `refresh_token`/`client_secret` into tool output.  |
| T-045 | SEC-ragent-tui-002                           | `ragent-tui/src/layout.rs:1225-1241`, `1908-1923`; `app/state.rs:481-497`     | Mask the API key and GitLab PAT in the provider-setup dialog.                                |
| T-046 | SEC-ragent-types-004                          | `ragent-types/src/sanitize.rs:18-50`, `195-206`                              | Extend `SECRET_PATTERN` to cover GitLab PATs and every env-sourced credential registered by the secret registry. |

### Also in MS-03

| Task  | Finding(s)                                  | Crate / location                                                             | Remediation                                                                                 |
| ----- | ------------------------------------------- | ---------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| T-047 | SEC-ragent-agent-004, SEC-ragent-team-005, SEC-ragent-team-006 | `hooks/mod.rs:519-536`, `1003-1080`; team hook + mailbox paths | Add an overall hook timeout that also kills the child process; validate hook commands; cap the mailbox message size. |
| T-048 | SEC-ragent-bench-002                        | `suites/humaneval.rs:338-341`, `suites/mbpp.rs:246-249`                     | Give the `python3` hidden-test path the same hard subprocess timeout as the native harness.  |
| T-049 | SEC-ragent-codeindex-003, SEC-ragent-codeindex-004 | `parser/*.rs` walkers; `graph/edges.rs:107-200`                        | Add an explicit depth/visited limit to the tree-sitter walkers (no unguarded Rust recursion); bound edge amplification during derivation. |
| T-050 | SEC-ragent-llm-006                          | `ragent-llm/src/providers/http_client.rs:238-241`, `ollama_cloud.rs:757-765` | Slice SSE frames on a char boundary (`str::get`/`floor_char_boundary`) instead of raw byte offsets. |
| T-051 | SEC-ragent-plugins-003                      | `ragent-plugins/src/add.rs:588-618`, `620-655`                              | Cap total decompressed bytes and entry count during extraction (the 50 MiB cap only covers compressed input). |
| T-052 | SEC-ragent-team-004                         | teammate spawn path                                                          | Enforce `max_teammates`; reject unbounded recursive spawning.                                |
| T-053 | SEC-ragent-telemetry-004                    | telemetry attribute recording                                                | Stop recording the model-controlled `tool.name` attribute, or redact it against an allowlist rather than a deny list. |
| T-054 | SEC-ragent-tools-core-004                   | `ragent-tools-core/src/bash.rs` temp-script/state path                       | Write the temp script and persistent state under `target/temp` (or a 0600 mktemp dir), not shared world-readable `/tmp`. |
| T-055 | SEC-ragent-tools-extended-002               | document write tools                                                         | Route `office_write`/`libre_write`/`pdf_write` output paths through the workspace containment check. |
| T-056 | SEC-ragent-tools-extended-004               | `http_request` tool                                                          | Reject caller-supplied routing/credential headers (`Host`, `Cookie`, `Authorization`, `Proxy-*`). |
| T-057 | SEC-ragent-tui-001                          | `widgets/message_widget.rs:707-708` and the listed sinks                     | Replace byte-offset truncation with `str::floor_char_boundary` (or char-safe helpers) everywhere tool args/output are summarised. |
| T-058 | SEC-ragent-types-002                        | `ragent-types/src/stderr_spool/spool.rs:46-57`, `79-111`, `117-128`          | Enforce the line cap on every write (count the pending partial line too) so a newline-free writer cannot grow the file without bound. |

---

### MS-03 status (complete)

Milestone MS-03 ("Network & secret hardening") is implemented in full. The
tasks T-025 .. T-058 were delivered as explicit bounds — byte caps, page/entry
budgets, clamped delays, origin-scoped credential attachment, and containment
checks — and are recorded in `SPEC.md` §4.6c. Notable guards:

- **Egress bounds** — MCP HTTP bodies, benchmark downloads, LLM error bodies and
  SSE buffers, masterfetch responses, the GitHub tree walk, and the GitLab jobs
  pagination all carry explicit caps/budgets.
- **Redirect safety** — research and masterfetch re-validate each hop, and the
  GitHub client attaches its token only to the configured API origin.
- **Redaction** — the secret registry is seeded from every credential env var;
  bash output (partial and complete), CI log excerpts, and SSE tool-call
  arguments are redacted; the provider-setup dialog masks typed credentials.
- **Containment and permissions** — document write tools, research file seeds,
  and the bash scratch files are confined; `http_request` refuses routing
  headers; the codeindex scanner applies its exclusion globs and the watcher
  path enforces size/containment.
- **Panic-free parsing** — `CronSchedule` no longer asserts its invariants,
  numbering-gap expansion and contradiction detection are bounded, edge
  derivation carries budgets, and the stderr spool enforces a byte ceiling.

Verification at the time of writing: `cargo fmt --all -- --check` clean,
`cargo check --workspace --all-targets` clean, `cargo clippy --workspace
--all-targets` clean apart from the pre-existing `redundant_pub_crate` warnings
(12), and `cargo test --workspace` green (10 112 passed, 0 failed) across three
consecutive full runs.

## Milestone MS-04 — Defence in depth (Low)

**Exit criteria:** each low finding fixed or triaged as accepted risk.

| Task  | Finding(s)                                  | Remediation |
| ----- | ------------------------------------------- | ----------- |
| T-059 | SEC-ragent-agent-009, SEC-ragent-bench-005, SEC-ragent-bench-006, SEC-ragent-bench-007 | Zip-bomb cap on archive import; contain `relative_path` reads; bound `--samples`; stop persisting base URLs (see T-037). |
| T-060 | SEC-ragent-codeindex-005, SEC-ragent-codeindex-006, SEC-ragent-codeindex-007 | Read-then-check size (or open-and-fstat) to close the TOCTOU; apply `extra_exclude_patterns`; resolve the FTS dir before deleting through it. |
| T-061 | SEC-ragent-config-006, SEC-ragent-config-007 | Actually apply `BUILTIN_DENYLIST` to the enforced lists; stop echoing the offending source line from config parse errors (it can contain a secret). |
| T-062 | SEC-ragent-llm-007, SEC-ragent-plugins-005, SEC-ragent-plugins-006, SEC-ragent-plugins-007 | Hand-written `Debug` excluding credentials; reject http redirects on package download; confine manifest `entry` to the plugin root; `..`-guard the marketplace join. |
| T-063 | SEC-ragent-research-006, SEC-ragent-research-007 | Neutralise backtick runs / `#### Source [#` prefixes in fenced source bodies; mask query strings and userinfo in the gather log. |
| T-064 | SEC-ragent-server-007, SEC-ragent-server-008 | Bind a permission reply to a pending request for that session; return generic error strings to clients and keep paths/stack traces in the log. |
| T-065 | SEC-ragent-specs-003, SEC-ragent-specs-004, SEC-ragent-specs-005 | Validate the spec name in `/spec create` and `/spec specify`; route `SpecId` deserialization through the constructor validation; validate `write_govcreate_spec`. |
| T-066 | SEC-ragent-storage-006, SEC-ragent-team-007, SEC-ragent-telemetry-005, SEC-ragent-tools-vcs-008, SEC-ragent-tui-003, SEC-ragent-tui-004, SEC-ragent-tui-005, SEC-ragent-types-006, SEC-tools-extended-007, SEC-tools-extended-008, SEC-tools-extended-009, SEC-tools-extended-010 | Fix non-ASCII diff-line panic; validate the agent name in `resolve_memory_dir`; apply redaction in the cardinality resolver and fail closed on lock poisoning; bound the tree walk; validate run ids in `/alog export`; team-name validation (see T-004); create the log spool 0600; remove `Debug` derive from credential-bearing events and redact them on the SSE stream; stop echoing `refresh_token`; SSRF-validate config base URLs; classify `browser eval` under its own permission category; SSRF-validate `crawl_urls`. |

### MS-04 status (complete)

Milestone MS-04 ("Defence in depth") is implemented in full. Tasks T-059 .. T-066
were delivered as the same guard shapes used by the earlier milestones — explicit
caps/budgets, containment checks, char-safe splits, and owner-only file modes —
and are recorded in `SPEC.md` §4.6d. Notable guards:

- **Resource caps** — the benchmark `--samples` flag is clamped to 100 before
  both the `Vec` pre-allocation and the generation loop; dataset downloads are
  streamed under a 64 MiB cap and the HumanEvalPack pagination loop carries a
  page/row budget; session-archive import keeps its entry-count and
  total-decompressed-size caps.
- **Containment** — the benchmark manifest `case_file`/`relative_path` joins are
  confined to the canonicalised data root; plugin manifest `entry`/`main`/
  `server.entry` must be a contained relative path; the marketplace
  wrapper-directory name must be a single normal component; a multi-component or
  `..` agent name is refused by `resolve_memory_dir`.
- **Char-safe and panic-free** — `apply_unified_diff` splits on the first char,
  so a multibyte diff line returns a `Result` instead of panicking.
- **Redaction and disclosure** — the config parse diagnostic redacts the echoed
  source line; research fenced bodies cannot close their fence or spoof a
  `#### Source [#N]` header; gather-log URLs drop userinfo and query strings;
  internal server errors are logged in full and returned generically;
  credential-bearing events expose presence-only fields on the SSE stream plus a
  `redacted_event_debug` helper.
- **Validation** — a permission reply is bound to the awaiting session;
  `SpecId` deserialises through its validating constructor and
  `write_govcreate_spec` re-validates; `/spec create`/`/spec specify` validate
  the name; `/alog export` accepts only `[A-Za-z0-9_-]+` run ids.
- **File modes and egress** — the log-window spool is created 0600 and
  re-asserted on every open; the bash scratch directory falls back to a
  process-private 0700 directory; config-supplied outbound base URLs (Telegram,
  Gmail, finance providers) are SSRF-checked; the `CrawlFetcher` SSRF obligation
  is stated on the trait.
- **Remaining Low items** — `SEC-ragent-bench-004/005` (manifest path
  containment) and `SEC-ragent-plugins-005` (http redirect on package download)
  were also closed here, and `SEC-ragent-tools-core-004` (scratch-dir
  confinement) was completed.

Verification at the time of writing: `cargo fmt --all -- --check` clean,
`cargo check --workspace --all-targets` clean, `cargo clippy --workspace
--all-targets` clean (0 warnings), and `cargo test --workspace` green
(10 128 passed, 0 failed).

---

## Milestone MS-05 — Prevent recurrence

**Exit criteria:** the shared guards exist, are documented in `SPEC.md`/`AGENTS.md`,
and CI fails a build that reintroduces the classes.

| Task  | Deliverable |
| ----- | ----------- |
| T-067 | Add shared helpers in `ragent-types` (or a new `ragent-security` module): `validate_identifier`, `validate_relative_component`, `contained_join`, `reject_option_like`, `clamp_retry_after`, `cap_read`. Migrate every site in this plan onto them. |
| T-068 | Add a `git`-invocation guard used by all VCS/plugin code: always `--`, always reject leading `-`. |
| T-069 | Extend the secret registry/redaction so `GET /config`, telemetry, logs, SSE, and tool output all share one redaction implementation. |
| T-070 | Add CI gates: a `cargo test` suite for the new guards, a check that every file tool calls a containment helper, and a `check-` script asserting no production `.unwrap()`/`expect()` was added on a security-relevant path. |
| T-071 | Record the accepted-risk register (findings deliberately not fixed) in this file with rationale and review date. |

### MS-05 status (complete)

**T-067 - shared guards.** `crates/ragent-types/src/guard.rs` now owns the single
implementation of every guard shape the earlier milestones re-derived:
`reject_option_like`, `is_safe_operand`, `validate_identifier`,
`validate_relative_component`, `contained_join`, `clamp_retry_after`, and
`cap_read`, with `MAX_IDENTIFIER_LEN` and `MAX_RETRY_AFTER` as the shared
constants. The module is re-exported from the `ragent-types` root and as
`ragent_tools_core::guard` for the tools crates. Migrated call sites:
`ragent-tools-vcs::git::reject_option_like` (all ~20 VCS call sites),
`ragent-plugins::add::is_safe_git_argument`, and
`ragent-bench::data::contained_join`. Regression suite:
`crates/ragent-types/tests/test_shared_guards.rs` (15 tests).

**T-068 - git-invocation guard.** Every VCS git path already inserts `--` before
positional arguments and rejects a leading `-`; the rule itself now lives in
`ragent_types::guard` and is shared with the plugin `git+` parser. The
duplication gate (`scripts/check-vcs-duplication.sh`) fails a build that
re-derives the leading-dash check or drops the delegation.

**T-069 - one redaction implementation.** `ragent-agent`, `ragent-storage`, and
`ragent-tools-core` re-export `ragent_types::sanitize` rather than holding a
second registry, and every disclosure surface (`GET /config`, telemetry
attributes, log lines, SSE payloads, tool output) calls into it. `Event` no
longer derives `Debug`: a hand-written impl renders through `DebugProxy`, which
reports credential-bearing fields (`CopilotDeviceFlowComplete.token`,
`CopilotDeviceFlowStartResult.device_code`) as presence flags and then applies
the shared `redact_secrets` pass to the whole rendering, so a
`tracing::debug!("{event:?}")` can no longer print an OAuth credential.
Regression suite: `crates/ragent-types/tests/test_unified_redaction.rs`
(4 tests).

**T-070 - CI gates.** Four script gates, each with a `--self-test` that proves it
fails on a seeded violation, wired into `.github/workflows/ci.yml` (job
`security-guards`) and `pre-flight.sh`:

| Gate | Script / command | Asserts |
| ---- | ---------------- | ------- |
| Redaction tests | `cargo test -p ragent-types --test test_unified_redaction` | one registry, masked `Event` `Debug` |
| File-tool containment | `scripts/check-file-tool-containment.sh` | every registered file tool calls a `check_path_within_*` helper |
| Panic-free production paths | `scripts/check-security-unwraps.sh` | no file exceeds its recorded `.unwrap()`/`.expect()` baseline |
| One guard per rule | `scripts/check-shared-guards.sh` | no crate re-defines a shared guard or a second secret registry |
| VCS git-argument rule | `scripts/check-vcs-duplication.sh` | the leading-dash git check is not re-derived |

The shared-guard and redaction suites also run through the workspace test job
(`cargo test -p ragent-types --test test_shared_guards --test
test_unified_redaction`).

The panic-free gate is a baseline gate: `scripts/security-unwrap-baseline.txt`
records the current per-file count of panicking calls (362 across 90 files) and
the gate fails any file that exceeds it. An intentional call can be exempted
with a same-line `// no-panic-ok: <reason>` comment.

**T-071 - accepted-risk register.** The register below now records the
grandfathered sites instead of claiming an empty list.

Verification at the time of writing: `cargo fmt --all -- --check` clean,
`cargo check --workspace --all-targets` clean, `cargo clippy --workspace
--all-targets` clean, all four script gates green with their self-tests, and
`cargo test --workspace` green (10,154 tests passed, 0 failed).

---

## Finding → Task index

| Finding                        | Sev      | Task        |
| ------------------------------ | -------- | ----------- |
| SEC-ragent-agent-001           | High     | T-007       |
| SEC-ragent-agent-002           | High     | T-008       |
| SEC-ragent-agent-003           | Medium   | T-025       |
| SEC-ragent-agent-004           | Medium   | T-047       |
| SEC-ragent-agent-005           | Medium   | T-036       |
| SEC-ragent-agent-006           | Medium   | T-026       |
| SEC-ragent-agent-007           | Medium   | T-004       |
| SEC-ragent-agent-008           | Medium   | T-003       |
| SEC-ragent-agent-009           | Low      | T-008, T-059|
| SEC-ragent-bench-001           | High     | T-009       |
| SEC-ragent-bench-002           | Medium   | T-009, T-048|
| SEC-ragent-bench-003           | Medium   | T-027       |
| SEC-ragent-bench-004           | Medium   | T-027       |
| SEC-ragent-bench-005           | Low      | T-059       |
| SEC-ragent-bench-006           | Low      | T-059       |
| SEC-ragent-bench-007           | Low      | T-037, T-059|
| SEC-ragent-codeindex-001       | High     | T-010       |
| SEC-ragent-codeindex-002       | Medium   | T-010       |
| SEC-ragent-codeindex-003       | Medium   | T-049       |
| SEC-ragent-codeindex-004       | Medium   | T-049       |
| SEC-ragent-codeindex-005       | Low      | T-060       |
| SEC-ragent-codeindex-006       | Low      | T-060       |
| SEC-ragent-codeindex-007       | Low      | T-060       |
| SEC-ragent-config-001          | High     | T-011       |
| SEC-ragent-config-002          | High     | T-011       |
| SEC-ragent-config-003          | High     | T-011       |
| SEC-ragent-config-004          | Medium   | T-038       |
| SEC-ragent-config-005          | Medium   | T-038       |
| SEC-ragent-config-006          | Low      | T-061       |
| SEC-ragent-config-007          | Low      | T-061       |
| SEC-ragent-llm-001             | High     | T-012       |
| SEC-ragent-llm-002             | High     | T-013       |
| SEC-ragent-llm-003             | Medium   | T-028       |
| SEC-ragent-llm-004             | Medium   | T-028       |
| SEC-ragent-llm-005             | Medium   | T-028       |
| SEC-ragent-llm-006             | Medium   | T-050       |
| SEC-ragent-llm-007             | Low      | T-012, T-062|
| SEC-ragent-plugins-001         | Critical | T-001       |
| SEC-ragent-plugins-002         | Critical | T-002       |
| SEC-ragent-plugins-003         | Medium   | T-051       |
| SEC-ragent-plugins-004         | Medium   | T-029       |
| SEC-ragent-plugins-005         | Low      | T-062       |
| SEC-ragent-plugins-006         | Low      | T-062       |
| SEC-ragent-plugins-007         | Low      | T-062       |
| SEC-ragent-research-001        | High     | T-014       |
| SEC-ragent-research-002        | Medium   | T-030       |
| SEC-ragent-research-003        | Medium   | T-030       |
| SEC-ragent-research-004        | Medium   | T-030       |
| SEC-ragent-research-005        | Medium   | T-016       |
| SEC-ragent-research-006        | Low      | T-063       |
| SEC-ragent-research-007        | Low      | T-063       |
| SEC-ragent-server-001          | High     | T-015       |
| SEC-ragent-server-002          | High     | T-016       |
| SEC-ragent-server-003          | Medium   | T-031       |
| SEC-ragent-server-004          | Medium   | T-014       |
| SEC-ragent-server-005          | Medium   | T-031       |
| SEC-ragent-server-006          | Medium   | T-039       |
| SEC-ragent-server-007          | Low      | T-064       |
| SEC-ragent-server-008          | Low      | T-064       |
| SEC-ragent-server-009          | Low      | T-031       |
| SEC-ragent-specs-001           | Medium   | T-040       |
| SEC-ragent-specs-002           | Medium   | T-040       |
| SEC-ragent-specs-003           | Low      | T-065       |
| SEC-ragent-specs-004           | Low      | T-065       |
| SEC-ragent-specs-005           | Low      | T-065       |
| SEC-ragent-storage-001         | High     | T-017       |
| SEC-ragent-storage-002         | High     | T-017       |
| SEC-ragent-storage-003         | Medium   | T-041       |
| SEC-ragent-storage-004         | Medium   | T-017       |
| SEC-ragent-storage-005         | Medium   | T-017       |
| SEC-ragent-storage-006         | Low      | T-066       |
| SEC-ragent-team-001            | Critical | T-003       |
| SEC-ragent-team-002            | Critical | T-004       |
| SEC-ragent-team-003            | High     | T-018       |
| SEC-ragent-team-004            | Medium   | T-052       |
| SEC-ragent-team-005            | Medium   | T-047       |
| SEC-ragent-team-006            | Medium   | T-047       |
| SEC-ragent-team-007            | Low      | T-066       |
| SEC-ragent-telemetry-001       | High     | T-019       |
| SEC-ragent-telemetry-002       | High     | T-020       |
| SEC-ragent-telemetry-003       | Medium   | T-032       |
| SEC-ragent-telemetry-004       | Medium   | T-053       |
| SEC-ragent-telemetry-005       | Low      | T-066       |
| SEC-ragent-tools-core-001      | Critical | T-005       |
| SEC-ragent-tools-core-002      | High     | T-021       |
| SEC-ragent-tools-core-003      | High     | T-021       |
| SEC-ragent-tools-core-004      | Medium   | T-054       |
| SEC-ragent-tools-core-005      | Medium   | T-042       |
| SEC-ragent-tools-vcs-001       | High     | T-022       |
| SEC-ragent-tools-vcs-002       | High     | T-022       |
| SEC-ragent-tools-vcs-003       | High     | T-022       |
| SEC-ragent-tools-vcs-004       | High     | T-022       |
| SEC-ragent-tools-vcs-005       | Medium   | T-034       |
| SEC-ragent-tools-vcs-006       | Medium   | T-043       |
| SEC-ragent-tools-vcs-007       | Medium   | T-034       |
| SEC-ragent-tools-vcs-008       | Low      | T-034, T-066|
| SEC-ragent-tui-001             | Medium   | T-057       |
| SEC-ragent-tui-002             | Medium   | T-045       |
| SEC-ragent-tui-003             | Low      | T-066       |
| SEC-ragent-tui-004             | Low      | T-004, T-066|
| SEC-ragent-tui-005             | Low      | T-066       |
| SEC-ragent-types-001           | High     | T-023       |
| SEC-ragent-types-002           | Medium   | T-058       |
| SEC-ragent-types-003           | Medium   | T-035       |
| SEC-ragent-types-004           | Medium   | T-046       |
| SEC-ragent-types-005           | Medium   | T-035       |
| SEC-ragent-types-006           | Low      | T-066       |
| SEC-tools-extended-001         | High     | T-024       |
| SEC-tools-extended-002         | Medium   | T-055       |
| SEC-tools-extended-003         | Medium   | T-044       |
| SEC-tools-extended-004         | Medium   | T-056       |
| SEC-tools-extended-005         | Medium   | T-033       |
| SEC-tools-extended-006         | Medium   | T-033       |
| SEC-tools-extended-007         | Low      | T-044, T-066|
| SEC-tools-extended-008         | Low      | T-066       |
| SEC-tools-extended-009         | Low      | T-066       |
| SEC-tools-extended-010         | Low      | T-033, T-066|

---

## Accepted-risk register

Findings that are deliberately not fixed, with the rationale, the compensating
control that keeps the class reachable-but-bounded, and the date to review the
decision. An entry here is a decision, not an omission: anything not listed has
been remediated.

**Register last reviewed: 2026-09-28** (MS-05 T-071). Next review: 2027-03-28
(six months), or on any change to the listed compensating control.

| # | Item | Finding class | Rationale | Compensating control | Review |
| - | ---- | ------------- | --------- | -------------------- | ------ |
| A-1 | Panicking `.unwrap()`/`.expect()` grandfathered on production paths | SEC-ragent-types-003/005, SEC-ragent-llm-006 and peers | 362 call sites across 90 files. Each is on a path that was already audited and judged unreachable, and rewriting all of them is a behavioural change far larger than the risk it removes. The risk that matters is a *new* one. | `scripts/check-security-unwraps.sh` records the per-file count in `scripts/security-unwrap-baseline.txt` and fails any file that exceeds it. A deliberate new call must carry a same-line `// no-panic-ok: <reason>` comment, which makes it reviewable. | 2027-03-28 |
| A-2 | `ragent-bench` and `ragent-tools-vcs` keep a crate-local guard name | SEC-ragent-bench-004, SEC-ragent-tools-vcs-001/002 | Renaming `contained_join` / `reject_option_like` at every call site would touch ~20 files for no behavioural gain. The rule itself is shared, which is what MS-05 asked for. | Both functions are thin adapters that call `ragent_types::guard` in their body. `scripts/check-shared-guards.py` allowlists the two files *only while the delegation call is present*, and `scripts/check-vcs-duplication.sh` fails if the leading-dash check is re-derived. | 2027-03-28 |
| A-3 | `browser`/`mf_screenshot` remain non-functional in the Rust runtime | SEC-tools-extended-001 (partially) | The integrated runtime has no headless browser engine; the navigation guard is enforced at the tool boundary but the tool itself cannot run. | The `browser` `open` action validates the scheme and applies the SSRF host check before any navigation, and `mf_screenshot` returns an explicit error recommending `mf_fetch`. | 2027-03-28 |
