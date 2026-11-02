pub use ragent_research::document;
pub use ragent_research::source;

/// Resolve the path of the report written for `name` under `research_root`.
///
/// Reports carry a dated name (`RESEARCH-YYYY-MM-DD.md`) or, for an item
/// created by an older ragent, the legacy undated `RESEARCH.md`. A test that
/// reads a run's output must ask the resolver rather than joining a hardcoded
/// file name.
pub async fn research_report_path(
    research_root: &std::path::Path,
    name: &str,
) -> std::path::PathBuf {
    let research_name = ragent_research::ResearchName::try_new(name)
        .unwrap_or_else(|e| panic!("invalid research name '{name}': {e}"));
    ragent_research::ResearchIo::find_research_md(research_root, &research_name)
        .await
        .unwrap_or_else(|e| panic!("locating the report for '{name}': {e}"))
}

/// Resolve the path of the `CORPA` companion written for `name`.
pub async fn research_corpa_path(
    research_root: &std::path::Path,
    name: &str,
) -> std::path::PathBuf {
    let research_name = ragent_research::ResearchName::try_new(name)
        .unwrap_or_else(|e| panic!("invalid research name '{name}': {e}"));
    ragent_research::ResearchIo::find_corpa_md(research_root, &research_name)
        .await
        .unwrap_or_else(|e| panic!("locating the CORPA companion for '{name}': {e}"))
}
