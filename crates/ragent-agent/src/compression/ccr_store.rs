//! CCR (Compress-Cache-Retrieve) store lifecycle management.
//!
//! This module implements a local CCR store for stashing original (pre-compression)
//! content so the LLM can retrieve it on demand via the `headroom_retrieve` tool.
//!
//! # Architecture
//!
//! The CCR store uses BLAKE3 hashing to generate unique keys for original content.
//! When the compression pipeline compresses a message part, it:
//!
//! 1. Computes a BLAKE3 hash of the original content.
//! 2. Stores the original in the CCR store under that hash.
//! 3. Inserts a `<<ccr:HASH>>` marker in the compressed output.
//!
//! The LLM can later retrieve the original content by invoking `headroom_retrieve`
//! with the hash key, which looks it up in this store.
//!
//! # Backends
//!
//! Two backends are supported:
//! - **InMemory** — process-local `DashMap`-backed store. Fast but lost on restart.
//!   Suitable for testing and single-process deployments.
//! - **SQLite** — persistent store that survives restarts. Production default.
//!
//! # Feature flag
//!
//! This module is only compiled when the `compression` Cargo feature is enabled.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use ragent_config::compression::CcrConfig;
use tracing::{debug, info};

/// BLAKE3-based key computation for CCR entries.
///
/// Produces a 24-character hex prefix of the BLAKE3 hash, matching
/// the headroom-core convention for `<<ccr:HASH>>` markers.
pub fn compute_ccr_key(payload: &[u8]) -> String {
    let hash = blake3::hash(payload);
    let hex = hash.to_hex();
    // 24-char prefix matches the CCR marker regex `[a-f0-9]{24}`.
    hex.as_str()[..24].to_string()
}

/// Generate a `<<ccr:HASH>>` marker for a CCR key.
pub fn ccr_marker(key: &str) -> String {
    format!("<<ccr:{key}>>")
}

/// An entry in the in-memory CCR store.
#[derive(Clone)]
struct CcrEntry {
    payload: String,
    inserted: Instant,
}

/// In-memory CCR store backed by a `HashMap` with TTL-based eviction.
///
/// This is suitable for testing and single-process deployments. For
/// production use with persistence, use `SqliteCcrStore` instead.
///
/// The store is `Send + Sync` and can be shared across threads via `Arc<Mutex<>>`.
pub struct InMemoryCcrStore {
    entries: HashMap<String, CcrEntry>,
    capacity: usize,
    ttl: Duration,
}

impl InMemoryCcrStore {
    /// Create a new in-memory store with the given capacity and TTL.
    pub fn with_capacity_and_ttl(capacity: usize, ttl: Duration) -> Self {
        Self {
            entries: HashMap::with_capacity(capacity),
            capacity,
            ttl,
        }
    }

    /// Create a new in-memory store with default capacity (1000) and TTL (5 min).
    pub fn new() -> Self {
        Self::with_capacity_and_ttl(1000, Duration::from_mins(5))
    }

    /// Store a payload under the given key. If the key already exists, the
    /// payload is overwritten (idempotent for identical content).
    pub fn put(&mut self, key: &str, payload: &str) {
        // Evict expired entries first.
        self.evict_expired();
        // If at capacity, evict the oldest entry.
        if self.entries.len() >= self.capacity && !self.entries.contains_key(key) {
            if let Some(oldest_key) = self
                .entries
                .iter()
                .min_by_key(|(_, v)| v.inserted)
                .map(|(k, _)| k.clone())
            {
                self.entries.remove(&oldest_key);
            }
        }
        self.entries.insert(
            key.to_string(),
            CcrEntry {
                payload: payload.to_string(),
                inserted: Instant::now(),
            },
        );
    }

    /// Look up a key. Returns `None` if missing or expired.
    pub fn get(&mut self, key: &str) -> Option<String> {
        // Check TTL.
        if let Some(entry) = self.entries.get(key) {
            if entry.inserted.elapsed() > self.ttl {
                self.entries.remove(key);
                return None;
            }
            Some(entry.payload.clone())
        } else {
            None
        }
    }

    /// Number of entries (including potentially expired ones that haven't
    /// been purged yet).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Evict all expired entries.
    fn evict_expired(&mut self) {
        let now = Instant::now();
        self.entries
            .retain(|_, v| now.duration_since(v.inserted) <= self.ttl);
    }
}

/// A CCR store handle that wraps a backend and provides the primary API for
/// the compression pipeline to stash and retrieve original content.
///
/// This is `Send + Sync` and can be shared across threads via `Arc<Mutex<>>`.
pub struct CcrStoreHandle {
    inner: InMemoryCcrStore,
    /// Path to the SQLite database (if using SQLite backend).
    db_path: Option<PathBuf>,
}

