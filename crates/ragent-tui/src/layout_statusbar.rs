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

/// Configuration for status bar rendering.
#[derive(Debug, Clone)]
pub struct StatusBarConfig {
    /// Enable verbose output (show full paths, complete labels)
    pub verbose: bool,
}

impl Default for StatusBarConfig {
    fn default() -> Self {
        Self { verbose: false }
    }
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
            0..=79 => ResponsiveMode::Minimal,
            80..=119 => ResponsiveMode::Compact,
            _ => ResponsiveMode::Full,
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

/// Spinner frames for animated indicators.
pub mod spinner {
    /// Spinner animation frames: ⠋ ⠙ ⠹ ⠸ ⠼ ⠴ ⠦ ⠧ ⠇ ⠏
    pub const FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    /// Get spinner frame for elapsed time (in milliseconds).
    pub fn frame(elapsed_ms: u64) -> &'static str {
        let idx = ((elapsed_ms / 45) as usize) % FRAMES.len();
        FRAMES[idx]
    }
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

    // Calculate gap
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

/// Build Line 1 right section: Session status
fn build_line1_right(
    app: &App,
    _config: &StatusBarConfig,
    _mode: ResponsiveMode,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    if !app.status.is_empty() && app.status != "Ready" {
        spans.push(Span::styled(
            format!("{} ", app.status),
            Style::default()
                .fg(colors::WARNING)
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        spans.push(Span::styled("Ready ", Style::default().fg(colors::HEALTHY)));
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

    // Provider with health indicator
    if let Some(label) = app.provider_model_label() {
        let (icon, health_color) = match app.provider_health_status() {
            Some(true) => (indicators::HEALTHY, colors::HEALTHY),
            Some(false) => (indicators::ERROR, colors::ERROR),
            None => (indicators::HEALTHY, colors::WARNING),
        };

        spans.push(Span::styled(
            format!("{} {} ", indicators::HEALTHY, label),
            Style::default()
                .fg(colors::TEXT)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!("{} ", icon),
            Style::default()
                .fg(health_color)
                .add_modifier(Modifier::BOLD),
        ));
    }

    // Context window info
    let (used, total) = app.token_usage;
    let ctx_label = match mode {
        ResponsiveMode::Full => format!("{}/{}", used, total),
        ResponsiveMode::Compact => {
            let pct = if total > 0 {
                (used as f32 / total as f32 * 100.0) as u32
            } else {
                0
            };
            format!("{}%", pct)
        }
        ResponsiveMode::Minimal => {
            let pct = if total > 0 {
                (used as f32 / total as f32 * 100.0) as u32
            } else {
                0
            };
            format!("{}%", pct)
        }
    };

    spans.push(Span::styled(
        format!("{:<12} ", ctx_label),
        Style::default().fg(colors::IN_PROGRESS),
    ));

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

/// Build Line 2 center section: Token usage + progress bar
fn build_line2_center(
    app: &App,
    _config: &StatusBarConfig,
    mode: ResponsiveMode,
) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    fn format_kilobytes(bytes: u64) -> String {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    }

    // Use quota_percent from rate-limit headers if available, otherwise show cumulative token count
    let display_text = if let Some(quota) = app.quota_percent {
        // Rate limit quota percentage (from provider headers)
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

        (label, color)
    } else {
        // Fallback: show cumulative token usage count
        let (input_total, output_total) = app.token_usage;
        let total = input_total.saturating_add(output_total);

        let label = match mode {
            ResponsiveMode::Full => format!("tokens: {}", total),
            ResponsiveMode::Compact => format!("{}", total),
            ResponsiveMode::Minimal => format!("{}", total),
        };

        (label, colors::TEXT)
    };

    spans.push(Span::styled(
        display_text.0,
        Style::default()
            .fg(display_text.1)
            .add_modifier(Modifier::BOLD),
    ));

    // Context window percentage from the most recent request.
    if let Some(ctx_detail) = app.context_window_display() {
        let ctx_pct = ctx_detail
            .split_whitespace()
            .next()
            .and_then(|p| p.trim_end_matches('%').parse::<u32>().ok())
            .unwrap_or(0);

        let ctx_color = if ctx_pct >= 95 {
            colors::ERROR
        } else if ctx_pct >= 80 {
            colors::WARNING
        } else {
            colors::HEALTHY
        };

        let ctx_label = match mode {
            ResponsiveMode::Full => format!(" | ctx: {ctx_detail}"),
            ResponsiveMode::Compact => format!(" | ctx: {ctx_detail}"),
            ResponsiveMode::Minimal => format!(" {}%", ctx_pct),
        };

        spans.push(Span::styled(ctx_label, Style::default().fg(ctx_color)));
    }

    if app.stream_out_bytes > 0 || app.stream_in_bytes > 0 {
        let io_label = match mode {
            ResponsiveMode::Full => format!(
                " | io: ↑{} ↓{}",
                format_kilobytes(app.stream_out_bytes),
                format_kilobytes(app.stream_in_bytes)
            ),
            ResponsiveMode::Compact => format!(
                " | ↑{} ↓{}",
                format_kilobytes(app.stream_out_bytes),
                format_kilobytes(app.stream_in_bytes)
            ),
            ResponsiveMode::Minimal => format!(
                " ↑{} ↓{}",
                app.stream_out_bytes / 1024,
                app.stream_in_bytes / 1024
            ),
        };

        spans.push(Span::styled(
            io_label,
            Style::default().fg(colors::IN_PROGRESS),
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
    let mut spans = Vec::new();

    if !config.verbose {
        return spans; // Defer to `/status` in minimal/compact
    }

    // Code Index status
    {
        let (icon, color) = if app.code_index_enabled {
            (indicators::SUCCESS, colors::HEALTHY)
        } else {
            (indicators::ERROR, colors::ERROR)
        };

        spans.push(Span::styled(
            format!("CodeIdx:{icon}  "),
            Style::default().fg(color),
        ));
    }

    // Internal LLM status
    {
        let (icon, color, label) = if app.internal_llm_config.enabled {
            if app.internal_llm_init_error.is_some() {
                // Enabled but failed to initialize
                (indicators::ERROR, colors::ERROR, "InternalLLM:err")
            } else if app.internal_llm_service.is_none() {
                // Enabled but still loading
                (indicators::BUSY, colors::IN_PROGRESS, "InternalLLM:...")
            } else if app.internal_llm_title_pending {
                // Enabled, loaded, but has pending work
                (indicators::BUSY, colors::IN_PROGRESS, "InternalLLM:busy")
            } else {
                // Enabled and ready
                (indicators::SUCCESS, colors::HEALTHY, "InternalLLM:on")
            }
        } else {
            // Disabled
            (indicators::ERROR, colors::ERROR, "InternalLLM:off")
        };

        spans.push(Span::styled(
            format!(" {label}:{icon}"),
            Style::default().fg(color),
        ));
    }

    spans
}

// ─────────────────────────────────────────────────────────────────────────────
// Styling Helper Functions
// ─────────────────────────────────────────────────────────────────────────────

/// Create a healthy (green) style.
#[allow(dead_code)]
fn style_healthy() -> Style {
    Style::default().fg(colors::HEALTHY)
}

/// Create a warning (yellow) style.
#[allow(dead_code)]
fn style_warning() -> Style {
    Style::default().fg(colors::WARNING)
}

/// Create an error (red) style.
#[allow(dead_code)]
fn style_error() -> Style {
    Style::default().fg(colors::ERROR)
}

/// Create an info/progress (cyan) style.
#[allow(dead_code)]
fn style_info() -> Style {
    Style::default().fg(colors::IN_PROGRESS)
}

/// Create a bold healthy style.
#[allow(dead_code)]
fn style_healthy_bold() -> Style {
    Style::default()
        .fg(colors::HEALTHY)
        .add_modifier(Modifier::BOLD)
}

/// Create a bold warning style.
#[allow(dead_code)]
fn style_warning_bold() -> Style {
    Style::default()
        .fg(colors::WARNING)
        .add_modifier(Modifier::BOLD)
}

/// Create a bold error style.
#[allow(dead_code)]
fn style_error_bold() -> Style {
    Style::default()
        .fg(colors::ERROR)
        .add_modifier(Modifier::BOLD)
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
    if max_len <= 3 {
        return "…".to_string();
    }

    let keep_left = (max_len - 1) / 2;
    let keep_right = max_len - 1 - keep_left;
    let left: String = path.chars().take(keep_left).collect();
    let right: String = path
        .chars()
        .skip(path_len.saturating_sub(keep_right))
        .collect();
    format!("{left}…{right}")
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
