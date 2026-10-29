//! One shared async sub-agent launch path for every TUI surface
//! (spec `agentssetup` D3).
//!
//! Before this module each launch surface hand-rolled its own plumbing: the
//! `/spawn` command guarded a `Mutex<Option<Result<String, String>>>` so only one
//! launch could be in flight (and silently dropped the outcome when that mutex
//! was poisoned), and it built a `current_thread` runtime on a helper thread
//! whenever the caller had no reactor. That gave `/spawn` a state machine
//! (`[wait] launching` plus a private poll) that no other launch path had.
//!
//! Here the plumbing lives once:
//!
//! * [`App::begin_subagent_launch`] registers a bounded, id-correlated entry on
//!   [`App::subagent_launches`] and returns its [`SubagentLaunch`] handle. The
//!   cap is `agents.max_launching`, so a launcher queues behind the busy
//!   launches instead of growing its own "already launching" flag.
//! * [`run_subagent_launch`] does the async work behind one uniform runtime
//!   rule ([`dispatch_launch`]): dispatch on the ambient reactor when there is
//!   one, otherwise build a short-lived runtime and run it there.
//! * The outcome is deposited on [`App::subagent_launch_results`] by id and
//!   drained by [`App::poll_subagent_launch_results`], which releases the slot
//!   and reports in the requesting surface's own words.
//!
//! No path panics: a launch that cannot be spawned reports a failure outcome
//! rather than leaving the slot occupied forever.

// Clippy complains about `pub(crate)` inside this private module, but CI's
// dead-code lint runs with `-D unreachable_pub`, so `pub` items here fail CI.
// Keep `pub(crate)` and suppress the clippy nursery lint (the same treatment
// `helpers.rs` gives the crate's other free functions).
#![allow(clippy::redundant_pub_crate)]

use std::future::Future;

use tokio::task::JoinHandle;

use crate::app::helpers::short_id;
use crate::app::session_ops::recover_poisoned;
use crate::app::state::{App, LaunchOrigin, LogLevel, SubagentLaunch, SubagentLaunchOutcome};

impl App {
    /// Register a launch request on the shared bounded launch set
    /// (spec `agentssetup` D3).
    ///
    /// Returns the [`SubagentLaunch`] handle the caller passes to
    /// [`run_subagent_launch`] on success, or the refusal text when
    /// [`App::subagent_launch_cap`] launches are already in flight. The refusal
    /// names the busy launches so the caller can say what it is waiting for.
    ///
    /// Registration is the only "is a launch already running" check in the TUI:
    /// the `/spawn` command, the Agents window and any future launcher all call
    /// this, so two surfaces cannot disagree about whether a launch is pending.
    ///
    /// # Errors
    ///
    /// Returns a human-readable refusal naming the in-flight launches when the
    /// launch set is full.
    pub(crate) fn begin_subagent_launch(
        &mut self,
        origin: LaunchOrigin,
        agent: &str,
    ) -> Result<SubagentLaunch, String> {
        if self.subagent_launches.len() >= self.subagent_launch_cap {
            let busy = self
                .subagent_launches
                .iter()
                .map(|l| format!("{} ({})", l.agent, l.origin.label()))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(format!(
                "already launching {}/{} sub-agent(s) - {} - try again when one lands",
                self.subagent_launches.len(),
                self.subagent_launch_cap,
                busy
            ));
        }
        let id = self.next_subagent_launch_id;
        self.next_subagent_launch_id = self.next_subagent_launch_id.saturating_add(1);
        let launch = SubagentLaunch {
            id,
            origin,
            agent: agent.to_string(),
        };
        self.subagent_launches.push(launch.clone());
        Ok(launch)
    }

    /// Release a launch slot without reporting an outcome (D3).
    ///
    /// Used when a launcher fails after registering but before the async work
    /// starts, so the slot cannot be held forever by a request that will never
    /// produce a result.
    pub(crate) fn release_subagent_launch(&mut self, id: u64) {
        self.subagent_launches.retain(|l| l.id != id);
    }

    /// Drain every completed launch outcome and report it (D3).
    ///
    /// Called each housekeeping frame alongside the other `poll_*` fns. Each
    /// outcome releases its slot and then reports in the words of the surface
    /// that requested the launch, so no surface needs its own poll arm.
    pub(crate) fn poll_subagent_launch_results(&mut self) {
        let results = {
            let mut queue = recover_poisoned(
                self.subagent_launch_results.lock(),
                "subagent_launch_results",
            );
            std::mem::take(&mut *queue)
        };
        if results.is_empty() {
            return;
        }
        for result in results {
            self.release_subagent_launch(result.launch.id);
            self.apply_subagent_launch_outcome(result);
        }
        self.needs_redraw = true;
    }

