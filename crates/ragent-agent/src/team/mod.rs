//! Team module - Agent Team Coordination for ragent.
//!
//! This module owns the team runtime (config, task, mailbox, store, manager,
//! swarm, classify) natively in `ragent-agent`.  The separate `ragent-team`
//! re-export shim crate was retired in ANTIPAT M7.8 (see
//! `docs/team-unification-decision.md`); previously these sources lived in
//! that crate and were compiled into `ragent-agent` via `#[path]`
//! attributes (see `REMPLAN.md` M3 / T3.3).
//!
//! A team consists of a lead session and one or more named *teammate*
//! sessions that coordinate via a shared task list and per-agent mailboxes
//! stored on disk.
//!
//! ## Sub-modules
//!
//! | Module | Contents |
//! |--------|----------|
//! | [`classify`] | Agent-type inference and fallback helpers for swarm decomposition |
//! | [`config`] | `TeamConfig`, `TeamMember`, `TeamStatus`, `MemberStatus` |
//! | [`hook_ledger`] | `hook-failures.jsonl` - bounded ledger of hooks that never ran (FR-019) |
//! | [`task`]   | `Task`, `TaskStatus`, `TaskList`, `TaskStore` |
//! | [`mailbox`]| `MailboxMessage`, `MessageType`, `Mailbox` |
//! | [`store`]  | `TeamStore`, `find_team_dir`, directory discovery |
//! | [`status_history`] | `status-history.jsonl` - the last member-status transitions (FR-036) |
//! | [`manager`]| `TeamManager` runtime for spawning/polling teammates |
//! | [`swarm`]  | LLM decomposition of a goal into parallel subtasks |

use chrono::{DateTime, Utc};

pub mod classify;
pub mod config;
pub mod hook_ledger;
pub mod mailbox;
pub mod manager;
pub mod status_history;
pub mod store;
/// Swarm - fleet-style auto-decomposition into parallel subtasks.
pub mod swarm;
pub mod task;

// -- Re-exports -----------------------------------------------------------------

pub use classify::{
    DEFAULT_AGENT_TYPE, KNOWN_AGENT_TYPES, extract_explicit_agent_type, infer_agent_type,
    is_known_agent_type, resolve_agent_type, strip_explicit_agent_type_hint,
};

pub use config::{
    DEFAULT_WATCHDOG_TIMEOUT_SECS, HookEntry, HookEvent, MemberStatus, MemoryScope, PlanStatus,
    TeamConfig, TeamMember, TeamSettings, TeamStatus, TransitionCause, resolve_memory_dir,
};
pub use hook_ledger::{
    HOOK_FAILURES_FILE, HOOK_FAILURES_ROTATED_FILE, HookFailure, HookFailureReason,
    MAX_LEDGER_BYTES, hook_failure_count, ledger_path, read_hook_failures, record_hook_failure,
    render_hook_failures, rotated_ledger_path, try_record_hook_failure,
};
pub use mailbox::{
    MAX_MESSAGE_BYTES, Mailbox, MailboxMessage, MessageType, deregister_notifier, register_notifier,
};
pub use manager::watchdog_check_interval;
pub use manager::{
    HookOutcome, MAX_HOOK_FEEDBACK_BYTES, StatusTransition, TeamManager,
    build_team_prompt_addition, run_hook, run_team_hook, teammate_retry_backoff,
    validate_hook_command,
};
pub use status_history::{
    MAX_HISTORY_BYTES, STATUS_HISTORY_FILE, STATUS_HISTORY_LIMIT, STATUS_HISTORY_ROTATED_FILE,
    StatusHistoryEntry, history_path, read_status_history, recent_status_history,
    record_status_transition, render_status_history, rotated_history_path,
    try_record_status_transition,
};
pub use store::{
    TeamStore, find_project_teams_dir, find_team_dir, global_teams_dir, slugify_team_name,
    validate_team_name,
};
pub use swarm::{
    DECOMPOSITION_SYSTEM_PROMPT, SwarmDecomposition, SwarmState, SwarmSubtask,
    build_decomposition_user_prompt, parse_decomposition, parse_decomposition_with_default,
};
pub use task::{
    CompletionRefusal, CompletionRefusalReason, Task, TaskCancellation, TaskClaim, TaskList,
    TaskStatus, TaskStore, TaskTransition, cancel_task_in_list, claim_task_in_list,
    complete_task_in_list, fire_task_created_hook, fire_task_hook, transition_task,
};

