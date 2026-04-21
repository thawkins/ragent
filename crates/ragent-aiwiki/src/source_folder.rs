//! Source folder management for AIWiki referenced folders.
//!
//! This module defines the `SourceFolder` struct and related functionality
//! for managing referenced folders that are scanned in-place during sync
//! without copying content into `aiwiki/raw/`.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// A referenced folder source for AIWiki.
///
/// Source folders are scanned in-place during sync, without copying
/// content into `aiwiki/raw/`. This is useful for project directories
/// like `docs/`, `src/`, or `examples/` that already exist and should
/// not be duplicated.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceFolder {
    /// Relative folder path from project root (e.g., "docs", "src", "crates/aiwiki/src").
    pub path: String,

    /// Human-readable label (e.g., "Documentation").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,

    /// Glob patterns for file matching (default: `["**/*"]`).
    /// Automatically populated from file spec shorthand.
    #[serde(default = "default_patterns")]
    pub patterns: Vec<String>,

    /// Whether to include subdirectories (always `true` by default).
    #[serde(default = "default_true")]
    pub recursive: bool,

    /// Whether this source is enabled for scanning.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Whether this source represents a single file instead of a directory.
    #[serde(default)]
    pub is_file: bool,
}

impl Default for SourceFolder {
    fn default() -> Self {
        Self {
            path: String::new(),
            label: None,
            patterns: default_patterns(),
            recursive: true,
            enabled: true,
            is_file: false,
        }
    }
}

impl SourceFolder {
    /// Create a new SourceFolder with the given path.
    ///
    /// Uses defaults for all other fields:
    /// - label: None
    /// - patterns: ["**/*"]
    /// - recursive: true
    /// - enabled: true
    ///
    /// # Example
    ///
    /// ```
    /// use ragent_aiwiki::source_folder::SourceFolder;
    ///
    /// let source = SourceFolder::new("docs");
    /// assert_eq!(source.path, "docs");
    /// assert_eq!(source.patterns, vec!["**/*"]);
    /// assert!(source.recursive);
    /// assert!(source.enabled);
    /// ```
    pub fn new(path: &str) -> Self {
        Self {
            path: path.to_string(),
            label: None,
            patterns: default_patterns(),
            recursive: true,
            enabled: true,
            is_file: false,
        }
    }

    /// Create a new SourceFolder for a single file.
    ///
    /// # Example
    ///
    /// ```
    /// use ragent_aiwiki::source_folder::SourceFolder;
    ///
    /// let source = SourceFolder::from_file_path("README.md");
    /// assert_eq!(source.path, "README.md");
    /// assert!(source.is_file);
    /// assert!(source.enabled);
    /// ```
    pub fn from_file_path(path: &str) -> Self {
        Self {
            path: path.to_string(),
            label: None,
            patterns: vec![path.to_string()],
            recursive: false,
            enabled: true,
            is_file: true,
        }
    }

