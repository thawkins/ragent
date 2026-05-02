//! Benchmark core regression tests.

use calamine::Reader;
use chrono::NaiveDate;
use ragent_bench::{
    ARTIFACTS_COLUMNS, ARTIFACTS_SHEET, BenchArtifactRecord, BenchCaseResult, BenchCommand,
    BenchInitMode, BenchInitTarget, BenchResultSummary, BenchRunConfig, BenchRunOptions,
    BenchTarget, CASES_COLUMNS, CASES_SHEET, METRICS_COLUMNS, METRICS_SHEET, MockBenchModelRunner,
    RUN_COLUMNS, RUN_SHEET, all_suites, bench_data_root, bench_data_root_for_language,
    expand_target, init_suite, init_suite_with_language, init_target, parse_bench_command,
    resolve_model_context, resolve_selected_model, run_target, verify_suite,
    workbook_debug_sidecar_path, workbook_output_path, workbook_resume_state_path,
    write_benchmark_workbook,
};
use ragent_core::Config;
use ragent_core::provider;
use ragent_core::storage::Storage;
use ragent_types::{ThinkingConfig, ThinkingLevel};
use sha2::{Digest, Sha256};
use std::fs;
use std::sync::atomic::AtomicBool;

#[test]
fn test_parse_bench_run_command() {
    let command = parse_bench_command(
        "run livecodebench --limit 5 --samples 2 --subset lite --release release_v6 --scenario codegeneration --language python --temperature 0.2 --top-p 0.9 --max-tokens 512 --deterministic --since 2026-01-01 --until 2026-01-31 --resume --no-exec --yes",
    )
    .expect("parse bench run");
    assert_eq!(
        command,
        BenchCommand::Run {
            target: BenchTarget::Suite("livecodebench".to_string()),
            options: BenchRunOptions {
                limit: Some(5),
                samples: 2,
                subset: Some("lite".to_string()),
                release: Some("release_v6".to_string()),
                scenario: Some("codegeneration".to_string()),
                language: Some("python".to_string()),
                temperature: Some(0.2),
                top_p: Some(0.9),
                max_tokens: Some(512),
                deterministic: true,
                since: Some(NaiveDate::from_ymd_opt(2026, 1, 1).expect("since date")),
                until: Some(NaiveDate::from_ymd_opt(2026, 1, 31).expect("until date")),
                resume: true,
                no_exec: true,
                yes: true,
            },
        }
    );
}

#[test]
fn test_parse_bench_run_cap_alias_command() {
    let command = parse_bench_command("run humaneval --cap 5").expect("parse bench run cap");
    assert_eq!(
        command,
        BenchCommand::Run {
            target: BenchTarget::Suite("humaneval".to_string()),
            options: BenchRunOptions {
                limit: Some(5),
                ..BenchRunOptions::default()
            },
        }
    );
}

#[test]
fn test_parse_bench_init_all_command() {
    let command = parse_bench_command("init all --verify-only").expect("parse bench init all");
    assert_eq!(
        command,
        BenchCommand::Init {
            target: BenchInitTarget::All,
            mode: BenchInitMode::Sample,
            language: None,
            force_download: false,
            verify_only: true,
        }
    );
}

#[test]
fn test_parse_bench_init_full_command() {
    let command = parse_bench_command("init full").expect("parse bench init full");
    assert_eq!(
        command,
        BenchCommand::Init {
            target: BenchInitTarget::Full,
            mode: BenchInitMode::Full,
            language: None,
            force_download: false,
            verify_only: false,
        }
    );
}

#[test]
fn test_parse_bench_init_language_command() {
    let command = parse_bench_command("init multipl-e --language rust").expect("parse bench init");
    assert_eq!(
        command,
        BenchCommand::Init {
            target: BenchInitTarget::Suite("multipl-e".to_string()),
            mode: BenchInitMode::Sample,
            language: Some("rust".to_string()),
            force_download: false,
            verify_only: false,
        }
    );
}

#[test]
fn test_parse_bench_open_last_command() {
    let command = parse_bench_command("open last").expect("parse bench open last");
    assert_eq!(command, BenchCommand::OpenLast);
}

#[test]
fn test_bench_data_root_builder() {
    let root = tempfile::tempdir().expect("tempdir");
    let path = bench_data_root(root.path(), "humaneval");
    assert!(path.ends_with("benches/data/humaneval/python"));
}

#[test]
fn test_model_resolution_slugifies_path_segments() {
    let resolved =
        resolve_selected_model("huggingface/nvidia/Llama 3.1:70B").expect("resolve model");
    assert_eq!(resolved.provider_id, "huggingface");
    assert_eq!(resolved.model_id, "nvidia/Llama 3.1:70B");
    assert_eq!(resolved.model_slug, "nvidia_Llama_3.1_70B");
}

#[test]
fn test_workbook_path_generation() {
    let root = tempfile::tempdir().expect("tempdir");
    let path = workbook_output_path(
        root.path(),
        "humaneval",
        "python",
        NaiveDate::from_ymd_opt(2026, 4, 28).expect("date"),
        "anthropic",
        "claude sonnet:4/20250514",
    );
    assert!(
        path.ends_with(
            "benches/humaneval/python/2026-04-28/anthropic/claude_sonnet_4_20250514.xlsx"
        )
    );
}

