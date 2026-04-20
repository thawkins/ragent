//! Button component for ragent TUI.
//!
//! Provides a standardized button component with various visual variants
//! for different actions in the TUI.
//!
//! # Example
//!
//! ```
//! use ratatui::style::Color;
//! use ragent_tui::widgets::button::{Button, ButtonVariant, ButtonState};
//!
//! // Primary button
//! let button = Button::new("OK", ButtonVariant::Primary);
//!
//! // Secondary button with disabled state
//! let button = Button::new("Cancel", ButtonVariant::Secondary)
//!     .with_state(ButtonState::Disabled);
//!
//! // Danger button with keyboard shortcut
//! let button = Button::with_shortcut("Delete", 'd', ButtonVariant::Danger);
//! ```

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Widget},
};

use crate::theme;

/// Button visual variants for different actions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    /// Primary action button (focus color)
    Primary,
    /// Secondary action button (primary color)
    Secondary,
    /// Danger action button (danger color)
    Danger,
    /// Success action button (success color)
    Success,
}

impl ButtonVariant {
    /// Get the background color for this variant (enabled state)
    pub fn enabled_bg_color(self) -> Color {
        match self {
            ButtonVariant::Primary => theme::colors::FOCUS_COLOR,
            ButtonVariant::Secondary => theme::colors::PRIMARY,
            ButtonVariant::Danger => theme::colors::DIALOG_DANGER,
            ButtonVariant::Success => theme::colors::DIALOG_SUCCESS,
        }
    }

    /// Get the background color for this variant (active/focused state)
    pub fn active_bg_color(self) -> Color {
        match self {
            ButtonVariant::Primary => theme::colors::FOCUS_COLOR,
            ButtonVariant::Secondary => theme::colors::PRIMARY,
            ButtonVariant::Danger => theme::colors::DIALOG_DANGER,
            ButtonVariant::Success => theme::colors::DIALOG_SUCCESS,
        }
    }

    /// Get the background color for this variant (disabled state)
    pub fn disabled_bg_color(self) -> Color {
        theme::colors::BORDER_INACTIVE
    }
}

/// Button state for different interaction modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    /// Button is enabled and interactive
    Enabled,
    /// Button is disabled and not interactive
    Disabled,
    /// Button is currently active/selected (hovered or keyboard focused)
    Active,
}

/// A reusable button component.
///
/// Provides standardized rendering for buttons with consistent
/// border colors, styling patterns, and focus indicators.
#[derive(Debug, Clone)]
pub struct Button<'a> {
    /// Button label text
    pub label: &'a str,
    /// Visual variant that determines button colors
    pub variant: ButtonVariant,
    /// Current state of the button
    pub state: ButtonState,
    /// Optional keyboard shortcut character
    pub shortcut: Option<char>,
    /// Whether to show a border
    pub bordered: bool,
    /// Button width (auto-calculated if None)
    pub width: Option<u16>,
    /// Button height (default: 1)
    pub height: u16,
}

impl<'a> Button<'a> {
    /// Create a new button with the given parameters
    ///
    /// # Example
    ///
    /// ```
    /// use ragent_tui::widgets::button::{Button, ButtonVariant};
    ///
    /// let button = Button::new("Click Me", ButtonVariant::Primary);
    /// ```
    pub fn new(label: &'a str, variant: ButtonVariant) -> Self {
        Self {
            label,
            variant,
            state: ButtonState::Enabled,
            shortcut: None,
            bordered: true,
            width: None,
            height: 1,
        }
    }

    /// Create a button with a keyboard shortcut
    ///
    /// # Example
    ///
    /// ```
    /// use ragent_tui::widgets::button::{Button, ButtonVariant};
    ///
    /// let button = Button::with_shortcut("Yes", 'y', ButtonVariant::Primary);
    /// ```
    pub fn with_shortcut(label: &'a str, shortcut: char, variant: ButtonVariant) -> Self {
        Self {
            label,
            variant,
            state: ButtonState::Enabled,
            shortcut: Some(shortcut),
            bordered: true,
            width: None,
            height: 1,
        }
    }

    /// Set the button state
    #[must_use]
    pub fn with_state(mut self, state: ButtonState) -> Self {
        self.state = state;
        self
    }

    /// Set the button shortcut character
    #[must_use]
    pub fn with_shortcut_char(mut self, shortcut: char) -> Self {
        self.shortcut = Some(shortcut);
        self
    }

