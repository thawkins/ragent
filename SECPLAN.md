# Security Remediation Plan (SECPLAN.md)

**Version:** 0.1.0-alpha.30  
**Date:** 2025-04-09  
**Status:** Consolidated Plan from Existing Security Audits  
**Scope:** ragent codebase security hardening

---

## Executive Summary

This document consolidates security findings from multiple security audits of the ragent codebase:

- `crates/ragent-code/SECURITY_FINDINGS.md` — Core security findings
- `crates/ragent-tui/security_findings.md` — TUI security findings
- `ragent-server-security.md` — Server security audit report
- `crates/ragent-core/COMPLIANCE.md` — Core compliance review
- `crates/ragent-server/COMPLIANCE.md` — Server compliance review

**Overall Risk Assessment:**
- Critical (P0): 2 issues — CORS misconfiguration, Global event stream leaks
- High (P1): 8 issues — Shell execution, path traversal, weak secret storage, SSE data leakage
- Medium (P2): 12 issues — Rate limiting, input validation, logging
- Low (P3): 8 issues — Error handling, content-type validation, timeouts

**Key Areas Requiring Attention:**
1. Command execution sandboxing (bash tool, dynamic context)
2. Path traversal protection in file operations
3. Secret storage and redaction improvements
4. Server CORS and event isolation
5. Rate limiting and DoS protection

---

## Milestone 1: Critical Security Fixes (P0)
**Timeline:** Week 1  
**Priority:** Immediate  
**Goal:** Address all Critical (P0) issues before any production deployment

### M1-T1: Fix CORS Configuration
- **Severity:** P0 - Critical
- **File:** `crates/ragent-server/src/routes/mod.rs:96-130`
- **Issue:** `CorsLayer::permissive()` allows all origins, methods, and headers
- **Impact:** Cross-origin attacks, token theft via XSS, CSRF vulnerabilities
- **Remediation:**
  ```rust
  let cors = CorsLayer::new()
      .allow_origin([
          "http://localhost:3000".parse::<HeaderValue>().unwrap(),
          // Add production origins via config
      ])
      .allow_methods([Method::GET, Method::POST])
      .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
      .allow_credentials(true);
  ```
- **Effort:** 2 hours
- **Owner:** Security team lead
- **Acceptance Criteria:**
  - [ ] CORS only allows configured origins
  - [ ] Rejects requests from unauthorized origins
  - [ ] Unit tests verify CORS restrictions

### M1-T2: Fix Global Event Stream Leakage
- **Severity:** P0 - Critical
- **File:** `crates/ragent-server/src/routes/mod.rs:454-466`
- **Issue:** `/events` endpoint broadcasts ALL events from ALL sessions
- **Impact:** Complete data leakage between sessions, multi-user deployments compromised
- **Remediation:**
  - Option A: Remove global `/events` endpoint entirely
  - Option B: Add mandatory session filtering with validation
  ```rust
  async fn events_stream(
      State(state): State<AppState>,
      Query(params): Query<EventsQuery>, // Require session_id
  ) -> impl IntoResponse {
      let session_id = params.session_id?;
      // Validate session ownership
      // Filter events by session_id
  }
  ```
- **Effort:** 3 hours
- **Owner:** Backend security engineer
- **Acceptance Criteria:**
  - [ ] Global events endpoint removed or requires session_id
  - [ ] Session ownership validated before streaming
  - [ ] Integration tests verify event isolation

### M1-T3: CORS Security Tests
- **File:** `crates/ragent-server/tests/test_security.rs`
- **Tasks:**
  - Test CORS preflight from unauthorized origins
  - Test authenticated requests from allowed origins
  - Test credentials header handling
- **Effort:** 2 hours
- **Owner:** QA engineer

### M1-T4: Event Isolation Tests
- **File:** `crates/ragent-server/tests/test_security.rs`
- **Tasks:**
  - Create two sessions, verify events don't leak between them
  - Test session_id filtering works correctly
  - Test unauthorized access to other session's events
- **Effort:** 2 hours
- **Owner:** QA engineer

---

## Milestone 2: High-Priority Security Hardening (P1)
**Timeline:** Weeks 2-3  
**Goal:** Address all High-severity issues

### M2-T1: Path Traversal Protection
- **Severity:** P1 - High
- **Files:**
  - `crates/ragent-core/src/tool/read.rs`
  - `crates/ragent-core/src/file_ops/mod.rs`
  - `crates/ragent-core/src/tool/str_replace_editor.rs`
