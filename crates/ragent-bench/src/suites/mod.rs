//! Phase 5 benchmark suite adapters.
//!
//! These adapters keep suite-specific prompt shaping and evaluation logic out of
//! the generic runner while preserving the normalized workbook schema.

pub mod apps;
pub mod bigcodebench;
pub mod crosscodeeval;
pub mod ds1000;
pub mod humaneval;
pub mod livecodebench;
pub mod mbpp;
mod metrics;
pub mod multipl_e;
pub mod repobench;
pub mod swebench;

use anyhow::{Result, anyhow};

use crate::command::BenchRunOptions;
use crate::data::BenchCaseFixture;
use crate::model::BenchGenerationResult;

/// Evaluation result for one benchmark case.
#[derive(Debug, Clone)]
pub struct BenchCaseEvaluation {
    /// Final case status.
    pub status: String,
    /// Primary numeric score for the case.
    pub score: Option<f64>,
    /// Response text selected for workbook output.
    pub selected_response: String,
    /// Count of exact-match samples for pass@k-style metrics.
    pub exact_match_count: usize,
    /// Whether the first sample matched exactly.
    pub first_sample_exact_match: bool,
    /// Notes written to the workbook.
    pub notes: String,
    /// Optional normalized error code.
    pub error_code: Option<String>,
    /// Optional normalized error message.
    pub error_message: Option<String>,
}

/// Normalized suite summary metric before workbook projection.
#[derive(Debug, Clone)]
pub struct BenchMetricEvaluation {
    /// Metric name such as `pass_at_1` or `accuracy`.
    pub metric_name: String,
    /// Metric value.
    pub metric_value: f64,
    /// Metric unit such as `ratio`.
    pub metric_unit: String,
    /// Count of passing cases if applicable.
    pub passed_count: Option<usize>,
    /// Count of failing cases if applicable.
    pub failed_count: Option<usize>,
    /// Count of skipped cases if applicable.
    pub skipped_count: Option<usize>,
    /// Metric notes.
    pub notes: String,
}

/// Suite-specific benchmark adapter.
pub trait BenchSuiteAdapter: Send + Sync {
    /// Canonical suite ID.
    fn suite_id(&self) -> &'static str;

    /// Build the provider-facing prompt for a benchmark case.
    fn build_prompt(&self, case: &BenchCaseFixture, options: &BenchRunOptions) -> String;

    /// Evaluate the generated samples for a benchmark case.
    fn evaluate_case(
        &self,
        case: &BenchCaseFixture,
        generation: &BenchGenerationResult,
        options: &BenchRunOptions,
    ) -> BenchCaseEvaluation;

    /// Summarize suite-level metrics across evaluated cases.
    fn summarize(
        &self,
        evaluations: &[BenchCaseEvaluation],
        options: &BenchRunOptions,
    ) -> Vec<BenchMetricEvaluation>;
}

/// Resolve the Phase 5 adapter for one suite.
///
/// # Errors
///
/// Returns an error when the suite is not part of the Phase 5 adapter set.
pub fn adapter_for_suite(suite_id: &str) -> Result<&'static dyn BenchSuiteAdapter> {
    match suite_id {
        "apps" => Ok(&apps::ADAPTER),
        "bigcodebench" => Ok(&bigcodebench::ADAPTER),
        "livecodebench" => Ok(&livecodebench::ADAPTER),
        "multipl-e" => Ok(&multipl_e::ADAPTER),
        "swebench-lite" => Ok(&swebench::LITE_ADAPTER),
        "swebench-verified" => Ok(&swebench::VERIFIED_ADAPTER),
        "humaneval" => Ok(&humaneval::ADAPTER),
        "mbpp" => Ok(&mbpp::ADAPTER),
        "ds1000" => Ok(&ds1000::ADAPTER),
        "repobench" => Ok(&repobench::ADAPTER),
        "crosscodeeval" => Ok(&crosscodeeval::ADAPTER),
        other => Err(anyhow!(
            "benchmark suite '{}' does not have a native benchmark adapter yet",
            other
        )),
    }
}

pub(crate) use metrics::{
    accuracy_metric, average_metric, best_exact_or_similarity_sample, codebleu_score,
    exact_match_count, first_sample_exact_match, pass_at_k, resolution_rate, skipped_metric,
};
