//! Cross-suite adapter coverage for the benchmark suites that the end-to-end
//! runner tests do not exercise directly: APPS, DS-1000, MultiPL-E, RepoBench,
//! CrossCodeEval, SWE-bench Lite/Verified, LiveCodeBench and BigCodeBench.
//!
//! Each adapter's `suite_id`, `build_prompt`, `evaluate_case` (both the
//! `--no-exec` skip and the scored paths) and `summarize` (skipped and scored)
//! branches are driven with small in-memory fixtures.

use ragent_bench::BenchRunOptions;
use ragent_bench::data::BenchCaseFixture;
use ragent_bench::model::{BenchGeneratedSample, BenchGenerationResult};
use ragent_bench::suites::adapter_for_suite;

const PATCH: &str = "diff --git a/x.py b/x.py\n--- a/x.py\n+++ b/x.py\n@@\n-return helper(value)\n+return helper(value)\n";

fn fixture(language: &str, reference: &str) -> BenchCaseFixture {
    BenchCaseFixture {
        case_id: "case-1".to_string(),
        prompt: "solve the task".to_string(),
        starter_code: None,
        reference: reference.to_string(),
        language: language.to_string(),
        test_code: None,
        entry_point: None,
        entry_class: None,
        execution_commands: Vec::new(),
        execution_timeouts_secs: Vec::new(),
        source_extension: None,
    }
}

fn generation(texts: &[&str]) -> BenchGenerationResult {
    BenchGenerationResult {
        samples: texts
            .iter()
            .map(|text| BenchGeneratedSample {
                text: (*text).to_string(),
                input_tokens: Some(1),
                output_tokens: Some(1),
                finish_reason: Some("stop".to_string()),
                duration_ms: 1,
            })
            .collect(),
    }
}

/// Every suite that exposes a native adapter, with a representative reference.
fn every_adapter() -> Vec<(&'static str, BenchCaseFixture)> {
    vec![
        (
            "apps",
            fixture("python", "a, b = map(int, input().split())\nprint(a + b)"),
        ),
        ("ds1000", fixture("python", "df[df['x'] > 10]")),
        (
            "multipl-e",
            fixture("rust", "fn max_in_list(items: &[i32]) -> i32 { 0 }"),
        ),
        ("repobench", fixture("python", "repository_completion()")),
        ("crosscodeeval", fixture("python", "return helper(value)")),
        ("swebench-lite", fixture("diff", PATCH)),
        ("swebench-verified", fixture("diff", PATCH)),
        ("livecodebench", fixture("python", "def solve():\n    pass")),
        (
            "bigcodebench",
            fixture("python", "def solution():\n    raise NotImplementedError"),
        ),
        ("humaneval", fixture("python", "return a + b")),
        ("mbpp", fixture("python", "def f(): return 1")),
    ]
}

#[test]
fn test_every_adapter_reports_its_suite_id() {
    for (suite_id, _) in every_adapter() {
        let adapter = adapter_for_suite(suite_id)
            .unwrap_or_else(|error| panic!("adapter for {suite_id}: {error}"));
        assert_eq!(adapter.suite_id(), suite_id);
    }
}

#[test]
fn test_every_adapter_build_prompt_mentions_the_task() {
    for (suite_id, case) in every_adapter() {
        let adapter = adapter_for_suite(suite_id).expect("adapter");
        let prompt = adapter.build_prompt(&case, &BenchRunOptions::default());
        assert!(
            prompt.contains("solve the task"),
            "{suite_id} prompt should embed the case prompt"
        );
    }
}

#[test]
fn test_every_adapter_no_exec_returns_skipped_status() {
    let options = BenchRunOptions {
        no_exec: true,
        ..BenchRunOptions::default()
    };
    for (suite_id, case) in every_adapter() {
        let adapter = adapter_for_suite(suite_id).expect("adapter");
        let evaluation = adapter.evaluate_case(&case, &generation(&[&case.reference]), &options);
        assert_eq!(evaluation.status, "skipped", "{suite_id} no-exec status");

        // Summaries must still produce at least one metric row when skipped.
        let metrics = adapter.summarize(std::slice::from_ref(&evaluation), &options);
        assert!(!metrics.is_empty(), "{suite_id} skipped metrics");
    }
}

