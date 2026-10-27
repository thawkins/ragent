//! message_widget tool_input_summary / tool_result_summary match arms.
//!
//! Table-driven coverage for the remaining arms of the two public pure
//! summarisers in `ragent_tui::widgets::message_widget`, plus the four small
//! display helpers they share (`truncate_str`, `pluralize`,
//! `capitalize_tool_name`, `canonical_tool_name`, `make_relative_path`).
//!
//! Every expectation here was derived from the source's own format strings.
//! Non-ASCII glyphs the source emits (`->`, `<-`, `<->`, `...`) are written as
//! `\u{..}` escapes so this file stays ASCII.
//!
//! Tests only - no production logic is touched.

use ragent_tui::widgets::message_widget::{
    canonical_tool_name, capitalize_tool_name, make_relative_path, pluralize, tool_input_summary,
    tool_result_summary, truncate_str,
};
use serde_json::json;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Working directory used by every summary call in this file.
const CWD: &str = "/proj";

/// Rightwards arrow the source emits for paths and transitions.
const ARROW: &str = "\u{2192}";
/// Leftwards arrow the source emits for `ExitPlanMode`.
const LEFT_ARROW: &str = "\u{2190}";
/// Left-right arrow the source emits for `diff_files`.
const BOTH_ARROW: &str = "\u{2194}";
/// Ellipsis the source appends when truncating.
const ELLIPSIS: &str = "\u{2026}";

/// `tool_input_summary` bound to [`CWD`].
fn input_summary(tool: &str, args: serde_json::Value) -> String {
    tool_input_summary(tool, &args, CWD)
}

/// `tool_result_summary` bound to [`CWD`], panicking on `None`.
fn result_summary(tool: &str, out: serde_json::Value, args: serde_json::Value) -> String {
    tool_result_summary(tool, &Some(out), &args, CWD).unwrap_or_else(|| format!("<none:{tool}>"))
}

/// Assert a table of `(tool, args)` pairs each produce the expected summary.
fn assert_input_table(cases: &[(&str, serde_json::Value, &str)]) {
    for (tool, args, expected) in cases {
        assert_eq!(
            input_summary(tool, args.clone()),
            *expected,
            "input summary mismatch for {tool} with {args}"
        );
    }
}

// ---------------------------------------------------------------------------
// Shared display helpers
// ---------------------------------------------------------------------------

#[test]
fn test_summary_helpers() {
    assert_eq!(pluralize(0, "line", "lines"), "0 lines");
    assert_eq!(pluralize(1, "line", "lines"), "1 line");
    assert_eq!(pluralize(2, "file", "files"), "2 files");

    assert_eq!(truncate_str("abcdef", 3), "abc...");
    assert_eq!(truncate_str("abc", 3), "abc");
    assert_eq!(truncate_str("ab", 5), "ab");
    assert_eq!(truncate_str("", 4), "");

    assert_eq!(capitalize_tool_name("read"), "Read");
    assert_eq!(capitalize_tool_name("mcp_server_tool"), "Mcp_server_tool");
    assert_eq!(capitalize_tool_name(""), "");

    assert_eq!(canonical_tool_name("update_file"), "Write");
    assert_eq!(canonical_tool_name("run_code"), "Bash");
    assert_eq!(canonical_tool_name("read"), "read");

    assert_eq!(make_relative_path("/proj/src/a.rs", "/proj"), "src/a.rs");
    assert_eq!(make_relative_path("/proj/src/a.rs", "/proj/"), "src/a.rs");
    assert_eq!(make_relative_path("/other/a.rs", "/proj"), "/other/a.rs");
    assert_eq!(
        make_relative_path("relative/a.rs", "/proj"),
        "relative/a.rs"
    );
}

// ---------------------------------------------------------------------------
// tool_input_summary
// ---------------------------------------------------------------------------

