//! Tests for Unicode cursor handling and editing invariants (Section 4.B).

#[path = "support/mod.rs"]
mod support;

// =========================================================================
// cursor_byte_pos_at_char_index
// =========================================================================

#[test]
fn test_cursor_byte_pos_ascii() {
    let mut app = support::make_app();
    app.input = "hello".into();
    assert_eq!(app.cursor_byte_pos_at_char_index(0), 0);
    assert_eq!(app.cursor_byte_pos_at_char_index(1), 1);
    assert_eq!(app.cursor_byte_pos_at_char_index(5), 5);
}

#[test]
fn test_cursor_byte_pos_emoji() {
    let mut app = support::make_app();
    // 🦀 is 4 bytes in UTF-8
    app.input = "a🦀b".into();
    assert_eq!(app.cursor_byte_pos_at_char_index(0), 0); // before 'a'
    assert_eq!(app.cursor_byte_pos_at_char_index(1), 1); // before '🦀'
    assert_eq!(app.cursor_byte_pos_at_char_index(2), 5); // before 'b'
    assert_eq!(app.cursor_byte_pos_at_char_index(3), 6); // end
}

#[test]
fn test_cursor_byte_pos_accented() {
    let mut app = support::make_app();
    // 'é' is 2 bytes (C3 A9)
    app.input = "café".into();
    assert_eq!(app.cursor_byte_pos_at_char_index(0), 0); // 'c'
    assert_eq!(app.cursor_byte_pos_at_char_index(1), 1); // 'a'
    assert_eq!(app.cursor_byte_pos_at_char_index(2), 2); // 'f'
    assert_eq!(app.cursor_byte_pos_at_char_index(3), 3); // 'é'
    assert_eq!(app.cursor_byte_pos_at_char_index(4), 5); // end
}

#[test]
fn test_cursor_byte_pos_cjk() {
    let mut app = support::make_app();
    // '中' is 3 bytes
    app.input = "中文".into();
    assert_eq!(app.cursor_byte_pos_at_char_index(0), 0);
    assert_eq!(app.cursor_byte_pos_at_char_index(1), 3);
    assert_eq!(app.cursor_byte_pos_at_char_index(2), 6);
}

#[test]
fn test_cursor_byte_pos_beyond_end_returns_len() {
    let mut app = support::make_app();
    app.input = "abc".into();
    assert_eq!(app.cursor_byte_pos_at_char_index(100), 3);
}

#[test]
fn test_cursor_byte_pos_empty_string() {
    let mut app = support::make_app();
    app.input.clear();
    assert_eq!(app.cursor_byte_pos_at_char_index(0), 0);
    assert_eq!(app.cursor_byte_pos_at_char_index(1), 0);
}

// =========================================================================
// insert_char_at_cursor — Unicode
// =========================================================================

#[test]
fn test_insert_char_emoji_at_start() {
    let mut app = support::make_app();
    app.input = "hello".into();
    app.input_cursor = 0;
    app.insert_char_at_cursor('🚀');
    assert_eq!(app.input, "🚀hello");
    assert_eq!(app.input_cursor, 1);
}

#[test]
fn test_insert_char_emoji_in_middle() {
    let mut app = support::make_app();
    app.input = "ab".into();
    app.input_cursor = 1;
    app.insert_char_at_cursor('中');
    assert_eq!(app.input, "a中b");
    assert_eq!(app.input_cursor, 2);
}

#[test]
fn test_insert_char_at_end_of_unicode() {
    let mut app = support::make_app();
    app.input = "🦀".into();
    app.input_cursor = 1; // after the emoji
    app.insert_char_at_cursor('!');
    assert_eq!(app.input, "🦀!");
    assert_eq!(app.input_cursor, 2);
}

// =========================================================================
// insert_text_at_cursor — Unicode
// =========================================================================

#[test]
fn test_insert_text_unicode_in_middle() {
    let mut app = support::make_app();
    app.input = "ab".into();
    app.input_cursor = 1;
    app.insert_text_at_cursor("日本語");
    assert_eq!(app.input, "a日本語b");
    assert_eq!(app.input_cursor, 4); // 1 + 3 chars
}

#[test]
fn test_insert_text_empty_is_noop() {
    let mut app = support::make_app();
    app.input = "hello".into();
    app.input_cursor = 2;
    app.insert_text_at_cursor("");
    assert_eq!(app.input, "hello");
    assert_eq!(app.input_cursor, 2);
}