- **Issue:** Tools accept paths without canonicalization or sandbox validation
- **Impact:** Data exfiltration, arbitrary file overwrites via symlinks
- **Remediation:**
  1. Implement `validate_path_in_sandbox()` helper:
     ```rust
     pub fn validate_path_in_sandbox(
         path: &Path,
         sandbox_root: &Path,
     ) -> Result<PathBuf, SecurityError> {
         let canonical = path.canonicalize()
             .map_err(|_| SecurityError::InvalidPath)?;
         if !canonical.starts_with(sandbox_root) {
             return Err(SecurityError::PathTraversal);
         }
         Ok(canonical)
     }
     ```
  2. Apply validation to all file operation tools (read, write, edit, commit)
  3. Check for symlink attacks (TOCTOU protection)
- **Effort:** 6 hours
- **Owner:** Security engineer
- **Acceptance Criteria:**
  - [ ] All file operations validate paths against session working directory
  - [ ] Absolute paths outside sandbox are rejected
  - [ ] Symlink attacks are prevented
  - [ ] Unit tests cover traversal attempts

### M2-T2: Secret Storage Hardening
- **Severity:** P1 - High
- **File:** `crates/ragent-core/src/storage/mod.rs`
- **Issue:** XOR obfuscation with fixed key — easily reversible
- **Impact:** Secrets exposed if DB is compromised
- **Remediation:**
  - ✅ **Status: PARTIALLY COMPLETED** in COMPLIANCE.md (Milestone 2)
  - Current implementation uses blake3-derived keystream encryption with `v2:` prefix
  - Legacy v1 (XOR) auto-detected and auto-migrated to v2 on read
  - **Remaining work:**
    - Consider migration to platform keyring for production
    - Document key management procedures
- **Effort:** 4 hours (remaining documentation + testing)
- **Owner:** Security architect
- **Acceptance Criteria:**
  - [ ] All secrets stored using v2 encryption
  - [ ] Legacy v1 format migrated automatically
  - [ ] Documentation complete for key management

### M2-T3: Secret Redaction in SSE Events
- **Severity:** P1 - High
- **File:** `crates/ragent-server/src/sse.rs:135-144`
- **Issue:** ToolResult.content and ModelResponse.text contain raw data without redaction
- **Impact:** API keys, passwords, file contents broadcast to SSE clients
- **Remediation:**
  - ✅ **Status: PARTIALLY COMPLETED** in COMPLIANCE.md (Milestone E.1)
  - Current: `redact_secrets()` applied in `event_to_parts()`
  - **Remaining work:**
    - Consider trimming large tool outputs (only preview in events)
    - Add tests verifying secrets are not in SSE payloads
- **Effort:** 3 hours
- **Owner:** Backend developer
- **Acceptance Criteria:**
  - [ ] All SSE payloads pass through redaction
  - [ ] Large tool outputs trimmed to preview
  - [ ] Unit tests verify redaction

### M2-T4: Remove/Fix bash -c Execution
- **Severity:** P1 - High
- **File:** `crates/ragent-core/src/tool/bash.rs`
- **Issue:** Arbitrary shell execution with blacklist-based filtering (bypassable)
- **Impact:** Remote code execution via command injection
- **Remediation:**
  - ✅ **Status: PARTIALLY COMPLETED** in COMPLIANCE.md (Milestone 3.1)
  - Current: Allowlist-based execution with ~80 known-safe executables
  - `validate_command()` extracts first token and checks against allowlist
  - **Remaining work:**
    - Review allowlist completeness
    - Add sandbox/container options for untrusted environments
    - Document security implications
- **Effort:** 4 hours (review + documentation)
- **Owner:** Security engineer
- **Acceptance Criteria:**
  - [ ] Allowlist reviewed and complete
  - [ ] Container sandbox option available
  - [ ] Security documentation published

### M2-T5: Dynamic Context Sandboxing
- **Severity:** P1 - High
- **File:** `crates/ragent-core/src/skill/context.rs`
- **Issue:** `!`command`` placeholders execute arbitrary commands via `sh -c`
- **Impact:** Command injection if skill bodies are attacker-controlled
- **Remediation:**
  - ✅ **Status: COMPLETED** in COMPLIANCE.md (Milestone 3.1)
  - Current: Same allowlist-based model as bash tool
  - **Remaining work:**
    - Verify opt-in flag (`allow-dynamic-context: true`) is enforced
    - Add skill signing/verification for production
