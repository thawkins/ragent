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

    let mut sorted: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    sorted.sort_by_key(|e| e.file_name());

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

#[cfg(test)]
mod tests {
    use super::*;

    fn candidates() -> Vec<PathBuf> {
        vec![
            PathBuf::from("src/main.rs"),
            PathBuf::from("src/lib.rs"),
            PathBuf::from("src/config/mod.rs"),
            PathBuf::from("Cargo.toml"),
            PathBuf::from("README.md"),
            PathBuf::from("tests/test_main.rs"),
            PathBuf::from("src/reference/mod.rs"),
            PathBuf::from("src/reference/parse.rs"),
        ]
    }

    #[test]
    fn test_exact_basename_match() {
        let results = fuzzy_match("main.rs", &candidates());
        assert!(!results.is_empty());
        assert_eq!(results[0].path, PathBuf::from("src/main.rs"));
        assert_eq!(results[0].score, 100);
    }

    #[test]
    fn test_prefix_match() {
        let results = fuzzy_match("main", &candidates());
        assert!(!results.is_empty());
        // "main.rs" should match with prefix score
        assert!(results[0].score >= 75);
    }

    #[test]
    fn test_substring_match() {
        let results = fuzzy_match("lib", &candidates());
        assert!(!results.is_empty());
        assert!(
            results
                .iter()
                .any(|m| m.path == PathBuf::from("src/lib.rs"))
        );
    }

    #[test]
    fn test_path_match() {
        let results = fuzzy_match("reference", &candidates());
        assert!(!results.is_empty());
        assert!(
            results
                .iter()
                .any(|m| m.path == PathBuf::from("src/reference/mod.rs"))
        );
    }

    #[test]
    fn test_case_insensitive() {
        let results = fuzzy_match("README", &candidates());
        assert!(!results.is_empty());
        assert_eq!(results[0].path, PathBuf::from("README.md"));
    }

    #[test]
    fn test_no_match() {
        let results = fuzzy_match("nonexistent", &candidates());
        assert!(results.is_empty());
    }

    #[test]
    fn test_empty_query() {
        let results = fuzzy_match("", &candidates());
        assert_eq!(results.len(), candidates().len());
    }

    #[test]
    fn test_empty_candidates() {
        let results = fuzzy_match("main", &[]);
        assert!(results.is_empty());
    }

    #[test]
    fn test_collect_project_files() {
        let tmp = std::env::temp_dir().join("ragent_test_fuzzy_collect");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("src")).expect("mkdir");
        std::fs::write(tmp.join("src/main.rs"), "fn main() {}").expect("write");
        std::fs::write(tmp.join("Cargo.toml"), "[package]").expect("write");
        std::fs::create_dir_all(tmp.join(".git")).expect("mkdir .git");
        std::fs::write(tmp.join(".git/HEAD"), "ref").expect("write");

        let files = collect_project_files(&tmp, 100);
        assert!(files.iter().any(|p| p == Path::new("src/main.rs")));
        assert!(files.iter().any(|p| p == Path::new("Cargo.toml")));
        // Directories should be included with trailing /
        assert!(files.iter().any(|p| p == Path::new("src/")));
        // .git should be skipped
        assert!(!files.iter().any(|p| p.starts_with(".git")));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_score_ordering() {
        let candidates = vec![
            PathBuf::from("src/lib/main.rs"),    // path match for "main"
            PathBuf::from("src/main_helper.rs"), // prefix match
            PathBuf::from("src/main.rs"),        // exact basename match
        ];
        let results = fuzzy_match("main.rs", &candidates);
        assert_eq!(results[0].path, PathBuf::from("src/main.rs"));
    }
}
