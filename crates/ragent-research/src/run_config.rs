//! Runtime configuration for `/research` runs: output formats and depth presets.
//!
//! This module holds the small value types that translate CLI flags such as
//! `--format` and `--depth` into the engine's concrete [`EngineConfig`].

use crate::engine::EngineConfig;
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
