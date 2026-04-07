# ragent-server Security Audit Report

**Date:** $(date +%Y-%m-%d)  
**Crate:** ragent-server  
**Scope:** Full security review of HTTP/SSE server implementation  
**Severity Levels:** Critical (P0), High (P1), Medium (P2), Low (P3)

---

## Executive Summary

This security audit identifies **12 security issues** in the ragent-server crate, ranging from critical CORS misconfiguration to medium-severity authentication and data leakage vulnerabilities. The server handles sensitive LLM API keys, user session data, and file system access, making security a high priority.

**Critical Issues (P0): 2**  
**High Issues (P1): 3**  
**Medium Issues (P2): 4**  
**Low Issues (P3): 3**

---

## Detailed Findings

### 1. CRITICAL: CORS Misconfiguration (CORS Allow-All)

**Severity:** P0 - Critical  
**Location:** `src/routes/mod.rs:96-130`  
**Issue:** The router applies `CorsLayer::permissive()` which allows **all origins**, **all methods**, and **all headers**.

```rust
pub fn router(state: AppState) -> Router {
    let protected = Router::new()
        // ... routes
        .layer(CorsLayer::permissive()) // DANGEROUS
}
```

**Impact:**
- Cross-origin requests from malicious websites can access the API
- Authentication tokens can be stolen via XSS if a victim visits a malicious site
- CSRF attacks are possible against state-changing endpoints
- Any website can make requests to the ragent server

**Remediation:**
```rust
// Restrict to specific origins
let cors = CorsLayer::new()
    .allow_origin([
        "http://localhost:3000".parse::<HeaderValue>().unwrap(),
        // Add production origins
    ])
    .allow_methods([Method::GET, Method::POST])
    .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE])
    .allow_credentials(true);
```

---

### 2. CRITICAL: Global Event Stream Leaks All Session Data

**Severity:** P0 - Critical  
**Location:** `src/routes/mod.rs:454-466`  
**Issue:** The `/events` endpoint broadcasts ALL events from ALL sessions to any authenticated client.

```rust
async fn events_stream(State(state): State<AppState>) -> Sse<impl Stream<...>> {
    let rx = state.event_bus.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|result| async move {
        match result {
            Ok(event) => Some(Ok(event_to_sse(&event))), // No session filtering!
            Err(_) => None,
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}
```

**Impact:**
- Any authenticated client can see events from ALL sessions
- Tool outputs, file contents, and LLM responses from other users are visible
- Multi-user deployments have complete data leakage between sessions

**Remediation:**
Remove the global `/events` endpoint or add mandatory session filtering:
```rust
async fn events_stream(
    State(state): State<AppState>,
    Query(params): Query<EventsQuery>, // Require session_id
) -> impl IntoResponse {
    let session_id = params.session_id?;
    // Filter events by session_id
}
```

---

### 3. HIGH: Sensitive Data in SSE Events Not Redacted

**Severity:** P1 - High  
**Location:** `src/sse.rs:135-144` (ToolResult), `src/sse.rs:118-125` (ModelResponse)  
**Issue:** Event payloads contain raw tool outputs and model responses without redaction.

```rust
#[derive(Serialize)]
struct ToolResultP<'a> {
    session_id: &'a str,
    call_id: &'a str,
    tool: &'a str,
    content: std::borrow::Cow<'a, str>, // Raw content, may contain secrets
    // ...
}
```

**Impact:**
- API keys, passwords, and tokens from tool outputs are broadcast to SSE clients
- File contents read by tools are exposed in event stream
- Database connection strings may leak

**Remediation:**
Apply `ragent_core::sanitize::redact_secrets` to all content fields:
```rust
content: std::borrow::Cow::Owned(redact_secrets(content)),
```

---

### 4. HIGH: No HTTPS Enforcement

**Severity:** P1 - High  
**Location:** `src/routes/mod.rs:76-83`  
**Issue:** The server has no mechanism to enforce HTTPS connections.

**Impact:**
- Authentication tokens are sent in plaintext over HTTP
- Man-in-the-middle attacks can intercept and modify requests
- Session hijacking via token theft on insecure networks

**Remediation:**
1. Add TLS configuration support
2. Add HSTS headers when TLS is enabled
3. Document HTTPS requirement for production

---

### 5. HIGH: Weak Token Generation and Storage

**Severity:** P1 - High  
**Location:** External (main.rs initialization)  
**Issue:** Token generation relies on external code; no minimum entropy requirements documented.

**Impact:**
- Short or predictable tokens can be brute-forced
- Tokens may not be securely generated

**Remediation:**
1. Enforce minimum token length (32+ bytes)
2. Use cryptographically secure random generation
3. Support token rotation
4. Add token expiration

---

### 6. MEDIUM: Rate Limiter Memory Exhaustion (DoS)

**Severity:** P2 - Medium  
**Location:** `src/routes/mod.rs:323-346`  
**Issue:** Rate limiter HashMap grows unbounded with unique session IDs.

```rust
let entry = limiter.entry(id.clone()).or_insert((0, now));
// No eviction of old entries
```

**Impact:**
- Memory exhaustion with many unique session IDs
- Denial of Service via memory exhaustion