// -- Prose spellings (FR-032, FR-033) ---------------------------------------------

/// The status word a **rendered surface** prints for `status`: the
/// [`MemberStatus::display_label`] word in square brackets.
///
/// FR-033 makes `display_label()` the single status-word mapping, and F-24 found
/// a third spelling (`plan_pending`, `shutting_down`) reaching user-visible text
/// only because prose used [`MemberStatus::as_str`] - the serialization token -
/// where a status *word* was meant. Every team prose report and refusal takes
/// its word from here, so a status has exactly two spellings and both are
/// deliberate: the wire token in JSON, and this word in prose.
///
/// # Examples
///
/// ```
/// use ragent_agent::team::{MemberStatus, status_word};
///
/// assert_eq!(status_word(MemberStatus::Working), "[working]");
/// // The two labels that differ from the wire token keep their prose spelling.
/// assert_eq!(status_word(MemberStatus::PlanPending), "[planning]");
/// assert_eq!(status_word(MemberStatus::ShuttingDown), "[stopping]");
/// assert_ne!(
///     MemberStatus::ShuttingDown.display_label(),
///     MemberStatus::ShuttingDown.as_str()
/// );
/// ```
#[must_use]
pub fn status_word(status: MemberStatus) -> String {
    format!("[{}]", status.display_label())
}

/// The status word a **one-line command banner** names for `status`: the
/// [`MemberStatus::display_label`] word with no decoration.
///
/// A banner (`self.status = ...`) has no room for brackets, so it takes the bare
/// label. It is the same word [`status_word`] prints, which is what keeps a
/// banner and the panel it describes in agreement (FR-033).
///
/// # Examples
///
/// ```
/// use ragent_agent::team::{MemberStatus, status_label};
///
/// assert_eq!(status_label(MemberStatus::PlanPending), "planning");
/// assert_eq!(
///     status_label(MemberStatus::Blocked),
///     MemberStatus::Blocked.display_label()
/// );
/// ```
#[must_use]
pub fn status_label(status: MemberStatus) -> &'static str {
    status.display_label()
}

/// Name a member given its name, id and status (FR-032).
///
/// Every refusal that is *about a member* starts here, so the offending entity
/// and its current state are stated the same way everywhere instead of being
/// re-typed per tool (and eventually disagreeing).
///
/// # Examples
///
/// ```
/// use ragent_agent::team::{MemberStatus, member_status_clause};
///
/// let text = member_status_clause("alpha", "tm-001", MemberStatus::Suspended);
/// assert_eq!(text, "teammate 'alpha' (tm-001) is in status [suspended]");
/// ```
#[must_use]
pub fn member_status_clause(name: &str, agent_id: &str, status: MemberStatus) -> String {
    format!(
        "teammate '{name}' ({agent_id}) is in status {}",
        status_word(status)
    )
}

/// Name a task and the status it currently holds (FR-032).
///
/// A team refusal that is *about a task* states the task ID and the status the
/// task actually has, taken from [`TaskStatus::as_str`]: a task status is
/// serialized and matched with that one spelling, so FR-033's
/// `display_label()` rule does not apply to it.
///
/// # Examples
///
/// ```
/// use ragent_agent::team::{TaskStatus, task_status_clause};
///
/// assert_eq!(
///     task_status_clause("task-001", TaskStatus::InProgress),
///     "task 'task-001' is in status in_progress"
/// );
/// ```
#[must_use]
pub fn task_status_clause(task_id: &str, status: TaskStatus) -> String {
    format!("task '{task_id}' is in status {}", status.as_str())
}

/// Name the task a member is working on, for a refusal that has to say why the
/// member cannot do something (FR-032).
///
/// `None` renders as "no task"; `Some` names the task ID, so a caller never has
/// to write "is busy" without saying with what.
///
/// # Examples
///
/// ```
/// use ragent_agent::team::current_task_clause;
///
/// assert_eq!(current_task_clause(None), "no task");
/// assert_eq!(current_task_clause(Some("task-004")), "task 'task-004'");
/// ```
#[must_use]
pub fn current_task_clause(current_task_id: Option<&str>) -> String {
    match current_task_id {
        Some(id) => format!("task '{id}'"),
        None => "no task".to_string(),
    }
}

