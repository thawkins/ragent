# Security Review Findings — ragent-core / ragent-server / ragent-tui

Reviewer: security-reviewer
Scope: crates/ragent-core, crates/ragent-server, crates/ragent-tui
Date: 2026-03-30

This document lists security and risky-code findings discovered during a focused review of the three crates above. For each finding: a short description, severity, reproducible steps (how to trigger / verify), and suggested fixes.

Summary of high-level problems
- Arbitrary shell execution surface (bash tool, other command invocations) with insufficient sanitisation. (High)
- Secrets stored with weak obfuscation in SQLite and potential SSE leak paths. (High)
- Synchronous std::sync::Mutex usage and blocking std::process::Command in async code — both create blocking/latency and deadlock risks. (High / Performance)
- File operation APIs (read/write/commit) accept filesystem paths from callers and lack robust canonicalisation / anti-TOCTOU / symlink protections. (High)
- Path resolution for the `read` tool permits absolute / parent traversal allowing data exfiltration. (High)
- Secret redaction regex can miss tokens and is brittle. (Medium)
- Multiple direct unwrap()/expect()/panic sites that could crash the server if triggered. (Medium)

For each finding below I include file references and steps to reproduce/verify.

---

1) Arbitrary shell/command execution (bash tool, other command callers)

Files / locations:
- crates/ragent-core/src/tool/bash.rs
- crates/ragent-core/src/skill/context.rs (runs `sh -c` - search shows usage)
- crates/ragent-core/src/provider/copilot.rs (uses std::process::Command to call `gh`)
- crates/ragent-tui/src/app.rs (detect_git_branch uses std::process::Command `git`)

Description:
BashTool executes user-specified strings using `bash -c` (bash.rs). The implementation attempts to blacklist dangerous literal substrings (DENIED_PATTERNS) but that approach is easy to bypass. The tool runs arbitrary commands in the agent's working directory and returns combined stdout/stderr into agent context.

Severity: High. Remote or local users with permission to call the tool can execute arbitrary commands. Blacklist checks are incomplete and other process-spawning uses (gh/git) further increase attack surface.

Reproduction steps:
1. Start an instance of ragent and call the `bash` tool with a command that uses a different interpreter to perform harmful actions — e.g.:
   - python -c "import os; os.system('rm -rf /some/target')"
   The string does not include the literal "rm -rf /" and may bypass the DENIED_PATTERNS blacklist.
2. Or run a command that chains commands via environment / shell constructs that avoid the literal blacklisted substrings.
3. Observe that the command runs and output is returned.

Suggested fixes:
- Avoid shell execution of user-provided command strings where possible. Provide an API that accepts a command and arguments (Vec<String>) and runs Command::new(program).args(args) without `-c` shell interpolation.
- If free-form shell execution is required, implement strict allowlist-based policy rather than blacklists; require explicit permission/consent for dangerous categories and document risks.
- Run commands under strong sandboxing (e.g., dedicated container, chroot, seccomp, or an unprivileged user) and restrict working directory visibility.
- For long-term: remove bash -c usages and expose specific tool functions rather than arbitrary shell access.
- Use async-aware process APIs (tokio::process::Command) when called from async code to avoid blocking the runtime inbox.

References:
- file: crates/ragent-core/src/tool/bash.rs (lines 81-88 executes bash -c)

---

2) Path traversal & arbitrary file access via Read tool and other path-accepting APIs

Files / locations:
- crates/ragent-core/src/tool/read.rs (resolve_path)
- crates/ragent-core/src/file_ops/mod.rs (staging, commit_all reads/writes)
- crates/ragent-core/src/skill/loader.rs and other places that read user paths

Description:
Tools that accept a `path` string (read, edit, file ops, edit staging) accept absolute paths and join relative paths with the working dir but do not canonicalize or restrict the resolved path to a safe project-root sandbox. This allows reading/writing files outside the intended working directory via `..` or absolute paths.

Severity: High (data exfiltration / accidental overwrites).

Reproduction steps:
1. From an agent session whose working_dir is /home/user/project call the `read` tool with path: `../../etc/passwd` or `/etc/passwd`.
2. The tool will resolve the path and read the file (if process user has permission) and return content.

For write path attack (commit):
1. Create a symlink inside project to point to e.g. /etc/passwd: ln -s /etc/passwd project/attack.txt
2. Stage an edit for project/attack.txt and commit
3. The commit_all code will follow the symlink and write update the target file, potentially overwriting system files.

