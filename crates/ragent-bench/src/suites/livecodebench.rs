//! LiveCodeBench benchmark adapter.

use crate::command::BenchRunOptions;
use crate::data::BenchCaseFixture;
use crate::model::BenchGenerationResult;
use crate::suites::{
    BenchCaseEvaluation, BenchMetricEvaluation, BenchSuiteAdapter, average_metric,
    best_exact_or_similarity_sample, exact_match_count, first_sample_exact_match, pass_at_k,
    skipped_metric,
};

pub(super) static ADAPTER: LiveCodeBenchAdapter = LiveCodeBenchAdapter;

pub(super) struct LiveCodeBenchAdapter;

impl BenchSuiteAdapter for LiveCodeBenchAdapter {
    fn suite_id(&self) -> &'static str {
        "livecodebench"
    }

    fn build_prompt(&self, case: &BenchCaseFixture, options: &BenchRunOptions) -> String {
        let release = options.release.as_deref().unwrap_or("default_release");
        let scenario = options.scenario.as_deref().unwrap_or("codegeneration");
        format!(
            "You are solving a LiveCodeBench `{scenario}` task from release `{release}`.\nReturn only the solution code.\n\nTask:\n{}\n",
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
        let scenario = options.scenario.as_deref().unwrap_or("codegeneration");
        if scenario != "codegeneration" {
            return BenchCaseEvaluation {
                status: "skipped".to_string(),
                score: None,
                selected_response,
                exact_match_count: exact_matches,
                first_sample_exact_match: first_exact,
                notes: format!(
                    "LiveCodeBench Phase 6 supports only the `codegeneration` scenario; got `{scenario}`."
                ),
                error_code: Some("unsupported_scenario".to_string()),
                error_message: Some(format!("unsupported scenario `{scenario}`")),
            };
        }
        if options.no_exec {
            return BenchCaseEvaluation {
                status: "skipped".to_string(),
                score: None,
                selected_response,
                exact_match_count: exact_matches,
                first_sample_exact_match: first_exact,
                notes: "LiveCodeBench generation completed; scenario execution skipped because --no-exec was set.".to_string(),
                error_code: None,
                error_message: None,
            };
        }

        let passed = exact_matches > 0;
        BenchCaseEvaluation {
            status: if passed { "passed" } else { "failed" }.to_string(),
            score: Some(if passed { 1.0 } else { similarity }),
            selected_response,
            exact_match_count: exact_matches,
            first_sample_exact_match: first_exact,
            notes: "LiveCodeBench native adapter supports the codegeneration scenario and records pass@k-style metrics.".to_string(),
            error_code: None,
            error_message: None,
        }
    }

    fn summarize(
        &self,
        evaluations: &[BenchCaseEvaluation],
        options: &BenchRunOptions,
    ) -> Vec<BenchMetricEvaluation> {
        let skipped = evaluations
            .iter()
            .filter(|evaluation| evaluation.status == "skipped")
            .count();
        if options.no_exec || skipped == evaluations.len() {
            return vec![
                skipped_metric(
                    "pass_at_1",
                    evaluations.len(),
                    "LiveCodeBench evaluation skipped because execution was unavailable.",
                ),
                skipped_metric(
                    "scenario_score",
                    evaluations.len(),
                    "LiveCodeBench evaluation skipped because execution was unavailable.",
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
        let scenario_scores = evaluations
            .iter()
            .map(|evaluation| {
                pass_at_k(
                    options.samples.max(1),
                    evaluation.exact_match_count,
                    options.samples.max(5),
                )
            })
            .collect::<Vec<_>>();
        vec![
            BenchMetricEvaluation {
                metric_name: "pass_at_1".to_string(),
                metric_value: pass_at_1,
                metric_unit: "ratio".to_string(),
                passed_count: Some(passed),
                failed_count: Some(failed),
                skipped_count: Some(skipped),
                notes: "LiveCodeBench native adapter records first-sample pass@1.".to_string(),
            },
            average_metric(
                "scenario_score",
                &scenario_scores,
                passed,
                failed,
                skipped,
                "LiveCodeBench scenario_score uses native pass@k over the supported codegeneration scenario.",
            ),
        ]
    }
}
