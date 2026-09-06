//! Status bar rendering engine with modular 3-section layout.
//!
//! This module provides a clean, responsive status bar design with semantic
//! color coding and adaptive behavior across different terminal sizes.
//!
//! The status bar consists of two lines:
//! - Line 1: Working directory (left), git branch (center), session status (right)
//! - Line 2: Provider info + context window + thinking level (left), token usage (center), service status (right)

use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
};

use crate::app::App;
use crate::utils::shorten_middle;

/// Configuration for status bar rendering.
#[derive(Debug, Clone, Default)]
pub struct StatusBarConfig {
    /// Enable verbose output (show full paths, complete labels)
    pub verbose: bool,
}

/// Responsive mode based on terminal width.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponsiveMode {
    /// Full (≥120 chars): All information, full paths, complete metrics
    Full,
    /// Compact (80-120 chars): Shortened paths, abbreviated labels
    Compact,
    /// Minimal (<80 chars): Critical info only, defer to `/status` command
    Minimal,
}

impl ResponsiveMode {
    /// Determine mode from terminal width.
    pub fn from_width(width: u16) -> Self {
        match width {
            0..=79 => Self::Minimal,
            80..=119 => Self::Compact,
            _ => Self::Full,
        }
    }
}

/// Color palette for status bar.
pub mod colors {
    use ratatui::style::Color;

    /// Healthy, ready, enabled, clean
    pub const HEALTHY: Color = Color::Green;

    /// Warning, slow, processing, changes
    pub const WARNING: Color = Color::Yellow;

    /// Error, failed, disabled, conflict
    pub const ERROR: Color = Color::Red;

    /// In progress, changed, syncing
    pub const IN_PROGRESS: Color = Color::Cyan;

    /// Labels, separators
    pub const LABEL: Color = Color::DarkGray;

    /// Text
    pub const TEXT: Color = Color::White;
}

/// Status indicators for semantic visual feedback.
pub mod indicators {
    /// Healthy/clean/ready status
    pub const HEALTHY: &str = "●";

    /// Partial/warning status
    pub const PARTIAL: &str = "◔";

    /// Error/failed/conflict status
    pub const ERROR: &str = "✗";

    /// Success/enabled/connected status
    pub const SUCCESS: &str = "✓";

    /// Sync needed status (diverged)
    pub const DIVERGED: &str = "↕";

    /// Busy/processing/loading indicator
    pub const BUSY: &str = "⟳";

    /// Unknown/pending status
    pub const UNKNOWN: &str = "•";

    /// Filled block for progress bars
    pub const FILLED: &str = "█";

    /// Empty block for progress bars
    pub const EMPTY: &str = "░";
}

/// Colored service icons for the Line 2 right section.
///
/// Each service gets a distinct glyph and a fixed accent color so the
/// indicators are recognizable at a glance without reading the text label.
/// The enabled/disabled state is conveyed by the trailing `✓`/`✗` marker
/// (green/red), while the icon itself keeps its own accent color.
pub mod service_icons {
    use ratatui::style::Color;

    /// Code Index — magnifying glass over a document.
    pub const CODE_INDEX: (&str, Color) = ("🔍", Color::Cyan);

    /// Activity Log — scroll/parchment.
    pub const ACTIVITY_LOG: (&str, Color) = ("📜", Color::Yellow);

    /// Autopilot — airplane.
    pub const AUTOPILOT: (&str, Color) = ("✈️", Color::Magenta);

    /// Edit Log — pencil.
    pub const EDIT_LOG: (&str, Color) = ("✏️", Color::LightBlue);

    /// Telemetry — satellite dish / signal.
    pub const TELEMETRY: (&str, Color) = ("📡", Color::LightGreen);

    /// YOLO — warning triangle (bold, changes command-validation behaviour).
    pub const YOLO: (&str, Color) = ("⚠️", Color::LightRed);
}

