//! External tests for `tests` from `crates/ragent-tui/src/widgets/message_widget.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_tui::widgets::message_widget::tool_input_summary;
use serde_json::json;

#[test]
fn test_team_tool_summary_includes_args() {
    let input = json!({
        "team_name": "alpha",
        "teammate_name": "reviewer-1",
        "agent_type": "general"
    });
    let summary = tool_input_summary("team_spawn", &input, "/tmp");
    // New format: "👥 spawn {agent_type}"
    assert!(summary.contains("👥 spawn"));
    assert!(summary.contains("general"));
}

#[test]
fn test_team_tool_summary_truncates_long_strings_with_three_dots() {
    let long = "x".repeat(160);
    let input = json!({
        "team_name": "alpha",
        "content": long
    });
    let summary = tool_input_summary("team_broadcast", &input, "/tmp");
    // New format: "👥 broadcast: {content}" with truncation
    assert!(summary.contains("👥 broadcast:"));
    // The string should be truncated (120 chars + "...")
    assert!(
        summary.len() <= 140, // "👥 broadcast: " + 120 + "..." with a little headroom
        "summary should be truncated, got: {summary} (len: {})",
        summary.len()
    );
}

#[test]
fn test_unknown_tool_summary_includes_args() {
    let input = json!({
        "path": "/tmp/file.txt",
        "limit": 10
    });
    let summary = tool_input_summary("some_new_tool", &input, "/tmp");
    assert!(summary.contains("path=\"/tmp/file.txt\""));
    assert!(summary.contains("limit=10"));
}

#[test]
fn test_read_tool_summary_uses_path_or_file_path() {
    // Canonical `path` parameter works.
    let input = json!({ "path": "/tmp/project/src/main.rs" });
    let summary = tool_input_summary("read", &input, "/tmp/project");
    assert!(summary.contains("📄 src/main.rs"), "got: {summary}");

    // Legacy/alias `file_path` parameter also works.
    let input = json!({ "file_path": "/tmp/project/Cargo.toml" });
    let summary = tool_input_summary("read", &input, "/tmp/project");
    assert!(summary.contains("📄 Cargo.toml"), "got: {summary}");

    // Neither parameter present.
    let input = json!({});
    let summary = tool_input_summary("read", &input, "/tmp/project");
    assert!(summary.is_empty(), "got: {summary}");
}

// ═══════════════════════════════════════════════════════════════════
// Widgets for extended tools (M6+): verify they are registered in
// `tool_input_summary` and render with a category icon + formatted args
// rather than falling through to the `summarize_tool_args` fallback,
// which dumps raw parameters without an icon.
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_apply_patch_input_summary_shows_icon() {
    let input = json!({"patch": "*** Begin Patch\n*** Update File: src/x.rs\n@@\n*** End Patch"});
    let summary = tool_input_summary("apply_patch", &input, "/tmp");
    assert!(summary.starts_with("📄"), "got: {summary}");
}

#[test]
fn test_bg_input_summary_spawn() {
    let input = json!({"action": "spawn", "command": "cargo build 2>&1"});
    let summary = tool_input_summary("bg", &input, "/tmp");
    assert!(summary.starts_with("🗂️"), "got: {summary}");
    assert!(summary.contains("spawn"));
    assert!(summary.contains("cargo build"));
}

#[test]
fn test_bg_input_summary_output_by_task() {
    let input = json!({"action": "output", "task_id": "abcdef0123456789"});
    let summary = tool_input_summary("bg", &input, "/tmp");
    assert!(summary.starts_with("🗂️"));
    assert!(summary.contains("output"));
    assert!(summary.contains("abcdef01"));
}

#[test]
fn test_browser_input_summary_open() {
    let input = json!({"action": "open", "url": "https://example.com"});
    let summary = tool_input_summary("browser", &input, "/tmp");
    assert!(summary.starts_with("🖥️"), "got: {summary}");
    assert!(summary.contains("example.com"));
}

