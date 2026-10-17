//! Adapter-level coverage for the HumanEval and MBPP suites, exercising the
//! prompt-shaping, `--no-exec` skip path, exact-match fallback, and summary
//! branches that the end-to-end runner tests do not reach.

use ragent_bench::BenchRunOptions;
use ragent_bench::data::BenchCaseFixture;
use ragent_bench::model::{BenchGeneratedSample, BenchGenerationResult};
use ragent_bench::suites::adapter_for_suite;

fn fixture(
    case_id: &str,
    prompt: &str,
    reference: &str,
    language: &str,
    starter_code: Option<&str>,
    test_code: Option<&str>,
) -> BenchCaseFixture {
    BenchCaseFixture {
        case_id: case_id.to_string(),
        prompt: prompt.to_string(),
        starter_code: starter_code.map(str::to_string),
        reference: reference.to_string(),
        language: language.to_string(),
        test_code: test_code.map(str::to_string),
        entry_point: Some("entry".to_string()),
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
                input_tokens: Some(10),
                output_tokens: Some(5),
                finish_reason: Some("stop".to_string()),
                duration_ms: 1,
            })
            .collect(),
    }
}

#[test]
fn test_humaneval_build_prompt_includes_or_omits_starter_code() {
    let adapter = adapter_for_suite("humaneval").expect("humaneval adapter");
    let options = BenchRunOptions::default();

    let bare = fixture(
        "h-1",
        "def add(a, b):",
        "return a + b",
        "python",
        None,
        None,
    );
    let bare_prompt = adapter.build_prompt(&bare, &options);
    assert!(bare_prompt.contains("HumanEval Python task"));
    assert!(bare_prompt.contains("def add(a, b):"));
    assert!(!bare_prompt.contains("Starter code:"));

    let with_starter = fixture(
        "h-2",
        "task",
        "ref",
        "rust",
        Some("pub fn entry() {}"),
        None,
    );
    let starter_prompt = adapter.build_prompt(&with_starter, &options);
    assert!(starter_prompt.contains("HumanEval Rust task"));
    assert!(starter_prompt.contains("Starter code:"));
    assert!(starter_prompt.contains("pub fn entry() {}"));

    // Unknown language falls back to the generic label.
    let unknown = fixture("h-3", "p", "r", "cobol", None, None);
    assert!(
        adapter
            .build_prompt(&unknown, &options)
            .contains("HumanEval HumanEval task")
    );
}

#[test]
fn test_humaneval_no_exec_returns_skipped_evaluation() {
    let adapter = adapter_for_suite("humaneval").expect("humaneval adapter");
    let case = fixture(
        "h-1",
        "task",
        "return a + b",
        "python",
        None,
        Some("def check(candidate):\n    pass\n"),
    );
    let options = BenchRunOptions {
        no_exec: true,
        ..BenchRunOptions::default()
    };
    let evaluation = adapter.evaluate_case(&case, &generation(&["return a + b"]), &options);
    assert_eq!(evaluation.status, "skipped");
    assert!(evaluation.score.is_none());
    assert_eq!(evaluation.exact_match_count, 1);
    assert!(evaluation.first_sample_exact_match);
    assert!(evaluation.notes.contains("--no-exec"));
}

#[test]
fn test_humaneval_no_exec_summary_emits_skipped_metrics() {
    let adapter = adapter_for_suite("humaneval").expect("humaneval adapter");
    let case = fixture("h-1", "task", "ref", "python", None, None);
    let options = BenchRunOptions {
        no_exec: true,
        ..BenchRunOptions::default()
    };
    let evaluation = adapter.evaluate_case(&case, &generation(&["ref"]), &options);

    let single = adapter.summarize(std::slice::from_ref(&evaluation), &options);
    assert_eq!(single.len(), 1);
    assert_eq!(single[0].metric_name, "pass_at_1");
    assert_eq!(single[0].skipped_count, Some(1));

    let multi_options = BenchRunOptions {
        samples: 3,
        ..options
    };
    let multi = adapter.summarize(std::slice::from_ref(&evaluation), &multi_options);
    assert_eq!(multi.len(), 2);
    assert_eq!(multi[1].metric_name, "pass_at_3");
}

