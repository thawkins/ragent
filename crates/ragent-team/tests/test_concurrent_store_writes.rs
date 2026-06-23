//! Concurrent-write regression tests for the team subsystem.
//!
//! These tests spawn multiple child processes (each running a slice of this
//! same integration-test binary) so that OS-level `flock` contention is
//! exercised, not just in-process locking.  They verify that no mailbox
//! messages or task claims/completions are lost when multiple writers hit the
//! same file-backed store.

use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use ragent_team::team::{
    Mailbox, MailboxMessage, MessageType, Task, TaskList, TaskStatus, TaskStore,
};

const MAILBOX_WORKERS: usize = 4;
const MAILBOX_MESSAGES_PER_WORKER: usize = 25;
const TASK_WORKERS: usize = 4;
const TASK_COUNT: usize = 40;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn current_test_binary() -> PathBuf {
    env::current_exe().expect("current test executable path")
}

fn spawn_worker(filter: &str, env_vars: &[(&str, &str)]) -> Result<std::process::Child> {
    let mut cmd = Command::new(current_test_binary());
    cmd.arg("--nocapture")
        .arg(filter)
        .envs(env_vars.iter().copied())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd.spawn()
        .with_context(|| format!("spawn worker process for {filter}"))
}

fn wait_for_workers(workers: Vec<std::process::Child>, filter: &str) -> Result<()> {
    for (i, mut worker) in workers.into_iter().enumerate() {
        let status = worker
            .wait()
            .with_context(|| format!("wait for worker {i} of {filter}"))?;
        if !status.success() {
            return Err(anyhow!(
                "worker {i} of {filter} exited with status {:?}",
                status.code()
            ));
        }
    }
    Ok(())
}

// ── Mailbox concurrent-write test ───────────────────────────────────────────────

/// Parent test: verify that many concurrent `Mailbox::push` calls do not lose
/// messages.
#[test]
fn parent_mailbox_concurrent_push() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let mailbox_dir = dir.path().to_path_buf();

    let mut workers = Vec::with_capacity(MAILBOX_WORKERS);
    for id in 0..MAILBOX_WORKERS {
        let env_vars = [
            ("RAGENT_TEST_MAILBOX_DIR", mailbox_dir.to_str().unwrap()),
            ("RAGENT_TEST_MAILBOX_WORKER_ID", &id.to_string()),
            (
                "RAGENT_TEST_MAILBOX_MESSAGES",
                &MAILBOX_MESSAGES_PER_WORKER.to_string(),
            ),
        ];
        workers.push(spawn_worker("worker_mailbox_push", &env_vars).expect("spawn mailbox worker"));
    }

    wait_for_workers(workers, "worker_mailbox_push").expect("mailbox workers failed");

    let mailbox = Mailbox::open(&mailbox_dir, "recipient").expect("open recipient mailbox");
    let messages = mailbox.read_all().expect("read mailbox");
    let expected = MAILBOX_WORKERS * MAILBOX_MESSAGES_PER_WORKER;
    assert_eq!(
        messages.len(),
        expected,
        "expected {expected} messages, got {}",
        messages.len()
    );

    let mut contents: Vec<String> = messages.into_iter().map(|m| m.content).collect();
    contents.sort();
    contents.dedup();
    assert_eq!(
        contents.len(),
        expected,
        "duplicate messages found after concurrent push"
    );
}

/// Worker test invoked by the parent above.  Pushes a unique batch of messages
/// to the shared mailbox and exits.
#[test]
fn worker_mailbox_push() {
    let dir = env::var("RAGENT_TEST_MAILBOX_DIR").ok();
    let worker_id = env::var("RAGENT_TEST_MAILBOX_WORKER_ID").ok();
    let count = env::var("RAGENT_TEST_MAILBOX_MESSAGES").ok();
    if dir.is_none() || worker_id.is_none() || count.is_none() {
        // Not invoked as a worker.
        return;
    }

    let dir = PathBuf::from(dir.unwrap());
    let worker_id = worker_id.unwrap();
    let count: usize = count.unwrap().parse().expect("valid message count");
    let mailbox = Mailbox::open(&dir, "recipient").expect("open mailbox");

    for seq in 0..count {
        let msg = MailboxMessage::new(
            "sender",
            "recipient",
            MessageType::Message,
            format!("worker-{worker_id}-{seq}"),
        );
        mailbox.push(msg).expect("push message");
    }
}

// ── TaskStore concurrent-write test ────────────────────────────────────────────

