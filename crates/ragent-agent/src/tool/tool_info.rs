//! `tool_info` and `commands_info` — read-only registry introspection tools.
//!
//! `tool_info` returns a JSON dump of every tool registered in the agent's
//! [`ToolRegistry`] (name, description, parameters schema, permission
//! category, source-family, hidden/visible state). `commands_info` does the
//! same for the TUI slash-command surface (trigger, description, subcommands,
//! flags) plus any plugin-contributed commands resolved live from the enabled
//! plugin store.
//!
//! The slash-command data comes from the static catalog in
//! [`super::command_catalog`], which mirrors `SLASH_COMMANDS` in
//! `crates/ragent-tui/src/app/state.rs` (`ragent-agent` cannot depend on
//! `ragent-tui`); a drift test in `crates/ragent-tui/tests/` keeps the two in
//! sync.

use std::sync::Arc;

use anyhow::Result;
use serde::Serialize;
use serde_json::{Value, json};

use super::command_catalog::COMMAND_CATALOG;
use super::{Tool, ToolContext, ToolOutput, ToolRegistry};

/// Serialisable snapshot of one registered tool.
#[derive(Debug, Clone, Serialize)]
struct ToolInfoEntry {
    /// Tool name used to invoke it.
    name: String,
    /// Human-readable description.
    description: String,
    /// JSON Schema for the tool's parameters.
    parameters: Value,
    /// Permission category (e.g. `file:read`, `bash`, `none`).
    permission_category: String,
    /// Where the tool comes from: `internal`, `mcp:<server-id>`, or
    /// `plugin:<id>`.
    source: String,
    /// For MCP wrappers: the original MCP server id and tool name.
    #[serde(skip_serializing_if = "Option::is_none")]
    mcp: Option<McpToolRef>,
    /// True when the tool is hidden from the model by a visibility switch.
    hidden: bool,
}

/// MCP provenance for a bridged tool (`mcp:<server>` source).
#[derive(Debug, Clone, Serialize)]
struct McpToolRef {
    /// MCP server id the tool was bridged from.
    server_id: String,
    /// Tool name as advertised by the MCP server.
    tool_name: String,
}

/// One plugin-contributed slash command in the `commands_info` report.
#[derive(Debug, Clone, Serialize)]
struct PluginCommandInfo {
    /// The plugin that contributes the command.
    plugin_id: String,
    /// The command's own name.
    name: String,
    /// The slash trigger actually registered (bare name or namespaced).
    trigger: String,
    /// Human-readable description.
    description: String,
    /// True when this is a prompt command (injected as a user turn).
    is_prompt_command: bool,
}

/// JSON envelope returned by [`CommandsInfoTool`].
#[derive(Debug, Clone, Serialize)]
struct CommandsInfoReport {
    /// Static catalog of built-in slash commands (from `SLASH_COMMANDS`).
    builtin: Vec<super::command_catalog::CommandCatalogEntry>,
    /// Slash commands contributed by enabled plugins (live resolution).
    plugin_commands: Vec<PluginCommandInfo>,
    /// Total number of slash commands (builtin + plugin).
    total: usize,
}

/// Read-only tool that dumps the full tool registry as JSON.
///
/// Lets the LLM answer "which tools are available right now, and what are
/// their parameters?" without guessing from the system-prompt summary.
pub struct ToolInfoTool;

/// Read-only tool that dumps the slash-command surface as JSON.
pub struct CommandsInfoTool;

