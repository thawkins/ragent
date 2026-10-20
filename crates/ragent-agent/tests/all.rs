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

// Crate-root shims: child test files below include private source modules via
// `#[path = "../src/..."]`. Those modules refer to `crate::llm`,
// `crate::event`, `crate::permission`, `crate::session::loop_state` and
// `crate::tool`, and a `#[path]`-included module resolves `crate::` against
// THIS binary root rather than the ragent-agent crate. Re-export the public
// crate paths here so those references resolve. See AGENTS.md ("Test Layout").
/// Crate-root `event` shim (see the note above).
pub mod event {
    pub use ragent_agent::event::*;
}
pub use ragent_agent::llm;
pub use ragent_agent::permission;
/// Crate-root `session` shim (see the note above).
pub mod session {
    /// Crate-root `session::loop_state` shim (see the note above).
    pub mod loop_state {
        pub use ragent_agent::session::loop_state::*;
    }
}
pub use ragent_agent::tool;

#[path = "test_acp_client.rs"]
mod test_acp_client;

#[path = "test_acp_server.rs"]
mod test_acp_server;

#[path = "test_acp_update_parsing.rs"]
mod test_acp_update_parsing;

#[path = "test_activity_writer.rs"]
mod test_activity_writer;

#[path = "test_agent_memory_deep.rs"]
mod test_agent_memory_deep;

#[path = "test_agent_perf_env.rs"]
mod test_agent_perf_env;

#[path = "test_agent_skill_hooks_extra.rs"]
mod test_agent_skill_hooks_extra;

#[path = "test_agent_thinking_defaults.rs"]
mod test_agent_thinking_defaults;

#[path = "test_agentskills_discovery.rs"]
mod test_agentskills_discovery;

#[path = "test_ask_user_default.rs"]
mod test_ask_user_default;

#[path = "test_automation_dispatch.rs"]
mod test_automation_dispatch;

#[path = "test_automation_helpers.rs"]
mod test_automation_helpers;

#[path = "test_automation_runtime.rs"]
mod test_automation_runtime;

#[path = "test_automation_service.rs"]
mod test_automation_service;

#[path = "test_backend_misc_more.rs"]
mod test_backend_misc_more;

#[path = "test_backend_registry.rs"]
mod test_backend_registry;

#[path = "test_bang_command.rs"]
mod test_bang_command;

#[path = "test_bg_service.rs"]
mod test_bg_service;

#[path = "test_bg_tool.rs"]
mod test_bg_tool;

#[path = "test_blueprint_seed_permissions.rs"]
mod test_blueprint_seed_permissions;

#[path = "test_builtin_agents.rs"]
mod test_builtin_agents;

#[path = "test_compaction_flow.rs"]
mod test_compaction_flow;

#[path = "test_compaction_integration.rs"]
mod test_compaction_integration;

#[path = "test_compaction_prompt.rs"]
mod test_compaction_prompt;

#[path = "test_compaction_serializer.rs"]
mod test_compaction_serializer;

#[path = "test_compaction_session_ops.rs"]
mod test_compaction_session_ops;

#[path = "test_concurrent_store_writes.rs"]
mod test_concurrent_store_writes;

#[path = "test_conflict_policy.rs"]
mod test_conflict_policy;

#[path = "test_container_backend.rs"]
mod test_container_backend;

#[path = "test_conversation_search.rs"]
mod test_conversation_search;

#[path = "test_cron_tools.rs"]
mod test_cron_tools;

#[path = "test_cross_await_mutex.rs"]
mod test_cross_await_mutex;

#[path = "test_custom_agent.rs"]
mod test_custom_agent;

#[path = "test_custom_agent_dirs.rs"]
mod test_custom_agent_dirs;

#[path = "test_dispatch_resolution.rs"]
mod test_dispatch_resolution;

#[path = "test_dry_run.rs"]
mod test_dry_run;

#[path = "test_dry_run_extra.rs"]
mod test_dry_run_extra;

#[path = "test_dump_registries.rs"]
mod test_dump_registries;

