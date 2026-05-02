//! Benchmark data root and manifest helpers.

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use crate::command::{BenchInitMode, BenchInitTarget};
use crate::registry::{
    BenchSuiteDef, all_suites, find_suite, resolve_suite_and_language, resolve_suite_language,
};

const MANIFEST_FILE: &str = "manifest.json";
const DATASET_DIR: &str = "dataset";
const CASES_FILE: &str = "cases.jsonl";
const MANIFEST_VERSION: u32 = 4;
const FULL_MBPP_URL: &str =
    "https://raw.githubusercontent.com/google-research/babelcode/main/data/hf_datasets/mbpp.jsonl";
const HUMANEVALPACK_ROWS_URL: &str = "https://datasets-server.huggingface.co/rows";
const FULL_INIT_SUPPORTED_SUITES: &[&str] = &["humaneval", "mbpp"];

/// One initialized benchmark case fixture.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BenchCaseFixture {
    /// Case identifier.
    pub case_id: String,
    /// Prompt or task text.
    pub prompt: String,
    /// Optional starter code or declaration prefix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub starter_code: Option<String>,
    /// Expected/reference text.
    pub reference: String,
    /// Language tag.
    pub language: String,
    /// Optional benchmark-native hidden/unit test payload.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub test_code: Option<String>,
    /// Optional benchmark-native callable entry point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_point: Option<String>,
    /// Optional benchmark-native class entry point.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_class: Option<String>,
    /// Optional benchmark-native execution commands.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub execution_commands: Vec<Vec<String>>,
    /// Optional benchmark-native execution timeouts in seconds.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub execution_timeouts_secs: Vec<u64>,
    /// Optional benchmark-native source file extension.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_extension: Option<String>,
}

/// On-disk manifest for initialized benchmark data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BenchDataManifest {
    /// Canonical suite ID.
    pub bench_name: String,
    /// Display name.
    pub display_name: String,
    /// Dataset language partition.
    pub language: String,
    /// Dataset revision string.
    pub revision: String,
    /// Source metadata used for this initialization.
    pub sources: Vec<BenchDataSource>,
    /// Initialization timestamp in UTC.
    pub initialized_at_utc: String,
    /// Relative dataset directory name.
    pub dataset_dir: String,
    /// Relative case fixture file name.
    pub case_file: String,
    /// Number of fixture cases.
    pub case_count: usize,
    /// Manifest status string.
    pub status: String,
    /// Format version for the on-disk manifest.
    pub manifest_version: u32,
    /// Files tracked for checksum verification.
    pub files: Vec<BenchDataFile>,
}

/// Source metadata recorded in the benchmark data manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BenchDataSource {
    /// Human-readable source kind.
    pub kind: String,
    /// Canonical source URL.
    pub url: String,
}

/// File metadata recorded in the benchmark data manifest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BenchDataFile {
    /// Relative path under the suite data root.
    pub relative_path: String,
    /// SHA-256 digest of the file contents.
    pub sha256: String,
    /// File size in bytes.
    pub bytes: u64,
}

/// Result of one `/bench init`.
#[derive(Debug, Clone)]
pub struct BenchInitOutcome {
    /// Suite metadata.
    pub suite: BenchSuiteDef,
    /// Effective dataset language.
    pub language: String,
    /// Data root path.
    pub data_root: PathBuf,
    /// Loaded or written manifest.
    pub manifest: BenchDataManifest,
    /// Whether data was created or rewritten.
    pub created: bool,
    /// Whether this invocation was verify-only.
    pub verified_only: bool,
    /// Initialization mode used for this suite.
    pub mode: BenchInitMode,
}

/// Progress event emitted during `/bench init`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BenchInitProgressEvent {
    /// One suite/language initialization is starting.
    Starting {
        /// Canonical benchmark suite ID.
        suite_id: String,
        /// Effective dataset language.
        language: String,
        /// Initialization mode.
        mode: BenchInitMode,
        /// Whether this event is for verify-only mode.
        verify_only: bool,
    },
    /// One suite/language initialization finished successfully.
    Finished {
        /// Canonical benchmark suite ID.
        suite_id: String,
        /// Effective dataset language.
        language: String,
        /// Initialization mode.
        mode: BenchInitMode,
        /// Whether this event is for verify-only mode.
        verify_only: bool,
        /// Number of normalized cases in the resulting manifest.
        case_count: usize,
        /// Final data root for this suite/language partition.
        data_root: PathBuf,
    },
}

/// Initialize or verify a benchmark target.
///
/// # Errors
///
/// Returns an error when the target is unsupported for initialization or any
/// concrete suite initialization fails.
pub fn init_target(
    project_root: &Path,
    target: &BenchInitTarget,
    mode: BenchInitMode,
    language: Option<&str>,
    force_download: bool,
    verify_only: bool,
) -> Result<Vec<BenchInitOutcome>> {
    init_target_with_progress(
        project_root,
        target,
        mode,
        language,
        force_download,
        verify_only,
        |_| {},
    )
}

