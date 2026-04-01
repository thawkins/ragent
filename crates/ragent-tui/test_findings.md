# Test Coverage Review — crates/ragent-tui

Reviewer: test-reviewer
Date: 2026-03-30

Summary
-------
I reviewed crates/ragent-tui (src/ and tests/) for test gaps, flaky tests, and CI omissions. The crate has a strong existing test suite (many focused tests cover percent_decode_path, clipboard temp file handling, provider detection via DB, many UI behaviors). Key gaps remain around secret masking, validation of discovered server executables before persisting into ragent.json, and a few potential flaky areas (external command invocation and environment-dependent detection). I list findings below with file:line pointers, severity, suggested tests and example test snippets, remediation steps, suggested owners and estimated effort.

Findings
--------

1) Missing secret-masking unit tests and audit of logging of secrets
- Files / refs:
  - crates/ragent-tui/src/app.rs: detect_provider (approx lines 786-828)
  - crates/ragent-tui/src/tracing_layer.rs (routes tracing records into TUI log)
- Severity: High
- Why: detect_provider reads environment variables (OPENAI_API_KEY, ANTHROPIC_API_KEY, GENERIC_OPENAI_API_KEY, GITHUB_COPILOT_TOKEN). There is no explicit helper documented/tested that masks secrets before they ever appear in logs/UI. CI could leak secrets in logs if code later logs raw values.
- Suggested tests to add:
  - Unit test for mask_secret(s: &str) behavior (ensure deterministic masking and edge cases)
  - Regression test ensuring push_log/tracing::... usages do not include raw secret substrings (a lint test or grep-based test in CI; unit test can assert that App::detect_provider does not return any string that equals an env value when represented as name/value in UI messages, but because detect_provider doesn't produce logs itself in a way that'll contain values, prefer a unit helper test for mask_secret and an automated grep CI check).
- Example test snippet (new file: crates/ragent-tui/tests/test_secret_mask.rs):

  ```rust
  #[test]
  fn mask_secret_hides_middle_and_short() {
      use ragent_tui::app::mask_secret;
      let long = "abcdefghijklmnopqrstuvwxyz";
      let masked = mask_secret(long);
      assert!(!masked.contains(long));
      assert!(masked.starts_with(&long[..4]));
      assert!(masked.ends_with(&long[long.len()-4..]));

      let short = "short";
      let masked_short = mask_secret(short);
      assert_eq!(masked_short, "***");
  }
  ```

- Remediation steps:
  - Implement mask_secret(s: &str) in app/state.rs (or another small helper module) and call it wherever env var-derived values could be logged or shown. Add unit tests above.
  - Add a CI lint step that greps crate sources for push_log/tracing::.*!(.*key|token|secret) or format!/println! usages containing variable names 'key', 'token', 'secret'. The script should be permissive about calls that use mask_secret.
- Owner: security-reviewer (implementation), test-reviewer (unit tests & CI check)
- Estimate: Low (2–4 hours)


2) Missing validation & tests for enable_discovered_server / enable_discovered_mcp_server
- Files / refs:
  - crates/ragent-tui/src/app.rs: enable_discovered_server (lines ~1205-1248)
  - crates/ragent-tui/src/app.rs: enable_discovered_mcp_server (lines ~1252-1293)
- Severity: High
- Why: These functions persist discovered executable paths (server.executable) and args into ragent.json. There is no validation preventing paths containing shell metacharacters or newline characters. If a path with malicious characters is later executed via a shell (in other components), this may enable command injection.
- Suggested tests to add:
  - Unit tests for a new function validate_executable_path(&Path) -> bool that rejects suspicious paths (control characters, shell metacharacters, and optionally require absolute or simple basename). Put tests in crates/ragent-tui/tests/test_enable_discovered_server.rs.
  - Tests asserting enable_discovered_server returns Err when given a DiscoveredServer whose executable contains ';' or '\n' or other forbidden chars.
- Example test snippet:

  ```rust
  #[test]
  fn validate_executable_path_rejects_shell_metacharacters() {
      use std::path::Path;
      use ragent_tui::app::validate_executable_path;
      assert!(!validate_executable_path(Path::new("/bin/sh;rm -rf /")));
      assert!(!validate_executable_path(Path::new("/tmp/bad|cmd")));
      assert!(validate_executable_path(Path::new("/usr/bin/clangd")));
  }
  ```

  And for enable_discovered_server behavior (pseudo):

  ```rust
  #[test]
  fn enable_discovered_server_rejects_malicious_path() {
      use ragent_core::lsp::discovery::DiscoveredServer;
      use ragent_tui::App;
      let app = make_minimal_app();
      let srv = DiscoveredServer::new("evil", std::path::PathBuf::from("/bin/sh;rm -rf /"), vec![], vec![]);
      let res = app.enable_discovered_server(&srv);
      assert!(res.is_err());
  }
  ```

