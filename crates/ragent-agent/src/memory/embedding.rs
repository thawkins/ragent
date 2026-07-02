//! Memory embedding providers and similarity helpers.
//!
//! Compatibility wrapper; the single source-of-truth implementation lives in
//! `ragent_tools_extended::memory::embedding`. This thin re-export matches
//! the pattern already used for `block` and `storage` (see DCREMOVALPLAN M5).
pub use ragent_tools_extended::memory::embedding::*;