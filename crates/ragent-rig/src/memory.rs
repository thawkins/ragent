//! Rig-backed conversation-memory policy adapter (T-011 / FR-014 / FR-020).
//!
//! This module implements the [`ConversationMemoryPolicy`] trait defined in
//! `ragent-agent::session::history` using the three policy semantics Rig's
//! `rig-memory` crate provides: **sliding window**, **token budget**, and
//! **compaction**.
//!
//! # Why not depend on `rig-memory` directly?
//!
//! `ragent-rig` pins `rig-core = "0.9"`, but the published `rig-memory` crate
//! tracks the `rig` umbrella crate at `0.40.x`, which pulls `rig-core 0.40` —
//! incompatible with our pinned range (FR-033). Rather than block T-011 on a
//! Rig version bump, the policy semantics are implemented here in the
//! `ragent-rig` adapter crate (the Rig integration surface) using the same
//! primitives ragent already uses (`estimate_chat_message_tokens`). When the
//! pinned Rig version catches up to one that ships `rig-memory`, the bodies of
//! [`SlidingWindowPolicy`], [`TokenBudgetPolicy`], and [`CompactionPolicy`] can
//! delegate to the upstream types without changing this module's public API.
//!
//! # Wiring
//!
//! The binary constructs a [`MemoryAdapter`] from `config.rig.memory` (when
//! `enabled`) and registers it on the `SessionProcessor` via
//! [`SessionProcessor::set_memory_policy`]. The processor then delegates
//! history trimming to the policy whenever `should_compress_with_reported`
//! reports the context window is exceeded (FR-014), instead of running the
//! Headroom compression pipeline (FR-020).
//!
//! This module is compiled only when the `memory` feature is enabled.

use std::sync::Arc;

use ragent_agent::session::history::{
    ConversationMemoryPolicy, TrimStats, estimate_chat_message_tokens,
};
use ragent_config::RigMemoryConfig;
use ragent_llm::llm::ChatMessage;

use crate::error::{Result, RigError};

/// A marker inserted in place of compacted history by [`CompactionPolicy`].
const COMPACTION_MARKER: &str = "[...earlier conversation compacted...]";

/// Build a boxed [`ConversationMemoryPolicy`] from a [`RigMemoryConfig`].
///
/// Returns `Ok(None)` when `config.enabled` is `false` (so the caller can pass
/// the result straight through to
/// [`SessionProcessor::set_memory_policy`](ragent_agent::session::processor::SessionProcessor::set_memory_policy)).
///
/// # Errors
///
/// Returns [`RigError::InvalidConfiguration`] when `config.policy` is not one
/// of the recognised policy names, or when `config.limit` is `0` (a zero limit
/// would trim every message and is almost certainly a misconfiguration).
pub fn build_memory_policy(
    config: &RigMemoryConfig,
) -> Result<Option<Arc<dyn ConversationMemoryPolicy>>> {
    if !config.enabled {
        return Ok(None);
    }
    if config.limit == 0 {
        return Err(RigError::InvalidConfiguration(
            "rig.memory.limit must be greater than 0".to_owned(),
        ));
    }
    let policy: Arc<dyn ConversationMemoryPolicy> = match config.policy.as_str() {
        "sliding_window" => Arc::new(SlidingWindowPolicy {
            limit: config.limit,
        }),
        "token_budget" => Arc::new(TokenBudgetPolicy {
            budget: config.limit,
        }),
        "compaction" => Arc::new(CompactionPolicy {
            keep_recent: config.limit,
        }),
        other => {
            return Err(RigError::InvalidConfiguration(format!(
                "unknown rig.memory.policy '{other}' (expected sliding_window, token_budget, or compaction)"
            )));
        }
    };
    Ok(Some(policy))
}

// ── Sliding window ──────────────────────────────────────────────────────────

/// Retain the most recent `limit` messages, dropping everything older.
///
/// A leading system message (if any) is always preserved in addition to the
/// window, and the very last message is never dropped.
#[derive(Debug, Clone)]
pub struct SlidingWindowPolicy {
    /// Maximum number of non-system messages to retain.
    pub limit: usize,
}