- Remediation steps:
  - Implement validate_executable_path(&Path) (reject newline, ';', '|', '&', '$', '\n', '\r' and shell glob characters '*', '?', '~' unless explicitly intended). On Unix, optionally check metadata.is_file() and execute bit.
  - Call validation from enable_discovered_server and enable_discovered_mcp_server and return a clear Err message when validation fails.
  - Add unit tests as above.
- Owner: security-reviewer (validation implementation), test-reviewer (tests)
- Estimate: Medium (3–6 hours)


3) detect_git_branch runs external `git` — add tests/guards and avoid flaky behavior
- Files / refs:
  - crates/ragent-tui/src/app.rs: detect_git_branch (lines ~1524-1536)
- Severity: Medium
- Why: detect_git_branch calls `git rev-parse` which depends on git being present and the current working directory being within a git repo. While there are no tests currently invoking this function, future tests or refactors might assume deterministic behavior leading to flakiness in CI environments. Also the function is difficult to unit-test as-is.
- Suggested tests to add / refactor:
  - Add a small unit test that ensures detect_git_branch returns None when `git` is not available or when cwd is a fresh temporarydir without a git repo. This avoids relying on repo state.
  - Consider refactoring detect_git_branch to a small injectable helper that accepts a Command-runner function so tests can inject a fake result.
- Example test snippet:

  ```rust
  #[test]
  fn detect_git_branch_none_in_empty_dir() {
      use tempfile::tempdir;
      let d = tempdir().unwrap();
      let prev = std::env::current_dir().unwrap();
      std::env::set_current_dir(d.path()).unwrap();
      assert!(ragent_tui::app::App::detect_git_branch().is_none());
      std::env::set_current_dir(prev).unwrap();
  }
  ```

- Remediation steps:
  - Add test above to assert safe behavior in empty dirs.
  - (Optional) Refactor detect_git_branch for testability (inject command runner), update callers.
- Owner: test-reviewer
- Estimate: Low (1–2 hours)


4) percent_decode_path hex parsing uses unwraps — tests exist but still consider hardening
- Files / refs:
  - crates/ragent-tui/src/app/state.rs: percent_decode_path (lines ~112-130)
  - Tests: crates/ragent-tui/tests/test_clipboard_tempfile.rs already has thorough tests for percent_decode_path (many cases including malformed sequences)
- Severity: Medium (informational)
- Why: The implementation uses from_utf8(...).unwrap_or("") inside u8::from_str_radix which is fragile. Tests cover many cases, including malformed sequences. Still recommend hardening implementation to avoid any unwraps and add a unit test for an invalid hex nibble like "%G1" (already covered) — existing tests test_percent_decode_invalid_hex covers %ZZ.
- Suggested action:
  - Replace the hex parsing with a small safe hex_val helper that matches bytes directly (no str conversions) and add a unit test asserting behavior for "%G1" and trailing "%" — tests appear to already cover such cases; review implementation change and ensure tests still pass.
- Owner: test-reviewer / security-reviewer
- Estimate: Low (1–2 hours)


