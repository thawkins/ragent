//! Verification-command gate for goal-driven agentic loops.
//!
//! This module implements the verification half of the loop contract
//! described in `specs/agentloop/SPEC.md` (FR-007): a loop may carry an
//! optional verification command whose exit status gates goal achievement.
//! When the model first responds without tool calls, the command runs
//! automatically — success completes the loop, failure with steps remaining
//! appends the failure output as an observation so the model can self-correct,
//! and failure without steps remaining ends the run as a budget exhaustion.
//!
//! The command executes in the session's working directory with a fixed
//! timeout and ASCII-truncated output capture, mirroring the shell handling
//! of the core bash tool.
//!
//! Dependencies: `tokio` (blocking offload), `std::process::Command`.

use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

/// Wall-clock limit for one verification-command run (10 minutes).
///
/// Verification commands are typically quick test/grep invocations; a run
/// that exceeds this budget is treated as a failure (the loop must not hang
/// waiting on a stalled verification step).
const VERIFICATION_TIMEOUT: Duration = Duration::from_secs(600);

/// Maximum number of characters kept from the head of the captured output.
const VERIFICATION_HEAD_CHARS: usize = 8000;

/// Maximum number of characters kept from the tail of the captured output.
const VERIFICATION_TAIL_CHARS: usize = 2000;

/// Truncate `text` to at most head + tail characters, joined by an elision
/// marker. The cut always falls on a UTF-8 character boundary.
fn truncate_head_tail(text: &str, head: usize, tail: usize) -> String {
    if text.chars().count() <= head + tail {
        return text.to_string();
    }
    let head_text: String = text.chars().take(head).collect();
    let total = text.chars().count();
    let tail_text: String = text.chars().skip(total - tail).collect();
    format!(
        "{head_text}\n... [output truncated: {} characters elided] ...\n{tail_text}",
        total - head - tail
    )
}

/// Result of running the loop's verification command (FR-007).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationOutcome {
    /// The command exited with status 0. Carries the captured output.
    Success(String),
    /// The command failed: non-zero exit, timeout, or spawn error.
    /// Carries a short human-readable reason and the captured output.
    Failure {
        /// Short reason: exit code, signal, timeout, or spawn error.
        reason: String,
        /// Captured stdout + stderr (ASCII-truncated).
        output: String,
    },
}

impl VerificationOutcome {
    /// Whether the verification command passed.
    pub fn passed(&self) -> bool {
        matches!(self, Self::Success(_))
    }

    /// The captured command output, for both variants.
    pub fn output(&self) -> &str {
        match self {
            Self::Success(output) | Self::Failure { output, .. } => output,
        }
    }

    /// One-line summary label used in the `Event::LoopTerminated`
    /// verification field and in user-facing notices.
    pub fn label(&self) -> String {
        match self {
            Self::Success(_) => "verification command passed".to_string(),
            Self::Failure { reason, .. } => {
                format!("verification command failed ({reason})")
            }
        }
    }
}

/// Run `cmd` in `working_dir`, capturing stdout and stderr with the fixed
/// verification timeout (FR-007).
///
/// The blocking process execution is off-loaded to `spawn_blocking` so the
/// async executor is not stalled while the command runs.
///
/// # Errors
/// Returns an error only when the command could not be spawned at all
/// (e.g. the program does not exist). A non-zero exit status, a timeout, or
/// an abnormal termination are reported as [`VerificationOutcome::Failure`],
/// not as errors — the gate must see them, not crash on them.
pub async fn run_verification_command(
    cmd: &str,
    working_dir: &std::path::Path,
) -> anyhow::Result<VerificationOutcome> {
    let script = cmd.to_string();
    let dir = working_dir.to_path_buf();
    tokio::task::spawn_blocking(move || execute_verification_script(&script, &dir))
        .await
        .map_err(|e| anyhow::anyhow!("verification command join error: {e}"))?
}