/// Initialize or verify a benchmark target with progress callbacks.
///
/// # Errors
///
/// Returns an error when the target is unsupported for initialization or any
/// concrete suite initialization fails.
pub fn init_target_with_progress<F>(
    project_root: &Path,
    target: &BenchInitTarget,
    mode: BenchInitMode,
    language: Option<&str>,
    force_download: bool,
    verify_only: bool,
    mut on_progress: F,
) -> Result<Vec<BenchInitOutcome>>
where
    F: FnMut(BenchInitProgressEvent),
{
    match target {
        BenchInitTarget::Suite(suite_id) => {
            let suite = find_suite(suite_id)
                .ok_or_else(|| anyhow!("unknown benchmark suite: {suite_id}"))?;
            if mode == BenchInitMode::Full && language.is_none() {
                let mut outcomes = Vec::new();
                for suite_language in suite.languages.iter().copied() {
                    on_progress(BenchInitProgressEvent::Starting {
                        suite_id: suite_id.to_string(),
                        language: suite_language.to_string(),
                        mode,
                        verify_only,
                    });
                    let outcome = init_suite_mode(
                        project_root,
                        suite_id,
                        mode,
                        Some(suite_language),
                        force_download,
                        verify_only,
                    )?;
                    on_progress(BenchInitProgressEvent::Finished {
                        suite_id: outcome.suite.id.to_string(),
                        language: outcome.language.clone(),
                        mode: outcome.mode,
                        verify_only: outcome.verified_only,
                        case_count: outcome.manifest.case_count,
                        data_root: outcome.data_root.clone(),
                    });
                    outcomes.push(outcome);
                }
                return Ok(outcomes);
            }
            let (suite, effective_language) = resolve_suite_and_language(suite_id, language)?;
            on_progress(BenchInitProgressEvent::Starting {
                suite_id: suite.id.to_string(),
                language: effective_language.to_string(),
                mode,
                verify_only,
            });
            let outcome = init_suite_mode(
                project_root,
                suite_id,
                mode,
                language,
                force_download,
                verify_only,
            )?;
            on_progress(BenchInitProgressEvent::Finished {
                suite_id: outcome.suite.id.to_string(),
                language: outcome.language.clone(),
                mode: outcome.mode,
                verify_only: outcome.verified_only,
                case_count: outcome.manifest.case_count,
                data_root: outcome.data_root.clone(),
            });
            Ok(vec![outcome])
        }
        BenchInitTarget::All => {
            ensure_full_init_supported_for_target(target, mode)?;
            let mut outcomes = Vec::new();
            for suite in all_suites() {
                let languages = if mode == BenchInitMode::Full && language.is_none() {
                    suite.languages.to_vec()
                } else {
                    vec![resolve_suite_language(suite, language)?]
                };
                for suite_language in languages {
                    on_progress(BenchInitProgressEvent::Starting {
                        suite_id: suite.id.to_string(),
                        language: suite_language.to_string(),
                        mode,
                        verify_only,
                    });
                    let outcome = init_suite_mode(
                        project_root,
                        suite.id,
                        mode,
                        Some(suite_language),
                        force_download,
                        verify_only,
                    )?;
                    on_progress(BenchInitProgressEvent::Finished {
                        suite_id: outcome.suite.id.to_string(),
                        language: outcome.language.clone(),
                        mode: outcome.mode,
                        verify_only: outcome.verified_only,
                        case_count: outcome.manifest.case_count,
                        data_root: outcome.data_root.clone(),
                    });
                    outcomes.push(outcome);
                }
            }
            Ok(outcomes)
        }
        BenchInitTarget::Full => {
            ensure_full_init_supported_for_target(target, BenchInitMode::Full)?;
            let mut outcomes = Vec::new();
            for suite in all_suites() {
                let languages = if let Some(requested_language) = language {
                    vec![resolve_suite_language(suite, Some(requested_language))?]
                } else {
                    suite.languages.to_vec()
                };
                for suite_language in languages {
                    on_progress(BenchInitProgressEvent::Starting {
                        suite_id: suite.id.to_string(),
                        language: suite_language.to_string(),
                        mode: BenchInitMode::Full,
                        verify_only,
                    });
                    let outcome = init_suite_mode(
                        project_root,
                        suite.id,
                        BenchInitMode::Full,
                        Some(suite_language),
                        force_download,
                        verify_only,
                    )?;
                    on_progress(BenchInitProgressEvent::Finished {
                        suite_id: outcome.suite.id.to_string(),
                        language: outcome.language.clone(),
                        mode: outcome.mode,
                        verify_only: outcome.verified_only,
                        case_count: outcome.manifest.case_count,
                        data_root: outcome.data_root.clone(),
                    });
                    outcomes.push(outcome);
                }
            }
            Ok(outcomes)
        }
    }
}

/// Build the initialized dataset root for one suite.
#[must_use]
pub fn bench_data_root(project_root: &Path, suite_id: &str) -> PathBuf {
    if let Some(suite) = find_suite(suite_id) {
        return bench_data_root_for_language(project_root, suite_id, suite.default_language);
    }
    project_root.join("benches").join("data").join(suite_id)
}

/// Build the initialized dataset root for one suite/language partition.
#[must_use]
pub fn bench_data_root_for_language(
    project_root: &Path,
    suite_id: &str,
    language: &str,
) -> PathBuf {
    project_root
        .join("benches")
        .join("data")
        .join(suite_id)
        .join(language)
}

/// Load a benchmark manifest from disk.
///
/// # Errors
///
/// Returns an error when the manifest file is missing or malformed.
pub fn load_manifest(data_root: &Path) -> Result<BenchDataManifest> {
    let manifest_path = data_root.join(MANIFEST_FILE);
    let raw = std::fs::read_to_string(&manifest_path)
        .with_context(|| format!("read manifest {}", manifest_path.display()))?;
    let manifest: BenchDataManifest = serde_json::from_str(&raw)
        .with_context(|| format!("parse manifest {}", manifest_path.display()))?;
    Ok(manifest)
}

/// Verify an initialized suite.
///
/// # Errors
///
/// Returns an error when the manifest or case file is missing or inconsistent.
pub fn verify_suite(project_root: &Path, suite_id: &str) -> Result<BenchDataManifest> {
    verify_suite_with_language(project_root, suite_id, None)
}

/// Verify an initialized suite/language dataset partition.
///
/// # Errors
///
/// Returns an error when the manifest or case file is missing or inconsistent.
pub fn verify_suite_with_language(
    project_root: &Path,
    suite_id: &str,
    requested_language: Option<&str>,
) -> Result<BenchDataManifest> {
    let (suite, language) = resolve_suite_and_language(suite_id, requested_language)?;
    let data_root = bench_data_root_for_language(project_root, suite_id, language);
    if !data_root.is_dir() {
        bail!(
            "Benchmark data for '{}' (language '{}') is not initialized. Run `/bench init {} --language {}` first.",
            suite_id,
            language,
            suite_id,
            language
        );
    }
    let manifest = load_manifest(&data_root)?;
    if manifest.bench_name != suite_id {
        bail!(
            "Benchmark manifest mismatch for '{}': found '{}'",
            suite_id,
            manifest.bench_name
        );
    }
    if manifest.manifest_version != MANIFEST_VERSION {
        bail!(
            "Benchmark manifest for '{}' has unsupported version {}.",
            suite_id,
            manifest.manifest_version
        );
    }
    if manifest.status != "ready" {
        bail!(
            "Benchmark manifest for '{}' is not ready (status='{}').",
            suite_id,
            manifest.status
        );
    }
    if manifest.language != language {
        bail!(
            "Benchmark manifest mismatch for '{}' language: expected '{}' but found '{}'.",
            suite_id,
            language,
            manifest.language
        );
    }
    if manifest.sources.is_empty() {
        bail!(
            "Benchmark manifest for '{}' does not record any sources.",
            suite_id
        );
    }
    if manifest.files.is_empty() {
        bail!(
            "Benchmark manifest for '{}' does not record any tracked files.",
            suite_id
        );
    }
    for file in &manifest.files {
        verify_manifest_file(&data_root, file)?;
    }
    let cases = load_cases_from_manifest(&data_root, &manifest)?;
    if cases.len() != manifest.case_count {
        bail!(
            "Benchmark manifest for '{}' expects {} case(s) but found {}.",
            suite_id,
            manifest.case_count,
            cases.len()
        );
    }
    validate_case_fixtures(suite.id, &manifest, &cases)?;
    Ok(manifest)
}

