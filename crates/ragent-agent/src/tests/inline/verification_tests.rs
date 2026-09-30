//! Inline tests for `verification.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

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
