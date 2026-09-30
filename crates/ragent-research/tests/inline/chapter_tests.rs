//! Inline tests for `chapter.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn partition_defaults_to_one_when_count_zero() {
    let plan = partition_topic("impact of GLP-1 drugs", 0);
    assert_eq!(plan.chapters.len(), 1);
}

#[test]
fn partition_clamps_count_to_twelve() {
    let plan = partition_topic("topic", 100);
    assert_eq!(plan.chapters.len(), 12);
}

#[test]
fn partition_empty_topic_produces_numbered_chapters() {
    let plan = partition_topic("", 3);
    assert_eq!(plan.chapters.len(), 3);
    assert_eq!(plan.chapters[0].title, "Chapter 1");
    assert!(plan.chapters[0].queries.is_empty());
}

#[test]
fn partition_uses_segments_when_enough_available() {
    let topic = "background; methods; findings; discussion; conclusion";
    let plan = partition_topic(topic, 3);
    assert_eq!(plan.chapters.len(), 3);
    assert!(plan.chapters[0].title.starts_with("Chapter 1:"));
    assert!(plan.chapters[0].queries.contains(&"background".to_string()));
    assert!(plan.total_queries() >= 5);
}

#[test]
fn partition_falls_back_to_dimensions_for_short_topic() {
    let plan = partition_topic("GLP-1 cardiovascular outcomes", 5);
    assert_eq!(plan.chapters.len(), 5);
    assert!(plan.chapters[0].title.contains("overview and background"));
    assert!(plan.chapters[4].title.contains("implications"));
}

#[test]
fn chapter_plan_collects_titles_and_queries() {
    let plan = partition_topic("a; b; c; d", 2);
    assert_eq!(plan.titles().len(), 2);
    assert_eq!(plan.queries_by_chapter().len(), 2);
}