//! Coverage-focused tests for [`ragent_server::sse::event_to_parts`].
//!
//! The primary per-variant tests live in `test_event_to_sse.rs`; this file
//! exercises the remaining [`Event`] variants (and the secret-redaction paths)
//! so the serialisation core is covered end to end. Real assertions are made on
//! the SSE event-type name and the parsed JSON payload.

use ragent_agent::event::Event;
use ragent_server::sse::{event_to_parts, redacted_event_debug};
use ragent_types::event::ToolCallBatchEntry;
use serde_json::Value;

/// Parse the JSON data string from `event_to_parts` into a `serde_json::Value`.
fn parts(event: &Event) -> (&'static str, Value) {
    let (name, json) = event_to_parts(event);
    let value: Value = serde_json::from_str(&json).expect("payload must be valid JSON");
    (name, value)
}

#[test]
fn test_agent_notice() {
    let (name, v) = parts(&Event::AgentNotice {
        session_id: "s1".into(),
        message: "heads up".into(),
    });
    assert_eq!(name, "agent_notice");
    assert_eq!(v["message"], "heads up");
}

#[test]
fn test_task_completed() {
    let (name, v) = parts(&Event::TaskCompleted {
        session_id: "s1".into(),
        summary: "done".into(),
    });
    assert_eq!(name, "task_completed");
    assert_eq!(v["summary"], "done");
}

#[test]
fn test_service_start_error() {
    let (name, v) = parts(&Event::ServiceStartError {
        session_id: "s1".into(),
        service: "ollama".into(),
        command_path: "/usr/bin/ollama".into(),
        stdout: "out".into(),
        stderr: "err".into(),
        error: "failed".into(),
    });
    assert_eq!(name, "service_start_error");
    assert_eq!(v["service"], "ollama");
    assert_eq!(v["command_path"], "/usr/bin/ollama");
    assert_eq!(v["stderr"], "err");
}

#[test]
fn test_compression_started() {
    let (name, v) = parts(&Event::CompressionStarted {
        session_id: "s1".into(),
        reason: "threshold".into(),
    });
    assert_eq!(name, "compression_started");
    assert_eq!(v["reason"], "threshold");
}

#[test]
fn test_compression_finished() {
    let (name, v) = parts(&Event::CompressionFinished {
        session_id: "s1".into(),
        original_tokens: 1000,
        compressed_tokens: 400,
        compression_ratio: 2.5,
        did_compress: true,
        reason: "auto".into(),
    });
    assert_eq!(name, "compression_finished");
    assert_eq!(v["original_tokens"], 1000);
    assert_eq!(v["compressed_tokens"], 400);
    assert_eq!(v["did_compress"], true);
}

#[test]
fn test_provider_loading_started() {
    let (name, v) = parts(&Event::ProviderLoadingStarted {
        provider_id: "openai".into(),
        provider_name: "OpenAI".into(),
    });
    assert_eq!(name, "provider_loading_started");
    assert_eq!(v["provider_id"], "openai");
}

#[test]
fn test_provider_loading_finished() {
    let (name, v) = parts(&Event::ProviderLoadingFinished {
        provider_id: "openai".into(),
        provider_name: "OpenAI".into(),
        models: vec![serde_json::json!({"id": "gpt-4o"})],
        error: None,
    });
    assert_eq!(name, "provider_loading_finished");
    assert_eq!(v["models"][0]["id"], "gpt-4o");
}

#[test]
fn test_router_classification() {
    let (name, v) = parts(&Event::RouterClassification {
        session_id: "s1".into(),
        tier: "high".into(),
        requested_tier: Some("low".into()),
        model: "anthropic:claude".into(),
        composite_score: 0.81,
        prompt: "hello".into(),
        dimensions: vec![("complexity".into(), 0.5), ("length".into(), 0.3)],
    });
    assert_eq!(name, "router_classification");
    assert_eq!(v["tier"], "high");
    assert_eq!(v["requested_tier"], "low");
    assert_eq!(v["dimensions"][0][0], "complexity");
}

#[test]
fn test_loop_terminated_with_optional_fields() {
    let (name, v) = parts(&Event::LoopTerminated {
        session_id: "s1".into(),
        status: "stopped".into(),
        iterations: 7,
        verification: Some("passed".into()),
        reason: Some("goal met".into()),
    });
    assert_eq!(name, "loop_terminated");
    assert_eq!(v["iterations"], 7);
    assert_eq!(v["verification"], "passed");
    assert_eq!(v["reason"], "goal met");
}

