//! Coverage tests for `data.rs` (manifest/case loading, verification, init
//! modes, progress events) and `model.rs` (slug/resolution helpers and the
//! mock model runner).
//!
//! All fixtures are written under `target/temp` so the suite stays hermetic;
//! no network is touched.

use ragent_bench::data::{bench_data_root, bench_data_root_for_language, load_cases};
use ragent_bench::model::{
    BenchModelRunner, MockBenchModelRunner, ResolvedModelSelection, resolve_selected_model,
    slugify_path_segment,
};
use ragent_bench::{
    BenchDataManifest, BenchInitMode, BenchInitProgressEvent, BenchInitTarget, BenchRunOptions,
    init_suite, init_suite_with_language, init_target_with_progress, load_manifest, verify_suite,
    verify_suite_with_language,
};
use ragent_types::ThinkingConfig;
use std::path::Path;
use std::sync::atomic::AtomicBool;

/// A temp directory under the workspace `target/temp`, kept out of `/tmp`.
fn work_dir(tag: &str) -> std::path::PathBuf {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/temp")
        .join(format!(
            "bench-data-{tag}-{}",
            uuid::Uuid::new_v4().simple()
        ));
    std::fs::create_dir_all(&dir).expect("create work dir");
    dir
}

fn sample_selection() -> ResolvedModelSelection {
    resolve_selected_model("anthropic/claude-sonnet-4").expect("selection")
}

#[test]
fn test_bench_data_root_falls_back_for_unknown_suite() {
    let root = work_dir("root-unknown");
    let path = bench_data_root(&root, "not-a-suite");
    assert_eq!(path, root.join("benches").join("data").join("not-a-suite"));
    assert!(
        bench_data_root(&root, "humaneval").ends_with("benches/data/humaneval/python"),
        "known suite uses its default language partition"
    );
    assert!(
        bench_data_root_for_language(&root, "mbpp", "rust").ends_with("benches/data/mbpp/rust")
    );
}

#[test]
fn test_load_manifest_roundtrips_and_reports_missing_and_malformed() {
    let root = work_dir("manifest");
    let init = init_suite(&root, "humaneval", false, false).expect("init humaneval");
    let manifest = load_manifest(&init.data_root).expect("load manifest");
    assert_eq!(manifest.bench_name, "humaneval");
    assert_eq!(manifest.status, "ready");

    let missing = load_manifest(&root.join("does-not-exist")).expect_err("missing manifest");
    assert!(missing.to_string().contains("read manifest"));

    let bad_root = work_dir("manifest-bad");
    std::fs::write(bad_root.join("manifest.json"), "{ not json").expect("write bad manifest");
    let malformed = load_manifest(&bad_root).expect_err("malformed manifest");
    assert!(malformed.to_string().contains("parse manifest"));
}

#[test]
fn test_load_cases_reads_fixtures_from_initialized_root() {
    let root = work_dir("load-cases");
    let init = init_suite(&root, "humaneval", false, false).expect("init humaneval");
    let cases = load_cases(&init.data_root).expect("load cases");
    assert_eq!(cases.len(), 1);
    assert_eq!(cases[0].case_id, "humaneval-sample-001");
    assert_eq!(cases[0].entry_point.as_deref(), Some("add"));
}

#[test]
fn test_load_cases_rejects_manifest_path_escape() {
    let root = work_dir("case-escape");
    let init = init_suite(&root, "humaneval", false, false).expect("init");
    let mut manifest = load_manifest(&init.data_root).expect("load manifest");
    // Craft a manifest whose case_file climbs out of the data root.
    manifest.case_file = "../../../../etc/passwd".to_string();
    let raw = serde_json::to_string_pretty(&manifest).expect("serialize");
    std::fs::write(init.data_root.join("manifest.json"), raw).expect("write crafted manifest");
    let error = load_cases(&init.data_root).expect_err("escape must be rejected");
    assert!(
        error.to_string().contains("benchmark manifest path"),
        "unexpected error: {error}"
    );
}

#[test]
fn test_verify_suite_reports_not_initialized() {
    let root = work_dir("verify-missing");
    let error = verify_suite(&root, "humaneval").expect_err("not initialized");
    assert!(error.to_string().contains("is not initialized"));
}

