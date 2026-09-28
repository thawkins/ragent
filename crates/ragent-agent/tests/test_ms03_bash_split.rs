//! MS-03 regression tests for the bash permission-splitting hardening.
//!
//! SEC-ragent-tools-core-002 follow-up (SECTASKS T-021): `split_bash_command`
//! decides which sub-commands are permission-checked. A `;` or `|` inside a
//! `$( ... )` command substitution used to split the outer invocation into two
//! separately-judged parts, so the payload inside the substitution could be
//! presented as a harmless extra sub-command.

use ragent_agent::session::permissions::split_bash_command;

#[test]
fn test_split_keeps_command_substitution_together() {
    let parts = split_bash_command("echo $(id; whoami)");
    assert_eq!(
        parts,
        vec!["echo $(id; whoami)".to_string()],
        "a `;` inside `$( ... )` must not split the command"
    );
}

#[test]
fn test_split_still_splits_top_level_separators() {
    let parts = split_bash_command("ls; pwd");
    assert_eq!(parts, vec!["ls".to_string(), "pwd".to_string()]);
}

#[test]
fn test_split_still_splits_pipelines() {
    // A single `|` (pipeline) is deliberately kept in one part by the splitter
    // - only `&&`, `||` and `;` separate sub-commands. The substitution-depth
    // guard must not change that.
    let parts = split_bash_command("cat file | grep needle");
    assert_eq!(parts, vec!["cat file | grep needle".to_string()]);
}

#[test]
fn test_split_still_splits_double_pipe() {
    let parts = split_bash_command("cat file || pwd");
    assert_eq!(parts, vec!["cat file".to_string(), "pwd".to_string()]);
}

#[test]
fn test_split_handles_nested_substitution() {
    let parts = split_bash_command("echo $(echo $(id)) ; pwd");
    assert_eq!(
        parts,
        vec!["echo $(echo $(id))".to_string(), "pwd".to_string()]
    );
}

#[test]
fn test_split_ignores_separator_inside_quotes() {
    let parts = split_bash_command("echo 'a;b'");
    assert_eq!(parts, vec!["echo 'a;b'".to_string()]);
}
