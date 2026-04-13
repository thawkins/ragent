//! LRU tree cache for incremental tree-sitter parsing.
//!
//! Caches parsed `tree_sitter::Tree` objects keyed by file path, enabling
//! incremental re-parsing when files change. The cache is bounded by entry
//! count and uses LRU eviction.

use lru::LruCache;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use tree_sitter::Tree;

/// Default maximum number of cached trees.
const DEFAULT_CAPACITY: usize = 1000;

/// An LRU cache of tree-sitter parse trees, keyed by file path.
pub struct TreeCache {
    cache: LruCache<PathBuf, Tree>,
}

impl TreeCache {
    /// Create a new tree cache with the given capacity.
    pub fn new(capacity: usize) -> Self {
        let cap = NonZeroUsize::new(capacity.max(1)).unwrap();
        Self {
            cache: LruCache::new(cap),
        }
    }

    /// Create a tree cache with the default capacity (1000).
    pub fn with_default_capacity() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }

    /// Get the cached tree for a file, if present (also promotes it in LRU order).
    pub fn get(&mut self, path: &Path) -> Option<&Tree> {
        self.cache.get(path)
    }

    /// Store a tree in the cache.
    pub fn put(&mut self, path: PathBuf, tree: Tree) {
        self.cache.put(path, tree);
    }

    /// Remove a tree from the cache (e.g. on file deletion).
    pub fn remove(&mut self, path: &Path) -> Option<Tree> {
        self.cache.pop(path)
    }

    /// Number of cached trees.
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Clear all cached trees.
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Maximum capacity.
    pub fn capacity(&self) -> usize {
        self.cache.cap().get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
