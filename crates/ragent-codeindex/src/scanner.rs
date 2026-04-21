//! File scanning, content hashing, and language detection.
//!
//! The scanner discovers source files in a project directory using
//! gitignore-aware traversal, computes content hashes, detects languages
//! by file extension, and filters out binary files and oversized files.

use crate::types::{ScanConfig, ScannedFile};
use anyhow::{Context, Result};
use ignore::WalkBuilder;
use rayon::prelude::*;
use std::fs;
use std::path::Path;
use tracing::{debug, trace, warn};

/// Hardcoded directory names that are always excluded from scanning.
const EXCLUDED_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "__pycache__",
    ".ragent",
    ".venv",
    "venv",
    "dist",
    "build",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
    "aiwiki",
];

/// Number of bytes to check for NUL to detect binary files.
const BINARY_CHECK_BYTES: usize = 8192;

/// Scan a project directory and return all discovered source files.
///
/// Uses `ignore::WalkBuilder` for gitignore-aware traversal and
/// `rayon` for parallel hashing. Skips binary files, oversized files,
/// and directories listed in `EXCLUDED_DIRS` plus any extra
/// exclusions in `config`.
pub fn scan_directory(root: &Path, config: &ScanConfig) -> Result<Vec<ScannedFile>> {
    let root = root
        .canonicalize()
        .with_context(|| format!("cannot canonicalize root: {}", root.display()))?;

    // Collect paths first (WalkBuilder is not Send-safe for parallel iteration).
    let mut paths: Vec<std::path::PathBuf> = Vec::new();

    let walker = WalkBuilder::new(&root)
        .hidden(true) // respect hidden files
        .git_ignore(true) // respect .gitignore
        .git_global(true)
        .git_exclude(true)
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warn!("walk error: {e}");
                continue;
            }
        };

        // Skip directories themselves — we only want files.
        if entry.file_type().map_or(true, |ft| !ft.is_file()) {
            continue;
        }

        let path = entry.path();

        // Skip excluded directories.
        if path.ancestors().any(|ancestor| {
            ancestor
                .file_name()
                .and_then(|n| n.to_str())
                .map_or(false, |name| {
                    EXCLUDED_DIRS.contains(&name)
                        || config.extra_exclude_dirs.contains(&name.to_string())
                })
        }) {
            trace!("skipping excluded path: {}", path.display());
            continue;
        }

        paths.push(path.to_path_buf());
    }

    debug!(
        "found {} candidate files in {}",
        paths.len(),
        root.display()
    );

    // Process files in parallel.
    let results: Vec<Option<ScannedFile>> = paths
        .par_iter()
        .map(|path| process_file(path, &root, config))
        .collect();

    let files: Vec<ScannedFile> = results.into_iter().flatten().collect();
    debug!("scanned {} source files", files.len());

    Ok(files)
}

/// Process a single file: check size, detect binary, hash, count lines.
fn process_file(path: &Path, root: &Path, config: &ScanConfig) -> Option<ScannedFile> {
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) => {
            warn!("cannot stat {}: {e}", path.display());
            return None;
        }
    };

    let size = metadata.len();

    // Skip oversized files.
    if size > config.max_file_size {
        trace!("skipping oversized file ({size} bytes): {}", path.display());
        return None;
    }

    // Skip empty files.
    if size == 0 {
        return None;
    }

    // Read the file.
    let content = match fs::read(path) {
        Ok(c) => c,
        Err(e) => {
            warn!("cannot read {}: {e}", path.display());
            return None;
        }
    };

    // Binary check: look for NUL bytes in the first chunk.
    if is_binary(&content) {
        trace!("skipping binary file: {}", path.display());
        return None;
    }

    let hash = hash_content(&content);
    let language = detect_language(path);
    let line_count = count_lines(&content);
    let mtime_ns = mtime_nanos(&metadata);
    let rel_path = path.strip_prefix(root).unwrap_or(path).to_path_buf();

    Some(ScannedFile {
        path: rel_path,
        hash,
        size,
        language,
        mtime_ns,
        line_count,
    })
}

/// Compute the blake3 hash of file content, returned as a hex string.
pub fn hash_content(content: &[u8]) -> String {
    blake3::hash(content).to_hex().to_string()
}

