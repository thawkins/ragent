//! Dynamic trigger rules — natural-language polling + sub-agent action
//! (spec `piegap` FR-002).
//!
//! This module implements the dynamic trigger rule engine that:
//!
//! - Parses natural-language trigger requests (e.g., "when $HOME/build.done
//!   exists, run cargo test") into structured [`TriggerRule`] objects.
//! - Polls each rule's condition on a configurable interval.
//! - When a condition matches, creates a [`TriggerEnvelope`] and submits it
//!   to the [`TriggerRuntime`] for deduplication and cycle suppression.
//! - If the envelope is dispatched, fires the action via the
//!   [`ActionDispatcher`] (which spawns a sub-agent with a fresh context in
//!   production, or records the dispatch in tests).
//!
//! ## Architecture
//!
//! The engine is decoupled from the LLM and sub-agent infrastructure through
//! two traits:
//!
//! - [`ConditionEvaluator`] — evaluates a natural-language condition string
//!   and returns `true` when it matches. In production this calls the LLM;
//!   in tests a simple pattern-matching implementation is used.
//!
//! - [`ActionDispatcher`] — dispatches a fired trigger's action prompt. In
//!   production this spawns a background sub-agent; in tests a recording
//!   implementation captures the dispatch for verification.
//!
//! This separation lets the engine be fully tested without LLM or sub-agent
//! dependencies, following the standalone module independence requirement
//! (FR-001).

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use ragent_config::TriggerConfig;
use ragent_types::trigger::{
    TriggerActionKind, TriggerEnvelope, TriggerRule, TriggerRuleId, TriggerRuleStatus,
    TriggerSourceKind,
};
use thiserror::Error;
use tracing::{debug, warn};

use super::runtime::{TriggerRuntime, TriggerRuntimeConfig};

/// Errors produced by the dynamic trigger engine.
#[derive(Debug, Error)]
pub enum DynamicTriggerError {
    /// The trigger system is disabled in configuration.
    #[error("trigger system is disabled")]
    Disabled,
    /// The maximum number of rules per session has been reached.
    #[error("maximum rules ({max}) reached, cannot add more")]
    MaxRulesReached {
        /// The configured maximum number of rules.
        max: usize,
    },
    /// A natural-language trigger request could not be parsed.
    #[error("failed to parse trigger request: {0}")]
    ParseFailed(String),
}

/// Evaluates a natural-language trigger condition.
///
/// In production this trait is implemented by a type that calls the LLM with
/// the condition string and interprets the response as a boolean. In tests,
/// [`SimpleConditionEvaluator`] provides pattern-based evaluation.
#[async_trait]
pub trait ConditionEvaluator: Send + Sync + 'static {
    /// Evaluates the given condition. Returns `true` if the condition
    /// currently matches.
    async fn evaluate(&self, condition: &str) -> bool;
}

/// Dispatches a fired trigger's action.
///
/// In production this spawns a background sub-agent with a fresh context.
/// In tests, [`NoopActionDispatcher`] records the dispatch for verification.
#[async_trait]
pub trait ActionDispatcher: Send + Sync + 'static {
    /// Dispatches the action prompt. Returns `Ok(())` on success.
    async fn dispatch(&self, action_prompt: &str, promote_to_chat: bool) -> anyhow::Result<()>;
}

/// A simple condition evaluator that matches based on file-existence or
/// exact-string patterns. Used in tests and as a fallback.
pub struct SimpleConditionEvaluator {
    /// Set of condition strings that evaluate to `true`.
    matching: Arc<parking_lot::Mutex<Vec<String>>>,
}

impl SimpleConditionEvaluator {
    /// Creates a new evaluator with no matching conditions.
    pub fn new() -> Self {
        Self {
            matching: Arc::new(parking_lot::Mutex::new(Vec::new())),
        }
    }

    /// Adds a condition that this evaluator will report as matching.
    pub fn add_matching(&self, condition: impl Into<String>) {
        self.matching.lock().push(condition.into());
    }

    /// Returns `true` if the given condition is in the matching set.
    pub fn matches(&self, condition: &str) -> bool {
        self.matching.lock().iter().any(|c| c == condition)
    }
}

impl Default for SimpleConditionEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ConditionEvaluator for SimpleConditionEvaluator {
    async fn evaluate(&self, condition: &str) -> bool {
        self.matches(condition)
    }
}

/// A no-op action dispatcher that records dispatched actions for test
/// verification.
pub struct NoopActionDispatcher {
    /// Record of all dispatched actions: (action_prompt, promote_to_chat).
    dispatched: Arc<parking_lot::Mutex<Vec<(String, bool)>>>,
}

