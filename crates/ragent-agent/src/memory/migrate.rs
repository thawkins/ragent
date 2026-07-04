//! Memory block migration and analysis helpers.
//!
//! Compatibility wrapper; the single source-of-truth implementation lives in
//! `ragent_tools_extended::memory::migrate`. This thin re-export matches the
//! pattern already used for `block` and `storage` (see DCREMOVALPLAN M5).
pub use ragent_tools_extended::memory::migrate::*;
