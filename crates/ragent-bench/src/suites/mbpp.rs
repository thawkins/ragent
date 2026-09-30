//! MBPP benchmark adapter.

use std::process::Command;

use crate::command::BenchRunOptions;
use crate::data::BenchCaseFixture;
use crate::model::BenchGenerationResult;
use crate::suites::{
    BenchCaseEvaluation, BenchMetricEvaluation, BenchSuiteAdapter, accuracy_metric,
    bench_temp_root, best_exact_or_similarity_sample, count_passed_failed, exact_match_count,
    first_sample_exact_match, skipped_metric, strip_code_fences,
};

pub(super) static ADAPTER: MbppAdapter = MbppAdapter;

pub(super) struct MbppAdapter;

impl BenchSuiteAdapter for MbppAdapter {
    fn suite_id(&self) -> &'static str {
        "mbpp"
    }

    fn build_prompt(&self, case: &BenchCaseFixture, _options: &BenchRunOptions) -> String {
        format!(
            "You are solving an MBPP {} task.\nReturn only {} code that satisfies the problem.\nImplement the requested entry point exactly as described.\n\nProblem:\n{}\n",
            mbpp_language_label(&case.language),
            mbpp_language_label(&case.language),
            case.prompt
        )
    }

    fn evaluate_case(
        &self,
        case: &BenchCaseFixture,
        generation: &BenchGenerationResult,
        options: &BenchRunOptions,
    ) -> BenchCaseEvaluation {
        let (selected_response, similarity) =
            best_exact_or_similarity_sample(generation, &case.reference);
        let exact_matches = exact_match_count(generation, &case.reference);
        let first_exact = first_sample_exact_match(generation, &case.reference);
        if options.no_exec {
            return BenchCaseEvaluation {
                status: "skipped".to_string(),
                score: None,
                selected_response,
                exact_match_count: exact_matches,
                first_sample_exact_match: first_exact,
                notes: "MBPP prompt generated; bundled assertion execution skipped because --no-exec was set.".to_string(),
                error_code: None,
                error_message: None,
            };
        }

        if let Some(test_code) = case.test_code.as_deref() {
            if !case.execution_commands.is_empty() {
                return evaluate_mbpp_native(case, generation, test_code);
            }
            return evaluate_mbpp_python_tests(case, generation, test_code);
        }

        let passed = exact_matches > 0;
        BenchCaseEvaluation {
            status: if passed { "passed" } else { "failed" }.to_string(),
            score: Some(if passed { 1.0 } else { similarity }),
            selected_response,
            exact_match_count: exact_matches,
            first_sample_exact_match: first_exact,
            notes: "MBPP fixture did not include bundled tests, so evaluation fell back to normalized exact-match proxy scoring.".to_string(),
            error_code: None,
            error_message: None,
        }
    }

    fn summarize(
        &self,
        evaluations: &[BenchCaseEvaluation],
        options: &BenchRunOptions,
    ) -> Vec<BenchMetricEvaluation> {
        if options.no_exec {
            return vec![skipped_metric(
                "accuracy",
                evaluations.len(),
                "MBPP evaluation skipped because --no-exec was set.",
            )];
        }

        let (passed, failed) = count_passed_failed(evaluations);
        vec![accuracy_metric(
            "accuracy",
            passed,
            failed,
            0,
            "MBPP uses bundled assertion execution for accuracy when tests are available.",
        )]
    }
}

fn evaluate_mbpp_python_tests(
    case: &BenchCaseFixture,
    generation: &BenchGenerationResult,
    test_code: &str,
) -> BenchCaseEvaluation {
    let fallback = best_exact_or_similarity_sample(generation, &case.reference);
    let total = generation.samples.len();
    crate::suites::evaluate_suite_samples(
        generation,
        fallback.0,
        |text| run_mbpp_python_assertions(text, test_code),
        |passed_count| {
            format!(
                "MBPP bundled assertions passed for {passed_count}/{total} generated sample(s)."
            )
        },
        || format!("MBPP bundled assertions failed for all {total} generated sample(s)."),
        "assertion_failed",
    )
}

fn evaluate_mbpp_native(
    case: &BenchCaseFixture,
    generation: &BenchGenerationResult,
    test_code: &str,
) -> BenchCaseEvaluation {
    let fallback = best_exact_or_similarity_sample(generation, &case.reference);
    let total = generation.samples.len();
    let label = mbpp_language_label(&case.language);
    crate::suites::evaluate_suite_samples(
        generation,
        fallback.0,
        |text| run_mbpp_native_harness(case, text, test_code),
        |passed_count| {
            format!(
                "MBPP native {label} harness passed for {passed_count}/{total} generated sample(s)."
            )
        },
        || format!("MBPP native {label} harness failed for all {total} generated sample(s)."),
        "assertion_failed",
    )
}