impl NoopActionDispatcher {
    /// Creates a new recording dispatcher.
    pub fn new() -> Self {
        Self {
            dispatched: Arc::new(parking_lot::Mutex::new(Vec::new())),
        }
    }

    /// Returns a snapshot of all dispatched actions.
    pub fn dispatched(&self) -> Vec<(String, bool)> {
        self.dispatched.lock().clone()
    }

    /// Returns the number of actions dispatched.
    pub fn count(&self) -> usize {
        self.dispatched.lock().len()
    }
}

impl Default for NoopActionDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ActionDispatcher for NoopActionDispatcher {
    async fn dispatch(&self, action_prompt: &str, promote_to_chat: bool) -> anyhow::Result<()> {
        self.dispatched
            .lock()
            .push((action_prompt.to_string(), promote_to_chat));
        Ok(())
    }
}

/// A parsed natural-language trigger request.
///
/// Produced by [`parse_trigger_request`], this holds the condition and action
/// extracted from a user request like "when $HOME/build.done exists, run cargo
/// test".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTriggerRequest {
    /// The condition part (e.g., "$HOME/build.done exists").
    pub condition: String,
    /// The action part (e.g., "run cargo test").
    pub action: String,
}

/// Parses a natural-language trigger request into condition and action.
///
/// Supports the following patterns:
///
/// - `"when <condition>, <action>"` or `"when <condition> then <action>"`
/// - `"if <condition>, <action>"` or `"if <condition> then <action>"`
/// - `"<condition> -> <action>"`
///
/// If no delimiter is found, the entire input is treated as the condition
/// and the action is left empty (which will be rejected by the engine).
///
/// # Examples
///
/// ```
/// # use ragent_agent::trigger::dynamic::parse_trigger_request;
/// let parsed = parse_trigger_request("when build.done exists, run cargo test").unwrap();
/// assert_eq!(parsed.condition, "build.done exists");
/// assert_eq!(parsed.action, "run cargo test");
/// ```
pub fn parse_trigger_request(input: &str) -> Result<ParsedTriggerRequest, DynamicTriggerError> {
    let input = input.trim();

    // Strip leading "when " or "if " keyword.
    let body = if let Some(rest) = input.strip_prefix("when ") {
        rest
    } else if let Some(rest) = input.strip_prefix("if ") {
        rest
    } else {
        input
    };

    // Try comma delimiter first: "condition, action"
    if let Some(idx) = body.find(',') {
        let condition = body[..idx].trim().to_string();
        let action = body[idx + 1..].trim().to_string();
        if condition.is_empty() || action.is_empty() {
            return Err(DynamicTriggerError::ParseFailed(
                "condition or action is empty".to_string(),
            ));
        }
        return Ok(ParsedTriggerRequest { condition, action });
    }

    // Try " then " delimiter: "condition then action"
    if let Some(idx) = body.to_lowercase().find(" then ") {
        let condition = body[..idx].trim().to_string();
        let action = body[idx + 5..].trim().to_string();
        if condition.is_empty() || action.is_empty() {
            return Err(DynamicTriggerError::ParseFailed(
                "condition or action is empty".to_string(),
            ));
        }
        return Ok(ParsedTriggerRequest { condition, action });
    }

    // Try " -> " delimiter: "condition -> action"
    if let Some(idx) = body.find(" -> ") {
        let condition = body[..idx].trim().to_string();
        let action = body[idx + 4..].trim().to_string();
        if condition.is_empty() || action.is_empty() {
            return Err(DynamicTriggerError::ParseFailed(
                "condition or action is empty".to_string(),
            ));
        }
        return Ok(ParsedTriggerRequest { condition, action });
    }

    // No delimiter found — treat entire input as condition, empty action.
    Err(DynamicTriggerError::ParseFailed(format!(
        "no condition/action delimiter found in: {input}"
    )))
}

/// The dynamic trigger rule engine.
///
/// Manages session-scoped trigger rules, polls their conditions on a
/// configurable interval, and dispatches actions when conditions match.
///
/// Thread-safe via `Arc`. The engine holds a reference to the
/// [`TriggerRuntime`] (which provides dedup and cycle suppression) and
/// delegates condition evaluation to a [`ConditionEvaluator`] and action
/// dispatch to an [`ActionDispatcher`].
pub struct DynamicTriggerEngine {
    /// The trigger runtime (shared with the session).
    runtime: TriggerRuntime,
    /// Configuration (poll interval, max rules, feature gate).
    config: TriggerConfig,
    /// Condition evaluator (LLM-backed in production, simple in tests).
    evaluator: Arc<dyn ConditionEvaluator>,
    /// Action dispatcher (sub-agent in production, recording in tests).
    dispatcher: Arc<dyn ActionDispatcher>,
}

