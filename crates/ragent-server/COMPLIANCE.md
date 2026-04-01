# ragent-server Compliance Report

Date: 2026-03-30

Prepared by: performance-reviewer (ragent-server-code-review team)

Overview
--------
This report audits crates/ragent-server for security issues, missing or weak tests, and performance problems with an emphasis on resource usage and hot paths. Findings include detailed file/line references, impact, remediation steps, suggested tests, profiling and benchmarking guidance, third-party dependency risks, and a prioritized remediation plan with milestones, roles, and acceptance criteria.

Executive summary
-----------------
- Security: No obvious catastrophic auth bypass was detected. The authentication middleware uses a constant-time comparator which is good. However there are issues worth addressing: case-sensitivity of the "Bearer" scheme parsing, potential event leakage of sensitive content via SSE, and general log/data redaction checks.
- Tests: The crate contains no unit or integration tests. Critical logic (auth middleware, rate limiting, SSE serialization, prompt optimization flow, task endpoints) lacks test coverage.
- Performance: Several blocking/synchronous operations and synchronization choices inside async handlers can cause thread-pool starvation or contention under load. The main issues are: use of std::fs::canonicalize (blocking IO) in async handler; use of std::sync::Mutex for rate limiting inside async code; unbounded growth of the in-memory rate-limiter map; frequent allocations in SSE serialization and event filtering hot path; and lack of benchmarks for high-frequency code paths.

Scope
-----
Files reviewed (high level):
- crates/ragent-server/src/routes/mod.rs
- crates/ragent-server/src/sse.rs
- crates/ragent-server/Cargo.toml

Key findings (detailed)
----------------------
1) Blocking filesystem calls inside async handlers
- Location: create_session() in src/routes/mod.rs
  - Lines: 235-258 (canonicalize and is_dir checks)
  - Code excerpt: "let canonical = match std::fs::canonicalize(path) { ... }"
- Severity: High (performance / availability)
- Impact: std::fs::canonicalize is a blocking syscall; calling it directly inside an async handler runs on a Tokio worker thread and may block the runtime's worker thread if many concurrent requests arrive. Under moderate-to-high load this can cause thread starvation and increased latency across unrelated requests.
- Recommended remediation:
  - Replace std::fs::canonicalize and other blocking filesystem operations with tokio::fs::canonicalize (async) or offload the blocking call to tokio::task::spawn_blocking. E.g.:
    - let canonical = tokio::fs::canonicalize(path).await?;
    - or: let canonical = tokio::task::spawn_blocking(move || std::fs::canonicalize(path)).await??;
  - Audit the rest of the codebase for other blocking std::fs or blocking DB/file operations used in async handlers and handle similarly.
- Suggested tests to add:
  - Unit tests for create_session that exercise validation paths (invalid path, non-directory, success) using tempdir. Use tokio::test.
  - Integration test that posts to /sessions concurrently to verify absence of thread-pool stalls (see benchmarking section).
- Estimated effort: small

2) Synchronous std::sync::Mutex used for rate limiter in async code
- Location: AppState.rate_limiter field and send_message() lock usage
  - Lines: AppState declaration lines 52-56 and send_message locking lines 322-334
  - Code excerpt: "pub rate_limiter: Arc<std::sync::Mutex<HashMap<String, (u32, Instant)>>>," and "let mut limiter = match state.rate_limiter.lock() { ... }"
- Severity: Medium (performance / correctness under load)
- Impact: std::sync::Mutex can block an executor thread when contended. In an async runtime, this can cause reduced concurrency and latency spikes. Additionally, the current rate limiter never evicts stale session entries, so memory growth is unbounded for many unique session IDs.
- Recommended remediation:
  - Replace std::sync::Mutex<HashMap<...>> with tokio::sync::Mutex or an async-aware concurrent map (dashmap) to avoid blocking. Prefer dashmap for lower contention and lock-free reads where possible.
  - Add eviction for stale entries: store last-seen Instant and periodically remove entries older than a threshold (e.g., 10 minutes) using a background task, or wrap entries in a small TTL LRU cache.
  - Consider using a rate-limiter crate (governor, moka with TTL, or similar) that supports async-friendly patterns and automatic cleanup.
