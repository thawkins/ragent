//! Inline tests for `layout_statusbar.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

#[test]
fn test_responsive_mode_from_width() {
    assert_eq!(ResponsiveMode::from_width(50), ResponsiveMode::Minimal);
    assert_eq!(ResponsiveMode::from_width(79), ResponsiveMode::Minimal);
    assert_eq!(ResponsiveMode::from_width(80), ResponsiveMode::Compact);
    assert_eq!(ResponsiveMode::from_width(119), ResponsiveMode::Compact);
    assert_eq!(ResponsiveMode::from_width(120), ResponsiveMode::Full);
    assert_eq!(ResponsiveMode::from_width(200), ResponsiveMode::Full);
}

#[test]
fn test_shorten_path() {
    assert_eq!(shorten_path("/home/user", 50), "/home/user");

    let long_path = "/very/long/path/that/exceeds/maximum";
    let shortened = shorten_path(long_path, 20);
    assert!(shortened.chars().count() <= 20);
    assert!(shortened.contains("..."));
}
