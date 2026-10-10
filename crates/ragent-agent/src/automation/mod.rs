//! Automation service (spec `openhands` T-016; FR-013, FR-014, FR-018, FR-033).
//!
//! A durable, schedulable, or webhook-triggered agent run, built on the
//! existing agent loop and (per assumption A6) the cron schedule grammar rather
//! than replacing the cron tools.
//!
//! - **Webhook ingress** (FR-013): an inbound POST to `/auto/<id>` enqueues a run
//!   with the request payload supplied as prompt context.
//! - **Scheduler** (FR-014): the [`AutomationService::spawn_scheduler`] loop
//!   evaluates every schedule-triggered automation once per tick and enqueues a
//!   run when its due time passes, using `ragent_types::parse_schedule`.
//! - **Run history** (FR-018): every run is recorded in SQLite
//!   (`automation_runs`) with the automation id, trigger kind, start/end times,
//!   outcome, selected backend, and an output reference; a terminal run is
//!   pruned to the configured cap and dispatched to its third-party targets.
//! - **Backend confinement** (FR-033): a run executes only in its configured
//!   execution backend. The selected backend is installed as a per-session
//!   override ([`apply_session_backend_override`]) consulted by the session
//!   processor, so a container/remote run is routed there and a relay failure
//!   never silently re-runs it on the host (FR-031).
//!
//! ## Secret safety
//!
//! Dispatch credentials are named, never stored; [`dispatch::resolve_token`]
//! reads them from the environment at dispatch time (FR-035).

pub mod dispatch;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::{DateTime, Utc};
use parking_lot::Mutex;

use ragent_config::{
    AutomationConfig, AutomationDefinition, AutomationTriggerKind, BackendConfig, Config,
    ExecutionBackendKind,
};
use ragent_types::cron::{CronSchedule, parse_schedule};
use ragent_types::{AutomationRun, AutomationTrigger, RunOutcome};

use crate::agent::{AgentMode, resolve_agent_with_customs_and_model};
use crate::session::processor::SessionProcessor;

pub use dispatch::{DispatchOutcome, payload_for, resolve_token};

/// Synthetic parent session id for automation-spawned runs (FR-018).
pub const AUTOMATION_PARENT_SESSION_ID: &str = "automation";

/// Process-wide map of `session id -> selected execution backend` for
/// automation runs (FR-033).
///
/// Held process-globally rather than on [`SessionProcessor`] so a backend
/// override reaches the turn without threading a new field through every
/// constructor site. Entries are inserted before a run's first turn and removed
/// when the run completes, so the map stays bounded by the number of in-flight
/// automations.
static BACKEND_OVERRIDES: Mutex<Option<HashMap<String, BackendConfig>>> = Mutex::new(None);

/// Install `descriptor` as the selected execution backend for `session_id`
/// (FR-033).
pub fn register_session_backend(session_id: &str, descriptor: BackendConfig) {
    let mut guard = BACKEND_OVERRIDES.lock();
    guard
        .get_or_insert_with(HashMap::new)
        .insert(session_id.to_string(), descriptor);
}

/// Remove a previously installed backend override for `session_id`.
pub fn clear_session_backend(session_id: &str) {
    let mut guard = BACKEND_OVERRIDES.lock();
    if let Some(map) = guard.as_mut() {
        map.remove(session_id);
    }
}

/// The backend override installed for `session_id`, if any (FR-033).
#[must_use]
pub fn session_backend_override(session_id: &str) -> Option<BackendConfig> {
    BACKEND_OVERRIDES
        .lock()
        .as_ref()
        .and_then(|map| map.get(session_id).cloned())
}

/// Return `config` with the session's backend override applied (FR-033).
///
/// A run with no override returns the original `Arc` unchanged (the common
/// path); a run with an override returns a modified clone that names the
/// selected backend, so the per-turn backend resolution routes tools there.
#[must_use]
pub fn apply_session_backend_override(config: &Arc<Config>, session_id: &str) -> Arc<Config> {
    let Some(descriptor) = session_backend_override(session_id) else {
        return Arc::clone(config);
    };
    let mut cfg = (**config).clone();
    cfg.execution_backend = Some(ragent_config::ExecutionBackend::Config(descriptor));
    Arc::new(cfg)
}

/// Resolve the descriptor for a backend label from the main config (FR-033).
///
/// A label that matches a registered `backends` entry by kind reuses that entry
/// (so a `remote` run gets the URL/key and a container run gets its
/// image/workspace). An unregistered label synthesises a descriptor that names
/// only the kind, which a non-local run then fails to provision rather than
/// falling back to the host (FR-031).
#[must_use]
pub fn backend_descriptor(config: &Config, label: &str) -> BackendConfig {
    if let Some(kind) = ExecutionBackendKind::from_label(label) {
        if let Some(entry) = config
            .backends
            .iter()
            .find(|b| b.kind_parsed() == Some(kind))
        {
            return entry.clone();
        }
    }
    BackendConfig {
        id: label.to_string(),
        kind: label.to_string(),
        ..BackendConfig::default()
    }
}

