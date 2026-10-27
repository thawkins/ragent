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
//! | [`task`]   | `Task`, `TaskStatus`, `TaskList`, `TaskStore` |
//! | [`mailbox`]| `MailboxMessage`, `MessageType`, `Mailbox` |
//! | [`store`]  | `TeamStore`, `find_team_dir`, directory discovery |
//! | [`manager`]| `TeamManager` runtime for spawning/polling teammates |
//! | [`swarm`]  | LLM decomposition of a goal into parallel subtasks |

pub mod classify;
pub mod config;
pub mod mailbox;
pub mod manager;
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
    HookEntry, HookEvent, MemberStatus, MemoryScope, PlanStatus, TeamConfig, TeamMember,
    TeamSettings, TeamStatus, resolve_memory_dir,
};
pub use mailbox::{
    MAX_MESSAGE_BYTES, Mailbox, MailboxMessage, MessageType, deregister_notifier, register_notifier,
};
pub use manager::{
    HookOutcome, MAX_HOOK_FEEDBACK_BYTES, TeamManager, build_team_prompt_addition, run_hook,
    run_team_hook, teammate_retry_backoff, validate_hook_command,
};
pub use store::{
    TeamStore, find_project_teams_dir, find_team_dir, global_teams_dir, slugify_team_name,
    validate_team_name,
};
pub use swarm::{
    DECOMPOSITION_SYSTEM_PROMPT, SwarmDecomposition, SwarmState, SwarmSubtask,
    build_decomposition_user_prompt, parse_decomposition, parse_decomposition_with_default,
};
pub use task::{Task, TaskList, TaskStatus, TaskStore};

// -- Guard helpers ---------------------------------------------------------------

/// The reason a teammate whose plan awaits the lead's verdict may not run
/// `tool_name`, or `None` when the call is allowed (T-307).
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
/// # Examples
///
/// ```
/// use ragent_agent::team::plan_gate_denial;
///
/// // Read-only tools are never blocked by a pending plan.
/// assert_eq!(plan_gate_denial("read", false, true), None);
/// // A teammate with a pending plan cannot mutate work.
/// assert!(plan_gate_denial("Write", false, true).is_some());
/// // The lead is never gated: it is the one who approves.
/// assert_eq!(plan_gate_denial("Write", true, true), None);
/// ```
#[must_use]
pub fn plan_gate_denial(tool_name: &str, is_lead: bool, plan_pending: bool) -> Option<String> {
    if is_lead || !plan_pending {
        return None;
    }
    // A team coordination tool is how the teammate learns the verdict and how
    // the lead is answered, so it stays available; everything that is not a
    // workspace write (reading, searching, computing) is not blocked either.
    if tool_name.starts_with("team_")
        || !crate::session::loop_state::LOOP_WRITE_TOOLS.contains(&tool_name)
    {
        return None;
    }
    Some(format!(
        "plan pending: '{tool_name}' is a mutating tool and your plan has not been \
         approved yet - wait for the lead's verdict with the team_* tools, or ask \
         the lead to approve the plan, before changing any work"
    ))
}
