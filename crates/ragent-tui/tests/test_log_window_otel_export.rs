//! spec `otel` FR-036: a log-window entry with no native `tracing` event behind
//! it - session activity assembled from the agent event bus - is also emitted as
//! a `tracing` event, so the OTLP logs layer forwards it to the collector.
//!
//! Every line the log window records is also re-emitted to `tracing` at the
//! entry's own severity (a tool call at `INFO`, a warning at `WARN`, an error at
//! `ERROR`), so the default `--log-level` and `telemetry.otel.logs.level` floors
//! both admit it and a tool call reaches the collector out of the box.
//!
//! The re-emit is **not** conditional on the log panel being open. A session
//! normally runs with the panel closed, so gating the export on `show_log` left
//! a collector with only the events that have a native `tracing` record behind
//! them - `Bash` emits `"Executing bash command"` on every call, while `Edit`,
//! `Write`, `Grep` and `read` emit nothing. Read from the collector that looks
//! like "only Bash tool calls are exported"
//! (`test_tool_call_is_exported_with_the_panel_closed` pins the fix).
//!
//! An earlier revision emitted every re-emit at `DEBUG` to keep the local and
//! remote streams disjoint. Both default floors sit above `DEBUG`, so a tool call
//! reached the collector only when the operator also raised `--log-level` - in
//! practice the export shipped nothing.
//!
//! The duplicate guard therefore lives on the *caller*: entries derived from a
//! record the OTLP layer already exported (the `TuiTracingLayer` drain, the
//! mirrored-child drain) go through `push_log_deferred` and are never re-emitted,
//! so a collector still sees the startup line and the telemetry-failure notices
//! exactly once (`test_tracing_record_drain_is_not_re_emitted` pins that).
//!
//! These tests drive the real push paths behind a capture subscriber and assert
//! the resulting event, rather than only calling the emitter directly.

use std::sync::{Arc, Mutex, OnceLock};

use ragent_tui::app::{LogLevel, emit_log_entry_to_tracing};

/// One captured `tracing` event: its level and message body.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Captured {
    level: tracing::Level,
    message: String,
    log_window: bool,
    log_window_level: Option<String>,
}

/// A minimal `tracing` layer that records every event it sees.
#[derive(Clone, Default)]
struct CaptureLayer {
    events: Arc<Mutex<Vec<Captured>>>,
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for CaptureLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        #[derive(Default)]
        struct V {
            message: String,
            log_window: bool,
            log_window_level: Option<String>,
        }

        impl tracing::field::Visit for V {
            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                match field.name() {
                    "message" => self.message = value.to_string(),
                    "log_window_level" => self.log_window_level = Some(value.to_string()),
                    _ => {}
                }
            }

            fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
                if field.name() == "log_window" {
                    self.log_window = value;
                }
            }

            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                if field.name() == "message" {
                    self.message = format!("{value:?}");
                }
            }
        }

        let mut visitor = V::default();
        event.record(&mut visitor);
        let entry = Captured {
            level: *event.metadata().level(),
            message: visitor.message,
            log_window: visitor.log_window,
            log_window_level: visitor.log_window_level,
        };
        if let Ok(mut guard) = self.events.lock() {
            guard.push(entry);
        }
    }
}

/// The capture sink the global subscriber writes into.
fn sink() -> &'static Arc<Mutex<Vec<Captured>>> {
    static SINK: OnceLock<Arc<Mutex<Vec<Captured>>>> = OnceLock::new();
    SINK.get_or_init(|| Arc::new(Mutex::new(Vec::new())))
}

/// Install the capture subscriber once for the whole binary.
///
/// `tracing` allows a single global subscriber per process, so this is
/// best-effort: a second call is a no-op and the first test's subscriber keeps
/// capturing, which is what these tests want anyway.
fn install_capture() {
    use tracing_subscriber::layer::SubscriberExt as _;
    use tracing_subscriber::util::SubscriberInitExt as _;

    static INSTALLED: OnceLock<()> = OnceLock::new();
    INSTALLED.get_or_init(|| {
        let layer = CaptureLayer {
            events: Arc::clone(sink()),
        };
        // `RUST_LOG=trace` equivalent: record every level so the assertions do
        // not depend on the ambient environment.
        let _ = tracing_subscriber::registry()
            .with(tracing_subscriber::filter::LevelFilter::TRACE)
            .with(layer)
            .try_init();
    });
}