#[test]
fn test_input_git_family() {
    assert_input_table(&[
        ("git_remote", json!({}), "remote -v"),
        (
            "git_remote",
            json!({"action": "add", "name": "upstream"}),
            "remote add upstream",
        ),
        (
            "git_remote",
            json!({"action": "remove", "name": "origin"}),
            "remote remove origin",
        ),
        (
            "git_remote",
            json!({"action": "set-url", "name": "origin"}),
            "remote set-url origin",
        ),
        (
            "git_remote",
            json!({"action": "prune", "name": "origin"}),
            "remote prune origin",
        ),
        ("git_remote", json!({"action": "prune"}), "remote prune"),
        ("git_pull", json!({}), "pull origin"),
        (
            "git_pull",
            json!({"remote": "upstream", "rebase": true}),
            "pull --rebase upstream",
        ),
        ("git_reset", json!({}), "reset --mixed"),
        (
            "git_reset",
            json!({"mode": "hard", "target": "HEAD~1"}),
            "reset --hard HEAD~1",
        ),
        ("git_reset", json!({"paths": ["a", "b"]}), "reset 2 path(s)"),
        ("git_reset", json!({"paths": []}), "reset --mixed"),
        ("git_fetch", json!({}), "fetch origin"),
        ("git_fetch", json!({"remote": "upstream"}), "fetch upstream"),
        ("git_fetch", json!({"all": true}), "fetch --all"),
        (
            "git_cherry_pick",
            json!({"commits": ["a", "b", "c"]}),
            "cherry-pick 3 commit(s)",
        ),
        ("git_cherry_pick", json!({}), "cherry-pick 0 commit(s)"),
        ("git_add", json!({"paths": ["a"]}), "add 1 path(s)"),
        ("git_add", json!({}), "add 0 path(s)"),
        ("git_tag", json!({}), "tag -l"),
        (
            "git_tag",
            json!({"action": "create", "name": "v1"}),
            "tag v1",
        ),
        (
            "git_tag",
            json!({"action": "delete", "name": "v1"}),
            "tag -d v1",
        ),
        (
            "git_tag",
            json!({"action": "show", "name": "v1"}),
            "tag show v1",
        ),
        ("git_tag", json!({"action": "show"}), "tag show"),
        (
            "git_push",
            json!({"tags": true, "remote": "upstream"}),
            "push --tags upstream",
        ),
        (
            "git_push",
            json!({"force": true, "branch": "main"}),
            "push --force-with-lease origin/main",
        ),
        (
            "git_push",
            json!({"force": true}),
            "push --force-with-lease origin",
        ),
        ("git_push", json!({"branch": "dev"}), "push origin/dev"),
        ("git_push", json!({}), "push origin"),
        ("git_status", json!({}), "status"),
        ("git_diff", json!({}), "diff working"),
        (
            "git_diff",
            json!({"stat": true, "target": "staged"}),
            "diff --stat staged",
        ),
        ("git_commit", json!({"amend": true}), "commit --amend"),
        ("git_log", json!({}), "log (20)"),
        (
            "git_log",
            json!({"limit": 5, "branch": "main"}),
            "log main (5)",
        ),
        ("git_merge", json!({"branch": "feature"}), "merge feature"),
        ("git_merge", json!({}), "merge ?"),
        ("git_checkout", json!({}), "checkout"),
        (
            "git_checkout",
            json!({"paths": ["a", "b"]}),
            "checkout 2 path(s)",
        ),
        ("git_branch", json!({}), "branch list (all)"),
        ("git_branch", json!({"all": false}), "branch list (local)"),
    ]);
}

#[test]
fn test_input_codeindex_symbols() {
    assert_eq!(
        input_summary(
            "codeindex_symbols",
            json!({"name": "foo", "kind": "function", "file_path": "/proj/src/a.rs"})
        ),
        "name:foo kind:function file:src/a.rs"
    );
    assert_eq!(input_summary("codeindex_symbols", json!({})), "all symbols");
    assert_eq!(
        input_summary("codeindex_symbols", json!({"kind": "struct"})),
        "kind:struct"
    );
}

#[test]
fn test_input_team_family() {
    assert_input_table(&[
        (
            "team_wait",
            json!({"agent_ids": ["a", "b"]}),
            "wait on 2 agent(s)",
        ),
        ("team_wait", json!({}), "wait on all agents"),
        (
            "team_approve_plan",
            json!({"approved": true}),
            "approve plan",
        ),
        (
            "team_approve_plan",
            json!({"approved": false}),
            "reject plan",
        ),
        ("team_approve_plan", json!({}), "reject plan"),
        (
            "team_shutdown_teammate",
            json!({"teammate": "tm-1"}),
            "shutdown tm-1",
        ),
        ("team_shutdown_teammate", json!({}), "shutdown ?"),
        ("team_task_claim", json!({"task_id": "t-1"}), "claim t-1"),
        ("team_task_claim", json!({}), "claim next"),
        ("team_task_create", json!({"title": "do it"}), "task: do it"),
        (
            "team_task_complete",
            json!({"task_id": "t-1"}),
            "complete t-1",
        ),
        ("team_task_complete", json!({}), "complete ?"),
        (
            "team_assign_task",
            json!({"task_id": "t-1", "to": "bob"}),
            "assign t-1 to bob",
        ),
        ("team_assign_task", json!({}), "assign ? to ?"),
        (
            "team_message",
            json!({"to": "bob", "content": "hi"}),
            "msg to bob: hi",
        ),
        ("team_broadcast", json!({"content": "hi"}), "broadcast: hi"),
        ("team_broadcast", json!({}), "broadcast"),
    ]);
}