impl DynamicTriggerEngine {
    /// Creates a new dynamic trigger engine.
    pub fn new(
        runtime: TriggerRuntime,
        config: TriggerConfig,
        evaluator: Arc<dyn ConditionEvaluator>,
        dispatcher: Arc<dyn ActionDispatcher>,
    ) -> Self {
        Self {
            runtime,
            config,
            evaluator,
            dispatcher,
        }
    }

    /// Returns `true` if the trigger system is enabled in config.
    pub fn is_enabled(&self) -> bool {
        self.config.is_enabled()
    }

    /// Returns the configured poll interval.
    pub fn poll_interval(&self) -> Duration {
        self.config.poll_interval()
    }

    /// Creates a trigger rule from a natural-language request and registers
    /// it with the runtime.
    ///
    /// The request is parsed into condition + action via
    /// [`parse_trigger_request`]. The rule is fire-once by default unless
    /// `repeating` is `true`.
    ///
    /// # Errors
    ///
    /// Returns [`DynamicTriggerError::Disabled`] if the trigger system is
    /// disabled, [`DynamicTriggerError::MaxRulesReached`] if the session has
    /// reached the configured rule limit, or [`DynamicTriggerError::ParseFailed`]
    /// if the request cannot be parsed.
    pub fn create_rule(
        &self,
        request: &str,
        repeating: bool,
        promote_to_chat: bool,
    ) -> Result<TriggerRuleId, DynamicTriggerError> {
        if !self.config.is_enabled() {
            return Err(DynamicTriggerError::Disabled);
        }

        if self.runtime.rule_count() >= self.config.max_rules {
            return Err(DynamicTriggerError::MaxRulesReached {
                max: self.config.max_rules,
            });
        }

        let parsed = parse_trigger_request(request)?;

        let mut rule = TriggerRule::new(&parsed.condition, &parsed.action);
        rule.fire_once = !repeating;
        rule.promote_to_chat = promote_to_chat;

        let id = self.runtime.add_rule(rule);
        debug!(rule_id = %id, repeating, promote_to_chat, "Dynamic trigger rule created");
        Ok(id)
    }

    /// Creates a trigger rule from an already-parsed request. This is used
    /// when restoring rules from a persisted session.
    pub fn create_rule_from_parts(
        &self,
        condition: &str,
        action: &str,
        repeating: bool,
        promote_to_chat: bool,
    ) -> Result<TriggerRuleId, DynamicTriggerError> {
        if !self.config.is_enabled() {
            return Err(DynamicTriggerError::Disabled);
        }

        if self.runtime.rule_count() >= self.config.max_rules {
            return Err(DynamicTriggerError::MaxRulesReached {
                max: self.config.max_rules,
            });
        }

        let mut rule = TriggerRule::new(condition, action);
        rule.fire_once = !repeating;
        rule.promote_to_chat = promote_to_chat;

        let id = self.runtime.add_rule(rule);
        debug!(rule_id = %id, "Dynamic trigger rule created from parts");
        Ok(id)
    }

    /// Polls all active rules, evaluating their conditions and dispatching
    /// actions for any that match.
    ///
    /// This is the main entry point for the polling loop. It should be called
    /// on each tick of the configured poll interval.
    ///
    /// For fire-once rules that have already fired, or disabled rules, the
    /// condition is skipped.
    ///
    /// Returns the number of rules that fired during this poll cycle.
    pub async fn poll_once(&self) -> usize {
        if !self.config.is_enabled() {
            return 0;
        }

        let rules = self.runtime.list_rules();
        let mut fired_count = 0;

        for rule in rules {
            // Skip disabled rules.
            if !rule.enabled {
                continue;
            }

            // Skip fire-once rules that have already fired.
            if rule.fire_once && rule.fired_at.is_some() {
                continue;
            }

            // Evaluate the condition.
            let matched = self.evaluator.evaluate(&rule.condition).await;
            if !matched {
                continue;
            }

            // Create a trigger envelope and submit to runtime for dedup/cycle.
            let envelope = TriggerEnvelope::new(
                TriggerSourceKind::Dynamic,
                rule.id.as_str(),
                &rule.condition,
                &rule.action,
                TriggerActionKind::SubAgent,
                rule.promote_to_chat,
            );

            let fired = self.runtime.process(envelope);

            if let Some(fired) = fired {
                // Dispatch the action.
                if let Err(e) = self
                    .dispatcher
                    .dispatch(
                        &fired.envelope.action_prompt,
                        fired.envelope.promote_to_chat,
                    )
                    .await
                {
                    warn!(rule_id = %rule.id, error = %e, "Failed to dispatch trigger action");
                } else {
                    fired_count += 1;
                    debug!(rule_id = %rule.id, "Dynamic trigger rule fired and dispatched");
                }
            }
        }

        if fired_count > 0 {
            debug!(fired_count, "Poll cycle completed");
        }

        fired_count
    }

