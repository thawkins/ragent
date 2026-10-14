//! Shared store-scan reporting primitives for the `/plugins` and `/connectors`
//! command families.
//!
//! Both family stores report a discovery-time failure as a row carrying the
//! crate's own error, so the two structs were structurally identical. The single
//! generic definition lives here; each crate aliases it with its own error type,
//! preserving the public name and fields.

/// A discovery-time failure carrying the crate's own error type.
///
/// Reporting needs a row per problematic directory even when no descriptor
/// exists, so the failure carries the crate's error `E`; the failing directory
/// travels on the surrounding scan result.
#[derive(Debug)]
pub struct ScanFailure<E> {
    /// The failure that prevented descriptor construction.
    pub error: E,
}
