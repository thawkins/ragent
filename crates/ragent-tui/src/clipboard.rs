//! Shared clipboard helpers for the TUI.
//!
//! Provides thin wrappers around [`arboard`] for reading/writing text and for
//! saving raw RGBA clipboard image data to a PNG temp file.  All TUI clipboard
//! operations should route through these helpers so behaviour (e.g. the Linux
//! `wait()` threading workaround) lives in a single place.
//!
//! # Clipboard image lifecycle
//!
//! Pasted images are persisted under `<cwd>/target/temp/` as `ragent_paste_*.png`
//! so they stay inside the project tree and are already covered by `.gitignore`.
//! On Unix the files are created with mode `0o600`.  On TUI startup any orphaned
//! `ragent_paste_*.png` files in that directory that are older than
//! [`CLIPBOARD_TEMP_MAX_AGE`] are removed.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use arboard::ImageData;
use image::{ExtendedColorType, ImageEncoder};

/// Maximum raw pixel buffer size we accept from the clipboard (50 MB).
const MAX_CLIPBOARD_IMAGE_BYTES: usize = 50 * 1024 * 1024;

/// Maximum dimension (width or height) we accept from the clipboard.
const MAX_CLIPBOARD_IMAGE_DIM: u32 = 16_384;

/// Orphaned clipboard temp files older than this are pruned on TUI startup.
pub const CLIPBOARD_TEMP_MAX_AGE: Duration = Duration::from_hours(24);

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

/// Return the project `target/temp/` directory, creating it if necessary.
///
/// Falls back to the OS temp directory if the project directory cannot be
/// created (for example, when running outside a writable project tree).
pub fn project_temp_dir() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let dir = cwd.join("target").join("temp");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        tracing::warn!(path=%dir.display(), error=%e, "failed to create project temp dir; falling back to OS temp");
        return std::env::temp_dir();
    }
    dir
}

/// Delete orphaned `ragent_paste_*.png` files in the project temp directory that
/// are older than `max_age`.
///
/// Returns the number of files removed.  Errors for individual files are logged
/// and do not abort the scan.
pub fn prune_clipboard_temp_files(max_age: Duration) -> std::io::Result<usize> {
    prune_clipboard_temp_files_in(&project_temp_dir(), max_age)
}

/// Delete orphaned `ragent_paste_*.png` files in `dir` that are older than
/// `max_age`.
///
/// This is the testable implementation; [`prune_clipboard_temp_files`] uses the
/// project `target/temp/` directory.
pub fn prune_clipboard_temp_files_in(
    dir: &std::path::Path,
    max_age: Duration,
) -> std::io::Result<usize> {
    let mut removed = 0usize;
    let now = std::time::SystemTime::now();

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) => {
            tracing::debug!(path=%dir.display(), error=%e, "cannot read clipboard temp dir");
            return Ok(0);
        }
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        if !name_str.starts_with("ragent_paste_")
            || !name_str.to_ascii_lowercase().ends_with(".png")
        {
            continue;
        }
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(path=%entry.path().display(), error=%e, "cannot stat clipboard temp file");
                continue;
            }
        };
        let modified = match meta.modified() {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!(path=%entry.path().display(), error=%e, "cannot read modification time");
                continue;
            }
        };
        let age = now.duration_since(modified).unwrap_or(Duration::ZERO);
        if age < max_age {
            continue;
        }
        if let Err(e) = std::fs::remove_file(entry.path()) {
            tracing::warn!(path=%entry.path().display(), error=%e, "failed to remove orphaned clipboard temp file");
        } else {
            removed += 1;
            tracing::debug!(path=%entry.path().display(), age=?age, "removed orphaned clipboard temp file");
        }
    }

    Ok(removed)
}

/// Encode `arboard::ImageData` (raw RGBA pixels) as a PNG saved to a
/// securely-created temp file under `target/temp/`.
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

    // Encode directly from the borrowed pixel buffer to avoid a `to_vec()` copy.
    // We must still validate that the buffer size matches the declared dimensions
    // before handing it to the PNG encoder.
    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| anyhow::anyhow!("clipboard image dimensions overflow"))?;
    if buf_len != expected {
        anyhow::bail!("clipboard image dimensions mismatch pixel buffer");
    }

    let temp_dir = project_temp_dir();
    let tmp_file = tempfile::Builder::new()
        .prefix("ragent_paste_")
        .suffix(".png")
        .tempfile_in(&temp_dir)
        .map_err(|e| anyhow::anyhow!("failed to create secure temp file: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let meta = tmp_file.as_file().metadata()?;
        let mut perms = meta.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(tmp_file.path(), perms)?;
    }

    let mut file = tmp_file.as_file();
    let encoder = image::codecs::png::PngEncoder::new(&mut file);
    encoder
        .write_image(&img_data.bytes, width, height, ExtendedColorType::Rgba8)
        .map_err(|e| anyhow::anyhow!("failed to encode clipboard image: {e}"))?;

    // Prevent auto-deletion — the caller owns the file lifecycle.
    let path = tmp_file
        .into_temp_path()
        .keep()
        .map_err(|_| anyhow::anyhow!("failed to persist temp image file"))?;

    Ok(path)
}