/// Run `f` with the capture sink emptied, then return what it recorded.
///
/// The sink is process-wide and every test in this file has its own process
/// under `cargo nextest`, so no cross-test lock is needed. The crate's global
/// lock is deliberately NOT taken: `capture` is called from inside these very
/// tests and `std::sync::Mutex` is not reentrant, so a nested acquisition would
/// deadlock the test.
///
/// The clear/read pair is not an emptiness assertion: a test asserts on the
/// records it recognises by message, because the log-window export is bounded by
/// a process-wide burst limit that a test's own traffic can exhaust.
fn capture<F: FnOnce()>(f: F) -> Vec<Captured> {
    install_capture();
    if let Ok(mut guard) = sink().lock() {
        guard.clear();
    }
    f();
    sink().lock().map(|g| g.clone()).unwrap_or_default()
}

#[test]
fn test_tool_log_entry_reaches_tracing_at_info() {
    let captured = capture(|| {
        emit_log_entry_to_tracing(LogLevel::Tool, "[1.1] tool call: Bash");
    });

    let event = captured
        .iter()
        .find(|c| c.message == "[1.1] tool call: Bash")
        .expect("tool log entry must be emitted as a tracing event");
    assert_eq!(
        event.level,
        tracing::Level::INFO,
        "a tool call is exported at INFO so the default `--log-level info` \
         and `telemetry.otel.logs.level: info` floors both admit it; a DEBUG \
         re-emit was dropped by both defaults and never reached a collector"
    );
    assert_eq!(
        event.log_window_level.as_deref(),
        Some("tool"),
        "the entry's own severity travels as an attribute so a collector can \
         still tell a tool call from a diagnostic"
    );
    assert!(
        event.log_window,
        "the event must carry log_window=true so an exporter can identify \
         log-window records"
    );
}

#[test]
fn test_warn_and_error_log_entries_carry_their_severity_as_an_attribute() {
    let captured = capture(|| {
        emit_log_entry_to_tracing(LogLevel::Warn, "warn entry");
        emit_log_entry_to_tracing(LogLevel::Error, "error entry");
    });

    let warn = captured
        .iter()
        .find(|c| c.message == "warn entry")
        .expect("warn entry must be emitted");
    assert_eq!(warn.level, tracing::Level::WARN);
    assert_eq!(warn.log_window_level.as_deref(), Some("warn"));

    let error = captured
        .iter()
        .find(|c| c.message == "error entry")
        .expect("error entry must be emitted");
    assert_eq!(error.level, tracing::Level::ERROR);
    assert_eq!(error.log_window_level.as_deref(), Some("error"));
}

#[test]
fn test_info_log_entry_reaches_tracing_at_info() {
    let captured = capture(|| {
        emit_log_entry_to_tracing(LogLevel::Info, "info entry");
    });

    let event = captured
        .iter()
        .find(|c| c.message == "info entry")
        .expect("info entry must be emitted");
    assert_eq!(event.level, tracing::Level::INFO);
    assert_eq!(event.log_window_level.as_deref(), Some("info"));
}

#[path = "support/mod.rs"]
mod support;

/// A tool call must reach `tracing` with the log panel **closed**, which is how
/// a session normally runs.
///
/// Gating the re-emit on `show_log` made a collector see only the tools that
/// carry a native `tracing` record of their own: `Bash` logs `"Executing bash
/// command"` on every call, while `Edit` and its siblings log nothing. From the
/// collector that looked like "only Bash tool calls are exported", even though
/// the panel-gated code treated every tool identically.
///
/// The assertion is deliberately made for both a tool that has a native event
/// (`Bash`) and one that does not (`Edit`), so the test fails if the export ever
/// becomes a consequence of the panel being visible again.
#[tokio::test(flavor = "current_thread")]
async fn test_tool_call_is_exported_with_the_panel_closed() {
    use ragent_agent::event::Event;

    for tool in ["Bash", "Edit"] {
        let mut app = support::make_app();
        // The default for a session: the log panel is not open.
        app.show_log = false;
        assert!(!app.show_log, "the test must run with the panel closed");
        let session_id = "test-session-closed".to_string();
        app.session_id = Some(session_id.clone());

        let captured = capture(|| {
            futures::executor::block_on(app.handle_event(Event::ToolCallStart {
                session_id: session_id.clone(),
                call_id: format!("call-{tool}"),
                tool: tool.to_string(),
            }));
        });

        let event = captured
            .iter()
            .find(|c| c.message.contains(&format!("tool call: {tool}")))
            .unwrap_or_else(|| {
                panic!(
                    "a `{tool}` tool call must reach tracing with the log panel closed, or the \
                     OTLP logs endpoint only ever sees the tools that log natively"
                )
            });
        assert_eq!(event.level, tracing::Level::INFO);
        assert_eq!(event.log_window_level.as_deref(), Some("tool"));
        assert!(event.log_window);
    }
}