    /// Parse a source folder from a shorthand specification.
    ///
    /// Supports:
    /// - `"docs"` → path: `"docs"`, patterns: `["**/*"]`
    /// - `"src/*.rs"` → path: `"src"`, patterns: `["**/*.rs"]`
    /// - `"tests/**/*.rs"` → path: `"tests"`, patterns: `["**/*.rs"]`
    ///
    /// # Parsing Rules
    ///
    /// 1. If the path contains a glob character (`*`, `?`, `{`, `[`), split at the
    ///    last `/` before the first glob → folder + file pattern
    /// 2. If no glob characters, the entire string is the folder path, file
    ///    pattern defaults to `**/*`
    /// 3. File specs like `*.ext` are automatically promoted to `**/*.ext` to
    ///    ensure recursive matching
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The folder portion is empty (e.g., `"*.rs"`)
    /// - The path is invalid
    ///
    /// # Example
    ///
    /// ```
    /// use ragent_aiwiki::source_folder::SourceFolder;
    ///
    /// // Simple folder path
    /// let source = SourceFolder::from_spec("docs").unwrap();
    /// assert_eq!(source.path, "docs");
    /// assert_eq!(source.patterns, vec!["**/*"]);
    ///
    /// // Path with file spec
    /// let source = SourceFolder::from_spec("src/*.rs").unwrap();
    /// assert_eq!(source.path, "src");
    /// assert_eq!(source.patterns, vec!["**/*.rs"]);
    ///
    /// // Explicit recursive glob
    /// let source = SourceFolder::from_spec("tests/**/*.rs").unwrap();
    /// assert_eq!(source.path, "tests");
    /// assert_eq!(source.patterns, vec!["**/*.rs"]);
    /// ```
    pub fn from_spec(spec: &str) -> crate::Result<Self> {
        let spec = spec.trim();

        if spec.is_empty() {
            return Err(crate::AiwikiError::Config(
                "Source folder spec cannot be empty".to_string(),
            ));
        }

        // Check for glob characters
        let glob_chars = ['*', '?', '{', '['];

        if let Some(first_glob_pos) = spec.find(&glob_chars[..]) {
            // Find the last '/' before the first glob character
            let folder_end = spec[..first_glob_pos].rfind('/').map_or(0, |p| p + 1);

            let folder = spec[..folder_end].trim_end_matches('/');
            let pattern = &spec[folder_end..];

            if folder.is_empty() {
                return Err(crate::AiwikiError::Config(
                    "Source folder spec must have a folder path before the file pattern"
                        .to_string(),
                ));
            }

            // Promote *.ext to **/*.ext for recursive matching
            let pattern = if pattern.starts_with("*.") && !pattern.contains('/') {
                format!("**/{}", pattern)
            } else {
                pattern.to_string()
            };

            Ok(Self {
                path: folder.to_string(),
                label: None,
                patterns: vec![pattern],
                recursive: true,
                enabled: true,
                is_file: false,
            })
        } else {
            // No glob characters, entire string is the folder path
            let folder = spec.trim_end_matches('/');
            Ok(Self {
                path: folder.to_string(),
                label: None,
                patterns: default_patterns(),
                recursive: true,
                enabled: true,
                is_file: false,
            })
        }
    }

    /// Validate that the source folder path is valid.
    ///
    /// Checks:
    /// - Path is not empty
    /// - Path is relative (does not start with `/`)
    /// - Path does not escape the project root (no `..` prefix that resolves outside root)
    /// - Path exists (as a directory for folders, as a file for file sources)
    ///
    /// # Errors
    ///
    /// Returns an error if any validation check fails.
    pub fn validate(&self, root: &Path) -> crate::Result<()> {
        // Check empty path
        if self.path.is_empty() {
            return Err(crate::AiwikiError::Config(
                "Source folder path cannot be empty".to_string(),
            ));
        }

        // Check for absolute path
        if Path::new(&self.path).is_absolute() {
            return Err(crate::AiwikiError::Config(format!(
                "Source folder path must be relative, not absolute: {}",
                self.path
            )));
        }

        // Check for path traversal that escapes project root
        let full_path = root.join(&self.path);
        let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let canonical_path = full_path
            .canonicalize()
            .unwrap_or_else(|_| full_path.clone());

        if !canonical_path.starts_with(&canonical_root) {
            return Err(crate::AiwikiError::Config(format!(
                "Source folder path escapes project root: {}",
                self.path
            )));
        }

        // Check that path exists
        if !full_path.exists() {
            return Err(crate::AiwikiError::Config(format!(
                "Source path does not exist: {}",
                self.path
            )));
        }

        // For file sources, the path must be a file
        // For directory sources, the path must be a directory
        if self.is_file {
            if !full_path.is_file() {
                return Err(crate::AiwikiError::Config(format!(
                    "Source file path is not a file: {}",
                    self.path
                )));
            }
        } else if !full_path.is_dir() {
            return Err(crate::AiwikiError::Config(format!(
                "Source folder path is not a directory: {}",
                self.path
            )));
        }

        Ok(())
    }

    /// Get the label or fall back to the path.
    pub fn label_or_path(&self) -> &str {
        self.label.as_deref().unwrap_or(&self.path)
    }

