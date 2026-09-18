//! Integration tests for the `/spec govcreate` content-reference classifier,
//! path-safety validator, and overwrite predicate (spec `govdoc` T-003,
//! FR-002, FR-006, FR-017, NFR-002, NFR-003).

use std::fs;
use std::path::Path;

use ragent_tools_extended::archdoc::{
    ContentRef, ContentRefError, classify_content_ref, is_forced_overwrite, validate_local_path,
};

// ---------------------------------------------------------------------------
// classify_content_ref
// ---------------------------------------------------------------------------

#[test]
fn test_classify_http_url() {
    let reference = classify_content_ref("http://docs.example.gov/arch").expect("http url");
    let ContentRef::Url(url) = reference else {
        panic!("expected a URL reference");
    };
    assert_eq!(url.scheme(), "http");
    assert_eq!(url.host_str(), Some("docs.example.gov"));
}

#[test]
fn test_classify_https_url() {
    let reference =
        classify_content_ref("https://docs.example.gov/architecture/payments").expect("https url");
    let ContentRef::Url(url) = reference else {
        panic!("expected a URL reference");
    };
    assert_eq!(url.scheme(), "https");
    assert_eq!(url.path(), "/architecture/payments");
}

#[test]
fn test_classify_relative_local_path() {
    let reference = classify_content_ref("./fixtures/datahub-docs").expect("local path");
    assert_eq!(
        reference,
        ContentRef::Local(Path::new("./fixtures/datahub-docs").to_path_buf())
    );
}

#[test]
fn test_classify_absolute_local_path() {
    let reference = classify_content_ref("/srv/arch/legacy-crm.pdf").expect("local path");
    assert_eq!(
        reference,
        ContentRef::Local(Path::new("/srv/arch/legacy-crm.pdf").to_path_buf())
    );
}

#[test]
fn test_classify_windows_drive_path_is_local() {
    // A single-letter scheme followed by a slash is a Windows path, not a URL.
    let reference = classify_content_ref(r"C:\docs\architecture").expect("local path");
    assert_eq!(
        reference,
        ContentRef::Local(Path::new(r"C:\docs\architecture").to_path_buf())
    );
}

#[test]
fn test_classify_rejects_ftp_scheme() {
    let error = classify_content_ref("ftp://files.example.gov/arch.zip").expect_err("reject ftp");
    assert_eq!(error, ContentRefError::UnsupportedScheme("ftp".to_string()));
}

#[test]
fn test_classify_accepts_mixed_case_https_scheme() {
    let reference = classify_content_ref("HTTPS://docs.example.gov/arch").expect("mixed-case url");
    let ContentRef::Url(url) = reference else {
        panic!("expected a URL reference");
    };
    assert_eq!(url.host_str(), Some("docs.example.gov"));
}

#[test]
fn test_classify_rejects_non_http_scheme_case_insensitively() {
    let error = classify_content_ref("FTP://files.example.gov/arch.zip").expect_err("reject ftp");
    assert!(matches!(error, ContentRefError::UnsupportedScheme(_)));
}

#[test]
fn test_classify_rejects_empty_reference() {
    assert_eq!(
        classify_content_ref("   ").expect_err("reject blank"),
        ContentRefError::Empty
    );
}

#[test]
fn test_classify_rejects_malformed_http_url() {
    // `http://` with no host cannot be parsed as an absolute URL.
    let error = classify_content_ref("http://").expect_err("reject malformed");
    assert!(matches!(error, ContentRefError::MalformedUrl(_)));
}

#[test]
fn test_classify_trims_surrounding_whitespace() {
    let reference = classify_content_ref("  https://docs.example.gov/arch  ").expect("trimmed url");
    let ContentRef::Url(url) = reference else {
        panic!("expected a URL reference");
    };
    assert_eq!(url.as_str(), "https://docs.example.gov/arch");
}

// ---------------------------------------------------------------------------
// validate_local_path - safety
// ---------------------------------------------------------------------------

#[test]
fn test_validate_local_path_accepts_in_tree_file() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::write(temp.path().join("sad.md"), "# Architecture\nA system.").expect("write fixture");

    let resolved = validate_local_path(Path::new("sad.md"), temp.path()).expect("valid path");
    assert_eq!(
        resolved,
        temp.path()
            .canonicalize()
            .expect("canon root")
            .join("sad.md")
    );
}

#[test]
fn test_validate_local_path_rejects_relative_escape() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("root");
    let outside = temp.path().join("outside");
    fs::create_dir_all(&root).expect("create root");
    fs::create_dir_all(&outside).expect("create outside");
    fs::write(outside.join("secret.md"), "outside the tree").expect("write outside file");

    let error = validate_local_path(Path::new("../outside"), &root).expect_err("reject escape");
    assert!(
        matches!(error, ContentRefError::PathEscape(_)),
        "got {error:?}"
    );
}