// -- Guard helpers ---------------------------------------------------------------

/// Whether `status` blocks a team teardown (spec `teamssetup` FR-022).
///
/// This is the one predicate behind [`blocking_members`] and
/// [`cleanup_refusal`]; call sites that need the blocking *count* as well as
/// the refusal text filter on this so the number and the text cannot disagree.
///
/// # Examples
///
/// ```
/// use ragent_agent::team::{MemberStatus, blocks_teardown};
///
/// assert!(blocks_teardown(MemberStatus::Working));
/// assert!(blocks_teardown(MemberStatus::ShuttingDown));
/// assert!(!blocks_teardown(MemberStatus::Suspended));
/// assert!(!blocks_teardown(MemberStatus::Stopped));
/// ```
#[must_use]
pub fn blocks_teardown(status: MemberStatus) -> bool {
    status == MemberStatus::ShuttingDown || status.is_live()
}

/// The members that block a team teardown (spec `teamssetup` FR-022).
///
/// A member blocks teardown while it is `is_live()` **or** `ShuttingDown`. The
/// two sets are identical today - `is_live()` is `Spawning | Working |
/// ShuttingDown` - but the rule is spelled out in both terms on purpose: the
/// question a teardown asks is "does this member's session still have an action
/// to complete on its own?", which is the definition of `is_live()`, and the
/// explicit `ShuttingDown` arm documents that the status which *sounds*
/// terminal is one of them. A member that is `is_parked()` (waiting on a task,
/// a plan verdict, a dependency, or the lead) or `is_terminal()` has nothing in
/// flight and must not block teardown.
///
/// `force: true` overrides this on the tool surface only; see
/// [`cleanup_refusal`].
///
/// # Examples
///
/// ```
/// use ragent_agent::team::{MemberStatus, blocking_members};
///
/// let statuses = [MemberStatus::Working, MemberStatus::Idle, MemberStatus::Stopped];
/// let names = ["alpha", "beta", "gamma"];
/// let blocking = blocking_members(names.iter().copied().zip(statuses));
/// assert_eq!(blocking, ["alpha [working]"]);
/// // The status word is the panel label, not the wire token (FR-033).
/// let shutting = [("delta", MemberStatus::ShuttingDown)];
/// assert_eq!(blocking_members(shutting), ["delta [stopping]"]);
/// ```
#[must_use]
pub fn blocking_members<'a, I>(members: I) -> Vec<String>
where
    I: IntoIterator<Item = (&'a str, MemberStatus)>,
{
    members
        .into_iter()
        .filter(|(_, status)| blocks_teardown(*status))
        // FR-033: the status word comes from `display_label()`, never from the
        // serialization token, so a refusal cannot print `shutting_down` where
        // every panel prints `stopping`.
        .map(|(name, status)| format!("{name} {}", status_word(status)))
        .collect()
}

/// The single teardown refusal text shared by `/team cleanup`, `/team delete`,
/// and the `team_cleanup` tool (spec `teamssetup` FR-022), or `None` when the
/// teardown may proceed.
///
/// FR-022 requires the three surfaces to agree; sharing this function is what
/// makes agreement structural rather than a review promise. Every caller passes
/// its own member list and gets byte-identical wording back, so a divergence
/// cannot be introduced by editing one surface.
///
/// The refusal names every blocking member with its status - entity and state,
/// as FR-021 requires - and names the two next actions that exist.
///
/// # Examples
///
/// ```
/// use ragent_agent::team::{MemberStatus, cleanup_refusal};
///
/// // Nothing live or shutting down: the teardown may proceed.
/// let idle = [("beta", MemberStatus::Idle), ("gamma", MemberStatus::Stopped)];
/// assert_eq!(cleanup_refusal("tguard", idle), None);
///
/// // A working member blocks it, and the text names the member and the status.
/// let live = [("alpha", MemberStatus::Working)];
/// let text = cleanup_refusal("tguard", live).expect("a live member blocks teardown"); // no-panic-ok: doc example
/// assert!(text.contains("alpha [working]"));
/// assert!(text.contains("team_shutdown_teammate"));
/// ```
#[must_use]
pub fn cleanup_refusal<'a, I>(team_name: &str, members: I) -> Option<String>
where
    I: IntoIterator<Item = (&'a str, MemberStatus)>,
{
    let blocking = blocking_members(members);
    if blocking.is_empty() {
        return None;
    }
    Some(format!(
        "Cannot clean up team '{team_name}': {} teammate(s) are still live or shutting down: {}.\n\
         Shut them down with team_shutdown_teammate on each, then retry; \
         `/team forcecleanup` deactivates and removes them regardless.",
        blocking.len(),
        blocking.join(", ")
    ))
}

/// The confirmation `/team delete` demands before it removes a team directory,
/// or `None` when the delete may proceed (spec `teamssetup` FR-023, FR-030,
/// FR-045).
///
/// Deleting a team is a permanent filesystem removal, so the destructive
/// direction must be an explicit second action rather than a single verb typed
/// once (F-28). FR-045 makes that true of **every** named team, not only the
/// session's active one, so the two branches below are both confirmations and
/// neither is a way past this function:
///
/// - **Names held back.** A name the operator asked to keep with a `-` prefix
///   cannot be deleted at all, whatever the roster says, so an `exclude` list is
///   checked first and wins over every other branch.
/// - **Live members.** A roster with any member that [`blocks_teardown`] would
///   first be refused outright by [`cleanup_refusal`]; only an explicit
///   `force: true` passes the guard, and that override is what the first
///   confirmation confirms.
/// - **A quiet roster.** No member is in flight, so there is nothing to force,
///   but the directory still disappears for good: the second confirmation names
///   the team and states that the directory itself is what is removed.
///
/// Every branch names the team and the two actions that exist (proceed, or
/// abandon) - entity, state and next action, as FR-021 requires of every refusal.
///
/// # Examples
///
/// ```
/// use ragent_agent::team::{MemberStatus, delete_confirmation};
///
/// // Even a quiet roster needs the second action (FR-045): the directory goes.
/// let quiet = [("beta", MemberStatus::Idle), ("gamma", MemberStatus::Stopped)];
/// let quiet_text =
///     delete_confirmation("tguard", quiet, []).expect("every delete is confirmed"); // no-panic-ok: doc example
/// assert!(quiet_text.contains("/team delete tguard --force"));
///
/// // A live member forces the confirmation, which names it and its status.
/// let live = [("alpha", MemberStatus::Working)];
/// let text = delete_confirmation("tguard", live, []).expect("a live member must be confirmed"); // no-panic-ok: doc example
/// assert!(text.contains("alpha [working]"));
/// assert!(text.contains("/team delete tguard --force"));
///
/// // An excluded name is never deleted, so nothing is confirmed either.
/// assert_eq!(delete_confirmation("tguard", live, ["tguard"]), None);
/// ```
#[must_use]
pub fn delete_confirmation<'a, I, E>(team_name: &str, members: I, exclude: E) -> Option<String>
where
    I: IntoIterator<Item = (&'a str, MemberStatus)>,
    E: IntoIterator<Item = &'a str>,
{
    if exclude.into_iter().any(|name| name == team_name) {
        return None;
    }
    let blocking = blocking_members(members);
    if blocking.is_empty() {
        return Some(format!(
            "Confirmation required to delete team '{team_name}'.\n\
             Its directory is removed permanently, with its tasks, mailbox and roster.\n\
             {}\n\
             Nothing has been deleted.",
            confirm_instruction(team_name)
        ));
    }
    Some(format!(
        "Confirmation required to delete team '{team_name}'.\n\
         {} teammate(s) are still live or shutting down: {}.\n\
         {}\n\
         Nothing has been deleted.",
        blocking.len(),
        blocking.join(", "),
        confirm_instruction(team_name)
    ))
}

/// The command that performs a confirmed `/team delete` for `team_name`.
fn confirm_command(team_name: &str) -> String {
    format!("/team delete {team_name} --force")
}

/// The next-action sentence naming [`confirm_command`], kept short on purpose.
///
/// The TUI renders the message through `html2text` at a fixed 120-column width,
/// which breaks a long line at a space. The instruction is therefore a short
/// line of its own: an operator who copies the whole rendered paragraph would
/// otherwise copy a command split across two lines.
fn confirm_instruction(team_name: &str) -> String {
    format!("Confirm with: {}", confirm_command(team_name))
}

/// The reason a teammate whose plan has not been approved may not run
/// `tool_name`, or `None` when the call is allowed (T-307, FR-014).
///
/// The rule itself lives here, next to the team runtime, rather than inside the
/// session processor: it is a team policy, and the processor only asks the
/// question (spec `uniformtools` FR-012 keeps the dispatch pipeline's stages
/// named in one place, and this is the stage's predicate).
///
/// Read-only tools and the team coordination tools stay available - that is how
/// a teammate learns the verdict and how the lead inspects the work - while a
/// mutating tool would let a plan reach the workspace before it is approved.
///
/// `plan_gate` is `true` when `TeamSettings.require_plan_approval` is set **and**
/// this teammate's `plan_status` is not `Approved`, i.e. it has submitted
/// nothing, has a plan still awaiting the verdict, or had one rejected (F-10).
///
/// The blocked classes are **write-class** (every tool in
/// [`crate::session::loop_state::LOOP_WRITE_TOOLS`]) and **bash-class** (`Bash`
/// and its `run_code` alias, whether or not the command happens to be a write:
/// the policy does not try to guess intent from a command string).
///
/// # Examples
///
/// ```
/// use ragent_agent::team::plan_gate_denial;
///
/// // Read-only tools are never blocked by an unapproved plan.
/// assert_eq!(plan_gate_denial("read", false, true), None);
/// // A teammate with an unapproved plan cannot mutate work...
/// let reason = plan_gate_denial("Write", false, true).expect("write-class is gated"); // no-panic-ok: doc example
/// // ...and the refusal names the tool that clears the gate.
/// assert!(reason.contains("team_submit_plan"));
/// // Bash is gated as a class, not by inspecting the command.
/// assert!(plan_gate_denial("Bash", false, true).is_some());
/// // The lead is never gated: it is the one who approves.
/// assert_eq!(plan_gate_denial("Write", true, true), None);
/// ```
#[must_use]
pub fn plan_gate_denial(tool_name: &str, is_lead: bool, plan_gate: bool) -> Option<String> {
    if is_lead || !plan_gate {
        return None;
    }
    // A team coordination tool is how the teammate learns the verdict and how
    // the lead is answered, so it stays available; everything that is not a
    // workspace write or a shell command (reading, searching, computing) is not
    // blocked either.
    if tool_name.starts_with("team_")
        || !(crate::session::loop_state::LOOP_WRITE_TOOLS.contains(&tool_name)
            || crate::session::loop_state::LOOP_BASH_TOOLS.contains(&tool_name))
    {
        return None;
    }
    Some(format!(
        "plan pending: '{tool_name}' is a write-class or bash-class tool and your plan \
         has not been approved yet - call team_submit_plan to register the plan (if you \
         have not), then wait for the lead's verdict with the team_* tools before \
         changing any work"
    ))
}

// -- Undelivered messages (FR-020) ------------------------------------------------

/// A message that was enqueued for a teammate which terminated before it
/// could be read (spec `teamssetup` FR-020).
///
/// The runtime never deletes such a message: the mailbox file keeps it with
/// `read: false`, so it is still there for an operator. This record is the
/// *report* of it - one entry per message, carrying enough context to tell the
/// lead who wrote it, when, and what it said - so a message that no teammate
/// will ever read is surfaced instead of silently stranded on disk (F-16).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndeliveredMessage {
    /// The recipient that terminated with this message unread (`agent_id`).
    pub agent_id: String,
    /// The recipient's display name, when the roster still has the member.
    pub recipient: String,
    /// The status the recipient terminated in (`stopped` or `failed`).
    pub recipient_status: MemberStatus,
    /// The sender's agent ID, or `"lead"`.
    pub from: String,
    /// The message's semantic type, as its wire token (`shutdown_request`).
    pub message_type: &'static str,
    /// The message body, exactly as stored.
    pub content: String,
    /// When the sender enqueued the message (UTC).
    pub sent_at: DateTime<Utc>,
    /// The message's stable identifier.
    pub message_id: String,
}

impl UndeliveredMessage {
    /// Render one report row: entity, state, and the content that would
    /// otherwise be lost.
    #[must_use]
    pub fn report_line(&self) -> String {
        format!(
            "- to {} ({}) {} - from {} | type {} | sent {} | id {}\n    {}",
            self.recipient,
            self.agent_id,
            // FR-033: the recipient's status word is the panel label, so this
            // row and the roster it is paired with cannot disagree.
            status_word(self.recipient_status),
            self.from,
            self.message_type,
            self.sent_at.format("%Y-%m-%d %H:%M:%S UTC"),
            self.message_id,
            self.content.replace('\n', "\n    ")
        )
    }
}

/// Every message still unread in the mailbox of a member that has terminated,
/// in roster order (spec `teamssetup` FR-020).
///
/// A message addressed to a teammate that is already gone is never drained -
/// the mailbox poll loop that reads it belongs to the teammate - so the only
/// place it can surface is a report. This function finds those messages by
/// asking the two questions that make one undelivered: *has the recipient
/// terminated?* ([`MemberStatus::is_terminal`]) and *is the message still
/// unread?* A member that is alive, parked, or merely shutting down is skipped
/// entirely: its mailbox will still be drained when it wakes, so reporting its
/// backlog would be noise on every status call.
///
/// A mailbox that cannot be read is skipped rather than failing the report:
/// the report's job is to surface messages, and a corrupt inbox is already
/// diagnosed by the mailbox layer itself.
///
/// # Examples
///
/// ```
/// use ragent_agent::team::{MemberStatus, MemberStatus::Stopped, undelivered_messages};
///
/// # let dir = std::path::Path::new("target/temp/doc-undelivered");
/// # let _ = std::fs::remove_dir_all(dir);
/// # std::fs::create_dir_all(dir).unwrap(); // no-panic-ok: doc example
/// # let store = ragent_agent::team::TeamStore::create("doc-team", "lead", dir, true).unwrap(); // no-panic-ok: doc example
/// # let mut store = store;
/// # let mut member = ragent_agent::team::TeamMember::new("alpha", "tm-001", "general");
/// # member.status = MemberStatus::Stopped;
/// # store.add_member(member).unwrap(); // no-panic-ok: doc example
/// // A stopped teammate with nothing unread has nothing to report.
/// assert!(undelivered_messages(&store).is_empty());
/// ```
#[must_use]
pub fn undelivered_messages(store: &TeamStore) -> Vec<UndeliveredMessage> {
    let mut found = Vec::new();
    for member in &store.config.members {
        if !member.status.is_terminal() {
            continue;
        }
        let Ok(mailbox) = store.mailbox(&member.agent_id) else {
            continue;
        };
        let Ok(messages) = mailbox.peek_unread() else {
            continue;
        };
        for message in messages {
            found.push(UndeliveredMessage {
                agent_id: member.agent_id.clone(),
                recipient: member.name.clone(),
                recipient_status: member.status,
                from: message.from.clone(),
                message_type: message_type_token(&message.message_type),
                content: message.content.clone(),
                sent_at: message.sent_at,
                message_id: message.message_id.clone(),
            });
        }
    }
    found
}

/// The wire token of a [`MessageType`], matching its on-disk `snake_case` form.
fn message_type_token(message_type: &MessageType) -> &'static str {
    match message_type {
        MessageType::Message => "message",
        MessageType::Broadcast => "broadcast",
        MessageType::PlanRequest => "plan_request",
        MessageType::PlanApproved => "plan_approved",
        MessageType::PlanRejected => "plan_rejected",
        MessageType::IdleNotify => "idle_notify",
        MessageType::ShutdownRequest => "shutdown_request",
        MessageType::ShutdownAck => "shutdown_ack",
        MessageType::TaskCancelled => "task_cancelled",
    }
}

/// Render the undelivered-message block shared by `/team show` and
/// `team_status` (spec `teamssetup` FR-020), or an empty string when there is
/// nothing undelivered.
///
/// Both surfaces call this one function so the two can never disagree about
/// what was stranded, and the block names the two actions that exist: read the
/// message, or start the recipient again. `heading_level` is the caller's
/// heading depth, because `/team show` nests the block under its own section
/// while `team_status` prints it at the top level.
///
/// # Examples
///
/// ```
/// use ragent_agent::team::{MemberStatus, UndeliveredMessage, render_undelivered};
///
/// let stranded = UndeliveredMessage {
///     agent_id: "tm-001".to_string(),
///     recipient: "alpha".to_string(),
///     recipient_status: MemberStatus::Stopped,
///     from: "lead".to_string(),
///     message_type: "message",
///     content: "please review".to_string(),
///     sent_at: chrono::Utc::now(),
///     message_id: "m-1".to_string(),
/// };
/// let block = render_undelivered("tguard", &[stranded], 2);
/// assert!(block.contains("## Undelivered messages (1)"));
/// assert!(block.contains("alpha (tm-001)"));
/// assert!(block.contains("please review"));
/// // Nothing stranded renders nothing at all - no empty section.
/// assert!(render_undelivered("tguard", &[], 2).is_empty());
/// ```
#[must_use]
pub fn render_undelivered(
    team_name: &str,
    undelivered: &[UndeliveredMessage],
    heading_level: usize,
) -> String {
    if undelivered.is_empty() {
        return String::new();
    }
    let hashes = "#".repeat(heading_level);
    let mut out = format!(
        "{hashes} Undelivered messages ({})\n\n\
         {} message(s) were still unread when their recipient terminated, so no \
         teammate will read them. They are kept in the mailbox, not deleted.\n\n",
        undelivered.len(),
        undelivered.len()
    );
    for message in undelivered {
        out.push_str(&message.report_line());
        out.push('\n');
    }
    out.push_str(&format!(
        "\nRead one with `team_read_messages` (as that recipient), or start the \
         recipient again in team '{team_name}'.\n"
    ));
    out
}

// -- Pending plans (FR-034) -------------------------------------------------------

/// A teammate plan waiting for the lead's verdict (spec `teamssetup` FR-034).
///
/// A teammate in [`MemberStatus::PlanPending`] is parked until the lead
/// approves or rejects the plan it submitted. The plan itself lives in the
/// lead's mailbox as the `plan_request` message the teammate pushed, so this
/// record is the *join* of the roster row and that message: the row says who is
/// waiting, the message says what they are waiting on.
///
/// The body is `None` when the plan request cannot be found - a membership row
/// can be `PlanPending` before its message is written, or after the lead's
/// mailbox has been archived - so the caller still reports *that* a verdict is
/// awaited instead of silently showing nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingPlan {
    /// The parked member's agent ID.
    pub agent_id: String,
    /// The parked member's display name.
    pub name: String,
    /// The plan text as the teammate submitted it, when the request is present.
    pub plan: Option<String>,
    /// When the teammate submitted the plan (UTC), when the request is present.
    pub submitted_at: Option<DateTime<Utc>>,
    /// The correlation id linking this request to its verdict, when known.
    pub request_id: Option<String>,
}