Suggested fixes:
- Canonicalize and validate: after resolving a path, call std::fs::canonicalize (or tokio::fs equivalent) and ensure the canonical path is a descendant of an allowed root (the session's working directory or a configured project root). Reject absolute paths outside the allowed root.
- Deny symlink writes: when performing write/rename operations, open files with flags that prevent following symlinks (O_NOFOLLOW) or check that the path is not a symlink before writing.
- Use atomic, safe file write helpers (tempfile crate with persist) and perform fsync on directory and file to avoid data loss.
- Add explicit permission categories and user consent for operations that write outside the project root.

References:
- read.rs resolve_path (lines 205–211)
- file_ops::commit_all writes tmp and rename without symlink checks (lines ~208–249 of file_ops/mod.rs)

---

3) Weak secret storage: XOR obfuscation in Storage

Files / locations:
- crates/ragent-core/src/storage/mod.rs (obfuscate_key / deobfuscate_key)

Description:
Provider API keys are stored in the database using a fixed repeating-key XOR followed by base64. The code explicitly documents this is only obfuscation and not encryption. The fixed key in source means anyone with DB access or repo can fully recover secrets using deobfuscate_key.

Severity: High (secrets compromise if DB file leaked or an attacker has read access).

Reproduction steps:
1. Obtain a copy of the sqlite DB or query provider_auth table (if attacker can read DB).
2. Use Storage::deobfuscate_key(encoded) or the same algorithm to recover the plaintext API key.

Suggested fixes:
- Use platform keyring/credential store for production (OS keyring, macOS Keychain, Windows Credential Manager, Linux secret service). Use crates like `keyring` or `secret-service`.
- If DB storage is required, encrypt secrets at rest using a process-level master key (provided via env or KMS) and use a vetted crypto library (AES-GCM with proper key management). Do not implement custom XOR obfuscation.
- Remove the hard-coded OBFUSCATION_KEY from source and rotate any existing keys stored using the old scheme.

Reference:
- storage/mod.rs lines 17–48 (obfuscate_key/deobfuscate_key)

---

4) Secrets leakage via SSE / event serialization

Files / locations:
- crates/ragent-server/src/sse.rs (event_to_sse and serialization)
- crates/ragent-server/src/routes/mod.rs (SSE endpoints event bus)
- crates/ragent-core/src/sanitize.rs (redact_secrets)

Description:
Server-sent events shuttle Event payloads (ToolResult.content, metadata, model responses) to subscribed clients. These payloads may include API keys, file contents, or tokens returned by providers. While COMPLIANCE.md contains mitigation notes and tests exist for some cases, SSE still exposes a high-leakage channel if payloads are not reliably sanitized. The sanitiser relies on regex patterns that may miss some secret formats.

Severity: High (secrets can be delivered to any authenticated SSE client; compromise of a token or misconfiguration leads to disclosure).

Reproduction steps:
1. Make a tool or skill emit a string containing an API key or file content in a ToolResult (for example, programmatically generate an event containing "sk-..." or full file content).
2. Connect to /events or a session SSE endpoint with a valid bearer token and observe the raw SSE payload containing the secret.

Suggested fixes:
- Before publishing to SSE, always pass event payloads through a robust sanitiser and ensure all sensitive fields (ToolResult.content, ToolResult.metadata, ModelResponse.text) are sanitized.
- Extend sanitiser to detect more token patterns and apply multiple heuristics: detect lines containing "Authorization:" headers, key-like base64 blobs, and common provider prefixes (sk-, key-, ghu_, ghp_, gho_, etc.). Consider building a denylist and canonicalizing values before matching.
- Consider redaction at the serialization boundary: remove or redact entire fields (e.g., send token_present: true instead of the token itself) rather than attempting to redact inlined tokens.
- Limit SSE scope: restrict per-session SSE subscriptions strictly; consider additional claims/roles and short-lived streaming tokens; log SSE subscribers and add admin controls to revoke streaming tokens.

References:
- ragent-server COMPLIANCE.md already notes SSE leakage risk; tests in tests/test_event_to_sse.rs exercise some redactions.
- sanitize.rs regex and implementation (may miss some tokens)

---

5) Use of std::sync::Mutex and blocking std::process::Command inside async functions

