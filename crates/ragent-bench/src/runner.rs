//! Local MVP benchmark runner.

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::command::{BenchRunOptions, BenchTarget};
use crate::data::{bench_data_root, load_cases, verify_suite};
use crate::model::BenchModelRunner;
use crate::registry::{expand_target, find_profile, requires_confirmation};
use crate::suites::{BenchCaseEvaluation, BenchMetricEvaluation, adapter_for_suite};
use crate::workbook::{
    BenchArtifactRecord, BenchCaseResult, BenchResultSummary, BenchRunConfig,
    workbook_debug_sidecar_path, workbook_output_path, workbook_resume_state_path,
    write_benchmark_workbook,
};

const BENCH_RUN_STATE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BenchRunStateSummary {
    metric_name: String,
    metric_value: f64,
    metric_unit: String,
    split_name: Option<String>,
    subset_name: Option<String>,
    language: Option<String>,
    sample_count: usize,
    passed_count: Option<usize>,
    failed_count: Option<usize>,
    skipped_count: Option<usize>,
    notes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BenchRunState {
    version: u32,
    run_id: String,
    bench_name: String,
    date_utc: String,
    provider_id: String,
    model_id: String,
    config_hash: String,
    workbook_relative_path: String,
    debug_relative_path: String,
    summaries: Vec<BenchRunStateSummary>,
}

/// Outcome of a completed benchmark run.
#[derive(Debug, Clone)]
pub struct BenchRunOutcome {
    /// Human-readable output for the TUI.
    pub message: String,
    /// Workbook paths written by this run.
    pub workbook_paths: Vec<std::path::PathBuf>,
    /// Per-suite summaries.
    pub summaries: Vec<BenchResultSummary>,
}

/// Snapshot of active benchmark progress for UI/status rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BenchRunProgress {
    /// Current concrete suite ID being processed.
    pub suite_id: String,
    /// 1-based suite index in the expanded run target.
    pub suite_index: usize,
    /// Number of suites in the expanded run target.
    pub total_suites: usize,
    /// Number of completed cases in the current suite.
    pub completed_cases: usize,
    /// Number of total cases in the current suite after limit expansion.
    pub total_cases: usize,
}

/// Shared progress handle for active benchmark runs.
#[derive(Debug, Clone, Default)]
pub struct BenchProgressHandle {
    inner: Arc<Mutex<Option<BenchRunProgress>>>,
}

impl BenchProgressHandle {
    /// Publish a fresh benchmark progress snapshot.
    pub fn set(&self, progress: BenchRunProgress) {
        match self.inner.lock() {
            Ok(mut guard) => *guard = Some(progress),
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                *guard = Some(progress);
            }
        }
    }

    /// Read the latest benchmark progress snapshot.
    #[must_use]
    pub fn snapshot(&self) -> Option<BenchRunProgress> {
        match self.inner.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    /// Clear the active benchmark progress snapshot.
    pub fn clear(&self) {
        match self.inner.lock() {
            Ok(mut guard) => *guard = None,
            Err(poisoned) => {
                let mut guard = poisoned.into_inner();
                *guard = None;
            }
        }
    }
}

/// Validate benchmark prerequisites before spawning a background run.
///
/// # Errors
///
/// Returns an error when confirmation is missing or required benchmark data has
/// not been initialized.
pub fn validate_run_prerequisites(
    project_root: &Path,
    target: &BenchTarget,
    options: &BenchRunOptions,
) -> Result<()> {
    if requires_confirmation(target) && !options.yes {
        bail!("This benchmark target requires `--yes`.");
    }
    for suite in expand_target(target)? {
        verify_suite(project_root, suite.id)?;
    }
    Ok(())
}

