//! Team module — Agent Team Coordination for ragent.
//!
//! This module is a thin source-level re-export of the implementation that
//! lives in [`ragent_team::team`].  The source files are included via
//! `#[path]` so that both crates share a single source-of-truth without
//! introducing a Cargo dependency cycle (see `docs/team-unification-decision.md`).
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

#[path = "../../../ragent-team/src/team/classify.rs"]
pub mod classify;
#[path = "../../../ragent-team/src/team/config.rs"]
pub mod config;
#[path = "../../../ragent-team/src/team/mailbox.rs"]
pub mod mailbox;
#[path = "../../../ragent-team/src/team/manager.rs"]
pub mod manager;
#[path = "../../../ragent-team/src/team/store.rs"]
pub mod store;
/// Swarm — fleet-style auto-decomposition into parallel subtasks.
#[path = "../../../ragent-team/src/team/swarm.rs"]
pub mod swarm;
#[path = "../../../ragent-team/src/team/task.rs"]
pub mod task;

// ── Re-exports ─────────────────────────────────────────────────────────────────

pub use classify::{
    DEFAULT_AGENT_TYPE, KNOWN_AGENT_TYPES, extract_explicit_agent_type, infer_agent_type,
    is_known_agent_type, resolve_agent_type, strip_explicit_agent_type_hint,
};

pub use config::{
    HookEntry, HookEvent, MemberStatus, MemoryScope, PlanStatus, TeamConfig, TeamMember,
    TeamSettings, TeamStatus, resolve_memory_dir,
};
pub use mailbox::{Mailbox, MailboxMessage, MessageType, deregister_notifier, register_notifier};
pub use manager::{
    HookOutcome, TeamManager, build_team_prompt_addition, run_hook, run_team_hook,
    teammate_retry_backoff,
};
pub use store::{TeamStore, find_project_teams_dir, find_team_dir, global_teams_dir};
pub use swarm::{
    DECOMPOSITION_SYSTEM_PROMPT, SwarmDecomposition, SwarmState, SwarmSubtask,
    build_decomposition_user_prompt, parse_decomposition, parse_decomposition_with_default,
};
pub use task::{Task, TaskList, TaskStatus, TaskStore};
