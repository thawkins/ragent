//! Consolidated integration-test target: one test binary per crate.
//!
//! Each former top-level `tests/test_*.rs` file is included as a child
//! module via `#[path]`, so it keeps its own file-relative `mod` /
//! `#[path]` resolution and its `test_<file>::<fn>` test name. See
//! `AGENTS.md` ("Test Layout") and
//! `docs/reports/testconsolidation-review.md`.

#[path = "test_surface_config_splice.rs"]
mod test_surface_config_splice;
#[path = "test_surface_harness_extra.rs"]
mod test_surface_harness_extra;
#[path = "test_surface_helpers.rs"]
mod test_surface_helpers;
#[path = "test_surface_manifest.rs"]
mod test_surface_manifest;
#[path = "test_surface_marketplace.rs"]
mod test_surface_marketplace;
#[path = "test_surface_net.rs"]
mod test_surface_net;
#[path = "test_surface_store.rs"]
mod test_surface_store;
#[path = "test_surface_store_extra.rs"]
mod test_surface_store_extra;
