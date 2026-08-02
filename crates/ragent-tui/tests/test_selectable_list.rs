//! External tests for `tests` from `crates/ragent-tui/src/widgets/selectable_list.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_tui::widgets::selectable_list::*;

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
