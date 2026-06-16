//! Shared helpers for code-index tools.
//!
//! The code-index store is protected by a single mutex that may be held for
//! long periods by the background re-indexing worker. Tools use the
//! non-blocking `try_*` query methods on [`ragent_codeindex::CodeIndex`] and
//! retry briefly; if the index is still busy they return a friendly "busy"
//! response instead of stalling the agent execution loop.

use std::time::{Duration, Instant};

use anyhow::Result;
use serde_json::json;

use crate::ToolOutput;

/// Tool output returned when the code index is temporarily locked.
pub(crate) fn busy_output(name: &str) -> ToolOutput {
    ToolOutput {
        content: format!(
            "Code index is currently busy (re-indexing in progress). \
             The `{name}` tool could not acquire the index lock. \
             Wait a moment and retry, or use `grep` / `glob` as fallback tools."
        ),
        metadata: Some(json!({
            "error": "codeindex_busy",
            "busy": true,
            "fallback_tools": ["grep", "glob"]
        })),
    }
}

/// Retry a non-blocking index operation until it succeeds, fails with an
/// error, or the timeout is reached.
///
/// `op` must return:
///
/// * `Ok(Some(value))` when the lock was acquired and the operation completed.
/// * `Ok(None)` when the index is currently busy (the lock was not acquired).
/// * `Err(...)` on a real failure such as a database error.
pub(crate) async fn with_retry<T, F>(op: F) -> Result<Option<T>>
where
    F: FnMut() -> Result<Option<T>> + Send,
{
    with_retry_for(Duration::from_secs(5), Duration::from_millis(100), op).await
}

async fn with_retry_for<T, F>(timeout: Duration, interval: Duration, mut op: F) -> Result<Option<T>>
where
    F: FnMut() -> Result<Option<T>> + Send,
{
    let deadline = Instant::now() + timeout;
    loop {
        match op()? {
            Some(value) => return Ok(Some(value)),
            None => {
                if Instant::now() >= deadline {
                    return Ok(None);
                }
                tokio::time::sleep(interval).await;
            }
        }
    }
}
