//! Inline tests for `planner.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[tokio::test]
async fn heuristic_plan_returns_sub_questions() {
    let planner = HeuristicPlanner::new();
    let plan = planner.plan("Rust async runtimes").await.unwrap();
    assert!(!plan.sub_questions.is_empty());
    assert_eq!(plan.topic, "Rust async runtimes");
    assert!(
        plan.sub_questions
            .iter()
            .any(|sq| sq.question.contains("Rust async runtimes"))
    );
}

#[tokio::test]
async fn heuristic_plan_empty_topic_fails() {
    let planner = HeuristicPlanner::new();
    assert!(planner.plan("   ").await.is_err());
}

#[tokio::test]
async fn heuristic_plan_splits_clauses() {
    let planner = HeuristicPlanner::new();
    let plan = planner
        .plan("tokio, async-std, and smol async runtimes")
        .await
        .unwrap();
    let ids: Vec<_> = plan.sub_questions.iter().map(|sq| &sq.id).collect();
    let unique: std::collections::HashSet<_> = ids.iter().copied().collect();
    assert_eq!(ids.len(), unique.len());
}

#[test]
fn parse_llm_questions_extracts_json_array() {
    let json =
        r#"[{"question":"What is X?","priority":9},{"question":"How does Y work?","priority":7}]"#;
    let qs = parse_llm_questions(json, "topic").unwrap();
    assert_eq!(qs.len(), 2);
    assert_eq!(qs[0].question, "What is X?");
    assert_eq!(qs[0].priority, 9);
}

#[test]
fn parse_llm_questions_handles_markdown_fence() {
    let text = "```json\n[{\"question\":\"Q1\",\"priority\":5}]\n```";
    let qs = parse_llm_questions(text, "topic").unwrap();
    assert_eq!(qs.len(), 1);
}

#[test]
fn parse_llm_questions_defaults_empty_priority() {
    let json = r#"[{"question":"Q1"}]"#;
    let qs = parse_llm_questions(json, "topic").unwrap();
    assert_eq!(qs[0].priority, 5);
}
