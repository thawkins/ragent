//! Bounded reading of an untrusted stream.
//!
//! Every place that reads a remote body into memory - a plugin-store index, a
//! connector catalogue, a surface manifest - needs the same guard: read in
//! chunks and refuse the body once it passes a byte ceiling, so a hostile or
//! misconfigured endpoint cannot exhaust memory before the size is known
//! (`Content-Length` is a hint, never a limit).
//!
//! The rule lives here once. Each caller keeps its own error type, so the
//! failure is mapped by the caller rather than by a generic error a crate
//! would have to translate; [`read_capped`] returns a [`CappedError`] that
//! says which of the two things went wrong, and [`CappedError::map_err`]
//! projects that into the caller's own type.

use std::io::Read;

/// Chunk size for [`read_capped`]: one 8 KiB read per iteration, so a body
/// just over the limit is refused after at most one chunk past it.
pub const READ_CHUNK_BYTES: usize = 8 * 1024;

/// Why [`read_capped`] stopped.
///
/// The two cases are deliberately distinct: a truncated body is a transport
/// failure the caller may retry, an oversize body is a policy refusal it must
/// never retry.
#[derive(Debug)]
pub enum CappedError {
    /// The reader failed; the payload is the underlying error's display text.
    Read(String),
    /// The body passed `limit` bytes; nothing past the ceiling was buffered.
    TooLarge {
        /// The ceiling that was exceeded.
        limit: u64,
    },
}

impl CappedError {
    /// Project this into the caller's error type.
    ///
    /// `on_read` receives the underlying read failure's message and
    /// `on_too_large` the ceiling, so each crate keeps its own vocabulary
    /// (`StoreError::Network` / `StoreError::TooLarge`,
    /// `ConnectorError::CatalogueFetch` / `CatalogueTooLarge`, ...).
    ///
    /// # Errors
    ///
    /// Always returns the error built by the closure that matches the variant.
    pub fn map_err<E>(
        self,
        on_read: impl FnOnce(String) -> E,
        on_too_large: impl FnOnce(u64) -> E,
    ) -> E {
        match self {
            Self::Read(message) => on_read(message),
            Self::TooLarge { limit } => on_too_large(limit),
        }
    }
}

impl std::fmt::Display for CappedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read(message) => write!(f, "read failed: {message}"),
            Self::TooLarge { limit } => write!(f, "body exceeds {limit} bytes"),
        }
    }
}

impl std::error::Error for CappedError {}

/// Read `reader` fully into memory, refusing it once more than `max_bytes` has
/// been read.
///
/// A body of exactly `max_bytes` is accepted; the first chunk that would push
/// the total past it is refused without being buffered.
///
/// # Errors
///
/// Returns [`CappedError::Read`] when the reader fails and
/// [`CappedError::TooLarge`] when the body passes `max_bytes`.
///
/// # Examples
///
/// ```
/// use ragent_types::read_capped::{CappedError, read_capped};
///
/// let body = std::io::Cursor::new(b"hello".to_vec());
/// assert_eq!(read_capped(body, 5).expect("exactly at the ceiling"), b"hello");
///
/// let body = std::io::Cursor::new(b"hello!".to_vec());
/// let err = read_capped(body, 5).expect_err("past the ceiling");
/// assert!(matches!(err, CappedError::TooLarge { limit: 5 }));
/// ```
pub fn read_capped(mut reader: impl Read, max_bytes: u64) -> Result<Vec<u8>, CappedError> {
    let mut bytes = Vec::new();
    let mut chunk = vec![0_u8; READ_CHUNK_BYTES];
    loop {
        let read = reader
            .read(&mut chunk)
            .map_err(|e| CappedError::Read(e.to_string()))?;
        if read == 0 {
            break;
        }
        // Compare in `u64` so a body larger than `usize::MAX` on a 32-bit
        // target is refused rather than wrapping the length arithmetic.
        if u64::try_from(bytes.len())
            .unwrap_or(u64::MAX)
            .saturating_add(read as u64)
            > max_bytes
        {
            return Err(CappedError::TooLarge { limit: max_bytes });
        }
        bytes.extend_from_slice(&chunk[..read]);
    }
    Ok(bytes)
}
