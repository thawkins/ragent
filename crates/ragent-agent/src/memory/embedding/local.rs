//! Local (in-process) embedding provider.
//!
//! Compatibility wrapper; the single source-of-truth implementation lives in
//! `ragent_tools_extended::memory::embedding::local`. This thin re-export
//! matches the pattern already used for `block` and `storage` (see
//! DCREMOVALPLAN M5).
pub use ragent_tools_extended::memory::embedding::local::*;