/// Render an automation prompt with the trigger payload substituted (FR-013).
///
/// `{{payload}}` is replaced with `payload`; other text is passed through. An
/// empty payload leaves the template unchanged apart from removing the
/// placeholder.
#[must_use]
pub fn render_prompt(template: &str, payload: &str) -> String {
    template.replace("{{payload}}", payload)
}

/// Compute the next-due time for a schedule expression (FR-014).
///
/// Returns `None` for an unparseable expression. Used by the run-list surfaces
/// (`/automation`, `ragent automation list`) to show a concrete `next_due`.
#[must_use]
pub fn compute_next_due(expr: &str, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    parse_schedule(expr, now).ok().map(|parsed| parsed.next_due)
}

/// How an [`AutomationService`] reports a run it could not enqueue.
#[derive(Debug, thiserror::Error)]
pub enum AutomationError {
    /// No automation (or no enabled automation) matches the id.
    #[error("no enabled automation matches `{0}`")]
    UnknownAutomation(String),
}

/// The automation service: owns the configured automations and enqueues runs
/// (FR-013, FR-014).
pub struct AutomationService {
    processor: Arc<SessionProcessor>,
    config: AutomationConfig,
    working_dir: PathBuf,
    /// In-flight runs, keyed by automation id (no-double-fire for the scheduler;
    /// FR-014).
    running: Arc<Mutex<HashMap<String, String>>>,
}

