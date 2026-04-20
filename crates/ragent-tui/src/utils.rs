//! Layout utilities for ragent TUI.
//!
//! Provides common layout helper functions used across the UI components.

use ratatui::layout::{Constraint, Direction, Layout, Rect};

/// Responsive layout constraints based on terminal width.
#[derive(Debug, Clone, Copy)]
pub enum ResponsiveBreakpoint {
    /// Small: < 80 columns
    Small,
    /// Medium: 80-120 columns
    Medium,
    /// Large: > 120 columns
    Large,
}

impl ResponsiveBreakpoint {
    /// Determine the breakpoint from terminal width.
    pub fn from_width(width: u16) -> Self {
        if width < 80 {
            Self::Small
        } else if width <= 120 {
            Self::Medium
        } else {
            Self::Large
        }
    }

    /// Get the percentage split for the log panel.
    /// Returns (messages_percent, log_percent)
    pub fn log_split(&self) -> (u16, u16) {
        match self {
            Self::Small => (70, 30),  // More space for messages on small screens
            Self::Medium => (60, 40), // Balanced split
            Self::Large => (55, 45),  // More room for log on large screens
        }
    }

    /// Get the minimum content width for this breakpoint.
    pub fn min_content_width(&self) -> u16 {
        match self {
            Self::Small => 40,
            Self::Medium => 60,
            Self::Large => 80,
        }
    }

    /// Get the status bar height for this breakpoint.
    pub fn status_bar_height(&self) -> u16 {
        match self {
            Self::Small => 2,  // Compact status bar
            Self::Medium => 2, // Standard
            Self::Large => 2,  // Standard (could be expanded in future)
        }
    }

    /// Get the button column width for the input area.
    pub fn button_column_width(&self) -> u16 {
        match self {
            Self::Small => 12,  // Narrower buttons
            Self::Medium => 18, // Standard
            Self::Large => 20,  // Wider buttons
        }
    }
}

/// Check if terminal size is below minimum recommended dimensions.
/// Returns true if terminal is too small for optimal experience.
pub fn is_below_minimum_size(area: Rect) -> bool {
    area.width < 40 || area.height < 12
}

/// Center a rectangle within the given area.
///
/// Returns a Rect centered in the provided area with the specified
/// percentage width and height.
pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Center a rectangle with maximum dimensions.
///
/// Similar to `centered_rect` but caps the width and height at the specified
/// maximum values. This prevents oversized dialogs on large screens.
///
/// # Arguments
///
/// * `percent_x` - Horizontal percentage of the area to use
/// * `percent_y` - Vertical percentage of the area to use
/// * `max_w` - Maximum width in columns
/// * `max_h` - Maximum height in rows
/// * `area` - The area to center within
pub fn centered_rect_max(
    percent_x: u16,
    percent_y: u16,
    max_w: u16,
    max_h: u16,
    area: Rect,
) -> Rect {
    let raw = centered_rect(percent_x, percent_y, area);
    let w = raw.width.min(max_w).min(area.width);
    let h = raw.height.min(max_h).min(area.height);
    let x = raw.x + (raw.width.saturating_sub(w) / 2);
    let y = raw.y + (raw.height.saturating_sub(h) / 2);
    Rect::new(x, y, w, h)
}

/// Truncate text with ellipsis if it exceeds the maximum length.
///
/// Returns the original string if it's within bounds, otherwise
/// returns a truncated version with "…" at the end.
pub fn truncate_with_ellipsis(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        text.to_string()
    } else if max_chars <= 1 {
        "…".to_string()
    } else {
        text.chars()
            .take(max_chars.saturating_sub(1))
            .collect::<String>()
            + "…"
    }
}