#[test]
fn test_every_adapter_summarizes_scored_cases() {
    // Scored (non-no-exec) executions should produce metric rows too.
    for (suite_id, case) in every_adapter() {
        let adapter = adapter_for_suite(suite_id).expect("adapter");
        let options = BenchRunOptions::default();
        let evaluation = adapter.evaluate_case(&case, &generation(&[&case.reference]), &options);
        let metrics = adapter.summarize(std::slice::from_ref(&evaluation), &options);
        assert!(!metrics.is_empty(), "{suite_id} scored metrics");
    }
}

#[test]
fn test_apps_prompt_records_subset_and_flags_codebleu_on_failure() {
    let adapter = adapter_for_suite("apps").expect("apps adapter");
    let case = fixture("python", "a, b = map(int, input().split())\nprint(a + b)");
    let prompt = adapter.build_prompt(
        &case,
        &BenchRunOptions {
            subset: Some("introductory".to_string()),
            ..BenchRunOptions::default()
        },
    );
    assert!(prompt.contains("subset=introductory"));

    let options = BenchRunOptions::default();
    let miss = adapter.evaluate_case(&case, &generation(&["totally different"]), &options);
    assert_eq!(miss.status, "failed");
    assert!(miss.notes.contains("codebleu="));
    assert!(miss.notes.contains("similarity="));

    let metrics = adapter.summarize(&[miss], &options);
    assert!(metrics.iter().any(|m| m.metric_name == "accuracy"));
    assert!(metrics.iter().any(|m| m.metric_name == "codebleu"));
}

#[test]
fn test_multipl_e_language_mismatch_skips_and_summarizes() {
    let adapter = adapter_for_suite("multipl-e").expect("multipl-e adapter");
    let case = fixture("rust", "fn max_in_list(items: &[i32]) -> i32 { 0 }");
    let options = BenchRunOptions {
        language: Some("python".to_string()),
        ..BenchRunOptions::default()
    };
    let evaluation = adapter.evaluate_case(&case, &generation(&["x"]), &options);
    assert_eq!(evaluation.status, "skipped");
    assert_eq!(
        evaluation.error_code.as_deref(),
        Some("unsupported_language")
    );

    // Summary over an all-skipped set returns skipped metrics.
    let metrics = adapter.summarize(&[evaluation], &options);
    assert!(metrics.iter().any(|m| m.metric_name == "pass_at_1"));
}

#[test]
fn test_multipl_e_matching_language_scores_with_pass_at_k() {
    let adapter = adapter_for_suite("multipl-e").expect("multipl-e adapter");
    let case = fixture("rust", "fn max_in_list(items: &[i32]) -> i32 { 0 }");
    let options = BenchRunOptions {
        language: Some("rust".to_string()),
        samples: 2,
        ..BenchRunOptions::default()
    };
    let evaluation = adapter.evaluate_case(&case, &generation(&[&case.reference]), &options);
    assert_eq!(evaluation.status, "passed");
    let metrics = adapter.summarize(&[evaluation], &options);
    assert!(metrics.iter().any(|m| m.metric_name == "pass_at_2"));
}

#[test]
fn test_repobench_scores_with_edit_similarity_even_when_passed() {
    let adapter = adapter_for_suite("repobench").expect("repobench adapter");
    let case = fixture("python", "repository_completion()");
    let options = BenchRunOptions::default();
    let evaluation =
        adapter.evaluate_case(&case, &generation(&["repository_completion()"]), &options);
    assert_eq!(evaluation.status, "passed");
    assert_eq!(evaluation.score, Some(1.0));

    let metrics = adapter.summarize(&[evaluation], &options);
    assert!(metrics.iter().any(|m| m.metric_name == "exact_match"));
    assert!(metrics.iter().any(|m| m.metric_name == "edit_similarity"));
}