#[path = "test_dynamic_trigger.rs"]
mod test_dynamic_trigger;

#[path = "test_editrenewal_aliases.rs"]
mod test_editrenewal_aliases;

#[path = "test_execution_backend_runtime.rs"]
mod test_execution_backend_runtime;

#[path = "test_file_ops.rs"]
mod test_file_ops;

#[path = "test_fuzzy_reference.rs"]
mod test_fuzzy_reference;

#[path = "test_gcf_encode_hook.rs"]
mod test_gcf_encode_hook;

#[path = "test_gcf_primer.rs"]
mod test_gcf_primer;

#[path = "test_gcf_raw_consumers.rs"]
mod test_gcf_raw_consumers;

#[path = "test_goal_engine.rs"]
mod test_goal_engine;

#[path = "test_hardening_invariants.rs"]
mod test_hardening_invariants;

#[path = "test_history_estimators.rs"]
mod test_history_estimators;

#[path = "test_history_version_cache.rs"]
mod test_history_version_cache;

#[path = "test_hooks.rs"]
mod test_hooks;

#[path = "test_initiative.rs"]
mod test_initiative;

#[path = "test_instruction_file_priority.rs"]
mod test_instruction_file_priority;

#[path = "test_instruction_includes.rs"]
mod test_instruction_includes;

#[path = "test_interim_tool_call_persist.rs"]
mod test_interim_tool_call_persist;

#[path = "test_knowledge_graph.rs"]
mod test_knowledge_graph;

#[path = "test_list_agents_tool.rs"]
mod test_list_agents_tool;

#[path = "test_loop_capture.rs"]
mod test_loop_capture;

#[path = "test_loop_checkpoints.rs"]
mod test_loop_checkpoints;

#[path = "test_loop_error_kind.rs"]
mod test_loop_error_kind;

#[path = "test_loop_error_stop.rs"]
mod test_loop_error_stop;

#[path = "test_loop_goal_prompt.rs"]
mod test_loop_goal_prompt;

#[path = "test_loop_interrupt.rs"]
mod test_loop_interrupt;

#[path = "test_loop_restrictions.rs"]
mod test_loop_restrictions;

#[path = "test_loop_state.rs"]
mod test_loop_state;

#[path = "test_loop_steps_extra.rs"]
mod test_loop_steps_extra;

#[path = "test_loop_steps_harness.rs"]
mod test_loop_steps_harness;

#[path = "test_loop_stop_guard.rs"]
mod test_loop_stop_guard;

#[path = "test_loop_telemetry.rs"]
mod test_loop_telemetry;

#[path = "test_loop_termination.rs"]
mod test_loop_termination;

#[path = "test_m3_lifecycle.rs"]
mod test_m3_lifecycle;

#[path = "test_m4_delivery.rs"]
mod test_m4_delivery;

#[path = "test_m6_resilience.rs"]
mod test_m6_resilience;

#[path = "test_m8_polish.rs"]
mod test_m8_polish;

#[path = "test_mcp_acp_more.rs"]
mod test_mcp_acp_more;

#[path = "test_mcp_adopt.rs"]
mod test_mcp_adopt;

#[path = "test_mcp_connect_timeout.rs"]
mod test_mcp_connect_timeout;

#[path = "test_mcp_discovery_extra.rs"]
mod test_mcp_discovery_extra;

#[path = "test_mcp_http.rs"]
mod test_mcp_http;

#[path = "test_mcp_http_transport.rs"]
mod test_mcp_http_transport;

#[path = "test_mcp_notification.rs"]
mod test_mcp_notification;

#[path = "test_mcp_refresh_more.rs"]
mod test_mcp_refresh_more;

#[path = "test_mcp_tool_index.rs"]
mod test_mcp_tool_index;

#[path = "test_mcp_tool_wrapper.rs"]
mod test_mcp_tool_wrapper;

#[path = "test_memory_extraction.rs"]
mod test_memory_extraction;

#[path = "test_memory_prompt_section.rs"]
mod test_memory_prompt_section;

#[path = "test_memory_visualisation.rs"]
mod test_memory_visualisation;

