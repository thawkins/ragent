//! Regression tests for the shared security guards (SECTASKS MS-05, T-067/T-068/T-070).
//!
//! Each guard existed as a per-crate copy before MS-05. These tests pin the
//! canonical behaviour so a future change cannot silently weaken every call site
//! at once.

use std::path::Path;
use std::time::Duration;

use ragent_types::guard::{
    MAX_IDENTIFIER_LEN, MAX_RETRY_AFTER, cap_read, clamp_retry_after, contained_join,
    is_safe_operand, reject_option_like, validate_identifier, validate_relative_component,
};

// ---------------------------------------------------------------------------
// T-068: option-like operand rejection (git argument injection)
// ---------------------------------------------------------------------------

#[test]
fn test_guard_reject_option_like_accepts_ordinary_operands() {
    for value in [
        "main",
        "v1.2.3",
        "feature/nested-branch",
        "origin",
        "HEAD~1",
    ] {
        assert!(
            reject_option_like(value, "ref").is_ok(),
            "'{value}' should be an acceptable operand"
        );
    }
}

#[test]
fn test_guard_reject_option_like_refuses_dash_prefixed_values() {
    let injections = [
        "--upload-pack=/bin/sh -c true",
        "--receive-pack=touch",
        "-c",
        "-o",
        "--output=/etc/passwd",
    ];
    for value in injections {
        let err =
            reject_option_like(value, "ref").expect_err("an option-like value must be refused");
        assert!(err.contains("ref"), "error should name the label: {err}");
    }
}

#[test]
fn test_guard_reject_option_like_refuses_empty_value() {
    assert!(reject_option_like("", "branch").is_err());
}

#[test]
fn test_guard_is_safe_operand_requires_the_restricted_charset() {
    assert!(is_safe_operand("feature/x-1.2"));
    assert!(!is_safe_operand("-leading"));
    assert!(!is_safe_operand("has space"));
    assert!(!is_safe_operand("shell;rm -rf"));
    assert!(!is_safe_operand("$(id)"));
    assert!(!is_safe_operand(""));
}

// ---------------------------------------------------------------------------
// T-067: identifier validation
// ---------------------------------------------------------------------------

#[test]
fn test_guard_validate_identifier_accepts_ordinary_components() {
    for value in ["weather-lsp", "my_plugin", "v1.2.3", "a"] {
        assert!(
            validate_identifier(value, "plugin id").is_ok(),
            "'{value}' should be a valid identifier"
        );
    }
}

#[test]
fn test_guard_validate_identifier_rejects_traversal_and_absolute_values() {
    for value in ["../x", "..", ".", "/tmp/x", "a/b", "a\\b", "with space", ""] {
        assert!(
            validate_identifier(value, "plugin id").is_err(),
            "'{value}' must be refused as an identifier"
        );
    }
}

#[test]
fn test_guard_validate_identifier_bounds_the_length() {
    let long = "a".repeat(MAX_IDENTIFIER_LEN + 1);
    assert!(validate_identifier(&long, "plugin id").is_err());

    let at_limit = "a".repeat(MAX_IDENTIFIER_LEN);
    assert!(validate_identifier(&at_limit, "plugin id").is_ok());
}

// ---------------------------------------------------------------------------
// T-067: relative-component validation and contained joins
// ---------------------------------------------------------------------------

#[test]
fn test_guard_validate_relative_component_accepts_contained_paths() {
    for value in ["dist/index.js", "./src/main.rs", "a/b/c.txt"] {
        assert!(
            validate_relative_component(value, "entry").is_ok(),
            "'{value}' should be a contained relative path"
        );
    }
}

#[test]
fn test_guard_validate_relative_component_rejects_escapes() {
    for value in ["/etc/passwd", "../escape", "a/../../b", "a\\b", "", "C:/x"] {
        assert!(
            validate_relative_component(value, "entry").is_err(),
            "'{value}' must be refused as an escape"
        );
    }
}

#[test]
fn test_guard_contained_join_builds_a_path_under_the_root() {
    let root = Path::new("/srv/data");
    let joined = contained_join(root, "cases/01.json", "case file")
        .expect("a contained relative path must join");
    assert_eq!(joined, root.join("cases/01.json"));
}

#[test]
fn test_guard_contained_join_refuses_escapes() {
    let root = Path::new("/srv/data");
    assert!(contained_join(root, "../etc/passwd", "case file").is_err());
    assert!(contained_join(root, "/etc/passwd", "case file").is_err());
    assert!(contained_join(root, "ok/../../etc/passwd", "case file").is_err());
    assert!(contained_join(root, "..", "case file").is_err());
}

/// A symlink planted at the join target must not resolve outside the root, even
/// though the *lexical* path is already contained.
#[test]
fn test_guard_contained_join_detects_a_symlink_escape() {
    let base = std::env::temp_dir().join(format!("ragent-guard-{}", std::process::id()));
    let root = base.join("root");
    let outside = base.join("outside");
    std::fs::create_dir_all(&root).expect("create root");
    std::fs::create_dir_all(&outside).expect("create outside");
    std::fs::write(outside.join("secret.txt"), b"x").expect("seed outside file");

    #[cfg(unix)]
    {
        let link = root.join("link");
        std::os::unix::fs::symlink(&outside, &link).expect("create symlink");
        assert!(
            contained_join(&root, "link/secret.txt", "case file").is_err(),
            "a symlinked directory must not let a contained join escape the root"
        );
    }

    let _ = std::fs::remove_dir_all(&base);
}

// ---------------------------------------------------------------------------
// T-067: retry-after clamping
// ---------------------------------------------------------------------------

#[test]
fn test_guard_clamp_retry_after_bounds_both_ends() {
    assert_eq!(
        clamp_retry_after(Duration::from_secs(5)),
        Duration::from_secs(5)
    );
    assert_eq!(
        clamp_retry_after(Duration::from_secs(9999)),
        MAX_RETRY_AFTER
    );
    assert_eq!(clamp_retry_after(Duration::ZERO), Duration::from_secs(1));
}

// ---------------------------------------------------------------------------
// T-067: capped reads
// ---------------------------------------------------------------------------

#[test]
fn test_guard_cap_read_keeps_the_prefix_and_respects_the_budget() {
    assert_eq!(cap_read(b"hello world", 5), "hello");
    assert_eq!(cap_read(b"hello", 99), "hello");
    assert_eq!(cap_read(b"hello", 0), "");
}

/// A cut point inside a multibyte character must drop the split character and
/// must not panic (the pre-MS-05 byte-slice form could panic).
#[test]
fn test_guard_cap_read_tolerates_a_split_utf8_boundary() {
    let text = "café";
    let bytes = text.as_bytes();
    // Byte index 4 falls inside the two-byte 'é'.
    assert_eq!(cap_read(bytes, 4), "caf");

    let emoji = "a🦀b";
    let out = cap_read(emoji.as_bytes(), 3);
    assert_eq!(out, "a");
}