#[test]
fn test_init_and_verify_suite_manifest() {
    let root = tempfile::tempdir().expect("tempdir");
    let init = init_suite(root.path(), "humaneval", false, false).expect("init humaneval");
    assert!(init.created);
    assert_eq!(init.manifest.bench_name, "humaneval");

    let manifest = verify_suite(root.path(), "humaneval").expect("verify humaneval");
    assert_eq!(manifest.case_count, 1);
    assert_eq!(manifest.status, "ready");
    assert_eq!(manifest.language, "python");
    assert_eq!(manifest.manifest_version, 4);
    assert_eq!(manifest.dataset_dir, "dataset");
    assert_eq!(manifest.case_file, "dataset/cases.jsonl");
    assert_eq!(manifest.sources.len(), 2);
    assert_eq!(manifest.files.len(), 1);
    assert_eq!(manifest.files[0].relative_path, "dataset/cases.jsonl");
    assert_eq!(manifest.files[0].sha256.len(), 64);
}

#[test]
fn test_init_target_all_creates_all_suite_roots() {
    let root = tempfile::tempdir().expect("tempdir");
    let outcomes = init_target(
        root.path(),
        &BenchInitTarget::All,
        BenchInitMode::Sample,
        None,
        false,
        false,
    )
    .expect("init all");

    assert_eq!(outcomes.len(), all_suites().len());
    for suite in all_suites() {
        let manifest = verify_suite(root.path(), suite.id)
            .unwrap_or_else(|error| panic!("verify {}: {error}", suite.id));
        assert_eq!(manifest.bench_name, suite.id);
    }
}

#[test]
fn test_init_target_full_errors_until_all_suites_support_full_ingestion() {
    let root = tempfile::tempdir().expect("tempdir");
    let error = init_target(
        root.path(),
        &BenchInitTarget::Full,
        BenchInitMode::Full,
        None,
        false,
        false,
    )
    .expect_err("full init should still be gated");
    assert!(error.to_string().contains("full"));
    assert!(error.to_string().contains("apps"));
}

#[test]
fn test_mbpp_registry_lists_multilingual_full_dataset_languages() {
    let mbpp = all_suites()
        .iter()
        .find(|suite| suite.id == "mbpp")
        .expect("mbpp suite");

    assert!(mbpp.languages.contains(&"python"));
    assert!(mbpp.languages.contains(&"rust"));
    assert!(mbpp.languages.contains(&"typescript"));
    assert!(mbpp.language_source_note.contains("gabeorlanski/bc-mbpp"));
}

#[test]
fn test_humaneval_registry_lists_humanevalpack_languages() {
    let humaneval = all_suites()
        .iter()
        .find(|suite| suite.id == "humaneval")
        .expect("humaneval suite");

    assert!(humaneval.languages.contains(&"python"));
    assert!(humaneval.languages.contains(&"rust"));
    assert!(humaneval.languages.contains(&"javascript"));
    assert!(
        humaneval
            .language_source_note
            .contains("bigcode/humanevalpack")
    );
}

#[test]
fn test_registry_includes_phase_one_and_future_suites() {
    let suites = all_suites();
    assert!(suites.iter().any(|suite| suite.id == "humaneval"));
    assert!(suites.iter().any(|suite| suite.id == "mbpp"));
    assert!(suites.iter().any(|suite| suite.id == "apps"));
    assert!(suites.iter().any(|suite| suite.id == "ds1000"));
    assert!(suites.iter().any(|suite| suite.id == "multipl-e"));
    assert!(suites.iter().any(|suite| suite.id == "repobench"));
    assert!(suites.iter().any(|suite| suite.id == "crosscodeeval"));
    assert!(suites.iter().any(|suite| suite.id == "swebench-lite"));
    assert!(suites.iter().any(|suite| suite.id == "livecodebench"));
    assert!(suites.iter().any(|suite| suite.id == "bigcodebench"));
}

#[test]
fn test_quick_profile_expands_to_humaneval_and_mbpp() {
    let suites = expand_target(&BenchTarget::Profile("quick".to_string())).expect("expand quick");
    let ids: Vec<_> = suites.iter().map(|suite| suite.id).collect();
    assert_eq!(ids, vec!["humaneval", "mbpp"]);
}

#[test]
fn test_resolve_model_context_uses_registry_config_storage_and_explicit_thinking() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .set_setting("generic_openai_api_base", "https://example.invalid/v1")
        .expect("set api base");
    let registry = provider::create_default_registry();
    let config = Config::default();

    let resolved = resolve_model_context(
        "generic_openai/gpt-4o-mini",
        &registry,
        &storage,
        &config,
        Some(ThinkingConfig::new(ThinkingLevel::High)),
    )
    .expect("resolve model context");

    assert_eq!(resolved.provider_id, "generic_openai");
    assert_eq!(resolved.provider_name, "Generic OpenAI API");
    assert_eq!(resolved.model_id, "gpt-4o-mini");
    assert_eq!(
        resolved.base_url.as_deref(),
        Some("https://example.invalid/v1")
    );
    assert_eq!(resolved.thinking_config.level, ThinkingLevel::High);
}

#[test]
fn test_init_suite_reuses_existing_valid_data_root() {
    let root = tempfile::tempdir().expect("tempdir");
    let first = init_suite(root.path(), "humaneval", false, false).expect("first init");
    let second = init_suite(root.path(), "humaneval", false, false).expect("second init");

    assert!(first.created);
    assert!(!second.created);
    assert_eq!(
        first.manifest.initialized_at_utc,
        second.manifest.initialized_at_utc
    );
}

#[test]
fn test_init_suite_with_language_partitions_multipl_e_data() {
    let root = tempfile::tempdir().expect("tempdir");
    let init = init_suite_with_language(root.path(), "multipl-e", Some("rust"), false, false)
        .expect("init multipl-e rust");

    assert_eq!(init.language, "rust");
    assert!(init.data_root.ends_with("benches/data/multipl-e/rust"));
    assert!(
        bench_data_root_for_language(root.path(), "multipl-e", "rust")
            .ends_with("benches/data/multipl-e/rust")
    );
}

