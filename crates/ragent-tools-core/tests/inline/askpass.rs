//! askpass module tests (M8/askpass).
//! Compiled as a submodule of askpass via #[path].

use super::*;

use std::sync::Arc;

use ragent_types::event::EventBus;

// ── Broker start/stop tests ──────────────────────────────────────────────

#[test]
fn test_broker_start_creates_helper_and_dir() {
    // On Windows the broker is inert; skip there.
    if is_windows() {
        return;
    }
    let id = format!(
        "askpass-start-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let broker = AskPassBroker::start(&id).expect("broker should start on posix");
    let helper = broker.helper_path.clone();
    let dir = broker.request_dir.clone();
    assert!(helper.exists(), "helper script should exist");
    assert!(dir.is_dir(), "request dir should exist");
    // Helper should be executable on unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&helper).unwrap().permissions().mode() & 0o777;
        assert!(
            mode & 0o100 != 0,
            "helper should be user-executable: {mode:o}"
        );
    }
    // env_vars should point at the helper and the request dir.
    let vars = broker.env_vars();
    assert_eq!(vars[0].0, "SUDO_ASKPASS");
    assert_eq!(vars[0].1, helper.to_string_lossy());
    assert_eq!(vars[1].0, "RAGENT_ASKPASS_DIR");
    assert_eq!(vars[1].1, dir.to_string_lossy());
    broker.stop();
    assert!(!helper.exists(), "helper removed on stop");
    assert!(!dir.exists(), "request dir removed on stop");
}

#[test]
fn test_broker_env_vars_paths_are_distinct() {
    if is_windows() {
        return;
    }
    let id = format!(
        "askpass-env-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let broker = AskPassBroker::start(&id).expect("broker should start");
    let vars = broker.env_vars();
    let askpass = PathBuf::from(&vars[0].1);
    let reqdir = PathBuf::from(&vars[1].1);
    assert_ne!(askpass, reqdir, "askpass and reqdir must differ");
    let name = askpass
        .file_name()
        .and_then(|n| n.to_str())
        .expect("helper has a file name");
    assert!(
        name.starts_with("ragent_askpass_") && name.ends_with(".sh"),
        "helper filename should be ragent_askpass_<stamp>.sh, got {name}"
    );
    assert!(reqdir.is_dir(), "request dir must exist");
    broker.stop();
}

// ── Helper script body content tests ────────────────────────────────────

#[test]
fn test_helper_body_uses_env_var_dir() {
    assert!(
        HELPER_BODY.contains("RAGENT_ASKPASS_DIR"),
        "helper must read RAGENT_ASKPASS_DIR"
    );
}

#[test]
fn test_helper_body_exits_nonzero_on_empty_response() {
    assert!(
        HELPER_BODY.contains("exit 1"),
        "helper must exit non-zero on empty/cancel"
    );
}

#[test]
fn test_helper_body_writes_request_and_reads_response() {
    assert!(HELPER_BODY.contains("request_"), "writes request_<id>");
    assert!(HELPER_BODY.contains("response_"), "reads response_<id>");
}

// ── watch_loop IPC tests ────────────────────────────────────────────────

/// Drive the watch loop directly: drop a request file, confirm a
/// [`Event::QuestionRequested`] is published, answer it, and confirm the
/// response file appears.
#[tokio::test]
async fn test_watch_loop_routes_request_to_dialog_and_writes_response() {
    if is_windows() {
        return;
    }
    let bus = Arc::new(EventBus::new(64));
    let mut rx = bus.subscribe();

    // Use a throwaway temp dir as the request dir.
    let dir = tempfile::tempdir().expect("tempdir");
    let dir = dir.path().to_path_buf();
    let session_id = "askpass-ipc-test".to_string();

    // Spawn the watch loop.
    let watch_dir = dir.clone();
    let bus2 = Arc::clone(&bus);
    let sid = session_id.clone();
    let handle = tokio::spawn(async move {
        watch_loop(watch_dir, sid, bus2).await;
    });

    // Drop a request file in.
    let id = "req1";
    let req_path = dir.join(format!("request_{id}"));
    std::fs::write(&req_path, "password for integration:\n").unwrap();

    // Expect a QuestionRequested for this id within 2s.
    let event = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
        .await
        .expect("timed out waiting for QuestionRequested")
        .expect("event bus closed");

    let Event::QuestionRequested {
        session_id: ref s,
        request_id: ref rid,
        question: ref q,
        ..
    } = event
    else {
        panic!("expected QuestionRequested, got {event:?}");
    };
    assert_eq!(s, &session_id);
    assert!(rid.starts_with("askpass-req1"));
    assert!(q.contains("sudo credentials"), "question text: {q}");

    // Answer with a password.
    bus.publish(Event::QuestionAnswered {
        session_id: session_id.clone(),
        request_id: rid.clone(),
        response: "supersecret".to_string(),
    });

    // The response file should appear with the password.
    let resp_path = dir.join(format!("response_{id}"));
    let content = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if let Ok(c) = std::fs::read_to_string(&resp_path) {
                return c;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("response file never appeared");
    assert_eq!(content, "supersecret");

    handle.abort();
}

/// When the user dismisses the question (sends the dismiss marker), the
/// watcher writes an empty response file so the helper exits non-zero.
#[tokio::test]
async fn test_watch_loop_cancel_writes_empty_response() {
    if is_windows() {
        return;
    }
    let bus = Arc::new(EventBus::new(64));
    let mut rx = bus.subscribe();

    let dir = tempfile::tempdir().expect("tempdir");
    let dir = dir.path().to_path_buf();
    let session_id = "askpass-cancel-test".to_string();

    let watch_dir = dir.clone();
    let bus2 = Arc::clone(&bus);
    let sid = session_id.clone();
    let handle = tokio::spawn(async move {
        watch_loop(watch_dir, sid, bus2).await;
    });

    let id = "req-cancel";
    std::fs::write(dir.join(format!("request_{id}")), "sudo prompt:\n").unwrap();

    let event = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv())
        .await
        .expect("timed out")
        .expect("closed");
    let Event::QuestionRequested { request_id, .. } = event else {
        panic!("expected QuestionRequested");
    };

    bus.publish(Event::QuestionAnswered {
        session_id,
        request_id,
        response: DISMISS_MARKER.to_string(),
    });

    let resp_path = dir.join(format!("response_{id}"));
    let content = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if let Ok(m) = std::fs::metadata(&resp_path)
                && m.is_file()
            {
                return std::fs::read(&resp_path).unwrap_or_default();
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    })
    .await
    .expect("response file never appeared");
    assert!(
        content.is_empty(),
        "cancel should write empty response, got {content:?}"
    );

    handle.abort();
}

// ── temp_base_dir + safe_session_id tests ────────────────────────────────

#[test]
fn test_safe_session_id_sanitizes() {
    assert_eq!(safe_session_id("abc-123"), "abc-123");
    assert_eq!(safe_session_id("abc 123"), "abc_123");
    assert_eq!(safe_session_id("abc/123"), "abc_123");
}

#[test]
fn test_temp_base_dir_is_under_tmp() {
    if is_windows() {
        return;
    }
    let dir = temp_base_dir("safe-id").expect("temp base dir");
    assert!(
        dir.starts_with("/tmp"),
        "temp base dir under /tmp: {}",
        dir.display()
    );
    let _ = std::fs::remove_dir_all(&dir);
}
