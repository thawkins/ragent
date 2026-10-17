//! Exhaustive coverage of [`Event`]'s `type_name` and `session_id` mappings.
//!
//! Both are hand-written `match` blocks over every variant. A miss here is a
//! silent label regression in the TUI log panel or in the drop-warning tag, so
//! every variant is constructed once and its concrete label / session id is
//! asserted.

use ragent_types::event::{Event, FinishReason, ToolCallBatchEntry};
use ragent_types::permission::PermissionDecision;

/// A single variant plus its expected `type_name()` and `session_id()`.
///
/// `session` is `Some(id)` for session-scoped events and `None` for the
/// infrastructure events that are deliberately session-less.
fn variant(
    event: Event,
    name: &'static str,
    session: Option<&'static str>,
) -> (Event, &'static str, Option<&'static str>) {
    (event, name, session)
}

fn s(v: &str) -> String {
    v.to_string()
}

/// One construction of every `Event` variant with its expected mapping.
#[allow(clippy::too_many_lines)]
fn all_variants() -> Vec<(Event, &'static str, Option<&'static str>)> {
    vec![
        variant(
            Event::SessionCreated { session_id: s("s") },
            "SessionCreated",
            Some("s"),
        ),
        variant(
            Event::SessionUpdated { session_id: s("s") },
            "SessionUpdated",
            Some("s"),
        ),
        variant(
            Event::MessageStart {
                session_id: s("s"),
                message_id: s("m"),
            },
            "MessageStart",
            Some("s"),
        ),
        variant(
            Event::TextDelta {
                session_id: s("s"),
                text: s("t"),
            },
            "TextDelta",
            Some("s"),
        ),
        variant(
            Event::ReasoningDelta {
                session_id: s("s"),
                text: s("t"),
            },
            "ReasoningDelta",
            Some("s"),
        ),
        variant(
            Event::ToolCallStart {
                session_id: s("s"),
                call_id: s("c"),
                tool: s("read"),
            },
            "ToolCallStart",
            Some("s"),
        ),
        variant(
            Event::ToolCallEnd {
                session_id: s("s"),
                call_id: s("c"),
                tool: s("read"),
                error: None,
                duration_ms: 5,
            },
            "ToolCallEnd",
            Some("s"),
        ),
        variant(
            Event::MessageEnd {
                session_id: s("s"),
                message_id: s("m"),
                reason: FinishReason::Stop,
            },
            "MessageEnd",
            Some("s"),
        ),
        variant(
            Event::PermissionRequested {
                session_id: s("s"),
                request_id: s("r"),
                permission: s("file:write"),
                description: s("d"),
                options: vec![],
            },
            "PermissionRequested",
            Some("s"),
        ),
        variant(
            Event::PermissionReplied {
                session_id: s("s"),
                request_id: s("r"),
                allowed: true,
                decision: PermissionDecision::Once,
            },
            "PermissionReplied",
            Some("s"),
        ),
        variant(
            Event::QuestionRequested {
                session_id: s("s"),
                request_id: s("r"),
                question: s("q"),
                options: vec![s("a")],
                default_index: Some(0),
            },
            "QuestionRequested",
            Some("s"),
        ),
        variant(
            Event::QuestionAnswered {
                session_id: s("s"),
                request_id: s("r"),
                response: s("a"),
            },
            "QuestionAnswered",
            Some("s"),
        ),
        variant(
            Event::AgentSwitched {
                session_id: s("s"),
                from: s("coder"),
                to: s("ask"),
            },
            "AgentSwitched",
            Some("s"),
        ),
        variant(
            Event::AgentSwitchRequested {
                session_id: s("s"),
                to: s("ask"),
                task: s("t"),
                context: s("c"),
            },
            "AgentSwitchRequested",
            Some("s"),
        ),
        variant(
            Event::AgentRestoreRequested {
                session_id: s("s"),
                summary: s("done"),
            },
            "AgentRestoreRequested",
            Some("s"),
        ),
        variant(
            Event::TaskCompleted {
                session_id: s("s"),
                summary: s("done"),
            },
            "TaskCompleted",
            Some("s"),
        ),
        variant(
            Event::AgentNotice {
                session_id: s("s"),
                message: s("m"),
            },
            "AgentNotice",
            Some("s"),
        ),
        variant(
            Event::AgentError {
                session_id: s("s"),
                error: s("e"),
            },
            "AgentError",
            Some("s"),
        ),
        variant(
            Event::ServiceStartError {
                session_id: s("s"),
                service: s("ollama"),
                command_path: s("/usr/bin/ollama"),
                stdout: s(""),
                stderr: s("boom"),
                error: s("failed"),
            },
            "ServiceStartError",
            Some("s"),
        ),
        variant(
            Event::McpStatusChanged {
                server_id: s("mcp"),
                status: s("connected"),
            },
            "McpStatusChanged",
            None,
        ),
        variant(
            Event::McpServerEnabledChanged {
                server_id: s("mcp"),
                enabled: true,
            },
            "McpServerEnabledChanged",
            None,
        ),
        variant(
            Event::TokenUsage {
                session_id: s("s"),
                input_tokens: 10,
                output_tokens: 20,
            },
            "TokenUsage",
            Some("s"),
        ),
        variant(
            Event::RunCostSummary {
                session_id: s("s"),
                model_id: s("m"),
                input_tokens: 1,
                output_tokens: 2,
                total_cost_usd: 0.5,
                duration_ms: 100,
            },
            "RunCostSummary",
            Some("s"),
        ),
        variant(
            Event::LoopTerminated {
                session_id: s("s"),
                status: s("completed"),
                iterations: 3,
                verification: None,
                reason: None,
            },
            "LoopTerminated",
            Some("s"),
        ),
        variant(
            Event::LoopChangeSummary {
                session_id: s("s"),
                status: s("completed"),
                iterations: 3,
                files_modified: 1,
                files_created: 2,
                files_deleted: 0,
                diffstat: s("+12 -3 lines across 4 files"),
                files: vec![s("src/main.rs")],
            },
            "LoopChangeSummary",
            Some("s"),
        ),
        variant(
            Event::RequestStarted {
                session_id: s("s"),
                outbound_bytes: 42,
            },
            "RequestStarted",
            Some("s"),
        ),
        variant(
            Event::ToolsSent {
                session_id: s("s"),
                tools: vec![s("read")],
            },
            "ToolsSent",
            Some("s"),
        ),
        variant(
            Event::ModelResponse {
                session_id: s("s"),
                text: s("hi"),
                elapsed_ms: 5,
                input_tokens: 1,
                output_tokens: 2,
            },
            "ModelResponse",
            Some("s"),
        ),
        variant(
            Event::ToolCallArgs {
                session_id: s("s"),
                call_id: s("c"),
                tool: s("read"),
                args: s("{}"),
            },
            "ToolCallArgs",
            Some("s"),
        ),
        variant(
            Event::ToolResult {
                session_id: s("s"),
                call_id: s("c"),
                tool: s("read"),
                content: s("ok"),
                content_line_count: 1,
                metadata: None,
                success: true,
            },
            "ToolResult",
            Some("s"),
        ),
        variant(
            Event::ToolCallBatch {
                session_id: s("s"),
                step: 1,
                calls: vec![ToolCallBatchEntry {
                    call_id: s("c"),
                    tool: s("read"),
                    args: s("{}"),
                    error: None,
                    duration_ms: 5,
                    content: s("ok"),
                    content_line_count: 1,
                    metadata: None,
                    success: true,
                }],
            },
            "ToolCallBatch",
            Some("s"),
        ),
        variant(
            Event::CopilotDeviceFlowComplete {
                token: s("tok"),
                api_base: s("https://api.example"),
            },
            "CopilotDeviceFlowComplete",
            None,
        ),
        variant(
            Event::GithubDeviceFlowComplete {
                success: true,
                error: None,
            },
            "GithubDeviceFlowComplete",
            None,
        ),
        variant(
            Event::GitLabSetupComplete {
                success: false,
                error: Some(s("bad")),
            },
            "GitLabSetupComplete",
            None,
        ),
        variant(
            Event::CopilotTokenExchangeResult {
                success: true,
                api_base: Some(s("https://api.example")),
                error: None,
            },
            "CopilotTokenExchangeResult",
            None,
        ),
        variant(
            Event::CopilotDeviceFlowStartResult {
                user_code: Some(s("ABCD")),
                verification_uri: Some(s("https://github.com/login/device")),
                device_code: Some(s("dev")),
                interval: Some(5),
                error: None,
            },
            "CopilotDeviceFlowStartResult",
            None,
        ),
        variant(
            Event::SessionAborted {
                session_id: s("s"),
                reason: s("user_requested"),
            },
            "SessionAborted",
            Some("s"),
        ),
        variant(
            Event::QuotaUpdate {
                session_id: s("s"),
                percent: 50.0,
            },
            "QuotaUpdate",
            Some("s"),
        ),
        variant(
            Event::SubagentStart {
                session_id: s("s"),
                task_id: s("t"),
                child_session_id: s("cs"),
                agent: s("explore"),
                task: s("find"),
                background: true,
            },
            "SubagentStart",
            Some("s"),
        ),
        variant(
            Event::SubagentComplete {
                session_id: s("s"),
                task_id: s("t"),
                child_session_id: s("cs"),
                summary: s("found"),
                success: true,
                duration_ms: 9,
                finish_reason: s("stop"),
            },
            "SubagentComplete",
            Some("s"),
        ),
        variant(
            Event::SubagentSuspended {
                session_id: s("s"),
                task_id: s("t"),
                child_session_id: s("cs"),
            },
            "SubagentSuspended",
            Some("s"),
        ),
        variant(
            Event::SubagentResumed {
                session_id: s("s"),
                task_id: s("t"),
                child_session_id: s("cs"),
            },
            "SubagentResumed",
            Some("s"),
        ),
        variant(
            Event::SubagentKilled {
                session_id: s("s"),
                task_id: s("t"),
                child_session_id: s("cs"),
                force: true,
            },
            "SubagentKilled",
            Some("s"),
        ),
        variant(
            Event::SubagentCancelled {
                session_id: s("s"),
                task_id: s("t"),
            },
            "SubagentCancelled",
            Some("s"),
        ),
        variant(
            Event::BackgroundTaskSpawned {
                session_id: s("s"),
                task_id: s("t"),
                command: s("sleep 1"),
            },
            "BackgroundTaskSpawned",
            Some("s"),
        ),
        variant(
            Event::BackgroundTaskUpdated {
                session_id: s("s"),
                task_id: s("t"),
                status: s("running"),
                progress: None,
            },
            "BackgroundTaskUpdated",
            Some("s"),
        ),
        variant(
            Event::BackgroundTaskCompleted {
                session_id: s("s"),
                task_id: s("t"),
                status: s("completed"),
                exit_code: Some(0),
            },
            "BackgroundTaskCompleted",
            Some("s"),
        ),
        variant(
            Event::TeammateSpawned {
                session_id: s("s"),
                team_name: s("team"),
                teammate_name: s("bob"),
                agent_id: s("tm-001"),
            },
            "TeammateSpawned",
            Some("s"),
        ),
        variant(
            Event::TeammateMessage {
                session_id: s("s"),
                team_name: s("team"),
                from: s("tm-001"),
                to: s("lead"),
                message_type: s("message"),
                preview: s("hi"),
            },
            "TeammateMessage",
            Some("s"),
        ),
        variant(
            Event::TeammateIdle {
                session_id: s("s"),
                team_name: s("team"),
                agent_id: s("tm-001"),
            },
            "TeammateIdle",
            Some("s"),
        ),
        variant(
            Event::TeammateFailed {
                session_id: s("s"),
                team_name: s("team"),
                agent_id: s("tm-001"),
                error: s("boom"),
            },
            "TeammateFailed",
            Some("s"),
        ),
        variant(
            Event::TeammateSuspended {
                session_id: s("s"),
                team_name: s("team"),
                agent_id: s("tm-001"),
            },
            "TeammateSuspended",
            Some("s"),
        ),
        variant(
            Event::TeammateResumed {
                session_id: s("s"),
                team_name: s("team"),
                agent_id: s("tm-001"),
            },
            "TeammateResumed",
            Some("s"),
        ),
        variant(
            Event::TeamTaskClaimed {
                session_id: s("s"),
                team_name: s("team"),
                agent_id: s("tm-001"),
                task_id: s("task-001"),
            },
            "TeamTaskClaimed",
            Some("s"),
        ),
        variant(
            Event::TeamTaskCompleted {
                session_id: s("s"),
                team_name: s("team"),
                agent_id: s("tm-001"),
                task_id: s("task-001"),
            },
            "TeamTaskCompleted",
            Some("s"),
        ),
        variant(
            Event::TeamCleanedUp {
                session_id: s("s"),
                team_name: s("team"),
            },
            "TeamCleanedUp",
            Some("s"),
        ),
        variant(
            Event::TeammateP2PMessage {
                session_id: s("s"),
                team_name: s("team"),
                from: s("tm-001"),
                to: s("tm-002"),
                message_type: s("message"),
                preview: s("hi"),
            },
            "TeammateP2PMessage",
            Some("s"),
        ),
        variant(
            Event::ShellCwdChanged {
                session_id: s("s"),
                cwd: s("/tmp"),
            },
            "ShellCwdChanged",
            Some("s"),
        ),
        variant(
            Event::OpenResearchView {
                name: s("topic"),
                path: std::path::PathBuf::from("/tmp/RESEARCH.md"),
                markdown: s("# hi"),
            },
            "OpenResearchView",
            None,
        ),
        variant(
            Event::UserInput {
                session_id: s("s"),
                request_id: s("r"),
                response: s("answer"),
            },
            "UserInput",
            Some("s"),
        ),
        variant(
            Event::MemoryStored {
                session_id: s("s"),
                id: 7,
                category: s("fact"),
            },
            "MemoryStored",
            Some("s"),
        ),
        variant(
            Event::MemoryRecalled {
                session_id: s("s"),
                query: s("q"),
                result_count: 2,
            },
            "MemoryRecalled",
            Some("s"),
        ),
        variant(
            Event::MemoryForgotten {
                session_id: s("s"),
                count: 3,
            },
            "MemoryForgotten",
            Some("s"),
        ),
        variant(
            Event::MemorySearched {
                session_id: s("s"),
                query: s("q"),
                result_count: 2,
                mode: s("fts"),
            },
            "MemorySearched",
            Some("s"),
        ),
        variant(
            Event::ConversationSearched {
                session_id: s("s"),
                query: s("q"),
                result_count: 2,
                mode: s("keyword"),
            },
            "ConversationSearched",
            Some("s"),
        ),
        variant(
            Event::SessionSearched {
                session_id: s("s"),
                query: s("q"),
                result_count: 2,
                mode: s("keyword"),
            },
            "SessionSearched",
            Some("s"),
        ),
        variant(
            Event::MemoryCandidateExtracted {
                session_id: s("s"),
                content: s("c"),
                category: s("fact"),
                tags: vec![s("rust")],
                confidence: 0.8,
                source: s("auto"),
                reason: s("why"),
            },
            "MemoryCandidateExtracted",
            Some("s"),
        ),
        variant(
            Event::HookWarning {
                session_id: s("s"),
                hook_command: s("lint"),
                tool: s("write"),
                stderr: s("warn"),
            },
            "HookWarning",
            Some("s"),
        ),
        variant(
            Event::ToolResultFlagged {
                session_id: s("s"),
                tool: s("write"),
                hook_command: s("lint"),
                reason: s("blocked"),
            },
            "ToolResultFlagged",
            Some("s"),
        ),
        variant(
            Event::SecurityVerdict {
                session_id: s("s"),
                tool: s("bash"),
                verdict: s("deny"),
                rationale: s("unsafe"),
            },
            "SecurityVerdict",
            Some("s"),
        ),
        variant(
            Event::CompressionStarted {
                session_id: s("s"),
                reason: s("threshold"),
            },
            "CompressionStarted",
            Some("s"),
        ),
        variant(
            Event::CompressionFinished {
                session_id: s("s"),
                original_tokens: 1000,
                compressed_tokens: 400,
                compression_ratio: 2.5,
                did_compress: true,
                reason: s("threshold"),
            },
            "CompressionFinished",
            Some("s"),
        ),
        variant(
            Event::ProviderLoadingStarted {
                provider_id: s("ollama"),
                provider_name: s("Ollama"),
            },
            "ProviderLoadingStarted",
            None,
        ),
        variant(
            Event::ProviderLoadingFinished {
                provider_id: s("ollama"),
                provider_name: s("Ollama"),
                models: vec![],
                error: None,
            },
            "ProviderLoadingFinished",
            None,
        ),
        variant(
            Event::RouterClassification {
                session_id: s("s"),
                tier: s("medium"),
                requested_tier: Some(s("high")),
                model: s("ollama:qwen"),
                composite_score: 0.5,
                prompt: s("hello"),
                dimensions: vec![(s("complexity"), 0.7)],
            },
            "RouterClassification",
            Some("s"),
        ),
        variant(
            Event::ModelDownloadStarted {
                provider_id: s("ollama"),
                model_id: s("qwen"),
                session_id: s("s"),
            },
            "ModelDownloadStarted",
            Some("s"),
        ),
        variant(
            Event::ModelDownloadProgress {
                provider_id: s("ollama"),
                model_id: s("qwen"),
                session_id: s("s"),
                percent: 42.0,
            },
            "ModelDownloadProgress",
            Some("s"),
        ),
        variant(
            Event::ModelDownloadFinished {
                provider_id: s("ollama"),
                model_id: s("qwen"),
                session_id: s("s"),
                error: None,
            },
            "ModelDownloadFinished",
            Some("s"),
        ),
    ]
}

