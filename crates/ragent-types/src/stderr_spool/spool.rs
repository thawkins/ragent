//! Line-counted, self-truncating stderr spool.
//!
//! A [`Spool`] appends whatever it is given to a file and keeps only the newest
//! [`SPOOL_MAX_LINES`] lines: each write counts its new newlines and, once the
//! cap is exceeded, rewrites the file with just its freshest lines. The cap is
//! generous (1000 lines) so the rewrite is amortised across many writes.
//!
//! It is also bounded in *bytes* ([`SPOOL_MAX_BYTES`]). SEC-ragent-types-002
//! (SECTASKS T-058): the line cap alone was not enforced for a writer that never
//! emits a newline (a `\r`-only progress renderer, a binary diagnostic, an
//! rquickjs `console.error`), so the file could grow without limit. Every write
//! now accounts for its bytes, and the byte cap is enforced even when there is
//! no newline to count.
//!
//! The type is deliberately free of process-global state — the fd-2
//! redirection lives in the binary — so the truncation logic is directly
//! testable and the TUI can share the same handle to mirror spooled text into
//! its log panel.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Maximum number of lines retained in the stderr spool file.
pub const SPOOL_MAX_LINES: usize = 1000;

/// Maximum number of bytes retained in the stderr spool file.
///
/// SEC-ragent-types-002 (SECTASKS T-058): the line cap cannot bind a writer
/// that never emits a newline, so a byte ceiling is enforced as well.
pub const SPOOL_MAX_BYTES: usize = 256 * 1024;

/// A line-counted spool file that truncates itself to the newest
/// [`SPOOL_MAX_LINES`] lines.
///
/// `Spool` is cheap to clone (`Arc` inside) so it can be handed to the
/// reader thread and to the log-mirror callback at the same time.
#[derive(Clone)]
pub struct Spool {
    inner: Arc<SpoolInner>,
}

struct SpoolInner {
    path: PathBuf,
    /// Running estimate of the newline count in `path`. It is seeded from the
    /// file on disk at construction (so an existing spool is not re-counted)
    /// and is refreshed after each rewrite.
    lines: Mutex<usize>,
    /// Running estimate of the byte size of `path`, used to enforce
    /// [`SPOOL_MAX_BYTES`] independently of the newline count.
    bytes: Mutex<usize>,
    /// Optional sink that receives each write before it is spooled. The TUI
    /// installs one that mirrors raw stderr into the log panel.
    mirror: Mutex<Option<Arc<dyn Fn(&str) + Send + Sync>>>,
}

impl Spool {
    /// Create a spool bound to `path`, seeding the line estimate from any
    /// existing content.
    #[must_use]
    pub fn new(path: PathBuf) -> Self {
        let lines = std::fs::read_to_string(&path)
            .map(|s| s.lines().count())
            .unwrap_or(0);
        let bytes = std::fs::metadata(&path)
            .map(|m| m.len() as usize)
            .unwrap_or(0);
        Self {
            inner: Arc::new(SpoolInner {
                path,
                lines: Mutex::new(lines),
                bytes: Mutex::new(bytes),
                mirror: Mutex::new(None),
            }),
        }
    }

    /// Path of the spool file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.inner.path
    }

    /// Attach a mirror sink invoked with the lossy-UTF-8 form of every chunk
    /// written to the spool.
    pub fn set_mirror(&self, mirror: Arc<dyn Fn(&str) + Send + Sync>) {
        if let Ok(mut slot) = self.inner.mirror.lock() {
            *slot = Some(mirror);
        }
    }

    /// Append `data` to the spool, truncating to the newest
    /// [`SPOOL_MAX_LINES`] lines when the cap is exceeded.
    ///
    /// Best-effort: all I/O errors are swallowed because a full disk or a
    /// missing directory must never turn a diagnostic write into a second
    /// failure on the crash path.
    pub fn write(&self, data: &[u8]) {
        if let Ok(mirror) = self.inner.mirror.lock() {
            if let Some(mirror) = mirror.as_ref() {
                mirror(&String::from_utf8_lossy(data));
            }
        }

        let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.inner.path)
        else {
            return;
        };
        if file.write_all(data).is_err() {
            return;
        }

        // SEC-ragent-types-002 (SECTASKS T-058): account for the bytes written
        // on *every* write, not only the writes that contain a newline. The old
        // `if added == 0 { return; }` early-out let a newline-free producer grow
        // the file without bound (the cap was never consulted) and turned each
        // later newline-bearing write into a full-file read+rewrite.
        let added = memchr::memchr_iter(b'\n', data).count();
        let Ok(mut lines) = self.inner.lines.lock() else {
            return;
        };
        let Ok(mut bytes) = self.inner.bytes.lock() else {
            return;
        };
        *lines += added;
        *bytes = bytes.saturating_add(data.len());
        if *lines > SPOOL_MAX_LINES || *bytes > SPOOL_MAX_BYTES {
            if let Some((kept_lines, kept_bytes)) =
                truncate_file(&self.inner.path, SPOOL_MAX_LINES, SPOOL_MAX_BYTES)
            {
                *lines = kept_lines;
                *bytes = kept_bytes;
            }
        }
    }
}

