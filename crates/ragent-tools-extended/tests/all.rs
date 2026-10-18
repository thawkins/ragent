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

/// The one process-wide lock that serialises tests which mutate or observe
/// process-global state (`set_current_dir`, `XDG_CONFIG_HOME`, `HOME`,
/// `PATH`).
///
/// Before consolidation each test file was its own binary, so a file-local
/// mutex was enough. One binary per crate merges them, so every mutating test
/// in every child file must contend on the SAME mutex; `crate::` resolves at
/// this binary root, which is what makes that possible. See AGENTS.md
/// ("Test Layout").
pub fn global_env_mutex() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

/// Acquire [`global_env_mutex`], recovering from a poisoned lock (a panicking
/// test must not wedge every later test).
pub fn global_env_lock() -> std::sync::MutexGuard<'static, ()> {
    global_env_mutex()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[path = "test_archdoc_author_split.rs"]
mod test_archdoc_author_split;

#[path = "test_archdoc_content_ref.rs"]
mod test_archdoc_content_ref;

#[path = "test_archdoc_extract.rs"]
mod test_archdoc_extract;

#[path = "test_archdoc_local_source.rs"]
mod test_archdoc_local_source;

#[path = "test_archdoc_url_source.rs"]
mod test_archdoc_url_source;

#[path = "test_browser_gate.rs"]
mod test_browser_gate;

#[path = "test_codeindex_backward_compat.rs"]
mod test_codeindex_backward_compat;

#[path = "test_codeindex_busy_state.rs"]
mod test_codeindex_busy_state;

#[path = "test_codeindex_communities.rs"]
mod test_codeindex_communities;

#[path = "test_codeindex_explain.rs"]
mod test_codeindex_explain;

#[path = "test_codeindex_godnodes.rs"]
mod test_codeindex_godnodes;

#[path = "test_codeindex_path.rs"]
mod test_codeindex_path;

#[path = "test_codeindex_registry.rs"]
mod test_codeindex_registry;

#[path = "test_codeindex_status_busy.rs"]
mod test_codeindex_status_busy;

#[path = "test_embedding.rs"]
mod test_embedding;

#[path = "test_govcreate_nfr004.rs"]
mod test_govcreate_nfr004;

#[path = "test_govcreate_runner.rs"]
mod test_govcreate_runner;

#[path = "test_govcreate_scaffold_reuse.rs"]
mod test_govcreate_scaffold_reuse;

#[path = "test_legacy_web_ssrf.rs"]
mod test_legacy_web_ssrf;

#[path = "test_lib_surface.rs"]
mod test_lib_surface;

#[path = "test_masterfetch_language.rs"]
mod test_masterfetch_language;

#[path = "test_mf_aux.rs"]
mod test_mf_aux;

#[path = "test_mf_cache.rs"]
mod test_mf_cache;

#[path = "test_mf_classify.rs"]
mod test_mf_classify;

#[path = "test_mf_consensus.rs"]
mod test_mf_consensus;

#[path = "test_mf_consensus_multieng.rs"]
mod test_mf_consensus_multieng;

#[path = "test_mf_engine_classification.rs"]
mod test_mf_engine_classification;

#[path = "test_mf_engine_resilience.rs"]
mod test_mf_engine_resilience;

#[path = "test_mf_envelope.rs"]
mod test_mf_envelope;

#[path = "test_mf_exa.rs"]
mod test_mf_exa;

#[path = "test_mf_exclude_engines.rs"]
mod test_mf_exclude_engines;

#[path = "test_mf_extractor.rs"]
mod test_mf_extractor;

#[path = "test_mf_fetch.rs"]
mod test_mf_fetch;

#[path = "test_mf_focus.rs"]
mod test_mf_focus;

#[path = "test_mf_langsearch.rs"]
mod test_mf_langsearch;

#[path = "test_mf_links.rs"]
mod test_mf_links;

