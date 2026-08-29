//! sudo askpass integration for the bash tool.
//!
//! When a command (or a script it runs) invokes `sudo` and sudo needs a
//! password, sudo normally opens `/dev/tty` directly to prompt the user. In a
//! non-interactive agent context this either hangs (if a tty is inherited) or
//! fails opaquely. This module wires sudo's `SUDO_ASKPASS` mechanism into
//! ragent's existing question dialog (the same one the `ask_user` tool uses)
//! so a credential request is surfaced as a prominent popup instead of a
//! buried tty prompt.
//!
//! ## How it works
//!
//! 1. [`AskPassBroker::start`] writes a tiny POSIX shell helper to a temp
//!    directory and creates a per-invocation request directory.
//! 2. The bash tool sets `SUDO_ASKPASS=<helper path>` and
//!    `RAGENT_ASKPASS_DIR=<request dir>` on the spawned process so any `sudo`
//!    invocation inside the command (including in child scripts) inherits
//!    them.
//! 3. When sudo needs a password it execs the helper. The helper writes the
//!    prompt to `request_<id>` in the request dir and polls for a
//!    `response_<id>` file.
//! 4. A background tokio task (started by
//!    [`AskPassBroker::spawn_watcher`]) polls the request dir, publishes
//!    [`Event::QuestionRequested`] for each request, waits for
//!    [`Event::QuestionAnswered`], and writes the answer to the response
//!    file. An empty or dismissed answer causes the helper to exit non-zero
//!    so sudo fails cleanly instead of hanging.
//! 5. [`AskPassBroker::stop`] cancels the watcher and removes temp files once
//!    the command finishes.
//!
//! On non-POSIX systems (Windows) this module is inert — [`AskPassBroker`]
//! is never constructed and sudo is not expected to be present.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::event::{Event, EventBus};

/// Sentinel response text the TUI emits when the user dismisses a question
/// with `Esc`. Treated as an explicit cancellation.
const DISMISS_MARKER: &str = "[User dismissed question]";

/// How long the watcher task waits between directory scans.
///
/// R-8: Increased from 100 ms to 500 ms to reduce idle CPU wakeups from
/// 10/s to 2/s during sudo-capable bash commands. The askpass broker
/// writes a response file, so sub-second latency is not critical.
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Maximum time the watcher waits for a single password response before
/// giving up (matched by the helper's own poll loop so both sides fail at
/// roughly the same time).
///
/// Note: written as `from_secs(120)` rather than `Duration::from_mins(2)`
/// because `from_mins` is nightly-only and this crate targets stable Rust
/// (the workspace happens to build on nightly, but the constant must not
/// depend on that).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

/// A handle that owns the temp helper script path and the request directory,
/// and drives the background watcher task for a single bash invocation.
pub(crate) struct AskPassBroker {
    /// Absolute path to the generated askpass helper script.
    helper_path: PathBuf,
    /// Per-invocation directory the helper writes request files into and the
    /// watcher reads from / writes responses into.
    request_dir: PathBuf,
    /// The background watcher task's abort handle.
    watcher: Option<tokio::task::JoinHandle<()>>,
}

impl AskPassBroker {
    /// Create the helper script and request directory and return a broker
    /// ready to drive a single bash invocation.
    ///
    /// Returns `None` (logging a warning) on any I/O failure or on Windows,
    /// letting the bash tool fall back to plain execution without askpass.
    pub(crate) fn start(session_id: &str) -> Option<Self> {
        if is_windows() {
            return None;
        }

        let base = temp_base_dir(session_id)?;
        let stamp = unique_stamp();
        let request_dir = base.join(format!("askpass_{stamp}"));

        if let Err(e) = std::fs::create_dir_all(&request_dir) {
            tracing::warn!(error = %e, dir = %request_dir.display(), "askpass: failed to create request dir");
            return None;
        }

        let helper_path = base.join(format!("ragent_askpass_{stamp}.sh"));
        if let Err(e) = std::fs::write(&helper_path, HELPER_BODY) {
            tracing::warn!(error = %e, path = %helper_path.display(), "askpass: failed to write helper");
            let _ = std::fs::remove_dir_all(&request_dir);
            return None;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            if let Err(e) =
                std::fs::set_permissions(&helper_path, std::fs::Permissions::from_mode(0o700))
            {
                tracing::warn!(error = %e, "askpass: failed to chmod helper");
                let _ = std::fs::remove_file(&helper_path);
                let _ = std::fs::remove_dir_all(&request_dir);
                return None;
            }
        }

        tracing::debug!(
            helper = %helper_path.display(),
            dir = %request_dir.display(),
            "askpass: broker ready"
        );

        Some(Self {
            helper_path,
            request_dir,
            watcher: None,
        })
    }

