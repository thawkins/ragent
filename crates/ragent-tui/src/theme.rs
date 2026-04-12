//! Theme module for ragent TUI.
//!
//! Provides centralized theming including colors, typography, spacing, and icons
//! to ensure consistent visual design across all UI components.

use ratatui::style::{Modifier, Style};

/// Semantic status colors
pub mod status {
    use ratatui::style::Color;

    /// Status color for successful operations (green)
    pub const SUCCESS: Color = Color::Green;
    /// Status color for errors (red)
    pub const ERROR: Color = Color::Red;
    /// Status color for warnings (yellow)
    pub const WARNING: Color = Color::Yellow;
    /// Status color for informational messages (cyan)
    pub const INFO: Color = Color::Cyan;
}

/// Accessible grays (WCAG AA compliant)
/// DarkGray was 2.9:1 contrast on Black, replaced with accessible values
pub mod colors {
    use ratatui::style::Color;

    /// Hint color (dimmed gray) for secondary information
    pub const HINT: Color = Color::Rgb(170, 170, 170); // Previously Color::DarkGray
    /// Disabled color for inactive elements
    pub const DISABLED: Color = Color::Rgb(140, 140, 140);
    /// Primary text color (white)
    pub const TEXT: Color = Color::White;
    /// Background color (black)
    pub const BACKGROUND: Color = Color::Black;

    /// Primary UI color (blue)
    pub const PRIMARY: Color = Color::Blue;
    /// Secondary UI color (cyan)
    pub const SECONDARY: Color = Color::Cyan;
    /// Muted color for less important elements
    pub const MUTED: Color = Color::Rgb(170, 170, 170);

    /// Dialog info color (cyan)
    pub const DIALOG_INFO: Color = Color::Cyan;
    /// Dialog warning color (yellow)
    pub const DIALOG_WARNING: Color = Color::Yellow;
    /// Dialog danger color (red)
    pub const DIALOG_DANGER: Color = Color::Red;
    /// Dialog success color (green)
    pub const DIALOG_SUCCESS: Color = Color::Green;

    /// Link color (cyan)
    pub const LINK_COLOR: Color = Color::Cyan;
    /// Link hover color (yellow)
    pub const LINK_HOVER: Color = Color::Yellow;

    /// Selection highlight background color (blue)
    pub const SELECTION_BG: Color = Color::Rgb(0, 100, 200);
    /// Selection highlight foreground color (white)
    pub const SELECTION_FG: Color = Color::White;

    /// Focus indicator color (yellow)
    pub const FOCUS_COLOR: Color = Color::Yellow;
    /// Focus border color (yellow)
    pub const FOCUS_BORDER: Color = Color::Yellow;
}

/// Typography system with semantic roles.

/// Heading text style (bold, for section titles)
pub fn heading() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

/// Emphasis style (bold, for important text)
pub fn emphasis() -> Style {
    Style::default().add_modifier(Modifier::BOLD)
}

/// Muted text style (dimmed, for secondary information)
pub fn muted() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

/// Error text style (red, for error messages)
pub fn error() -> Style {
    Style::default().fg(status::ERROR)
}

/// Success text style (green, for success messages)
pub fn success() -> Style {
    Style::default().fg(status::SUCCESS)
}

/// Spacing tokens for consistent layout.

/// Extra small spacing (1px)
pub const SPACING_XS: u16 = 1;
/// Small spacing (2px)
pub const SPACING_SM: u16 = 2;
/// Medium spacing (4px)
pub const SPACING_MD: u16 = 4;
/// Large spacing (8px)
pub const SPACING_LG: u16 = 8;
/// Extra large spacing (16px)
pub const SPACING_XL: u16 = 16;

/// Layout constants.
/// Maximum content width for centered dialogs
pub const LAYOUT_MAX_CONTENT_WIDTH: u16 = 88;
/// Border padding (assumed but not standardized before)
pub const LAYOUT_BORDER_PADDING: u16 = 2;

/// Dialog size presets.
/// Small dialog: 60x30
pub const LAYOUT_SMALL_DIALOG: (u16, u16) = (60, 30);
/// Medium dialog: 70x40
pub const LAYOUT_MEDIUM_DIALOG: (u16, u16) = (70, 40);
/// Large dialog: 90x70
pub const LAYOUT_LARGE_DIALOG: (u16, u16) = (90, 70);

/// Standardized icon set.
///
/// Decision: Unicode symbols only (no Nerd Fonts dependency for accessibility).
/// All icons use consistent width to prevent rendering inconsistencies.

/// Thought/Reasoning icon
pub const ICON_THOUGHT: &str = "◌ ";
/// File icon
pub const ICON_FILE: &str = "📄 ";
/// Task icon
pub const ICON_TASK: &str = "▶ ";
/// Success icon (checkmark circle)
pub const ICON_SUCCESS: &str = "● ";
/// Error icon (X)
pub const ICON_ERROR: &str = "✗ ";
/// Running icon (diamond)
pub const ICON_RUNNING: &str = "◆ ";
/// Spawning icon (circle)
pub const ICON_SPAWNING: &str = "◌ ";
/// Idle icon (bullet)
pub const ICON_IDLE: &str = "● ";
/// Blocked icon (circle with vertical line)
pub const ICON_BLOCKED: &str = "⊘ ";
/// Failed icon (X)
pub const ICON_FAILED: &str = "✗ ";
/// Selected indicator (triangle)
pub const ICON_SELECTED: &str = "▸ ";
/// Unselected indicator (space)
pub const ICON_UNSELECTED: &str = "  ";

/// Focus indicator constants module for keyboard navigation.
pub mod focus {
    /// Selected item prefix (unicode triangle)
    pub const SELECTED: &str = "▸ ";
    /// Unselected item prefix (space)
    pub const UNSELECTED: &str = "  ";
}