impl AutomationService {
    /// Build a service for `config` rooted at `working_dir`.
    #[must_use]
    pub fn new(
        processor: Arc<SessionProcessor>,
        config: AutomationConfig,
        working_dir: PathBuf,
    ) -> Self {
        Self {
            processor,
            config,
            working_dir,
            running: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// The resolved automation configuration.
    #[must_use]
    pub fn config(&self) -> &AutomationConfig {
        &self.config
    }

    /// Whether the service is enabled (FR-013, FR-014).
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.config.is_enabled()
    }

    /// The configured automation definitions, in declaration order.
    #[must_use]
    pub fn definitions(&self) -> &[AutomationDefinition] {
        &self.config.automations
    }

    /// Whether an automation has an in-flight run.
    #[must_use]
    pub fn is_running(&self, automation_id: &str) -> bool {
        self.running.lock().contains_key(automation_id)
    }

    /// Look up an enabled automation definition by id.
    #[must_use]
    pub fn definition(&self, id: &str) -> Option<&AutomationDefinition> {
        self.config.get(id).filter(|a| a.enabled)
    }

    /// Enqueue a run for `automation_id` triggered by `trigger`, with `payload`
    /// supplied as prompt context (FR-013, FR-014).
    ///
    /// The run-history record is inserted in the running state before the agent
    /// turn is spawned; the turn's completion finalises the record, dispatches
    /// to the configured targets, and prunes the history (FR-018).
    ///
    /// # Errors
    ///
    /// Returns [`AutomationError::UnknownAutomation`] when no enabled automation
    /// matches the id, or a storage error when the run record cannot be written.
    pub async fn enqueue(
        self: &Arc<Self>,
        automation_id: &str,
        trigger: AutomationTrigger,
        payload: &str,
    ) -> anyhow::Result<AutomationRun> {
        let def = self
            .definition(automation_id)
            .cloned()
            .ok_or_else(|| AutomationError::UnknownAutomation(automation_id.to_string()))?;

        let backend = def.backend_label().to_string();
        let run = AutomationRun::start(
            uuid::Uuid::new_v4().to_string(),
            def.id.clone(),
            trigger,
            backend,
        );
        let record = run.clone();
        self.processor
            .storage_op(move |s| s.insert_automation_run(&record))
            .await?;

        self.running.lock().insert(def.id.clone(), run.id.clone());

        let svc = Arc::clone(self);
        let def_for_task = def.clone();
        let payload = payload.to_string();
        let prompt = render_prompt(&def.prompt, &payload);
        let run_for_task = run.clone();
        tokio::spawn(async move {
            svc.execute_run(run_for_task, def_for_task, prompt).await;
        });

        Ok(run)
    }

    /// Enqueue a manual run (the `/automation run` and CLI surfaces).
    ///
    /// # Errors
    ///
    /// As [`Self::enqueue`].
    pub async fn run_now(self: &Arc<Self>, automation_id: &str) -> anyhow::Result<AutomationRun> {
        self.enqueue(automation_id, AutomationTrigger::Manual, "")
            .await
    }

    /// Execute one enqueued run to completion: install the backend override,
    /// run the agent turn, finalise the record, dispatch, and prune (FR-018,
    /// FR-033).
    async fn execute_run(
        self: Arc<Self>,
        run: AutomationRun,
        def: AutomationDefinition,
        prompt: String,
    ) {
        let config = self.processor.load_config_cached();
        let descriptor = backend_descriptor(&config, &run.backend);

        // Create the run's child session and confine its turn to the selected
        // backend before the first tool call is dispatched (FR-033).
        let child = match self
            .processor
            .session_manager
            .create_session(self.working_dir.clone())
        {
            Ok(child) => child,
            Err(e) => {
                self.finalise(&run, RunOutcome::Error, None).await;
                self.running.lock().remove(&run.automation_id);
                tracing::warn!(automation = %run.automation_id, error = %e, "automation run: session create failed");
                return;
            }
        };
        register_session_backend(&child.id, descriptor);

        // Hold the override only for the duration of the turn, so it is removed on
        // every exit path (including a panic in `run_turn`) rather than leaking the
        // session override for the process lifetime (FR-033).
        let result = {
            let _guard = SessionBackendGuard {
                session_id: &child.id,
            };
            self.run_turn(&config, &def, &child.id, &prompt).await
        };
        clear_session_backend(&child.id);

        let (outcome, output_ref) = match result {
            Ok(text) => {
                let path = self.write_output(&run.id, &text);
                (RunOutcome::Success, path)
            }
            Err(e) => {
                tracing::warn!(automation = %run.automation_id, run = %run.id, error = %e, "automation run failed");
                (RunOutcome::Error, Some(child.id.clone()))
            }
        };

        self.finalise(&run, outcome, output_ref).await;
        self.running.lock().remove(&run.automation_id);
    }

    /// Resolve the agent and run one turn in the child session (FR-033).
    async fn run_turn(
        &self,
        config: &Arc<Config>,
        def: &AutomationDefinition,
        session_id: &str,
        prompt: &str,
    ) -> anyhow::Result<String> {
        let agent_name = def
            .agent
            .clone()
            .unwrap_or_else(|| config.default_agent.clone());
        let mut agent = Arc::unwrap_or_clone(resolve_agent_with_customs_and_model(
            &agent_name,
            config,
            &self.working_dir,
            &self.processor.provider_registry,
        )?);
        agent.mode = AgentMode::Subagent;
        let cancel = Arc::new(AtomicBool::new(false));
        let message = self
            .processor
            .process_message(session_id, prompt, &agent, cancel)
            .await?;
        Ok(message.text_content())
    }

    /// Write the run's full output under `log/automation/<run-id>.md` (FR-018).
    fn write_output(&self, run_id: &str, text: &str) -> Option<String> {
        let dir = self
            .config
            .output_dir
            .clone()
            .unwrap_or_else(|| self.working_dir.join("log").join("automation"));
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::warn!(run = %run_id, dir = %dir.display(), error = %e, "automation run: could not create the output directory");
            return None;
        }
        let path = dir.join(format!("{run_id}.md"));
        if let Err(e) = std::fs::write(&path, text) {
            tracing::warn!(run = %run_id, path = %path.display(), error = %e, "automation run: could not write the run output");
            return None;
        }
        Some(path.display().to_string())
    }

    /// Finalise a run: persist its terminal state, dispatch to third-party
    /// targets, and prune the history to the cap (FR-018).
    async fn finalise(&self, run: &AutomationRun, outcome: RunOutcome, output_ref: Option<String>) {
        let id = run.id.clone();
        let out_ref = output_ref.clone();
        if let Err(e) = self
            .processor
            .storage_op(move |s| s.finish_automation_run(&id, outcome, out_ref.as_deref()))
            .await
        {
            tracing::warn!(
                run = %run.id,
                error = %e,
                "automation run: could not persist the terminal state"
            );
        }

        // Build the terminal view for dispatch/pruning.
        let mut terminal = run.clone();
        terminal.outcome = Some(outcome);
        terminal.ended_at = Some(Utc::now());
        terminal.output_ref = output_ref;

        if let Some(def) = self.config.get(&terminal.automation_id) {
            for result in
                dispatch::dispatch_all(&def.dispatch, &terminal.automation_id, &terminal).await
            {
                match result {
                    DispatchOutcome::Delivered { .. } => {}
                    DispatchOutcome::Inert { kind, reason } => tracing::debug!(
                        automation = %terminal.automation_id,
                        kind = %kind,
                        reason = %reason,
                        "automation dispatch target is inert"
                    ),
                    DispatchOutcome::Failed { kind, error } => tracing::warn!(
                        automation = %terminal.automation_id,
                        kind = %kind,
                        error = %error,
                        "automation dispatch failed"
                    ),
                }
            }
        }
        let automation_id = terminal.automation_id.clone();
        let cap = self.config.run_history_cap;
        if let Err(e) = self
            .processor
            .storage_op(move |s| s.prune_automation_runs(&automation_id, cap))
            .await
        {
            tracing::warn!(
                automation = %terminal.automation_id,
                error = %e,
                "automation run: could not prune run history"
            );
        }
    }

