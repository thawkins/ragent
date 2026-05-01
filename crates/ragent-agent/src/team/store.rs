//! Compatibility wrapper for shared team store helpers.
//!
//! The source-of-truth implementation lives in `ragent-team`.

#[path = "../../../ragent-team/src/team/store.rs"]
mod shared_store;

pub use shared_store::*;