/// Label abbreviations for compact and minimal modes.
pub mod abbreviations {
    /// Get abbreviated label based on responsive mode.
    pub fn label(label: &str, for_full_mode: bool) -> &str {
        if for_full_mode {
            return label;
        }

        match label {
            "tokens" => "tok",
            "provider" => "pvd",
            "context" => "ctx",
            "tasks" => "t",
            "health" => "hlth",
            "code_index" => "idx",
            "memory" => "mem",
            "git" => "git",
            "branch" => "br",
            "status" => "sts",
            _ => label,
        }
    }

    /// Get abbreviated service name.
    pub fn service(service: &str) -> &str {
        match service {
            "code_index" => "Idx",
            "memory" => "Mem",
            _ => service,
        }
    }

    /// Get abbreviated provider name.
    pub fn provider(name: &str) -> &str {
        match name {
            "anthropic" => "An",
            "claude" => "Cl",
            "openai" => "OAI",
            "gpt" => "GPT",
            "gemini" => "Gm",
            "hugging_face" => "HF",
            "copilot" => "CoPilot",
            "ollama" => "Oll",
            "azure_foundry" => "AzF",
            _ => name,
        }
    }
}

/// Build status bar for a given area.
///
/// Splits the area into 2 lines and renders both with responsive layout.
pub fn render_status_bar_v2(frame: &mut Frame, app: &mut App, area: Rect) {
    let mode = ResponsiveMode::from_width(area.width);
    let config = StatusBarConfig {
        verbose: !matches!(mode, ResponsiveMode::Minimal),
    };

    // C-006: populate the cached status-bar model label before rendering so
    // the hot per-frame path serves it from the cache rather than recomputing
    // (and hitting SQLite + Config::load) on every frame.
    app.update_cached_provider_model_label(app.compute_provider_model_label());

    // Split area into 2 lines
    let line1_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: 1,
    };

    let line2_area = Rect {
        x: area.x,
        y: area.y + 1,
        width: area.width,
        height: 1,
    };

    let line1 = build_line1(app, &config, mode, line1_area.width);
    let line2 = build_line2(app, &config, mode, line2_area.width);

    frame.render_widget(ratatui::widgets::Paragraph::new(line1), line1_area);
    frame.render_widget(ratatui::widgets::Paragraph::new(line2), line2_area);
}

/// Build Line 1: Context & Status
fn build_line1(
    app: &App,
    config: &StatusBarConfig,
    mode: ResponsiveMode,
    width: u16,
) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();

    // Application name and version prefix — identifies the running build at a glance.
    spans.push(Span::styled(
        format!(
            "{} {} v{} ",
            indicators::HEALTHY,
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION")
        ),
        Style::default()
            .fg(colors::HEALTHY)
            .add_modifier(Modifier::BOLD),
    ));

    // Left section: Working directory
    let left = build_line1_left(app, config, mode);
    spans.extend(left);

    // Center section: Git branch
    let center = build_line1_center(app, config, mode);
    let center_width: u16 = center.iter().map(|s| s.width() as u16).sum();

    // Right section: Status message
    let right = build_line1_right(app, config, mode);
    let right_width: u16 = right.iter().map(|s| s.width() as u16).sum();

    let left_width: u16 = spans.iter().map(|s| s.width() as u16).sum();

    // Calculate gap between sections
    let total_used = left_width
        .saturating_add(center_width)
        .saturating_add(right_width);
    let gap_size = width.saturating_sub(total_used);

    // Add center section
    spans.extend(center);

    // Add gap
    if gap_size > 0 {
        spans.push(Span::raw(" ".repeat(gap_size as usize)));
    }

    // Add right section
    spans.extend(right);

    Line::from(spans)
}

