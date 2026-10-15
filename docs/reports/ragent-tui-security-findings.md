# Security Findings — crates/ragent-tui

Date: 2026-03-30
Reviewer: security-reviewer (tm-001)

Summary
-------
This review covers unsafe code, input validation, secrets handling, filesystem/tempfile usage, use of OS APIs (process/clipboard), and dependency surface for the crates/ragent-tui crate.

High-level outcome: No use of Rust `unsafe` was found. The largest risks are:
- Secrets exposure in logs and UI (High)
- External command/executable handling and possible path injection (High)
- Percent-decoding & path handling for file:// URIs (Medium)
- Clipboard image handling & temp file lifecycle (Low→Medium)

Detailed findings
-----------------
1) Secret exposure via logging and UI
- Severity: High
- Files & lines:
  - crates/ragent-tui/src/app.rs: detect_provider reads env vars (lines ~760-870; specific matches at 786-821, 832, 860). See lines: 786-795 (ANTHROPIC), 800-807 (OPENAI), 812-828 (GENERIC_OPENAI fallback), 832-839 (GITHUB_COPILOT_TOKEN), 860-866 (OLLAMA_HOST).
  - crates/ragent-tui/src/lib.rs: run_tui drains tracing records into the app log (lib.rs lines ~158-170).
  - crates/ragent-tui/src/tracing_layer.rs: forwarding of full tracing messages into the TUI (lines 72-96).
  - Many push_log / tracing calls across src/app.rs (many locations; see push_log call at 6325 for implementation and ~112 occurrences across the crate).
- Impact: Raw API keys or tokens may be displayed in the on-screen log panel, written to persisted logs, or captured by CI artifacts / screenshots. Because the TUI intentionally displays tracing records, any unmasked secret in a tracing::log or push_log will be visible to users and anyone with access to logs.
- Remediation:
  - Introduce a mask_secret helper and use it at every env::var read site and before any user-visible or tracing::...! macro call that could include an API key/token/secret.
  - Audit all push_log and tracing::...! usages and remove any direct interpolation of variables named key/token/secret. Replace with masked values, or log presence only.
  - Add a CI grep lint that fails when a push_log/tracing::...!(...) contains unsubstituted key/token/secret names (except in a designated mask helper).
- Suggested patch (high level — insert in crates/ragent-tui/src/app/state.rs):

pub fn mask_secret(s: &str) -> String {
    if s.len() <= 8 {
        "***".to_string()
    } else {
        format!("{}***{}", &s[..4], &s[s.len()-4..])
    }
}

Usage example (in app.rs):
if let Ok(key) = std::env::var("OPENAI_API_KEY") {
    let masked = mask_secret(&key);
    tracing::debug!("OpenAI API key present: {}", masked);
    // use `key` for client creation but never log it or push to UI
}

Owners & estimate: security-reviewer; estimate: medium (3-5h) to implement mask helper and audit logging, plus test authoring by test-reviewer (2h).


2) External command invocation and enabling discovered executables
- Severity: High
- Files & lines:
  - crates/ragent-tui/src/app.rs: detect_git_branch uses std::process::Command::new("git") (lines 1524-1530).
  - crates/ragent-tui/src/app.rs: enable_discovered_server / enable_discovered_mcp_server (around lines ~1200-1295) persist discovered server executable paths/args to ragent.json via atomic_config_update in state.rs.
- Impact: If discovered executable paths or args are not validated, an attacker (or maliciously crafted discovery input) may cause unexpected executables to be recorded in config and later invoked, potentially leading to command injection or escalation. Even if Command::new is used without shell, storing paths with shell metacharacters is dangerous if future code passes them to a shell or concatenates them.
- Remediation:
  - Add validate_executable_path(path: &Path) -> Result<(), String> that rejects paths containing control/newline characters or shell metacharacters (';', '|', '&', '$', '>', '<', '`', '\n', '\r', '"', '\'') and checks metadata when possible (is_file + executable bit on Unix).
  - Call validate_executable_path before persisting discovered server entries. If validation fails, show a clear error and do not write to ragent.json.
  - Ensure any future invocations use Command::new(path) with explicit args slice (no shell -c). Add a code comment in config persistence noting this invariant.
- Suggested validate_executable_path implementation (insert into app.rs or state.rs):

fn validate_executable_path(p: &std::path::Path) -> bool {
    if let Some(s) = p.to_str() {
        // Reject control characters and common shell metacharacters
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

- Owners & estimate: security-reviewer; estimate: medium (2-4h) to implement validation and tests.


3) Percent-decoding for file:// paths (percent_decode_path)
- Severity: Medium
- Files & lines:
  - crates/ragent-tui/src/app/state.rs: percent_decode_path currently uses from_utf8(...).unwrap_or("") when parsing hex bytes (lines ~112-121). This uses unwrap_or and allocates an intermediate &str which is fragile.
- Impact: Malformed % sequences may be interpreted incorrectly. On non-Unix systems, non-UTF-8 sequences are lossy (String::from_utf8_lossy) — acceptable but should be explicit. Avoid unwraps and use safe hex nibble parsing.
- Remediation & suggested patch (replace percent_decode_path):

pub fn percent_decode_path(s: &str) -> std::path::PathBuf {
    fn hex_val(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    }

    let input = s.as_bytes();
    let mut bytes = Vec::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        if input[i] == b'%' && i + 2 < input.len() {
            if let (Some(h), Some(l)) = (hex_val(input[i + 1]), hex_val(input[i + 2])) {
                bytes.push(h << 4 | l);
                i += 3;
                continue;
            }
        }
        bytes.push(input[i]);
        i += 1;
    }
    bytes_to_path(&bytes)
}

