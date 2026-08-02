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
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let cap = NonZeroUsize::new(capacity.max(1)).unwrap();
        Self {
            cache: LruCache::new(cap),
        }
    }

    /// Create a tree cache with the default capacity (1000).
    #[must_use]
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
    #[must_use]
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Clear all cached trees.
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Maximum capacity.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.cache.cap().get()
    }
}