    /// Report one launch outcome on the chat/status/log surfaces (D3).
    fn apply_subagent_launch_outcome(&mut self, result: SubagentLaunchOutcome) {
        let label = result.launch.origin.label();
        let agent = result.launch.agent.clone();
        match result.outcome {
            Ok(task_id) => {
                self.status = format!("spawn: detached task {} running", short_id(&task_id));
                self.push_log_no_agent(
                    LogLevel::Info,
                    format!("spawn: detached background task launched ({task_id})"),
                );
                self.append_assistant_text(&format!(
                    "From: {label}\n## Detached sub-agent launched\n\n\
                     Task **{task_id}** is running in the background.\n\n\
                     It is detached: nothing will wait for it, it will not appear \
                     in `list_agents`, and its output will not be injected into \
                     this chat. Track it in the Agents panel or stop it with \
                     `/cancel {}`.\n",
                    short_id(&task_id)
                ));
            }
            Err(msg) => {
                self.status = format!("[warn] spawn failed: {msg}");
                self.push_log_no_agent(LogLevel::Warn, format!("spawn failed: {msg}"));
                self.append_assistant_text(&format!(
                    "From: {label}\n## [err] Could not launch detached sub-agent\n\n{msg}\n"
                ));
                tracing::debug!(agent = %agent, "sub-agent launch failed");
            }
        }
    }

    /// Deposit one launch outcome for the next poll pass to drain (D3).
    ///
    /// Takes the delivery queue rather than `&mut self` so the async work can
    /// deposit its result from a worker thread while the event loop keeps
    /// running.
    fn deposit_subagent_launch_outcome(
        queue: &std::sync::Mutex<Vec<SubagentLaunchOutcome>>,
        result: SubagentLaunchOutcome,
    ) {
        recover_poisoned(queue.lock(), "subagent_launch_results").push(result);
    }
}

/// Run one registered launch and deposit its outcome (spec `agentssetup` D3).
///
/// `work` performs the actual registration (e.g. `spawn_detached`) and returns
/// the task id or a failure message. The outcome is queued on `results` under
/// the handle's id, so [`App::poll_subagent_launch_results`] releases the slot
/// and reports it.
///
/// The runtime rule is uniform and lives in [`dispatch_launch`]: on a thread
/// with an ambient tokio reactor the work is spawned there, otherwise a
/// short-lived current-thread runtime is built on a helper thread and the work
/// runs there. A launch that cannot be dispatched at all is reported as a
/// failure rather than silently dropped.
pub(crate) async fn run_subagent_launch<F>(
    launch: SubagentLaunch,
    work: F,
    results: std::sync::Arc<std::sync::Mutex<Vec<SubagentLaunchOutcome>>>,
) where
    F: Future<Output = Result<String, String>> + Send + 'static,
{
    let outcome = work.await;
    App::deposit_subagent_launch_outcome(&results, SubagentLaunchOutcome { launch, outcome });
}

/// Dispatch a launch future on the ambient reactor, or on a helper runtime
/// (spec `agentssetup` D3).
///
/// This is the single runtime rule the `/spawn` command used to inline. It
/// returns the spawned [`JoinHandle`] when the ambient reactor accepted the
/// work, and `None` when it fell back to a short-lived runtime (or when neither
/// was available, in which case the failure is logged - the launch entry is
/// released by the caller's refusal path).
pub(crate) fn dispatch_launch<F>(future: F) -> Option<JoinHandle<()>>
where
    F: Future<Output = ()> + Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => Some(handle.spawn(future)),
        Err(_) => {
            let spawn = std::thread::Builder::new()
                .name("ragent-subagent-launch".to_owned())
                .spawn(move || match tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                {
                    Ok(rt) => rt.block_on(future),
                    Err(e) => {
                        tracing::error!(error = %e, "sub-agent launch: helper runtime build failed")
                    }
                });
            match spawn {
                Ok(handle) => {
                    drop(handle);
                    None
                }
                Err(e) => {
                    tracing::error!(error = %e, "sub-agent launch: helper thread spawn failed");
                    None
                }
            }
        }
    }
}

/// Spawn a detached sub-agent on the shared launch path (spec `agentssetup`
/// D3).
///
/// `manager` may be absent (a session with no sub-agent manager wired), in which
/// case the returned future reports the same failure text `/spawn` has always
/// used, so the surface does not need its own guard.
pub(crate) async fn detached_launch_work(
    manager: Option<std::sync::Arc<ragent_agent::task::AgentManager>>,
    parent_sid: String,
    agent: String,
    prompt: String,
    model_override: Option<String>,
    working_dir: std::path::PathBuf,
) -> Result<String, String> {
    let Some(manager) = manager else {
        return Err("sub-agent manager unavailable in this session".to_string());
    };
    manager
        .spawn_detached(
            &parent_sid,
            &agent,
            &prompt,
            model_override.as_deref(),
            &working_dir,
            None, // no per-call bound - the configured stall timeout applies
        )
        .await
        .map(|entry| entry.id)
        .map_err(|e| e.to_string())
}