#[test]
fn test_verify_suite_detects_checksum_mismatch() {
    let root = tempfile::tempdir().expect("tempdir");
    init_suite(root.path(), "humaneval", false, false).expect("init humaneval");
    let cases_path = root
        .path()
        .join("benches/data/humaneval/python/dataset/cases.jsonl");
    std::fs::write(&cases_path, "tampered\n").expect("tamper cases");

    let error = verify_suite(root.path(), "humaneval").expect_err("checksum mismatch");
    let message = error.to_string();
    assert!(
        message.contains("checksum mismatch") || message.contains("size mismatch"),
        "unexpected verification error: {message}"
    );
}

#[test]
fn test_init_suite_rebuilds_invalid_existing_data() {
    let root = tempfile::tempdir().expect("tempdir");
    init_suite(root.path(), "humaneval", false, false).expect("init humaneval");
    let cases_path = root
        .path()
        .join("benches/data/humaneval/python/dataset/cases.jsonl");
    std::fs::write(&cases_path, "tampered\n").expect("tamper cases");

    let rebuilt = init_suite(root.path(), "humaneval", false, false).expect("rebuild invalid data");
    assert!(rebuilt.created);
    let manifest = verify_suite(root.path(), "humaneval").expect("verify rebuilt humaneval");
    assert_eq!(manifest.case_count, 1);
}

#[test]
fn test_run_target_uses_mock_runner_generation_metadata() {
    let root = tempfile::tempdir().expect("tempdir");
    init_suite(root.path(), "humaneval", false, false).expect("init humaneval");
    let selection =
        resolve_selected_model("anthropic/claude-sonnet-4-20250514").expect("resolve model");
    let runner = MockBenchModelRunner::new(
        selection,
        vec!["def add(a, b):\n    return a + b".to_string()],
    );

    let outcome = run_target(
        root.path(),
        &runner,
        "/bench run humaneval --samples 2 --deterministic",
        &BenchTarget::Suite("humaneval".to_string()),
        &BenchRunOptions {
            samples: 2,
            deterministic: true,
            ..BenchRunOptions::default()
        },
        &AtomicBool::new(false),
    )
    .expect("run benchmark");

    assert_eq!(outcome.workbook_paths.len(), 1);
    assert!(outcome.workbook_paths[0].exists());
    assert_eq!(outcome.summaries[0].sample_count, 1);
    assert!(
        outcome
            .summaries
            .iter()
            .any(|summary| summary.metric_name == "pass_at_1")
    );
    assert!(
        outcome
            .summaries
            .iter()
            .any(|summary| summary.metric_name == "pass_at_2")
    );
    assert!(outcome.message.contains("2 sample(s) generated"));
}

#[test]
fn test_run_target_all_writes_one_workbook_per_suite() {
    let root = tempfile::tempdir().expect("tempdir");
    init_target(
        root.path(),
        &BenchInitTarget::All,
        BenchInitMode::Sample,
        None,
        false,
        false,
    )
    .expect("init all");
    let selection =
        resolve_selected_model("anthropic/claude-sonnet-4-20250514").expect("resolve model");
    let runner = MockBenchModelRunner::new(
        selection,
        vec!["def solve():\n    return 'bench-output'".to_string()],
    );

    let outcome = run_target(
        root.path(),
        &runner,
        "/bench run all --yes",
        &BenchTarget::All,
        &BenchRunOptions {
            yes: true,
            ..BenchRunOptions::default()
        },
        &AtomicBool::new(false),
    )
    .expect("run all benchmarks");

    assert_eq!(outcome.workbook_paths.len(), all_suites().len());
    assert!(outcome.message.contains("bigcodebench"));
    assert!(outcome.message.contains("swebench-lite"));
}

#[test]
fn test_humaneval_adapter_emits_pass_at_k_metrics() {
    let root = tempfile::tempdir().expect("tempdir");
    init_suite(root.path(), "humaneval", false, false).expect("init humaneval");
    let selection =
        resolve_selected_model("anthropic/claude-sonnet-4-20250514").expect("resolve model");
    let runner = MockBenchModelRunner::new(
        selection,
        vec!["def add(a, b):\n    return a + b".to_string()],
    );

    let outcome = run_target(
        root.path(),
        &runner,
        "/bench run humaneval",
        &BenchTarget::Suite("humaneval".to_string()),
        &BenchRunOptions::default(),
        &AtomicBool::new(false),
    )
    .expect("run humaneval");

    assert!(
        outcome
            .summaries
            .iter()
            .any(|summary| summary.metric_name == "pass_at_1" && summary.metric_value == 1.0)
    );
    assert!(
        outcome
            .summaries
            .iter()
            .any(|summary| summary.metric_name == "pass_at_1")
    );
    assert_eq!(
        outcome
            .summaries
            .iter()
            .filter(|summary| summary.metric_name == "pass_at_1")
            .count(),
        1
    );
}

#[test]
fn test_humaneval_executes_hidden_tests_for_body_only_completion() {
    let root = tempfile::tempdir().expect("tempdir");
    init_suite(root.path(), "humaneval", false, false).expect("init humaneval");
    let selection =
        resolve_selected_model("anthropic/claude-sonnet-4-20250514").expect("resolve model");
    let runner = MockBenchModelRunner::new(selection, vec!["return a + b".to_string()]);

    let outcome = run_target(
        root.path(),
        &runner,
        "/bench run humaneval",
        &BenchTarget::Suite("humaneval".to_string()),
        &BenchRunOptions::default(),
        &AtomicBool::new(false),
    )
    .expect("run humaneval body completion");

    assert!(
        outcome
            .summaries
            .iter()
            .any(|summary| summary.metric_name == "pass_at_1" && summary.metric_value == 1.0)
    );
}

