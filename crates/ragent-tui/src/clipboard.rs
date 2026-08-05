//! Shared clipboard helpers for the TUI.
//!
//! Provides thin wrappers around [`arboard`] for reading/writing text and for
//! saving raw RGBA clipboard image data to a PNG temp file.  All TUI clipboard
//! operations should route through these helpers so behaviour (e.g. the Linux
//! `wait()` threading workaround) lives in a single place.

use std::path::PathBuf;

use anyhow::Result;
use arboard::ImageData;
use image::{ImageBuffer, Rgba};

/// Maximum raw pixel buffer size we accept from the clipboard (50 MB).
const MAX_CLIPBOARD_IMAGE_BYTES: usize = 50 * 1024 * 1024;

/// Maximum dimension (width or height) we accept from the clipboard.
const MAX_CLIPBOARD_IMAGE_DIM: u32 = 16_384;

/// Read raw image data from the system clipboard.
///
/// Returns `None` if the clipboard is unavailable or does not contain an image.
pub fn get_clipboard_image() -> Option<arboard::ImageData<'static>> {
    arboard::Clipboard::new()
        .ok()
        .and_then(|mut cb| cb.get_image().ok())
        .map(|img| {
            // Convert borrowed bytes to an owned `Cow` so the returned value
            // owns its data and is not tied to the clipboard object's lifetime.
            arboard::ImageData {
                width: img.width,
                height: img.height,
                bytes: std::borrow::Cow::Owned(img.bytes.into_owned()),
            }
        })
}

/// Read plain text from the system clipboard.
///
/// Returns `None` if the clipboard is unavailable or does not contain text.
pub fn get_clipboard_text() -> Option<String> {
    arboard::Clipboard::new()
        .ok()
        .and_then(|mut cb| cb.get_text().ok())
}

/// Synchronous, deterministic version of [`get_clipboard_text`] used by tests.
///
/// Uses a process-wide singleton clipboard so that writes and reads are
/// serialized and the underlying X11 selection stays alive.
#[doc(hidden)]
pub fn get_clipboard_text_sync() -> Option<String> {
    with_sync_clipboard(|cb| Ok(cb.get_text().ok()))
        .ok()
        .flatten()
}

/// Write plain text to the system clipboard without blocking the caller.
///
/// On Linux, arboard's `set().wait()` blocks until another application takes
/// ownership of the clipboard, so the write is performed on a background
/// thread.  Errors are silently ignored because clipboard writes are best-
/// effort UI conveniences.
pub fn set_clipboard_text(text: &str) {
    let text = text.to_owned();
    std::thread::spawn(move || {
        let _ = set_clipboard_text_sync(&text);
    });
}

/// Synchronous variant of [`set_clipboard_text`] used by tests.
///
/// Uses a process-wide singleton clipboard so that writes and reads are
/// serialized and the underlying X11 selection stays alive.  It is not
/// intended for normal TUI use because, on Linux, the clipboard content may
/// disappear when the process exits.
#[doc(hidden)]
pub fn set_clipboard_text_sync(text: &str) -> anyhow::Result<()> {
    with_sync_clipboard(|cb| {
        #[cfg(target_os = "linux")]
        {
            cb.set().text(text.to_string())?;
        }
        #[cfg(not(target_os = "linux"))]
        {
            cb.set_text(text.to_string())?;
        }
        Ok(())
    })
}

/// Run `f` against the test-only singleton clipboard.
fn with_sync_clipboard<F, R>(f: F) -> anyhow::Result<R>
where
    F: FnOnce(&mut arboard::Clipboard) -> anyhow::Result<R>,
{
    use std::sync::{Mutex, OnceLock};

    static CLIPBOARD: OnceLock<Mutex<Option<arboard::Clipboard>>> = OnceLock::new();

    let lock = CLIPBOARD.get_or_init(|| Mutex::new(None));
    let mut guard = match lock.lock() {
        Ok(g) => g,
        Err(poisoned) => poisoned.into_inner(),
    };

    if guard.is_none() {
        *guard = Some(arboard::Clipboard::new()?);
    }

    let cb = guard.as_mut().expect("clipboard was just initialised");
    f(cb)
}

/// Encode `arboard::ImageData` (raw RGBA pixels) as a PNG saved to a
/// securely-created temp file.
///
/// Returns the path of the written file.  The file is persisted (not
/// auto-deleted) so the caller can attach it to a message.
///
/// # Errors
///
/// Returns an error if:
/// - The image exceeds the maximum allowed size or dimensions
/// - The image dimensions don't match the pixel buffer size
/// - The temporary file cannot be created or written
pub fn clipboard_image_to_temp(img_data: &ImageData<'_>) -> Result<PathBuf> {
    let buf_len = img_data.bytes.len();
    if buf_len > MAX_CLIPBOARD_IMAGE_BYTES {
        anyhow::bail!(
            "clipboard image too large ({:.1} MB, limit {:.0} MB)",
            buf_len as f64 / (1024.0 * 1024.0),
            MAX_CLIPBOARD_IMAGE_BYTES as f64 / (1024.0 * 1024.0),
        );
    }

    let width = img_data.width as u32;
    let height = img_data.height as u32;
    if width > MAX_CLIPBOARD_IMAGE_DIM || height > MAX_CLIPBOARD_IMAGE_DIM {
        anyhow::bail!(
            "clipboard image dimensions too large ({width}×{height}, max {MAX_CLIPBOARD_IMAGE_DIM}×{MAX_CLIPBOARD_IMAGE_DIM})"
        );
    }

    let bytes = img_data.bytes.as_ref().to_vec();
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(width, height, bytes)
        .ok_or_else(|| anyhow::anyhow!("clipboard image dimensions mismatch pixel buffer"))?;

    // Create a secure temporary file (O_EXCL, restrictive permissions).
    let tmp_file = tempfile::Builder::new()
        .prefix("ragent_paste_")
        .suffix(".png")
        .tempfile()
        .map_err(|e| anyhow::anyhow!("failed to create secure temp file: {e}"))?;

    img.save(tmp_file.path())?;

    // Prevent auto-deletion — the caller owns the file lifecycle.
    let path = tmp_file
        .into_temp_path()
        .keep()
        .map_err(|_| anyhow::anyhow!("failed to persist temp image file"))?;

    Ok(path)
}
