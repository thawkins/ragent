//! Additional tests-only coverage for the auto-updater
//! (`ragent_agent::updater`).
//!
//! The sibling `test_updater_extra.rs` covers the digest *validation* refusals.
//! Here an in-process Axum mock (no network, no real GitHub) drives the
//! `download_and_replace_verified` download path up to the point where it
//! refuses to touch the filesystem: a non-2xx response and a SHA-256 mismatch.
//! Both bail *before* any write, so the running test binary is never replaced.

use std::sync::Arc;

use axum::{Router, extract::State, http::StatusCode, routing::get};
use ragent_agent::updater::download_and_replace_verified;

#[derive(Clone)]
struct MockState {
    status: StatusCode,
    body: Vec<u8>,
}

async fn serve(State(state): State<Arc<MockState>>) -> (StatusCode, Vec<u8>) {
    (state.status, state.body.clone())
}

async fn start_mock(state: MockState) -> (String, tokio::task::JoinHandle<()>) {
    let app = Router::new()
        .route("/download", get(serve))
        .with_state(Arc::new(state));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind mock server");
    let addr = listener.local_addr().expect("local addr");
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    (format!("http://{addr}/download"), handle)
}

/// A valid, all-zero SHA-256 digest: 64 lowercase hex characters.
const ZERO_DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";

#[tokio::test]
async fn download_and_replace_verified_reports_a_non_success_status() {
    let (url, handle) = start_mock(MockState {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        body: b"boom".to_vec(),
    })
    .await;
    let err = download_and_replace_verified(&url, Some(ZERO_DIGEST))
        .await
        .expect_err("a 5xx response must fail the download");
    assert!(
        err.to_string().contains("Download failed: HTTP"),
        "err: {err}"
    );
    handle.abort();
}

#[tokio::test]
async fn download_and_replace_verified_rejects_a_sha256_mismatch() {
    // The mock serves bytes whose digest is not the all-zero digest, so the
    // verified download refuses before it writes or renames anything.
    let (url, handle) = start_mock(MockState {
        status: StatusCode::OK,
        body: b"definitely-not-the-zero-digest-input".to_vec(),
    })
    .await;
    let err = download_and_replace_verified(&url, Some(ZERO_DIGEST))
        .await
        .expect_err("a digest mismatch must be refused");
    assert!(
        err.to_string().contains("failed SHA-256 verification"),
        "err: {err}"
    );
    handle.abort();
}
