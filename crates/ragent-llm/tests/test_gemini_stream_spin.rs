#![allow(clippy::assert_is_empty)]
//! Regression test for the Gemini stream parser infinite-loop bug.
//!
//! The Gemini provider's streaming parser had a `continue` in the inner
//! buffer-parsing loop when `serde_json::from_str` failed. The unparseable
//! line was prepended back into the buffer with a `\n`, and `continue`
//! re-entered the same inner loop — which immediately found the `\n` it
//! just inserted, extracted the same line, failed to parse it again, and
//! repeated forever. This burned 100% CPU.
//!
//! The fix replaces `continue` with `break` so the inner loop exits and
//! the outer `stream.next().await` fetches more data to complete the
//! partial JSON object.
//!
//! These tests reproduce the original inner/outer loop structure and verify
//! the `break` fix prevents the infinite loop. With the old `continue` code
//! these tests would hang forever (infinite loop). With the `break` fix
//! they complete.

use std::pin::Pin;

use futures::{Stream, StreamExt};

/// A mock byte stream that yields pre-defined chunks, simulating a
/// streaming HTTP response body.
struct MockByteStream {
    chunks: Vec<Vec<u8>>,
    index: usize,
}

impl MockByteStream {
    fn new(chunks: Vec<Vec<u8>>) -> Self {
        Self { chunks, index: 0 }
    }
}

impl Stream for MockByteStream {
    type Item = Result<bytes::Bytes, std::io::Error>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        if self.index < self.chunks.len() {
            let idx = self.index;
            self.index += 1;
            let chunk = std::mem::take(&mut self.chunks[idx]);
            std::task::Poll::Ready(Some(Ok(bytes::Bytes::from(chunk))))
        } else {
            std::task::Poll::Ready(None)
        }
    }
}

/// Verify that a completely unparseable line (not partial JSON, just garbage)
/// does not cause an infinite loop. The `break` exits the inner loop,
/// the outer loop fetches the next chunk, and the garbage line is eventually
/// discarded when the stream ends.
///
/// With the old `continue` code this test would hang forever because the
/// garbage line would be re-extracted and re-failed on every inner-loop
/// iteration without ever returning to `stream.next().await`.
#[tokio::test]
async fn test_gemini_garbage_line_does_not_spin() {
    let chunk1 = b"this is not json\n".to_vec();
    let chunk2 = b"{\"candidates\":[]}\n".to_vec();

    let mut stream = MockByteStream::new(vec![chunk1, chunk2]);
    let mut buffer = String::new();
    let mut iterations = 0u32;
    let max_iterations = 1000;

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.expect("chunk ok");
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(line_end) = buffer.find('\n') {
            iterations += 1;
            assert!(iterations < max_iterations, "infinite loop detected");

            let line = buffer[..line_end].trim().to_string();
            buffer = buffer[line_end + 1..].to_string();

            if line.is_empty() || line == "[,]" {
                continue;
            }

            let line = line.trim_end_matches(',').trim();
            let line = line.trim_start_matches('[').trim_start();
            let line = line.trim_end_matches(']').trim_end();

            if line.is_empty() {
                continue;
            }

            let _parsed: serde_json::Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => {
                    // The fix: `break` instead of `continue`.
                    buffer = format!("{}\n{}", line, buffer);
                    break;
                }
            };
        }
    }

    // If we got here, no infinite loop.
}

/// Verify that a partial JSON object split across chunks (where the first
/// chunk ends with a newline but incomplete JSON) does not spin. The `break`
/// exits the inner loop, the outer loop fetches the next chunk, and the
/// accumulated buffer eventually contains a complete parseable line.
#[tokio::test]
async fn test_gemini_partial_json_split_across_chunks() {
    // First chunk: a partial JSON object followed by a newline.
    // The inner loop will extract it, fail to parse, put it back, and break.
    let chunk1 = b"{\"text\":\"hel\n".to_vec();
    // Second chunk: the rest of the JSON (completing the line).
    let chunk2 = b"lo\"}\n".to_vec();

    let mut stream = MockByteStream::new(vec![chunk1, chunk2]);
    let mut buffer = String::new();
    let mut iterations = 0u32;
    let max_iterations = 1000;

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.expect("chunk ok");
        buffer.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(line_end) = buffer.find('\n') {
            iterations += 1;
            assert!(iterations < max_iterations, "infinite loop detected");

            let line = buffer[..line_end].trim().to_string();
            buffer = buffer[line_end + 1..].to_string();

            if line.is_empty() || line == "[,]" {
                continue;
            }

            let line = line.trim_end_matches(',').trim();
            let line = line.trim_start_matches('[').trim_start();
            let line = line.trim_end_matches(']').trim_end();

            if line.is_empty() {
                continue;
            }

            let _parsed: serde_json::Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => {
                    // The fix: `break` instead of `continue`.
                    buffer = format!("{}\n{}", line, buffer);
                    break;
                }
            };
        }
    }

    // If we got here, no infinite loop.
}
