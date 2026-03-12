//! Build script for ragent-tui: embeds the build timestamp.

fn main() {
    // Embed the build timestamp so /about can display it.
    let now = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    println!("cargo:rustc-env=BUILD_TIMESTAMP={now}");
    // Re-run only when the build script itself changes (not on every compile).
    println!("cargo:rerun-if-changed=build.rs");
}
