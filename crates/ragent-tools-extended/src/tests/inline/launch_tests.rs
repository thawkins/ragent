//! Inline tests for `launch.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_browser_binary_candidates_nonempty() {
    let candidates = browser_binary_candidates();
    // On any platform, we should have at least one candidate path.
    // On unknown platforms, the list may be empty - skip the assertion.
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    assert!(
        !candidates.is_empty(),
        "should have platform-specific browser candidates"
    );
}

#[test]
fn test_default_debug_port() {
    assert_eq!(DEFAULT_DEBUG_PORT, 9222);
}

#[cfg(unix)]
#[test]
fn test_which_binary_nonexistent() {
    let result = which_binary("this-binary-definitely-does-not-exist-12345");
    assert!(result.is_err());
}
