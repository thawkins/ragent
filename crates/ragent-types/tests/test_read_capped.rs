//! The shared bounded-read rule (`ragent_types::read_capped`).
//!
//! Every caller that fetches a remote catalogue or manifest into memory keeps
//! its own error type but shares this ceiling logic, so these tests pin the
//! rule once: a body at the ceiling is accepted, one byte past it is refused
//! without buffering, and a reader failure is reported as a read failure rather
//! than as "too large" (a retryable transport error must not be mistaken for a
//! policy refusal).

use std::io::{Cursor, Read};

use ragent_types::read_capped::{CappedError, READ_CHUNK_BYTES, read_capped};

#[test]
fn test_body_under_the_ceiling_is_returned_whole() {
    let body = vec![7_u8; 100];
    let bytes = read_capped(Cursor::new(body.clone()), 1000).expect("under the ceiling");
    assert_eq!(bytes, body);
}

#[test]
fn test_body_exactly_at_the_ceiling_is_accepted() {
    let body = vec![1_u8; 50];
    let bytes = read_capped(Cursor::new(body.clone()), 50).expect("at the ceiling is allowed");
    assert_eq!(bytes, body);
}

#[test]
fn test_body_one_byte_past_the_ceiling_is_refused() {
    let err = read_capped(Cursor::new(vec![1_u8; 51]), 50).expect_err("one past must fail");
    assert!(
        matches!(err, CappedError::TooLarge { limit: 50 }),
        "{err:?}"
    );
    // The message names the ceiling so a log line is actionable.
    assert!(err.to_string().contains("50"), "{err}");
}

#[test]
fn test_an_empty_body_is_accepted_even_with_a_zero_ceiling() {
    assert_eq!(
        read_capped(Cursor::new(Vec::new()), 0).expect("empty body"),
        Vec::<u8>::new()
    );
}

#[test]
fn test_a_body_larger_than_one_chunk_is_read_across_chunks() {
    // The reader is drained in `READ_CHUNK_BYTES` slices, so a body spanning
    // several chunks must still arrive byte-exact.
    let size = READ_CHUNK_BYTES * 2 + 17;
    let body: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
    let bytes = read_capped(Cursor::new(body.clone()), size as u64).expect("multi-chunk body");
    assert_eq!(bytes, body);
}

#[test]
fn test_a_reader_failure_is_reported_as_read_not_too_large() {
    /// A reader that fails after `ok_bytes` good bytes.
    struct FailsAfter {
        remaining: usize,
    }
    impl Read for FailsAfter {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.remaining == 0 {
                return Err(std::io::Error::other("connection reset"));
            }
            let n = self.remaining.min(buf.len());
            buf[..n].fill(b'x');
            self.remaining -= n;
            Ok(n)
        }
    }

    let err = read_capped(FailsAfter { remaining: 10 }, 1024).expect_err("reader fails");
    match &err {
        CappedError::Read(message) => {
            assert!(message.contains("connection reset"), "{message}");
        }
        CappedError::TooLarge { .. } => {
            panic!("a reader failure must not be described as an oversize body: {err:?}");
        }
    }
}

#[test]
fn test_map_err_projects_each_variant_into_the_caller_type() {
    // The error type stays per-crate; only the classification is shared.
    let oversize = read_capped(Cursor::new(vec![0_u8; 4]), 3).expect_err("oversize");
    let projected: String = oversize.map_err(
        |message| format!("read: {message}"),
        |limit| format!("too large: {limit}"),
    );
    assert_eq!(projected, "too large: 3");

    struct Fails;
    impl Read for Fails {
        fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("boom"))
        }
    }
    let read_err = read_capped(Fails, 3).expect_err("read failure");
    let projected: String = read_err.map_err(
        |message| format!("read: {message}"),
        |limit| format!("too large: {limit}"),
    );
    assert_eq!(projected, "read: boom");
}
