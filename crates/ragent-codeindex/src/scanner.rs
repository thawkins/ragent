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

    // Compile the configured exclusion globs once.
    //
    // SEC-ragent-codeindex-006 (SECTASKS T-060): `ScanConfig.extra_exclude_patterns`
    // was declared and persisted by the config subsystem but never consulted, so a
    // user who added `*.snap` or `secrets/**` still had those files indexed and
    // searchable. Build a matcher here and apply it to every candidate path.
    let exclude_matcher = build_exclude_matcher(&config.extra_exclude_patterns);

    // Collect paths first (WalkBuilder is not Send-safe for parallel iteration).
    let mut paths: Vec<std::path::PathBuf> = Vec::new();

    let walker = WalkBuilder::new(&root)
        .hidden(true) // respect hidden files
        .git_ignore(true) // respect .gitignore
        .git_global(true)
        .git_exclude(true)
        .follow_links(false) // SEC-ragent-codeindex-001: never traverse symlinks
        .build();

    for entry in walker {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warn!("walk error: {e}");
                continue;
            }
        };

        // Skip directories themselves - we only want files.
        if entry.file_type().is_none_or(|ft| !ft.is_file()) {
            continue;
        }

        let path = entry.path();

        // Skip symlinks: a link inside the tree may point anywhere, and
        // following it indexes content outside the project root.
        if entry.file_type().is_some_and(|ft| ft.is_symlink()) {
            trace!("skipping symlink: {}", path.display());
            continue;
        }

        // Skip excluded directories.
        if path.ancestors().any(|ancestor| {
            ancestor
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|name| {
                    EXCLUDED_DIRS.contains(&name)
                        || config.extra_exclude_dirs.contains(&name.to_string())
                })
        }) {
            trace!("skipping excluded path: {}", path.display());
            continue;
        }

        // Apply the configured exclusion globs (SEC-ragent-codeindex-006).
        if let Some(matcher) = &exclude_matcher
            && matcher.is_match(path)
        {
            trace!("skipping glob-excluded path: {}", path.display());
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

    // Re-check the size against the bytes actually read.
    //
    // SEC-ragent-codeindex-005 (SECTASKS T-060): `fs::metadata` above is only
    // advisory - a file that grows between the stat and the read (or a device
    // that reports a small size) is read in full by `fs::read`, bypassing the
    // cap. Derive the stored size from the content and drop the file when the
    // real size exceeds the limit.
    let size = content.len() as u64;
    if size == 0 {
        return None;
    }
    if size > config.max_file_size {
        trace!(
            "skipping file that grew past the cap ({size} bytes): {}",
            path.display()
        );
        return None;
    }

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
#[must_use]
pub fn hash_content(content: &[u8]) -> String {
    blake3::hash(content).to_hex().to_string()
}

/// Build a glob matcher for the configured exclusion patterns.
///
/// SEC-ragent-codeindex-006 (SECTASKS T-060). Returns `None` when no pattern is
/// configured, or when every pattern failed to compile (the failure is logged so
/// a typo in `codeindex.extra_exclude_patterns` is visible rather than silent).
fn build_exclude_matcher(patterns: &[String]) -> Option<globset::GlobSet> {
    if patterns.is_empty() {
        return None;
    }
    let mut builder = globset::GlobSetBuilder::new();
    let mut added = 0usize;
    for pattern in patterns {
        match globset::Glob::new(pattern) {
            Ok(glob) => {
                builder.add(glob);
                added += 1;
            }
            Err(e) => warn!("ignoring invalid codeindex exclusion glob '{pattern}': {e}"),
        }
    }
    if added == 0 {
        return None;
    }
    match builder.build() {
        Ok(set) => Some(set),
        Err(e) => {
            warn!("cannot build codeindex exclusion glob set: {e}");
            None
        }
    }
}

/// Compute the blake3 hash of a file on disk.
pub fn hash_file(path: &Path) -> Result<String> {
    let content = fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    Ok(hash_content(&content))
}

/// Detect the programming language of a file based on its extension.
///
/// Returns `None` for unrecognised extensions.
#[must_use]
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

/// List of supported languages for the code index.
/// These are the language identifiers returned by `detect_language`.
pub const SUPPORTED_LANGUAGES: &[&str] = &[
    "rust",
    "python",
    "typescript",
    "tsx",
    "javascript",
    "jsx",
    "go",
    "c",
    "c_header",
    "cpp",
    "cpp_header",
    "java",
    "kotlin",
    "ruby",
    "swift",
    "csharp",
    "lua",
    "shell",
    "zsh",
    "fish",
    "toml",
    "yaml",
    "json",
    "xml",
    "html",
    "css",
    "scss",
    "sql",
    "markdown",
    "protobuf",
    "zig",
    "nim",
    "elixir",
    "erlang",
    "haskell",
    "ocaml",
    "r",
    "dart",
    "php",
    "perl",
    "verilog",
    "vhdl",
    "terraform",
    "openscad",
    "cmake",
    "gradle",
    "gradle_kts",
    "maven",
    "nix",
    "hcl",
];

/// Check if content looks like a binary file (contains NUL bytes in the first chunk).
pub fn is_binary(content: &[u8]) -> bool {
    let check_len = content.len().min(BINARY_CHECK_BYTES);
    content[..check_len].contains(&0)
}

/// Count the number of newline characters in file content.
pub fn count_lines(content: &[u8]) -> u64 {
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
