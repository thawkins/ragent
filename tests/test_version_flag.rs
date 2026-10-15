//! Integration tests for the `--version` / `-V` CLI flag.
//!
//! `ragent --version` and its short form `ragent -V` must print the version and
//! exit 0 *without* starting the TUI and *without* leaving a crash marker the
//! next start would report as an unclean exit. Each test spawns the built
//! `ragent` binary inside a fresh empty tempdir so the crash marker (written
//! relative to the session working directory) is observed in isolation.

use std::process::{Command, Output};

/// Run `ragent` with `args` inside a fresh empty tempdir and return the tempdir
/// path plus the captured output.
fn ragent_in_empty_cwd(args: &[&str]) -> (std::path::PathBuf, Output) {
    let temp = tempfile::tempdir().expect("tempdir");
    let output = Command::new(env!("CARGO_BIN_EXE_ragent"))
        .args(args)
        .current_dir(temp.path())
        .output()
        .expect("spawn ragent");
    let dir = temp.path().to_path_buf();
    // Keep the tempdir alive until after assertions by leaking it; the test
    // process is short-lived and the tree is small.
    std::mem::forget(temp);
    (dir, output)
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// `ragent --version` prints `ragent <version>` on stdout and exits 0.
#[test]
fn long_version_prints_version_and_exits_zero() {
    let (_dir, out) = ragent_in_empty_cwd(&["--version"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert_eq!(
        stdout(&out).trim(),
        format!("ragent {}", env!("CARGO_PKG_VERSION"))
    );
}

/// The short form `ragent -V` behaves identically.
#[test]
fn short_version_prints_version_and_exits_zero() {
    let (_dir, out) = ragent_in_empty_cwd(&["-V"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    assert_eq!(
        stdout(&out).trim(),
        format!("ragent {}", env!("CARGO_PKG_VERSION"))
    );
}

/// A version query must not leave the impression of an unclean exit. Like
/// `--help`, the process writes a `clean exit` crash marker on its normal
/// return; that marker must never be `running`, which the next start would
/// report as a phantom crash.
#[test]
fn version_flag_leaves_no_unclean_marker() {
    let (dir, out) = ragent_in_empty_cwd(&["-V"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let marker = dir.join("log").join("panics").join("last-crash.json");
    if marker.exists() {
        let text = std::fs::read_to_string(&marker).expect("read marker");
        assert!(
            !text.contains("\"status\": \"running\""),
            "version query left an unclean marker: {text}"
        );
    }
}

/// A second version query must not warn about a phantom previous crash.
#[test]
fn version_flag_does_not_warn_about_previous_crash() {
    let (dir, _first) = ragent_in_empty_cwd(&["-V"]);
    let second = Command::new(env!("CARGO_BIN_EXE_ragent"))
        .args(["--version"])
        .current_dir(&dir)
        .output()
        .expect("spawn ragent");
    assert_eq!(second.status.code(), Some(0));
    let err = stderr(&second);
    assert!(
        !err.contains("previous ragent session"),
        "unexpected phantom crash warning: {err}"
    );
}

/// The version flag is documented in `--help` as a top-level option.
#[test]
fn help_lists_version_flag() {
    let (_dir, out) = ragent_in_empty_cwd(&["--help"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let help = stdout(&out);
    assert!(
        help.contains("--version"),
        "help did not list --version:\n{help}"
    );
}