#[path = "test_memory_visualisation_extra.rs"]
mod test_memory_visualisation_extra;

#[path = "test_metadata_builder.rs"]
mod test_metadata_builder;

#[path = "test_model_info_tool.rs"]
mod test_model_info_tool;

#[path = "test_ms03_bash_split.rs"]
mod test_ms03_bash_split;

#[path = "test_ms03_team_guards.rs"]
mod test_ms03_team_guards;

#[path = "test_no_percall_regex.rs"]
mod test_no_percall_regex;

#[path = "test_one_shot.rs"]
mod test_one_shot;

#[path = "test_one_shot_more.rs"]
mod test_one_shot_more;

#[path = "test_orchestrator_coordinator.rs"]
mod test_orchestrator_coordinator;

#[path = "test_orchestrator_leader.rs"]
mod test_orchestrator_leader;

#[path = "test_orchestrator_registry_extra.rs"]
mod test_orchestrator_registry_extra;

#[path = "test_orchestrator_transport.rs"]
mod test_orchestrator_transport;

#[path = "test_os_info_tool.rs"]
mod test_os_info_tool;

#[path = "test_perf022_jsonl_mailbox.rs"]
mod test_perf022_jsonl_mailbox;

#[path = "test_perf023_task_cache.rs"]
mod test_perf023_task_cache;

#[path = "test_permission_helpers.rs"]
mod test_permission_helpers;

#[path = "test_plugin_bridge.rs"]
mod test_plugin_bridge;

#[path = "test_plugin_hooks.rs"]
mod test_plugin_hooks;

#[path = "test_precompiled_regexes.rs"]
mod test_precompiled_regexes;

#[path = "test_processor_deep_paths.rs"]
mod test_processor_deep_paths;

#[path = "test_processor_loop_paths.rs"]
mod test_processor_loop_paths;

#[path = "test_processor_more_paths.rs"]
mod test_processor_more_paths;

#[path = "test_profiler.rs"]
mod test_profiler;

#[path = "test_prompt_builders_more.rs"]
mod test_prompt_builders_more;

#[path = "test_prompt_context_cache.rs"]
mod test_prompt_context_cache;

#[path = "test_ragent_info_tool.rs"]
mod test_ragent_info_tool;

#[path = "test_reference_resolve.rs"]
mod test_reference_resolve;

#[path = "test_remote_acp_dispatch.rs"]
mod test_remote_acp_dispatch;

#[path = "test_remote_backend.rs"]
mod test_remote_backend;

#[path = "test_research_adapter_extra2.rs"]
mod test_research_adapter_extra2;

#[path = "test_research_adapter_helpers.rs"]
mod test_research_adapter_helpers;

#[path = "test_research_adapter_more.rs"]
mod test_research_adapter_more;

#[path = "test_research_web_author.rs"]
mod test_research_web_author;

#[path = "test_rollback_capture.rs"]
mod test_rollback_capture;

#[path = "test_run_cost.rs"]
mod test_run_cost;

#[path = "test_run_cost_summary.rs"]
mod test_run_cost_summary;

#[path = "test_sandbox_secrets.rs"]
mod test_sandbox_secrets;

#[path = "test_security_analyzer.rs"]
mod test_security_analyzer;

#[path = "test_security_analyzer_loop.rs"]
mod test_security_analyzer_loop;

#[path = "test_session_archive.rs"]
mod test_session_archive;

#[path = "test_session_archive_more.rs"]
mod test_session_archive_more;

#[path = "test_session_cache.rs"]
mod test_session_cache;

#[path = "test_session_manager_lifecycle.rs"]
mod test_session_manager_lifecycle;

#[path = "test_session_processor.rs"]
mod test_session_processor;

#[path = "test_session_search.rs"]
mod test_session_search;

#[path = "test_session_text_helpers.rs"]
mod test_session_text_helpers;

#[path = "test_skill_catalog.rs"]
mod test_skill_catalog;

#[path = "test_skill_invoke.rs"]
mod test_skill_invoke;