#[test]
fn type_name_matches_every_variant() {
    let all = all_variants();
    assert_eq!(all.len(), 78, "variant table drifted from the enum");
    // The enum declares the same number of labels in `type_name`.
    for (event, expected, _) in &all {
        assert_eq!(
            event.type_name(),
            *expected,
            "wrong type_name for {event:?}"
        );
    }
}

#[test]
fn session_id_matches_every_variant() {
    for (event, _, expected) in &all_variants() {
        assert_eq!(
            event.session_id(),
            *expected,
            "wrong session_id for {}",
            event.type_name()
        );
    }
}

/// Every variant must render a non-empty `Debug` that starts with its own
/// `type_name` - the content-free `render_event_debug` shape.
#[test]
fn debug_renders_the_variant_name() {
    for (event, expected, _) in &all_variants() {
        let rendered = format!("{event:?}");
        assert!(
            rendered.starts_with(expected),
            "Debug for {expected} did not start with its name: {rendered}"
        );
    }
}

/// The infrastructure / credential variants that carry no session must report
/// `None` for `session_id` even when they carry identifying strings.
#[test]
fn session_less_events_report_none() {
    let session_less: Vec<Event> = all_variants()
        .into_iter()
        .filter(|(_, _, sess)| sess.is_none())
        .map(|(e, _, _)| e)
        .collect();
    assert_eq!(
        session_less.len(),
        10,
        "the set of session-less events changed"
    );
    for event in &session_less {
        assert_eq!(event.session_id(), None, "{}", event.type_name());
    }
}