- Suggested tests to add:
  - Unit tests that simulate concurrent sends to the same session id and to many different ids to verify correctness and performance.
  - Property tests to check counter reset behavior after one minute boundary.
- Estimated effort: small-medium

3) Event broadcasting and potential sensitive payload leakage
- Location: event_to_sse in src/sse.rs and events_stream / send_message SSE usage
  - Lines: sse.rs: full mapping (1-347), send_message: filtering stream lines 369-387, events_stream: lines 450-461
- Severity: Medium (security / privacy)
- Impact: Events serialized in event_to_sse often include full payloads (e.g., ToolResult.content, ToolResult.metadata, ModelResponse.text). If events contain secrets (API keys, tokens, file content), they may be leaked to any authenticated SSE client subscribed to /events or a per-session SSE stream. While authentication middleware protects endpoints, any compromised token or misconfigured client can receive secrets.
- Recommended remediation:
  - Ensure events are sanitized before publishing to EventBus. Either:
    - Sanitize event payloads at the producer (where events are constructed) using ragent_core::sanitize::redact_secrets, or
    - Apply sanitization inside event_to_sse to redact sensitive fields before serialization.
  - Add a policy: events that contain long or binary data (tool output, file contents) should be trimmed (e.g., include only a preview and an id to fetch full content via an authenticated REST endpoint). Avoid streaming raw file/text content via SSE.
- Suggested tests to add:
  - Unit tests that validate event_to_sse does not include secrets (create a fake Event::ToolResult with known secret inside content and ensure event_to_sse output is redacted or trimmed).
  - Integration test ensuring only authorized clients receive events.
- Estimated effort: small

4) Unbounded allocations and repeated JSON creation in hot path
- Location: sse.rs event_to_sse and send_message filtering stream
  - Lines: sse.rs: match arms 22-346 and final serialization line 346: SseEvent::default().event(event_type).data(data.to_string())
  - send_message: stream creation lines 369-383
- Severity: Medium (performance / CPU)
- Impact: When many events are produced per second, event_to_sse constructs serde_json::Value via json! macro for each event then calls to_string, allocating intermediate structures. Filtering via event_matches_session also does exhaustive pattern matching for each event. This per-event overhead increases CPU use and heap allocations.
- Recommended remediation:
  - Implement Serialize on the Event enum and use serde_json::to_string(&filtered_view) to avoid constructing intermediate Values with json! for each arm. Alternatively, construct a small lightweight struct for SSE with only required fields and serialize it.
  - Consider precomputing event payloads at the time events are published if events are immutable; i.e., publish a pre-serialized SSE payload to the EventBus to avoid serializing repeatedly per subscriber. This trades memory for CPU.
  - For event filtering, if Event contains a session_id field (Option<String>) as a direct accessor, use that instead of exhaustive matching; or add a helper method on Event to return optional session_id to avoid repeated pattern matching. Example: fn session_id(&self) -> Option<&str> { match self { Event::X{session_id,..} => Some(session_id), ... _ => None } }
- Suggested tests to add:
  - Micro-benchmarks (criteria) for event serialization: measure allocations and throughput before/after change.
  - Unit test to ensure event serialization of every variant produces expected event name and JSON schema.
- Estimated effort: medium

5) Rate limiter algorithm correctness and edge cases
- Location: send_message rate limiting logic lines 322-343
- Severity: Low-Medium (correctness / DoS mitigation)
- Impact: The limiter uses a 60-requests-per-minute window per session but the algorithm uses a rolling window implemented by comparing duration_since(entry.1).as_secs() >= 60 then resetting. This effectively implements a fixed window per session, which can be bursty at window boundaries. Also, the initial entry uses now as start; if many sessions are created, the HashMap grows unbounded.
- Recommended remediation:
  - Use a token-bucket or leaky-bucket algorithm for smoother rate limiting, or use a battle-tested crate like governor or tower::limit. Token bucket reduces burst at edges.
  - Implement eviction for stale session entries.
