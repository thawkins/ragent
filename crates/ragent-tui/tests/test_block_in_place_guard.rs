//! Guard test for FUNC-015: a `Handle::current().block_on(..)` on the TUI's
//! async runtime must be wrapped in `tokio::task::block_in_place` so the block
//! does not stall the runtime's worker thread (or panic on a current-thread
//! runtime). A bare `block_on` on the UI runtime would deadlock the event loop.
//!
//! `rt.block_on(..)` against a *dedicated* `Runtime::new()` created inside a
//! spawned OS thread is exempt and not matched here.
//!
//! Sites legitimately converted to plain `self.<async>.await` (the slash
//! dispatcher itself is an `async fn`, so awaiting is the correct fix, not
//! block_in_place) must be listed in `AWAITED_EXEMPTIONS` so the guard keeps
//! at least one live block_on/block_in_place site to protect.

use std::path::PathBuf;

/// Call sites that were `Handle::current().block_on(..)` inside `block_in_place`
/// but are now plain `.await`s because the enclosing slash handler gained
/// `async`. Each entry is the line number at head-of-file time of the site in
/// `src/app/slash.rs` whose replacement should be an `.await`, paired with the
/// surrounding async fn name so drift is detected.
const AWAITED_EXEMPTIONS: &[(&str, &str)] = &[
    // /mcp discover — McpClient::discover()
    ("mcp_discover", "McpClient::discover().await"),
    // /mcp connect|disconnect — set_mcp_server_enabled
    (
        "mcp_connect_disconnect",
        "self.set_mcp_server_enabled(id, sub == \"connect\").await",
    ),
];

fn slash_source() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/app/slash.rs");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

#[test]
fn func015_handle_block_on_is_inside_block_in_place() {
    let source = slash_source();
    let lines: Vec<&str> = source.lines().collect();

    let mut checked = 0usize;
    for (index, line) in lines.iter().enumerate() {
        // Match either the single-line `Handle::current().block_on(` form or
        // a `Handle::current()` receiver whose `.block_on(..)` continues on
        // the next line (rustfmt splits long receivers).
        let has_handle_current = line.contains("Handle::current()");
        let block_on_same_line = line.contains("Handle::current().block_on");
        let block_on_next_line = has_handle_current
            && lines
                .get(index + 1)
                .is_some_and(|next| next.trim_start().starts_with(".block_on"));
        if !block_on_same_line && !block_on_next_line {
            continue;
        }
        checked += 1;
        // The wrapping `block_in_place` must appear on this line or within a
        // short window before it (the wrapping closure may span several lines
        // between the `block_in_place(|| {` opening and the `block_on` call).
        let start = index.saturating_sub(8);
        let window = lines[start..=index].join("\n");
        assert!(
            window.contains("block_in_place"),
            "slash.rs:{} calls Handle::current().block_on without wrapping it in \
             block_in_place; this stalls the TUI runtime (FUNC-015)\n    {}",
            index + 1,
            line.trim()
        );
    }

    // Every exempted former block_on site must actually have been converted
    // to the listed plain `.await` in the source, so removing the block_on
    // silently (without adopting await) fails here.
    for (label, expected_await) in AWAITED_EXEMPTIONS {
        assert!(
            source.contains(expected_await),
            "exemption `{label}` expects slash.rs to contain `{expected_await}`; \
             if the site was changed again, update or remove this exemption"
        );
    }

    assert!(
        checked > 0,
        "expected at least one Handle::current().block_on site to guard"
    );
}
