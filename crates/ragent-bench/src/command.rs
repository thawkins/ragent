//! `/bench` command parsing.

use anyhow::{Result, anyhow, bail};
use chrono::NaiveDate;

use crate::registry::find_profile;

/// A benchmark initialization target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BenchInitTarget {
    /// Initialize one benchmark suite.
    Suite(String),
    /// Initialize every registered suite using the selected init mode.
    All,
    /// Initialize every registered suite using full upstream dataset ingestion.
    Full,
}

/// Benchmark initialization mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchInitMode {
    /// Write local sample fixtures only.
    Sample,
    /// Download and normalize full upstream benchmark data.
    Full,
}

/// A benchmark run target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BenchTarget {
    /// Run a single benchmark suite by canonical ID.
    Suite(String),
    /// Run a named profile.
    Profile(String),
    /// Run every currently registered suite/profile member.
    All,
}

/// Options for `/bench run`.
#[derive(Debug, Clone, PartialEq)]
pub struct BenchRunOptions {
    /// Optional case limit.
    pub limit: Option<usize>,
    /// Number of samples per case.
    pub samples: usize,
    /// Optional benchmark subset selector.
    pub subset: Option<String>,
    /// Optional benchmark release/version selector.
    pub release: Option<String>,
    /// Optional benchmark scenario selector.
    pub scenario: Option<String>,
    /// Optional target language selector.
    pub language: Option<String>,
    /// Optional temperature override for model sampling.
    pub temperature: Option<f32>,
    /// Optional nucleus sampling override.
    pub top_p: Option<f32>,
    /// Optional max-token override for completions.
    pub max_tokens: Option<u32>,
    /// Force deterministic request settings for reproducibility.
    pub deterministic: bool,
    /// Optional inclusive lower date bound.
    pub since: Option<NaiveDate>,
    /// Optional inclusive upper date bound.
    pub until: Option<NaiveDate>,
    /// Resume a prior partial run.
    pub resume: bool,
    /// Skip execution/evaluation and generate outputs only.
    pub no_exec: bool,
    /// Explicit confirmation for expensive targets.
    pub yes: bool,
}

impl Default for BenchRunOptions {
    fn default() -> Self {
        Self {
            limit: None,
            samples: 1,
            subset: None,
            release: None,
            scenario: None,
            language: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            deterministic: false,
            since: None,
            until: None,
            resume: false,
            no_exec: false,
            yes: false,
        }
    }
}

/// Parsed `/bench` command.
#[derive(Debug, Clone, PartialEq)]
pub enum BenchCommand {
    /// Show command usage.
    Help,
    /// List available suites and profiles.
    List,
    /// Show current benchmark defaults.
    Show,
    /// Initialize benchmark data.
    Init {
        /// Initialization target.
        target: BenchInitTarget,
        /// Initialization mode.
        mode: BenchInitMode,
        /// Optional language selector.
        language: Option<String>,
        /// Force a full rebuild of the local data root.
        force_download: bool,
        /// Verify without mutating the dataset root.
        verify_only: bool,
    },
    /// Run a suite/profile/all target.
    Run {
        /// Run target.
        target: BenchTarget,
        /// Parsed run options.
        options: BenchRunOptions,
    },
    /// Show current or previous run status.
    Status,
    /// Show the latest workbook result paths.
    OpenLast,
    /// Cancel the active benchmark run.
    Cancel,
}

/// Parse the arguments passed after `/bench`.
///
/// # Errors
///
/// Returns an error when the subcommand or flag combination is invalid.
pub fn parse_bench_command(args: &str) -> Result<BenchCommand> {
    let parts: Vec<&str> = args.split_whitespace().collect();
    let Some(sub) = parts.first().copied() else {
        return Ok(BenchCommand::Help);
    };

    match sub {
        "help" => Ok(BenchCommand::Help),
        "list" => Ok(BenchCommand::List),
        "show" => Ok(BenchCommand::Show),
        "status" => Ok(BenchCommand::Status),
        "open" => parse_open(&parts[1..]),
        "cancel" => Ok(BenchCommand::Cancel),
        "init" => parse_init(&parts[1..]),
        "run" => parse_run(&parts[1..]),
        other => Err(anyhow!("unknown /bench subcommand: {other}")),
    }
}

fn parse_open(parts: &[&str]) -> Result<BenchCommand> {
    match parts {
        ["last"] => Ok(BenchCommand::OpenLast),
        [] => bail!("usage: /bench open last"),
        [other, ..] => bail!("unknown /bench open target: {other}"),
    }
}