#[test]
fn test_loop_terminated_without_optional_fields() {
    let (name, v) = parts(&Event::LoopTerminated {
        session_id: "s1".into(),
        status: "stopped".into(),
        iterations: 1,
        verification: None,
        reason: None,
    });
    assert_eq!(name, "loop_terminated");
    assert!(v["verification"].is_null());
    assert!(v["reason"].is_null());
}

#[test]
fn test_loop_change_summary() {
    let (name, v) = parts(&Event::LoopChangeSummary {
        session_id: "s1".into(),
        status: "done".into(),
        iterations: 3,
        files_modified: 2,
        files_created: 1,
        files_deleted: 0,
        diffstat: "3 files changed".into(),
        files: vec!["a.rs".into(), "b.rs".into()],
    });
    assert_eq!(name, "loop_change_summary");
    assert_eq!(v["files_modified"], 2);
    assert_eq!(v["files"][1], "b.rs");
}

#[test]
fn test_model_download_started() {
    let (name, v) = parts(&Event::ModelDownloadStarted {
        provider_id: "ollama".into(),
        model_id: "qwen".into(),
        session_id: "s1".into(),
    });
    assert_eq!(name, "model_download_started");
    assert_eq!(v["model_id"], "qwen");
}

#[test]
fn test_model_download_progress() {
    let (name, v) = parts(&Event::ModelDownloadProgress {
        provider_id: "ollama".into(),
        model_id: "qwen".into(),
        session_id: "s1".into(),
        percent: 42.5,
    });
    assert_eq!(name, "model_download_progress");
    assert_eq!(v["percent"], 42.5);
}

#[test]
fn test_model_download_finished_with_error() {
    let (name, v) = parts(&Event::ModelDownloadFinished {
        provider_id: "ollama".into(),
        model_id: "qwen".into(),
        session_id: "s1".into(),
        error: Some("download failed".into()),
    });
    assert_eq!(name, "model_download_finished");
    assert_eq!(v["error"], "download failed");
}

#[test]
fn test_mcp_server_enabled_changed() {
    let (name, v) = parts(&Event::McpServerEnabledChanged {
        server_id: "srv".into(),
        enabled: true,
    });
    assert_eq!(name, "mcp_server_enabled_changed");
    assert_eq!(v["enabled"], true);
}

#[test]
fn test_request_started() {
    let (name, v) = parts(&Event::RequestStarted {
        session_id: "s1".into(),
        outbound_bytes: 4096,
    });
    assert_eq!(name, "request_started");
    assert_eq!(v["outbound_bytes"], 4096);
}

#[test]
fn test_tool_call_batch() {
    let (name, v) = parts(&Event::ToolCallBatch {
        session_id: "s1".into(),
        step: 2,
        calls: vec![ToolCallBatchEntry {
            call_id: "c1".into(),
            tool: "read".into(),
            args: "{\"path\":\"a\"}".into(),
            error: None,
            duration_ms: 5,
            content: "file contents".into(),
            content_line_count: 1,
            metadata: None,
            success: true,
        }],
    });
    assert_eq!(name, "tool_call_batch");
    assert_eq!(v["step"], 2);
    assert_eq!(v["calls"][0]["tool"], "read");
}

#[test]
fn test_copilot_token_exchange_result() {
    let (name, v) = parts(&Event::CopilotTokenExchangeResult {
        success: false,
        api_base: None,
        error: Some("bad token".into()),
    });
    assert_eq!(name, "copilot_token_exchange_result");
    assert_eq!(v["success"], false);
    assert_eq!(v["error"], "bad token");
}

#[test]
fn test_copilot_device_flow_start_result_hides_device_code() {
    let (name, v) = parts(&Event::CopilotDeviceFlowStartResult {
        user_code: Some("ABCD-1234".into()),
        verification_uri: Some("https://github.com/login/device".into()),
        device_code: Some("secret-device-code".into()),
        interval: Some(5),
        error: None,
    });
    assert_eq!(name, "copilot_device_flow_start_result");
    assert_eq!(v["user_code"], "ABCD-1234");
    assert_eq!(v["device_code_present"], true);
    assert!(v.get("device_code").is_none());
}

