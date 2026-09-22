//! Build script for ragent-agent.
//!
//! Embeds compile-time build metadata into the binary so the `ragent_info`
//! tool can report the running ragent version, when the binary was built, and
//! (best-effort) the git commit and compiler it was built from.
//!
//! The following environment variables are emitted via `cargo:rustc-env` and
//! consumed in `crates/ragent-agent/src/tool/ragent_info.rs`:
//!
//! - `BUILD_TIME` — always present; captured as `YYYY-MM-DD HH:MM:SS UTC`.
//! - `GIT_COMMIT` / `GIT_COMMIT_SUBJECT` — best-effort; omitted when `git` is
//!   unavailable or the source tree is not a repository.
//! - `RUSTC_VERSION` — best-effort; omitted when the compiler cannot be run.

use std::process::Command;
use std::time::SystemTime;

/// Runs `program` with `args` and returns its trimmed stdout on success.
///
/// Returns `None` when the process cannot be spawned, exits non-zero, or
/// produces empty (or non-UTF-8) output. Build metadata gathered this way is
/// always optional.
fn cmd_output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Emits the build metadata as compile-time environment variables.
fn main() {
    // Re-run only when the build script itself changes; the timestamp is
    // intentionally not a rerun trigger because it would invalidate the build
    // on every invocation (matching ragent-tui and ragent-research).
    println!("cargo:rerun-if-changed=build.rs");

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system time before epoch");
    let build_time = chrono::DateTime::from_timestamp(now.as_secs() as i64, 0)
        .expect("invalid timestamp")
        .format("%Y-%m-%d %H:%M:%S UTC")
        .to_string();
    println!("cargo:rustc-env=BUILD_TIME={build_time}");

    if let Some(commit) = cmd_output("git", &["rev-parse", "--short", "HEAD"]) {
        println!("cargo:rustc-env=GIT_COMMIT={commit}");
        if let Some(subject) = cmd_output("git", &["log", "-1", "--format=%s"]) {
            println!("cargo:rustc-env=GIT_COMMIT_SUBJECT={subject}");
        }
    }

    if let Ok(rustc) = std::env::var("RUSTC") {
        if let Some(version) = cmd_output(&rustc, &["--version"]) {
            println!("cargo:rustc-env=RUSTC_VERSION={version}");
        }
    }
}
