//! Tests for the LLM security analyzer (spec `openhands` T-015; FR-006,
//! FR-016, FR-017).
//!
//! Covers verdict parsing (strict JSON, prose fallback, deny-wins ordering, and
//! the fail-safe `ask` default) and the `analyze_tool_action` stream/timeout
//! behaviour. The live-loop integration - verdict published before execution, a
//! `deny` reported to the model, an `allow` satisfying a bare prompt but never
//! an explicit rule - is covered by `test_security_analyzer_loop.rs`; the
//! `PreToolUse` exit-code semantics are covered by `test_hooks.rs`.

use std::sync::Arc;

use ragent_agent::llm::LlmClient;
use ragent_agent::permission::PermissionAction;
use ragent_agent::security::{SecurityVerdict, analyze_tool_action, parse_verdict};

// -- parse_verdict -----------------------------------------------------------

#[test]
fn test_parse_verdict_strict_allow_json() {
    let v = parse_verdict(r#"{"verdict": "allow", "rationale": "read-only within project"}"#);
    assert_eq!(v.decision, PermissionAction::Allow);
    assert_eq!(v.rationale, "read-only within project");
    assert_eq!(v.label(), "allow");
}

#[test]
fn test_parse_verdict_strict_deny_json() {
    let v = parse_verdict(r#"{"verdict":"deny","rationale":"destroying /etc"}"#);
    assert_eq!(v.decision, PermissionAction::Deny);
    assert_eq!(v.rationale, "destroying /etc");
}

#[test]
fn test_parse_verdict_strict_ask_json() {
    let v = parse_verdict(r#"{"verdict":"ask","rationale":"ambiguous target"}"#);
    assert_eq!(v.decision, PermissionAction::Ask);
    assert_eq!(v.rationale, "ambiguous target");
}

#[test]
fn test_parse_verdict_json_embedded_in_prose() {
    let raw = "Sure! Here is my assessment:\n```json\n\
               {\"verdict\": \"deny\", \"rationale\": \"exfiltrates secrets\"}\n```\n";
    let v = parse_verdict(raw);
    assert_eq!(v.decision, PermissionAction::Deny);
    assert_eq!(v.rationale, "exfiltrates secrets");
}

#[test]
fn test_parse_verdict_prose_fallback_deny_wins_over_allow() {
    // No JSON object at all: the keyword scan must pick `deny` even though the
    // word "allow" also appears.
    let raw = "I would not allow this; I must deny the request.";
    let v = parse_verdict(raw);
    assert_eq!(v.decision, PermissionAction::Deny);
    assert!(v.rationale.contains("parsed from prose"));
}

#[test]
fn test_parse_verdict_prose_fallback_allow() {
    let v = parse_verdict("This is safe and I allow it to proceed.");
    assert_eq!(v.decision, PermissionAction::Allow);
}

#[test]
fn test_parse_verdict_empty_is_fail_safe_ask() {
    let v = parse_verdict("   ");
    assert_eq!(v.decision, PermissionAction::Ask);
    assert!(v.rationale.contains("empty response"));
}

#[test]
fn test_parse_verdict_unrecognised_is_fail_safe_ask() {
    let v = parse_verdict("the quick brown fox jumps over the lazy dog");
    assert_eq!(v.decision, PermissionAction::Ask);
    assert!(v.rationale.contains("unrecognised"));
}

#[test]
fn test_parse_verdict_malformed_json_is_fail_safe_ask() {
    // Unbalanced braces: extraction fails, and no verdict keyword is present.
    let v = parse_verdict(r#"{"verdict": "zebra""#);
    assert_eq!(v.decision, PermissionAction::Ask);
}

#[test]
fn test_parse_verdict_malformed_json_with_allow_keyword_scans_prose() {
    // Unbalanced JSON still carries a recognisable "allow" token; the prose
    // scan recovers it rather than failing safe.
    let v = parse_verdict(r#"{"verdict": "allow""#);
    assert_eq!(v.decision, PermissionAction::Allow);
}

#[test]
fn test_parse_verdict_unknown_label_falls_back_to_prose_scan() {
    // The JSON object parses, but `verdict` is not a recognised label; the
    // whole body still contains "deny" so the prose scan wins.
    let v = parse_verdict(r#"{"verdict": "maybe", "rationale": "unsure"} - on balance I deny"#);
    assert_eq!(v.decision, PermissionAction::Deny);
}

#[test]
fn test_fail_safe_verdict_is_always_ask() {
    let v = SecurityVerdict::fail_safe("provider down");
    assert_eq!(v.decision, PermissionAction::Ask);
    assert!(v.rationale.contains("provider down"));
}

// -- analyze_tool_action -----------------------------------------------------

/// A client that yields pre-scripted text chunks, optionally after a delay.
struct StaticClient {
    chunks: Vec<String>,
    delay: Option<std::time::Duration>,
}

#[async_trait::async_trait]
impl LlmClient for StaticClient {
    async fn chat(
        &self,
        _request: ragent_agent::llm::ChatRequest,
    ) -> anyhow::Result<
        std::pin::Pin<Box<dyn futures::Stream<Item = ragent_agent::llm::StreamEvent> + Send>>,
    > {
        use ragent_agent::llm::{LlmFinishReason, StreamEvent};
        if let Some(d) = self.delay {
            tokio::time::sleep(d).await;
        }
        let mut events: Vec<StreamEvent> = self
            .chunks
            .iter()
            .map(|c| StreamEvent::TextDelta { text: c.clone() })
            .collect();
        events.push(StreamEvent::Finish {
            reason: LlmFinishReason::Stop,
        });
        Ok(Box::pin(futures::stream::iter(events)))
    }
}

#[tokio::test]
async fn test_analyze_tool_action_collects_streamed_verdict() {
    let client: Arc<dyn LlmClient> = Arc::new(StaticClient {
        chunks: vec![
            "{\"verdict\":".to_string(),
            "\"allow\",\"rationale\":\"read\"}".to_string(),
        ],
        delay: None,
    });
    let verdict = analyze_tool_action(&client, "m", "read", "{}", 5).await;
    assert_eq!(verdict.decision, PermissionAction::Allow);
    assert_eq!(verdict.rationale, "read");
}

#[tokio::test]
async fn test_analyze_tool_action_empty_response_is_fail_safe() {
    let client: Arc<dyn LlmClient> = Arc::new(StaticClient {
        chunks: vec![],
        delay: None,
    });
    let verdict = analyze_tool_action(&client, "m", "bash", "{}", 5).await;
    assert_eq!(verdict.decision, PermissionAction::Ask);
}
