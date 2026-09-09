//! Tests for the last-prompt tag rendered on the status bar's top line.
//!
//! Covers:
//! - `prompt_display_text` truncation semantics (first 32 chars, `....`
//!   suffix when longer, empty string for an empty prompt).
//! - The tag renders in square brackets on the first status-bar line.
//! - The tag renders immediately after the `Branch: `-labelled git branch
//!   section, as one group placed right after the cwd display.
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

/// Deterministic cwd for layout tests: longer than the Full-mode 25-column
/// pad so no filler padding is inserted between the path and the branch.
const TEST_CWD: &str = "~/demo/a/very/long/project/path";

#[test]
fn test_statusbar_tag_group_follows_branch_after_cwd() {
    let mut app = support::make_app();
    app.cwd = TEST_CWD.to_string();
    app.last_prompt = "hello world".to_string();
    app.git_branch = Some("main".to_string());
    let frame = render_app_to_string(&mut app);
    let line1 = frame.lines().next().unwrap_or("");
    let cwd_col = char_col_of(line1, TEST_CWD);
    let branch_col = char_col_of(line1, "Branch: main");
    let icon_end = char_col_of(line1, "Branch: main ●") + "Branch: main ●".chars().count();
    let tag_col = char_col_of(line1, "[hello world]");
    assert!(
        cwd_col < branch_col,
        "cwd (col {cwd_col}) must render before the branch label (col {branch_col}); \
         line was: {line1}"
    );
    // Branch label sits immediately after the cwd path: one separator space.
    assert_eq!(
        branch_col,
        cwd_col + TEST_CWD.len() + 1,
        "branch label must render immediately after the cwd path; line was: {line1}"
    );
    assert!(
        branch_col < tag_col,
        "branch label (col {branch_col}) must render before the prompt tag \
         (col {tag_col}); line was: {line1}"
    );
    // Tag sits immediately after the branch status icon: one separator space.
    assert_eq!(
        tag_col,
        icon_end + 1,
        "tag must render immediately after the branch section; line was: {line1}"
    );
}

#[test]
fn test_statusbar_tag_group_shortens_cwd_to_fit() {
    let mut app = support::make_app();
    app.cwd = format!("~/{}/{}", "d".repeat(60), "e".repeat(40));
    app.last_prompt = "x".repeat(40); // 38-column tag
    app.git_branch = Some("main".to_string());
    let frame = render_app_to_string(&mut app);
    let line1 = frame.lines().next().unwrap_or("");
    let expected = format!("[{}....]", "x".repeat(32));
    assert!(
        line1.contains(&expected),
        "tag must still render when the cwd is shortened; line was: {line1}"
    );
    assert!(
        line1.contains("Branch: main"),
        "branch label must render when the cwd is shortened; line was: {line1}"
    );
    let full_cwd = format!("~/{}/{}", "d".repeat(60), "e".repeat(40));
    assert!(
        !line1.contains(&full_cwd),
        "over-long cwd must be shortened; line was: {line1}"
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
    let branch_col = char_col_of(line1, "Branch: main");
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
    // Branch label sits immediately after the cwd path; the tag follows the
    // branch section with a single separator space (no centring gap).
    let icon_end = char_col_of(line1, "Branch: main ●") + "Branch: main ●".chars().count();
    assert_eq!(
        tag_col,
        icon_end + 1,
        "tag must sit immediately after the branch section; line was: {line1}"
    );
}
