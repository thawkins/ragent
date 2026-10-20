//! Crash-dump capture for aborts that bypass the panic hook.
//!
//! ragent's [`crate::panic_hook`] writes a full report for *unwinding* panics,
//! but a stack overflow is not one: once the guard page is hit the Rust runtime
//! prints `thread "..." has overflowed its stack` and calls `abort()`. Nothing
//! after that point runs in-process, so the only place a stack overflow can be
//! trapped is outside the process.
//!
//! This module records the information needed to find and read that crash
//! after the fact. At startup it stamps `log/panics/last-crash.json` with the
//! session identity (pid, executable, args, working directory, thread name) and
//! marks the session as *running*; a clean shutdown overwrites the marker with
//! a `clean exit` record. If the next ragent start finds a session still marked
//! `running` whose pid is gone, the previous run died without unwinding -
//! an abort, a stack overflow, a SIGKILL, or a `std::process::exit`.
//!
//! The record also carries the platform's core-dump *retrieval command* so the
//! crash can be inspected without the operator having to remember whether this
//! host uses `coredumpctl`, a `core_pattern` file, or a macOS `cores/` directory.

use std::io::Write;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};

use serde::{Deserialize, Serialize};

/// Upper bound on the bytes read back from the crash marker (SEC-076).
///
/// A crash record is a few hundred bytes; 64 KiB leaves ample headroom while
/// refusing to pull an unbounded file into memory.
const MAX_MARKER_BYTES: u64 = 64 * 1024;

/// Create (or truncate) `path` owner-only (mode `0o600` on Unix).
///
/// ANTIPAT A-02/A-03: the crash marker, panic reports, and the stderr spool
/// were all created with the process umask (typically `0644`), so a crash or
/// panic record - which carries the full argv - was world-readable. Every
/// crash-path file is created owner-only, and a pre-existing file has its mode
/// re-asserted (`mode()` only applies at creation).
///
/// # Errors
///
/// Returns the underlying [`std::io::Error`] when the file cannot be created.
///
/// SEC-018: the crash marker, panic reports, and stderr spool are opened with
/// `truncate(true)`, so a symlink planted at one of these paths would truncate
/// its target. Reject a symlinked path outright, and additionally open with
/// `O_NOFOLLOW` on Unix so the check and the open cannot be raced.
pub fn create_owner_only(path: &Path) -> std::io::Result<std::fs::File> {
    // Reject a symlinked path: `truncate(true)` would otherwise follow the
    // link and destroy the target file (SEC-018).
    if let Ok(meta) = std::fs::symlink_metadata(path)
        && meta.file_type().is_symlink()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("refusing to open symlinked path '{}'", path.display()),
        ));
    }
    #[cfg(unix)]
    {
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            // SEC-018: refuse to follow a symlink even if one is planted
            // between the check above and this open (TOCTOU).
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)?;
        // `mode(0o600)` only applies at creation; tighten a pre-existing file.
        let _ = file.set_permissions(std::fs::Permissions::from_mode(0o600)); // INTENTIONAL: best-effort permission tighten (mode set at create)
        Ok(file)
    }
    #[cfg(not(unix))]
    {
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)
    }
}

/// Session marker written to `log/panics/last-crash.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashRecord {
    /// Process id of the ragent run.
    pub pid: u32,
    /// Absolute path of the running executable.
    pub exe: String,
    /// Full command line the process was started with.
    pub args: Vec<String>,
    /// Working directory at startup.
    pub cwd: String,
    /// `running` while the process lives; `clean exit` after a normal return.
    pub status: String,
    /// Thread name recorded at startup (the TUI main thread).
    pub thread: String,
    /// RFC-3339 UTC timestamp of the most recent write to this record.
    pub updated_at: String,
    /// Platform-specific command that lists and reads this host's core dumps.
    pub core_dump_hint: String,
}

impl CrashRecord {
    /// True when the process that wrote this record never unwound.
    #[must_use]
    pub fn is_unclean(&self) -> bool {
        self.status == "running"
    }
}

/// Build the crash-marker path (`<working_dir>/log/panics/last-crash.json`).
#[must_use]
pub fn record_path(working_dir: &Path) -> PathBuf {
    working_dir
        .join("log")
        .join("panics")
        .join("last-crash.json")
}

/// Platform-specific instructions for locating a core dump from this host.
///
/// Picks `coredumpctl` when the local `core_pattern` pipes into
/// `systemd-coredump`, otherwise falls back to a filesystem `core` path or the
/// macOS `cores/` directory convention.
#[must_use]
pub fn core_dump_hint() -> String {
    if cfg!(target_os = "macos") {
        return "macOS: /cores/core.<pid> (check `ls -l /cores`)".to_string();
    }
    if cfg!(target_os = "linux") {
        if let Ok(pattern) = std::fs::read_to_string("/proc/sys/kernel/core_pattern")
            && pattern.trim_start().starts_with('|')
        {
            let helper = pattern
                .trim()
                .trim_start_matches('|')
                .split_whitespace()
                .next()
                .unwrap_or("systemd-coredump")
                .to_string();
            return format!(
                "Linux: coredumpctl list / coredumpctl info <pid> / coredumpctl debug <pid> (core_pattern pipes to {helper})"
            );
        }
        return "Linux: check `cat /proc/sys/kernel/core_pattern` for the core file location"
            .to_string();
    }
    "unknown platform: check the OS core-dump configuration".to_string()
}

