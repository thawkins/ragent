#![allow(clippy::assert_is_empty)]
//! Integration tests for dynamic trigger rules (spec `piegap` FR-002 / T-002).
//!
//! These tests verify the dynamic trigger engine's public API: parsing,
//! rule creation, polling, fire-once vs repeating, persistence, configuration,
//! and promote_to_chat propagation. No real LLM or sub-agent is required —
//! `SimpleConditionEvaluator` and `NoopActionDispatcher` stand in for those
//! dependencies.

use std::sync::Arc;
use std::time::Duration;

use ragent_agent::trigger::dynamic::{
    DynamicTriggerEngine, DynamicTriggerError, NoopActionDispatcher, SimpleConditionEvaluator,
    parse_trigger_request,
};
use ragent_agent::trigger::runtime::{TriggerRuntime, TriggerRuntimeConfig};
use ragent_config::TriggerConfig;
use ragent_types::trigger::TriggerRuleStatus;

// ── Helper: build a default engine with the given evaluator and dispatcher ──

fn make_engine(
    evaluator: Arc<SimpleConditionEvaluator>,
    dispatcher: Arc<NoopActionDispatcher>,
) -> DynamicTriggerEngine {
    let runtime = TriggerRuntime::new(TriggerRuntimeConfig {
        dedup_window: Duration::from_secs(0), // no dedup so repeating rules can fire
        max_cycles: 100,
    });
    let config = TriggerConfig::default();
    DynamicTriggerEngine::new(runtime, config, evaluator, dispatcher)
}

fn make_engine_with_config(
    config: TriggerConfig,
    evaluator: Arc<SimpleConditionEvaluator>,
    dispatcher: Arc<NoopActionDispatcher>,
) -> DynamicTriggerEngine {
    let runtime = TriggerRuntime::new(TriggerRuntimeConfig {
        dedup_window: Duration::from_secs(0),
        max_cycles: 100,
    });
    DynamicTriggerEngine::new(runtime, config, evaluator, dispatcher)
}

// ── Parsing tests ────────────────────────────────────────��─────────────────

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

// ��─ Rule creation tests ────────────────────────────────────────────────────

#[test]
fn test_create_rule_from_request() {
    let evaluator = Arc::new(SimpleConditionEvaluator::new());
    let dispatcher = Arc::new(NoopActionDispatcher::new());
    let engine = make_engine(evaluator, dispatcher);

    let id = engine
        .create_rule("when file exists, run tests", false, false)
        .unwrap();
    assert_eq!(engine.rule_count(), 1);

    let rule = engine.list_rules().into_iter().next().unwrap();
    assert_eq!(rule.condition, "file exists");
    assert_eq!(rule.action, "run tests");
    assert!(rule.fire_once);
    assert!(rule.enabled);
    assert!(!rule.promote_to_chat);
    assert_eq!(rule.status(), TriggerRuleStatus::Active);

    // The rule ID should match.
    assert_eq!(rule.id.as_str(), id.as_str());
}

#[test]
fn test_create_repeating_rule() {
    let evaluator = Arc::new(SimpleConditionEvaluator::new());
    let dispatcher = Arc::new(NoopActionDispatcher::new());
    let engine = make_engine(evaluator, dispatcher);

    engine
        .create_rule("when file exists, run tests", true, false)
        .unwrap();
    let rule = engine.list_rules().into_iter().next().unwrap();
    assert!(!rule.fire_once); // repeating → fire_once = false
}

#[test]
fn test_create_rule_promote_to_chat() {
    let evaluator = Arc::new(SimpleConditionEvaluator::new());
    let dispatcher = Arc::new(NoopActionDispatcher::new());
    let engine = make_engine(evaluator, dispatcher);

    engine
        .create_rule("when file exists, run tests", false, true)
        .unwrap();
    let rule = engine.list_rules().into_iter().next().unwrap();
    assert!(rule.promote_to_chat);
}

#[test]
fn test_create_rule_disabled_config() {
    let evaluator = Arc::new(SimpleConditionEvaluator::new());
    let dispatcher = Arc::new(NoopActionDispatcher::new());
    let config = TriggerConfig {
        enabled: false,
        poll_interval_secs: 30,
        max_rules: 32,
    };
    let engine = make_engine_with_config(config, evaluator, dispatcher);

    let result = engine.create_rule("when file exists, run tests", false, false);
    assert!(matches!(result, Err(DynamicTriggerError::Disabled)));
    assert_eq!(engine.rule_count(), 0);
}

#[test]
fn test_create_rule_max_rules_reached() {
    let evaluator = Arc::new(SimpleConditionEvaluator::new());
    let dispatcher = Arc::new(NoopActionDispatcher::new());
    let config = TriggerConfig {
        enabled: true,
        poll_interval_secs: 30,
        max_rules: 2,
    };
    let engine = make_engine_with_config(config, evaluator, dispatcher);

    engine.create_rule("when a, do b", false, false).unwrap();
    engine.create_rule("when c, do d", false, false).unwrap();
    assert_eq!(engine.rule_count(), 2);

    let result = engine.create_rule("when e, do f", false, false);
    assert!(matches!(
        result,
        Err(DynamicTriggerError::MaxRulesReached { max: 2 })
    ));
}

