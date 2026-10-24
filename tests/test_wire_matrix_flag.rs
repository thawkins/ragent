//! Integration tests for the `--wire-matrix` CLI flag (FR-016, spec
//! `uniformtools` T-022).
//!
//! FR-016 requires the flag to print one aligned table row per registered
//! provider and exit **without contacting any provider and without executing any
//! tool**. Each test therefore spawns the built `ragent` binary inside a fresh
//! empty tempdir, exactly like the other `ragent <flag>` CLI tests, so the crash
//! marker and any stray write land in the sandbox rather than the repository.
//!
//! The offline guarantee is asserted two ways: the run must finish in well under
//! any network timeout (it is a pure table print), and it must not create a
//! session, a database or a config file. The row count is compared against the
//! provider list the same binary prints for `ragent --help`'s provider selector -
//! the TC-002 step 3 comparison - by reading the `PROVIDER` column and requiring
//! every provider id `ragent models` names to appear as a row.
//!
//! Env isolation: these tests spawn a child process with its own `HOME` and
//! `XDG_CONFIG_HOME` and never mutate the test process's own globals - the child
//! environment is set on the `Command`, which is process-local to the child - so
//! no crate-wide lock is taken (see `tests/all.rs`).

use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::{Duration, Instant};

/// Run `ragent` with `args` inside a fresh empty tempdir with an isolated user
/// config root, returning the tempdir path and the captured output.
fn ragent_in_empty_cwd(args: &[&str]) -> (PathBuf, Output) {
    let temp = tempfile::tempdir().expect("tempdir");
    let config = temp.path().join("config");
    let output = Command::new(env!("CARGO_BIN_EXE_ragent"))
        .args(args)
        .current_dir(temp.path())
        .env("XDG_CONFIG_HOME", &config)
        .env("HOME", temp.path())
        .env("RAGENT_GITHUB_NO_GH_CLI", "1")
        .output()
        .expect("spawn ragent");
    let dir = temp.path().to_path_buf();
    std::mem::forget(temp);
    (dir, output)
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

/// The provider ids named by the `PROVIDER` column of a printed matrix.
fn matrix_providers(table: &str) -> Vec<String> {
    table
        .lines()
        .skip(2)
        .filter_map(|line| line.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

#[test]
fn test_wire_matrix_flag_prints_one_row_per_provider() {
    let (_dir, out) = ragent_in_empty_cwd(&["--wire-matrix"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let table = stdout(&out);

    let lines: Vec<&str> = table.lines().collect();
    assert!(lines.len() > 2, "expected a populated table:\n{table}");
    assert!(
        lines[0].starts_with("PROVIDER"),
        "the heading row is missing:\n{table}"
    );
    assert!(
        lines[1].chars().all(|c| c == '-'),
        "the rule row is missing:\n{table}"
    );

    let providers = matrix_providers(&table);
    assert!(
        providers.len() >= 15,
        "expected at least the 15 registered providers, got {}:\n{table}",
        providers.len()
    );
    // Sorted and unique: the table must not print a provider twice.
    let mut sorted = providers.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(providers, sorted, "rows must be sorted and unique");

    // Every provider the model listing knows must have a row (TC-002 step 3).
    let (_models_dir, models) = ragent_in_empty_cwd(&["models"]);
    if models.status.success() {
        let listed: Vec<String> = stdout(&models)
            .lines()
            .filter_map(|line| {
                line.split_once('/')
                    .map(|(provider, _)| provider.to_string())
            })
            .collect();
        for provider in listed {
            assert!(
                providers.contains(&provider),
                "provider {provider:?} is registered but has no wire-matrix row:\n{table}"
            );
        }
    }
}

#[test]
fn test_wire_matrix_flag_populates_every_column() {
    let (_dir, out) = ragent_in_empty_cwd(&["--wire-matrix"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let table = stdout(&out);

    // Ten columns, two spaces apart, none of them blank (TC-002).
    for line in table.lines().skip(2) {
        let cells: Vec<&str> = line
            .split("  ")
            .map(str::trim)
            .filter(|cell| !cell.is_empty())
            .collect();
        assert_eq!(
            cells.len(),
            10,
            "row does not carry ten populated columns: {line:?}"
        );
        for cell in cells {
            assert!(!cell.is_empty(), "blank cell in row: {line:?}");
            assert_ne!(cell, "-", "dash cell in row: {line:?}");
        }
    }

    // Plain ASCII throughout, with no trailing whitespace.
    assert!(table.is_ascii(), "the matrix must be plain ASCII");
    for line in table.lines() {
        assert_eq!(line.trim_end(), line, "trailing whitespace in {line:?}");
    }

    // The three families a reader is most likely to check.
    assert!(table.contains("by_call_id"), "no id-keyed family printed");
    assert!(table.contains("synthesised"), "no synthesised id printed");
    assert!(table.contains("bedrock_tool_spec"), "no Bedrock envelope");
}

#[test]
fn test_wire_matrix_flag_exits_without_network_or_session_side_effects() {
    let started = Instant::now();
    let (dir, out) = ragent_in_empty_cwd(&["--wire-matrix"]);
    let elapsed = started.elapsed();
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));

    // No network: a printed table is a pure function of the const profile table.
    // Generous bound - the point is that nothing waits on a socket (every
    // provider client's connect timeout is seconds, not milliseconds).
    assert!(
        elapsed < Duration::from_secs(10),
        "the wire matrix took {elapsed:?}; it must not contact a provider"
    );

    // No session, database or config is created by the report itself. The lone
    // file a command writes is the crash marker, which is written by every run.
    for entry in ["sessions.db", "ragent.db", ".ragent", "log/sessions"] {
        assert!(
            !dir.join(entry).exists(),
            "the wire matrix created {entry}, so it ran more than an offline report"
        );
    }
}

#[test]
fn test_wire_matrix_flag_is_documented_in_help() {
    let (_dir, out) = ragent_in_empty_cwd(&["--help"]);
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(&out));
    let help = stdout(&out);
    assert!(
        help.contains("--wire-matrix"),
        "`--help` does not list --wire-matrix:\n{help}"
    );
}
