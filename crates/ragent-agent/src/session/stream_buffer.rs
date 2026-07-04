//! Stream-buffer and stall-detection helpers for the agent loop.
//!
//! [`StreamBuffer`] coalesces small text/reasoning deltas from the LLM stream
//! and flushes them to the event bus in batches, reducing per-token allocation
//! and channel-send overhead. The stall-detection [`RegexSet`] catches Ollama
//! "planning" phrases so the agent loop can nudge the model into acting.

use std::time::Instant;

use regex::RegexSet;
use tokio::time::Duration;

/// Maximum accumulated text/reasoning delta characters before forcing a flush.
pub const STREAM_BUFFER_SIZE_THRESHOLD: usize = 256;
/// Maximum milliseconds between stream event flushes.
pub const STREAM_BUFFER_FLUSH_MS: u64 = 50;

/// Pre-compiled regex set for Ollama stall detection (planning phrases).
/// Built once on first use to avoid re-scanning with 12 literal `.contains()`
/// calls every step.
static STALL_PATTERN_SET: std::sync::OnceLock<RegexSet> = std::sync::OnceLock::new();

/// Returns a reference to the lazily-initialised stall-detection [`RegexSet`].
pub fn stall_pattern_set() -> &'static RegexSet {
    STALL_PATTERN_SET.get_or_init(|| {
        RegexSet::new([
            r"Let me",
            r"I'll",
            r"I will",
            r"I'm going to",
            r"let me",
            r"start by",
            r"begin by",
            r"First,",
            r"First I",
            r"exploring",
            r"examine",
            r"analyze",
        ])
        .expect("stall patterns are valid regex")
    })
}

/// Buffers incoming text and reasoning deltas from the LLM stream and
/// flushes them to the event bus in batches. This reduces per-token
/// allocation and channel-send overhead by coalescing small deltas.
///
/// Tool-call events (`ToolCallStart`, `ToolCallEnd`) are forwarded
/// immediately so that sequencing is preserved.
pub(crate) struct StreamBuffer {
    text: String,
    reasoning: String,
    flush_size: usize,
    flush_interval: Duration,
    last_flush: Instant,
}

impl StreamBuffer {
    /// Create a new buffer with the default flush thresholds.
    pub(crate) fn new() -> Self {
        Self {
            text: String::new(),
            reasoning: String::new(),
            flush_size: STREAM_BUFFER_SIZE_THRESHOLD,
            flush_interval: Duration::from_millis(STREAM_BUFFER_FLUSH_MS),
            last_flush: Instant::now(),
        }
    }

    /// Append a text delta. Returns `Some(text)` if a flush is needed.
    pub(crate) fn push_text(&mut self, text: &str) -> Option<String> {
        self.text.push_str(text);
        if self.should_flush() {
            Some(self.drain_text())
        } else {
            None
        }
    }

    /// Append a reasoning delta. Returns `Some(reasoning)` if a flush is needed.
    pub(crate) fn push_reasoning(&mut self, text: &str) -> Option<String> {
        self.reasoning.push_str(text);
        if self.should_flush() {
            Some(self.drain_reasoning())
        } else {
            None
        }
    }

    /// Drain any remaining buffered text.
    pub(crate) fn drain_text(&mut self) -> String {
        std::mem::take(&mut self.text)
    }

    /// Drain any remaining buffered reasoning text.
    pub(crate) fn drain_reasoning(&mut self) -> String {
        std::mem::take(&mut self.reasoning)
    }

    fn should_flush(&self) -> bool {
        self.text.len() >= self.flush_size
            || self.reasoning.len() >= self.flush_size
            || self.last_flush.elapsed() >= self.flush_interval
    }

    /// Reset the flush timer after an explicit flush.
    pub(crate) fn reset_timer(&mut self) {
        self.last_flush = Instant::now();
    }
}
