//! Coverage tests for `ragent_tui::input_field::InputField` and the global
//! quit-key predicate in `ragent_tui::input`.
//!
//! These exercise the self-contained single-line editor used by the provider
//! setup / router dialogs: cursor movement (char/word/home/end), keyboard
//! selection, cut/copy/paste, and the full `handle_key` key-event matrix.
//! No production code is touched.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use ragent_tui::clipboard::ClipboardTestOverrideGuard;
use ragent_tui::input::is_global_quit_key;
use ragent_tui::input_field::InputField;

fn k(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn kmod(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

// ---------------------------------------------------------------------------
// Construction / accessors
// ---------------------------------------------------------------------------

#[test]
fn new_is_empty_and_default() {
    let f = InputField::new();
    assert!(f.is_empty());
    assert_eq!(f.len_chars(), 0);
    assert_eq!(f.cursor(), 0);
    assert_eq!(f.text(), "");
    assert_eq!(f, InputField::default());
}

#[test]
fn with_text_places_cursor_at_end() {
    let f = InputField::with_text("hello");
    assert_eq!(f.text(), "hello");
    assert_eq!(f.cursor(), 5);
    assert!(!f.is_empty());
    assert_eq!(f.len_chars(), 5);
}

#[test]
fn with_text_multibyte_len_chars_counts_chars() {
    let f = InputField::with_text("aé中🚀");
    assert_eq!(f.len_chars(), 4);
    assert_eq!(f.cursor(), 4);
}

#[test]
fn set_cursor_clamps_to_len() {
    let mut f = InputField::with_text("abc");
    f.set_cursor(99);
    assert_eq!(f.cursor(), 3);
    f.set_cursor(1);
    assert_eq!(f.cursor(), 1);
}

#[test]
fn text_mut_can_be_edited_directly() {
    let mut f = InputField::new();
    f.text_mut().push_str("direct");
    assert_eq!(f.text(), "direct");
}

// ---------------------------------------------------------------------------
// Insertion
// ---------------------------------------------------------------------------

#[test]
fn insert_char_middle_and_end() {
    let mut f = InputField::with_text("ac");
    f.set_cursor(1);
    f.insert_char('b');
    assert_eq!(f.text(), "abc");
    assert_eq!(f.cursor(), 2);
    f.insert_char('!');
    assert_eq!(f.text(), "ab!c");
    assert_eq!(f.cursor(), 3);
}

#[test]
fn insert_char_multibyte_updates_cursor_by_chars() {
    let mut f = InputField::new();
    f.insert_char('🚀');
    assert_eq!(f.text(), "🚀");
    assert_eq!(f.cursor(), 1);
    f.insert_char('x');
    assert_eq!(f.text(), "🚀x");
}

#[test]
fn insert_char_replaces_selection() {
    let mut f = InputField::with_text("hello");
    f.select_all();
    f.insert_char('Z');
    assert_eq!(f.text(), "Z");
    assert_eq!(f.cursor(), 1);
    assert!(f.selection_range().is_none());
}

#[test]
fn insert_str_empty_is_noop() {
    let mut f = InputField::with_text("keep");
    f.set_cursor(2);
    f.insert_str("");
    assert_eq!(f.text(), "keep");
    assert_eq!(f.cursor(), 2);
}

#[test]
fn insert_str_middle_and_multibyte() {
    let mut f = InputField::with_text("ab");
    f.set_cursor(1);
    f.insert_str("日本語");
    assert_eq!(f.text(), "a日本語b");
    assert_eq!(f.cursor(), 4);
}

#[test]
fn insert_str_replaces_selection() {
    let mut f = InputField::with_text("abcdef");
    f.set_cursor(2);
    f.select_right();
    f.select_right();
    f.insert_str("XY");
    assert_eq!(f.text(), "abXYef");
}

// ---------------------------------------------------------------------------
// Deletion
// ---------------------------------------------------------------------------

#[test]
fn backspace_at_start_is_noop() {
    let mut f = InputField::with_text("abc");
    f.set_cursor(0);
    f.backspace();
    assert_eq!(f.text(), "abc");
    assert_eq!(f.cursor(), 0);
}

#[test]
fn backspace_removes_prev_char_and_multibyte() {
    let mut f = InputField::with_text("a中");
    f.backspace();
    assert_eq!(f.text(), "a");
    assert_eq!(f.cursor(), 1);
}

#[test]
fn backspace_with_selection_removes_range_only() {
    let mut f = InputField::with_text("abcdef");
    f.set_cursor(1);
    f.select_right();
    f.select_right();
    f.backspace();
    assert_eq!(f.text(), "adef");
    assert!(f.selection_range().is_none());
}

#[test]
fn delete_at_or_past_end_is_noop() {
    let mut f = InputField::with_text("ab");
    f.set_cursor(2);
    f.delete();
    assert_eq!(f.text(), "ab");
}

#[test]
fn delete_removes_char_under_cursor() {
    let mut f = InputField::with_text("abc");
    f.set_cursor(1);
    f.delete();
    assert_eq!(f.text(), "ac");
    assert_eq!(f.cursor(), 1);
}

#[test]
fn delete_with_selection_removes_range() {
    let mut f = InputField::with_text("abcdef");
    f.select_all();
    f.delete();
    assert_eq!(f.text(), "");
    assert!(f.selection_range().is_none());
}

#[test]
fn delete_prev_word_and_delete_to_end() {
    let mut f = InputField::with_text("hello world");
    f.delete_prev_word();
    // Only the word body is removed; the separating space remains and the
    // cursor sits at the end of the text.
    assert_eq!(f.text(), "hello ");
    assert_eq!(f.cursor(), 6);
    // Delete-to-end from the end is a no-op.
    f.delete_to_end();
    assert_eq!(f.text(), "hello ");
    // Delete-to-end from before the trailing space removes it.
    f.set_cursor(5);
    f.delete_to_end();
    assert_eq!(f.text(), "hello");
}

#[test]
fn delete_prev_word_at_start_is_noop() {
    let mut f = InputField::with_text("abc");
    f.set_cursor(0);
    f.delete_prev_word();
    assert_eq!(f.text(), "abc");
}

#[test]
fn delete_to_end_at_end_is_noop() {
    let mut f = InputField::with_text("abc");
    f.delete_to_end();
    assert_eq!(f.text(), "abc");
}

// ---------------------------------------------------------------------------
// Cursor movement
// ---------------------------------------------------------------------------

#[test]
fn move_left_saturates_move_right_clamps() {
    let mut f = InputField::with_text("ab");
    f.move_left();
    assert_eq!(f.cursor(), 1);
    f.move_right();
    assert_eq!(f.cursor(), 2);
    f.move_right();
    assert_eq!(f.cursor(), 2);
    f.move_left();
    f.move_left();
    assert_eq!(f.cursor(), 0);
    f.move_left();
    assert_eq!(f.cursor(), 0);
}

#[test]
fn move_home_and_end() {
    let mut f = InputField::with_text("hello");
    f.move_home();
    assert_eq!(f.cursor(), 0);
    f.move_end();
    assert_eq!(f.cursor(), 5);
}

#[test]
fn move_word_left_from_end_skips_trailing_spaces_then_word() {
    let mut f = InputField::with_text("foo bar baz");
    f.move_word_left();
    assert_eq!(f.cursor(), 8); // start of "baz"
    f.move_word_left();
    assert_eq!(f.cursor(), 4); // start of "bar"
    f.move_word_left();
    assert_eq!(f.cursor(), 0);
    f.move_word_left();
    assert_eq!(f.cursor(), 0);
}

#[test]
fn move_word_right_skips_word_then_spaces() {
    let mut f = InputField::with_text("foo bar baz");
    f.set_cursor(0);
    f.move_word_right();
    assert_eq!(f.cursor(), 4);
    f.move_word_right();
    assert_eq!(f.cursor(), 8);
    f.move_word_right();
    assert_eq!(f.cursor(), 11);
    f.move_word_right();
    assert_eq!(f.cursor(), 11);
}

#[test]
fn move_word_left_from_mid_word_goes_to_word_start() {
    let mut f = InputField::with_text("hello world");
    f.set_cursor(9);
    f.move_word_left();
    assert_eq!(f.cursor(), 6);
}

// ---------------------------------------------------------------------------
// Selection
// ---------------------------------------------------------------------------

#[test]
fn select_left_and_right_build_range() {
    let mut f = InputField::with_text("abcd");
    f.set_cursor(2);
    f.select_left();
    assert_eq!(f.selection_range(), Some((1, 2)));
    f.select_left();
    assert_eq!(f.selection_range(), Some((0, 2)));
    f.select_right();
    assert_eq!(f.selection_range(), Some((1, 2)));
    f.select_right();
    assert_eq!(f.selection_range(), None);
    f.select_left();
    assert_eq!(f.selected_text().as_deref(), Some("b"));
}

#[test]
fn select_right_clamps_and_cursor_equal_anchor_is_none() {
    let mut f = InputField::with_text("ab");
    f.set_cursor(0);
    f.select_right();
    f.select_right();
    assert_eq!(f.cursor(), 2);
    assert_eq!(f.selection_range(), Some((0, 2)));
    // Moving the cursor back onto the anchor collapses the range to None.
    f.set_cursor(0);
    assert_eq!(f.selection_range(), None);
}

#[test]
fn select_all_and_clear_selection() {
    let mut f = InputField::with_text("hello");
    f.select_all();
    assert_eq!(f.selection_range(), Some((0, 5)));
    f.clear_selection();
    assert_eq!(f.selection_range(), None);
    assert_eq!(f.selected_text(), None);
}

#[test]
fn selected_text_multibyte() {
    let mut f = InputField::with_text("héllo");
    f.select_all();
    assert_eq!(f.selected_text().as_deref(), Some("héllo"));
}

// ---------------------------------------------------------------------------
// Clipboard
// ---------------------------------------------------------------------------

#[test]
fn paste_from_clipboard_inserts_override_text() {
    let _g = ClipboardTestOverrideGuard::new("pasted");
    let mut f = InputField::with_text("ab");
    f.set_cursor(1);
    f.paste_text_from_clipboard();
    assert_eq!(f.text(), "apastedb");
}

#[test]
fn paste_clipboard_alias_strips_carriage_returns() {
    let _g = ClipboardTestOverrideGuard::new("a\r\nb");
    let mut f = InputField::new();
    f.paste_clipboard();
    assert_eq!(f.text(), "a\nb");
}

#[test]
fn copy_and_cut_selection_do_not_panic() {
    let mut f = InputField::with_text("hello");
    f.select_all();
    f.copy_selection();
    f.cut_selection();
    assert_eq!(f.text(), "");
    // Cutting with no selection is a no-op.
    f.cut_selection();
    assert_eq!(f.text(), "");
    f.copy_selection();
}

// ---------------------------------------------------------------------------
// handle\_key matrix
// ---------------------------------------------------------------------------

#[test]
fn handle_key_ctrl_a_selects_all() {
    let mut f = InputField::with_text("abc");
    assert!(f.handle_key(kmod(KeyCode::Char('a'), KeyModifiers::CONTROL)));
    assert_eq!(f.selection_range(), Some((0, 3)));
}

#[test]
fn handle_key_ctrl_c_and_x_return_true() {
    let mut f = InputField::with_text("abc");
    f.select_all();
    assert!(f.handle_key(kmod(KeyCode::Char('c'), KeyModifiers::CONTROL)));
    assert!(f.handle_key(kmod(KeyCode::Char('x'), KeyModifiers::CONTROL)));
    assert_eq!(f.text(), "");
}

#[test]
fn handle_key_ctrl_v_pastes_override() {
    let _g = ClipboardTestOverrideGuard::new("Z");
    let mut f = InputField::new();
    assert!(f.handle_key(kmod(KeyCode::Char('v'), KeyModifiers::CONTROL)));
    assert_eq!(f.text(), "Z");
}

#[test]
fn handle_key_ctrl_shift_v_pastes_override() {
    let _g = ClipboardTestOverrideGuard::new("Y");
    let mut f = InputField::new();
    assert!(f.handle_key(kmod(
        KeyCode::Char('v'),
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    )));
    assert_eq!(f.text(), "Y");
}

#[test]
fn handle_key_ctrl_w_deletes_prev_word() {
    let mut f = InputField::with_text("foo bar");
    assert!(f.handle_key(kmod(KeyCode::Char('w'), KeyModifiers::CONTROL)));
    assert_eq!(f.text(), "foo ");
}

#[test]
fn handle_key_ctrl_k_deletes_to_end() {
    let mut f = InputField::with_text("abcdef");
    f.set_cursor(2);
    assert!(f.handle_key(kmod(KeyCode::Char('k'), KeyModifiers::CONTROL)));
    assert_eq!(f.text(), "ab");
}

#[test]
fn handle_key_ctrl_u_clears_all() {
    let mut f = InputField::with_text("abcdef");
    assert!(f.handle_key(kmod(KeyCode::Char('u'), KeyModifiers::CONTROL)));
    assert_eq!(f.text(), "");
    assert_eq!(f.cursor(), 0);
}

#[test]
fn handle_key_plain_and_shift_chars_insert() {
    let mut f = InputField::new();
    assert!(f.handle_key(k(KeyCode::Char('a'))));
    assert!(f.handle_key(kmod(KeyCode::Char('B'), KeyModifiers::SHIFT)));
    assert_eq!(f.text(), "aB");
}

#[test]
fn handle_key_backspace_and_delete() {
    let mut f = InputField::with_text("ab");
    assert!(f.handle_key(k(KeyCode::Backspace)));
    assert_eq!(f.text(), "a");
    f.set_cursor(0);
    assert!(f.handle_key(k(KeyCode::Delete)));
    assert_eq!(f.text(), "");
}

#[test]
fn handle_key_word_navigation_control() {
    let mut f = InputField::with_text("one two three");
    f.set_cursor(0);
    assert!(f.handle_key(kmod(KeyCode::Right, KeyModifiers::CONTROL)));
    assert_eq!(f.cursor(), 4);
    assert!(f.handle_key(kmod(KeyCode::Left, KeyModifiers::CONTROL)));
    assert_eq!(f.cursor(), 0);
}

#[test]
fn handle_key_shift_selection() {
    let mut f = InputField::with_text("abc");
    f.set_cursor(0);
    assert!(f.handle_key(kmod(KeyCode::Right, KeyModifiers::SHIFT)));
    assert_eq!(f.selection_range(), Some((0, 1)));
    assert!(f.handle_key(kmod(KeyCode::Left, KeyModifiers::SHIFT)));
    assert_eq!(f.cursor(), 0);
}

#[test]
fn handle_key_ctrl_shift_word_selection() {
    let mut f = InputField::with_text("one two three");
    f.set_cursor(0);
    assert!(f.handle_key(kmod(
        KeyCode::Right,
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    )));
    // Ctrl+Shift+Right moves the cursor by a whole word; the underlying
    // word-move helper clears the anchor, so no selection is retained.
    assert_eq!(f.cursor(), 4);
    assert!(f.selection_range().is_none());
    assert!(f.handle_key(kmod(
        KeyCode::Left,
        KeyModifiers::CONTROL | KeyModifiers::SHIFT,
    )));
    assert_eq!(f.cursor(), 0);
    assert!(f.selection_range().is_none());
}

#[test]
fn handle_key_plain_arrows_and_home_end() {
    let mut f = InputField::with_text("abc");
    assert!(f.handle_key(k(KeyCode::Left)));
    assert_eq!(f.cursor(), 2);
    assert!(f.handle_key(k(KeyCode::Right)));
    assert_eq!(f.cursor(), 3);
    assert!(f.handle_key(k(KeyCode::Home)));
    assert_eq!(f.cursor(), 0);
    assert!(f.handle_key(k(KeyCode::End)));
    assert_eq!(f.cursor(), 3);
}

#[test]
fn handle_key_ctrl_home_and_end() {
    let mut f = InputField::with_text("abc");
    assert!(f.handle_key(kmod(KeyCode::Home, KeyModifiers::CONTROL)));
    assert_eq!(f.cursor(), 0);
    assert!(f.handle_key(kmod(KeyCode::End, KeyModifiers::CONTROL)));
    assert_eq!(f.cursor(), 3);
}

#[test]
fn handle_key_unhandled_returns_false() {
    let mut f = InputField::new();
    assert!(!f.handle_key(k(KeyCode::F(5))));
    assert!(!f.handle_key(k(KeyCode::Tab)));
}

// ---------------------------------------------------------------------------
// Global quit-key predicate
// ---------------------------------------------------------------------------

#[test]
fn is_global_quit_key_matches_ctrl_c_and_ctrl_d_only() {
    assert!(is_global_quit_key(kmod(
        KeyCode::Char('c'),
        KeyModifiers::CONTROL
    )));
    assert!(is_global_quit_key(kmod(
        KeyCode::Char('d'),
        KeyModifiers::CONTROL
    )));
    assert!(!is_global_quit_key(k(KeyCode::Char('c'))));
    assert!(!is_global_quit_key(k(KeyCode::Char('x'))));
    assert!(!is_global_quit_key(kmod(
        KeyCode::Char('c'),
        KeyModifiers::ALT
    )));
    assert!(!is_global_quit_key(k(KeyCode::Enter)));
}
