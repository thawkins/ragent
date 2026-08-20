//! Contained-panic marker for cooperating with the global panic hook.
//!
//! Some code paths deliberately wrap fallible third-party parsing (PDF font
//! decoding, readability, html2text, prompt classifiers) in
//! [`std::panic::catch_unwind`] so that a dependency bug degrades into an
//! `Err` instead of killing the process. However, Rust's panic *hook* fires
//! at panic time — before the unwind reaches the `catch_unwind` frame — so
//! every caught panic still wrote a `log/panic-*.log` file and, in the TUI,
//! tore down the terminal even though the caller recovered successfully.
//!
//! This module provides a tiny thread-local "contained" flag. While
//! [`run`] is executing, the flag is set; the global panic hooks installed in
//! `src/panic_hook.rs` and `ragent-tui` check [`is_active`] and skip their
//! crash-path behaviour (log writing, terminal teardown) when a contained
//! panic fires. A panic outside all containers is reported exactly as before.

/// Set the thread-local contained-panic flag.
///
/// Private — use [`run`], which wraps the call and guarantees the flag is
/// cleared on all exit paths (including when `f` panics).
fn set_active(active: bool) {
    PANIC_CONTAINED.with(|c| c.set(active));
}

thread_local! {
    static PANIC_CONTAINED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Returns `true` when the current thread is inside a [`run`] container.
///
/// Global panic hooks call this to decide whether a panic is deliberate —
/// i.e. about to be caught by a `catch_unwind` inside [`run`] — or an actual
/// crash that requires terminal teardown and a panic log.
pub fn is_active() -> bool {
    PANIC_CONTAINED.with(std::cell::Cell::get)
}

/// Execute `f` inside a contained-panic container.
///
/// Semantically identical to `std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))`,
/// but sets the thread-local contained flag for the duration of the call so
/// the global panic hooks can distinguish "panic that is about to be caught"
/// from "panic that will crash the process".
///
/// The flag is cleared regardless of how `f` exits, and a panic in `f` is
/// returned as `Err` like `catch_unwind`. Only use this for code paths where
/// recovering from a panic is explicitly intended and the result degrades
/// gracefully (fallback extraction, degraded tier, etc.).
pub fn run<F: FnOnce() -> R + std::panic::UnwindSafe, R>(f: F) -> std::thread::Result<R> {
    struct ResetOnDrop;

    impl Drop for ResetOnDrop {
        fn drop(&mut self) {
            set_active(false);
        }
    }

    set_active(true);
    let _guard = ResetOnDrop;
    std::panic::catch_unwind(f)
}
