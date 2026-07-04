//! Integration tests for `ragent-prompt_opt` prompt-optimization API.
//!
//! Covers `optimize()`, `system_prompt()`, and `OptMethod::from_str` alias
//! resolution. These were relocated from the inline `#[cfg(test)]` module in
//! `src/lib.rs` (T-003 of the testconsolidate spec).

use std::str::FromStr;

use async_trait::async_trait;

use ragent_prompt_opt::{Completer, OptMethod, optimize, system_prompt};

/// A test-only Completer that echoes system+user for deterministic assertions.
struct MockCompleter;

#[async_trait]
impl Completer for MockCompleter {
    async fn complete(&self, system: &str, user: &str) -> anyhow::Result<String> {
        Ok(format!("[system:{} chars] [user:{}]", system.len(), user))
    }
}

#[tokio::test]
async fn test_optimize_returns_result() {
    let c = MockCompleter;
    let out = optimize(OptMethod::CoStar, "Write a blog post", &c)
        .await
        .unwrap();
    assert!(out.contains("[system:"));
    assert!(out.contains("Write a blog post"));
}

#[tokio::test]
async fn test_system_prompt_non_empty() {
    for method in OptMethod::all() {
        let sp = system_prompt(*method);
        assert!(!sp.is_empty(), "{} system prompt is empty", method.name());
    }
}

#[test]
fn test_from_str_aliases() {
    assert_eq!(OptMethod::from_str("costar").ok(), Some(OptMethod::CoStar));
    assert_eq!(OptMethod::from_str("co-star").ok(), Some(OptMethod::CoStar));
    assert_eq!(OptMethod::from_str("CO_STAR").ok(), Some(OptMethod::CoStar));
    assert_eq!(
        OptMethod::from_str("cot").ok(),
        Some(OptMethod::ChainOfThought)
    );
    assert_eq!(
        OptMethod::from_str("chain-of-thought").ok(),
        Some(OptMethod::ChainOfThought)
    );
    assert_eq!(
        OptMethod::from_str("azure").ok(),
        Some(OptMethod::Microsoft)
    );
    assert_eq!(OptMethod::from_str("ms").ok(), Some(OptMethod::Microsoft));
    assert_eq!(OptMethod::from_str("q*").ok(), Some(OptMethod::QStar));
    assert_eq!(OptMethod::from_str("o1").ok(), Some(OptMethod::O1Style));
    assert_eq!(OptMethod::from_str("badname").ok(), None::<OptMethod>);
}

#[test]
fn test_help_table_contains_all_methods() {
    let table = OptMethod::help_table();
    for method in OptMethod::all() {
        assert!(
            table.contains(method.name()),
            "help table missing {}",
            method.name()
        );
    }
}
