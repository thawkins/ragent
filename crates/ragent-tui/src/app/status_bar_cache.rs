//! Status-bar span cache (FR-003, FR-008).
//!
//! The status bar is rendered every frame, but its content only changes when
//! one of a small set of inputs changes (status string, agent name, model,
//! working directory, git branch, task count, processing flag, etc.).
//!
//! This module provides [`StatusBarCache`] which stores the two pre-built
//! `Vec<Span<'static>>` lines alongside a signature of the inputs they were
//! built from.  On each render the caller compares the current inputs to the
//! cached signature; if they match, the cached spans are reused directly,
//! avoiding per-frame `format!()` and `String::clone()` allocations.

use ratatui::text::Span;

/// Cached status-bar lines and the signature they were built from.
///
/// The signature is a tuple of all inputs that affect the status bar content.
/// When `signature_matches` returns `true`, the caller can reuse `line1` and
/// `line2` without rebuilding them.
#[derive(Clone)]
pub struct StatusBarCache {
    /// Pre-built top status line (session / agent / cwd / git / status).
    pub line1: Vec<Span<'static>>,
    /// Pre-built bottom status line (model / tokens / tasks / index / log).
    pub line2: Vec<Span<'static>>,

    // ── Signature fields ──────────────────────────────────────────────────
    /// Current status text used to build line1.
    pub status: String,
    /// Current agent name displayed in line1.
    pub agent_name: String,
    /// Currently selected model identifier, if any.
    pub selected_model: Option<String>,
    /// Current working directory displayed in line1.
    pub cwd: String,
    /// Current git branch name displayed in line1.
    pub git_branch: String,
    /// Number of active session tasks shown in line2.
    pub active_tasks_len: usize,
    /// Number of background tasks shown in line2.
    pub bg_tasks_len: usize,
    /// Whether a model response is currently in flight.
    pub is_processing: bool,
    /// Whether the code index is currently enabled.
    pub code_index_enabled: bool,
    /// Whether the log panel is visible.
    pub show_log: bool,
    /// Whether the agents window is visible.
    pub show_agents_window: bool,
    /// Whether the teams window is visible.
    pub show_teams_window: bool,
    /// Whether the tasks panel is visible.
    pub show_tasks_panel: bool,
    /// Whether the memory panel is visible.
    pub show_memory: bool,
    /// Whether the profile panel is visible.
    pub show_profile: bool,
    /// Whether the telemetry panel is visible.
    pub show_telemetry: bool,
    /// Whether the model list is currently loading.
    pub model_loading: bool,
    /// Latest provider health check result, if known.
    pub provider_health: Option<bool>,
    /// Terminal width used for layout.
    pub width: u16,
}

impl StatusBarCache {
    /// Returns `true` when every signature field matches the provided values,
    /// meaning the cached lines are still valid and can be reused.
    #[allow(clippy::fn_params_excessive_bools)]
    pub fn signature_matches(
        &self,
        status: &str,
        agent_name: &str,
        selected_model: Option<&str>,
        cwd: &str,
        git_branch: &str,
        active_tasks_len: usize,
        bg_tasks_len: usize,
        is_processing: bool,
        code_index_enabled: bool,
        show_log: bool,
        show_agents_window: bool,
        show_teams_window: bool,
        show_tasks_panel: bool,
        show_memory: bool,
        show_profile: bool,
        show_telemetry: bool,
        model_loading: bool,
        provider_health: Option<bool>,
        width: u16,
    ) -> bool {
        self.status == status
            && self.agent_name == agent_name
            && self.selected_model.as_deref() == selected_model
            && self.cwd == cwd
            && self.git_branch == git_branch
            && self.active_tasks_len == active_tasks_len
            && self.bg_tasks_len == bg_tasks_len
            && self.is_processing == is_processing
            && self.code_index_enabled == code_index_enabled
            && self.show_log == show_log
            && self.show_agents_window == show_agents_window
            && self.show_teams_window == show_teams_window
            && self.show_tasks_panel == show_tasks_panel
            && self.show_memory == show_memory
            && self.show_profile == show_profile
            && self.show_telemetry == show_telemetry
            && self.model_loading == model_loading
            && self.provider_health == provider_health
            && self.width == width
    }
}