/// Initialize or verify one suite's data root.
///
/// # Errors
///
/// Returns an error when the suite is unknown or filesystem operations fail.
pub fn init_suite(
    project_root: &Path,
    suite_id: &str,
    force_download: bool,
    verify_only: bool,
) -> Result<BenchInitOutcome> {
    init_suite_with_language(project_root, suite_id, None, force_download, verify_only)
}

/// Initialize or verify one suite's data root for the selected language.
///
/// # Errors
///
/// Returns an error when the suite is unknown or filesystem operations fail.
pub fn init_suite_with_language(
    project_root: &Path,
    suite_id: &str,
    language: Option<&str>,
    force_download: bool,
    verify_only: bool,
) -> Result<BenchInitOutcome> {
    init_suite_mode(
        project_root,
        suite_id,
        BenchInitMode::Sample,
        language,
        force_download,
        verify_only,
    )
}

/// Initialize or verify one suite's data root using the requested mode.
///
/// # Errors
///
/// Returns an error when the suite is unknown, full-data ingestion is not yet
/// supported, or filesystem/network operations fail.
pub fn init_suite_mode(
    project_root: &Path,
    suite_id: &str,
    mode: BenchInitMode,
    requested_language: Option<&str>,
    force_download: bool,
    verify_only: bool,
) -> Result<BenchInitOutcome> {
    let (suite, language) = resolve_suite_and_language(suite_id, requested_language)?;
    let suite = *suite;
    let language = language.to_string();
    let data_root = bench_data_root_for_language(project_root, suite_id, &language);
    let data_parent = project_root.join("benches").join("data");

    if verify_only {
        let manifest = verify_suite_with_language(project_root, suite_id, Some(&language))?;
        return Ok(BenchInitOutcome {
            suite,
            language,
            data_root,
            manifest,
            created: false,
            verified_only: true,
            mode,
        });
    }

    if !force_download
        && let Ok(manifest) = verify_suite_with_language(project_root, suite_id, Some(&language))
    {
        return Ok(BenchInitOutcome {
            suite,
            language,
            data_root,
            manifest,
            created: false,
            verified_only: false,
            mode,
        });
    }

    std::fs::create_dir_all(&data_parent)
        .with_context(|| format!("create benchmark data parent {}", data_parent.display()))?;

    if data_root.exists() {
        std::fs::remove_dir_all(&data_root)
            .with_context(|| format!("remove benchmark data root {}", data_root.display()))?;
    }

    let staging_root = data_parent.join(format!(
        ".{}.{}.tmp-{}",
        suite_id,
        language,
        uuid::Uuid::new_v4().simple()
    ));
    let build_result = (|| -> Result<()> {
        std::fs::create_dir_all(&staging_root)
            .with_context(|| format!("create staging benchmark root {}", staging_root.display()))?;
        let dataset_dir = staging_root.join(DATASET_DIR);
        std::fs::create_dir_all(&dataset_dir)
            .with_context(|| format!("create dataset directory {}", dataset_dir.display()))?;

        let prepared =
            prepare_suite_dataset(&staging_root, suite_id, &language, mode, force_download)?;
        let fixtures = prepared.fixtures;
        let case_file = Path::new(DATASET_DIR).join(CASES_FILE);
        write_cases(&staging_root, &case_file, &fixtures)?;
        let mut files = vec![manifest_file_for_path(&staging_root, &case_file)?];
        for relative_path in prepared.tracked_paths {
            files.push(manifest_file_for_path(&staging_root, &relative_path)?);
        }
        let manifest = BenchDataManifest {
            bench_name: suite.id.to_string(),
            display_name: suite.display_name.to_string(),
            language: language.clone(),
            revision: suite.revision.to_string(),
            sources: suite
                .sources
                .iter()
                .map(|url| BenchDataSource {
                    kind: "dataset".to_string(),
                    url: (*url).to_string(),
                })
                .collect(),
            initialized_at_utc: Utc::now().to_rfc3339(),
            dataset_dir: DATASET_DIR.to_string(),
            case_file: case_file.display().to_string(),
            case_count: fixtures.len(),
            status: "ready".to_string(),
            manifest_version: MANIFEST_VERSION,
            files,
        };
        write_manifest(&staging_root, &manifest)?;
        Ok(())
    })();

    if let Err(error) = build_result {
        let _ = std::fs::remove_dir_all(&staging_root);
        return Err(error);
    }

    if let Some(parent) = data_root.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create benchmark language root {}", parent.display()))?;
    }
    std::fs::rename(&staging_root, &data_root).with_context(|| {
        format!(
            "finalize benchmark data root {} -> {}",
            staging_root.display(),
            data_root.display()
        )
    })?;
    let manifest = verify_suite_with_language(project_root, suite_id, Some(&language))?;

    Ok(BenchInitOutcome {
        suite,
        language,
        data_root,
        manifest,
        created: true,
        verified_only: false,
        mode,
    })
}

/// Load the case fixtures for one initialized suite.
///
/// # Errors
///
/// Returns an error when the case file is missing or malformed.
pub fn load_cases(data_root: &Path) -> Result<Vec<BenchCaseFixture>> {
    let manifest = load_manifest(data_root)?;
    load_cases_from_manifest(data_root, &manifest)
}

