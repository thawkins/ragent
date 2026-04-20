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
    /// Border color for inactive elements (gray)
    pub const BORDER_INACTIVE: Color = Color::Rgb(100, 100, 100);
    /// Primary text color (white)
    pub const TEXT_PRIMARY: Color = Color::White;
}

/// High contrast colors (WCAG AAA compliant)
/// Pure black/white with maximum contrast ratios for accessibility
pub mod high_contrast {
    use ratatui::style::Color;

    /// Pure white text on black background (21:1 contrast)
    pub const TEXT: Color = Color::White;
    /// Pure black background
    pub const BACKGROUND: Color = Color::Black;
    /// Pure white for emphasis
    pub const TEXT_EMPHASIS: Color = Color::White;
    /// Gray for secondary text (still high contrast)
    pub const HINT: Color = Color::Rgb(200, 200, 200);
    /// Disabled elements (clearly distinguishable)
    pub const DISABLED: Color = Color::Rgb(160, 160, 160);

    /// Pure blue for primary UI (distinct from text)
    pub const PRIMARY: Color = Color::Rgb(0, 150, 255);
    /// Bright cyan for secondary elements
    pub const SECONDARY: Color = Color::Rgb(0, 255, 255);
    /// Muted elements use darker gray
    pub const MUTED: Color = Color::Rgb(128, 128, 128);

    /// Dialog colors with bold styling
    pub const DIALOG_INFO: Color = Color::Cyan;
    /// Warning uses bright yellow
    pub const DIALOG_WARNING: Color = Color::Rgb(255, 255, 0);
    /// Danger uses bright red
    pub const DIALOG_DANGER: Color = Color::Rgb(255, 50, 50);
    /// Success uses bright green
    pub const DIALOG_SUCCESS: Color = Color::Rgb(50, 255, 50);

    /// Links are underlined + bright cyan
    pub const LINK_COLOR: Color = Color::Rgb(0, 255, 255);
    /// Link hover is bright yellow
    pub const LINK_HOVER: Color = Color::Rgb(255, 255, 0);

    /// Selection uses white on blue (high contrast)
    pub const SELECTION_BG: Color = Color::Rgb(0, 100, 255);
    /// Selection foreground color.
    pub const SELECTION_FG: Color = Color::White;

    /// Focus uses bright yellow with bold borders
    pub const FOCUS_COLOR: Color = Color::Rgb(255, 255, 0);
    /// Focus border color.
    pub const FOCUS_BORDER: Color = Color::Rgb(255, 255, 0);
    /// Inactive borders use medium gray
    pub const BORDER_INACTIVE: Color = Color::Rgb(128, 128, 128);
    /// Primary text color.
    pub const TEXT_PRIMARY: Color = Color::White;
}

/// Theme mode enum for switching between default and high contrast themes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeMode {
    /// Default theme with standard colors
    #[default]
    Default,
    /// High contrast theme for accessibility (WCAG AAA)
    HighContrast,
}

impl ThemeMode {
    /// Parse a theme mode from a string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "default" => Some(Self::Default),
            "high-contrast" | "high_contrast" | "highcontrast" => Some(Self::HighContrast),
            _ => None,
        }
    }

    /// Get the display name for the theme mode
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::HighContrast => "high-contrast",
        }
    }
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

/// Warning text style (yellow, for warning messages)
pub fn warning() -> Style {
    Style::default().fg(status::WARNING)
}

/// Info text style (cyan, for informational messages)
pub fn info() -> Style {
    Style::default().fg(status::INFO)
}

/// Loading text style (bold cyan with dimmed effect)
pub fn loading() -> Style {
    Style::default()
        .fg(status::INFO)
        .add_modifier(Modifier::BOLD | Modifier::DIM)
}

/// Disabled text style (gray, for inactive elements)
pub fn disabled() -> Style {
    Style::default()
        .fg(colors::DISABLED)
        .add_modifier(Modifier::DIM)
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
    /// Focus indicator (diamond) for keyboard focus
    pub const FOCUSED: &str = "◆ ";
    /// Button role indicator (Braille pattern) for screen reader support
    pub const BUTTON: &str = "⣿ ";
    /// Generic interactive element indicator
    pub const INTERACTIVE: &str = "◆ ";
}

