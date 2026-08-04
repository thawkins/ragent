# ragent Security Remediation Plan (SECPLAN)

**Version:** 0.1.0-beta.35-security-1  
**Status:** Draft — pending review  
**Scope:** Full Cargo workspace (`crates/`, `src/`, `tests/`) including TUI, server, agent runtime, tool surface, storage, code index, and supply chain.  
**Audience:** Maintainers, security reviewer, contributors.

---

## 1. Executive Summary

ragent is a locally-run AI coding agent with a large tool surface (~150 tools across 18 categories), a built-in HTTP server with SSE streaming, shell execution, file operations, provider credential storage, and external network calls. The project has already undergone several security reviews and has good foundational controls (workspace `unsafe_code = deny`, SSRF validation for web fetch/crawl, regex + exact-match secret redaction, bearer-token constant-time comparison, `cargo-audit`/`cargo-deny` CI, and a `deny.toml` with explicit advisory exceptions). However, a number of medium-to-high risk issues remain unaddressed.

This plan consolidates findings from prior reports and a live code audit into a single prioritized remediation roadmap. The highest-value fixes are:

1. Stop treating the storage encryption as real at-rest protection — move provider keys out of the SQLite DB or protect them with OS keyring/TPM-backed AEAD.
2. Contain all file-system tools to the session working directory with canonicalization and symlink/TOCTOU-safe writes.
3. Reduce shell-execution blast radius: avoid `bash -c` for parsed commands, tighten allowlists/denylists, and provide sandbox opt-in.
4. Harden server auth: trim/normalize bearer tokens, add per-token revocation/scopes, and stop storing a single plaintext token in `AppState`.
5. Sanitize SSE payloads structurally, not only with regex redaction.

---

## 2. Methodology

- **Existing reports reviewed:**
  - `crates/ragent-tui/security_findings.md`
  - `crates/ragent-codeindex/SECURITY_FINDINGS.md`
  - `crates/ragent-codeindex/COMPLIANCE.md`
  - `crates/ragent-server/COMPLIANCE.md`
- **Live audit files:**
  - `crates/ragent-storage/src/storage.rs` (credential storage)
  - `crates/ragent-tools-core/src/bash.rs` (shell execution)
  - `crates/ragent-tools-core/src/path_util.rs` (path resolution)
  - `crates/ragent-server/src/routes/mod.rs` (auth middleware)
  - `crates/ragent-server/src/sse.rs` (SSE serialization)
  - `crates/ragent-agent/src/sanitize.rs` / `crates/ragent-types/src/sanitize.rs` (secret redaction)
  - `crates/ragent-tools-extended/src/masterfetch/security.rs` (SSRF validation)
- **Dependency posture:** `deny.toml`, `.github/workflows/security-audit.yml`, `Cargo.lock`.
- **Risk severity convention:**
  - **P0 (Critical):** Data loss, remote code execution, credential compromise, or auth bypass.
  - **P1 (High):** Significant blast-radius reduction needed; exploit requires local access or user action but is reliable.
  - **P2 (Medium):** Defense-in-depth gaps, information disclosure in logs, or denial-of-service.
  - **P3 (Low):** Polish, monitoring, and hardening that can be deferred.

---

## 3. Risk Register

