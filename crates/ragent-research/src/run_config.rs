//! Runtime configuration for `/research` runs: output formats and depth presets.
//!
//! This module holds the small value types that translate CLI flags such as
//! `--format` and `--depth` into the engine's concrete [`EngineConfig`].

use crate::engine::EngineConfig;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Output artifact requested via `--format` (FR-012).
///
/// Supported artifacts:
///
/// - `report` — the default multi-section research report.
/// - `executive-summary` — one-page summary.
/// - `comparison-table` — comparison table across key entities.
/// - `source-bibliography` — bibliography of all captured sources.
/// - `imrad` — IMRaD-compliant scientific/technical report format
///   (Abstract, Introduction, Methods, Results, Discussion, References Index),
///   as specified in `specs/imradreport/SPEC.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Full multi-section research report (default).
    #[default]
    Report,
    /// One-page executive summary.
    ExecutiveSummary,
    /// Comparison table across key entities.
    ComparisonTable,
    /// Bibliography of all captured sources.
    SourceBibliography,
    /// IMRaD-compliant scientific/technical report format (Introduction, Methods,
    /// Results, and Discussion) — FR-001 / FR-002 of specs/imradreport.
    Imrad,
}

impl OutputFormat {
    /// Parse a format from its CLI name. Returns `None` for unknown values.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "report" | "default" => Some(Self::Report),
            "executive-summary" | "executive_summary" | "summary" => Some(Self::ExecutiveSummary),
            "comparison-table" | "comparison_table" | "comparison" => Some(Self::ComparisonTable),
            "source-bibliography" | "source_bibliography" | "bibliography" => {
                Some(Self::SourceBibliography)
            }
            "imrad" | "im-rad" | "scientific" => Some(Self::Imrad),
            _ => None,
        }
    }

    /// CLI-compatible name for this format.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Report => "report",
            Self::ExecutiveSummary => "executive-summary",
            Self::ComparisonTable => "comparison-table",
            Self::SourceBibliography => "source-bibliography",
            Self::Imrad => "imrad",
        }
    }
}

impl FromStr for OutputFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("unknown output format: {s}"))
    }
}

/// Research tier requested via `--tier` (FR-001 of specs/hyperresearch).
///
/// Tiers select the depth of the adversarial research pipeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    /// Bounded quick run: decompose → width sweep → draft → polish.
    Light,
    /// Deep adversarial run: all 16 pipeline steps (default).
    #[default]
    Full,
    /// Chaptered mega-run for long-form reports.
    Dissertation,
}

impl Tier {
    /// Parse a tier from its CLI name. Returns `None` for unknown values.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "light" => Some(Self::Light),
            "full" => Some(Self::Full),
            "dissertation" | "diss" => Some(Self::Dissertation),
            _ => None,
        }
    }

    /// CLI-compatible name for this tier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Full => "full",
            Self::Dissertation => "dissertation",
        }
    }

    /// Minimum number of sources the vault must already contain before the
    /// system skips issuing new web searches for this tier (FR-016, T-021).
    ///
    /// These thresholds are intentionally conservative: they reflect the
    /// idea that a `light` run needs only enough sources for a quick answer,
    /// while `full` and `dissertation` runs benefit from a broader corpus.
    #[must_use]
    pub const fn sufficient_sources(self) -> usize {
        match self {
            Self::Light => 3,
            Self::Full => 8,
            Self::Dissertation => 15,
        }
    }
}

impl FromStr for Tier {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("unknown tier: {s}"))
    }
}

/// Research depth preset requested via `--depth` (FR-011). Each preset maps to
/// an [`EngineConfig`] with different iteration and source budgets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Depth {
    /// Fewer iterations and sources — good for quick orientation.
    #[default]
    Shallow,
    /// Balanced default.
    Standard,
    /// More iterations and sources — thorough but slower.
    Deep,
}

