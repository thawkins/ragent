//! Fixed benchmark workbook schema and XLSX writing.

use anyhow::{Context, Result};
use chrono::NaiveDate;
use ragent_tools_core::xlsx::write_xlsx;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use crate::model::slugify_path_segment;

/// Fixed benchmark workbook run sheet name.
pub const RUN_SHEET: &str = "run";
/// Fixed benchmark workbook metrics sheet name.
pub const METRICS_SHEET: &str = "metrics";
/// Fixed benchmark workbook cases sheet name.
pub const CASES_SHEET: &str = "cases";
/// Fixed benchmark workbook artifacts sheet name.
pub const ARTIFACTS_SHEET: &str = "artifacts";

/// Fixed `run` worksheet columns.
pub const RUN_COLUMNS: &[&str] = &[
    "run_id",
    "bench_name",
    "date_utc",
    "started_at_utc",
    "finished_at_utc",
    "provider_id",
    "provider_name",
    "model_id",
    "model_slug",
    "thinking_enabled",
    "thinking_level",
    "thinking_budget_tokens",
    "command",
    "status",
    "dataset_revision",
    "evaluator_version",
    "git_commit_sha",
    "project_root",
    "data_root",
    "data_revision",
    "notes",
];

/// Fixed `metrics` worksheet columns.
pub const METRICS_COLUMNS: &[&str] = &[
    "run_id",
    "bench_name",
    "metric_name",
    "metric_value",
    "metric_unit",
    "split_name",
    "subset_name",
    "language",
    "sample_count",
    "passed_count",
    "failed_count",
    "skipped_count",
    "notes",
];

/// Fixed `cases` worksheet columns.
pub const CASES_COLUMNS: &[&str] = &[
    "run_id",
    "bench_name",
    "case_id",
    "case_index",
    "split_name",
    "subset_name",
    "language",
    "prompt_hash",
    "response_hash",
    "status",
    "score",
    "duration_ms",
    "tokens_input",
    "tokens_output",
    "samples_generated",
    "sandbox_backend",
    "error_code",
    "error_message",
    "notes",
];

/// Fixed `artifacts` worksheet columns.
pub const ARTIFACTS_COLUMNS: &[&str] = &[
    "run_id",
    "bench_name",
    "artifact_kind",
    "artifact_label",
    "relative_path",
    "content_hash",
    "notes",
];

/// Workbook row describing one benchmark run.
#[derive(Debug, Clone)]
pub struct BenchRunConfig {
    /// Stable run identifier.
    pub run_id: String,
    /// Canonical benchmark ID.
    pub bench_name: String,
    /// UTC date folder for the run.
    pub date_utc: String,
    /// Run start timestamp.
    pub started_at_utc: String,
    /// Run finish timestamp.
    pub finished_at_utc: String,
    /// Provider ID.
    pub provider_id: String,
    /// Provider display name.
    pub provider_name: String,
    /// Model ID.
    pub model_id: String,
    /// Slugged model ID.
    pub model_slug: String,
    /// Whether thinking was enabled for the run.
    pub thinking_enabled: bool,
    /// Effective thinking level.
    pub thinking_level: String,
    /// Effective thinking token budget.
    pub thinking_budget_tokens: Option<u32>,
    /// Original command string.
    pub command: String,
    /// Overall benchmark run status.
    pub status: String,
    /// Dataset/spec revision.
    pub dataset_revision: String,
    /// Git commit SHA for the project under test.
    pub git_commit_sha: String,
    /// Project root path.
    pub project_root: String,
    /// Initialized benchmark data root.
    pub data_root: String,
    /// Dataset revision.
    pub data_revision: String,
    /// Evaluator version string.
    pub evaluator_version: String,
    /// Freeform notes/warnings.
    pub notes: String,
}

