//! Tests for the `/research open` markdown viewer rendering.

mod support;

use std::path::PathBuf;

// markdown_to_lines is private in layout.rs, so we test via the public
// render path using a small wrapper that exposes it for tests.
use ragent_tui::layout::markdown_to_lines_testable;

#[test]
fn test_markdown_to_lines_renders_headings() {
    let base = PathBuf::from("/");
    let lines = markdown_to_lines_testable("# Title\n\n## Subtitle\nbody", &base, 80);
    let rendered: Vec<String> = lines.iter().map(std::string::ToString::to_string).collect();
    assert!(rendered.iter().any(|l| l.contains("Title")));
    assert!(rendered.iter().any(|l| l.contains("Subtitle")));
    assert!(rendered.iter().any(|l| l.contains("body")));
}

#[test]
fn test_markdown_to_lines_renders_code_block() {
    let base = PathBuf::from("/");
    let md = "```rust\nlet x = 1;\n```";
    let lines = markdown_to_lines_testable(md, &base, 80);
    let rendered: Vec<String> = lines.iter().map(std::string::ToString::to_string).collect();
    assert!(rendered.iter().any(|l| l.contains("let x = 1;")));
}

#[test]
fn test_markdown_to_lines_mermaid_block_shows_label() {
    let base = PathBuf::from("/");
    let md = "```mermaid\ngraph TD;\nA-->B;\n```";
    let lines = markdown_to_lines_testable(md, &base, 80);
    let rendered: Vec<String> = lines.iter().map(std::string::ToString::to_string).collect();
    assert!(
        rendered.iter().any(|l| l.contains("Mermaid diagram")),
        "expected mermaid label, got: {rendered:?}"
    );
    assert!(rendered.iter().any(|l| l.contains("A-->B")));
}

#[test]
fn test_markdown_to_lines_image_renders_placeholder() {
    let base = PathBuf::from("/tmp");
    let md = "![diagram](assets/diagram.png)";
    let lines = markdown_to_lines_testable(md, &base, 80);
    let rendered: Vec<String> = lines.iter().map(std::string::ToString::to_string).collect();
    let joined = rendered.join(" ");
    assert!(
        joined.contains("[Image: diagram"),
        "expected image placeholder, got: {joined}"
    );
}

#[test]
fn test_markdown_to_lines_link_renders_text_and_url() {
    let base = PathBuf::from("/");
    let md = "See [docs](https://example.com/docs).";
    let lines = markdown_to_lines_testable(md, &base, 80);
    let rendered: Vec<String> = lines.iter().map(std::string::ToString::to_string).collect();
    let joined = rendered.join(" ");
    assert!(
        joined.contains("[docs]"),
        "expected link text, got: {joined}"
    );
    assert!(
        joined.contains("(https://example.com/docs)"),
        "expected link url, got: {joined}"
    );
}

#[test]
fn test_markdown_to_lists_renders_bullet() {
    let base = PathBuf::from("/");
    let md = "- first\n- second";
    let lines = markdown_to_lines_testable(md, &base, 80);
    let rendered: Vec<String> = lines.iter().map(std::string::ToString::to_string).collect();
    assert!(
        rendered.iter().any(|l| l.contains("• first")),
        "expected bullet list, got: {rendered:?}"
    );
}

#[test]
fn test_markdown_to_lines_footer_note_present() {
    let base = PathBuf::from("/");
    let lines = markdown_to_lines_testable("hello", &base, 80);
    let rendered: Vec<String> = lines.iter().map(std::string::ToString::to_string).collect();
    assert!(
        rendered.iter().any(|l| l.contains("Esc to close")),
        "expected footer note, got: {rendered:?}"
    );
}

#[cfg(test)]
mod interaction_tests {
    use crossterm::event::{
        KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use ratatui::layout::Rect;

    use ragent_tui::app::ResearchViewState;

    use super::{PathBuf, support};

    fn make_research_view() -> ResearchViewState {
        ResearchViewState {
            name: "test".to_string(),
            path: PathBuf::from("/tmp/research/test/RESEARCH.md"),
            base_dir: PathBuf::from("/tmp/research/test"),
            markdown: "line\n".repeat(200),
            scroll_offset: 10,
            max_scroll: 100,
            line_cache: ragent_tui::app::OutputViewLineCache {
                lines: Vec::new(),
                wrapped_lines: Vec::new(),
                content_lines: Vec::new(),
                wrapped_count: 0,
                cache_width: 0,
                source_generation: 0,
            },
        }
    }

    fn mouse_scroll_up(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: col,
            row,
            modifiers: KeyModifiers::empty(),
        }
    }

    fn mouse_scroll_down(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: col,
            row,
            modifiers: KeyModifiers::empty(),
        }
    }

    fn mouse_click(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: col,
            row,
            modifiers: KeyModifiers::empty(),
        }
    }

    #[test]
    fn test_research_view_esc_closes() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(
            app.research_view.is_none(),
            "Esc should close research view"
        );
    }

    #[test]
    fn test_research_view_page_down_scrolls() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 5);
    }

    #[test]
    fn test_research_view_page_up_scrolls() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 15);
    }

    #[test]
    fn test_research_view_down_arrow_scrolls() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 9);
    }

    #[test]
    fn test_research_view_up_arrow_scrolls() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 11);
    }

    #[test]
    fn test_research_view_ctrl_page_end_jumps_to_end() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::PageDown, KeyModifiers::CONTROL));
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 0);
    }

    #[test]
    fn test_research_view_ctrl_page_home_jumps_to_start() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::PageUp, KeyModifiers::CONTROL));
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 100);
    }

    #[test]
    fn test_research_view_mouse_scroll_inside() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.research_view_area = Rect::new(5, 5, 70, 20);

        app.handle_mouse_event(mouse_scroll_down(10, 10));
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 7);

        app.handle_mouse_event(mouse_scroll_up(10, 10));
        assert_eq!(app.research_view.as_ref().unwrap().scroll_offset, 10);
    }

    #[test]
    fn test_research_view_click_outside_closes() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.research_view_area = Rect::new(5, 5, 70, 20);

        app.handle_mouse_event(mouse_click(2, 2));
        assert!(
            app.research_view.is_none(),
            "click outside should close research view"
        );
    }

    #[test]
    fn test_research_view_click_inside_keeps_open() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.research_view_area = Rect::new(5, 5, 70, 20);

        app.handle_mouse_event(mouse_click(10, 10));
        assert!(
            app.research_view.is_some(),
            "click inside should keep research view open"
        );
    }

    #[test]
    fn test_research_view_keys_do_not_type_into_input() {
        let mut app = support::make_app();
        app.research_view = Some(make_research_view());
        app.handle_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE));
        assert!(app.research_view.is_some());
        assert!(app.input.is_empty());
    }
}
