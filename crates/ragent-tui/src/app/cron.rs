//! Background cron scheduler for the agent cron system (spec `agentchron`).
//!
//! This module implements the background scheduler loop that periodically
//! (at most every 30 seconds) evaluates all enabled cron events and
//! fires those whose next-due time has passed (FR-010).
//!
//! The scheduler runs on a dedicated `tokio::spawn` background task so it
//! never blocks the interactive TUI event loop (FR-017).
//!
//! ## T-010: Agent spawning and next_due advancement
//!
//! When a due event is found, the scheduler spawns a background agent run via
//! the `new_agent` / `spawn_background` path (FR-004, FR-005). For repeating
//! events, `next_due` is advanced by one duration interval. For one-shot
//! events, the event is disabled so it does not fire again. Each execution is
//! logged to `<working_dir>/log/cron-<timestamp>.jsonl` (FR-003, FR-006).
//!
//! ## T-011: Disabled-skip + unknown-agent-guard
//!
//! Disabled due events are queried separately and logged as `"skipped"`
//! (FR-007, FR-011) — they are never fired. Their `next_due` is advanced to
//! prevent re-logging on every tick.
//!
//! Before spawning, the agent type is validated with
//! `resolve_agent_with_customs` (FR-016). Unknown agent types are logged as
//! `"error"` and not spawned; the event is still advanced/disabled so it does
//! not retry on every tick.
//!
//! The no-double-fire guard (T-012, FR-012) prevents a repeating event from
//! firing a second concurrent run while its previous execution is still active.
//! A shared `RunningEvents` set tracks which repeating event IDs are currently
//! running; the scheduler skips and logs `"skipped"` for any due event already
//! in the set.
//!
//! See `specs/agentchron/SPEC.md` for the full specification.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::Utc;
use ragent_storage::Storage;
use ragent_tools_core::cron_log::{CronOutcome, log_cron_execution};

/// Shared set of repeating event IDs whose previous execution is still
/// running (FR-012). The scheduler checks this before firing to avoid
/// spawning a second concurrent run for the same event.
type RunningEvents = Arc<Mutex<HashSet<String>>>;

/// Scheduler tick interval (FR-010: at most every 30 seconds).
const CRON_TICK_INTERVAL_SECS: u64 = 30;

/// Synthetic parent session ID used for cron-spawned agent runs.
const CRON_PARENT_SESSION_ID: &str = "cron-scheduler";

/// Handle to the background cron scheduler task.
///
/// Created by [`start_cron_scheduler`] and used to stop the scheduler
/// cleanly on TUI shutdown. The scheduler is also stopped automatically
/// when the handle is dropped.
pub struct CronSchedulerHandle {
    cancel: Arc<AtomicBool>,
}

