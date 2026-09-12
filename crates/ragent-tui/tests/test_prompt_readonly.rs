//! T-016 regression tests: FR-012 read-only / no-LLM guarantee (spec `prompts`).
//!
//! FR-012: the `/prompt` command SHALL make no LLM request, SHALL not write to
//! any file, SQLite store, or configuration, and SHALL NOT persist its rendered
//! output into the session message history (output is TUI-display only,
//! following the `/toolchain` channel pattern), so repeated invocations leave
//! the session and token usage untouched.
//!
//! Verified at the observable level:
//!
//! - no LLM request: the cumulative token-usage counters stay `(0, 0)` (they
//!   are only incremented by LLM stream events in the TUI event handler);
//! - no persisted-session mutation: the persisted message list read back from
//!   the session manager is byte-identical before and after repeated `/prompt`
//!   invocations;
//! - display-only delivery: the report still appears in the in-memory TUI
//!   message window (the `/toolchain` channel pattern) even though nothing was
//!   persisted.

mod support;
use support::make_app;

/// Run a `/prompt` subcommand and return the rendered assistant text.
///
/// `handle_prompt_render` calls `tokio::task::block_in_place`, which is only
/// legal on the multi-threaded runtime, so the harness spins one up (the
/// production TUI runs on the multi-thread runtime as well).
fn prompt_report(input: &str) -> String {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("multi-thread runtime")
        .block_on(async move {
            let mut app = make_app();
            app.execute_slash_command(input);
            app.messages
                .last()
                .map(|m| m.text_content())
                .unwrap_or_default()
        })
}

#[test]
fn test_prompt_no_llm_request_token_usage_unchanged() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("multi-thread runtime")
        .block_on(async move {
            let mut app = make_app();
            app.execute_slash_command("/prompt help"); // creates the session via the shared gate
            let before = app.token_usage;
            app.execute_slash_command("/prompt general");
            app.execute_slash_command("/prompt subagent general");
            app.execute_slash_command("/prompt primary general");
            assert_eq!(
                app.token_usage, before,
                "FR-012: /prompt must make no LLM request (token usage unchanged)"
            );
            assert_eq!(
                app.token_usage,
                (0, 0),
                "FR-012: token counters must be untouched by /prompt"
            );
        });
}

#[test]
fn test_prompt_persists_nothing_to_session_history() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("multi-thread runtime")
        .block_on(async move {
            let mut app = make_app();
            app.execute_slash_command("/prompt help"); // creates the session via the shared gate
            let sid = app
                .session_id
                .clone()
                .expect("session created by the slash-command gate");
            let persisted_before = app
                .session_processor
                .session_manager
                .get_messages(&sid)
                .expect("read persisted session history");

            app.execute_slash_command("/prompt general");
            app.execute_slash_command("/prompt subagent general");

            let persisted_after = app
                .session_processor
                .session_manager
                .get_messages(&sid)
                .expect("read persisted session history");
            assert_eq!(
                persisted_before.len(),
                persisted_after.len(),
                "FR-012: /prompt must not persist anything into the session history"
            );
            // Message does not implement PartialEq; compare the observable
            // projection (id + text content) instead.
            let snapshot = |msgs: &[ragent_types::Message]| -> Vec<(String, String)> {
                msgs.iter()
                    .map(|m| (m.id.clone(), m.text_content()))
                    .collect()
            };
            assert_eq!(
                snapshot(&persisted_before),
                snapshot(&persisted_after),
                "FR-012: persisted session history must be identical across /prompt runs"
            );
        });
}

#[test]
fn test_prompt_display_only_output_is_rendered() {
    // The report must still be displayed in the TUI message window
    // (TUI-display-only channel), while nothing is persisted (the companion
    // test above proves that half).
    let text = prompt_report("/prompt general");
    assert!(
        text.starts_with("From: /prompt"),
        "FR-012: the report must be rendered as TUI-display output, got: {text}"
    );
}

#[test]
fn test_prompt_repeated_invocations_are_stable() {
    let first = prompt_report("/prompt general");
    let second = prompt_report("/prompt general");
    assert_eq!(
        first, second,
        "FR-012: repeated /prompt invocations must leave the report identical"
    );
}
