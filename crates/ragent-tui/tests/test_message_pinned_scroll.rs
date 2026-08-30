//! Scroll-pinning regression tests for the message timeline panel.
//!
//! Verifies that when the transcript exceeds one screen the bottom-pinned
//! view (`scroll_offset == 0`) shows the true tail of the transcript and
//! that newly appended messages are visible at the bottom.  Regression for
//! the wrapped-vs-unwrapped coordinate mismatch: scroll geometry was
//! measured in wrapped lines while the visible-window slice was cut from
//! un-wrapped lines, so any message containing a line wider than the pane
//! broke the pinned view.
//!
//! Also checks that the cached wrapped rows respect the pane width and that
//! top-anchored scrolling still reaches the first message.

use std::sync::Arc;

use ratatui::{Terminal, backend::TestBackend, layout::Rect};

use ragent_agent::message::{Message, MessagePart, Role};
use ragent_agent::{
    agent, event::EventBus, permission::PermissionChecker, provider, session::SessionManager,
    session::processor::SessionProcessor, storage::Storage, tool,
};
use ragent_tui::App;
use unicode_width::UnicodeWidthStr;

/// Build an [`App`] backed by an in-memory database.
fn make_app() -> App {
    let event_bus = Arc::new(EventBus::default());
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let provider_registry = Arc::new(provider::create_default_registry());
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![])));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let session_processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: provider_registry.clone(),
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        agent_manager: std::sync::OnceLock::new(),
        bg_service: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_agent::StreamConfig::default(),
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        llm_client_cache: parking_lot::RwLock::new(std::collections::HashMap::new()),
        cached_config: parking_lot::Mutex::new(None),
        team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(
            std::collections::HashMap::new(),
        )),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        telemetry: std::sync::Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        activity_log: std::sync::OnceLock::new(),
        skill_registry_cache: parking_lot::Mutex::new(None),
    });
    let agent_info =
        agent::resolve_agent("general", &Default::default()).expect("resolve general agent");

    App::new(
        event_bus,
        storage,
        provider_registry,
        session_processor,
        Arc::unwrap_or_clone(agent_info),
        true,
        std::path::PathBuf::new(),
    )
}

/// Assistant message whose text is one long unbroken word plus a short tail.
/// The long word wraps into more display rows than the un-wrapped line
/// count, which is exactly the case that used to break the bottom-pinned
/// slice.
fn long_and_tail_messages() -> Vec<Message> {
    let sid = "session-pinning";
    let long_word = "x".repeat(200);
    vec![
        Message::new(
            sid,
            Role::User,
            vec![MessagePart::Text {
                text: "start of transcript".to_string(),
            }],
        ),
        Message::new(
            sid,
            Role::Assistant,
            vec![MessagePart::Text {
                text: format!("{long_word}\nTAIL-MARKER"),
            }],
        ),
    ]
}

/// Render the messages panel into a `TestBackend` terminal and return the
/// rendered text of each inner row (top row first).
fn render_message_area(app: &mut App, area: Rect) -> Vec<String> {
    let mut terminal =
        Terminal::new(TestBackend::new(area.width, area.height)).expect("test terminal");
    terminal
        .draw(|frame| {
            ragent_tui::layout::render_messages(
                frame,
                app,
                Rect::new(0, 0, area.width, area.height),
            );
        })
        .expect("draw");
    let buffer = terminal.backend().buffer().clone();
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect::<String>()
        })
        .collect()
}

#[test]
fn test_bottom_pinned_view_shows_last_message_with_wrapping() {
    let mut app = make_app();
    for msg in long_and_tail_messages() {
        app.messages.push(msg);
    }

    // Pane: 40 columns x 10 rows → inner width 38, 8 visible rows.
    let area = Rect::new(0, 0, 40, 10);
    render_message_area(&mut app, area);

    // The long word must have produced overflow so the pinned path is
    // actually exercised.
    assert!(
        app.message_max_scroll > 0,
        "content should overflow the pane, max_scroll={}",
        app.message_max_scroll
    );

    // With scroll_offset == 0 (bottom-pinned) the tail marker of the last
    // message must be visible in the rendered output.
    app.scroll_offset = 0;
    let rows = render_message_area(&mut app, area);
    let joined = rows.join("\n");
    assert!(
        joined.contains("TAIL-MARKER"),
        "pinned view must show the newest message; rendered:\n{joined}"
    );
}

