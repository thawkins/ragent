//! Unit tests for `masterfetch::cache` — SQLite WAL content cache
//! (T-035, FR-018, NFR-003).
//!
//! Covers:
//! - get/set round-trip (all fields preserved)
//! - TTL expiry (immediate expiry with `ttl=0`, normal TTL stays fresh)
//! - size-cap eviction (oldest entries evicted when `max_bytes` exceeded)
//! - `clear_expired` vs `clear_all` (selective vs full purge)
//! - WAL mode (file-based cache reports `journal_mode = wal`)
//! - bad content not cached (`content_ok = false` is never stored)
//!
//! Plus edge cases: key isolation (same URL, different params), overwrite
//! semantics, empty-cache lookups, `entry_count` / `total_bytes` accounting.

use std::fs;

use ragent_tools_extended::masterfetch::cache::{
    CacheKey, ContentCache, ContentCacheConfig, DEFAULT_CACHE_TTL, DEFAULT_MAX_BYTES,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A unique temp directory that removes itself on drop.
struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "ragent_mf_cache_test_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        fs::create_dir_all(&dir).expect("creating temp dir");
        Self(dir)
    }

    fn join(&self, name: &str) -> std::path::PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Build a simple cache key for `https://example.com/page-{n}`.
fn key(n: u32) -> CacheKey {
    CacheKey::new(format!("https://example.com/page-{n}"))
}

/// Insert a well-formed entry with `content_ok = true`.
fn insert(cache: &ContentCache, k: &CacheKey, content: &str, ttl: u64) {
    cache
        .set_cached(k, content, true, 200, "text/markdown", ttl)
        .expect("set_cached");
}

// ===========================================================================
// get/set round-trip
// ===========================================================================

#[test]
fn test_get_on_empty_cache_returns_none() {
    let cache = ContentCache::open_in_memory().expect("open");
    assert!(cache.get_cached(&key(1)).expect("get").is_none());
}

#[test]
fn test_get_on_nonexistent_key_returns_none() {
    let cache = ContentCache::open_in_memory().expect("open");
    insert(&cache, &key(1), "hello", 3600);
    assert!(cache.get_cached(&key(2)).expect("get").is_none());
}

#[test]
fn test_round_trip_basic_content() {
    let cache = ContentCache::open_in_memory().expect("open");
    let k = key(1);
    insert(&cache, &k, "# Hello World", 3600);

    let entry = cache.get_cached(&k).expect("get").expect("entry exists");
    assert_eq!(entry.content, "# Hello World");
}

#[test]
fn test_round_trip_all_fields_preserved() {
    let cache = ContentCache::open_in_memory().expect("open");
    let k = CacheKey::new("https://example.com/article")
        .with_extraction_type("html")
        .with_css_selector(Some("main"))
        .with_pages(Some("1-3"));

    cache
        .set_cached(
            &k,
            "<p>body</p>",
            true,
            404,
            "text/html; charset=utf-8",
            7200,
        )
        .expect("set");

    let entry = cache.get_cached(&k).expect("get").expect("entry");
    assert_eq!(entry.content, "<p>body</p>");
    assert!(entry.content_ok);
    assert_eq!(entry.status_code, 404);
    assert_eq!(entry.content_type, "text/html; charset=utf-8");
}

#[test]
fn test_round_trip_size_bytes_equals_content_len() {
    let cache = ContentCache::open_in_memory().expect("open");
    let k = key(1);
    let content = "ABCDE"; // 5 bytes
    insert(&cache, &k, content, 3600);

    let entry = cache.get_cached(&k).expect("get").expect("entry");
    assert_eq!(entry.size_bytes, content.len());
}

#[test]
fn test_round_trip_created_and_expires_set() {
    let cache = ContentCache::open_in_memory().expect("open");
    let k = key(1);
    let ttl = 3600_u64;
    insert(&cache, &k, "content", ttl);

    let entry = cache.get_cached(&k).expect("get").expect("entry");
    assert!(entry.created_at > 0);
    assert_eq!(entry.expires_at, entry.created_at + ttl);
}

#[test]
fn test_round_trip_default_ttl_constant() {
    // Verify the DEFAULT_CACHE_TTL constant is 3600 (1 hour).
    assert_eq!(DEFAULT_CACHE_TTL, 3600);
}