| ID  | Finding | Severity | Area | Owner | Effort | Milestone |
|-----|---------|----------|------|-------|--------|-----------|
| SEC-01 | Provider credentials stored in SQLite with reversible v2 keystream (blake3-derived) and legacy v1 XOR fallback | P0 | Storage / Credentials | ragent-storage | 3–5 d | M1 |
| SEC-02 | Single static bearer token stored as plaintext `String` in `AppState`; no revocation or scopes | P1 | Server / Auth | ragent-server | 2–3 d | M1 |
| SEC-03 | File tools do not canonicalize or enforce workspace containment; symlink/TOCTOU risk in commit path | P0 | Tool / Filesystem | ragent-tools-core | 2–4 d | M2 |
| SEC-04 | `bash` tool executes arbitrary shell strings via temp script + `bash -c`; banned lists are string-based | P1 | Tool / Shell | ragent-tools-core | 3–7 d | M3 |
| SEC-05 | SSE/Events rely on regex redaction only; secrets can leak through non-matching patterns or custom fields | P1 | Server / Events | ragent-server + ragent-agent | 2–4 d | M1 |
| SEC-06 | Auth middleware extracts token with `header[7..]` and does not trim trailing whitespace/normalize scheme | P2 | Server / Auth | ragent-server | 0.5 d | M1 |
| SEC-07 | Rate limiter uses `tokio::sync::Mutex<HashMap>` and in-memory state only | P2 | Server / Performance | ragent-server | 1–2 d | M4 |
| SEC-08 | `unwrap()`/poisoned-lock panics in non-test code (mutex/rwlock poison) | P2 | Runtime robustness | all crates | 2–3 d | M4 |
| SEC-09 | MCP discovery enumerates executables from `PATH` and npm global dirs; no signature or sandbox verification | P2 | Agent / MCP | ragent-agent | 1–2 d | M3 |
| SEC-10 | Dependency tree has 862+ crates, multiple duplicate major versions, and several ignored RUSTSEC advisories | P2 | Supply chain | CI / workspace | ongoing | M5 |
| SEC-11 | Clipboard temp files and TUI UI may expose secrets in log buffers / screen history | P2 | TUI / UX | ragent-tui | 1–2 d | M5 |
| SEC-12 | Error/debug logs print raw commands and full errors inconsistently | P2 | Logging | all crates | 1–2 d | M1 |

---

## 4. Detailed Findings

### SEC-01 — Credential storage is reversible (P0)

**Locations:**
- `crates/ragent-storage/src/storage.rs:102` — `encrypt_key` (blake3 keystream)
- `crates/ragent-storage/src/storage.rs:138` — `decrypt_key`
- `crates/ragent-storage/src/storage.rs:189-228` — legacy `obfuscate_key`/`deobfuscate_key` still decode v1 rows
- `crates/ragent-storage/src/storage.rs:1172-1258` — `set_provider_auth` / `get_provider_auth` / `seed_secret_registry`
- Used by: `crates/ragent-llm/src/providers/*`, `crates/ragent-tui/src/app/models.rs`, `crates/ragent-tools-extended/src/gmail.rs`, `crates/ragent-tools-vcs/src/gitlab/auth.rs`.

**Observation:**
The DB stores provider API keys, Copilot tokens, GitLab PATs, and Gmail OAuth bundles in a column called `api_key`. `encrypt_key` derives a symmetric stream from a constant salt + machine/user identifier via blake3 and XORs the plaintext. This is not authenticated encryption (no AEAD) and is reversible by anyone who can read the DB and derive the same key material. `get_provider_auth` also silently returns an empty string on decode failure, which can cause unauthenticated provider calls.

**Recommended fix:**
1. Replace the custom stream cipher with an AEAD (e.g., `aes-gcm` or `chacha20poly1305`) using a key from a `KeyProvider` trait.
2. Implement concrete providers:
   - `EnvKeyProvider` — derive from `RAGENT_MASTER_KEY` or similar.
   - `KeyringKeyProvider` — use the OS credential store (`keyring` crate) so the key is not stored next to the ciphertext.
3. Mark legacy v1 rows for migration on first successful unlock; fail closed if migration is not possible.
4. Change `get_provider_auth` to return `Result<Option<SecretString>>` using the `secrecy` crate so plaintext keys are zeroed and cannot be logged accidentally.
5. Add a CLI command or `/config` flow to rotate/re-encrypt stored credentials.

---

### SEC-02 — Single static bearer token in `AppState` (P1)

**Locations:**
- `crates/ragent-server/src/routes/mod.rs:54-55` — `pub auth_token: String`
- `crates/ragent-server/src/routes/mod.rs:136-177` — `auth_middleware`
- `crates/ragent-server/src/routes/mod.rs:142-151` — hand-rolled constant-time comparison

**Observation:**
The server configuration holds one static bearer token in memory as a `String`. All API clients share it, there is no revocation, no expiration, no per-token scope, and the token is embedded in server state. While comparison is constant-time, a leaked token grants full API access until the server is restarted.

