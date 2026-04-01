//! Tests for secure temp file handling, percent-decoding, and clipboard limits.
//!
//! Covers Tasks 3.1, 3.2, and 3.3 from the TUI compliance plan (Milestone 3).

use ragent_tui::app::{percent_decode_path, save_clipboard_image_to_temp};

// =========================================================================
// percent_decode_path — ASCII
// =========================================================================

#[test]
fn test_percent_decode_no_encoding() {
    let path = percent_decode_path("/home/user/file.txt");
    assert_eq!(path, std::path::PathBuf::from("/home/user/file.txt"));
}

#[test]
fn test_percent_decode_space() {
    let path = percent_decode_path("/home/user/my%20file.txt");
    assert_eq!(path, std::path::PathBuf::from("/home/user/my file.txt"));
}

#[test]
fn test_percent_decode_multiple_encoded() {
    let path = percent_decode_path("/a%20b%2Fc%23d");
    // %20 = space, %2F = /, %23 = #
    assert_eq!(path, std::path::PathBuf::from("/a b/c#d"));
}

#[test]
fn test_percent_decode_hash_and_percent() {
    let path = percent_decode_path("/dir%23name/file%25.txt");
    // %23 = #, %25 = %
    assert_eq!(path, std::path::PathBuf::from("/dir#name/file%.txt"));
}

// =========================================================================
// percent_decode_path — multi-byte UTF-8
// =========================================================================

#[test]
fn test_percent_decode_utf8_accented() {
    // "café" → c a f %C3%A9
    let path = percent_decode_path("/caf%C3%A9");
    assert_eq!(path, std::path::PathBuf::from("/café"));
}

#[test]
fn test_percent_decode_utf8_cjk() {
    // 中 = U+4E2D = E4 B8 AD in UTF-8
    let path = percent_decode_path("/%E4%B8%AD%E6%96%87");
    assert_eq!(path, std::path::PathBuf::from("/中文"));
}

#[test]
fn test_percent_decode_utf8_emoji() {
    // 🦀 = U+1F980 = F0 9F A6 80 in UTF-8
    let path = percent_decode_path("/emoji_%F0%9F%A6%80");
    assert_eq!(path, std::path::PathBuf::from("/emoji_🦀"));
}

// =========================================================================
// percent_decode_path — malformed sequences
// =========================================================================

#[test]
fn test_percent_decode_trailing_percent() {
    // Lone % at the end — pass through as literal
    let path = percent_decode_path("/file%");
    assert_eq!(path, std::path::PathBuf::from("/file%"));
}

#[test]
fn test_percent_decode_percent_single_hex() {
    // %A (only one hex digit) — pass through as literal
    let path = percent_decode_path("/file%A");
    assert_eq!(path, std::path::PathBuf::from("/file%A"));
}

#[test]
fn test_percent_decode_invalid_hex() {
    // %ZZ is not valid hex — pass through as literal
    let path = percent_decode_path("/file%ZZname");
    assert_eq!(path, std::path::PathBuf::from("/file%ZZname"));
}

#[test]
fn test_percent_decode_empty() {
    let path = percent_decode_path("");
    assert_eq!(path, std::path::PathBuf::from(""));
}

#[test]
fn test_percent_decode_only_percent() {
    let path = percent_decode_path("%%%");
    // First % tries to read "%%" as hex → invalid, passed through literally.
    // Second % has only one char left → passed through literally.
    // Third % is the last char → passed through literally.
    assert_eq!(path, std::path::PathBuf::from("%%%"));
}

// =========================================================================
// percent_decode_path — non-UTF-8 on Unix
// =========================================================================

#[cfg(unix)]
#[test]
fn test_percent_decode_non_utf8_bytes() {
    use std::os::unix::ffi::OsStrExt;

    // %FF is not valid UTF-8, but on Unix it should survive as a raw byte.
    let path = percent_decode_path("/file%FF");
    let os_bytes = path.as_os_str().as_bytes();
    assert_eq!(os_bytes, b"/file\xFF");
}

#[cfg(unix)]
#[test]
fn test_percent_decode_latin1_sequence() {
    use std::os::unix::ffi::OsStrExt;

    // %E9 alone (not part of a multi-byte UTF-8 sequence) is 0xE9 (Latin-1 'é')
    let path = percent_decode_path("/caf%E9");
    let os_bytes = path.as_os_str().as_bytes();
    assert_eq!(os_bytes, b"/caf\xE9");
}