/// Metric row for one benchmark workbook.
#[derive(Debug, Clone)]
pub struct BenchResultSummary {
    /// Stable run identifier.
    pub run_id: String,
    /// Canonical benchmark ID.
    pub bench_name: String,
    /// Primary metric name.
    pub metric_name: String,
    /// Primary metric value.
    pub metric_value: f64,
    /// Metric unit (`ratio`, `count`, `seconds`, `score`, etc.).
    pub metric_unit: String,
    /// Optional dataset split.
    pub split_name: Option<String>,
    /// Optional dataset subset.
    pub subset_name: Option<String>,
    /// Optional target language.
    pub language: Option<String>,
    /// Number of attempted cases or samples.
    pub sample_count: usize,
    /// Count of passing cases when available.
    pub passed_count: Option<usize>,
    /// Count of failing cases when available.
    pub failed_count: Option<usize>,
    /// Count of skipped cases when available.
    pub skipped_count: Option<usize>,
    /// Metric notes.
    pub notes: String,
}

/// Per-case workbook row.
#[derive(Debug, Clone)]
pub struct BenchCaseResult {
    /// Stable run identifier.
    pub run_id: String,
    /// Benchmark ID.
    pub bench_name: String,
    /// Case identifier.
    pub case_id: String,
    /// 1-based case index.
    pub case_index: usize,
    /// Dataset split name.
    pub split_name: Option<String>,
    /// Dataset subset name.
    pub subset_name: Option<String>,
    /// Language tag.
    pub language: Option<String>,
    /// Normalized prompt hash.
    pub prompt_hash: String,
    /// Response hash.
    pub response_hash: String,
    /// Case status.
    pub status: String,
    /// Optional numeric score.
    pub score: Option<f64>,
    /// End-to-end case duration in milliseconds.
    pub duration_ms: Option<u64>,
    /// Prompt/input token count.
    pub tokens_input: Option<u64>,
    /// Completion/output token count.
    pub tokens_output: Option<u64>,
    /// Number of generated samples.
    pub samples_generated: usize,
    /// Sandbox/runtime backend.
    pub sandbox_backend: String,
    /// Normalized error code.
    pub error_code: Option<String>,
    /// Normalized short error summary.
    pub error_message: Option<String>,
    /// Additional case notes.
    pub notes: String,
}

/// Artifact row written beside a benchmark workbook.
#[derive(Debug, Clone)]
pub struct BenchArtifactRecord {
    /// Stable run identifier.
    pub run_id: String,
    /// Benchmark ID.
    pub bench_name: String,
    /// Artifact kind.
    pub artifact_kind: String,
    /// Human-readable artifact label.
    pub artifact_label: String,
    /// Relative artifact path.
    pub relative_path: String,
    /// Optional content hash.
    pub content_hash: Option<String>,
    /// Artifact note.
    pub notes: String,
}

/// Build the canonical output workbook path for one suite.
#[must_use]
pub fn workbook_output_path(
    project_root: &Path,
    bench_name: &str,
    date_utc: NaiveDate,
    provider_id: &str,
    model_id: &str,
) -> PathBuf {
    project_root
        .join("benches")
        .join(bench_name)
        .join(date_utc.format("%Y-%m-%d").to_string())
        .join(provider_id)
        .join(format!("{}.xlsx", slugify_path_segment(model_id)))
}

/// Build the stable resume state sidecar path for one workbook.
#[must_use]
pub fn workbook_resume_state_path(path: &Path) -> PathBuf {
    append_sidecar_suffix(path, "run.json")
}

/// Build the config-specific debug sidecar path for one workbook.
#[must_use]
pub fn workbook_debug_sidecar_path(path: &Path, config_hash: &str) -> PathBuf {
    append_sidecar_suffix(path, &format!("{config_hash}.debug.json"))
}

