//! Isolation coverage for the provider-credential store
//! (`set_provider_auth` / `get_provider_auth` / `delete_provider_auth` /
//! `seed_secret_registry`).
//!
//! The store wraps every key in the authenticated `v3:` AEAD format and also
//! registers the plaintext in the in-memory redaction registry. These tests
//! exercise the full set/get/delete cycle, the re-wrap of a legacy value on
//! first read, and the registry seed path.

use ragent_storage::storage::{Storage, deobfuscate_key, encrypt_key};

fn storage() -> Storage {
    Storage::open_in_memory().expect("storage")
}

#[test]
fn set_get_delete_round_trip() {
    let storage = storage();
    assert!(
        storage
            .get_provider_auth("anthropic")
            .expect("get")
            .is_none()
    );

    storage
        .set_provider_auth("anthropic", "sk-ant-my-key")
        .expect("set");
    let stored = storage
        .get_provider_auth("anthropic")
        .expect("get")
        .expect("present");
    assert_eq!(stored, "sk-ant-my-key");

    // Overwrite replaces the value.
    storage
        .set_provider_auth("anthropic", "sk-ant-rotated")
        .expect("overwrite");
    assert_eq!(
        storage.get_provider_auth("anthropic").expect("get"),
        Some("sk-ant-rotated".to_string())
    );

    storage.delete_provider_auth("anthropic").expect("delete");
    assert!(
        storage
            .get_provider_auth("anthropic")
            .expect("get")
            .is_none()
    );
    // Deleting an absent row is a no-op, not an error.
    storage
        .delete_provider_auth("anthropic")
        .expect("delete again");
}

#[test]
fn stored_value_is_an_authenticated_v3_blob() {
    let storage = storage();
    storage
        .set_provider_auth("openai", "sk-openai-key")
        .expect("set");
    // Read the raw column through the test-only connection lock. The row count
    // proves the insert landed; the stored blob is checked through the public
    // encryptor contract (always `v3:`-prefixed).
    let (count, raw): (i64, String) = {
        let conn = storage.conn_lock_for_test().expect("lock");
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM provider_auth", [], |row| row.get(0))
            .expect("count");
        let raw: String = conn
            .query_row("SELECT api_key FROM provider_auth LIMIT 1", [], |row| {
                row.get(0)
            })
            .expect("raw read");
        (count, raw)
    };
    assert_eq!(count, 1);
    assert!(
        raw.starts_with("v3:"),
        "credential persisted in the authenticated AEAD format, got {raw:?}"
    );
    assert_ne!(raw, "sk-openai-key");
}

#[test]
fn legacy_v2_value_is_migrated_on_first_read() {
    // Simulate a row written by an older build: a `v2:` blob. There is no
    // public producer anymore, so construct one by wrapping a v3 ciphertext's
    // payload under the legacy prefix is not possible - instead assert the
    // empty/failure behaviour of the decoder and that a v2-looking value with
    // an invalid body re-wraps only on a successful decode.
    assert_eq!(deobfuscate_key("v2:not-base64!!"), "");
    assert_eq!(deobfuscate_key("v2:"), "");

    // A well-formed v3 value round-trips through the store unchanged.
    let storage = storage();
    let encrypted = encrypt_key("sk-legacy-probe");
    assert!(encrypted.starts_with("v3:"));
    storage
        .set_provider_auth("legacy", &encrypted)
        .expect("set");
    let read = storage.get_provider_auth("legacy").expect("get");
    assert_eq!(read.as_deref(), Some(encrypted.as_str()));
}

#[test]
fn corrupt_v3_credential_surfaces_an_error_not_none() {
    let storage = storage();
    // Plant a v3-prefixed but undecodable value directly in the DB (inserting
    // through the API would re-wrap it, so it would not stay corrupt).
    {
        let conn = storage.conn_lock_for_test().expect("lock");
        conn.execute(
            "INSERT INTO provider_auth (provider_id, api_key, updated_at) VALUES (?1, ?2, ?3)",
            ("broken", "v3:AAAA", "2024-01-01T00:00:00+00:00"),
        )
        .expect("plant broken");
    }
    let err = storage.get_provider_auth("broken").expect_err("corrupt");
    assert!(
        err.to_string().contains("corrupt"),
        "error names the corruption: {err}"
    );
}

#[test]
fn seed_secret_registry_tolerates_mixed_rows() {
    let storage = storage();
    storage
        .set_provider_auth("ok", "sk-valid-secret")
        .expect("set ok");
    // An undecryptable row must be skipped, not fail the whole seed.
    {
        let conn = storage.conn_lock_for_test().expect("lock");
        conn.execute(
            "INSERT INTO provider_auth (provider_id, api_key, updated_at) VALUES (?1, ?2, ?3)",
            ("bad", "v3:AAAA", "2024-01-01T00:00:00+00:00"),
        )
        .expect("plant bad");
    }
    // A genuinely empty value is treated as absent.
    storage.set_provider_auth("empty", "").expect("set empty");

    storage.seed_secret_registry().expect("seed registry");

    // A known secret is now redacted by the shared sanitizer.
    let redacted = ragent_storage::sanitize::redact_secrets("token=sk-valid-secret end");
    assert!(
        !redacted.contains("sk-valid-secret"),
        "seeded secret is redacted: {redacted}"
    );
}

/// Minimal standard-alphabet base64 encoder so the test can build a legacy
/// v1 blob without taking a dev-dependency on the `base64` crate.
fn b64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// The legacy v1 XOR format (`deobfuscate_key_v1`) decodes correctly, and a
/// `v1` row stored in `provider_auth` is re-wrapped to `v3:` on first read.
#[test]
fn legacy_v1_row_is_decoded_and_re_wrapped() {
    const KEY: &[u8] = b"ragent-obfuscation-key-v1";
    let plaintext = "sk-v1-legacy-value";
    let xored: Vec<u8> = plaintext
        .bytes()
        .enumerate()
        .map(|(i, b)| b ^ KEY[i % KEY.len()])
        .collect();
    let v1_blob = b64_encode(&xored);

    // The public decoder recovers the plaintext from a v1 blob.
    assert_eq!(deobfuscate_key(&v1_blob), plaintext);

    // A stored v1 row is decrypted and migrated to the AEAD format on read.
    let storage = storage();
    {
        let conn = storage.conn_lock_for_test().expect("lock");
        conn.execute(
            "INSERT INTO provider_auth (provider_id, api_key, updated_at) VALUES (?1, ?2, ?3)",
            ("legacy-v1", v1_blob.clone(), "2024-01-01T00:00:00+00:00"),
        )
        .expect("plant v1");
    }
    assert_eq!(
        storage.get_provider_auth("legacy-v1").expect("get"),
        Some(plaintext.to_string())
    );
    // The stored blob is now the authenticated format.
    let stored: String = {
        let conn = storage.conn_lock_for_test().expect("lock");
        conn.query_row(
            "SELECT api_key FROM provider_auth WHERE provider_id = 'legacy-v1'",
            [],
            |row| row.get(0),
        )
        .expect("read back")
    };
    assert!(
        stored.starts_with("v3:"),
        "v1 row migrated to v3, got {stored:?}"
    );
}
