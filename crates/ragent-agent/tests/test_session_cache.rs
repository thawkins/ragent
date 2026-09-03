#![allow(clippy::assert_is_empty)]
//! External tests for `tests` from `crates/ragent-agent/src/session/cache.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_agent::session::cache::*;
use ragent_types::ThinkingConfig;

#[test]
fn test_cached_basic() {
    let mut cached: Cached<String> = Cached::new();
    assert!(cached.get(current_cache_version()).is_none());

    cached.set("test".to_string());
    assert_eq!(
        cached.get(current_cache_version()),
        Some("test".to_string())
    );

    invalidate_all_caches();
    assert!(cached.get(current_cache_version()).is_none());
}

#[test]
fn test_session_state_stores_thinking_config() {
    let mut state = SessionState::new("test-session");
    state.set_thinking(ThinkingConfig::off());
    assert_eq!(state.thinking(), &ThinkingConfig::off());
}

#[test]
fn test_session_state_persists_last_reported_input_tokens() {
    let mut state = SessionState::new("test-session");
    assert_eq!(state.last_reported_input_tokens(), 0);
    state.set_last_reported_input_tokens(12345);
    assert_eq!(state.last_reported_input_tokens(), 12345);
    state.clear();
    assert_eq!(state.last_reported_input_tokens(), 0);
}
