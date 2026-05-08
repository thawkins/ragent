//! RepoBench benchmark adapter.

use crate::command::BenchRunOptions;
use crate::data::BenchCaseFixture;
use crate::model::BenchGenerationResult;
use crate::suites::{
    BenchCaseEvaluation, BenchMetricEvaluation, BenchSuiteAdapter, accuracy_metric, average_metric,
    count_passed_failed, evaluate_exact_match_case, skipped_metrics_for_suite,
};

pub(super) static ADAPTER: RepoBenchAdapter = RepoBenchAdapter;

pub(super) struct RepoBenchAdapter;

impl BenchSuiteAdapter for RepoBenchAdapter {
    fn suite_id(&self) -> &'static str {
        "repobench"
    }

    fn build_prompt(&self, case: &BenchCaseFixture, _options: &BenchRunOptions) -> String {
        format!(
            "You are solving a RepoBench repository-completion task.\nComplete only the missing repository code span.\n\nMasked task:\n{}\n",
            case.prompt
        )
    }

    fn evaluate_case(
        &self,
        case: &BenchCaseFixture,
        generation: &BenchGenerationResult,
        options: &BenchRunOptions,
    ) -> BenchCaseEvaluation {
        // Note: RepoBench uses edit similarity directly as score, not as fallback
        let result = evaluate_exact_match_case(case, generation, options, "RepoBench", |_, similarity| {
            format!("RepoBench MVP adapter records exact-match and edit-similarity scoring; CodeBLEU follows in a later phase. (similarity={similarity:.3})")
        });
        // Override: RepoBench uses similarity as score even when passed
        BenchCaseEvaluation {
            score: Some(crate::suites::edit_similarity(
                &result.selected_response,
                &case.reference,
            )),
            ..result
        }
    }

    fn summarize(
        &self,
        evaluations: &[BenchCaseEvaluation],
        options: &BenchRunOptions,
    ) -> Vec<BenchMetricEvaluation> {
        if options.no_exec {
            return skipped_metrics_for_suite(
                &["exact_match", "edit_similarity"],
                evaluations.len(),
                "RepoBench",
            );
        }

        let (passed, failed) = count_passed_failed(evaluations);
        let similarities = evaluations
            .iter()
            .filter_map(|evaluation| evaluation.score)
            .collect::<Vec<_>>();
        vec![
            accuracy_metric(
                "exact_match",
                passed,
                failed,
                0,
                "RepoBench MVP uses normalized exact-match scoring for exact match.",
            ),
            average_metric(
                "edit_similarity",
                &similarities,
                passed,
                failed,
                0,
                "RepoBench MVP uses normalized edit similarity over the best sample per case.",
            ),
        ]
    }
}