#[test]
fn test_validate_local_path_rejects_absolute_escape() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("root");
    let outside = temp.path().join("outside");
    fs::create_dir_all(&root).expect("create root");
    fs::create_dir_all(&outside).expect("create outside");
    fs::write(outside.join("secret.md"), "outside the tree").expect("write outside file");

    let error = validate_local_path(&outside, &root).expect_err("reject absolute escape");
    assert!(
        matches!(error, ContentRefError::PathEscape(_)),
        "got {error:?}"
    );
}

#[cfg(unix)]
#[test]
fn test_validate_local_path_rejects_symlink_escape() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("root");
    let outside = temp.path().join("outside");
    fs::create_dir_all(&root).expect("create root");
    fs::create_dir_all(&outside).expect("create outside");
    fs::write(outside.join("secret.md"), "outside the tree").expect("write outside file");
    // A symlink inside the tree pointing outside it must be refused, because
    // canonicalisation follows links.
    std::os::unix::fs::symlink(&outside, root.join("link")).expect("create symlink");

    let error = validate_local_path(Path::new("link"), &root).expect_err("reject symlink escape");
    assert!(
        matches!(error, ContentRefError::PathEscape(_)),
        "got {error:?}"
    );
}

// ---------------------------------------------------------------------------
// validate_local_path - readability causes (FR-006)
// ---------------------------------------------------------------------------

#[test]
fn test_validate_local_path_reports_path_not_found() {
    let temp = tempfile::tempdir().expect("tempdir");
    let error =
        validate_local_path(Path::new("missing-sad.pdf"), temp.path()).expect_err("not found");
    assert!(
        matches!(error, ContentRefError::PathNotFound(_)),
        "got {error:?}"
    );
}

#[test]
fn test_validate_local_path_reports_unsupported_format_for_file() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::write(temp.path().join("image.png"), "not a document").expect("write fixture");

    let error = validate_local_path(Path::new("image.png"), temp.path()).expect_err("unsupported");
    assert!(
        matches!(error, ContentRefError::UnsupportedFormat(_)),
        "got {error:?}"
    );
}

#[test]
fn test_validate_local_path_reports_empty_corpus_for_empty_file() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::write(temp.path().join("empty.md"), "").expect("write empty fixture");

    let error = validate_local_path(Path::new("empty.md"), temp.path()).expect_err("empty corpus");
    assert!(
        matches!(error, ContentRefError::EmptyCorpus(_)),
        "got {error:?}"
    );
}

#[test]
fn test_validate_local_path_accepts_directory_with_supported_nested_file() {
    let temp = tempfile::tempdir().expect("tempdir");
    let docs = temp.path().join("docs");
    fs::create_dir_all(docs.join("nested")).expect("create nested");
    fs::write(docs.join("nested/interfaces.md"), "interfaces").expect("write nested file");

    let resolved = validate_local_path(Path::new("docs"), temp.path()).expect("valid directory");
    assert_eq!(resolved, docs.canonicalize().expect("canon docs"));
}

#[test]
fn test_validate_local_path_reports_unsupported_format_for_directory() {
    let temp = tempfile::tempdir().expect("tempdir");
    let docs = temp.path().join("docs");
    fs::create_dir_all(&docs).expect("create docs");
    fs::write(docs.join("diagram.png"), "binary").expect("write unsupported file");

    let error = validate_local_path(Path::new("docs"), temp.path()).expect_err("unsupported dir");
    assert!(
        matches!(error, ContentRefError::UnsupportedFormat(_)),
        "got {error:?}"
    );
}

#[test]
fn test_validate_local_path_reports_empty_corpus_for_empty_directory() {
    let temp = tempfile::tempdir().expect("tempdir");
    let docs = temp.path().join("docs");
    fs::create_dir_all(&docs).expect("create empty docs");

    let error = validate_local_path(Path::new("docs"), temp.path()).expect_err("empty dir");
    assert!(
        matches!(error, ContentRefError::EmptyCorpus(_)),
        "got {error:?}"
    );
}

#[test]
fn test_validate_local_path_returns_canonical_path() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::write(temp.path().join("arch.md"), "architecture").expect("write fixture");

    let resolved =
        validate_local_path(Path::new("./arch.md"), temp.path()).expect("valid relative path");
    assert!(resolved.is_absolute(), "resolved path must be canonical");
    assert_eq!(
        resolved,
        temp.path()
            .canonicalize()
            .expect("canon root")
            .join("arch.md")
    );
}

// ---------------------------------------------------------------------------
// is_forced_overwrite (FR-017)
// ---------------------------------------------------------------------------

#[test]
fn test_is_forced_overwrite_true_when_exists_and_forced() {
    assert!(is_forced_overwrite(true, true));
}

#[test]
fn test_is_forced_overwrite_false_when_exists_without_force() {
    assert!(!is_forced_overwrite(true, false));
}

#[test]
fn test_is_forced_overwrite_false_when_missing() {
    assert!(!is_forced_overwrite(false, false));
    assert!(!is_forced_overwrite(false, true));
}