    /// Set whether the button should show a border
    #[must_use]
    pub fn bordered(mut self, bordered: bool) -> Self {
        self.bordered = bordered;
        self
    }

    /// Set a fixed width for the button (default is auto-calculated)
    #[must_use]
    pub fn with_width(mut self, width: u16) -> Self {
        self.width = Some(width);
        self
    }

    /// Set the button height (default: 1)
    #[must_use]
    pub fn with_height(mut self, height: u16) -> Self {
        self.height = height.max(1);
        self
    }

    /// Get the background color for this button based on state
    pub fn bg_color(&self) -> Color {
        match self.state {
            ButtonState::Enabled => self.variant.enabled_bg_color(),
            ButtonState::Active => self.variant.active_bg_color(),
            ButtonState::Disabled => self.variant.disabled_bg_color(),
        }
    }

    /// Get the foreground (text) color for this button based on state
    pub fn fg_color(&self) -> Color {
        match self.state {
            ButtonState::Enabled | ButtonState::Active => theme::colors::TEXT_PRIMARY,
            ButtonState::Disabled => theme::colors::HINT,
        }
    }

    /// Get the border color for this button based on state
    pub fn border_color(&self) -> Color {
        match self.state {
            ButtonState::Enabled => self.variant.enabled_bg_color(),
            ButtonState::Active => theme::colors::FOCUS_COLOR,
            ButtonState::Disabled => theme::colors::BORDER_INACTIVE,
        }
    }

    /// Get the horizontal padding for button content
    pub fn horizontal_padding(&self) -> u16 {
        if self.bordered { 2 } else { 1 }
    }

    /// Get the vertical padding for button content
    pub fn vertical_padding(&self) -> u16 {
        if self.bordered { 1 } else { 0 }
    }

    /// Calculate the content width
    fn content_width(&self) -> u16 {
        let label_width = self.label.chars().count() as u16;
        if let Some(_shortcut) = self.shortcut {
            // [X]Label format
            4 + label_width // "[X]" + label
        } else {
            label_width
        }
    }

    /// Calculate the total button width
    pub fn total_width(&self) -> u16 {
        if let Some(w) = self.width {
            w.max(4) // Minimum width of 4
        } else {
            self.content_width() + (self.horizontal_padding() * 2)
        }
    }

    /// Calculate the total button height
    pub fn total_height(&self) -> u16 {
        self.height + (self.vertical_padding() * 2)
    }

    /// Get the styled label text for rendering
    ///
    /// Includes accessibility annotations for screen reader support:
    /// - ⣿ prefix indicates button role
    pub fn styled_label(&self) -> Line<'_> {
        let fg = self.fg_color();
        let bg = self.bg_color();

        if let Some(shortcut) = self.shortcut {
            let style = Style::default().fg(fg).bg(bg);
            let shortcut_style = if self.state == ButtonState::Active {
                style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                style
            };
            Line::from(vec![
                Span::styled("[", style),
                Span::styled(shortcut.to_string(), shortcut_style),
                Span::styled("]", style),
                Span::styled(self.label, style),
            ])
        } else {
            let style = Style::default().fg(fg).bg(bg);
            let label_style = if self.state == ButtonState::Active {
                style.add_modifier(Modifier::BOLD)
            } else {
                style
            };
            Line::from(Span::styled(self.label, label_style))
        }
    }

    /// Get the styled label with button role indicator for accessibility
    ///
    /// Includes "⣿" prefix to indicate button role for screen readers.
    /// Use this in contexts where multiple element types are displayed together.
    pub fn styled_label_with_role(&self) -> Line<'_> {
        use crate::theme::accessibility;

        let fg = self.fg_color();
        let bg = self.bg_color();
        let role_style = Style::default().fg(fg).bg(bg);

        if let Some(shortcut) = self.shortcut {
            let style = Style::default().fg(fg).bg(bg);
            let shortcut_style = if self.state == ButtonState::Active {
                style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
            } else {
                style
            };
            Line::from(vec![
                Span::styled(accessibility::ROLE_BUTTON, role_style),
                Span::styled("[", style),
                Span::styled(shortcut.to_string(), shortcut_style),
                Span::styled("]", style),
                Span::styled(self.label, style),
            ])
        } else {
            let style = Style::default().fg(fg).bg(bg);
            Line::from(vec![
                Span::styled(accessibility::ROLE_BUTTON, role_style),
                Span::styled(self.label, style),
            ])
        }
    }

    /// Get a Span representation for use in button bars
    pub fn to_span(&self) -> Span<'_> {
        let fg = self.fg_color();
        let bg = self.bg_color();
        let style = Style::default().fg(fg).bg(bg);

        if let Some(shortcut) = self.shortcut {
            let shortcut_style = if self.state == ButtonState::Active {
                style.add_modifier(Modifier::BOLD)
            } else {
                style
            };
            Span::styled(format!("[{}]{} ", shortcut, self.label), shortcut_style)
        } else {
            Span::styled(format!("{} ", self.label), style)
        }
    }
}