**Remediation:**
Add eviction for stale entries:
```rust
// Evict entries older than 120 seconds
const EVICTION_WINDOW_SECS: u64 = 120;
const MAX_ENTRIES: usize = 10_000;
if limiter.len() > MAX_ENTRIES {
    limiter.retain(|_, (_, ts)| now.duration_since(*ts).as_secs() < EVICTION_WINDOW_SECS);
}
```

---

### 7. MEDIUM: No Input Validation on Permission Reply

**Severity:** P2 - Medium  
**Location:** `src/routes/mod.rs:440-452`  
**Issue:** Permission replies are accepted without validating the request_id exists.

```rust
async fn reply_permission(...) -> impl IntoResponse {
    let allowed = body.decision != PermissionReplyDecision::Deny;
    state.event_bus.publish(Event::PermissionReplied { ... }); // No validation!
    Json(serde_json::json!({ "ok": true }))
}
```

**Impact:**
- Spurious permission replies for non-existent requests
- Race conditions between request and reply
- Potential for replay attacks

**Remediation:**
Validate request exists before accepting reply:
```rust
if !state.permission_checker.has_request(&req_id).await {
    return error_response(StatusCode::NOT_FOUND, "permission request not found");
}
```

---

### 8. MEDIUM: Path Traversal Risk in Session Creation

**Severity:** P2 - Medium  
**Location:** `src/routes/mod.rs:229-275`  
**Issue:** Directory path is canonicalized but not validated against allowed directories.

**Impact:**
- Sessions could be created pointing to system directories
- File operations may access sensitive system paths
- Path traversal via symlinks after canonicalization

**Remediation:**
Add allowlist validation:
```rust
let allowed_prefixes = [std::env::current_dir()?, PathBuf::from("/tmp")];
if !allowed_prefixes.iter().any(|p| canonical.starts_with(p)) {
    return error_response(StatusCode::FORBIDDEN, "directory not in allowed list");
}
```

---

### 9. MEDIUM: API Key Retrieval Without Audit Logging

**Severity:** P2 - Medium  
**Location:** `src/routes/mod.rs:753-757`  
**Issue:** API keys are retrieved from storage without audit logging.

```rust
let api_key = self
    .storage
    .get_provider_auth(&self.provider_id)
    .context("reading API key")?
    .unwrap_or_default();
```

**Impact:**
- No audit trail of API key access
- Cannot detect unauthorized key usage

**Remediation:**
Add audit logging for sensitive operations:
```rust
tracing::info!(provider = %self.provider_id, "API key accessed for prompt optimization");
```

---

### 10. LOW: Information Disclosure in Error Messages

**Severity:** P3 - Low  
**Location:** Throughout routes  
**Issue:** Internal error details are exposed to clients.

```rust
Err(e) => error_response(
    StatusCode::INTERNAL_SERVER_ERROR,
    e.to_string(), // Exposes internal details
),
```

**Impact:**
- Attackers can gather information about internal structure
- File paths, database details may leak

**Remediation:**
Return generic error messages to clients; log details internally:
```rust
tracing::error!(error = %e, "Database error");
error_response(StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
```

---

### 11. LOW: Missing Content-Type Validation

**Severity:** P3 - Low  
**Location:** All POST endpoints  
**Issue:** No validation that Content-Type is application/json.

**Impact:**
- CSRF via form submissions (if cookies were used)
- Content-type confusion attacks

**Remediation:**
Add Content-Type validation middleware or handler checks.

---

### 12. LOW: No Request Size Limits

**Severity:** P3 - Low  
**Location:** All POST endpoints  
**Issue:** No maximum request body size is enforced.

**Impact:**
- Large request bodies can cause memory exhaustion
- DoS via large JSON payloads

**Remediation:**
Add Axum's `DefaultBodyLimit` layer:
```rust
.layer(DefaultBodyLimit::max(1024 * 1024)) // 1MB limit
```

---

## Remediation Plan

### Milestone 1: Critical Security Fixes (Week 1)

**Goal:** Address all P0 issues immediately

| Task | ID | Description | File | Effort |
|------|-----|-------------|------|--------|
| M1-T1 | Fix CORS configuration | Restrict CORS to specific origins | `src/routes/mod.rs` | 2h |
| M1-T2 | Remove or fix global events endpoint | Add session filtering or remove endpoint | `src/routes/mod.rs` | 3h |
| M1-T3 | Write tests for CORS | Verify CORS restrictions work | `tests/test_security.rs` | 2h |
| M1-T4 | Write tests for event isolation | Verify session event isolation | `tests/test_security.rs` | 2h |

**Acceptance Criteria:**
- [ ] CORS only allows configured origins
- [ ] Global events endpoint removed or requires session_id
- [ ] All P0 tests passing

---

### Milestone 2: Data Protection (Week 2)

**Goal:** Prevent sensitive data leakage

