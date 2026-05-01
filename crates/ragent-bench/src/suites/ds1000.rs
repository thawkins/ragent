//! DS-1000 benchmark adapter.

use crate::command::BenchRunOptions;
use crate::data::BenchCaseFixture;
use crate::model::BenchGenerationResult;
use crate::suites::{
    BenchCaseEvaluation, BenchMetricEvaluation, BenchSuiteAdapter, accuracy_metric,
    best_exact_or_similarity_sample, exact_match_count, first_sample_exact_match, skipped_metric,
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
                notes: "DS-1000 prompt generated; constraint and testcase execution skipped because --no-exec was set.".to_string(),
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
            notes: "DS-1000 MVP adapter uses normalized exact-match scoring for insertion accuracy until native constraint evaluation lands.".to_string(),
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
                "DS-1000 evaluation skipped because --no-exec was set.",
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
            "DS-1000 MVP uses normalized exact-match scoring for accuracy.",
        )]
    }
}
