//! Tests for the last-prompt tag rendered on the status bar's top line.
//!
//! Covers:
//! - `prompt_display_text` truncation semantics (first 32 chars, `....`
//!   suffix when longer, empty string for an empty prompt).
//! - The tag renders in square brackets on the first status-bar line.
//! - The tag is centred around the horizontal midpoint of the line.
//! - No tag renders when no prompt has been submitted yet.

use ragent_tui::layout_statusbar::prompt_display_text;
use ragent_tui::{App, layout};
use ratatui::{Terminal, backend::TestBackend};

#[path = "support/mod.rs"]
mod support;

/// Render the app at 120x40 and return the visible frame text.
fn render_app_to_string(app: &mut App) -> String {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("render app");
    let rows: Vec<String> = (0..40)
        .map(|y| {
            (0..120)
                .map(|x| {
                    terminal
                        .backend()
                        .buffer()
                        .cell((x, y))
                        .map(|c| c.symbol().to_string())
                        .unwrap_or_default()
                })
                .collect::<String>()
        })
        .collect();
    rows.join("\n")
}

/// Find the character (column) index of `needle` in `row`.
///
/// `str::find` returns a byte offset; the status bar contains multi-byte
/// glyphs (`●`, `…`) so the byte offset is not the rendered column.
fn char_col_of(row: &str, needle: &str) -> usize {
    let byte_idx = row
        .find(needle)
        .unwrap_or_else(|| panic!("'{needle}' not found in row: {row}"));
    row[..byte_idx].chars().count()
}

// ─────────────────────────────────────────────────────────────────────────────
// prompt_display_text unit tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_prompt_display_text_empty_prompt() {
    assert_eq!(prompt_display_text("", 32), "");
}

#[test]
fn test_prompt_display_text_short_prompt_unchanged() {
    assert_eq!(prompt_display_text("hello world", 32), "[hello world]");
}

#[test]
fn test_prompt_display_text_exactly_32_chars_no_ellipsis() {
    let prompt = "a".repeat(32);
    assert_eq!(prompt_display_text(&prompt, 32), format!("[{prompt}]"));
}

#[test]
fn test_prompt_display_text_longer_than_32_gets_dots() {
    let prompt = "b".repeat(40);
    let expected = format!("[{}....]", "b".repeat(32));
    assert_eq!(prompt_display_text(&prompt, 32), expected);
}

#[test]
fn test_prompt_display_text_multibyte_chars_counted_as_chars() {
    // Truncation counts characters, not bytes: 32 multibyte chars + "....".
    let prompt = "é".repeat(40);
    let display = prompt_display_text(&prompt, 32);
    assert!(display.starts_with('['));
    assert!(display.ends_with("....]"));
    // 32 chars + "...." + 2 brackets.
    assert_eq!(display.chars().count(), 32 + 4 + 2);
}

// ─────────────────────────────────────────────────────────────────────────────
// Slash-command submissions populate the tag
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_execute_slash_command_sets_last_prompt() {
    let mut app = support::make_app();
    app.execute_slash_command("/about");
    assert_eq!(
        app.last_prompt, "/about",
        "slash command must be recorded as the last prompt so the status-bar \
         tag shows it"
    );
}

#[test]
fn test_execute_slash_command_trims_whitespace() {
    let mut app = support::make_app();
    app.execute_slash_command("  /agents  ");
    assert_eq!(app.last_prompt, "/agents");
}

#[test]
fn test_statusbar_renders_slash_command_tag() {
    let mut app = support::make_app();
    // `/about` runs without a tokio runtime and leaves a short status, so the
    // tag fits the line even with the status text on the right.
    app.execute_slash_command("/about");
    let frame = render_app_to_string(&mut app);
    let line1 = frame.lines().next().unwrap_or("");
    assert!(
        line1.contains("[/about]"),
        "top status-bar line must show the submitted slash command; \
         line was: {line1}"
    );
}

