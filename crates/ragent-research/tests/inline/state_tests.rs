//! Inline tests for `state.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::source::{LocalSourceKind, Source};
use chrono::Utc;
use std::path::PathBuf;

fn web_source(url: &str) -> Source {
    Source::Web {
        published_at: None,
        url: url.to_string(),
        title: "title".to_string(),
        captured_at: Utc::now(),
        body_path: PathBuf::from("sources/web-01.md"),
        body: "body".to_string(),
        relevance: String::new(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".into(),
        language: None,
        oa_recovery: None,
        author: None,
    }
}

#[test]
fn test_state_starts_empty() {
    let state = ResearchState::new("Rust async runtimes");
    assert_eq!(state.plan.topic, "Rust async runtimes");
    assert!(state.plan.sub_questions.is_empty());
    assert!(state.sources.is_empty());
    assert!(state.gaps.is_empty());
    assert_eq!(state.iteration_count, 0);
    assert!(state.evaluation_score.is_none());
}

#[test]
fn test_add_and_update_sub_question() {
    let mut state = ResearchState::new("topic");
    state.add_sub_question("q1", "What is tokio?", 10);
    assert_eq!(state.pending_sub_questions().len(), 1);

    assert!(state.set_sub_question_status("q1", SubQuestionStatus::InProgress));
    assert_eq!(
        state.plan.sub_questions[0].status,
        SubQuestionStatus::InProgress
    );

    assert!(state.set_sub_question_status("q1", SubQuestionStatus::Answered));
    assert!(state.is_complete());
    assert!(!state.set_sub_question_status("missing", SubQuestionStatus::Answered));
}

#[test]
fn test_complete_requires_no_active_gaps() {
    let mut state = ResearchState::new("topic");
    state.add_sub_question("q1", "question", 1);
    state.set_sub_question_status("q1", SubQuestionStatus::Answered);

    state.add_gap("g1", "missing citation", vec!["q1".to_string()]);
    assert!(!state.is_complete());

    assert!(state.resolve_gap("g1"));
    assert!(state.is_complete());
    assert!(!state.resolve_gap("g1"));
}

#[test]
fn test_source_deduplication() {
    let mut state = ResearchState::new("topic");
    state.add_source(web_source("https://example.com/a"));
    state.add_source(web_source("https://example.com/a"));
    assert_eq!(state.sources.len(), 1);
}

#[test]
fn test_counts() {
    let mut state = ResearchState::new("topic");
    state.add_sub_question("q1", "a", 1);
    state.add_sub_question("q2", "b", 1);
    state.set_sub_question_status("q1", SubQuestionStatus::Answered);
    state.add_source(web_source("https://example.com"));
    state.add_gap("g1", "gap", vec![]);
    state.increment_iteration();
    state.set_evaluation_score(72);

    let counts = state.counts();
    assert_eq!(counts.sub_questions_total, 2);
    assert_eq!(counts.sub_questions_answered, 1);
    assert_eq!(counts.sources_total, 1);
    assert_eq!(counts.gaps_total, 1);
    assert_eq!(counts.gaps_active, 1);
    assert_eq!(counts.iteration_count, 1);
    assert_eq!(counts.evaluation_score, Some(72));
    assert_eq!(counts.failed_attempts_total, 0);
}

#[test]
fn test_record_failed_source() {
    let mut state = ResearchState::new("topic");
    state.record_failed_source(Some("https://x.com"), "timeout");
    assert_eq!(state.failed_attempts.len(), 1);
    assert_eq!(
        state.failed_attempts[0].source.as_deref(),
        Some("https://x.com")
    );
    assert_eq!(state.counts().failed_attempts_total, 1);
}
#[test]
fn test_plan_status_counts() {
    let mut plan = ResearchPlan::new("topic");
    plan.sub_questions.push(SubQuestion {
        id: "q1".into(),
        question: "a".into(),
        status: SubQuestionStatus::Answered,
        priority: 1,
    });
    plan.sub_questions.push(SubQuestion {
        id: "q2".into(),
        question: "b".into(),
        status: SubQuestionStatus::Pending,
        priority: 1,
    });
    let counts = plan.status_counts();
    assert_eq!(counts.get("answered"), Some(&1));
    assert_eq!(counts.get("pending"), Some(&1));
}

#[test]
fn test_same_source_local() {
    let a = Source::Local {
        path: "src/lib.rs".to_string(),
        kind: LocalSourceKind::InProject,
        captured_at: Utc::now(),
        body_path: PathBuf::from("sources/local-01.md"),
        relevance: "high".to_string(),
        body: "body".to_string(),
    };
    let b = Source::Local {
        path: "src/lib.rs".to_string(),
        kind: LocalSourceKind::InProject,
        captured_at: Utc::now(),
        body_path: PathBuf::from("sources/local-02.md"),
        relevance: "medium".to_string(),
        body: "other".to_string(),
    };
    assert!(same_source(&a, &b));
}
