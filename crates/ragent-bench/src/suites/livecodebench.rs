//! `LiveCodeBench` benchmark adapter.

use crate::command::BenchRunOptions;
use crate::data::BenchCaseFixture;
use crate::model::BenchGenerationResult;
use crate::suites::{
    BenchCaseEvaluation, BenchMetricEvaluation, BenchSuiteAdapter, average_metric,
    best_exact_or_similarity_sample, count_passed_failed, evaluate_exact_match_case,
    exact_match_count, first_sample_exact_match, pass_at_k, skipped_metrics_for_suite,
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
        // Pre-check: scenario validation
        let scenario = options.scenario.as_deref().unwrap_or("codegeneration");
        if scenario != "codegeneration" {
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
                notes: format!(
                    "LiveCodeBench Phase 6 supports only the `codegeneration` scenario; got `{scenario}`."
                ),
                error_code: Some("unsupported_scenario".to_string()),
                error_message: Some(format!("unsupported scenario `{scenario}`")),
            };
        }

        // Delegate to standard helper for remaining logic
        evaluate_exact_match_case(
            case,
            generation,
            options,
            "LiveCodeBench",
            |_passed, _similarity| {
                "LiveCodeBench native adapter supports the codegeneration scenario and records pass@k-style metrics.".to_string()
            },
        )
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
            return skipped_metrics_for_suite(
                &["pass_at_1", "scenario_score"],
                evaluations.len(),
                "LiveCodeBench",
            );
        }

        let (passed, failed) = count_passed_failed(evaluations);
        let pass_at_1 = crate::suites::pass_at_1(evaluations);
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
