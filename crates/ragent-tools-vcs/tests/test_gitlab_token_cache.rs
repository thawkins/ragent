//! PERF-059: the resolved GitLab PAT must be read and decrypted from the
//! credential store once, not on every request.
//!
//! These tests drive `gitlab::auth::load_token` through a counting
//! `StorageBackend` and assert the credential store is consulted exactly once
//! for repeated calls, and again after `save_token` invalidates the cache.
//!
//! The test is skipped when the ambient environment or config already supplies
//! a GitLab token, because those layers take precedence over the store and
//! would make the store-call count meaningless.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use ragent_tools_vcs::gitlab::auth::{load_token, save_token};
use ragent_tools_vcs::storage::StorageBackend;

/// A `StorageBackend` that returns a canned token and counts credential reads.
struct CountingStorage {
    token: Mutex<Option<String>>,
    get_calls: AtomicUsize,
}

impl CountingStorage {
    fn new(token: &str) -> Self {
        Self {
            token: Mutex::new(Some(token.to_string())),
            get_calls: AtomicUsize::new(0),
        }
    }

    fn get_calls(&self) -> usize {
        self.get_calls.load(Ordering::SeqCst)
    }
}

impl StorageBackend for CountingStorage {
    fn get_provider_auth(&self, _provider_id: &str) -> anyhow::Result<Option<String>> {
        self.get_calls.fetch_add(1, Ordering::SeqCst);
        Ok(self
            .token
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    }

    fn set_provider_auth(&self, _provider_id: &str, api_key: &str) -> anyhow::Result<()> {
        *self
            .token
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(api_key.to_string());
        Ok(())
    }

    fn delete_provider_auth(&self, _provider_id: &str) -> anyhow::Result<()> {
        *self
            .token
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        Ok(())
    }

    fn get_setting(&self, _key: &str) -> anyhow::Result<Option<String>> {
        Ok(None)
    }

    fn set_setting(&self, _key: &str, _value: &str) -> anyhow::Result<()> {
        Ok(())
    }

    fn delete_setting(&self, _key: &str) -> anyhow::Result<()> {
        Ok(())
    }
}

/// The GitLab token cache is process-global, so these tests must not interleave.
static TEST_LOCK: Mutex<()> = Mutex::new(());

fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Whether an ambient token layer (env or config) would shadow the store.
fn ambient_token_present() -> bool {
    std::env::var("GITLAB_TOKEN").is_ok_and(|t| !t.is_empty())
}

#[test]
fn test_token_read_once_per_storage_handle() {
    let _lock = test_lock();
    if ambient_token_present() {
        eprintln!("skipping: GITLAB_TOKEN is set in the ambient environment");
        return;
    }

    let token = format!("glpat-counting-{}", std::process::id());
    let store = CountingStorage::new(&token);

    // Seed through `save_token` so the cache is provably empty at the start,
    // regardless of whether a previous test's storage handle occupied this
    // address (which would otherwise alias the process-global cache key).
    save_token(&store, &token).expect("seed");
    assert_eq!(store.get_calls(), 0);

    // First call populates the cache from the store...
    for _ in 0..5 {
        let resolved = load_token(&store);
        assert_eq!(resolved.as_deref(), Some(token.as_str()));
    }

    // ...and the remaining four calls are served from the cache.
    assert_eq!(
        store.get_calls(),
        1,
        "the credential store must be read exactly once for repeated loads"
    );
}

#[test]
fn test_save_token_invalidates_cache() {
    let _lock = test_lock();
    if ambient_token_present() {
        eprintln!("skipping: GITLAB_TOKEN is set in the ambient environment");
        return;
    }

    let store = CountingStorage::new("glpat-original");
    save_token(&store, "glpat-original").expect("seed");
    assert_eq!(load_token(&store).as_deref(), Some("glpat-original"));
    assert_eq!(load_token(&store).as_deref(), Some("glpat-original"));
    assert_eq!(store.get_calls(), 1);

    save_token(&store, "glpat-rotated").expect("save");

    let reads_after_save = store.get_calls();
    assert_eq!(
        load_token(&store).as_deref(),
        Some("glpat-rotated"),
        "a saved token must be visible immediately"
    );
    assert_eq!(
        store.get_calls(),
        reads_after_save + 1,
        "saving must invalidate the cache so the new token is re-read"
    );
}
