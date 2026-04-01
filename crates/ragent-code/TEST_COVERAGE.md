TEST COVERAGE REVIEW: crates/ragent-core, ragent-server, ragent-tui

Date: 2026-03-30
Author: test-reviewer (team: code-review-ragent-code)

Summary
-------
I reviewed the tests present under:
- crates/ragent-core/tests
- crates/ragent-server/tests
- crates/ragent-tui/tests

I ran an automated search to map src modules to test references and identified modules with no direct test references. Below I list gaps and recommended tests (unit, integration, and property) to improve coverage, plus high-priority security/auth checks related to session/permission boundaries.

Method
------
- Enumerated source files in each crate and searched test files for direct references to symbols/file basenames.
- Collected files that show no direct references from the test suite as candidates for missing tests.
- Suggested concrete tests to add (with filenames and brief test descriptions).

Findings and Recommendations
----------------------------
A. crates/ragent-core

Files with no direct test references (candidates for adding tests)
- src/tool/file_lock.rs
- src/tool/file_ops_tool.rs
- src/tool/libreoffice_common.rs
- src/tool/libreoffice_info.rs
- src/tool/libreoffice_read.rs
- src/tool/libreoffice_write.rs

Reasons these are important
- file_lock.rs: concurrency and correctness of file locks affects many higher-level features (multiedit, file ops). It's low-level and brittle under race conditions.
- file_ops_tool.rs and libreoffice_*: heavy I/O and external-binary interactions need error, fallback, and integration tests (including when external dependencies are absent).

Recommended tests (unit/integration/property)
1) Unit: file_lock
- File: crates/ragent-core/tests/test_file_lock.rs
- Tests:
  - lock_acquire_release: acquire lock on temp file, ensure subsequent release frees it.
  - double_acquire: acquiring twice by same process returns expected behavior (error or reentrant behavior) — assert contract.
  - concurrent_locking: spawn threads to attempt locking same path; assert only one holds the lock at a time (use short sleeps/timeouts).
  - recover_from_crash: simulate stale lock file and assert cleanup path (if implemented).

2) Integration: file_ops_tool
- File: crates/ragent-core/tests/test_file_ops_tool.rs
- Tests:
  - successful read/write of temporary files through the tool wrapper.
  - error cases: missing file, permission denied (use tempdir with restrictive perms).
  - ensure correct translations of paths and error messages.

3) Integration/Mock: libreoffice_* tools
- File: crates/ragent-core/tests/test_libreoffice_tools.rs
- Tests:
  - When libreoffice binary missing: tool returns a predictable error (test by temporarily overriding PATH to empty).
  - When binary present (if CI provides or via mocking wrapper): basic convert/read/write roundtrip using small sample files (ODS/ODT/ODP) placed under tests/fixtures.
  - Ensure failure modes are handled (bad file, unsupported format).

4) Property tests (proptest/quickcheck): id and sanitize
- File: crates/ragent-core/tests/test_properties.rs
- Targets:
  - id.rs: uniqueness and parse round-trip; property: generated id parses and matches pattern and two independent generators do not collide in many trials.
  - sanitize.rs: idempotency and normalization: sanitize(sanitize(input)) == sanitize(input); outputs ascii/utf8 invariants.

5) Missing coverage around external provider adapters (already many tests exist) — add negative tests for provider timeouts and invalid responses (mock HTTP responses). If not already present, add tests that assert the provider registry falls back to other providers.

B. crates/ragent-server

Files with no direct test references
- src/lib.rs

Observations
- routes/mod.rs and sse.rs already have tests. lib.rs exports helpers / startup functions that have no tests.

Recommended tests
1) Unit: lib.rs exports
- File: crates/ragent-server/tests/test_lib.rs
- Tests:
  - startup_config_roundtrip: create a minimal configuration via code and ensure builder helpers behave and return configured values.
  - exported types: basic serialization/deserialization and default values.

2) Integration: HTTP endpoints auth/permission tests (high priority)
- File: crates/ragent-server/tests/test_routes_auth.rs
- Tests:
  - unauthenticated_access: ensure protected endpoints return 401/403 as documented (use actix/test or reqwest against test server instance).
  - session_revocation: simulate a session token that is revoked and assert endpoint denies actions that require session.
  - SSE: ensure events sent to SSE clients respect session scoping (already covered by event_to_sse tests but add auth negative cases).

3) Property tests: route parameter parsing
- File: crates/ragent-server/tests/test_routes_params_prop.rs
- Tests: property-based fuzzing of route parameters to ensure server returns safe errors (no panics) for arbitrary inputs.

C. crates/ragent-tui

Files with no direct test references
- src/widgets/permission_dialog.rs
- src/widgets/message_widget.rs (no direct references in search — note: there are #[cfg(test)] in the message_widget file but tests may be inline/private)
- src/logo.rs
- src/tips.rs
- src/tracing_layer.rs
- src/layout_teams.rs
- src/lib.rs

Observations
- There are multiple large integration tests under tests/ already (scrolling, slash commands, selection). However some UI pieces are untested:
  - permission_dialog: user flows when granting or denying permissions.
  - message_widget: formatting, wrapping, and selection rendering edge cases.
  - tracing_layer: ensure log formatting and layering do not panic.
  - logo/tips: simple static content functions should have smoke tests to guard accidental regressions.

Recommended tests
1) Unit: permission_dialog
- File: crates/ragent-tui/tests/test_permission_dialog.rs
- Tests:
  - show_dialog_state: ensure dialog builds with expected text given a permission request object.
  - accept_reject_flow: simulate pressing keys for accept/reject and assert returned choice.

