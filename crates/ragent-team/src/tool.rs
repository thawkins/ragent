//! Compatibility re-exports for the extracted team layer.

pub use ragent_agent::tool::{
    TeamContext, TeamManagerInterface, Tool, ToolContext, ToolOutput, ToolRegistry,
};

/// Metadata builder utilities reused by the extracted team tools.
pub mod metadata {
    pub use ragent_agent::tool::metadata::*;
}

/// Create the standard tool registry with the extracted team tools added on top.
#[must_use]
pub fn create_default_registry() -> ToolRegistry {
    let registry = ragent_agent::tool::create_default_registry();
    crate::tools::register_team_tools(&registry);
    registry
}
