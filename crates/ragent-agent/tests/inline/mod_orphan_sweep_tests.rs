//! Inline tests for `mod.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;

/// A stdio child leads its own process group (`process_group(0)` at spawn)
/// and shutdown kills the *group*: launcher commands (`npx`, ...) fork the
/// real server (`node`) and exit, so killing only the recorded pid orphans
/// the process actually holding the stdio pipes. Proven with a `sh` child
/// that forks a grandchild `sleep` before replacing itself.
///
/// No rmcp transport is used: `().serve(transport)` would block on a
/// JSON-RPC handshake `sleep` never answers. The shutdown path the fix
/// targets only reads the recorded pid and kills the group, which a plain
/// child exercises exactly.
#[tokio::test]
async fn shutdown_kills_the_stdio_child_process_group() {
    let mut child = tokio::process::Command::new("sh")
        .args(["-c", "sh -c 'sleep 600' & exec sleep 600"])
        .process_group(0)
        .kill_on_drop(true)
        .spawn()
        .expect("spawn sh");
    let leader_pid = child.id().expect("child pid is recorded");

    // Directly exercise the group-kill primitives: this is the same pair
    // `kill_stdio_child` issues for the pid the stdio spawn records.
    kill_stdio_child(leader_pid);
    let _ = child.wait().await;

    // The group must be gone: probing the leader pid (kill with signal 0)
    // and the process group (killpg with signal 0) must both return ESRCH.
    #[allow(unsafe_code)]
    // approved: kill/killpg signal-0 probes are harmless; no safe std alternative
    unsafe {
        assert_eq!(
            libc::kill(leader_pid as libc::pid_t, 0),
            -1,
            "leader pid {leader_pid} must be killed by shutdown"
        );
        assert_eq!(
            libc::killpg(leader_pid as libc::pid_t, 0),
            -1,
            "process group {leader_pid} must be empty after shutdown"
        );
    }
}