#[test]
fn test_humaneval_executes_rust_native_tests() {
    let root = tempfile::tempdir().expect("tempdir");
    let data_root = bench_data_root_for_language(root.path(), "humaneval", "rust");
    fs::create_dir_all(data_root.join("dataset")).expect("create rust humaneval data root");

    let cases_path = data_root.join("dataset/cases.jsonl");
    let case = serde_json::json!({
        "case_id": "Rust/0",
        "prompt": "Write a Rust function `has_close_elements(numbers:Vec<f32>, threshold: f32) -> bool`.",
        "starter_code": "fn has_close_elements(numbers: Vec<f32>, threshold: f32) -> bool {\n",
        "reference": "    false\n}\n",
        "language": "rust",
        "test_code": "PLACEHOLDER_CODE_BODY\n\n#[cfg(test)]\nmod tests {\n    use super::*;\n\n    #[test]\n    fn test_has_close_elements() {\n        assert!(has_close_elements(vec![1.0, 2.0, 2.1], 0.2));\n        assert!(!has_close_elements(vec![1.0, 2.0, 3.0], 0.1));\n    }\n}\n",
        "entry_point": "has_close_elements",
        "execution_commands": [["rustc", "--test", "__FILENAME__", "-o", "./__FILENAME__.exe"], ["./__FILENAME__.exe"]],
        "execution_timeouts_secs": [10, 10],
        "source_extension": "rs"
    });
    fs::write(&cases_path, format!("{case}\n")).expect("write rust humaneval case");
    let cases_bytes = fs::read(&cases_path).expect("read rust humaneval case bytes");
    let cases_sha = format!("{:x}", Sha256::digest(&cases_bytes));

    let manifest_path = data_root.join("manifest.json");
    let manifest = serde_json::json!({
        "bench_name": "humaneval",
        "display_name": "HumanEval",
        "language": "rust",
        "revision": "HumanEvalPack-1.0",
        "sources": [
            {"kind": "dataset", "url": "https://github.com/openai/human-eval"},
            {"kind": "dataset", "url": "https://huggingface.co/datasets/bigcode/humanevalpack"}
        ],
        "initialized_at_utc": "2026-05-02T00:00:00Z",
        "dataset_dir": "dataset",
        "case_file": "dataset/cases.jsonl",
        "case_count": 1,
        "status": "ready",
        "manifest_version": 4,
        "files": [
            {"relative_path": "dataset/cases.jsonl", "sha256": cases_sha, "bytes": cases_bytes.len()}
        ]
    });
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).expect("serialize manifest"),
    )
    .expect("write rust humaneval manifest");

    let selection =
        resolve_selected_model("anthropic/claude-sonnet-4-20250514").expect("resolve model");
    let runner = MockBenchModelRunner::new(
        selection,
        vec![
            "for i in 0..numbers.len() {\n        for j in (i + 1)..numbers.len() {\n            if (numbers[i] - numbers[j]).abs() < threshold {\n                return true;\n            }\n        }\n    }\n    false\n}"
                .to_string(),
        ],
    );

    let outcome = run_target(
        root.path(),
        &runner,
        "/bench run humaneval --language rust",
        &BenchTarget::Suite("humaneval".to_string()),
        &BenchRunOptions {
            language: Some("rust".to_string()),
            ..BenchRunOptions::default()
        },
        &AtomicBool::new(false),
    )
    .expect("run humaneval rust");

    assert!(
        outcome
            .summaries
            .iter()
            .any(|summary| summary.metric_name == "pass_at_1" && summary.metric_value == 1.0)
    );
}

#[test]
fn test_repobench_adapter_emits_exact_match_and_edit_similarity() {
    let root = tempfile::tempdir().expect("tempdir");
    init_suite(root.path(), "repobench", false, false).expect("init repobench");
    let selection =
        resolve_selected_model("anthropic/claude-sonnet-4-20250514").expect("resolve model");
    let runner = MockBenchModelRunner::new(selection, vec!["repository_completion(".to_string()]);

    let outcome = run_target(
        root.path(),
        &runner,
        "/bench run repobench",
        &BenchTarget::Suite("repobench".to_string()),
        &BenchRunOptions::default(),
        &AtomicBool::new(false),
    )
    .expect("run repobench");

    assert!(
        outcome
            .summaries
            .iter()
            .any(|summary| summary.metric_name == "exact_match" && summary.metric_value == 0.0)
    );
    let edit_similarity = outcome
        .summaries
        .iter()
        .find(|summary| summary.metric_name == "edit_similarity")
        .expect("edit similarity metric");
    assert!(edit_similarity.metric_value > 0.0);
    assert!(edit_similarity.metric_value < 1.0);
}

#[test]
fn test_run_target_no_exec_marks_mbpp_as_skipped() {
    let root = tempfile::tempdir().expect("tempdir");
    init_suite(root.path(), "mbpp", false, false).expect("init mbpp");
    let selection =
        resolve_selected_model("anthropic/claude-sonnet-4-20250514").expect("resolve model");
    let runner = MockBenchModelRunner::new(
        selection,
        vec!["def is_palindrome(s):\n    return s == s[::-1]".to_string()],
    );

    let outcome = run_target(
        root.path(),
        &runner,
        "/bench run mbpp --no-exec",
        &BenchTarget::Suite("mbpp".to_string()),
        &BenchRunOptions {
            no_exec: true,
            ..BenchRunOptions::default()
        },
        &AtomicBool::new(false),
    )
    .expect("run mbpp no-exec");

    assert!(
        outcome
            .summaries
            .iter()
            .any(|summary| summary.metric_name == "accuracy" && summary.skipped_count == Some(1))
    );
}

