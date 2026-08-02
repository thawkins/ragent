//! External tests for `tests` from `crates/ragent-tui/src/widgets/dialog.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_tui::widgets::dialog::*;
use ratatui::style::Color;

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