Files / locations:
- crates/ragent-core/src/provider/copilot.rs (static SESSION_TOKEN_CACHE: std::sync::Mutex)
- other places call std::process::Command (copilot.rs find_gh_cli_token, ragent-tui detect_git_branch, etc.)

Description:
A global static std::sync::Mutex is used to protect a token cache but it is locked inside async functions (try_copilot_token_exchange and other await points). std::sync::Mutex is blocking; holding it or acquiring it while awaiting can block the executor threads and create contention or deadlocks. Similarly, std::process::Command::output() is blocking — called in synchronous functions that may be invoked from async contexts, causing thread blocking.

Severity: High (both correctness and performance risk; can degrade throughput, cause thread pool starvation, and increase latency). Also unwrap() on Mutex lock can panic if poisoned.

Reproduction steps:
1. Simulate many concurrent requests that call into code that executes try_copilot_token_exchange — e.g., numerous agent actions that request copilot model discovery.
2. Observe high latency, executor stalls, or contention when threads block attempting to lock the std::sync::Mutex or waiting on external process completion.

Suggested fixes:
- Replace std::sync::Mutex static usage with tokio::sync::Mutex or an async-aware lock or use parking_lot::Mutex but be careful not to hold while awaiting. For a global cache, consider using an async lock with minimal critical section or atomic types (RwLock) and clone cached data out before awaiting.
- Replace std::process::Command usages with tokio::process::Command when invoked from async code; spawn blocking processes on a dedicated blocking thread via tokio::task::spawn_blocking if external blocking is unavoidable.
- Avoid holding locks across await points: limit scope of lock to pure-memory operations and release before performing network/IO.
- Do not call .unwrap() on mutex locks — map and propagate errors instead of panicking on poisoned locks.

References:
- copilot.rs SESSION_TOKEN_CACHE static (line near top of provider/copilot.rs)
- try_copilot_token_exchange uses lock().unwrap() inside async function (lines ~930)
- find_gh_cli_token uses std::process::Command::output (blocking), called in sync function find_gh_cli_token (may be invoked from async call chains)

---

6) Weak/missable secret sanitiser (regex)

Files / locations:
- crates/ragent-core/src/sanitize.rs

Description:
The secret-redaction regex is a reasonable starter, but it is easy to craft tokens that do not match the pattern (different prefixes, separators, punctuation, underscores, etc.). The regex also captures the literal "Bearer <token>" only if preceded by the exact word and token chars match the limited class. Other sensitive strings (e.g., environment variables printed without these prefixes, base64 API keys, JWTs with dots) may not be matched.

Severity: Medium — may result in missed redactions and secret leakage.

Reproduction steps:
1. Create an event that contains a token with an underscore or a dot (e.g., "Bearer ghu_abcdef.ghi") and publish it.
2. Subscribe to SSE or inspect logs — the token may not be redacted.

Suggested fixes:
- Expand the sanitizer to include additional known token formats used by providers (ghu_, gho_, github_pat_, ghp_, jwt patterns with dots, generic long base64-like strings). Use layered checks rather than a single regex.
- Wherever possible, avoid returning or logging secrets at all. For example return token_present boolean instead of token content.
- Add unit tests enumerating known token formats and confirm redaction across the codebase.

Reference:
- sanitize.rs (simple regex at lines 5–9)

---

7) Multiple unwraps / panics in runtime (service crash surface)

Files / locations:
- Many locations across crates; examples: provider/copilot.rs lock().unwrap(), storage.deobfuscate_key unwraps in UTF-8 conversion, file_ops concurrency permits unwrap, etc.

Description:
Multiple `.unwrap()`/`.expect()` uses (notably on Mutex::lock, Connection operations, file path operations) can lead to panics and crash the service if erroneous inputs or poisoned locks appear in production.

Severity: Medium.

Reproduction steps:
1. Cause a Mutex to be poisoned by causing panic while holding the lock (harder to trigger deliberately but possible during tests or bugs).
2. Observe next access panics due to unwrap.

Suggested fixes:
- Replace `unwrap()` on locks with proper error handling (map_err to a Result and propagate). Use `LockResult` mapping macros already present for DB lock (see lock_conn!) and mirror that pattern for other mutexes.
- Audit codebase for runtime unwraps in non-test code and convert to error returns with context.

---

8) File write atomicity and symlink/TOCTOU risks in commit_all

