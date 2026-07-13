//! Marker for mock-model support (kept for backwards compatibility).
//!
//! The real mock-model harness lives in [`crate::testing`] (compiled when the
//! `mock` feature is enabled). This tiny module provides the historical
//! `MockSupport` marker so existing callers and the crate doc-list still
//! compile.

/// A marker type indicating that mock-model support is compiled in.
#[derive(Debug, Default)]
pub struct MockSupport;

impl MockSupport {
    /// Creates a new `MockSupport` instance.
    pub fn new() -> Self {
        Self
    }

    /// Returns `true` to signal that mock utilities are available.
    pub fn available(&self) -> bool {
        true
    }
}