#[test]
fn test_input_path_family() {
    // Plain-ASCII expectations that carry no arrow glyph.
    assert_input_table(&[
        ("search", json!({"query": "foo"}), "\"foo\""),
        (
            "search",
            json!({"query": "foo", "path": "/proj/src"}),
            "\"foo\" in src",
        ),
        ("move_file", json!({"source": "/proj/a"}), "a"),
        (
            "apply_patch",
            json!({"path": "/proj/change.patch"}),
            "/proj/change.patch",
        ),
        ("apply_patch", json!({}), "apply patch"),
    ]);

    // Move / rename / copy render as "<src> <arrow> <dst>".
    for tool in ["move_file", "rename_file", "copy_file"] {
        assert_eq!(
            input_summary(tool, json!({"source": "/proj/a", "destination": "/proj/b"})),
            format!("a {ARROW} b"),
            "{tool} with explicit source/destination"
        );
    }
    // `rename_file` accepts the legacy `from`/`to` field names.
    assert_eq!(
        input_summary("rename_file", json!({"from": "/proj/a", "to": "/proj/b"})),
        format!("a {ARROW} b")
    );
    // `copy_file` always emits both sides, even when the destination is absent.
    assert_eq!(
        input_summary("copy_file", json!({"source": "/proj/a"})),
        format!("a {ARROW} ")
    );

    // `diff_files` uses the left-right arrow between the two paths.
    assert_eq!(
        input_summary(
            "diff_files",
            json!({"path_a": "/proj/a", "path_b": "/proj/b"})
        ),
        format!("a {BOTH_ARROW} b")
    );
}

#[test]
fn test_input_lsp_family() {
    for tool in ["lsp_hover", "lsp_definition", "lsp_references"] {
        assert_eq!(
            input_summary(
                tool,
                json!({"path": "/proj/src/lib.rs", "line": 3, "column": 5})
            ),
            "src/lib.rs L3:5",
            "{tool} with a path"
        );
        assert_eq!(input_summary(tool, json!({})), "L0:0", "{tool} bare");
    }
}

#[test]
fn test_input_network_env_family() {
    assert_input_table(&[
        (
            "http_request",
            json!({"method": "POST", "url": "https://example.org/x"}),
            "POST https://example.org/x",
        ),
        (
            "http_request",
            json!({"url": "https://example.org/x"}),
            "GET https://example.org/x",
        ),
        (
            "web_request",
            json!({"method": "DELETE", "url": "https://example.org/y"}),
            "DELETE https://example.org/y",
        ),
        ("get_env", json!({"key": "PATH"}), "PATH"),
        ("get_env", json!({"name": "HOME"}), "HOME"),
        ("get_env", json!({}), "all vars"),
        ("calculator", json!({"expression": "1+1"}), "1+1"),
    ]);
}

#[test]
fn test_input_agent_plan_question_family() {
    assert_input_table(&[
        ("question", json!({"question": "Continue?"}), "Continue?"),
        (
            "AskUserQuestion",
            json!({"query": "Which one?"}),
            "Which one?",
        ),
        (
            "agent_complete",
            json!({"summary": "first line\nsecond line"}),
            "first line",
        ),
        ("spec_list", json!({}), "list all"),
    ]);

    // Arrow-prefixed renderings for the agent / plan tools.
    assert_eq!(
        input_summary("new_agent", json!({"agent": "explore", "task": "find it"})),
        format!("explore {ARROW} find it")
    );
    assert_eq!(
        input_summary("EnterPlanMode", json!({"task": "build it"})),
        format!("{ARROW} build it")
    );
    assert_eq!(
        input_summary("ExitPlanMode", json!({"summary": "done"})),
        format!("{LEFT_ARROW} done")
    );
    assert_eq!(
        input_summary(
            "spec_task_update",
            json!({"spec_id": "auth", "task_id": "T-001", "status": "completed"})
        ),
        format!("auth T-001 {ARROW} completed")
    );
}