    /// Returns the environment variables to inject into the spawned command so
    /// that sudo (and any child it spawns) uses the askpass helper.
    pub(crate) fn env_vars(&self) -> [(&'static str, String); 2] {
        [
            (
                "SUDO_ASKPASS",
                self.helper_path.to_string_lossy().into_owned(),
            ),
            (
                "RAGENT_ASKPASS_DIR",
                self.request_dir.to_string_lossy().into_owned(),
            ),
        ]
    }

    /// Spawn the background watcher that routes askpass requests through the
    /// event-bus question dialog.
    ///
    /// `session_id` is the owning session (so the TUI knows which session the
    /// question belongs to). `event_bus` is the shared bus used both to
    /// publish the request and to subscribe for the matching answer.
    pub(crate) fn spawn_watcher(&mut self, session_id: String, event_bus: Arc<EventBus>) {
        let dir = self.request_dir.clone();
        self.watcher = Some(tokio::spawn(async move {
            watch_loop(dir, session_id, event_bus).await;
        }));
    }

    /// Cancel the watcher and remove the temp helper + request directory.
    pub(crate) fn stop(self) {
        if let Some(handle) = self.watcher {
            handle.abort();
        }
        let _ = std::fs::remove_file(&self.helper_path);
        let _ = std::fs::remove_dir_all(&self.request_dir);
    }

    /// Absolute path to the request directory (mainly useful for logging).
    #[allow(dead_code)] // kept for test diagnostics; used by the inline askpass tests
    pub(crate) fn request_dir(&self) -> &Path {
        &self.request_dir
    }
}

/// Background loop that scans `dir` for new `request_<id>` files and routes
/// each through the event bus.
///
/// `dir` is the broker's request directory for this invocation. The per-request
/// task is given the directory path so it can write `response_<id>` next to the
/// matching `request_<id>` file the helper is polling for.
async fn watch_loop(dir: PathBuf, session_id: String, event_bus: Arc<EventBus>) {
    let mut seen: HashSet<String> = HashSet::new();

    loop {
        // Scan for new requests. If the directory has vanished the broker is
        // shutting down, so stop cleanly.
        let entries = match std::fs::read_dir(&dir) {
            Ok(it) => it,
            Err(_) => break,
        };

        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name_str) = name.to_str() else {
                continue;
            };
            let Some(id) = name_str.strip_prefix("request_") else {
                continue;
            };
            let id = id.to_string();
            if seen.contains(&id) {
                continue;
            }

            let prompt_path = dir.join(format!("request_{id}"));
            let prompt = std::fs::read_to_string(&prompt_path)
                .unwrap_or_default()
                .trim()
                .to_string();
            let display_prompt = if prompt.is_empty() {
                "A command is requesting sudo credentials. Enter password (shown in plain text):"
                    .to_string()
            } else {
                format!(
                    "A command is requesting sudo credentials ({prompt}). Enter password (shown in plain text):"
                )
            };

            seen.insert(id.clone());

            let bus = Arc::clone(&event_bus);
            let sid = session_id.clone();
            let response_path = dir.join(format!("response_{id}"));
            tokio::spawn(async move {
                publish_question_and_wait(&bus, sid, id, display_prompt, response_path).await;
            });
        }

        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

/// Publish a [`Event::QuestionRequested`], block until the matching
/// [`Event::QuestionAnswered`] arrives (or the timeout elapses), then write
/// the password (or a cancel marker) to `response_path`.
///
/// `request_file_id` is the helper's unique id (without the `request_`/
/// `response_` prefix) and is embedded in the event's `request_id` so the TUI
/// reply can be correlated back to this waiter.
async fn publish_question_and_wait(
    event_bus: &Arc<EventBus>,
    session_id: String,
    request_file_id: String,
    prompt: String,
    response_path: PathBuf,
) {
    let request_id = format!("askpass-{request_file_id}");

    // Subscribe before publishing so we don't miss the reply.
    let mut rx = event_bus.subscribe();

    let request = Event::QuestionRequested {
        session_id: session_id.clone(),
        request_id: request_id.clone(),
        question: prompt,
        options: Vec::new(),
    };
    event_bus.publish(request);

    let deadline = tokio::time::sleep(REQUEST_TIMEOUT);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            () = &mut deadline => {
                tracing::warn!("askpass: timed out waiting for password response");
                write_cancel(&response_path);
                return;
            }
            res = rx.recv() => {
                match res {
                    Ok(Event::QuestionAnswered {
                        session_id: ref s,
                        request_id: ref rid,
                        response: ref r,
                    }) if s == &session_id && rid == &request_id => {
                        if r.is_empty() || r == DISMISS_MARKER {
                            write_cancel(&response_path);
                        } else {
                            let _ = std::fs::write(&response_path, r.as_bytes());
                        }
                        return;
                    }
                    Ok(_) => { /* unrelated event */ }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => { /* keep waiting */ }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        tracing::warn!("askpass: event bus closed while waiting for password");
                        write_cancel(&response_path);
                        return;
                    }
                }
            }
        }
    }
}