/// Compute the blake3 hash of a file on disk.
pub fn hash_file(path: &Path) -> Result<String> {
    let content = fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    Ok(hash_content(&content))
}

/// Detect the programming language of a file based on its extension.
///
/// Returns `None` for unrecognised extensions.
pub fn detect_language(path: &Path) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    let lang = match ext.as_str() {
        "rs" => "rust",
        "py" | "pyi" => "python",
        "ts" => "typescript",
        "tsx" => "tsx",
        "js" => "javascript",
        "jsx" => "jsx",
        "go" => "go",
        "c" => "c",
        "h" => "c_header",
        "cpp" | "cc" | "cxx" => "cpp",
        "hpp" | "hxx" | "hh" => "cpp_header",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "rb" => "ruby",
        "swift" => "swift",
        "cs" => "csharp",
        "lua" => "lua",
        "sh" | "bash" => "shell",
        "zsh" => "zsh",
        "fish" => "fish",
        "toml" => "toml",
        "yaml" | "yml" => "yaml",
        "json" => "json",
        "xml" => "xml",
        "html" | "htm" => "html",
        "css" => "css",
        "scss" => "scss",
        "sql" => "sql",
        "md" | "markdown" => "markdown",
        "proto" => "protobuf",
        "zig" => "zig",
        "nim" => "nim",
        "ex" | "exs" => "elixir",
        "erl" | "hrl" => "erlang",
        "hs" => "haskell",
        "ml" | "mli" => "ocaml",
        "r" => "r",
        "dart" => "dart",
        "php" => "php",
        "pl" | "pm" => "perl",
        "v" | "sv" => "verilog",
        "vhd" | "vhdl" => "vhdl",
        "tf" | "tfvars" => "terraform",
        "scad" => "openscad",
        "cmake" => "cmake",
        "gradle" => "gradle",
        "nix" => "nix",
        _ => {
            // Filename-based detection for files without standard extensions.
            let filename = path.file_name()?.to_str()?.to_lowercase();
            match filename.as_str() {
                "cmakelists.txt" => "cmake",
                "pom.xml" => "maven",
                _ => {
                    // Handle double extensions like .gradle.kts
                    let name = path.file_stem()?.to_str()?.to_lowercase();
                    if name.ends_with(".gradle") && ext == "kts" {
                        return Some("gradle_kts".to_string());
                    }
                    return None;
                }
            }
        }
    };
    Some(lang.to_string())
}

/// Check if content looks like a binary file (contains NUL bytes in the first chunk).
fn is_binary(content: &[u8]) -> bool {
    let check_len = content.len().min(BINARY_CHECK_BYTES);
    content[..check_len].contains(&0)
}

/// Count the number of newline characters in file content.
fn count_lines(content: &[u8]) -> u64 {
    bytecount(content)
}

/// Fast newline counting.
#[allow(clippy::naive_bytecount)]
fn bytecount(data: &[u8]) -> u64 {
    data.iter().filter(|&&b| b == b'\n').count() as u64
}

/// Extract modification time as nanoseconds since Unix epoch.
fn mtime_nanos(metadata: &fs::Metadata) -> i64 {
    use std::time::UNIX_EPOCH;
    metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_language_rust() {
        assert_eq!(
            detect_language(Path::new("src/main.rs")),
            Some("rust".to_string())
        );
    }

    #[test]
    fn test_detect_language_python() {
        assert_eq!(
            detect_language(Path::new("app.py")),
            Some("python".to_string())
        );
    }

    #[test]
    fn test_detect_language_unknown() {
        assert_eq!(detect_language(Path::new("README")), None);
    }

    #[test]
    fn test_hash_content_deterministic() {
        let h1 = hash_content(b"hello world");
        let h2 = hash_content(b"hello world");
        assert_eq!(h1, h2);
        assert_ne!(h1, hash_content(b"different content"));
    }

    #[test]
    fn test_is_binary() {
        assert!(is_binary(b"hello\x00world"));
        assert!(!is_binary(b"hello world\n"));
    }

    #[test]
    fn test_count_lines() {
        assert_eq!(count_lines(b"line1\nline2\nline3\n"), 3);
        assert_eq!(count_lines(b"no newline"), 0);
    }
}
