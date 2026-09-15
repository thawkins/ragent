//! Guard test for FUNC-015: a `Handle::current().block_on(..)` on the TUI's
//! async runtime must be wrapped in `tokio::task::block_in_place` so the block
//! does not stall the runtime's worker thread (or panic on a current-thread
//! runtime). A bare `block_on` on the UI runtime would deadlock the event loop.
//!
//! `rt.block_on(..)` against a *dedicated* `Runtime::new()` created inside a
//! spawned OS thread is exempt and not matched here.

use std::path::PathBuf;

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
        if !line.contains("Handle::current().block_on") {
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

    assert!(
        checked > 0,
        "expected at least one Handle::current().block_on site to guard"
    );
}
