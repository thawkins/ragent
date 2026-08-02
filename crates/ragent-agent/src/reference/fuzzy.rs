//! Fuzzy file matching for bare `@name` references.
//!
//! Walks the project tree to collect candidate files, then scores them
//! against a query string using a simple multi-tier matching algorithm.

use std::path::{Path, PathBuf};

/// Maximum number of project files to index for autocomplete.
const MAX_PROJECT_FILES: usize = 10_000;

/// Directories to skip during project file collection.
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    ".git",
    "__pycache__",
    "dist",
    "build",
    ".next",
    ".nuxt",
    "vendor",
    ".cargo",
];

/// A fuzzy match result with its score and path.
#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    /// The matched file path (relative to project root).
    pub path: PathBuf,
    /// Match score (higher is better).
    pub score: u32,
}

/// Walk the project directory tree and collect file paths.
///
/// Skips hidden files/directories and well-known generated directories.
/// Returns at most `MAX_PROJECT_FILES` relative paths.
///
/// # Errors
///
/// This function does not return errors. File system errors during directory
/// traversal are silently ignored and traversal continues with remaining entries.
#[must_use]
pub fn collect_project_files(working_dir: &Path, max: usize) -> Vec<PathBuf> {
    let limit = max.min(MAX_PROJECT_FILES);
    let mut files = Vec::new();
    walk_dir(working_dir, working_dir, &mut files, limit);
    files
}

fn walk_dir(root: &Path, dir: &Path, files: &mut Vec<PathBuf>, max: usize) {
    if files.len() >= max {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut sorted: Vec<_> = entries.filter_map(std::result::Result::ok).collect();
    sorted.sort_by_key(std::fs::DirEntry::file_name);

    for entry in sorted {
        if files.len() >= max {
            break;
        }

        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip hidden entries
        if name_str.starts_with('.') {
            continue;
        }

        let path = entry.path();

        if path.is_dir() {
            if SKIP_DIRS.contains(&name_str.as_ref()) {
                continue;
            }
            // Add the directory itself (with trailing separator)
            if let Ok(rel) = path.strip_prefix(root) {
                let mut dir_str = rel.to_string_lossy().to_string();
                dir_str.push('/');
                files.push(PathBuf::from(dir_str));
            }
            walk_dir(root, &path, files, max);
        } else if let Ok(rel) = path.strip_prefix(root) {
            files.push(rel.to_path_buf());
        }
    }
}

/// Score and rank candidate files against a query string.
///
/// Returns matches sorted by score descending. The scoring tiers are:
/// 1. Exact basename match (score 100)
/// 2. Basename prefix match (score 75)
/// 3. Basename substring match (score 50)
/// 4. Path component substring match (score 25)
///
/// Case-insensitive matching is used throughout.
///
/// # Errors
///
/// This function does not return errors. Empty queries and empty candidate lists
/// both return an empty vector.
#[must_use]
pub fn fuzzy_match(query: &str, candidates: &[PathBuf]) -> Vec<FuzzyMatch> {
    if query.is_empty() {
        // Return all candidates with equal score for initial menu
        return candidates
            .iter()
            .take(50)
            .map(|p| FuzzyMatch {
                path: p.clone(),
                score: 1,
            })
            .collect();
    }

    let query_lower = query.to_lowercase();
    let mut matches: Vec<FuzzyMatch> = Vec::new();

    for candidate in candidates {
        let path_str = candidate.to_string_lossy().to_lowercase();
        // For directories (trailing '/'), use the directory name for basename matching
        let basename = if path_str.ends_with('/') {
            let trimmed = path_str.trim_end_matches('/');
            trimmed.rsplit('/').next().unwrap_or(trimmed).to_string()
        } else {
            candidate
                .file_name()
                .map(|n| n.to_string_lossy().to_lowercase())
                .unwrap_or_default()
        };

        let score = if basename == query_lower {
            // Exact basename match
            100
        } else if basename.starts_with(&query_lower) {
            // Basename prefix match
            75
        } else if basename.contains(&query_lower) {
            // Basename substring match
            50
        } else if path_str.contains(&query_lower) {
            // Path substring match
            25
        } else {
            continue;
        };

        matches.push(FuzzyMatch {
            path: candidate.clone(),
            score,
        });
    }

    // Sort by score descending, then by path length ascending (prefer shorter paths)
    matches.sort_by(|a, b| {
        b.score.cmp(&a.score).then_with(|| {
            a.path
                .to_string_lossy()
                .len()
                .cmp(&b.path.to_string_lossy().len())
        })
    });

    matches
}
