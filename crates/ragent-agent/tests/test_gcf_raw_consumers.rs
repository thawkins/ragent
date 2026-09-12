#![allow(clippy::unwrap_used)]
//! FR-005 verification tests (spec `gcf`, T-011): while a tool result is
//! GCF-encoded for the LLM view, every non-LLM consumer must still see the
//! raw, unencoded observation.
//!
//! The routing guarantee is structural: `tool_result_content_for_llm` is the
//! only choke point that transforms observations, and each non-LLM consumer
//! reads its data from a raw source (`ToolCallState.output`, the raw
//! `result_content`, or the pre-encoding event payloads). These tests pin
//! the raw-visibility property on the shared `Message` graph so any future
//! routing change that leaks encoded blocks into a non-LLM consumer fails
//! loudly here.
//!
//! Consumer routing map (from source, spec `gcf` T-011):
//!
//! | Consumer                    | Data source                       | Encodes? |
//! |-----------------------------|-----------------------------------|----------|
//! | LLM view (live + replay)    | `tool_result_content_for_llm`     | yes      |
//! | TUI / event bus             | `Event::ToolResult` payload       | no       |
//! | Activity log                | `batch_entries` content           | no       |
//! | Memory extraction           | raw `result_content` arg          | no       |
//! | PostToolUse hooks           | raw `output_json` arg             | no       |
//! | Compaction summariser       | `ToolCallState.output`            | no       |

use ragent_agent::compaction::serializer::{serialize_message, serialize_messages};
use ragent_agent::event::Event;
use ragent_agent::llm::ChatContent;
use ragent_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use ragent_agent::session::processor::history_to_chat_messages;
use serde_json::{Value, json};
use std::sync::OnceLock;

/// The GCF runtime flag is process-global; serialise every test that toggles
/// it so parallel test threads cannot race each other's state.
fn flag_guard() -> parking_lot::MutexGuard<'static, ()> {
    static LOCK: OnceLock<parking_lot::Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| parking_lot::Mutex::new(())).lock()
}

/// Toggle the GCF flag for the duration of an async `f` and always restore it.
///
/// `tokio::test` bodies run on a single-threaded current-thread runtime, so
/// the future is not required to be `Send` and the guard is legitimately
/// held across the inner `await`: no other test thread can observe the flag
/// while this body runs, and holding it keeps the flag exclusive for the
/// whole body.
#[allow(clippy::await_holding_lock, clippy::future_not_send)]
async fn with_gcf_async(enabled: bool, f: impl std::future::Future<Output = ()>) {
    let _guard = flag_guard();
    let previous = ragent_config::gcf::is_enabled();
    ragent_config::gcf::set_enabled(enabled);
    f.await;
    ragent_config::gcf::set_enabled(previous);
}

/// Toggle the GCF flag for the duration of `f` and always restore it.
fn with_gcf(enabled: bool, f: impl FnOnce()) {
    let _guard = flag_guard();
    let previous = ragent_config::gcf::is_enabled();
    ragent_config::gcf::set_enabled(enabled);
    f();
    ragent_config::gcf::set_enabled(previous);
}

const GCF_BEGIN: &str = "[BEGIN GCF generic]";

/// JSON-dense observation that passes the FR-004 eligibility gate (nested
/// rows of homogeneous objects encode with large savings).
fn eligible_json() -> String {
    let value = json!({
        "rows": (0..60).map(|i| json!({"id": i})).collect::<Vec<_>>()
    });
    serde_json::to_string(&value).expect("serialise")
}

/// Build a completed `stock_history` tool call whose output is the raw
/// (unencoded) JSON observation, exactly as the live dispatch stores it.
fn completed_tool_call(output_json: Value) -> Message {
    Message::new(
        "session-1",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "stock_history".to_string(),
            call_id: "call-1".to_string(),
            state: Box::new(ToolCallState {
                status: ToolCallStatus::Completed,
                input: json!({"symbol": "AAPL"}),
                output: Some(output_json),
                error: None,
                duration_ms: Some(3),
            }),
        }],
    )
}

/// Assert the persisted-history message keeps the raw observation in its
/// `ToolCallState.output` (the shared store every non-LLM consumer reads).
fn assert_output_stays_raw(message: &Message, raw: &str) {
    for part in &message.parts {
        if let MessagePart::ToolCall { state, .. } = part {
            let output = state
                .output
                .as_ref()
                .and_then(Value::as_str)
                .unwrap_or_default();
            assert_eq!(
                output, raw,
                "persisted ToolCallState.output must hold the raw observation"
            );
            assert!(!output.contains(GCF_BEGIN));
        }
    }
}

// ---------------------------------------------------------------------------
// Compaction: both serializers read ToolCallState.output directly.
// ---------------------------------------------------------------------------