**Recommended fix:**
1. Add a `tokens` table to Storage with `id`, `hash` (Argon2id or at least SHA-256 + salt), `name`, `scopes`, `created_at`, `revoked_at`.
2. Move token validation into Storage-backed middleware:
   - Extract the bearer token.
   - Hash it and look up the hash in the DB.
   - Reject revoked tokens.
3. Keep a small in-memory positive cache with TTL to avoid DB lookup on every SSE event.
4. Retire the plaintext `auth_token` field from `AppState`.

---

### SEC-03 — File tool path containment missing (P0)

**Locations:**
- `crates/ragent-tools-core/src/path_util.rs` — current resolver is a simple join; no `std::fs::canonicalize`, no workspace-root check.
- File-affected tools: `read.rs`, `write.rs`, `create.rs`, `edit.rs`, `multiedit.rs`, `patch.rs`, `apply_patch.rs`, `rm.rs`, `copy.rs`, `move_file.rs`, `append_file.rs`, `office_*.rs`, `libreoffice_*.rs`, `pdf_*.rs`.
- `commit_all` / snapshot path: reported in `crates/ragent-codeindex/COMPLIANCE.md` section 8 as TOCTOU/symlink risk.

**Observation:**
Tools receive a relative or absolute path and resolve it against the working directory. There is no canonicalization step, no containment check, and no `O_NOFOLLOW`/`openat` hardening. A malicious prompt or compromised sub-agent can read or overwrite files outside the session workspace, including `/etc/passwd`, `~/.ssh/id_rsa`, or the ragent SQLite DB.

**Recommended fix:**
1. Introduce `resolve_and_validate(root: &Path, user_path: &str) -> Result<PathBuf>` that:
   - Strips traversal segments (`..`) before touching the filesystem.
   - Canonicalizes the resolved path.
   - Verifies the canonical path is within `root`.
   - Rejects absolute paths that are outside `root`.
2. Update every file tool to call `resolve_and_validate`.
3. For writes, use atomic temp-file + `fs::rename` **and** re-canonicalize the target immediately before rename; refuse if the canonical target leaves `root`.
4. On Unix, open parent directory FD and use `openat` with `O_NOFOLLOW` / `O_EXCL` where available; at minimum document the symlink race and warn users not to run ragent on untrusted repositories.
5. Add integration tests in `crates/ragent-tools-core/tests/test_path_safety.rs` that attempt escape via `../`, symlinks, and absolute paths.

---

### SEC-04 — `bash` tool arbitrary shell execution (P1)

**Locations:**
- `crates/ragent-tools-core/src/bash.rs` — writes user command to `/tmp/ragent-*.sh` and runs `bash -c`.
- `crates/ragent-tools-core/src/askpass.rs` — sudo/askpass helper.

**Observation:**
The tool has a 7-layer defense model (safe-command whitelist, banned commands, denied patterns, directory escape prevention, syntax validation, obfuscation detection, user allowlist/denylist), but the command is still passed as a free-form shell string. This is inherently fragile: quoting, command substitution, environment injection, and allowlist bypasses remain possible. The temp script is created in `/tmp` and is world-readable by default on many systems.

**Recommended fix:**
1. Prefer `argv` execution: add an optional `argv: Vec<String>` field to the `bash` tool schema and run `tokio::process::Command::new(program).args(argv)` without a shell when `argv` is supplied.
2. Keep the shell path available under a strict opt-in config flag (`shell: { enabled: true, sandbox: ... }`) and default it to `false` for untrusted environments.
3. When shell is required, run it in a sandbox option:
   - `bwrap` (bubblewrap) with no network, read-only root except the workspace.
   - Or `firejail` / Docker / podman container with minimal privileges.
   - Fallback: restrict `PATH`, clear env except allowlisted variables, set `HOME` to a temp dir, and drop capabilities.
4. Never write scripts to `/tmp`; if a script is needed, write to a workspace temp dir with `0600` permissions.
5. Add integration tests for command injection, backtick substitution, and `$(...)` bypasses.

---

### SEC-05 — SSE/event secret leakage through non-regex patterns (P1)

