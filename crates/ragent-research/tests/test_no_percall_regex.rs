//! Guard test: no per-call `Regex::new` on the research hot paths (PERF-064..068).
//!
//! M5 hoisted every regex that ran per-page / per-candidate / per-plan behind a
//! `OnceLock`/`LazyLock` static. This test scans the research crate sources and
//! fails if a `Regex::new` reappears outside a hoisted or cached construction
//! site, mirroring the `test_shared_http_client` guard from M4.

use std::fs;
use std::path::{Path, PathBuf};

/// Files on per-page / per-candidate / per-plan paths that must contain no
/// direct regex compilation outside a hoisted or cached site.
const GUARDED_FILES: &[&str] = &[
    "src/web_date.rs",
    "src/clarify.rs",
    "src/planner.rs",
    "src/cluster.rs",
    "src/session/topic.rs",
    "src/document.rs",
    "src/web_gatherer/relevance.rs",
    "src/session/fallback.rs",
    "src/analysis/parser.rs",
    "src/diagram.rs",
    "src/polarity.rs",
];

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
            "{relative}:{} compiles a regex on a hot path; hoist it to a \
             OnceLock/LazyLock static (PERF-064..068)\n    {}",
            index + 1,
            line.trim()
        );
    }
}

#[test]
fn research_hot_paths_compile_no_regex_per_call() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for relative in GUARDED_FILES {
        assert_source_clean(&manifest_dir, relative);
    }
}