#[test]
fn test_run_target_executes_mbpp_assertions() {
    let root = tempfile::tempdir().expect("tempdir");
    init_suite(root.path(), "mbpp", false, false).expect("init mbpp");
    let selection =
        resolve_selected_model("anthropic/claude-sonnet-4-20250514").expect("resolve model");
    let runner = MockBenchModelRunner::new(
        selection,
        vec!["def is_palindrome(s):\n    return ''.join(reversed(s)) == s".to_string()],
    );

    let outcome = run_target(
        root.path(),
        &runner,
        "/bench run mbpp",
        &BenchTarget::Suite("mbpp".to_string()),
        &BenchRunOptions::default(),
        &AtomicBool::new(false),
    )
    .expect("run mbpp");

    assert!(
        outcome
            .summaries
            .iter()
            .any(|summary| summary.metric_name == "accuracy" && summary.metric_value == 1.0)
    );
}

#[test]
fn test_run_target_executes_mbpp_rust_native_harness() {
    let root = tempfile::tempdir().expect("tempdir");
    let data_root = bench_data_root_for_language(root.path(), "mbpp", "rust");
    fs::create_dir_all(data_root.join("dataset")).expect("create rust data root");

    let cases_path = data_root.join("dataset/cases.jsonl");
    let case = serde_json::json!({
        "case_id": "mbpp-rust-1",
        "prompt": "pub fn is_palindrome(s: &str) -> bool {",
        "reference": "",
        "language": "rust",
        "test_code": "PLACEHOLDER_CODE_BODY\n\nfn main() {\n    println!(\"TEST-0...{}\", if PLACEHOLDER_FN_NAME(\"level\") { \"PASSED\" } else { \"FAILED\" });\n    println!(\"TEST-1...{}\", if !PLACEHOLDER_FN_NAME(\"abc\") { \"PASSED\" } else { \"FAILED\" });\n}\n",
        "entry_point": "is_palindrome",
        "entry_class": "Solution",
        "execution_commands": [["rustc", "__FILENAME__", "-o", "./__FILENAME__.exe"], ["./__FILENAME__.exe"]],
        "execution_timeouts_secs": [10, 10],
        "source_extension": "rs"
    });
    fs::write(&cases_path, format!("{case}\n")).expect("write rust mbpp case");
    let cases_bytes = fs::read(&cases_path).expect("read rust mbpp case bytes");
    let cases_sha = format!("{:x}", Sha256::digest(&cases_bytes));

    let manifest_path = data_root.join("manifest.json");
    let manifest = serde_json::json!({
        "bench_name": "mbpp",
        "display_name": "MBPP",
        "language": "rust",
        "revision": "BC-MBPP-1.0",
        "sources": [
            {"kind": "dataset", "url": "https://huggingface.co/datasets/google-research-datasets/mbpp"},
            {"kind": "dataset", "url": "https://huggingface.co/datasets/gabeorlanski/bc-mbpp"}
        ],
        "initialized_at_utc": "2026-05-02T00:00:00Z",
        "dataset_dir": "dataset",
        "case_file": "dataset/cases.jsonl",
        "case_count": 1,
        "status": "ready",
        "manifest_version": 4,
        "files": [
            {"relative_path": "dataset/cases.jsonl", "sha256": cases_sha, "bytes": cases_bytes.len()}
        ]
    });
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).expect("serialize manifest"),
    )
    .expect("write rust mbpp manifest");

    let selection =
        resolve_selected_model("anthropic/claude-sonnet-4-20250514").expect("resolve model");
    let runner = MockBenchModelRunner::new(
        selection,
        vec![
            "rust\npub fn is_palindrome(s: &str) -> bool {\n    s.chars().eq(s.chars().rev())\n}"
                .to_string(),
        ],
    );

    let outcome = run_target(
        root.path(),
        &runner,
        "/bench run mbpp --language rust",
        &BenchTarget::Suite("mbpp".to_string()),
        &BenchRunOptions {
            language: Some("rust".to_string()),
            ..BenchRunOptions::default()
        },
        &AtomicBool::new(false),
    )
    .expect("run mbpp rust");

    assert!(
        outcome
            .summaries
            .iter()
            .any(|summary| summary.metric_name == "accuracy" && summary.metric_value == 1.0)
    );
}

#[test]
fn test_quick_profile_runs_humaneval_and_mbpp() {
    let root = tempfile::tempdir().expect("tempdir");
    init_suite(root.path(), "humaneval", false, false).expect("init humaneval");
    init_suite(root.path(), "mbpp", false, false).expect("init mbpp");
    let selection =
        resolve_selected_model("anthropic/claude-sonnet-4-20250514").expect("resolve model");
    let runner = MockBenchModelRunner::new(
        selection,
        vec!["def add(a, b):\n    return a + b".to_string()],
    );

    let outcome = run_target(
        root.path(),
        &runner,
        "/bench run quick",
        &BenchTarget::Profile("quick".to_string()),
        &BenchRunOptions::default(),
        &AtomicBool::new(false),
    )
    .expect("run quick profile");

    assert_eq!(outcome.workbook_paths.len(), 2);
    assert!(
        outcome
            .summaries
            .iter()
            .any(|summary| summary.bench_name == "humaneval")
    );
    assert!(
        outcome
            .summaries
            .iter()
            .any(|summary| summary.bench_name == "mbpp")
    );
}

