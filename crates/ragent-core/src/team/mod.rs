//! Team module — Agent Team Coordination for ragent.
//!
//! This module provides the data structures and I/O layer for the "Teams"
//! capability.  A team consists of a lead session and one or more named
//! *teammate* sessions that coordinate via a shared task list and per-agent
//! mailboxes stored on disk.
//!
//! ## Sub-modules
//!
//! | Module | Contents |
//! |--------|----------|
//! | [`config`] | `TeamConfig`, `TeamMember`, `TeamStatus`, `MemberStatus` |
//! | [`task`]   | `Task`, `TaskStatus`, `TaskList`, `TaskStore` |
//! | [`mailbox`]| `MailboxMessage`, `MessageType`, `Mailbox` |
//! | [`store`]  | `TeamStore`, `find_team_dir`, directory discovery |

pub mod config;
pub mod mailbox;
pub mod manager;
pub mod store;
/// Swarm — fleet-style auto-decomposition into parallel subtasks.
pub mod swarm;
pub mod task;

// ── Re-exports ────────────────────────────────────────────────────────────────

pub use config::{HookEntry, HookEvent, MemberStatus, MemoryScope, PlanStatus, TeamConfig, TeamMember, TeamSettings, TeamStatus, resolve_memory_dir};
pub use mailbox::{Mailbox, MailboxMessage, MessageType, register_notifier, deregister_notifier};
pub use manager::{HookOutcome, TeamManager, build_team_prompt_addition, run_hook, run_team_hook};
pub use store::{TeamStore, find_team_dir, find_project_teams_dir, global_teams_dir};
pub use swarm::{SwarmDecomposition, SwarmState, SwarmSubtask, parse_decomposition, build_decomposition_user_prompt, DECOMPOSITION_SYSTEM_PROMPT};
pub use task::{Task, TaskList, TaskStatus, TaskStore};
