//! Benchmark registry, data management, workbook output, and local MVP runners.
//!
//! This crate owns the `/bench` command backend for the local benchmark MVP.

pub mod command;
pub mod data;
pub mod model;
pub mod registry;
pub mod runner;
pub mod suites;
pub mod workbook;

pub use command::{
    BenchCommand, BenchInitMode, BenchInitTarget, BenchRunOptions, BenchTarget, parse_bench_command,
};
pub use data::{
    BenchCaseFixture, BenchDataFile, BenchDataManifest, BenchDataSource, BenchInitOutcome,
    BenchInitProgressEvent, bench_data_root, bench_data_root_for_language, init_suite,
    init_suite_with_language, init_target, init_target_with_progress, load_manifest, verify_suite,
    verify_suite_with_language,
};
pub use model::{
    BenchGeneratedSample, BenchGenerationResult, BenchModelRunner, LiveBenchModelRunner,
    MockBenchModelRunner, ResolvedModelSelection, resolve_model_context, resolve_selected_model,
    slugify_path_segment,
};
pub use registry::{
    BenchProfileDef, BenchSuiteDef, all_profiles, all_suites, expand_target, find_profile,
    find_suite, requires_confirmation, resolve_suite_and_language, resolve_suite_language,
};
pub use runner::{
    BenchProgressHandle, BenchRunEvent, BenchRunOutcome, BenchRunProgress, run_target,
    run_target_with_progress, validate_run_prerequisites,
};
pub use suites::{
    BenchCaseEvaluation, BenchMetricEvaluation, BenchSuiteAdapter, adapter_for_suite,
};
pub use workbook::{
    ARTIFACTS_COLUMNS, ARTIFACTS_SHEET, BenchArtifactRecord, BenchCaseResult, BenchResultSummary,
    BenchRunConfig, CASES_COLUMNS, CASES_SHEET, METRICS_COLUMNS, METRICS_SHEET, RUN_COLUMNS,
    RUN_SHEET, workbook_debug_sidecar_path, workbook_output_path, workbook_resume_state_path,
    write_benchmark_workbook,
};
