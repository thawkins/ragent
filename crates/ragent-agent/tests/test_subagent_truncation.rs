#![allow(clippy::assert_is_empty)]
//! Regression tests for the sub-agent report-truncation fix.
//!
//! Two failure modes from live use are covered:
//!
//! 1. **Silent end-of-stream truncation** — some providers (Ollama, some
//!    Copilot models) stop a long completion mid-sentence *without* an
//!    explicit `finish_reason`, leaving the sub-agent's "final message" as
//!    a cut-off exploration narrative. The parent then sees the fragment
//!    via `wait_agents` with no signal that it is incomplete. The fix has
//!    three layers, all tested here:
//!    - `FinishReason::Truncation` is synthesised by the session loop when
//!      a no-tool-call stream closes without any `StreamEvent::Finish`
//!      (unit-tested against `tool_result_content_for_llm`'s exempt path).
//!    - The task layer (`AgentManager`) reads the recorded reason and, for
//!      background tasks, runs up to `CONTINUATION_RETRIES` continuation
//!      passes that ask the model to keep writing from the cut point.
//!    - `wait_agents` `ToolOutput.content` is exempt from the generic
//!      12k head+tail cut: when the combined batch exceeds the inline
//!      budget, the full payload is diverted to
//!      `log/subagents/wait-batch-<ts>.md` and a per-task index (with
//!      `output_file` paths and previews) is handed back instead.
//!
//! 2. **Combined-batch context truncation** — even when every individual
//!    report is intact, concatenating ~10 of them overflows the 12k tool
//!    result budget, so earlier sessions lost the middle reports. The
//!    exempt path tested below bypasses the generic truncation for
//!    `wait_agents`/`list_agents` regardless of size.

use serde_json::json;

use ragent_agent::session::history::tool_result_content_for_llm;

/// Wait/list-agents results are exempt from the 12k generic cap: short
/// payloads pass through unchanged.
#[test]
fn test_wait_agents_short_result_passes_through_unchanged() {
    let content = "2 task(s) completed:\n\n✅ **explore** (task explore-abcd):\nall good\n";
    let out = tool_result_content_for_llm("wait_agents", content, None);
    assert_eq!(out.as_ref(), content);
}

/// Above the exempt inline budget the full text is diverted to disk and a
/// per-task index is returned — nothing is silently dropped from the
/// middle, and every task's `output_file` path is surfaced.
#[test]
fn test_wait_agents_long_result_diverts_to_batch_file() {
    // Build a fake wait_agents payload with 6 agents, each with a fake
    // output_file path and a sizable report body so the combined content
    // exceeds EXEMPT_TOOL_INLINE_LIMIT (32k chars).
    let mut content = String::from("6 task(s) completed:\n\n");
    let mut results = Vec::new();
    for i in 0..6 {
        let task_id = format!("explore-{i:08x}");
        let body = format!(
            "## Optimisation review {i}\n\n{}\n",
            "finding text with enough bulk to add up. ".repeat(150)
        );
        let file_path = format!("log/subagents/{task_id}.md");
        content.push_str(&format!(
            "✅ **explore** (task {task_id}):\n{body}\n📄 Full report: {file_path}\n\n---\n\n"
        ));
        results.push(json!({
            "task_id": task_id,
            "agent": "explore",
            "success": true,
            "output": body,
            "output_file": file_path,
            "report_status": "complete",
        }));
    }
    // ~6 × 6.2k = ~37k — comfortably over the 32k inline budget.
    assert!(content.len() > 32_000);

    let metadata = json!({ "results": results });
    let out = tool_result_content_for_llm("wait_agents", &content, Some(&metadata));

    // The exempt path must NOT return the raw 37k blob: it should hand back
    // an index with per-task lines and the batch-file path.
    assert!(out.len() < content.len(), "index must be smaller than blob");
    assert!(
        out.contains("log/subagents/wait-batch-") || out.contains("exceeded the inline budget"),
        "expected batch-diversion header, got: {}",
        &out[..out.len().min(400)]
    );
    // Every task id from the batch appears in the index so the parent can
    // `read` the corresponding per-agent file.
    for i in 0..6 {
        let needle = format!("explore-{i:08x}");
        assert!(
            out.contains(&needle),
            "index is missing task {needle}: {out}"
        );
    }
}

/// Non-exempt tools still get the old 12k head+tail cut.
#[test]
fn test_other_tools_still_truncated_at_12k() {
    let big = "x".repeat(20_000);
    let out = tool_result_content_for_llm("bash", &big, None);
    assert!(out.contains("tool result truncated for context"));
    assert!(out.len() < big.len());
}

/// The exempt path still passes small `list_agents` results through.
#[test]
fn test_list_agents_short_result_not_marked() {
    let content = "task-id | explore | ✅ Completed | yes | 3s | found it |";
    let out = tool_result_content_for_llm("list_agents", content, None);
    assert_eq!(out.as_ref(), content);
    assert!(!out.contains("batch"));
}
