//! Parse `@` references from user input text.
//!
//! Detects patterns like `@filename`, `@path/to/file`, `@path/to/dir/`,
//! and `@https://example.com` within prompt text and classifies each into
//! a [`FileRef`] variant.
//!
//! # Examples
//!
//! ```
//! use ragent_core::reference::parse::parse_refs;
//!
//! let refs = parse_refs("Look at @src/main.rs and @lib/");
//! assert_eq!(refs.len(), 2);
//! assert_eq!(refs[0].raw, "src/main.rs");
//! assert_eq!(refs[1].raw, "lib/");
//! ```

use std::ops::Range;
use std::path::PathBuf;

/// Classification of an `@` reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileRef {
    /// A file path (absolute or relative).
    File(PathBuf),
    /// A directory path (ends with `/`).
    Directory(PathBuf),
    /// A URL (starts with `http://` or `https://`).
    Url(String),
    /// A bare name to be fuzzy-matched against project files.
    Fuzzy(String),
}

/// A parsed `@` reference with its position in the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRef {
    /// The raw text after `@` (without the `@` prefix).
    pub raw: String,
    /// The classification of this reference.
    pub kind: FileRef,
    /// Byte range in the original input string covering `@raw`.
    pub span: Range<usize>,
}

/// Detect and classify all `@` references in the input text.
///
/// Rules:
/// - `@` followed by non-whitespace characters forms a reference
/// - `@` preceded by an alphanumeric char is NOT a reference (e.g. `user@example.com`)
/// - References ending with `/` are classified as directories
/// - References starting with `http://` or `https://` are URLs
/// - References containing `/` or `.` are treated as file paths
/// - Everything else is a fuzzy name
///
/// # Examples
///
/// ```
/// use ragent_core::reference::parse::{parse_refs, FileRef};
///
/// let refs = parse_refs("Check @src/main.rs please");
/// assert_eq!(refs.len(), 1);
/// assert!(matches!(refs[0].kind, FileRef::File(_)));
/// ```
#[must_use]
pub fn parse_refs(input: &str) -> Vec<ParsedRef> {
    let mut refs = Vec::new();
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        if bytes[i] == b'@' {
            // Skip if preceded by an alphanumeric char (e.g. email)
            if i > 0 && (bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'.') {
                i += 1;
                continue;
            }

            let start = i; // position of '@'
            i += 1; // skip the '@'

            // Collect non-whitespace chars after '@'
            let ref_start = i;
            while i < len && !bytes[i].is_ascii_whitespace() {
                i += 1;
            }

            if i > ref_start {
                let raw = &input[ref_start..i];

                // Skip if the raw text is empty or starts with a non-path char
                if raw.is_empty() {
                    continue;
                }

                let kind = classify_ref(raw);
                refs.push(ParsedRef {
                    raw: raw.to_string(),
                    kind,
                    span: start..i,
                });
            }
        } else {
            i += 1;
        }
    }

    refs
}

/// Classify a raw reference string into a [`FileRef`] variant.
fn classify_ref(raw: &str) -> FileRef {
    // URLs
    if raw.starts_with("http://") || raw.starts_with("https://") {
        return FileRef::Url(raw.to_string());
    }

    // Directories (trailing slash)
    if raw.ends_with('/') {
        let path = raw.trim_end_matches('/');
        return FileRef::Directory(PathBuf::from(path));
    }

    // File paths (contain path separators or look like relative/absolute paths)
    if raw.contains('/') || raw.starts_with('.') || raw.starts_with('~') {
        return FileRef::File(PathBuf::from(raw));
    }

    // If it has a file extension, treat as a file path
    if raw.contains('.') && !raw.starts_with('.') {
        let parts: Vec<&str> = raw.rsplitn(2, '.').collect();
        if parts.len() == 2 && !parts[0].is_empty() && parts[0].len() <= 10 {
            return FileRef::File(PathBuf::from(raw));
        }
    }

    // Everything else is a fuzzy name
    FileRef::Fuzzy(raw.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_file() {
        let refs = parse_refs("Look at @src/main.rs");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].raw, "src/main.rs");
        assert!(matches!(refs[0].kind, FileRef::File(_)));
        assert_eq!(refs[0].span, 8..20);
    }

    #[test]
    fn test_parse_directory() {
        let refs = parse_refs("Check @src/");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].raw, "src/");
        assert!(matches!(refs[0].kind, FileRef::Directory(_)));
    }

    #[test]
    fn test_parse_url() {
        let refs = parse_refs("See @https://example.com/docs");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].raw, "https://example.com/docs");
        assert!(matches!(refs[0].kind, FileRef::Url(_)));
    }

    #[test]
    fn test_parse_fuzzy_name() {
        let refs = parse_refs("Fix @Cargo");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].raw, "Cargo");
        assert!(matches!(refs[0].kind, FileRef::Fuzzy(_)));
    }

    #[test]
    fn test_parse_file_with_extension() {
        let refs = parse_refs("Edit @main.rs");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].raw, "main.rs");
        assert!(matches!(refs[0].kind, FileRef::File(_)));
    }

    #[test]
    fn test_skip_email_addresses() {
        let refs = parse_refs("Contact user@example.com for help");
        assert!(
            refs.is_empty(),
            "email addresses should not be parsed as refs"
        );
    }

    #[test]
    fn test_skip_email_with_dot_prefix() {
        let refs = parse_refs("Email dev.team@corp.com");
        assert!(refs.is_empty());
    }

    #[test]
    fn test_multiple_refs() {
        let refs = parse_refs("Compare @src/main.rs with @src/lib.rs");
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].raw, "src/main.rs");
        assert_eq!(refs[1].raw, "src/lib.rs");
    }

    #[test]
    fn test_ref_at_start() {
        let refs = parse_refs("@README.md is important");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].raw, "README.md");
    }

    #[test]
    fn test_ref_at_end() {
        let refs = parse_refs("Check @Cargo.toml");
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].raw, "Cargo.toml");
    }

    #[test]
    fn test_no_refs() {
        let refs = parse_refs("Just plain text");
        assert!(refs.is_empty());
    }

    #[test]
    fn test_bare_at_sign() {
        let refs = parse_refs("@ alone");
        assert!(refs.is_empty(), "bare @ with space should produce no ref");
    }

    #[test]
    fn test_at_end_of_string() {
        let refs = parse_refs("trailing @");
        assert!(refs.is_empty());
    }

    #[test]
    fn test_adjacent_refs() {
        let refs = parse_refs("@src/a.rs @src/b.rs");
        assert_eq!(refs.len(), 2);
    }

    #[test]
    fn test_relative_path() {
        let refs = parse_refs("Edit @./local/file.txt");
        assert_eq!(refs.len(), 1);
        assert!(matches!(refs[0].kind, FileRef::File(_)));
    }

    #[test]
    fn test_span_correctness() {
        let input = "Read @file.txt now";
        let refs = parse_refs(input);
        assert_eq!(refs.len(), 1);
        assert_eq!(&input[refs[0].span.clone()], "@file.txt");
    }

    #[test]
    fn test_url_http() {
        let refs = parse_refs("See @http://localhost:3000/api");
        assert_eq!(refs.len(), 1);
        assert!(matches!(refs[0].kind, FileRef::Url(_)));
    }

    #[test]
    fn test_dotfile() {
        let refs = parse_refs("Check @.gitignore");
        assert_eq!(refs.len(), 1);
        assert!(matches!(refs[0].kind, FileRef::File(_)));
    }
}