/// Accessibility constants for screen reader support and ARIA-like annotations.
pub mod accessibility {
    /// Braille pattern to indicate a button element (role=button)
    pub const ROLE_BUTTON: &str = "⣿";
    /// Braille pattern for selected/pressed state
    pub const STATE_SELECTED: &str = "⣿";
    /// Indicator for focusable elements
    pub const ROLE_INTERACTIVE: &str = "◆";
    /// Indicator for expandable/collapsible sections
    pub const ROLE_EXPANDABLE: &str = "▸";
    /// Indicator for collapsed sections
    pub const ROLE_COLLAPSED: &str = "▸";
    /// Indicator for expanded sections
    pub const ROLE_EXPANDED: &str = "▾";
    /// Loading state indicator (Braille pattern)
    pub const STATE_LOADING: &str = "⣿";
    /// Progress indicator prefix
    pub const PROGRESS_PREFIX: &str = "[";
    /// Progress indicator suffix
    pub const PROGRESS_SUFFIX: &str = "]";
    /// Progress bar filled character
    pub const PROGRESS_FILL: &str = "█";
    /// Progress bar empty character
    pub const PROGRESS_EMPTY: &str = "░";

    /// Animation frames for indeterminate loading spinner
    pub const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    /// Get the current spinner frame based on elapsed time
    ///
    /// # Arguments
    /// * `elapsed_ms` - Milliseconds elapsed since loading started
    /// * `interval_ms` - Milliseconds per frame (default: 80ms)
    pub fn spinner_frame(elapsed_ms: u64, interval_ms: u64) -> &'static str {
        let frame = (elapsed_ms / interval_ms.max(1)) as usize % SPINNER_FRAMES.len();
        SPINNER_FRAMES[frame]
    }

    /// Get a progress bar string for determinate progress
    ///
    /// # Arguments
    /// * `progress` - Progress value from 0.0 to 1.0
    /// * `width` - Width of the progress bar in characters
    ///
    /// # Example
    /// ```
    /// use ragent_tui::theme::accessibility::progress_bar;
    ///
    /// let bar = progress_bar(0.5, 20); // "[██████████░░░░░░░░░░]"
    /// ```
    pub fn progress_bar(progress: f32, width: usize) -> String {
        let filled = ((progress.clamp(0.0, 1.0) * width as f32) as usize).min(width);
        let empty = width.saturating_sub(filled);

        format!(
            "{}{}{}{}{}",
            PROGRESS_PREFIX,
            PROGRESS_FILL.repeat(filled),
            PROGRESS_EMPTY.repeat(empty),
            PROGRESS_SUFFIX,
            if progress >= 1.0 { " ✓" } else { "" }
        )
    }

    /// Get a labeled progress bar with percentage
    ///
    /// # Arguments
    /// * `progress` - Progress value from 0.0 to 1.0
    /// * `width` - Width of the progress bar in characters
    /// * `label` - Label to display before the bar
    ///
    /// # Example
    /// ```
    /// use ragent_tui::theme::accessibility::labeled_progress_bar;
    ///
    /// let bar = labeled_progress_bar(0.5, 20, "Loading"); // "Loading [██████████░░░░░░░░░░] 50%"
    /// ```
    pub fn labeled_progress_bar(progress: f32, width: usize, label: &str) -> String {
        let bar = progress_bar(progress, width);
        let percent = (progress.clamp(0.0, 1.0) * 100.0) as u8;
        format!("{} {} {}%", label, bar, percent)
    }
}

/// Standardized loading spinner using braille patterns (rotates: ◐◓◑◒)
pub const LOADING_FRAMES: &[&str] = &["◐", "◓", "◑", "◒"];

/// Get the current loading frame based on elapsed time
///
/// # Arguments
/// * `elapsed_ms` - Milliseconds elapsed since loading started
/// * `interval_ms` - Milliseconds per frame (default: 250ms)
pub fn loading_frame(elapsed_ms: u64, interval_ms: u64) -> &'static str {
    let frame = (elapsed_ms / interval_ms.max(1)) as usize % LOADING_FRAMES.len();
    LOADING_FRAMES[frame]
}

/// Status message categories for consistent status bar feedback
///
/// This enum standardizes how status messages are categorized and displayed,
/// ensuring consistent visual feedback across the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusCategory {
    /// General informational messages (cyan/white)
    Info,
    /// Successful operation completion (green)
    Success,
    /// Warning messages that need attention (yellow)
    Warning,
    /// Error messages (red)
    Error,
    /// Ongoing work with progress indication (cyan with spinner)
    Working,
}