#[test]
fn test_round_trip_unicode_content() {
    let cache = ContentCache::open_in_memory().expect("open");
    let k = key(1);
    let content = "Hello — 世界 🦀 — café";
    insert(&cache, &k, content, 3600);

    let entry = cache.get_cached(&k).expect("get").expect("entry");
    assert_eq!(entry.content, content);
    // size_bytes is the byte length, not char count.
    assert_eq!(entry.size_bytes, content.len());
}

// ===========================================================================
// Key isolation — same URL, different parameters must not collide
// ===========================================================================

#[test]
fn test_key_isolation_different_extraction_types() {
    let cache = ContentCache::open_in_memory().expect("open");
    let k1 = CacheKey::new("https://example.com/p").with_extraction_type("markdown");
    let k2 = CacheKey::new("https://example.com/p").with_extraction_type("html");

    insert(&cache, &k1, "markdown-content", 3600);
    insert(&cache, &k2, "html-content", 3600);

    assert_eq!(
        cache.get_cached(&k1).expect("get").unwrap().content,
        "markdown-content"
    );
    assert_eq!(
        cache.get_cached(&k2).expect("get").unwrap().content,
        "html-content"
    );
    assert_eq!(cache.entry_count().expect("count"), 2);
}

#[test]
fn test_key_isolation_different_css_selectors() {
    let cache = ContentCache::open_in_memory().expect("open");
    let k1 = CacheKey::new("https://example.com/p").with_css_selector(Some("main"));
    let k2 = CacheKey::new("https://example.com/p").with_css_selector(Some("aside"));

    insert(&cache, &k1, "main-content", 3600);
    insert(&cache, &k2, "aside-content", 3600);

    assert_eq!(
        cache.get_cached(&k1).expect("get").unwrap().content,
        "main-content"
    );
    assert_eq!(
        cache.get_cached(&k2).expect("get").unwrap().content,
        "aside-content"
    );
}

#[test]
fn test_key_isolation_different_pages() {
    let cache = ContentCache::open_in_memory().expect("open");
    let k1 = CacheKey::new("https://example.com/p").with_pages(Some("1"));
    let k2 = CacheKey::new("https://example.com/p").with_pages(Some("2"));

    insert(&cache, &k1, "page-1", 3600);
    insert(&cache, &k2, "page-2", 3600);

    assert_eq!(
        cache.get_cached(&k1).expect("get").unwrap().content,
        "page-1"
    );
    assert_eq!(
        cache.get_cached(&k2).expect("get").unwrap().content,
        "page-2"
    );
}

#[test]
fn test_key_isolation_all_components_combined() {
    let cache = ContentCache::open_in_memory().expect("open");
    let k1 = CacheKey::new("https://example.com/p")
        .with_extraction_type("markdown")
        .with_css_selector(Some("main"))
        .with_pages(Some("1"));
    let k2 = CacheKey::new("https://example.com/p")
        .with_extraction_type("html")
        .with_css_selector(Some("aside"))
        .with_pages(Some("2"));

    insert(&cache, &k1, "entry-1", 3600);
    insert(&cache, &k2, "entry-2", 3600);

    assert_eq!(cache.entry_count().expect("count"), 2);
    assert_eq!(
        cache.get_cached(&k1).expect("get").unwrap().content,
        "entry-1"
    );
    assert_eq!(
        cache.get_cached(&k2).expect("get").unwrap().content,
        "entry-2"
    );
}

#[test]
fn test_key_with_none_css_selector_and_pages_matches_default_key() {
    let cache = ContentCache::open_in_memory().expect("open");
    let k_default = CacheKey::new("https://example.com/p");
    let k_explicit = CacheKey::new("https://example.com/p")
        .with_css_selector(None::<&str>)
        .with_pages(None::<&str>);

    insert(&cache, &k_default, "default-content", 3600);

    // Both keys normalise css_selector and pages to the empty sentinel, so
    // they should resolve to the same row.
    assert_eq!(
        cache.get_cached(&k_explicit).expect("get").unwrap().content,
        "default-content"
    );
}

// ===========================================================================
// Overwrite semantics — same key replaces existing entry
// ===========================================================================

