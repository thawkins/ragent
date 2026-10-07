//! Integration tests for the shared `format_size` helper (T-301).
//!
//! The helper has a single definition in `ragent_types::strutil` and is
//! imported by both former call sites (`ragent-agent` reference resolver and
//! `ragent-tools-core` `list`). These tests pin the human-readable format.

use ragent_types::strutil::format_size;

#[test]
fn test_format_size_bytes() {
    assert_eq!(format_size(0), "0 B");
    assert_eq!(format_size(512), "512 B");
    assert_eq!(format_size(1023), "1023 B");
}

#[test]
fn test_format_size_kilobytes() {
    assert_eq!(format_size(1024), "1.0 KB");
    assert_eq!(format_size(1536), "1.5 KB");
    assert_eq!(format_size(1024 * 1024 - 1), "1024.0 KB");
}

#[test]
fn test_format_size_megabytes() {
    assert_eq!(format_size(1024 * 1024), "1.0 MB");
    assert_eq!(format_size(3 * 1024 * 1024 / 2), "1.5 MB");
}
