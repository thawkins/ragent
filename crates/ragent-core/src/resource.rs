//! Process resource limits — bounded concurrency for child process spawns
//! and tool execution.
//!
//! Provides global [`Semaphore`](tokio::sync::Semaphore) instances that gate:
//!
//! 1. **Process spawning** — how many child processes may be in flight at once.
//!    All process-spawning call sites (BashTool, dynamic context commands, MCP
//!    stdio servers) should acquire a permit from [`acquire_process_permit`].
//!
//! 2. **Tool execution** — how many tool calls may run concurrently within a
//!    single agent loop iteration.  The session processor acquires a permit
//!    from [`acquire_tool_permit`] before spawning each tool call task.
//!
//! ## Why not `setrlimit`?
//!
//! True per-process resource limits (`RLIMIT_NPROC`, `RLIMIT_CPU`, etc.)
//! require [`std::os::unix::process::CommandExt::pre_exec`], which is an
//! `unsafe` function.  The workspace has `unsafe_code = "deny"`, so we use
//! application-level concurrency control instead.  This gives us bounded
//! parallelism (preventing fork-bomb scenarios) and graceful back-pressure
//! when many tools run concurrently.

use std::sync::LazyLock;

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

// ── Child-process concurrency ────────────────────────────────────

/// Maximum number of concurrent child processes the agent may spawn.
///
/// This covers BashTool executions, dynamic context commands, and MCP stdio
/// server processes.  The value is intentionally conservative — most workloads
/// need at most 4-6 concurrent processes.
pub const MAX_CONCURRENT_PROCESSES: usize = 16;

/// Global semaphore that bounds child-process concurrency.
static PROCESS_SEMAPHORE: LazyLock<std::sync::Arc<Semaphore>> =
    LazyLock::new(|| std::sync::Arc::new(Semaphore::new(MAX_CONCURRENT_PROCESSES)));

/// Acquire a permit to spawn a child process.
///
/// Blocks (asynchronously) until a permit is available.  The permit is
/// released when it is dropped, so callers should hold it for the lifetime
/// of the child process.
///
/// # Errors
///
/// Returns an error only if the semaphore is closed (should never happen
/// during normal operation).
pub async fn acquire_process_permit() -> anyhow::Result<OwnedSemaphorePermit> {
    PROCESS_SEMAPHORE
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| anyhow::anyhow!("process semaphore closed"))
}

/// Return the number of permits currently available.
///
/// Useful for diagnostics and testing.
pub fn available_process_permits() -> usize {
    PROCESS_SEMAPHORE.available_permits()
}

// ── Tool-execution concurrency ───────────────────────────────────

/// Maximum number of tool calls that may execute concurrently within
/// the agent loop.
///
/// This prevents a single agent turn with many tool calls from
/// overwhelming the system with parallel work.
pub const MAX_CONCURRENT_TOOLS: usize = 5;

/// Global semaphore that bounds concurrent tool executions.
static TOOL_SEMAPHORE: LazyLock<std::sync::Arc<Semaphore>> =
    LazyLock::new(|| std::sync::Arc::new(Semaphore::new(MAX_CONCURRENT_TOOLS)));

/// Acquire a permit to execute a tool call.
///
/// The permit is released when dropped.  Hold it for the duration
/// of the tool execution.
///
/// # Errors
///
/// Returns an error only if the semaphore is closed.
pub async fn acquire_tool_permit() -> anyhow::Result<OwnedSemaphorePermit> {
    TOOL_SEMAPHORE
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| anyhow::anyhow!("tool semaphore closed"))
}

/// Return the number of tool-execution permits currently available.
///
/// Useful for diagnostics and testing.
pub fn available_tool_permits() -> usize {
    TOOL_SEMAPHORE.available_permits()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_acquire_and_release_permit() {
        let initial = available_process_permits();
        let permit = acquire_process_permit().await.unwrap();
        assert_eq!(available_process_permits(), initial - 1);
        drop(permit);
        // Tokio may need a tick to reclaim.
        tokio::task::yield_now().await;
        assert_eq!(available_process_permits(), initial);
    }

    #[tokio::test]
    async fn test_multiple_permits() {
        let initial = available_process_permits();
        let mut permits = Vec::new();
        for _ in 0..4 {
            permits.push(acquire_process_permit().await.unwrap());
        }
        assert_eq!(available_process_permits(), initial - 4);
        drop(permits);
        tokio::task::yield_now().await;
        assert_eq!(available_process_permits(), initial);
    }

    #[tokio::test]
    async fn test_tool_permit_acquire_release() {
        let initial = available_tool_permits();
        assert_eq!(initial, MAX_CONCURRENT_TOOLS);
        let permit = acquire_tool_permit().await.unwrap();
        assert_eq!(available_tool_permits(), initial - 1);
        drop(permit);
        tokio::task::yield_now().await;
        assert_eq!(available_tool_permits(), initial);
    }

    #[tokio::test]
    async fn test_tool_permit_concurrent_limit() {
        let initial = available_tool_permits();
        let mut permits = Vec::new();
        for _ in 0..MAX_CONCURRENT_TOOLS {
            permits.push(acquire_tool_permit().await.unwrap());
        }
        assert_eq!(available_tool_permits(), 0, "All permits should be taken");
        drop(permits);
        tokio::task::yield_now().await;
        assert_eq!(available_tool_permits(), initial);
    }
}