#[test]
fn test_input_memory_initiative_family() {
    assert_input_table(&[
        ("memory_store", json!({"category": "fact"}), "store fact"),
        ("memory_forget", json!({}), "forget"),
        ("memory_forget", json!({"id": "3"}), "forget 3"),
        (
            "initiative",
            json!({"action": "create", "title": "Goal"}),
            "create \"Goal\"",
        ),
        (
            "initiative",
            json!({"action": "update", "id": "g1"}),
            "update g1",
        ),
        (
            "initiative",
            json!({"action": "read", "id": "g1"}),
            "read g1",
        ),
        (
            "initiative",
            json!({"action": "close", "id": "g1"}),
            "close g1",
        ),
        (
            "initiative",
            json!({"action": "checkpoint", "id": "g1"}),
            "checkpoint g1",
        ),
        ("initiative", json!({"action": "list"}), "list"),
    ]);
}

#[test]
fn test_input_bash_keeps_raw_arguments_when_fields_are_unknown() {
    // A bash command with a recognised field name renders as a shell prompt.
    assert_eq!(
        input_summary("Bash", json!({"command": "ls -la"})),
        format!("$ ls -la")
    );
    assert_eq!(
        input_summary("Bash", json!({"command": "a\nb"})),
        format!("$ a ; b")
    );
    // An alias resolves to bash before the match.
    assert_eq!(
        input_summary("run_code", json!({"code": "echo hi"})),
        "$ echo hi"
    );
    // Unrecognised field names fall back to a compact raw-args rendering.
    assert_eq!(
        input_summary("read", json!({"whatever": "x"})),
        "whatever=\"x\""
    );
    assert_eq!(input_summary("Write", json!({"p": 1})), "p=1");
    assert_eq!(
        input_summary("Edit", json!({"file": "a.rs"})),
        "file=\"a.rs\""
    );
    // An empty input object yields an empty summary.
    assert_eq!(input_summary("read", json!({})), "");
    // `update_file` is an alias of `write`, so the same fallback applies.
    assert_eq!(
        input_summary("update_file", json!({"other": true})),
        "other=true"
    );
}

// ---------------------------------------------------------------------------
// tool_result_summary
// ---------------------------------------------------------------------------

#[test]
fn test_result_file_family() {
    assert_eq!(
        result_summary("read", json!({"lines": 1}), json!({})),
        "1 line read"
    );
    assert_eq!(
        result_summary(
            "read",
            json!({"lines": 3, "summarised": true, "total_lines": 100}),
            json!({})
        ),
        "3 lines read (summarised, 100 total)"
    );
    assert_eq!(
        result_summary("Write", json!({"lines": 5}), json!({"path": "/proj/a.rs"})),
        "5 lines written to a.rs"
    );
    assert_eq!(
        result_summary("create", json!({"lines": 1}), json!({"path": "/proj/a.rs"})),
        "1 line created in a.rs"
    );
    assert_eq!(
        result_summary(
            "Edit",
            json!({"path": "/proj/a.rs", "old_lines": 2, "new_lines": 3}),
            json!({})
        ),
        format!("Edited a.rs: 2 {ARROW} 3 lines")
    );
    assert_eq!(
        result_summary("multiedit", json!({"edits": 2, "file_count": 1}), json!({})),
        "2 edits across 1 file"
    );
    assert_eq!(
        result_summary("patch", json!({"hunks": 3, "files": 2}), json!({})),
        "3 hunks applied across 2 files"
    );
    assert_eq!(
        result_summary("rm", json!({"deleted": true}), json!({})),
        "deleted"
    );
    assert_eq!(result_summary("rm", json!({}), json!({})), "failed");
    assert_eq!(
        result_summary("append_to_file", json!({"bytes": 12}), json!({})),
        "12 bytes appended"
    );
    assert_eq!(
        result_summary("append_to_file", json!({}), json!({})),
        "appended"
    );
    assert_eq!(
        result_summary("diff_files", json!({"changes": 1}), json!({})),
        "1 change found"
    );
    assert_eq!(
        result_summary("copy_file", json!({"bytes": 99}), json!({})),
        "99 bytes copied"
    );
}