/// Build Line 2: Resources & Services
fn build_line2(
    app: &App,
    config: &StatusBarConfig,
    mode: ResponsiveMode,
    width: u16,
) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();

    // Left section: Provider info
    let left = build_line2_left(app, config, mode);
    spans.extend(left);

    // Center section: Token usage
    let center = build_line2_center(app, config, mode);
    let center_width: u16 = center.iter().map(|s| s.width() as u16).sum();

    // Right section: Service status
    let right = build_line2_right(app, config, mode);
    let right_width: u16 = right.iter().map(|s| s.width() as u16).sum();

    let left_width: u16 = spans.iter().map(|s| s.width() as u16).sum();

    // Codeindex busy indicators (top-right, after the service icons). These
    // render in every responsive mode because they are appended in
    // `build_line2` itself rather than `build_line2_right` (which defers to
    // `/status` in non-verbose modes). Style mirrors the compression busy tag
    // in `build_line2_left`: bold, warning/cyan glyph + short label. Their
    // width is reserved before the gap is computed so the tags stay inside
    // the terminal width instead of being clipped.
    let mut busy: Vec<Span<'static>> = Vec::new();
    if app.code_index_busy {
        busy.push(Span::styled(
            format!("{}idx ", indicators::BUSY),
            Style::default()
                .fg(colors::WARNING)
                .add_modifier(Modifier::BOLD),
        ));
    }
    if app.code_index_graph_busy {
        busy.push(Span::styled(
            format!("{}graph ", indicators::BUSY),
            Style::default()
                .fg(colors::IN_PROGRESS)
                .add_modifier(Modifier::BOLD),
        ));
    }
    let busy_width: u16 = busy.iter().map(|s| s.width() as u16).sum();

    // Calculate gap
    let total_used = left_width
        .saturating_add(center_width)
        .saturating_add(right_width)
        .saturating_add(busy_width);
    let gap_size = width.saturating_sub(total_used);

    // Add center section
    spans.extend(center);

    // Add gap
    if gap_size > 0 {
        spans.push(Span::raw(" ".repeat(gap_size as usize)));
    }

    // Add right section
    spans.extend(right);

    // Add codeindex busy indicators
    spans.extend(busy);

    Line::from(spans)
}

// ─────────────────────────────────────────────────────────────────────────────
// Line 1 Section Builders
// ─────────────────────────────────────────────────────────────────────────────

/// Build Line 1 left section: Working directory path
fn build_line1_left(
    app: &App,
    _config: &StatusBarConfig,
    mode: ResponsiveMode,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    let (path, pad_width) = match mode {
        ResponsiveMode::Full => (app.cwd.clone(), 25),
        ResponsiveMode::Compact => (shorten_path(&app.cwd, 20), 15),
        ResponsiveMode::Minimal => (shorten_path(&app.cwd, 15), 10),
    };

    spans.push(Span::styled(
        format!(" {:<width$} ", path, width = pad_width),
        Style::default().fg(colors::TEXT),
    ));

    spans
}

/// Build Line 1 center section: Git branch + status
fn build_line1_center(
    app: &App,
    _config: &StatusBarConfig,
    _mode: ResponsiveMode,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    if let Some(ref branch) = app.git_branch {
        let (status_icon, status_color) = get_git_status_indicator();

        spans.push(Span::styled(
            format!("{} ", branch),
            Style::default().fg(colors::TEXT),
        ));
        spans.push(Span::styled(
            status_icon.to_string(),
            Style::default()
                .fg(status_color)
                .add_modifier(Modifier::BOLD),
        ));
    }

    spans
}

