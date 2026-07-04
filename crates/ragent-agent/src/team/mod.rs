//! Team module — Agent Team Coordination for ragent.
//!
//! This module owns the team runtime (config, task, mailbox, store, manager,
//! swarm, classify) natively in `ragent-agent`.  Previously these sources
//! lived in `ragent-team` and were compiled into `ragent-agent` via
//! `#[path]` attributes; they have been moved here to eliminate the
//! `#[path]` cycle workaround (see `REMPLAN.md` M3 / T3.3).
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
/// Swarm — fleet-style auto-decomposition into parallel subtasks.
pub mod swarm;
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