#[test]
fn test_overwrite_same_key_replaces_content() {
    let cache = ContentCache::open_in_memory().expect("open");
    let k = key(1);

    insert(&cache, &k, "old-content", 3600);
    insert(&cache, &k, "new-content", 3600);

    let entry = cache.get_cached(&k).expect("get").expect("entry");
    assert_eq!(entry.content, "new-content");
    assert_eq!(cache.entry_count().expect("count"), 1);
}

#[test]
fn test_overwrite_updates_size_bytes() {
    let cache = ContentCache::open_in_memory().expect("open");
    let k = key(1);

    insert(&cache, &k, "short", 3600);
    let first = cache.get_cached(&k).expect("get").unwrap();
    assert_eq!(first.size_bytes, 5);

    insert(&cache, &k, "a much longer content string", 3600);
    let second = cache.get_cached(&k).expect("get").unwrap();
    assert_eq!(second.size_bytes, 28);
    assert_eq!(cache.total_bytes().expect("total"), 28);
}

// ===========================================================================
// TTL expiry
// ===========================================================================

#[test]
fn test_ttl_zero_immediately_expired() {
    // ttl_seconds == 0 → expires_at == created_at. A subsequent get_cached
    // sees expires_at <= now and lazily deletes the row → returns None.
    let cache = ContentCache::open_in_memory().expect("open");
    let k = key(1);

    cache
        .set_cached(&k, "ephemeral", true, 200, "text/plain", 0)
        .expect("set");

    // The row is stored but immediately expired, so get should return None
    // and delete it.
    assert!(cache.get_cached(&k).expect("get").is_none());
}

#[test]
fn test_ttl_zero_entry_count_decreases_after_get() {
    let cache = ContentCache::open_in_memory().expect("open");
    let k = key(1);

    cache
        .set_cached(&k, "ephemeral", true, 200, "text/plain", 0)
        .expect("set");

    // The row exists in the table immediately after insert (even though
    // expired). The get_cached call lazily deletes it.
    assert_eq!(cache.entry_count().expect("count"), 1);
    let _ = cache.get_cached(&k).expect("get");
    assert_eq!(cache.entry_count().expect("count"), 0);
}

#[test]
fn test_ttl_normal_still_fresh() {
    // A generous TTL should keep the entry retrievable.
    let cache = ContentCache::open_in_memory().expect("open");
    let k = key(1);
    insert(&cache, &k, "fresh-content", 3600);

    let entry = cache.get_cached(&k).expect("get").expect("entry");
    assert_eq!(entry.content, "fresh-content");
    assert!(entry.expires_at > entry.created_at);
}

#[test]
fn test_ttl_long_expiry_far_in_future() {
    let cache = ContentCache::open_in_memory().expect("open");
    let k = key(1);
    let ttl = 86400_u64; // 24 hours
    insert(&cache, &k, "long-lived", ttl);

    let entry = cache.get_cached(&k).expect("get").expect("entry");
    assert_eq!(entry.expires_at - entry.created_at, ttl);
}

// ===========================================================================
// Bad content not cached (FR-018)
// ===========================================================================

#[test]
fn test_bad_content_not_stored() {
    let cache = ContentCache::open_in_memory().expect("open");
    let k = key(1);

    cache
        .set_cached(&k, "bad content", false, 403, "text/html", 3600)
        .expect("set");

    assert_eq!(cache.entry_count().expect("count"), 0);
    assert!(cache.get_cached(&k).expect("get").is_none());
}

#[test]
fn test_bad_content_does_not_affect_existing_good_entry() {
    let cache = ContentCache::open_in_memory().expect("open");
    let k = key(1);

    insert(&cache, &k, "good-content", 3600);
    // Attempt to overwrite with bad content — should be a no-op.
    cache
        .set_cached(&k, "bad-content", false, 500, "text/html", 3600)
        .expect("set");

    let entry = cache.get_cached(&k).expect("get").expect("entry");
    assert_eq!(entry.content, "good-content");
    assert_eq!(entry.status_code, 200);
}

#[test]
fn test_bad_content_total_bytes_unchanged() {
    let cache = ContentCache::open_in_memory().expect("open");
    let k = key(1);

    insert(&cache, &k, "good", 3600);
    let bytes_before = cache.total_bytes().expect("total");

    cache
        .set_cached(
            &k,
            "bad content that is longer",
            false,
            500,
            "text/html",
            3600,
        )
        .expect("set");

    assert_eq!(cache.total_bytes().expect("total"), bytes_before);
}