- Suggested tests:
  - Tests that exercise token-bucket properties across boundary conditions.
- Estimated effort: small

6) prompt_opt ServerCompleter: provider registry created per request
- Location: Completer::complete implementation lines 747-785
- Severity: Low-Medium (performance / external call overhead)
- Impact: Each call constructs a new ProviderRegistry and calls registry.get(...) and provider.create_client which may perform dynamic provider registration. If prompt_opt is called frequently, constructing registry each time may be wasteful.
- Recommended remediation:
  - Cache ProviderRegistry at AppState initialization and reuse it (either in AppState or a shared Arc<ProviderRegistry>) to avoid repeated construction.
  - Ensure provider clients are created per-request when they must be ephemeral; but if provider create_client is expensive, use connection pooling or reuse clients when safe.
- Suggested tests:
  - Unit tests for prompt_opt_handler to verify behavior when provider or model missing and when provider create_client fails (mock provider).
- Estimated effort: small

7) Missing tests
- Location: entire crate
- Severity: High (maintainability)
- Impact: No unit or integration tests exist for API handlers; regressions may go unnoticed and CI coverage is absent.
- Recommended remediation:
  - Add unit tests for these components:
    - event_to_sse: cover all Event variants mapping and JSON shape.
    - auth_middleware: accept valid token, reject missing/bad token, test case-insensitive scheme.
    - rate limiter logic: correct counting, reset after 60s, concurrency.
    - create_session: invalid path, non-directory, success (use tempdir and tokio::fs::canonicalize when refactored).
    - prompt_opt_handler: validation errors when missing provider/model/method; successful optimization flow using a mock Completer that returns deterministic result.
    - spawn_task and task endpoints: success and failure paths — these will likely be integration tests requiring mocking of Storage and TaskManager or using a lightweight in-memory Storage for tests.
  - Add integration tests that run the Router in-process (Router::with_state) and send HTTP requests using reqwest::Client or axum::test::TestClient (or hyper test) to exercise authentication and SSE endpoints.
- Suggested test framework: tokio::test + assert_json_diff or similar utilities. For benchmarks use criterion.
- Estimated effort: medium

8) Lack of benchmarks / profiling
- Severity: Medium
- Impact: No objective measurement of hot paths or improvements.
- Recommended remediation:
  - Add benchmarks (use criterion) for:
    - event_to_sse serialization for a representative Event::ToolResult, Event::TextDelta, Event::ModelResponse.
    - event filtering (BroadcastStream + filter_map) with varying numbers of subscribers and event rates.
    - rate-limiter under concurrent access patterns.
    - create_session path canonicalize vs tokio::fs::canonicalize vs spawn_blocking.
  - Add a small microbench crate under crates/ragent-server/benches or use dev-dependencies with criterion.
  - Use tokio-console during load testing to identify blocked tasks or parking threads.
- Suggested acceptance metric: reduce allocations/serialization time for event_to_sse by at least 30% or show improved end-to-end throughput under a defined load profile.

9) Dependency risks
- Location: crates/ragent-server/Cargo.toml
- Severity: Low-Medium (supply chain)
- Impact: Dependencies are referenced via workspace; must ensure the workspace lockfile and published versions are up-to-date. Use cargo-audit to detect known vulnerabilities.
- Recommended remediation:
  - Run cargo update periodically and cargo audit in CI. Pin major versions where required to avoid accidental breaking changes from workspace updates.
  - Ensure third-party crates used for HTTP/async (axum, tokio, tower-http, serde) are regularly updated and run tests after updates.

Prioritized remediation plan
---------------------------
Order: Fix correctness/security and blocking I/O first, then address performance improvements and tests.

