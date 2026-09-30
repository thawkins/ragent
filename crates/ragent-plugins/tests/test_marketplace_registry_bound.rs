//! Tests for the bounded marketplace-document registry (ANTIPAT L7):
//! `record_document` must not grow the process-global registry without bound,
//! and eviction must be oldest-first so a freshly recorded document (the one an
//! install is about to look up) is never the one dropped.

use ragent_plugins::marketplace::{document_key, lookup, record_document};

/// A minimal marketplace document that carries an inline `lspServers` section
/// for the plugin name `plugin`.
fn document_for(plugin: &str) -> String {
    format!(r#"{{"plugins":[{{"name":"{plugin}","lspServers":{{"x":{{"command":"x"}}}}}}]}}"#)
}

#[test]
fn registry_evicts_oldest_document_beyond_the_cap() {
    // Record far more documents than the registry cap. The oldest must be
    // evicted, and the most recent must still resolve.
    let total = 100usize;
    let mut keys = Vec::with_capacity(total);
    for i in 0..total {
        let origin = format!("https://example.org/marketplace-{i}.json");
        let doc = document_for(&format!("plugin-{i}"));
        keys.push((document_key(&origin, doc.as_bytes()), i));
        record_document(&origin, doc.as_bytes());
    }

    // The newest document (recorded last) is always retained.
    let (newest_key, newest_index) = keys.last().expect("at least one document");
    assert!(
        lookup(newest_key, &format!("plugin-{newest_index}")).is_some(),
        "the most recently recorded document must still answer"
    );

    // The oldest document must have been evicted (its key no longer answers).
    let (oldest_key, oldest_index) = &keys[0];
    assert!(
        lookup(oldest_key, &format!("plugin-{oldest_index}")).is_none(),
        "the oldest document must be evicted once the cap is exceeded"
    );
}

#[test]
fn re_recording_the_same_key_does_not_evict_it() {
    let origin = "https://example.org/stable.json";
    let doc = document_for("stable-plugin");
    let key = document_key(origin, doc.as_bytes());

    // Fill the registry with unrelated documents, then re-record the stable
    // key repeatedly. Refreshing an existing key must not grow the registry,
    // so the stable document survives and stays resolvable.
    for i in 0..100 {
        let other = format!("https://example.org/other-{i}.json");
        let other_doc = document_for(&format!("other-{i}"));
        record_document(&other, other_doc.as_bytes());
    }
    for _ in 0..5 {
        record_document(origin, doc.as_bytes());
    }

    assert!(
        lookup(&key, "stable-plugin").is_some(),
        "re-recording an existing key must keep it resolvable"
    );
}
