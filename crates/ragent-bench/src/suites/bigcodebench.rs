//! `BigCodeBench` benchmark adapter.

use crate::command::BenchRunOptions;
use crate::data::BenchCaseFixture;
use crate::model::BenchGenerationResult;
use crate::suites::{
    BenchCaseEvaluation, BenchMetricEvaluation, BenchSuiteAdapter, average_metric,
    count_passed_failed, evaluate_exact_match_case, pass_at_k, skipped_metrics_for_suite,
};

pub(super) static ADAPTER: BigCodeBenchAdapter = BigCodeBenchAdapter;

pub(super) struct BigCodeBenchAdapter;

// NOTE: intentional duplication — see DUPPLAN.md Milestone J.
// Per-suite prompt builders; identical shape is coincidental.
impl BenchSuiteAdapter for BigCodeBenchAdapter {
    fn suite_id(&self) -> &'static str {
        "bigcodebench"
    }

    fn build_prompt(&self, case: &BenchCaseFixture, _options: &BenchRunOptions) -> String {
        format!(
            "You are solving a BigCodeBench practical coding task.\nReturn only the complete solution code.\n\nTask:\n{}\n",
            case.prompt
        )
    }

    fn evaluate_case(
        &self,
        case: &BenchCaseFixture,
        generation: &BenchGenerationResult,
        options: &BenchRunOptions,
    ) -> BenchCaseEvaluation {
        evaluate_exact_match_case(case, generation, options, "BigCodeBench", |_, _| {
            "BigCodeBench native adapter records pass@k and CodeBLEU-style similarity for practical tasks.".to_string()
        })
    }

    fn summarize(
        &self,
        evaluations: &[BenchCaseEvaluation],
        options: &BenchRunOptions,
    ) -> Vec<BenchMetricEvaluation> {
        if options.no_exec {
            return skipped_metrics_for_suite(
                &["pass_at_1", "pass_at_k", "codebleu"],
                evaluations.len(),
                "BigCodeBench",
            );
        }

        let (passed, failed) = count_passed_failed(evaluations);
        let pass_at_1 = crate::suites::pass_at_1(evaluations);
        let pass_at_k_values = evaluations
            .iter()
            .map(|evaluation| {
                pass_at_k(
                    options.samples.max(1),
                    evaluation.exact_match_count,
                    options.samples.max(1),
                )
            })
            .collect::<Vec<_>>();
        let codebleu_values = evaluations
            .iter()
            .filter_map(|evaluation| evaluation.score)
            .collect::<Vec<_>>();
        vec![
            BenchMetricEvaluation {
                metric_name: "pass_at_1".to_string(),
                metric_value: pass_at_1,
                metric_unit: "ratio".to_string(),
                passed_count: Some(passed),
                failed_count: Some(failed),
                skipped_count: Some(0),
                notes: "BigCodeBench native adapter records first-sample pass@1.".to_string(),
            },
            average_metric(
                "pass_at_k",
                &pass_at_k_values,
                passed,
                failed,
                0,
                "BigCodeBench native adapter records pass@k over generated candidates.",
            ),
            average_metric(
                "codebleu",
                &codebleu_values,
                passed,
                failed,
                0,
                "BigCodeBench native adapter uses a native CodeBLEU-style token overlap score.",
            ),
        ]
    }
}
