//! Build script for ragent-research.
//!
//! Embeds the compile-time timestamp into the binary so each generated
//! `RESEARCH.md` can record when the producing ragent binary was built.

use std::time::SystemTime;

/// Emits `COMPILE_TIME` as a compile-time environment variable.
///
/// The value is captured as `YYYY-MM-DD HH:MM:SS UTC` and consumed in
/// `crates/ragent-research/src/document.rs` via `option_env!("COMPILE_TIME")`.
fn main() {
    // Only re-run when the build script itself changes; the compile-time
    // timestamp is intentionally not a rerun trigger because it would
    // invalidate the build on every invocation.
    println!("cargo:rerun-if-changed=build.rs");
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .expect("system time before epoch");
    let dt = chrono::DateTime::from_timestamp(now.as_secs() as i64, 0)
        .expect("invalid timestamp")
        .format("%Y-%m-%d %H:%M:%S UTC");
    println!("cargo:rustc-env=COMPILE_TIME={dt}");
}