/// Drive a real `ToolCallStart` event through the TUI's event handler, which is
/// the path a tool call takes into the log window, and assert the resulting log
/// line also reached `tracing` - the end-to-end contract of this change.
#[tokio::test(flavor = "current_thread")]
async fn test_tool_call_event_reaches_tracing_from_the_log_window() {
    use ragent_agent::event::Event;

    let mut app = support::make_app();
    // The panel is opened here only because this test also asserts the entry is
    // recorded in the log window itself; the export does not depend on it (see
    // `test_tool_call_is_exported_with_the_panel_closed`).
    app.show_log = true;
    // `handle_event` only records a log line for the session it considers
    // current, and the bare test `App` has none set, so pin one.
    let session_id = "test-session-1".to_string();
    app.session_id = Some(session_id.clone());

    // The capture must wrap the emit, so the event is handled inside `capture`.
    // `capture` takes a synchronous closure and `handle_event` is async, so the
    // block runs the future to completion on this single-threaded runtime.
    let captured = capture(|| {
        futures::executor::block_on(app.handle_event(Event::ToolCallStart {
            session_id: session_id.clone(),
            call_id: "call-1".to_string(),
            tool: "Bash".to_string(),
        }));
    });

    let event = captured
        .iter()
        .find(|c| c.message.contains("tool call: Bash"))
        .expect("a tool call recorded in the log window must reach tracing");
    assert_eq!(event.level, tracing::Level::INFO);
    assert_eq!(event.log_window_level.as_deref(), Some("tool"));
    assert!(
        event.log_window,
        "the exported record must be marked log_window=true"
    );
}

/// An `INF` line the agent event bus puts in the panel - context compression
/// starting, a response finishing, an agent notice - has no native `tracing`
/// event behind it, so it must be re-emitted at its own severity to reach the
/// OTLP logs endpoint.
///
/// This is the complement of `test_tracing_record_drain_is_not_re_emitted`: a
/// `TuiTracingLayer` record has already been exported and must stay quiet, while
/// an event-bus line has not and must not. Routing the latter through the quiet
/// `push_log` left it visible in the panel and absent from the collector.
#[test]
fn test_event_bus_info_line_is_exported() {
    let mut app = support::make_app();
    app.show_log = true;

    let captured = capture(|| {
        ragent_tui::app::push_event_bus_log_for_tests(
            &mut app,
            LogLevel::Info,
            "Context compression started",
        );
    });

    let messages = ragent_tui::app::log_window_messages_for_tests(&app);
    assert!(
        messages.iter().any(|m| m == "Context compression started"),
        "the event-bus line must still reach the log window; got {messages:?}"
    );

    let event = captured
        .iter()
        .find(|c| c.message == "Context compression started")
        .expect("an event-bus log line must be re-emitted to tracing, or the OTLP logs endpoint never sees it");
    assert_eq!(
        event.level,
        tracing::Level::INFO,
        "an INF panel entry must travel at INFO so the default floors admit it"
    );
    assert_eq!(event.log_window_level.as_deref(), Some("info"));
    assert!(event.log_window);
}