**Locations:**
- `crates/ragent-server/src/sse.rs` — serializes full event payloads; redacts only specific fields (`ModelResponse.text`, `ToolResult.content`).
- `crates/ragent-agent/src/sanitize.rs` / `crates/ragent-types/src/sanitize.rs` — regex + exact-match redaction.

**Observation:**
`event_to_sse` applies `redact_secrets` to a few free-text fields, but event variants carry many other fields (`error`, `description`, `context`, `question`, `notice`, etc.) that may contain provider keys or tokens. Redaction is regex-based and can miss custom token formats, base64 keys, or JWTs with unusual characters. The exact-match registry helps but depends on `register_secret` being called for every secret; registry population is not guaranteed for transient secrets (e.g., OAuth tokens from MCP or research adapters).

**Recommended fix:**
1. Define an `Event` serialization policy that excludes entire secret-bearing fields from SSE by default, or converts them to a `SafeDisplay` type.
2. At event construction time, wrap provider keys and session tokens in `secrecy::SecretString`; implement `Serialize` to emit `"[REDACTED]"`.
3. Add a pre-serialization pass over all string fields in `Event` using `redact_secrets` automatically (e.g., walk the `serde_json::Value` tree and mask leaves that match registered secrets or common patterns).
4. Add tests for redaction of non-standard tokens, base64 keys, and nested JSON content.

---

### SEC-06 — Auth middleware token extraction quirks (P2)

**Locations:**
- `crates/ragent-server/src/routes/mod.rs:159-161`

**Observation:**
The middleware checks `header.len() > 7` and slices `header[7..]` assuming `Bearer ` (7 chars). It does not trim trailing whitespace, so a token with trailing spaces fails comparison. The scheme check is case-insensitive, which is good, but no normalization is applied.

**Recommended fix:**
1. Use `.strip_prefix("Bearer ")` or equivalent after case-insensitive scheme match, then `.trim()`.
2. Reject empty tokens explicitly with `401`.
3. Add tests for trailing spaces/tabs, lowercase/uppercase scheme, missing scheme, and missing header.

---

### SEC-07 — In-memory rate limiter with coarse lock (P2)

**Locations:**
- `crates/ragent-server/src/routes/mod.rs` — `rate_limiter: tokio::sync::Mutex<HashMap<...>>`

**Observation:**
A single async mutex guards the per-session rate-limit map. Under concurrent load this serializes all protected requests. Entries are evicted only on access after 120 s.

**Recommended fix:**
1. Replace with `dashmap::DashMap` or a sharded `Mutex`/`RwLock` keyed by session.
2. Consider a token-bucket algorithm (`governor` crate or custom) with per-token quotas.
3. Persist rate-limit state across restarts only if necessary (likely not for v0.1); otherwise document the in-memory limitation.

---

### SEC-08 — `unwrap()` and poisoned-lock panics (P2)

**Locations:**
- Scattered across the workspace; grep for `.unwrap()` and `.expect()` in non-test `src/` files. Prior reports highlighted `std::sync::Mutex` poison panics.
- Storage uses `lock_conn!` macro for DB lock mapping; mirror this pattern for other mutexes.

**Observation:**
While many unwraps are in test code, non-test paths that call `.unwrap()` on `Mutex` guards or async locks can panic if another task panicked while holding the lock. This turns recoverable errors into denial-of-service.

**Recommended fix:**
1. Audit non-test `.unwrap()` and `.expect()` and convert to `Result` propagation with context.
2. For lock poisoning, use `lock().map_err(...)` or switch to `parking_lot`/`tokio::sync::Mutex` (no poisoning).
3. Add `cargo clippy` lints or a CI step that flags new `unwrap()`/`expect()` in `src/`.

---

### SEC-09 — MCP server discovery trusts executables (P2)

**Locations:**
- `crates/ragent-agent/src/mcp/discovery.rs`
- `crates/ragent-agent/src/mcp/mod.rs:106, 387` (logs MCP command)

**Observation:**
Auto-discovery scans `PATH`, npm global `node_modules`, and well-known registry directories and marks found executables as discovered but disabled. However, a malicious binary named `mcp-server-*` placed early in `PATH` could be launched later by user opt-in. Logs of the MCP command may also leak tokens if command-line args contain secrets.