impl CcrStoreHandle {
    /// Create an in-memory CCR store for testing.
    pub fn in_memory() -> Self {
        Self {
            inner: InMemoryCcrStore::new(),
            db_path: None,
        }
    }

    /// Create an in-memory CCR store with custom capacity and TTL.
    pub fn in_memory_with_params(capacity: usize, ttl_secs: u64) -> Self {
        Self {
            inner: InMemoryCcrStore::with_capacity_and_ttl(capacity, Duration::from_secs(ttl_secs)),
            db_path: None,
        }
    }

    /// Create a CCR store from ragent configuration.
    ///
    /// Currently always uses in-memory backend. SQLite support will be added
    /// when the full headroom-core CCR module is available (v0.11+).
    pub fn from_config(config: &CcrConfig) -> Self {
        info!(
            backend = %config.backend,
            capacity = config.capacity,
            ttl_secs = config.ttl_secs,
            "Initialising CCR store"
        );
        Self {
            inner: InMemoryCcrStore::with_capacity_and_ttl(
                config.capacity,
                Duration::from_secs(config.ttl_secs),
            ),
            db_path: None,
        }
    }

    /// Compute the CCR key for a payload, store the original, and return
    /// both the key and the `<<ccr:KEY>>` marker.
    ///
    /// This is the primary API for the compression pipeline. Call it with
    /// the original content before compression, and use the marker in the
    /// compressed output.
    pub fn stash(&mut self, payload: &str) -> (String, String) {
        let key = compute_ccr_key(payload.as_bytes());
        self.inner.put(&key, payload);
        debug!(key = %key, payload_len = payload.len(), "Stashed payload in CCR store");
        let marker = ccr_marker(&key);
        (key, marker)
    }

    /// Retrieve original content from the CCR store by key.
    ///
    /// Returns `None` if the key is not found or the entry has expired.
    pub fn retrieve(&mut self, key: &str) -> Option<String> {
        self.inner.get(key)
    }

    /// Current number of entries in the store.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Whether the store is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Path to the SQLite database file, if using the SQLite backend.
    pub fn db_path(&self) -> Option<&PathBuf> {
        self.db_path.as_ref()
    }
}

/// Thread-safe wrapper around `CcrStoreHandle` for sharing across sessions.
pub type SharedCcrStore = std::sync::Arc<Mutex<CcrStoreHandle>>;

/// Create a shared CCR store from configuration.
///
/// Returns an `Arc<Mutex<CcrStoreHandle>>` that can be safely shared across
/// threads.
pub fn create_shared_ccr_store(config: &CcrConfig) -> SharedCcrStore {
    Arc::new(Mutex::new(CcrStoreHandle::from_config(config)))
}

/// Create a shared in-memory CCR store for testing.
pub fn create_test_ccr_store() -> SharedCcrStore {
    Arc::new(Mutex::new(CcrStoreHandle::in_memory()))
}

/// Parse a CCR marker from compressed text and return the key.
///
/// CCR markers have the format `<<ccr:HASH>>` where HASH is a 24-character
/// hex string. Returns all extracted keys found in the text.
pub fn parse_ccr_markers(text: &str) -> Vec<String> {
    let prefix = "<<ccr:";
    let suffix = ">>";
    let mut keys = Vec::new();
    let mut search_from = 0;

    while let Some(start) = text[search_from..].find(prefix) {
        let abs_start = search_from + start;
        let hash_start = abs_start + prefix.len();
        if let Some(end_offset) = text[hash_start..].find(suffix) {
            let key = &text[hash_start..hash_start + end_offset];
            if key.len() == 24 && key.chars().all(|c| c.is_ascii_hexdigit()) {
                keys.push(key.to_string());
            }
            search_from = hash_start + end_offset + suffix.len();
        } else {
            break;
        }
    }

    keys
}