/// The same rule holds for the `WRN` and `ERR` event-bus lines: each carries its
/// own severity onto the wire so a collector can filter on it.
#[test]
fn test_event_bus_warn_and_error_lines_are_exported() {
    let mut app = support::make_app();
    app.show_log = true;

    let captured = capture(|| {
        ragent_tui::app::push_event_bus_log_for_tests(
            &mut app,
            LogLevel::Warn,
            "agent notice: retrying",
        );
        ragent_tui::app::push_event_bus_log_for_tests(
            &mut app,
            LogLevel::Error,
            "agent notice: provider failed",
        );
    });

    let warn = captured
        .iter()
        .find(|c| c.message == "agent notice: retrying")
        .expect("a WRN event-bus line must be re-emitted");
    assert_eq!(warn.level, tracing::Level::WARN);
    assert_eq!(warn.log_window_level.as_deref(), Some("warn"));

    let error = captured
        .iter()
        .find(|c| c.message == "agent notice: provider failed")
        .expect("an ERR event-bus line must be re-emitted");
    assert_eq!(error.level, tracing::Level::ERROR);
    assert_eq!(error.log_window_level.as_deref(), Some("error"));
}

/// Mirrored child-process stderr stays in the panel and the spool, and is not
/// re-emitted to `tracing`.
///
/// An MCP server launched through `npx` writes hundreds of banner and diagnostic
/// lines to stderr at startup. Those lines are not ragent's own events, so the
/// OTLP logs layer never saw them; re-emitting them would turn one panel line
/// into one exported record each.
#[test]
fn test_mirrored_stderr_is_not_exported() {
    let mut app = support::make_app();
    app.show_log = true;

    let captured = capture(|| {
        ragent_tui::app::push_mirrored_stderr_for_tests(
            &mut app,
            "npm warn deprecated inflight@1.0.6\nmcp server listening on stdio\n\n",
        );
    });

    let messages = ragent_tui::app::log_window_messages_for_tests(&app);
    assert!(
        messages
            .iter()
            .any(|m| m == "stderr: mcp server listening on stdio"),
        "the mirrored stderr line must still reach the log window; got {messages:?}"
    );
    assert_eq!(
        messages
            .iter()
            .filter(|m| m.starts_with("stderr: "))
            .count(),
        2,
        "one panel entry per non-empty stderr line; got {messages:?}"
    );
    assert!(
        !captured.iter().any(|c| c.message.starts_with("stderr: ")),
        "mirrored child stderr is documentation of another process's output, \
         not one of ragent's own events, so it must not be re-emitted; captured {:?}",
        captured
            .iter()
            .map(|c| c.message.as_str())
            .collect::<Vec<_>>()
    );
}

/// A tool call still exports when mirrored stderr is recorded beside it.
#[test]
fn test_regular_log_entry_still_exports_alongside_mirrored_stderr() {
    let mut app = support::make_app();
    app.show_log = true;

    let captured = capture(|| {
        ragent_tui::app::push_mirrored_stderr_for_tests(&mut app, "child chatter\n");
        emit_log_entry_to_tracing(LogLevel::Tool, "[1.1] tool call: Bash");
    });

    let tool = captured
        .iter()
        .find(|c| c.message == "[1.1] tool call: Bash")
        .expect("a tool-call log entry must still reach tracing");
    assert_eq!(tool.level, tracing::Level::INFO);
    assert_eq!(tool.log_window_level.as_deref(), Some("tool"));

    assert!(
        !captured.iter().any(|c| c.message.starts_with("stderr: ")),
        "the stderr line beside it stays local"
    );
}

/// The drain of `TuiTracingLayer` records into the panel records the line but
/// never re-emits it, so a collector never sees one line twice.
///
/// Every record on that channel is already a `tracing` event the OTLP logs layer
/// exported natively at its own severity. The panel copy is documentation, so the
/// drain goes through `push_log_deferred` and produces no second event at all -
/// which holds at *every* `telemetry.otel.logs.level`, not just the levels above
/// one chosen echo severity.
#[test]
fn test_tracing_record_drain_is_not_re_emitted() {
    let mut app = support::make_app();
    app.show_log = true;

    let captured = capture(|| {
        ragent_tui::app::push_tracing_record_for_tests(&mut app, "startup line");
    });

    let messages = ragent_tui::app::log_window_messages_for_tests(&app);
    assert!(
        messages.iter().any(|m| m == "startup line"),
        "the drained record must still reach the log panel; got {messages:?}"
    );
    assert!(
        !captured.iter().any(|c| c.message == "startup line"),
        "the drained line was already exported natively; re-emitting the panel \
         copy would duplicate it in the collector at any logs.level. captured {:?}",
        captured
            .iter()
            .map(|c| c.message.as_str())
            .collect::<Vec<_>>()
    );
}
