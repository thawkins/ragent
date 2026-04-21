//! OASF (Open Agentic Schema Framework) data structures for custom agent records.
//!
//! Custom agents are stored as UTF-8 JSON files using a ragent-adapted profile
//! of the OASF standard (<https://schema.oasf.outshift.com/>). The file uses the
//! standard OASF envelope (`name`, `description`, `version`, `schema_version`,
//! `skills`, `domains`, `locators`, `modules`) and embeds ragent-specific
//! configuration inside a module with `type: "ragent/agent/v1"`.
//!
//! # Minimal Example
//!
//! ```json
//! {
//!   "name": "my-reviewer",
//!   "description": "Code reviewer focused on security",
//!   "version": "1.0.0",
//!   "schema_version": "0.7.0",
//!   "modules": [{
//!     "type": "ragent/agent/v1",
//!     "payload": {
//!       "system_prompt": "You are an expert code reviewer...",
//!       "mode": "primary",
//!       "max_steps": 30
//!     }
//!   }]
//! }
//! ```

use serde::{Deserialize, Serialize};

/// The OASF module type identifier for ragent agent definitions.
pub const RAGENT_MODULE_TYPE: &str = "ragent/agent/v1";

/// Top-level OASF agent record.
///
/// This is the structure serialised to / from `.json` files in the
/// `.ragent/agents/` discovery directories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OasfAgentRecord {
    /// Unique identifier for the agent (kebab-case, e.g. `"my-reviewer"`).
    pub name: String,
    /// One-line human-readable description shown in the TUI picker.
    pub description: String,
    /// Semantic version of this agent definition (e.g. `"1.0.0"`).
    pub version: String,
    /// OASF schema version this record targets (e.g. `"0.7.0"`).
    pub schema_version: String,
    /// Authors in `"Name <email>"` format.
    #[serde(default)]
    pub authors: Vec<String>,
    /// RFC 3339 creation timestamp.
    pub created_at: Option<String>,
    /// OASF skill taxonomy annotations (informational, not ragent skill invocation).
    #[serde(default)]
    pub skills: Vec<OasfSkill>,
    /// OASF domain taxonomy annotations (informational).
    #[serde(default)]
    pub domains: Vec<OasfDomain>,
    /// Source-code or registry references.
    #[serde(default)]
    pub locators: Vec<OasfLocator>,
    /// Extension modules. Must contain exactly one `ragent/agent/v1` module.
    #[serde(default)]
    pub modules: Vec<OasfModule>,
}

/// An OASF skill annotation for a record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OasfSkill {
    /// Hierarchical skill name from the OASF taxonomy
    /// (e.g. `"software_engineering/code_review"`).
    pub name: String,
    /// Numeric skill class identifier from the OASF catalog.
    pub id: u64,
}

/// An OASF domain annotation for a record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OasfDomain {
    /// Hierarchical domain name from the OASF taxonomy
    /// (e.g. `"technology/software_development"`).
    pub name: String,
    /// Numeric domain class identifier from the OASF catalog.
    pub id: u64,
}

/// A locator pointing to source code or a package registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OasfLocator {
    /// Locator type: `"source_code"`, `"docker"`, `"url"`, etc.
    #[serde(rename = "type")]
    pub locator_type: String,
    /// One or more URL strings for this locator.
    #[serde(default)]
    pub urls: Vec<String>,
}

/// An OASF extension module attached to a record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OasfModule {
    /// Module type discriminator. For ragent agents this is `"ragent/agent/v1"`.
    #[serde(rename = "type")]
    pub module_type: String,
    /// Module-specific payload. Decoded separately depending on `module_type`.
    pub payload: serde_json::Value,
}

/// Payload of a `ragent/agent/v1` module — the ragent-specific agent definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagentAgentPayload {
    /// System prompt for the agent.
    ///
    /// Supports template variables: `{{WORKING_DIR}}`, `{{FILE_TREE}}`,
    /// `{{AGENTS_MD}}`, `{{DATE}}`.
    pub system_prompt: String,

    /// Agent availability mode: `"primary"`, `"subagent"`, or `"all"`.
    /// Defaults to `"all"` when absent.
    pub mode: Option<String>,

    /// Maximum number of agentic loop iterations. Defaults to `100`.
    pub max_steps: Option<u32>,

    /// Model sampling temperature override (0.0–2.0).
    pub temperature: Option<f32>,

    /// Nucleus sampling override.
    pub top_p: Option<f32>,

    /// Model binding in `"provider:model"` format
    /// (e.g. `"anthropic:claude-haiku-4-5"`). Inherits global model when absent.
    pub model: Option<String>,

    /// Ragent skill names to preload for this agent.
    #[serde(default)]
    pub skills: Vec<String>,

    /// Permission ruleset governing tool access. Inherits default permissions
    /// when absent.
    pub permissions: Option<Vec<RagentPermissionRule>>,

    /// If `true`, the agent is omitted from user-visible pickers. Defaults to `false`.
    pub hidden: Option<bool>,

    /// Persistent memory scope: `"user"`, `"project"`, or `"none"` (default).
    /// When enabled, the agent receives a dedicated memory directory.
    #[serde(default)]
    pub memory: Option<String>,

    /// Arbitrary key-value options forwarded verbatim to the LLM provider.
    pub options: Option<serde_json::Value>,
}

/// A single permission rule in a `ragent/agent/v1` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagentPermissionRule {
    /// Permission type: `"read"`, `"edit"`, `"bash"`, `"web"`, `"question"`,
    /// `"todo"`, `"plan_enter"`, `"plan_exit"`.
    pub permission: String,
    /// Glob pattern the rule applies to (e.g. `"**"`, `"src/**"`).
    pub pattern: String,
    /// Action to take: `"allow"`, `"deny"`, or `"ask"`.
    pub action: String,
}