#[test]
fn test_apps_adapter_emits_accuracy_and_codebleu() {
    let root = tempfile::tempdir().expect("tempdir");
    init_suite(root.path(), "apps", false, false).expect("init apps");
    let selection =
        resolve_selected_model("anthropic/claude-sonnet-4-20250514").expect("resolve model");
    let runner = MockBenchModelRunner::new(
        selection,
        vec!["a, b = map(int, input().split())\nprint(a + b)".to_string()],
    );

    let outcome = run_target(
        root.path(),
        &runner,
        "/bench run apps",
        &BenchTarget::Suite("apps".to_string()),
        &BenchRunOptions {
            yes: true,
            ..BenchRunOptions::default()
        },
        &AtomicBool::new(false),
    )
    .expect("run apps");

    assert!(
        outcome
            .summaries
            .iter()
            .any(|summary| summary.metric_name == "accuracy")
    );
    assert!(
        outcome
            .summaries
            .iter()
            .any(|summary| summary.metric_name == "codebleu")
    );
}

#[test]
fn test_multipl_e_runs_selected_rust_language_partition() {
    let root = tempfile::tempdir().expect("tempdir");
    init_suite_with_language(root.path(), "multipl-e", Some("rust"), false, false)
        .expect("init multipl-e rust");
    let selection =
        resolve_selected_model("anthropic/claude-sonnet-4-20250514").expect("resolve model");
    let runner = MockBenchModelRunner::new(
        selection,
        vec!["fn max_in_list(items: &[i32]) -> i32 { *items.iter().max().unwrap() }".to_string()],
    );

    let outcome = run_target(
        root.path(),
        &runner,
        "/bench run multipl-e --language rust",
        &BenchTarget::Suite("multipl-e".to_string()),
        &BenchRunOptions {
            language: Some("rust".to_string()),
            yes: true,
            ..BenchRunOptions::default()
        },
        &AtomicBool::new(false),
    )
    .expect("run multipl-e");

    assert!(
        outcome
            .summaries
            .iter()
            .all(|summary| summary.language.as_deref() == Some("rust"))
    );
}

#[test]
fn test_livecodebench_skips_unsupported_scenario() {
    let root = tempfile::tempdir().expect("tempdir");
    init_suite(root.path(), "livecodebench", false, false).expect("init livecodebench");
    let selection =
        resolve_selected_model("anthropic/claude-sonnet-4-20250514").expect("resolve model");
    let runner = MockBenchModelRunner::new(selection, vec!["def solve():\n    pass".to_string()]);

    let outcome = run_target(
        root.path(),
        &runner,
        "/bench run livecodebench --scenario repair",
        &BenchTarget::Suite("livecodebench".to_string()),
        &BenchRunOptions {
            scenario: Some("repair".to_string()),
            yes: true,
            ..BenchRunOptions::default()
        },
        &AtomicBool::new(false),
    )
    .expect("run livecodebench");

    assert!(
        outcome
            .summaries
            .iter()
            .all(|summary| summary.skipped_count == Some(1))
    );
}

#[test]
fn test_bigcodebench_emits_pass_at_k_and_codebleu() {
    let root = tempfile::tempdir().expect("tempdir");
    init_suite(root.path(), "bigcodebench", false, false).expect("init bigcodebench");
    let selection =
        resolve_selected_model("anthropic/claude-sonnet-4-20250514").expect("resolve model");
    let runner = MockBenchModelRunner::new(
        selection,
        vec!["def solution():\n    raise NotImplementedError".to_string()],
    );

    let outcome = run_target(
        root.path(),
        &runner,
        "/bench run bigcodebench --samples 3",
        &BenchTarget::Suite("bigcodebench".to_string()),
        &BenchRunOptions {
            samples: 3,
            yes: true,
            ..BenchRunOptions::default()
        },
        &AtomicBool::new(false),
    )
    .expect("run bigcodebench");

    assert!(
        outcome
            .summaries
            .iter()
            .any(|summary| summary.metric_name == "pass_at_k")
    );
    assert!(
        outcome
            .summaries
            .iter()
            .any(|summary| summary.metric_name == "codebleu")
    );
}

#[test]
fn test_swebench_lite_emits_resolution_metrics() {
    let root = tempfile::tempdir().expect("tempdir");
    init_suite(root.path(), "swebench-lite", false, false).expect("init swebench-lite");
    let selection =
        resolve_selected_model("anthropic/claude-sonnet-4-20250514").expect("resolve model");
    let runner = MockBenchModelRunner::new(
        selection,
        vec!["diff --git a/module.py b/module.py".to_string()],
    );

    let outcome = run_target(
        root.path(),
        &runner,
        "/bench run swebench-lite --yes",
        &BenchTarget::Suite("swebench-lite".to_string()),
        &BenchRunOptions {
            yes: true,
            ..BenchRunOptions::default()
        },
        &AtomicBool::new(false),
    )
    .expect("run swebench-lite");

    assert!(
        outcome
            .summaries
            .iter()
            .any(|summary| summary.metric_name == "resolution_rate" && summary.metric_value == 1.0)
    );
    assert!(
        outcome
            .summaries
            .iter()
            .any(|summary| summary.metric_name == "instances_resolved"
                && summary.metric_value == 1.0)
    );
}

