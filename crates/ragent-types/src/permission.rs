//! Permission types
//!
//! Core permission types used by the event system.
//! Full permission checking logic is in ragent-config.

use serde::{Deserialize, Serialize};

/// Permission request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub permission: Permission,
    pub resource: String,
    pub description: String,
    pub agent_id: Option<String>,
    #[serde(default)]
    pub created_at: u64,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_timeout() -> u64 {
    120
}

/// Permission decision from user
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PermissionDecision {
    Once,
    Always,
    Deny,
}

/// Permission types
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    FileRead,
    FileWrite,
    FileEdit,
    BashExecute,
    WebFetch,
    WebSearch,
    PlanEnter,
    AgentSpawn,
    ConfigWrite,
    MemoryWrite,
    TodoWrite,
    Custom(String),
}

impl std::fmt::Display for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Permission::FileRead => write!(f, "file:read"),
            Permission::FileWrite => write!(f, "file:write"),
            Permission::FileEdit => write!(f, "file:edit"),
            Permission::BashExecute => write!(f, "bash:execute"),
            Permission::WebFetch => write!(f, "web:fetch"),
            Permission::WebSearch => write!(f, "web:search"),
            Permission::PlanEnter => write!(f, "plan:enter"),
            Permission::AgentSpawn => write!(f, "agent:spawn"),
            Permission::ConfigWrite => write!(f, "config:write"),
            Permission::MemoryWrite => write!(f, "memory:write"),
            Permission::TodoWrite => write!(f, "todo:write"),
            Permission::Custom(s) => write!(f, "{}", s),
        }
    }
}