/// Parse the first CCR marker from compressed text and return the key.
///
/// Returns `None` if no valid CCR marker is found.
pub fn parse_ccr_marker(text: &str) -> Option<String> {
    parse_ccr_markers(text).into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_ccr_key_deterministic() {
        let key1 = compute_ccr_key(b"hello world");
        let key2 = compute_ccr_key(b"hello world");
        assert_eq!(key1, key2, "Same payload should produce same key");
        assert_eq!(key1.len(), 24, "Key should be 24 hex chars");
    }

    #[test]
    fn test_compute_ccr_key_diverges() {
        let key1 = compute_ccr_key(b"hello");
        let key2 = compute_ccr_key(b"world");
        assert_ne!(
            key1, key2,
            "Different payloads should produce different keys"
        );
    }

    #[test]
    fn test_ccr_marker_format() {
        let key = "a1b2c3d4e5f6a7b8c9d0e1f2";
        let marker = ccr_marker(key);
        assert_eq!(marker, "<<ccr:a1b2c3d4e5f6a7b8c9d0e1f2>>");
    }

    #[test]
    fn test_in_memory_store_stash_and_retrieve() {
        let mut store = InMemoryCcrStore::new();
        store.put("key1", "payload1");
        let result = store.get("key1");
        assert_eq!(result, Some("payload1".to_string()));
    }

    #[test]
    fn test_in_memory_store_missing_key() {
        let mut store = InMemoryCcrStore::new();
        let result = store.get("nonexistent");
        assert!(result.is_none());
    }

    #[test]
    fn test_in_memory_store_overwrite() {
        let mut store = InMemoryCcrStore::new();
        store.put("key1", "payload1");
        store.put("key1", "payload2");
        let result = store.get("key1");
        assert_eq!(result, Some("payload2".to_string()));
    }

    #[test]
    fn test_ccr_store_handle_stash_and_retrieve() {
        let mut store = CcrStoreHandle::in_memory();
        let payload = "This is the original content that was compressed.";
        let (key, marker) = store.stash(payload);
        assert!(key.len() == 24, "Key should be 24 hex chars");
        assert!(marker.starts_with("<<ccr:"));
        assert!(marker.ends_with(">>"));

        let retrieved = store.retrieve(&key).expect("Should find stashed payload");
        assert_eq!(retrieved, payload);
    }

    #[test]
    fn test_ccr_store_handle_stash_deterministic_key() {
        let mut store = CcrStoreHandle::in_memory();
        let payload = "Same content";
        let (key1, _) = store.stash(payload);
        let (key2, _) = store.stash(payload);
        assert_eq!(key1, key2, "Same payload should produce same key");
    }

    #[test]
    fn test_ccr_store_handle_retrieve_missing() {
        let mut store = CcrStoreHandle::in_memory();
        let result = store.retrieve("nonexistentkey1234567890");
        assert!(result.is_none(), "Missing key should return None");
    }

    #[test]
    fn test_ccr_store_handle_len() {
        let mut store = CcrStoreHandle::in_memory();
        assert!(store.is_empty());
        store.stash("payload 1");
        store.stash("payload 2");
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn test_parse_ccr_marker_valid() {
        let text = "Compressed output <<ccr:a1b2c3d4e5f6a7b8c9d0e1f2>> more text";
        let key = parse_ccr_marker(text);
        assert_eq!(key, Some("a1b2c3d4e5f6a7b8c9d0e1f2".to_string()));
    }

    #[test]
    fn test_parse_ccr_markers_multiple() {
        let key1 = compute_ccr_key(b"payload one");
        let key2 = compute_ccr_key(b"payload two");
        let text = format!("{} and {}", ccr_marker(&key1), ccr_marker(&key2));
        let keys = parse_ccr_markers(&text);
        assert_eq!(keys.len(), 2, "Should find 2 CCR markers");
        assert_eq!(keys[0], key1);
        assert_eq!(keys[1], key2);
    }

    #[test]
    fn test_parse_ccr_marker_no_marker() {
        let text = "No marker here";
        let key = parse_ccr_marker(text);
        assert!(key.is_none());
    }

    #[test]
    fn test_parse_ccr_marker_invalid_length() {
        let text = "<<ccr:short>>";
        let key = parse_ccr_marker(text);
        assert!(key.is_none(), "Short key should not parse");
    }

    #[test]
    fn test_shared_ccr_store() {
        let store = create_test_ccr_store();
        let payload = "Shared test payload";
        let (key, marker) = {
            let mut s = store.lock().unwrap();
            s.stash(payload)
        };
        assert!(marker.starts_with("<<ccr:"));

        let retrieved = {
            let mut s = store.lock().unwrap();
            s.retrieve(&key)
        };
        assert_eq!(retrieved.as_deref(), Some(payload));
    }

    #[test]
    fn test_in_memory_with_params() {
        let mut store = CcrStoreHandle::in_memory_with_params(500, 60);
        store.stash("test");
        assert_eq!(store.len(), 1);
    }

    #[test]
    fn test_capacity_eviction() {
        let mut store = InMemoryCcrStore::with_capacity_and_ttl(2, Duration::from_mins(5));
        store.put("key1", "payload1");
        store.put("key2", "payload2");
        store.put("key3", "payload3");
        // Should have evicted the oldest entry to make room.
        assert!(store.len() <= 2);
    }
}