#[test]
fn test_swebench_verified_no_exec_marks_resolution_skipped() {
    let root = tempfile::tempdir().expect("tempdir");
    init_suite(root.path(), "swebench-verified", false, false).expect("init swebench-verified");
    let selection =
        resolve_selected_model("anthropic/claude-sonnet-4-20250514").expect("resolve model");
    let runner = MockBenchModelRunner::new(
        selection,
        vec!["diff --git a/service.py b/service.py".to_string()],
    );

    let outcome = run_target(
        root.path(),
        &runner,
        "/bench run swebench-verified --no-exec --yes",
        &BenchTarget::Suite("swebench-verified".to_string()),
        &BenchRunOptions {
            no_exec: true,
            yes: true,
            ..BenchRunOptions::default()
        },
        &AtomicBool::new(false),
    )
    .expect("run swebench-verified no-exec");

    assert!(
        outcome
            .summaries
            .iter()
            .all(|summary| summary.skipped_count == Some(1))
    );
}

#[test]
fn test_swebench_lite_skips_unsupported_scenario() {
    let root = tempfile::tempdir().expect("tempdir");
    init_suite(root.path(), "swebench-lite", false, false).expect("init swebench-lite");
    let selection =
        resolve_selected_model("anthropic/claude-sonnet-4-20250514").expect("resolve model");
    let runner = MockBenchModelRunner::new(
        selection,
        vec!["diff --git a/module.py b/module.py".to_string()],
    );

    let outcome = run_target(
        root.path(),
        &runner,
        "/bench run swebench-lite --scenario execution --yes",
        &BenchTarget::Suite("swebench-lite".to_string()),
        &BenchRunOptions {
            scenario: Some("execution".to_string()),
            yes: true,
            ..BenchRunOptions::default()
        },
        &AtomicBool::new(false),
    )
    .expect("run swebench-lite execution");

    assert!(
        outcome
            .summaries
            .iter()
            .all(|summary| summary.skipped_count == Some(1))
    );
}

#[test]
fn test_agentic_profile_runs_swebench_lite_and_livecodebench() {
    let root = tempfile::tempdir().expect("tempdir");
    init_suite(root.path(), "swebench-lite", false, false).expect("init swebench-lite");
    init_suite(root.path(), "livecodebench", false, false).expect("init livecodebench");
    let selection =
        resolve_selected_model("anthropic/claude-sonnet-4-20250514").expect("resolve model");
    let runner = MockBenchModelRunner::new(
        selection,
        vec!["diff --git a/module.py b/module.py".to_string()],
    );

    let outcome = run_target(
        root.path(),
        &runner,
        "/bench run agentic --yes",
        &BenchTarget::Profile("agentic".to_string()),
        &BenchRunOptions {
            yes: true,
            ..BenchRunOptions::default()
        },
        &AtomicBool::new(false),
    )
    .expect("run agentic profile");

    assert_eq!(outcome.workbook_paths.len(), 2);
    assert!(
        outcome
            .summaries
            .iter()
            .any(|summary| summary.bench_name == "swebench-lite")
    );
    assert!(
        outcome
            .summaries
            .iter()
            .any(|summary| summary.bench_name == "livecodebench")
    );
}

#[test]
fn test_run_target_resume_reuses_matching_workbook() {
    let root = tempfile::tempdir().expect("tempdir");
    init_suite(root.path(), "humaneval", false, false).expect("init humaneval");
    let selection =
        resolve_selected_model("anthropic/claude-sonnet-4-20250514").expect("resolve model");
    let initial_runner =
        MockBenchModelRunner::new(selection.clone(), vec!["first output".to_string()]);

    let initial = run_target(
        root.path(),
        &initial_runner,
        "/bench run humaneval",
        &BenchTarget::Suite("humaneval".to_string()),
        &BenchRunOptions::default(),
        &AtomicBool::new(false),
    )
    .expect("initial run");

    let workbook_path = initial.workbook_paths[0].clone();
    let workbook_before = std::fs::read(&workbook_path).expect("read initial workbook");
    let resume_state_path = workbook_resume_state_path(&workbook_path);
    let resume_state_raw =
        std::fs::read_to_string(&resume_state_path).expect("read stable resume state");
    let resume_state_json: serde_json::Value =
        serde_json::from_str(&resume_state_raw).expect("parse resume state");
    let config_hash = resume_state_json["config_hash"]
        .as_str()
        .expect("config hash")
        .to_string();
    let debug_state_path = workbook_debug_sidecar_path(&workbook_path, &config_hash[..12]);
    assert!(debug_state_path.exists());

    let resume_runner = MockBenchModelRunner::new(selection, vec!["second output".to_string()]);
    let resumed = run_target(
        root.path(),
        &resume_runner,
        "/bench run humaneval --resume",
        &BenchTarget::Suite("humaneval".to_string()),
        &BenchRunOptions {
            resume: true,
            ..BenchRunOptions::default()
        },
        &AtomicBool::new(false),
    )
    .expect("resume run");

    assert!(resumed.message.contains("resumed existing workbook"));
    assert_eq!(
        workbook_before,
        std::fs::read(&workbook_path).expect("read resumed workbook")
    );
}