/// Synchronous execution of the verification script.
///
/// Spawns `bash -c <script>` with piped output, drains stdout/stderr on
/// helper threads (so a chatty child cannot deadlock on a full pipe buffer),
/// and polls the child with a poll interval until either it exits or the
/// deadline passes — on expiry the child is killed and the run reports a
/// timeout failure. The blocking work runs on the tokio blocking pool via
/// the async wrapper.
fn execute_verification_script(
    script: &str,
    working_dir: &std::path::Path,
) -> anyhow::Result<VerificationOutcome> {
    use std::io::Read;

    let result = Command::new("bash")
        .arg("-c")
        .arg(script)
        .current_dir(working_dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn();

    let mut child = match result {
        Ok(child) => child,
        Err(e) => {
            return Ok(VerificationOutcome::Failure {
                reason: format!("spawn error: {e}"),
                output: String::new(),
            });
        }
    };

    // Drain stdout/stderr concurrently so the child never blocks on a full
    // pipe while the poll loop waits for it to exit.
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();
    let stdout_handle = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(mut pipe) = stdout_pipe {
            let _ = pipe.read_to_string(&mut buf);
        }
        buf
    });
    let stderr_handle = std::thread::spawn(move || {
        let mut buf = String::new();
        if let Some(mut pipe) = stderr_pipe {
            let _ = pipe.read_to_string(&mut buf);
        }
        buf
    });

    // Poll-wait loop with a hard kill at the deadline.
    let poll_interval = Duration::from_millis(100);
    let mut waited = Duration::ZERO;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {}
            Err(e) => {
                let _ = child.kill();
                let _ = stdout_handle.join();
                let _ = stderr_handle.join();
                return Ok(VerificationOutcome::Failure {
                    reason: format!("wait error: {e}"),
                    output: String::new(),
                });
            }
        }
        if waited >= VERIFICATION_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            let stdout = stdout_handle.join().unwrap_or_default();
            let stderr = stderr_handle.join().unwrap_or_default();
            let captured = truncate_head_tail(
                format!("{stdout}{stderr}").trim(),
                VERIFICATION_HEAD_CHARS,
                VERIFICATION_TAIL_CHARS,
            );
            return Ok(VerificationOutcome::Failure {
                reason: format!("timed out after {}s", VERIFICATION_TIMEOUT.as_secs()),
                output: captured,
            });
        }
        std::thread::sleep(poll_interval);
        waited += poll_interval;
    };

    let stdout = stdout_handle.join().unwrap_or_default();
    let stderr = stderr_handle.join().unwrap_or_default();

    let mut captured = stdout;
    if !stderr.trim().is_empty() {
        if !captured.is_empty() {
            captured.push('\n');
        }
        captured.push_str(&stderr);
    }
    let captured = truncate_head_tail(
        captured.trim(),
        VERIFICATION_HEAD_CHARS,
        VERIFICATION_TAIL_CHARS,
    );

    let status = match status {
        Some(status) => status,
        None => {
            return Ok(VerificationOutcome::Failure {
                reason: "child exited without a status".to_string(),
                output: String::new(),
            });
        }
    };
    if status.success() {
        return Ok(VerificationOutcome::Success(captured));
    }
    let reason = describe_verification_status(&status);
    Ok(VerificationOutcome::Failure {
        reason,
        output: captured,
    })
}

/// Describe a process exit status for the verification-failure reason,
/// mirroring the bash tool's exit-status description.
fn describe_verification_status(status: &std::process::ExitStatus) -> String {
    if let Some(code) = status.code() {
        return format!("exit code {code}");
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(sig) = status.signal() {
            return format!("killed by signal {sig}");
        }
    }
    "abnormal termination".to_string()
}

/// Observation message appended to the model's context when the verification
/// command fails but steps remain (FR-007): the failure output becomes the
/// next observation so the model can correct its work.
pub fn verification_failure_observation(cmd: &str, outcome: &VerificationOutcome) -> String {
    format!(
        "System note: the goal's verification command failed. \
         The loop cannot be marked complete until it passes.\n\
         Command: {}\n\
         Result: {}\n\
         Output:\n{}",
        cmd,
        outcome.label(),
        outcome.output()
    )
}

/// Convenience wrapper used by tests and callers that only need the async
/// result as an `Arc`-friendly owned value.
pub async fn run_verification_command_owned(
    cmd: Arc<String>,
    working_dir: &std::path::Path,
) -> anyhow::Result<VerificationOutcome> {
    run_verification_command(&cmd, working_dir).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "ragent-verification-test-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[tokio::test]
    async fn success_and_failure_status_are_reported() {
        let dir = temp_dir();
        let ok = run_verification_command("true", &dir).await.expect("run");
        assert!(ok.passed());
        assert!(matches!(ok, VerificationOutcome::Success(_)));

        let fail = run_verification_command("exit 3", &dir).await.expect("run");
        assert!(!fail.passed());
        match fail {
            VerificationOutcome::Failure { reason, output } => {
                assert!(reason.contains('3'), "reason mentions exit code: {reason}");
                assert_eq!(output, String::new());
            }
            other => panic!("expected failure, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn stderr_is_captured_and_trimmed() {
        let dir = temp_dir();
        let outcome = run_verification_command("echo out; echo err >&2", &dir)
            .await
            .expect("run");
        assert!(outcome.passed());
        assert!(outcome.output().contains("out"));
        assert!(outcome.output().contains("err"));
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[tokio::test]
    async fn missing_program_is_a_failure_not_an_error() {
        let dir = temp_dir();
        let outcome = run_verification_command("definitely-not-a-real-program-xyz", &dir)
            .await
            .expect("run");
        // `bash` itself exists, so the spawn succeeds and bash reports the
        // missing command with exit status 127.
        match outcome {
            VerificationOutcome::Failure { reason, .. } => {
                assert!(reason.contains("exit code"), "reason: {reason}");
            }
            other => panic!("expected failure, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn long_output_is_head_tail_truncated() {
        let dir = temp_dir();
        let script = "for i in $(seq 1 3000); do echo line-$i; done";
        let outcome = run_verification_command(script, &dir).await.expect("run");
        let output = outcome.output();
        assert!(output.contains("line-1"));
        assert!(output.contains("line-3000"));
        assert!(output.contains("truncated"));
        assert!(output.chars().count() <= VERIFICATION_HEAD_CHARS + VERIFICATION_TAIL_CHARS + 200);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn failure_observation_includes_command_and_output() {
        let dir = temp_dir();
        let outcome = run_verification_command("echo boom; exit 2", &dir)
            .await
            .expect("run");
        let observation = verification_failure_observation("cargo test", &outcome);
        assert!(observation.contains("cargo test"));
        assert!(observation.contains("boom"));
        assert!(observation.contains("exit code 2"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
