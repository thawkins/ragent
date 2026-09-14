//! Process-wide shared `reqwest::Client` for VCS API calls (PERF-058).
//!
//! Every GitHub/GitLab tool used to construct a fresh `reqwest::Client` per
//! call, which builds a new TLS configuration and connection pool each time.
//! One cached client is cloned (an `Arc` bump) into each client struct so the
//! TLS pool and keep-alive connections are reused across calls.

use std::sync::OnceLock;
use std::time::Duration;

use reqwest::Client;

/// Connect timeout in seconds.
const CONNECT_TIMEOUT_SECS: u64 = 30;

/// Request timeout in seconds.
const REQUEST_TIMEOUT_SECS: u64 = 120;

/// Maximum idle connections retained per host.
const MAX_IDLE_PER_HOST: usize = 8;

/// Return a clone of the process-wide shared HTTP client.
///
/// The client is built once on first use (connection-pool limits, connect and
/// request timeouts, TCP keep-alive) and reused for every subsequent call.
#[must_use]
pub fn shared_client() -> Client {
    static CACHED: OnceLock<Client> = OnceLock::new();
    CACHED
        .get_or_init(|| {
            Client::builder()
                .pool_max_idle_per_host(MAX_IDLE_PER_HOST)
                .pool_idle_timeout(Duration::from_secs(90))
                .connect_timeout(Duration::from_secs(CONNECT_TIMEOUT_SECS))
                .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
                .tcp_keepalive(Duration::from_secs(60))
                .build()
                .unwrap_or_else(|e| {
                    tracing::warn!(
                        error = %e,
                        "Failed to build shared VCS HTTP client, using defaults"
                    );
                    Client::new()
                })
        })
        .clone()
}
