//! MultiPL-E benchmark adapter.

use crate::command::BenchRunOptions;
use crate::data::BenchCaseFixture;
use crate::model::BenchGenerationResult;
use crate::suites::{
    BenchCaseEvaluation, BenchMetricEvaluation, BenchSuiteAdapter, best_exact_or_similarity_sample,
    exact_match_count, first_sample_exact_match, pass_at_k, skipped_metric,
};

pub(super) static ADAPTER: MultiPlEAdapter = MultiPlEAdapter;

pub(super) struct MultiPlEAdapter;

impl BenchSuiteAdapter for MultiPlEAdapter {
    fn suite_id(&self) -> &'static str {
        "multipl-e"
    }

    fn build_prompt(&self, case: &BenchCaseFixture, options: &BenchRunOptions) -> String {
        let language = options
            .language
            .as_deref()
            .unwrap_or(case.language.as_str());
        format!(
            "You are solving a MultiPL-E task for the target language `{language}`.\nReturn only the translated solution code.\n\nTask:\n{}\n",
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
        if let Some(language) = &options.language
            && language != &case.language
        {
            return BenchCaseEvaluation {
                status: "skipped".to_string(),
                score: None,
                selected_response,
                exact_match_count: exact_matches,
                first_sample_exact_match: first_exact,
                notes: format!(
                    "MultiPL-E sample fixture only supports language `{}`; requested `{language}`.",
                    case.language
                ),
                error_code: Some("unsupported_language".to_string()),
                error_message: Some(format!("unsupported language `{language}`")),
            };
        }
        if options.no_exec {
            return BenchCaseEvaluation {
                status: "skipped".to_string(),
                score: None,
                selected_response,
                exact_match_count: exact_matches,
                first_sample_exact_match: first_exact,
                notes: "MultiPL-E generation completed; language-aware execution skipped because --no-exec was set.".to_string(),
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
            notes: "MultiPL-E native adapter uses normalized exact-match scoring as a functional-correctness proxy.".to_string(),
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
                    "pass_at_1",
                    evaluations.len(),
                    "MultiPL-E evaluation skipped because execution was unavailable.",
                ),
                skipped_metric(
                    &format!("pass_at_{}", options.samples.max(1)),
                    evaluations.len(),
                    "MultiPL-E evaluation skipped because execution was unavailable.",
                ),
            ];
        }

        let pass_at_1 = if evaluations.is_empty() {
            0.0
        } else {
            evaluations
                .iter()
                .filter(|evaluation| evaluation.first_sample_exact_match)
                .count() as f64
                / evaluations.len() as f64
        };
        let pass_at_k_value = if evaluations.is_empty() {
            0.0
        } else {
            evaluations
                .iter()
                .map(|evaluation| {
                    pass_at_k(
                        options.samples.max(1),
                        evaluation.exact_match_count,
                        options.samples.max(1),
                    )
                })
                .sum::<f64>()
                / evaluations.len() as f64
        };
        let passed = evaluations
            .iter()
            .filter(|evaluation| evaluation.status == "passed")
            .count();
        let failed = evaluations
            .iter()
            .filter(|evaluation| evaluation.status == "failed")
            .count();
        vec![
            BenchMetricEvaluation {
                metric_name: "pass_at_1".to_string(),
                metric_value: pass_at_1,
                metric_unit: "ratio".to_string(),
                passed_count: Some(passed),
                failed_count: Some(failed),
                skipped_count: Some(skipped),
                notes: "MultiPL-E native adapter uses normalized exact-match scoring for pass@1."
                    .to_string(),
            },
            BenchMetricEvaluation {
                metric_name: format!("pass_at_{}", options.samples.max(1)),
                metric_value: pass_at_k_value,
                metric_unit: "ratio".to_string(),
                passed_count: Some(passed),
                failed_count: Some(failed),
                skipped_count: Some(skipped),
                notes: "MultiPL-E native adapter uses normalized exact-match scoring for pass@k."
                    .to_string(),
            },
        ]
    }
}
