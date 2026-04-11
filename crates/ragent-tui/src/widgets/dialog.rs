//! Dialog component for ragent TUI.
//!
//! Provides a standardized dialog component with various visual variants
//! for different types of modal dialogs in the UI.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// Dialog visual variants for different use cases
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogVariant {
    /// Info dialog (cyan border)
    Info,
    /// Warning dialog (yellow border)
    Warning,
    /// Danger dialog (red border)
    Danger,
    /// Success dialog (green border)
    Success,
}

/// Dialog size presets
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogSize {
    /// Small dialog: 60x30
    Small,
    /// Medium dialog: 70x40
    Medium,
    /// Large dialog: 90x70
    Large,
    /// Custom size specified as (width, height) percentages
    Custom(u16, u16),
}

/// Dialog alignment options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogAlignment {
    /// Center the dialog in the available area
    Center,
    /// Align dialog to the top
    Top,
    /// Align dialog to the bottom
    Bottom,
}

/// A reusable dialog component.
///
/// Provides standardized rendering for modal dialogs with consistent
/// border colors, titles, and layout patterns.
pub struct Dialog {
    /// Dialog title
    pub title: String,
    /// Visual variant that determines border color
    pub variant: DialogVariant,
    /// Size preset or custom dimensions
    pub size: DialogSize,
    /// Alignment of the dialog in the available area
    pub alignment: DialogAlignment,
}

impl Dialog {
    /// Create a new dialog with the given parameters
    pub fn new(title: impl Into<String>, variant: DialogVariant) -> Self {
        Self {
            title: title.into(),
            variant,
            size: DialogSize::Medium,
            alignment: DialogAlignment::Center,
        }
    }

    /// Set the dialog size
    #[must_use]
    pub fn with_size(mut self, size: DialogSize) -> Self {
        self.size = size;
        self
    }

    /// Set the dialog alignment
    #[must_use]
    pub fn with_alignment(mut self, alignment: DialogAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Get the border color based on the variant
    pub fn border_color(&self) -> Color {
        match self.variant {
            DialogVariant::Info => crate::theme::colors::DIALOG_INFO,
            DialogVariant::Warning => crate::theme::colors::DIALOG_WARNING,
            DialogVariant::Danger => crate::theme::colors::DIALOG_DANGER,
            DialogVariant::Success => crate::theme::colors::DIALOG_SUCCESS,
        }
    }

    /// Get the title style for this dialog
    pub fn title_style(&self) -> Style {
        Style::default()
            .fg(self.border_color())
            .add_modifier(ratatui::style::Modifier::BOLD)
    }

    /// Get the block style for this dialog
    pub fn block_style(&self) -> Style {
        Style::default().fg(self.border_color())
    }
}

/// Dialog content types that can be rendered
#[derive(Debug, Clone)]
pub enum DialogContent<'a> {
    /// Plain text content
    Text(String),
    /// Formatted lines content
    Lines(Vec<Line<'a>>),
    /// Paragraph content
    Paragraph(Box<Paragraph<'a>>),
}

    impl<'a> Dialog {
        /// Render the dialog with the given content
        pub fn render<'b>(&'b self, _area: Rect, content: DialogContent<'a>) -> DialogRender<'a, 'b> {
            DialogRender {
                dialog: self,
                content,
            }
        }
    }  /// A rendered dialog that can be displayed
  pub struct DialogRender<'a, 'b> {
      dialog: &'b Dialog,
      content: DialogContent<'a>,
  }
impl Widget for DialogRender<'_, '_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer)
    where
        Self: Sized,
    {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(&self.dialog.title, self.dialog.title_style()))
            .border_style(self.dialog.block_style());

        let content_widget = match &self.content {
            DialogContent::Text(text) => Paragraph::new(text.clone()).block(block),
            DialogContent::Lines(lines) => Paragraph::new(lines.clone()).block(block),
            DialogContent::Paragraph(p) => (**p).clone().block(block),
        };

        content_widget.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dialog_creation() {
        let dialog = Dialog::new("Test Dialog", DialogVariant::Info);
        assert_eq!(dialog.title, "Test Dialog");
        assert_eq!(dialog.variant, DialogVariant::Info);
    }

    #[test]
    fn test_dialog_border_color() {
        let dialog = Dialog::new("Test", DialogVariant::Danger);
        assert_eq!(dialog.border_color(), Color::Red);
    }
}
