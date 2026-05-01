//! SWE-bench patch-generation adapters.

use crate::command::BenchRunOptions;
use crate::data::BenchCaseFixture;
use crate::model::BenchGenerationResult;
use crate::suites::{
    BenchCaseEvaluation, BenchMetricEvaluation, BenchSuiteAdapter, best_exact_or_similarity_sample,
    codebleu_score, exact_match_count, first_sample_exact_match, resolution_rate, skipped_metric,
};

pub(super) static LITE_ADAPTER: SweBenchAdapter = SweBenchAdapter {
    suite_id: "swebench-lite",
    label: "SWE-bench Lite",
    strict_threshold: 0.9,
};

pub(super) static VERIFIED_ADAPTER: SweBenchAdapter = SweBenchAdapter {
    suite_id: "swebench-verified",
    label: "SWE-bench Verified",
    strict_threshold: 0.95,
};

pub(super) struct SweBenchAdapter {
    suite_id: &'static str,
    label: &'static str,
    strict_threshold: f64,
}

impl BenchSuiteAdapter for SweBenchAdapter {
    fn suite_id(&self) -> &'static str {
        self.suite_id
    }

    fn build_prompt(&self, case: &BenchCaseFixture, options: &BenchRunOptions) -> String {
        let scenario = options.scenario.as_deref().unwrap_or("repair");
        format!(
            "You are solving a {} patch-generation task in the `{}` scenario.\nYou are given an issue description and repository snapshot context.\nReturn only a unified diff patch.\n\nIssue:\n{}\n",
            self.label, scenario, case.prompt
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
        let scenario = options.scenario.as_deref().unwrap_or("repair");
        if scenario != "repair" {
            return BenchCaseEvaluation {
                status: "skipped".to_string(),
                score: None,
                selected_response,
                exact_match_count: exact_matches,
                first_sample_exact_match: first_exact,
                notes: format!(
                    "{} currently supports only the `repair` scenario; got `{scenario}`.",
                    self.label
                ),
                error_code: Some("unsupported_scenario".to_string()),
                error_message: Some(format!("unsupported scenario `{scenario}`")),
            };
        }
        if options.no_exec {
            return BenchCaseEvaluation {
                status: "skipped".to_string(),
                score: None,
                selected_response,
                exact_match_count: exact_matches,
                first_sample_exact_match: first_exact,
                notes: format!(
                    "{} patch generation completed; repository patch application and test execution were skipped because --no-exec was set.",
                    self.label
                ),
                error_code: None,
                error_message: None,
            };
        }

        let codebleu = codebleu_score(&selected_response, &case.reference);
        let looks_like_patch = selected_response.contains("diff --git");
        let resolved = exact_matches > 0 || (looks_like_patch && codebleu >= self.strict_threshold);
        BenchCaseEvaluation {
            status: if resolved { "passed" } else { "failed" }.to_string(),
            score: Some(if resolved {
                1.0
            } else {
                codebleu.max(similarity)
            }),
            selected_response,
            exact_match_count: exact_matches,
            first_sample_exact_match: first_exact,
            notes: format!(
                "{} uses native patch-shape and diff-similarity checks as a resolution proxy until full repo materialization and isolated test execution land.",
                self.label
            ),
            error_code: None,
            error_message: None,
        }
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
            return vec![
                skipped_metric(
                    "resolution_rate",
                    evaluations.len(),
                    "SWE-bench evaluation skipped because execution was unavailable.",
                ),
                BenchMetricEvaluation {
                    metric_name: "instances_resolved".to_string(),
                    metric_value: 0.0,
                    metric_unit: "count".to_string(),
                    passed_count: Some(0),
                    failed_count: Some(0),
                    skipped_count: Some(evaluations.len()),
                    notes: "SWE-bench instance resolution count is zero because execution was unavailable.".to_string(),
                },
            ];
        }

        let resolved = evaluations
            .iter()
            .filter(|evaluation| evaluation.status == "passed")
            .count();
        let failed = evaluations
            .iter()
            .filter(|evaluation| evaluation.status == "failed")
            .count();
        let attempted = resolved + failed;
        vec![
            BenchMetricEvaluation {
                metric_name: "resolution_rate".to_string(),
                metric_value: resolution_rate(resolved, attempted),
                metric_unit: "ratio".to_string(),
                passed_count: Some(resolved),
                failed_count: Some(failed),
                skipped_count: Some(skipped),
                notes: format!(
                    "{} uses native patch-shape and diff-similarity checks as a resolution-rate proxy.",
                    self.label
                ),
            },
            BenchMetricEvaluation {
                metric_name: "instances_resolved".to_string(),
                metric_value: resolved as f64,
                metric_unit: "count".to_string(),
                passed_count: Some(resolved),
                failed_count: Some(failed),
                skipped_count: Some(skipped),
                notes: format!(
                    "{} counts resolved instances as passed patch cases.",
                    self.label
                ),
            },
        ]
    }
}