#[test]
fn test_bad_content_set_returns_ok() {
    // set_cached with content_ok=false should return Ok(()) — it's a
    // deliberate no-op, not an error.
    let cache = ContentCache::open_in_memory().expect("open");
    let k = key(1);
    let result = cache.set_cached(&k, "bad", false, 500, "text/html", 3600);
    assert!(result.is_ok());
}

// ===========================================================================
// Size cap eviction
// ===========================================================================

#[test]
fn test_size_cap_evicts_oldest() {
    // Set max_bytes to 30 bytes. Insert 3 entries of 10 bytes each = 30,
    // then insert a 4th → total would be 40, exceeding the cap. The oldest
    // entry (page-1) should be evicted.
    let config = ContentCacheConfig { max_bytes: 30 };
    let cache = ContentCache::open_in_memory_with_config(config).expect("open");

    insert(&cache, &key(1), "aaaaaaaaaa", 3600); // 10 bytes
    insert(&cache, &key(2), "bbbbbbbbbb", 3600); // 10 bytes
    insert(&cache, &key(3), "cccccccccc", 3600); // 10 bytes → total 30, at cap

    assert_eq!(cache.entry_count().expect("count"), 3);
    assert_eq!(cache.total_bytes().expect("total"), 30);

    // Insert a 4th → total 40, exceeds 30, oldest (page-1) evicted → 30.
    insert(&cache, &key(4), "dddddddddd", 3600);

    assert_eq!(cache.entry_count().expect("count"), 3);
    assert!(cache.get_cached(&key(1)).expect("get").is_none());
    assert!(cache.get_cached(&key(2)).expect("get").is_some());
    assert!(cache.get_cached(&key(3)).expect("get").is_some());
    assert!(cache.get_cached(&key(4)).expect("get").is_some());
}

#[test]
fn test_size_cap_evicts_multiple_entries() {
    // max_bytes = 20. Insert 2× 10-byte entries (total 20, at cap), then
    // insert a 20-byte entry → total 40, must evict 2 oldest to get to 20.
    let config = ContentCacheConfig { max_bytes: 20 };
    let cache = ContentCache::open_in_memory_with_config(config).expect("open");

    insert(&cache, &key(1), "aaaaaaaaaa", 3600); // 10 bytes
    insert(&cache, &key(2), "bbbbbbbbbb", 3600); // 10 bytes
    insert(&cache, &key(3), "cccccccccccccccccccc", 3600); // 20 bytes

    // After inserting key(3), total was 40 → evict key(1) (40-10=30) → still
    // over → evict key(2) (30-10=20) → at cap. Only key(3) remains.
    assert_eq!(cache.entry_count().expect("count"), 1);
    assert!(cache.get_cached(&key(1)).expect("get").is_none());
    assert!(cache.get_cached(&key(2)).expect("get").is_none());
    assert!(cache.get_cached(&key(3)).expect("get").is_some());
}

#[test]
fn test_size_cap_single_entry_exceeding_cap_kept() {
    // If a single entry exceeds max_bytes, it is inserted and then eviction
    // runs. Since it's the only entry, evict_to_cap removes it (it's the
    // oldest and total > max). So the cache ends up empty.
    // Actually: evict_to_cap evicts oldest entries until total <= max. With
    // one entry larger than max, it evicts that entry → cache empty.
    let config = ContentCacheConfig { max_bytes: 5 };
    let cache = ContentCache::open_in_memory_with_config(config).expect("open");

    insert(&cache, &key(1), "this is way too long", 3600); // 20 bytes

    // The single entry exceeds the cap, so it's evicted.
    assert_eq!(cache.entry_count().expect("count"), 0);
}

#[test]
fn test_size_cap_default_is_100mib() {
    assert_eq!(DEFAULT_MAX_BYTES, 100 * 1024 * 1024);
}

#[test]
fn test_size_cap_zero_max_bytes_evicts_everything() {
    // max_bytes = 0 → every insert is immediately evicted.
    let config = ContentCacheConfig { max_bytes: 0 };
    let cache = ContentCache::open_in_memory_with_config(config).expect("open");

    insert(&cache, &key(1), "content", 3600);
    assert_eq!(cache.entry_count().expect("count"), 0);
}