#[path = "test_skill_manage.rs"]
mod test_skill_manage;

#[path = "test_spawn_detached.rs"]
mod test_spawn_detached;

#[path = "test_spec_task_mirror.rs"]
mod test_spec_task_mirror;

#[path = "test_spec_tools.rs"]
mod test_spec_tools;

#[path = "test_step_stall_budget.rs"]
mod test_step_stall_budget;

#[path = "test_storage_format_version_cache.rs"]
mod test_storage_format_version_cache;

#[path = "test_storage_op_blocking.rs"]
mod test_storage_op_blocking;

#[path = "test_stream_config_defaults.rs"]
mod test_stream_config_defaults;

#[path = "test_structured_memory_store.rs"]
mod test_structured_memory_store;

#[path = "test_structured_memory_tool.rs"]
mod test_structured_memory_tool;

#[path = "test_subagent_interactive_block.rs"]
mod test_subagent_interactive_block;

#[path = "test_subagent_summary_nudge.rs"]
mod test_subagent_summary_nudge;

#[path = "test_subagent_truncation.rs"]
mod test_subagent_truncation;

#[path = "test_swarm_agent_assignment.rs"]
mod test_swarm_agent_assignment;

#[path = "test_system_prompt_cache.rs"]
mod test_system_prompt_cache;

#[path = "test_task_registry_bookkeeping.rs"]
mod test_task_registry_bookkeeping;

#[path = "test_task_registry_snapshot.rs"]
mod test_task_registry_snapshot;

#[path = "test_task_tool_family_prompts.rs"]
mod test_task_tool_family_prompts;

#[path = "test_task_tools_cache_more.rs"]
mod test_task_tools_cache_more;

#[path = "test_team_classify.rs"]
mod test_team_classify;

#[path = "test_team_manager.rs"]
mod test_team_manager;

#[path = "test_team_manager_extra.rs"]
mod test_team_manager_extra;

#[path = "test_team_manager_runtime.rs"]
mod test_team_manager_runtime;

#[path = "test_team_manager_spawn.rs"]
mod test_team_manager_spawn;

#[path = "test_team_name_validation.rs"]
mod test_team_name_validation;

#[path = "test_team_store_extra.rs"]
mod test_team_store_extra;

#[path = "test_team_swarm.rs"]
mod test_team_swarm;

#[path = "test_team_task_complete_event.rs"]
mod test_team_task_complete_event;

#[path = "test_team_task_store.rs"]
mod test_team_task_store;

#[path = "test_teammate_retry_backoff.rs"]
mod test_teammate_retry_backoff;

#[path = "test_template_discovery.rs"]
mod test_template_discovery;

#[path = "test_text_toolcalls.rs"]
mod test_text_toolcalls;

#[path = "test_thinking_pipeline.rs"]
mod test_thinking_pipeline;

#[path = "test_tool_adapter_events.rs"]
mod test_tool_adapter_events;

#[path = "test_tool_adapters_extra2.rs"]
mod test_tool_adapters_extra2;

#[path = "test_tool_info.rs"]
mod test_tool_info;

#[path = "test_tool_registry_extra.rs"]
mod test_tool_registry_extra;

#[path = "test_tool_repeat_guard.rs"]
mod test_tool_repeat_guard;

#[path = "test_tool_result_arc_str.rs"]
mod test_tool_result_arc_str;

#[path = "test_task_suspend_resume.rs"]
mod test_task_suspend_resume;

#[path = "test_tool_task_adapters.rs"]
mod test_tool_task_adapters;

#[path = "test_tool_visibility.rs"]
mod test_tool_visibility;

#[path = "test_trigger_runtime.rs"]
mod test_trigger_runtime;

#[path = "test_updater.rs"]
mod test_updater;

#[path = "test_updater_extra.rs"]
mod test_updater_extra;

#[path = "test_updater_more.rs"]
mod test_updater_more;

#[path = "test_verification_gate.rs"]
mod test_verification_gate;

#[path = "test_wait_agents_results_meta.rs"]
mod test_wait_agents_results_meta;
