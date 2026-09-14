//! Guard test: no per-page `Regex::new` in the masterfetch extraction paths
//! (PERF-064).
//!
//! The metadata extractor previously compiled ~12 regexes per HTML page parse.
//! M5 hoisted each one behind a `LazyLock` static. This test fails if a direct
//! regex compilation reappears in the guarded modules outside such a site.

use std::fs;
use std::path::{Path, PathBuf};

/// Masterfetch modules on the per-page parse path.
const GUARDED_FILES: &[&str] = &["src/masterfetch/metadata.rs", "src/masterfetch/youtube.rs"];

/// A `Regex::new` call is acceptable when it sits inside a lazily-initialised
/// static or an explicit cache lookup.
fn is_hoisted_or_cached(lines: &[&str], index: usize) -> bool {
    let start = index.saturating_sub(6);
    let window = lines[start..=index].join("\n");
    window.contains("LazyLock::new")
        || window.contains("get_or_init")
        || window.contains("OnceLock")
        || window.contains("CACHE")
        || window.contains(".entry(")
}

fn assert_source_clean(manifest_dir: &Path, relative: &str) {
    let path: PathBuf = manifest_dir.join(relative);
    let source =
        fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let lines: Vec<&str> = source.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        if !line.contains("Regex::new") {
            continue;
        }
        assert!(
            is_hoisted_or_cached(&lines, index),
            "{relative}:{} compiles a regex on the per-page parse path; hoist \
             it to a OnceLock/LazyLock static (PERF-064)\n    {}",
            index + 1,
            line.trim()
        );
    }
}

#[test]
fn masterfetch_page_parse_compiles_no_regex_per_call() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for relative in GUARDED_FILES {
        assert_source_clean(&manifest_dir, relative);
    }
}
