//! Second batch of `message_widget` summary coverage: the arms the first
//! `test_message_widget_summaries.rs` pass left behind (the GitLab / GitHub
//! repository family, the masterfetch trio, and the tool/info family).
//!
//! Tests only - no production logic is touched.

use ragent_tui::widgets::message_widget::{tool_input_summary, tool_result_summary};
use serde_json::json;

/// The working directory every summary is rendered against.
const CWD: &str = "/proj";

// ---------------------------------------------------------------------------
// tool_input_summary arms
// ---------------------------------------------------------------------------

#[test]
fn input_run_code_alias_uses_the_bash_summary() {
    // `run_code` is canonicalised to `bash`, so it renders as `$ <cmd>`.
    let out = tool_input_summary("run_code", &json!({"code": "echo hi"}), CWD);
    assert!(out.starts_with("$ "), "got {out}");
    assert!(out.contains("echo hi"), "got {out}");
}

#[test]
fn input_gitlab_repository_arms() {
    let cases = [
        (
            "gitlab_get_issue",
            json!({"project": "g/p", "iid": 3}),
            "issue #3",
        ),
        ("gitlab_list_mrs", json!({"project": "g/p"}), "MRs"),
        (
            "gitlab_get_pipeline",
            json!({"project": "g/p", "pipeline_id": 12}),
            "pipeline #12",
        ),
        (
            "gitlab_list_jobs",
            json!({"project": "g/p", "pipeline_id": 7}),
            "jobs for pipeline #7",
        ),
        (
            "gitlab_list_pipelines",
            json!({"project": "g/p", "status": "success"}),
            "pipelines (success)",
        ),
    ];
    for (tool, input, expected) in cases {
        let out = tool_input_summary(tool, &input, CWD);
        assert!(!out.is_empty(), "{tool} produced nothing");
        assert!(out.contains(expected), "{tool} -> {out}");
    }
}

#[test]
fn input_gitlab_create_and_comment_arms() {
    for tool in [
        "gitlab_create_issue",
        "gitlab_comment_issue",
        "gitlab_close_issue",
        "gitlab_retry_job",
        "gitlab_retry_pipeline",
        "gitlab_get_job_log",
    ] {
        let out = tool_input_summary(
            tool,
            &json!({"project": "g/p", "iid": 4, "job_id": 9, "pipeline_id": 2, "body": "hello"}),
            CWD,
        );
        assert!(!out.is_empty(), "{tool} produced nothing");
    }
}

#[test]
fn input_github_list_prs_arm() {
    let out = tool_input_summary("github_list_prs", &json!({"state": "open"}), CWD);
    assert!(!out.is_empty(), "github_list_prs produced nothing");
}

// ---------------------------------------------------------------------------
// tool_result_summary arms
// ---------------------------------------------------------------------------

#[test]
fn result_network_and_masterfetch_arms() {
    let cases = [
        (
            "mf_fetch",
            json!({"url": "https://example.invalid/x", "status": 200}),
            "200",
        ),
        ("mf_crawl", json!({"pages_fetched": 3}), "page"),
        ("mf_cache_clear", json!({"purged": 2}), "purged"),
    ];
    for (tool, out, expected) in cases {
        let summary = tool_result_summary(tool, &Some(out), &json!({}), CWD)
            .unwrap_or_else(|| panic!("{tool} produced no summary"));
        assert!(
            summary.to_lowercase().contains(expected),
            "{tool} -> {summary}"
        );
    }
}

#[test]
fn result_tool_and_info_arms() {
    for tool in [
        "model_info",
        "ragent_info",
        "codeindex_communities",
        "open",
        "conversation_search",
        "session_search",
        "skill_manage",
        "initiative",
        "apply_patch",
    ] {
        let summary = tool_result_summary(
            tool,
            &Some(json!({"lines": 1, "count": 2, "summary": "done"})),
            &json!({"skill": "graphify", "action": "list"}),
            CWD,
        );
        assert!(
            summary.is_some(),
            "{tool} must produce a summary for a known arm"
        );
    }
}

#[test]
fn result_absent_output_returns_none() {
    // `None` is returned when the tool produced no result object at all.
    assert!(
        tool_result_summary("read", &None, &json!({}), CWD).is_none(),
        "an absent result must return None"
    );
}
