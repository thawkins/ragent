# COMPLIANCE — crates/ragent-tui

Date: 2026-03-30
Lead: ragent-tui-code-review

Executive Summary
-----------------
This report consolidates findings from security, test, and performance reviewers covering the crates/ragent-tui directory. Overall the crate is well-exercised with many focused tests, and contains no unsafe Rust. The highest-priority risks are:
- Secret exposure via logs/UI and tracing records (High)
- Persisting and later invoking discovered executables without validation (High)
- Excessive redraws and quadratic rendering costs in active-agents panel (High)

The remaining issues are medium/low severity performance and testing gaps (percent-decoding hardening, blocking model discovery, clipboard temp file lifecycle, and CI lint additions). Detailed findings and remediation steps are included below along with Milestones and Tasks to remediate.

Detailed Findings
-----------------
See the appended files for full reviewer outputs:
- security findings: crates/ragent-tui/security_findings.md
- test findings: crates/ragent-tui/test_findings.md
- performance findings: crates/ragent-tui/performance_findings.md

Remediation Recommendations (high-level)
----------------------------------------
1) Secret masking helper and logging audit — implement mask_secret helper, audit all tracing/push_log uses, add CI lint to detect raw secret logging.
2) Executable path validation — implement validate_executable_path and enforce before persisting discovered server entries.
3) Redraw optimization — add app.needs_redraw flag and only draw when necessary.
4) Active tasks traversal — replace repeated scans with a parent->children map to avoid O(n^2) behaviour.
5) Convert blocking discovery/IO on UI thread to async background tasks.
6) Add unit tests for all new helpers and CI linting

Milestones & Tasks
------------------
Milestone 1 — Secrets & Logging (owner: security-reviewer)
- Task 1.1: Implement mask_secret helper in crates/ragent-tui/src/app/state.rs
  - Owner: security-reviewer
  - Estimate: medium
  - Acceptance: mask_secret unit tests exist and pass, and all env-var reads that could be logged use masked values in sample traces.
- Task 1.2: Audit all push_log/tracing::...! usages and replace raw interpolations of env values with masked outputs or presence-only logs.
  - Owner: security-reviewer
  - Estimate: medium
  - Acceptance: grep-based CI lint (below) shows zero failures for unmasked secret logging patterns.
- Task 1.3: Add CI lint job that fails on suspicious logging patterns (suggested script at crates/ragent-tui/.github/scripts/lint_secrets.sh)
  - Owner: test-reviewer / lead
  - Estimate: low
  - Acceptance: CI job added and green on main branch with current code after fixes.

Milestone 2 — Executable Validation (owner: security-reviewer)
- Task 2.1: Implement validate_executable_path(&Path) and unit tests in crates/ragent-tui/tests/test_enable_discovered_server.rs
  - Owner: security-reviewer
  - Estimate: medium
  - Acceptance: tests cover metacharacter rejection cases and positive case with /usr/bin true executable.
- Task 2.2: Call validate_executable_path before atomic_config_update in enable_discovered_server and enable_discovered_mcp_server; return user-friendly error when invalid.
  - Owner: security-reviewer
  - Estimate: low
  - Acceptance: integration scenario where a DiscoveredServer with malicious path is rejected and not persisted.

Milestone 3 — Performance Hotfixes (owner: performance-reviewer)
- Task 3.1: Implement dirty redraw flag (app.needs_redraw) and gate terminal.draw calls. Update event handlers to mark dirty when state changes.
  - Owner: performance-reviewer
  - Estimate: medium
  - Acceptance: manual profiling shows reduction in draw calls when idle; CI tests unaffected.
- Task 3.2: Replace quadratic active-tasks traversal with one-pass parent->children map; avoid cloning app.active_tasks per-frame.
  - Owner: performance-reviewer
  - Estimate: medium
  - Acceptance: unit test or microbenchmark demonstrates O(n) traversal and reduced allocations.
- Task 3.3: Reduce per-frame allocations in render path (cache custom_names, reuse buffers, use Arc<String> in md cache)
  - Owner: performance-reviewer
  - Estimate: medium
  - Acceptance: measurable allocation reduction via profiling.

Milestone 4 — Async IO & Clipboard Hardening
- Task 4.1: Convert blocking model discovery to async: spawn background tasks and publish results via event bus.
  - Owner: performance-reviewer
  - Estimate: medium
  - Acceptance: UI remains responsive while discovery happens; tests simulate delayed discovery and confirm UI updated when ready.
- Task 4.2: Ensure temp files created for clipboard images use restrictive permissions on Unix and have documented lifecycle/auto-prune policy.
  - Owner: security-reviewer
  - Estimate: low
  - Acceptance: temp files created have 0o600 permissions and docs updated.

Milestone 5 — Tests & CI
- Task 5.1: Add unit tests for mask_secret, validate_executable_path, detect_git_branch empty-dir behavior, percent_decode_path hardening.
  - Owner: test-reviewer
  - Estimate: medium
  - Acceptance: new tests pass in CI (cargo test --lib && cargo test for crate)
- Task 5.2: Add CI lint job to detect secret logging and dangerous persistence patterns.
  - Owner: lead / test-reviewer
  - Estimate: low
  - Acceptance: CI contains the job and it passes.

Appendix — Suggested patches and code snippets
---------------------------------------------
See crates/ragent-tui/security_findings.md, test_findings.md and performance_findings.md for code suggestions and concrete patches. If you wish, I can implement Task 1.1 and Task 2.1 now and run cargo test for crates/ragent-tui. Confirm which tasks to implement first.