#[test]
fn test_wrapped_rows_never_exceed_inner_width() {
    let mut app = make_app();
    app.messages = long_and_tail_messages();

    let area = Rect::new(0, 0, 40, 10);
    render_message_area(&mut app, area);

    let inner_width = (area.width - 2) as usize;
    for (i, group) in app.message_line_cache.iter().enumerate() {
        for (j, row) in group.wrapped_lines.iter().enumerate() {
            let width: usize = row.spans.iter().map(|s| s.content.width()).sum();
            assert!(
                width <= inner_width,
                "cache group {i} row {j} is {width} wide, exceeds inner width {inner_width}"
            );
        }
        assert_eq!(
            group.wrapped_count as usize,
            group.wrapped_lines.len(),
            "wrapped_count must mirror wrapped_lines"
        );
    }
}

#[test]
fn test_scroll_to_top_still_reaches_first_message() {
    let mut app = make_app();
    app.messages = long_and_tail_messages();

    let area = Rect::new(0, 0, 40, 10);
    render_message_area(&mut app, area);

    let max_scroll = app.message_max_scroll;
    assert!(max_scroll > 0);
    app.scroll_offset = max_scroll;
    let rows = render_message_area(&mut app, area);
    let joined = rows.join("\n");
    assert!(
        joined.contains("start of transcript"),
        "top-anchored view must show the first message; rendered:\n{joined}"
    );
}

#[test]
fn test_appended_message_becomes_visible_while_pinned() {
    let mut app = make_app();
    app.messages = long_and_tail_messages();

    let area = Rect::new(0, 0, 40, 10);
    render_message_area(&mut app, area);

    // A new message arrives while the view is bottom-pinned.
    app.messages.push(Message::new(
        "session-pinning",
        Role::Assistant,
        vec![MessagePart::Text {
            text: "APPENDED-VISIBLE-MARKER".to_string(),
        }],
    ));
    app.scroll_offset = 0;
    let rows = render_message_area(&mut app, area);
    let joined = rows.join("\n");
    assert!(
        joined.contains("APPENDED-VISIBLE-MARKER"),
        "appended message must appear in the pinned view; rendered:\n{joined}"
    );
}
// ---------- Wrap parity with ratatui's Paragraph ----------

/// The cached pre-wrapped row count must exactly match
/// `Paragraph::line_count` over the same cached lines at the same width;
/// otherwise the scroll geometry (built from the cache) drifts from the
/// painted output.  The cache's per-message `wrapped_lines` are produced by
/// `wrap_line_styled`, so this pins that port to ratatui's WordWrapper
/// across awkward input: wide CJK, emoji, tabs, NBSP, long unbroken words,
/// and leading/trailing whitespace.
#[test]
fn test_wrap_row_counts_match_paragraph_line_count() {
    let samples: Vec<String> = vec![
        String::new(),
        " ".to_string(),
        "   ".repeat(30),
        "short line".to_string(),
        "The quick brown fox jumps over the lazy dog repeatedly".to_string(),
        "x".repeat(200),
        "word ".repeat(60),
        "Hello \u{4e07}\u{56fd}\u{9645} code \u{8a9e} samples \u{6f22}\u{5b57} here".to_string(),
        "emoji \u{2705} pipeline \u{1f52c} with \u{1f4ce} marks".to_string(),
        "combination \u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467} family glyph".to_string(),
        "tab\tseparated\tvalues\tacross\tthe\trow".to_string(),
        "nbsp\u{00a0}keeps\u{00a0}words\u{00a0}together".to_string(),
        "hyphen-joined-inline-identifiers-are-hard-to-break".to_string(),
        "line with trailing spaces     ".to_string(),
    ];

    for sample in &samples {
        let mut app = make_app();
        app.messages.push(Message::new(
            "session-parity",
            Role::Assistant,
            vec![MessagePart::Text {
                text: sample.clone(),
            }],
        ));

        for pane_width in [3u16, 6, 12, 24, 42, 80, 128] {
            // Tall pane so nothing is sliced; the cache still wraps at the
            // pane's inner width.
            let area = Rect::new(0, 0, pane_width, 400);
            render_message_area(&mut app, area);

            let group = &app.message_line_cache[0];
            let para = ratatui::widgets::Paragraph::new(group.lines.clone())
                .wrap(ratatui::widgets::Wrap { trim: false });
            let expected = para.line_count(pane_width - 2);
            assert_eq!(
                group.wrapped_count as usize,
                expected,
                "sample {:?} at pane width {pane_width}: cache has {} rows, Paragraph says {expected}",
                truncate_for_log(sample),
                group.wrapped_count
            );
        }
    }
}

