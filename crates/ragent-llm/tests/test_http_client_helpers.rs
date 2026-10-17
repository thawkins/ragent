//! Coverage tests for the shared HTTP-client helpers in
//! `ragent_llm::providers::http_client`.
//!
//! These exercise the pure buffer/line/redaction helpers directly and drive
//! `read_body_capped` / `execute_with_retry` against a local one-shot server
//! (no real network egress).

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use ragent_llm::provider::http_client::{
    MAX_SSE_BUFFER_BYTES, append_stream_chunk_capped, create_http_client, execute_with_retry,
    read_body_capped, redact_frame, sse_buffer_exceeded, take_sse_line,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[test]
fn test_take_sse_line_drains_and_returns_complete_lines() {
    let mut buffer = String::from("data: one\n\ndata: two\n");
    assert_eq!(take_sse_line(&mut buffer).as_deref(), Some("data: one"));
    // An empty line is a legitimate complete line.
    assert_eq!(take_sse_line(&mut buffer).as_deref(), Some(""));
    assert_eq!(take_sse_line(&mut buffer).as_deref(), Some("data: two"));
    // No trailing newline -> nothing to take yet.
    assert_eq!(take_sse_line(&mut buffer), None);
    assert_eq!(buffer.len(), 0);
}

#[test]
fn test_sse_buffer_exceeded_uses_strict_greater_than() {
    // The cap is exclusive: exactly the limit is still allowed.
    let at_limit = "a".repeat(MAX_SSE_BUFFER_BYTES);
    assert!(!sse_buffer_exceeded(&at_limit));
    let over = "a".repeat(MAX_SSE_BUFFER_BYTES + 1);
    assert!(sse_buffer_exceeded(&over));
}

#[test]
fn test_append_stream_chunk_capped_reports_overflow() {
    let mut buffer = String::new();
    assert!(append_stream_chunk_capped(&mut buffer, "hello"));
    assert_eq!(buffer, "hello");

    let big = "x".repeat(MAX_SSE_BUFFER_BYTES + 5);
    let mut buffer = String::new();
    assert!(!append_stream_chunk_capped(&mut buffer, &big));
}

#[test]
fn test_redact_frame_masks_registered_secret_and_caps_length() {
    ragent_types::sanitize::register_secret("sk-super-secret-token");
    let frame = format!(
        "Authorization: Bearer sk-super-secret-token {}",
        "z".repeat(1000)
    );
    let out = redact_frame(&frame);
    assert!(
        !out.contains("sk-super-secret-token"),
        "secret leaked: {out}"
    );
    assert!(out.len() <= 512, "frame not capped: {} bytes", out.len());
}

/// Spawn a one-shot server that writes `body` with `status` and returns its
/// base URL plus the received request bytes.
async fn spawn_once(status: u16, body: String) -> (String, tokio::sync::oneshot::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let (tx, rx) = tokio::sync::oneshot::channel::<String>();
    tokio::spawn(async move {
        let Ok((mut socket, _)) = listener.accept().await else {
            return;
        };
        let mut req = vec![0_u8; 8192];
        let n = socket.read(&mut req).await.unwrap_or(0);
        let _ = tx.send(String::from_utf8_lossy(&req[..n]).into_owned());
        let response = format!(
            "HTTP/1.1 {status} X\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });
    (format!("http://{addr}"), rx)
}

#[tokio::test]
async fn test_read_body_capped_truncates_to_limit() {
    let (url, _rx) = spawn_once(200, "abcdefghij".to_string()).await;
    let response = create_http_client().get(&url).send().await.expect("send");
    let body = read_body_capped(response, 4).await;
    assert_eq!(body, "abcd");
}

#[tokio::test]
async fn test_read_body_capped_full_when_under_limit() {
    let (url, _rx) = spawn_once(200, "short".to_string()).await;
    let response = create_http_client().get(&url).send().await.expect("send");
    let body = read_body_capped(response, 64).await;
    assert_eq!(body, "short");
}

#[tokio::test]
async fn test_execute_with_retry_returns_success_immediately() {
    let (url, _rx) = spawn_once(200, "ok".to_string()).await;
    let client = create_http_client();
    let response = execute_with_retry(
        || {
            let client = client.clone();
            let url = url.clone();
            async move { client.get(&url).send().await }
        },
        2,
    )
    .await
    .expect("response");
    assert_eq!(response.status(), 200);
    assert_eq!(read_body_capped(response, 64).await, "ok");
}

#[tokio::test]
async fn test_execute_with_retry_retries_server_error_then_succeeds() {
    // A server that answers 500 on the first connection and 200 on the second.
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_srv = Arc::clone(&attempts);
    tokio::spawn(async move {
        for _ in 0..2 {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let mut req = vec![0_u8; 4096];
            let _ = socket.read(&mut req).await;
            let first = attempts_srv.fetch_add(1, Ordering::SeqCst) == 0;
            let (status, body) = if first {
                ("500 Internal Server Error", "boom")
            } else {
                ("200 OK", "recovered")
            };
            let response = format!(
                "HTTP/1.1 {status}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
    });
    let url = format!("http://{addr}");
    let client = create_http_client();
    let response = execute_with_retry(
        || {
            let client = client.clone();
            let url = url.clone();
            async move { client.get(&url).send().await }
        },
        2,
    )
    .await
    .expect("response");
    assert_eq!(response.status(), 200);
    assert_eq!(read_body_capped(response, 64).await, "recovered");
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_execute_with_retry_client_error_is_not_retried() {
    let (url, _keep_tx) = spawn_once(404, "nope".to_string()).await;
    let client = create_http_client();
    let err = match execute_with_retry(
        || {
            let client = client.clone();
            let url = url.clone();
            async move { client.get(&url).send().await }
        },
        3,
    )
    .await
    {
        Ok(resp) => panic!("expected error, got {}", resp.status()),
        Err(e) => e.to_string(),
    };
    assert!(err.contains("404"), "error: {err}");
    assert!(err.contains("not retryable"), "error: {err}");
}

#[tokio::test]
async fn test_execute_with_retry_429_without_retry_budget_surfaces_body() {
    // max_retries = 0 means the 429 branch is skipped and the client-error
    // branch returns the (capped) body without sleeping.
    let (url, _keep_tx) = spawn_once(429, "slow down".to_string()).await;
    let client = create_http_client();
    let err = match execute_with_retry(
        || {
            let client = client.clone();
            let url = url.clone();
            async move { client.get(&url).send().await }
        },
        0,
    )
    .await
    {
        Ok(resp) => panic!("expected error, got {}", resp.status()),
        Err(e) => e.to_string(),
    };
    assert!(err.contains("429"), "error: {err}");
    assert!(err.contains("slow down"), "error: {err}");
}

#[tokio::test]
async fn test_create_streaming_client_and_defaults() {
    // Both constructors must produce a usable client (covers the builder path).
    let _ = ragent_llm::provider::http_client::create_streaming_http_client();
    let _ = create_http_client();
    assert_eq!(
        ragent_llm::provider::http_client::STREAM_CHUNK_IDLE_TIMEOUT_SECS,
        120
    );
    assert_eq!(
        ragent_llm::provider::http_client::DEFAULT_STREAM_TIMEOUT_SECS,
        600
    );
    assert_eq!(
        ragent_llm::provider::http_client::MAX_ERROR_BODY_BYTES,
        64 * 1024
    );
}

#[tokio::test]
async fn test_execute_with_retry_recovers_from_429_using_retry_after() {
    // First connection: 429 with `Retry-After: 0` (clamped, no real wait);
    // second connection: 200.
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_srv = Arc::clone(&attempts);
    tokio::spawn(async move {
        for _ in 0..2 {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let mut req = vec![0_u8; 4096];
            let _ = socket.read(&mut req).await;
            let first = attempts_srv.fetch_add(1, Ordering::SeqCst) == 0;
            let response = if first {
                let body = "slow down";
                format!(
                    "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 0\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
            } else {
                let body = "ok";
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
            };
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
    });
    let url = format!("http://{addr}");
    let client = create_http_client();
    let response = execute_with_retry(
        || {
            let client = client.clone();
            let url = url.clone();
            async move { client.get(&url).send().await }
        },
        2,
    )
    .await
    .expect("response");
    assert_eq!(response.status(), 200);
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn test_execute_with_retry_exhausts_on_retryable_connection_error() {
    // Port 9 (discard) is not listening on 127.0.0.1 in this environment; a
    // connect failure is retryable, so with max_retries = 1 the loop retries
    // once (sleeping the first backoff) and then returns a non-retryable error
    // once the budget is exhausted.
    let client = create_http_client();
    let url = "http://127.0.0.1:9/".to_string();
    let err = match execute_with_retry(
        || {
            let client = client.clone();
            let url = url.clone();
            async move { client.get(&url).send().await }
        },
        1,
    )
    .await
    {
        Ok(resp) => panic!("expected error, got {}", resp.status()),
        Err(e) => e.to_string(),
    };
    assert!(
        err.contains("not retryable"),
        "exhausted retry should surface a non-retryable error: {err}"
    );
}

#[tokio::test]
async fn test_execute_with_retry_retries_then_exhausts_on_server_error() {
    // A server that always answers 500; with max_retries = 1 it is retried once
    // and then the last recorded error is returned.
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        for _ in 0..2 {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            let mut req = vec![0_u8; 4096];
            let _ = socket.read(&mut req).await;
            let body = "always broken";
            let response = format!(
                "HTTP/1.1 500 Internal Server Error\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
    });
    let url = format!("http://{addr}");
    let client = create_http_client();
    // On exhaustion the last 5xx response is returned to the caller (the
    // caller is responsible for classifying a non-success status), not an Err.
    let response = execute_with_retry(
        || {
            let client = client.clone();
            let url = url.clone();
            async move { client.get(&url).send().await }
        },
        1,
    )
    .await
    .expect("a 5xx response is surfaced rather than errored");
    assert_eq!(response.status(), 500);
    assert_eq!(read_body_capped(response, 64).await, "always broken");
}
