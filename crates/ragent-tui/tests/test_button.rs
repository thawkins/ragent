//! External tests for `tests` from `crates/ragent-tui/src/widgets/button.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_tui::widgets::button::*;
use ratatui::layout::Alignment;

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
    let button = Button::new("Click Me", ButtonVariant::Primary).with_state(ButtonState::Active);
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
