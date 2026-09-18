//! Stream-chunk UTF-8 decoding tests for `append_stream_chunk` (FUNC-033).

use ragent_llm::providers::http_client::append_stream_chunk;

#[test]
fn multibyte_char_split_across_chunks_is_preserved() {
    // 'é' is 0xC3 0xA9 — two bytes. Splitting between them must not corrupt.
    let (mut out, mut pending) = (String::new(), Vec::new());
    append_stream_chunk(&mut out, &mut pending, b"caf");
    append_stream_chunk(&mut out, &mut pending, &[0xC3]);
    assert_eq!(out, "caf");
    assert_eq!(pending, vec![0xC3]);

    append_stream_chunk(&mut out, &mut pending, &[0xA9]);
    assert_eq!(out, "caf\u{e9}");
    assert!(pending.is_empty());
}

#[test]
fn multibyte_char_split_three_ways_across_chunks() {
    // '─' is 0xE2 0x94 0x80 — three bytes.
    let (mut out, mut pending) = (String::new(), Vec::new());
    append_stream_chunk(&mut out, &mut pending, &[0xE2]);
    assert!(out.is_empty());
    append_stream_chunk(&mut out, &mut pending, &[0x94]);
    assert!(out.is_empty());
    append_stream_chunk(&mut out, &mut pending, &[0x80]);
    assert_eq!(out, "\u{2500}");
    assert!(pending.is_empty());
}

#[test]
fn invalid_byte_is_replaced_in_place_and_does_not_stall() {
    // 0xFF is never valid UTF-8.
    let (mut out, mut pending) = (String::new(), Vec::new());
    append_stream_chunk(&mut out, &mut pending, b"ok");
    append_stream_chunk(&mut out, &mut pending, &[0xFF]);
    append_stream_chunk(&mut out, &mut pending, b"tail");
    assert_eq!(out, "ok\u{FFFD}tail");
    assert!(pending.is_empty());
}

#[test]
fn invalid_byte_then_valid_head_is_not_double_corrupted() {
    // Repro of the bug class fixed by driving the flush from
    // `Utf8Error::error_len`: an invalid byte followed by the first byte of a
    // valid 2-byte character must NOT lose the second byte of that character.
    let (mut out, mut pending) = (String::new(), Vec::new());
    // Push 0xFF (invalid) + 0xC3 (head of 'é') + 0xA9 (tail) in a single chunk.
    // The byte-count heuristic sees 3 bytes pending and flushes everything as
    // lossy, corrupting the 'é' into U+FFFD. The error_len-driven flush keeps
    // 0xC3 in pending until 0xA9 arrives.
    append_stream_chunk(&mut out, &mut pending, &[0xFF, 0xC3]);
    append_stream_chunk(&mut out, &mut pending, &[0xA9]);
    assert_eq!(out, "\u{FFFD}\u{e9}");
    assert!(pending.is_empty());
}

#[test]
fn multiple_invalid_bytes_each_emit_one_replacement() {
    // Each ill-formed sequence gets its own U+FFFD (matches W3C/Unicode
    // substitution-of-maximal-subparts behavior).
    let (mut out, mut pending) = (String::new(), Vec::new());
    append_stream_chunk(&mut out, &mut pending, &[0xFF, 0xFE, 0xFF]);
    assert_eq!(out, "\u{FFFD}\u{FFFD}\u{FFFD}");
    assert!(pending.is_empty());
}

#[test]
fn clean_ascii_chunks_stream_unchanged() {
    let (mut out, mut pending) = (String::new(), Vec::new());
    for chunk in [&b"hello"[..], b" ", b"world"] {
        append_stream_chunk(&mut out, &mut pending, chunk);
    }
    assert_eq!(out, "hello world");
    assert!(pending.is_empty());
}
