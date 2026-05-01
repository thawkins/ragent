//! RepoBench benchmark adapter.

use crate::command::BenchRunOptions;
use crate::data::BenchCaseFixture;
use crate::model::BenchGenerationResult;
use crate::suites::{
    BenchCaseEvaluation, BenchMetricEvaluation, BenchSuiteAdapter, accuracy_metric, average_metric,
    best_exact_or_similarity_sample, exact_match_count, first_sample_exact_match, skipped_metric,
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
                notes: "RepoBench prompt generated; local completion scoring skipped because --no-exec was set.".to_string(),
                error_code: None,
                error_message: None,
            };
        }

        let passed = exact_matches > 0;
        BenchCaseEvaluation {
            status: if passed { "passed" } else { "failed" }.to_string(),
            score: Some(similarity),
            selected_response,
            exact_match_count: exact_matches,
            first_sample_exact_match: first_exact,
            notes: "RepoBench MVP adapter records exact-match and edit-similarity scoring; CodeBLEU follows in a later phase.".to_string(),
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
                    "exact_match",
                    evaluations.len(),
                    "RepoBench evaluation skipped because --no-exec was set.",
                ),
                skipped_metric(
                    "edit_similarity",
                    evaluations.len(),
                    "RepoBench evaluation skipped because --no-exec was set.",
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
