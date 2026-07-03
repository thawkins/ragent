//! Team runtime and team coordination tools for ragent.
//!
//! This crate is a thin re-export shim.  The team runtime (config, task,
//! mailbox, store, manager, swarm, classify) and the team tools
//! (`team_create`, `team_spawn`, `team_message`, etc.) now live natively in
//! `ragent-agent::team` and `ragent-agent::tool`.  They were previously
//! compiled into `ragent-agent` via `#[path]` attributes, which created a
//! logical dependency cycle (see `REMPLAN.md` M3 / T3.5).
//!
//! All existing `ragent_team::team::*` and `ragent_team::tool::*` import
//! sites continue to resolve unchanged via the re-exports below.

pub use ragent_agent::{agent, event, message, session};

/// Re-export `apply_teammate_model_override` from the team manager so the
/// `test_team_manager` integration test can reach it via `ragent_team::`.
pub use ragent_agent::team::manager::apply_teammate_model_override;

/// Team runtime: config, task, mailbox, store, manager, swarm, classify.
///
/// Re-exported from `ragent_agent::team` — see `REMPLAN.md` M3 for the
/// consolidation history.
pub mod team {
    pub use ragent_agent::team::*;
}

/// Tool API types shared between the agent and team tools.
///
/// Re-exported from `ragent_agent::tool`.
pub mod tool {
    pub use ragent_agent::tool::{
        TeamContext, TeamManagerInterface, Tool, ToolContext, ToolOutput, ToolRegistry,
    };

    /// Metadata builder utilities reused by the extracted team tools.
    pub mod metadata {
        pub use ragent_agent::tool::metadata::*;
    }

    /// Create the standard tool registry with the team tools added on top.
    #[must_use]
    pub fn create_default_registry() -> ToolRegistry {
        ragent_agent::tool::create_default_registry()
    }
}

/// Team coordination tool modules (create, spawn, message, tasks, etc.).
///
/// Re-exported from `ragent_agent::tool`.
pub mod tools {
    pub use ragent_agent::tool::{
        team_approve_plan, team_assign_task, team_broadcast, team_cleanup, team_create,
        team_idle, team_memory_read, team_memory_write, team_message, team_read_messages,
        team_shutdown_ack, team_shutdown_teammate, team_spawn, team_status, team_submit_plan,
        team_task_claim, team_task_complete, team_task_create, team_task_list, team_wait,
    };
}

pub use team::*;