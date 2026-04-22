# Security Remediation Checklist

Use this checklist to track remediation work for the security findings previously identified in this codebase.

## High priority

### 1. Prevent forged permission approvals through the HTTP API
- [ ] Add a server-side pending-permission registry keyed by `(session_id, request_id)`.
- [ ] Reject permission replies for unknown sessions.
- [ ] Reject permission replies for unknown request IDs.
- [ ] Reject permission replies for expired or already-consumed requests.
- [ ] Bind permission replies to the original requester/client context where possible.
- [ ] Verify the target session exists before publishing `Event::PermissionReplied`.
- [ ] Add tests:
  - [ ] `test_reply_permission_rejects_unknown_session`
  - [ ] `test_reply_permission_rejects_unknown_request_id`
  - [ ] `test_reply_permission_rejects_request_id_for_different_session`

### 2. Stop global SSE event leakage across authenticated clients
- [ ] Scope SSE subscriptions to an authorized session or client identity.
- [ ] Filter event payloads server-side before streaming.
- [ ] Split any global/admin event stream into a separate privileged endpoint.
- [ ] Review SSE payloads for sensitive content exposure (tool args/results, prompts, permission descriptions, team traffic).
- [ ] Add tests:
  - [ ] `test_events_stream_does_not_emit_other_sessions_when_scoped_subscription_requested`

## Medium priority

### 3. Remove permission bypass for mutating team/task tools
- [ ] Audit `check_permission_with_prompt()` for all hardwired auto-allow branches.
- [ ] Replace prefix/suffix auto-approval (`team_*`, `*_task`) with a narrow explicit allowlist.
- [ ] Ensure mutating or spawning team/task tools go through normal permission checks.
- [ ] Re-evaluate whether codeindex tool bypasses should remain hardwired or move to explicit read-only permission policy.
- [ ] Add tests:
  - [ ] `test_check_permission_with_prompt_auto_approves_only_hardwired_helper_tools`
  - [ ] `test_check_permission_with_prompt_does_not_auto_approve_similar_non_helper_tool_names`

### 4. Replace permissive CORS on authenticated API routes
- [ ] Remove `CorsLayer::permissive()` from protected API routes.
- [ ] Introduce an explicit origin allowlist.
- [ ] Restrict allowed methods and headers to the minimum required.
- [ ] Default browser access to disabled for local-only deployments.
- [ ] Add tests:
  - [ ] `test_protected_routes_do_not_return_permissive_cors_headers_by_default`

### 5. Replace custom reversible credential storage with authenticated encryption
- [ ] Inventory all credential storage and migration paths in `ragent-storage`.
- [ ] Remove the custom Blake3/XOR scheme for new writes.
- [ ] Remove legacy fixed-key XOR fallback storage.
- [ ] Adopt a well-reviewed AEAD primitive such as `aes-gcm` or `chacha20poly1305`.
- [ ] Add integrity protection so ciphertext tampering is detectable.
- [ ] Use an OS credential store or a user-supplied master key for key management.
- [ ] Migrate legacy stored credentials on read/write.
- [ ] Document the migration and failure/recovery behavior.

## Low priority / hardening

### 6. Improve bearer-token lifecycle management
- [ ] Allow the server bearer token to be supplied via config, env, or CLI.
- [ ] Add an operator-controlled rotation model.
- [ ] Consider supporting per-client or scoped tokens.
- [ ] Document recommended token handling for local and shared deployments.

## Security test coverage checklist

### 7. Add integration tests for command-execution permission enforcement
- [ ] Create or expand tests under `crates/ragent-agent/tests/`.
- [ ] Add tests:
  - [ ] `test_bash_permission_denies_when_any_subcommand_is_denied`
  - [ ] `test_bash_permission_checks_each_subcommand_after_timeout_prefix_stripped`
  - [ ] `test_bash_permission_does_not_split_on_quoted_semicolons_or_pipes`
  - [ ] `test_bash_safe_command_short_circuits_permission_prompt`
  - [ ] `test_check_permission_with_prompt_auto_approves_only_hardwired_helper_tools`
  - [ ] `test_check_permission_with_prompt_does_not_auto_approve_similar_non_helper_tool_names`

### 8. Add end-to-end tests for bash guard-layer bypass resistance
- [ ] Add tests:
  - [ ] `test_bash_execute_rejects_banned_tool_before_spawn`
  - [ ] `test_bash_execute_rejects_denied_command_in_pipeline`
  - [ ] `test_bash_execute_rejects_directory_escape_via_absolute_path_and_parent_segments`
  - [ ] `test_bash_execute_rejects_obfuscated_eval_patterns`
  - [ ] `test_bash_execute_rejects_newline_separated_dangerous_command`
  - [ ] `test_bash_execute_cleans_up_temp_script_after_failure`
  - [ ] `test_bash_execute_cleans_up_temp_script_after_timeout`

### 9. Expand HTTP auth and cross-client abuse-case tests
- [ ] Add tests:
  - [ ] `test_reply_permission_rejects_unknown_session`
  - [ ] `test_reply_permission_rejects_unknown_request_id`
  - [ ] `test_reply_permission_rejects_request_id_for_different_session`
  - [ ] `test_events_stream_does_not_emit_other_sessions_when_scoped_subscription_requested`
  - [ ] `test_protected_routes_do_not_return_permissive_cors_headers_by_default`
  - [ ] `test_auth_rejects_bearer_token_with_trailing_garbage_or_whitespace_variants`

### 10. Make new security tests deterministic and isolated
- [ ] Use unique session IDs per test.
- [ ] Avoid shared `/tmp` collisions in bash/state-file tests.
- [ ] Subscribe to broadcast channels before triggering events.
- [ ] Use deterministic timing controls where practical (`tokio::time::pause()`, explicit short timeouts).
- [ ] Serialize or reset tests that mutate global allow/deny lists.

## Performance and resilience follow-ups

### 11. Put expensive team/task operations back behind normal approval flow
- [ ] Remove the unbounded fast path for mutating `team_*` and `*_task` operations.
- [ ] Confirm that expensive orchestration actions remain gated by permissions and/or explicit policy.
- [ ] Measure provider-token, CPU, and memory amplification risk after changes.

### 12. Reduce hot-path contention in the rate limiter
- [ ] Avoid full-map `retain()` sweeps on the request hot path.
- [ ] Move eviction to a periodic/background cleanup strategy, or use a more concurrency-friendly structure.
- [ ] Benchmark p95/p99 latency with large numbers of session IDs.

### 13. Remove blocking filesystem I/O from async bash execution paths
- [ ] Replace `std::fs` calls in async execution paths with `tokio::fs` or `spawn_blocking`.
- [ ] Reduce temporary file churn for repeated bash invocations.
- [ ] Benchmark concurrent bash execution throughput and tail latency.

### 14. Cache repeated permission-check work
- [ ] Cache canonical project-root resolution where safe.
- [ ] Precompile permission-rule glob matchers when loading config.
- [ ] Benchmark large permission-heavy workflows before and after.

### 15. Reduce repeated full-file memory-block loads
- [ ] Reuse scope lookup results instead of reloading memory blocks repeatedly.
- [ ] Avoid duplicate `MEMORY.md` loads during teammate spawn cycles.
- [ ] Measure latency with large memory files.

### 16. Make bash-output truncation cheaper and more accurate
- [ ] Avoid rescanning the entire output just to compute truncation metadata.
- [ ] Report omitted bytes or exact omitted lines instead of derived estimates.
- [ ] Measure CPU and memory behavior with very large command output.
