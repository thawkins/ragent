//! Build script for ragent-tui.
//!
//! Embeds the compile-time timestamp into the binary so the TUI can display
//! when the application was built.

use std::time::SystemTime;

/// Emits `COMPILE_TIME` as a compile-time environment variable.
///
/// The value is captured as `YYYY-MM-DD HH:MM:SS UTC` and consumed in
/// `crates/ragent-tui/src/lib.rs` via `option_env!("COMPILE_TIME")`.
fn main() {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system time before epoch");
    let dt = chrono::DateTime::from_timestamp(now.as_secs() as i64, 0)
        .expect("invalid timestamp")
        .format("%Y-%m-%d %H:%M:%S UTC");
    println!("cargo:rustc-env=COMPILE_TIME={dt}");
}