/// Rewrite `path` so it holds at most `max_lines` lines and `max_bytes` bytes.
///
/// Returns `(lines_retained, bytes_retained)`, or `None` if the file could not
/// be read or rewritten.
///
/// SEC-ragent-types-002 (SECTASKS T-058): the byte ceiling is applied to the
/// whole file. A file that is over the byte cap with no line structure at all
/// (a single multi-megabyte line) is truncated to its newest `max_bytes` bytes,
/// and the read itself is bounded so a runaway file is never loaded whole.
fn truncate_file(path: &Path, max_lines: usize, max_bytes: usize) -> Option<(usize, usize)> {
    // Bound the read: never load more than a small multiple of the byte cap.
    let read_limit = max_bytes.saturating_mul(2).max(1024 * 1024) as u64;
    let mut raw = Vec::new();
    {
        let file = std::fs::File::open(path).ok()?;
        use std::io::Read as _;
        file.take(read_limit).read_to_end(&mut raw).ok()?;
    }
    let content = String::from_utf8_lossy(&raw).into_owned();

    // First cut on bytes so a newline-free file cannot survive the cap.
    let mut rebuilt = if content.len() > max_bytes {
        let start = content.len() - max_bytes;
        // Step forward to a char boundary so the slice is valid UTF-8.
        let start = (start..=content.len())
            .find(|i| content.is_char_boundary(*i))
            .unwrap_or(content.len());
        content[start..].to_string()
    } else {
        content
    };

    // Then cut on lines.
    let lines: Vec<&str> = rebuilt.lines().collect();
    let keep = &lines[lines.len().saturating_sub(max_lines)..];
    rebuilt = keep.join("\n");
    // Preserve the trailing newline so the next append starts on a fresh line.
    if !rebuilt.is_empty() {
        rebuilt.push('\n');
    }

    std::fs::write(path, rebuilt.as_bytes()).ok()?;
    Some((rebuilt.lines().count(), rebuilt.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spool_truncates_to_the_newest_lines() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("stderr.log");
        let spool = Spool::new(path.clone());

        // Write 1200 lines through the spool in two batches.
        for i in 0..700 {
            spool.write(format!("line-{i}\n").as_bytes());
        }
        for i in 700..1200 {
            spool.write(format!("line-{i}\n").as_bytes());
        }

        let content = std::fs::read_to_string(&path).expect("read spool");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), SPOOL_MAX_LINES);
        // The newest lines survive, the oldest are gone.
        assert_eq!(lines[lines.len() - 1], "line-1199");
        assert_eq!(lines[0], format!("line-{}", 1200 - SPOOL_MAX_LINES));
    }

    #[test]
    fn spool_counts_newlines_from_existing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("stderr.log");
        // Seed a file already at the cap.
        let seeded: String = (0..SPOOL_MAX_LINES).map(|i| format!("old-{i}\n")).collect();
        std::fs::write(&path, seeded).expect("seed");

        let spool = Spool::new(path.clone());
        spool.write(b"fresh\n");

        let content = std::fs::read_to_string(&path).expect("read spool");
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), SPOOL_MAX_LINES);
        assert_eq!(lines[lines.len() - 1], "fresh");
    }

    #[test]
    fn spool_appends_a_newline_free_write_under_the_cap() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("stderr.log");
        std::fs::write(&path, "existing\n").expect("seed");
        let spool = Spool::new(path.clone());

        spool.write(b"partial output without newline");

        let content = std::fs::read_to_string(&path).expect("read spool");
        assert_eq!(content, "existing\npartial output without newline");
    }

    #[test]
    fn spool_bounds_a_newline_free_stream() {
        // SEC-ragent-types-002 (SECTASKS T-058): a producer that never emits a
        // newline used to grow the file without bound because the line cap was
        // only consulted when `added > 0`.
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("stderr.log");
        let spool = Spool::new(path.clone());

        let chunk = "x".repeat(64 * 1024);
        for _ in 0..40 {
            spool.write(chunk.as_bytes());
        }

        let len = std::fs::metadata(&path).expect("stat spool").len() as usize;
        assert!(
            len <= SPOOL_MAX_BYTES + chunk.len(),
            "newline-free spool grew to {len} bytes (cap {SPOOL_MAX_BYTES})"
        );
    }

    #[test]
    fn spool_mirrors_each_write() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("stderr.log");
        let spool = Spool::new(path);
        let seen: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
        let sink = Arc::clone(&seen);
        spool.set_mirror(Arc::new(move |s: &str| {
            if let Ok(mut acc) = sink.lock() {
                acc.push_str(s);
            }
        }));

        spool.write(b"hello ");
        spool.write(b"world\n");

        assert_eq!(seen.lock().expect("lock").as_str(), "hello world\n");
    }
}