#[test]
fn test_browser_input_summary_default_action() {
    let input = json!({"expression": "document.title"});
    let summary = tool_input_summary("browser", &input, "/tmp");
    assert!(summary.starts_with("🖥️"));
}

#[test]
fn test_gmail_input_summary_search() {
    let input = json!({"action": "search", "query": "from:ci is:unread"});
    let summary = tool_input_summary("gmail", &input, "/tmp");
    assert!(summary.starts_with("📧"), "got: {summary}");
    assert!(summary.contains("ci"));
}

#[test]
fn test_gmail_input_summary_send() {
    let input = json!({"action": "send", "to": "a@b.c", "subject": "hello"});
    let summary = tool_input_summary("gmail", &input, "/tmp");
    assert!(summary.starts_with("📧"));
    assert!(summary.contains("send"));
    assert!(summary.contains("a@b.c"));
}

#[test]
fn test_send_channel_message_input_summary() {
    let input = json!({"action": "send", "channel": "telegram", "message": "hi"});
    let summary = tool_input_summary("send_channel_message", &input, "/tmp");
    assert!(summary.starts_with("📨"), "got: {summary}");
    assert!(summary.contains("telegram"));
}

#[test]
fn test_open_input_summary() {
    let input = json!({"action": "url", "target": "https://github.com/"});
    let summary = tool_input_summary("open", &input, "/tmp");
    assert!(summary.starts_with("📂"), "got: {summary}");
    assert!(summary.contains("github.com"));
}

#[test]
fn test_conversation_search_input_summary() {
    let input = json!({"query": "memory store"});
    let summary = tool_input_summary("conversation_search", &input, "/tmp");
    assert!(summary.starts_with("🔎"), "got: {summary}");
    assert!(summary.contains("memory store"));
}

#[test]
fn test_session_search_input_summary() {
    let input = json!({"query": "tool window bug"});
    let summary = tool_input_summary("session_search", &input, "/tmp");
    assert!(summary.starts_with("🔎"), "got: {summary}");
    assert!(summary.contains("tool window bug"));
}

#[test]
fn test_initiative_input_summary_create() {
    let input = json!({"action": "create", "id": "x", "title": "Ship M7"});
    let summary = tool_input_summary("initiative", &input, "/tmp");
    assert!(summary.starts_with("🎯"), "got: {summary}");
    assert!(summary.contains("Ship M7"));
}

#[test]
fn test_initiative_input_summary_checkpoint() {
    let input = json!({"action": "checkpoint", "id": "api-v2", "progress": 75});
    let summary = tool_input_summary("initiative", &input, "/tmp");
    assert!(summary.starts_with("🎯"));
    assert!(summary.contains("checkpoint"));
}

#[test]
fn test_skill_manage_input_summary_list() {
    let input = json!({"action": "list"});
    let summary = tool_input_summary("skill_manage", &input, "/tmp");
    assert!(summary.starts_with("🧩"), "got: {summary}");
}

#[test]
fn test_skill_manage_input_summary_load() {
    let input = json!({"action": "load", "name": "simplify"});
    let summary = tool_input_summary("skill_manage", &input, "/tmp");
    assert!(summary.starts_with("🧩"));
    assert!(summary.contains("simplify"));
}

// ═══════════════════════════════════════════════════════════════════
// Plot ANSI rendering
// ═══════════════════════════════════════════════════════════════════

#[test]
fn test_plot_output_lines_prefers_ansi_canvas() {
    use ragent_tui::widgets::message_widget::plot_output_lines;
    use serde_json::json;

    let output = json!({
        "status": "success",
        "plot": "plain\ncanvas",
        "plot_ansi": "\u{1b}[36mcyan\u{1b}[0m\n\u{1b}[31mred\u{1b}[0m"
    });
    let lines = plot_output_lines(&Some(output)).expect("expected plot lines");
    assert_eq!(lines.len(), 2, "one line per canvas row");
    // The ANSI canvas is preferred over the plain-text one.
    assert!(
        lines[0].to_string().contains("cyan"),
        "first line should carry the ANSI-coloured text, got: {:?}",
        lines[0]
    );
    assert!(
        lines[1].to_string().contains("red"),
        "second line should carry the ANSI-coloured text, got: {:?}",
        lines[1]
    );
}