impl ConversationMemoryPolicy for SlidingWindowPolicy {
    fn trim(&self, messages: &mut Vec<ChatMessage>, _context_window: usize) -> TrimStats {
        let original_count = messages.len();
        if original_count == 0 {
            return TrimStats {
                original_count: 0,
                retained_count: 0,
                removed_tokens: 0,
            };
        }

        let original_tokens: u64 = messages.iter().map(estimate_chat_message_tokens).sum();

        // Preserve a leading system message.
        let has_leading_system = messages
            .first()
            .is_some_and(|m| m.role.eq_ignore_ascii_case("system"));
        let body_start = if has_leading_system { 1 } else { 0 };
        let body_len = original_count.saturating_sub(body_start);

        // If the body is already within the window, do nothing.
        if body_len <= self.limit {
            return TrimStats {
                original_count,
                retained_count: original_count,
                removed_tokens: 0,
            };
        }

        let keep_from = original_count - self.limit;
        let mut retained: Vec<ChatMessage> = Vec::with_capacity(self.limit + 1);
        if has_leading_system {
            retained.push(messages[0].clone());
        }
        retained.extend(messages[keep_from..].iter().cloned());

        let retained_tokens: u64 = retained.iter().map(estimate_chat_message_tokens).sum();
        *messages = retained;
        TrimStats {
            original_count,
            retained_count: messages.len(),
            removed_tokens: original_tokens.saturating_sub(retained_tokens),
        }
    }

    fn name(&self) -> &str {
        "rig-sliding-window"
    }
}

// ── Token budget ────────────────────────────────────────────────────────────

/// Drop oldest messages (after any leading system message) until the estimated
/// token total of the retained prefix fits within `budget` tokens.
///
/// The most recent message is always retained even if it alone exceeds the
/// budget, so the agent always has the current turn to respond to.
#[derive(Debug, Clone)]
pub struct TokenBudgetPolicy {
    /// Maximum estimated token count for the retained history.
    pub budget: usize,
}

impl ConversationMemoryPolicy for TokenBudgetPolicy {
    fn trim(&self, messages: &mut Vec<ChatMessage>, _context_window: usize) -> TrimStats {
        let original_count = messages.len();
        if original_count == 0 {
            return TrimStats {
                original_count: 0,
                retained_count: 0,
                removed_tokens: 0,
            };
        }
        let original_tokens: u64 = messages.iter().map(estimate_chat_message_tokens).sum();

        let has_leading_system = messages
            .first()
            .is_some_and(|m| m.role.eq_ignore_ascii_case("system"));
        let body_start = if has_leading_system { 1 } else { 0 };

        // Walk from the most recent message backwards, accumulating tokens
        // until adding the next-older message would exceed the budget.
        let mut running: u64 = 0;
        let mut cutoff = original_count; // retain messages[cutoff..]
        for idx in (body_start..original_count).rev() {
            let t = estimate_chat_message_tokens(&messages[idx]);
            if running + t > self.budget as u64 && cutoff < original_count {
                // Keep at least the most recent message.
                break;
            }
            running += t;
            cutoff = idx;
        }

        // If we never moved the cutoff, nothing exceeded the budget.
        if cutoff <= body_start {
            return TrimStats {
                original_count,
                retained_count: original_count,
                removed_tokens: 0,
            };
        }

        let mut retained: Vec<ChatMessage> = Vec::with_capacity(original_count - cutoff + 1);
        if has_leading_system {
            retained.push(messages[0].clone());
        }
        retained.extend(messages[cutoff..].iter().cloned());

        let retained_tokens: u64 = retained.iter().map(estimate_chat_message_tokens).sum();
        *messages = retained;
        TrimStats {
            original_count,
            retained_count: messages.len(),
            removed_tokens: original_tokens.saturating_sub(retained_tokens),
        }
    }

    fn name(&self) -> &str {
        "rig-token-budget"
    }
}

// ── Compaction ──────────────────────────────────────────────────────────────

/// Replace the oldest messages (after any leading system message) with a
/// single compacted-marker assistant message, keeping the `keep_recent` most
/// recent turns verbatim.
///
/// This is the in-process fallback for Rig's compaction policy: a real LLM
/// summarisation step would require a completion call, which the trimming path
/// must not make (it runs inside the agent loop's compression hook). The
/// marker preserves the structural fact that earlier history existed without
/// paying for a summarisation round-trip. When a future Rig version ships a
/// compaction policy that can summarise synchronously, this body can delegate
/// to it.
#[derive(Debug, Clone)]
pub struct CompactionPolicy {
    /// Number of most-recent messages to retain verbatim.
    pub keep_recent: usize,
}