| Task | ID | Description | File | Effort |
|------|-----|-------------|------|--------|
| M2-T1 | Redact secrets in SSE events | Apply redact_secrets to all content fields | `src/sse.rs` | 3h |
| M2-T2 | Add content trimming | Trim large tool outputs in events | `src/sse.rs` | 2h |
| M2-T3 | Write secret redaction tests | Verify secrets are redacted | `tests/test_security.rs` | 2h |
| M2-T4 | Add audit logging | Log API key access and sensitive operations | `src/routes/mod.rs` | 2h |

**Acceptance Criteria:**
- [ ] No secrets in SSE event payloads
- [ ] Tool outputs limited to preview (e.g., 1000 chars)
- [ ] All sensitive operations audited

---

### Milestone 3: Authentication & Authorization Hardening (Week 2-3)

**Goal:** Strengthen auth and session security

| Task | ID | Description | File | Effort |
|------|-----|-------------|------|--------|
| M3-T1 | Enforce token minimum length | Reject weak tokens at startup | `src/routes/mod.rs` | 1h |
| M3-T2 | Add permission request validation | Validate request_id before reply | `src/routes/mod.rs` | 2h |
| M3-T3 | Add directory allowlist | Validate session directories | `src/routes/mod.rs` | 2h |
| M3-T4 | Add session ownership checks | Verify user owns session | `src/routes/mod.rs` | 4h |

**Acceptance Criteria:**
- [ ] Tokens must be 32+ bytes
- [ ] Permission replies validated
- [ ] Sessions restricted to allowed directories

---

### Milestone 4: DoS Protection (Week 3)

**Goal:** Prevent resource exhaustion attacks

| Task | ID | Description | File | Effort |
|------|-----|-------------|------|--------|
| M4-T1 | Fix rate limiter memory growth | Add entry eviction | `src/routes/mod.rs` | 2h |
| M4-T2 | Add request size limits | Limit body size to 1MB | `src/routes/mod.rs` | 1h |
| M4-T3 | Add request timeout middleware | Timeout slow requests | `src/routes/mod.rs` | 2h |
| M4-T4 | Write DoS protection tests | Test rate limiting and limits | `tests/test_security.rs` | 2h |

**Acceptance Criteria:**
- [ ] Rate limiter bounded to 10k entries
- [ ] Request body limit enforced
- [ ] Timeout middleware active

---

### Milestone 5: TLS & Production Readiness (Week 4)

**Goal:** Enable secure deployment

| Task | ID | Description | File | Effort |
|------|-----|-------------|------|--------|
| M5-T1 | Add TLS configuration | Support HTTPS certificates | `src/routes/mod.rs` | 4h |
| M5-T2 | Add HSTS headers | Security headers for HTTPS | `src/routes/mod.rs` | 1h |
| M5-T3 | Sanitize error messages | Return generic errors to clients | `src/routes/mod.rs` | 2h |
| M5-T4 | Add security documentation | Document security requirements | `docs/security.md` | 2h |
| M5-T5 | Security audit tests | Complete security test suite | `tests/test_security.rs` | 4h |

**Acceptance Criteria:**
- [ ] TLS can be configured
- [ ] Error messages sanitized
- [ ] Security documentation complete
- [ ] All security tests passing

---

## Testing Requirements

### New Test File: `tests/test_security.rs`

```rust
//! Security-focused integration tests

// Test cases to implement:
// 1. CORS rejects cross-origin requests
// 2. Events endpoint requires session_id or is removed
// 3. Secrets are redacted in SSE events
// 4. Rate limiter evicts old entries
// 5. Permission replies validated
// 6. Directory allowlist enforced
// 7. Request size limits enforced
// 8. Error messages sanitized
// 9. Token minimum length enforced
// 10. Session isolation verified
```

### Existing Tests to Update

- `tests/test_integration.rs`: Add CORS and security test cases

---

## Dependency Security

Review these dependencies for known vulnerabilities:

| Crate | Usage | Risk |
|-------|-------|------|
| axum | Web framework | Low - actively maintained |
| tower-http | CORS, middleware | Low - check for CORS issues |
| tokio | Async runtime | Low - widely audited |
| serde_json | JSON parsing | Low - fuzzed extensively |

**Action:** Run `cargo audit` regularly in CI.

---

## Security Checklist

Before production deployment:

- [ ] CORS restricted to known origins
- [ ] Global events endpoint removed or fixed
- [ ] HTTPS enforced with valid certificates
- [ ] Secrets redacted in all outputs
- [ ] Rate limiting bounded and tested
- [ ] Input validation on all endpoints
- [ ] Error messages sanitized
- [ ] Request size limits configured
- [ ] Security tests passing
- [ ] Security documentation published
- [ ] `cargo audit` passes
- [ ] Security review completed

---

## References

- [OWASP Top 10 2021](https://owasp.org/Top10/)
- [OWASP API Security Top 10](https://owasp.org/www-project-api-security/)
- [CORS Security Guide](https://developer.mozilla.org/en-US/docs/Web/HTTP/CORS)
- [SSE Security Considerations](https://html.spec.whatwg.org/multipage/server-sent-events.html)

---

**Next Steps:**

1. Schedule Milestone 1 tasks immediately (P0 issues)
2. Assign owners to each milestone
3. Set up security-focused CI checks
4. Plan security review meeting after Milestone 2