#[path = "test_mf_metadata.rs"]
mod test_mf_metadata;

#[path = "test_mf_openalex.rs"]
mod test_mf_openalex;

#[path = "test_mf_openalex_live.rs"]
mod test_mf_openalex_live;

#[path = "test_mf_orchestrator.rs"]
mod test_mf_orchestrator;

#[path = "test_mf_orchestrator_diag.rs"]
mod test_mf_orchestrator_diag;

#[path = "test_mf_panic_repro.rs"]
mod test_mf_panic_repro;

#[path = "test_mf_pdf.rs"]
mod test_mf_pdf;

#[path = "test_mf_permission_categories.rs"]
mod test_mf_permission_categories;

#[path = "test_mf_perplexity.rs"]
mod test_mf_perplexity;

#[path = "test_mf_robots.rs"]
mod test_mf_robots;

#[path = "test_mf_search_author.rs"]
mod test_mf_search_author;

#[path = "test_mf_search_tool.rs"]
mod test_mf_search_tool;

#[path = "test_mf_security.rs"]
mod test_mf_security;

#[path = "test_mf_security_notation.rs"]
mod test_mf_security_notation;

#[path = "test_mf_serper.rs"]
mod test_mf_serper;

#[path = "test_mf_supplement.rs"]
mod test_mf_supplement;

#[path = "test_mf_tavily.rs"]
mod test_mf_tavily;

#[path = "test_mf_urlnorm.rs"]
mod test_mf_urlnorm;

#[path = "test_mf_version.rs"]
mod test_mf_version;

#[path = "test_mf_visibility.rs"]
mod test_mf_visibility;

#[path = "test_mf_wikipedia.rs"]
mod test_mf_wikipedia;

#[path = "test_mf_wikipedia_live.rs"]
mod test_mf_wikipedia_live;

#[path = "test_mf_youtube.rs"]
mod test_mf_youtube;

#[path = "test_no_percall_regex.rs"]
mod test_no_percall_regex;

#[path = "test_pdf_expert_cff.rs"]
mod test_pdf_expert_cff;

#[path = "test_pdf_roundtrip.rs"]
mod test_pdf_roundtrip;

#[path = "test_project_scaffold_docs.rs"]
mod test_project_scaffold_docs;

#[path = "test_project_scaffold_emit.rs"]
mod test_project_scaffold_emit;

#[path = "test_project_scaffold_flags.rs"]
mod test_project_scaffold_flags;

#[path = "test_project_scaffold_gitinit.rs"]
mod test_project_scaffold_gitinit;

#[path = "test_project_scaffold_gitlab_remote.rs"]
mod test_project_scaffold_gitlab_remote;

#[path = "test_project_scaffold_guard.rs"]
mod test_project_scaffold_guard;

#[path = "test_project_scaffold_help.rs"]
mod test_project_scaffold_help;

#[path = "test_project_scaffold_layout.rs"]
mod test_project_scaffold_layout;

#[path = "test_project_scaffold_recipes.rs"]
mod test_project_scaffold_recipes;

#[path = "test_project_scaffold_remote.rs"]
mod test_project_scaffold_remote;

#[path = "test_project_scaffold_stack.rs"]
mod test_project_scaffold_stack;

#[path = "test_project_scaffold_workspace.rs"]
mod test_project_scaffold_workspace;

#[path = "test_t017_govcreate_harness.rs"]
mod test_t017_govcreate_harness;

#[path = "test_task_create.rs"]
mod test_task_create;

#[path = "test_task_get.rs"]
mod test_task_get;

#[path = "test_task_list.rs"]
mod test_task_list;

#[path = "test_task_unblock.rs"]
mod test_task_unblock;

#[path = "test_task_update.rs"]
mod test_task_update;

#[path = "test_tavily.rs"]
mod test_tavily;

#[path = "test_websearch.rs"]
mod test_websearch;