fn load_cases_from_manifest(
    data_root: &Path,
    manifest: &BenchDataManifest,
) -> Result<Vec<BenchCaseFixture>> {
    let cases_path = data_root.join(&manifest.case_file);
    let file = std::fs::File::open(&cases_path)
        .with_context(|| format!("open case file {}", cases_path.display()))?;
    let reader = BufReader::new(file);
    let mut cases = Vec::new();
    for line in reader.lines() {
        let line = line.with_context(|| format!("read case line {}", cases_path.display()))?;
        if line.trim().is_empty() {
            continue;
        }
        let fixture: BenchCaseFixture = serde_json::from_str(&line)
            .with_context(|| format!("parse case line {}", cases_path.display()))?;
        cases.push(fixture);
    }
    Ok(cases)
}

fn write_manifest(data_root: &Path, manifest: &BenchDataManifest) -> Result<()> {
    let manifest_path = data_root.join(MANIFEST_FILE);
    let raw = serde_json::to_string_pretty(manifest)?;
    std::fs::write(&manifest_path, raw)
        .with_context(|| format!("write manifest {}", manifest_path.display()))?;
    Ok(())
}

fn write_cases(
    data_root: &Path,
    relative_case_path: &Path,
    fixtures: &[BenchCaseFixture],
) -> Result<()> {
    let cases_path = data_root.join(relative_case_path);
    let mut file = std::fs::File::create(&cases_path)
        .with_context(|| format!("create case file {}", cases_path.display()))?;
    for fixture in fixtures {
        let line = serde_json::to_string(fixture)?;
        writeln!(file, "{line}")
            .with_context(|| format!("write case file {}", cases_path.display()))?;
    }
    Ok(())
}

struct PreparedSuiteDataset {
    fixtures: Vec<BenchCaseFixture>,
    tracked_paths: Vec<PathBuf>,
}

fn ensure_full_init_supported_for_target(
    target: &BenchInitTarget,
    mode: BenchInitMode,
) -> Result<()> {
    if mode != BenchInitMode::Full && !matches!(target, BenchInitTarget::Full) {
        return Ok(());
    }
    let unsupported: Vec<_> = all_suites()
        .iter()
        .map(|suite| suite.id)
        .filter(|suite_id| !FULL_INIT_SUPPORTED_SUITES.contains(suite_id))
        .collect();
    if unsupported.is_empty() {
        return Ok(());
    }
    let joined = unsupported.join(", ");
    if matches!(target, BenchInitTarget::Full) {
        bail!(
            "benchmark init target `full` is not ready yet; full dataset ingestion is still missing for: {joined}"
        );
    }
    bail!(
        "full dataset ingestion is not available for every suite yet; missing support for: {joined}"
    );
}

fn prepare_suite_dataset(
    staging_root: &Path,
    suite_id: &str,
    language: &str,
    mode: BenchInitMode,
    force_download: bool,
) -> Result<PreparedSuiteDataset> {
    match mode {
        BenchInitMode::Sample => Ok(PreparedSuiteDataset {
            fixtures: suite_fixtures(suite_id, language)?,
            tracked_paths: Vec::new(),
        }),
        BenchInitMode::Full => {
            prepare_full_suite_dataset(staging_root, suite_id, language, force_download)
        }
    }
}

fn suite_fixtures(suite_id: &str, language: &str) -> Result<Vec<BenchCaseFixture>> {
    match suite_id {
        "humaneval" => init_humaneval(language),
        "mbpp" => init_mbpp(language),
        "apps" => init_apps(language),
        "ds1000" => init_ds1000(language),
        "multipl-e" => init_multipl_e(language),
        "repobench" => init_repobench(language),
        "crosscodeeval" => init_crosscodeeval(language),
        "swebench-lite" => init_swebench_lite(language),
        "swebench-verified" => init_swebench_verified(language),
        "livecodebench" => init_livecodebench(language),
        "bigcodebench" => init_bigcodebench(language),
        other => bail!("unknown benchmark suite: {other}"),
    }
}

fn prepare_full_suite_dataset(
    staging_root: &Path,
    suite_id: &str,
    language: &str,
    force_download: bool,
) -> Result<PreparedSuiteDataset> {
    match suite_id {
        "humaneval" => init_humaneval_full(staging_root, language, force_download),
        "mbpp" => init_mbpp_full(staging_root, language, force_download),
        other => bail!("full benchmark dataset initialization is not yet supported for `{other}`"),
    }
}

fn verify_manifest_file(data_root: &Path, file: &BenchDataFile) -> Result<()> {
    let path = data_root.join(&file.relative_path);
    let metadata = std::fs::metadata(&path)
        .with_context(|| format!("stat tracked benchmark file {}", path.display()))?;
    if metadata.len() != file.bytes {
        bail!(
            "Benchmark data file '{}' size mismatch: expected {} bytes but found {}.",
            file.relative_path,
            file.bytes,
            metadata.len()
        );
    }
    let digest = sha256_file(&path)?;
    if digest != file.sha256 {
        bail!(
            "Benchmark data file '{}' checksum mismatch.",
            file.relative_path
        );
    }
    Ok(())
}

