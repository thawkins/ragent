# Release Notes

## v0.1.0-beta.10

### Added
- Tavily search backend migrated into the `mf_search` multi-engine framework; it now runs in parallel with DuckDuckGo, Brave, and optional LangSearch.
- New `TavilyEngine` implementing the `SearchEngine` trait in `ragent-tools-extended`.
- `mf_search` description updated to mention Tavily and both optional API keys.
- Research system (`ragent-research`) now depends on `ragent-tools-extended` and its web-gathering adapter prefers `mf_search`, falling back to legacy `websearch`.
- New `parse_mf_search_metadata` helper maps `mf_search` JSON metadata into research-layer `WebSearchHit` rows while preserving `search_tool` and `search_engine` provenance.
- TUI now recognises `mf_search` in tool input/result summaries.
- Fixed missing `tempfile` and `calamine` dev-dependencies in `ragent-bench` so the workspace test suite compiles.

### Changed
- Legacy `websearch` tool docs now clarify it is retained for direct agent use and backwards compatibility; research workflows prefer `mf_search`.

### Fixed
- `WebSearchHit` provenance fields are now populated consistently across `websearch`, `mf_search`, and research output.
