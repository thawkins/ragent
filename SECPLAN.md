# Security Remediation Plan (SECPLAN.md)

> Generated from a focused security review of the ragent codebase.
> Priorities follow the project convention: 0 = Critical, 1 = High, 2 = Medium, 3 = Low.

## 0 — Critical

### C-001 — SSRF: legacy `http_request` and `webfetch` bypass MasterFetch SSRF guard
- **Where:** `crates/ragent-tools-extended/src/http_request.rs:70-109`, `crates/ragent-tools-extended/src/webfetch.rs:100-109`
- **Risk:** Tools accept arbitrary URLs/methods/headers/bodies without calling `masterfetch::security::validate_url`. They can reach `169.254.169.254`, `127.0.0.1`, `localhost`, internal APIs, and cloud metadata endpoints.
- **Remediation:** Route every URL in `http_request` and `webfetch` through `ragent_tools_extended::masterfetch::security::validate_url` before issuing the request. Reject non-http(s) schemes and blocked hosts/IPs. Add regression tests that prove loopback/metadata/private IPs are rejected.

### C-002 — Path traversal in file tools
- **Where:** `crates/ragent-tools-core/src/lib.rs:130-174` (`check_path_within_root`), `crates/ragent-tools-core/src/glob.rs:70-94`, `crates/ragent-tools-core/src/path_util.rs:24-30` (`resolve_path`), and callers such as `replace.rs`, `read.rs`, `write.rs`, `move.rs`, `copy.rs`, `append.rs`, `mkdir.rs`, `rm.rs`.
- **Risk:** `check_path_within_root` relies on `starts_with` after partial canonicalization and can be bypassed when the target does not exist or the root is not canonicalized. `glob` never validates that its `path` parameter stays inside the working root.
- **Remediation:** Harden `check_path_within_root` to canonicalize both the working root and the target path and verify the target is a true prefix using path components (not string `starts_with`). Enforce the check on all file-tool entry points, including `glob`, `list`, and `file_info`. Add tests for `..`, symlink escape, and absolute-path traversal.

### C-003 — Shell command injection via state-file quoting
- **Where:** `crates/ragent-tools-core/src/bash.rs:847-873` (`build_posix_wrapper`), `crates/ragent-tools-core/src/bash.rs:1199-1200` (script file write)
- **Risk:** The wrapper interpolates `state_file`, `script_file`, and the command into shell and PowerShell scripts without shell escaping. A session ID or working directory containing quotes/backticks can break out of the wrapper.
- **Remediation:** Escape every interpolated value according to the target shell (POSIX single-quote or PowerShell quoting), or pass values via environment variables or argv instead of embedding them in generated scripts. Add property tests with malicious session IDs and paths.

## 1 — High

### H-001 — TUI slash commands mutate security policy without confirmation
- **Where:** `crates/ragent-tui/src/app/slash.rs:4193-4209` (`/bash add allow`), `slash.rs:4211-4226` (`/bash add deny`), `slash.rs:4441-4473` (`/dirs add allow/deny`), `slash.rs:4536-4582` (`/yolo`)
- **Risk:** Any TUI user (or an LLM acting on a malicious prompt) can widen the command allowlist, shrink the denylist, enable YOLO mode, or broaden file permissions, bypassing the 7-layer shell security.
- **Remediation:** Add a `SlashCommandPermission` gate requiring explicit confirmation for security-affecting mutations. Persisted lists should require the same authorization level as `/yolo`. Log every mutation with the current session ID.

### H-002 — TUI slash commands are vulnerable to prompt injection
- **Where:** `slash.rs:1564-1600` (`/init`), `slash.rs:4713-4796` (`/swarm`), `slash.rs:4907-6840` (`/spec` family), `slash.rs:2575-2592` (`/system`), `slash.rs:8147-8284` (skill fallback), `swarm.rs:145-158`
- **Risk:** Raw user `args` are interpolated into LLM prompts and system instructions. An attacker can override prior instructions by embedding delimiters.
- **Remediation:** Treat slash arguments as untrusted user content. Escape or delimit them before interpolation, or wrap them in an unambiguous structure (e.g., XML/JSON with content escaping). For `/system`, require confirmation before replacing the system prompt.

### H-003 — `/update install` downloads and replaces the binary without verification
- **Where:** `slash.rs:7036-7040`, `updater/mod.rs:125-156`
- **Risk:** Release assets are selected by platform substring and installed over the running executable with no signature, checksum, or HTTPS pinning verification.
- **Remediation:** Publish a signed manifest (signature + SHA-256) alongside release assets and verify it before replacing the binary. Pin the HTTPS connection and validate the certificate chain. Fall back to requiring the user to confirm the update hash.

### H-004 — Archive import deserializes untrusted JSON into security-sensitive types
- **Where:** `crates/ragent-agent/src/session/archive.rs:423-500`
- **Risk:** `serde_json::from_str` is used for `manifest.json`, `transcript.json`, `triggers.json`, and `cron_jobs.json`. Although checksums are verified, no schema/version/length validation prevents oversized or malformed payloads from being deserialized into cron/trigger structures that may execute later.
- **Remediation:** Add deserialization limits (max length, max array size, max string length), require a supported `manifest_version`, validate cron/trigger payloads after deserialization, and refuse imports whose checksum verification is disabled by default. Treat imported archives as untrusted.

### H-005 — `/webapi enable` starts an HTTP server with minimal hardening
- **Where:** `slash.rs:7215-7268`, `routes/mod.rs:80-141`
- **Risk:** The server may bind to all interfaces with a token printed to the UI. The token can leak into logs/transcripts, and the shutdown path aborts the task without graceful drain.
- **Remediation:** Default to `127.0.0.1` binding only. Store tokens in the encrypted credential store instead of printing them in plaintext. Redact the token in logs and bug reports. Implement graceful shutdown with request draining.