- **Effort:** 3 hours
- **Owner:** Security engineer
- **Acceptance Criteria:**
  - [ ] Dynamic context requires explicit opt-in
  - [ ] Allowlist enforced for all commands
  - [ ] Documentation warns of risks

### M2-T6: MCP Server Validation
- **Severity:** P1 - High
- **File:** `crates/ragent-core/src/mcp/mod.rs`
- **Issue:** MCP server configs with arbitrary commands can launch malicious binaries
- **Remediation:**
  - ✅ **Status: COMPLETED** in COMPLIANCE.md (Milestone 3.2)
  - Current: `validate_mcp_config()` rejects shell metacharacters
  - Concurrent MCP connections bounded by `MCP_SPAWN_SEMAPHORE` (max 8)
  - **Remaining work:**
    - Document MCP security requirements
- **Effort:** 2 hours
- **Owner:** Security engineer
- **Acceptance Criteria:**
  - [ ] MCP config validation documented
  - [ ] Security guidelines for MCP server authors published

### M2-T7: Replace std::sync::Mutex in Async Contexts
- **Severity:** P1 - High
- **Files:**
  - `crates/ragent-core/src/provider/copilot.rs` (SESSION_TOKEN_CACHE)
  - `crates/ragent-server/src/routes/mod.rs` (rate_limiter)
- **Issue:** Blocking mutex in async code causes thread pool starvation, deadlocks
- **Remediation:**
  - ✅ **Status: COMPLETED** in COMPLIANCE.md
  - Current: Uses `tokio::sync::Mutex` where appropriate
  - `SESSION_TOKEN_CACHE` uses std::sync::Mutex with poison handling
  - **Remaining work:**
    - Review remaining std::sync::Mutex usage
    - Document rationale for short-lived locks
- **Effort:** 2 hours (review + documentation)
- **Owner:** Performance engineer
- **Acceptance Criteria:**
  - [ ] All async contexts use tokio::sync::Mutex or DashMap
  - [ ] Document where std::sync::Mutex is intentionally used

### M2-T8: TOCTOU Protection in File Writes
- **Severity:** P1 - High
- **File:** `crates/ragent-core/src/file_ops/mod.rs`
- **Issue:** Symlink attacks possible during commit_all
- **Impact:** Arbitrary file overwrite via race condition
- **Remediation:**
  1. Use `O_NOFOLLOW` when opening files (platform-specific)
  2. Validate canonical path before and after temp file creation
  3. Consider using `openat`-style APIs on Unix
  4. Use `tempfile` crate with `persist()` for atomic writes
- **Effort:** 6 hours
- **Owner:** Security engineer
- **Acceptance Criteria:**
  - [ ] Symlink attacks prevented
  - [ ] TOCTOU window minimized
  - [ ] Unit tests demonstrate protection

---

## Milestone 3: Authentication & Authorization (P2)
**Timeline:** Weeks 3-4  
**Goal:** Strengthen auth mechanisms and session security

### M3-T1: Rate Limiter Hardening
- **Severity:** P2 - Medium
- **File:** `crates/ragent-server/src/routes/mod.rs:323-346`
- **Issue:** HashMap grows unbounded; std::sync::Mutex in async context
- **Remediation:**
  - ✅ **Status: PARTIALLY COMPLETED**
  - Current: Eviction of entries older than 120s
  - **Remaining work:**
    - Add maximum entry limit (10,000)
    - Consider DashMap for better concurrency
    - Add token bucket algorithm option
- **Effort:** 4 hours
- **Owner:** Backend developer
- **Acceptance Criteria:**
  - [ ] Rate limiter bounded to 10k entries
  - [ ] Memory exhaustion prevented
  - [ ] Performance tests pass

### M3-T2: Permission Request Validation
- **Severity:** P2 - Medium
- **File:** `crates/ragent-server/src/routes/mod.rs:440-452`
- **Issue:** Permission replies accepted without validating request_id exists
- **Impact:** Spurious replies, replay attacks
- **Remediation:**
  ```rust
  if !state.permission_checker.has_request(&req_id).await {
      return error_response(StatusCode::NOT_FOUND, "permission request not found");
  }
  ```
- **Effort:** 3 hours
- **Owner:** Backend developer
- **Acceptance Criteria:**
  - [ ] Permission replies validated against pending requests
  - [ ] Expired/replayed requests rejected
  - [ ] Tests cover race conditions

