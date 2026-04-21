//! Source helper utilities for the sync module.
//!
//! Provides functions for scanning source folders, building state keys,
//! and resolving file paths for both single-file and directory sources.

use crate::{Result, SourceFolder};
use std::path::{Path, PathBuf};
use tokio::fs;

/// Scan a source folder (or single file) and return a list of absolute file paths.
///
/// For single-file sources the returned list contains just that one file.
/// For directory sources the folder is walked recursively, filtering by
/// the source's glob patterns and the provided ignore patterns.
///
/// # Arguments
///
/// * `root` - Project root directory.
/// * `source` - The source folder configuration.
/// * `ignore_patterns` - Patterns for files to skip.
///
/// # Returns
///
/// A list of absolute `PathBuf`s for every matching file.
pub async fn scan_source_folder(
    root: &Path,
    source: &SourceFolder,
    ignore_patterns: &[impl AsRef<str>],
) -> Result<Vec<PathBuf>> {
    let source_path = root.join(&source.path);

    if source.is_file || source_path.is_file() {
        if source_path.exists() {
            // Check ignore patterns for single files
            let file_name = source_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if ignore_patterns
                .iter()
                .any(|p| pattern_matches(p.as_ref(), file_name))
            {
                return Ok(Vec::new());
            }
            return Ok(vec![source_path]);
        }
        return Err(crate::AiwikiError::Config(format!(
            "File does not exist: {}",
            source_path.display()
        )));
    }

    if !source_path.exists() {
        return Err(crate::AiwikiError::Config(format!(
            "Source folder does not exist: {}",
            source_path.display()
        )));
    }

    if !source_path.is_dir() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();
    scan_dir_recursive(&source_path, source, ignore_patterns, &mut results).await?;
    Ok(results)
}

/// Check if a file name matches a glob pattern.
fn pattern_matches(pattern: &str, file_name: &str) -> bool {
    // Simple glob matching for ignore patterns
    if pattern == file_name {
        return true;
    }
    if pattern.starts_with("*.") {
        let ext = &pattern[1..]; // Get the extension including the dot
        if let Some(file_ext) = file_name.rfind('.') {
            return file_name[file_ext..] == *ext;
        }
    }
    if pattern.contains('*') {
        // Simple wildcard matching
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.len() == 2 {
            let prefix = parts[0];
            let suffix = parts[1];
            return file_name.starts_with(prefix) && file_name.ends_with(suffix);
        }
    }
    false
}

/// Recursively walk a directory collecting files that match the source patterns.
async fn scan_dir_recursive(
    dir: &Path,
    source: &SourceFolder,
    ignore_patterns: &[impl AsRef<str>],
    out: &mut Vec<PathBuf>,
) -> Result<()> {
    let mut entries = fs::read_dir(dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Skip hidden files/dirs and common non-source directories
        if file_name.starts_with('.') {
            continue;
        }

        // Check ignore patterns
        if ignore_patterns
            .iter()
            .any(|p| pattern_matches(p.as_ref(), file_name))
        {
            continue;
        }

        if path.is_dir() {
            if source.recursive {
                Box::pin(scan_dir_recursive(&path, source, ignore_patterns, out)).await?;
            }
        } else if path.is_file() {
            // Check if file matches source patterns using the source's matcher
            let relative_path = path
                .strip_prefix(dir)
                .map_err(|e| crate::AiwikiError::State(e.to_string()))?;
            let relative_str = relative_path.to_string_lossy().to_string();

            // Use source.matches() which has proper glob matching with ** support
            if source.matches(&relative_str) {
                out.push(path);
            }
        }
    }
    Ok(())
}

/// Count the total number of scannable files across all enabled sources.
///
/// # Arguments
///
/// * `root` - Project root directory.
/// * `sources` - Slice of source folder configs.
/// * `ignore_patterns` - Patterns for files to skip.
pub async fn count_source_files(
    root: &Path,
    sources: &[SourceFolder],
    ignore_patterns: &[impl AsRef<str>],
) -> Result<usize> {
    let mut total = 0;
    for source in sources.iter().filter(|s| s.enabled) {
        let files = scan_source_folder(root, source, ignore_patterns).await?;
        total += files.len();
    }
    Ok(total)
}

/// Build a state key for a referenced source file.
///
/// The key format is `ref:<source_path>/<relative_path>`.
/// For single-file sources pass an empty `relative` to get `ref:<source>/`.
///
/// # Arguments
///
/// * `source_path` - The source folder path (e.g. `"docs"` or `"SPEC.md"`).
/// * `relative` - Relative path of the file inside the source folder.
pub fn make_ref_key(source_path: &str, relative: &str) -> String {
    if relative.is_empty() {
        format!("ref:{}/", source_path)
    } else {
        format!("ref:{}/{}", source_path, relative)
    }
}

