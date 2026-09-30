//! Inline tests for `dynamic.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

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