impl ConversationMemoryPolicy for CompactionPolicy {
    fn trim(&self, messages: &mut Vec<ChatMessage>, _context_window: usize) -> TrimStats {
        let original_count = messages.len();
        if original_count == 0 {
            return TrimStats {
                original_count: 0,
                retained_count: 0,
                removed_tokens: 0,
            };
        }
        let original_tokens: u64 = messages.iter().map(estimate_chat_message_tokens).sum();

        let has_leading_system = messages
            .first()
            .is_some_and(|m| m.role.eq_ignore_ascii_case("system"));
        let body_start = if has_leading_system { 1 } else { 0 };
        let body_len = original_count.saturating_sub(body_start);

        // Nothing to compact if the body is already within the kept window.
        if body_len <= self.keep_recent {
            return TrimStats {
                original_count,
                retained_count: original_count,
                removed_tokens: 0,
            };
        }

        let compact_up_to = original_count - self.keep_recent;
        let mut retained: Vec<ChatMessage> = Vec::with_capacity(self.keep_recent + 2);
        if has_leading_system {
            retained.push(messages[0].clone());
        }
        // Insert a single compaction-marker assistant message summarising the
        // dropped range. The role is `assistant` so providers treat it as
        // prior context, not a new user turn.
        retained.push(ChatMessage {
            role: "assistant".to_string(),
            content: ragent_llm::llm::ChatContent::Text(COMPACTION_MARKER.to_string()),
        });
        retained.extend(messages[compact_up_to..].iter().cloned());

        let retained_tokens: u64 = retained.iter().map(estimate_chat_message_tokens).sum();
        *messages = retained;
        TrimStats {
            original_count,
            retained_count: messages.len(),
            removed_tokens: original_tokens.saturating_sub(retained_tokens),
        }
    }

    fn name(&self) -> &str {
        "rig-compaction"
    }
}

// ── Legacy `MemoryAdapter` handle ───────────────────────────────────────────
//
// The original T-001 placeholder exposed a `MemoryAdapter` struct. It is kept
// as a thin convenience wrapper so existing imports continue to resolve, and
// now delegates to [`build_memory_policy`] so callers can build a policy
// without touching the free function directly.

/// A thin handle to a Rig conversation-memory policy.
///
/// Construct with [`MemoryAdapter::from_config`] and pass
/// [`MemoryAdapter::policy`] to
/// [`SessionProcessor::set_memory_policy`](ragent_agent::session::processor::SessionProcessor::set_memory_policy).
#[derive(Clone)]
pub struct MemoryAdapter {
    /// The configured policy limit (messages for sliding window, tokens for
    /// token budget, recent-turns for compaction).
    pub limit: usize,
    /// The resolved policy, ready to hand to the session processor.
    policy: Arc<dyn ConversationMemoryPolicy>,
}

impl std::fmt::Debug for MemoryAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryAdapter")
            .field("limit", &self.limit)
            .field("policy", &self.policy.name())
            .finish()
    }
}

impl MemoryAdapter {
    /// Creates a memory adapter with the given policy limit, defaulting to a
    /// sliding-window policy.
    pub fn new(limit: usize) -> Self {
        let policy: Arc<dyn ConversationMemoryPolicy> = Arc::new(SlidingWindowPolicy { limit });
        Self { limit, policy }
    }

    /// Build a [`MemoryAdapter`] from a [`RigMemoryConfig`].
    ///
    /// # Errors
    ///
    /// Propagates errors from [`build_memory_policy`].
    pub fn from_config(config: &RigMemoryConfig) -> Result<Self> {
        let policy = build_memory_policy(config)?.ok_or(RigError::MemoryNotEnabled)?;
        let limit = config.limit;
        Ok(Self { limit, policy })
    }

    /// Returns the configured limit.
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Returns the underlying policy, ready to register on the session
    /// processor.
    #[must_use]
    pub fn policy(&self) -> Arc<dyn ConversationMemoryPolicy> {
        Arc::clone(&self.policy)
    }