impl Depth {
    /// Parse a depth from its CLI name. Returns `None` for unknown values.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "shallow" => Some(Self::Shallow),
            "standard" => Some(Self::Standard),
            "deep" => Some(Self::Deep),
            _ => None,
        }
    }

    /// CLI-compatible name for this depth.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Shallow => "shallow",
            Self::Standard => "standard",
            Self::Deep => "deep",
        }
    }

    /// Convert this depth + optional `--iterations` override into an engine
    /// configuration.
    #[must_use]
    pub fn engine_config(
        self,
        iterations_override: Option<u32>,
        force_deeper: bool,
    ) -> EngineConfig {
        let mut cfg = match self {
            Self::Shallow => EngineConfig {
                max_iterations: 1,
                max_sources_per_question: 2,
                max_concurrency: 2,
                force_deeper,
            },
            Self::Standard => EngineConfig {
                max_iterations: 3,
                max_sources_per_question: 3,
                max_concurrency: 4,
                force_deeper,
            },
            Self::Deep => EngineConfig {
                max_iterations: 5,
                max_sources_per_question: 5,
                max_concurrency: 6,
                force_deeper: true,
            },
        };
        if let Some(n) = iterations_override {
            cfg.max_iterations = n.max(1);
        }
        cfg
    }
}

impl FromStr for Depth {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s).ok_or_else(|| format!("unknown depth: {s}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tiers() {
        assert_eq!(Tier::parse("light"), Some(Tier::Light));
        assert_eq!(Tier::parse("full"), Some(Tier::Full));
        assert_eq!(Tier::parse("dissertation"), Some(Tier::Dissertation));
        assert_eq!(Tier::parse("diss"), Some(Tier::Dissertation));
        assert_eq!(Tier::parse("invalid"), None);
        assert_eq!(Tier::default(), Tier::Full);
    }

    #[test]
    fn tier_as_str_round_trips() {
        for tier in [Tier::Light, Tier::Full, Tier::Dissertation] {
            assert_eq!(Tier::parse(tier.as_str()), Some(tier));
        }
    }

    #[test]
    fn tier_sufficient_sources_matches_tier_depth() {
        assert_eq!(Tier::Light.sufficient_sources(), 3);
        assert_eq!(Tier::Full.sufficient_sources(), 8);
        assert_eq!(Tier::Dissertation.sufficient_sources(), 15);
    }

    #[test]
    fn parse_output_formats() {
        assert_eq!(OutputFormat::parse("report"), Some(OutputFormat::Report));
        assert_eq!(
            OutputFormat::parse("executive-summary"),
            Some(OutputFormat::ExecutiveSummary)
        );
        assert_eq!(
            OutputFormat::parse("comparison_table"),
            Some(OutputFormat::ComparisonTable)
        );
        assert_eq!(
            OutputFormat::parse("source-bibliography"),
            Some(OutputFormat::SourceBibliography)
        );
        assert_eq!(OutputFormat::parse("imrad"), Some(OutputFormat::Imrad));
        assert_eq!(OutputFormat::parse("im-rad"), Some(OutputFormat::Imrad));
        assert_eq!(OutputFormat::parse("scientific"), Some(OutputFormat::Imrad));
        assert_eq!(OutputFormat::Imrad.as_str(), "imrad");
        assert_eq!(OutputFormat::parse("nonsense"), None);
    }

    #[test]
    fn depth_presets() {
        let shallow = Depth::Shallow.engine_config(None, false);
        assert_eq!(shallow.max_iterations, 1);
        assert_eq!(shallow.max_sources_per_question, 2);

        let standard = Depth::Standard.engine_config(None, false);
        assert_eq!(standard.max_iterations, 3);

        let deep = Depth::Deep.engine_config(None, false);
        assert_eq!(deep.max_iterations, 5);
        assert!(deep.force_deeper);
    }

    #[test]
    fn iterations_override_wins() {
        let cfg = Depth::Shallow.engine_config(Some(10), false);
        assert_eq!(cfg.max_iterations, 10);
    }
}