/// A row of buttons arranged horizontally
#[derive(Debug, Clone)]
pub struct ButtonBar<'a> {
    /// The buttons in this row
    pub buttons: Vec<Button<'a>>,
    /// Spacing between buttons (default: 2)
    pub button_spacing: u16,
    /// Alignment of the button bar
    pub alignment: Alignment,
}

impl<'a> ButtonBar<'a> {
    /// Create a new button bar
    pub fn new() -> Self {
        Self {
            buttons: Vec::new(),
            button_spacing: 2,
            alignment: Alignment::Center,
        }
    }

    /// Create a button bar with the given buttons
    pub fn with_buttons(buttons: Vec<Button<'a>>) -> Self {
        Self {
            buttons,
            button_spacing: 2,
            alignment: Alignment::Center,
        }
    }

    /// Set the spacing between buttons
    #[must_use]
    pub fn with_spacing(mut self, spacing: u16) -> Self {
        self.button_spacing = spacing;
        self
    }

    /// Set the alignment of the button bar
    #[must_use]
    pub fn with_alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Add a button to the bar
    #[must_use]
    pub fn push(mut self, button: Button<'a>) -> Self {
        self.buttons.push(button);
        self
    }

    /// Calculate the total width needed for this button bar
    pub fn total_width(&self) -> u16 {
        if self.buttons.is_empty() {
            return 0;
        }
        let buttons_width: u16 = self.buttons.iter().map(|b| b.total_width()).sum();
        let spacing: u16 = if self.buttons.len() > 1 {
            (self.buttons.len() - 1) as u16 * self.button_spacing
        } else {
            0
        };
        buttons_width + spacing
    }

    /// Calculate the maximum height needed for this button bar
    pub fn max_height(&self) -> u16 {
        self.buttons
            .iter()
            .map(|b| b.total_height())
            .max()
            .unwrap_or(1)
    }

    /// Render the button bar to a frame at the given area
    pub fn render_to_frame(&self, frame: &mut ratatui::Frame, area: Rect) {
        if self.buttons.is_empty() {
            return;
        }

        let total_width = self.total_width();
        let available_width = area.width;

        // Calculate starting X position based on alignment
        let start_x = match self.alignment {
            Alignment::Left => area.x,
            Alignment::Center => {
                if total_width >= available_width {
                    area.x
                } else {
                    area.x + (available_width - total_width) / 2
                }
            }
            Alignment::Right => {
                if total_width >= available_width {
                    area.x
                } else {
                    area.x + available_width - total_width
                }
            }
        };

        let mut current_x = start_x;
        let max_height = self.max_height();
        let y = area.y + (area.height.saturating_sub(max_height)) / 2;

        for (i, button) in self.buttons.iter().enumerate() {
            // Check if we have room for this button
            let button_width = button
                .total_width()
                .min(available_width.saturating_sub(current_x - area.x));
            if button_width == 0 {
                break;
            }

            let button_area = Rect::new(
                current_x,
                y,
                button_width,
                button.total_height().min(area.height),
            );

            frame.render_widget(ButtonRender::new(button, button_area), button_area);

            // Move to next button position
            if i < self.buttons.len() - 1 {
                current_x = current_x.saturating_add(button_width + self.button_spacing);
            }
        }
    }
}

impl Default for ButtonBar<'_> {
    fn default() -> Self {
        Self::new()
    }
}

/// Rendered button that can be displayed as a widget
pub struct ButtonRender<'a, 'b> {
    button: &'b Button<'a>,
    _area: Rect,
}

impl<'a, 'b> ButtonRender<'a, 'b> {
    /// Create a new button render
    pub fn new(button: &'b Button<'a>, area: Rect) -> Self {
        Self {
            button,
            _area: area,
        }
    }
}

