#![allow(clippy::assert_is_empty)]
//! External tests for `tests` from `crates/ragent-tools-extended/src/masterfetch/search/tavily.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_tools_extended::masterfetch::search::{SearchEngine, tavily::*};

#[test]
fn test_default_engine_has_empty_key() {
    let engine = TavilyEngine::default();
    assert!(engine.api_key().is_empty());
}

#[test]
fn test_engine_name_is_tavily() {
    let engine = TavilyEngine::new("test");
    assert_eq!(engine.name(), "tavily");
}
