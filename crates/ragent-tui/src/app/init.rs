//! `App::new` and provider-setup helpers for the TUI.
use std::collections::{HashMap, VecDeque};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::Instant;

use lru::LruCache;

use ratatui::layout::Rect;

use ragent_agent::{
    StartupTimings, agent::AgentInfo, event::EventBus, provider::ProviderRegistry,
    session::processor::SessionProcessor, storage::Storage,
};

use crate::tips;

// State types from app/state.rs
use crate::app::state::{App, LogLevel, ScreenMode};

// Theme status types
use crate::theme::StatusHistory;

impl App {
    /// Collapse `$HOME` at the front of `path` to `~` for display.
    ///
    /// The inverse of [`expand_home_path`]; used wherever a real path is
    /// rendered to the user. Paths are never collapsed for filesystem use.
    pub(crate) fn collapse_home_path(path: &std::path::Path) -> String {
        let s = path.display().to_string();
        if let Some(home) = std::env::var_os("HOME") {
            let home = home.to_string_lossy();
            if let Some(rest) = s.strip_prefix(home.as_ref()) {
                return format!("~{rest}");
            }
        }
        s
    }

    /// Expand a leading `~` to `$HOME` and return the real filesystem path.
    ///
    /// Defensive guard for display-derived strings (restored sessions,
    /// persisted settings): a literal `~/...` value joined onto the process
    /// cwd would create a bogus `~/` directory tree on disk.
    pub(crate) fn expand_home_path(path: &std::path::Path) -> std::path::PathBuf {
        let s = path.display().to_string();
        if let Some(rest) = s.strip_prefix("~") {
            if rest.starts_with('/') || rest.is_empty() {
                if let Some(home) = std::env::var_os("HOME") {
                    if !home.is_empty() {
                        return std::path::PathBuf::from(home).join(rest.trim_start_matches('/'));
                    }
                }
            }
        }
        path.to_path_buf()
    }
    /// Construct a new `App` instance wired to the supplied event bus,
    /// storage, provider registry, and session processor.
    ///
    /// Resolves the current working directory (collapsing `$HOME` to `~`),
    /// seeds the initial agent info, and prepares default UI state.
    pub fn new(
        event_bus: Arc<EventBus>,
        storage: Arc<Storage>,
        provider_registry: Arc<ProviderRegistry>,
        session_processor: Arc<SessionProcessor>,
        mut agent_info: AgentInfo,
        show_log: bool,
        db_path: std::path::PathBuf,
    ) -> Self {
        let mut sub = StartupTimings::new();

        let cwd_path = std::env::current_dir().unwrap_or_default();
        let cwd = Self::collapse_home_path(&cwd_path);

        let t0 = Instant::now();
        let git_branch = Self::detect_git_branch(&cwd_path);
        sub.record("App: git branch detect", t0.elapsed());

        let t0 = Instant::now();
        let configured_provider = Self::detect_provider(&storage);
        // Best-effort startup cleanup; failure leaves stale rows only.
        if let Err(e) = storage.delete_discovered_models("huggingface") {
            tracing::warn!(error = %e, "stale huggingface model cleanup failed");
        }
        sub.record("App: provider detect", t0.elapsed());

        // Ensure the initial agent_info has a model if none was provided.
        let t0 = Instant::now();
        if agent_info.model.is_none() {
            if let Some(model_ref) =
                ragent_agent::agent::resolve_default_model(&agent_info, &provider_registry)
            {
                tracing::info!(
                    agent = %agent_info.name,
                    provider = %model_ref.provider_id,
                    model = %model_ref.model_id,
                    "Auto-assigned default model to initial agent in TUI"
                );
                agent_info.model = Some(model_ref);
            }
        }
        sub.record("App: model resolution", t0.elapsed());

        let agent_name = agent_info.name.clone();

        let log_window_path = {
            let log_dir = crate::app::helpers::log_root().join("logwindow");
            // Best-effort directory creation; failure only disables the
            // per-session log window file.
            if let Err(e) = std::fs::create_dir_all(&log_dir) {
                tracing::debug!(error = %e, "log window directory creation failed");
            }
            let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");
            Some(log_dir.join(format!("logwindow-{ts}.log")))
        };

        let t0 = Instant::now();
        let builtin_agents = ragent_agent::agent::create_builtin_agents();
        sub.record("App: builtin agents", t0.elapsed());

        let t0 = Instant::now();
        let builtin_names: std::collections::HashSet<String> =
            builtin_agents.iter().map(|a| a.name.clone()).collect();

        let (custom_defs, mut all_diagnostics) =
            ragent_agent::agent::custom::load_custom_agents(&cwd_path);
        sub.record("App: custom agent loading", t0.elapsed());

        let cycleable_agents: Vec<AgentInfo> = builtin_agents
            .into_iter()
            .filter(|a| !a.hidden)
            .chain(
                custom_defs
                    .iter()
                    .filter(|d| !d.agent_info.hidden)
                    .map(|d| {
                        let mut info = d.agent_info.as_ref().clone();
                        if builtin_names.contains(&info.name) {
                            let new_name = format!("custom:{}", info.name);
                            all_diagnostics.push(format!(
                          "custom agent '{}' collides with a built-in agent name - loaded as '{}'",
                          info.name, new_name
                      ));
                            info.name = new_name;
                        }
                        info
                    }),
            )
            .collect();
        let current_agent_index = cycleable_agents
            .iter()
            .position(|a| a.name == agent_info.name)
            .unwrap_or(0);

        // Pre-format the custom-agent warning log entries so
        // `all_diagnostics` can be moved (not cloned) into the struct below.
        let agent_diag_logs: Vec<String> = all_diagnostics
            .iter()
            .map(|diag| format!("[custom agents] {}", diag))
            .collect();

        // Load persisted model selection
        let t0 = Instant::now();
        let app_config = match ragent_agent::Config::load() {
            Ok(cfg) => cfg,
            Err(e) => {
                tracing::warn!(error = %e, "config load failed, using defaults");
                Default::default()
            }
        };
        // FR-028: report a hook whose `matcher` names only a retired tool
        // spelling. The interactive surface's load is here - immediately after
        // the configuration is read and before any turn runs - so the warning
        // is in the log window before the first prompt, which is what TC-023
        // step 4 asks for. The matcher is not rewritten: the file belongs to
        // its author, and a load-time rewrite would leave the on-disk text
        // disagreeing with the running session.
        let _reported_stale_matchers = ragent_agent::hooks::report_retired_matcher_spellings_once(
            &ragent_agent::hooks::parse_hook_configs(&app_config.hooks),
        );
        sub.record("App: config reload", t0.elapsed());

        let t0 = Instant::now();
        let selected_model = storage.get_setting("selected_model").ok().flatten();
        let selected_model_ctx_window = storage
            .get_setting("selected_model_ctx_window")
            .ok()
            .flatten()
            .and_then(|s| s.parse::<usize>().ok());
        let selected_thinking_level = Self::load_persisted_thinking_level(storage.as_ref());
        sub.record("App: settings load", t0.elapsed());
        let t0 = Instant::now();
        let sidebar = crate::sessions_sidebar::SidebarState::load(&storage);
        let mut app = Self {
            session: {
                let mut session =
                    crate::app::SessionView::new(agent_name, agent_info, current_agent_index);
                session.directory = cwd_path.clone();
                session.selected_model = selected_model;
                session.selected_model_ctx_window = selected_model_ctx_window;
                session.selected_thinking_level = selected_thinking_level;
                session.active_backend = app_config
                    .effective_execution_backend()
                    .as_str()
                    .to_string();
                session
            },
            background: crate::app::SessionRegistry::default(),
            sidebar,
            request_counts: Arc::new(ragent_agent::project::requests::PullRequestCounts::new(
                Arc::clone(&storage),
                Arc::clone(&event_bus),
            )),
            pending_project_sessions: Arc::new(std::sync::Mutex::new(Vec::new())),
            last_request_sweep: None,
            worktree_root: None,
            project_services: std::collections::HashMap::new(),
            bench_origin_session: None,
            newproj_origin_session: None,
            govcreate_origin_session: None,
            is_running: true,
            event_bus,
            storage,
            status: "ready".to_string(),
            status_set_at: None,
            status_snapshot: String::new(),
            quota_percent: None,
            model_loading_state: None,
            model_download_state: None,
            current_screen: ScreenMode::Chat,
            tip: tips::random_tip(),
            cwd,
            plugin_commands: crate::app::state::plugin_commands_for_surface(&cwd_path),
            cwd_path: cwd_path.clone(),
            git_branch,
            provider_setup: None,
            provider_setup_area: Rect::default(),
            configured_provider,
            provider_registry,
            session_processor,
            cycleable_agents,
            slash_menu: None,
            cached_skill_registry: None,
            file_menu: None,
            file_menu_show_hidden: false,
            project_files_cache: None,
            project_files_cache_cwd: None,
            project_files_cache_refreshed_at: None,
            project_files_cache_count: 0,
            input_history: Vec::new(),
            input_queue_capacity: app_config.effective_input_queue_capacity(),
            queue_menu_open: false,
            queue_menu_selected: 0,
            queue_menu_area: Rect::default(),
            queue_show_open: false,
            queue_show_selected: 0,
            queue_show_area: Rect::default(),
            queue_clear_confirm_open: false,
            queue_clear_confirm_selected: crate::app::QUEUE_CLEAR_CONFIRM_NO,
            queue_clear_confirm_area: Rect::default(),
            memory_clear_confirm_open: false,
            memory_clear_confirm_selected: crate::app::MEMORY_CLEAR_CONFIRM_NO,
            db_prune_confirm_open: false,
            db_prune_confirm_selected: crate::app::DB_PRUNE_CONFIRM_NO,
            db_prune_confirm_days: None,
            db_retention_days: app_config.storage.retention.message_days,
            plugin_store: None,
            plugin_store_result: Arc::new(std::sync::Mutex::new(None)),
            plugin_store_install_result: Arc::new(std::sync::Mutex::new(None)),
            plugin_store_probe_result: Arc::new(std::sync::Mutex::new(None)),
            plugin_store_fetcher: ragent_plugins::default_fetcher(),
            store_manifest_source: std::sync::Arc::new(ragent_plugins::NetworkManifestSource::new()),
            store_enrich_result: Arc::new(std::sync::Mutex::new(Vec::new())),
            store_enrich_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            plugin_store_area: Rect::default(),
            plugin_store_nav_area: Rect::default(),
            plugin_store_marketplace_area: Rect::default(),
            plugin_store_result_area: Rect::default(),
            plugin_store_search_area: Rect::default(),
            plugin_store_clear_area: Rect::default(),
            connector_store: None,
            connector_catalogue_result: Arc::new(std::sync::Mutex::new(None)),
            connector_catalogue_install_result: Arc::new(std::sync::Mutex::new(None)),
            catalogue_manifest_source: std::sync::Arc::new(
                ragent_connectors::catalogue_enrich::NetworkManifestSource::new(),
            ),
            connector_enrich_result: Arc::new(std::sync::Mutex::new(Vec::new())),
            connector_enrich_generation: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            connector_store_area: Rect::default(),
            connector_store_nav_area: Rect::default(),
            connector_store_marketplace_area: Rect::default(),
            connector_store_result_area: Rect::default(),
            connector_store_search_area: Rect::default(),
            connector_store_clear_area: Rect::default(),
            backend_panel: None,
            backend_panel_area: Rect::default(),
            show_log,
            show_profile: false,
            show_tasks_panel: false,
            show_memory: false,
            show_telemetry: false,
            show_context_panel: false,
            log_entries: Vec::new(),
            log_line_cache: Vec::new(),
            log_cache_width: 0,
            log_seq: 0,
            withheld_log_exports: 0,
            log_window_path,
            log_scroll_offset: 0,
            profile_scroll_offset: 0,
            tasks_scroll_offset: 0,
            memory_scroll_offset: 0,
            telemetry_scroll_offset: 0,
            message_area: Rect::default(),
            log_area: Rect::default(),
            profile_area: Rect::default(),
            tasks_area: Rect::default(),
            memory_area: Rect::default(),
            telemetry_area: Rect::default(),
            context_panel_area: Rect::default(),
            log_max_scroll: 0,
            profile_max_scroll: 0,
            tasks_max_scroll: 0,
            memory_max_scroll: 0,
            telemetry_max_scroll: 0,
            context_panel_max_scroll: 0,
            context_scroll_offset: 0,
            active_agents_scroll_offset: 0,
            active_agents_max_scroll: 0,
            active_agents_area: Rect::default(),
            agents_cursor_subagent: None,
            agents_subagent_task_ids: Vec::new(),
            agents_row_task_ids: Vec::new(),
            active_agents_custom_names: std::collections::HashSet::new(),
            active_agents_custom_names_version: 0,
            active_agents_teammate_ids: std::collections::HashSet::new(),
            active_agents_teammate_ids_version: 0,
            scrollbar_drag: None,
            text_selection: None,
            log_content_lines: Vec::new(),
            profile_content_lines: Vec::new(),
            tasks_content_lines: Vec::new(),
            memory_content_lines: Vec::new(),
            memory_cache_entries: Vec::new(),
            memory_cache_count: 0,
            memory_cache_dirty: true,
            memory_cursor: 0,
            memory_row_line_indices: Vec::new(),
            memory_row_count: 0,
            memory_view: None,
            pending_memory_delete: None,
            telemetry_content_lines: Vec::new(),
            context_content_lines: Vec::new(),
            input_area: Rect::default(),
            teams_area: Rect::default(),
            teams_rows_area: Rect::default(),
            output_view_area: Rect::default(),
            research_view_area: Rect::default(),
            memory_view_area: Rect::default(),
            agents_button_area: Rect::default(),
            teams_button_area: Rect::default(),
            show_agents_window: false,
            show_teams_window: false,
            mcp_servers: crate::app::slash::mcp_display_servers(
                &[],
                &app_config.mcp,
                &cwd_path,
                &std::collections::HashMap::new(),
            ),
            mcp_status_map: std::collections::HashMap::new(),
            mcp_client_adopted: false,
            mcp_startup_reported: false,
            mcp_enabled_map: ragent_agent::mcp::McpEnableLedger::load().to_map(),
            mcp_discover: None,
            plugins_list: None,
            loop_setup: None,
            loop_setup_draft: None,
            tool_step_map: HashMap::new(),
            pending_tool_args: HashMap::new(),
            last_step_per_session: HashMap::new(),
            substep_counter_per_session: HashMap::new(),
            active_tasks: Vec::new(),
            agents_history: VecDeque::new(),
            agents_history_cap: app_config.effective_agents_completed_history(),
            tui_event_lag: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            seen_event_lag: 0,
            active_tasks_reconcile_result: Arc::new(std::sync::Mutex::new(None)),
            active_tasks_reconcile_last: std::time::Instant::now(),
            active_tasks_reconcile_inflight: false,
            bg_tasks: Vec::new(),
            show_shortcuts: false,
            quit_armed: false,
            context_menu: None,
            history_file_path: None,
            history_picker: None,
            config_save_picker: None,
            selected_agent_index: None,
            custom_agent_defs: custom_defs,
            custom_agent_diagnostics: all_diagnostics,
            show_teams: false,
            teams_scroll_offset: 0,
            teams_max_scroll: 0,
            teams_cursor_teammate: None,
            teams_teammate_agent_ids: Vec::new(),
            teams_row_agent_ids: Vec::new(),
            subagent_launches: Vec::new(),
            subagent_launch_results: Arc::new(std::sync::Mutex::new(Vec::new())),
            subagent_launch_cap: app_config.effective_agents_max_launching(),
            next_subagent_launch_id: 1,
            bench_result: Arc::new(std::sync::Mutex::new(None)),
            output_view: None,
            research_view: None,
            active_bench_task_id: None,
            active_bench_summary: None,
            active_bench_started_at: None,
            active_bench_cancel: None,
            active_bench_progress: None,
            bench_last_summary: None,
            bench_last_workbooks: Vec::new(),
            bench_last_finished_at: None,
            bench_mock_outputs: None,
            newproj_progress: Arc::new(std::sync::Mutex::new(Vec::new())),
            newproj_progress_text: None,
            newproj_progress_slug: None,
            newproj_result: Arc::new(std::sync::Mutex::new(None)),
            newproj_running: false,
            govcreate_progress: Arc::new(std::sync::Mutex::new(Vec::new())),
            govcreate_progress_text: None,
            govcreate_progress_slug: None,
            govcreate_result: Arc::new(std::sync::Mutex::new(None)),
            govcreate_running: false,
            govcreate_cancel: Arc::new(std::sync::Mutex::new(None)),
            websearch_test_result: Arc::new(std::sync::Mutex::new(None)),
            db_path,
            history_dirty: false,
            history_save_deadline: None,
            md_render_cache: LruCache::new(NonZeroUsize::new(256).expect("256 > 0")),
            md_worker: crate::app::MdWorker::new(),
            status_bar_cache: None,
            model_picker_rows_cache: None,
            jobs_last_poll: std::time::Instant::now(),
            housekeeping_runs: 0,
            sid_to_display_name: HashMap::new(),
            next_agent_index: 1,
            webapi_server: None,
            webapi_addr: ragent_server::DEFAULT_LISTEN_ADDR.to_string(),
            webapi_token: None,
            memory_entry_count: 0,
            memory_entry_count_pending: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            memory_last_updated: None,
            memory_stats_last_refresh: std::time::Instant::now(),
            theme_mode: crate::theme::ThemeMode::Default,
            mouse_enabled: true,
            status_history: StatusHistory::new(),
            needs_redraw: true,
            code_index: None,
            code_index_enabled: app_config.code_index.enabled,
            code_index_stats_cache: None,
            code_index_stats_last_refresh: std::time::Instant::now(),
            code_index_busy: false,
            code_index_graph_busy: false,
            code_index_graph_spawned: false,
            code_index_reindex_spawned: false,
            code_index_bg_result: Arc::new(std::sync::Mutex::new(None)),
            code_index_watch_session: None,
            spec_manager: None,
            startup_timings: Some(sub),
            config_paths: app_config.config_paths.clone(),
            router_enabled: false,
            router_draft_config: None,
            router_draft_providers: Vec::new(),
            router_draft_selected_ids: Vec::new(),
            pending_router_save: None,
            row_action_confirm: None,
            row_action_confirm_area: Rect::default(),
            row_action_confirm_yes_area: Rect::default(),
            row_action_confirm_no_area: Rect::default(),
            research_progress: Vec::new(),
            research_live_runs: std::sync::Arc::default(),
            skill_registry_cache: None,
            skill_dirs_cache: Vec::new(),
            skill_registry_last_refresh: std::time::Instant::now(),
            trigger_runtime: None,
        }; // end Self { ... }
        if let Some(ref mut st) = app.startup_timings {
            st.record("App: struct init", t0.elapsed());
        }
        // Log any warnings from custom agent loading into the log panel
        for msg in agent_diag_logs {
            app.push_log_no_agent(LogLevel::Warn, msg);
        }

        let t0 = Instant::now();
        // Initialise the bash allowlist/denylist from config
        ragent_agent::bash_lists::load_from_config();

        // Initialise the directory allowlist/denylist from config
        ragent_agent::dir_lists::load_from_config();

        // Migrate legacy file-based GitLab credentials into the database
        ragent_agent::gitlab::auth::migrate_legacy_files(app.storage.as_ref());

        // Restore the Model Router as the active provider if it was the
        // persisted current model when the previous session exited.
        app.restore_router_state();

        if let Some(ref mut st) = app.startup_timings {
            st.record(
                "App: post-init (bash/dir lists, gitlab, router)",
                t0.elapsed(),
            );
        }

        app
    }
}