#[test]
fn test_verify_suite_rejects_manifest_field_mismatches() {
    let root = work_dir("verify-mismatch");
    let init = init_suite(&root, "humaneval", false, false).expect("init");
    let base = load_manifest(&init.data_root).expect("manifest");

    // bench_name mismatch
    let mut wrong_name = base.clone();
    wrong_name.bench_name = "mbpp".to_string();
    write_manifest(&init.data_root, &wrong_name);
    assert!(
        verify_suite_with_language(&root, "humaneval", Some("python"))
            .expect_err("bench_name mismatch")
            .to_string()
            .contains("manifest mismatch")
    );

    // version mismatch
    let mut wrong_version = base.clone();
    wrong_version.manifest_version = 999;
    write_manifest(&init.data_root, &wrong_version);
    assert!(
        verify_suite_with_language(&root, "humaneval", Some("python"))
            .expect_err("version mismatch")
            .to_string()
            .contains("unsupported version")
    );

    // status mismatch
    let mut wrong_status = base.clone();
    wrong_status.status = "stale".to_string();
    write_manifest(&init.data_root, &wrong_status);
    assert!(
        verify_suite_with_language(&root, "humaneval", Some("python"))
            .expect_err("status mismatch")
            .to_string()
            .contains("not ready")
    );

    // language mismatch
    let mut wrong_language = base.clone();
    wrong_language.language = "rust".to_string();
    write_manifest(&init.data_root, &wrong_language);
    assert!(
        verify_suite_with_language(&root, "humaneval", Some("python"))
            .expect_err("language mismatch")
            .to_string()
            .contains("language")
    );

    // no sources
    let mut no_sources = base.clone();
    no_sources.sources.clear();
    write_manifest(&init.data_root, &no_sources);
    assert!(
        verify_suite_with_language(&root, "humaneval", Some("python"))
            .expect_err("no sources")
            .to_string()
            .contains("does not record any sources")
    );

    // no tracked files
    let mut no_files = base.clone();
    no_files.files.clear();
    write_manifest(&init.data_root, &no_files);
    assert!(
        verify_suite_with_language(&root, "humaneval", Some("python"))
            .expect_err("no files")
            .to_string()
            .contains("does not record any tracked files")
    );

    // case-count mismatch
    let mut wrong_count = base.clone();
    wrong_count.case_count = 42;
    write_manifest(&init.data_root, &wrong_count);
    assert!(
        verify_suite_with_language(&root, "humaneval", Some("python"))
            .expect_err("case count")
            .to_string()
            .contains("case(s) but found")
    );

    // restored manifest verifies cleanly
    write_manifest(&init.data_root, &base);
    verify_suite_with_language(&root, "humaneval", Some("python")).expect("clean verify");
}

/// Write a manifest into a suite data root (test-fixture helper).
fn write_manifest(data_root: &Path, manifest: &BenchDataManifest) {
    let raw = serde_json::to_string_pretty(manifest).expect("serialize manifest");
    std::fs::write(data_root.join("manifest.json"), raw).expect("write manifest");
}

#[test]
fn test_init_suite_verify_only_returns_existing_manifest() {
    let root = work_dir("verify-only");
    init_suite(&root, "humaneval", false, false).expect("init");
    let outcome = ragent_bench::data::init_suite_mode(
        &root,
        "humaneval",
        BenchInitMode::Sample,
        Some("python"),
        false,
        true,
    )
    .expect("verify-only init");
    assert!(outcome.verified_only);
    assert!(!outcome.created);
    assert_eq!(outcome.manifest.case_count, 1);
}

#[test]
fn test_init_suite_with_language_rejects_unsupported_language() {
    let root = work_dir("bad-language");
    let error = init_suite_with_language(&root, "humaneval", Some("cobol"), false, false)
        .expect_err("unsupported language");
    assert!(error.to_string().contains("does not support language"));
}

#[test]
fn test_init_target_suite_reports_start_and_finish_progress() {
    let root = work_dir("progress");
    let mut events: Vec<BenchInitProgressEvent> = Vec::new();
    let outcomes = init_target_with_progress(
        &root,
        &BenchInitTarget::Suite("mbpp".to_string()),
        BenchInitMode::Sample,
        Some("python"),
        false,
        false,
        |event| events.push(event),
    )
    .expect("init mbpp");

    assert_eq!(outcomes.len(), 1);
    assert_eq!(events.len(), 2);
    assert!(matches!(
        &events[0],
        BenchInitProgressEvent::Starting {
            suite_id,
            language,
            mode: BenchInitMode::Sample,
            verify_only: false,
        } if suite_id == "mbpp" && language == "python"
    ));
    assert!(matches!(
        &events[1],
        BenchInitProgressEvent::Finished {
            suite_id,
            language,
            case_count: 1,
            verify_only: false,
            ..
        } if suite_id == "mbpp" && language == "python"
    ));
}