fn manifest_file_for_path(data_root: &Path, relative_path: &Path) -> Result<BenchDataFile> {
    let full_path = data_root.join(relative_path);
    let metadata = std::fs::metadata(&full_path)
        .with_context(|| format!("stat benchmark file {}", full_path.display()))?;
    Ok(BenchDataFile {
        relative_path: relative_path.display().to_string(),
        sha256: sha256_file(&full_path)?,
        bytes: metadata.len(),
    })
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes =
        std::fs::read(path).with_context(|| format!("read benchmark file {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
#[derive(Debug, Deserialize)]
struct HumanEvalFullRecord {
    task_id: String,
    prompt: String,
    canonical_solution: String,
    test: String,
    entry_point: String,
}

#[derive(Debug, Deserialize)]
struct HumanevalPackRecord {
    task_id: String,
    prompt: String,
    declaration: String,
    canonical_solution: String,
    entry_point: String,
    #[serde(default)]
    import: String,
    #[serde(default)]
    test_setup: String,
    test: String,
    instruction: String,
}

#[derive(Debug, Deserialize)]
struct DatasetRowsResponse<T> {
    rows: Vec<DatasetRowEnvelope<T>>,
}

#[derive(Debug, Deserialize)]
struct DatasetRowEnvelope<T> {
    row: T,
}

#[derive(Debug, Deserialize)]
struct BcMbppFullRecord {
    qid: String,
    language: String,
    extension: String,
    #[serde(default)]
    commands: Vec<Vec<String>>,
    #[serde(default)]
    timeouts: Vec<u64>,
    #[serde(default)]
    entry_cls_name: String,
    #[serde(default)]
    entry_fn_name: String,
    signature_with_docstring: String,
    text: String,
    #[serde(default)]
    test_code: String,
    #[serde(default)]
    solution_python: String,
}

fn download_url_bytes(url: &str) -> Result<Vec<u8>> {
    let url = url.to_string();
    std::thread::spawn(move || -> Result<Vec<u8>> {
        let response = reqwest::blocking::Client::new()
            .get(&url)
            .send()
            .with_context(|| format!("download benchmark data from {url}"))?
            .error_for_status()
            .with_context(|| format!("benchmark data request failed for {url}"))?;
        let bytes = response
            .bytes()
            .with_context(|| format!("read benchmark data response body from {url}"))?;
        Ok(bytes.to_vec())
    })
    .join()
    .map_err(|_| anyhow!("benchmark download worker thread panicked"))?
}

fn download_url_bytes_cached(url: &str, cache_name: &str, force_refresh: bool) -> Result<Vec<u8>> {
    let cache_dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("target")
        .join("temp")
        .join("bench-downloads");
    std::fs::create_dir_all(&cache_dir)
        .with_context(|| format!("create benchmark cache directory {}", cache_dir.display()))?;
    let cache_path = cache_dir.join(cache_name);
    if !force_refresh && cache_path.is_file() {
        return std::fs::read(&cache_path)
            .with_context(|| format!("read benchmark cache {}", cache_path.display()));
    }
    let bytes = download_url_bytes(url)?;
    std::fs::write(&cache_path, &bytes)
        .with_context(|| format!("write benchmark cache {}", cache_path.display()))?;
    Ok(bytes)
}

fn write_downloaded_file(
    staging_root: &Path,
    relative_path: &Path,
    bytes: &[u8],
) -> Result<PathBuf> {
    let full_path = staging_root.join(relative_path);
    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create benchmark download directory {}", parent.display()))?;
    }
    std::fs::write(&full_path, bytes)
        .with_context(|| format!("write benchmark download {}", full_path.display()))?;
    Ok(relative_path.to_path_buf())
}

fn require_fixture_language<'a>(
    suite_id: &str,
    requested_language: &'a str,
    expected_language: &str,
) -> Result<&'a str> {
    if requested_language != expected_language {
        bail!(
            "benchmark suite '{}' does not provide init fixtures for language '{}'; use '{}'",
            suite_id,
            requested_language,
            expected_language
        );
    }
    Ok(requested_language)
}

fn init_humaneval_full(
    staging_root: &Path,
    language: &str,
    force_download: bool,
) -> Result<PreparedSuiteDataset> {
    let (canonical_language, config) = canonical_humanevalpack_language(language)?;
    let records = fetch_humanevalpack_records(config, force_download)?;
    let fixtures = records
        .into_iter()
        .map(|record| humanevalpack_fixture_from_record(canonical_language, record))
        .collect::<Result<Vec<_>>>()?;
    if fixtures.is_empty() {
        bail!("HumanEvalPack did not contain any fixtures for language '{language}'");
    }

    let relative_path = Path::new(DATASET_DIR)
        .join("raw")
        .join(format!("humanevalpack-{canonical_language}.json"));
    let tracked = write_downloaded_file(
        staging_root,
        &relative_path,
        serde_json::to_vec_pretty(&fixtures)
            .context("serialize humaneval fixtures")?
            .as_slice(),
    )?;
    Ok(PreparedSuiteDataset {
        fixtures,
        tracked_paths: vec![tracked],
    })
}

fn init_mbpp_full(
    staging_root: &Path,
    language: &str,
    force_download: bool,
) -> Result<PreparedSuiteDataset> {
    let requested_language = canonical_mbpp_language(language)?;
    let relative_path = Path::new(DATASET_DIR).join("raw").join("bc-mbpp.jsonl");
    let bytes = download_url_bytes_cached(FULL_MBPP_URL, "mbpp-bc-mbpp.jsonl", force_download)?;
    let raw = String::from_utf8(bytes).context("decode MBPP dataset as UTF-8")?;
    let mut raw_partition = String::new();
    let mut fixtures = Vec::new();
    for line in raw.lines().filter(|line| !line.trim().is_empty()) {
        let record: BcMbppFullRecord =
            serde_json::from_str(line).with_context(|| "parse BC-MBPP jsonl line")?;
        if canonical_mbpp_language(&record.language)? != requested_language {
            continue;
        }
        raw_partition.push_str(line);
        raw_partition.push('\n');
        fixtures.push(bc_mbpp_fixture_from_record(record)?);
    }
    if fixtures.is_empty() {
        bail!("BC-MBPP did not contain any fixtures for language '{language}'");
    }
    let tracked = write_downloaded_file(staging_root, &relative_path, raw_partition.as_bytes())?;
    Ok(PreparedSuiteDataset {
        fixtures,
        tracked_paths: vec![tracked],
    })
}

fn fixture(case_id: &str, prompt: &str, reference: &str, language: &str) -> Vec<BenchCaseFixture> {
    vec![BenchCaseFixture {
        case_id: case_id.to_string(),
        prompt: prompt.to_string(),
        starter_code: None,
        reference: reference.to_string(),
        language: language.to_string(),
        test_code: None,
        entry_point: None,
        entry_class: None,
        execution_commands: Vec::new(),
        execution_timeouts_secs: Vec::new(),
        source_extension: None,
    }]
}

fn init_humaneval(language: &str) -> Result<Vec<BenchCaseFixture>> {
    require_fixture_language("humaneval", language, "python")?;
    Ok(vec![BenchCaseFixture {
        case_id: "humaneval-sample-001".to_string(),
        prompt: "def add(a, b):\n    \"\"\"Return the sum of two integers.\"\"\"\n".to_string(),
        starter_code: None,
        reference: "    return a + b\n".to_string(),
        language: "python".to_string(),
        test_code: Some(
            "def check(candidate):\n    assert candidate(1, 2) == 3\n    assert candidate(-5, 8) == 3\n    assert candidate(0, 0) == 0\n".to_string(),
        ),
        entry_point: Some("add".to_string()),
        entry_class: None,
        execution_commands: Vec::new(),
        execution_timeouts_secs: Vec::new(),
        source_extension: None,
    }])
}