    /// Returns `RigError::MemoryNotEnabled` if memory support is unavailable.
    ///
    /// Kept for backwards compatibility with the original placeholder API.
    pub fn ensure_available(&self) -> Result<()> {
        if self.limit == 0 {
            return Err(RigError::MemoryNotEnabled);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ragent_llm::llm::{ChatContent, ChatMessage};

    fn msg(role: &str, text: &str) -> ChatMessage {
        ChatMessage {
            role: role.to_string(),
            content: ChatContent::Text(text.to_string()),
        }
    }

    fn long_history(n: usize) -> Vec<ChatMessage> {
        (0..n)
            .map(|i| {
                let role = if i % 2 == 0 { "user" } else { "assistant" };
                msg(role, &format!("message {i} with some content"))
            })
            .collect()
    }

    fn system_plus_history(sys: &str, n: usize) -> Vec<ChatMessage> {
        let mut v = vec![msg("system", sys)];
        v.extend(long_history(n));
        v
    }

    fn content_text(msg: &ChatMessage) -> &str {
        match &msg.content {
            ChatContent::Text(t) => t.as_str(),
            ChatContent::Parts(_) => "",
        }
    }

    // ── SlidingWindowPolicy ─────────────────────────────────────────────────

    #[test]
    fn sliding_window_keeps_last_n_plus_system() {
        let policy = SlidingWindowPolicy { limit: 3 };
        let mut msgs = system_plus_history("system prompt", 10);
        let stats = policy.trim(&mut msgs, 8192);
        assert_eq!(stats.original_count, 11);
        // system + last 3 = 4
        assert_eq!(stats.retained_count, 4);
        assert_eq!(msgs.len(), 4);
        assert_eq!(msgs[0].role, "system");
        // The last kept body message must be the original last message.
        assert_eq!(
            content_text(msgs.last().unwrap()),
            "message 9 with some content"
        );
        assert!(stats.removed_tokens > 0);
    }

    #[test]
    fn sliding_window_no_op_when_under_limit() {
        let policy = SlidingWindowPolicy { limit: 10 };
        let mut msgs = long_history(5);
        let stats = policy.trim(&mut msgs, 8192);
        assert_eq!(stats.retained_count, 5);
        assert_eq!(stats.removed_tokens, 0);
        assert_eq!(msgs.len(), 5);
    }

    #[test]
    fn sliding_window_empty_input_is_safe() {
        let policy = SlidingWindowPolicy { limit: 3 };
        let mut msgs: Vec<ChatMessage> = Vec::new();
        let stats = policy.trim(&mut msgs, 8192);
        assert_eq!(stats, TrimStats::default());
        assert!(msgs.is_empty());
    }

    #[test]
    fn sliding_window_name() {
        let policy = SlidingWindowPolicy { limit: 5 };
        assert_eq!(policy.name(), "rig-sliding-window");
    }

    // ── TokenBudgetPolicy ───────────────────────────────────────────────────

    #[test]
    fn token_budget_drops_oldest_until_under_budget() {
        let policy = TokenBudgetPolicy { budget: 60 };
        let mut msgs = long_history(10);
        let stats = policy.trim(&mut msgs, 8192);
        assert_eq!(stats.original_count, 10);
        assert!(stats.retained_count < 10);
        // The most recent message is always retained.
        assert_eq!(
            content_text(msgs.last().unwrap()),
            "message 9 with some content"
        );
        assert!(stats.removed_tokens > 0);
    }

    #[test]
    fn token_budget_no_op_when_under_budget() {
        let policy = TokenBudgetPolicy { budget: 10_000 };
        let mut msgs = long_history(3);
        let stats = policy.trim(&mut msgs, 8192);
        assert_eq!(stats.retained_count, 3);
        assert_eq!(stats.removed_tokens, 0);
    }

    #[test]
    fn token_budget_keeps_system_message() {
        let policy = TokenBudgetPolicy { budget: 1 };
        let mut msgs = system_plus_history("sys", 5);
        let _ = policy.trim(&mut msgs, 8192);
        assert_eq!(msgs[0].role, "system");
        // At least the last message survives.
        assert!(!msgs.is_empty());
    }

    #[test]
    fn token_budget_name() {
        let policy = TokenBudgetPolicy { budget: 100 };
        assert_eq!(policy.name(), "rig-token-budget");
    }

    // ── CompactionPolicy ────────────────────────────────────────────────────

    #[test]
    fn compaction_inserts_marker_and_keeps_recent() {
        let policy = CompactionPolicy { keep_recent: 2 };
        let mut msgs = system_plus_history("sys", 8);
        let stats = policy.trim(&mut msgs, 8192);
        assert_eq!(stats.original_count, 9);
        // system + marker + last 2 = 4
        assert_eq!(msgs.len(), 4);
        assert_eq!(msgs[0].role, "system");
        // The second message is the compaction marker.
        assert!(content_text(&msgs[1]).contains("compacted"));
        // The last two are the original last two.
        assert_eq!(
            content_text(msgs.last().unwrap()),
            "message 7 with some content"
        );
        assert!(stats.removed_tokens > 0);
    }

    #[test]
    fn compaction_no_op_when_under_keep_recent() {
        let policy = CompactionPolicy { keep_recent: 10 };
        let mut msgs = long_history(4);
        let stats = policy.trim(&mut msgs, 8192);
        assert_eq!(stats.retained_count, 4);
        assert_eq!(stats.removed_tokens, 0);
        assert!(msgs.iter().all(|m| !content_text(m).contains("compacted")));
    }

    #[test]
    fn compaction_name() {
        let policy = CompactionPolicy { keep_recent: 5 };
        assert_eq!(policy.name(), "rig-compaction");
    }

    // ── build_memory_policy + MemoryAdapter ─────────────────────────────────

    #[test]
    fn build_memory_policy_returns_none_when_disabled() {
        let cfg = RigMemoryConfig {
            enabled: false,
            policy: "sliding_window".to_string(),
            limit: 10,
        };
        let p = build_memory_policy(&cfg).expect("disabled is not an error");
        assert!(p.is_none());
    }

    #[test]
    fn build_memory_policy_rejects_zero_limit() {
        let cfg = RigMemoryConfig {
            enabled: true,
            policy: "sliding_window".to_string(),
            limit: 0,
        };
        assert!(build_memory_policy(&cfg).is_err());
    }

    #[test]
    fn build_memory_policy_rejects_unknown_policy() {
        let cfg = RigMemoryConfig {
            enabled: true,
            policy: "future_policy".to_string(),
            limit: 10,
        };
        let err = build_memory_policy(&cfg)
            .err()
            .expect("unknown policy should error");
        assert!(err.to_string().contains("future_policy"));
    }

    #[test]
    fn build_memory_policy_sliding_window() {
        let cfg = RigMemoryConfig {
            enabled: true,
            policy: "sliding_window".to_string(),
            limit: 4,
        };
        let p = build_memory_policy(&cfg).unwrap().unwrap();
        assert_eq!(p.name(), "rig-sliding-window");
    }

    #[test]
    fn build_memory_policy_token_budget() {
        let cfg = RigMemoryConfig {
            enabled: true,
            policy: "token_budget".to_string(),
            limit: 1024,
        };
        let p = build_memory_policy(&cfg).unwrap().unwrap();
        assert_eq!(p.name(), "rig-token-budget");
    }

    #[test]
    fn build_memory_policy_compaction() {
        let cfg = RigMemoryConfig {
            enabled: true,
            policy: "compaction".to_string(),
            limit: 3,
        };
        let p = build_memory_policy(&cfg).unwrap().unwrap();
        assert_eq!(p.name(), "rig-compaction");
    }

    #[test]
    fn memory_adapter_from_config_builds_policy() {
        let cfg = RigMemoryConfig {
            enabled: true,
            policy: "token_budget".to_string(),
            limit: 512,
        };
        let adapter = MemoryAdapter::from_config(&cfg).expect("build adapter");
        assert_eq!(adapter.limit(), 512);
        assert_eq!(adapter.policy().name(), "rig-token-budget");
    }

    #[test]
    fn memory_adapter_from_config_disabled_is_error() {
        let cfg = RigMemoryConfig {
            enabled: false,
            policy: "sliding_window".to_string(),
            limit: 10,
        };
        assert!(MemoryAdapter::from_config(&cfg).is_err());
    }

    #[test]
    fn memory_adapter_new_defaults_to_sliding_window() {
        let adapter = MemoryAdapter::new(20);
        assert_eq!(adapter.limit(), 20);
        assert_eq!(adapter.policy().name(), "rig-sliding-window");
        assert!(adapter.ensure_available().is_ok());
    }

    #[test]
    fn memory_adapter_zero_limit_is_unavailable() {
        let adapter = MemoryAdapter::new(0);
        assert!(adapter.ensure_available().is_err());
    }

    #[test]
    fn adapter_policy_trims_via_trait_object() {
        // The processor holds the policy as `Arc<dyn ConversationMemoryPolicy>`,
        // so verify the trait-object dispatch works end-to-end.
        let cfg = RigMemoryConfig {
            enabled: true,
            policy: "sliding_window".to_string(),
            limit: 2,
        };
        let policy = build_memory_policy(&cfg).unwrap().unwrap();
        let mut msgs = long_history(6);
        let stats = policy.trim(&mut msgs, 8192);
        assert_eq!(stats.original_count, 6);
        assert_eq!(stats.retained_count, 2);
        assert_eq!(msgs.len(), 2);
    }
}
