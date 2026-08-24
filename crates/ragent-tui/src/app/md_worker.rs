//! Long-lived markdown-rendering worker thread (FR-010, FR-002).
//!
//! The TUI's `render_markdown_pipeline` converts markdown to HTML via
//! `pulldown-cmark` and then HTML to plain text via `html2text`.  The
//! `html2text` step may panic on malformed HTML (word-wrapper subtraction
//! overflow), so it must run on a dedicated thread — never the UI thread.
//!
//! Previously, every cache miss spawned a **new** OS thread (`std::thread::
//! Builder::spawn` + `join`), which is expensive during streaming: each
//! `TextDelta` event produces a different accumulated text, misses the
//! cache, and spawns a thread.  This module replaces that pattern with a
//! single long-lived worker thread that receives rendering requests via an
//! `mpsc` channel and returns results via a `oneshot` channel, eliminating
//! per-call thread creation overhead (FR-010) while keeping the computation
//! off the UI thread (FR-002).

use std::sync::mpsc;

/// A request to render HTML to plain text.
struct MdRequest {
    /// The HTML input to convert.
    html: String,
    /// Sender for the rendered plain-text result.
    response_tx: mpsc::Sender<Result<String, String>>,
}

/// Handle to the long-lived markdown worker thread.
///
/// Created once at `App::new` and held for the lifetime of the application.
/// Call [`MdWorker::render`] to send HTML to the worker and block until the
/// plain-text result is returned.
pub struct MdWorker {
    sender: mpsc::Sender<MdRequest>,
}

impl MdWorker {
    /// Spawn the worker thread and return a handle.
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<MdRequest>();
        std::thread::Builder::new()
            .name("md-html2text".to_string())
            .spawn(move || {
                // Process requests until the channel is closed.
                while let Ok(req) = rx.recv() {
                    let result = html2text::from_read(req.html.as_bytes(), 120);
                    // If the response channel is closed (caller dropped), just
                    // continue to the next request.
                    let _ = req.response_tx.send(result.map_err(|e| format!("{e:?}")));
                }
            })
            .expect("failed to spawn md-html2text worker thread");
        Self { sender: tx }
    }

    /// Send HTML to the worker and block until the plain-text result arrives.
    ///
    /// The `html2text` computation runs on the worker thread, not the caller's
    /// thread, so panics in `html2text` unwind only the worker thread and are
    /// caught by the `Result` return (FR-002).  The worker is automatically
    /// restarted if it panics (see [`Self::render`]).
    pub fn render(&self, html: &str) -> Result<String, String> {
        let (response_tx, response_rx) = mpsc::channel();
        self.sender
            .send(MdRequest {
                html: html.to_string(),
                response_tx,
            })
            .map_err(|_| "worker channel closed".to_string())?;
        response_rx
            .recv()
            .map_err(|_| "worker dropped response".to_string())?
    }
}
