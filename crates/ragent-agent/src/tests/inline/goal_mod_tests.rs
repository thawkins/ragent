//! Relocated inline tests for `goal/mod.rs` (ANTIPAT M2 test relocation).
//!
//! The body previously lived in an inline `#[cfg(test)] mod tests` in the
//! source file; it is now compiled from this file via a `#[path]` hook.

use super::*;

#[test]
fn test_goal_new() {
    let goal = GoalCondition::new("All tests pass");
    assert_eq!(goal.description, "All tests pass");
    assert!(!goal.satisfied);
    assert_eq!(goal.evaluation_count, 0);
    assert!(goal.last_evaluated.is_none());
}

#[test]
fn test_goal_record_evaluation() {
    let mut goal = GoalCondition::new("Tests pass");
    goal.record_evaluation(true, Some("All 10 tests passed".to_string()));

    assert!(goal.satisfied);
    assert_eq!(goal.evaluation_count, 1);
    assert!(goal.last_evaluated.is_some());
    assert_eq!(goal.last_reasoning, Some("All 10 tests passed".to_string()));
}

#[test]
fn test_goal_summary() {
    let goal = GoalCondition::new("Feature complete");
    let summary = goal.summary();
    assert!(summary.contains("Feature complete"));
    assert!(summary.contains("not satisfied"));
}

#[test]
fn test_parse_evaluation_yes() {
    // Test the parsing logic directly
    let text = "SATISFIED: YES\nCONFIDENCE: 0.95\nREASONING: All tests have passed and the feature is implemented.";

    let mut satisfied = false;
    let mut confidence = None;
    let mut reasoning = String::new();

    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("SATISFIED:") {
            let value = line.strip_prefix("SATISFIED:").unwrap_or("").trim();
            satisfied = value.eq_ignore_ascii_case("YES");
        } else if line.starts_with("CONFIDENCE:") {
            let value = line.strip_prefix("CONFIDENCE:").unwrap_or("").trim();
            confidence = value.parse::<f64>().ok();
        } else if line.starts_with("REASONING:") {
            reasoning = line
                .strip_prefix("REASONING:")
                .unwrap_or("")
                .trim()
                .to_string();
        }
    }

    assert!(satisfied);
    assert_eq!(confidence, Some(0.95));
    assert!(reasoning.contains("All tests"));
}

#[test]
fn test_build_context() {
    let messages = vec![
        Message::new(
            "session-1",
            Role::User,
            vec![MessagePart::Text {
                text: "Run the tests".to_string(),
            }],
        ),
        Message::new(
            "session-1",
            Role::Assistant,
            vec![MessagePart::Text {
                text: "Running cargo test...".to_string(),
            }],
        ),
    ];

    let context = build_evaluation_context(&messages, 1000);
    assert!(context.contains("User"));
    assert!(context.contains("Assistant"));
    assert!(context.contains("Run the tests"));
}