impl PendingPlan {
    /// The approve action an operator or agent takes on this plan.
    ///
    /// The string is the exact `team_approve_plan` invocation, so the row
    /// teaches the action rather than only naming it.
    #[must_use]
    pub fn approve_action(&self, team_name: &str) -> String {
        format!(
            "team_approve_plan {{ team_name: \"{team_name}\", teammate: \"{}\", approved: true }}",
            self.agent_id
        )
    }

    /// The reject action, which requires feedback (the tool refuses without it).
    #[must_use]
    pub fn reject_action(&self, team_name: &str) -> String {
        format!(
            "team_approve_plan {{ team_name: \"{team_name}\", teammate: \"{}\", approved: false, feedback: \"...\" }}",
            self.agent_id
        )
    }

    /// Render one report row: who is parked, what they submitted, and both
    /// actions that resolve it.
    #[must_use]
    pub fn report_line(&self, team_name: &str) -> String {
        let submitted = self
            .submitted_at
            .map(|at| at.format("%Y-%m-%d %H:%M:%S UTC").to_string())
            .unwrap_or_else(|| "unknown time".to_string());
        let plan = match self.plan.as_deref() {
            Some(plan) => plan.replace('\n', "\n    "),
            None => "(plan request not found in the lead's mailbox)".to_string(),
        };
        format!(
            "- {} ({}) {} - submitted {submitted}\n    {plan}\n    \
             approve: {}\n    reject:  {}",
            self.name,
            self.agent_id,
            // FR-033: the parked status is the panel label, not the wire token.
            status_word(MemberStatus::PlanPending),
            self.approve_action(team_name),
            self.reject_action(team_name)
        )
    }
}