Milestone A — Safety & Correctness (High priority)
- Goal: Remove blocking calls inside async handlers, fix blocking Mutex usage, and ensure no obvious secret leakage.
- Owner: backend-dev / security-reviewer
- Acceptance criteria: All async handlers no longer perform blocking std::fs calls (or have spawn_blocking), rate limiter uses non-blocking primitives, and unit tests validate no secret leakage in SSE serialization.
- Status: COMPLETE (A.1, A.2, A.3 all done).
- Lint guard: Added #![deny(clippy::await_holding_lock)] to ragent-server lib.rs to prevent future regressions.

Tasks:
A.1 ✅ DONE — Replace std::fs::canonicalize with tokio::fs::canonicalize or spawn_blocking in create_session (small)
 - Files: src/routes/mod.rs lines ~235-258
 - Resolution: Replaced std::fs::canonicalize with tokio::fs::canonicalize(path).await.
   Also replaced blocking Path::is_dir() with tokio::fs::metadata().await.

A.2 ✅ DONE — Replace std::sync::Mutex rate_limiter with dashmap or tokio::sync::Mutex, add eviction (medium)
 - Files: src/routes/mod.rs AppState definition and send_message locking
 - Resolution: Changed to Arc<tokio::sync::Mutex<HashMap<...>>> with async .lock().await.
   Added on-access eviction of entries older than 120s when map exceeds 10,000 entries.

A.3 ✅ DONE — Sanitize events before publishing/serializing (small)
 - Files: src/sse.rs, event producers in ragent-core where events are published (search for event_bus.publish)
 - Resolution: Applied redact_secrets() to ToolResult.content and ModelResponse.text
   in event_to_parts() via Cow<str> for zero-alloc passthrough on clean strings.
   See Milestone E.1 for full details and tests.
 - Owner: security-reviewer
 - Acceptance: 6 unit tests proving secrets are redacted and clean text passes through.

Milestone B — Performance Improvements (Medium priority)
- Goal: Reduce per-event serialization cost and CPU overhead in event filtering, add caching where safe.
- Status: COMPLETE (B.1 and B.2 both done).

Tasks:
B.1 ✅ DONE — Implement a lightweight SSE payload struct and Serialize Event -> SSEPayload conversion (medium)
 - Files: src/sse.rs, benches/bench_sse.rs, Cargo.toml
 - Resolution: Replaced all json! macro calls in event_to_sse with ~25 typed
   #[derive(Serialize)] payload structs that borrow from Event fields. Payloads are
   serialized directly via serde_json::to_string, avoiding intermediate
   serde_json::Value tree allocations. Added event_type_name() helper. Added
   criterion benchmark (bench_sse) covering 7 representative event variants plus
   a mixed-batch benchmark.

B.2 ✅ DONE — Add session_id accessor on Event to avoid repeated exhaustive pattern matching (small)
 - Files: src/routes/mod.rs
 - Resolution: Event::session_id() already existed in ragent-core. Replaced the
   80-line exhaustive event_matches_session function with a one-liner:
   event.session_id() == Some(session_id). Eliminates redundant pattern matching
   and auto-adapts to new Event variants.

Milestone C — Tests & CI (High priority)
- Goal: Add test coverage for all critical paths and add benchmarks to CI.
- Status: COMPLETE (C.1, C.2, and C.3 all done). 50 tests pass (37 unit + 10 integration + 3 doctests).

Tasks:
C.1 ✅ DONE — Add unit tests for event_to_sse (small)
 - Files: crates/ragent-server/tests/test_event_to_sse.rs, src/sse.rs
 - Resolution: Added public event_to_parts() function returning (event_name, json_string)
   for testability. Created 37 unit tests covering every Event variant: correct event
   names, JSON field presence, special cases (CopilotDeviceFlowComplete token redaction,
   LspStatus Debug format, optional fields null handling).

