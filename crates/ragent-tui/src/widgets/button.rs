//! Button component for ragent TUI.
//!
//! Provides a standardized button component with various visual variants
//! and states for interactive UI elements.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Widget, Paragraph},
};

/// Button visual variants for different actions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    /// Primary action button (blue)
    Primary,
    /// Secondary action button (cyan)
    Secondary,
    /// Danger action button (red)
    Danger,
}

/// Button state for different interaction modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    /// Button is enabled and interactive
    Enabled,
    /// Button is disabled and not interactive
    Disabled,
    /// Button is currently active/selected
    Active,
}

/// A reusable button component.
///
/// Provides standardized rendering for buttons with consistent
/// colors, states, and layout patterns.
pub struct Button<'a> {
    /// Button label text
    pub label: &'a str,
    /// Visual variant that determines button colors
    pub variant: ButtonVariant,
    /// Current state of the button
    pub state: ButtonState,
    /// Optional icon to display before the label
    pub icon: Option<&'a str>,
}

impl<'a> Button<'a> {
    /// Create a new button with the given parameters
    pub fn new(label: &'a str, variant: ButtonVariant) -> Self {
        Self {
            label,
            variant,
            state: ButtonState::Enabled,
            icon: None,
        }
    }

    /// Set the button state
    #[must_use]
    pub fn with_state(mut self, state: ButtonState) -> Self {
        self.state = state;
        self
    }

    /// Set the button icon
    #[must_use]
    pub fn with_icon(mut self, icon: &'a str) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Get the background color for this button based on state
    pub fn bg_color(&self) -> Color {
        match (self.state, self.variant) {
            (ButtonState::Enabled, ButtonVariant::Primary) => Color::Blue,
            (ButtonState::Enabled, ButtonVariant::Secondary) => Color::Cyan,
            (ButtonState::Enabled, ButtonVariant::Danger) => Color::Red,
            (ButtonState::Disabled, _) => Color::Rgb(80, 80, 80),
            (ButtonState::Active, ButtonVariant::Primary) => Color::Rgb(0, 80, 180),
            (ButtonState::Active, ButtonVariant::Secondary) => Color::Rgb(0, 120, 180),
            (ButtonState::Active, ButtonVariant::Danger) => Color::Rgb(150, 0, 0),
        }
    }

    /// Get the foreground (text) color for this button based on state
    pub fn fg_color(&self) -> Color {
        match self.state {
            ButtonState::Enabled | ButtonState::Active => Color::White,
            ButtonState::Disabled => Color::Rgb(170, 170, 170),
        }
    }

    /// Get the padding for button content
    pub fn padding(&self) -> (u16, u16) {
        (4, 2) // (horizontal, vertical)
    }

    /// Calculate the button width
    pub fn width(&self) -> u16 {
        let label_len = self.label.chars().count() as u16;
        let icon_len = self.icon.map(|i| i.chars().count() as u16).unwrap_or(0);
        let (h_pad, _) = self.padding();
        label_len + icon_len + (h_pad * 2)
    }
}

  /// Rendered button that can be displayed
  pub struct ButtonRender<'a, 'b> {
      button: &'b Button<'a>,
  }
      impl<'a, 'b> ButtonRender<'a, 'b> {
          /// Create a new button render
          pub fn new(button: &'b Button<'a>, _area: Rect) -> Self {
              Self { button }
          }    /// Get the content lines for this button
    fn content(&self) -> Vec<Line<'a>> {
        let mut lines = Vec::new();

        let icon = self.button.icon.unwrap_or("");
        let text = format!("{}{}", icon, self.button.label);

        lines.push(Line::from(Span::styled(
            text,
            Style::default()
                .fg(self.button.fg_color())
                .add_modifier(ratatui::style::Modifier::BOLD),
        )));

        lines
    }
}

    impl Widget for ButtonRender<'_, '_> {
          fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer)
          where
              Self: Sized,
          {          let content = self.content();

          // Create block with appropriate styling
          let block = Block::default()
              .borders(Borders::ALL)
              .border_style(Style::default().fg(self.button.bg_color()));

          // Create paragraph with content
          let paragraph = Paragraph::new(content)
              .block(block)
              .alignment(ratatui::layout::Alignment::Center);

          paragraph.render(area, buf);
      }
  }
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_button_creation() {
        let button = Button::new("Click Me", ButtonVariant::Primary);
        assert_eq!(button.label, "Click Me");
        assert_eq!(button.variant, ButtonVariant::Primary);
        assert_eq!(button.state, ButtonState::Enabled);
        assert_eq!(button.icon, None);
    }

    #[test]
    fn test_button_state() {
        let button = Button::new("Click Me", ButtonVariant::Primary)
            .with_state(ButtonState::Active);
        assert_eq!(button.state, ButtonState::Active);
    }

    #[test]
    fn test_button_width() {
        let button = Button::new("OK", ButtonVariant::Primary);
        assert_eq!(button.width(), 10); // 2 chars + 4*2 padding
    }
}
