//! Content-reference classification, local-path safety validation, and the
//! FR-017 overwrite predicate for `/spec govcreate` (spec `govdoc` T-003).
//!
//! Three network-free entry points keep the front end of a govcreate run
//! testable in isolation (NFR-002):
//!
//! - [`classify_content_ref`] decides whether a raw reference is an
//!   `http`/`https` URL or a local path, rejecting any other URL scheme
//!   (NFR-003).
//! - [`validate_local_path`] resolves a local reference against the invoking
//!   directory, refuses one that escapes that tree after canonicalisation (so
//!   symlinks cannot smuggle it out, NFR-003), and reports the FR-006
//!   readability cause - path not found, permission denied, unsupported
//!   format, or empty corpus - before any scaffold write.
//! - [`is_forced_overwrite`] is the pure FR-017 overwrite decision.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use url::Url;

use crate::document_extract::detect_document_format;

/// A content reference classified from its raw string form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentRef {
    /// An `http://` or `https://` URL reference.
    Url(Url),
    /// A local file or directory reference (not yet validated).
    Local(PathBuf),
}

/// The specific cause of a classification or local-path validation failure.
///
/// The local-path variants mirror the four FR-006 causes so the runner can
/// report the precise reason a content reference was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentRefError {
    /// The reference was empty or whitespace only.
    Empty,
    /// A URL scheme other than `http`/`https` (NFR-003).
    UnsupportedScheme(String),
    /// A URL-shaped reference that could not be parsed.
    MalformedUrl(String),
    /// The local path resolves outside the invoking directory tree (NFR-003).
    PathEscape(PathBuf),
    /// The local path does not exist (FR-006: path not found).
    PathNotFound(PathBuf),
    /// The local path cannot be read (FR-006: permission denied).
    PermissionDenied(PathBuf),
    /// The local reference has an unsupported document format (FR-006).
    UnsupportedFormat(PathBuf),
    /// The local reference yields no usable text (FR-006: empty corpus).
    EmptyCorpus(PathBuf),
    /// An unexpected I/O error while inspecting the local path.
    Io {
        /// The path being inspected when the error occurred.
        path: PathBuf,
        /// The underlying OS error message.
        detail: String,
    },
}

impl std::fmt::Display for ContentRefError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "content reference is empty"),
            Self::UnsupportedScheme(scheme) => write!(
                f,
                "unsupported content-reference scheme '{scheme}' (only http:// and https:// are supported)"
            ),
            Self::MalformedUrl(detail) => {
                write!(f, "malformed URL content reference: {detail}")
            }
            Self::PathEscape(path) => write!(
                f,
                "content reference escapes the invoking directory tree: {}",
                path.display()
            ),
            Self::PathNotFound(path) => write!(f, "path not found: {}", path.display()),
            Self::PermissionDenied(path) => write!(f, "permission denied: {}", path.display()),
            Self::UnsupportedFormat(path) => write!(f, "unsupported format: {}", path.display()),
            Self::EmptyCorpus(path) => {
                write!(f, "empty corpus: {} yields no usable text", path.display())
            }
            Self::Io { path, detail } => write!(f, "cannot read {}: {detail}", path.display()),
        }
    }
}

impl std::error::Error for ContentRefError {}

/// Classify a raw content reference as a URL or a local path (FR-002, NFR-003).
///
/// Purely lexical: `http://` and `https://` prefixes become [`ContentRef::Url`],
/// a Windows drive-letter prefix (`C:\`) stays a local path, and any other
/// reference carrying a URL scheme is rejected as
/// [`ContentRefError::UnsupportedScheme`].
///
/// # Errors
///
/// [`ContentRefError::Empty`] when the reference is blank,
/// [`ContentRefError::UnsupportedScheme`] for a non-HTTP(S) scheme, and
/// [`ContentRefError::MalformedUrl`] when an HTTP(S) reference cannot be parsed.
pub fn classify_content_ref(input: &str) -> Result<ContentRef, ContentRefError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ContentRefError::Empty);
    }
    if let Some(scheme) = url_scheme(trimmed) {
        if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") {
            let url = Url::parse(trimmed)
                .map_err(|err| ContentRefError::MalformedUrl(err.to_string()))?;
            return Ok(ContentRef::Url(url));
        }
        return Err(ContentRefError::UnsupportedScheme(scheme.to_string()));
    }
    Ok(ContentRef::Local(PathBuf::from(trimmed)))
}

/// Return the leading URL scheme of `input`, if it has one.
///
/// A scheme is `[A-Za-z][A-Za-z0-9+.-]*` followed by `:`. The one-letter form
/// followed by `/` or `\` is treated as a Windows drive path (`C:\docs`), not a
/// scheme, so a Windows path is never misclassified as a URL.
fn url_scheme(input: &str) -> Option<&str> {
    let colon = input.find(':')?;
    let scheme = &input[..colon];
    let mut chars = scheme.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        return None;
    }
    if scheme.len() == 1 && matches!(input.as_bytes().get(colon + 1).copied(), Some(b'/' | b'\\')) {
        return None;
    }
    Some(scheme)
}