#[test]
fn test_create_rule_parse_failed() {
    let evaluator = Arc::new(SimpleConditionEvaluator::new());
    let dispatcher = Arc::new(NoopActionDispatcher::new());
    let engine = make_engine(evaluator, dispatcher);

    let result = engine.create_rule("no delimiter here", false, false);
    assert!(matches!(result, Err(DynamicTriggerError::ParseFailed(_))));
}

// ── Polling tests ──────────────────────────────────────────────────────────

#[tokio::test]
async fn test_poll_fires_matching_rule() {
    let evaluator = Arc::new(SimpleConditionEvaluator::new());
    evaluator.add_matching("file exists");
    let dispatcher = Arc::new(NoopActionDispatcher::new());
    let engine = make_engine(evaluator.clone(), dispatcher.clone());

    engine
        .create_rule("when file exists, run tests", false, false)
        .unwrap();

    let fired = engine.poll_once().await;
    assert_eq!(fired, 1);
    assert_eq!(dispatcher.count(), 1);
    assert_eq!(dispatcher.dispatched()[0].0, "run tests");
    assert!(!dispatcher.dispatched()[0].1);
}

#[tokio::test]
async fn test_poll_skips_non_matching_rule() {
    let evaluator = Arc::new(SimpleConditionEvaluator::new());
    // No matching conditions added — nothing will match.
    let dispatcher = Arc::new(NoopActionDispatcher::new());
    let engine = make_engine(evaluator, dispatcher.clone());

    engine
        .create_rule("when file exists, run tests", false, false)
        .unwrap();

    let fired = engine.poll_once().await;
    assert_eq!(fired, 0);
    assert_eq!(dispatcher.count(), 0);
}

#[tokio::test]
async fn test_poll_skips_disabled_rule() {
    let evaluator = Arc::new(SimpleConditionEvaluator::new());
    evaluator.add_matching("file exists");
    let dispatcher = Arc::new(NoopActionDispatcher::new());
    let engine = make_engine(evaluator, dispatcher.clone());

    let id = engine
        .create_rule("when file exists, run tests", false, false)
        .unwrap();
    engine.disable_rule(id.as_str());

    let fired = engine.poll_once().await;
    assert_eq!(fired, 0);
    assert_eq!(dispatcher.count(), 0);
}

#[tokio::test]
async fn test_fire_once_rule_does_not_refire() {
    let evaluator = Arc::new(SimpleConditionEvaluator::new());
    evaluator.add_matching("file exists");
    let dispatcher = Arc::new(NoopActionDispatcher::new());
    let engine = make_engine(evaluator, dispatcher.clone());

    let id = engine
        .create_rule("when file exists, run tests", false, false)
        .unwrap();

    // First poll fires.
    let fired = engine.poll_once().await;
    assert_eq!(fired, 1);
    assert_eq!(dispatcher.count(), 1);

    // Rule should now be in Fired status.
    assert_eq!(
        engine.rule_status(id.as_str()),
        Some(TriggerRuleStatus::Fired)
    );

    // Second poll should not refire (fire-once rule).
    let fired = engine.poll_once().await;
    assert_eq!(fired, 0);
    assert_eq!(dispatcher.count(), 1); // unchanged
}

#[tokio::test]
async fn test_repeating_rule_refires() {
    let evaluator = Arc::new(SimpleConditionEvaluator::new());
    evaluator.add_matching("file exists");
    let dispatcher = Arc::new(NoopActionDispatcher::new());
    let engine = make_engine(evaluator, dispatcher.clone());

    engine
        .create_rule("when file exists, run tests", true, false)
        .unwrap();

    // First poll fires.
    let fired = engine.poll_once().await;
    assert_eq!(fired, 1);
    assert_eq!(dispatcher.count(), 1);

    // Second poll fires again (repeating rule).
    let fired = engine.poll_once().await;
    assert_eq!(fired, 1);
    assert_eq!(dispatcher.count(), 2);
}

#[tokio::test]
async fn test_poll_promote_to_chat_propagated() {
    let evaluator = Arc::new(SimpleConditionEvaluator::new());
    evaluator.add_matching("file exists");
    let dispatcher = Arc::new(NoopActionDispatcher::new());
    let engine = make_engine(evaluator, dispatcher.clone());

    engine
        .create_rule("when file exists, run tests", false, true)
        .unwrap();

    let fired = engine.poll_once().await;
    assert_eq!(fired, 1);
    assert!(dispatcher.dispatched()[0].1); // promote_to_chat = true
}

#[tokio::test]
async fn test_poll_disabled_config_noops() {
    let evaluator = Arc::new(SimpleConditionEvaluator::new());
    evaluator.add_matching("file exists");
    let dispatcher = Arc::new(NoopActionDispatcher::new());
    let config = TriggerConfig {
        enabled: false,
        poll_interval_secs: 30,
        max_rules: 32,
    };
    let engine = make_engine_with_config(config, evaluator, dispatcher.clone());

    // When config is disabled, poll_once returns 0 even if rules were
    // somehow registered (e.g. restored before disabling).
    let fired = engine.poll_once().await;
    assert_eq!(fired, 0);
}

