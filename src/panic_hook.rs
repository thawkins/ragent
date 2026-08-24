//! Panic hook that writes full panic logs to the `log/panics/` directory.
//!
//! When installed via [`install`], this module replaces the default Rust panic
//! hook with one that captures the panic message, location, and a full
//! backtrace, then writes them to a timestamped file under
//! `log/panics/panic-*.log` in the current working directory. The default hook
//! output is still printed to stderr so existing behaviour is preserved.
//!
//! The backtrace is captured with [`Backtrace::force_capture`], which always
//! produces a trace regardless of the `RUST_BACKTRACE` environment variable.
//! Symbol names and file locations in the trace are most informative when the
//! binary is built with debug symbols (debug builds or `RUSTFLAGS="-C
//! symbol-mapping"` for release).

use std::backtrace::Backtrace;
use std::io::Write;
use std::path::PathBuf;

use chrono::Utc;

/// Install the panic hook that writes full panic logs to `log/panics/`.
///
/// Should be called as early as possible in `main` so that panics during
/// initialisation are also captured. The hook chains to the previous (default)
/// hook after writing the log file, preserving stderr output.
pub fn install() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // A panic raised inside a deliberate contained-panic container
        // (ragent_types::panic_guard) is about to be caught by the caller and
        // degraded gracefully — do not write a panic report or chain to the
        // default hook (which prints "thread panicked" noise and, in the TUI,
        // tears down the terminal even though the app keeps running).
        if ragent_types::panic_guard::is_active() {
            return;
        }
        // Always write the log file first, then chain to the default hook.
        write_panic_log(info);
        default_hook(info);
    }));
}

/// Build the path to the panics directory (`<working_dir>/log/panics`).
fn panics_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("log")
        .join("panics")
}

/// Build a unique panic log file path from the current UTC timestamp.
fn panic_log_path(panics_dir: &std::path::Path) -> PathBuf {
    let timestamp = Utc::now().format("%Y%m%d-%H%M%S-%6f").to_string();
    panics_dir.join(format!("panic-{timestamp}.log"))
}

/// Write a full panic log to the `log/panics/` directory.
///
/// This is a best-effort operation: any I/O failure is silently ignored so
/// that the panic path itself never causes a secondary failure.
fn write_panic_log(info: &std::panic::PanicHookInfo<'_>) {
    let panics_dir = panics_dir();

    // Create the panics directory if it doesn't exist.
    if let Err(e) = std::fs::create_dir_all(&panics_dir) {
        eprintln!(
            "panic_hook: failed to create panics directory {}: {e}",
            panics_dir.display()
        );
        return;
    }

    let path = panic_log_path(&panics_dir);

    // Capture the backtrace immediately — it is only valid on this thread
    // at this point in the unwinding process.
    let backtrace = Backtrace::force_capture();

    let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S.%6fZ").to_string();
    let location = info
        .location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
        .unwrap_or_else(|| "<unknown>".to_string());

    let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    };

    let pid = std::process::id();
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "<unknown>".to_string());

    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "<unknown>".to_string());

    let args: Vec<String> = std::env::args().collect();

    let rust_backtrace =
        std::env::var("RUST_BACKTRACE").unwrap_or_else(|_| "<not set>".to_string());
    let rust_lib_backtrace =
        std::env::var("RUST_LIB_BACKTRACE").unwrap_or_else(|_| "<not set>".to_string());

    let mut content = String::new();
    content.push_str("===============================================================\n");
    content.push_str(" ragent panic report\n");
    content.push_str("===============================================================\n\n");

    content.push_str(&format!("Timestamp (UTC): {timestamp}\n"));
    content.push_str(&format!("PID:             {pid}\n"));
    content.push_str(&format!("Executable:      {exe}\n"));
    content.push_str(&format!("Working dir:     {cwd}\n"));
    content.push_str(&format!("Location:        {location}\n"));
    content.push_str("\n");
    content.push_str(&format!("Command-line args ({}): \n", args.len()));
    for (i, arg) in args.iter().enumerate() {
        content.push_str(&format!("  [{i}] {arg}\n"));
    }
    content.push_str("\n");
    content.push_str("Environment:\n");
    content.push_str(&format!("  RUST_BACKTRACE=    {rust_backtrace}\n"));
    content.push_str(&format!("  RUST_LIB_BACKTRACE={rust_lib_backtrace}\n"));
    content.push_str("\n");
    content.push_str("---------------------------------------------------------------\n");
    content.push_str(" Panic message\n");
    content.push_str("---------------------------------------------------------------\n");
    content.push_str(&payload);
    content.push_str("\n\n");
    content.push_str("---------------------------------------------------------------\n");
    content.push_str(" Backtrace (full)\n");
    content.push_str("---------------------------------------------------------------\n");
    content.push_str(&format!("{backtrace}\n"));
    content.push_str("\n");
    content.push_str("===============================================================\n");
    content.push_str(" End of panic report\n");
    content.push_str("===============================================================\n");

    match std::fs::File::create(&path).and_then(|mut f| f.write_all(content.as_bytes())) {
        Ok(()) => {
            eprintln!(
                "ragent: panic captured — full report written to {}",
                path.display()
            );
        }
        Err(e) => {
            eprintln!(
                "ragent: panic captured but failed to write log to {}: {e}",
                path.display()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_panic_log_path_format() {
        let dir = PathBuf::from("/tmp/test-log");
        let path = panic_log_path(&dir);
        let name = path.file_name().unwrap().to_string_lossy();
        assert!(
            name.starts_with("panic-"),
            "expected panic- prefix, got {name}"
        );
        assert!(name.ends_with(".log"), "expected .log suffix, got {name}");
    }

    #[test]
    fn test_panics_dir_under_cwd() {
        let dir = panics_dir();
        assert!(dir.ends_with("panics"));
    }
}
