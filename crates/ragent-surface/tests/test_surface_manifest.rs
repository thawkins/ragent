//! Coverage for `ragent-surface::manifest` (spec `pluginstores` FR-003, FR-048;
//! spec `connectors` FR-044, T-026).
//!
//! Exercises the manifest metadata merge, the GitHub owner/repo split, the
//! manifest-body parser, the bounded URL cache, and the network fetch path
//! (including the non-success, oversized-content-length, and over-cap-body
//! branches) against a minimal in-process HTTP server.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread::JoinHandle;

use ragent_surface::manifest::{
    MAX_MANIFEST_BYTES, ManifestMeta, ManifestUpdate, NetworkManifestSource, cache_get, cache_put,
    fetch_and_parse, github_owner_repo, parse_manifest_meta,
};

/// One canned HTTP/1.1 response served to exactly one connection.
struct Server {
    url: String,
    handle: Option<JoinHandle<()>>,
}

impl Server {
    fn spawn(response: Vec<u8>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let addr = listener.local_addr().expect("local addr");
        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0_u8; 4096];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(&response);
                let _ = stream.flush();
            }
        });
        Self {
            url: format!("http://{addr}/manifest.json"),
            handle: Some(handle),
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn http_response(status: &str, extra_headers: &str, body: &[u8]) -> Vec<u8> {
    let mut out = format!("HTTP/1.1 {status}\r\n");
    out.push_str(extra_headers);
    out.push_str("Connection: close\r\n\r\n");
    let mut bytes = out.into_bytes();
    bytes.extend_from_slice(body);
    bytes
}

#[test]
fn manifest_meta_is_empty_only_when_both_fields_are_absent() {
    let empty = ManifestMeta::default();
    assert!(empty.is_empty());
    assert_eq!(empty.version, None);
    assert_eq!(empty.description, None);

    let version_only = ManifestMeta {
        version: Some("1.2.3".to_string()),
        description: None,
    };
    assert!(!version_only.is_empty());

    let description_only = ManifestMeta {
        version: None,
        description: Some("does things".to_string()),
    };
    assert!(!description_only.is_empty());
}

#[test]
fn manifest_meta_merge_prefers_the_first_non_blank_value() {
    let mut target = ManifestMeta::default();
    target.merge(ManifestMeta {
        version: Some("2.0.0".to_string()),
        description: Some("added".to_string()),
    });
    assert_eq!(target.version.as_deref(), Some("2.0.0"));
    assert_eq!(target.description.as_deref(), Some("added"));

    // A second merge must not overwrite values already present.
    target.merge(ManifestMeta {
        version: Some("9.9.9".to_string()),
        description: Some("later".to_string()),
    });
    assert_eq!(target.version.as_deref(), Some("2.0.0"));
    assert_eq!(target.description.as_deref(), Some("added"));

    // A merge that only fills the gap keeps the existing field.
    let mut partial = ManifestMeta {
        version: Some("1.0.0".to_string()),
        description: None,
    };
    partial.merge(ManifestMeta {
        version: None,
        description: Some("filled".to_string()),
    });
    assert_eq!(partial.version.as_deref(), Some("1.0.0"));
    assert_eq!(partial.description.as_deref(), Some("filled"));
}

#[test]
fn manifest_update_carries_the_id_and_metadata() {
    let update = ManifestUpdate {
        id: "plug-a".to_string(),
        meta: ManifestMeta {
            version: Some("0.1.0".to_string()),
            description: Some("desc".to_string()),
        },
    };
    assert_eq!(update.id, "plug-a");
    assert_eq!(update.meta.version.as_deref(), Some("0.1.0"));
    assert_eq!(update.meta.description.as_deref(), Some("desc"));
}

#[test]
fn github_owner_repo_extracts_owner_and_repo() {
    assert_eq!(
        github_owner_repo("https://github.com/acme/tool"),
        Some(("acme".to_string(), "tool".to_string()))
    );
    assert_eq!(
        github_owner_repo("https://github.com/acme/tool.git"),
        Some(("acme".to_string(), "tool".to_string()))
    );
    assert_eq!(
        github_owner_repo("https://www.github.com/acme/tool/tree/main/sub"),
        Some(("acme".to_string(), "tool".to_string()))
    );
    assert_eq!(github_owner_repo("https://gitlab.com/acme/tool"), None);
    assert_eq!(github_owner_repo("https://github.com/onlyone"), None);
    assert_eq!(github_owner_repo("not a url"), None);
}

#[test]
fn parse_manifest_meta_reads_version_and_description() {
    let bytes = br#"{"version":"3.1.4","description":"a tool"}"#;
    let meta = parse_manifest_meta(bytes);
    assert_eq!(meta.version.as_deref(), Some("3.1.4"));
    assert_eq!(meta.description.as_deref(), Some("a tool"));
}

#[test]
fn parse_manifest_meta_falls_back_through_interface_blocks() {
    let short = br#"{"interface":{"shortDescription":"short text"}}"#;
    assert_eq!(
        parse_manifest_meta(short).description.as_deref(),
        Some("short text")
    );
    let long = br#"{"interface":{"longDescription":"long text"}}"#;
    assert_eq!(
        parse_manifest_meta(long).description.as_deref(),
        Some("long text")
    );
    let iface = br#"{"interface":{"description":"iface text"}}"#;
    assert_eq!(
        parse_manifest_meta(iface).description.as_deref(),
        Some("iface text")
    );
    let top_long = br#"{"longDescription":"top long"}"#;
    assert_eq!(
        parse_manifest_meta(top_long).description.as_deref(),
        Some("top long")
    );
    // The top-level description wins over the interface block.
    let both = br#"{"description":"top","interface":{"shortDescription":"iface"}}"#;
    assert_eq!(
        parse_manifest_meta(both).description.as_deref(),
        Some("top")
    );
}

#[test]
fn parse_manifest_meta_treats_blank_and_wrong_typed_fields_as_absent() {
    let blank = br#"{"version":"   ","description":"  "}"#;
    let meta = parse_manifest_meta(blank);
    assert_eq!(meta.version, None);
    assert_eq!(meta.description, None);

    let wrong = br#"{"version":42,"description":[1,2]}"#;
    let meta = parse_manifest_meta(wrong);
    assert_eq!(meta.version, None);
    assert_eq!(meta.description, None);

    let malformed = b"{ this is not json";
    assert_eq!(parse_manifest_meta(malformed), ManifestMeta::default());
}

#[test]
fn manifest_cache_round_trips_hits_and_negative_entries() {
    let hit_url = "https://example.test/cache-hit.json";
    let negative_url = "https://example.test/cache-miss.json";
    let unknown_url = "https://example.test/cache-unknown.json";

    cache_put(
        hit_url.to_string(),
        Some(ManifestMeta {
            version: Some("1.0.0".to_string()),
            description: None,
        }),
    );
    assert_eq!(
        cache_get(hit_url).and_then(|m| m.version).as_deref(),
        Some("1.0.0")
    );

    // A negative result is cached as an empty metadata value.
    cache_put(negative_url.to_string(), None);
    assert_eq!(cache_get(negative_url), Some(ManifestMeta::default()));

    assert_eq!(cache_get(unknown_url), None);
}

#[test]
fn network_manifest_source_builds_a_usable_client() {
    let source = NetworkManifestSource::new();
    let client = source.client().expect("client builds");
    let server = Server::spawn(http_response(
        "200 OK",
        "Content-Type: application/json\r\n",
        br#"{"version":"9.8.7","description":"net"}"#,
    ));
    let meta = fetch_and_parse(client, &server.url).expect("fetch succeeds");
    assert_eq!(meta.version.as_deref(), Some("9.8.7"));
    assert_eq!(meta.description.as_deref(), Some("net"));
}

#[test]
fn fetch_and_parse_returns_none_on_a_non_success_status() {
    let client = reqwest::blocking::Client::builder()
        .build()
        .expect("client");
    let server = Server::spawn(http_response("404 Not Found", "", b"missing"));
    assert_eq!(fetch_and_parse(&client, &server.url), None);
}

#[test]
fn fetch_and_parse_rejects_an_oversized_content_length() {
    let client = reqwest::blocking::Client::builder()
        .build()
        .expect("client");
    let big = (MAX_MANIFEST_BYTES + 1).to_string();
    let headers = format!("Content-Length: {big}\r\n");
    let server = Server::spawn(http_response("200 OK", &headers, b"{}"));
    assert_eq!(fetch_and_parse(&client, &server.url), None);
}

#[test]
fn fetch_and_parse_rejects_a_body_that_exceeds_the_cap() {
    let client = reqwest::blocking::Client::builder()
        .build()
        .expect("client");
    // No Content-Length: the length is discovered only while reading, so the
    // capped reader must refuse the over-cap body.
    let body = vec![b'x'; (MAX_MANIFEST_BYTES + 1024) as usize];
    let server = Server::spawn(http_response("200 OK", "", &body));
    assert_eq!(fetch_and_parse(&client, &server.url), None);
}
