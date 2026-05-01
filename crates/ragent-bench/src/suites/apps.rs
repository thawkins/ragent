//! APPS benchmark adapter.

use crate::command::BenchRunOptions;
use crate::data::BenchCaseFixture;
use crate::model::BenchGenerationResult;
use crate::suites::{
    BenchCaseEvaluation, BenchMetricEvaluation, BenchSuiteAdapter, accuracy_metric,
    best_exact_or_similarity_sample, codebleu_score, exact_match_count, first_sample_exact_match,
    skipped_metric,
};

pub(super) static ADAPTER: AppsAdapter = AppsAdapter;

pub(super) struct AppsAdapter;

impl BenchSuiteAdapter for AppsAdapter {
    fn suite_id(&self) -> &'static str {
        "apps"
    }

    fn build_prompt(&self, case: &BenchCaseFixture, options: &BenchRunOptions) -> String {
        let subset = options
            .subset
            .as_deref()
            .map_or("sampled".to_string(), |value| format!("subset={value}"));
        format!(
            "You are solving an APPS competitive-programming task ({subset}).\nReturn only a complete program solution.\n\nProblem:\n{}\n",
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
                notes: "APPS program generation completed; testcase execution skipped because --no-exec was set.".to_string(),
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
            notes: "APPS native adapter records accuracy and CodeBLEU-style similarity for generated programs.".to_string(),
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
                    "accuracy",
                    evaluations.len(),
                    "APPS evaluation skipped because --no-exec was set.",
                ),
                skipped_metric(
                    "codebleu",
                    evaluations.len(),
                    "APPS evaluation skipped because --no-exec was set.",
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
        let scores = evaluations
            .iter()
            .filter_map(|evaluation| evaluation.score)
            .collect::<Vec<_>>();
        vec![
            accuracy_metric(
                "accuracy",
                passed,
                failed,
                0,
                "APPS native adapter uses exact-match accuracy for generated programs.",
            ),
            crate::suites::average_metric(
                "codebleu",
                &scores,
                passed,
                failed,
                0,
                "APPS native adapter uses a native CodeBLEU-style token overlap score.",
            ),
        ]
    }
}
