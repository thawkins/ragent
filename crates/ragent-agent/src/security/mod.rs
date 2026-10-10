//! LLM security analyzer (spec `openhands` FR-006, FR-016, FR-017).
//!
//! When the `security_analyzer` mode is enabled, each proposed tool action is
//! evaluated by an LLM that returns an `allow` / `ask` / `deny` verdict plus a
//! rationale. The verdict is published as [`Event::SecurityVerdict`] *before*
//! the action is permitted or refused (FR-017), and it is applied as a
//! tightening-only layer over the static permission rules:
//!
//! - `deny` is a hard denial (reported to the model so it can correct course);
//! - `allow` can satisfy a bare `Ask` (no explicit policy rule), but never
//!   overrides an explicit policy `Deny` or a forced destructive-action
//!   checkpoint;
//! - `ask` - and *any* analyzer failure (missing model, provider error,
//!   unparseable output, timeout) - degrades to the normal interactive flow, so
//!   a broken analyzer can never silently broaden access.
//!
//! `pre_tool_use` hook exit-code semantics (FR-016) are independent and
//! unchanged: exit code 2 blocks and returns the hook stderr as the tool result;
//! exit code 1 warns and continues.
//!
//! [`Event::SecurityVerdict`]: ragent_types::event::Event::SecurityVerdict

use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use serde_json::Value;

use crate::llm::{ChatContent, ChatMessage, ChatRequest, LlmClient, StreamEvent};
use crate::permission::PermissionAction;

/// The analyzer's verdict for one proposed tool action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityVerdict {
    /// The decided action (`allow`, `ask`, or `deny`).
    pub decision: PermissionAction,
    /// Human-readable justification shown to the user (FR-017).
    pub rationale: String,
}

impl SecurityVerdict {
    /// Construct a verdict.
    #[must_use]
    pub fn new(decision: PermissionAction, rationale: impl Into<String>) -> Self {
        Self {
            decision,
            rationale: rationale.into(),
        }
    }

    /// The lowercase verdict label (`"allow"`, `"ask"`, `"deny"`).
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self.decision {
            PermissionAction::Allow => "allow",
            PermissionAction::Ask => "ask",
            PermissionAction::Deny => "deny",
        }
    }

    /// The fail-safe verdict used whenever the analyzer cannot produce a real
    /// answer: escalate to the normal interactive flow rather than widening
    /// access.
    #[must_use]
    pub fn fail_safe(reason: impl Into<String>) -> Self {
        Self::new(
            PermissionAction::Ask,
            format!(
                "security analyzer unavailable ({}); falling back to the standard \
                 permission flow",
                reason.into()
            ),
        )
    }
}

/// The maximum output tokens to request from the analyzer call.
///
/// A verdict plus a short rationale fits comfortably in a few hundred tokens.
const ANALYZER_MAX_TOKENS: u32 = 400;

/// System prompt for the analyzer call.
const ANALYZER_SYSTEM_PROMPT: &str = "\
You are a security analyzer guarding an autonomous coding agent. You are shown \
a single proposed tool action and must decide whether it may proceed. Reply with \
ONLY a JSON object, no prose, no code fences:\n\
{\"verdict\": \"allow\" | \"ask\" | \"deny\", \"rationale\": \"<one sentence>\"}\n\
\n\
- \"allow\": the action is clearly safe (read-only, within the project, no \
destructive or exfiltration risk).\n\
- \"ask\": the action is plausibly legitimate but risky, ambiguous, or \
irreversible enough that a human should confirm it.\n\
- \"deny\": the action is dangerous (destructive, credential/secret access, \
data exfiltration, disabling safety controls) and must not run.\n\
When uncertain, prefer \"ask\". Never \"allow\" a destructive, secret-exposing, \
or security-disabling action.";

/// Build the analyzer [`ChatRequest`].
///
/// The request carries one user message describing the tool and its input, no
/// tools, and a bounded output budget. `tool_input` is the JSON string supplied
/// by the model.
#[must_use]
pub fn build_analyzer_request(model: &str, tool_name: &str, tool_input: &str) -> ChatRequest {
    let user = format!(
        "Proposed tool action:\n- tool: {tool_name}\n- input: {tool_input}\n\n\
         Return your verdict as JSON."
    );
    ChatRequest {
        model: model.to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text(user),
        }]),
        tools: Arc::new(Vec::new()),
        temperature: Some(0.0),
        top_p: None,
        max_tokens: Some(ANALYZER_MAX_TOKENS),
        system: Some(Arc::from(ANALYZER_SYSTEM_PROMPT)),
        options: std::collections::HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    }
}