/// Resolve the source-family of a registered tool for the `source` column.
///
/// Mirrors the logic used by the TUI `/tools` report:
/// - `mcp:<server-id>` for MCP-bridged tools (`McpToolWrapper`, identified
///   through [`Tool::mcp_wrapper_info`]); the server id may itself contain
///   `_`, so it cannot be recovered from the mangled ragent name alone);
/// - `plugin:<id>` for plugin-registered tools (`plugin_<id>_<tool>`);
/// - the visibility-switch family name for family-gated built-ins;
/// - `internal` otherwise.
fn tool_source(tool: &Arc<dyn Tool>) -> String {
    if let Some((server_id, _tool_name)) = tool.mcp_wrapper_info() {
        return format!("mcp:{server_id}");
    }
    let name = tool.name();
    if let Some(rest) = name.strip_prefix("plugin_") {
        if let Some((id, _tool)) = rest.split_once('_') {
            return format!("plugin:{id}");
        }
        return "plugin".to_string();
    }
    for switch in [
        "office",
        "github",
        "gitlab",
        "teams",
        "agents",
        "plan",
        "codeindex",
        "masterfetch",
        "browser",
        "finance",
    ] {
        if let Some(names) = ragent_config::tool_family_names(switch)
            && names.contains(&name)
        {
            return format!("visibility:{switch}");
        }
    }
    "internal".to_string()
}

/// Build the sorted snapshot of every registered tool (visible + hidden).
fn registry_snapshot(registry: &ToolRegistry) -> Vec<ToolInfoEntry> {
    let hidden = registry.hidden();
    let mut entries: Vec<ToolInfoEntry> = registry
        .all_tools()
        .into_iter()
        .map(|tool| {
            let name = tool.name().to_string();
            let mcp = tool
                .mcp_wrapper_info()
                .map(|(server_id, tool_name)| McpToolRef {
                    server_id: server_id.to_string(),
                    tool_name: tool_name.to_string(),
                });
            ToolInfoEntry {
                hidden: hidden.contains(&name),
                source: tool_source(&tool),
                description: tool.description().to_string(),
                parameters: tool.parameters_schema(),
                permission_category: tool.permission_category().to_string(),
                mcp,
                name,
            }
        })
        .collect();
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    entries
}

#[async_trait::async_trait]
impl Tool for ToolInfoTool {
    fn name(&self) -> &'static str {
        "tool_info"
    }

    fn description(&self) -> &'static str {
        "Return a JSON-encoded dump of the tool registry: every registered \
         tool with its name, description, parameters JSON schema, permission \
         category, source (internal / mcp:<server> / plugin:<id> / \
         visibility:<family>), and hidden state. Read-only; takes no \
         parameters and always succeeds."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "none"
    }

    async fn execute(&self, _input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let entries = registry_snapshot(&ctx.tool_registry);
        let total = entries.len();
        let visible = entries.iter().filter(|e| !e.hidden).count();
        let value = json!({
            "total": total,
            "visible": visible,
            "hidden": total - visible,
            "tools": entries,
        });
        Ok(ToolOutput {
            content: serde_json::to_string_pretty(&value)?,
            metadata: Some(value),
        })
    }
}

#[async_trait::async_trait]
impl Tool for CommandsInfoTool {
    fn name(&self) -> &'static str {
        "commands_info"
    }

    fn description(&self) -> &'static str {
        "Return a JSON-encoded catalog of every slash command, including the \
         built-in TUI commands (trigger, description, subcommands, flags) and \
         the commands contributed by enabled plugins. Read-only; takes no \
         parameters and always succeeds."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "none"
    }

    async fn execute(&self, _input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let existing: std::collections::BTreeSet<String> = COMMAND_CATALOG
            .iter()
            .map(|c| c.trigger.to_string())
            .collect();
        let plugin_commands: Vec<PluginCommandInfo> =
            crate::plugin::plugin_commands(&ctx.working_dir, &existing)
                .into_iter()
                .map(|cmd| PluginCommandInfo {
                    plugin_id: cmd.plugin_id,
                    name: cmd.name,
                    trigger: cmd.trigger,
                    description: cmd.description,
                    is_prompt_command: cmd.prompt.is_some(),
                })
                .collect();
        let report = CommandsInfoReport {
            builtin: COMMAND_CATALOG.to_vec(),
            total: COMMAND_CATALOG.len() + plugin_commands.len(),
            plugin_commands,
        };
        let value = serde_json::to_value(&report)?;
        Ok(ToolOutput {
            content: serde_json::to_string_pretty(&value)?,
            metadata: Some(value),
        })
    }
}
