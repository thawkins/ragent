//! HumanEval benchmark adapter.

use std::process::Command;

use crate::command::BenchRunOptions;
use crate::data::BenchCaseFixture;
use crate::model::BenchGenerationResult;
use crate::suites::{
    BenchCaseEvaluation, BenchMetricEvaluation, BenchSuiteAdapter, bench_temp_root,
    best_exact_or_similarity_sample, exact_match_count, first_sample_exact_match, pass_at_k,
    skipped_metric, strip_code_fences,
};

pub(super) static ADAPTER: HumanEvalAdapter = HumanEvalAdapter;

pub(super) struct HumanEvalAdapter;

impl BenchSuiteAdapter for HumanEvalAdapter {
    fn suite_id(&self) -> &'static str {
        "humaneval"
    }

    fn build_prompt(&self, case: &BenchCaseFixture, _options: &BenchRunOptions) -> String {
        let starter = case.starter_code.as_deref().unwrap_or("");
        if starter.is_empty() {
            format!(
                "You are solving a HumanEval {} task.\nReturn only the function implementation.\n\nTask:\n{}\n",
                humaneval_language_label(&case.language),
                case.prompt
            )
        } else {
            format!(
                "You are solving a HumanEval {} task.\nReturn only the function implementation.\n\nTask:\n{}\n\nStarter code:\n{}\n",
                humaneval_language_label(&case.language),
                case.prompt,
                starter
            )
        }
    }

    fn evaluate_case(
        &self,
        case: &BenchCaseFixture,
        generation: &BenchGenerationResult,
        options: &BenchRunOptions,
    ) -> BenchCaseEvaluation {
        if options.no_exec {
            let (selected_response, _) =
                best_exact_or_similarity_sample(generation, &case.reference);
            let exact_matches = exact_match_count(generation, &case.reference);
            let first_exact = first_sample_exact_match(generation, &case.reference);
            return BenchCaseEvaluation {
                status: "skipped".to_string(),
                score: None,
                selected_response,
                exact_match_count: exact_matches,
                first_sample_exact_match: first_exact,
                notes: "HumanEval prompt generated; hidden-test execution skipped because --no-exec was set.".to_string(),
                error_code: None,
                error_message: None,
            };
        }

        if let (Some(test_code), Some(entry_point)) =
            (case.test_code.as_deref(), case.entry_point.as_deref())
        {
            if !case.execution_commands.is_empty() {
                return evaluate_humaneval_native_tests(case, generation, test_code, entry_point);
            }
            return evaluate_humaneval_hidden_tests(case, generation, test_code, entry_point);
        }

        fallback_proxy_evaluation(case, generation)
    }

    fn summarize(
        &self,
        evaluations: &[BenchCaseEvaluation],
        options: &BenchRunOptions,
    ) -> Vec<BenchMetricEvaluation> {
        let sample_count = options.samples.max(1);
        if options.no_exec {
            let mut metrics = vec![skipped_metric(
                "pass_at_1",
                evaluations.len(),
                "HumanEval evaluation skipped because --no-exec was set.",
            )];
            if sample_count > 1 {
                metrics.push(skipped_metric(
                    &format!("pass_at_{sample_count}"),
                    evaluations.len(),
                    "HumanEval evaluation skipped because --no-exec was set.",
                ));
            }
            return metrics;
        }

        let pass_at_1 = if evaluations.is_empty() {
            0.0
        } else {
            evaluations
                .iter()
                .filter(|evaluation| evaluation.first_sample_exact_match)
                .count() as f64
                / evaluations.len() as f64
        };
        let pass_at_k_value = if evaluations.is_empty() {
            0.0
        } else {
            evaluations
                .iter()
                .map(|evaluation| {
                    pass_at_k(sample_count, evaluation.exact_match_count, sample_count)
                })
                .sum::<f64>()
                / evaluations.len() as f64
        };
        let passed_count = evaluations
            .iter()
            .filter(|evaluation| evaluation.status == "passed")
            .count();
        let failed_count = evaluations
            .iter()
            .filter(|evaluation| evaluation.status == "failed")
            .count();

        let mut metrics = vec![BenchMetricEvaluation {
            metric_name: "pass_at_1".to_string(),
            metric_value: pass_at_1,
            metric_unit: "ratio".to_string(),
            passed_count: Some(passed_count),
            failed_count: Some(failed_count),
            skipped_count: Some(0),
            notes: "HumanEval uses hidden-test execution when dataset test payload is available."
                .to_string(),
        }];
        if sample_count > 1 {
            metrics.push(BenchMetricEvaluation {
                metric_name: format!("pass_at_{sample_count}"),
                metric_value: pass_at_k_value,
                metric_unit: "ratio".to_string(),
                passed_count: Some(passed_count),
                failed_count: Some(failed_count),
                skipped_count: Some(0),
                notes:
                    "HumanEval uses hidden-test execution when dataset test payload is available."
                        .to_string(),
            });
        }
        metrics
    }
}

