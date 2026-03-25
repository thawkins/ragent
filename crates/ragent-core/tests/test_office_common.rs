//! Tests for test_office_common.rs

//! Unit tests for office_common helpers.

use std::path::Path;

use ragent_core::tool::office_common::{detect_format, resolve_path, truncate_output, OfficeFormat, MAX_OUTPUT_BYTES};

#[test]
fn test_detect_format_supported_extensions() {
    assert_eq!(detect_format(Path::new("/tmp/test.docx")).unwrap(), OfficeFormat::Docx);
    assert_eq!(detect_format(Path::new("/tmp/test.xlsx")).unwrap(), OfficeFormat::Xlsx);
    assert_eq!(detect_format(Path::new("/tmp/test.pptx")).unwrap(), OfficeFormat::Pptx);
}

#[test]
fn test_detect_format_unsupported_extension_error() {
    let err = detect_format(Path::new("/tmp/test.txt")).unwrap_err();
    assert!(err.to_string().contains("Unsupported file extension"));
}

#[test]
fn test_detect_format_legacy_extension_error() {
    let err = detect_format(Path::new("/tmp/test.doc")).unwrap_err();
    assert!(err.to_string().contains("Legacy Office format"));
}

#[test]
fn test_detect_format_no_extension_error() {
    let err = detect_format(Path::new("/tmp/test")).unwrap_err();
    assert!(err.to_string().contains("no extension"));
}

#[test]
fn test_resolve_path_relative_and_absolute() {
    let cwd = Path::new("/tmp/work");
    let abs = resolve_path(cwd, "/etc/config");
    assert_eq!(abs, Path::new("/etc/config"));

    let rel = resolve_path(cwd, "sub/dir/file.txt");
    assert_eq!(rel, Path::new("/tmp/work/sub/dir/file.txt"));
}

#[test]
fn test_truncate_output_respects_max_bytes() {
    let base = "a".repeat(MAX_OUTPUT_BYTES + 1);
    let truncated = truncate_output(base.clone());
    assert!(truncated.contains("Output truncated"));
    assert!(truncated.len() < base.len());
}

#[test]
fn test_truncate_output_no_truncate_when_small() {
    let base = "a".repeat(10);
    let out = truncate_output(base.clone());
    assert_eq!(out, base);
}