impl CronSchedulerHandle {
    /// Signal the scheduler loop to stop.
    pub fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl Drop for CronSchedulerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Start the background cron scheduler (FR-010, FR-017).
///
/// Returns a [`CronSchedulerHandle`] that can be used to stop the
/// scheduler on shutdown. The scheduler runs on a background tokio task
/// and never blocks the TUI event loop.
///
/// # Arguments
///
/// - `storage` — shared SQLite storage handle for querying due cron events.
/// - `session_processor` �� shared session processor for spawning agent runs
///   via the `new_agent` / `spawn_background` path (FR-004, FR-005).
/// - `working_dir` — project working directory used for execution logging
///   (`<working_dir>/log/cron-<timestamp>.jsonl`).
pub fn start_cron_scheduler(
    storage: Arc<Storage>,
    session_processor: Arc<ragent_agent::session::processor::SessionProcessor>,
    working_dir: PathBuf,
) -> CronSchedulerHandle {
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_clone = Arc::clone(&cancel);
    let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
    tokio::spawn(async move {
        cron_scheduler_loop(
            storage,
            session_processor,
            working_dir,
            cancel_clone,
            running_events,
        )
        .await;
    });
    CronSchedulerHandle { cancel }
}

/// The main scheduler loop: tick every 30 seconds, evaluate due events.
///
/// Exits cleanly when the `cancel` flag is set.
async fn cron_scheduler_loop(
    storage: Arc<Storage>,
    session_processor: Arc<ragent_agent::session::processor::SessionProcessor>,
    working_dir: PathBuf,
    cancel: Arc<AtomicBool>,
    running_events: RunningEvents,
) {
    tracing::info!("Cron scheduler started ({}s tick)", CRON_TICK_INTERVAL_SECS);

    loop {
        if cancel.load(Ordering::Relaxed) {
            tracing::info!("Cron scheduler stopping (cancel signal)");
            break;
        }

        // Execute one tick.
        cron_tick(&storage, &session_processor, &working_dir, &running_events).await;

        // Wait for the next tick interval, checking the cancel flag
        // every second so shutdown is responsive.
        for _ in 0..CRON_TICK_INTERVAL_SECS {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    tracing::info!("Cron scheduler stopped");
}

/// Execute one scheduler tick: query due events and process them (FR-004,
/// FR-005, FR-007, FR-011, FR-016).
///
/// Queries [`Storage::list_due_cron_events`] for enabled events whose
/// `next_due` has passed. For each due event:
///
/// - **Repeating** (FR-004): spawns a background agent run via
///   `spawn_background` and advances `next_due` by one duration interval.
/// - **One-shot** (FR-005): spawns a background agent run and disables the
///   event so it does not fire again.
///
/// Before spawning, the agent type is validated (FR-016); unknown agents are
/// logged as `"error"` and not spawned.
///
/// Disabled due events are also queried and logged as `"skipped"` (FR-007,
/// FR-011). Their `next_due` is advanced so they are not re-logged every tick.
///
/// Each execution is logged to `<working_dir>/log/cron-<timestamp>.jsonl`
/// (FR-003, FR-006).
///
/// FR-012 (no-double-fire): before firing a repeating event, the scheduler
/// checks the `running_events` set. If the event ID is already present (its
/// previous execution is still running), the current cycle is skipped and
/// logged as `"skipped"` instead of spawning a second concurrent run.
async fn cron_tick(
    storage: &Storage,
    session_processor: &ragent_agent::session::processor::SessionProcessor,
    working_dir: &std::path::Path,
    running_events: &RunningEvents,
) {
    let now = Utc::now();

    // Process enabled due events (fire or log error for unknown agent).
    match storage.list_due_cron_events(&now) {
        Ok(events) => {
            if events.is_empty() {
                tracing::debug!("Cron tick: no due events");
            } else {
                tracing::info!("Cron tick: {} due event(s)", events.len());
                for event in &events {
                    // FR-012: no-double-fire guard for repeating events.
                    // If this repeating event's previous execution is still
                    // running, skip this cycle and log "skipped".
                    let is_repeating =
                        event.duration_secs.is_some() && event.schedule_form != "one_shot";
                    if is_repeating {
                        let already_running = running_events
                            .lock()
                            .map(|set| set.contains(&event.id))
                            .unwrap_or(false);
                        if already_running {
                            tracing::info!(
                                event_id = %event.id,
                                "Skipping due repeating event — previous execution still running (FR-012)",
                            );
                            log_cron_execution(
                                working_dir,
                                &event.id,
                                &event.agent_type,
                                &event.prompt,
                                &event.schedule_raw,
                                CronOutcome::Skipped,
                                Some("Previous execution still running"),
                                None,
                            );
                            // Advance next_due so this cycle is not re-evaluated
                            // on every subsequent tick.
                            advance_repeating_event(storage, event, now);
                            continue;
                        }
                    }

                    fire_cron_event(
                        storage,
                        session_processor,
                        working_dir,
                        event,
                        now,
                        running_events,
                    )
                    .await;
                }
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "Cron tick: failed to list due events");
        }
    }

    // Process disabled due events: log "skipped" and advance next_due
    // (FR-007, FR-011).
    match storage.list_disabled_due_cron_events(&now) {
        Ok(events) => {
            if !events.is_empty() {
                tracing::debug!("Cron tick: {} disabled due event(s) to skip", events.len());
                for event in &events {
                    skip_disabled_event(storage, working_dir, event, now);
                }
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "Cron tick: failed to list disabled due events");
        }
    }
}

/// Fire a single due cron event: spawn an agent run and advance/disable.
///
/// FR-004 (repeating): spawn agent run + advance `next_due` by one interval.
/// FR-005 (one-shot): spawn agent run + disable event.
/// FR-016: unknown agent types are logged as `"error"` and not spawned.
/// FR-012: after a successful spawn of a repeating event, the event ID is
/// added to `running_events` so subsequent ticks skip it while the run is
/// active. A monitor task removes the ID when the background run completes.
async fn fire_cron_event(
    storage: &Storage,
    session_processor: &ragent_agent::session::processor::SessionProcessor,
    working_dir: &std::path::Path,
    event: &ragent_storage::CronEventRow,
    now: chrono::DateTime<Utc>,
    running_events: &RunningEvents,
) {
    tracing::info!(
        event_id = %event.id,
        agent_type = %event.agent_type,
        schedule_form = %event.schedule_form,
        "Firing cron event",
    );

    // FR-016: validate the agent type before spawning. If the agent type is
    // unknown to the system (not a built-in and no custom OASF definition),
    // log "error" and skip the spawn.
    let (all_agents, _) = ragent_agent::agent::load_all_agents(working_dir);
    let agent_known = all_agents.iter().any(|a| a.name == event.agent_type);
    if !agent_known {
        tracing::warn!(
            event_id = %event.id,
            agent_type = %event.agent_type,
            "Unknown agent type for cron event; skipping spawn",
        );

        log_cron_execution(
            working_dir,
            &event.id,
            &event.agent_type,
            &event.prompt,
            &event.schedule_raw,
            CronOutcome::Error,
            Some(&format!("Unknown agent type: {}", event.agent_type)),
            None,
        );

        // Still advance/disable so the event doesn't retry on every tick.
        advance_or_disable_after_error(storage, event, now);
        return;
    }

    // Determine if this is a repeating or one-shot event.
    let is_repeating = event.duration_secs.is_some() && event.schedule_form != "one_shot";

    // Attempt to spawn a background agent run via the new_agent path.
    // For stateful events, load the cross-run loop state and inject it
    // into the prompt (FR-004).
    let effective_prompt = if event.stateful {
        let state =
            ragent_agent::loop_state::LoopState::load(working_dir, &event.id).unwrap_or_default();
        ragent_agent::loop_state::inject_state_into_prompt(&event.prompt, &state)
    } else {
        event.prompt.clone()
    };
    let spawn_result =
        spawn_agent_run(session_processor, event, &effective_prompt, working_dir).await;

    // Always advance/disable after the spawn attempt, so the event does not
    // fire again on the next tick regardless of spawn success or failure.
    if is_repeating {
        // FR-004: advance next_due by one duration interval.
        advance_repeating_event(storage, event, now);
    } else {
        // FR-005: disable the one-shot event so it does not fire again.
        if let Err(e) = storage.set_cron_event_enabled(&event.id, false) {
            tracing::warn!(
                event_id = %event.id,
                error = %e,
                "Failed to disable one-shot cron event after firing",
            );
        }
    }

    // Log the execution outcome (FR-003, FR-006).
    match spawn_result {
        Ok(task_entry) => {
            // FR-012: for repeating events, track the event ID as running so
            // subsequent ticks skip it while this background run is active.
            // A monitor task removes the ID when the run completes.
            if is_repeating {
                if let Ok(mut set) = running_events.lock() {
                    set.insert(event.id.clone());
                }
                spawn_completion_monitor(
                    session_processor,
                    &task_entry.id,
                    event.id.clone(),
                    Arc::clone(running_events),
                    event.stateful,
                    working_dir.to_path_buf(),
                );
            }

            log_cron_execution(
                working_dir,
                &event.id,
                &event.agent_type,
                &event.prompt,
                &event.schedule_raw,
                CronOutcome::Success,
                None,
                Some(&task_entry.id),
            );
        }
        Err(e) => {
            tracing::warn!(
                event_id = %event.id,
                agent_type = %event.agent_type,
                error = %e,
                "Failed to spawn agent run for cron event",
            );

            log_cron_execution(
                working_dir,
                &event.id,
                &event.agent_type,
                &event.prompt,
                &event.schedule_raw,
                CronOutcome::Error,
                Some(&e.to_string()),
                None,
            );
        }
    }
}

/// Spawn a background agent run for a due cron event via the `new_agent` path.
///
/// Uses `AgentManager::spawn_background` which creates an isolated session,
/// resolves the agent, and runs the prompt in a background tokio task.
async fn spawn_agent_run(
    session_processor: &ragent_agent::session::processor::SessionProcessor,
    event: &ragent_storage::CronEventRow,
    prompt: &str,
    working_dir: &std::path::Path,
) -> anyhow::Result<ragent_agent::task::TaskEntry> {
    let agent_manager = session_processor
        .agent_manager
        .get()
        .ok_or_else(|| anyhow::anyhow!("AgentManager not initialized"))?;

    agent_manager
        .spawn_background(
            CRON_PARENT_SESSION_ID,
            &event.agent_type,
            prompt,
            None, // no model override — use the configured default
            working_dir,
        )
        .await
}

/// Spawn a background monitor task that polls the AgentManager until the
/// spawned agent run completes, then removes the event ID from the
/// `running_events` set (FR-012).
///
/// This runs on its own tokio task so it does not block the scheduler loop.
/// The poll interval is 5 seconds — short enough for responsiveness but
/// not so frequent as to cause lock contention.
fn spawn_completion_monitor(
    session_processor: &ragent_agent::session::processor::SessionProcessor,
    task_id: &str,
    event_id: String,
    running_events: RunningEvents,
    stateful: bool,
    working_dir: PathBuf,
) {
    let agent_manager = match session_processor.agent_manager.get().cloned() {
        Some(tm) => tm,
        None => {
            // Should not happen (we just spawned via the AgentManager), but
            // if it does, remove the event ID immediately.
            if let Ok(mut set) = running_events.lock() {
                set.remove(&event_id);
            }
            return;
        }
    };

    let task_id = task_id.to_string();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;

            let task = agent_manager.get_task(&task_id).await;
            let done = match task {
                Some(entry) => entry.status != ragent_agent::task::TaskStatus::Running,
                None => true, // task was removed — consider it done
            };

            if done {
                if let Ok(mut set) = running_events.lock() {
                    set.remove(&event_id);
                }
                tracing::info!(
                    event_id = %event_id,
                    task_id = %task_id,
                    "Cron background run completed; removed from running_events",
                );

                // FR-004: For stateful events, parse the completed task's
                // output for `<loop-state>` and `<inbox>` tags.
                if stateful {
                    if let Some(entry) = agent_manager.get_task(&task_id).await {
                        if let Some(result) = &entry.result {
                            let parsed = ragent_agent::loop_state::parse_tags(result);
                            if !parsed.loop_state.is_empty() {
                                let state = ragent_agent::loop_state::LoopState {
                                    content: parsed.loop_state.clone(),
                                };
                                if let Err(e) = state.save(&working_dir, &event_id) {
                                    tracing::warn!(
                                        event_id = %event_id,
                                        error = %e,
                                        "Failed to save loop state for stateful cron event",
                                    );
                                }
                            }
                            if !parsed.inbox_entries.is_empty() {
                                let entries: Vec<_> = parsed
                                    .inbox_entries
                                    .iter()
                                    .map(|content| {
                                        ragent_agent::loop_state::InboxEntry::new(
                                            &event_id, content,
                                        )
                                    })
                                    .collect();
                                if let Err(e) = ragent_agent::loop_state::write_inbox_entries(
                                    &working_dir,
                                    &entries,
                                ) {
                                    tracing::warn!(
                                        event_id = %event_id,
                                        error = %e,
                                        "Failed to write inbox entries for stateful cron event",
                                    );
                                }
                            }
                        }
                    }
                }

                break;
            }
        }
    });
}

