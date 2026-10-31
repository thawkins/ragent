//! Regression tests for the thread-aware TUI panic hook
//! (`ragent_tui::should_teardown_terminal`).
//!
//! The telemetry-enabled startup crash had two halves: the OTLP exporter
//! panicked on its own OS thread ("there is no reactor running"), and the
//! process-wide panic hook then restored the terminal from *that* thread -
//! leaving ragent running with a destroyed alternate screen and a raw-mode
//! input stream leaking escape sequences to the shell.
//!
//! The policy is that only a panic on the UI thread tears the terminal down.
//! These tests pin that policy; without them a refactor could silently restore
//! the "any thread tears down the display" behaviour.

use ragent_tui::should_teardown_terminal;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::ThreadId;

/// A panic on the UI thread with no contained-panic guard active must restore
/// the terminal, so the backtrace is readable on a sane screen.
#[test]
fn test_ui_thread_panic_tears_down_terminal() {
    let ui = std::thread::current().id();
    assert!(
        should_teardown_terminal(false, ui, ui),
        "a panic on the UI thread must restore the terminal"
    );
}

/// A panic on a spawned helper thread must NOT restore the terminal: the
/// process keeps running and the display has to stay intact.
#[test]
fn test_helper_thread_panic_leaves_terminal_intact() {
    let ui = std::thread::current().id();
    let helper = std::thread::spawn(|| std::thread::current().id())
        .join()
        .expect("helper thread should not panic");
    assert_ne!(helper, ui, "the helper must run on a different thread");
    assert!(
        !should_teardown_terminal(false, helper, ui),
        "a panic on a helper thread must not tear down the terminal"
    );
}

/// A panic inside a contained-panic container (a supervisor that catches it
/// and continues) must never tear the terminal down, even on the UI thread.
#[test]
fn test_contained_panic_never_tears_down_terminal() {
    let ui = std::thread::current().id();
    assert!(
        !should_teardown_terminal(true, ui, ui),
        "a contained panic is caught by its caller and must leave the screen alone"
    );
    let helper = std::thread::spawn(|| std::thread::current().id())
        .join()
        .expect("helper thread should not panic");
    assert!(
        !should_teardown_terminal(true, helper, ui),
        "a contained panic on a helper thread must also leave the screen alone"
    );
}

/// End-to-end exercise of the real hook wiring: a helper-thread panic runs the
/// installed hook, and the teardown branch is skipped.
///
/// The process-wide hook cannot be observed directly without touching the real
/// terminal, so this test installs a stand-in hook that reproduces the exact
/// call the production closure makes and records whether the teardown branch
/// was taken by that off-thread panic.
///
/// The hook is process-global, so the whole test holds the crate-wide lock and
/// restores the previous hook before returning.
#[test]
fn test_offthread_panic_skips_teardown_branch_end_to_end() {
    let _guard = crate::global_env_lock();

    // Silence the backtrace the stand-in hook would otherwise print for a
    // deliberately triggered panic.
    let previous = std::panic::take_hook();
    let ui_thread = std::thread::current().id();
    let teardown_taken = Arc::new(AtomicBool::new(false));

    let flag = Arc::clone(&teardown_taken);
    std::panic::set_hook(Box::new(move |_info| {
        // Exactly the production condition, minus the crossterm calls.
        let contained = ragent_types::panic_guard::is_active();
        if should_teardown_terminal(contained, std::thread::current().id(), ui_thread) {
            flag.store(true, Ordering::SeqCst);
        }
    }));

    let result = std::thread::spawn(|| panic!("deliberate off-thread panic")).join();
    std::panic::set_hook(previous);

    assert!(result.is_err(), "the helper thread must have panicked");
    assert!(
        !teardown_taken.load(Ordering::SeqCst),
        "the teardown branch must not run for a helper-thread panic"
    );
}

/// The same end-to-end wiring on the UI thread must take the teardown branch.
#[test]
fn test_ui_thread_panic_takes_teardown_branch_end_to_end() {
    let _guard = crate::global_env_lock();

    let previous = std::panic::take_hook();
    let ui_thread = std::thread::current().id();
    let teardown_taken = Arc::new(AtomicBool::new(false));

    let flag = Arc::clone(&teardown_taken);
    std::panic::set_hook(Box::new(move |_info| {
        let contained = ragent_types::panic_guard::is_active();
        if should_teardown_terminal(contained, std::thread::current().id(), ui_thread) {
            flag.store(true, Ordering::SeqCst);
        }
    }));

    // Catch the panic on this (the UI) thread so the test itself survives.
    let caught = std::panic::catch_unwind(|| panic!("deliberate UI-thread panic"));
    std::panic::set_hook(previous);

    assert!(caught.is_err(), "the UI thread must have panicked");
    assert!(
        teardown_taken.load(Ordering::SeqCst),
        "the teardown branch must run for a UI-thread panic"
    );
}

/// Guard against the two thread-id arguments being interchangeable: the
/// decision must be a strict identity comparison, not an "is set" check.
#[test]
fn test_thread_identity_is_strict() {
    let a: ThreadId = std::thread::current().id();
    let b = std::thread::spawn(|| std::thread::current().id())
        .join()
        .expect("helper thread should not panic");
    assert!(should_teardown_terminal(false, a, a));
    assert!(!should_teardown_terminal(false, b, a));
    assert!(!should_teardown_terminal(false, a, b));
    assert!(should_teardown_terminal(false, b, b));
}