#[test]
fn test_size_cap_total_bytes_reflects_eviction() {
    let config = ContentCacheConfig { max_bytes: 20 };
    let cache = ContentCache::open_in_memory_with_config(config).expect("open");

    insert(&cache, &key(1), "aaaaaaaaaa", 3600); // 10 bytes
    insert(&cache, &key(2), "bbbbbbbbbb", 3600); // 10 bytes → total 20

    assert_eq!(cache.total_bytes().expect("total"), 20);

    insert(&cache, &key(3), "cccccccccc", 3600); // 10 → total 30, evict key(1) → 20

    assert_eq!(cache.total_bytes().expect("total"), 20);
}

// ===========================================================================
// clear_expired vs clear_all
// ===========================================================================

#[test]
fn test_clear_expired_removes_only_expired_entries() {
    let cache = ContentCache::open_in_memory().expect("open");

    // Insert one immediately-expired entry and one fresh entry.
    cache
        .set_cached(&key(1), "expired", true, 200, "text/plain", 0)
        .expect("set");
    insert(&cache, &key(2), "fresh", 3600);

    assert_eq!(cache.entry_count().expect("count"), 2);

    let purged = cache.clear_expired().expect("clear_expired");
    assert_eq!(purged, 1);
    assert_eq!(cache.entry_count().expect("count"), 1);
    assert!(cache.get_cached(&key(2)).expect("get").is_some());
}

#[test]
fn test_clear_expired_with_no_expired_returns_zero() {
    let cache = ContentCache::open_in_memory().expect("open");
    insert(&cache, &key(1), "fresh", 3600);

    let purged = cache.clear_expired().expect("clear_expired");
    assert_eq!(purged, 0);
    assert_eq!(cache.entry_count().expect("count"), 1);
}

#[test]
fn test_clear_expired_on_empty_cache_returns_zero() {
    let cache = ContentCache::open_in_memory().expect("open");
    let purged = cache.clear_expired().expect("clear_expired");
    assert_eq!(purged, 0);
}

#[test]
fn test_clear_expired_removes_all_expired() {
    let cache = ContentCache::open_in_memory().expect("open");

    // 3 expired entries + 1 fresh.
    cache
        .set_cached(&key(1), "exp1", true, 200, "text/plain", 0)
        .expect("set");
    cache
        .set_cached(&key(2), "exp2", true, 200, "text/plain", 0)
        .expect("set");
    cache
        .set_cached(&key(3), "exp3", true, 200, "text/plain", 0)
        .expect("set");
    insert(&cache, &key(4), "fresh", 3600);

    let purged = cache.clear_expired().expect("clear_expired");
    assert_eq!(purged, 3);
    assert_eq!(cache.entry_count().expect("count"), 1);
}

#[test]
fn test_clear_all_removes_everything() {
    let cache = ContentCache::open_in_memory().expect("open");
    insert(&cache, &key(1), "a", 3600);
    insert(&cache, &key(2), "b", 3600);
    insert(&cache, &key(3), "c", 3600);

    assert_eq!(cache.entry_count().expect("count"), 3);

    let purged = cache.clear_all().expect("clear_all");
    assert_eq!(purged, 3);
    assert_eq!(cache.entry_count().expect("count"), 0);
    assert_eq!(cache.total_bytes().expect("total"), 0);
}

#[test]
fn test_clear_all_on_empty_cache_returns_zero() {
    let cache = ContentCache::open_in_memory().expect("open");
    let purged = cache.clear_all().expect("clear_all");
    assert_eq!(purged, 0);
}

#[test]
fn test_clear_all_removes_expired_and_fresh() {
    let cache = ContentCache::open_in_memory().expect("open");
    cache
        .set_cached(&key(1), "expired", true, 200, "text/plain", 0)
        .expect("set");
    insert(&cache, &key(2), "fresh", 3600);

    let purged = cache.clear_all().expect("clear_all");
    assert_eq!(purged, 2);
    assert_eq!(cache.entry_count().expect("count"), 0);
}

