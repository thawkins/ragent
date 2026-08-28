//! Trigger envelope types for the trigger system (spec `piegap`).
//!
//! This module defines the foundational data types shared by the trigger
//! runtime and all trigger sources (dynamic rules, MCP notification hooks).
//!
//! A [`TriggerEnvelope`] is the normalized representation of a trigger event,
//! regardless of whether it originated from a user-defined dynamic rule or
//! an MCP server push notification. The trigger runtime consumes envelopes,
//! applies deduplication and cycle suppression, and dispatches the associated
//! action.
//!
//! A [`TriggerRule`] is a session-scoped rule that polls a condition and fires
//! an action when the condition matches. Rules are fire-once by default and
//! persist alongside the active session.
//!
//! See `specs/piegap/SPEC.md` FR-002 and FR-003 for the full specification.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Typed identifier for a trigger rule.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TriggerRuleId(pub String);

impl std::fmt::Display for TriggerRuleId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TriggerRuleId {
    /// Creates a new random trigger rule ID.
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Returns the ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for TriggerRuleId {
    fn default() -> Self {
        Self::new()
    }
}

impl From<String> for TriggerRuleId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for TriggerRuleId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// The kind of source that produced a trigger envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerSourceKind {
    /// A user-defined dynamic trigger rule that polled a condition and matched.
    Dynamic,
    /// An MCP server that pushed a notification frame.
    McpNotification,
}

/// How a trigger action should be delivered to the parent session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerActionKind {
    /// Inject a bounded summary into the parent chat without a model call
    /// (FR-003 `inject_summary`).
    InjectSummary,
    /// Inject a prompt and run one model turn in the parent's full tool
    /// context (FR-003 `inject_and_run`).
    InjectAndRun,
    /// Run the action in a sub-agent with a fresh context (FR-002 dynamic
    /// trigger rules).
    SubAgent,
}

/// Lifecycle status of a trigger rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerRuleStatus {
    /// Rule is active and polling.
    Active,
    /// Rule has been disabled by the user.
    Disabled,
    /// Fire-once rule has already fired and will not fire again.
    Fired,
}

/// A normalized trigger event produced by any trigger source.
///
/// All trigger sources — dynamic rules, MCP notification hooks — convert
/// their source-specific data into a `TriggerEnvelope` before submitting it
/// to the trigger runtime. This uniform representation lets the runtime
/// apply deduplication and cycle suppression uniformly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerEnvelope {
    /// Unique identifier for this envelope (generated at creation).
    pub id: String,
    /// The kind of source that produced this envelope.
    pub source_kind: TriggerSourceKind,
    /// Identifier of the source rule or MCP server that produced this envelope.
    pub source_id: String,
    /// When the envelope was created.
    pub timestamp: DateTime<Utc>,
    /// A short human-readable summary of the trigger event (bounded to
    /// 500 characters).
    pub summary: String,
    /// The action prompt to execute when this trigger fires.
    pub action_prompt: String,
    /// How the action should be delivered.
    pub action_kind: TriggerActionKind,
    /// Whether the result should be promoted to the main chat feed.
    pub promote_to_chat: bool,
    /// A stable hash of the envelope content used for deduplication.
    ///
    /// Computed from `source_id` + `summary` + `action_prompt` so that
    /// duplicate notifications from the same source with the same content
    /// are suppressed.
    pub dedup_hash: u64,
}

impl TriggerEnvelope {
    /// Maximum length of the `summary` field (FR-003: bounded summary).
    pub const SUMMARY_MAX: usize = 500;

    /// Creates a new trigger envelope, computing the dedup hash and
    /// truncating the summary to the bounded length.
    pub fn new(
        source_kind: TriggerSourceKind,
        source_id: impl Into<String>,
        summary: impl Into<String>,
        action_prompt: impl Into<String>,
        action_kind: TriggerActionKind,
        promote_to_chat: bool,
    ) -> Self {
        let source_id = source_id.into();
        let summary = truncate_chars(&summary.into(), Self::SUMMARY_MAX);
        let action_prompt = action_prompt.into();
        let dedup_hash = compute_dedup_hash(&source_id, &summary, &action_prompt);
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            source_kind,
            source_id,
            timestamp: Utc::now(),
            summary,
            action_prompt,
            action_kind,
            promote_to_chat,
            dedup_hash,
        }
    }
}

/// A session-scoped trigger rule (FR-002).
///
/// Dynamic trigger rules poll a condition on a configurable interval and,
/// when the condition matches, fire an action in a sub-agent with a fresh
/// context. Rules are fire-once by default and persist alongside the active
/// session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerRule {
    /// Unique identifier for this rule.
    pub id: TriggerRuleId,
    /// Natural-language description of the condition to poll
    /// (e.g., "when $HOME/build.done exists").
    pub condition: String,
    /// Natural-language description of the action to execute when the
    /// condition matches.
    pub action: String,
    /// Whether the rule is fire-once (default) or repeating.
    pub fire_once: bool,
    /// Whether the rule is currently enabled.
    pub enabled: bool,
    /// Whether rule output should be promoted to the main chat feed.
    pub promote_to_chat: bool,
    /// When the rule was created.
    pub created_at: DateTime<Utc>,
    /// When the rule last fired, if ever.
    pub fired_at: Option<DateTime<Utc>>,
}