// ── Rule management tests ─────────────────────────────────────────────────

#[test]
fn test_enable_disable_rule() {
    let evaluator = Arc::new(SimpleConditionEvaluator::new());
    let dispatcher = Arc::new(NoopActionDispatcher::new());
    let engine = make_engine(evaluator, dispatcher);

    let id = engine
        .create_rule("when file exists, run tests", false, false)
        .unwrap();

    assert!(engine.disable_rule(id.as_str()));
    assert_eq!(
        engine.rule_status(id.as_str()),
        Some(TriggerRuleStatus::Disabled)
    );

    assert!(engine.enable_rule(id.as_str()));
    assert_eq!(
        engine.rule_status(id.as_str()),
        Some(TriggerRuleStatus::Active)
    );
}

#[test]
fn test_remove_rule() {
    let evaluator = Arc::new(SimpleConditionEvaluator::new());
    let dispatcher = Arc::new(NoopActionDispatcher::new());
    let engine = make_engine(evaluator, dispatcher);

    let id = engine
        .create_rule("when file exists, run tests", false, false)
        .unwrap();
    assert_eq!(engine.rule_count(), 1);

    assert!(engine.remove_rule(id.as_str()));
    assert_eq!(engine.rule_count(), 0);
    assert!(!engine.remove_rule(id.as_str()));
}

// ── Persistence tests ─────────────────────────────────────────────────────

#[test]
fn test_serialize_restore_rules() {
    let evaluator = Arc::new(SimpleConditionEvaluator::new());
    let dispatcher = Arc::new(NoopActionDispatcher::new());
    let engine = make_engine(evaluator, dispatcher);

    engine
        .create_rule("when file exists, run tests", false, false)
        .unwrap();
    engine
        .create_rule("when tests pass, deploy", true, true)
        .unwrap();
    assert_eq!(engine.rule_count(), 2);

    // Serialize.
    let json = engine.serialize_rules().unwrap();
    assert!(json.contains("file exists"));
    assert!(json.contains("tests pass"));

    // Create a fresh engine and restore.
    let evaluator2 = Arc::new(SimpleConditionEvaluator::new());
    let dispatcher2 = Arc::new(NoopActionDispatcher::new());
    let engine2 = make_engine(evaluator2, dispatcher2);
    assert_eq!(engine2.rule_count(), 0);

    let restored = engine2.restore_rules(&json).unwrap();
    assert_eq!(restored, 2);
    assert_eq!(engine2.rule_count(), 2);

    // Verify the restored rules match.
    let rules = engine2.list_rules();
    assert!(rules.iter().any(|r| r.condition == "file exists"));
    assert!(
        rules
            .iter()
            .any(|r| r.condition == "tests pass" && !r.fire_once)
    );
}

#[test]
fn test_restore_rules_clears_existing() {
    let evaluator = Arc::new(SimpleConditionEvaluator::new());
    let dispatcher = Arc::new(NoopActionDispatcher::new());
    let engine = make_engine(evaluator, dispatcher);

    engine.create_rule("when a, do b", false, false).unwrap();
    assert_eq!(engine.rule_count(), 1);

    // Restore from JSON with different rules.
    let json = r#"[
        {
            "id": "test-1",
            "condition": "x exists",
            "action": "do y",
            "fire_once": true,
            "enabled": true,
            "promote_to_chat": false,
            "created_at": "2024-01-01T00:00:00Z",
            "fired_at": null
        }
    ]"#;
    let restored = engine.restore_rules(json).unwrap();
    assert_eq!(restored, 1);
    assert_eq!(engine.rule_count(), 1);

    // The old rule should be gone.
    let rules = engine.list_rules();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0].condition, "x exists");
}

// ── Configuration tests ────────────────────────────────────────────────────

#[test]
fn test_config_poll_interval() {
    let config = TriggerConfig {
        enabled: true,
        poll_interval_secs: 60,
        max_rules: 32,
    };
    assert_eq!(config.poll_interval(), Duration::from_secs(60));
}

#[test]
fn test_config_default_poll_interval() {
    let config = TriggerConfig::default();
    assert_eq!(config.poll_interval(), Duration::from_secs(30));
}

#[test]
fn test_config_disabled() {
    let config = TriggerConfig {
        enabled: false,
        poll_interval_secs: 30,
        max_rules: 32,
    };
    assert!(!config.is_enabled());
}

#[test]
fn test_config_is_empty_at_defaults() {
    let config = TriggerConfig::default();
    assert!(config.is_empty());
}

#[test]
fn test_config_not_empty_when_customized() {
    let config = TriggerConfig {
        enabled: false,
        poll_interval_secs: 30,
        max_rules: 32,
    };
    assert!(!config.is_empty());
}