/// Every teammate parked in [`MemberStatus::PlanPending`], in roster order,
/// paired with the plan it submitted (spec `teamssetup` FR-034).
///
/// The plan body is read from the `plan_request` message the teammate pushed
/// into the lead's mailbox. That message is matched on the member's
/// `plan_request_id` when it has one (M5-T4's correlation id, which
/// [`TeamMember::plan_request_id`] carries) and otherwise on the most recent
/// `plan_request` that member sent, so a plan submitted before the id was
/// recorded still surfaces.
///
/// An unreadable mailbox is skipped rather than failing the report: the report
/// exists to surface parked teammates, and a team with no lead mailbox yet
/// simply reports the plans as unknown rather than reporting nothing.
#[must_use]
pub fn pending_plans(store: &TeamStore) -> Vec<PendingPlan> {
    let parked: Vec<&TeamMember> = store
        .config
        .members
        .iter()
        .filter(|member| member.status == MemberStatus::PlanPending)
        .collect();
    if parked.is_empty() {
        return Vec::new();
    }
    // The lead's inbox is read once for the whole roster (NFR-002 spirit: one
    // read per report, never one per row).
    let lead_inbox = store
        .mailbox("lead")
        .and_then(|mailbox| mailbox.peek_unread())
        .unwrap_or_default();
    parked
        .into_iter()
        .map(|member| {
            let request = plan_request_for(&lead_inbox, member);
            PendingPlan {
                agent_id: member.agent_id.clone(),
                name: member.name.clone(),
                plan: request.map(|message| message.content.clone()),
                submitted_at: request.map(|message| message.sent_at),
                request_id: member
                    .plan_request_id
                    .clone()
                    .or_else(|| request.and_then(|message| message.correlation_id.clone())),
            }
        })
        .collect()
}