impl TriggerRule {
    /// Creates a new trigger rule with sensible defaults (fire-once, enabled).
    pub fn new(condition: impl Into<String>, action: impl Into<String>) -> Self {
        Self {
            id: TriggerRuleId::new(),
            condition: condition.into(),
            action: action.into(),
            fire_once: true,
            enabled: true,
            promote_to_chat: false,
            created_at: Utc::now(),
            fired_at: None,
        }
    }

    /// Returns the current lifecycle status of the rule.
    pub fn status(&self) -> TriggerRuleStatus {
        if self.fire_once && self.fired_at.is_some() {
            TriggerRuleStatus::Fired
        } else if !self.enabled {
            TriggerRuleStatus::Disabled
        } else {
            TriggerRuleStatus::Active
        }
    }
}

/// The result of a trigger firing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerFired {
    /// The envelope that fired.
    pub envelope: TriggerEnvelope,
    /// The ID of the rule that produced this firing, if applicable.
    pub rule_id: Option<TriggerRuleId>,
}

// ─── Internal helpers ──────────────────────────────────────────────────

/// Truncates a string to at most `max` characters, appending an ellipsis
/// if truncation occurred.
fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
}

/// Computes a stable non-cryptographic hash for deduplication.
///
/// Uses FNV-1a for simplicity and speed — this is not a security-sensitive
/// hash, just a dedup key.
fn compute_dedup_hash(source_id: &str, summary: &str, action_prompt: &str) -> u64 {
    fn fnv1a(data: &str, mut hash: u64) -> u64 {
        hash ^= 0xcbf2_9ce4_8422_2325; // FNV offset basis (re-seed per segment)
        for byte in data.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3); // FNV prime
        }
        hash
    }
    let mut h = 0u64;
    h = fnv1a(source_id, h);
    h = fnv1a(summary, h);
    h = fnv1a(action_prompt, h);
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trigger_rule_id_roundtrip() {
        let id = TriggerRuleId::new();
        let s = id.to_string();
        let id2 = TriggerRuleId::from(s);
        assert_eq!(id, id2);
    }

    #[test]
    fn test_trigger_rule_defaults() {
        let rule = TriggerRule::new("file exists", "print it");
        assert!(rule.fire_once);
        assert!(rule.enabled);
        assert!(!rule.promote_to_chat);
        assert_eq!(rule.status(), TriggerRuleStatus::Active);
    }

    #[test]
    fn test_trigger_rule_status_disabled() {
        let mut rule = TriggerRule::new("cond", "act");
        rule.enabled = false;
        assert_eq!(rule.status(), TriggerRuleStatus::Disabled);
    }

    #[test]
    fn test_trigger_rule_status_fired() {
        let mut rule = TriggerRule::new("cond", "act");
        rule.fired_at = Some(Utc::now());
        assert_eq!(rule.status(), TriggerRuleStatus::Fired);
    }

    #[test]
    fn test_envelope_dedup_hash_stable() {
        let e1 = TriggerEnvelope::new(
            TriggerSourceKind::McpNotification,
            "server-1",
            "build done",
            "report it",
            TriggerActionKind::InjectSummary,
            false,
        );
        let e2 = TriggerEnvelope::new(
            TriggerSourceKind::McpNotification,
            "server-1",
            "build done",
            "report it",
            TriggerActionKind::InjectSummary,
            false,
        );
        assert_eq!(e1.dedup_hash, e2.dedup_hash);
    }

    #[test]
    fn test_envelope_dedup_hash_differs_on_content() {
        let e1 = TriggerEnvelope::new(
            TriggerSourceKind::McpNotification,
            "server-1",
            "build done",
            "report it",
            TriggerActionKind::InjectSummary,
            false,
        );
        let e2 = TriggerEnvelope::new(
            TriggerSourceKind::McpNotification,
            "server-1",
            "build failed",
            "report it",
            TriggerActionKind::InjectSummary,
            false,
        );
        assert_ne!(e1.dedup_hash, e2.dedup_hash);
    }

    #[test]
    fn test_envelope_summary_truncation() {
        let long_summary = "x".repeat(1000);
        let e = TriggerEnvelope::new(
            TriggerSourceKind::Dynamic,
            "rule-1",
            &long_summary,
            "act",
            TriggerActionKind::SubAgent,
            false,
        );
        assert!(e.summary.chars().count() <= TriggerEnvelope::SUMMARY_MAX);
    }
}
