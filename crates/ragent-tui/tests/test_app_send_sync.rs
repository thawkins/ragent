//! Regression guard: `App` must stay `Send + Sync`.
//!
//! The TUI event loop holds `App` across `.await` points, so it must be `Send`.
//! The plugin subsystem owns `rquickjs` sandbox contexts (`!Send + !Sync`); the
//! `/plugins` command surface (spec `plugins` T-014) therefore creates and
//! drops its `PluginSession` inside a single synchronous dispatch rather than
//! storing one on `App`. This test fails if that invariant is broken.

fn assert_send_sync<T: Send + Sync>() {}

/// `App` is `Send + Sync`, so it may be held across `.await` points.
#[test]
fn app_is_send_and_sync() {
    assert_send_sync::<ragent_tui::App>();
}