**Recommended fix:**
1. Display discovered server full executable path and origin in the TUI so the user can verify.
2. Allow users to pin allowed MCP binaries by absolute path in `ragent.json`.
3. When launching, pass secrets via environment variables or stdin, never as command-line arguments.
4. Run MCP servers in the same sandbox policy as `bash` (SEC-04) when available.

---

### SEC-10 — Supply-chain / dependency risk (P2)

**Locations:**
- `Cargo.lock` — 862+ unique crates.
- `deny.toml` — 13 explicitly ignored RUSTSEC advisories.
- `.github/workflows/security-audit.yml` — weekly `cargo audit` + `cargo deny`.
- Duplicate major versions observed: `axum 0.7.9` vs `0.8.9`, `axum-core 0.4.5` vs `0.5.6`, `bitflags 1.x` vs `2.x`, etc.

**Observation:**
The workspace has a large dependency tree. Several advisories are ignored because upstream upgrades are blocked (e.g., `lopdf`, `quick-xml`, `ratatui` transitive deps). This is a reasonable short-term posture, but each ignored advisory needs an expiration/review date and a tracking issue.

**Recommended fix:**
1. Add `until` dates to each `deny.toml` ignore entry.
2. Create a `SEC-10-supply-chain.md` tracking doc in `docs/security/` mapping each ignored RUSTSEC to a root dependency, blocker, and target resolution version.
3. Reduce duplicate major versions where possible (especially `axum`) by aligning telemetry stack versions or feature-gating OTEL gRPC.
4. Run `cargo audit` locally before releases and fail pre-flight if new advisories appear.

---

### SEC-11 — TUI secret exposure in UI and clipboard (P2)

**Locations:**
- `crates/ragent-tui/security_findings.md` — prior findings on API key fields, clipboard temp files, run-cost banner.
- `crates/ragent-tui/src/input.rs:943, 1127` — provider key entry.

**Observation:**
API-key entry fields are unmasked in dialogs for visibility. Clipboard operations may create world-readable temp files on some platforms. Session history in the TUI may retain secret-bearing messages.

**Recommended fix:**
1. Add a per-field mask toggle (default masked) for API keys.
2. Ensure clipboard provider writes temp files with `0600` and deletes them immediately.
3. Apply `redact_secrets` to any text rendered in the TUI log panel that originates from tool results or model responses.

---

### SEC-12 — Inconsistent log redaction (P2)

**Locations:**
- `crates/ragent-tools-core/src/bash.rs:1105` — uses `redact_secrets` on the command.
- `crates/ragent-server/src/routes/mod.rs:391` — uses `redact_secrets` on error.
- Many other `tracing::info!` / `tracing::debug!` calls print raw strings without redaction.

**Observation:**
Redaction is opt-in per call site. It is easy for contributors to add a new log line that leaks a secret.

**Recommended fix:**
1. Provide `tracing` field wrappers or a `redact(s: &str)` helper in `ragent-types` that is the default for free-form fields.
2. Add a lint/test that scans `src/` for `api_key`, `token`, `secret`, `command` string interpolations in logging macros and flags them for review.

---

## 5. Remediation Roadmap

### Milestone M1 — Secrets & Authentication Hardening (P0–P1)
**Goal:** Provider keys and server tokens are no longer stored or compared in plaintext, and logs/SSE are reliably redacted.

| Task | Owner | Effort | Acceptance Criteria |
|------|-------|--------|---------------------|
| M1.1 Replace storage v1/v2 obfuscation with AEAD + `KeyProvider` | ragent-storage | 3–5 d | `encrypt_key`/`decrypt_key` removed or backed by AEAD; `SecretString` returned from `get_provider_auth`; migration command exists; existing v1/v1 rows upgraded automatically. |
| M1.2 Replace `AppState.auth_token` with hashed token store | ragent-server | 2–3 d | Tokens table with hash + salt + revocation; middleware validates against Storage; `auth_token: String` removed from `AppState`. |
| M1.3 Structural SSE redaction + auto-walk all event string fields | ragent-server, ragent-agent | 2–3 d | Every `Event` variant serialized for SSE has all string leaves redacted by `redact_secrets`; tests for non-standard tokens pass. |
| M1.4 Audit and normalize log redaction | all crates | 1–2 d | CI/static check flags new un-redacted logging of secret/command fields. |
| M1.5 Trim/normalize bearer token extraction | ragent-server | 0.5 d | Tests for trailing whitespace, case-insensitive scheme, and empty token pass. |