#[test]
fn test_plot_output_lines_multi_colour_line_gets_distinct_spans() {
    use ragent_tui::widgets::message_widget::plot_output_lines;
    use ratatui::style::Color;
    use serde_json::json;

    // One row carrying three colour runs, exactly as `buffer_to_ansi` emits
    // them: each colour change starts a new SGR sequence. The parser must
    // flush the pending text run at every escape so each run keeps its own
    // colour; a regression re-renders the whole line with the last colour.
    let output = json!({
        "status": "success",
        "plot": "abc123",
        "plot_ansi": "\u{1b}[31mabc\u{1b}[32mdef\u{1b}[34m123\u{1b}[0m"
    });
    let lines = plot_output_lines(&Some(output)).expect("expected plot lines");
    assert_eq!(lines.len(), 1);
    let spans = &lines[0].spans;
    let colored: Vec<(String, Color)> = spans
        .iter()
        .filter_map(|s| s.style.fg.map(|c| (s.content.to_string(), c)))
        .collect();
    assert!(
        colored.contains(&("abc".to_string(), Color::Red)),
        "expected a red 'abc' span, got: {colored:?}"
    );
    assert!(
        colored.contains(&("def".to_string(), Color::Green)),
        "expected a green 'def' span, got: {colored:?}"
    );
    assert!(
        colored.contains(&("123".to_string(), Color::Blue)),
        "expected a blue '123' span, got: {colored:?}"
    );
}

#[test]
fn test_plot_output_lines_bg_only_runs_keep_background() {
    use ragent_tui::widgets::message_widget::plot_output_lines;
    use ratatui::style::Color;
    use serde_json::json;

    // Heatmap cells are painted via background colour only (fg == bg is
    // reset to a contrasting fg). `buffer_to_ansi` emits fg and bg as
    // separate SGR sequences; each colour change must still split spans.
    let output = json!({
        "status": "success",
        "plot": "xx",
        "plot_ansi": "\u{1b}[38;2;68;1;84m\u{1b}[48;2;68;1;84mx\u{1b}[38;2;59;82;139m\u{1b}[48;2;59;82;139mx\u{1b}[0m"
    });
    let lines = plot_output_lines(&Some(output)).expect("expected plot lines");
    assert_eq!(lines.len(), 1);
    let bgs: Vec<Option<Color>> = lines[0]
        .spans
        .iter()
        .filter(|s| !s.content.trim().is_empty())
        .map(|s| s.style.bg)
        .collect();
    assert_eq!(bgs.len(), 2, "two colour runs expected, got: {bgs:?}");
    assert_ne!(
        bgs[0], bgs[1],
        "the two cells should carry different background colours"
    );
    assert_eq!(bgs[0], Some(Color::Rgb(68, 1, 84)), "viridis dark purple");
    assert_eq!(bgs[1], Some(Color::Rgb(59, 82, 139)), "viridis blue");
}

#[test]
fn test_plot_output_lines_falls_back_to_plain_canvas() {
    use ragent_tui::widgets::message_widget::plot_output_lines;
    use serde_json::json;

    let output = json!({
        "status": "success",
        "plot": "plain\ncanvas"
    });
    let lines = plot_output_lines(&Some(output)).expect("expected plot lines");
    assert_eq!(lines.len(), 2);
    assert!(lines[0].to_string().contains("plain"));
    assert!(lines[1].to_string().contains("canvas"));
}

#[test]
fn test_plot_output_lines_none_when_no_canvas() {
    use ragent_tui::widgets::message_widget::plot_output_lines;
    use serde_json::json;

    assert!(plot_output_lines(&None).is_none());
    assert!(plot_output_lines(&Some(json!({ "status": "error" }))).is_none());
    assert!(plot_output_lines(&Some(json!({ "plot": "   " }))).is_none());
}
