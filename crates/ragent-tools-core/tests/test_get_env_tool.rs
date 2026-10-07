//! Tests for the `get_env` tool, covering the value-shape redaction added by
//! audit T-109: a credential stored in a *non*-sensitive-named variable must
//! still be masked.
//!
//! `std::env::set_var` is `unsafe` in Rust 2024, so this target opts back in
//! explicitly; env mutation is contained to this test binary and the variable
//! names are unique to avoid cross-test interference.

#![allow(unsafe_code)]

use std::sync::Arc;

use serde_json::json;

use ragent_tools_core::get_env::GetEnvTool;
use ragent_tools_core::{Tool, ToolContext};
use ragent_types::event::EventBus;

fn make_ctx() -> ToolContext {
    let dir = std::env::current_dir().expect("cwd");
    ToolContext {
        session_id: "test".to_string(),
        working_dir: dir.clone(),
        event_bus: Arc::new(EventBus::new(16)),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: vec![dir],
    }
}

fn set_var(name: &str, value: &str) {
    // SAFETY: the variable name is unique to this test binary; no other
    // thread reads it concurrently.
    unsafe { std::env::set_var(name, value) };
}

async fn read(name: &str) -> String {
    let out = GetEnvTool
        .execute(json!({ "name": name }), &make_ctx())
        .await
        .expect("get_env execute");
    out.content
}

#[tokio::test]
async fn test_value_shaped_secret_redacted_even_with_innocuous_name() {
    // The variable name carries none of KEY/SECRET/TOKEN/PASSWORD, but the
    // value is a key-shaped credential - it must still be masked.
    set_var(
        "RAGENT_TEST_INNOCUOUS_NAME",
        "sk-proj-3f8a2b1c9d0e4f5a6b7c8d9e0f1a2b3c",
    );
    let content = read("RAGENT_TEST_INNOCUOUS_NAME").await;
    assert!(
        !content.contains("sk-proj-3f8a2b1c9d0e4f5a6b7c8d9e0f1a2b3c"),
        "key-shaped value must be redacted regardless of the variable name: {content}"
    );
    assert!(
        content.contains("[REDACTED]"),
        "expected redaction marker: {content}"
    );
}

#[tokio::test]
async fn test_sensitive_name_is_fully_redacted() {
    set_var("RAGENT_TEST_MY_API_KEY", "short");
    let content = read("RAGENT_TEST_MY_API_KEY").await;
    assert!(
        content.contains("***REDACTED***"),
        "sensitive-named variable must be fully redacted: {content}"
    );
}

#[tokio::test]
async fn test_innocuous_name_and_value_returned_verbatim() {
    set_var("RAGENT_TEST_PLAIN_VALUE", "hello world");
    let content = read("RAGENT_TEST_PLAIN_VALUE").await;
    assert_eq!(content, "RAGENT_TEST_PLAIN_VALUE=hello world");
}

#[tokio::test]
async fn test_missing_variable_reports_not_set() {
    let content = read("RAGENT_TEST_DEFINITELY_NOT_SET_9f3a7c").await;
    assert_eq!(content, "RAGENT_TEST_DEFINITELY_NOT_SET_9f3a7c=(not set)");
}
