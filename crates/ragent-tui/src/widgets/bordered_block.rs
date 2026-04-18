//! Bordered block widget with theme-aware styling.
//!
//! Provides a standardized block component that automatically applies
//! the active theme's border colors and styles for consistent UI appearance.

use ratatui::{
    style::{Color, Style},
    widgets::{Block, BorderType, Borders},
};

/// Border state variants for different interactive states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BorderState {
    /// Default border state for non-interactive elements
    #[default]
    Default,
    /// Active border state for focused/interactive elements
    Active,
    /// Inactive border state for disabled elements
    Inactive,
}

/// A theme-aware bordered block component.
///
/// Wraps ratatui's `Block` with automatic application of theme colors
/// for borders based on the current border state. This ensures consistent
/// border styling across all UI components.
///
/// # Example
///
/// ```
/// use ragent_tui::widgets::bordered_block::{BorderedBlock, BorderState};
///
/// // Create a default bordered block with theme colors
/// let block = BorderedBlock::new("Panel Title", BorderState::Default);
///
/// // Create an active (focused) bordered block
/// let active_block = BorderedBlock::new("Active Panel", BorderState::Active);
/// ```
pub struct BorderedBlock {
    /// The underlying block widget
    block: Block<'static>,
    /// Current border state
    state: BorderState,
}

impl BorderedBlock {
    /// Create a new bordered block with the given title and state.
    ///
    /// # Arguments
    ///
    /// * `title` - The title text to display in the block border
    /// * `state` - The border state determining which theme colors to apply
    pub fn new(title: impl Into<String>, state: BorderState) -> Self {
        let title = title.into();
        let border_color = Self::border_color_for_state(state);

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        Self { block, state }
    }

    /// Create a new bordered block without a title.
    ///
    /// # Arguments
    ///
    /// * `state` - The border state determining which theme colors to apply
    pub fn without_title(state: BorderState) -> Self {
        let border_color = Self::border_color_for_state(state);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        Self {
            block,
            state,
        }
    }

    /// Get the border color for a given state from the theme.
    fn border_color_for_state(state: BorderState) -> Color {
        match state {
            BorderState::Default => crate::theme::borders::DEFAULT,
            BorderState::Active => crate::theme::borders::ACTIVE,
            BorderState::Inactive => crate::theme::borders::INACTIVE,
        }
    }

    /// Set the border type.
    #[must_use]
    pub fn border_type(mut self, border_type: BorderType) -> Self {
        self.block = self.block.border_type(border_type);
        self
    }

    /// Get the current border state.
    pub fn state(&self) -> BorderState {
        self.state
    }

    /// Convert into the underlying Block widget.
    pub fn into_block(self) -> Block<'static> {
        self.block
    }

    /// Get a reference to the underlying Block widget.
    pub fn block(&self) -> &Block<'static> {
        &self.block
    }
}

impl From<BorderedBlock> for Block<'static> {
    fn from(bordered: BorderedBlock) -> Self {
        bordered.block
    }
}

/// Convenience function to create a default bordered block.
///
/// # Example
///
/// ```
/// use ragent_tui::widgets::bordered_block::default_block;
///
/// let block = default_block("My Panel");
/// ```
pub fn default_block(title: impl Into<String>) -> Block<'static> {
    BorderedBlock::new(title, BorderState::Default).into_block()
}

/// Convenience function to create an active (focused) bordered block.
///
/// # Example
///
/// ```
/// use ragent_tui::widgets::bordered_block::active_block;
///
/// let block = active_block("Active Panel");
/// ```
pub fn active_block(title: impl Into<String>) -> Block<'static> {
    BorderedBlock::new(title, BorderState::Active).into_block()
}

/// Convenience function to create an inactive (disabled) bordered block.
///
/// # Example
///
/// ```
/// use ragent_tui::widgets::bordered_block::inactive_block;
///
/// let block = inactive_block("Disabled Panel");
/// ```
pub fn inactive_block(title: impl Into<String>) -> Block<'static> {
    BorderedBlock::new(title, BorderState::Inactive).into_block()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bordered_block_creation() {
        let block = BorderedBlock::new("Test", BorderState::Default);
        assert_eq!(block.state(), BorderState::Default);
    }

    #[test]
    fn test_bordered_block_without_title() {
        let block = BorderedBlock::without_title(BorderState::Active);
        assert_eq!(block.state(), BorderState::Active);
    }

    #[test]
    fn test_default_block() {
        let _block = default_block("Test");
        // Just verify it compiles and runs
    }

    #[test]
    fn test_active_block() {
        let _block = active_block("Test");
        // Just verify it compiles and runs
    }

    #[test]
    fn test_inactive_block() {
        let _block = inactive_block("Test");
        // Just verify it compiles and runs
    }

    #[test]
    fn test_border_color_for_state() {
        let default_color = BorderedBlock::border_color_for_state(BorderState::Default);
        let active_color = BorderedBlock::border_color_for_state(BorderState::Active);
        let inactive_color = BorderedBlock::border_color_for_state(BorderState::Inactive);

        // Colors should be different for different states
        assert_ne!(default_color, active_color);
        assert_ne!(default_color, inactive_color);
        assert_ne!(active_color, inactive_color);
    }
}