#[test]
fn test_clear_expired_then_clear_all() {
    let cache = ContentCache::open_in_memory().expect("open");
    cache
        .set_cached(&key(1), "expired", true, 200, "text/plain", 0)
        .expect("set");
    insert(&cache, &key(2), "fresh1", 3600);
    insert(&cache, &key(3), "fresh2", 3600);

    // First clear expired → 1 removed.
    assert_eq!(cache.clear_expired().expect("clear"), 1);
    assert_eq!(cache.entry_count().expect("count"), 2);

    // Then clear all → 2 removed.
    assert_eq!(cache.clear_all().expect("clear"), 2);
    assert_eq!(cache.entry_count().expect("count"), 0);
}

#[test]
fn test_cache_reusable_after_clear_all() {
    let cache = ContentCache::open_in_memory().expect("open");
    insert(&cache, &key(1), "first", 3600);
    cache.clear_all().expect("clear");

    // Cache should still accept new entries after clearing.
    insert(&cache, &key(2), "second", 3600);
    assert_eq!(cache.entry_count().expect("count"), 1);
    assert_eq!(
        cache.get_cached(&key(2)).expect("get").unwrap().content,
        "second"
    );
}

// ===========================================================================
// WAL mode (file-based cache)
// ===========================================================================

#[test]
fn test_wal_mode_file_based_cache() {
    let tmp = TempDir::new();
    let db_path = tmp.join("cache.db");
    let cache = ContentCache::open(&db_path).expect("open file cache");

    // Query the journal_mode — should be "wal" for a file-based DB.
    let mode = cache.journal_mode().expect("journal_mode");
    assert_eq!(mode.to_lowercase(), "wal");
}

#[test]
fn test_wal_mode_persists_across_reopen() {
    let tmp = TempDir::new();
    let db_path = tmp.join("cache.db");

    // Create and close.
    {
        let cache = ContentCache::open(&db_path).expect("open");
        insert(&cache, &key(1), "persisted", 3600);
    }

    // Reopen — WAL mode should persist and data should survive.
    let cache = ContentCache::open(&db_path).expect("reopen");
    let mode = cache.journal_mode().expect("journal_mode");
    assert_eq!(mode.to_lowercase(), "wal");

    let entry = cache.get_cached(&key(1)).expect("get").expect("entry");
    assert_eq!(entry.content, "persisted");
}

#[test]
fn test_file_cache_round_trip() {
    let tmp = TempDir::new();
    let db_path = tmp.join("cache.db");
    let cache = ContentCache::open(&db_path).expect("open");

    let k = CacheKey::new("https://example.com/doc")
        .with_extraction_type("markdown")
        .with_css_selector(Some("article"));

    cache
        .set_cached(&k, "# Doc", true, 200, "text/markdown", 3600)
        .expect("set");

    let entry = cache.get_cached(&k).expect("get").expect("entry");
    assert_eq!(entry.content, "# Doc");
    assert_eq!(entry.status_code, 200);
}

#[test]
fn test_file_cache_data_persists_across_reopen() {
    let tmp = TempDir::new();
    let db_path = tmp.join("cache.db");

    {
        let cache = ContentCache::open(&db_path).expect("open");
        insert(&cache, &key(1), "data-1", 3600);
        insert(&cache, &key(2), "data-2", 3600);
    }

    let cache = ContentCache::open(&db_path).expect("reopen");
    assert_eq!(cache.entry_count().expect("count"), 2);
    assert_eq!(
        cache.get_cached(&key(1)).expect("get").unwrap().content,
        "data-1"
    );
    assert_eq!(
        cache.get_cached(&key(2)).expect("get").unwrap().content,
        "data-2"
    );
}

#[test]
fn test_file_cache_with_custom_config() {
    let tmp = TempDir::new();
    let db_path = tmp.join("cache_custom.db");
    let config = ContentCacheConfig { max_bytes: 100 };
    let cache = ContentCache::open_with_config(&db_path, config).expect("open");

    insert(&cache, &key(1), "content", 3600);
    assert_eq!(cache.entry_count().expect("count"), 1);
}

// ===========================================================================
// entry_count and total_bytes accounting
// ===========================================================================

#[test]
fn test_entry_count_empty_cache() {
    let cache = ContentCache::open_in_memory().expect("open");
    assert_eq!(cache.entry_count().expect("count"), 0);
}

