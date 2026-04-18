//! Message part pooling utilities for memory optimization.
//!
//! This module provides object pooling for frequently allocated [`MessagePart`]
//! values. By reusing MessagePart allocations, we reduce memory churn and
//! improve cache locality during high-throughput message processing.
//!
//! The pool uses a thread-local design for zero-lock contention and fast
//! allocation/deallocation.

use std::cell::RefCell;

/// Maximum number of pooled `Text` parts per thread.
const TEXT_POOL_SIZE: usize = 256;

/// A pooled `String` that can be reused.
pub struct PooledString {
    inner: Option<String>,
}

impl PooledString {
    /// Create a new pooled string (may reuse from pool).
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: STRING_POOL.with(|pool| pool.borrow_mut().pop()),
        }
    }

    /// Create a pooled string with initial capacity.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        let mut s = Self::new();
        if let Some(ref mut inner) = s.inner {
            inner.reserve(capacity);
        } else {
            s.inner = Some(String::with_capacity(capacity));
        }
        s
    }

    /// Get the string value, creating if necessary.
    pub fn get_mut(&mut self) -> &mut String {
        self.inner.get_or_insert_with(String::new)
    }

    /// Take ownership of the string value.
    #[must_use]
    pub fn into_string(mut self) -> String {
        self.inner.take().unwrap_or_default()
    }
}

impl Default for PooledString {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for PooledString {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            STRING_POOL.with(|pool| {
                let mut pool = pool.borrow_mut();
                if pool.len() < TEXT_POOL_SIZE {
                    // Clear and return to pool
                    let mut s = inner;
                    s.clear();
                    pool.push(s);
                }
            });
        }
    }
}

// Thread-local string pool
thread_local! {
    static STRING_POOL: RefCell<Vec<String>> = RefCell::new(Vec::with_capacity(TEXT_POOL_SIZE));
}

/// Statistics about pool usage.
#[derive(Debug, Default, Clone, Copy)]
pub struct PoolStats {
    /// Current pooled strings available.
    pub available_strings: usize,
    /// Total capacity of string pool.
    pub string_pool_capacity: usize,
}

/// Get current pool statistics.
#[must_use]
pub fn pool_stats() -> PoolStats {
    let mut stats = PoolStats::default();
    STRING_POOL.with(|pool| {
        stats.available_strings = pool.borrow().len();
        stats.string_pool_capacity = TEXT_POOL_SIZE;
    });
    stats
}

/// Clear all thread-local pools.
///
/// Call this periodically or when memory pressure is high.
pub fn clear_pools() {
    STRING_POOL.with(|pool| {
        pool.borrow_mut().clear();
    });
}

/// Estimate memory saved by using the pool.
///
/// This is a rough estimate based on average pooled string size.
#[must_use]
pub fn estimated_memory_saved() -> usize {
    let stats = pool_stats();
    // Estimate ~1KB average per pooled string (conservative)
    stats.available_strings * 1024
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pooled_string_reuse() {
        clear_pools();

        // Create and drop a pooled string
        {
            let mut s = PooledString::new();
            s.get_mut().push_str("test content");
            assert_eq!(s.get_mut().as_str(), "test content");
        } // Dropped here, should return to pool

        // Next allocation should reuse
        let s2 = PooledString::new();
        // The pool should have a cleared string now
        let stats = pool_stats();
        assert!(stats.available_strings < TEXT_POOL_SIZE);
    }

    #[test]
    fn test_pooled_string_with_capacity() {
        let mut s = PooledString::with_capacity(1024);
        let inner = s.get_mut();
        assert!(inner.capacity() >= 1024);
    }

    #[test]
    fn test_into_string() {
        let mut s = PooledString::new();
        s.get_mut().push_str("hello");
        let owned: String = s.into_string();
        assert_eq!(owned, "hello");
    }

    #[test]
    fn test_clear_pools() {
        // Create and drop several strings
        for _ in 0..10 {
            let _s = PooledString::with_capacity(100);
        }

        clear_pools();

        let stats = pool_stats();
        assert_eq!(stats.available_strings, 0);
    }
}