#[test]
fn test_statusbar_slash_command_tag_truncated_like_prompts() {
    let mut app = support::make_app();
    // Unknown command suffix is ignored; `/about` keeps the status short so
    // the tag has room to render.
    app.execute_slash_command(&format!("/about {}", "c".repeat(40)));
    let frame = render_app_to_string(&mut app);
    let line1 = frame.lines().next().unwrap_or("");
    let expected = format!("[/about {}....]", "c".repeat(25));
    assert!(
        line1.contains(&expected),
        "long slash command must be truncated with the .... suffix; \
         line was: {line1}"
    );
}

// ─────────────────────────────────────────────────────────────────��───────────
// Rendered status-bar integration tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_statusbar_renders_last_prompt_tag() {
    let mut app = support::make_app();
    app.last_prompt = "x".repeat(40);
    let frame = render_app_to_string(&mut app);
    let line1 = frame.lines().next().unwrap_or("");
    let expected = format!("[{}....]", "x".repeat(32));
    assert!(
        line1.contains(&expected),
        "top status-bar line must contain the truncated last-prompt tag \
         {expected}; line was: {line1}"
    );
}

#[test]
fn test_statusbar_last_prompt_tag_is_centered() {
    let mut app = support::make_app();
    // 40-char prompt -> tag is "[<32 chars>....]" = 32 + 4 + 2 = 38 columns.
    app.last_prompt = "x".repeat(40);
    let frame = render_app_to_string(&mut app);
    let line1 = frame.lines().next().unwrap_or("");
    let tag = format!("[{}....]", "x".repeat(32));
    let tag_width = tag.chars().count() as u16; // ASCII tag: chars == columns
    let expected_start = (120u16 / 2).saturating_sub(tag_width / 2) as usize;
    let found = char_col_of(line1, &tag);
    assert_eq!(
        found, expected_start,
        "tag must start at column {expected_start} so its midpoint sits on \
         the line's centre (col 60); line was: {line1}"
    );
}

#[test]
fn test_statusbar_short_prompt_tag_is_centered() {
    let mut app = support::make_app();
    // Short prompt: tag "[hello]" is 7 columns; centre start = 60 - 3 = 57.
    app.last_prompt = "hello".to_string();
    let frame = render_app_to_string(&mut app);
    let line1 = frame.lines().next().unwrap_or("");
    let found = char_col_of(line1, "[hello]");
    assert_eq!(
        found, 57,
        "short tag must be centred around column 60; line was: {line1}"
    );
}

#[test]
fn test_statusbar_no_prompt_no_tag() {
    let mut app = support::make_app();
    let frame = render_app_to_string(&mut app);
    let line1 = frame.lines().next().unwrap_or("");
    assert!(
        !line1.contains('['),
        "no bracket tag may render when no prompt has been submitted; \
         line was: {line1}"
    );
}

#[test]
fn test_statusbar_branch_before_tag_and_cwd_before_branch() {
    let mut app = support::make_app();
    app.last_prompt = "hello world".to_string();
    app.git_branch = Some("main".to_string());
    let frame = render_app_to_string(&mut app);
    let line1 = frame.lines().next().unwrap_or("");
    let cwd_col = char_col_of(line1, "~/");
    let branch_col = char_col_of(line1, "main");
    let tag_col = char_col_of(line1, "[hello world]");
    assert!(
        cwd_col < branch_col,
        "cwd (col {cwd_col}) must render before the git branch (col {branch_col}); \
         line was: {line1}"
    );
    assert!(
        branch_col < tag_col,
        "git branch (col {branch_col}) must render before the prompt tag \
         (col {tag_col}); line was: {line1}"
    );
    // Branch ends right before the tag: branch + status icon + gap of 1.
    assert!(
        tag_col - (branch_col + "main".len()) <= 4,
        "tag must sit immediately after the branch section (gap {} cols); \
         line was: {line1}",
        tag_col - (branch_col + "main".len())
    );
    // Tag stays centred: midpoint = 60, tag width 13 -> start col 54.
    assert_eq!(tag_col, 54, "tag must remain centred; line was: {line1}");
}
