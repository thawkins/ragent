//! Consolidated integration-test target: one test binary per crate.
//!
//! Each former top-level `tests/test_*.rs` file is included as a child
//! module via `#[path]`, so it keeps its own file-relative `mod` /
//! `#[path]` resolution and its `test_<file>::<fn>` test name. See
//! `AGENTS.md` ("Test Layout") and
//! `docs/reports/testconsolidation-review.md`.

#![allow(clippy::duplicate_mod)]
// ^ Intentional: each `#[path]`-included child file declares its own
// `mod support;` / `mod slash_support;`, so a shared helper file is a module
// of several children. Under one-binary-per-crate that loads it more than
// once, which clippy::duplicate_mod flags; hoisting it would require editing
// every child file. See AGENTS.md ("Test Layout").

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

#[path = "test_active_tasks_reconcile.rs"]
mod test_active_tasks_reconcile;
#[path = "test_agent_notice_separation.rs"]
mod test_agent_notice_separation;
#[path = "test_agent_switching.rs"]
mod test_agent_switching;
#[path = "test_agents_teams_cursor.rs"]
mod test_agents_teams_cursor;
#[path = "test_agents_window_frame.rs"]
mod test_agents_window_frame;
#[path = "test_agents_window_table.rs"]
mod test_agents_window_table;
#[path = "test_agentui_t019_offline.rs"]
mod test_agentui_t019_offline;
#[path = "test_app_send_sync.rs"]
mod test_app_send_sync;
#[path = "test_automation_command.rs"]
mod test_automation_command;
#[path = "test_azure_resource_flow.rs"]
mod test_azure_resource_flow;
#[path = "test_backend_command.rs"]
mod test_backend_command;
#[path = "test_bench_command.rs"]
mod test_bench_command;
#[path = "test_block_in_place_guard.rs"]
mod test_block_in_place_guard;
#[path = "test_blueprints.rs"]
mod test_blueprints;
#[path = "test_busy_send_guard.rs"]
mod test_busy_send_guard;
#[path = "test_button.rs"]
mod test_button;
#[path = "test_button_counts.rs"]
mod test_button_counts;
#[path = "test_clipboard.rs"]
mod test_clipboard;
#[path = "test_clipboard_tempfile.rs"]
mod test_clipboard_tempfile;
#[path = "test_codeindex_backward_compat.rs"]
mod test_codeindex_backward_compat;
#[path = "test_codeindex_disabled_guard.rs"]
mod test_codeindex_disabled_guard;
#[path = "test_codeindex_empty_graph_guard.rs"]
mod test_codeindex_empty_graph_guard;
#[path = "test_codeindex_indicators.rs"]
mod test_codeindex_indicators;
#[path = "test_codeindex_skillgen.rs"]
mod test_codeindex_skillgen;
#[path = "test_command_catalog.rs"]
mod test_command_catalog;
#[path = "test_compaction_label.rs"]
mod test_compaction_label;
#[path = "test_compaction_result.rs"]
mod test_compaction_result;
#[path = "test_compression_indicator.rs"]
mod test_compression_indicator;
#[path = "test_config_write.rs"]
mod test_config_write;
#[path = "test_configured_provider_selection.rs"]
mod test_configured_provider_selection;
#[path = "test_connector_catalogue_enrich.rs"]
mod test_connector_catalogue_enrich;
#[path = "test_connector_catalogue_marketplace_warning.rs"]
mod test_connector_catalogue_marketplace_warning;
#[path = "test_connector_catalogue_mouse.rs"]
mod test_connector_catalogue_mouse;
#[path = "test_connector_catalogue_panel.rs"]
mod test_connector_catalogue_panel;
#[path = "test_connectors_command.rs"]
mod test_connectors_command;
#[path = "test_context_panel_t002.rs"]
mod test_context_panel_t002;
#[path = "test_context_panel_t004.rs"]
mod test_context_panel_t004;
#[path = "test_context_panel_t005.rs"]
mod test_context_panel_t005;
#[path = "test_context_panel_t006.rs"]
mod test_context_panel_t006;
#[path = "test_context_panel_t007.rs"]
mod test_context_panel_t007;
#[path = "test_context_panel_t008.rs"]
mod test_context_panel_t008;
#[path = "test_context_panel_t009.rs"]
mod test_context_panel_t009;
#[path = "test_context_panel_t010.rs"]
mod test_context_panel_t010;
#[path = "test_context_panel_t012.rs"]
mod test_context_panel_t012;
#[path = "test_context_panel_t013.rs"]
mod test_context_panel_t013;
#[path = "test_context_panel_t015.rs"]
mod test_context_panel_t015;
#[path = "test_context_panel_t016.rs"]
mod test_context_panel_t016;
#[path = "test_cron_add_positional.rs"]
mod test_cron_add_positional;
#[path = "test_dialog.rs"]
mod test_dialog;
#[path = "test_dirs_show.rs"]
mod test_dirs_show;
#[path = "test_dispatch_ops_deep.rs"]
mod test_dispatch_ops_deep;
#[path = "test_event_handler_more.rs"]
mod test_event_handler_more;
#[path = "test_event_swarm_research_deep.rs"]
mod test_event_swarm_research_deep;
#[path = "test_force_cleanup_modal.rs"]
mod test_force_cleanup_modal;
#[path = "test_gcf_command.rs"]
mod test_gcf_command;
#[path = "test_gcf_indicator.rs"]
mod test_gcf_indicator;
#[path = "test_gcf_slash_registration.rs"]
mod test_gcf_slash_registration;
#[path = "test_govcreate_dispatch.rs"]
mod test_govcreate_dispatch;
#[path = "test_govcreate_poll.rs"]
mod test_govcreate_poll;
#[path = "test_govcreate_registration.rs"]
mod test_govcreate_registration;
#[path = "test_hard_break_lines.rs"]
mod test_hard_break_lines;
#[path = "test_history.rs"]
mod test_history;
#[path = "test_hit_rect_lifecycle.rs"]
mod test_hit_rect_lifecycle;
#[path = "test_hook_warning_flagged.rs"]
mod test_hook_warning_flagged;
#[path = "test_i18n_command.rs"]
mod test_i18n_command;
#[path = "test_init_config.rs"]
mod test_init_config;
#[path = "test_input_deep2.rs"]
mod test_input_deep2;
#[path = "test_input_deep3.rs"]
mod test_input_deep3;
#[path = "test_input_field_extra.rs"]
mod test_input_field_extra;
#[path = "test_input_handler_more.rs"]
mod test_input_handler_more;
#[path = "test_input_more.rs"]
mod test_input_more;
#[path = "test_input_queue.rs"]
mod test_input_queue;
#[path = "test_input_queue_command.rs"]
mod test_input_queue_command;
#[path = "test_input_queue_drain.rs"]
mod test_input_queue_drain;
#[path = "test_input_queue_enqueue.rs"]
mod test_input_queue_enqueue;
#[path = "test_input_queue_error_drain.rs"]
mod test_input_queue_error_drain;
#[path = "test_input_queue_geometry.rs"]
mod test_input_queue_geometry;
#[path = "test_input_queue_guard.rs"]
mod test_input_queue_guard;
#[path = "test_input_queue_menu_binding.rs"]
mod test_input_queue_menu_binding;
#[path = "test_input_queue_menu_clear.rs"]
mod test_input_queue_menu_clear;
#[path = "test_input_queue_menu_clear_confirm.rs"]
mod test_input_queue_menu_clear_confirm;
#[path = "test_input_queue_menu_clear_gating.rs"]
mod test_input_queue_menu_clear_gating;
#[path = "test_input_queue_menu_esc.rs"]
mod test_input_queue_menu_esc;
#[path = "test_input_queue_menu_integration.rs"]
mod test_input_queue_menu_integration;
#[path = "test_input_queue_menu_navigation.rs"]
mod test_input_queue_menu_navigation;
#[path = "test_input_queue_menu_next.rs"]
mod test_input_queue_menu_next;
#[path = "test_input_queue_menu_render.rs"]
mod test_input_queue_menu_render;
#[path = "test_input_queue_menu_stop_resume.rs"]
mod test_input_queue_menu_stop_resume;
#[path = "test_input_queue_show_panel.rs"]
mod test_input_queue_show_panel;
#[path = "test_input_queue_slash.rs"]
mod test_input_queue_slash;
#[path = "test_input_unicode.rs"]
mod test_input_unicode;
#[path = "test_keyboard_control.rs"]
mod test_keyboard_control;
#[path = "test_layout_deep.rs"]
mod test_layout_deep;
#[path = "test_layout_extra.rs"]
mod test_layout_extra;
#[path = "test_lib_schedule_deep.rs"]
mod test_lib_schedule_deep;
#[path = "test_loop_dialog.rs"]
mod test_loop_dialog;
#[path = "test_markdown_table.rs"]
mod test_markdown_table;
#[path = "test_markdown_table_rows.rs"]
mod test_markdown_table_rows;
#[path = "test_mcp_discover_panel.rs"]
mod test_mcp_discover_panel;
#[path = "test_memory_clear_confirm.rs"]
mod test_memory_clear_confirm;
#[path = "test_memory_panel.rs"]
mod test_memory_panel;
#[path = "test_message_edit_seq_cache.rs"]
mod test_message_edit_seq_cache;
#[path = "test_message_pinned_scroll.rs"]
mod test_message_pinned_scroll;
#[path = "test_message_widget_duration_tests.rs"]
mod test_message_widget_duration_tests;
#[path = "test_message_widget_more.rs"]
mod test_message_widget_more;
#[path = "test_message_widget_summaries.rs"]
mod test_message_widget_summaries;
#[path = "test_message_widget_summaries2.rs"]
mod test_message_widget_summaries2;
#[path = "test_message_widget_tests.rs"]
mod test_message_widget_tests;
#[path = "test_misc_modules_deep.rs"]
mod test_misc_modules_deep;
#[path = "test_model_picker_widths.rs"]
mod test_model_picker_widths;
#[path = "test_models_state_misc.rs"]
mod test_models_state_misc;
#[path = "test_ms03_mask_secret.rs"]
mod test_ms03_mask_secret;
#[path = "test_ms04_tui_source_guards.rs"]
mod test_ms04_tui_source_guards;
#[path = "test_new_command.rs"]
mod test_new_command;
#[path = "test_osinfo_command.rs"]
mod test_osinfo_command;
#[path = "test_osinfo_registration.rs"]
mod test_osinfo_registration;
#[path = "test_output_view_footer.rs"]
mod test_output_view_footer;
#[path = "test_output_view_live_steps.rs"]
mod test_output_view_live_steps;
#[path = "test_perf_message_cache.rs"]
mod test_perf_message_cache;
#[path = "test_perf_render_idle.rs"]
mod test_perf_render_idle;
#[path = "test_perf_single_copy_lines.rs"]
mod test_perf_single_copy_lines;
#[path = "test_permission_countdown.rs"]
mod test_permission_countdown;
#[path = "test_plugin_commands.rs"]
mod test_plugin_commands;
#[path = "test_plugin_store_browser.rs"]
mod test_plugin_store_browser;
#[path = "test_plugin_store_category_acceptance.rs"]
mod test_plugin_store_category_acceptance;
#[path = "test_plugin_store_category_filter.rs"]
mod test_plugin_store_category_filter;
#[path = "test_plugin_store_category_launch.rs"]
mod test_plugin_store_category_launch;
#[path = "test_plugin_store_category_mouse.rs"]
mod test_plugin_store_category_mouse;
#[path = "test_plugin_store_category_nav.rs"]
mod test_plugin_store_category_nav;
#[path = "test_plugin_store_default_behavior.rs"]
mod test_plugin_store_default_behavior;
#[path = "test_plugin_store_enrich.rs"]
mod test_plugin_store_enrich;
#[path = "test_plugin_store_failure.rs"]
mod test_plugin_store_failure;
#[path = "test_plugin_store_fetch.rs"]
mod test_plugin_store_fetch;
#[path = "test_plugin_store_filter.rs"]
mod test_plugin_store_filter;
#[path = "test_plugin_store_install.rs"]
mod test_plugin_store_install;
#[path = "test_plugin_store_installed_category.rs"]
mod test_plugin_store_installed_category;
#[path = "test_plugin_store_launch.rs"]
mod test_plugin_store_launch;
#[path = "test_plugin_store_marketplaces.rs"]
mod test_plugin_store_marketplaces;
#[path = "test_plugin_store_panel.rs"]
mod test_plugin_store_panel;
#[path = "test_plugin_store_probe.rs"]
mod test_plugin_store_probe;
#[path = "test_plugin_store_seam.rs"]
mod test_plugin_store_seam;
#[path = "test_plugin_store_state.rs"]
mod test_plugin_store_state;
#[path = "test_plugin_store_stores_report.rs"]
mod test_plugin_store_stores_report;
#[path = "test_plugins_command.rs"]
mod test_plugins_command;
#[path = "test_plugins_list_panel.rs"]
mod test_plugins_list_panel;
#[path = "test_prompt_readonly.rs"]
mod test_prompt_readonly;
#[path = "test_prompt_size_cap.rs"]
mod test_prompt_size_cap;
#[path = "test_prompt_subagent_render.rs"]
mod test_prompt_subagent_render;
#[path = "test_prompt_tool_free.rs"]
mod test_prompt_tool_free;
#[path = "test_prompt_units.rs"]
mod test_prompt_units;
#[path = "test_provider_detection.rs"]
mod test_provider_detection;
#[path = "test_question_multiple_choice.rs"]
mod test_question_multiple_choice;
#[path = "test_render_cost_scaling.rs"]
mod test_render_cost_scaling;
#[path = "test_research_app_more.rs"]
mod test_research_app_more;
#[path = "test_research_clarification.rs"]
mod test_research_clarification;
#[path = "test_research_cluster.rs"]
mod test_research_cluster;
#[path = "test_research_progress_config.rs"]
mod test_research_progress_config;
#[path = "test_research_progress_extra.rs"]
mod test_research_progress_extra;
#[path = "test_research_progress_sanitize.rs"]
mod test_research_progress_sanitize;
#[path = "test_research_swarm_deep.rs"]
mod test_research_swarm_deep;
#[path = "test_research_viewer.rs"]
mod test_research_viewer;
#[path = "test_rollback_flow.rs"]
mod test_rollback_flow;
#[path = "test_router_save_dialog.rs"]
mod test_router_save_dialog;
#[path = "test_router_setup.rs"]
mod test_router_setup;
#[path = "test_row_action_confirm.rs"]
mod test_row_action_confirm;
#[path = "test_row_action_dispatch.rs"]
mod test_row_action_dispatch;
#[path = "test_row_click_destinations.rs"]
mod test_row_click_destinations;
#[path = "test_run_cost_banner.rs"]
mod test_run_cost_banner;
#[path = "test_scrolling.rs"]
mod test_scrolling;
#[path = "test_security_command.rs"]
mod test_security_command;
#[path = "test_selectable_list.rs"]
mod test_selectable_list;
#[path = "test_session_events_deep.rs"]
mod test_session_events_deep;
#[path = "test_session_models_state_deep.rs"]
mod test_session_models_state_deep;
#[path = "test_session_ops_more.rs"]
mod test_session_ops_more;
#[path = "test_session_resume.rs"]
mod test_session_resume;
#[path = "test_setup_dialog_chrome.rs"]
mod test_setup_dialog_chrome;
#[path = "test_setup_selector_scroll.rs"]
mod test_setup_selector_scroll;
#[path = "test_shorten_middle.rs"]
mod test_shorten_middle;
#[path = "test_skill_command.rs"]
mod test_skill_command;
#[path = "test_slash_arms_deep.rs"]
mod test_slash_arms_deep;
#[path = "test_slash_codeindex.rs"]
mod test_slash_codeindex;
#[path = "test_slash_codeindex_deep.rs"]
mod test_slash_codeindex_deep;
#[path = "test_slash_commands.rs"]
mod test_slash_commands;
#[path = "test_slash_config_write.rs"]
mod test_slash_config_write;
#[path = "test_slash_deep2.rs"]
mod test_slash_deep2;
#[path = "test_slash_heavy_deep.rs"]
mod test_slash_heavy_deep;
#[path = "test_slash_help.rs"]
mod test_slash_help;
#[path = "test_slash_info.rs"]
mod test_slash_info;
#[path = "test_slash_menu_escape.rs"]
mod test_slash_menu_escape;
#[path = "test_slash_misc.rs"]
mod test_slash_misc;
#[path = "test_slash_misc_deep2.rs"]
mod test_slash_misc_deep2;
#[path = "test_slash_router.rs"]
mod test_slash_router;
#[path = "test_slash_session.rs"]
mod test_slash_session;
#[path = "test_slash_spec.rs"]
mod test_slash_spec;
#[path = "test_slash_spec_deep.rs"]
mod test_slash_spec_deep;
#[path = "test_slash_spec_deep2.rs"]
mod test_slash_spec_deep2;
#[path = "test_slash_teams_deep.rs"]
mod test_slash_teams_deep;
#[path = "test_slash_tools.rs"]
mod test_slash_tools;
#[path = "test_spec_jtbd.rs"]
mod test_spec_jtbd;
#[path = "test_status_expiry.rs"]
mod test_status_expiry;
#[path = "test_statusbar_last_prompt.rs"]
mod test_statusbar_last_prompt;
#[path = "test_statusbar_layout.rs"]
mod test_statusbar_layout;
#[path = "test_stop_confirm_dialog.rs"]
mod test_stop_confirm_dialog;
#[path = "test_stream_metrics.rs"]
mod test_stream_metrics;
#[path = "test_subagent_events.rs"]
mod test_subagent_events;
#[path = "test_subagent_step_log.rs"]
mod test_subagent_step_log;
#[path = "test_swarm_more.rs"]
mod test_swarm_more;
#[path = "test_tasks_panel.rs"]
mod test_tasks_panel;
#[path = "test_teammate_strip.rs"]
mod test_teammate_strip;
#[path = "test_teams_tui.rs"]
mod test_teams_tui;
#[path = "test_teams_window_frame.rs"]
mod test_teams_window_frame;
#[path = "test_teams_window_table.rs"]
mod test_teams_window_table;
#[path = "test_telemetry_slash.rs"]
mod test_telemetry_slash;
#[path = "test_text_selection.rs"]
mod test_text_selection;
#[path = "test_think_narration_scrub.rs"]
mod test_think_narration_scrub;
#[path = "test_thinking_defaults.rs"]
mod test_thinking_defaults;
#[path = "test_thinking_selector.rs"]
mod test_thinking_selector;
#[path = "test_tool_display.rs"]
mod test_tool_display;
#[path = "test_tool_event_state.rs"]
mod test_tool_event_state;
#[path = "test_tools_visibility_command.rs"]
mod test_tools_visibility_command;
#[path = "test_widgets_and_events_extra.rs"]
mod test_widgets_and_events_extra;
#[path = "test_window_lifecycle.rs"]
mod test_window_lifecycle;
#[path = "test_window_routing.rs"]
mod test_window_routing;
#[path = "test_window_scroll_wheel.rs"]
mod test_window_scroll_wheel;