fn truncate_for_log(s: &str) -> String {
    s.chars().take(24).collect()
}
// ---------- Whitespace-only wrapped rows must not shift the painted tail ----------

/// ratatui 0.29 re-wraps a whitespace-only input row (e.g. `"  "`) into TWO
/// painted rows: a blank row plus the row of spaces (`WordWrapper
/// ::process_input` pushes `vec![]` before draining the pending whitespace,
/// and trim=false keeps the whitespace run).  The messages window therefore
/// used to consume one extra painted row per whitespace-only cached row,
/// shifting the visible tail below the scroll geometry: bottom-pinned showed
/// the last lines cut off and newly appended lines stayed hidden until the
/// user scrolled.  Regression: the painted window must end exactly at the
/// cached tail for a transcript whose window contains whitespace-only rows.
#[test]
fn test_pinned_tail_not_shifted_by_whitespace_only_rows() {
    let mut app = make_app();
    // Markdown-style content whose wrapping produces whitespace-only cached
    // rows: blank lines with indentation and trailing-space lines, in (and
    // just above) the bottom window, then a long tail of further messages.
    let text = format!(
        "intro\n  \ntrailing-spaces-line{}        \n  \nmid content A\n  \nmid content B\n  \nTAIL-MARKER",
        "x".repeat(30)
    );
    app.messages.push(Message::new(
        "session-pinning",
        Role::User,
        vec![MessagePart::Text {
            text: "question".to_string(),
        }],
    ));
    app.messages.push(Message::new(
        "session-pinning",
        Role::Assistant,
        vec![MessagePart::Text { text }],
    ));

    // Pane: 40 columns x 10 rows -> inner width 38, 8 visible rows.
    let area = Rect::new(0, 0, 40, 10);

    app.scroll_offset = 0;
    let rows = render_message_area(&mut app, area);
    let joined = rows.join("\n");
    assert!(
        joined.contains("TAIL-MARKER"),
        "pinned view must show the newest line; rendered:\n{joined}"
    );

    // Strict check: the painted inner rows must be exactly the LAST
    // `visible` cached wrapped rows of the transcript (one painted row per
    // cached row, in order).  Before the fix, whitespace-only rows painted an
    // extra blank row and the bottom of the window lost the tail rows.
    let visible = (area.height - 2) as usize;
    let cached: Vec<String> = app
        .message_line_cache
        .iter()
        .flat_map(|g| g.wrapped_lines.iter())
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.clone())
                .collect::<String>()
        })
        .collect();
    let expected: Vec<String> = cached[cached.len() - visible..].to_vec();
    // Drop the border rows (first and last painted rows) before comparing.
    let painted_inner: Vec<String> = rows[1..rows.len() - 1]
        .iter()
        .map(|r| r.trim_end().to_string())
        .collect();
    for (painted, expected_text) in painted_inner.iter().zip(expected.iter()) {
        let exp_trim = expected_text.trim_end();
        // Strip the leading border glyph and every trailing border/scrollbar
        // glyph from the concatenated buffer row.
        let mut chars: Vec<char> = painted.chars().collect();
        if matches!(chars.first(), Some('│' | '|')) {
            chars.remove(0);
        }
        let border_glyphs = ['│', '║', '|', '▲', '▼', '█', '▒', '░'];
        while matches!(chars.last(), Some(c) if border_glyphs.contains(c)) {
            chars.pop();
        }
        let painted_text: String = chars.into_iter().collect();
        assert_eq!(
            painted_text.trim_end(),
            exp_trim,
            "painted inner row diverged from cached row: painted={painted:?} expected={exp_trim:?}"
        );
    }

    // And appending a message while pinned must surface it immediately.
    app.messages.push(Message::new(
        "session-pinning",
        Role::Assistant,
        vec![MessagePart::Text {
            text: "APPENDED-VISIBLE-MARKER-2".to_string(),
        }],
    ));
    app.scroll_offset = 0;
    let rows = render_message_area(&mut app, area);
    let joined = rows.join("\n");
    assert!(
        joined.contains("APPENDED-VISIBLE-MARKER-2"),
        "appended message must be visible at the bottom without scrolling; rendered:\n{joined}"
    );
}