/// Execute a benchmark run against initialized local data.
///
/// # Errors
///
/// Returns an error when prerequisites fail, the run is cancelled, or workbook
/// output cannot be written.
pub fn run_target(
    project_root: &Path,
    model_runner: &dyn BenchModelRunner,
    command: &str,
    target: &BenchTarget,
    options: &BenchRunOptions,
    cancel: &AtomicBool,
) -> Result<BenchRunOutcome> {
    run_target_with_progress(
        project_root,
        model_runner,
        command,
        target,
        options,
        cancel,
        None,
    )
}

/// Execute a benchmark run against initialized local data with optional progress reporting.
///
/// # Errors
///
/// Returns an error when prerequisites fail, the run is cancelled, or workbook
/// output cannot be written.
pub fn run_target_with_progress(
    project_root: &Path,
    model_runner: &dyn BenchModelRunner,
    command: &str,
    target: &BenchTarget,
    options: &BenchRunOptions,
    cancel: &AtomicBool,
    progress: Option<&BenchProgressHandle>,
) -> Result<BenchRunOutcome> {
    validate_run_prerequisites(project_root, target, options)?;
    let selection = model_runner.selection();

    let suites = expand_target(target)?;
    if suites.is_empty() {
        let profile_name = match target {
            BenchTarget::Profile(id) => id.as_str(),
            _ => "target",
        };
        let profile = find_profile(profile_name)
            .ok_or_else(|| anyhow!("unknown benchmark profile: {profile_name}"))?;
        bail!(
            "Benchmark profile '{}' is not available in the local MVP yet ({})",
            profile.id,
            profile.description
        );
    }

    let git_commit_sha = git_commit_sha(project_root);
    let mut workbook_paths = Vec::new();
    let mut summaries = Vec::new();
    let mut message = String::from("From: /bench run\n## Benchmark Run\n\n");
    let total_suites = suites.len();
    for (suite_index, suite) in suites.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            bail!("benchmark run cancelled");
        }
        let started_at = Utc::now();
        let manifest = verify_suite(project_root, suite.id)?;
        let data_root = bench_data_root(project_root, suite.id);
        let cases = load_cases(&data_root)?;
        let adapter = adapter_for_suite(suite.id)?;
        let limited_cases: Vec<_> = match options.limit {
            Some(limit) => cases.into_iter().take(limit).collect(),
            None => cases,
        };
        if let Some(progress) = progress {
            progress.set(BenchRunProgress {
                suite_id: suite.id.to_string(),
                suite_index: suite_index + 1,
                total_suites,
                completed_cases: 0,
                total_cases: limited_cases.len(),
            });
        }
        let date = Utc::now().date_naive();
        let workbook_path = workbook_output_path(
            project_root,
            suite.id,
            date,
            &selection.provider_id,
            &selection.model_id,
        );
        let config_hash = build_run_config_hash(
            suite.id,
            &date.format("%Y-%m-%d").to_string(),
            &git_commit_sha,
            &manifest.revision,
            options,
            selection,
        );
        let resume_state_path = workbook_resume_state_path(&workbook_path);
        let debug_state_path = workbook_debug_sidecar_path(&workbook_path, &config_hash[..12]);
        if options.resume
            && let Some(resumed_summaries) = try_resume_existing_run(
                project_root,
                suite.id,
                &workbook_path,
                &resume_state_path,
                &config_hash,
            )?
        {
            let resumed_sample_count = resumed_summaries
                .first()
                .map(|summary| summary.sample_count)
                .unwrap_or(0);
            message.push_str(&format!(
                "- `{}` -> `{}` (resumed existing workbook; {} sample(s) recorded)\n",
                suite.id,
                workbook_path.display(),
                resumed_sample_count
            ));
            workbook_paths.push(workbook_path);
            summaries.extend(resumed_summaries);
            continue;
        }
        let run_config = BenchRunConfig {
            run_id: build_run_id(suite.id, selection),
            bench_name: suite.id.to_string(),
            date_utc: date.format("%Y-%m-%d").to_string(),
            started_at_utc: started_at.to_rfc3339(),
            finished_at_utc: Utc::now().to_rfc3339(),
            provider_id: selection.provider_id.clone(),
            provider_name: selection.provider_name.clone(),
            model_id: selection.model_id.clone(),
            model_slug: selection.model_slug.clone(),
            thinking_enabled: selection.thinking_config.is_effective_enabled(),
            thinking_level: format!("{:?}", selection.thinking_config.level).to_lowercase(),
            thinking_budget_tokens: selection.thinking_config.budget_tokens,
            command: command.to_string(),
            status: "completed".to_string(),
            dataset_revision: manifest.revision.clone(),
            git_commit_sha: git_commit_sha.clone(),
            project_root: project_root.display().to_string(),
            data_root: data_root.display().to_string(),
            data_revision: manifest.revision.clone(),
            evaluator_version: env!("CARGO_PKG_VERSION").to_string(),
            notes: build_run_notes(options, selection, &config_hash),
        };

        let case_outcomes: Vec<_> = limited_cases
            .iter()
            .enumerate()
            .map(|(idx, case)| {
                let prompt = adapter.build_prompt(case, options);
                let generation = model_runner.generate(&prompt, options, cancel)?;
                let evaluation = adapter.evaluate_case(case, &generation, options);
                let response_hash = hash_string(&evaluation.selected_response);
                let aggregated = aggregate_generation_stats(&generation);

                if let Some(progress) = progress {
                    progress.set(BenchRunProgress {
                        suite_id: suite.id.to_string(),
                        suite_index: suite_index + 1,
                        total_suites,
                        completed_cases: idx + 1,
                        total_cases: limited_cases.len(),
                    });
                }

                Ok((
                    BenchCaseResult {
                        run_id: run_config.run_id.clone(),
                        bench_name: suite.id.to_string(),
                        case_id: case.case_id.clone(),
                        case_index: idx + 1,
                        split_name: None,
                        subset_name: options.subset.clone(),
                        language: Some(case.language.clone()),
                        prompt_hash: hash_string(&prompt),
                        response_hash,
                        status: evaluation.status.clone(),
                        score: evaluation.score,
                        duration_ms: Some(aggregated.duration_ms),
                        tokens_input: Some(aggregated.input_tokens),
                        tokens_output: Some(aggregated.output_tokens),
                        samples_generated: generation.samples.len(),
                        sandbox_backend: if options.no_exec {
                            "generation_only".to_string()
                        } else {
                            "native_mvp".to_string()
                        },
                        error_code: evaluation.error_code.clone(),
                        error_message: evaluation.error_message.clone(),
                        notes: case_result_notes(&evaluation, aggregated.finish_reasons),
                    },
                    evaluation,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let (case_results, evaluations): (Vec<_>, Vec<_>) = case_outcomes.into_iter().unzip();

        let suite_metrics = adapter.summarize(&evaluations, options);
        let summary_rows = build_summary_rows(
            &run_config.run_id,
            suite.id,
            options,
            &case_results,
            &suite_metrics,
        );
        let run_state = BenchRunState {
            version: BENCH_RUN_STATE_VERSION,
            run_id: run_config.run_id.clone(),
            bench_name: suite.id.to_string(),
            date_utc: run_config.date_utc.clone(),
            provider_id: selection.provider_id.clone(),
            model_id: selection.model_id.clone(),
            config_hash: config_hash.clone(),
            workbook_relative_path: relative_path(project_root, &workbook_path),
            debug_relative_path: relative_path(project_root, &debug_state_path),
            summaries: summary_rows
                .iter()
                .map(|summary| BenchRunStateSummary {
                    metric_name: summary.metric_name.clone(),
                    metric_value: summary.metric_value,
                    metric_unit: summary.metric_unit.clone(),
                    split_name: summary.split_name.clone(),
                    subset_name: summary.subset_name.clone(),
                    language: summary.language.clone(),
                    sample_count: summary.sample_count,
                    passed_count: summary.passed_count,
                    failed_count: summary.failed_count,
                    skipped_count: summary.skipped_count,
                    notes: summary.notes.clone(),
                })
                .collect(),
        };
        let mut artifacts = vec![BenchArtifactRecord {
            run_id: run_config.run_id.clone(),
            bench_name: suite.id.to_string(),
            artifact_kind: "data_root".to_string(),
            artifact_label: "Initialized dataset root".to_string(),
            relative_path: relative_path(project_root, &data_root),
            content_hash: None,
            notes: format!("dataset revision {}", manifest.revision),
        }];

        let run_state_json = serde_json::to_string_pretty(&run_state)
            .context("serialize benchmark run state sidecar")?;
        write_sidecar(&resume_state_path, &run_state_json)?;
        write_sidecar(&debug_state_path, &run_state_json)?;
        let run_state_hash = hash_string(&run_state_json);
        artifacts.push(BenchArtifactRecord {
            run_id: run_config.run_id.clone(),
            bench_name: suite.id.to_string(),
            artifact_kind: "run_state".to_string(),
            artifact_label: "Resume state sidecar".to_string(),
            relative_path: relative_path(project_root, &resume_state_path),
            content_hash: Some(run_state_hash.clone()),
            notes: format!("config hash {config_hash}"),
        });
        artifacts.push(BenchArtifactRecord {
            run_id: run_config.run_id.clone(),
            bench_name: suite.id.to_string(),
            artifact_kind: "debug_state".to_string(),
            artifact_label: "Config-specific debug sidecar".to_string(),
            relative_path: relative_path(project_root, &debug_state_path),
            content_hash: Some(run_state_hash),
            notes: format!("config hash {config_hash}"),
        });
        write_benchmark_workbook(
            &workbook_path,
            &run_config,
            &summary_rows,
            &case_results,
            &artifacts,
        )?;
        let primary_metric = summary_rows
            .first()
            .map(|summary| format!("{}={:.3}", summary.metric_name, summary.metric_value))
            .unwrap_or_else(|| "no metrics".to_string());

        message.push_str(&format!(
            "- `{}` -> `{}` ({} case(s), {} sample(s) generated; {})\n",
            suite.id,
            workbook_path.display(),
            case_results.len(),
            case_results
                .iter()
                .map(|case| case.samples_generated)
                .sum::<usize>(),
            primary_metric
        ));
        workbook_paths.push(workbook_path);
        summaries.extend(summary_rows);
    }
    if let Some(progress) = progress {
        progress.clear();
    }
    Ok(BenchRunOutcome {
        message,
        workbook_paths,
        summaries,
    })
}

fn build_run_id(suite_id: &str, selection: &crate::model::ResolvedModelSelection) -> String {
    let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    let mut hasher = Sha256::new();
    hasher.update(suite_id.as_bytes());
    hasher.update(selection.provider_id.as_bytes());
    hasher.update(selection.model_id.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    format!(
        "bench-{}-{}-{}-{}",
        timestamp,
        selection.provider_slug,
        selection.model_slug,
        &digest[..8]
    )
}

fn build_run_notes(
    options: &BenchRunOptions,
    selection: &crate::model::ResolvedModelSelection,
    config_hash: &str,
) -> String {
    let mut notes = Vec::new();
    notes.push(format!("config_hash={config_hash}"));
    if let Some(model_display_name) = &selection.model_display_name {
        notes.push(format!("model_display_name={model_display_name}"));
    }
    if let Some(base_url) = &selection.base_url {
        notes.push(format!("base_url={base_url}"));
    }
    if let Some(context_window) = selection.context_window {
        notes.push(format!("context_window={context_window}"));
    }
    if let Some(max_output_tokens) = selection.max_output_tokens {
        notes.push(format!("max_output_tokens={max_output_tokens}"));
    }
    if let Some(request_multiplier) = selection.request_multiplier {
        notes.push(format!("request_multiplier={request_multiplier}"));
    }
    if let Some(limit) = options.limit {
        notes.push(format!("limit={limit}"));
    }
    if let Some(subset) = &options.subset {
        notes.push(format!("subset={subset}"));
    }
    if let Some(release) = &options.release {
        notes.push(format!("release={release}"));
    }
    if let Some(scenario) = &options.scenario {
        notes.push(format!("scenario={scenario}"));
    }
    if let Some(language) = &options.language {
        notes.push(format!("language={language}"));
    }
    if let Some(temperature) = options.temperature {
        notes.push(format!("temperature={temperature}"));
    }
    if let Some(top_p) = options.top_p {
        notes.push(format!("top_p={top_p}"));
    }
    if let Some(max_tokens) = options.max_tokens {
        notes.push(format!("max_tokens={max_tokens}"));
    }
    if options.deterministic {
        notes.push("deterministic=true".to_string());
    }
    if let Some(since) = options.since {
        notes.push(format!("since={since}"));
    }
    if let Some(until) = options.until {
        notes.push(format!("until={until}"));
    }
    if options.resume {
        notes.push("resume=true".to_string());
    }
    if options.no_exec {
        notes.push("no_exec=true".to_string());
    }
    if notes.is_empty() {
        "Phase 5 benchmark run with suite-specific MVP adapters enabled.".to_string()
    } else {
        format!(
            "Phase 5 benchmark run with parsed options: {}.",
            notes.join(", ")
        )
    }
}

fn hash_string(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn build_run_config_hash(
    suite_id: &str,
    date_utc: &str,
    git_commit_sha: &str,
    manifest_revision: &str,
    options: &BenchRunOptions,
    selection: &crate::model::ResolvedModelSelection,
) -> String {
    let mut hasher = Sha256::new();
    for segment in [
        format!("suite={suite_id}"),
        format!("date_utc={date_utc}"),
        format!("provider_id={}", selection.provider_id),
        format!("model_id={}", selection.model_id),
        format!("model_slug={}", selection.model_slug),
        format!("git_commit_sha={git_commit_sha}"),
        format!("manifest_revision={manifest_revision}"),
        format!(
            "thinking_enabled={}",
            selection.thinking_config.is_effective_enabled()
        ),
        format!("thinking_level={:?}", selection.thinking_config.level),
        format!(
            "thinking_budget_tokens={}",
            selection
                .thinking_config
                .budget_tokens
                .map_or_else(|| "none".to_string(), |value| value.to_string())
        ),
        format!(
            "limit={}",
            options
                .limit
                .map_or_else(|| "none".to_string(), |value| value.to_string())
        ),
        format!("samples={}", options.samples),
        format!(
            "subset={}",
            options.subset.clone().unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "release={}",
            options
                .release
                .clone()
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "scenario={}",
            options
                .scenario
                .clone()
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "language={}",
            options
                .language
                .clone()
                .unwrap_or_else(|| "none".to_string())
        ),
        format!(
            "temperature={}",
            options
                .temperature
                .map_or_else(|| "none".to_string(), |value| value.to_string())
        ),
        format!(
            "top_p={}",
            options
                .top_p
                .map_or_else(|| "none".to_string(), |value| value.to_string())
        ),
        format!(
            "max_tokens={}",
            options
                .max_tokens
                .map_or_else(|| "none".to_string(), |value| value.to_string())
        ),
        format!("deterministic={}", options.deterministic),
        format!(
            "since={}",
            options
                .since
                .map_or_else(|| "none".to_string(), |value| value.to_string())
        ),
        format!(
            "until={}",
            options
                .until
                .map_or_else(|| "none".to_string(), |value| value.to_string())
        ),
        format!("no_exec={}", options.no_exec),
    ] {
        hasher.update(segment.as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

fn try_resume_existing_run(
    project_root: &Path,
    suite_id: &str,
    workbook_path: &Path,
    resume_state_path: &Path,
    config_hash: &str,
) -> Result<Option<Vec<BenchResultSummary>>> {
    if !workbook_path.exists() || !resume_state_path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(resume_state_path).with_context(|| {
        format!(
            "read benchmark resume state sidecar {}",
            resume_state_path.display()
        )
    })?;
    let state: BenchRunState = serde_json::from_str(&raw).with_context(|| {
        format!(
            "parse benchmark resume state sidecar {}",
            resume_state_path.display()
        )
    })?;
    if state.version != BENCH_RUN_STATE_VERSION
        || state.bench_name != suite_id
        || state.config_hash != config_hash
    {
        return Ok(None);
    }
    let workbook_relative = relative_path(project_root, workbook_path);
    if state.workbook_relative_path != workbook_relative {
        return Ok(None);
    }
    Ok(Some(
        state
            .summaries
            .into_iter()
            .map(|summary| BenchResultSummary {
                run_id: state.run_id.clone(),
                bench_name: state.bench_name.clone(),
                metric_name: summary.metric_name,
                metric_value: summary.metric_value,
                metric_unit: summary.metric_unit,
                split_name: summary.split_name,
                subset_name: summary.subset_name,
                language: summary.language,
                sample_count: summary.sample_count,
                passed_count: summary.passed_count,
                failed_count: summary.failed_count,
                skipped_count: summary.skipped_count,
                notes: summary.notes,
            })
            .collect(),
    ))
}

struct AggregatedGenerationStats {
    input_tokens: u64,
    output_tokens: u64,
    duration_ms: u64,
    finish_reasons: Vec<String>,
}

fn aggregate_generation_stats(
    generation: &crate::model::BenchGenerationResult,
) -> AggregatedGenerationStats {
    AggregatedGenerationStats {
        input_tokens: generation
            .samples
            .iter()
            .filter_map(|sample| sample.input_tokens)
            .sum(),
        output_tokens: generation
            .samples
            .iter()
            .filter_map(|sample| sample.output_tokens)
            .sum(),
        duration_ms: generation
            .samples
            .iter()
            .map(|sample| sample.duration_ms)
            .sum(),
        finish_reasons: generation
            .samples
            .iter()
            .filter_map(|sample| sample.finish_reason.clone())
            .collect(),
    }
}

fn case_result_notes(evaluation: &BenchCaseEvaluation, finish_reasons: Vec<String>) -> String {
    if finish_reasons.is_empty() {
        evaluation.notes.clone()
    } else {
        format!(
            "{} Finish reasons: {}.",
            evaluation.notes,
            finish_reasons.join(", ")
        )
    }
}

fn build_summary_rows(
    run_id: &str,
    suite_id: &str,
    options: &BenchRunOptions,
    case_results: &[BenchCaseResult],
    metrics: &[BenchMetricEvaluation],
) -> Vec<BenchResultSummary> {
    metrics
        .iter()
        .map(|metric| BenchResultSummary {
            run_id: run_id.to_string(),
            bench_name: suite_id.to_string(),
            metric_name: metric.metric_name.clone(),
            metric_value: metric.metric_value,
            metric_unit: metric.metric_unit.clone(),
            split_name: None,
            subset_name: options.subset.clone(),
            language: options
                .language
                .clone()
                .or_else(|| case_results.first().and_then(|case| case.language.clone())),
            sample_count: case_results.len(),
            passed_count: metric.passed_count,
            failed_count: metric.failed_count,
            skipped_count: metric.skipped_count,
            notes: metric.notes.clone(),
        })
        .collect()
}

fn write_sidecar(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create sidecar directory {}", parent.display()))?;
    }
    std::fs::write(path, content)
        .with_context(|| format!("write benchmark sidecar {}", path.display()))
}

fn relative_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn git_commit_sha(project_root: &Path) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(project_root)
        .arg("rev-parse")
        .arg("HEAD")
        .output();
    match output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        _ => "unknown".to_string(),
    }
}
