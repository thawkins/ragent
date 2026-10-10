//! Browser/screenshot capability gating (spec `openhands` FR-029, T-017).
//!
//! The `browser` CDP tool was removed (v1.0.129). FR-029 requires the remaining
//! screenshot capability to be provided **only** when a real headless engine is
//! present, and to **never be advertised when no engine is present**. These tests
//! pin that contract for the default build:
//!
//! - the default build compiles no drivable headless-browser driver, so
//!   [`headless_engine_present`] is `false` and `mf_screenshot` is absent from
//!   [`create_extended_registry`]'s advertised tool set (test-plan TC-014,
//!   "no engine present" branch);
//! - the always-on `mf_*` tools are unaffected (the gate removes exactly one
//!   tool);
//! - [`detect_headless_engine`] is a pure `PATH` probe that agrees with the host.

use ragent_tools_extended::create_extended_registry;
use ragent_tools_extended::masterfetch::tools::screenshot::{
    HEADLESS_ENGINE_BINARIES, detect_headless_engine, headless_engine_present,
};

/// The default build advertises no screenshot tool: no engine, no capability.
#[cfg(not(feature = "headless-browser"))]
#[test]
fn test_default_build_does_not_advertise_the_screenshot_tool() {
    // No `headless-browser` feature in the default build, so no drivable engine.
    assert!(
        !headless_engine_present(),
        "default build must not claim a drivable headless engine (FR-029)"
    );

    let registry = create_extended_registry();
    assert!(
        registry.get("mf_screenshot").is_none(),
        "mf_screenshot must not be registered when no engine is present (FR-029)"
    );
    let advertised: Vec<String> = registry.definitions().into_iter().map(|d| d.name).collect();
    assert!(
        !advertised.iter().any(|n| n == "mf_screenshot"),
        "mf_screenshot must never be advertised when it cannot work (FR-029)"
    );
}

/// Gating one capability does not remove the always-on masterfetch tools.
#[test]
fn test_other_masterfetch_tools_remain_advertised() {
    let registry = create_extended_registry();
    let advertised: std::collections::HashSet<String> =
        registry.definitions().into_iter().map(|d| d.name).collect();
    for name in [
        "mf_fetch",
        "mf_crawl",
        "mf_search",
        "mf_cache_clear",
        "mf_version",
    ] {
        assert!(
            advertised.contains(name),
            "'{name}' should still be advertised (the gate removes only the screenshot tool)"
        );
    }
}

/// `detect_headless_engine` is a pure `PATH` probe whose result agrees with the
/// host: `Some` iff a candidate binary resolves, and any hit is a known engine.
#[test]
fn test_detect_headless_engine_agrees_with_the_host_path() {
    let found = detect_headless_engine();
    if let Some(path) = &found {
        let file = path
            .file_name()
            .and_then(std::ffi::OsStr::to_str)
            .unwrap_or("");
        // Strip a Windows executable suffix before comparing.
        let stem = file.rsplit_once('.').map_or(file, |(before, _ext)| before);
        assert!(
            HEADLESS_ENGINE_BINARIES.contains(&stem),
            "detected engine '{file}' is not one of the known candidates"
        );
        assert!(path.is_file(), "a detected engine must be a real file");
    }
}