#[test]
fn test_gitlab_setup_complete() {
    let (name, v) = parts(&Event::GitLabSetupComplete {
        success: true,
        error: None,
    });
    assert_eq!(name, "gitlab_setup_complete");
    assert_eq!(v["success"], true);
}

#[test]
fn test_subagent_suspended_resumed_killed() {
    let (n1, v1) = parts(&Event::SubagentSuspended {
        session_id: "s".into(),
        task_id: "t".into(),
        child_session_id: "c".into(),
    });
    assert_eq!(n1, "subagent_suspended");
    assert_eq!(v1["event"], "suspended");

    let (n2, v2) = parts(&Event::SubagentResumed {
        session_id: "s".into(),
        task_id: "t".into(),
        child_session_id: "c".into(),
    });
    assert_eq!(n2, "subagent_resumed");
    assert_eq!(v2["event"], "resumed");

    let (n3, v3) = parts(&Event::SubagentKilled {
        session_id: "s".into(),
        task_id: "t".into(),
        child_session_id: "c".into(),
        force: true,
    });
    assert_eq!(n3, "subagent_killed");
    assert_eq!(v3["force"], true);
}

#[test]
fn test_background_task_events() {
    let (n1, v1) = parts(&Event::BackgroundTaskSpawned {
        session_id: "s".into(),
        task_id: "t".into(),
        command: "sleep 1".into(),
    });
    assert_eq!(n1, "background_task_spawned");
    assert_eq!(v1["event"], "spawned");
    assert_eq!(v1["command"], "sleep 1");

    let (n2, v2) = parts(&Event::BackgroundTaskUpdated {
        session_id: "s".into(),
        task_id: "t".into(),
        status: "running".into(),
        progress: Some(serde_json::json!({"pct": 50})),
    });
    assert_eq!(n2, "background_task_updated");
    assert_eq!(v2["event"], "updated");
    assert_eq!(v2["progress"]["pct"], 50);

    let (n3, v3) = parts(&Event::BackgroundTaskCompleted {
        session_id: "s".into(),
        task_id: "t".into(),
        status: "exited".into(),
        exit_code: Some(0),
    });
    assert_eq!(n3, "background_task_completed");
    assert_eq!(v3["event"], "completed");
    assert_eq!(v3["exit_code"], 0);
}

#[test]
fn test_teammate_failed_suspended_resumed() {
    let (n1, v1) = parts(&Event::TeammateFailed {
        session_id: "s".into(),
        team_name: "team".into(),
        agent_id: "tm-1".into(),
        error: "boom".into(),
    });
    assert_eq!(n1, "teammate_failed");
    assert_eq!(v1["error"], "boom");

    let (n2, _) = parts(&Event::TeammateSuspended {
        session_id: "s".into(),
        team_name: "team".into(),
        agent_id: "tm-1".into(),
    });
    assert_eq!(n2, "teammate_suspended");

    let (n3, _) = parts(&Event::TeammateResumed {
        session_id: "s".into(),
        team_name: "team".into(),
        agent_id: "tm-1".into(),
    });
    assert_eq!(n3, "teammate_resumed");
}

#[test]
fn test_teammate_p2p_message() {
    let (name, v) = parts(&Event::TeammateP2PMessage {
        session_id: "s".into(),
        team_name: "team".into(),
        from: "tm-1".into(),
        to: "tm-2".into(),
        message_type: "chat".into(),
        preview: "hi".into(),
    });
    assert_eq!(name, "teammate_p2p_message");
    assert_eq!(v["from"], "tm-1");
    assert_eq!(v["preview"], "hi");
}

#[test]
fn test_shell_cwd_changed() {
    let (name, v) = parts(&Event::ShellCwdChanged {
        session_id: "s".into(),
        cwd: "/work".into(),
    });
    assert_eq!(name, "shell_cwd_changed");
    assert_eq!(v["cwd"], "/work");
}

#[test]
fn test_user_input() {
    let (name, v) = parts(&Event::UserInput {
        session_id: "s".into(),
        request_id: "r".into(),
        response: "yes".into(),
    });
    assert_eq!(name, "user_input");
    assert_eq!(v["response"], "yes");
}