### Milestone M2 — File Path Containment (P0)
**Goal:** File tools cannot operate outside the session workspace.

| Task | Owner | Effort | Acceptance Criteria |
|------|-------|--------|---------------------|
| M2.1 Implement `resolve_and_validate(root, path)` | ragent-tools-core | 0.5–1 d | Canonicalizes, rejects traversal, rejects absolute escapes, unit tests for `../`, symlinks, absolute paths. |
| M2.2 Update all file tools to use the new resolver | ragent-tools-core, ragent-tools-extended | 1–2 d | `read`, `write`, `create`, `edit`, `multiedit`, `patch`, `apply_patch`, `rm`, `copy`, `move`, `append`, office/pdf tools all call `resolve_and_validate`. |
| M2.3 TOCTOU-safe commit writes | ragent-storage / snapshot layer | 1–2 d | Temp file + rename with re-canonicalization; unit tests for symlink overwrite attempts. |
| M2.4 Integration test: workspace escape attempts | tests/ | 0.5 d | Attempts to read/write outside temp workspace are rejected. |

### Milestone M3 — Shell Execution & MCP Sandboxing (P1)
**Goal:** Arbitrary shell execution is reduced and sandboxed; MCP servers are launched safely.

| Task | Owner | Effort | Acceptance Criteria |
|------|-------|--------|---------------------|
| M3.1 Add `argv` mode to `bash` tool and default to no-shell | ragent-tools-core | 1–2 d | New `argv` parameter works; simple commands run without shell; tests verify no shell interpretation. |
| M3.2 Shell opt-in + sandbox policy (bwrap/firejail/container) | ragent-tools-core | 2–4 d | `shell.enabled` and `shell.sandbox` config keys exist; when sandbox available, network is blocked, root is read-only except workspace. |
| M3.3 Harden temp script creation | ragent-tools-core | 0.5 d | Scripts written to workspace temp with mode `0600`; cleaned up on exit. |
| M3.4 MCP launch hardening | ragent-agent | 1–2 d | Secrets passed via env/stdin; allowed MCP paths configurable; logs redact command args. |
| M3.5 Integration tests for command injection | tests/ | 0.5–1 d | `$(...)`, backticks, piping, and shell-meta bypasses are rejected or run in no-shell mode. |

### Milestone M4 — Concurrency & Robustness (P2)
**Goal:** Remove coarse locks and poison-panic paths.

| Task | Owner | Effort | Acceptance Criteria |
|------|-------|--------|---------------------|
| M4.1 Replace rate-limit mutex with `DashMap`/sharded map | ragent-server | 0.5–1 d | Benchmark or stress test shows no contention under concurrent SSE clients. |
| M4.2 Replace `RwLock<HashMap>` task manager if hotspot | ragent-team / ragent-agent | 1–2 d | `DashMap` or sharded storage; no lock poisoning. |
| M4.3 Audit unwrap/expect in non-test code | all crates | 2–3 d | Number of non-test `.unwrap()`/`.expect()` reduced by 80%; CI check fails on new ones. |
| M4.4 SQLite WAL + busy_timeout for storage | ragent-storage | 0.5–1 d | `PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;` on open; startup lock contention reduced (see memory insight on FTS warmup). |

### Milestone M5 — Supply Chain & Monitoring (P2–P3)
**Goal:** Dependency and operational risk is continuously visible.

| Task | Owner | Effort | Acceptance Criteria |
|------|-------|--------|---------------------|
| M5.1 Document ignored advisories with expiration dates | CI/docs | 0.5 d | `docs/security/supply-chain-advisories.md` exists with `until` dates and upstream tracking links. |
| M5.2 Reduce duplicate major versions where feasible | workspace | 2–3 d | `axum`/`axum-core` duplicates resolved or feature-gated. |
| M5.3 Add `cargo audit` to `pre-flight.sh` failure path | scripts | 0.5 d | `pre-flight.sh` exits non-zero if `cargo audit` finds new un-ignored advisories. |
| M5.4 TUI secret-mask toggle & clipboard temp hardening | ragent-tui | 1–2 d | API key fields default masked; clipboard temp files use restrictive permissions. |
| M5.5 Security dashboard slash command | ragent-tui / server | 1–2 d | `/security status` or `/audit` shows pending advisories, sandbox status, and token store summary. |

