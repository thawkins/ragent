//! Consolidated integration-test target: one test binary per crate.
//!
//! Each former top-level `tests/test_*.rs` file is included as a child
//! module via `#[path]`, so it keeps its own file-relative `mod` /
//! `#[path]` resolution and its `test_<file>::<fn>` test name. See
//! `AGENTS.md` ("Test Layout") and
//! `docs/reports/testconsolidation-review.md`.

#![allow(clippy::duplicate_mod)]
// ^ Intentional: each `#[path]`-included child file declares its own
// `mod support;` / `mod <fixture>;`, so a shared helper file is a module of
// several children. Under one-binary-per-crate that loads it more than once,
// which clippy::duplicate_mod flags; hoisting it would require editing every
// child file. See AGENTS.md ("Test Layout").

// Crate-root shims: `src/session/fallback.rs` (included by
// `test_session_fallback.rs` below via `#[path]`) refers to `crate::document`
// and `crate::source`, which must exist at this binary's root. The prelude file
// is the single source of truth for those re-exports.
#[path = "all_prelude.rs"]
mod all_prelude;

pub use all_prelude::{document, source};

#[path = "test_analysis_engine.rs"]
mod test_analysis_engine;
#[path = "test_analysis_engine_builders.rs"]
mod test_analysis_engine_builders;
#[path = "test_assemble_imrad.rs"]
mod test_assemble_imrad;
#[path = "test_chunked_synthesis_diagram.rs"]
mod test_chunked_synthesis_diagram;
#[path = "test_clarification_brief_entity.rs"]
mod test_clarification_brief_entity;
#[path = "test_cli_parser_full.rs"]
mod test_cli_parser_full;
#[path = "test_cli_session_event_json.rs"]
mod test_cli_session_event_json;
#[path = "test_comparison_table.rs"]
mod test_comparison_table;
#[path = "test_concepts_format.rs"]
mod test_concepts_format;
#[path = "test_concepts_section.rs"]
mod test_concepts_section;
#[path = "test_contradiction.rs"]
mod test_contradiction;
#[path = "test_control_char_sanitization.rs"]
mod test_control_char_sanitization;
#[path = "test_corpa_companion.rs"]
mod test_corpa_companion;
#[path = "test_corpus_critic.rs"]
mod test_corpus_critic;
#[path = "test_findings_diagram_section.rs"]
mod test_findings_diagram_section;
#[path = "test_findings_diagram_section_skeleton.rs"]
mod test_findings_diagram_section_skeleton;
#[path = "test_hit_author_propagation.rs"]
mod test_hit_author_propagation;
#[path = "test_hyperresearch_manual.rs"]
mod test_hyperresearch_manual;
#[path = "test_ms04_source_neutralisation.rs"]
mod test_ms04_source_neutralisation;
#[path = "test_no_percall_regex.rs"]
mod test_no_percall_regex;
#[path = "test_open_access_helpers.rs"]
mod test_open_access_helpers;
#[path = "test_output_limits_ordering.rs"]
mod test_output_limits_ordering;
#[path = "test_patcher_extra.rs"]
mod test_patcher_extra;
#[path = "test_property.rs"]
mod test_property;
#[path = "test_provider_stats.rs"]
mod test_provider_stats;
#[path = "test_reconcile.rs"]
mod test_reconcile;
#[path = "test_research_baseline.rs"]
mod test_research_baseline;
#[path = "test_research_config_event.rs"]
mod test_research_config_event;
#[path = "test_research_create_synthesis.rs"]
mod test_research_create_synthesis;
#[path = "test_research_e2e.rs"]
mod test_research_e2e;
#[path = "test_research_integration.rs"]
mod test_research_integration;
#[path = "test_research_invocation_frontmatter.rs"]
mod test_research_invocation_frontmatter;
#[path = "test_research_invocation_replay.rs"]
mod test_research_invocation_replay;
#[path = "test_research_limits.rs"]
mod test_research_limits;
#[path = "test_research_output_limit_args.rs"]
mod test_research_output_limit_args;
#[path = "test_research_run_request.rs"]
mod test_research_run_request;
#[path = "test_scoreboard.rs"]
mod test_scoreboard;
#[path = "test_scoreboard_imrad.rs"]
mod test_scoreboard_imrad;
#[path = "test_scoreboard_reductions.rs"]
mod test_scoreboard_reductions;
#[path = "test_scoreboard_report.rs"]
mod test_scoreboard_report;
#[path = "test_search_budget.rs"]
mod test_search_budget;
#[path = "test_search_budget_gatherer.rs"]
mod test_search_budget_gatherer;
#[path = "test_search_retry_clamp.rs"]
mod test_search_retry_clamp;
#[path = "test_session_fallback.rs"]
mod test_session_fallback;
#[path = "test_session_topic.rs"]
mod test_session_topic;
#[path = "test_session_topic_extra.rs"]
mod test_session_topic_extra;
#[path = "test_source_vault.rs"]
mod test_source_vault;
#[path = "test_template_merge.rs"]
mod test_template_merge;
#[path = "test_url_cloak.rs"]
mod test_url_cloak;
#[path = "test_verify_func065.rs"]
mod test_verify_func065;
#[path = "test_web_gather_log.rs"]
mod test_web_gather_log;
#[path = "test_web_gatherer_engine_exclusion.rs"]
mod test_web_gatherer_engine_exclusion;
#[path = "test_web_gatherer_exclusions.rs"]
mod test_web_gatherer_exclusions;
#[path = "test_web_gatherer_helpers.rs"]
mod test_web_gatherer_helpers;
#[path = "test_web_search_tool_exclusions.rs"]
mod test_web_search_tool_exclusions;
#[path = "test_web_time_deadline.rs"]
mod test_web_time_deadline;