#[test]
fn test_memory_stored_recalled_forgotten_searched() {
    let (n1, v1) = parts(&Event::MemoryStored {
        session_id: "s".into(),
        id: 7,
        category: "fact".into(),
    });
    assert_eq!(n1, "memory_stored");
    assert_eq!(v1["id"], 7);
    assert_eq!(v1["category"], "fact");

    let (n2, v2) = parts(&Event::MemoryRecalled {
        session_id: "s".into(),
        query: "q".into(),
        result_count: 3,
    });
    assert_eq!(n2, "memory_recalled");
    assert_eq!(v2["result_count"], 3);

    let (n3, v3) = parts(&Event::MemoryForgotten {
        session_id: "s".into(),
        count: 2,
    });
    assert_eq!(n3, "memory_forgotten");
    assert_eq!(v3["count"], 2);

    let (n4, v4) = parts(&Event::MemorySearched {
        session_id: "s".into(),
        query: "q".into(),
        result_count: 1,
        mode: "keyword".into(),
    });
    assert_eq!(n4, "memory_searched");
    assert_eq!(v4["mode"], "keyword");
}

#[test]
fn test_conversation_and_session_searched() {
    let (n1, v1) = parts(&Event::ConversationSearched {
        session_id: "s".into(),
        query: "q".into(),
        result_count: 4,
        mode: "semantic".into(),
    });
    assert_eq!(n1, "conversation_searched");
    assert_eq!(v1["result_count"], 4);

    let (n2, v2) = parts(&Event::SessionSearched {
        session_id: "s".into(),
        query: "q".into(),
        result_count: 5,
        mode: "fts".into(),
    });
    assert_eq!(n2, "session_searched");
    assert_eq!(v2["mode"], "fts");
}

#[test]
fn test_memory_candidate_extracted() {
    let (name, v) = parts(&Event::MemoryCandidateExtracted {
        session_id: "s".into(),
        content: "remember me".into(),
        category: "preference".into(),
        tags: vec!["a".into(), "b".into()],
        confidence: 0.9,
        source: "auto".into(),
        reason: "useful".into(),
    });
    assert_eq!(name, "memory_candidate_extracted");
    assert_eq!(v["tags"][0], "a");
    assert_eq!(v["confidence"], 0.9);
}

#[test]
fn test_hook_warning_and_tool_result_flagged() {
    let (n1, v1) = parts(&Event::HookWarning {
        session_id: "s".into(),
        hook_command: "lint".into(),
        tool: "write".into(),
        stderr: "warning".into(),
    });
    assert_eq!(n1, "hook_warning");
    assert_eq!(v1["stderr"], "warning");

    let (n2, v2) = parts(&Event::ToolResultFlagged {
        session_id: "s".into(),
        tool: "write".into(),
        hook_command: "lint".into(),
        reason: "style".into(),
    });
    assert_eq!(n2, "tool_result_flagged");
    assert_eq!(v2["reason"], "style");
}

#[test]
fn test_security_verdict() {
    let (name, v) = parts(&Event::SecurityVerdict {
        session_id: "s".into(),
        tool: "bash".into(),
        verdict: "deny".into(),
        rationale: "dangerous".into(),
    });
    assert_eq!(name, "security_verdict");
    assert_eq!(v["verdict"], "deny");
    assert_eq!(v["rationale"], "dangerous");
}

#[test]
fn test_open_research_view() {
    let (name, v) = parts(&Event::OpenResearchView {
        name: "topic".into(),
        path: std::path::PathBuf::from("/research/topic.md"),
        markdown: "# heading".into(),
    });
    assert_eq!(name, "open_research_view");
    assert_eq!(v["name"], "topic");
    assert_eq!(v["path"], "/research/topic.md");
}

#[test]
fn test_redacted_event_debug_hides_credentials() {
    // SEC-ragent-types-006: the Debug rendering of a credential-bearing event
    // must not leak the credential material.
    let event = Event::CopilotDeviceFlowComplete {
        token: "gho_supersecrettokenvalue".into(),
        api_base: "https://api.example".into(),
    };
    let rendered = redacted_event_debug(&event);
    assert!(
        !rendered.contains("gho_supersecrettokenvalue"),
        "credential leaked in debug: {rendered}"
    );
}
