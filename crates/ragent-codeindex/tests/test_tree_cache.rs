//! External tests for `tests` from `crates/ragent-codeindex/src/tree_cache.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_codeindex::tree_cache::TreeCache;
use std::path::{Path, PathBuf};
use tree_sitter::Tree;

fn make_tree(source: &str) -> Tree {
    let mut parser = tree_sitter::Parser::new();
    let language = tree_sitter_rust::LANGUAGE;
    parser.set_language(&language.into()).unwrap();
    parser.parse(source.as_bytes(), None).unwrap()
}

#[test]
fn test_put_and_get() {
    let mut cache = TreeCache::new(10);
    let tree = make_tree("fn main() {}");
    cache.put(PathBuf::from("src/main.rs"), tree);

    assert_eq!(cache.len(), 1);
    assert!(cache.get(Path::new("src/main.rs")).is_some());
    assert!(cache.get(Path::new("src/lib.rs")).is_none());
}

#[test]
fn test_remove() {
    let mut cache = TreeCache::new(10);
    cache.put(PathBuf::from("src/main.rs"), make_tree("fn main() {}"));
    assert_eq!(cache.len(), 1);

    let removed = cache.remove(Path::new("src/main.rs"));
    assert!(removed.is_some());
    assert_eq!(cache.len(), 0);
}

#[test]
fn test_lru_eviction() {
    let mut cache = TreeCache::new(2);
    cache.put(PathBuf::from("a.rs"), make_tree("fn a() {}"));
    cache.put(PathBuf::from("b.rs"), make_tree("fn b() {}"));
    cache.put(PathBuf::from("c.rs"), make_tree("fn c() {}"));

    // "a.rs" should have been evicted.
    assert_eq!(cache.len(), 2);
    assert!(cache.get(Path::new("a.rs")).is_none());
    assert!(cache.get(Path::new("b.rs")).is_some());
    assert!(cache.get(Path::new("c.rs")).is_some());
}

#[test]
fn test_clear() {
    let mut cache = TreeCache::new(10);
    cache.put(PathBuf::from("a.rs"), make_tree("fn a() {}"));
    cache.put(PathBuf::from("b.rs"), make_tree("fn b() {}"));
    cache.clear();
    assert!(cache.is_empty());
}

#[test]
fn test_capacity() {
    let cache = TreeCache::new(42);
    assert_eq!(cache.capacity(), 42);
}

#[test]
fn test_default_capacity() {
    let cache = TreeCache::with_default_capacity();
    assert_eq!(cache.capacity(), 1000);
}