### H-006 — Credential storage may leak secrets to logs and transcripts
- **Where:** `crates/ragent-storage/src/storage.rs` (provider auth), `crates/ragent-agent/src/session/processor.rs:2125-2172`, `crates/ragent-tools-core/src/sanitize.rs` (redaction), `slash.rs` (bug report generation)
- **Risk:** API keys are stored in SQLite; `redact_secrets` is regex-based and may miss novel secret formats. Session transcripts and bug reports can still contain user secrets.
- **Remediation:** Use the encrypted credential store for all provider tokens. Audit `redact_secrets` patterns against common key formats (`sk-`, `AIza*`, `ghp_`, `hf_`, `Bearer`, etc.). Scrub messages before writing bug reports or session archives.

## 2 — Medium

### M-001 — `/doctor` spawns executables from PATH without hardening
- **Where:** `slash.rs:7117-7211`, `slash.rs:7126`, `slash.rs:7134`
- **Risk:** `git --version` and `rg --version` are looked up via PATH. A manipulated PATH can cause arbitrary code execution.
- **Remediation:** Resolve known-safe full paths at startup or use a fixed allowlist of directories. Validate executable identity before spawning.

### M-002 — Research tool accepts unvalidated `--from-url` and `file://` URLs
- **Where:** `crates/ragent-tui/src/app/research.rs:110-140`, `crates/ragent-research/src/cli.rs`, `crates/ragent-research/src/web_gatherer.rs`
- **Risk:** Research fetches can hit internal hosts or local files if the URL is not routed through `validate_url`.
- **Remediation:** Pass research URLs through `masterfetch::security::validate_url`. Explicitly reject `file://`, `ftp://`, and private-range targets. Add allowlist support for internal research endpoints.

### M-003 — `extract_quoted_prompt` is fragile and can smuggle unquoted tokens
- **Where:** `slash.rs:9420-9429`
- **Risk:** The function uses the first and last `"`, so input like `foo "bar" baz "qux"` extracts `bar" → "qux`, leaving unquoted tokens to be parsed as command arguments.
- **Remediation:** Replace with a proper quoted-token parser or reject input containing more than one balanced quote pair when a quoted prompt is expected.

### M-004 — Spec/team commands use `block_in_place` + `block_on` extensively
- **Where:** `slash.rs:4990`, `slash.rs:5490`, and similar throughout the spec handlers
- **Risk:** Calling `block_on` inside `block_in_place` risks executor contention and runtime panics if called outside a Tokio runtime.
- **Remediation:** Convert spec operations into proper async handlers awaited directly by the TUI event loop. Remove nested `block_on` calls.

### M-005 — Team cleanup/delete lacks robust active-teammate checks
- **Where:** `slash.rs:3214-3281`, `slash.rs:3624-3676`, `slash.rs:3678-3763`, `slash.rs:3765-3918`
- **Risk:** Directory removal is guarded by an in-memory cache that may be stale or racy. `/team forcecleanup` can be confirmed by appending `confirm`, which is easy to script.
- **Remediation:** Check the canonical team runtime state before destructive operations. Require typed confirmation for `forcecleanup` and log the operation.

## 3 — Low

### L-001 — CDP browser traffic is plaintext `ws://` to localhost
- **Where:** `crates/ragent-tools-extended/src/browser/cdp.rs:169`, `browser/mod.rs:62`
- **Risk:** Browser DevTools Protocol is unencrypted and has no origin/auth check, but it is localhost-only.
- **Remediation:** Document the risk. When Chrome supports it, prefer `wss://` or require an `--remote-debugging-auth-token` and validate it on connect.

### L-002 — `/goal` command stores no state
- **Where:** `slash.rs:9739-9759`
- **Risk:** The autonomous stop hook is advertised but not wired.
- **Remediation:** Either implement persistence for `GoalCondition` or remove/hide the command until it is functional.

### L-003 — `ragent_config::bash_lists` and `dir_lists` are global mutable statics
- **Where:** `ragent_config/src/bash_lists.rs`, `ragent_config/src/dir_lists.rs` (implied from usage in `slash.rs`)
- **Risk:** Runtime mutation of security policy from multiple tasks is racy and hard to test.
- **Remediation:** Move mutable policy state behind an async-aware lock and reload from a canonical, validated config file on startup.

## Verification checklist

- [ ] C-001: `http_request` and `webfetch` reject loopback/private/metadata URLs.
- [ ] C-002: All file tools reject traversal outside the working root in tests.
- [x] C-003: Malicious session IDs and paths cannot break out of the bash wrapper.
- [ ] H-001: Security-affecting slash commands require explicit confirmation.
- [ ] H-002: Prompt-injection payloads in slash args no longer alter agent behavior.
- [ ] H-003: Binary updates verify a publisher signature or checksum manifest.
- [ ] H-004: Archive import validates manifest version, checksums, and payload limits.
- [ ] H-005: Web API defaults to localhost and does not log its bearer token.
- [ ] H-006: API keys are stored encrypted and redacted from logs/transcripts.
- [ ] M-001: `/doctor` resolves executables to known-safe paths.
- [ ] M-002: Research URL fetches pass SSRF validation.
- [ ] M-003: `extract_quoted_prompt` handles or rejects multiple quote pairs.
- [ ] M-004: No nested `block_on` remains in TUI slash handlers.
- [ ] M-005: Team deletion checks canonical runtime state and logs operations.