2) Unit/Widget: message_widget
- File: crates/ragent-tui/tests/test_message_widget.rs
- Tests:
  - rendering_long_lines: render message longer than column width; assert wrapping and selection indices are consistent.
  - selection_and_copy: simulate selection mode and ensure the widget returns the correct substring.
  - timestamp_display: ensure timestamps are formatted to 2 decimal places for mm units if that codepath exists (per project units guideline).

3) Unit: tracing_layer
- File: crates/ragent-tui/tests/test_tracing_layer.rs
- Tests:
  - layering_no_panic: attach layer to a test subscriber and emit logs on hot path; ensure no panics and expected fields present.

4) Small smoke tests: logo/tips/lib
- File: crates/ragent-tui/tests/test_assets.rs
- Tests:
  - logo_contains_expected: returns non-empty string and contains project name.
  - tips_non_empty: tips collection length > 0 and contains at least one tip without forbidden characters.

Cross-cutting / High Priority Security & Auth Tests
--------------------------------------------------
This is high priority and addresses task-001 (audited authentication boundaries):
- Core code places permission/session logic in:
  - crates/ragent-core/src/permission/mod.rs
  - team manager and mailbox (crates/ragent-core/src/team/manager.rs and mailbox.rs)
  - session processing (crates/ragent-core/src/session/processor.rs)
  - server routes (crates/ragent-server/src/routes/mod.rs)

Recommended tests (security-focused)
1) Permission checks unit tests
- File: crates/ragent-core/tests/test_permission_checks.rs
- Tests:
  - privileged_action_denied_without_permission: exercise an action that requires a permission and assert permission::check returns an explicit error/denied state.
  - permission_metadata_edgecases: malformed metadata should not grant access.

2) Session boundary integration tests (core + server)
- File: crates/ragent-server/tests/test_session_auth_integration.rs
- Tests:
  - request_with_invalid_session: send request with expired/invalid token and assert 401.
  - request_with_insufficient_scope: session valid but missing permission; assert 403.
  - session_state_transitions: after revocation, subsequent requests fail.

3) Team mailbox/manager state transition tests
- File: crates/ragent-core/tests/test_team_auth_transitions.rs
- Tests:
  - only_team_owner_can_assign_tasks: verify manager enforces permission when calling assign API.
  - spawn/cleanup: ensure unauthorized spawn attempts are blocked and do not create resources.

Suggested Test Implementation Notes
----------------------------------
- Use tempdir (tempfile crate) for filesystem tests to avoid CI pollution.
- For concurrency tests, prefer crossbeam scoped threads or std threads with small timeouts; ensure tests are robust and do not hang.
- For external-binary tests (libreoffice), prefer to mock by temporarily setting PATH or providing a small stub executable in tests/fixtures that simulates behavior.
- For server integration tests, use actix-web::test utilities or spin up the server on a random port and use reqwest. Ensure tests clean up sockets and do not depend on global state.
- For property tests, add proptest to dev-dependencies and create lightweight properties that don't make tests flaky.

Priority / Roadmap (short)
- P0 (security): session/auth endpoint tests (server) + permission unit tests (core).
- P1: file_lock concurrency tests + libreoffice tooling error cases.
- P2: message_widget and permission_dialog unit tests in TUI + tracing_layer smoke tests.
- P3: property tests (id, sanitize) and route parameter fuzzing.

Suggested test filenames (summary)
- crates/ragent-core/tests/test_file_lock.rs
- crates/ragent-core/tests/test_file_ops_tool.rs
- crates/ragent-core/tests/test_libreoffice_tools.rs
- crates/ragent-core/tests/test_properties.rs
- crates/ragent-core/tests/test_permission_checks.rs
- crates/ragent-core/tests/test_team_auth_transitions.rs

- crates/ragent-server/tests/test_lib.rs
- crates/ragent-server/tests/test_routes_auth.rs
- crates/ragent-server/tests/test_session_auth_integration.rs
- crates/ragent-server/tests/test_routes_params_prop.rs

- crates/ragent-tui/tests/test_permission_dialog.rs
- crates/ragent-tui/tests/test_message_widget.rs
- crates/ragent-tui/tests/test_tracing_layer.rs
- crates/ragent-tui/tests/test_assets.rs

Notes / Caveats
----------------
- Many modules already have comprehensive tests; the above focuses on modules found to have no direct test references and on security/auth boundaries.
- The "no direct test references" approach finds modules that are not referenced by name in tests — some modules may still be covered indirectly through higher-level integration tests. Before writing tests, check for existing behavioral coverage and avoid duplication.
- Keep tests deterministic: avoid long sleeps; use synchronization primitives and timeouts.

Next Steps
----------
- I can begin implementing high-priority tests (P0) first (session/auth tests), then the file_lock concurrency tests. Confirm if plan approved, then I will open PR(s) with the test files and CI-friendly implementations.

