//! Benchmark data root and manifest helpers.

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};

use crate::command::{BenchInitMode, BenchInitTarget};
use crate::registry::{BenchSuiteDef, all_suites, find_suite};

const MANIFEST_FILE: &str = "manifest.json";
const DATASET_DIR: &str = "dataset";
const CASES_FILE: &str = "cases.jsonl";
const FULL_HUMANEVAL_URL: &str =
    "https://raw.githubusercontent.com/openai/human-eval/master/data/HumanEval.jsonl.gz";
const FULL_MBPP_URL: &str =
    "https://raw.githubusercontent.com/google-research/google-research/master/mbpp/mbpp.jsonl";
const FULL_INIT_SUPPORTED_SUITES: &[&str] = &["humaneval", "mbpp"];

/// One initialized benchmark case fixture.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BenchCaseFixture {
    /// Case identifier.
    pub case_id: String,
    /// Prompt or task text.
    pub prompt: String,
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
}

/// On-disk manifest for initialized benchmark data.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BenchDataManifest {
    /// Canonical suite ID.
    pub bench_name: String,
    /// Display name.
    pub display_name: String,
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
    force_download: bool,
    verify_only: bool,
) -> Result<Vec<BenchInitOutcome>> {
    match target {
        BenchInitTarget::Suite(suite_id) => Ok(vec![init_suite_mode(
            project_root,
            suite_id,
            mode,
            force_download,
            verify_only,
        )?]),
        BenchInitTarget::All => {
            ensure_full_init_supported_for_target(target, mode)?;
            all_suites()
                .iter()
                .map(|suite| {
                    init_suite_mode(project_root, suite.id, mode, force_download, verify_only)
                })
                .collect()
        }
        BenchInitTarget::Full => {
            ensure_full_init_supported_for_target(target, BenchInitMode::Full)?;
            all_suites()
                .iter()
                .map(|suite| {
                    init_suite_mode(
                        project_root,
                        suite.id,
                        BenchInitMode::Full,
                        force_download,
                        verify_only,
                    )
                })
                .collect()
        }
    }
}

