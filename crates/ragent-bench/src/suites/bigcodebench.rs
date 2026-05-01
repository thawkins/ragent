//! BigCodeBench benchmark adapter.

use crate::command::BenchRunOptions;
use crate::data::BenchCaseFixture;
use crate::model::BenchGenerationResult;
use crate::suites::{
    BenchCaseEvaluation, BenchMetricEvaluation, BenchSuiteAdapter, average_metric,
    best_exact_or_similarity_sample, codebleu_score, exact_match_count, first_sample_exact_match,
    pass_at_k, skipped_metric,
};

pub(super) static ADAPTER: BigCodeBenchAdapter = BigCodeBenchAdapter;

pub(super) struct BigCodeBenchAdapter;

impl BenchSuiteAdapter for BigCodeBenchAdapter {
    fn suite_id(&self) -> &'static str {
        "bigcodebench"
    }

    fn build_prompt(&self, case: &BenchCaseFixture, _options: &BenchRunOptions) -> String {
        format!(
            "You are solving a BigCodeBench practical coding task.\nReturn only the complete solution code.\n\nTask:\n{}\n",
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
        let codebleu = codebleu_score(&selected_response, &case.reference);
        if options.no_exec {
            return BenchCaseEvaluation {
                status: "skipped".to_string(),
                score: None,
                selected_response,
                exact_match_count: exact_matches,
                first_sample_exact_match: first_exact,
                notes: "BigCodeBench generation completed; sandboxed execution skipped because --no-exec was set.".to_string(),
                error_code: None,
                error_message: None,
            };
        }

        let passed = exact_matches > 0;
        BenchCaseEvaluation {
            status: if passed { "passed" } else { "failed" }.to_string(),
            score: Some(if passed { 1.0 } else { codebleu.max(similarity) }),
            selected_response,
            exact_match_count: exact_matches,
            first_sample_exact_match: first_exact,
            notes: "BigCodeBench native adapter records pass@k and CodeBLEU-style similarity for practical tasks.".to_string(),
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
            return vec![
                skipped_metric(
                    "pass_at_1",
                    evaluations.len(),
                    "BigCodeBench evaluation skipped because --no-exec was set.",
                ),
                skipped_metric(
                    "pass_at_k",
                    evaluations.len(),
                    "BigCodeBench evaluation skipped because --no-exec was set.",
                ),
                skipped_metric(
                    "codebleu",
                    evaluations.len(),
                    "BigCodeBench evaluation skipped because --no-exec was set.",
                ),
            ];
        }

        let passed = evaluations
            .iter()
            .filter(|evaluation| evaluation.status == "passed")
            .count();
        let failed = evaluations
            .iter()
            .filter(|evaluation| evaluation.status == "failed")
            .count();
        let pass_at_1 = if evaluations.is_empty() {
            0.0
        } else {
            evaluations
                .iter()
                .filter(|evaluation| evaluation.first_sample_exact_match)
                .count() as f64
                / evaluations.len() as f64
        };
        let pass_at_k_values = evaluations
            .iter()
            .map(|evaluation| {
                pass_at_k(
                    options.samples.max(1),
                    evaluation.exact_match_count,
                    options.samples.max(1),
                )
            })
            .collect::<Vec<_>>();
        let codebleu_values = evaluations
            .iter()
            .filter_map(|evaluation| evaluation.score)
            .collect::<Vec<_>>();
        vec![
            BenchMetricEvaluation {
                metric_name: "pass_at_1".to_string(),
                metric_value: pass_at_1,
                metric_unit: "ratio".to_string(),
                passed_count: Some(passed),
                failed_count: Some(failed),
                skipped_count: Some(0),
                notes: "BigCodeBench native adapter records first-sample pass@1.".to_string(),
            },
            average_metric(
                "pass_at_k",
                &pass_at_k_values,
                passed,
                failed,
                0,
                "BigCodeBench native adapter records pass@k over generated candidates.",
            ),
            average_metric(
                "codebleu",
                &codebleu_values,
                passed,
                failed,
                0,
                "BigCodeBench native adapter uses a native CodeBLEU-style token overlap score.",
            ),
        ]
    }
}