5) Missing CI lint step to detect accidental logging of secrets and insecure persistence patterns
- Files: repository CI configuration (.github/workflows/*) — not present in this crate; workspace-level CI should be updated.
- Severity: Medium
- Why: Automated detection of accidental logging or persistence of command strings with shell metacharacters helps prevent regressions.
- Suggested action:
  - Add a job in the repository CI (or crate-level check invoked by workspace CI) that runs a small shell script or Rust-based grep to scan crates/ragent-tui/src for suspicious patterns. Example rules:
    - fail if push_log(... variable containing 'key' | 'token' | 'secret' ) or tracing::.*!(.*key|token|secret) appear
    - fail if any enable_discovered_* persists a command string containing shell metacharacters (best-effort static detection)
  - Add the unit tests described above to the crate test suite.
- Owner: lead / performance-reviewer (CI), test-reviewer (test harness)
- Estimate: Medium (3–5 hours)


6) Tests manipulating current_dir sometimes use global state — concurrency guard present but ensure consistent locking in all tests
- Files / refs:
  - crates/ragent-tui/tests/test_force_cleanup_modal.rs (uses static CWD_LOCK)
  - Multiple tests change current_dir (grep showed many instances)
- Severity: Low
- Why: Many tests temporarily change the current directory. The codebase already uses a static Mutex (CWD_LOCK) in some tests to serialise such tests; not all tests do. This can cause flakiness under parallel test execution if tests run with --test-threads>1 and touch cwd concurrently.
- Suggested actions:
  - Ensure tests that change cwd either acquire a common global lock (e.g., static MUTEX) or use spawn_subprocess style tests where the child process changes cwd without affecting the parent.
  - Alternatively, mark cwd-changing tests with #[serial] (via serial_test crate) or run cargo test with single-threaded execution for this crate in CI.
- Owner: test-reviewer
- Estimate: Low (1–2 hours)


7) Coverage blind spots: UI rendering and integration run_tui path
- Files / refs:
  - crates/ragent-tui/src/lib.rs: run_tui (lines ~76-226)
- Severity: Medium
- Why: run_tui is an async function that manipulates the terminal, LSP manager, event bus and draws to a real terminal. There are no integration tests that exercise the full run_tui path in CI (expected), which is OK, but some of its logic (history file IO, LSP startup wiring) would benefit from smoke tests.
- Suggested tests:
  - Add a smoke/integration test that starts run_tui in a headless mode or mocking terminal backend. Alternatively expose smaller units (e.g., App::new + check_provider_health + set_history_file + load_history) and test their behavior with temp dirs and an in-memory Storage.
- Example (sketch): a test that constructs App with show_log=false, sets a temporary history path, toggles load_history/save_history, and asserts no panics. This avoids launching alternate screen and raw mode.
- Owner: test-reviewer
- Estimate: Medium (3–6 hours)


Recommended Test Files to Add
-----------------------------
- crates/ragent-tui/tests/test_secret_mask.rs (mask_secret tests)
- crates/ragent-tui/tests/test_enable_discovered_server.rs (validate_executable_path + enable_discovered_server rejects)
- crates/ragent-tui/tests/test_detect_git_branch.rs (empty-dir behavior)
- Add CI lint script: .github/workflows/ci.yml update or script at .github/scripts/lint_secrets.sh

Appendix — concrete code snippets
---------------------------------
- mask_secret implementation (put into src/app/state.rs):

  ```rust
  /// Mask a secret for display in logs/UI (preserve a prefix/suffix and hide middle).
  pub fn mask_secret(s: &str) -> String {
      if s.len() <= 8 {
          "***".to_string()
      } else {
          format!("{}***{}", &s[..4], &s[s.len()-4..])
      }
  }
  ```

- validate_executable_path (put into src/app.rs near enable_discovered_server):

  ```rust
  pub fn validate_executable_path(path: &std::path::Path) -> bool {
      if let Some(s) = path.to_str() {
          // Reject control/newline and common shell metacharacters
          if s.contains('\n') || s.contains('\r') || s.contains(';') || s.contains('|') || s.contains('&') || s.contains('$') {
              return false;
          }
          // Reject globs/tildes unless explicitly allowed
          if s.contains('*') || s.contains('?') || s.contains('~') {
              return false;
          }
      }
      #[cfg(unix)]
      {
          if let Ok(meta) = std::fs::metadata(path) {
              if !meta.is_file() { return false; }
              use std::os::unix::fs::PermissionsExt;
              if meta.permissions().mode() & 0o111 == 0 { return false; }
          }
      }
      true
  }
  ```

- CI grep example (shell script):

  ```sh
  #!/usr/bin/env bash
  set -euo pipefail
  if grep -R --line-number -E "push_log\(|tracing::.*!\(|format!\(|println!\(" crates/ragent-tui/src | grep -E "key|token|secret"; then
    echo "Potential secret logging found in ragent-tui/src" >&2
    exit 1
  fi
  ```

Closing notes
-------------
Overall test coverage is good for many TUI-specific units (percent-decoding, clipboard image handling, provider DB logic, many slash-command flows). Prioritise: (1) secret masking + CI lint (High), (2) validate discovered-executable paths + tests (High), (3) guard cwd mutation tests and add a small detect_git_branch test (Medium).

If you want, I can implement the mask_secret helper + unit tests and add the validate_executable_path + tests. Which would you like me to implement first?