#[test]
fn test_init_target_rejects_unknown_suite() {
    let root = work_dir("unknown-suite");
    let error = init_target_with_progress(
        &root,
        &BenchInitTarget::Suite("nope".to_string()),
        BenchInitMode::Sample,
        None,
        false,
        false,
        |_| {},
    )
    .expect_err("unknown suite");
    assert!(error.to_string().contains("unknown benchmark suite"));
}

#[test]
fn test_slugify_path_segment_folds_and_falls_back() {
    assert_eq!(slugify_path_segment("a b  c"), "a_b_c");
    assert_eq!(
        slugify_path_segment("nvidia/Llama 3.1:70B"),
        "nvidia_Llama_3.1_70B"
    );
    // Only underscores are folded/trimmed; `.` and `-` are preserved verbatim.
    assert_eq!(slugify_path_segment("--keep.dots--"), "--keep.dots--");
    assert_eq!(slugify_path_segment("__x__"), "x");
    assert_eq!(slugify_path_segment("a__b"), "a_b");
    assert_eq!(slugify_path_segment("!!!"), "unknown");
    assert_eq!(slugify_path_segment(""), "unknown");
    assert_eq!(slugify_path_segment("abc"), "abc");
}

#[test]
fn test_resolve_selected_model_rejects_empty_and_missing_slash() {
    assert!(
        resolve_selected_model("   ")
            .expect_err("empty selection")
            .to_string()
            .contains("no model is currently selected")
    );
    assert!(
        resolve_selected_model("justamodel")
            .expect_err("no provider")
            .to_string()
            .contains("provider/model")
    );
    let resolved = resolve_selected_model(" ollama/qwen2.5:1.5b ").expect("trimmed selection");
    assert_eq!(resolved.provider_id, "ollama");
    assert_eq!(resolved.model_id, "qwen2.5:1.5b");
    assert_eq!(resolved.provider_name, "ollama");
    assert!(resolved.model_display_name.is_none());
    assert!(resolved.context_window.is_none());
    assert_eq!(resolved.thinking_config, ThinkingConfig::off());
}

#[test]
fn test_mock_runner_selection_and_sample_generation() {
    let runner = MockBenchModelRunner::new(
        sample_selection(),
        vec!["first".to_string(), "second".to_string()],
    );
    assert_eq!(runner.selection().provider_id, "anthropic");

    let cancel = AtomicBool::new(false);
    let options = BenchRunOptions {
        samples: 2,
        deterministic: true,
        ..BenchRunOptions::default()
    };
    let result = runner
        .generate("prompt", &options, &cancel)
        .expect("generate");
    assert_eq!(result.samples.len(), 2);
    assert_eq!(result.samples[0].text, "first");
    assert_eq!(result.samples[1].text, "second");
    assert_eq!(result.samples[0].input_tokens, Some(16));
    assert_eq!(result.samples[0].output_tokens, Some(8));
    assert_eq!(result.samples[0].finish_reason.as_deref(), Some("stop"));
}

#[test]
fn test_mock_runner_clamps_zero_samples_and_reuses_first_output() {
    let runner = MockBenchModelRunner::new(sample_selection(), vec!["only".to_string()]);
    let cancel = AtomicBool::new(false);
    let result = runner
        .generate(
            "prompt",
            &BenchRunOptions {
                samples: 0,
                ..BenchRunOptions::default()
            },
            &cancel,
        )
        .expect("generate");
    assert_eq!(result.samples.len(), 1);
    assert_eq!(result.samples[0].text, "only");

    // Requesting more samples than outputs falls back to the first output.
    let many = runner
        .generate(
            "prompt",
            &BenchRunOptions {
                samples: 3,
                ..BenchRunOptions::default()
            },
            &cancel,
        )
        .expect("generate many");
    assert_eq!(many.samples.len(), 3);
    assert!(many.samples.iter().all(|sample| sample.text == "only"));
}

#[test]
fn test_mock_runner_with_no_outputs_yields_empty_samples() {
    let runner = MockBenchModelRunner::new(sample_selection(), Vec::new());
    let result = runner
        .generate(
            "prompt",
            &BenchRunOptions::default(),
            &AtomicBool::new(false),
        )
        .expect("generate");
    assert_eq!(result.samples.len(), 1);
    assert_eq!(result.samples[0].text, "");
}

#[test]
fn test_mock_runner_honours_cancellation() {
    let runner = MockBenchModelRunner::new(sample_selection(), vec!["x".to_string()]);
    let error = runner
        .generate(
            "prompt",
            &BenchRunOptions::default(),
            &AtomicBool::new(true),
        )
        .expect_err("cancelled");
    assert!(error.to_string().contains("cancelled"));
}