/// Build the initialized dataset root for one suite.
#[must_use]
pub fn bench_data_root(project_root: &Path, suite_id: &str) -> PathBuf {
    project_root.join("benches").join("data").join(suite_id)
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
    let data_root = bench_data_root(project_root, suite_id);
    if !data_root.is_dir() {
        bail!(
            "Benchmark data for '{}' is not initialized. Run `/bench init {}` first.",
            suite_id,
            suite_id
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
    if manifest.manifest_version != 2 {
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
    init_suite_mode(
        project_root,
        suite_id,
        BenchInitMode::Sample,
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
    force_download: bool,
    verify_only: bool,
) -> Result<BenchInitOutcome> {
    let suite =
        *find_suite(suite_id).ok_or_else(|| anyhow!("unknown benchmark suite: {suite_id}"))?;
    let data_root = bench_data_root(project_root, suite_id);
    let data_parent = project_root.join("benches").join("data");

    if verify_only {
        let manifest = verify_suite(project_root, suite_id)?;
        return Ok(BenchInitOutcome {
            suite,
            data_root,
            manifest,
            created: false,
            verified_only: true,
            mode,
        });
    }

    if !force_download && let Ok(manifest) = verify_suite(project_root, suite_id) {
        return Ok(BenchInitOutcome {
            suite,
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
        ".{}.tmp-{}",
        suite_id,
        uuid::Uuid::new_v4().simple()
    ));
    let build_result = (|| -> Result<()> {
        std::fs::create_dir_all(&staging_root)
            .with_context(|| format!("create staging benchmark root {}", staging_root.display()))?;
        let dataset_dir = staging_root.join(DATASET_DIR);
        std::fs::create_dir_all(&dataset_dir)
            .with_context(|| format!("create dataset directory {}", dataset_dir.display()))?;

        let prepared = prepare_suite_dataset(&staging_root, suite_id, mode)?;
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
            manifest_version: 2,
            files,
        };
        write_manifest(&staging_root, &manifest)?;
        Ok(())
    })();

    if let Err(error) = build_result {
        let _ = std::fs::remove_dir_all(&staging_root);
        return Err(error);
    }

    std::fs::rename(&staging_root, &data_root).with_context(|| {
        format!(
            "finalize benchmark data root {} -> {}",
            staging_root.display(),
            data_root.display()
        )
    })?;
    let manifest = verify_suite(project_root, suite_id)?;

    Ok(BenchInitOutcome {
        suite,
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
    mode: BenchInitMode,
) -> Result<PreparedSuiteDataset> {
    match mode {
        BenchInitMode::Sample => Ok(PreparedSuiteDataset {
            fixtures: suite_fixtures(suite_id)?,
            tracked_paths: Vec::new(),
        }),
        BenchInitMode::Full => prepare_full_suite_dataset(staging_root, suite_id),
    }
}

fn suite_fixtures(suite_id: &str) -> Result<Vec<BenchCaseFixture>> {
    match suite_id {
        "humaneval" => init_humaneval(),
        "mbpp" => init_mbpp(),
        "apps" => init_apps(),
        "ds1000" => init_ds1000(),
        "multipl-e" => init_multipl_e(),
        "repobench" => init_repobench(),
        "crosscodeeval" => init_crosscodeeval(),
        "swebench-lite" => init_swebench_lite(),
        "swebench-verified" => init_swebench_verified(),
        "livecodebench" => init_livecodebench(),
        "bigcodebench" => init_bigcodebench(),
        other => bail!("unknown benchmark suite: {other}"),
    }
}

fn prepare_full_suite_dataset(staging_root: &Path, suite_id: &str) -> Result<PreparedSuiteDataset> {
    match suite_id {
        "humaneval" => init_humaneval_full(staging_root),
        "mbpp" => init_mbpp_full(staging_root),
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

#[derive(Debug, Deserialize)]
struct HumanEvalFullRecord {
    task_id: String,
    prompt: String,
    canonical_solution: String,
    test: String,
    entry_point: String,
}

#[derive(Debug, Deserialize)]
struct MbppFullRecord {
    task_id: u64,
    text: String,
    code: String,
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

fn decompress_gzip(bytes: &[u8], label: &str) -> Result<String> {
    let mut decoder = GzDecoder::new(bytes);
    let mut output = String::new();
    decoder
        .read_to_string(&mut output)
        .with_context(|| format!("decompress gzip benchmark dataset {label}"))?;
    Ok(output)
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

fn parse_jsonl_records<T>(raw: &str, label: &str) -> Result<Vec<T>>
where
    T: for<'de> Deserialize<'de>,
{
    raw.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).with_context(|| format!("parse {label} jsonl line")))
        .collect()
}

fn init_humaneval_full(staging_root: &Path) -> Result<PreparedSuiteDataset> {
    let relative_path = Path::new(DATASET_DIR)
        .join("raw")
        .join("HumanEval.jsonl.gz");
    let bytes = download_url_bytes(FULL_HUMANEVAL_URL)?;
    let tracked = write_downloaded_file(staging_root, &relative_path, &bytes)?;
    let raw = decompress_gzip(&bytes, "HumanEval")?;
    let records: Vec<HumanEvalFullRecord> = parse_jsonl_records(&raw, "HumanEval")?;
    let fixtures = records
        .into_iter()
        .map(humaneval_fixture_from_record)
        .collect();
    Ok(PreparedSuiteDataset {
        fixtures,
        tracked_paths: vec![tracked],
    })
}

fn init_mbpp_full(staging_root: &Path) -> Result<PreparedSuiteDataset> {
    let relative_path = Path::new(DATASET_DIR).join("raw").join("mbpp.jsonl");
    let bytes = download_url_bytes(FULL_MBPP_URL)?;
    let tracked = write_downloaded_file(staging_root, &relative_path, &bytes)?;
    let raw = String::from_utf8(bytes).context("decode MBPP dataset as UTF-8")?;
    let records: Vec<MbppFullRecord> = parse_jsonl_records(&raw, "MBPP")?;
    let fixtures = records
        .into_iter()
        .map(|record| BenchCaseFixture {
            case_id: format!("mbpp-{}", record.task_id),
            prompt: record.text,
            reference: record.code,
            language: "python".to_string(),
            test_code: None,
            entry_point: None,
        })
        .collect();
    Ok(PreparedSuiteDataset {
        fixtures,
        tracked_paths: vec![tracked],
    })
}

fn fixture(case_id: &str, prompt: &str, reference: &str, language: &str) -> Vec<BenchCaseFixture> {
    vec![BenchCaseFixture {
        case_id: case_id.to_string(),
        prompt: prompt.to_string(),
        reference: reference.to_string(),
        language: language.to_string(),
        test_code: None,
        entry_point: None,
    }]
}

fn init_humaneval() -> Result<Vec<BenchCaseFixture>> {
    Ok(vec![BenchCaseFixture {
        case_id: "humaneval-sample-001".to_string(),
        prompt: "def add(a, b):\n    \"\"\"Return the sum of two integers.\"\"\"\n".to_string(),
        reference: "    return a + b\n".to_string(),
        language: "python".to_string(),
        test_code: Some(
            "def check(candidate):\n    assert candidate(1, 2) == 3\n    assert candidate(-5, 8) == 3\n    assert candidate(0, 0) == 0\n".to_string(),
        ),
        entry_point: Some("add".to_string()),
    }])
}

fn init_mbpp() -> Result<Vec<BenchCaseFixture>> {
    Ok(fixture(
        "mbpp-sample-001",
        "Return True if a string is a palindrome.",
        "def is_palindrome(s):\n    return s == s[::-1]",
        "python",
    ))
}

fn humaneval_fixture_from_record(record: HumanEvalFullRecord) -> BenchCaseFixture {
    BenchCaseFixture {
        case_id: record.task_id,
        prompt: record.prompt,
        reference: record.canonical_solution,
        language: "python".to_string(),
        test_code: Some(record.test),
        entry_point: Some(record.entry_point),
    }
}

#[cfg(test)]
mod tests {
    use super::{HumanEvalFullRecord, humaneval_fixture_from_record};

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
}

fn init_apps() -> Result<Vec<BenchCaseFixture>> {
    Ok(fixture(
        "apps-sample-001",
        "Read two integers and print their sum.",
        "a, b = map(int, input().split())\nprint(a + b)",
        "python",
    ))
}

fn init_ds1000() -> Result<Vec<BenchCaseFixture>> {
    Ok(fixture(
        "ds1000-sample-001",
        "Filter a pandas DataFrame to rows where column x > 10.",
        "df[df['x'] > 10]",
        "python",
    ))
}

fn init_multipl_e() -> Result<Vec<BenchCaseFixture>> {
    Ok(fixture(
        "multipl-e-sample-001",
        "Return the maximum element in a list.",
        "def max_in_list(items):\n    return max(items)",
        "python",
    ))
}

fn init_repobench() -> Result<Vec<BenchCaseFixture>> {
    Ok(fixture(
        "repobench-sample-001",
        "Complete the masked repository function call.",
        "repository_completion()",
        "python",
    ))
}

fn init_crosscodeeval() -> Result<Vec<BenchCaseFixture>> {
    Ok(fixture(
        "crosscodeeval-sample-001",
        "Use cross-file context to finish the helper implementation.",
        "return helper(value)",
        "python",
    ))
}

fn init_swebench_lite() -> Result<Vec<BenchCaseFixture>> {
    Ok(fixture(
        "swebench-lite-sample-001",
        "Patch the repository so the failing test now passes.",
        "diff --git a/module.py b/module.py",
        "diff",
    ))
}

fn init_swebench_verified() -> Result<Vec<BenchCaseFixture>> {
    Ok(fixture(
        "swebench-verified-sample-001",
        "Apply a repository patch that resolves the reported issue.",
        "diff --git a/service.py b/service.py",
        "diff",
    ))
}

fn init_livecodebench() -> Result<Vec<BenchCaseFixture>> {
    Ok(fixture(
        "livecodebench-sample-001",
        "Implement the required function for this timed coding task.",
        "def solve():\n    pass",
        "python",
    ))
}

fn init_bigcodebench() -> Result<Vec<BenchCaseFixture>> {
    Ok(fixture(
        "bigcodebench-sample-001",
        "Write a complete solution for the described practical coding task.",
        "def solution():\n    raise NotImplementedError",
        "python",
    ))
}
