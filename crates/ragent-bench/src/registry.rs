//! Benchmark suite and profile registry.

use anyhow::{Result, anyhow};

use crate::command::BenchTarget;

/// Static benchmark suite metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BenchSuiteDef {
    /// Canonical suite ID.
    pub id: &'static str,
    /// Human-readable name.
    pub display_name: &'static str,
    /// Dataset or evaluator revision string.
    pub revision: &'static str,
    /// Canonical data source URLs.
    pub sources: &'static [&'static str],
    /// Short suite description.
    pub description: &'static str,
    /// Available language tags for the current suite fixtures.
    pub languages: &'static [&'static str],
    /// Whether this suite is expensive enough to require `--yes`.
    pub expensive: bool,
}

/// Static benchmark profile metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BenchProfileDef {
    /// Profile ID.
    pub id: &'static str,
    /// Profile description.
    pub description: &'static str,
    /// Member suite IDs.
    pub suites: &'static [&'static str],
    /// Whether the profile should require `--yes`.
    pub expensive: bool,
}

const SUITES: &[BenchSuiteDef] = &[
    BenchSuiteDef {
        id: "humaneval",
        display_name: "HumanEval",
        revision: "HumanEval-1.2",
        sources: &["https://github.com/openai/human-eval"],
        description: "Function-level Python code generation benchmark.",
        languages: &["python"],
        expensive: false,
    },
    BenchSuiteDef {
        id: "mbpp",
        display_name: "MBPP",
        revision: "MBPP-1.0",
        sources: &["https://huggingface.co/datasets/google-research-datasets/mbpp"],
        description: "Mostly Basic Programming Problems for Python generation.",
        languages: &["python"],
        expensive: false,
    },
    BenchSuiteDef {
        id: "apps",
        display_name: "APPS",
        revision: "APPS-1.0",
        sources: &["https://github.com/hendrycks/apps"],
        description: "Competitive-programming style code generation benchmark.",
        languages: &["python"],
        expensive: true,
    },
    BenchSuiteDef {
        id: "ds1000",
        display_name: "DS-1000",
        revision: "DS1000-1.0",
        sources: &["https://ds1000-code-gen.github.io/"],
        description: "Data-science oriented Python code generation tasks.",
        languages: &["python"],
        expensive: false,
    },
    BenchSuiteDef {
        id: "multipl-e",
        display_name: "MultiPL-E",
        revision: "MultiPL-E-1.0",
        sources: &["https://github.com/nuprl/MultiPL-E"],
        description: "Translated multi-language code generation benchmark.",
        languages: &["python"],
        expensive: true,
    },
    BenchSuiteDef {
        id: "repobench",
        display_name: "RepoBench",
        revision: "RepoBench-1.0",
        sources: &["https://github.com/Leolty/repobench"],
        description: "Repository-level masked code completion benchmark.",
        languages: &["python"],
        expensive: false,
    },
    BenchSuiteDef {
        id: "crosscodeeval",
        display_name: "CrossCodeEval",
        revision: "CrossCodeEval-1.0",
        sources: &["https://github.com/amazon-science/cceval"],
        description: "Cross-file completion benchmark with retrieval context.",
        languages: &["python"],
        expensive: false,
    },
    BenchSuiteDef {
        id: "swebench-lite",
        display_name: "SWE-bench Lite",
        revision: "SWE-bench-Lite-1.0",
        sources: &[
            "https://www.swebench.com/SWE-bench/",
            "https://github.com/SWE-bench/SWE-bench/blob/main/docs/guides/evaluation.md",
        ],
        description: "Repository patch-generation benchmark using the Lite subset.",
        languages: &["diff"],
        expensive: true,
    },
    BenchSuiteDef {
        id: "swebench-verified",
        display_name: "SWE-bench Verified",
        revision: "SWE-bench-Verified-1.0",
        sources: &[
            "https://www.swebench.com/SWE-bench/",
            "https://github.com/SWE-bench/SWE-bench/blob/main/docs/guides/evaluation.md",
        ],
        description: "Repository patch-generation benchmark using Verified instances.",
        languages: &["diff"],
        expensive: true,
    },
    BenchSuiteDef {
        id: "livecodebench",
        display_name: "LiveCodeBench",
        revision: "LiveCodeBench-1.0",
        sources: &["https://github.com/LiveCodeBench/LiveCodeBench"],
        description: "Contamination-aware coding evaluation across benchmark release windows.",
        languages: &["python"],
        expensive: true,
    },
    BenchSuiteDef {
        id: "bigcodebench",
        display_name: "BigCodeBench",
        revision: "BigCodeBench-1.0",
        sources: &["https://github.com/bigcode-project/bigcodebench"],
        description: "Practical challenging code generation benchmark with pass@k scoring.",
        languages: &["python"],
        expensive: true,
    },
];

const PROFILES: &[BenchProfileDef] = &[
    BenchProfileDef {
        id: "quick",
        description: "Fast MVP smoke profile.",
        suites: &["humaneval", "mbpp"],
        expensive: false,
    },
    BenchProfileDef {
        id: "standard",
        description: "Local MVP profile covering all Phase A suites.",
        suites: &["humaneval", "mbpp", "ds1000", "repobench", "crosscodeeval"],
        expensive: false,
    },
    BenchProfileDef {
        id: "agentic",
        description: "Real-world or workflow-oriented benchmark profile.",
        suites: &["swebench-lite", "livecodebench"],
        expensive: true,
    },
];

/// Return all registered suites.
#[must_use]
pub fn all_suites() -> &'static [BenchSuiteDef] {
    SUITES
}

/// Return all registered profiles.
#[must_use]
pub fn all_profiles() -> &'static [BenchProfileDef] {
    PROFILES
}

/// Look up one suite by canonical ID.
#[must_use]
pub fn find_suite(id: &str) -> Option<&'static BenchSuiteDef> {
    SUITES.iter().find(|suite| suite.id == id)
}

/// Look up one profile by ID.
#[must_use]
pub fn find_profile(id: &str) -> Option<&'static BenchProfileDef> {
    PROFILES.iter().find(|profile| profile.id == id)
}

/// Expand a run target into the concrete suite list.
///
/// # Errors
///
/// Returns an error when the suite/profile is unknown.
pub fn expand_target(target: &BenchTarget) -> Result<Vec<&'static BenchSuiteDef>> {
    match target {
        BenchTarget::Suite(id) => find_suite(id)
            .map(|suite| vec![suite])
            .ok_or_else(|| anyhow!("unknown benchmark suite: {id}")),
        BenchTarget::Profile(id) => {
            let profile =
                find_profile(id).ok_or_else(|| anyhow!("unknown benchmark profile: {id}"))?;
            profile
                .suites
                .iter()
                .map(|suite_id| {
                    find_suite(suite_id).ok_or_else(|| {
                        anyhow!("profile '{}' references unknown suite '{}'", id, suite_id)
                    })
                })
                .collect()
        }
        BenchTarget::All => Ok(SUITES.iter().collect()),
    }
}

/// Return whether a target should require explicit confirmation.
#[must_use]
pub fn requires_confirmation(target: &BenchTarget) -> bool {
    match target {
        BenchTarget::All => true,
        BenchTarget::Profile(id) => find_profile(id).is_some_and(|profile| profile.expensive),
        BenchTarget::Suite(id) => find_suite(id).is_some_and(|suite| suite.expensive),
    }
}