/// Advance a repeating event's `next_due` by one duration interval (FR-004).
///
/// Parses the event's schedule fields from the storage row, reconstructs a
/// `CronSchedule`, calls `advance_next_due`, and persists the new timestamp.
fn advance_repeating_event(
    storage: &Storage,
    event: &ragent_storage::CronEventRow,
    now: chrono::DateTime<Utc>,
) {
    // Parse the current next_due from the stored ISO-8601 string.
    let current_next_due = match chrono::DateTime::parse_from_rfc3339(&event.next_due) {
        Ok(dt) => dt.with_timezone(&Utc),
        Err(e) => {
            tracing::warn!(
                event_id = %event.id,
                next_due = %event.next_due,
                error = %e,
                "Failed to parse next_due; skipping advancement",
            );
            return;
        }
    };

    // Parse the schedule form.
    let form = match serde_json::from_str::<ragent_types::CronForm>(&format!(
        "\"{}\"",
        event.schedule_form
    )) {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!(
                event_id = %event.id,
                form = %event.schedule_form,
                error = %e,
                "Failed to parse schedule form; skipping advancement",
            );
            return;
        }
    };

    // Parse start_at (optional, only for OneShot and RepeatFrom).
    let start_at = event
        .start_at
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    // Reconstruct the CronSchedule.
    let schedule = ragent_types::CronSchedule {
        form,
        start_at,
        duration_secs: event.duration_secs,
    };

    // Compute the advanced next_due.
    match schedule.advance_next_due(current_next_due, now) {
        Some(new_next_due) => {
            if let Err(e) = storage.update_cron_event_next_due(&event.id, &new_next_due, Some(&now))
            {
                tracing::warn!(
                    event_id = %event.id,
                    error = %e,
                    "Failed to update next_due after firing",
                );
            }
            tracing::info!(
                event_id = %event.id,
                new_next_due = %new_next_due,
                "Advanced cron event next_due",
            );
        }
        None => {
            // This should not happen for repeating events, but handle gracefully.
            tracing::warn!(
                event_id = %event.id,
                "advance_next_due returned None for a repeating event; disabling",
            );
            let _ = storage.set_cron_event_enabled(&event.id, false);
        }
    }
}

/// Advance or disable an event after a non-spawn error (e.g. unknown agent
/// type). This prevents the event from being re-evaluated on every subsequent
/// tick.
///
/// - **Repeating**: advance `next_due` by one interval (same as a normal fire).
/// - **One-shot**: disable the event.
fn advance_or_disable_after_error(
    storage: &Storage,
    event: &ragent_storage::CronEventRow,
    now: chrono::DateTime<Utc>,
) {
    let is_repeating = event.duration_secs.is_some() && event.schedule_form != "one_shot";
    if is_repeating {
        advance_repeating_event(storage, event, now);
    } else if let Err(e) = storage.set_cron_event_enabled(&event.id, false) {
        tracing::warn!(
            event_id = %event.id,
            error = %e,
            "Failed to disable one-shot cron event after error",
        );
    }
}