impl Widget for ButtonRender<'_, '_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let bg = self.button.bg_color();
        let fg = self.button.fg_color();
        let border_color = self.button.border_color();

        // Clear the area with background color
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    cell.set_style(Style::default().bg(bg));
                }
            }
        }

        // Render border if enabled
        if self.button.bordered && area.width >= 4 && area.height >= 1 {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color));

            // Render content inside the border
            let inner = block.clone().inner(area);
            block.clone().render(area, buf);
            if inner.width > 0 && inner.height > 0 {
                self.render_content(inner, buf, fg, bg);
            }
        } else {
            // Render content directly
            self.render_content(area, buf, fg, bg);
        }
    }
}

impl ButtonRender<'_, '_> {
    fn render_content(&self, area: Rect, buf: &mut Buffer, _fg: Color, bg: Color) {
        let line = self.button.styled_label();

        // Center the content vertically
        let content_y = area.y + area.height.saturating_sub(1) / 2;
        if content_y >= area.bottom() {
            return;
        }

        // Calculate horizontal position based on content width
        let content_width = self.button.content_width();
        let start_x = if content_width >= area.width {
            area.x
        } else {
            area.x + (area.width - content_width) / 2
        };

        // Render the text
        let mut x = start_x;
        for span in line.spans {
            let text = span.content.to_string();
            let span_style = span.style;
            for ch in text.chars() {
                if x >= area.right() {
                    break;
                }
                if let Some(cell) = buf.cell_mut((x, content_y)) {
                    cell.set_char(ch);
                    cell.set_style(span_style.patch(Style::default().bg(bg)));
                }
                x += 1;
            }
        }
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
        assert!(button.shortcut.is_none());
    }

    #[test]
    fn test_button_with_shortcut() {
        let button = Button::with_shortcut("Yes", 'y', ButtonVariant::Primary);
        assert_eq!(button.shortcut, Some('y'));
        assert_eq!(button.label, "Yes");
    }

    #[test]
    fn test_button_state() {
        let button =
            Button::new("Click Me", ButtonVariant::Primary).with_state(ButtonState::Active);
        assert_eq!(button.state, ButtonState::Active);
    }

    #[test]
    fn test_button_width_calculation() {
        let button = Button::new("OK", ButtonVariant::Primary);
        // "OK" + 2*2 padding = 6, but minimum should be respected
        assert_eq!(button.total_width(), 6);

        let button_with_shortcut = Button::with_shortcut("Yes", 'y', ButtonVariant::Primary);
        // "[y]Yes" = 6 + 2*2 padding = 10
        assert_eq!(button_with_shortcut.total_width(), 10);
    }

    #[test]
    fn test_button_bar_creation() {
        let bar = ButtonBar::new()
            .push(Button::new("OK", ButtonVariant::Primary))
            .push(Button::new("Cancel", ButtonVariant::Secondary));

        assert_eq!(bar.buttons.len(), 2);
        assert_eq!(bar.button_spacing, 2);
        assert_eq!(bar.alignment, Alignment::Center);
    }

    #[test]
    fn test_button_bar_total_width() {
        let bar = ButtonBar::new()
            .with_spacing(2)
            .push(Button::new("OK", ButtonVariant::Primary))
            .push(Button::new("Cancel", ButtonVariant::Secondary));

        let expected = 6 + 2 + 10; // OK(6) + spacing(2) + Cancel(10)
        assert_eq!(bar.total_width(), expected);
    }

    #[test]
    fn test_button_styled_label() {
        let button = Button::new("OK", ButtonVariant::Primary);
        let line = button.styled_label();
        assert_eq!(line.spans.len(), 1);

        let button_with_shortcut = Button::with_shortcut("Yes", 'y', ButtonVariant::Primary);
        let line = button_with_shortcut.styled_label();
        assert_eq!(line.spans.len(), 4); // "[", "y", "]", "Yes"
    }

    #[test]
    fn test_button_color_methods() {
        let primary = Button::new("Test", ButtonVariant::Primary);
        let secondary = Button::new("Test", ButtonVariant::Secondary);
        let danger = Button::new("Test", ButtonVariant::Danger);
        let success = Button::new("Test", ButtonVariant::Success);

        // Test that colors are different for different variants
        assert_ne!(
            primary.variant.enabled_bg_color(),
            secondary.variant.enabled_bg_color()
        );
        assert_ne!(
            danger.variant.enabled_bg_color(),
            success.variant.enabled_bg_color()
        );
    }
}