fn init_mbpp(language: &str) -> Result<Vec<BenchCaseFixture>> {
    require_fixture_language("mbpp", language, "python")?;
    Ok(vec![BenchCaseFixture {
        case_id: "mbpp-sample-001".to_string(),
        prompt: "Return True if a string is a palindrome.".to_string(),
        starter_code: None,
        reference: "def is_palindrome(s):\n    return s == s[::-1]".to_string(),
        language: "python".to_string(),
        test_code: build_mbpp_test_code(
            "",
            &[
                "assert is_palindrome('level') is True".to_string(),
                "assert is_palindrome('abc') is False".to_string(),
                "assert is_palindrome('a') is True".to_string(),
            ],
            &[],
        ),
        entry_point: None,
        entry_class: None,
        execution_commands: Vec::new(),
        execution_timeouts_secs: Vec::new(),
        source_extension: None,
    }])
}

fn build_mbpp_test_code(
    test_setup_code: &str,
    test_list: &[String],
    challenge_test_list: &[String],
) -> Option<String> {
    let mut lines = Vec::new();
    let setup = test_setup_code.trim();
    if !setup.is_empty() {
        lines.push(setup.to_string());
    }
    lines.extend(
        test_list
            .iter()
            .chain(challenge_test_list.iter())
            .map(|test| test.trim().to_string())
            .filter(|test| !test.is_empty()),
    );
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn canonical_humanevalpack_language(language: &str) -> Result<(&'static str, &'static str)> {
    match language {
        "cpp" | "CPP" => Ok(("cpp", "cpp")),
        "go" | "Go" => Ok(("go", "go")),
        "java" | "Java" => Ok(("java", "java")),
        "javascript" | "JavaScript" | "js" => Ok(("javascript", "js")),
        "python" | "Python" => Ok(("python", "python")),
        "rust" | "Rust" => Ok(("rust", "rust")),
        other => bail!("unsupported HumanEvalPack language '{other}'"),
    }
}

fn fetch_humanevalpack_records(
    config: &str,
    force_download: bool,
) -> Result<Vec<HumanevalPackRecord>> {
    let mut records = Vec::new();
    let mut offset = 0usize;
    let page_size = 100usize;
    loop {
        let url = format!(
            "{HUMANEVALPACK_ROWS_URL}?dataset=bigcode%2Fhumanevalpack&config={config}&split=test&offset={offset}&length={page_size}"
        );
        let cache_name = format!("humanevalpack-{config}-{offset}.json");
        let bytes = download_url_bytes_cached(&url, &cache_name, force_download)?;
        let response: DatasetRowsResponse<HumanevalPackRecord> = serde_json::from_slice(&bytes)
            .with_context(|| format!("parse HumanEvalPack rows response for config '{config}'"))?;
        let fetched = response.rows.len();
        records.extend(response.rows.into_iter().map(|row| row.row));
        if fetched < page_size {
            break;
        }
        offset += page_size;
    }
    Ok(records)
}

fn humanevalpack_fixture_from_record(
    language: &str,
    record: HumanevalPackRecord,
) -> Result<BenchCaseFixture> {
    let test_code =
        build_humanevalpack_test_code(language, &record.import, &record.test_setup, &record.test)?;
    let (commands, extension) = humanevalpack_commands_for_language(language);
    let timeout_count = commands.len();
    Ok(BenchCaseFixture {
        case_id: record.task_id,
        prompt: record.instruction,
        starter_code: Some(if record.prompt.trim().is_empty() {
            record.declaration
        } else {
            record.prompt
        }),
        reference: record.canonical_solution,
        language: language.to_string(),
        test_code: Some(test_code),
        entry_point: Some(record.entry_point),
        entry_class: humanevalpack_entry_class_for_language(language),
        execution_commands: commands,
        execution_timeouts_secs: vec![10; timeout_count],
        source_extension: Some(extension.to_string()),
    })
}

fn humanevalpack_entry_class_for_language(language: &str) -> Option<String> {
    if language == "java" {
        Some("Solution".to_string())
    } else {
        None
    }
}

fn humanevalpack_commands_for_language(language: &str) -> (Vec<Vec<String>>, &'static str) {
    match language {
        "cpp" => (
            vec![
                vec![
                    "g++".to_string(),
                    "__FILENAME__".to_string(),
                    "-o".to_string(),
                    "./__FILENAME__.exe".to_string(),
                ],
                vec!["./__FILENAME__.exe".to_string()],
            ],
            "cpp",
        ),
        "go" => (
            vec![vec!["go".to_string(), "test".to_string(), ".".to_string()]],
            "go",
        ),
        "java" => (
            vec![
                vec!["javac".to_string(), "__FILENAME__".to_string()],
                vec!["java".to_string(), "Main".to_string()],
            ],
            "java",
        ),
        "javascript" => (
            vec![vec!["node".to_string(), "__FILENAME__".to_string()]],
            "js",
        ),
        "python" => (Vec::new(), "py"),
        "rust" => (
            vec![
                vec![
                    "rustc".to_string(),
                    "--test".to_string(),
                    "__FILENAME__".to_string(),
                    "-o".to_string(),
                    "./__FILENAME__.exe".to_string(),
                ],
                vec!["./__FILENAME__.exe".to_string()],
            ],
            "rs",
        ),
        _ => (Vec::new(), "txt"),
    }
}

fn build_humanevalpack_test_code(
    language: &str,
    import_block: &str,
    test_setup: &str,
    test: &str,
) -> Result<String> {
    if language == "python" {
        let mut sections = Vec::new();
        if !import_block.trim().is_empty() {
            sections.push(import_block.trim().to_string());
        }
        if !test_setup.trim().is_empty() {
            sections.push(test_setup.trim().to_string());
        }
        if !test.trim().is_empty() {
            sections.push(test.trim().to_string());
        }
        if sections.is_empty() {
            bail!("HumanEvalPack test payload was empty for language '{language}'");
        }
        return Ok(sections.join("\n\n"));
    }

    let mut sections = Vec::new();
    if !import_block.trim().is_empty() {
        sections.push(import_block.trim().to_string());
    }
    if !test_setup.trim().is_empty() {
        sections.push(match language {
            "go" => rewrite_humanevalpack_go_test_setup(test_setup),
            _ => test_setup.trim().to_string(),
        });
    }
    let rendered_test = match language {
        "go" => rewrite_humanevalpack_go_test(test),
        _ => test.trim().to_string(),
    };
    sections.push("PLACEHOLDER_CODE_BODY".to_string());
    if !rendered_test.is_empty() {
        sections.push(rendered_test);
    }
    if sections.is_empty() {
        bail!("HumanEvalPack test payload was empty for language '{language}'");
    }
    Ok(sections.join("\n\n"))
}

fn rewrite_humanevalpack_go_test_setup(test_setup: &str) -> String {
    test_setup
        .replace("\"github.com/stretchr/testify/assert\"\n", "")
        .replace(
            "import (\n    \"testing\"\n)\n",
            "import (\n    \"reflect\"\n    \"testing\"\n)\n\nfunc assertEqual(t *testing.T, expected any, actual any) {\n    if !reflect.DeepEqual(expected, actual) {\n        t.Fatalf(\"expected %#v, got %#v\", expected, actual)\n    }\n}\n",
        )
}

fn rewrite_humanevalpack_go_test(test: &str) -> String {
    test.replace("assert := assert.New(t)\n", "")
        .replace("assert := assert.New(t)\r\n", "")
        .replace("assert.Equal(", "assertEqual(t, ")
}

fn canonical_mbpp_language(language: &str) -> Result<&'static str> {
    match language {
        "python" | "Python" => Ok("python"),
        "cpp" | "C++" => Ok("cpp"),
        "csharp" | "CSharp" => Ok("csharp"),
        "dart" | "Dart" => Ok("dart"),
        "go" | "Go" => Ok("go"),
        "haskell" | "Haskell" => Ok("haskell"),
        "java" | "Java" => Ok("java"),
        "javascript" | "Javascript" => Ok("javascript"),
        "julia" | "Julia" => Ok("julia"),
        "kotlin" | "Kotlin" => Ok("kotlin"),
        "lua" | "Lua" => Ok("lua"),
        "php" | "PHP" => Ok("php"),
        "r" | "R" => Ok("r"),
        "rust" | "Rust" => Ok("rust"),
        "scala" | "Scala" => Ok("scala"),
        "typescript" | "TypeScript" => Ok("typescript"),
        other => bail!("unsupported BC-MBPP language '{other}'"),
    }
}