/// Write (or overwrite) the crash marker for the current process.
///
/// # Errors
///
/// Returns an error when the `log/panics/` directory cannot be created or the
/// marker file cannot be written.
pub fn write_record(working_dir: &Path, status: &str) -> std::io::Result<PathBuf> {
    let path = record_path(working_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let record = CrashRecord {
        pid: std::process::id(),
        exe: std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "<unknown>".to_string()),
        // ANTIPAT A-02: argv can carry a credential (`ragent auth openai sk-...`,
        // `--model ...sk-...`), and this record is written to disk. Route every
        // argument through the shared redactor so the marker never stores a
        // secret in plain text.
        args: std::env::args()
            .map(|a| ragent_types::sanitize::redact_secrets(&a))
            .collect(),
        cwd: working_dir.display().to_string(),
        status: status.to_string(),
        thread: std::thread::current().name().unwrap_or("main").to_string(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        core_dump_hint: core_dump_hint(),
    };
    let json = serde_json::to_string_pretty(&record)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    // SEC-018: never write the marker in place. A symlink planted at the
    // marker path would be followed by an in-place open (truncating its
    // target); an atomic temp-file-then-rename replace sidesteps that and
    // also means a concurrent reader sees either the old or the new record,
    // never a half-written one.
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let tmp_path = dir.join(format!(
        ".last-crash.{}.{}.tmp",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    let write_result = (|| -> std::io::Result<()> {
        let mut file = create_owner_only(&tmp_path)?;
        file.write_all(json.as_bytes())?;
        file.write_all(b"\n")?;
        file.flush()?;
        Ok(())
    })();
    if let Err(e) = write_result {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e);
    }
    if let Err(e) = std::fs::rename(&tmp_path, &path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(e);
    }
    Ok(path)
}

/// Report whether a previous, unfinished ragent session is on record.
///
/// Reads `<working_dir>/log/panics/last-crash.json`; when it still says
/// `running` and its pid is no longer alive, returns the record so the caller
/// can point the operator at the stale crash and the core-dump command.
///
/// # Errors
///
/// Returns an error when the marker exists but cannot be read or parsed.
pub fn previous_unclean_exit(working_dir: &Path) -> std::io::Result<Option<CrashRecord>> {
    let path = record_path(working_dir);
    // SEC-076: treat a symlinked marker as absent (do not follow the link),
    // and refuse to slurp an arbitrarily large file.
    let Ok(meta) = std::fs::symlink_metadata(&path) else {
        return Ok(None);
    };
    if meta.file_type().is_symlink() || meta.len() > MAX_MARKER_BYTES {
        return Ok(None);
    }
    use std::io::Read as _;
    let file = std::fs::File::open(&path)?;
    let mut content = String::new();
    let mut reader = file.take(MAX_MARKER_BYTES);
    reader.read_to_string(&mut content)?;
    let record: CrashRecord = serde_json::from_str(&content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    if record.is_unclean() && !process_alive(record.pid) {
        return Ok(Some(record));
    }
    Ok(None)
}

/// Test whether a pid is still alive.
///
/// # Safety note
///
/// Unix uses the one approved `unsafe` FFI call for signal 0 (a liveness probe
/// that does not deliver a signal); Windows uses an `OpenProcess` handle and
/// `GetExitCodeProcess` via windows-sys (kernel32 imports are statically
/// linked by `windows_link`). The invariant is that `pid` is a value read back
/// from a record file; an invalid pid simply yields `false`.
#[allow(unsafe_code)]
fn process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    #[cfg(unix)]
    {
        // SAFETY: `kill` with signal 0 performs only an existence/permission
        // check and cannot deliver a signal. The return value is used directly.
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, STILL_ACTIVE};
        use windows_sys::Win32::System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };
        // SAFETY: `OpenProcess`/`GetExitCodeProcess`/`CloseHandle` take plain
        // integer and pointer-to-integer arguments; the handle is closed on
        // every path and the pid is only probed, never signalled.
        unsafe {
            let handle: HANDLE = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if handle.is_null() {
                return false;
            }
            let mut code: u32 = 0;
            let ok = GetExitCodeProcess(handle, &mut code) != 0;
            CloseHandle(handle);
            ok && code != STILL_ACTIVE
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        // Unknown platform: a stale marker is reported (the safe default for a
        // crash scanner) rather than silently hiding it.
        true
    }
}

#[cfg(all(test, unix))]
#[path = "../tests/inline/crash_dump_tests.rs"]
mod tests;