fn fallback_proxy_evaluation(
    case: &BenchCaseFixture,
    generation: &BenchGenerationResult,
) -> BenchCaseEvaluation {
    let (selected_response, similarity) =
        best_exact_or_similarity_sample(generation, &case.reference);
    let exact_matches = exact_match_count(generation, &case.reference);
    let first_exact = first_sample_exact_match(generation, &case.reference);
    let passed = exact_matches > 0;
    BenchCaseEvaluation {
        status: if passed { "passed" } else { "failed" }.to_string(),
        score: Some(if passed { 1.0 } else { similarity }),
        selected_response,
        exact_match_count: exact_matches,
        first_sample_exact_match: first_exact,
        notes: "HumanEval dataset did not include hidden tests, so evaluation fell back to normalized exact-match proxy scoring.".to_string(),
        error_code: None,
        error_message: None,
    }
}

fn evaluate_humaneval_hidden_tests(
    case: &BenchCaseFixture,
    generation: &BenchGenerationResult,
    test_code: &str,
    entry_point: &str,
) -> BenchCaseEvaluation {
    let fallback = fallback_proxy_evaluation(case, generation);
    let mut passed_count = 0usize;
    let mut first_sample_passed = false;
    let mut first_error = None;
    let mut selected_response = fallback.selected_response.clone();

    for (idx, sample) in generation.samples.iter().enumerate() {
        match run_humaneval_hidden_tests(case, &sample.text, test_code, entry_point) {
            Ok(()) => {
                passed_count += 1;
                if idx == 0 {
                    first_sample_passed = true;
                }
                if passed_count == 1 {
                    selected_response = sample.text.clone();
                }
            }
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }

    let passed = passed_count > 0;
    BenchCaseEvaluation {
        status: if passed { "passed" } else { "failed" }.to_string(),
        score: Some(if passed { 1.0 } else { 0.0 }),
        selected_response,
        exact_match_count: passed_count,
        first_sample_exact_match: first_sample_passed,
        notes: if passed {
            format!(
                "HumanEval hidden-test execution passed for {passed_count}/{} generated sample(s).",
                generation.samples.len()
            )
        } else {
            format!(
                "HumanEval hidden-test execution failed for all {} generated sample(s).",
                generation.samples.len()
            )
        },
        error_code: if passed {
            None
        } else {
            Some("hidden_test_failed".to_string())
        },
        error_message: if passed { None } else { first_error },
    }
}

fn evaluate_humaneval_native_tests(
    case: &BenchCaseFixture,
    generation: &BenchGenerationResult,
    test_code: &str,
    entry_point: &str,
) -> BenchCaseEvaluation {
    let fallback = fallback_proxy_evaluation(case, generation);
    let mut passed_count = 0usize;
    let mut first_sample_passed = false;
    let mut first_error = None;
    let mut selected_response = fallback.selected_response.clone();

    for (idx, sample) in generation.samples.iter().enumerate() {
        match run_humaneval_native_tests(case, &sample.text, test_code, entry_point) {
            Ok(()) => {
                passed_count += 1;
                if idx == 0 {
                    first_sample_passed = true;
                }
                if passed_count == 1 {
                    selected_response = sample.text.clone();
                }
            }
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }

    let passed = passed_count > 0;
    BenchCaseEvaluation {
        status: if passed { "passed" } else { "failed" }.to_string(),
        score: Some(if passed { 1.0 } else { 0.0 }),
        selected_response,
        exact_match_count: passed_count,
        first_sample_exact_match: first_sample_passed,
        notes: if passed {
            format!(
                "HumanEval native {} tests passed for {passed_count}/{} generated sample(s).",
                humaneval_language_label(&case.language),
                generation.samples.len()
            )
        } else {
            format!(
                "HumanEval native {} tests failed for all {} generated sample(s).",
                humaneval_language_label(&case.language),
                generation.samples.len()
            )
        },
        error_code: if passed {
            None
        } else {
            Some("hidden_test_failed".to_string())
        },
        error_message: if passed { None } else { first_error },
    }
}

fn run_humaneval_hidden_tests(
    case: &BenchCaseFixture,
    sample: &str,
    test_code: &str,
    entry_point: &str,
) -> Result<(), String> {
    let temp_root = bench_temp_root().map_err(|error| format!("prepare temp root: {error}"))?;
    let script_path = temp_root.join(format!("humaneval-{}.py", uuid::Uuid::new_v4().simple()));
    let tests_literal =
        serde_json::to_string(test_code).map_err(|error| format!("encode tests: {error}"))?;
    let entry_literal = serde_json::to_string(entry_point)
        .map_err(|error| format!("encode entry point: {error}"))?;
    let mut last_error = None;

    for candidate_source in humaneval_candidate_variants(case, sample, entry_point) {
        let candidate_literal = serde_json::to_string(&candidate_source)
            .map_err(|error| format!("encode candidate: {error}"))?;
        let script = [
            "import signal".to_string(),
            "def _timeout_handler(signum, frame):".to_string(),
            "    raise TimeoutError('HumanEval execution timed out')".to_string(),
            "signal.signal(signal.SIGALRM, _timeout_handler)".to_string(),
            "signal.alarm(10)".to_string(),
            "namespace = {}".to_string(),
            format!("candidate_source = {candidate_literal}"),
            format!("test_source = {tests_literal}"),
            format!("entry_point = {entry_literal}"),
            "exec(candidate_source, namespace)".to_string(),
            "exec(test_source, namespace)".to_string(),
            "candidate = namespace.get(entry_point)".to_string(),
            "if candidate is None:".to_string(),
            "    raise AssertionError(f'entry point {entry_point} was not defined')".to_string(),
            "check = namespace.get('check')".to_string(),
            "if check is None:".to_string(),
            "    raise AssertionError('HumanEval test payload did not define check(candidate)')"
                .to_string(),
            "check(candidate)".to_string(),
            "signal.alarm(0)".to_string(),
        ]
        .join("\n");
        std::fs::write(&script_path, script).map_err(|error| {
            format!("write HumanEval script {}: {error}", script_path.display())
        })?;

        let output = Command::new("python3")
            .arg(&script_path)
            .output()
            .map_err(|error| format!("launch python3: {error}"))?;
        if output.status.success() {
            let _ = std::fs::remove_file(&script_path);
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        last_error = Some(if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("python exited with status {}", output.status)
        });
    }

    let _ = std::fs::remove_file(&script_path);
    Err(last_error.unwrap_or_else(|| "HumanEval execution failed".to_string()))
}

fn run_humaneval_native_tests(
    case: &BenchCaseFixture,
    sample: &str,
    test_code: &str,
    entry_point: &str,
) -> Result<(), String> {
    let temp_root = bench_temp_root().map_err(|error| format!("prepare temp root: {error}"))?;
    let run_root = temp_root.join(format!("humaneval-{}", uuid::Uuid::new_v4().simple()));
    std::fs::create_dir_all(&run_root)
        .map_err(|error| format!("create HumanEval temp root {}: {error}", run_root.display()))?;

    let result = (|| -> Result<(), String> {
        let file_name = humaneval_native_file_name(case);
        if case.language == "go" {
            std::fs::write(run_root.join("go.mod"), "module humanevalpack\n\ngo 1.22\n")
                .map_err(|error| format!("write go.mod: {error}"))?;
        }

        let source_path = run_root.join(&file_name);
        let mut last_error = None;
        for candidate_source in humaneval_candidate_variants(case, sample, entry_point) {
            let rendered = render_humaneval_native_source(test_code, &candidate_source);
            std::fs::write(&source_path, rendered).map_err(|error| {
                format!("write HumanEval source {}: {error}", source_path.display())
            })?;

            let mut last_stdout = String::new();
            let mut last_stderr = String::new();
            let mut command_failed = None;
            for (index, command_parts) in case.execution_commands.iter().enumerate() {
                let timeout_secs = case
                    .execution_timeouts_secs
                    .get(index)
                    .copied()
                    .unwrap_or(10);
                let rendered_parts = command_parts
                    .iter()
                    .map(|part| part.replace("__FILENAME__", &file_name))
                    .collect::<Vec<_>>();
                let Some(program) = rendered_parts.first() else {
                    return Err("HumanEval command list was empty".to_string());
                };
                let output = Command::new("timeout")
                    .arg(format!("{timeout_secs}s"))
                    .arg(program)
                    .args(rendered_parts.iter().skip(1))
                    .current_dir(&run_root)
                    .output()
                    .map_err(|error| format!("launch HumanEval command `{program}`: {error}"))?;
                last_stdout = String::from_utf8_lossy(&output.stdout).to_string();
                last_stderr = String::from_utf8_lossy(&output.stderr).to_string();
                if !output.status.success() {
                    let detail = [last_stderr.trim(), last_stdout.trim()]
                        .into_iter()
                        .find(|part| !part.is_empty())
                        .unwrap_or("HumanEval native command failed");
                    command_failed = Some(format!(
                        "HumanEval command `{}` failed: {}",
                        rendered_parts.join(" "),
                        detail
                    ));
                    break;
                }
            }

            if let Some(error) = command_failed {
                last_error = Some(error);
                continue;
            }
            if last_stderr.contains("FAILED")
                || last_stdout.contains("FAILED")
                || last_stderr.contains("Assertion failed")
                || last_stdout.contains("Assertion failed")
            {
                last_error = Some(
                    format!("{}\n{}", last_stdout.trim(), last_stderr.trim())
                        .trim()
                        .to_string(),
                );
                continue;
            }
            return Ok(());
        }

        Err(last_error.unwrap_or_else(|| "HumanEval native execution failed".to_string()))
    })();

    let _ = std::fs::remove_dir_all(&run_root);
    result
}

fn humaneval_candidate_variants(
    case: &BenchCaseFixture,
    sample: &str,
    entry_point: &str,
) -> Vec<String> {
    let completion = strip_code_fences(sample);
    let starter = case.starter_code.as_deref().unwrap_or(&case.prompt);
    let mut variants = Vec::new();
    if completion.contains(entry_point) {
        variants.push(completion.clone());
    }
    if case.language == "python" {
        variants.push(format!(
            "{}{}",
            starter,
            indent_completion_body(&completion)
        ));
        variants.push(format!("{}{}", starter, completion));
    } else {
        variants.push(close_open_braces(&format!(
            "{}{}",
            starter,
            indent_completion_body(&completion)
        )));
        variants.push(close_open_braces(&format!("{starter}{completion}")));
    }
    variants
}

fn render_humaneval_native_source(test_code: &str, candidate: &str) -> String {
    test_code.replace("PLACEHOLDER_CODE_BODY", candidate)
}

fn humaneval_native_file_name(case: &BenchCaseFixture) -> String {
    match case.language.as_str() {
        "go" => "humaneval_test.go".to_string(),
        "java" => "Main.java".to_string(),
        _ => format!(
            "humaneval.{}",
            case.source_extension.as_deref().unwrap_or("txt")
        ),
    }
}

fn close_open_braces(source: &str) -> String {
    let open_count = source.chars().filter(|ch| *ch == '{').count();
    let close_count = source.chars().filter(|ch| *ch == '}').count();
    if open_count <= close_count {
        return source.to_string();
    }
    let mut rendered = source.trim_end().to_string();
    for _ in 0..(open_count - close_count) {
        rendered.push_str("\n}");
    }
    rendered.push('\n');
    rendered
}

fn humaneval_language_label(language: &str) -> &'static str {
    match language {
        "cpp" => "C++",
        "go" => "Go",
        "java" => "Java",
        "javascript" => "JavaScript",
        "python" => "Python",
        "rust" => "Rust",
        _ => "HumanEval",
    }
}

fn indent_completion_body(sample: &str) -> String {
    sample
        .lines()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else if line.starts_with(' ') || line.starts_with('\t') {
                line.to_string()
            } else {
                format!("    {line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}
