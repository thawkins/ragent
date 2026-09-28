//! Truncating stderr spool shared by the ragent binary and the TUI.
//!
//! The TUI runs on the alternate screen, so anything written to the process's
//! raw stderr — Rust's default panic hook, `eprintln!`, C-library diagnostics —
//! is painted over by the next ratatui frame and effectively lost. The binary
//! redirects fd 2 into a [`Spool`] before entering the TUI and the TUI attaches
//! a mirror sink that also surfaces the text in its log panel.
//!
//! The spool keeps only the newest [`SPOOL_MAX_LINES`] lines: each write counts
//! new newlines and rewrites the file with its freshest lines once the cap is
//! exceeded. The cap is generous (1000 lines) so the rewrite cost is amortised
//! across many writes, and the rewrite is skipped entirely when a write
//! contains no newline.

mod spool;

pub use spool::{SPOOL_MAX_LINES, Spool};