fn bc_mbpp_fixture_from_record(record: BcMbppFullRecord) -> Result<BenchCaseFixture> {
    let language = canonical_mbpp_language(&record.language)?.to_string();
    Ok(BenchCaseFixture {
        case_id: format!("mbpp-{}", record.qid),
        prompt: if record.signature_with_docstring.trim().is_empty() {
            record.text
        } else {
            record.signature_with_docstring
        },
        starter_code: None,
        reference: if language == "python" {
            record.solution_python
        } else {
            String::new()
        },
        language,
        test_code: Some(record.test_code),
        entry_point: (!record.entry_fn_name.trim().is_empty()).then_some(record.entry_fn_name),
        entry_class: (!record.entry_cls_name.trim().is_empty()).then_some(record.entry_cls_name),
        execution_commands: record.commands,
        execution_timeouts_secs: record.timeouts,
        source_extension: Some(record.extension),
    })
}

fn validate_case_fixtures(
    suite_id: &str,
    manifest: &BenchDataManifest,
    cases: &[BenchCaseFixture],
) -> Result<()> {
    if suite_id == "mbpp"
        && cases.iter().any(|case| {
            case.test_code
                .as_deref()
                .is_none_or(|test_code| test_code.trim().is_empty())
        })
    {
        let rebuild_hint = if manifest
            .files
            .iter()
            .any(|file| file.relative_path == "dataset/raw/mbpp.jsonl")
        {
            "/bench init mbpp --full --force-download"
        } else {
            "/bench init mbpp --force-download"
        };
        bail!(
            "Benchmark data for 'mbpp' is stale and missing bundled tests. Rebuild it with `{rebuild_hint}`."
        );
    }
    Ok(())
}