#[test]
fn test_crosscodeeval_scored_path_emits_completion_metrics() {
    let adapter = adapter_for_suite("crosscodeeval").expect("crosscodeeval adapter");
    let case = fixture("python", "return helper(value)");
    let options = BenchRunOptions::default();
    let evaluation = adapter.evaluate_case(&case, &generation(&["return helper(value)"]), &options);
    let metrics = adapter.summarize(&[evaluation], &options);
    assert!(
        metrics
            .iter()
            .any(|m| m.metric_name == "completion_accuracy")
    );
    assert!(metrics.iter().any(|m| m.metric_name == "edit_similarity"));
}

#[test]
fn test_swebench_verified_supports_only_the_repair_scenario() {
    let adapter = adapter_for_suite("swebench-verified").expect("verified adapter");
    let case = fixture("diff", PATCH);
    let options = BenchRunOptions {
        scenario: Some("feature".to_string()),
        ..BenchRunOptions::default()
    };
    let evaluation = adapter.evaluate_case(&case, &generation(&[PATCH]), &options);
    assert_eq!(evaluation.status, "skipped");
    assert_eq!(
        evaluation.error_code.as_deref(),
        Some("unsupported_scenario")
    );

    let metrics = adapter.summarize(&[evaluation], &options);
    assert!(
        metrics
            .iter()
            .any(|m| m.metric_name == "instances_resolved")
    );
}

#[test]
fn test_swebench_lite_repair_scenario_resolves_patch() {
    let adapter = adapter_for_suite("swebench-lite").expect("lite adapter");
    let case = fixture("diff", PATCH);
    let options = BenchRunOptions::default();
    let evaluation = adapter.evaluate_case(&case, &generation(&[PATCH]), &options);
    assert_eq!(evaluation.status, "passed");
    assert_eq!(evaluation.score, Some(1.0));

    let metrics = adapter.summarize(&[evaluation], &options);
    let rate = metrics
        .iter()
        .find(|m| m.metric_name == "resolution_rate")
        .expect("resolution_rate");
    assert_eq!(rate.metric_value, 1.0);
}

#[test]
fn test_livecodebench_rejects_unsupported_scenario_and_scores_default() {
    let adapter = adapter_for_suite("livecodebench").expect("livecodebench adapter");
    let case = fixture("python", "def solve():\n    pass");

    let unsupported = BenchRunOptions {
        scenario: Some("debugging".to_string()),
        ..BenchRunOptions::default()
    };
    let skipped = adapter.evaluate_case(&case, &generation(&["x"]), &unsupported);
    assert_eq!(skipped.status, "skipped");
    assert_eq!(skipped.error_code.as_deref(), Some("unsupported_scenario"));

    let options = BenchRunOptions::default();
    let scored = adapter.evaluate_case(&case, &generation(&["def solve():\n    pass"]), &options);
    assert_eq!(scored.status, "passed");
    let metrics = adapter.summarize(&[scored], &options);
    assert!(metrics.iter().any(|m| m.metric_name == "pass_at_1"));
    assert!(metrics.iter().any(|m| m.metric_name == "scenario_score"));
}

#[test]
fn test_bigcodebench_scored_path_emits_three_metrics() {
    let adapter = adapter_for_suite("bigcodebench").expect("bigcodebench adapter");
    let case = fixture("python", "def solution():\n    raise NotImplementedError");
    let options = BenchRunOptions::default();
    let evaluation = adapter.evaluate_case(
        &case,
        &generation(&["def solution():\n    raise NotImplementedError"]),
        &options,
    );
    let metrics = adapter.summarize(&[evaluation], &options);
    let names: Vec<&str> = metrics.iter().map(|m| m.metric_name.as_str()).collect();
    assert_eq!(names, vec!["pass_at_1", "pass_at_k", "codebleu"]);
}
