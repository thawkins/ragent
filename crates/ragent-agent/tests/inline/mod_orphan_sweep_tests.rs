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
    //
    // Under parallel load the launcher's `&` fork can land *after* the first
    // `killpg`, leaving one group member alive; that is the exact window the
    // production sweep re-enumerates for, so bound-poll until the group is
    // empty rather than asserting on a single racy sample. The child has to be
    // reaped each pass: an unreaped zombie still answers both signal-0 probes
    // as a live member of its group.
    let mut leader_alive = true;
    let mut group_alive = true;
    for _ in 0..200 {
        kill_stdio_child(leader_pid);
        let _ = child.try_wait();
        // The leader pid must return ESRCH to a signal-0 probe, and the group
        // must hold no live member once teardown settles. A killed grandchild
        // is re-parented to PID 1; where PID 1 is not an init that reaps
        // promptly (a container), it lingers as a zombie that still answers
        // `killpg(pgid, 0)`. A zombie has exited and holds no pipes, so it is
        // not a survivor: only non-zombie members count.
        #[allow(unsafe_code)]
        // approved: kill signal-0 probe is harmless; no safe std alternative
        unsafe {
            leader_alive = libc::kill(leader_pid as libc::pid_t, 0) == 0;
        }
        group_alive = process_group_members(leader_pid).into_iter().any(|pid| {
            std::fs::read_to_string(format!("/proc/{pid}/stat"))
                .ok()
                .and_then(|stat| stat_field_after_comm(&stat, 0).map(|s| s != "Z"))
                .unwrap_or(false)
        });
        if !leader_alive && !group_alive {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    let _ = child.wait().await;

    assert!(
        !leader_alive,
        "leader pid {leader_pid} must be killed by shutdown"
    );
    assert!(
        !group_alive,
        "process group {leader_pid} must be empty after shutdown"
    );
}
