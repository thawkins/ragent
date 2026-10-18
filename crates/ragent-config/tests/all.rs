//! Consolidated integration-test target: one test binary per crate.
//!
//! Each former top-level `tests/test_*.rs` file is included as a child
//! module via `#[path]`, so it keeps its own file-relative `mod` /
//! `#[path]` resolution and its `test_<file>::<fn>` test name. See
//! `AGENTS.md` ("Test Layout") and
//! `docs/reports/testconsolidation-review.md`.

#[path = "test_activity_log_persistence.rs"]
mod test_activity_log_persistence;

#[path = "test_agent_perf_config.rs"]
mod test_agent_perf_config;

#[path = "test_api_key_config_fields.rs"]
mod test_api_key_config_fields;

#[path = "test_automation_config.rs"]
mod test_automation_config;

#[path = "test_backend_and_research_defaults.rs"]
mod test_backend_and_research_defaults;

#[path = "test_bash_lists_runtime.rs"]
mod test_bash_lists_runtime;

#[path = "test_code_index_config.rs"]
mod test_code_index_config;

#[path = "test_codeindex_permission_matching.rs"]
mod test_codeindex_permission_matching;

#[path = "test_codeindex_toggle_persistence.rs"]
mod test_codeindex_toggle_persistence;

#[path = "test_compaction.rs"]
mod test_compaction;

#[path = "test_config_merge_deep.rs"]
mod test_config_merge_deep;

#[path = "test_config_parse_errors.rs"]
mod test_config_parse_errors;

#[path = "test_config_save.rs"]
mod test_config_save;

#[path = "test_connectors_config.rs"]
mod test_connectors_config;

#[path = "test_credential_env.rs"]
mod test_credential_env;

#[path = "test_default_config_creation.rs"]
mod test_default_config_creation;

#[path = "test_dir_lists_runtime.rs"]
mod test_dir_lists_runtime;

#[path = "test_dirs_merge.rs"]
mod test_dirs_merge;

#[path = "test_edit_log_and_gcf_toggles.rs"]
mod test_edit_log_and_gcf_toggles;

#[path = "test_execution_backend_config.rs"]
mod test_execution_backend_config;

#[path = "test_experimental_flags.rs"]
mod test_experimental_flags;

#[path = "test_gcf_config.rs"]
mod test_gcf_config;

#[path = "test_github_token.rs"]
mod test_github_token;

#[path = "test_i18n_catalog.rs"]
mod test_i18n_catalog;

#[path = "test_i18n_discovery.rs"]
mod test_i18n_discovery;

#[path = "test_input_queue_config.rs"]
mod test_input_queue_config;

#[path = "test_list_config_scope_and_patch.rs"]
mod test_list_config_scope_and_patch;

#[path = "test_loop_config.rs"]
mod test_loop_config;

#[path = "test_merge_project_privileged.rs"]
mod test_merge_project_privileged;

#[path = "test_ms0_config_guards.rs"]
mod test_ms0_config_guards;

#[path = "test_ms5_config_consistency.rs"]
mod test_ms5_config_consistency;

#[path = "test_openai_config.rs"]
mod test_openai_config;

#[path = "test_permission.rs"]
mod test_permission;

#[path = "test_permission_checker_extra.rs"]
mod test_permission_checker_extra;

#[path = "test_piegap_config.rs"]
mod test_piegap_config;

#[path = "test_plugin_stores_config.rs"]
mod test_plugin_stores_config;

#[path = "test_plugins_config.rs"]
mod test_plugins_config;

#[path = "test_research_config.rs"]
mod test_research_config;

#[path = "test_runtime_flag_toggles.rs"]
mod test_runtime_flag_toggles;

#[path = "test_sdd_config.rs"]
mod test_sdd_config;

#[path = "test_small_sections_coverage.rs"]
mod test_small_sections_coverage;

#[path = "test_stream_config_defaults.rs"]
mod test_stream_config_defaults;

#[path = "test_telemetry_config.rs"]
mod test_telemetry_config;

#[path = "test_thinking_config.rs"]
mod test_thinking_config;

#[path = "test_tool_visibility.rs"]
mod test_tool_visibility;

#[path = "test_user_dirs_helpers.rs"]
mod test_user_dirs_helpers;

#[path = "test_yolo_persistence.rs"]
mod test_yolo_persistence;
