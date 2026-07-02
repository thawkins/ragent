//! Integration tests for `ragent-types` process/tool resource limits.
//!
//! Relocated from the inline `#[cfg(test)]` module in `src/resource.rs`
//! (T-010 of the testconsolidate spec). All tested functions are public.

use ragent_types::resource::{
    acquire_process_permit, acquire_tool_permit, available_process_permits,
    available_tool_permits,
};
use serial_test::serial;

#[tokio::test]
#[serial]
async fn test_acquire_and_release_permit() {
    let before = available_process_permits();
    let permit = acquire_process_permit().await.unwrap();
    assert_eq!(available_process_permits(), before - 1);
    drop(permit);
    tokio::task::yield_now().await;
    assert_eq!(available_process_permits(), before);
}

#[tokio::test]
#[serial]
async fn test_multiple_permits() {
    let before = available_process_permits();
    let mut permits = Vec::new();
    for _ in 0..4 {
        permits.push(acquire_process_permit().await.unwrap());
    }
    assert_eq!(available_process_permits(), before - 4);
    drop(permits);
    tokio::task::yield_now().await;
    assert_eq!(available_process_permits(), before);
}

#[tokio::test]
#[serial]
async fn test_tool_permit_acquire_release() {
    let before = available_tool_permits();
    let permit = acquire_tool_permit().await.unwrap();
    assert_eq!(available_tool_permits(), before - 1);
    drop(permit);
    tokio::task::yield_now().await;
    assert_eq!(available_tool_permits(), before);
}

#[tokio::test]
#[serial]
async fn test_tool_permit_concurrent_limit() {
    let before = available_tool_permits();
    let mut permits = Vec::new();
    for _ in 0..before {
        permits.push(acquire_tool_permit().await.unwrap());
    }
    assert_eq!(available_tool_permits(), 0, "All permits should be taken");
    drop(permits);
    tokio::task::yield_now().await;
    assert_eq!(available_tool_permits(), before);
}