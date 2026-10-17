//! Coverage for the small utility surfaces that the TUI and tool crates
//! consume: the typed id newtypes ([`ragent_types::id`]), the embedding
//! serialisation helpers ([`ragent_types::embedding`]), and the remaining
//! `strutil` formatters (`is_http_url`, `path_file_name`, `bearer_auth_header`,
//! `short_prefix`, `short_suffix`, `format_duration_compact`,
//! `format_duration_hms`, `format_mmss`).

use ragent_types::embedding::{deserialise_embedding, serialise_embedding};
use ragent_types::id::{EventId, MessageId, ProviderId, RunId, SessionId, ToolCallId};
use ragent_types::strutil::{
    bearer_auth_header, format_duration_compact, format_duration_hms, format_mmss, is_http_url,
    path_file_name, short_prefix, short_suffix,
};

#[test]
fn id_newtypes_construct_generate_and_render() {
    let generated = SessionId::new();
    assert_ne!(generated.as_str(), "");
    assert_eq!(generated.to_string(), generated.as_str());
    assert_eq!(generated.as_ref(), generated.as_str());

    let from_str = MessageId::from("msg-42");
    assert_eq!(from_str.as_str(), "msg-42");
    let from_string = MessageId::from("msg-42".to_string());
    assert_eq!(from_str, from_string);

    // Default generates a fresh id; two defaults are never equal.
    let a = ProviderId::default();
    let b = ProviderId::default();
    assert_ne!(a, b);

    // Every wrapper exposes the same constructor surface.
    assert_ne!(ToolCallId::new().as_str(), "");
    assert_ne!(RunId::new().as_str(), "");
    assert_ne!(EventId::new().as_str(), "");
}

#[test]
fn id_newtypes_hash_and_round_trip_through_serde() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(RunId::from("run-1"));
    set.insert(RunId::from("run-1"));
    set.insert(RunId::from("run-2"));
    assert_eq!(set.len(), 2);

    let id = RunId::from("run-9");
    let json = serde_json::to_string(&id).expect("serialize");
    assert_eq!(json, "\"run-9\"");
    let back: RunId = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, id);
}

#[test]
fn embedding_round_trips_and_preserves_order() {
    let values = vec![0.0_f32, -1.5, 3.25, f32::MAX, f32::MIN];
    let blob = serialise_embedding(&values);
    assert_eq!(blob.len(), values.len() * 4);
    let recovered = deserialise_embedding(&blob, values.len()).expect("deserialize");
    assert_eq!(recovered, values);
}

#[test]
fn embedding_rejects_a_dimension_mismatch() {
    let blob = serialise_embedding(&[1.0_f32, 2.0]);
    let err = deserialise_embedding(&blob, 3).expect_err("wrong dimensions must fail");
    assert!(err.to_string().contains("does not match"));
}

#[test]
fn embedding_rejects_a_non_multiple_of_four_blob() {
    let err = deserialise_embedding(&[0u8, 1, 2], 0).expect_err("3-byte blob with 0 dims fails");
    assert!(err.to_string().contains("does not match"));
}

#[test]
fn embedding_of_an_empty_vector_is_empty() {
    let blob = serialise_embedding(&[]);
    assert_eq!(blob.len(), 0);
    let recovered = deserialise_embedding(&blob, 0).expect("empty round-trips");
    assert_eq!(recovered.len(), 0);
}

#[test]
fn is_http_url_accepts_only_http_and_https() {
    assert!(is_http_url("http://example.com"));
    assert!(is_http_url("https://example.com/x?y=1"));
    assert!(!is_http_url("ftp://example.com"));
    assert!(!is_http_url("file:///tmp/x"));
    assert!(!is_http_url("HTTP://example.com"));
    assert!(!is_http_url(""));
    assert!(!is_http_url("example.com"));
}

#[test]
fn path_file_name_yields_the_leaf_or_empty() {
    assert_eq!(path_file_name(std::path::Path::new("/a/b/c.rs")), "c.rs");
    assert_eq!(path_file_name(std::path::Path::new("leaf.txt")), "leaf.txt");
    // A path ending in `..` has no file name component.
    assert_eq!(path_file_name(std::path::Path::new("/a/..")), "");
    assert_eq!(path_file_name(std::path::Path::new("/")), "");
}

#[test]
fn bearer_auth_header_builds_the_exact_header_value() {
    assert_eq!(bearer_auth_header("abc123"), "Bearer abc123");
    assert_eq!(bearer_auth_header(""), "Bearer ");
}

#[test]
fn short_prefix_and_suffix_handle_multi_byte_and_short_inputs() {
    assert_eq!(short_prefix("abcdefghij", 8), "abcdefgh");
    assert_eq!(short_prefix("abc", 8), "abc");
    // Multi-byte: slicing by characters never panics.
    assert_eq!(short_prefix("\u{e9}\u{e9}\u{e9}", 2), "\u{e9}\u{e9}");

    assert_eq!(short_suffix("abcdefghij", 8), "cdefghij");
    assert_eq!(short_suffix("abc", 8), "abc");
    assert_eq!(short_suffix("\u{e9}\u{e9}\u{e9}", 2), "\u{e9}\u{e9}");
}

#[test]
fn format_duration_compact_covers_all_three_bands() {
    assert_eq!(format_duration_compact(0), "0s");
    assert_eq!(format_duration_compact(34), "34s");
    assert_eq!(format_duration_compact(59), "59s");
    assert_eq!(format_duration_compact(154), "2m34s");
    assert_eq!(format_duration_compact(3599), "59m59s");
    assert_eq!(format_duration_compact(3900), "1h5m");
    // Negative values clamp to zero rather than rendering a sign.
    assert_eq!(format_duration_compact(-5), "0s");
}

#[test]
fn format_duration_hms_keeps_all_units_with_spaces() {
    assert_eq!(format_duration_hms(5), "5s");
    assert_eq!(format_duration_hms(154), "2m 34s");
    assert_eq!(format_duration_hms(3900), "1h 5m 0s");
    assert_eq!(format_duration_hms(-1), "0s");
}

#[test]
fn format_mmss_zero_pads_the_seconds() {
    assert_eq!(format_mmss(0), "0:00");
    assert_eq!(format_mmss(5), "0:05");
    assert_eq!(format_mmss(154), "2:34");
    assert_eq!(format_mmss(3661), "61:01");
}