/// Write one benchmark workbook using the fixed schema.
///
/// # Errors
///
/// Returns an error when the target directory or workbook cannot be written.
pub fn write_benchmark_workbook(
    path: &Path,
    run: &BenchRunConfig,
    summaries: &[BenchResultSummary],
    cases: &[BenchCaseResult],
    artifacts: &[BenchArtifactRecord],
) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create workbook directory {}", parent.display()))?;
    }

    let content = json!({
        "sheets": [
            {
                "name": RUN_SHEET,
                "rows": [
                    string_row(RUN_COLUMNS.iter().copied()),
                    string_vec_row(vec![
                        run.run_id.clone(),
                        run.bench_name.clone(),
                        run.date_utc.clone(),
                        run.started_at_utc.clone(),
                        run.finished_at_utc.clone(),
                        run.provider_id.clone(),
                        run.provider_name.clone(),
                        run.model_id.clone(),
                        run.model_slug.clone(),
                        run.thinking_enabled.to_string(),
                        run.thinking_level.clone(),
                        run.thinking_budget_tokens.map_or_else(String::new, |v| v.to_string()),
                        run.command.clone(),
                        run.status.clone(),
                        run.dataset_revision.clone(),
                        run.evaluator_version.clone(),
                        run.git_commit_sha.clone(),
                        run.project_root.clone(),
                        run.data_root.clone(),
                        run.data_revision.clone(),
                        run.notes.clone(),
                    ]),
                ],
            },
            {
                "name": METRICS_SHEET,
                "rows": sheet_rows(
                    METRICS_COLUMNS,
                    summaries.iter().map(|summary| {
                        vec![
                            summary.run_id.clone(),
                            summary.bench_name.clone(),
                            summary.metric_name.clone(),
                            summary.metric_value.to_string(),
                            summary.metric_unit.clone(),
                            summary.split_name.clone().unwrap_or_default(),
                            summary.subset_name.clone().unwrap_or_default(),
                            summary.language.clone().unwrap_or_default(),
                            summary.sample_count.to_string(),
                            summary.passed_count.map_or_else(String::new, |v| v.to_string()),
                            summary.failed_count.map_or_else(String::new, |v| v.to_string()),
                            summary.skipped_count.map_or_else(String::new, |v| v.to_string()),
                            summary.notes.clone(),
                        ]
                    }),
                ),
            },
            {
                "name": CASES_SHEET,
                "rows": sheet_rows(
                    CASES_COLUMNS,
                    cases.iter().map(|case| {
                        vec![
                            case.run_id.clone(),
                            case.bench_name.clone(),
                            case.case_id.clone(),
                            case.case_index.to_string(),
                            case.split_name.clone().unwrap_or_default(),
                            case.subset_name.clone().unwrap_or_default(),
                            case.language.clone().unwrap_or_default(),
                            case.prompt_hash.clone(),
                            case.response_hash.clone(),
                            case.status.clone(),
                            case.score.map_or_else(String::new, |v| v.to_string()),
                            case.duration_ms.map_or_else(String::new, |v| v.to_string()),
                            case.tokens_input.map_or_else(String::new, |v| v.to_string()),
                            case.tokens_output.map_or_else(String::new, |v| v.to_string()),
                            case.samples_generated.to_string(),
                            case.sandbox_backend.clone(),
                            case.error_code.clone().unwrap_or_default(),
                            case.error_message.clone().unwrap_or_default(),
                            case.notes.clone(),
                        ]
                    }),
                ),
            },
            {
                "name": ARTIFACTS_SHEET,
                "rows": sheet_rows(
                    ARTIFACTS_COLUMNS,
                    artifacts.iter().map(|artifact| {
                        vec![
                            artifact.run_id.clone(),
                            artifact.bench_name.clone(),
                            artifact.artifact_kind.clone(),
                            artifact.artifact_label.clone(),
                            artifact.relative_path.clone(),
                            artifact.content_hash.clone().unwrap_or_default(),
                            artifact.notes.clone(),
                        ]
                    }),
                ),
            }
        ]
    });

    write_xlsx(path, &content)
        .with_context(|| format!("write benchmark workbook {}", path.display()))?;
    Ok(())
}

fn append_sidecar_suffix(path: &Path, suffix: &str) -> PathBuf {
    let file_name = path
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "workbook.xlsx".to_string());
    path.with_file_name(format!("{file_name}.{suffix}"))
}

fn sheet_rows<I>(headers: &[&str], rows: I) -> Vec<Vec<Value>>
where
    I: IntoIterator<Item = Vec<String>>,
{
    std::iter::once(string_row(headers.iter().copied()))
        .chain(rows.into_iter().map(string_vec_row))
        .collect()
}

fn string_row<'a, I>(values: I) -> Vec<Value>
where
    I: IntoIterator<Item = &'a str>,
{
    values
        .into_iter()
        .map(|value| Value::String(value.to_string()))
        .collect()
}

fn string_vec_row(values: Vec<String>) -> Vec<Value> {
    values.into_iter().map(Value::String).collect()
}