### M3-T3: Session Directory Allowlist
- **Severity:** P2 - Medium
- **File:** `crates/ragent-server/src/routes/mod.rs:229-275`
- **Issue:** Sessions can point to system directories
- **Remediation:**
  ```rust
  let allowed_prefixes = [
      std::env::current_dir()?,
      PathBuf::from(std::env::var("HOME")?),
      PathBuf::from("/tmp"),
  ];
  if !allowed_prefixes.iter().any(|p| canonical.starts_with(p)) {
      return error_response(StatusCode::FORBIDDEN, "directory not in allowed list");
  }
  ```
- **Effort:** 3 hours
- **Owner:** Backend developer
- **Acceptance Criteria:**
  - [ ] Sessions restricted to allowed directories
  - [ ] System directories blocked
  - [ ] Configuration allows custom allowlist

### M3-T4: API Key Access Audit Logging
- **Severity:** P2 - Medium
- **File:** `crates/ragent-server/src/routes/mod.rs:753-757`
- **Issue:** No audit trail of API key access
- **Remediation:**
  ```rust
  tracing::info!(
      provider = %self.provider_id,
      session_id = %session_id,
      "API key accessed for prompt optimization"
  );
  ```
- **Effort:** 2 hours
- **Owner:** Backend developer
- **Acceptance Criteria:**
  - [ ] All API key access logged
  - [ ] Audit logs include session/provider context
  - [ ] Documentation explains audit log format

### M3-T5: Secret Masking in TUI Logs
- **Severity:** P2 - Medium
- **File:** `crates/ragent-tui/src/app.rs`
- **Issue:** API keys may be displayed in TUI log panel via tracing
- **Remediation:**
  1. Implement `mask_secret()` helper:
     ```rust
     pub fn mask_secret(s: &str) -> String {
         if s.len() <= 8 {
             "***".to_string()
         } else {
             format!("{}***{}", &s[..4], &s[s.len()-4..])
         }
     }
     ```
  2. Audit all `push_log` and `tracing::` calls
  3. Apply masking before logging env vars
- **Effort:** 4 hours
- **Owner:** TUI developer
- **Acceptance Criteria:**
  - [ ] mask_secret() implemented and used
  - [ ] All env var reads masked before logging
  - [ ] No raw secrets in TUI logs