#[test]
fn test_entry_count_after_inserts() {
    let cache = ContentCache::open_in_memory().expect("open");
    insert(&cache, &key(1), "aaa", 3600);
    insert(&cache, &key(2), "bbb", 3600);
    insert(&cache, &key(3), "ccc", 3600);
    assert_eq!(cache.entry_count().expect("count"), 3);
}

#[test]
fn test_entry_count_after_overwrite() {
    let cache = ContentCache::open_in_memory().expect("open");
    let k = key(1);
    insert(&cache, &k, "old", 3600);
    insert(&cache, &k, "new", 3600);
    assert_eq!(cache.entry_count().expect("count"), 1);
}

#[test]
fn test_total_bytes_empty_cache() {
    let cache = ContentCache::open_in_memory().expect("open");
    assert_eq!(cache.total_bytes().expect("total"), 0);
}

#[test]
fn test_total_bytes_multiple_entries() {
    let cache = ContentCache::open_in_memory().expect("open");
    insert(&cache, &key(1), "aaa", 3600); // 3 bytes
    insert(&cache, &key(2), "bb", 3600); // 2 bytes
    insert(&cache, &key(3), "c", 3600); // 1 byte
    assert_eq!(cache.total_bytes().expect("total"), 6);
}

#[test]
fn test_total_bytes_after_clear_all() {
    let cache = ContentCache::open_in_memory().expect("open");
    insert(&cache, &key(1), "aaa", 3600);
    insert(&cache, &key(2), "bbb", 3600);
    cache.clear_all().expect("clear");
    assert_eq!(cache.total_bytes().expect("total"), 0);
}

// ===========================================================================
// CacheKey builder
// ===========================================================================

#[test]
fn test_cache_key_new_defaults() {
    let k = CacheKey::new("https://example.com");
    assert_eq!(k.url, "https://example.com");
    assert_eq!(k.extraction_type, "");
    assert_eq!(k.css_selector, None);
    assert_eq!(k.pages, None);
}

#[test]
fn test_cache_key_with_extraction_type() {
    let k = CacheKey::new("https://example.com").with_extraction_type("markdown");
    assert_eq!(k.extraction_type, "markdown");
}

#[test]
fn test_cache_key_with_css_selector_some() {
    let k = CacheKey::new("https://example.com").with_css_selector(Some("main"));
    assert_eq!(k.css_selector.as_deref(), Some("main"));
}

#[test]
fn test_cache_key_with_css_selector_none() {
    let k = CacheKey::new("https://example.com").with_css_selector(Some("main"));
    let k = k.with_css_selector(None::<&str>);
    assert_eq!(k.css_selector, None);
}

#[test]
fn test_cache_key_with_pages_some() {
    let k = CacheKey::new("https://example.com").with_pages(Some("1-3"));
    assert_eq!(k.pages.as_deref(), Some("1-3"));
}

#[test]
fn test_cache_key_with_pages_none() {
    let k = CacheKey::new("https://example.com").with_pages(Some("1"));
    let k = k.with_pages(None::<&str>);
    assert_eq!(k.pages, None);
}

#[test]
fn test_cache_key_equality() {
    let k1 = CacheKey::new("https://example.com")
        .with_extraction_type("html")
        .with_css_selector(Some("main"))
        .with_pages(Some("1"));
    let k2 = CacheKey::new("https://example.com")
        .with_extraction_type("html")
        .with_css_selector(Some("main"))
        .with_pages(Some("1"));
    assert_eq!(k1, k2);
}

#[test]
fn test_cache_key_inequality_different_url() {
    let k1 = CacheKey::new("https://a.com");
    let k2 = CacheKey::new("https://b.com");
    assert_ne!(k1, k2);
}

#[test]
fn test_cache_key_clone() {
    let k1 = CacheKey::new("https://example.com").with_extraction_type("markdown");
    let k2 = k1.clone();
    assert_eq!(k1, k2);
}

// ===========================================================================
// Config
// ===========================================================================

#[test]
fn test_config_default() {
    let config = ContentCacheConfig::default();
    assert_eq!(config.max_bytes, DEFAULT_MAX_BYTES);
}

#[test]
fn test_config_custom_max_bytes() {
    let config = ContentCacheConfig { max_bytes: 1024 };
    assert_eq!(config.max_bytes, 1024);
}