#[test]
fn test_result_directory_and_file_info_family() {
    assert_eq!(
        result_summary("list", json!({"count": 2, "path": "/proj/src"}), json!({})),
        "2 entries in src"
    );
    assert_eq!(
        result_summary("make_directory", json!({}), json!({})),
        "directory created"
    );
    assert_eq!(
        result_summary(
            "file_info",
            json!({"kind": "file", "size": 2_097_152}),
            json!({})
        ),
        "file (2.0 MiB)"
    );
    assert_eq!(
        result_summary(
            "file_info",
            json!({"kind": "file", "size": 2048}),
            json!({})
        ),
        "file (2.0 KiB)"
    );
    assert_eq!(
        result_summary("file_info", json!({"kind": "file", "size": 10}), json!({})),
        "file (10 bytes)"
    );
    assert_eq!(
        result_summary("file_info", json!({"kind": "dir"}), json!({})),
        "dir"
    );
}

#[test]
fn test_result_search_family() {
    assert_eq!(
        result_summary("search", json!({"count": 3}), json!({})),
        "3 found"
    );
    assert_eq!(
        result_summary("search", json!({"count": 3, "truncated": true}), json!({})),
        "3+ found"
    );
    assert_eq!(
        result_summary("Grep", json!({"count": 4, "file_count": 2}), json!({})),
        "4 matched in 2 files searched"
    );
    assert_eq!(
        result_summary(
            "Grep",
            json!({"count": 4, "file_count": 1, "truncated": true}),
            json!({})
        ),
        "4+ matched in 1 file searched"
    );
    assert_eq!(
        result_summary("Glob", json!({"count": 1}), json!({})),
        "1 file found"
    );
    assert_eq!(
        result_summary("Glob", json!({"count": 5, "truncated": true}), json!({})),
        "5 files+ found"
    );
}

#[test]
fn test_result_bash_family() {
    assert_eq!(
        result_summary(
            "Bash",
            json!({"timed_out": true, "duration_ms": 1500}),
            json!({})
        ),
        "timed out after 1500ms"
    );
    assert_eq!(
        result_summary("Bash", json!({"lines": 2, "exit_code": 0}), json!({})),
        format!("2 lines{ELLIPSIS} (exit 0)")
    );
    assert_eq!(
        result_summary("Bash", json!({"lines": 1}), json!({})),
        format!("1 line{ELLIPSIS}")
    );
}

#[test]
fn test_result_network_family() {
    assert_eq!(
        result_summary("calculator", json!({"result": "42"}), json!({})),
        "= 42"
    );
    assert_eq!(
        result_summary(
            "webfetch",
            json!({"lines": 10, "http_status": 200}),
            json!({})
        ),
        "10 lines (HTTP 200)"
    );
    assert_eq!(
        result_summary("websearch", json!({"count": 2}), json!({})),
        "2 results found"
    );
    // `mf_search` prefixes its count with a globe glyph, so match the suffix.
    let mf = result_summary("mf_search", json!({"total_results": 1}), json!({}));
    assert!(mf.ends_with("1 result found"), "got: {mf}");
    assert_eq!(
        result_summary(
            "http_request",
            json!({"http_status": 404, "lines": 2}),
            json!({})
        ),
        "HTTP 404 (2 lines)"
    );
    assert_eq!(
        result_summary("get_env", json!({"count": 1}), json!({})),
        "1 var"
    );
    assert_eq!(result_summary("get_env", json!({}), json!({})), "not found");
}

#[test]
fn test_result_interaction_family() {
    assert_eq!(
        result_summary("question", json!({"response": "yes"}), json!({})),
        "yes"
    );
    assert_eq!(
        result_summary(
            "AskUserQuestion",
            json!({"content": "picked two"}),
            json!({})
        ),
        "picked two"
    );
    assert_eq!(
        result_summary("think", json!({}), json!({})),
        "Thinking ..."
    );
    assert_eq!(
        result_summary("think", json!({"thought": "hmm"}), json!({})),
        "hmm"
    );
    // The thought is read from the tool input when the output omits it.
    assert_eq!(
        result_summary("think", json!({}), json!({"thought": "from input"})),
        "from input"
    );
    assert_eq!(
        result_summary("agent_complete", json!({}), json!({})),
        "Task complete"
    );
    assert_eq!(
        result_summary("agent_complete", json!({"summary": "all done"}), json!({})),
        "all done"
    );
}

#[test]
fn test_result_none_without_output() {
    assert!(tool_result_summary("read", &None, &json!({}), CWD).is_none());
    assert!(tool_result_summary("anything", &None, &json!({}), CWD).is_none());
}
