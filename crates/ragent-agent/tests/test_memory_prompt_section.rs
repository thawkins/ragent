#![allow(clippy::assert_is_empty)]
//! Tests for token-budget memory injection into the system prompt.

use std::path::PathBuf;

use ragent_agent::agent::build_memory_prompt_section;
use ragent_config::{Config, MemoryConfig, config::RetrievalConfig};
use ragent_storage::Storage;

fn make_storage_with_memories(dir: &std::path::Path, count: usize, content_len: usize) -> Storage {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    let project = dir.to_string_lossy().to_string();
    for i in 0..count {
        let content = format!("memory-{i}-{}", "x".repeat(content_len));
        storage
            .create_memory(&content, "fact", "test", 0.7, &project, "", &[])
            .expect("create memory");
    }
    storage
}

fn make_config(max_rows: usize, max_tokens: Option<usize>) -> Config {
    let mut config = Config::default();
    config.memory = MemoryConfig {
        enabled: true,
        tier: "structured".to_string(),
        structured: Default::default(),
        semantic: Default::default(),
        retrieval: RetrievalConfig {
            max_memories_per_prompt: max_rows,
            max_memory_tokens: max_tokens,
            recency_weight: 0.3,
            relevance_weight: 0.7,
        },
        auto_extract: Default::default(),
        decay: Default::default(),
        cross_project: Default::default(),
    };
    config
}

#[test]
fn test_default_budget_includes_many_memories() {
    let dir = PathBuf::from("/tmp/test-memory-budget");
    let storage = make_storage_with_memories(&dir, 150, 200);
    let config = make_config(200, Some(4_000));

    let section = build_memory_prompt_section(&dir, Some(&storage), Some(&config.memory));

    assert!(
        section.contains("## Relevant Memories"),
        "section should have header: {section}"
    );
    let kept = section
        .lines()
        .filter(|l| l.starts_with("- [fact]"))
        .count();
    assert!(
        kept > 5,
        "default 4000-token budget should include more than the old 5-memory cap; got {kept}\n{section}"
    );
    assert!(
        section.contains("omitted"),
        "when rows exceed the budget a truncation note should be present: {section}"
    );
}

#[test]
fn test_small_budget_caps_injection() {
    let dir = PathBuf::from("/tmp/test-memory-small-budget");
    let storage = make_storage_with_memories(&dir, 10, 200);
    let config = make_config(100, Some(50));

    let section = build_memory_prompt_section(&dir, Some(&storage), Some(&config.memory));
    let kept = section
        .lines()
        .filter(|l| l.starts_with("- [fact]"))
        .count();

    assert_eq!(
        kept, 0,
        "a 50-token budget cannot fit a 200-char memory entry after the header; got {kept}\n{section}"
    );
}

#[test]
fn test_row_cap_limits_fetch_when_budget_disabled() {
    let dir = PathBuf::from("/tmp/test-memory-row-cap");
    let storage = make_storage_with_memories(&dir, 20, 10);
    let config = make_config(3, None);

    let section = build_memory_prompt_section(&dir, Some(&storage), Some(&config.memory));
    let kept = section
        .lines()
        .filter(|l| l.starts_with("- [fact]"))
        .count();

    assert_eq!(
        kept, 3,
        "when max_memory_tokens is None, max_memories_per_prompt should be the hard cap; got {kept}\n{section}"
    );
    assert!(
        !section.contains("omitted"),
        "count-cap path should not emit a token-budget truncation note: {section}"
    );
}

#[test]
fn test_infinite_budget_includes_all_rows() {
    let dir = PathBuf::from("/tmp/test-memory-infinite");
    let storage = make_storage_with_memories(&dir, 7, 20);
    let config = make_config(100, None);

    let section = build_memory_prompt_section(&dir, Some(&storage), Some(&config.memory));
    let kept = section
        .lines()
        .filter(|l| l.starts_with("- [fact]"))
        .count();

    assert_eq!(
        kept, 7,
        "with max_memory_tokens=None and max_memories_per_prompt=100, all 7 rows should be injected; got {kept}\n{section}"
    );
}