Files / locations:
- crates/ragent-core/src/file_ops/mod.rs commit_all implementation

Description:
The commit_all implementation writes a tmp file next to the target then performs `tokio::fs::rename(tmp, path)`. While this is an atomic pattern on POSIX when tmp is in same filesystem, the code doesn't prevent an attacker from changing the path into a symlink between checks (TOCTOU), nor does it open the target securely to avoid following symlinks. A malicious actor with write access to the directory could replace files or symlinks between the checksum check and rename, resulting in overwriting critical files.

Severity: High (arbitrary file overwrite).

Reproduction steps:
1. Create a symlink inside the project that points to /etc/important.
2. Stage an edit for that symlink path and call commit_all.
3. The server will write to the temp file and rename, thereby overwriting /etc/important.

Suggested fixes:
- Use platform APIs that open files with O_NOFOLLOW / O_TMPFILE if available, or perform open-by-fd checks and rename via fs::replace if supported.
- Validate that the canonicalized target path is inside the allowed root before writing; re-check before final rename and fail if it changed.
- Consider using the `fs-secure` or `openat`-style approach to avoid symlink races. On Unix, open the parent directory FD and use openat with O_NOFOLLOW / O_EXCL.
- Make commits via a dedicated worker with minimal privileges and run them under a distinct unprivileged user or sandbox.

Reference:
- file_ops/mod.rs lines ~206–250 (tmp backup write and rename)

---

9) Logging partial secrets

Files / locations:
- crates/ragent-core/src/provider/copilot.rs logs first 8 chars of token (println! and tracing::info lines)

Description:
Some logs print token prefixes (first 8 chars) which may leak secret fragments in log storage. While commonly used to identify tokens, storing token prefixes increases risk if logs are accessible.

Severity: Medium / Low depending on log retention and access.

Suggested fixes:
- Avoid logging token content even partially. Log only `token_present: true` or a hashed identifier (HMAC) derived from token using a key not stored with logs.

---

10) Misc: permission and rate limiting hardening

Files / locations:
- crates/ragent-server/src/routes/mod.rs (auth middleware, rate limiter structure)

Notes:
- Auth middleware uses constant-time comparison which is good; however bearer token extraction uses header[7..] without trimming trailing spaces — recommend trimming and normalising the provided token before comparison.
- Rate limiter is in-memory and protected by tokio::sync::Mutex; it evicts entries older than 120s only on access — this is acceptable for prototype but consider robust rate-limiting algorithms (token bucket) for production and per-token or per-client quotas persisted across restarts.

Suggested improvements:
- Trim and canonicalize auth header token before comparison.
- Consider using crate(s) such as `governor` or `tower` middleware for rate-limiting and per-key quotas.

---

Suggested short-term action items (priority order)
1. Patch immediate high-risk items:
   - Stop using free-form `bash -c` for arbitrary user-provided strings; at minimum, make this tool require administrator consent and stronger allowlist/sandboxing. (High)
   - Canonicalize paths for read/write tools and reject paths outside the session working dir. (High)
   - Replace fixed XOR obfuscation with secure storage of secrets (keyring / KMS / encrypted at rest). Rotate stored provider credentials. (High)
   - Replace std::sync::Mutex and blocking process::Command calls used in async paths with async-friendly primitives (tokio::sync::Mutex, tokio::process::Command or spawn_blocking). (High / Performance)
   - Sanitize SSE payloads by removing entire fields that can contain secrets before publishing rather than relying on regex-only redaction. (High)

2. Medium-term:
   - Harden file write logic to avoid TOCTOU and symlink attacks.
   - Improve secret redaction tests and regex coverage.
   - Audit and remove/handle remaining unwraps in production code paths.
   - Add CI tests that exercise large-scale concurrency against copilot token cache to ensure no deadlocks.

3. Monitoring / config changes:
   - Reduce log retention of any logs containing token fragments; consider scrubbing historic logs.
   - Add feature flags that disable all external process invocation in hardened builds.

---

If you want, I can:
- Produce a prioritized patch plan with specific diffs for the highest-risk items (e.g., change SESSION_TOKEN_CACHE to tokio::sync::Mutex; canonicalize and check paths in read.rs; remove bash -c usage) and submit it for review.
- Implement and test a focused fix (one of the above) and run cargo check / tests.

End of findings.