#[test]
fn test_run_target_resume_miss_rewrites_when_config_changes() {
    let root = tempfile::tempdir().expect("tempdir");
    init_suite(root.path(), "humaneval", false, false).expect("init humaneval");
    let selection =
        resolve_selected_model("anthropic/claude-sonnet-4-20250514").expect("resolve model");
    let initial_runner =
        MockBenchModelRunner::new(selection.clone(), vec!["first output".to_string()]);

    let initial = run_target(
        root.path(),
        &initial_runner,
        "/bench run humaneval",
        &BenchTarget::Suite("humaneval".to_string()),
        &BenchRunOptions::default(),
        &AtomicBool::new(false),
    )
    .expect("initial run");

    let workbook_path = initial.workbook_paths[0].clone();
    let workbook_before = std::fs::read(&workbook_path).expect("read initial workbook");
    let resume_state_path = workbook_resume_state_path(&workbook_path);
    let resume_state_before =
        std::fs::read_to_string(&resume_state_path).expect("read initial resume state");

    let rerun_runner = MockBenchModelRunner::new(selection, vec!["different output".to_string()]);
    let rerun = run_target(
        root.path(),
        &rerun_runner,
        "/bench run humaneval --resume --max-tokens 17",
        &BenchTarget::Suite("humaneval".to_string()),
        &BenchRunOptions {
            resume: true,
            max_tokens: Some(17),
            ..BenchRunOptions::default()
        },
        &AtomicBool::new(false),
    )
    .expect("rerun with changed config");

    assert!(!rerun.message.contains("resumed existing workbook"));
    assert_ne!(
        workbook_before,
        std::fs::read(&workbook_path).expect("read rewritten workbook")
    );
    assert_ne!(
        resume_state_before,
        std::fs::read_to_string(&resume_state_path).expect("read updated resume state")
    );
}

#[test]
fn test_workbook_schema_headers_are_stable() {
    let root = tempfile::tempdir().expect("tempdir");
    let workbook_path = root.path().join("schema.xlsx");
    let run = BenchRunConfig {
        run_id: "run-1".to_string(),
        bench_name: "humaneval".to_string(),
        date_utc: "2026-04-28".to_string(),
        started_at_utc: "2026-04-28T08:23:01Z".to_string(),
        finished_at_utc: "2026-04-28T08:24:01Z".to_string(),
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        model_id: "claude-sonnet-4-20250514".to_string(),
        model_slug: "claude-sonnet-4-20250514".to_string(),
        thinking_enabled: true,
        thinking_level: "high".to_string(),
        thinking_budget_tokens: Some(16_000),
        command: "/bench run humaneval".to_string(),
        status: "completed".to_string(),
        dataset_revision: "HumanEval-1.2".to_string(),
        evaluator_version: env!("CARGO_PKG_VERSION").to_string(),
        git_commit_sha: "deadbeef".to_string(),
        project_root: root.path().display().to_string(),
        data_root: root
            .path()
            .join("benches/data/humaneval/python")
            .display()
            .to_string(),
        data_revision: "HumanEval-1.2".to_string(),
        notes: "ok".to_string(),
    };
    let metrics = vec![BenchResultSummary {
        run_id: run.run_id.clone(),
        bench_name: run.bench_name.clone(),
        metric_name: "pass_at_1".to_string(),
        metric_value: 1.0,
        metric_unit: "ratio".to_string(),
        split_name: None,
        subset_name: None,
        language: Some("python".to_string()),
        sample_count: 1,
        passed_count: Some(1),
        failed_count: Some(0),
        skipped_count: Some(0),
        notes: "ok".to_string(),
    }];
    let cases = vec![BenchCaseResult {
        run_id: run.run_id.clone(),
        bench_name: run.bench_name.clone(),
        case_id: "case-1".to_string(),
        case_index: 1,
        split_name: None,
        subset_name: None,
        language: Some("python".to_string()),
        prompt_hash: "abc".to_string(),
        response_hash: "def".to_string(),
        status: "passed".to_string(),
        score: Some(1.0),
        duration_ms: Some(10),
        tokens_input: Some(1),
        tokens_output: Some(2),
        samples_generated: 1,
        sandbox_backend: "python".to_string(),
        error_code: None,
        error_message: None,
        notes: "ok".to_string(),
    }];
    let artifacts = vec![BenchArtifactRecord {
        run_id: run.run_id.clone(),
        bench_name: run.bench_name.clone(),
        artifact_kind: "log".to_string(),
        artifact_label: "run log".to_string(),
        relative_path: "artifacts/run.log".to_string(),
        content_hash: Some("123".to_string()),
        notes: "ok".to_string(),
    }];

    write_benchmark_workbook(&workbook_path, &run, &metrics, &cases, &artifacts)
        .expect("write workbook");

    let mut workbook = calamine::open_workbook_auto(&workbook_path).expect("open workbook");
    let sheet_names = workbook.sheet_names().to_vec();
    assert_eq!(
        sheet_names,
        vec![
            RUN_SHEET.to_string(),
            METRICS_SHEET.to_string(),
            CASES_SHEET.to_string(),
            ARTIFACTS_SHEET.to_string()
        ]
    );

    let run_headers = workbook
        .worksheet_range(RUN_SHEET)
        .expect("run sheet")
        .rows()
        .next()
        .expect("run header row")
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    assert_eq!(
        run_headers,
        RUN_COLUMNS
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>()
    );

    let metrics_headers = workbook
        .worksheet_range(METRICS_SHEET)
        .expect("metrics sheet")
        .rows()
        .next()
        .expect("metrics header row")
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    assert_eq!(
        metrics_headers,
        METRICS_COLUMNS
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>()
    );

    let case_headers = workbook
        .worksheet_range(CASES_SHEET)
        .expect("cases sheet")
        .rows()
        .next()
        .expect("cases header row")
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    assert_eq!(
        case_headers,
        CASES_COLUMNS
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>()
    );

    let artifact_headers = workbook
        .worksheet_range(ARTIFACTS_SHEET)
        .expect("artifacts sheet")
        .rows()
        .next()
        .expect("artifacts header row")
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    assert_eq!(
        artifact_headers,
        ARTIFACTS_COLUMNS
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>()
    );
}