/// Find the `plan_request` message that belongs to `member`.
///
/// Prefers the correlation id the roster recorded; falls back to the newest
/// `plan_request` from that agent so a pre-correlation plan still resolves.
fn plan_request_for<'a>(
    lead_inbox: &'a [MailboxMessage],
    member: &TeamMember,
) -> Option<&'a MailboxMessage> {
    let is_plan_request = |message: &&MailboxMessage| {
        message.message_type == MessageType::PlanRequest && message.from == member.agent_id
    };
    if let Some(request_id) = member.plan_request_id.as_deref()
        && let Some(message) = lead_inbox
            .iter()
            .filter(is_plan_request)
            .find(|message| message.correlation_id.as_deref() == Some(request_id))
    {
        return Some(message);
    }
    lead_inbox
        .iter()
        .filter(is_plan_request)
        .max_by_key(|message| message.sent_at)
}

/// Render the pending-plan block shared by `/team show` and `team_status`
/// (spec `teamssetup` FR-034), or an empty string when nothing is parked.
///
/// Both surfaces call this one function so they can never disagree about which
/// plans are waiting, and so neither shows a `plan_pending` member without the
/// plan and the action that unblocks them. `heading_level` is the caller's
/// heading depth, matching [`render_undelivered`].
///
/// # Examples
///
/// ```
/// use ragent_agent::team::{PendingPlan, render_pending_plans};
///
/// let waiting = PendingPlan {
///     agent_id: "tm-001".to_string(),
///     name: "alpha".to_string(),
///     plan: Some("1. read the spec".to_string()),
///     submitted_at: None,
///     request_id: Some("r-1".to_string()),
/// };
/// let block = render_pending_plans("tplan", &[waiting], 2);
/// assert!(block.contains("## Plans awaiting a verdict (1)"));
/// assert!(block.contains("1. read the spec"));
/// assert!(block.contains("approved: true"));
/// // Nothing parked renders nothing at all - no empty section.
/// assert!(render_pending_plans("tplan", &[], 2).is_empty());
/// ```
#[must_use]
pub fn render_pending_plans(
    team_name: &str,
    pending: &[PendingPlan],
    heading_level: usize,
) -> String {
    if pending.is_empty() {
        return String::new();
    }
    let hashes = "#".repeat(heading_level);
    let mut out = format!(
        "{hashes} Plans awaiting a verdict ({})\n\n\
         {} teammate(s) are parked {}: they will not write or run a \
         shell command until the plan is approved or rejected.\n\n",
        pending.len(),
        pending.len(),
        // FR-033: the status word is the panel label, so this sentence cannot
        // spell the state differently from the row above it.
        status_word(MemberStatus::PlanPending)
    );
    for plan in pending {
        out.push_str(&plan.report_line(team_name));
        out.push('\n');
    }
    out.push_str(
        "\nA rejection requires feedback; the teammate revises the plan and \
         resubmits with `team_submit_plan`.\n",
    );
    out
}