/// Build Line 1 right section: Session status plus optional live web-phase
/// deadline countdown.
pub fn build_line1_right(
    app: &App,
    _config: &StatusBarConfig,
    _mode: ResponsiveMode,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    if !app.status.is_empty() && !app.status.eq_ignore_ascii_case("Ready") {
        spans.push(Span::styled(
            format!("{} ", app.status),
            Style::default()
                .fg(colors::WARNING)
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        spans.push(Span::styled("Ready ", Style::default().fg(colors::HEALTHY)));
    }

    // Live web-phase deadline countdown (FR-010, FR-011, FR-013). If a
    // `/research create` web phase has a stored deadline, show the remaining
    // wall-clock time formatted as M:SS in the top-right wait segment. The
    // ResearchProgress tracker clears the deadline when the web phase ends or
    // the run finishes.
    if let Some(deadline) = app
        .research_progress
        .iter()
        .filter(|p| !p.done)
        .find_map(|p| p.web_phase_deadline)
    {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if !remaining.is_zero() {
            let total_secs = remaining.as_secs();
            let minutes = total_secs / 60;
            let seconds = total_secs % 60;
            spans.push(Span::styled(
                format!("web:{minutes}:{seconds:02} "),
                Style::default()
                    .fg(colors::IN_PROGRESS)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }

    spans
}

// ─────────────────────────────────────────────────────────────────────────────
// Line 2 Section Builders
// ─────────────────────────────────────────────────────────────────────────────

/// Build Line 2 left section: Provider + health + context window
fn build_line2_left(
    app: &App,
    _config: &StatusBarConfig,
    mode: ResponsiveMode,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    // Compression / Compaction activity indicator — always visible, even in Minimal mode.
    if app.compact_in_progress || app.compress_in_progress {
        let label = if app.compact_in_progress && app.compress_in_progress {
            "cmp+compress"
        } else if app.compact_in_progress {
            "compacting"
        } else {
            "compressing"
        };
        let short_label = match mode {
            ResponsiveMode::Minimal => "⟳",
            _ => label,
        };
        spans.push(Span::styled(
            format!("{} ", short_label),
            Style::default()
                .fg(colors::WARNING)
                .add_modifier(Modifier::BOLD),
        ));
    }

    // Provider with health indicator. C-006: `provider_model_label()` serves
    // from a cached field (recomputed only when the model/provider/thinking
    // changes), so this hot render path does not hit SQLite/Config per frame.
    if let Some(label) = app.provider_model_label() {
        let (icon, health_color) = match app.provider_health_status() {
            Some(true) => (indicators::HEALTHY, colors::HEALTHY),
            Some(false) => (indicators::ERROR, colors::ERROR),
            None => (indicators::HEALTHY, colors::WARNING),
        };

        spans.push(Span::styled(
            format!("{} ", icon),
            Style::default()
                .fg(health_color)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!("{} ", label),
            Style::default()
                .fg(colors::TEXT)
                .add_modifier(Modifier::BOLD),
        ));
    }

    // Thinking level indicator
    if let Some(level) = app.selected_thinking_level {
        let short = App::thinking_level_short(level);
        let (level_color, level_icon) = if level.is_enabled() {
            (colors::HEALTHY, "🧠")
        } else {
            (colors::LABEL, "💭")
        };
        let level_str = if mode == ResponsiveMode::Full || mode == ResponsiveMode::Compact {
            format!("{} {} ", level_icon, short)
        } else {
            format!("{} ", level_icon)
        };
        spans.push(Span::styled(level_str, Style::default().fg(level_color)));
    }

    spans
}

/// Build Line 2 center section: Rate-limit quota progress bar (when available).
/// Token count and context-window usage are shown in the Context side panel
/// instead of the status bar.
fn build_line2_center(
    app: &App,
    _config: &StatusBarConfig,
    mode: ResponsiveMode,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    // Only render a center indicator when the provider reports a rate-limit
    // quota percentage. All other usage metrics live in the Context panel.
    if let Some(quota) = app.quota_percent {
        let percent = quota as u32;
        let color = if percent >= 95 {
            colors::ERROR
        } else if percent >= 80 {
            colors::WARNING
        } else {
            colors::HEALTHY
        };

        // Progress bar: 10 chars with filled and empty blocks
        let filled = (percent / 10) as usize;
        let empty = 10_usize.saturating_sub(filled);
        let bar = format!(
            "{}{}",
            indicators::FILLED.repeat(filled),
            indicators::EMPTY.repeat(empty)
        );

        let label = match mode {
            ResponsiveMode::Full => format!("quota: {}% {}", percent, bar),
            ResponsiveMode::Compact => format!("{}% {}", percent, bar),
            ResponsiveMode::Minimal => format!("{}%", percent),
        };

        spans.push(Span::styled(
            label,
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ));
    }

    spans
}

/// Build Line 2 right section: Service status indicators
fn build_line2_right(
    app: &App,
    config: &StatusBarConfig,
    _mode: ResponsiveMode,
) -> Vec<Span<'static>> {
    // Push one `<icon>✓/✗ ` indicator span: the icon keeps its own accent
    // color, while the trailing marker conveys enabled (green ✓) vs disabled
    // (red ✗). Shared by every service indicator on this line.
    fn push_indicator(
        spans: &mut Vec<Span<'static>>,
        icon: (&'static str, Color),
        enabled: bool,
        bold: bool,
    ) {
        let (marker, color) = if enabled {
            (indicators::SUCCESS, colors::HEALTHY)
        } else {
            (indicators::ERROR, colors::ERROR)
        };
        let mut style = Style::default().fg(icon.1);
        if bold {
            style = style.add_modifier(Modifier::BOLD);
        }
        spans.push(Span::styled(format!("{} ", icon.0), style));
        spans.push(Span::styled(
            format!("{marker} "),
            Style::default().fg(color),
        ));
    }

    let mut spans = Vec::new();

    if !config.verbose {
        return spans; // Defer to `/status` in minimal/compact
    }

    // Code Index status
    push_indicator(
        &mut spans,
        service_icons::CODE_INDEX,
        app.code_index_enabled,
        false,
    );

    // Activity-log status
    push_indicator(
        &mut spans,
        service_icons::ACTIVITY_LOG,
        ragent_config::activity_log::is_enabled(),
        false,
    );

    // Autopilot status
    push_indicator(
        &mut spans,
        service_icons::AUTOPILOT,
        app.autopilot_enabled,
        false,
    );

    // Edit-log status
    push_indicator(
        &mut spans,
        service_icons::EDIT_LOG,
        ragent_tools_core::edit_log::is_edit_log_enabled(),
        false,
    );

    // Telemetry (OpenTelemetry metrics export) status
    push_indicator(
        &mut spans,
        service_icons::TELEMETRY,
        app.session_processor.telemetry.is_enabled(),
        false,
    );

    // YOLO mode status (bold — it changes command-validation behaviour)
    push_indicator(
        &mut spans,
        service_icons::YOLO,
        ragent_config::yolo::is_enabled(),
        true,
    );

    spans
}

// ─────────────────────────────────────────────────────────────────────────────
// Utility Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Get git status indicator character and color.
fn get_git_status_indicator() -> (&'static str, Color) {
    // Default indicator: healthy status
    // TODO: Integrate with git module to get actual status
    (indicators::HEALTHY, colors::HEALTHY)
}

/// Shorten a path using ~ for home directory and truncation.
fn shorten_path(path: &str, max_len: usize) -> String {
    let path_len = path.chars().count();
    if path_len <= max_len {
        return path.to_string();
    }

    // Try to shorten with ~
    if let Ok(home) = std::env::var("HOME") {
        if let Some(stripped) = path.strip_prefix(&home) {
            let tilde_path = format!("~{}", stripped);
            if tilde_path.chars().count() <= max_len {
                return tilde_path;
            }
        }
    }

    // Fall back to truncation: show beginning and end
    shorten_middle(path, max_len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_responsive_mode_from_width() {
        assert_eq!(ResponsiveMode::from_width(50), ResponsiveMode::Minimal);
        assert_eq!(ResponsiveMode::from_width(79), ResponsiveMode::Minimal);
        assert_eq!(ResponsiveMode::from_width(80), ResponsiveMode::Compact);
        assert_eq!(ResponsiveMode::from_width(119), ResponsiveMode::Compact);
        assert_eq!(ResponsiveMode::from_width(120), ResponsiveMode::Full);
        assert_eq!(ResponsiveMode::from_width(200), ResponsiveMode::Full);
    }

    #[test]
    fn test_shorten_path() {
        assert_eq!(shorten_path("/home/user", 50), "/home/user");

        let long_path = "/very/long/path/that/exceeds/maximum";
        let shortened = shorten_path(long_path, 20);
        assert!(shortened.chars().count() <= 20);
        assert!(shortened.contains('…'));
    }
}
