//! Extracted team coordination tools.

pub use ragent_agent::tool::{Tool, ToolContext, ToolOutput};

use std::sync::Arc;

use ragent_agent::tool::ToolRegistry;

pub mod team_approve_plan;
pub mod team_assign_task;
pub mod team_broadcast;
pub mod team_cleanup;
pub mod team_create;
pub mod team_idle;
pub mod team_memory_read;
pub mod team_memory_write;
pub mod team_message;
pub mod team_read_messages;
pub mod team_shutdown_ack;
pub mod team_shutdown_teammate;
pub mod team_spawn;
pub mod team_status;
pub mod team_submit_plan;
pub mod team_task_claim;
pub mod team_task_complete;
pub mod team_task_create;
pub mod team_task_list;
pub mod team_wait;

/// Register all extracted team tools into an existing tool registry.
pub fn register_team_tools(registry: &ToolRegistry) {
    registry.register(Arc::new(team_approve_plan::TeamApprovePlanTool));
    registry.register(Arc::new(team_assign_task::TeamAssignTaskTool));
    registry.register(Arc::new(team_broadcast::TeamBroadcastTool));
    registry.register(Arc::new(team_cleanup::TeamCleanupTool));
    registry.register(Arc::new(team_create::TeamCreateTool));
    registry.register(Arc::new(team_idle::TeamIdleTool));
    registry.register(Arc::new(team_memory_read::TeamMemoryReadTool));
    registry.register(Arc::new(team_memory_write::TeamMemoryWriteTool));
    registry.register(Arc::new(team_message::TeamMessageTool));
    registry.register(Arc::new(team_read_messages::TeamReadMessagesTool));
    registry.register(Arc::new(team_shutdown_ack::TeamShutdownAckTool));
    registry.register(Arc::new(team_shutdown_teammate::TeamShutdownTeammateTool));
    registry.register(Arc::new(team_spawn::TeamSpawnTool));
    registry.register(Arc::new(team_status::TeamStatusTool));
    registry.register(Arc::new(team_submit_plan::TeamSubmitPlanTool));
    registry.register(Arc::new(team_task_claim::TeamTaskClaimTool));
    registry.register(Arc::new(team_task_complete::TeamTaskCompleteTool));
    registry.register(Arc::new(team_task_create::TeamTaskCreateTool));
    registry.register(Arc::new(team_task_list::TeamTaskListTool));
    registry.register(Arc::new(team_wait::TeamWaitTool));
}