---

## 6. Verification Checklist

Before marking SECPLAN complete:

- [ ] M1.1: `provider_auth` table no longer contains v1 XOR or v2 blake3-XOR ciphertext; AEAD ciphertext present.
- [ ] M1.1: `Storage::get_provider_auth` returns `secrecy::SecretString` and plaintext is not logged anywhere.
- [ ] M1.2: `AppState` has no `auth_token: String`; middleware validates hashed tokens from Storage.
- [ ] M1.3: SSE test `test_tool_result_redacts_bearer_token` passes and new tests for non-standard tokens pass.
- [ ] M2.1: `test_resolve_and_validate_prevents_escape` passes for `../`, absolute paths, and symlink escapes.
- [ ] M2.2: All file tools use `resolve_and_validate`; grep shows zero remaining direct `Path::join` on user input.
- [ ] M3.1: `bash` tool supports `argv` and defaults to no-shell when configured.
- [ ] M3.2: Sandbox policy is applied when `shell.sandbox` is enabled; integration test confirms no network.
- [ ] M4.3: CI step fails on new `.unwrap()`/`.expect()` in non-test `src/`.
- [ ] M5.3: `pre-flight.sh` fails on unignored `cargo audit` advisories.
- [ ] All existing tests pass (`cargo test`) and `cargo fmt --check` is clean.

---

## 7. Ongoing Security Practices

1. **Pre-release:** Run `pre-flight.sh`, `cargo audit`, `cargo deny check`, and the full test suite.
2. **Reviews:** Require security review for any change touching `bash`, file tools, auth, storage encryption, or server routes.
3. **Fuzzing:** Add property tests for path resolution, edit ranges, and secret redaction.
4. **Dependency cadence:** Revisit `deny.toml` ignored advisories monthly; set calendar reminders tied to the `until` dates.
5. **Threat model:** Update this plan when adding new tool categories (MCP, new VCS providers, browser automation, etc.).

---

## 8. Appendix: Quick Code Pointers

| Concern | File(s) |
|---------|---------|
| Storage encryption | `crates/ragent-storage/src/storage.rs` lines ~93–230, ~1170–1260 |
| Server auth middleware | `crates/ragent-server/src/routes/mod.rs` lines ~54–55, ~136–177 |
| SSE redaction | `crates/ragent-server/src/sse.rs` lines ~549, ~584 |
| Secret redaction | `crates/ragent-agent/src/sanitize.rs`, `crates/ragent-types/src/sanitize.rs` |
| Shell execution | `crates/ragent-tools-core/src/bash.rs` |
| Path resolution | `crates/ragent-tools-core/src/path_util.rs` |
| SSRF validation | `crates/ragent-tools-extended/src/masterfetch/security.rs` |
| MCP discovery | `crates/ragent-agent/src/mcp/discovery.rs` |
| Supply chain config | `deny.toml`, `.github/workflows/security-audit.yml` |
| Prior reports | `crates/ragent-tui/security_findings.md`, `crates/ragent-codeindex/SECURITY_FINDINGS.md`, `crates/ragent-codeindex/COMPLIANCE.md`, `crates/ragent-server/COMPLIANCE.md` |

---

## 9. References

- OWASP Top 10: https://owasp.org/www-project-top-ten/
- `secrecy` crate: https://crates.io/crates/secrecy
- `aes-gcm`: https://docs.rs/aes-gcm
- `keyring`: https://crates.io/crates/keyring
- `governor` rate limiter: https://crates.io/crates/governor
- `cargo-deny`: https://embarkstudios.github.io/cargo-deny/
- Bubblewrap sandbox: https://github.com/containers/bubblewrap

---

*End of SECPLAN.md.*