fn run_mbpp_python_assertions(sample: &str, test_code: &str) -> Result<(), String> {
    let temp_root = bench_temp_root().map_err(|error| format!("prepare temp root: {error}"))?;
    let script_path = temp_root.join(format!("mbpp-{}.py", uuid::Uuid::new_v4().simple()));
    let candidate_literal = serde_json::to_string(&strip_code_fences(sample))
        .map_err(|error| format!("encode candidate: {error}"))?;
    let tests_literal =
        serde_json::to_string(test_code).map_err(|error| format!("encode tests: {error}"))?;
    let script = [
        "import signal".to_string(),
        "def _timeout_handler(signum, frame):".to_string(),
        "    raise TimeoutError('MBPP execution timed out')".to_string(),
        "signal.signal(signal.SIGALRM, _timeout_handler)".to_string(),
        format!(
            "signal.alarm({})",
            crate::exec_guard::FIXTURE_DEFAULT_TIMEOUT_SECS
        ),
        "namespace = {}".to_string(),
        format!("candidate_source = {candidate_literal}"),
        format!("test_source = {tests_literal}"),
        "exec(candidate_source, namespace)".to_string(),
        "exec(test_source, namespace)".to_string(),
        "signal.alarm(0)".to_string(),
    ]
    .join("\n");
    std::fs::write(&script_path, script)
        .map_err(|error| format!("write MBPP script {}: {error}", script_path.display()))?;

    // SEC-ragent-bench-002 (SECTASKS T-009/T-048): the in-script
    // `signal.alarm` is defeated by untrusted fixture code, so the subprocess
    // gets the same hard `timeout` wrapper as the native harness.
    let output = Command::new("timeout")
        .arg(format!(
            "{}s",
            crate::exec_guard::FIXTURE_SUBPROCESS_TIMEOUT_SECS
        ))
        .arg("python3")
        .arg(&script_path)
        .output()
        .map_err(|error| format!("launch python3: {error}"))?;
    let _ = std::fs::remove_file(&script_path); // INTENTIONAL: best-effort temp cleanup
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Err(if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("python exited with status {}", output.status)
    })
}

fn run_mbpp_native_harness(
    case: &BenchCaseFixture,
    sample: &str,
    test_code: &str,
) -> Result<(), String> {
    let temp_root = bench_temp_root().map_err(|error| format!("prepare temp root: {error}"))?;
    let run_root = temp_root.join(format!("mbpp-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&run_root).map_err(|error| {
        format!(
            "create native MBPP temp dir {}: {error}",
            run_root.display()
        )
    })?;

    let result = (|| -> Result<(), String> {
        let extension = case.source_extension.as_deref().unwrap_or("txt");
        let source_name = format!("candidate.{extension}");
        let source_path = run_root.join(&source_name);
        let rendered = render_mbpp_native_test(case, sample, test_code);
        std::fs::write(&source_path, rendered).map_err(|error| {
            format!(
                "write native MBPP source {}: {error}",
                source_path.display()
            )
        })?;

        let (last_stdout, last_stderr) =
            crate::suites::run_fixture_commands(case, &run_root, &source_name, "native MBPP")?;

        verify_native_mbpp_output(&last_stdout, &last_stderr)
    })();

    let _ = std::fs::remove_dir_all(&run_root); // INTENTIONAL: best-effort temp cleanup
    result
}

fn render_mbpp_native_test(case: &BenchCaseFixture, sample: &str, test_code: &str) -> String {
    let entry_point = case.entry_point.as_deref().unwrap_or("solution");
    let entry_class = case.entry_class.as_deref().unwrap_or("Solution");
    test_code
        .replace("PLACEHOLDER_CODE_BODY", &strip_code_fences(sample))
        .replace("PLACEHOLDER_FN_NAME", entry_point)
        .replace("PLACEHOLDER_CLS_NAME", entry_class)
}

fn verify_native_mbpp_output(stdout: &str, stderr: &str) -> Result<(), String> {
    let combined = if stderr.trim().is_empty() {
        stdout.to_string()
    } else if stdout.trim().is_empty() {
        stderr.to_string()
    } else {
        format!("{stdout}\n{stderr}")
    };
    let test_lines = combined
        .lines()
        .filter(|line| line.contains("TEST-"))
        .collect::<Vec<_>>();
    if test_lines.is_empty() {
        return Err(format!(
            "native MBPP harness did not report any test lines: {}",
            combined.trim()
        ));
    }
    if test_lines.iter().all(|line| line.contains("PASSED")) {
        return Ok(());
    }
    let failing = test_lines
        .iter()
        .find(|line| !line.contains("PASSED"))
        .copied()
        .unwrap_or("native MBPP harness reported an unknown failure");
    Err(failing.to_string())
}

fn mbpp_language_label(language: &str) -> &'static str {
    match language {
        "cpp" => "C++",
        "csharp" => "C#",
        "dart" => "Dart",
        "go" => "Go",
        "haskell" => "Haskell",
        "java" => "Java",
        "javascript" => "JavaScript",
        "julia" => "Julia",
        "kotlin" => "Kotlin",
        "lua" => "Lua",
        "php" => "PHP",
        "python" => "Python",
        "r" => "R",
        "rust" => "Rust",
        "scala" => "Scala",
        "typescript" => "TypeScript",
        _ => "MBPP",
    }
}
