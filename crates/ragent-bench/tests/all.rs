//! Consolidated integration-test target: one test binary per crate.
//!
//! Each former top-level `tests/test_*.rs` file is included as a child
//! module via `#[path]`, so it keeps its own file-relative `mod` /
//! `#[path]` resolution and its `test_<file>::<fn>` test name. See
//! `AGENTS.md` ("Test Layout") and
//! `docs/reports/testconsolidation-review.md`.

#[path = "test_bench_core.rs"]
mod test_bench_core;
#[path = "test_bench_data_model.rs"]
mod test_bench_data_model;
#[path = "test_bench_metrics.rs"]
mod test_bench_metrics;
#[path = "test_exec_guard.rs"]
mod test_exec_guard;
#[path = "test_mock_replay.rs"]
mod test_mock_replay;
#[path = "test_suite_adapters.rs"]
mod test_suite_adapters;
#[path = "test_suite_adapters_all.rs"]
mod test_suite_adapters_all;
#[path = "test_vendor_slug_partitioning.rs"]
mod test_vendor_slug_partitioning;
