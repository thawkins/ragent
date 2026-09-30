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
//! (FR-007, FR-011) - they are never fired. Their `next_due` is advanced to
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

/// Whether a cron event repeats (FR-012): it has a repeat duration and is
/// not a one-shot schedule.
fn is_repeating_event(event: &ragent_storage::CronEventRow) -> bool {
    event.duration_secs.is_some() && event.schedule_form != "one_shot"
}

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
/// - `storage` - shared SQLite storage handle for querying due cron events.
/// - `session_processor` - shared session processor for spawning agent runs
///   via the `new_agent` / `spawn_background` path (FR-004, FR-005).
/// - `working_dir` - project working directory used for execution logging
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

        // Idle-CPU fix: wait for the next tick with a single long sleep
        // instead of a 1 s interruptible poll. The old loop woke once per
        // second purely to re-check the cancel flag (process-lifetime cost
        // for zero benefit - shutdown just waited out the current second).
        // A sleep longer than force-exit (3 s after loop teardown) needs no
        // responsiveness: cancellation now takes effect at most one full
        // tick late, which the 30 s tick interval already bounds.
        tokio::time::sleep(Duration::from_secs(CRON_TICK_INTERVAL_SECS)).await;
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
                // Idle-CPU fix: the agent-types filesystem scan is only
                // needed when an event is actually about to fire. Loading
                // it unconditionally scanned the agents directories every
                // 30 s even with zero cron events configured.
                let (all_agents, _) = ragent_agent::agent::load_all_agents(working_dir);
                for event in &events {
                    // FR-012: no-double-fire guard for repeating events.
                    // If this repeating event's previous execution is still
                    // running, skip this cycle and log "skipped".
                    let is_repeating = is_repeating_event(event);
                    if is_repeating {
                        // FR-012 fail-safe: if the mutex is poisoned we cannot
                        // know whether the previous run finished, so treat the
                        // event as still running (skip) rather than risk a
                        // concurrent double-fire.
                        let already_running = running_events
                            .lock()
                            .map_or(true, |set| set.contains(&event.id));
                        if already_running {
                            tracing::info!(
                                event_id = %event.id,
                                "Skipping due repeating event - previous execution still running (FR-012)",
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
                        &all_agents,
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
    all_agents: &[Arc<ragent_agent::agent::AgentInfo>],
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
    let is_repeating = is_repeating_event(event);

    // Attempt to spawn a background agent run via the new_agent path.
    // For stateful events, load the cross-run loop state and inject it
    // into the prompt (FR-004).
    let effective_prompt = if event.stateful {
        // A corrupt or unreadable state file falls back to a fresh state, but
        // the loss of accumulated cross-run context is surfaced so it can be
        // diagnosed.
        let state = match ragent_agent::loop_state::LoopState::load(working_dir, &event.id) {
            Ok(state) => state,
            Err(e) => {
                tracing::warn!(
                    event_id = %event.id,
                    error = %e,
                    "failed to load loop state; starting fresh",
                );
                ragent_agent::loop_state::LoopState::default()
            }
        };
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
            None, // no model override - use the configured default
            working_dir,
        )
        .await
}

/// Spawn a background monitor task that polls the AgentManager until the
/// spawned agent run completes, then removes the event ID from the
/// `running_events` set (FR-012).
///
/// This runs on its own tokio task so it does not block the scheduler loop.
/// The poll interval is 5 seconds - short enough for responsiveness but
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
            let done = match &task {
                Some(entry) => entry.status != ragent_agent::task::TaskStatus::Running,
                None => true, // task was removed - consider it done
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
                    if let Some(entry) = &task {
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
            // FUNC-027: surface a failed disable instead of swallowing it - a
            // silent failure means the event is re-evaluated on every tick.
            if let Err(e) = storage.set_cron_event_enabled(&event.id, false) {
                tracing::warn!(
                    event_id = %event.id,
                    error = %e,
                    "failed to disable cron event after advance_next_due returned None"
                );
            }
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
    let is_repeating = is_repeating_event(event);
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
    let is_repeating = is_repeating_event(event);
    if is_repeating {
        advance_repeating_event(storage, event, now);
    } else {
        // Set next_due to far future so this disabled one-shot event is not
        // returned by list_disabled_due_cron_events on every tick.
        if let Ok(far_future) = chrono::DateTime::parse_from_rfc3339("9999-12-31T23:59:59Z") {
            // FUNC-027: surface a failed advance instead of swallowing it - a
            // silent failure means this disabled one-shot is re-listed every
            // tick.
            if let Err(e) =
                storage.update_cron_event_next_due(&event.id, &far_future.with_timezone(&Utc), None)
            {
                tracing::warn!(
                    event_id = %event.id,
                    error = %e,
                    "failed to advance next_due for disabled one-shot cron event"
                );
            }
        }
    }
}

#[cfg(test)]
#[path = "../tests/inline/cron_tests.rs"]
mod tests;
