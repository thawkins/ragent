//! HumanEval benchmark adapter.

use std::path::PathBuf;
use std::process::Command;

use crate::command::BenchRunOptions;
use crate::data::BenchCaseFixture;
use crate::model::BenchGenerationResult;
use crate::suites::{
    BenchCaseEvaluation, BenchMetricEvaluation, BenchSuiteAdapter, best_exact_or_similarity_sample,
    exact_match_count, first_sample_exact_match, pass_at_k, skipped_metric,
};

pub(super) static ADAPTER: HumanEvalAdapter = HumanEvalAdapter;

pub(super) struct HumanEvalAdapter;

impl BenchSuiteAdapter for HumanEvalAdapter {
    fn suite_id(&self) -> &'static str {
        "humaneval"
    }

    fn build_prompt(&self, case: &BenchCaseFixture, _options: &BenchRunOptions) -> String {
        format!(
            "You are solving a HumanEval Python task.\nReturn only the function implementation.\n\nTask:\n{}\n",
            case.prompt
        )
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

fn run_humaneval_hidden_tests(
    case: &BenchCaseFixture,
    sample: &str,
    test_code: &str,
    entry_point: &str,
) -> Result<(), String> {
    let temp_root = humaneval_temp_root().map_err(|error| format!("prepare temp root: {error}"))?;
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

fn humaneval_candidate_variants(
    case: &BenchCaseFixture,
    sample: &str,
    entry_point: &str,
) -> Vec<String> {
    let completion = strip_code_fences(sample);
    let mut variants = Vec::new();
    if completion.contains(&format!("def {entry_point}(")) {
        variants.push(completion.clone());
    }
    variants.push(format!(
        "{}{}",
        case.prompt,
        indent_completion_body(&completion)
    ));
    variants.push(format!("{}{}", case.prompt, completion));
    variants
}

fn strip_code_fences(sample: &str) -> String {
    let trimmed = sample.trim();
    if let Some(start) = trimmed.find("```") {
        let fenced = &trimmed[start + 3..];
        let fenced = fenced
            .strip_prefix("python")
            .or_else(|| fenced.strip_prefix("py"))
            .unwrap_or(fenced);
        if let Some(end) = fenced.find("```") {
            return fenced[..end].trim_matches('\n').to_string();
        }
    }
    trimmed.to_string()
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

fn humaneval_temp_root() -> std::io::Result<PathBuf> {
    let root = std::env::current_dir()?.join("target").join("temp");
    std::fs::create_dir_all(&root)?;
    Ok(root)
}