/// Parse a `ref:` state key back into `(source_path, file_path)`.
///
/// Returns `None` if the key does not start with `ref:`.
///
/// # Examples
///
/// ```
/// use ragent_aiwiki::sync::parse_ref_key;
///
/// let (src, file) = parse_ref_key("ref:docs/intro.md").unwrap();
/// assert_eq!(src, "docs");
/// assert_eq!(file, "intro.md");
/// ```
pub fn parse_ref_key(key: &str) -> Option<(String, String)> {
    let rest = key.strip_prefix("ref:")?;
    // Split on the first `/` — everything before is the source name,
    // everything after is the relative file path.
    if let Some(idx) = rest.find('/') {
        let source = rest[..idx].to_string();
        let file = rest[idx + 1..].to_string();
        Some((source, file))
    } else {
        // Malformed key — no slash after ref:
        None
    }
}

/// Resolve the absolute file path for a state key.
///
/// If `path` starts with `ref:` the file is resolved relative to the
/// project root; otherwise it is resolved relative to `raw_dir`.
///
/// # Arguments
///
/// * `root` - Project root directory.
/// * `raw_dir` - The wiki raw directory.
/// * `path` - A state key or relative path.
pub fn resolve_file_path(root: &Path, raw_dir: &Path, path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("ref:") {
        // For ref: keys, strip the "ref:" prefix and join with root.
        // rest is "source_path/relative" or "source_path/" for single files.
        if let Some(idx) = rest.find('/') {
            let source = &rest[..idx];
            let relative = &rest[idx + 1..];
            if relative.is_empty() {
                // Single file source — the source path IS the file
                root.join(source)
            } else {
                root.join(source).join(relative)
            }
        } else {
            root.join(rest)
        }
    } else {
        raw_dir.join(path)
    }
}

/// Extract the source name from a `ref:` state key.
///
/// Returns the source path portion (before the first `/`) or the full
/// path if it does not contain a `ref:` prefix.
///
/// # Arguments
///
/// * `path` - A state key string.
pub fn get_source_name(path: &str) -> &str {
    if let Some(rest) = path.strip_prefix("ref:") {
        if let Some(idx) = rest.find('/') {
            &rest[..idx]
        } else {
            rest
        }
    } else {
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_ref_key_directory() {
        assert_eq!(make_ref_key("docs", "intro.md"), "ref:docs/intro.md");
    }

    #[test]
    fn test_make_ref_key_single_file() {
        assert_eq!(make_ref_key("SPEC.md", ""), "ref:SPEC.md/");
    }

    #[test]
    fn test_parse_ref_key_directory() {
        let (src, file) = parse_ref_key("ref:docs/intro.md").unwrap();
        assert_eq!(src, "docs");
        assert_eq!(file, "intro.md");
    }

    #[test]
    fn test_parse_ref_key_single_file() {
        let (src, file) = parse_ref_key("ref:SPEC.md/").unwrap();
        assert_eq!(src, "SPEC.md");
        assert_eq!(file, "");
    }

    #[test]
    fn test_parse_ref_key_not_ref() {
        assert!(parse_ref_key("some/path.md").is_none());
    }

    #[test]
    fn test_resolve_file_path_ref_directory() {
        let root = Path::new("/project");
        let raw = Path::new("/project/aiwiki/raw");
        let resolved = resolve_file_path(root, raw, "ref:docs/intro.md");
        assert_eq!(resolved, PathBuf::from("/project/docs/intro.md"));
    }

    #[test]
    fn test_resolve_file_path_ref_single_file() {
        let root = Path::new("/project");
        let raw = Path::new("/project/aiwiki/raw");
        let resolved = resolve_file_path(root, raw, "ref:SPEC.md/");
        assert_eq!(resolved, PathBuf::from("/project/SPEC.md"));
    }

    #[test]
    fn test_resolve_file_path_raw() {
        let root = Path::new("/project");
        let raw = Path::new("/project/aiwiki/raw");
        let resolved = resolve_file_path(root, raw, "notes.md");
        assert_eq!(resolved, PathBuf::from("/project/aiwiki/raw/notes.md"));
    }

    #[test]
    fn test_get_source_name_ref() {
        assert_eq!(get_source_name("ref:docs/intro.md"), "docs");
    }

    #[test]
    fn test_get_source_name_plain() {
        assert_eq!(get_source_name("intro.md"), "intro.md");
    }
}