    /// List an automation's run history, newest first (FR-018).
    pub async fn list_runs(
        &self,
        automation_id: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<AutomationRun>> {
        let id = automation_id.to_string();
        self.processor
            .storage_op(move |s| s.list_automation_runs(&id, limit))
            .await
    }

    /// Fetch a single run-history record by id (FR-018).
    pub async fn get_run(&self, run_id: &str) -> anyhow::Result<Option<AutomationRun>> {
        let id = run_id.to_string();
        self.processor
            .storage_op(move |s| s.get_automation_run(&id))
            .await
    }

    /// Spawn the background scheduler loop (FR-014).
    ///
    /// The loop ticks every `scheduler_tick_secs`, enqueues a run for each due
    /// schedule-triggered automation (skipping one whose previous run is still
    /// in flight), and advances its next-due time.
    #[must_use]
    pub fn spawn_scheduler(self: &Arc<Self>) -> AutomationSchedulerHandle {
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = Arc::clone(&cancel);
        let svc = Arc::clone(self);
        tokio::spawn(async move {
            svc.scheduler_loop(cancel_clone).await;
        });
        AutomationSchedulerHandle { cancel }
    }

    /// The scheduler loop body (FR-014).
    async fn scheduler_loop(self: Arc<Self>, cancel: Arc<AtomicBool>) {
        if !self.is_enabled() {
            return;
        }
        // Seed the next-due time for every schedule automation.
        let now = Utc::now();
        let mut schedule: HashMap<String, (CronSchedule, DateTime<Utc>)> = HashMap::new();
        for def in self.config.enabled_definitions() {
            if let AutomationTriggerKind::Schedule { schedule: expr } = &def.trigger {
                if let Ok(parsed) = parse_schedule(expr, now) {
                    schedule.insert(def.id.clone(), (parsed.schedule, parsed.next_due));
                }
            }
        }
        let tick = self.config.scheduler_tick_secs.max(1);
        loop {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let now = Utc::now();
            for def in self.config.enabled_definitions() {
                let AutomationTriggerKind::Schedule { schedule: expr } = &def.trigger else {
                    continue;
                };
                // Read only the due time; the not-yet-due case is the common one and
                // must not pay for a `CronSchedule` clone.
                let due = match schedule.get(&def.id) {
                    Some((_, due)) => *due,
                    None => match parse_schedule(expr, now) {
                        Ok(parsed) => {
                            let due = parsed.next_due;
                            schedule.insert(def.id.clone(), (parsed.schedule, due));
                            due
                        }
                        Err(_) => continue,
                    },
                };
                if now >= due && !self.is_running(&def.id) {
                    if let Err(e) = self.enqueue(&def.id, AutomationTrigger::Schedule, "").await {
                        tracing::warn!(automation = %def.id, error = %e, "automation scheduler: enqueue failed");
                    }
                    // Clone the schedule only on the due path, then advance or drop it.
                    let sched = schedule.get(&def.id).map(|(s, _)| s.clone());
                    if let Some(sched) = sched {
                        match sched.advance_next_due(due, now) {
                            Some(next) => {
                                schedule.insert(def.id.clone(), (sched, next));
                            }
                            None => {
                                schedule.remove(&def.id);
                            }
                        }
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(tick)).await;
        }
    }
}

/// Handle to the background automation scheduler task (FR-014).
pub struct AutomationSchedulerHandle {
    cancel: Arc<AtomicBool>,
}

impl AutomationSchedulerHandle {
    /// Signal the scheduler loop to stop.
    pub fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl Drop for AutomationSchedulerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Removes a session's execution-backend override when dropped (FR-033).
///
/// A guard rather than a trailing call, so an aborted or panicked run turn cannot
/// leak the override for the process lifetime.
struct SessionBackendGuard<'a> {
    session_id: &'a str,
}

impl Drop for SessionBackendGuard<'_> {
    fn drop(&mut self) {
        clear_session_backend(self.session_id);
    }
}