    /// Returns the current trigger runtime configuration.
    pub fn runtime_config(&self) -> &TriggerRuntimeConfig {
        &self.runtime.config
    }

    /// Returns a reference to the underlying trigger runtime.
    pub fn runtime(&self) -> &TriggerRuntime {
        &self.runtime
    }

    /// Returns the trigger configuration.
    pub fn config(&self) -> &TriggerConfig {
        &self.config
    }

    /// Serializes all registered rules to JSON for session persistence.
    ///
    /// This is used to persist trigger rules alongside the active session
    /// and restore them on resume (FR-002).
    pub fn serialize_rules(&self) -> anyhow::Result<String> {
        let rules = self.runtime.list_rules();
        Ok(serde_json::to_string_pretty(&rules)?)
    }

    /// Restores rules from a JSON string produced by [`serialize_rules`].
    ///
    /// Clears existing rules first, then deserializes and registers each
    /// restored rule. Used when resuming a session (FR-002).
    pub fn restore_rules(&self, json: &str) -> anyhow::Result<usize> {
        let rules: Vec<TriggerRule> = serde_json::from_str(json)?;

        // Clear existing rules before restoring.
        self.runtime.clear();

        let count = rules.len();
        for rule in rules {
            self.runtime.add_rule(rule);
        }

        debug!(restored = count, "Dynamic trigger rules restored");
        Ok(count)
    }

    /// Returns the status of a rule, or `None` if not found.
    pub fn rule_status(&self, rule_id: &str) -> Option<TriggerRuleStatus> {
        self.runtime.get_rule(rule_id).map(|r| r.status())
    }

    /// Removes a rule by ID. Returns `true` if the rule was found and removed.
    pub fn remove_rule(&self, rule_id: &str) -> bool {
        self.runtime.remove_rule(rule_id)
    }

    /// Enables a rule by ID. Returns `true` if the rule was found.
    pub fn enable_rule(&self, rule_id: &str) -> bool {
        self.runtime.enable_rule(rule_id)
    }

    /// Disables a rule by ID. Returns `true` if the rule was found.
    pub fn disable_rule(&self, rule_id: &str) -> bool {
        self.runtime.disable_rule(rule_id)
    }

    /// Returns a snapshot of all registered rules.
    pub fn list_rules(&self) -> Vec<TriggerRule> {
        self.runtime.list_rules()
    }

    /// Returns the number of registered rules.
    pub fn rule_count(&self) -> usize {
        self.runtime.rule_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_when_comma() {
        let parsed = parse_trigger_request("when build.done exists, run cargo test").unwrap();
        assert_eq!(parsed.condition, "build.done exists");
        assert_eq!(parsed.action, "run cargo test");
    }

    #[test]
    fn test_parse_if_comma() {
        let parsed = parse_trigger_request("if tests pass, deploy to staging").unwrap();
        assert_eq!(parsed.condition, "tests pass");
        assert_eq!(parsed.action, "deploy to staging");
    }

    #[test]
    fn test_parse_then_delimiter() {
        let parsed = parse_trigger_request("when file exists then run tests").unwrap();
        assert_eq!(parsed.condition, "file exists");
        assert_eq!(parsed.action, "run tests");
    }

    #[test]
    fn test_parse_arrow_delimiter() {
        let parsed = parse_trigger_request("file exists -> run tests").unwrap();
        assert_eq!(parsed.condition, "file exists");
        assert_eq!(parsed.action, "run tests");
    }

    #[test]
    fn test_parse_no_delimiter_fails() {
        assert!(parse_trigger_request("just a condition").is_err());
    }

    #[test]
    fn test_parse_empty_condition_fails() {
        assert!(parse_trigger_request("when , do something").is_err());
    }

    #[test]
    fn test_parse_empty_action_fails() {
        assert!(parse_trigger_request("when something, ").is_err());
    }

    #[test]
    fn test_simple_evaluator_matches() {
        let eval = SimpleConditionEvaluator::new();
        eval.add_matching("file exists");
        assert!(eval.matches("file exists"));
        assert!(!eval.matches("file does not exist"));
    }

    #[tokio::test]
    async fn test_noop_dispatcher_records() {
        let dispatcher = NoopActionDispatcher::new();
        dispatcher.dispatch("run tests", false).await.unwrap();
        dispatcher.dispatch("deploy", true).await.unwrap();
        assert_eq!(dispatcher.count(), 2);
        let dispatched = dispatcher.dispatched();
        assert_eq!(dispatched[0], ("run tests".to_string(), false));
        assert_eq!(dispatched[1], ("deploy".to_string(), true));
    }
}