#[cfg(test)]
fn humaneval_fixture_from_record(record: HumanEvalFullRecord) -> BenchCaseFixture {
    BenchCaseFixture {
        case_id: record.task_id,
        prompt: record.prompt,
        starter_code: None,
        reference: record.canonical_solution,
        language: "python".to_string(),
        test_code: Some(record.test),
        entry_point: Some(record.entry_point),
        entry_class: None,
        execution_commands: Vec::new(),
        execution_timeouts_secs: Vec::new(),
        source_extension: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BcMbppFullRecord, BenchCaseFixture, BenchDataManifest, BenchDataSource,
        HumanEvalFullRecord, MANIFEST_VERSION, bc_mbpp_fixture_from_record, build_mbpp_test_code,
        canonical_mbpp_language, humaneval_fixture_from_record, validate_case_fixtures,
    };

    #[test]
    fn test_humaneval_full_record_preserves_hidden_tests() {
        let fixture = humaneval_fixture_from_record(HumanEvalFullRecord {
            task_id: "HumanEval/0".to_string(),
            prompt: "def add(a, b):\n".to_string(),
            canonical_solution: "    return a + b\n".to_string(),
            test: "def check(candidate):\n    assert candidate(1, 2) == 3\n".to_string(),
            entry_point: "add".to_string(),
        });

        assert_eq!(fixture.case_id, "HumanEval/0");
        assert_eq!(fixture.entry_point.as_deref(), Some("add"));
        assert!(
            fixture
                .test_code
                .as_deref()
                .is_some_and(|test| test.contains("check(candidate)"))
        );
    }

    #[test]
    fn test_build_mbpp_test_code_preserves_setup_and_assertions() {
        let payload = build_mbpp_test_code(
            "import math",
            &["assert solve(1) == 2".to_string()],
            &["assert solve(2) == 3".to_string()],
        )
        .expect("mbpp test payload");

        assert!(payload.contains("import math"));
        assert!(payload.contains("assert solve(1) == 2"));
        assert!(payload.contains("assert solve(2) == 3"));
    }

    #[test]
    fn test_bc_mbpp_language_aliases_resolve_to_cli_slugs() {
        assert_eq!(canonical_mbpp_language("Rust").expect("rust slug"), "rust");
        assert_eq!(canonical_mbpp_language("C++").expect("cpp slug"), "cpp");
        assert_eq!(
            canonical_mbpp_language("TypeScript").expect("typescript slug"),
            "typescript"
        );
    }

    #[test]
    fn test_bc_mbpp_full_record_builds_native_fixture() {
        let fixture = bc_mbpp_fixture_from_record(BcMbppFullRecord {
            qid: "7".to_string(),
            language: "Rust".to_string(),
            extension: "rs".to_string(),
            commands: vec![
                vec!["rustc".to_string(), "__FILENAME__".to_string()],
                vec!["./__FILENAME__.exe".to_string()],
            ],
            timeouts: vec![10, 10],
            entry_cls_name: "Solution".to_string(),
            entry_fn_name: "answer".to_string(),
            signature_with_docstring: "pub fn answer() -> i32 {".to_string(),
            text: "Return the answer.".to_string(),
            test_code: "PLACEHOLDER_CODE_BODY\nfn main() { println!(\"TEST-0...PASSED\"); }"
                .to_string(),
            solution_python: "def answer():\n    return 42".to_string(),
        })
        .expect("native bc-mbpp fixture");

        assert_eq!(fixture.case_id, "mbpp-7");
        assert_eq!(fixture.language, "rust");
        assert_eq!(fixture.entry_point.as_deref(), Some("answer"));
        assert_eq!(fixture.source_extension.as_deref(), Some("rs"));
        assert_eq!(fixture.execution_commands.len(), 2);
        assert_eq!(fixture.reference, "");
        assert!(
            fixture
                .test_code
                .as_deref()
                .is_some_and(|tests| tests.contains("PLACEHOLDER_CODE_BODY"))
        );
    }

    #[test]
    fn test_validate_case_fixtures_rejects_stale_mbpp_cases() {
        let manifest = BenchDataManifest {
            bench_name: "mbpp".to_string(),
            display_name: "MBPP".to_string(),
            language: "python".to_string(),
            revision: "BC-MBPP-1.0".to_string(),
            sources: vec![BenchDataSource {
                kind: "dataset".to_string(),
                url: "https://example.invalid/mbpp".to_string(),
            }],
            initialized_at_utc: "2026-05-02T00:00:00Z".to_string(),
            dataset_dir: "dataset".to_string(),
            case_file: "dataset/cases.jsonl".to_string(),
            case_count: 1,
            status: "ready".to_string(),
            manifest_version: MANIFEST_VERSION,
            files: vec![],
        };
        let cases = vec![BenchCaseFixture {
            case_id: "mbpp-1".to_string(),
            prompt: "Return True if a string is a palindrome.".to_string(),
            starter_code: None,
            reference: "def is_palindrome(s):\n    return s == s[::-1]".to_string(),
            language: "python".to_string(),
            test_code: None,
            entry_point: None,
            entry_class: None,
            execution_commands: Vec::new(),
            execution_timeouts_secs: Vec::new(),
            source_extension: None,
        }];

        let error = validate_case_fixtures("mbpp", &manifest, &cases)
            .expect_err("stale mbpp fixtures should be rejected");
        assert!(
            error
                .to_string()
                .contains("stale and missing bundled tests")
        );
    }
}

fn init_apps(language: &str) -> Result<Vec<BenchCaseFixture>> {
    require_fixture_language("apps", language, "python")?;
    Ok(fixture(
        "apps-sample-001",
        "Read two integers and print their sum.",
        "a, b = map(int, input().split())\nprint(a + b)",
        "python",
    ))
}

fn init_ds1000(language: &str) -> Result<Vec<BenchCaseFixture>> {
    require_fixture_language("ds1000", language, "python")?;
    Ok(fixture(
        "ds1000-sample-001",
        "Filter a pandas DataFrame to rows where column x > 10.",
        "df[df['x'] > 10]",
        "python",
    ))
}

fn init_multipl_e(language: &str) -> Result<Vec<BenchCaseFixture>> {
    match language {
        "python" => Ok(fixture(
            "multipl-e-python-sample-001",
            "Return the maximum element in a list.",
            "def max_in_list(items):\n    return max(items)",
            "python",
        )),
        "rust" => Ok(fixture(
            "multipl-e-rust-sample-001",
            "Return the maximum element in a list.",
            "fn max_in_list(items: &[i32]) -> i32 {\n    *items.iter().max().unwrap()\n}",
            "rust",
        )),
        other => bail!(
            "benchmark suite 'multipl-e' does not provide init fixtures for language '{}'; supported languages: python, rust",
            other
        ),
    }
}

fn init_repobench(language: &str) -> Result<Vec<BenchCaseFixture>> {
    require_fixture_language("repobench", language, "python")?;
    Ok(fixture(
        "repobench-sample-001",
        "Complete the masked repository function call.",
        "repository_completion()",
        "python",
    ))
}

fn init_crosscodeeval(language: &str) -> Result<Vec<BenchCaseFixture>> {
    require_fixture_language("crosscodeeval", language, "python")?;
    Ok(fixture(
        "crosscodeeval-sample-001",
        "Use cross-file context to finish the helper implementation.",
        "return helper(value)",
        "python",
    ))
}

fn init_swebench_lite(language: &str) -> Result<Vec<BenchCaseFixture>> {
    require_fixture_language("swebench-lite", language, "diff")?;
    Ok(fixture(
        "swebench-lite-sample-001",
        "Patch the repository so the failing test now passes.",
        "diff --git a/module.py b/module.py",
        "diff",
    ))
}

fn init_swebench_verified(language: &str) -> Result<Vec<BenchCaseFixture>> {
    require_fixture_language("swebench-verified", language, "diff")?;
    Ok(fixture(
        "swebench-verified-sample-001",
        "Apply a repository patch that resolves the reported issue.",
        "diff --git a/service.py b/service.py",
        "diff",
    ))
}

fn init_livecodebench(language: &str) -> Result<Vec<BenchCaseFixture>> {
    require_fixture_language("livecodebench", language, "python")?;
    Ok(fixture(
        "livecodebench-sample-001",
        "Implement the required function for this timed coding task.",
        "def solve():\n    pass",
        "python",
    ))
}

fn init_bigcodebench(language: &str) -> Result<Vec<BenchCaseFixture>> {
    require_fixture_language("bigcodebench", language, "python")?;
    Ok(fixture(
        "bigcodebench-sample-001",
        "Write a complete solution for the described practical coding task.",
        "def solution():\n    raise NotImplementedError",
        "python",
    ))
}
