//! CrossCodeEval benchmark adapter.

use crate::command::BenchRunOptions;
use crate::data::BenchCaseFixture;
use crate::model::BenchGenerationResult;
use crate::suites::{
    BenchCaseEvaluation, BenchMetricEvaluation, BenchSuiteAdapter, accuracy_metric, average_metric,
    count_passed_failed, evaluate_exact_match_case, skipped_metrics_for_suite,
};

pub(super) static ADAPTER: CrossCodeEvalAdapter = CrossCodeEvalAdapter;

pub(super) struct CrossCodeEvalAdapter;

impl BenchSuiteAdapter for CrossCodeEvalAdapter {
    fn suite_id(&self) -> &'static str {
        "crosscodeeval"
    }

    fn build_prompt(&self, case: &BenchCaseFixture, _options: &BenchRunOptions) -> String {
        format!(
            "You are solving a CrossCodeEval cross-file completion task.\nUse the described helper context and return only the missing completion.\n\nContext task:\n{}\n",
            case.prompt
        )
    }

    fn evaluate_case(
        &self,
        case: &BenchCaseFixture,
        generation: &BenchGenerationResult,
        options: &BenchRunOptions,
    ) -> BenchCaseEvaluation {
        evaluate_exact_match_case(case, generation, options, "CrossCodeEval", |_, similarity| {
            format!("CrossCodeEval MVP adapter records completion accuracy and edit similarity from reconstructed prompt contexts. (similarity={similarity:.3})")
        })
    }

    fn summarize(
        &self,
        evaluations: &[BenchCaseEvaluation],
        options: &BenchRunOptions,
    ) -> Vec<BenchMetricEvaluation> {
        if options.no_exec {
            return skipped_metrics_for_suite(
                &["completion_accuracy", "edit_similarity"],
                evaluations.len(),
                "CrossCodeEval",
            );
        }

        let (passed, failed) = count_passed_failed(evaluations);
        let similarities = evaluations
            .iter()
            .filter_map(|evaluation| evaluation.score)
            .collect::<Vec<_>>();
        vec![
            accuracy_metric(
                "completion_accuracy",
                passed,
                failed,
                0,
                "CrossCodeEval MVP uses normalized exact-match scoring for completion accuracy.",
            ),
            average_metric(
                "edit_similarity",
                &similarities,
                passed,
                failed,
                0,
                "CrossCodeEval MVP uses normalized edit similarity over the best sample per case.",
            ),
        ]
    }
}
