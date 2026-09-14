//! Guard test: no per-call `Regex::new` in the agent hot paths (PERF-067).
//!
//! M5 hoisted the template-placeholder regex (previously recompiled on every
//! loop iteration) behind a `LazyLock` static. This test fails if a direct
//! regex compilation reappears in the guarded module outside such a site.

use std::fs;
use std::path::{Path, PathBuf};

/// Agent modules on per-call paths.
const GUARDED_FILES: &[&str] = &["src/template/mod.rs"];

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
            "{relative}:{} compiles a regex on a per-call path; hoist it to a \
             OnceLock/LazyLock static (PERF-067)\n    {}",
            index + 1,
            line.trim()
        );
    }
}

#[test]
fn agent_hot_paths_compile_no_regex_per_call() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for relative in GUARDED_FILES {
        assert_source_clean(&manifest_dir, relative);
    }
}