/// Parse a raw analyzer response into a verdict (FR-006).
///
/// Accepts a strict `{"verdict": ..., "rationale": ...}` object anywhere in the
/// response body, and falls back to a keyword scan when the model wraps its
/// answer in prose. Any unparseable or unrecognised response degrades to
/// [`PermissionAction::Ask`] so a malformed verdict never widens access.
#[must_use]
pub fn parse_verdict(raw: &str) -> SecurityVerdict {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return SecurityVerdict::fail_safe("empty response");
    }

    if let Some(obj) = extract_json_object(trimmed) {
        let verdict = obj
            .get("verdict")
            .and_then(Value::as_str)
            .map(str::trim)
            .map(str::to_ascii_lowercase);
        let rationale = obj
            .get("rationale")
            .and_then(Value::as_str)
            .map(str::trim)
            .unwrap_or("")
            .to_string();
        if let Some(decision) = verdict.as_deref().and_then(decision_from_label) {
            let rationale_text = if rationale.is_empty() {
                format!("analyzer verdict: {}", verdict_label(&decision))
            } else {
                rationale
            };
            return SecurityVerdict::new(decision, rationale_text);
        }
    }

    // Keyword fallback: scan the whole response. Order matters - an explicit
    // deny must win over any incidental mention of "allow".
    let lower = trimmed.to_ascii_lowercase();
    for (needle, decision) in [
        ("deny", PermissionAction::Deny),
        ("ask", PermissionAction::Ask),
        ("allow", PermissionAction::Allow),
    ] {
        if lower.contains(needle) {
            let rationale_text = format!(
                "analyzer verdict (parsed from prose): {}",
                verdict_label(&decision)
            );
            return SecurityVerdict::new(decision, rationale_text);
        }
    }

    SecurityVerdict::fail_safe("unrecognised response")
}

/// Find the first balanced `{...}` JSON object in `text` and parse it.
fn extract_json_object(text: &str) -> Option<Value> {
    let start = text.find('{')?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in text[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let end = start + offset + ch.len_utf8();
                    return serde_json::from_str(&text[start..end]).ok();
                }
            }
            _ => {}
        }
    }
    None
}

/// Map a JSON `verdict` label to a [`PermissionAction`].
fn decision_from_label(label: &str) -> Option<PermissionAction> {
    match label {
        "allow" | "allowed" => Some(PermissionAction::Allow),
        "ask" | "confirm" | "prompt" => Some(PermissionAction::Ask),
        "deny" | "denied" | "block" | "blocked" => Some(PermissionAction::Deny),
        _ => None,
    }
}

/// The lowercase label for a decision (used in fallback rationales).
fn verdict_label(decision: &PermissionAction) -> &'static str {
    SecurityVerdict::new(decision.clone(), "").label()
}

/// Evaluate one proposed tool action with the analyzer model (FR-006).
///
/// Sends a single no-tools completion through `client` and drains the streamed
/// text within `timeout_secs`. Any provider error, stream stall, empty response,
/// or unparseable output yields a fail-safe `ask` verdict so the caller falls
/// back to the standard permission flow.
pub async fn analyze_tool_action(
    client: &Arc<dyn LlmClient>,
    model_id: &str,
    tool_name: &str,
    tool_input: &str,
    timeout_secs: u64,
) -> SecurityVerdict {
    let request = build_analyzer_request(model_id, tool_name, tool_input);
    let budget = Duration::from_secs(timeout_secs.max(1));

    let call = async {
        let mut stream = client.chat(request).await?;
        let mut raw = String::new();
        while let Some(event) = stream.next().await {
            match event {
                StreamEvent::TextDelta { text } => raw.push_str(&text),
                StreamEvent::Error { message } => {
                    anyhow::bail!("analyzer provider error: {message}");
                }
                StreamEvent::Finish { .. } => break,
                _ => {}
            }
        }
        Ok::<String, anyhow::Error>(raw)
    };

    match tokio::time::timeout(budget, call).await {
        Ok(Ok(raw)) => parse_verdict(&raw),
        Ok(Err(e)) => SecurityVerdict::fail_safe(format!("{e:#}")),
        Err(_) => SecurityVerdict::fail_safe(format!("timed out after {timeout_secs}s")),
    }
}