#[test]
fn test_insert_text_mixed_emoji() {
    let mut app = support::make_app();
    app.input.clear();
    app.input_cursor = 0;
    app.insert_text_at_cursor("Hi 🌍!");
    assert_eq!(app.input, "Hi 🌍!");
    assert_eq!(app.input_cursor, 5); // H i space 🌍 !
}

// =========================================================================
// delete_prev_char — Unicode boundaries
// =========================================================================

#[test]
fn test_delete_prev_char_removes_emoji() {
    let mut app = support::make_app();
    app.input = "a🦀b".into();
    app.input_cursor = 2; // after '🦀'
    app.delete_prev_char();
    assert_eq!(app.input, "ab");
    assert_eq!(app.input_cursor, 1);
}

#[test]
fn test_delete_prev_char_removes_accented() {
    let mut app = support::make_app();
    app.input = "café".into();
    app.input_cursor = 4; // end
    app.delete_prev_char();
    assert_eq!(app.input, "caf");
    assert_eq!(app.input_cursor, 3);
}

#[test]
fn test_delete_prev_char_at_start_is_noop() {
    let mut app = support::make_app();
    app.input = "🦀".into();
    app.input_cursor = 0;
    app.delete_prev_char();
    assert_eq!(app.input, "🦀");
    assert_eq!(app.input_cursor, 0);
}

// =========================================================================
// delete_next_char — Unicode boundaries
// =========================================================================

#[test]
fn test_delete_next_char_removes_emoji() {
    let mut app = support::make_app();
    app.input = "a🦀b".into();
    app.input_cursor = 1; // before '🦀'
    app.delete_next_char();
    assert_eq!(app.input, "ab");
    assert_eq!(app.input_cursor, 1);
}

#[test]
fn test_delete_next_char_at_end_is_noop() {
    let mut app = support::make_app();
    app.input = "中".into();
    app.input_cursor = 1; // end
    app.delete_next_char();
    assert_eq!(app.input, "中");
    assert_eq!(app.input_cursor, 1);
}

#[test]
fn test_delete_next_char_cjk() {
    let mut app = support::make_app();
    app.input = "中文字".into();
    app.input_cursor = 0;
    app.delete_next_char();
    assert_eq!(app.input, "文字");
    assert_eq!(app.input_cursor, 0);
}

// =========================================================================
// remove_input_char_range — boundaries and Unicode
// =========================================================================

#[test]
fn test_remove_char_range_middle_unicode() {
    let mut app = support::make_app();
    app.input = "a🦀中b".into();
    app.input_cursor = 3;
    // Remove the emoji and CJK char (indices 1..3)
    app.remove_input_char_range(1, 3);
    assert_eq!(app.input, "ab");
    assert_eq!(app.input_cursor, 1); // clamped to start of range
}

#[test]
fn test_remove_char_range_start_eq_end_is_noop() {
    let mut app = support::make_app();
    app.input = "hello".into();
    app.input_cursor = 2;
    app.remove_input_char_range(2, 2);
    assert_eq!(app.input, "hello");
}

#[test]
fn test_remove_char_range_beyond_bounds() {
    let mut app = support::make_app();
    app.input = "abc".into();
    app.input_cursor = 3;
    // Range extends past input length — should clamp gracefully.
    app.remove_input_char_range(1, 100);
    assert_eq!(app.input, "a");
    assert_eq!(app.input_cursor, 1);
}

#[test]
fn test_remove_char_range_entire_string() {
    let mut app = support::make_app();
    app.input = "🌍🦀".into();
    app.input_cursor = 2;
    app.remove_input_char_range(0, 2);
    assert!(app.input.is_empty());
    assert_eq!(app.input_cursor, 0);
}

// =========================================================================
// input_len_chars — Unicode
// =========================================================================

#[test]
fn test_input_len_chars_ascii() {
    let mut app = support::make_app();
    app.input = "hello".into();
    assert_eq!(app.input_len_chars(), 5);
}

#[test]
fn test_input_len_chars_emoji() {
    let mut app = support::make_app();
    app.input = "🦀🌍🚀".into();
    assert_eq!(app.input_len_chars(), 3);
}

#[test]
fn test_input_len_chars_mixed() {
    let mut app = support::make_app();
    app.input = "café🦀".into();
    assert_eq!(app.input_len_chars(), 5); // c a f é 🦀
}
