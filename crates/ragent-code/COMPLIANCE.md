# COMPLIANCE REVIEW — crates/ragent-code

NOTE: crates/ragent-code does not exist in this workspace. This compliance document targets similarly-named crates and the code paths that exercise agent command execution, auth, secret handling, and file tools. Findings reference concrete files in the repository under crates/ragent-server and crates/ragent-core.

1) Executive summary

- Major issues (High severity):
  - Secrets are stored with reversible XOR "obfuscation" using a hardcoded key (crates/ragent-core/src/storage/mod.rs). This offers almost no protection against an attacker with DB access.
  - Arbitrary shell execution via BashTool uses `bash -c` and only substring-blocks a small list of dangerous patterns; this leaves many destructive or evasive commands possible and logs full command text (crates/ragent-core/src/tool/bash.rs).
  - File tools (edit/create/rm/patch/multiedit/read/.. ) resolve paths by joining with the working directory without canonicalizing and verifying the resulting path is inside the session working directory. This allows path traversal and access outside the intended workspace (many files under crates/ragent-core/src/tool/*; examples: edit.rs, create.rs, rm.rs, read.rs, multiedit.rs, patch.rs).

- Medium issues:
  - Single static bearer token stored as plain String in AppState and used for all API clients; no token store, no expiration or per-token scope (crates/ragent-server/src/routes/mod.rs). In-memory token handling and logging must be hardened (use secrecy::SecretString, avoid logging tokens).
  - Logging of potentially sensitive values (commands, full errors) without always applying redact_secrets. Some error logs use redact_secrets, but many info/debug logs still print raw command/prompt text which may include secrets (bash.rs tracing::info). 
  - SQLite Connection wrapped in Mutex serializes all DB access; this can be a performance bottleneck (crates/ragent-core/src/storage/mod.rs).

- Low issues:
  - get_provider_auth silently returns empty string on decode failure, which can cause downstream clients to attempt unauthenticated calls or fail in unclear ways (crates/ragent-core/src/storage/mod.rs).
  - Several tools allow absolute paths; absolute paths may be intentionally allowed but must be validated against session policy.

2) Security issues (with severity, reproduction steps, suggested fixes)

Issue A — Reversible "obfuscation" of API keys (High)
- Location: crates/ragent-core/src/storage/mod.rs (OBFUSCATION_KEY, obfuscate_key, deobfuscate_key)
- Severity: High — attacker with DB file or backup can trivially recover API keys
- Reproduction steps:
  1. Obtain sqlite DB `provider_auth` row (api_key column contains base64 string).
  2. Base64-decode and XOR with the static OBFUSCATION_KEY to recover plaintext (see deobfuscate_key implementation).
- Impact: full compromise of provider API keys, which can be used to exfiltrate data, consume billing quota, or make privileged LLM calls.
- Suggested fixes (short-term -> long-term):
  1. Immediate (low-effort): Document the weakness and ensure any released artifacts and backups are treated as sensitive. Add runtime detection of weak obfuscation during startup and print a warning (non-sensitive).
  2. Medium: Replace obfuscation with an encryption mechanism keyed by a secret stored outside the DB (environment variable or OS keystore). Use a well-vetted crate (e.g., `ring` or `aes-gcm` from `aes-gcm`/`aead`) and seal keys in the OS keyring or use `secrecy::SecretString` for in-memory handling.
  3. Long-term: Integrate with OS-native secure storage (macOS Keychain, Windows Credential Manager, Linux secret service) via `keyring` crate or provide an optional KMS-based encryption key. Provide a migration tool that can re-encrypt existing keys.
- Concrete code changes:
  - Remove OBFUSCATION_KEY constant and obfuscate_key/deobfuscate_key public methods. Replace with encrypt_key/decrypt_key that use an AEAD and a KeyProvider abstraction.
  - Store encrypted bytes (base64) in DB. Ensure decrypt returns Result<Option<SecretString>, Error> and fails loudly if key missing or corrupted.
  - Use zeroing memory types (secrecy::SecretVec/SecretString) for in-memory key handling.

Issue B — Unsafe shell execution (High)
- Location: crates/ragent-core/src/tool/bash.rs (BashTool::execute)
- Severity: High
- Reproduction steps:
  1. Call the bash tool with a command string such as: "sh -c 'rm -rf /tmp/somefile'" or use multiple steps or obfuscated forms that avoid the DENIED_PATTERNS substring checks (e.g. using environment variables, hex representation, or path tricks). The DENIED_PATTERNS list is trivial to bypass.
  2. Observe the process is executed via `bash -c` with the full command and working_dir, possibly deleting files outside the intended sandbox.
- Impact: Arbitrary command execution, data destruction, privilege escalation if service runs with high privileges.
- Suggested fixes:
  1. Avoid invoking a shell with a single string. Prefer executing specific binaries with an argv vector (Command::new("/bin/ls").arg("-l")). If arbitrary commands are required, parse and validate them strictly or require explicit operator opt-in.
  2. Implement a strict allowlist/denylist for commands (prefer allowlist) and validate binary path against a secure absolute path, not just name.
  3. Sandbox execution: run commands in a container (e.g., Firecracker, Docker), use seccomp, chroot, drop capabilities, run under a dedicated unprivileged user, use cgroups to limit CPU/memory, and mount the working directory read-only if appropriate.
  4. Remove or mask logging of full command strings. If logging is necessary, redact sensitive tokens and limit log level.
  5. Always sanitize the environment (clear PATH and set a safe PATH) and avoid inherited file descriptors.
- Concrete code changes:
  - Replace `Command::new("bash").arg("-c").arg(command)` with a function that either: a) parses a JSON array of argv (preferred), or b) rejects commands containing shell metacharacters and executes via Command::new(binary). Provide an explicit `shell` tool only if operator config enables it and only inside a strict sandbox.

Issue C — Path traversal / working-dir escape in file tools (High)
- Location examples: crates/ragent-core/src/tool/edit.rs (resolve_path at end), create.rs, rm.rs, read.rs, multiedit.rs, patch.rs, office_* tools
- Severity: High
- Reproduction steps:
  1. Create a session pointing to /home/user/project (server ensures canonicalized on session creation).
  2. Use a file tool with path "../secrets/.env" or "/etc/passwd" (absolute) and observe the tool will join/accept and operate on it because resolve_path allows absolute and non-canonical relative paths.
- Impact: Agents or tools can read, edit, or delete files outside the intended project workspace.
- Suggested fixes:
  1. Centralize path resolution and enforce a containment check: after constructing the candidate path, canonicalize it (tokio::fs::canonicalize) and verify it starts_with the canonical working_dir. Reject operations if not inside the working directory unless an explicit operator policy allows it.
  2. For absolute paths, canonicalize and perform the same containment check (or deny absolute paths entirely unless explicitly permitted via configuration).
  3. Add a unit/integration test that fails if any tool allows operating outside working_dir.
- Concrete code changes:
  - Add a new helper API in crate::tool::path_utils::resolve_and_validate(working_dir, path_str) -> Result<PathBuf, Error>. Replace all resolve_path implementations in tools with calls to this helper and return a clear error message when outside allowed area.

Issue D — Single static bearer token + in-memory plain string and incomplete token management (Medium-High)
- Location: crates/ragent-server/src/routes/mod.rs (AppState.auth_token, auth_middleware)
- Severity: Medium-High
- Reproduction steps:
  1. Configure server with a single token; any client supplying Authorization: Bearer <token> can access all protected endpoints.
  2. No built-in revocation, rotation, per-token scope, or logging masking.
- Impact: If token is leaked, full API compromise until operator rotates token and restarts the server.
- Suggested fixes:
  1. Replace single-string token with a token store table in Storage (provider: tokens) permitting multiple tokens, scopes (read/write/task permissions), created_at, expires_at, and optional human owner. Authenticate by lookup using secure constant-time comparison.
  2. Store tokens with hashed form (e.g., HMAC or bcrypt/argon2 of token) so server verifies by hashing the presented token; do not store plaintext tokens. Use salted hashes and avoid reversible storage.
  3. Use secrecy::SecretString for token storage in memory and avoid logging tokens. Use redact_secrets for any logs that may contain tokens.
  4. Add admin endpoints for token rotation and revocation.
- Concrete code changes:
  - Implement token table and Storage APIs: set_token, get_token (hashed), delete_token, list_tokens.
  - In auth_middleware, fetch token metadata and compare hashed token using constant_time_eq on the hash or HMAC.

Issue E — Logging of sensitive payloads (Medium)
- Location: e.g., crates/ragent-core/src/tool/bash.rs (tracing::info!(command = %command, ...)) and many places where prompts or commands are logged.
- Severity: Medium
- Reproduction: Observe logs by running server and invoking tools that include provider keys or secrets in the commands/prompts; the logs will contain those strings.
- Suggestion: Ensure redact_secrets is used before logging free-form strings. Introduce a single helper log_redacted(s: &str) -> masked that uses crate::sanitize::redact_secrets. Use structured logging fields for non-secret metadata only.

3) Test coverage gaps and recommended tests

Priority tests to catch the above issues. Add tests under relevant crates (crate unit tests in ragent-core/tests and integration tests in ragent-server/tests). Suggested tests:

Unit tests (fast, low-effort)
- storage: test that provider API keys are encrypted (after migration) and that decryption fails when key provider not configured. (new tests for Storage set/get provider auth using new crypto layer).
- path utils: tests for resolve_and_validate to ensure relative and absolute paths that escape working_dir are rejected; property tests with proptest: random path components containing .., symlinks, unicode, long names.
- bash tool: unit tests asserting that `execute` rejects shell metacharacters when shell-disallowed mode is active; that logging does not include command when config FLAG_HIDE_COMMANDS is set (mock tracing). Use tokio::test.
- auth_middleware: tests verifying authorization succeeds for correct token and fails for wrong; tests for missing header, token with whitespace, prefix-case-insensitivity (Bearer vs bearer). (crates/ragent-server/src/routes/tests)

Integration tests (requires process / DB)
- File tools: create an in-memory session with a temporary directory and attempt to read/write files both inside and outside the directory; assert outside operations are rejected.
- TaskManager / spawn_background: spawn background tasks up to limit and assert limit enforced.
- End-to-end: start router with test AppState, set provider auth in Storage, call POST /opt and ensure provider auth is retrieved and not leaked in logs.

Property tests / fuzzing
- find_replacement_range: fuzz inputs for edge cases (already present tests exist but extend coverage). Use proptest to generate content/needle/new_str combos and assert no panics and correct invariants (edits produce consistent file lengths when replacing whole-line variants).

Test cases to add (file & names):
- crates/ragent-core/tests/test_path_safety.rs: test_resolve_and_validate_prevents_escape
- crates/ragent-core/tests/test_bash_tool_safety.rs: test_bash_tool_rejects_shell_meta, test_bash_tool_timeout
- crates/ragent-server/tests/test_auth_middleware.rs: test_auth_allowed, test_auth_rejected, test_auth_case_insensitive_prefix
- crates/ragent-core/tests/test_storage_key_encryption.rs: test_provider_key_encrypt_decrypt_and_migration

4) Performance hotspots and suggested optimizations

Observed hotspots and mitigations:

A. Global DB Mutex (crates/ragent-core/src/storage/mod.rs)
- Observation: Storage wraps rusqlite::Connection in std::sync::Mutex, serializing access. For workloads with many concurrent reads/writes (SSE events, message writes), this becomes a contention point.
- Recommendation:
  - Use a connection pool (e.g., `r2d2` + rusqlite or switch to `sqlx` with SQLite support) or adopt per-thread connection model with serialized WAL mode for concurrent reads. Enable WAL journal mode to improve concurrency for reads.
  - If sticking with rusqlite single connection, consider splitting read-only fast paths into an in-memory cache (LRU) and only serialize writes.

Estimated effort: Medium (1–2 days) to enable WAL and tune; larger (3–5 days) to adopt pooling.

B. rate_limiter Mutex<HashMap> in AppState (crates/ragent-server/src/routes/mod.rs)
- Observation: Using tokio::sync::Mutex<HashMap<...>> for per-session rate limiting may cause contention when many sessions are active.
- Recommendation: Use a concurrent map like `dashmap` or sharded lock map, or use per-session atomic counters with TTL (e.g., tokio::time::Instant stored in shards). Alternately, use a token-bucket algorithm with per-session state stored in DashMap.

Estimated effort: Small (2–4 hours) to replace with DashMap and unit tests.

C. TaskManager RwLock over large HashMap (crates/ragent-core/src/task/mod.rs)
- Observation: tasks: Arc<RwLock<HashMap<...>>> may be a hotspot if many tasks are inserted/updated concurrently.
- Recommendation: Use DashMap or fine-grained locking per-entry for writes. Keep RwLock for low-frequency operations and use atomic flags for cancel; or use tokio::sync::Mutex per-task id entry.

Estimated effort: Medium (1 day) for conversion and tests.

D. EventBus broadcast and SSE streams
- Observation: Broadcasting large events to many SSE clients can cause memory pressure (cloning events) and slowdowns.
- Recommendation: Ensure Event objects are small or use Arc-ed payloads, and rate-limit SSE or apply backpressure for slow clients. Consider dropping heavy payloads and send deltas.

Estimated effort: Small-medium (half-day to 1 day) to instrument and tune.

5) Remediation plan — Milestones and Tasks

Milestone 1 — Secrets and token hardening (Priority: P0, estimated 3–8 days)
- Task 1.1 (2d) — Replace storage obfuscation with encryption using an AEAD; implement KeyProvider trait (env var + keyring) and Secret types. Add migration command-line tool to re-encrypt existing keys. Update Storage.set/get_provider_auth signatures.
- Task 1.2 (1d) — Replace AppState.auth_token String with token store: hashed tokens table + Storage APIs, and implement middleware changes. Add tests for auth middleware and token revocation.
- Task 1.3 (1d) — Ensure redact_secrets applied in all logs dealing with secrets. Introduce log helper.

Milestone 2 — Path containment & file tool safety (Priority: P0, estimated 2–4 days)
- Task 2.1 (0.5d) — Add centralized path resolver resolve_and_validate that canonicalizes and enforces containment.
- Task 2.2 (1d) — Update all tools (edit, create, rm, read, write, multiedit, patch, office_* etc.) to use the new resolver and add unit tests.
- Task 2.3 (0.5d) — Add integration test verifying tools cannot operate outside session workspace.

Milestone 3 — Shell execution hardening (Priority: P0, estimated 3–7 days)
- Task 3.1 (1d) — Change BashTool to avoid shell whenever possible: accept argv array; add config flag to enable/disable raw shell.
- Task 3.2 (1–2d) — Add allowlist/denylist and argument parsing; restrict PATH and environment, drop capabilities.
- Task 3.3 (1–3d) — Implement sandboxing options (containerized execution) or integrate with an existing sandbox runner. Provide operator config to toggle sandbox mode.

Milestone 4 — Concurrency & performance (Priority: P1, estimated 2–5 days)
- Task 4.1 (0.5d) — Replace rate_limiter Mutex<HashMap> with DashMap; add tests.
- Task 4.2 (1–3d) — Replace TaskManager tasks RwLock with DashMap or sharded storage.
- Task 4.3 (1d) — Evaluate DB performance: enable WAL mode and consider connection pooling.
- Task 4.4 (0.5d) — SSE backpressure: limit number of active SSE clients per session and drop heavy events.

Milestone 5 — Tests, CI, and verification (Priority: P0, estimated 2–3 days)
- Task 5.1 (1d) — Add unit tests described above.
- Task 5.2 (1d) — Add integration tests for path containment, auth middleware, and bash tool policy.
- Task 5.3 (0.5d) — Add CI checks: forbid embedding OBFUSCATION_KEY constant; static analysis to detect logging of fields named "command", "api_key", "token" without redaction.

Total estimated effort: 10–27 days depending on depth of sandboxing and migration tooling.

6) References and checklist for verification

References
- OWASP Top 10 and Injection guidance: https://owasp.org/www-project-top-ten/
- Rust secrecy patterns: `secrecy` crate: https://crates.io/crates/secrecy
- AEAD guidance: `ring` or `aes-gcm`: https://docs.rs/aes-gcm
- Secure storage recommendations / keyrings: `keyring` crate: https://crates.io/crates/keyring
- Process sandboxing: containers (Docker), seccomp, and user namespaces.

Verification checklist (before merge):
- [ ] Secrets: No hardcoded reversible obfuscation constants remain. Storage uses AEAD and KeyProvider. Migration tool exists or documented path.
- [ ] Token auth: AppState no longer stores static plaintext auth_token; token store with hashed tokens implemented and middleware verified.
- [ ] Path containment: All file tools use resolve_and_validate which canonicalizes and enforces containment. Unit tests added.
- [ ] Bash/shell: BashTool no longer executes arbitrary shell strings by default. Shell execution requires explicit enable and runs in a sandboxed environment or is disallowed.
- [ ] Logging: No un-redacted logging of commands, prompts, or secrets. Automated tests to detect logging of fields named api_key/token/command.
- [ ] Tests: New unit and integration tests added (see section 3). CI runs them.
- [ ] Performance: rate_limiter and TaskManager locking hotspots addressed and benchmarks added where needed.

Appendix — Quick code pointers (exact files)
- Storage obfuscation: crates/ragent-core/src/storage/mod.rs (OBFUSCATION_KEY, obfuscate_key/deobfuscate_key)
- Provider auth get/set: same file, Storage::set_provider_auth, get_provider_auth
- Bash tool (shell execution): crates/ragent-core/src/tool/bash.rs
- File tools resolving paths: crates/ragent-core/src/tool/edit.rs (resolve_path), create.rs, rm.rs, read.rs, multiedit.rs, patch.rs, office_* files.
- Path resolution helpers duplicated across tools — search for fn resolve_path in crates/ragent-core/src/tool/
- Auth middleware and AppState: crates/ragent-server/src/routes/mod.rs (AppState.auth_token and auth_middleware)
- Rate limiter: crates/ragent-server/src/routes/mod.rs (send_message rate limiter using tokio::sync::Mutex<HashMap<...>>) 

If you want, I will:
- Prepare a patch that implements resolve_and_validate and replaces resolve_path occurrences (small, high-value change).
- Create unit tests for resolve_and_validate and auth_middleware.
- Draft the storage encryption KeyProvider skeleton and migration tool design.

-- security-reviewer
