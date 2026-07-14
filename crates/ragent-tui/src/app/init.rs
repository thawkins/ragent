//! `App::new` and provider-setup helpers for the TUI.
use std::collections::{HashMap, VecDeque};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::sync::atomic::AtomicU8;

use lru::LruCache;

use ratatui::layout::Rect;

use ragent_agent::{
    agent::AgentInfo, event::EventBus, provider::ProviderRegistry,
    session::processor::SessionProcessor, storage::Storage,
};

use crate::tips;

// Prompt optimization templates

// State types from app/state.rs
use crate::app::state::{App, LogLevel, ScreenMode};

// Helpers

// Re-export status types from theme
use crate::theme::StatusHistory;

impl App {
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
        let cwd = std::env::current_dir()
            .map(|p| {
                let path = p.display().to_string();
                if let Some(home) = std::env::var_os("HOME") {
                    let home = home.to_string_lossy();
                    if let Some(rest) = path.strip_prefix(home.as_ref()) {
                        return format!("~{rest}");
                    }
                }
                path
            })
            .unwrap_or_default();

        let git_branch = Self::detect_git_branch();

        let configured_provider = Self::detect_provider(&storage);
        let _ = storage.delete_discovered_models("huggingface");

        // Ensure the initial agent_info has a model if none was provided.
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

        let agent_name = agent_info.name.clone();

        let cwd_path = std::env::current_dir().unwrap_or_default();
        let builtin_agents = ragent_agent::agent::create_builtin_agents();
        let builtin_names: std::collections::HashSet<String> =
            builtin_agents.iter().map(|a| a.name.clone()).collect();

        let (custom_defs, mut all_diagnostics) =
            ragent_agent::agent::custom::load_custom_agents(&cwd_path);

