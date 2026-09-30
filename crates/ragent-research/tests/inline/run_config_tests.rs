//! Inline tests for `run_config.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn parse_research_modes() {
    assert_eq!(ResearchMode::parse("tiered"), Some(ResearchMode::Tiered));
    assert_eq!(ResearchMode::parse("tier"), Some(ResearchMode::Tiered));
    assert_eq!(
        ResearchMode::parse("supervisor"),
        Some(ResearchMode::Supervisor)
    );
    assert_eq!(
        ResearchMode::parse("competitive"),
        Some(ResearchMode::Competitive)
    );
    assert_eq!(
        ResearchMode::parse("competative"),
        Some(ResearchMode::Competitive),
        "common misspelling must map to Competitive"
    );
    assert_eq!(ResearchMode::parse("comp"), Some(ResearchMode::Competitive));
    assert_eq!(ResearchMode::parse("invalid"), None);
    assert_eq!(ResearchMode::default(), ResearchMode::Tiered);
    for mode in [
        ResearchMode::Tiered,
        ResearchMode::Supervisor,
        ResearchMode::Competitive,
    ] {
        assert_eq!(ResearchMode::parse(mode.as_str()), Some(mode));
    }
}

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