C.2 ✅ DONE — Add integration tests for auth_middleware, send_message rate limiting, and SSE stream (medium)
 - Files: crates/ragent-server/tests/test_integration.rs
 - Resolution: 10 integration tests using in-process router with tower::ServiceExt::oneshot.
   Auth tests: health bypass, missing auth, wrong token, non-Bearer scheme, valid token.
   Rate limiter tests: under limit, over limit (429), window reset after 60s.
   SSE tests: auth required, content-type text/event-stream.
   Test AppState factory uses Storage::open_in_memory() for isolation.

C.3 ✅ DONE — Add benchmarks (criterion) for event serialization and rate limiter (medium)
 - Files: crates/ragent-server/benches/bench_sse.rs, Cargo.toml
 - Resolution: Extended bench_sse.rs with event_to_parts benchmark group (7 variants)
   and rate_limiter benchmark group (single-session insert, multi-session lookup with
   1000 pre-populated sessions). Bench compiles under release profile.

Milestone D — Hardening & Dependency Checks (Low-Medium priority)
- Goal: CI-integrate cargo-audit and keep dependencies up to date.
- Status: COMPLETE (D.1 and D.2 both done).

Tasks:
D.1 ✅ DONE — Add cargo-audit in CI and run periodic audits (small)
 - Files: .github/workflows/security-audit.yml, deny.toml
 - Resolution: Added GitHub Actions workflow running cargo-audit and cargo-deny on
   push/PR to main plus weekly schedule (Monday 06:00 UTC). Created deny.toml with
   advisory deny policy, license allowlist (MIT, Apache-2.0, BSD, ISC, etc.),
   wildcard ban, and source restrictions (crates.io only).

D.2 ✅ DONE — Review third party crates for minimal required features and limit feature flags (small)
 - Files: Cargo.toml (workspace)
 - Resolution: Audited all ~50 workspace dependencies. Already well-optimized:
   reqwest uses default-features=false with rustls-tls (no native-tls/OpenSSL),
   image/zip/printpdf disable defaults to avoid unnecessary codecs/compression.
   Added section-based organization and inline documentation of feature rationale
   for every dependency group. tokio "full" retained intentionally (binary uses
   fs, net, process, signal, rt-multi-thread). No unnecessary features found.

Milestone E — Sanitization, Auth Hardening & Observability (Medium priority)
- Goal: Complete remaining A.3 event sanitization, fix auth scheme case-sensitivity, add tracing instrumentation to hot-path handlers, and extend test coverage for previously untested endpoints.
- Owner: security-reviewer / backend-dev
- Status: COMPLETE (E.1, E.2, E.3, E.4 all done). 62 tests pass (43 unit + 16 integration + 3 doctests).

Tasks:
E.1 ✅ DONE — Sanitize sensitive fields in SSE event payloads (small)
 - Files: src/sse.rs
 - Resolution: Applied ragent_core::sanitize::redact_secrets() to ToolResult.content
   and ModelResponse.text in event_to_parts() before serialization. Uses Cow<str>
   to avoid allocation when no secrets are present (zero-cost passthrough).
   CopilotDeviceFlowComplete token was already redacted to boolean.
   Added 6 unit tests proving sk-*, key-*, Bearer tokens are redacted in output
   and clean text passes through unmodified.
 - Acceptance: Unit tests assert secrets are replaced with [REDACTED] in JSON output.

E.2 ✅ DONE — Case-insensitive Bearer auth scheme parsing (small)
 - Files: src/routes/mod.rs (auth_middleware)
 - Resolution: Replaced `header.starts_with("Bearer ")` with
   `header[..7].eq_ignore_ascii_case("Bearer ")` per RFC 7230 (HTTP header values
   for scheme names are case-insensitive). Added 3 integration tests for lowercase
   "bearer", uppercase "BEARER", and mixed-case "BeArEr".
 - Acceptance: Integration tests proving all case variants are accepted.

E.3 ✅ DONE — Tracing instrumentation on hot-path handlers (small)
 - Files: src/routes/mod.rs
 - Resolution: Added #[tracing::instrument] to 5 key handlers: create_session,
   send_message, abort_session, events_stream, spawn_task. State and body params
   are skipped; session_id is captured as a span field where available.
   tracing crate was already a workspace dependency.
 - Acceptance: Spans visible in tracing-subscriber output when RUST_LOG is set.

