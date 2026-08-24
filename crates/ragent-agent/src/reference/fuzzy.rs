//! Fuzzy file matching for bare `@name` references.
//!
//! Walks the project tree to collect candidate files, then scores them
//! against a query string using a simple multi-tier matching algorithm.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

/// Maximum number of project files to index for autocomplete.
const MAX_PROJECT_FILES: usize = 10_000;

/// Time-to-live for the cached project file list. After this duration the
/// cache is considered stale and the directory is re-scanned (FR-009).
const CACHE_TTL: Duration = Duration::from_secs(30);

/// Cached file list for a project directory, with the directory mtime observed
/// when the scan was performed.
struct ProjectFileCacheEntry {
    /// Relative file/directory paths collected from the project tree.
    files: Vec<PathBuf>,
    /// Directory modification time observed when `files` was built.
    dir_mtime: Option<SystemTime>,
    /// Wall-clock instant when `files` was built.
    fetched_at: Instant,
}

/// Process-wide cache of project file lists keyed by canonical working directory.
static PROJECT_FILE_CACHE: OnceLock<Mutex<HashMap<PathBuf, ProjectFileCacheEntry>>> =
    OnceLock::new();

/// Return the global project-file cache map.
fn project_file_cache() -> &'static Mutex<HashMap<PathBuf, ProjectFileCacheEntry>> {
    PROJECT_FILE_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

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
    let now = Instant::now();

    // Canonicalize the key so that equivalent paths share a cache entry.
    let cache_key = match std::fs::canonicalize(working_dir) {
        Ok(path) => path,
        Err(_) => working_dir.to_path_buf(),
    };

    // Current directory mtime; if unavailable we treat the cache as invalid.
    let current_mtime = std::fs::metadata(working_dir)
        .and_then(|m| m.modified())
        .ok();

    // Check for a fresh cache entry: TTL has not expired and the directory
    // mtime has not changed.
    if let Ok(cache) = project_file_cache().lock() {
        if let Some(entry) = cache.get(&cache_key) {
            let ttl_fresh = now.duration_since(entry.fetched_at) < CACHE_TTL;
            let mtime_fresh = current_mtime.is_some() && entry.dir_mtime == current_mtime;
            if ttl_fresh && mtime_fresh {
                return entry.files.iter().take(limit).cloned().collect();
            }
        }
    }

    // Collect the full project tree (up to the global limit) so the cached
    // value can satisfy later requests with different per-call `max` values.
    let mut files = Vec::new();
    walk_dir(working_dir, working_dir, &mut files, MAX_PROJECT_FILES);

    // Take the truncated subset for the caller before moving the full list
    // into the cache.  This avoids cloning up to 10 000 PathBufs — we only
    // clone the `limit` items the caller actually needs.
    let result: Vec<PathBuf> = files.iter().take(limit).cloned().collect();

    // Store the un-truncated list in the cache. If the mutex is poisoned we
    // gracefully skip caching and still return the collected paths.
    if let Ok(mut cache) = project_file_cache().lock() {
        cache.insert(
            cache_key,
            ProjectFileCacheEntry {
                files,
                dir_mtime: current_mtime,
                fetched_at: now,
            },
        );
    }

    result
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

    // FR-009: use ASCII lowercasing for the query.  File names in practice
    // are ASCII; `to_ascii_lowercase` avoids a heap allocation when the
    // query contains only ASCII characters.
    let query_lower = query.to_ascii_lowercase();
    let mut matches: Vec<(FuzzyMatch, usize)> = Vec::new();

    for candidate in candidates {
        // Allocate path_str once per candidate and reuse for all tier checks.
        let path_str = candidate.to_string_lossy();
        let path_lower = path_str.to_ascii_lowercase();

        // For directories (trailing '/'), use the directory name for basename
        // matching.  Borrow from path_lower to avoid a second allocation.
        let basename_lower: &str = if path_lower.ends_with('/') {
            let trimmed = path_lower.trim_end_matches('/');
            trimmed.rsplit('/').next().unwrap_or(trimmed)
        } else {
            // Extract the last path component from the lowercase path.
            path_lower.rsplit('/').next().unwrap_or(&path_lower)
        };

        let score = if basename_lower == query_lower {
            100
        } else if basename_lower.starts_with(&query_lower) {
            75
        } else if basename_lower.contains(&query_lower) {
            50
        } else if path_lower.contains(&query_lower) {
            25
        } else {
            continue;
        };

        // FR-009: pre-compute the path string length for the sort comparator
        // so the sort closure does not call `to_string_lossy()` (which
        // allocates) on every comparison.
        let path_len = path_str.len();
        matches.push((
            FuzzyMatch {
                path: candidate.clone(),
                score,
            },
            path_len,
        ));
    }

    // Sort by score descending, then by path length ascending (prefer shorter
    // paths).  The pre-computed `path_len` avoids per-comparison allocation.
    matches.sort_by(|a, b| b.0.score.cmp(&a.0.score).then_with(|| a.1.cmp(&b.1)));

    matches.into_iter().map(|(m, _)| m).collect()
}