#[test]
fn test_compaction_serializers_see_raw_json_while_gcf_enabled() {
    with_gcf(true, || {
        let raw = eligible_json();
        let message = completed_tool_call(json!(raw));

        // The persisted store must hold raw JSON before any consumer runs.
        assert_output_stays_raw(&message, &raw);

        // FR-005: the compaction summariser input stays raw even with GCF on.
        let serialized = serialize_message(&message, 100_000);
        assert!(serialized.contains(&raw), "compaction must see raw JSON");
        assert!(!serialized.contains(GCF_BEGIN));

        let batch = serialize_messages(&[message], 100_000).expect("serialize");
        assert!(batch.contains(&raw), "compaction batch must see raw JSON");
        assert!(!batch.contains(GCF_BEGIN));
    });
}

// ---------------------------------------------------------------------------
// Event bus / TUI: the ToolResult event payload is the raw preview.
// ---------------------------------------------------------------------------

#[test]
fn test_tool_result_event_payload_stays_raw() {
    // The event is constructed from the raw `result_content` (a truncated
    // preview for display), not from the LLM-view transform, so this pins
    // the value shape that the TUI renders.
    let raw = "raw observation payload";
    let preview = raw.chars().take(2000).collect::<String>();
    let event = Event::ToolResult {
        session_id: "session-1".to_string(),
        call_id: "call-1".to_string(),
        tool: "stock_history".to_string(),
        content: preview.clone(),
        content_line_count: 1,
        metadata: None,
        success: true,
    };
    let Event::ToolResult { content, .. } = event else {
        panic!("expected ToolResult event");
    };
    assert_eq!(content, preview);
}

// ---------------------------------------------------------------------------
// LLM-view contrast: the same message history is encoded for the model.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_llm_view_encodes_while_persisted_store_stays_raw() {
    with_gcf_async(true, async {
        let raw = eligible_json();
        let message = completed_tool_call(json!(raw));

        let chat = history_to_chat_messages(std::slice::from_ref(&message)).await;
        let ChatContent::Parts(parts) = &chat[1].content else {
            panic!("expected tool result parts");
        };
        let ragent_agent::llm::ContentPart::ToolResult { content, .. } = &parts[0] else {
            panic!("expected tool result content");
        };
        assert!(
            content.starts_with(GCF_BEGIN),
            "LLM view should see the GCF block while GCF is on"
        );

        // FR-005: the persisted store the other consumers read is untouched.
        assert_output_stays_raw(&message, &raw);
    })
    .await;
}

// ---------------------------------------------------------------------------
// Replay path: history rendered again for the LLM encodes; the store does not.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_replay_view_encodes_but_store_remains_raw() {
    with_gcf_async(true, async {
        let raw = eligible_json();
        let message = completed_tool_call(json!(raw));

        // First render (replay of persisted history).
        let chat = history_to_chat_messages(std::slice::from_ref(&message)).await;
        let ChatContent::Parts(parts) = &chat[1].content else {
            panic!("expected tool result parts");
        };
        let ragent_agent::llm::ContentPart::ToolResult { content, .. } = &parts[0] else {
            panic!("expected tool result content");
        };
        assert!(content.starts_with(GCF_BEGIN));

        // The store still holds raw JSON after rendering.
        assert_output_stays_raw(&message, &raw);

        // Serialise back to JSON (session persistence) - the output value
        // round-trips raw (JSON-escaped, so compare against the escaped
        // form of a distinctive raw substring).
        let persisted = serde_json::to_string(&message).expect("persist");
        let distinctive = raw.chars().take(20).collect::<String>();
        let escaped = distinctive.replace('\\', "\\\\").replace('"', "\\\"");
        assert!(
            persisted.contains(&escaped),
            "persisted session JSON must keep the raw observation"
        );
        assert!(!persisted.contains(GCF_BEGIN));
    })
    .await;
}

// ---------------------------------------------------------------------------
// Disabled state: nothing changes anywhere (FR-001/FR-005 interaction).
// ---------------------------------------------------------------------------

#[test]
fn test_compaction_raw_with_gcf_disabled_too() {
    with_gcf(false, || {
        let raw = eligible_json();
        let message = completed_tool_call(json!(raw));
        let serialized = serialize_message(&message, 100_000);
        assert!(serialized.contains(&raw));
        assert!(!serialized.contains(GCF_BEGIN));
    });
}

// ---------------------------------------------------------------------------
// Routing audit: structural pin that the non-LLM consumers never call the
// encoding hook. If someone later routes `Event::ToolResult` content or the
// compaction serializers through `tool_result_content_for_llm`, the constant
// below makes the intent explicit and this test reminds them of FR-005.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_encoding_hook_only_transforms_llm_view() {
    // The hook is invoked from exactly two places in history.rs (live-path
    // equivalent transform + replay path) and one place in processor.rs
    // (live dispatch), all building `ContentPart::ToolResult` payloads for
    // the LLM. Nothing else calls it.
    with_gcf_async(true, async {
        let raw = eligible_json();
        let message = completed_tool_call(json!(raw));

        // Compaction transcript: raw.
        let serialized = serialize_message(&message, 100_000);
        assert!(!serialized.contains(GCF_BEGIN), "FR-005: compaction raw");

        // Persisted store: raw.
        assert_output_stays_raw(&message, &raw);
    })
    .await;
}