- Owners & estimate: test-reviewer (implement tests), security-reviewer to author change; estimate: low/med (2-3h including tests).


4) Clipboard image handling & temp files
- Severity: Low → Medium (depending on threat model)
- Files & lines:
  - crates/ragent-tui/src/app/state.rs: save_clipboard_image_to_temp enforces limits and uses tempfile::Builder (lines ~161-201). Usage occurs at app.rs paste_image_from_clipboard (around 5151-5204).
- Impact: Large clipboard images are rejected; temp files are persisted (kept) and not auto-deleted. Persisted files may remain on disk; if they contain sensitive content they could leak. The temp file permissions rely on the OS default umask — consider explicit restrictive permissions.
- Remediation:
  - When creating the temp file, set explicit restrictive permissions on Unix (0o600) after persistence, or create with permissions via platform-specific APIs.
  - Consider adding an auto-prune policy or a user-visible location for attachments and a note in docs about lifecycle.
- Suggested change (post-save):
  #[cfg(unix)] {
      use std::os::unix::fs::PermissionsExt;
      let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
  }

- Owners & estimate: owner: security-reviewer; estimate: low (1-2h).


5) Atomic config writes: atomic_config_update
- Severity: Low → Medium
- Files & lines:
  - crates/ragent-tui/src/app/state.rs: atomic_config_update implements file locking and NamedTempFile persist (lines 42-93). Good: uses fs2 flock and temp file rename.
  - Call-sites in app.rs (enable_discovered_server and enable_discovered_mcp_server) already use atomic_config_update — confirm all config writes use it.
- Remediation:
  - Audit remaining code to ensure any direct writes to ragent.json or other config files use atomic_config_update.
  - Add tests to simulate concurrent writers where feasible.
- Owners & estimate: security-reviewer; estimate: low (1-2h for audit & small fixes).


Additional notes
----------------
- No occurrences of "unsafe" code in the crate (grep found only a comment in tests). This is good.
- arboard clipboard usage happens synchronously in several places; ensure any platform-specific clipboard APIs do not leak file descriptors or secrets. get_clipboard() and set_clipboard() handle text only and are called from UI/event thread — acceptable for a TUI.
- Many push_log/tracing::... calls exist; these must be audited to prevent accidental secret leakage.

Concrete next steps (recommended prioritised)
-------------------------------------------
1) Implement mask_secret and audit all env var reads and logs (High). Owner: security-reviewer. Estimate: medium.
2) Implement validate_executable_path and call it before persisting discovered servers (High). Owner: security-reviewer. Estimate: medium.
3) Harden percent_decode_path as suggested and add unit tests (Medium). Owner: test-reviewer / security-reviewer. Estimate: low.
4) Set restrictive permissions on persisted clipboard attachments and document lifecycle (Low). Owner: security-reviewer. Estimate: low.
5) Add CI lint to detect accidental logging of variables containing key/token/secret (Medium). Owner: lead/test-reviewer. Estimate: low/medium.

Appendix: suggested patches (copy into the appropriate files)
------------------------------------------------------------
1) mask_secret (crates/ragent-tui/src/app/state.rs)

Add near other helpers (for example after percent_decode_path):

pub fn mask_secret(s: &str) -> String {
    // Return a short masked representation suitable for logs/UI.
    if s.len() <= 8 {
        "***".to_string()
    } else {
        format!("{}***{}", &s[..4], &s[s.len()-4..])
    }
}

Add unit test in crates/ragent-tui/tests/test_secret_mask.rs per example in COMPLIANCE.md.


2) validate_executable_path (crates/ragent-tui/src/app.rs)

Add helper (near other helpers at top of file):

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

Usage: call before atomic_config_update when enabling discovered servers. Return an error to the user if invalid.


3) percent_decode_path safe hex parsing (crates/ragent-tui/src/app/state.rs)

Replace current implementation (avoid unwraps and use explicit hex_val as in this file's Detailed findings above).


Timeline & owners summary
-------------------------
- Mask secrets / logging audit — owner: security-reviewer (tm-001) — estimate: medium (3-5h)
- Add masking unit tests and CI lint — owner: test-reviewer — estimate: low (2-3h)
- Executable path validation and tests — owner: security-reviewer — estimate: medium (2-4h)
- Percent-decode hardening & tests — owner: test-reviewer — estimate: low (2h)
- Clipboard temp perms & lifecycle docs — owner: security-reviewer — estimate: low (1-2h)

If you want I can implement the mask_secret + audit (Task 1.1) and validate_executable_path (Task 2.1) now and run cargo test for the crate. Confirm which tasks to implement and I will follow up with a patch.


End of report.
