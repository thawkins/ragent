//! Selectable list component for ragent TUI.
//!
//! Provides a reusable selectable list component for displaying and
//! navigating collections of items with keyboard navigation support.

use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Widget},
};

/// A selectable list component for displaying items with navigation.
///
/// Provides standardized rendering for lists with support for
/// keyboard navigation and selection highlighting.
pub struct SelectableList<'a, T: 'a> {
    /// List of items to display
    pub items: Vec<T>,
    /// Currently selected item index
    pub selected: usize,
    /// Function to convert item to display string
    pub display_fn: fn(&T) -> String,
    /// Optional prefix for selected items
    pub selected_prefix: &'a str,
    /// Optional prefix for unselected items
    pub unselected_prefix: &'a str,
}

impl<'a, T> SelectableList<'a, T> {
    /// Create a new selectable list with the given items
    ///
    /// Uses theme constants for accessibility-friendly selection indicators.
    /// The default selected prefix "▸ " is chosen for its visibility in screen readers.
    pub fn new(items: Vec<T>, display_fn: fn(&T) -> String) -> Self {
        use crate::theme::focus;

        Self {
            items,
            selected: 0,
            display_fn,
            selected_prefix: focus::SELECTED,
            unselected_prefix: focus::UNSELECTED,
        }
    }

    /// Create a new selectable list with focus indicator prefix
    ///
    /// Uses "◆ " as the selected prefix to indicate keyboard focus,
    /// which is more appropriate for focused lists in forms.
    pub fn new_focus_list(items: Vec<T>, display_fn: fn(&T) -> String) -> Self {
        use crate::theme::focus;

        Self {
            items,
            selected: 0,
            display_fn,
            selected_prefix: focus::FOCUSED,
            unselected_prefix: focus::UNSELECTED,
        }
    }

    /// Set the selected prefix (default: "▸ ")
    ///
    /// For accessibility, consider using theme constants:
    /// - `theme::focus::SELECTED` for selected items
    /// - `theme::focus::FOCUSED` for keyboard focus
    #[must_use]
    pub fn with_selected_prefix(mut self, prefix: &'a str) -> Self {
        self.selected_prefix = prefix;
        self
    }

    /// Set the unselected prefix (default: "  ")
    #[must_use]
    pub fn with_unselected_prefix(mut self, prefix: &'a str) -> Self {
        self.unselected_prefix = prefix;
        self
    }

    /// Select the next item
    pub fn next(&mut self) {
        if !self.items.is_empty() {
            self.selected = (self.selected + 1) % self.items.len();
        }
    }

    /// Select the previous item
    pub fn prev(&mut self) {
        if !self.items.is_empty() {
            self.selected = if self.selected == 0 {
                self.items.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    /// Get the current selection
    pub fn selection(&self) -> Option<&T> {
        self.items.get(self.selected)
    }

    /// Set the selection index
    pub fn set_selected(&mut self, index: usize) {
        if index < self.items.len() {
            self.selected = index;
        }
    }
}

/// Rendered selectable list that can be displayed
pub struct SelectableListRender<'a, T> {
    list: &'a SelectableList<'a, T>,
}
impl<'a, T> SelectableListRender<'a, T> {
    /// Create a new selectable list render
    pub fn new(list: &'a SelectableList<'a, T>, _area: Rect) -> Self {
        Self { list }
    }
    /// Get the list items for rendering
    fn list_items(&self) -> Vec<ListItem<'a>> {
        self.list
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let (prefix, style) = if i == self.list.selected {
                    (
                        self.list.selected_prefix,
                        Style::default().fg(Color::Yellow),
                    )
                } else {
                    (self.list.unselected_prefix, Style::default())
                };
                let text = format!("{}{}", prefix, (self.list.display_fn)(item));
                ListItem::new(Line::from(Span::styled(text, style)))
            })
            .collect()
    }
}

impl<T> Widget for SelectableListRender<'_, T> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer)
    where
        Self: Sized,
    {
        let list_items = self.list_items();
        let list = List::new(list_items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Rgb(170, 170, 170))),
            )
            .highlight_style(Style::default().add_modifier(ratatui::style::Modifier::REVERSED));

        let mut state = ListState::default();
        state.select(Some(self.list.selected));

        list.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_selectable_list_creation() {
        let items = vec!["Item 1", "Item 2", "Item 3"];
        let list = SelectableList::new(items, |s| s.to_string());
        assert_eq!(list.items.len(), 3);
        assert_eq!(list.selected, 0);
    }

    #[test]
    fn test_selectable_list_navigation() {
        let items = vec!["Item 1", "Item 2", "Item 3"];
        let mut list = SelectableList::new(items, |s| s.to_string());

        list.next();
        assert_eq!(list.selected, 1);

        list.next();
        assert_eq!(list.selected, 2);

        list.next();
        assert_eq!(list.selected, 0); // wraps around
    }

    #[test]
    fn test_selectable_list_prev() {
        let items = vec!["Item 1", "Item 2", "Item 3"];
        let mut list = SelectableList::new(items, |s| s.to_string());

        list.prev();
        assert_eq!(list.selected, 2); // wraps to end
    }
}
