//! DS-1000 benchmark adapter.

use crate::command::BenchRunOptions;
use crate::data::BenchCaseFixture;
use crate::model::BenchGenerationResult;
use crate::suites::{
    BenchCaseEvaluation, BenchMetricEvaluation, BenchSuiteAdapter, accuracy_metric,
    count_passed_failed, evaluate_exact_match_case, skipped_metrics_for_suite,
};

pub(super) static ADAPTER: Ds1000Adapter = Ds1000Adapter;

pub(super) struct Ds1000Adapter;

impl BenchSuiteAdapter for Ds1000Adapter {
    fn suite_id(&self) -> &'static str {
        "ds1000"
    }

    fn build_prompt(&self, case: &BenchCaseFixture, _options: &BenchRunOptions) -> String {
        format!(
            "You are solving a DS-1000 Python insertion task.\nReturn only the code snippet that should fill the target location.\n\nTask:\n{}\n",
            case.prompt
        )
    }

    fn evaluate_case(
        &self,
        case: &BenchCaseFixture,
        generation: &BenchGenerationResult,
        options: &BenchRunOptions,
    ) -> BenchCaseEvaluation {
        evaluate_exact_match_case(case, generation, options, "DS-1000", |_, _| {
            "DS-1000 MVP adapter uses normalized exact-match scoring for insertion accuracy until native constraint evaluation lands.".to_string()
        })
    }

    fn summarize(
        &self,
        evaluations: &[BenchCaseEvaluation],
        options: &BenchRunOptions,
    ) -> Vec<BenchMetricEvaluation> {
        if options.no_exec {
            return skipped_metrics_for_suite(
                &["accuracy"],
                evaluations.len(),
                "DS-1000",
            );
        }

        let (passed, failed) = count_passed_failed(evaluations);
        vec![accuracy_metric(
            "accuracy",
            passed,
            failed,
            0,
            "DS-1000 MVP uses normalized exact-match scoring for accuracy.",
        )]
    }
}