        let cycleable_agents: Vec<AgentInfo> = builtin_agents
            .into_iter()
            .filter(|a| !a.hidden)
            .chain(
                custom_defs
                    .iter()
                    .filter(|d| !d.agent_info.hidden)
                    .map(|d| {
                        let mut info = d.agent_info.clone();
                        if builtin_names.contains(&info.name) {
                            let new_name = format!("custom:{}", info.name);
                            all_diagnostics.push(format!(
                        "custom agent '{}' collides with a built-in agent name — loaded as '{}'",
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

        // Load persisted model selection
        let app_config = ragent_agent::Config::load().unwrap_or_default();
        let selected_model = storage.get_setting("selected_model").ok().flatten();
        let selected_model_ctx_window = storage
            .get_setting("selected_model_ctx_window")
            .ok()
            .flatten()
            .and_then(|s| s.parse::<usize>().ok());
        let selected_thinking_level = Self::load_persisted_thinking_level(storage.as_ref());

        let mut app = Self {
            messages: Vec::new(),
            input: String::new(),
            input_cursor: 0,
            kb_select_anchor: None,
            scroll_offset: 0,
            is_running: true,
            event_bus,
            storage,
            session_id: None,
            agent_name,
            status: "ready".to_string(),
            status_set_at: None,
            status_snapshot: String::new(),
            permission_queue: VecDeque::new(),
            question_queue: VecDeque::new(),
            pending_question_input: String::new(),
            question_selected_index: 0,
            token_usage: (0, 0),
            llm_request_stats: Vec::new(),
            last_input_tokens: 0,
            stream_in_bytes: 0,
            stream_out_bytes: 0,
            quota_percent: None,
            model_loading_state: None,
            model_download_state: None,
            tool_visibility: app_config.tool_visibility.clone(),
            current_screen: ScreenMode::Chat,
            tip: tips::random_tip(),
            cwd,
            shell_cwd: None,
            git_branch,
            provider_setup: None,
            configured_provider,
            provider_registry,
            selected_model,
            selected_model_ctx_window,
            selected_thinking_level,
            session_processor,
            agent_info,
            cycleable_agents,
            current_agent_index,
            provider_health: Arc::new(AtomicU8::new(0)),
            slash_menu: None,
            file_menu: None,
            file_menu_show_hidden: false,
            project_files_cache: None,
            project_files_cache_cwd: None,
            project_files_cache_refreshed_at: None,
            project_files_cache_count: 0,
            input_history: Vec::new(),
            history_index: None,
            history_draft: String::new(),
            show_log,
            show_profile: false,
            show_todo: false,
            show_memory: false,
            log_entries: Vec::new(),
            log_scroll_offset: 0,
            profile_scroll_offset: 0,
            todo_scroll_offset: 0,
            memory_scroll_offset: 0,
            message_area: Rect::default(),
            log_area: Rect::default(),
            profile_area: Rect::default(),
            todo_area: Rect::default(),
            memory_area: Rect::default(),
            message_max_scroll: 0,
            log_max_scroll: 0,
            profile_max_scroll: 0,
            todo_max_scroll: 0,
            memory_max_scroll: 0,
            active_agents_scroll_offset: 0,
            active_agents_max_scroll: 0,
            active_agents_area: Rect::default(),
            agent_row_button_areas: Vec::new(),
            agent_row_button_task_ids: Vec::new(),
            agent_row_kill_areas: Vec::new(),
            agent_row_kill_task_ids: Vec::new(),
            scrollbar_drag: None,
            text_selection: None,
            message_content_lines: Vec::new(),
            log_content_lines: Vec::new(),
            profile_content_lines: Vec::new(),
            todo_content_lines: Vec::new(),
            memory_content_lines: Vec::new(),
            input_area: Rect::default(),
            teams_area: Rect::default(),
            output_view_area: Rect::default(),
            research_view_area: Rect::default(),
            agents_button_area: Rect::default(),
            teams_button_area: Rect::default(),
            show_agents_window: false,
            show_teams_window: false,
            agents_close_button_area: Rect::default(),
            teams_close_button_area: Rect::default(),
            mcp_servers: Vec::new(),
            mcp_discover: None,
            force_new_message: false,
            agent_stack: Vec::new(),
            pending_plan_task: None,
            pending_plan_restore: None,
            pending_forcecleanup: None,
            is_processing: false,
            cancel_flag: None,
            auto_compact_in_progress: false,
            compact_in_progress: false,
            compress_in_progress: false,
            auto_compact_failed: false,
            pending_send_after_compact: None,
            agent_halted: false,
            tool_step_map: HashMap::new(),
            pending_tool_args: HashMap::new(),
            last_step_per_session: HashMap::new(),
            substep_counter_per_session: HashMap::new(),
            active_tasks: Vec::new(),
            show_shortcuts: false,
            quit_armed: false,
            context_menu: None,
            pending_attachments: Vec::new(),
            history_file_path: None,
            history_picker: None,
            selected_agent_session_id: None,
            selected_agent_index: None,
            custom_agent_defs: custom_defs,
            custom_agent_diagnostics: all_diagnostics.clone(),
            active_team: None,
            team_members: Vec::new(),
            team_message_counts: HashMap::new(),
            show_teams: false,
            teams_scroll_offset: 0,
            teams_max_scroll: 0,
            team_row_button_areas: Vec::new(),
            team_row_button_agent_ids: Vec::new(),
            team_row_kill_areas: Vec::new(),
            team_row_kill_agent_ids: Vec::new(),
            focused_teammate: None,
            swarm_state: None,
            swarm_result: Arc::new(std::sync::Mutex::new(None)),
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
            opt_result: Arc::new(std::sync::Mutex::new(None)),
            db_path,
            history_dirty: false,
            history_save_deadline: None,
            md_render_cache: LruCache::new(NonZeroUsize::new(256).unwrap()),
            autopilot_enabled: false,
            autopilot_token_budget: None,
            autopilot_time_limit_secs: None,
            autopilot_started_at: None,
            autopilot_pending_continue: None,
            spec_impl_state: None,
            sid_to_display_name: HashMap::new(),
            next_agent_index: 1,
            prompt_start_time: None,
            tool_time_ms: 0,
            llm_time_ms: 0,
            plan_approval_pending: None,
            role_mode: None,
            webapi_server: None,
            webapi_addr: "127.0.0.1:3000".to_string(),
            webapi_token: None,
            memory_browser: None,
            memory_browser_close_area: Rect::default(),
            memory_browser_area: Rect::default(),
            memory_block_count: 0,
            memory_entry_count: 0,
            memory_last_updated: None,
            memory_stats_last_refresh: std::time::Instant::now(),
            swarm_unblock_last_poll: std::time::Instant::now(),
            swarm_completion_last_poll: std::time::Instant::now(),
            theme_mode: crate::theme::ThemeMode::Default,
            mouse_enabled: true,
            status_history: StatusHistory::new(),
            needs_redraw: true,
            code_index: None,
            code_index_enabled: app_config.code_index.enabled,
            code_index_stats_cache: None,
            code_index_stats_last_refresh: std::time::Instant::now(),
            code_index_busy: false,
            code_index_watch_session: None,
            spec_manager: None,
            active_spec: None,
            config_paths: app_config.config_paths.clone(),
            router_enabled: false,
            router_current_tier: None,
            router_draft_config: None,
            router_draft_providers: Vec::new(),
            router_draft_selected_ids: Vec::new(),
            research_progress: Vec::new(),
        }; // end Self { ... }
        // Log any warnings from custom agent loading into the log panel
        for diag in &all_diagnostics {
            app.push_log_no_agent(LogLevel::Warn, format!("[custom agents] {}", diag));
        }

        // Initialise the bash allowlist/denylist from config
        ragent_agent::bash_lists::load_from_config();

        // Initialise the directory allowlist/denylist from config
        ragent_agent::dir_lists::load_from_config();

        // Migrate legacy file-based GitLab credentials into the database
        ragent_agent::gitlab::auth::migrate_legacy_files(app.storage.as_ref());

        app
    }
}
