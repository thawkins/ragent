#![allow(clippy::assert_is_empty)]
//! Regression tests for C-001: SSRF guard on legacy `http_request` and `webfetch` tools.
//!
//! Verifies that both tools reject private IP ranges, localhost, cloud metadata,
//! DNS-rebinding services, alternate IP notations, and non-HTTP(S) schemes
//! before issuing any outbound request.

use ragent_tools_extended::http_request::HttpRequestTool;
use ragent_tools_extended::webfetch::WebFetchTool;
use ragent_tools_extended::{Tool, ToolContext};
use serde_json::json;
use std::sync::Arc;

fn ctx() -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: std::env::temp_dir(),
        event_bus: Arc::new(ragent_types::event::EventBus::new(64)),
        storage: None,
        code_index: None,
        config: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    }
}

#[tokio::test]
async fn http_request_rejects_loopback_ipv4() {
    let tool = HttpRequestTool;
    let out = tool
        .execute(json!({"url": "http://127.0.0.1/"}), &ctx())
        .await;
    assert!(
        out.is_err(),
        "expected loopback IPv4 to be rejected: {out:?}"
    );
    let err = out.unwrap_err().to_string();
    assert!(
        err.contains("URL failed security validation") || err.contains("127.0.0.1"),
        "error should mention SSRF validation: {err}"
    );
}

#[tokio::test]
async fn http_request_rejects_localhost() {
    let tool = HttpRequestTool;
    let out = tool
        .execute(json!({"url": "http://localhost/admin"}), &ctx())
        .await;
    assert!(out.is_err(), "expected localhost to be rejected: {out:?}");
}

#[tokio::test]
async fn http_request_rejects_cloud_metadata() {
    let tool = HttpRequestTool;
    let out = tool
        .execute(
            json!({"url": "http://169.254.169.254/latest/meta-data/"}),
            &ctx(),
        )
        .await;
    assert!(
        out.is_err(),
        "expected cloud metadata IP to be rejected: {out:?}"
    );
}

#[tokio::test]
async fn http_request_rejects_private_ipv4() {
    let tool = HttpRequestTool;
    let out = tool
        .execute(json!({"url": "http://10.0.0.1/"}), &ctx())
        .await;
    assert!(
        out.is_err(),
        "expected private IPv4 to be rejected: {out:?}"
    );
}

#[tokio::test]
async fn http_request_rejects_alternate_ip_notation() {
    let tool = HttpRequestTool;
    // 0177.0.0.1 is octal for 127.0.0.1.
    let out = tool
        .execute(json!({"url": "http://0177.0.0.1/"}), &ctx())
        .await;
    assert!(
        out.is_err(),
        "expected alternate-notation loopback to be rejected: {out:?}"
    );
}

#[tokio::test]
async fn http_request_rejects_file_scheme() {
    let tool = HttpRequestTool;
    let out = tool
        .execute(json!({"url": "file:///etc/passwd"}), &ctx())
        .await;
    assert!(
        out.is_err(),
        "expected file:// scheme to be rejected: {out:?}"
    );
}

#[tokio::test]
async fn webfetch_rejects_loopback_ipv4() {
    let tool = WebFetchTool;
    let out = tool
        .execute(json!({"url": "http://127.0.0.1/"}), &ctx())
        .await;
    assert!(
        out.is_err(),
        "expected loopback IPv4 to be rejected: {out:?}"
    );
}

#[tokio::test]
async fn webfetch_rejects_localhost() {
    let tool = WebFetchTool;
    let out = tool
        .execute(json!({"url": "http://localhost/admin"}), &ctx())
        .await;
    assert!(out.is_err(), "expected localhost to be rejected: {out:?}");
}

#[tokio::test]
async fn webfetch_rejects_cloud_metadata() {
    let tool = WebFetchTool;
    let out = tool
        .execute(
            json!({"url": "http://169.254.169.254/latest/meta-data/"}),
            &ctx(),
        )
        .await;
    assert!(
        out.is_err(),
        "expected cloud metadata IP to be rejected: {out:?}"
    );
}

#[tokio::test]
async fn webfetch_rejects_private_ipv4() {
    let tool = WebFetchTool;
    let out = tool
        .execute(json!({"url": "http://192.168.1.1/"}), &ctx())
        .await;
    assert!(
        out.is_err(),
        "expected private IPv4 to be rejected: {out:?}"
    );
}

#[tokio::test]
async fn webfetch_rejects_dns_rebinding() {
    let tool = WebFetchTool;
    let out = tool
        .execute(json!({"url": "http://1.1.1.1.nip.io/"}), &ctx())
        .await;
    assert!(
        out.is_err(),
        "expected DNS rebinding suffix to be rejected: {out:?}"
    );
}

#[tokio::test]
async fn webfetch_rejects_file_scheme() {
    let tool = WebFetchTool;
    let out = tool
        .execute(json!({"url": "file:///etc/passwd"}), &ctx())
        .await;
    assert!(
        out.is_err(),
        "expected file:// scheme to be rejected: {out:?}"
    );
}

#[tokio::test]
async fn http_request_allows_public_url() {
    let tool = HttpRequestTool;
    // Validation should pass; the request itself may fail due to no network.
    let out = tool
        .execute(json!({"url": "https://example.com/"}), &ctx())
        .await;
    assert!(
        !out.as_ref()
            .is_err_and(|e| e.to_string().contains("URL failed security validation")),
        "public URL should not fail SSRF validation: {out:?}"
    );
}

#[tokio::test]
async fn webfetch_allows_public_url() {
    let tool = WebFetchTool;
    let out = tool
        .execute(json!({"url": "https://example.com/"}), &ctx())
        .await;
    assert!(
        !out.as_ref()
            .is_err_and(|e| e.to_string().contains("URL failed security validation")),
        "public URL should not fail SSRF validation: {out:?}"
    );
}