E.4 ✅ DONE — Additional test coverage for create_session and auth (small)
 - Files: tests/test_integration.rs
 - Resolution: Added 3 create_session tests (invalid path → 400, not-a-directory
   /dev/null → 400, success /tmp → 201). Combined with E.1 sanitization tests
   (6 tests) and E.2 auth tests (3 tests), total new tests = 12.

Where to add benchmarks and profiling hooks
------------------------------------------
- Benchmarks location: crates/ragent-server/benches/
  - bench_event_serialization.rs — benchmark event_to_sse variants using criterion
  - bench_rate_limiter.rs — benchmark DashMap vs tokio::sync::Mutex implementations
  - bench_canonicalize.rs — benchmark tokio::fs::canonicalize vs spawn_blocking
- Runtime profiling recommendations:
  - Enable tokio-console in development and profiling CI to catch blocking tasks.
  - Add tracing spans in hot code paths (send_message processing, task spawn/complete, event publish) and add histograms for event serialization durations.

Third-party dependency risks and fixes
-------------------------------------
- Recommendation: add a CI job that runs cargo audit and cargo update --workspace (careful review before merge). Pin or approve updates to axum/tokio/tower-http and serde dependencies. Regularly run `cargo update -p tokio --precise <version>` in a controlled PR when necessary.

Suggested tests (summary)
-------------------------
- Unit tests:
  - event_to_sse: one test per Event variant asserting event name + JSON keys
  - auth_middleware: accept/reject token tests including case-insensitive Bearer scheme
  - rate limiter: reset after 60s, counts increment correctly
  - create_session: canonicalize success & failure
  - prompt_opt_handler: missing provider/model/method error cases and a mocked successful path
- Integration tests:
  - start_router_in_process: authenticated vs unauthenticated requests
  - SSE stream: subscribe to /events and assert events are received for session activities
  - spawn_task + task endpoints: end-to-end using an in-memory TaskManager if available
- Benchmarks:
  - event serialization
  - rate limiting under concurrency
  - canonicalize blocking vs async

Low-risk, measurable optimizations
---------------------------------
1. Replace std::sync::Mutex -> DashMap (low risk) — reduces lock contention and avoids blocking async threads.
2. Use tokio::fs::canonicalize or spawn_blocking for filesystem ops (low risk) — prevents blocking runtime threads.
3. Add session_id() accessor on Event (low risk) — reduces matching overhead and centralizes logic.
4. Build lightweight SSE payload structs and use serde_json::to_string on typed structs instead of json! macros (medium risk; pay attention to schema compatibility) — reduces allocations and temporary Value objects.

Appendix: concrete code pointers
-------------------------------
- create_session: crates/ragent-server/src/routes/mod.rs lines 235-259
- rate limiter: AppState struct (lines ~40-57) and send_message lock lines 322-343
- SSE mapping and serialization: crates/ragent-server/src/sse.rs entire file (lines 1-347)
- send_message event stream filter: crates/ragent-server/src/routes/mod.rs lines 369-387
- prompt_opt Completer: crates/ragent-server/src/routes/mod.rs lines 747-785

Next steps / suggested immediate changes (first 48 hours)
--------------------------------------------------------
- Replace blocking std::fs::canonicalize with tokio::fs::canonicalize in create_session. (A.1)
- Replace std::sync::Mutex rate limiter with dashmap and add a simple eviction policy. (A.2)
- Add unit tests for event_to_sse and auth_middleware, and a small integration test that exercises create_session. (C.1, C.2)

Contact
-------
If you want I can implement a PR for Milestone A.1 and A.2 (small changes with tests), and create the initial benchmark scaffolding for Milestone B.1. Reply with which milestones you'd like me to implement and I'll submit a plan and then the changes.