/// Validate a local content reference for safety and readability (FR-006, NFR-003).
///
/// Resolves `path` against `invoking_root` (when relative), canonicalises it,
/// and refuses any resolution that escapes `invoking_root` - including via a
/// symlink, because canonicalisation follows links. It then reports the
/// specific FR-006 readability cause before any scaffold write. On success it
/// returns the canonical, in-tree path so callers read exactly what was
/// validated.
///
/// # Errors
///
/// [`ContentRefError::PathEscape`], [`ContentRefError::PathNotFound`],
/// [`ContentRefError::PermissionDenied`], [`ContentRefError::UnsupportedFormat`],
/// [`ContentRefError::EmptyCorpus`], or [`ContentRefError::Io`].
pub fn validate_local_path(path: &Path, invoking_root: &Path) -> Result<PathBuf, ContentRefError> {
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        invoking_root.join(path)
    };

    let root = invoking_root
        .canonicalize()
        .map_err(|err| ContentRefError::Io {
            path: invoking_root.to_path_buf(),
            detail: err.to_string(),
        })?;

    let canonical = candidate.canonicalize().map_err(|err| map_io(path, err))?;

    if !canonical.starts_with(&root) {
        return Err(ContentRefError::PathEscape(canonical));
    }

    let metadata = std::fs::metadata(&canonical).map_err(|err| map_io(path, err))?;
    if metadata.is_dir() {
        validate_directory(&canonical)?;
    } else if metadata.is_file() {
        validate_file(&canonical)?;
    } else {
        // Neither a regular file nor a directory (socket, fifo, device).
        return Err(ContentRefError::UnsupportedFormat(path.to_path_buf()));
    }

    Ok(canonical)
}

/// Pure FR-017 decision: whether an existing spec will be overwritten.
///
/// True only when a spec directory already exists *and* `--force` was given. A
/// missing directory is always writable (so this is `false`, not a refusal),
/// and an existing directory without `--force` is refused; the caller
/// distinguishes those two `false` cases with the existence flag it already
/// holds. Kept filesystem- and network-free so it is unit-testable (NFR-002).
#[must_use]
pub const fn is_forced_overwrite(spec_dir_exists: bool, force: bool) -> bool {
    spec_dir_exists && force
}

/// Validate a single-file reference: supported format, readable, non-empty.
fn validate_file(file: &Path) -> Result<(), ContentRefError> {
    if detect_document_format(file).is_err() {
        return Err(ContentRefError::UnsupportedFormat(file.to_path_buf()));
    }
    // Opening catches a read-permission failure that `metadata` would miss.
    std::fs::File::open(file).map_err(|err| map_io(file, err))?;
    let metadata = std::fs::metadata(file).map_err(|err| map_io(file, err))?;
    if metadata.len() == 0 {
        return Err(ContentRefError::EmptyCorpus(file.to_path_buf()));
    }
    Ok(())
}

/// Validate a directory reference: readable and holding at least one supported
/// document anywhere in its tree.
fn validate_directory(dir: &Path) -> Result<(), ContentRefError> {
    let (saw_file, saw_supported) = scan_for_supported(dir)?;
    if saw_supported {
        Ok(())
    } else if saw_file {
        Err(ContentRefError::UnsupportedFormat(dir.to_path_buf()))
    } else {
        Err(ContentRefError::EmptyCorpus(dir.to_path_buf()))
    }
}

/// Recursively search `dir` for a supported document.
///
/// Returns `(saw_any_file, saw_supported_file)` and stops at the first
/// supported file. Symlinked entries are not followed (`DirEntry::file_type`
/// does not resolve links), so an in-tree symlink can neither escape the tree
/// nor create a scan loop.
fn scan_for_supported(dir: &Path) -> Result<(bool, bool), ContentRefError> {
    let mut saw_file = false;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = std::fs::read_dir(&current).map_err(|err| map_io(&current, err))?;
        for entry in entries {
            let entry = entry.map_err(|err| map_io(&current, err))?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|err| map_io(&path, err))?;
            if file_type.is_dir() {
                stack.push(path);
            } else if file_type.is_file() {
                saw_file = true;
                if detect_document_format(&path).is_ok() {
                    return Ok((true, true));
                }
            }
        }
    }
    Ok((saw_file, false))
}

/// Map an [`std::io::Error`] to the closest [`ContentRefError`] cause.
fn map_io(path: &Path, err: std::io::Error) -> ContentRefError {
    match err.kind() {
        ErrorKind::NotFound => ContentRefError::PathNotFound(path.to_path_buf()),
        ErrorKind::PermissionDenied => ContentRefError::PermissionDenied(path.to_path_buf()),
        _ => ContentRefError::Io {
            path: path.to_path_buf(),
            detail: err.to_string(),
        },
    }
}