#[test]
fn test_humaneval_fallback_proxy_scores_exact_and_similarity() {
    let adapter = adapter_for_suite("humaneval").expect("humaneval adapter");
    // No test_code and no entry point -> fallback proxy evaluation.
    let case = fixture("h-1", "task", "return a + b", "python", None, None);
    let options = BenchRunOptions::default();

    let exact = adapter.evaluate_case(&case, &generation(&["return a + b"]), &options);
    assert_eq!(exact.status, "passed");
    assert_eq!(exact.score, Some(1.0));
    assert!(exact.notes.contains("normalized exact-match proxy"));

    let miss = adapter.evaluate_case(&case, &generation(&["totally different"]), &options);
    assert_eq!(miss.status, "failed");
    assert!(miss.score.is_some_and(|score| score < 1.0));
    assert_eq!(miss.exact_match_count, 0);
}

#[test]
fn test_humaneval_summary_computes_pass_at_k_and_counts() {
    let adapter = adapter_for_suite("humaneval").expect("humaneval adapter");
    let case = fixture("h-1", "task", "return a + b", "python", None, None);
    let options = BenchRunOptions {
        samples: 2,
        ..BenchRunOptions::default()
    };
    let passed = adapter.evaluate_case(&case, &generation(&["return a + b"]), &options);
    let failed = adapter.evaluate_case(&case, &generation(&["nope"]), &options);
    let metrics = adapter.summarize(&[passed, failed], &options);
    assert!(metrics.iter().any(|m| m.metric_name == "pass_at_1"));
    assert!(metrics.iter().any(|m| m.metric_name == "pass_at_2"));
    let pass_at_1 = metrics
        .iter()
        .find(|m| m.metric_name == "pass_at_1")
        .expect("pass_at_1");
    assert_eq!(pass_at_1.passed_count, Some(1));
    assert_eq!(pass_at_1.failed_count, Some(1));
}

#[test]
fn test_mbpp_build_prompt_names_entry_point_language() {
    let adapter = adapter_for_suite("mbpp").expect("mbpp adapter");
    let options = BenchRunOptions::default();
    let case = fixture("m-1", "Return the sum.", "ref", "rust", None, None);
    let prompt = adapter.build_prompt(&case, &options);
    assert!(prompt.contains("MBPP Rust task"));
    assert!(prompt.contains("Return only Rust code"));
    assert!(prompt.contains("Return the sum."));
}

#[test]
fn test_mbpp_no_exec_returns_skipped_evaluation_and_summary() {
    let adapter = adapter_for_suite("mbpp").expect("mbpp adapter");
    let case = fixture("m-1", "prompt", "def f(): return 1", "python", None, None);
    let options = BenchRunOptions {
        no_exec: true,
        ..BenchRunOptions::default()
    };
    let evaluation = adapter.evaluate_case(&case, &generation(&["def f(): return 1"]), &options);
    assert_eq!(evaluation.status, "skipped");
    assert!(evaluation.notes.contains("--no-exec"));

    let metrics = adapter.summarize(std::slice::from_ref(&evaluation), &options);
    assert_eq!(metrics.len(), 1);
    assert_eq!(metrics[0].metric_name, "accuracy");
    assert_eq!(metrics[0].skipped_count, Some(1));
}

#[test]
fn test_mbpp_fallback_scores_without_bundled_tests() {
    let adapter = adapter_for_suite("mbpp").expect("mbpp adapter");
    let case = fixture("m-1", "prompt", "def f(): return 1", "python", None, None);
    let options = BenchRunOptions::default();

    let exact = adapter.evaluate_case(&case, &generation(&["def f(): return 1"]), &options);
    assert_eq!(exact.status, "passed");
    assert!(exact.notes.contains("did not include bundled tests"));

    let miss = adapter.evaluate_case(&case, &generation(&["other"]), &options);
    assert_eq!(miss.status, "failed");

    let metrics = adapter.summarize(&[exact, miss], &options);
    assert_eq!(metrics.len(), 1);
    assert_eq!(metrics[0].metric_name, "accuracy");
    assert_eq!(metrics[0].passed_count, Some(1));
    assert_eq!(metrics[0].failed_count, Some(1));
}

#[test]
fn test_adapter_for_suite_rejects_unknown_suite() {
    // `dyn BenchSuiteAdapter` is not `Debug`, so match rather than `expect_err`.
    match adapter_for_suite("does-not-exist") {
        Ok(_) => panic!("unknown suite should not resolve an adapter"),
        Err(error) => assert!(
            error
                .to_string()
                .contains("does not have a native benchmark adapter")
        ),
    }
}