// =========================================================================
// save_clipboard_image_to_temp — happy path
// =========================================================================

#[test]
fn test_save_clipboard_image_creates_file() {
    // Create a tiny 2×2 RGBA image.
    let pixels: Vec<u8> = vec![
        255, 0, 0, 255, // red
        0, 255, 0, 255, // green
        0, 0, 255, 255, // blue
        255, 255, 0, 255, // yellow
    ];
    let img = arboard::ImageData {
        width: 2,
        height: 2,
        bytes: std::borrow::Cow::Owned(pixels),
    };

    let path = save_clipboard_image_to_temp(&img).expect("should create temp file");
    assert!(path.exists(), "temp file should exist on disk");
    assert_eq!(
        path.extension().and_then(|e| e.to_str()),
        Some("png"),
        "file should have .png extension"
    );

    // Verify the file starts with a PNG magic number.
    let header = std::fs::read(&path).unwrap();
    assert!(header.len() >= 8);
    assert_eq!(&header[..4], b"\x89PNG");

    // Clean up.
    let _ = std::fs::remove_file(&path);
}

#[test]
fn test_save_clipboard_image_unique_paths() {
    let pixels: Vec<u8> = vec![0u8; 4 * 4]; // 1×1 RGBA
    let img = arboard::ImageData {
        width: 1,
        height: 1,
        bytes: std::borrow::Cow::Owned(pixels),
    };

    let path1 = save_clipboard_image_to_temp(&img).unwrap();
    let path2 = save_clipboard_image_to_temp(&img).unwrap();
    assert_ne!(path1, path2, "each call should produce a unique temp file");

    let _ = std::fs::remove_file(&path1);
    let _ = std::fs::remove_file(&path2);
}

// =========================================================================
// save_clipboard_image_to_temp — size limit
// =========================================================================

#[test]
fn test_save_clipboard_image_rejects_oversized_buffer() {
    // 50 MB + 1 byte
    let too_big = vec![0u8; 50 * 1024 * 1024 + 1];
    let img = arboard::ImageData {
        width: 1,
        height: 1,
        bytes: std::borrow::Cow::Owned(too_big),
    };

    let err = save_clipboard_image_to_temp(&img).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("too large"), "error should mention size: {msg}");
}

#[test]
fn test_save_clipboard_image_rejects_oversized_dimensions() {
    // 16385 × 1 exceeds MAX_CLIPBOARD_IMAGE_DIM.
    let pixels = vec![0u8; 16385 * 4];
    let img = arboard::ImageData {
        width: 16385,
        height: 1,
        bytes: std::borrow::Cow::Owned(pixels),
    };

    let err = save_clipboard_image_to_temp(&img).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("dimensions too large"),
        "error should mention dimensions: {msg}"
    );
}

#[test]
fn test_save_clipboard_image_dimension_mismatch() {
    // 2×2 but only 4 bytes (not 16).
    let pixels = vec![0u8; 4];
    let img = arboard::ImageData {
        width: 2,
        height: 2,
        bytes: std::borrow::Cow::Owned(pixels),
    };

    let err = save_clipboard_image_to_temp(&img).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("mismatch"),
        "error should mention mismatch: {msg}"
    );
}

// =========================================================================
// is_image_path
// =========================================================================

#[test]
fn test_is_image_path_recognized_extensions() {
    use ragent_tui::app::is_image_path;
    use std::path::Path;

    for ext in &["png", "jpg", "jpeg", "gif", "webp", "bmp", "tiff", "tif"] {
        let name = format!("image.{ext}");
        let p = Path::new(&name);
        assert!(is_image_path(p), "{ext} should be recognized");
    }
}

#[test]
fn test_is_image_path_case_insensitive() {
    use ragent_tui::app::is_image_path;
    use std::path::Path;

    assert!(is_image_path(Path::new("photo.PNG")));
    assert!(is_image_path(Path::new("photo.Jpg")));
}

#[test]
fn test_is_image_path_rejects_non_image() {
    use ragent_tui::app::is_image_path;
    use std::path::Path;

    assert!(!is_image_path(Path::new("readme.md")));
    assert!(!is_image_path(Path::new("script.rs")));
    assert!(!is_image_path(Path::new("noext")));
}
