//! Redirect the process's stderr into a truncating spool while the TUI runs.
//!
//! The TUI owns the alternate screen, so anything written to raw stderr -
//! Rust's default panic hook, `eprintln!`, C-library diagnostics - is painted
//! over by the next ratatui frame and lost. This module replaces the
//! process-wide standard-error *file descriptor* (fd 2) with the write end of a
//! pipe and drains that pipe on a dedicated thread into a
//! [`ragent_types::stderr_spool::Spool`], which keeps only the newest 1000
//! lines.
//!
//! This is the only module in the binary that performs FFI: Rust's standard
//! library exposes no way to replace the C-level stderr descriptor that the
//! default panic hook and C libraries write to. The redirection happens once,
//! at startup, before the TUI threads are spawned.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use ragent_types::stderr_spool::Spool;

/// Whether [`install`] successfully redirected the process's stderr (fd 2) into
/// a spool. Read by the binary's top-level error handler to decide whether a
/// startup failure would otherwise be hidden behind the redirect and must be
/// echoed to stdout instead.
static ACTIVE: AtomicBool = AtomicBool::new(false);

/// Whether stderr is currently redirected into a spool (see [`install`]).
#[must_use]
pub fn is_active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

/// Redirect fd 2 into a truncating spool file under `log_dir`.
///
/// Returns the [`Spool`] so the caller can attach a mirror sink (the TUI uses
/// this to surface raw stderr in its log panel). Returns `None` - leaving
/// stderr untouched - when the spool directory cannot be created or the `dup2`
/// redirection fails.
///
/// # Safety
///
/// Uses `libc::dup`, `libc::dup2`, and `libc::pipe`. The invariant is that the
/// saved original descriptor is deliberately leaked so any later
/// `write(2, ...)` still reaches the terminal, and that every descriptor is
/// checked before use and closed exactly once on the failure paths.
#[allow(unsafe_code)]
pub fn install(log_dir: &Path) -> Option<Spool> {
    if std::fs::create_dir_all(log_dir).is_err() {
        return None;
    }
    let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let path = log_dir.join(format!("stderr-{ts}.log"));
    let spool = Spool::new(path);

    let spool_for_thread = spool.clone();
    let mut fds = [0i32; 2];
    // SAFETY: `fds` is a valid two-element array; `pipe` writes both entries.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return None;
    }
    let (read_fd, write_fd) = fds.into();

    // SAFETY: `dup` returns a new descriptor or -1; -1 is rejected below.
    let saved_stderr = unsafe { libc::dup(libc::STDERR_FILENO) };
    if saved_stderr < 0 {
        // SAFETY: both descriptors came from `pipe` and are still open.
        unsafe {
            libc::close(read_fd);
            libc::close(write_fd);
        }
        return None;
    }

    // SAFETY: both descriptors are valid and `dup2` atomically replaces fd 2.
    if unsafe { libc::dup2(write_fd, libc::STDERR_FILENO) } != libc::STDERR_FILENO {
        // SAFETY: all three descriptors are valid and owned by this function.
        unsafe {
            libc::close(read_fd);
            libc::close(write_fd);
            libc::close(saved_stderr);
        }
        return None;
    }
    // The write end is now fd 2; close our extra handle so the pipe's write
    // side is owned solely by stderr.
    // SAFETY: `write_fd` is the extra duplicate created by `pipe`, now unused.
    unsafe {
        libc::close(write_fd);
    }
    // `saved_stderr` is intentionally leaked (see the safety note above).

    // Drain the pipe on a dedicated thread and spool each chunk.
    std::thread::Builder::new()
        .name("ragent-stderr-spool".to_string())
        .spawn(move || {
            use std::io::Read as _;
            use std::os::fd::FromRawFd;
            // SAFETY: `read_fd` is the read end of the pipe created above and
            // is not owned by any other `File`, so wrapping it is sound.
            let mut reader = unsafe { std::fs::File::from_raw_fd(read_fd) };
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => spool_for_thread.write(&buf[..n]),
                }
            }
        })
        .ok()?;

    // Only now is the redirect fully in place: record it so the top-level error
    // handler knows a startup `Error:` line would land in the spool instead of
    // the terminal.
    ACTIVE.store(true, Ordering::Relaxed);
    Some(spool)
}
