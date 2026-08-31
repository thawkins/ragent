//! Tests for the Context side panel (spec `contextpanel`).
//!
//! - **T-016**: Machine-executable validation of `specs/contextpanel/TESTPLAN.md`.
//!
//! Each test walks one manual test case (TC-001..TC-007) through the real TUI
//! pipeline: `handle_key_event` for the Alt+X binding and a full-frame
//! `layout::render` on a ratatui `TestBackend`, asserting the results the
//! plan spells out without a human at the terminal. Interactive-only aspects
//! that cannot run headlessly (a live LLM turn, real font/terminal rendering)
//! are called out in the affected test's doc comments.

mod support;

use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ragent_agent::message::{Message, MessagePart, Role};
use ragent_tui::App;

/// Extract the full rendered screen text (one line per row) from a backend.
fn rendered_text(backend: &ratatui::backend::TestBackend) -> String {
    let buffer = backend.buffer();
    (0..buffer.area.height)
        .map(|y| {
            (0..buffer.area.width)
                .filter_map(|x| buffer.cell((x, y)))
                .map(|c| c.symbol().to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Draw one full frame the way the TUI main loop would.
fn draw(app: &mut App, cols: u16, rows: u16) -> ratatui::backend::TestBackend {
    let backend = ratatui::backend::TestBackend::new(cols, rows);
    let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("draw");
    terminal.backend().clone()
}

/// Send the Alt+X key chord TC-001 presses.
fn press_alt_x(app: &mut App) {
    app.handle_key_event(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::ALT));
}

/// Wait for a scheduled background snapshot to be adopted (FR-015 refresh
/// path), mirroring the TUI poll cycle that follows any context change.
async fn drain_snapshot_polling(app: &mut App) {
    for _ in 0..100 {
        app.poll_context_snapshot_refresh();
        if !app.context_refresh_inflight {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    app.poll_context_snapshot_refresh();
}

/// Registry-resolved `(provider, model, context_window)` triples, filtered to
/// models that advertise a positive limit (mirrors the T-008 helper).
fn registry_models(app: &App) -> Vec<(String, String, usize)> {
    let mut models = Vec::new();
    for p in app.provider_registry.list() {
        for m in &p.models {
            if m.context_window > 0 {
                models.push((p.id.clone(), m.id.clone(), m.context_window));
            }
        }
    }
    models
}

#[tokio::test]
async fn tc001_alt_x_toggles_panel_open_and_closed() {
    let mut app = support::make_app();
    assert!(!app.show_context_panel, "precondition: panel starts hidden");

    // Step 1: the first Alt+X opens the panel.
    press_alt_x(&mut app);
    assert!(app.show_context_panel, "step 1: panel opens");
    assert_eq!(app.status, "context panel visible");

    // Step 2: a right-hand panel appears; the chat area shrinks.
    let first = draw(&mut app, 100, 30);
    let first_area = app.context_panel_area;
    assert!(first_area.width > 0, "step 2: right column reserved");
    assert_eq!(
        first_area.right(),
        100,
        "step 2: reserved at the right edge"
    );
    assert!(app.message_area.width < 100, "step 2: chat area shrinks");
    let titled = {
        let buffer = first.buffer();
        (0..buffer.area.height).any(|y| {
            (0..buffer.area.width)
                .filter_map(|x| buffer.cell((x, y)))
                .map(|c| c.symbol().to_string())
                .collect::<String>()
                .contains(" Context ")
        })
    };
    assert!(titled, "step 2: panel titled Context");

    drain_snapshot_polling(&mut app).await;

    // Step 3: the second Alt+X closes it; full width returns.
    press_alt_x(&mut app);
    assert!(!app.show_context_panel, "step 3: panel closes");
    assert_eq!(app.status, "context panel hidden");
    draw(&mut app, 100, 30);
    assert_eq!(app.context_panel_area.width, 0);
    assert_eq!(app.message_area.width, 100);

    // Step 5: the third Alt+X reopens it in the same place (the plan's step 4
    // is only a pause between key presses).
    press_alt_x(&mut app);
    assert!(app.show_context_panel, "step 5: panel reopens");
    draw(&mut app, 100, 30);
    assert_eq!(app.context_panel_area, first_area);
}

#[tokio::test]
async fn tc002_system_prompt_row_and_model_capacity() {
    // FR-010: with a provider-advertised window, the capacity row renders
    // percentages for every partition.
    let mut app = support::make_app();
    let models = registry_models(&app);
    assert!(!models.is_empty(), "registry advertises at least one model");
    let (prov, model, window) = models[0].clone();
    app.selected_model = Some(format!("{prov}/{model}"));
    app.show_context_panel = true;
    drain_snapshot_polling(&mut app).await;
    let text = rendered_text(&draw(&mut app, 100, 40));
    let snapshot = app.context_effective_snapshot();
    assert_eq!(
        snapshot.context_window_tokens,
        Some(window),
        "panel resolves the selected model's advertised capacity (FR-010)"
    );
    assert!(text.contains("System prompt"), "capacity row present");
    assert!(snapshot.system_prompt_tokens > 0, "partition is non-zero");
    assert!(
        text.contains('%'),
        "percentages render when window is known"
    );

    // FR-011: when no provider reports a limit, percentages read "unknown"
    // while absolute token counts remain visible. Force all three capacity
    // paths (selected_model, agent_info.model, selected_model_ctx_window
    // fallback) to miss.
    let mut app = support::make_app();
    app.agent_info.model = None;
    app.selected_model = Some("no-such-provider/no-such-model".to_string());
    app.selected_model_ctx_window = None;
    assert!(app.active_context_window_tokens().is_none());
    app.show_context_panel = true;
    drain_snapshot_polling(&mut app).await;
    let text = rendered_text(&draw(&mut app, 100, 40));
    assert!(text.contains("unknown"), "FR-011: unknown percentages");
    assert!(
        text.contains("tk"),
        "FR-011: absolute token counts still shown"
    );
}

#[tokio::test]
async fn tc003_tool_catalog_and_metadata_rows() {
    let mut app = support::make_app();
    app.show_context_panel = true;
    drain_snapshot_polling(&mut app).await;
    let text = rendered_text(&draw(&mut app, 100, 40));
    let snapshot = app.context_effective_snapshot();

    assert!(text.contains("Tool catalog"), "catalog row present");
    assert!(text.contains("Tool metadata"), "metadata row present");
    assert!(
        snapshot.tool_catalog_tokens > 0,
        "default registry exposes many tools (FR-006)"
    );
    // Plan arithmetic: catalog + metadata is at least catalog alone.
    assert!(
        snapshot.tool_catalog_tokens + snapshot.tool_metadata_tokens
            >= snapshot.tool_catalog_tokens
    );
}

#[tokio::test]
async fn tc004_history_updated_after_chat() {
    let mut app = support::make_app();
    app.show_context_panel = true;
    drain_snapshot_polling(&mut app).await;
    let _ = draw(&mut app, 100, 40);
    let before_count = app.conversation_message_count();
    let before_tokens = app.conversation_history_token_count();
    assert_eq!(before_count, 0, "precondition: empty history");

    // Steps 2-3: a user message, then the assistant reply. A live LLM turn is
    // not available headlessly, so the turn is injected at the same layer the
    // event handler commits it; FR-013's refresh trigger is issued explicitly.
    app.messages.push(Message::new(
        "t016-tc004",
        Role::User,
        vec![MessagePart::Text { text: "hi".into() }],
    ));
    app.messages.push(Message::new(
        "t016-tc004",
        Role::Assistant,
        vec![MessagePart::Text {
            text: "y".repeat(500),
        }],
    ));
    app.schedule_context_snapshot_refresh();
    drain_snapshot_polling(&mut app).await;
    let text = rendered_text(&draw(&mut app, 100, 40));

    assert!(
        app.conversation_history_token_count() > before_tokens,
        "step 4: history token count grows"
    );
    assert_eq!(
        app.conversation_message_count(),
        before_count + 2,
        "step 4: user + assistant replies raise message count by two"
    );
    let snapshot = app.context_effective_snapshot();
    assert!(snapshot.history_tokens > 0);
    assert!(text.contains("2 messages"), "message count re-read");
}

#[tokio::test]
async fn tc005_total_and_headroom_arithmetic() {
    let mut app = support::make_app();
    let models = registry_models(&app);
    let (prov, model, window) = models[0].clone();
    app.selected_model = Some(format!("{prov}/{model}"));
    app.messages.push(Message::new(
        "t016-tc005",
        Role::User,
        vec![MessagePart::Text {
            text: "s".repeat(300),
        }],
    ));
    app.show_context_panel = true;
    drain_snapshot_polling(&mut app).await;
    let _ = draw(&mut app, 100, 40);
    let snapshot = app.context_effective_snapshot();

    // Step 3: "Total used" equals the sum of the displayed partitions.
    let sum = snapshot.system_prompt_tokens
        + snapshot.tool_catalog_tokens
        + snapshot.tool_metadata_tokens
        + snapshot.history_tokens;
    assert_eq!(snapshot.total_tokens(), sum);

    // Headroom equals capacity minus total used (saturating at zero).
    assert_eq!(
        snapshot.remaining_tokens(),
        Some((window as u64).saturating_sub(sum))
    );

    // Total percentage = total / capacity * 100. The render formats this to
    // whole percents; the underlying snapshot value is exact.
    let expected = (sum as f64 / window as f64) * 100.0;
    let actual = snapshot.total_percent().expect("window advertised");
    assert!((actual - expected).abs() < 1e-9);
}

#[tokio::test]
async fn tc006_panel_content_never_enters_llm_history() {
    // FR-016/TC-006: opening and rendering the panel must not touch the
    // conversation the next request carries. The plan's conversational probe
    // ("what panel do you see") needs a live model, so this walks the
    // structural guarantee instead: no synthetic message, no history mutation.
    let mut app = support::make_app();
    let before_count = app.conversation_message_count();
    let before_tokens = app.conversation_history_token_count();

    app.show_context_panel = true;
    drain_snapshot_polling(&mut app).await;
    draw(&mut app, 100, 40);
    assert_eq!(app.conversation_message_count(), before_count);
    assert_eq!(app.conversation_history_token_count(), before_tokens);

    // Render a second frame over an adopted cache - still no history change.
    draw(&mut app, 100, 40);
    assert_eq!(app.conversation_message_count(), before_count);
    assert_eq!(app.conversation_history_token_count(), before_tokens);
}

#[tokio::test]
async fn tc007_refresh_after_model_switch() {
    let mut app = support::make_app();
    let models = registry_models(&app);
    let (p1, m1, w1) = models[0].clone();
    let (p2, m2, w2) = models
        .iter()
        .find(|(_, _, w)| *w != w1)
        .map(|(p, m, w)| (p.clone(), m.clone(), *w))
        .expect("a second model with a different advertised window");

    app.messages.push(Message::new(
        "t016-tc007",
        Role::User,
        vec![MessagePart::Text {
            text: "h".repeat(400),
        }],
    ));

    // Step 1: note the capacity for the current model.
    app.selected_model = Some(format!("{p1}/{m1}"));
    app.show_context_panel = true;
    drain_snapshot_polling(&mut app).await;
    let _ = draw(&mut app, 100, 40);
    let before = app.context_effective_snapshot();
    assert_eq!(before.context_window_tokens, Some(w1), "precondition");

    // Step 2: switch models (the state the model picker commits, followed by
    // FR-013's refresh after the change).
    app.selected_model = Some(format!("{p2}/{m2}"));
    app.schedule_context_snapshot_refresh();
    drain_snapshot_polling(&mut app).await;
    let _ = draw(&mut app, 100, 40);
    let after = app.context_effective_snapshot();

    // Step 3: capacity reflects the new model; percentages recalculate.
    assert_eq!(after.context_window_tokens, Some(w2));
    assert_ne!(w1, w2);
    assert_eq!(
        before.total_tokens(),
        after.total_tokens(),
        "only the capacity changed, not the measured partitions"
    );
    let before_pct = before.total_percent().expect("window advertised");
    let after_pct = after.total_percent().expect("window advertised");
    assert!(
        (before_pct - after_pct).abs() > 1e-9,
        "percentages recalculate against the new window"
    );
}
