//! APPS benchmark adapter.

use crate::command::BenchRunOptions;
use crate::data::BenchCaseFixture;
use crate::model::BenchGenerationResult;
use crate::suites::{
    BenchCaseEvaluation, BenchMetricEvaluation, BenchSuiteAdapter, accuracy_metric, codebleu_score,
    count_passed_failed, evaluate_exact_match_case, skipped_metrics_for_suite,
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
        // Pre-compute codebleu for the notes provider closure
        let (selected_response, similarity) =
            crate::suites::best_exact_or_similarity_sample(generation, &case.reference);
        let codebleu = codebleu_score(&selected_response, &case.reference);

        evaluate_exact_match_case(case, generation, options, "APPS", |passed, _| {
            if passed {
                "APPS native adapter records accuracy and CodeBLEU-style similarity for generated programs.".to_string()
            } else {
                format!(
                    "APPS native adapter records accuracy and CodeBLEU-style similarity for generated programs. (similarity={similarity:.3}, codebleu={codebleu:.3})"
                )
            }
        })
    }

    fn summarize(
        &self,
        evaluations: &[BenchCaseEvaluation],
        options: &BenchRunOptions,
    ) -> Vec<BenchMetricEvaluation> {
        if options.no_exec {
            return skipped_metrics_for_suite(&["accuracy", "codebleu"], evaluations.len(), "APPS");
        }

        let (passed, failed) = count_passed_failed(evaluations);
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
