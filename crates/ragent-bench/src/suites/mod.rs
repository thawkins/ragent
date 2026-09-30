//! Phase 5 benchmark suite adapters.
//!
//! These adapters keep suite-specific prompt shaping and evaluation logic out of
//! the generic runner while preserving the normalized workbook schema.

use std::path::PathBuf;

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
            "benchmark suite '{other}' does not have a native benchmark adapter yet"
        )),
    }
}

pub use metrics::{
    accuracy_metric, average_metric, best_exact_or_similarity_sample, codebleu_score,
    count_passed_failed, edit_similarity, evaluate_exact_match_case, exact_match_count,
    first_sample_exact_match, pass_at_1, pass_at_k, resolution_rate, skipped_metric,
    skipped_metrics_for_suite,
};

pub(crate) fn strip_code_fences(sample: &str) -> String {
    let trimmed = sample.trim();
    if let Some(start) = trimmed.find("```") {
        let fenced = &trimmed[start + 3..];
        if let Some(end) = fenced.find("```") {
            return strip_leading_language_label(fenced[..end].trim_matches('\n'));
        }
    }
    strip_leading_language_label(trimmed)
}

fn strip_leading_language_label(sample: &str) -> String {
    let trimmed = sample.trim();
    let mut lines = trimmed.lines();
    let Some(first_line) = lines.next() else {
        return String::new();
    };
    if is_language_label(first_line) {
        return lines
            .collect::<Vec<_>>()
            .join("\n")
            .trim_matches('\n')
            .to_string();
    }
    trimmed.to_string()
}

fn is_language_label(line: &str) -> bool {
    // Using a static HashSet for O(1) lookups
    use std::sync::OnceLock;
    static LANG_SET: OnceLock<std::collections::HashSet<&'static str>> = OnceLock::new();
    let set = LANG_SET.get_or_init(|| {
        [
            "c++",
            "cpp",
            "c#",
            "csharp",
            "dart",
            "go",
            "haskell",
            "java",
            "javascript",
            "js",
            "julia",
            "kotlin",
            "lua",
            "php",
            "py",
            "python",
            "r",
            "rs",
            "rust",
            "scala",
            "ts",
            "typescript",
        ]
        .into_iter()
        .collect()
    });
    set.contains(line.trim().to_ascii_lowercase().as_str())
}

pub(crate) fn bench_temp_root() -> std::io::Result<PathBuf> {
    let root = std::env::current_dir()?.join("target").join("temp");
    std::fs::create_dir_all(&root)?;
    Ok(root)
}

/// Run a case's `execution_commands` sequence in `run_root`, returning the
/// final command's `(stdout, stderr)` on success.
///
/// Each command is run through `timeout` using the per-index timeout, with
/// `__FILENAME__` substituted and the program validated by
/// [`crate::exec_guard::validate_fixture_command`]. HumanEval and MBPP shared
/// near-identical copies of this loop (see `ANTIPAT.md` M3.16).
///
/// # Errors
///
/// Returns the suite-labelled failure message when a command exits non-zero,
/// when the command list is empty, or when the process cannot be launched.
pub(crate) fn run_fixture_commands(
    case: &BenchCaseFixture,
    run_root: &std::path::Path,
    file_name: &str,
    label: &str,
) -> Result<(String, String), String> {
    let mut last_stdout = String::new();
    let mut last_stderr = String::new();
    for (index, command_parts) in case.execution_commands.iter().enumerate() {
        let timeout_secs = case
            .execution_timeouts_secs
            .get(index)
            .copied()
            .unwrap_or(crate::exec_guard::FIXTURE_DEFAULT_TIMEOUT_SECS);
        let rendered_parts = command_parts
            .iter()
            .map(|part| part.replace("__FILENAME__", file_name))
            .collect::<Vec<_>>();
        // SEC-ragent-bench-001 (SECTASKS T-009): the fixture supplies the
        // program; it must be an allowlisted toolchain binary.
        crate::exec_guard::validate_fixture_command(&rendered_parts)?;
        let Some(program) = rendered_parts.first() else {
            return Err(format!("{label} command list was empty"));
        };
        let output = std::process::Command::new("timeout")
            .arg(format!("{timeout_secs}s"))
            .arg(program)
            .args(rendered_parts.iter().skip(1))
            .current_dir(run_root)
            .output()
            .map_err(|error| format!("launch {label} command `{program}`: {error}"))?;
        last_stdout = String::from_utf8_lossy(&output.stdout).to_string();
        last_stderr = String::from_utf8_lossy(&output.stderr).to_string();
        if !output.status.success() {
            let detail = [last_stderr.trim(), last_stdout.trim()]
                .into_iter()
                .find(|part| !part.is_empty())
                .unwrap_or("command failed");
            return Err(format!(
                "{label} command `{}` failed: {}",
                rendered_parts.join(" "),
                detail
            ));
        }
    }
    Ok((last_stdout, last_stderr))
}

/// Evaluate every generated sample with `run_sample`, returning the shared
/// [`BenchCaseEvaluation`] shape.
///
/// HumanEval and MBPP both ran an identical per-sample loop (count passes,
/// record the first pass's response, keep the first error); this is that loop,
/// parameterised on the runner and on the suite-specific notes/error text
/// (see `ANTIPAT.md` M3.16).
#[allow(clippy::too_many_arguments)]
pub(crate) fn evaluate_suite_samples(
    generation: &BenchGenerationResult,
    fallback: String,
    run_sample: impl Fn(&str) -> Result<(), String>,
    passed_notes: impl Fn(usize) -> String,
    failed_notes: impl Fn() -> String,
    error_code: &str,
) -> BenchCaseEvaluation {
    let mut passed_count = 0usize;
    let mut first_sample_passed = false;
    let mut first_error = None;
    let mut selected_response = fallback;

    for (idx, sample) in generation.samples.iter().enumerate() {
        match run_sample(&sample.text) {
            Ok(()) => {
                passed_count += 1;
                if idx == 0 {
                    first_sample_passed = true;
                }
                if passed_count == 1 {
                    selected_response = sample.text.clone();
                }
            }
            Err(error) => {
                if first_error.is_none() {
                    first_error = Some(error);
                }
            }
        }
    }

    let passed = passed_count > 0;
    BenchCaseEvaluation {
        status: if passed { "passed" } else { "failed" }.to_string(),
        score: Some(if passed { 1.0 } else { 0.0 }),
        selected_response,
        exact_match_count: passed_count,
        first_sample_exact_match: first_sample_passed,
        notes: if passed {
            passed_notes(passed_count)
        } else {
            failed_notes()
        },
        error_code: if passed {
            None
        } else {
            Some(error_code.to_string())
        },
        error_message: if passed { None } else { first_error },
    }
}

#[cfg(test)]
#[path = "../tests/inline/mod_tests.rs"]
mod tests;
