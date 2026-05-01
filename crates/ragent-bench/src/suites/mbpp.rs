//! MBPP benchmark adapter.

use crate::command::BenchRunOptions;
use crate::data::BenchCaseFixture;
use crate::model::BenchGenerationResult;
use crate::suites::{
    BenchCaseEvaluation, BenchMetricEvaluation, BenchSuiteAdapter, accuracy_metric,
    best_exact_or_similarity_sample, exact_match_count, first_sample_exact_match, skipped_metric,
};

pub(super) static ADAPTER: MbppAdapter = MbppAdapter;

pub(super) struct MbppAdapter;

impl BenchSuiteAdapter for MbppAdapter {
    fn suite_id(&self) -> &'static str {
        "mbpp"
    }

    fn build_prompt(&self, case: &BenchCaseFixture, _options: &BenchRunOptions) -> String {
        format!(
            "You are solving an MBPP Python task.\nReturn only Python code that satisfies the problem.\n\nProblem:\n{}\n",
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

        let passed = exact_matches > 0;
        BenchCaseEvaluation {
            status: if passed { "passed" } else { "failed" }.to_string(),
            score: Some(if passed { 1.0 } else { similarity }),
            selected_response,
            exact_match_count: exact_matches,
            first_sample_exact_match: first_exact,
            notes: "MBPP MVP adapter uses normalized exact-match scoring as a proxy for bundled assertion execution.".to_string(),
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

        let passed = evaluations
            .iter()
            .filter(|evaluation| evaluation.status == "passed")
            .count();
        let failed = evaluations
            .iter()
            .filter(|evaluation| evaluation.status == "failed")
            .count();
        vec![accuracy_metric(
            "accuracy",
            passed,
            failed,
            0,
            "MBPP MVP uses normalized exact-match scoring for accuracy.",
        )]
    }
}