/// Write an empty response file so the helper prints nothing and exits
/// non-zero (sudo then reports a clean authentication failure).
fn write_cancel(response_path: &Path) {
    let _ = std::fs::write(response_path, b"");
}

// ── Temp directory helpers ──────────────────────────────────────────────────

/// Resolve the base directory for askpass temp files for the given session.
///
/// Uses `/tmp` on Unix. Returns `None` if `/tmp` is not writable.
fn temp_base_dir(session_id: &str) -> Option<PathBuf> {
    let safe = crate::bash::safe_session_id(session_id);
    let dir = PathBuf::from("/tmp").join(format!("ragent_{safe}"));
    if std::fs::create_dir_all(&dir).is_err() {
        return None;
    }
    Some(dir)
}

/// A monotonic-ish unique stamp for file names.
fn unique_stamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let micros = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_micros());
    let rand = {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    };
    format!("{micros}_{rand}")
}

/// Returns `true` when running on Windows (askpass is inert there).
const fn is_windows() -> bool {
    cfg!(target_os = "windows")
}

// ── Helper script body ──────────────────────────────────────────────────────

/// The POSIX shell body of the askpass helper.
///
/// sudo calls this with the prompt as `$1`. The helper writes the prompt to a
/// request file in `$RAGENT_ASKPASS_DIR`, then polls for the matching
/// `response_*` file. If the response is non-empty it is printed to stdout
/// (sudo consumes it as the password); otherwise the helper exits non-zero so
/// sudo reports an authentication failure instead of hanging on the tty.
const HELPER_BODY: &str = r#"#!/bin/sh
# ragent sudo askpass helper — bridges sudo credential requests to ragent's
# question dialog via a file-based IPC channel in $RAGENT_ASKPASS_DIR.
#
# Invoked by sudo with the prompt text as $1.

set -u

PROMPT="${1:-sudo password:}"
DIR="${RAGENT_ASKPASS_DIR:-}"
if [ -z "$DIR" ] || [ ! -d "$DIR" ]; then
    # No IPC channel — fail rather than touch the tty.
    exit 1
fi

# Unique request id = pid + timestamp.
ID="$$.$(date +%s%N 2>/dev/null || echo $$)"

REQ="$DIR/request_$ID"
RESP="$DIR/response_$ID"

# Stash the prompt for the watcher to display.
printf '%s\n' "$PROMPT" > "$REQ"

# Poll for the response file (max ~120s).
I=0
while [ ! -f "$RESP" ]; do
    I=$((I + 1))
    if [ "$I" -gt 1200 ]; then
        # Timed out — clean up and fail.
        rm -f "$REQ"
        exit 1
    fi
    sleep 0.1
done

# Read the password.
PW="$(cat "$RESP")"

# Clean up.
rm -f "$REQ" "$RESP"

if [ -z "$PW" ]; then
    # Empty response means the user cancelled.
    exit 1
fi

# sudo reads the password from the helper's stdout.
printf '%s\n' "$PW"
"#;
// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "../tests/inline/askpass.rs"]
mod askpass_tests;