impl StatusCategory {
    /// Get the icon associated with this status category
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Info => "ℹ ",
            Self::Success => crate::theme::ICON_SUCCESS,
            Self::Warning => "⚠ ",
            Self::Error => crate::theme::ICON_ERROR,
            Self::Working => crate::theme::ICON_RUNNING,
        }
    }

    /// Get the color associated with this status category
    pub fn color(&self) -> ratatui::style::Color {
        match self {
            Self::Info => status::INFO,
            Self::Success => status::SUCCESS,
            Self::Warning => status::WARNING,
            Self::Error => status::ERROR,
            Self::Working => status::INFO,
        }
    }

    /// Get the style associated with this status category
    pub fn style(&self) -> ratatui::style::Style {
        use ratatui::style::Modifier;
        match self {
            Self::Working => ratatui::style::Style::default()
                .fg(self.color())
                .add_modifier(Modifier::BOLD),
            _ => ratatui::style::Style::default().fg(self.color()),
        }
    }

    /// Format a message with the appropriate prefix and styling
    pub fn format(&self, message: &str) -> String {
        format!("{} {}", self.icon(), message)
    }

    /// Get a display name for the category
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
            Self::Working => "working",
        }
    }
}

/// A status message with category and timestamp for history tracking
#[derive(Debug, Clone)]
pub struct StatusMessage {
    /// The message content
    pub message: String,
    /// The status category
    pub category: StatusCategory,
    /// When the message was created
    pub timestamp: std::time::SystemTime,
}

impl StatusMessage {
    /// Create a new status message
    pub fn new(message: impl Into<String>, category: StatusCategory) -> Self {
        Self {
            message: message.into(),
            category,
            timestamp: std::time::SystemTime::now(),
        }
    }

    /// Create an info status message
    pub fn info(message: impl Into<String>) -> Self {
        Self::new(message, StatusCategory::Info)
    }

    /// Create a success status message
    pub fn success(message: impl Into<String>) -> Self {
        Self::new(message, StatusCategory::Success)
    }

    /// Create a warning status message
    pub fn warning(message: impl Into<String>) -> Self {
        Self::new(message, StatusCategory::Warning)
    }

    /// Create an error status message
    pub fn error(message: impl Into<String>) -> Self {
        Self::new(message, StatusCategory::Error)
    }

    /// Create a working status message
    pub fn working(message: impl Into<String>) -> Self {
        Self::new(message, StatusCategory::Working)
    }

    /// Format the message with category prefix
    pub fn formatted(&self) -> String {
        self.category.format(&self.message)
    }

    /// Get the style for rendering this message
    pub fn style(&self) -> ratatui::style::Style {
        self.category.style()
    }

    /// Get age of the message in seconds
    pub fn age_secs(&self) -> u64 {
        self.timestamp.elapsed().unwrap_or_default().as_secs()
    }
}

/// Status message history for tracking recent messages
#[derive(Debug, Clone, Default)]
pub struct StatusHistory {
    messages: std::collections::VecDeque<StatusMessage>,
    max_size: usize,
}

impl StatusHistory {
    /// Create a new status history with default capacity
    pub fn new() -> Self {
        Self {
            messages: std::collections::VecDeque::new(),
            max_size: 100,
        }
    }

    /// Create a new status history with custom capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            messages: std::collections::VecDeque::new(),
            max_size: capacity,
        }
    }

    /// Add a message to the history
    pub fn push(&mut self, message: StatusMessage) {
        if self.messages.len() >= self.max_size {
            self.messages.pop_front();
        }
        self.messages.push_back(message);
    }

    /// Get all messages in chronological order (oldest first)
    pub fn messages(&self) -> &std::collections::VecDeque<StatusMessage> {
        &self.messages
    }

    /// Get recent messages (newest first, up to n)
    pub fn recent(&self, n: usize) -> Vec<&StatusMessage> {
        self.messages.iter().rev().take(n).collect()
    }

    /// Get messages filtered by category
    pub fn by_category(&self, category: StatusCategory) -> Vec<&StatusMessage> {
        self.messages
            .iter()
            .filter(|m| m.category == category)
            .collect()
    }

    /// Clear all messages
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Get the number of messages in history
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Check if history is empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}