fn parse_init(parts: &[&str]) -> Result<BenchCommand> {
    let Some(raw_target) = parts.first().copied() else {
        bail!(
            "usage: /bench init <suite-or-all-or-full> [--full] [--language LANG] [--force-download] [--verify-only]"
        );
    };

    let target = match raw_target {
        "all" => BenchInitTarget::All,
        "full" => BenchInitTarget::Full,
        other => BenchInitTarget::Suite(other.to_string()),
    };

    let mut mode = if matches!(target, BenchInitTarget::Full) {
        BenchInitMode::Full
    } else {
        BenchInitMode::Sample
    };
    let mut language = None;
    let mut force_download = false;
    let mut verify_only = false;
    let mut i = 1;
    while i < parts.len() {
        match parts[i] {
            "--full" => {
                mode = BenchInitMode::Full;
                i += 1;
            }
            "--language" => {
                language = Some(parse_string_flag(parts, &mut i, "--language")?);
            }
            "--force-download" => {
                force_download = true;
                i += 1;
            }
            "--verify-only" => {
                verify_only = true;
                i += 1;
            }
            other => bail!("unknown /bench init flag: {other}"),
        }
    }

    Ok(BenchCommand::Init {
        target,
        mode,
        language,
        force_download,
        verify_only,
    })
}

fn parse_run(parts: &[&str]) -> Result<BenchCommand> {
    let Some(raw_target) = parts.first().copied() else {
        bail!(
            "usage: /bench run <suite-or-profile> [--limit N|--cap N] [--samples K] [--subset NAME] [--release VERSION] [--scenario NAME] [--language LANG] [--temperature F] [--top-p F] [--max-tokens N] [--deterministic] [--since YYYY-MM-DD] [--until YYYY-MM-DD] [--resume] [--no-exec] [--yes]"
        );
    };

    let target = match raw_target {
        "all" => BenchTarget::All,
        other if find_profile(other).is_some() => BenchTarget::Profile(other.to_string()),
        other => BenchTarget::Suite(other.to_string()),
    };

    let mut options = BenchRunOptions::default();
    let mut i = 1;
    while i < parts.len() {
        match parts[i] {
            "--limit" | "--cap" => {
                let flag = parts[i];
                options.limit = Some(parse_usize_flag(parts, &mut i, flag)?);
            }
            "--samples" => {
                options.samples = parse_usize_flag(parts, &mut i, "--samples")?;
            }
            "--subset" => {
                options.subset = Some(parse_string_flag(parts, &mut i, "--subset")?);
            }
            "--release" => {
                options.release = Some(parse_string_flag(parts, &mut i, "--release")?);
            }
            "--scenario" => {
                options.scenario = Some(parse_string_flag(parts, &mut i, "--scenario")?);
            }
            "--language" => {
                options.language = Some(parse_string_flag(parts, &mut i, "--language")?);
            }
            "--temperature" => {
                options.temperature = Some(parse_f32_flag(parts, &mut i, "--temperature")?);
            }
            "--top-p" => {
                options.top_p = Some(parse_f32_flag(parts, &mut i, "--top-p")?);
            }
            "--max-tokens" => {
                options.max_tokens = Some(parse_u32_flag(parts, &mut i, "--max-tokens")?);
            }
            "--deterministic" => {
                options.deterministic = true;
                i += 1;
            }
            "--since" => {
                options.since = Some(parse_date_flag(parts, &mut i, "--since")?);
            }
            "--until" => {
                options.until = Some(parse_date_flag(parts, &mut i, "--until")?);
            }
            "--resume" => {
                options.resume = true;
                i += 1;
            }
            "--no-exec" => {
                options.no_exec = true;
                i += 1;
            }
            "--yes" => {
                options.yes = true;
                i += 1;
            }
            other => bail!("unknown /bench run flag: {other}"),
        }
    }

    Ok(BenchCommand::Run { target, options })
}

fn parse_string_flag(parts: &[&str], index: &mut usize, flag: &str) -> Result<String> {
    let Some(value) = parts.get(*index + 1) else {
        bail!("{flag} requires a value");
    };
    *index += 2;
    Ok((*value).to_string())
}

fn parse_date_flag(parts: &[&str], index: &mut usize, flag: &str) -> Result<NaiveDate> {
    let value = parse_string_flag(parts, index, flag)?;
    NaiveDate::parse_from_str(&value, "%Y-%m-%d")
        .map_err(|_| anyhow!("{flag} must be in YYYY-MM-DD format"))
}

fn parse_f32_flag(parts: &[&str], index: &mut usize, flag: &str) -> Result<f32> {
    let value = parse_string_flag(parts, index, flag)?;
    value
        .parse()
        .map_err(|_| anyhow!("{flag} requires a floating-point value"))
}

fn parse_u32_flag(parts: &[&str], index: &mut usize, flag: &str) -> Result<u32> {
    let value = parse_string_flag(parts, index, flag)?;
    value
        .parse()
        .map_err(|_| anyhow!("{flag} requires an integer value"))
}

fn parse_usize_flag(parts: &[&str], index: &mut usize, flag: &str) -> Result<usize> {
    let value = parse_string_flag(parts, index, flag)?;
    value
        .parse()
        .map_err(|_| anyhow!("{flag} requires an integer value"))
}