    /// Check if a file path matches this source's patterns.
    ///
    /// The file path should be relative to the source folder.
    /// For single file sources, the path is matched directly against the file name.
    ///
    /// # Example
    ///
    /// ```
    /// use ragent_aiwiki::source_folder::SourceFolder;
    ///
    /// let source = SourceFolder::from_spec("src/*.rs").unwrap();
    /// assert!(source.matches("main.rs"));
    /// assert!(source.matches("lib/mod.rs"));
    /// assert!(!source.matches("README.md"));
    /// ```
    pub fn matches(&self, file_path: &str) -> bool {
        // For single file sources, check if the path matches directly
        if self.is_file {
            // file_path will be the same as self.path for single file sources
            return file_path == self.path
                || Path::new(file_path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n == self.path || self.path.ends_with(n))
                    .unwrap_or(false);
        }

        // For directory sources, use pattern matching
        for pattern in &self.patterns {
            if Self::glob_match(pattern, file_path) {
                return true;
            }
        }
        false
    }

    /// Simple glob matching implementation.
    ///
    /// Supports `*` (matches any characters) and `**` (matches any path components).
    fn glob_match(pattern: &str, path: &str) -> bool {
        let pattern = pattern.trim_start_matches("./");
        let path = path.trim_start_matches("./");

        // Handle **/ prefix for recursive matching
        if let Some(rest) = pattern.strip_prefix("**/") {
            // **/foo matches foo, bar/foo, a/b/foo, etc.
            if let Some(pos) = path.rfind('/') {
                // Check if the suffix matches
                let suffix = &path[pos + 1..];
                return Self::simple_glob_match(rest, suffix)
                    || Self::glob_match(pattern, &path[..pos]);
            } else {
                // No directory components, just check the whole string
                return Self::simple_glob_match(rest, path);
            }
        }

        // Handle **/ suffix or middle
        if pattern.contains("/**/") {
            let parts: Vec<&str> = pattern.split("/**/").collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];

                if !path.starts_with(prefix) {
                    return false;
                }
                let rest = &path[prefix.len()..];

                // The suffix can appear anywhere after the prefix
                return rest.contains(&format!("/{}", suffix.trim_start_matches('/')))
                    || rest.trim_start_matches('/') == suffix.trim_start_matches('/');
            }
        }

        Self::simple_glob_match(pattern, path)
    }

    /// Simple glob matching without ** (double asterisk).
    fn simple_glob_match(pattern: &str, path: &str) -> bool {
        // Split both by /
        let pattern_parts: Vec<&str> = pattern.split('/').collect();
        let path_parts: Vec<&str> = path.split('/').collect();

        // For simple patterns like "*.rs", allow matching at any depth
        if pattern_parts.len() == 1 && pattern_parts[0].contains('*') {
            let pattern_part = pattern_parts[0];
            return path_parts.iter().any(|p| Self::match_part(pattern_part, p));
        }

        if pattern_parts.len() != path_parts.len() {
            return false;
        }

        pattern_parts
            .iter()
            .zip(path_parts.iter())
            .all(|(p, s)| Self::match_part(p, s))
    }

    /// Match a single path component with glob patterns.
    fn match_part(pattern: &str, s: &str) -> bool {
        if pattern == "*" {
            return true;
        }
        if pattern == s {
            return true;
        }

        // Handle * suffix (e.g., "*.rs")
        if let Some(prefix) = pattern.strip_suffix('*') {
            return s.starts_with(prefix);
        }

        // Handle * prefix (e.g., "test_*")
        if let Some(suffix) = pattern.strip_prefix('*') {
            return s.ends_with(suffix);
        }

        // Handle pattern with * in the middle (e.g., "a*b")
        if let Some(pos) = pattern.find('*') {
            let prefix = &pattern[..pos];
            let suffix = &pattern[pos + 1..];
            return s.starts_with(prefix) && s.ends_with(suffix) && s.len() >= pattern.len() - 1;
        }

        false
    }
}

fn default_patterns() -> Vec<String> {
    vec!["**/*".to_string()]
}

const fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_folder_new_defaults() {
        let source = SourceFolder::new("docs");
        assert_eq!(source.path, "docs");
        assert_eq!(source.label, None);
        assert_eq!(source.patterns, vec!["**/*"]);
        assert!(source.recursive);
        assert!(source.enabled);
    }

    #[test]
    fn test_from_spec_simple_folder() {
        let source = SourceFolder::from_spec("docs").unwrap();
        assert_eq!(source.path, "docs");
        assert_eq!(source.patterns, vec!["**/*"]);
    }

    #[test]
    fn test_from_spec_folder_with_trailing_slash() {
        let source = SourceFolder::from_spec("docs/").unwrap();
        assert_eq!(source.path, "docs");
        assert_eq!(source.patterns, vec!["**/*"]);
    }

    #[test]
    fn test_from_spec_with_glob_pattern() {
        let source = SourceFolder::from_spec("src/*.rs").unwrap();
        assert_eq!(source.path, "src");
        assert_eq!(source.patterns, vec!["**/*.rs"]);
    }

    #[test]
    fn test_from_spec_with_explicit_recursive_glob() {
        let source = SourceFolder::from_spec("tests/**/*.rs").unwrap();
        assert_eq!(source.path, "tests");
        assert_eq!(source.patterns, vec!["**/*.rs"]);
    }

    #[test]
    fn test_from_spec_nested_path_with_pattern() {
        let source = SourceFolder::from_spec("crates/aiwiki/src/*.rs").unwrap();
        assert_eq!(source.path, "crates/aiwiki/src");
        assert_eq!(source.patterns, vec!["**/*.rs"]);
    }

    #[test]
    fn test_from_spec_nested_path_only() {
        let source = SourceFolder::from_spec("crates/aiwiki/src").unwrap();
        assert_eq!(source.path, "crates/aiwiki/src");
        assert_eq!(source.patterns, vec!["**/*"]);
    }

    #[test]
    fn test_from_spec_empty_fails() {
        let result = SourceFolder::from_spec("");
        assert!(result.is_err());
    }

    #[test]
    fn test_from_spec_glob_only_fails() {
        let result = SourceFolder::from_spec("*.rs");
        assert!(result.is_err());
    }

    #[test]
    fn test_glob_match_basic() {
        assert!(SourceFolder::glob_match("*.rs", "main.rs"));
        assert!(SourceFolder::glob_match("*.rs", "lib.rs"));
        assert!(!SourceFolder::glob_match("*.rs", "main.txt"));
    }

    #[test]
    fn test_glob_match_recursive() {
        assert!(SourceFolder::glob_match("**/*.rs", "main.rs"));
        assert!(SourceFolder::glob_match("**/*.rs", "src/main.rs"));
        assert!(SourceFolder::glob_match("**/*.rs", "src/lib/mod.rs"));
        assert!(!SourceFolder::glob_match("**/*.rs", "README.md"));
    }

    #[test]
    fn test_glob_match_prefix() {
        assert!(SourceFolder::glob_match("test_*.rs", "test_main.rs"));
        assert!(!SourceFolder::glob_match("test_*.rs", "main.rs"));
    }

    #[test]
    fn test_matches() {
        let source = SourceFolder::from_spec("src/*.rs").unwrap();
        assert!(source.matches("main.rs"));
        assert!(source.matches("lib/mod.rs"));
        assert!(!source.matches("README.md"));
    }

    #[test]
    fn test_label_or_path() {
        let mut source = SourceFolder::new("docs");
        assert_eq!(source.label_or_path(), "docs");

        source.label = Some("Documentation".to_string());
        assert_eq!(source.label_or_path(), "Documentation");
    }

    #[test]
    fn test_default() {
        let source: SourceFolder = Default::default();
        assert!(source.path.is_empty());
        assert_eq!(source.patterns, vec!["**/*"]);
        assert!(source.recursive);
        assert!(source.enabled);
        assert!(!source.is_file);
    }

    #[test]
    fn test_from_file_path() {
        let source = SourceFolder::from_file_path("README.md");
        assert_eq!(source.path, "README.md");
        assert_eq!(source.patterns, vec!["README.md"]);
        assert!(source.is_file);
        assert!(!source.recursive);
        assert!(source.enabled);
        assert_eq!(source.label, None);
    }
}