/// Skip a disabled due event (FR-007, FR-011).
///
/// Logs a `"skipped"` outcome and advances `next_due` so the event is not
/// re-logged on every tick:
///
/// - **Repeating**: advance `next_due` by one interval.
/// - **One-shot**: set `next_due` to a far-future timestamp so it never
///   becomes due again (the event is already disabled).
///
/// Does **not** spawn an agent run (FR-011: "shall not fire").
fn skip_disabled_event(
    storage: &Storage,
    working_dir: &std::path::Path,
    event: &ragent_storage::CronEventRow,
    now: chrono::DateTime<Utc>,
) {
    tracing::info!(
        event_id = %event.id,
        agent_type = %event.agent_type,
        "Skipping disabled cron event",
    );

    log_cron_execution(
        working_dir,
        &event.id,
        &event.agent_type,
        &event.prompt,
        &event.schedule_raw,
        CronOutcome::Skipped,
        Some("Event is disabled"),
        None,
    );

    // Advance next_due to prevent re-logging on every tick.
    let is_repeating = event.duration_secs.is_some() && event.schedule_form != "one_shot";
    if is_repeating {
        advance_repeating_event(storage, event, now);
    } else {
        // Set next_due to far future so this disabled one-shot event is not
        // returned by list_disabled_due_cron_events on every tick.
        let far_future = chrono::DateTime::parse_from_rfc3339("9999-12-31T23:59:59Z")
            .expect("valid far-future timestamp")
            .with_timezone(&Utc);
        let _ = storage.update_cron_event_next_due(&event.id, &far_future, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ragent_storage::Storage;
    use ragent_types::{CronEvent, CronSchedule};

    /// Verify that `cron_tick` does not panic when there are no events.
    #[tokio::test]
    async fn test_cron_tick_no_events() {
        let storage = Storage::open_in_memory().expect("storage");
        let working_dir = unique_temp_dir();
        let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
        // SessionProcessor is not available in unit tests, but cron_tick
        // should handle the no-events case without needing it.
        // We test the no-events path by calling cron_tick with a mock
        // session processor reference. Since there are no due events,
        // the processor is never accessed.
        let processor = create_test_processor();
        cron_tick(&storage, &processor, &working_dir, &running_events).await;
    }

    /// Verify that `cron_tick` picks up a due event and processes it
    /// (spawning fails gracefully because no AgentManager is set).
    #[tokio::test]
    async fn test_cron_tick_with_due_event() {
        let storage = Storage::open_in_memory().expect("storage");
        let working_dir = unique_temp_dir();
        let past = chrono::Utc::now() - chrono::Duration::hours(1);
        let event = CronEvent::new(
            "test-due".to_string(),
            "general".to_string(),
            "test prompt".to_string(),
            CronSchedule::one_shot(past),
            "at past".to_string(),
            past,
        );
        storage.insert_cron_event(&event).expect("insert");
        let processor = create_test_processor();
        let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
        cron_tick(&storage, &processor, &working_dir, &running_events).await;
        // The one-shot event should be disabled after firing (FR-005),
        // even though the spawn failed (no AgentManager set).
        let row = storage
            .get_cron_event("test-due")
            .expect("get")
            .expect("found");
        assert!(
            !row.enabled,
            "one-shot event should be disabled after firing"
        );
    }

    /// Verify that a repeating event's next_due is advanced after firing.
    #[tokio::test]
    async fn test_cron_tick_repeating_event_advances_next_due() {
        let storage = Storage::open_in_memory().expect("storage");
        let working_dir = unique_temp_dir();
        let now = chrono::Utc::now();
        let past = now - chrono::Duration::hours(2);
        let event = CronEvent::new(
            "test-repeat".to_string(),
            "general".to_string(),
            "test prompt".to_string(),
            CronSchedule::repeat_from(past, 3600), // 1h interval
            "from past every 1h".to_string(),
            past,
        );
        storage.insert_cron_event(&event).expect("insert");
        let processor = create_test_processor();
        let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
        cron_tick(&storage, &processor, &working_dir, &running_events).await;
        // The repeating event should still be enabled with an advanced next_due.
        let row = storage
            .get_cron_event("test-repeat")
            .expect("get")
            .expect("found");
        assert!(row.enabled, "repeating event should still be enabled");
        let new_next_due =
            chrono::DateTime::parse_from_rfc3339(&row.next_due).expect("parse next_due");
        assert!(
            new_next_due.with_timezone(&Utc) > now,
            "next_due should be advanced to the future"
        );
    }

    /// Verify that a disabled due event is skipped and logged as "skipped"
    /// (FR-007, FR-011). The event should not be fired (no agent spawn
    /// attempted) and its next_due should be advanced.
    #[tokio::test]
    async fn test_cron_tick_disabled_event_skipped() {
        let storage = Storage::open_in_memory().expect("storage");
        let working_dir = unique_temp_dir();
        let now = chrono::Utc::now();
        let past = now - chrono::Duration::hours(1);
        let event = CronEvent::new(
            "test-disabled".to_string(),
            "general".to_string(),
            "test prompt".to_string(),
            CronSchedule::repeat_from(past, 3600), // 1h interval
            "from past every 1h".to_string(),
            past,
        );
        storage.insert_cron_event(&event).expect("insert");
        // Disable the event.
        storage
            .set_cron_event_enabled("test-disabled", false)
            .expect("disable");

        let processor = create_test_processor();
        let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
        cron_tick(&storage, &processor, &working_dir, &running_events).await;

        // The disabled event should NOT be fired — it remains disabled.
        let row = storage
            .get_cron_event("test-disabled")
            .expect("get")
            .expect("found");
        assert!(
            !row.enabled,
            "disabled event should still be disabled after tick"
        );
        // next_due should be advanced to the future (skip logged + advanced).
        let new_next_due =
            chrono::DateTime::parse_from_rfc3339(&row.next_due).expect("parse next_due");
        assert!(
            new_next_due.with_timezone(&Utc) > now,
            "disabled repeating event next_due should be advanced after skip"
        );

        // Verify a "skipped" log entry was written.
        let logs = ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("test-disabled"));
        assert!(
            logs.iter().any(|e| e.outcome == "skipped"),
            "expected a 'skipped' log entry for disabled event"
        );
    }

    /// Verify that an event with an unknown agent type is logged as "error"
    /// and not spawned (FR-016). The event should still be advanced/disabled
    /// so it doesn't retry on every tick.
    #[tokio::test]
    async fn test_cron_tick_unknown_agent_type_error() {
        let storage = Storage::open_in_memory().expect("storage");
        let working_dir = unique_temp_dir();
        let past = chrono::Utc::now() - chrono::Duration::hours(1);
        let event = CronEvent::new(
            "test-unknown-agent".to_string(),
            "nonexistent-agent-xyz".to_string(),
            "test prompt".to_string(),
            CronSchedule::one_shot(past),
            "at past".to_string(),
            past,
        );
        storage.insert_cron_event(&event).expect("insert");
        let processor = create_test_processor();
        let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
        cron_tick(&storage, &processor, &working_dir, &running_events).await;

        // The one-shot event should be disabled after the error (not retried).
        let row = storage
            .get_cron_event("test-unknown-agent")
            .expect("get")
            .expect("found");
        assert!(
            !row.enabled,
            "one-shot event with unknown agent should be disabled after error"
        );

        // Verify an "error" log entry was written with the unknown agent message.
        let logs =
            ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("test-unknown-agent"));
        let error_entry = logs
            .iter()
            .find(|e| e.outcome == "error")
            .expect("expected an 'error' log entry");
        assert!(
            error_entry
                .error
                .as_ref()
                .is_some_and(|msg| msg.contains("Unknown agent type")),
            "error message should mention 'Unknown agent type', got: {:?}",
            error_entry.error
        );
    }

    /// Verify that a repeating event whose previous execution is still
    /// running is skipped and logged as `"skipped"` (FR-012).
    ///
    /// We simulate a running event by manually inserting its ID into the
    /// `running_events` set before calling `cron_tick`. The scheduler should
    /// skip the cycle, log `"skipped"`, and advance `next_due` — without
    /// attempting to spawn an agent run.
    #[tokio::test]
    async fn test_cron_tick_repeating_event_double_fire_guard() {
        let storage = Storage::open_in_memory().expect("storage");
        let working_dir = unique_temp_dir();
        let now = chrono::Utc::now();
        let past = now - chrono::Duration::hours(2);
        let event = CronEvent::new(
            "test-double-fire".to_string(),
            "general".to_string(),
            "test prompt".to_string(),
            CronSchedule::repeat_from(past, 3600), // 1h interval
            "from past every 1h".to_string(),
            past,
        );
        storage.insert_cron_event(&event).expect("insert");

        // Simulate a still-running previous execution by pre-inserting the
        // event ID into the running_events set.
        let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
        {
            let mut set = running_events.lock().expect("lock");
            set.insert("test-double-fire".to_string());
        }

        let processor = create_test_processor();
        cron_tick(&storage, &processor, &working_dir, &running_events).await;

        // The repeating event should still be enabled (it was skipped, not fired).
        let row = storage
            .get_cron_event("test-double-fire")
            .expect("get")
            .expect("found");
        assert!(
            row.enabled,
            "skipped repeating event should still be enabled"
        );

        // next_due should be advanced to the future (skip logged + advanced).
        let new_next_due =
            chrono::DateTime::parse_from_rfc3339(&row.next_due).expect("parse next_due");
        assert!(
            new_next_due.with_timezone(&Utc) > now,
            "skipped repeating event next_due should be advanced to the future"
        );

        // Verify a "skipped" log entry was written with the double-fire reason.
        let logs =
            ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("test-double-fire"));
        let skipped_entry = logs
            .iter()
            .find(|e| e.outcome == "skipped")
            .expect("expected a 'skipped' log entry for double-fire guard");
        assert!(
            skipped_entry
                .error
                .as_ref()
                .is_some_and(|msg| msg.contains("Previous execution still running")),
            "skipped log entry should mention 'Previous execution still running', got: {:?}",
            skipped_entry.error
        );

        // The event ID should still be in the running_events set (the guard
        // does not remove it — only the completion monitor does).
        {
            let set = running_events.lock().expect("lock");
            assert!(
                set.contains("test-double-fire"),
                "event ID should remain in running_events set after double-fire skip"
            );
        }
    }

    /// Integration test: a repeating event fires, logs, and advances
    /// `next_due` by exactly one duration interval (FR-004, FR-006).
    ///
    /// Because the test processor has no `AgentManager`, the spawn fails and
    /// the outcome is `"error"`. We still verify the full pipeline:
    /// the log entry is written with the correct fields, the event remains
    /// enabled, and `next_due` advances by exactly one interval.
    #[tokio::test]
    async fn test_integration_repeating_event_fires_logs_advances() {
        let storage = Storage::open_in_memory().expect("storage");
        let working_dir = unique_temp_dir();
        let now = chrono::Utc::now();
        let past = now - chrono::Duration::hours(2);
        let duration_secs: i64 = 3600; // 1h interval
        let event = CronEvent::new(
            "integration-repeat".to_string(),
            "general".to_string(),
            "run integration test".to_string(),
            CronSchedule::repeat_from(past, duration_secs),
            "from past every 1h".to_string(),
            past,
        );
        storage.insert_cron_event(&event).expect("insert");

        let processor = create_test_processor();
        let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
        cron_tick(&storage, &processor, &working_dir, &running_events).await;

        // FR-004: repeating event should still be enabled after firing.
        let row = storage
            .get_cron_event("integration-repeat")
            .expect("get")
            .expect("found");
        assert!(
            row.enabled,
            "repeating event should still be enabled after firing"
        );

        // FR-004: next_due should be advanced by exactly one duration interval
        // from the original next_due (which was `past`).
        let original_next_due = event.next_due;
        let new_next_due =
            chrono::DateTime::parse_from_rfc3339(&row.next_due).expect("parse new next_due");
        let expected_advance = original_next_due + chrono::Duration::seconds(duration_secs);
        // The advance may skip ahead multiple intervals if behind, so the new
        // next_due must be >= original + one interval and in the future.
        assert!(
            new_next_due.with_timezone(&Utc) >= expected_advance,
            "next_due should be advanced by at least one interval: \
             original={original_next_due}, new={new_next_due}, expected>={expected_advance}"
        );
        assert!(
            new_next_due.with_timezone(&Utc) > now,
            "next_due should be in the future after advancement"
        );

        // FR-006: a log entry should have been written with the correct fields.
        let logs =
            ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("integration-repeat"));
        assert!(
            !logs.is_empty(),
            "expected at least one log entry for the fired repeating event"
        );
        let entry = &logs[0];
        assert_eq!(entry.event_id, "integration-repeat");
        assert_eq!(entry.agent_type, "general");
        assert_eq!(entry.prompt, "run integration test");
        assert_eq!(entry.schedule, "from past every 1h");
        // Outcome is "error" because spawn fails without a AgentManager.
        assert_eq!(
            entry.outcome, "error",
            "outcome should be 'error' (spawn fails without AgentManager)"
        );
        assert!(
            entry.error.is_some(),
            "error field should be populated when outcome is 'error'"
        );
        // Timestamp should be a valid RFC 3339 string.
        chrono::DateTime::parse_from_rfc3339(&entry.timestamp)
            .expect("timestamp should be valid RFC 3339");
    }

    /// Integration test: a one-shot event fires, logs, and is disabled
    /// (FR-005, FR-006).
    ///
    /// Because the test processor has no `AgentManager`, the spawn fails and
    /// the outcome is `"error"`. We still verify the full pipeline:
    /// the log entry is written with the correct fields, and the event is
    /// disabled so it does not fire again.
    #[tokio::test]
    async fn test_integration_one_shot_event_fires_logs_disabled() {
        let storage = Storage::open_in_memory().expect("storage");
        let working_dir = unique_temp_dir();
        let past = chrono::Utc::now() - chrono::Duration::hours(1);
        let event = CronEvent::new(
            "integration-oneshot".to_string(),
            "coder".to_string(),
            "do a one-shot thing".to_string(),
            CronSchedule::one_shot(past),
            "at past".to_string(),
            past,
        );
        storage.insert_cron_event(&event).expect("insert");

        let processor = create_test_processor();
        let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
        cron_tick(&storage, &processor, &working_dir, &running_events).await;

        // FR-005: one-shot event should be disabled after firing.
        let row = storage
            .get_cron_event("integration-oneshot")
            .expect("get")
            .expect("found");
        assert!(
            !row.enabled,
            "one-shot event should be disabled after firing"
        );

        // FR-006: a log entry should have been written with the correct fields.
        let logs =
            ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("integration-oneshot"));
        assert!(
            !logs.is_empty(),
            "expected at least one log entry for the fired one-shot event"
        );
        let entry = &logs[0];
        assert_eq!(entry.event_id, "integration-oneshot");
        assert_eq!(entry.agent_type, "coder");
        assert_eq!(entry.prompt, "do a one-shot thing");
        assert_eq!(entry.schedule, "at past");
        assert_eq!(
            entry.outcome, "error",
            "outcome should be 'error' (spawn fails without AgentManager)"
        );
        assert!(
            entry.error.is_some(),
            "error field should be populated when outcome is 'error'"
        );
        // Timestamp should be a valid RFC 3339 string.
        chrono::DateTime::parse_from_rfc3339(&entry.timestamp)
            .expect("timestamp should be valid RFC 3339");
    }

    /// Integration test: the JSONL log entry contains all fields required by
    /// FR-006 (event id, agent type, prompt, outcome, error, timestamp).
    ///
    /// Fires both a repeating and a one-shot event in separate working dirs,
    /// then reads back the logs and verifies every FR-006 field is present
    /// and correctly typed.
    #[tokio::test]
    async fn test_integration_log_entry_contains_all_fr006_fields() {
        // --- Repeating event ---
        let storage = Storage::open_in_memory().expect("storage");
        let working_dir = unique_temp_dir();
        let past = chrono::Utc::now() - chrono::Duration::hours(1);
        let event = CronEvent::new(
            "fr006-repeat".to_string(),
            "general".to_string(),
            "fr006 repeating prompt".to_string(),
            CronSchedule::repeat_from(past, 3600),
            "from past every 1h".to_string(),
            past,
        );
        storage.insert_cron_event(&event).expect("insert");
        let processor = create_test_processor();
        let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
        cron_tick(&storage, &processor, &working_dir, &running_events).await;

        let logs = ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("fr006-repeat"));
        assert_eq!(
            logs.len(),
            1,
            "expected exactly one log entry for the repeating event"
        );
        let entry = &logs[0];
        // FR-006: event id
        assert_eq!(entry.event_id, "fr006-repeat");
        // FR-006: agent type
        assert_eq!(entry.agent_type, "general");
        // FR-006: prompt
        assert_eq!(entry.prompt, "fr006 repeating prompt");
        // FR-006: outcome
        assert!(
            matches!(entry.outcome.as_str(), "success" | "error" | "skipped"),
            "outcome should be one of success/error/skipped, got: {}",
            entry.outcome
        );
        // FR-006: error message (present when outcome is "error")
        if entry.outcome == "error" {
            assert!(
                entry.error.is_some(),
                "error field must be populated when outcome is 'error'"
            );
        }
        // FR-006: timestamp (valid RFC 3339)
        let ts = chrono::DateTime::parse_from_rfc3339(&entry.timestamp)
            .expect("valid RFC 3339 timestamp");
        // Timestamp should be recent (within the last few seconds).
        let now = chrono::Utc::now();
        let age = now.signed_duration_since(ts.with_timezone(&Utc));
        assert!(
            age.num_seconds() < 10,
            "log timestamp should be recent, got age={age}"
        );
        // FR-006: schedule field
        assert_eq!(entry.schedule, "from past every 1h");

        // --- One-shot event ---
        let storage2 = Storage::open_in_memory().expect("storage");
        let working_dir2 = unique_temp_dir();
        let event2 = CronEvent::new(
            "fr006-oneshot".to_string(),
            "ask".to_string(),
            "fr006 one-shot prompt".to_string(),
            CronSchedule::one_shot(past),
            "at past".to_string(),
            past,
        );
        storage2.insert_cron_event(&event2).expect("insert");
        let processor2 = create_test_processor();
        let running_events2: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
        cron_tick(&storage2, &processor2, &working_dir2, &running_events2).await;

        let logs2 =
            ragent_tools_core::cron_log::read_cron_log(&working_dir2, Some("fr006-oneshot"));
        // The one-shot event may produce two log entries in the same tick:
        // 1. "error" from fire_cron_event (spawn fails → disabled)
        // 2. "skipped" from skip_disabled_event (disabled event found again
        //    in the same tick's disabled-due query before next_due is pushed
        //    to the far future). We verify the "error" entry has all fields.
        let error_entry2 = logs2
            .iter()
            .find(|e| e.outcome == "error")
            .expect("expected an 'error' log entry for the one-shot event");
        assert_eq!(error_entry2.event_id, "fr006-oneshot");
        assert_eq!(error_entry2.agent_type, "ask");
        assert_eq!(error_entry2.prompt, "fr006 one-shot prompt");
        assert_eq!(error_entry2.schedule, "at past");
        assert!(
            error_entry2.error.is_some(),
            "error field must be populated when outcome is 'error'"
        );
        let ts2 = chrono::DateTime::parse_from_rfc3339(&error_entry2.timestamp)
            .expect("valid RFC 3339 timestamp");
        let age2 = now.signed_duration_since(ts2.with_timezone(&Utc));
        assert!(
            age2.num_seconds() < 10,
            "log timestamp should be recent, got age={age2}"
        );
    }

    /// Integration test: a disabled one-shot event is skipped (not fired),
    /// logged with all FR-006 fields, and not re-logged on a second tick
    /// (FR-007, FR-011).
    ///
    /// The existing unit test covers disabled *repeating* events. This test
    /// covers the disabled *one-shot* path: `skip_disabled_event` pushes
    /// `next_due` to a far-future timestamp (9999-12-31) so that a second
    /// tick does not re-encounter the event.
    #[tokio::test]
    async fn test_integration_disabled_one_shot_skipped_not_refired() {
        let storage = Storage::open_in_memory().expect("storage");
        let working_dir = unique_temp_dir();
        let past = chrono::Utc::now() - chrono::Duration::hours(1);
        let event = CronEvent::new(
            "int-disabled-oneshot".to_string(),
            "general".to_string(),
            "disabled one-shot prompt".to_string(),
            CronSchedule::one_shot(past),
            "at past".to_string(),
            past,
        );
        storage.insert_cron_event(&event).expect("insert");
        // Disable the event before the tick.
        storage
            .set_cron_event_enabled("int-disabled-oneshot", false)
            .expect("disable");

        let processor = create_test_processor();
        let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));

        // First tick: should skip the disabled event.
        cron_tick(&storage, &processor, &working_dir, &running_events).await;

        // FR-011: event remains disabled (never fired).
        let row = storage
            .get_cron_event("int-disabled-oneshot")
            .expect("get")
            .expect("found");
        assert!(
            !row.enabled,
            "disabled one-shot should still be disabled after tick"
        );

        // FR-007: a "skipped" log entry should have been written.
        let logs =
            ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("int-disabled-oneshot"));
        let skipped = logs
            .iter()
            .find(|e| e.outcome == "skipped")
            .expect("expected a 'skipped' log entry for disabled one-shot");

        // FR-006: verify all log fields.
        assert_eq!(skipped.event_id, "int-disabled-oneshot");
        assert_eq!(skipped.agent_type, "general");
        assert_eq!(skipped.prompt, "disabled one-shot prompt");
        assert_eq!(skipped.schedule, "at past");
        assert!(
            skipped
                .error
                .as_ref()
                .is_some_and(|m| m.contains("disabled")),
            "skipped log should mention 'disabled', got: {:?}",
            skipped.error
        );
        chrono::DateTime::parse_from_rfc3339(&skipped.timestamp).expect("valid RFC 3339 timestamp");

        // The one-shot's next_due should be pushed to far future (9999-12-31).
        let new_next_due =
            chrono::DateTime::parse_from_rfc3339(&row.next_due).expect("parse next_due");
        assert!(
            new_next_due.with_timezone(&Utc) > chrono::Utc::now() + chrono::Duration::days(365),
            "disabled one-shot next_due should be pushed to far future, got: {new_next_due}"
        );

        // Second tick: the event should NOT be re-logged (far-future next_due
        // means it's no longer "due").
        cron_tick(&storage, &processor, &working_dir, &running_events).await;
        let logs2 =
            ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("int-disabled-oneshot"));
        let skipped_count = logs2.iter().filter(|e| e.outcome == "skipped").count();
        assert_eq!(
            skipped_count, 1,
            "disabled one-shot should not be re-skipped on second tick, got {skipped_count} skip entries"
        );
    }

    /// Integration test: a disabled event with an unknown agent type is
    /// skipped (FR-007/FR-011 take priority over FR-016), not errored.
    ///
    /// The scheduler processes disabled events in a separate query
    /// (`list_disabled_due_cron_events`) that never reaches the agent-type
    /// validation. So the log should show `"skipped"`, not `"error"`, even
    /// though the agent type is invalid.
    #[tokio::test]
    async fn test_integration_disabled_unknown_agent_skipped_not_errored() {
        let storage = Storage::open_in_memory().expect("storage");
        let working_dir = unique_temp_dir();
        let past = chrono::Utc::now() - chrono::Duration::hours(1);
        let event = CronEvent::new(
            "int-disabled-unknown".to_string(),
            "nonexistent-agent-xyz".to_string(),
            "disabled unknown agent prompt".to_string(),
            CronSchedule::repeat_from(past, 3600),
            "from past every 1h".to_string(),
            past,
        );
        storage.insert_cron_event(&event).expect("insert");
        storage
            .set_cron_event_enabled("int-disabled-unknown", false)
            .expect("disable");

        let processor = create_test_processor();
        let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
        cron_tick(&storage, &processor, &working_dir, &running_events).await;

        let logs =
            ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("int-disabled-unknown"));
        // Should be "skipped", NOT "error" — disabled check takes priority.
        assert!(
            logs.iter().any(|e| e.outcome == "skipped"),
            "disabled event with unknown agent should be 'skipped', not 'error'"
        );
        assert!(
            !logs.iter().any(|e| e.outcome == "error"),
            "disabled event should not produce an 'error' entry (FR-011 takes priority over FR-016)"
        );
    }

    /// Integration test: an unknown agent type on a *repeating* event logs
    /// `"error"` and advances `next_due` (FR-016). Unlike the one-shot case
    /// (which is disabled after error), a repeating event should remain
    /// enabled with `next_due` moved to the future.
    #[tokio::test]
    async fn test_integration_unknown_agent_repeating_advances_not_disabled() {
        let storage = Storage::open_in_memory().expect("storage");
        let working_dir = unique_temp_dir();
        let now = chrono::Utc::now();
        let past = now - chrono::Duration::hours(2);
        let event = CronEvent::new(
            "int-unknown-repeat".to_string(),
            "nonexistent-agent-xyz".to_string(),
            "unknown agent repeating prompt".to_string(),
            CronSchedule::repeat_from(past, 3600),
            "from past every 1h".to_string(),
            past,
        );
        storage.insert_cron_event(&event).expect("insert");

        let processor = create_test_processor();
        let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));
        cron_tick(&storage, &processor, &working_dir, &running_events).await;

        // FR-016: repeating event should remain enabled (not disabled).
        let row = storage
            .get_cron_event("int-unknown-repeat")
            .expect("get")
            .expect("found");
        assert!(
            row.enabled,
            "repeating event with unknown agent should remain enabled (advanced, not disabled)"
        );

        // next_due should be advanced to the future.
        let new_next_due =
            chrono::DateTime::parse_from_rfc3339(&row.next_due).expect("parse next_due");
        assert!(
            new_next_due.with_timezone(&Utc) > now,
            "repeating event next_due should be advanced after unknown-agent error"
        );

        // FR-006: verify the error log entry has all required fields.
        let logs =
            ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("int-unknown-repeat"));
        let error_entry = logs
            .iter()
            .find(|e| e.outcome == "error")
            .expect("expected an 'error' log entry for unknown agent type");
        assert_eq!(error_entry.event_id, "int-unknown-repeat");
        assert_eq!(error_entry.agent_type, "nonexistent-agent-xyz");
        assert_eq!(error_entry.prompt, "unknown agent repeating prompt");
        assert_eq!(error_entry.schedule, "from past every 1h");
        assert!(
            error_entry
                .error
                .as_ref()
                .is_some_and(|m| m.contains("Unknown agent type")),
            "error message should mention 'Unknown agent type', got: {:?}",
            error_entry.error
        );
        chrono::DateTime::parse_from_rfc3339(&error_entry.timestamp)
            .expect("valid RFC 3339 timestamp");
    }

    /// Integration test: the double-fire skip (FR-012) produces a log entry
    /// with all FR-006 fields, the event remains enabled, `next_due` is
    /// advanced, and the event ID stays in the `running_events` set.
    ///
    /// After the running set is manually cleared (simulating completion
    /// monitor removal), a second tick fires the event normally (producing
    /// an `"error"` log since spawn still fails without a AgentManager).
    #[tokio::test]
    async fn test_integration_double_fire_skip_then_fire_after_clear() {
        let storage = Storage::open_in_memory().expect("storage");
        let working_dir = unique_temp_dir();
        let now = chrono::Utc::now();
        let past = now - chrono::Duration::hours(2);
        let event = CronEvent::new(
            "int-double-fire".to_string(),
            "general".to_string(),
            "double-fire integration prompt".to_string(),
            CronSchedule::repeat_from(past, 3600),
            "from past every 1h".to_string(),
            past,
        );
        storage.insert_cron_event(&event).expect("insert");

        let running_events: RunningEvents = Arc::new(Mutex::new(HashSet::new()));

        // Simulate a still-running previous execution.
        {
            let mut set = running_events.lock().expect("lock");
            set.insert("int-double-fire".to_string());
        }

        let processor = create_test_processor();

        // First tick: should skip due to double-fire guard (FR-012).
        cron_tick(&storage, &processor, &working_dir, &running_events).await;

        // FR-012: event remains enabled (skipped, not fired).
        let row = storage
            .get_cron_event("int-double-fire")
            .expect("get")
            .expect("found");
        assert!(
            row.enabled,
            "double-fire skipped event should remain enabled"
        );

        // next_due should be advanced to the future.
        let next_due_1 =
            chrono::DateTime::parse_from_rfc3339(&row.next_due).expect("parse next_due");
        assert!(
            next_due_1.with_timezone(&Utc) > now,
            "next_due should be advanced after double-fire skip"
        );

        // FR-006: verify the skipped log entry has all required fields.
        let logs =
            ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("int-double-fire"));
        let skipped = logs
            .iter()
            .find(|e| e.outcome == "skipped")
            .expect("expected a 'skipped' log entry for double-fire");
        assert_eq!(skipped.event_id, "int-double-fire");
        assert_eq!(skipped.agent_type, "general");
        assert_eq!(skipped.prompt, "double-fire integration prompt");
        assert_eq!(skipped.schedule, "from past every 1h");
        assert!(
            skipped
                .error
                .as_ref()
                .is_some_and(|m| m.contains("Previous execution still running")),
            "skipped log should mention 'Previous execution still running', got: {:?}",
            skipped.error
        );
        chrono::DateTime::parse_from_rfc3339(&skipped.timestamp).expect("valid RFC 3339 timestamp");

        // Event ID should still be in running_events (guard doesn't remove it).
        {
            let set = running_events.lock().expect("lock");
            assert!(
                set.contains("int-double-fire"),
                "event ID should remain in running_events after double-fire skip"
            );
        }

        // Simulate completion: clear the running_events set.
        {
            let mut set = running_events.lock().expect("lock");
            set.clear();
        }

        // Manually set next_due back to the past so the event is due again
        // (the first tick already advanced it to the future).
        let past_again = chrono::Utc::now() - chrono::Duration::hours(1);
        storage
            .update_cron_event_next_due("int-double-fire", &past_again, None)
            .expect("reset next_due");

        // Second tick: should now fire normally (no double-fire guard).
        // The spawn fails (no AgentManager) so outcome is "error".
        cron_tick(&storage, &processor, &working_dir, &running_events).await;

        let logs2 =
            ragent_tools_core::cron_log::read_cron_log(&working_dir, Some("int-double-fire"));
        // Should now have both a "skipped" (first tick) and an "error" (second
        // tick, spawn failed) entry.
        assert!(
            logs2.iter().any(|e| e.outcome == "skipped"),
            "should still have the 'skipped' entry from the first tick"
        );
        assert!(
            logs2.iter().any(|e| e.outcome == "error"),
            "should have an 'error' entry from the second tick (spawn failed after guard cleared)"
        );

        // The event should still be enabled (repeating event after error is
        // advanced, not disabled).
        let row2 = storage
            .get_cron_event("int-double-fire")
            .expect("get")
            .expect("found");
        assert!(
            row2.enabled,
            "repeating event should remain enabled after post-guard fire attempt"
        );
    }

    /// Verify that the scheduler handle can be created and stopped.
    #[test]
    fn test_scheduler_handle_stop() {
        let handle = CronSchedulerHandle {
            cancel: Arc::new(AtomicBool::new(false)),
        };
        assert!(!handle.cancel.load(Ordering::Relaxed));
        handle.stop();
        assert!(handle.cancel.load(Ordering::Relaxed));
    }

    /// Verify that dropping the handle signals stop.
    #[test]
    fn test_scheduler_handle_drop_stops() {
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = Arc::clone(&cancel);
        {
            let _handle = CronSchedulerHandle {
                cancel: cancel_clone,
            };
        } // dropped here
        assert!(cancel.load(Ordering::Relaxed));
    }

    /// Create a minimal `SessionProcessor` for unit tests.
    ///
    /// The `AgentManager` is not set, so `spawn_background` will fail with
    /// "AgentManager not initialized". This is intentional — we want to
    /// verify that the tick handles spawn failures gracefully.
    fn create_test_processor() -> ragent_agent::session::processor::SessionProcessor {
        use std::sync::Arc;

        let storage = Arc::new(ragent_storage::Storage::open_in_memory().expect("storage"));
        let event_bus = Arc::new(ragent_agent::EventBus::new(1024));
        let session_manager = Arc::new(ragent_agent::session::SessionManager::new(
            storage,
            event_bus.clone(),
        ));
        let provider_registry = Arc::new(ragent_llm::ProviderRegistry::new());
        let tool_registry = Arc::new(ragent_agent::tool::ToolRegistry::new());
        let permission_checker = Arc::new(parking_lot::RwLock::new(
            ragent_agent::permission::PermissionChecker::new(Vec::new()),
        ));

        ragent_agent::session::processor::SessionProcessor {
            session_manager,
            provider_registry,
            tool_registry,
            permission_checker,
            event_bus,
            agent_manager: std::sync::OnceLock::new(),
            team_manager: std::sync::OnceLock::new(),
            team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(
                std::collections::HashMap::new(),
            )),
            mcp_client: std::sync::OnceLock::new(),
            code_index: std::sync::OnceLock::new(),
            bg_service: std::sync::OnceLock::new(),
            last_message_finish_reason: tokio::sync::RwLock::new(std::collections::HashMap::new()),
            active_spec: tokio::sync::RwLock::new(None),
            spec_manager: std::sync::OnceLock::new(),
            cached_tool_definitions: parking_lot::RwLock::new(None),
            cached_tool_names: parking_lot::RwLock::new(None),
            cached_tool_definition_bytes: parking_lot::RwLock::new(None),
            llm_client_cache: parking_lot::RwLock::new(std::collections::HashMap::new()),
            stream_config: ragent_agent::StreamConfig::default(),
            extraction_engine: std::sync::OnceLock::new(),
            auto_approve: false,
            system_prompt_cache: parking_lot::RwLock::new(None),
            read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
            cached_config: parking_lot::Mutex::new(None),
            telemetry: std::sync::Arc::new(ragent_telemetry::TelemetrySubsystem::disabled()),
            skill_body_cache: std::sync::Arc::new(std::sync::RwLock::new(
                std::collections::HashMap::new(),
            )),
        }
    }

    /// Create a unique temporary directory for cron log isolation.
    ///
    /// Each test gets its own directory so that log entries from one test
    /// do not interfere with another test's `read_cron_log` assertions.
    fn unique_temp_dir() -> std::path::PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let pid = std::process::id();
        let dir = std::env::temp_dir().join(format!("ragent-cron-test-{pid}-{id}"));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }
}