/// Parent test: verify that concurrent `claim_next` / `complete` calls do not
/// drop or double-claim tasks.
#[test]
fn parent_task_store_concurrent_claims() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let team_dir = dir.path().to_path_buf();

    // Seed tasks.json with a batch of independent pending tasks.
    let mut list = TaskList::new("concurrent-test");
    for i in 1..=TASK_COUNT {
        list.tasks
            .push(Task::new(format!("task-{i:03}"), "test task"));
    }
    let task_path = team_dir.join("tasks.json");
    std::fs::write(
        &task_path,
        serde_json::to_string_pretty(&list).expect("serialize task list"),
    )
    .expect("write tasks.json");

    let mut workers = Vec::with_capacity(TASK_WORKERS);
    for id in 0..TASK_WORKERS {
        let env_vars = [
            ("RAGENT_TEST_TASK_DIR", team_dir.to_str().unwrap()),
            ("RAGENT_TEST_TASK_WORKER_ID", &id.to_string()),
        ];
        workers.push(spawn_worker("worker_task_claimer", &env_vars).expect("spawn task worker"));
    }

    wait_for_workers(workers, "worker_task_claimer").expect("task workers failed");

    let store = TaskStore::open(&team_dir).expect("open task store");
    let list = store.read().expect("read task list");

    let completed: Vec<&Task> = list
        .tasks
        .iter()
        .filter(|t| t.status == TaskStatus::Completed)
        .collect();
    assert_eq!(
        completed.len(),
        TASK_COUNT,
        "expected all {} tasks to be completed, got {}",
        TASK_COUNT,
        completed.len()
    );

    let mut ids: Vec<String> = completed.iter().map(|t| t.id.clone()).collect();
    ids.sort();
    let unique = {
        let mut u = ids.clone();
        u.dedup();
        u
    };
    assert_eq!(
        ids.len(),
        unique.len(),
        "duplicate task IDs in completed set"
    );

    for task in &completed {
        assert!(
            task.assigned_to.is_some(),
            "completed task {} has no assigned_to",
            task.id
        );
    }
}

/// Worker test invoked by the parent above.  Claims available tasks one at a
/// time and completes them until the store is exhausted.
#[test]
fn worker_task_claimer() {
    let dir = env::var("RAGENT_TEST_TASK_DIR").ok();
    let worker_id = env::var("RAGENT_TEST_TASK_WORKER_ID").ok();
    if dir.is_none() || worker_id.is_none() {
        return;
    }

    let dir = PathBuf::from(dir.unwrap());
    let worker_id = worker_id.unwrap();
    let agent_id = format!("worker-{worker_id}");
    let store = TaskStore::open(&dir).expect("open task store");

    loop {
        match store.claim_next(&agent_id) {
            Ok((Some(task), false)) => {
                store
                    .complete(&task.id, &agent_id)
                    .expect("complete claimed task");
            }
            Ok((None, _)) => break,
            Ok((Some(_), true)) => {
                // Should not happen because we immediately complete each claim,
                // but treat as exhaustion to avoid an infinite loop.
                break;
            }
            Err(e) => panic!("claim_next failed for {agent_id}: {e}"),
        }
    }
}

// ── Direct unit-level regression tests ─────────────────────────────────────────

/// A simpler in-process test that directly checks the atomic-write helper by
/// interleaving many pushes from multiple threads.  Because `flock` is
/// advisory and per-process, this does not prove cross-process safety, but it
/// does prove the locking path itself does not panic or corrupt the JSON file.
#[test]
fn test_mailbox_threaded_push_does_not_corrupt() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let mailbox = Mailbox::open(dir.path(), "tm-001").expect("open mailbox");
    let mailbox = std::sync::Arc::new(mailbox);

    let mut handles = Vec::new();
    for t in 0..8 {
        let mb = Arc::clone(&mailbox);
        handles.push(std::thread::spawn(move || {
            for i in 0..50 {
                mb.push(MailboxMessage::new(
                    "lead",
                    "tm-001",
                    MessageType::Message,
                    format!("thread-{t}-{i}"),
                ))
                .expect("push");
            }
        }));
    }

    for h in handles {
        h.join().expect("thread join");
    }

    let messages = mailbox.read_all().expect("read mailbox");
    assert_eq!(messages.len(), 400, "expected 400 threaded messages");
}

/// A simple in-process test that `TaskStore::add_task` serialises correctly
/// under thread contention and never duplicates task IDs.
#[test]
fn test_task_store_threaded_add_does_not_duplicate() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let store = TaskStore::open(dir.path()).expect("open store");
    let store = std::sync::Arc::new(store);

    let mut handles = Vec::new();
    for t in 0..4 {
        let s = Arc::clone(&store);
        handles.push(std::thread::spawn(move || {
            for i in 0..25 {
                let id = format!("thread-{t}-{i}");
                let _ = s.add_task(Task::new(&id, "concurrent add"));
            }
        }));
    }

    for h in handles {
        h.join().expect("thread join");
    }

    let list = store.read().expect("read tasks");
    let mut ids: Vec<String> = list.tasks.iter().map(|t| t.id.clone()).collect();
    ids.sort();
    let unique = {
        let mut u = ids.clone();
        u.dedup();
        u
    };
    assert_eq!(
        ids.len(),
        unique.len(),
        "duplicate task IDs after concurrent add_task"
    );
}
