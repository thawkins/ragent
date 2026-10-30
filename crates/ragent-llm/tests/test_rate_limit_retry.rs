//! Tests for the HTTP 429 / 5xx retry behaviour of
//! [`ragent_llm::provider::http_client::execute_with_retry`].
//!
//! These drive the real retry loop against a scripted local TCP server that
//! answers each connection with the next status in a queue, so the assertions
//! are about what the retry machinery *does* (how many requests it makes, what
//! it finally returns, whether a `Retry-After` shorter than the default backoff
//! is honoured) rather than about arithmetic recomputed in the test body.
//!
//! Timing note: `execute_with_retry` sleeps its backoff before a retry, and the
//! schedule starts at 500 ms. Every test here either supplies a short
//! `Retry-After` (which replaces the backoff) or keeps the retry count to one, so
//! the whole file stays well inside the per-test timeout.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use ragent_llm::provider::http_client::execute_with_retry;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt};
use tokio::net::TcpListener;

/// How the scripted server should answer a single connection.
#[derive(Clone, Copy)]
enum Reply {
    /// Answer with this status and a short JSON body.
    Status(u16),
    /// Answer `429` with `Retry-After: <secs>` and a short body.
    RateLimited(u64),
}

/// Spawn a local server that answers the first `len(replies)` connections with
/// the given replies, in order, and returns its base URL plus a counter of the
/// connections it actually served.
async fn spawn_scripted_server(replies: Vec<Reply>) -> anyhow::Result<(String, Arc<AtomicUsize>)> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let served = Arc::new(AtomicUsize::new(0));
    let served_out = Arc::clone(&served);

    tokio::spawn(async move {
        for reply in replies {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            // Drain the request head before replying: a client whose bytes are
            // still in flight when the socket closes sees a TCP RST, which
            // surfaces as a spurious connection error in the retry loop.
            let mut buf = [0u8; 8192];
            let _ = socket.read(&mut buf).await;

            let (head, body) = match reply {
                Reply::Status(code) => (
                    format!("HTTP/1.1 {code} Scripted\r\nContent-Length: {}\r\n", 19),
                    "{\"error\":\"scripted\"}".to_string(),
                ),
                Reply::RateLimited(secs) => (
                    format!(
                        "HTTP/1.1 429 Too Many Requests\r\nRetry-After: {secs}\r\nContent-Length: {}\r\n",
                        19
                    ),
                    "{\"error\":\"scripted\"}".to_string(),
                ),
            };
            let response = format!("{head}Connection: close\r\n\r\n{body}");
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.shutdown().await;
            served_out.fetch_add(1, Ordering::SeqCst);
        }
    });

    Ok((format!("http://{addr}/v1/chat"), served))
}

/// A `reqwest::Client` with no default timeout; the retry loop's own sleeps are
/// the only delay in these tests.
fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .build()
        .expect("build test client")
}

#[tokio::test]
async fn test_success_on_first_attempt_does_not_retry() {
    let (url, served) = spawn_scripted_server(vec![Reply::Status(200)])
        .await
        .expect("spawn server");

    let response = execute_with_retry(
        || async { client().get(truncate_for_log(&url)).send().await },
        3,
    )
    .await
    .expect("a 200 must be returned as-is");

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(
        served.load(Ordering::SeqCst),
        1,
        "a successful first attempt must not be retried"
    );
}

#[tokio::test]
async fn test_rate_limit_retry_after_is_honoured_then_succeeds() {
    // 429 with `Retry-After: 1` replaces the 500 ms default backoff, so the
    // retry happens after the header's delay and the second attempt's 200 is
    // what the caller sees. Proving the header is honoured (not just the
    // attempt count) is what the placeholder file never did.
    let (url, served) = spawn_scripted_server(vec![Reply::RateLimited(1), Reply::Status(200)])
        .await
        .expect("spawn server");
    let started = Instant::now();

    let response = execute_with_retry(
        || async { client().get(truncate_for_log(&url)).send().await },
        3,
    )
    .await
    .expect("the retry after a 429 must return the second attempt's response");

    let elapsed = started.elapsed();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(
        served.load(Ordering::SeqCst),
        2,
        "exactly one retry must be made after the 429"
    );
    assert!(
        elapsed >= Duration::from_secs(1),
        "the Retry-After delay must be observed, elapsed was {elapsed:?}"
    );
}

#[tokio::test]
async fn test_exhausted_retries_report_the_last_status() {
    // Three 500s with `max_retries = 2` means three attempts in total (the
    // initial one plus two retries) and then an error naming the status.
    let (url, served) = spawn_scripted_server(vec![
        Reply::RateLimited(0),
        Reply::RateLimited(0),
        Reply::RateLimited(0),
    ])
    .await
    .expect("spawn server");

    let err = execute_with_retry(
        || async { client().get(truncate_for_log(&url)).send().await },
        2,
    )
    .await
    .expect_err("all retries exhausted must surface an error");

    let message = err.to_string();
    assert!(
        message.contains("429"),
        "the final error must name the status the server kept answering, got: {message}"
    );
    assert_eq!(
        served.load(Ordering::SeqCst),
        3,
        "max_retries = 2 means one attempt plus two retries"
    );
}

#[tokio::test]
async fn test_client_error_is_not_retried() {
    // A 400 is not transient: the loop must return the error immediately after
    // a single request rather than burning the retry budget on it.
    let (url, served) = spawn_scripted_server(vec![Reply::Status(400), Reply::Status(200)])
        .await
        .expect("spawn server");

    let err = execute_with_retry(
        || async { client().get(truncate_for_log(&url)).send().await },
        3,
    )
    .await
    .expect_err("a 400 must be returned as an error");

    assert!(
        err.to_string().contains("not retryable"),
        "a client error must be classified as non-retryable, got: {err}"
    );
    assert_eq!(
        served.load(Ordering::SeqCst),
        1,
        "a 400 must not consume a retry"
    );
}

/// Reborrow the URL as a `&str` that outlives the closure.
///
/// `execute_with_retry` takes a `Fn() -> Future` closure, so the URL cannot be
/// moved into it; this keeps the borrow explicit and named.
fn truncate_for_log(url: &str) -> &str {
    url
}