### M3-T6: Validate Executable Paths
- **Severity:** P2 - Medium
- **File:** `crates/ragent-tui/src/app.rs`
- **Issue:** Discovered server paths not validated before persistence
- **Remediation:**
  ```rust
  fn validate_executable_path(p: &std::path::Path) -> bool {
      if let Some(s) = p.to_str() {
          if s.chars().any(|c| matches!(c, '\n' | '\r' | ';' | '|' | '&' | '$' | '>' | '<' | '`' | '\'' | '"' | '*')) {
              return false;
          }
      }
      #[cfg(unix)] {
          if let Ok(meta) = std::fs::metadata(p) {
              if !meta.is_file() { return false; }
              use std::os::unix::fs::PermissionsExt;
              if meta.permissions().mode() & 0o111 == 0 { return false; }
          }
      }
      true
  }
  ```
- **Effort:** 3 hours
- **Owner:** TUI developer
- **Acceptance Criteria:**
  - [ ] Executable paths validated before persistence
  - [ ] Shell metacharacters rejected
  - [ ] Tests verify validation

---

## Milestone 4: Data Protection & Privacy (P2)
**Timeline:** Weeks 4-5  
**Goal:** Prevent sensitive data leakage

### M4-T1: HTTPS Enforcement
- **Severity:** P2 - Medium
- **File:** `crates/ragent-server/src/routes/mod.rs:76-83`
- **Issue:** No TLS configuration or HTTPS enforcement
- **Impact:** Token theft via MITM attacks
- **Remediation:**
  1. Add TLS configuration support (certificate paths, key paths)
  2. Add HSTS headers when TLS enabled
  3. Document HTTPS requirement for production
- **Effort:** 6 hours
- **Owner:** Infrastructure engineer
- **Acceptance Criteria:**
  - [ ] TLS can be configured via ragent.json
  - [ ] HSTS headers added when TLS enabled
  - [ ] Documentation explains HTTPS setup

### M4-T2: Secret Redaction Improvements
- **Severity:** P2 - Medium
- **File:** `crates/ragent-core/src/sanitize.rs`
- **Issue:** Regex-based redaction may miss some token formats
- **Remediation:**
  - ✅ **Status: PARTIALLY COMPLETED**
  - Current: Covers JWTs, GitHub tokens, Slack tokens, AWS keys, sk_live_/sk_test_
  - **Remaining work:**
    - Consider value-based redaction using SecretRegistry
    - Add more token patterns as discovered
- **Effort:** 3 hours
- **Owner:** Security engineer
- **Acceptance Criteria:**
  - [ ] SecretRegistry used for exact-match redaction
  - [ ] Documentation lists covered patterns

### M4-T3: Error Message Sanitization
- **Severity:** P3 - Low
- **File:** Throughout routes
- **Issue:** Internal error details exposed to clients
- **Remediation:**
  ```rust
  // Instead of:
  Err(e) => error_response(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
  
  // Use:
  tracing::error!(error = %e, "Database error");
  error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
  ```
- **Effort:** 4 hours (audit all error responses)
- **Owner:** Backend developer
- **Acceptance Criteria:**
  - [ ] No internal details in client-facing errors
  - [ ] Full errors logged server-side
  - [ ] Tests verify generic error messages

### M4-T4: Content-Type Validation
- **Severity:** P3 - Low
- **File:** All POST endpoints
- **Issue:** No validation that Content-Type is application/json
- **Remediation:**
  - Add Content-Type validation middleware
  - Reject requests with incorrect Content-Type
- **Effort:** 2 hours
- **Owner:** Backend developer
- **Acceptance Criteria:**
  - [ ] All POST endpoints validate Content-Type
  - [ ] Reject non-JSON requests with 415

---

## Milestone 5: DoS Protection & Resource Limits (P2-P3)
**Timeline:** Weeks 5-6  
**Goal:** Prevent resource exhaustion attacks

### M5-T1: Request Size Limits
- **Severity:** P3 - Low
- **File:** All POST endpoints
- **Issue:** No maximum request body size enforced
- **Remediation:**
  ```rust
  .layer(DefaultBodyLimit::max(1024 * 1024)) // 1MB limit
  ```
- **Effort:** 2 hours
- **Owner:** Backend developer
- **Acceptance Criteria:**
  - [ ] Request body limit enforced (1MB default)
  - [ ] Configurable via ragent.json
  - [ ] Tests verify limit rejection

### M5-T2: Request Timeout Middleware
- **Severity:** P3 - Low
- **File:** Router configuration
- **Issue:** Slow requests can consume resources indefinitely
- **Remediation:**
  - Add timeout middleware (30s default)
  - Make configurable via ragent.json
- **Effort:** 2 hours
- **Owner:** Backend developer
- **Acceptance Criteria:**
  - [ ] Timeout middleware active
  - [ ] Slow requests return 504
  - [ ] Configurable timeout value

### M5-T3: Percent-Decode Hardening
- **Severity:** P2 - Medium
- **File:** `crates/ragent-tui/src/app/state.rs:112-121`
- **Issue:** Uses `unwrap_or` and may mishandle malformed sequences
- **Remediation:**
  ```rust
  pub fn percent_decode_path(s: &str) -> std::path::PathBuf {
      fn hex_val(b: u8) -> Option<u8> {
          match b {
              b'0'..=b'9' => Some(b - b'0'),
              b'a'..=b'f' => Some(b - b'a' + 10),
              b'A'..=b'F' => Some(b - b'A' + 10),
              _ => None,
          }
      }
      // ... safe implementation without unwrap
  }
  ```
- **Effort:** 2 hours
- **Owner:** TUI developer
- **Acceptance Criteria:**
  - [ ] No unwrap in percent_decode_path
  - [ ] Malformed sequences handled gracefully
  - [ ] Tests verify safe parsing

### M5-T4: Clipboard Temp File Permissions
- **Severity:** P3 - Low
- **File:** `crates/ragent-tui/src/app/state.rs:161-201`
- **Issue:** Temp file permissions rely on OS default umask
- **Remediation:**
  ```rust
  #[cfg(unix)] {
      use std::os::unix::fs::PermissionsExt;
      let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
  }
  ```
- **Effort:** 1 hour
- **Owner:** TUI developer
- **Acceptance Criteria:**
  - [ ] Clipboard temp files have 0o600 permissions
  - [ ] Tests verify permissions on Unix

---

## Milestone 6: Testing & CI Integration (Ongoing)
**Timeline:** Parallel with all milestones  
**Goal:** Ensure security features are tested and monitored

### M6-T1: Security Test Suite
- **File:** `crates/ragent-server/tests/test_security.rs`
- **Test Cases:**
  1. CORS rejects cross-origin requests
  2. Events endpoint requires session_id or is removed
  3. Secrets are redacted in SSE events
  4. Rate limiter evicts old entries
  5. Permission replies validated
  6. Directory allowlist enforced
  7. Request size limits enforced
  8. Error messages sanitized
  9. Token minimum length enforced
  10. Session isolation verified
  11. Path traversal blocked
  12. Symlink attacks prevented
- **Effort:** 16 hours
- **Owner:** QA engineer

### M6-T2: cargo-audit CI Integration
- **File:** `.github/workflows/security-audit.yml`
- **Tasks:**
  - Run `cargo audit` on PRs and main branch
  - Run `cargo deny` for license/advisory checks
  - Schedule weekly automated audits
- **Effort:** 2 hours
- **Owner:** DevOps engineer
- **Acceptance Criteria:**
  - [ ] CI job runs on every PR
  - [ ] Security advisories fail build
  - [ ] Weekly reports generated

### M6-T3: Secret Detection CI
- **File:** `.github/workflows/secret-detection.yml`
- **Tasks:**
  - Add CI lint that fails when `push_log` or `tracing::` contains unmasked secrets
  - Scan for hardcoded API keys
  - Block commits with secrets
- **Effort:** 4 hours
- **Owner:** DevOps engineer
- **Acceptance Criteria:**
  - [ ] CI detects unmasked secrets
  - [ ] Hardcoded keys blocked
  - [ ] Documentation explains rules

---

## Summary Table

| Milestone | Priority | Timeline | Effort | Status |
|-----------|----------|----------|--------|--------|
| M1: Critical Fixes | P0 | Week 1 | 9h | 🔴 Not Started |
| M2: High-Priority | P1 | Weeks 2-3 | 32h | 🟡 Partially Complete |
| M3: Auth & AuthZ | P2 | Weeks 3-4 | 19h | 🟡 Partially Complete |
| M4: Data Protection | P2-P3 | Weeks 4-5 | 15h | 🟡 Partially Complete |
| M5: DoS Protection | P2-P3 | Weeks 5-6 | 7h | 🔴 Not Started |
| M6: Testing & CI | Ongoing | Parallel | 22h | 🟡 Partially Complete |

**Total Estimated Effort:** ~104 hours (~13 person-days)

---

## Security Checklist

Before production deployment, ensure all items are complete:

### Critical (P0)
- [ ] CORS restricted to known origins
- [ ] Global events endpoint removed or fixed
- [ ] All P0 tests passing

### High (P1)
- [ ] Path traversal protection in place
- [ ] Secrets encrypted at rest (not just obfuscated)
- [ ] bash -c execution removed or sandboxed
- [ ] Dynamic context requires explicit opt-in
- [ ] MCP configs validated
- [ ] Async-safe mutex usage throughout
- [ ] TOCTOU protection in file writes

### Medium (P2)
- [ ] Rate limiting bounded
- [ ] Permission requests validated
- [ ] Session directories allowlisted
- [ ] API key access audited
- [ ] TUI secrets masked in logs
- [ ] Executable paths validated
- [ ] HTTPS enforced (production)
- [ ] Request size limits configured

### Low (P3)
- [ ] Error messages sanitized
- [ ] Content-Type validated
- [ ] Request timeouts configured
- [ ] Clipboard temp file permissions restricted

### Testing & CI
- [ ] Security tests passing
- [ ] `cargo audit` passes
- [ ] Secret detection CI active
- [ ] Security documentation published

---

## References

- [OWASP Top 10 2021](https://owasp.org/Top10/)
- [OWASP API Security Top 10](https://owasp.org/www-project-api-security/)
- [OWASP Command Injection Prevention](https://cheatsheetseries.owasp.org/cheatsheets/Command_Injection_Prevention_Cheat_Sheet.html)
- [CORS Security Guide](https://developer.mozilla.org/en-US/docs/Web/HTTP/CORS)
- [SSE Security Considerations](https://html.spec.whatwg.org/multipage/server-sent-events.html)

---

## Contact

- Security Lead: security-reviewer
- Backend Lead: backend-dev
- TUI Lead: tui-dev
- QA Lead: qa-engineer

---

**Document History:**
- v0.1.0 (2025-04-09): Initial consolidated plan from existing security audits

**Next Steps:**
1. Prioritize Milestone 1 (Critical) tasks
2. Assign owners and create tracking issues
3. Schedule weekly security standups during remediation
4. Review and update this plan weekly
