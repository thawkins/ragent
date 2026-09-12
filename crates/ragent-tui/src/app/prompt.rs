//! Report data structures and rendering helpers for the `/prompt` slash
//! command.
//!
//! `/prompt` is a read-only inspector: it re-runs the canonical system-prompt
//! assembler (`ragent_agent::agent::build_system_prompt_with_storage_and_memory_and_config`)
//! for the currently selected agent preset, applies the same effective
//! tool-surface filter chain the session loop applies, and renders the result
//! as a markdown report. No LLM call, no writes, no session mutation.
//!
//! This module holds the pure data types (headers, roster rows) and the
//! renderers; the `impl App` glue that resolves live context inputs lives in
//! `slash.rs` (see `handle_prompt_command`, T-013/T-014).
//!
//! Dependencies: `ragent_agent::agent` (assembler, `AgentInfo`, `AgentMode`),
//! `ragent_agent::session::prompt_builders` (tool-reference renderers).

use std::sync::Arc;

use ragent_agent::agent::{AgentInfo, AgentMode};
use ragent_agent::llm::ToolDefinition;

/// Maximum rendered report length before truncation (FR-013).
pub const PROMPT_REPORT_MAX_CHARS: usize = 100_000;

/// Render the `/prompt help` page (FR-003), also shown for a bare `/prompt`.
///
/// Lists every subcommand with its arguments and a one-line description so
/// the command surface is discoverable without reading documentation.
pub fn render_help() -> String {
    "## /prompt — Agent System Prompt Inspector\n\n\
     Shows exactly what the LLM receives as its system prompt for an agent:\n\
     the assembled prompt body plus the effective `## Available Tools`\n\
     reference. **Read-only** — no LLM call, no writes, no session mutation.\n\n\
     | Command | Description |\n\
     |---|---|\n\
     | `/prompt help` | Show this help page |\n\
     | `/prompt primary [agent]` | Assembled primary-mode system prompt + compact tool reference |\n\
     | `/prompt subagent [agent]` | Subagent-mode prompt + detailed tool reference (interactive tools excluded) |\n\
     | `/prompt list` | Agent roster with mode and source badges |\n\
     | `/prompt <agent-name>` | Alias for `/prompt primary <agent-name>` |\n\n\
     The `[agent]` argument is optional and matched case-insensitively against\n\
     the built-in roster plus custom agents (`/prompt list` shows the names).\n\
     Tool-free agents (custom `max_steps: 1` single-shot definitions) render\n\
     `(no tools)` in the header with no tool-reference section.\n"
        .to_string()
}

/// Header summary line for a rendered prompt report (FR-004 / FR-005).
///
/// Rendered as the first line of every prompt body so the agent, its prompt
/// source, the forced mode, the effective tool count, and the rendered size
/// are visible without scrolling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptHeader {
    /// Resolved agent name (e.g. `coder`).
    pub agent_name: String,
    /// Prompt source: `built-in` (built-in roster) or `custom` (OASF load).
    pub source: PromptSource,
    /// Mode the assembly was rendered for: `primary` or `subagent`.
    pub mode: AgentMode,
    /// Number of tools in the effective tool surface; `None` when the agent
    /// is tool-free (rendered as `(no tools)` per FR-009).
    pub tool_count: Option<usize>,
    /// Length of the assembled prompt body in characters, before any
    /// truncation applied to the final report.
    pub body_chars: usize,
}

impl PromptHeader {
    /// Render the header summary as a single markdown line.
    pub fn render(&self) -> String {
        let source = match self.source {
            PromptSource::Builtin => "built-in",
            PromptSource::Custom => "custom",
        };
        let mode = match self.mode {
            AgentMode::Primary | AgentMode::All => "primary",
            AgentMode::Subagent => "subagent",
        };
        let tools = match self.tool_count {
            Some(n) => format!("tools: {n}"),
            None => "(no tools)".to_string(),
        };
        format!(
            "**Prompt report** - agent: `{}` - source: {} - mode: {} - {} - size: {} chars\n",
            self.agent_name, source, mode, tools, self.body_chars
        )
    }
}

/// Where an agent definition came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromptSource {
    /// Agent from the built-in roster (`agent::builtin_agents()`).
    Builtin,
    /// Agent loaded from the custom OASF discovery directories.
    Custom,
}

/// One roster row for `/prompt list` (FR-007).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RosterEntry {
    /// Agent name as accepted by `/prompt <agent-name>`.
    pub name: String,
    /// Mode badge (`primary`/`subagent`/`all`).
    pub mode: AgentMode,
    /// Source badge.
    pub source: PromptSource,
    /// Truncated one-line description.
    pub description: String,
}

/// Render the `/prompt list` roster as markdown (FR-007): one line per
/// available agent (non-hidden built-ins plus customs) with its mode badge
/// (`primary`/`subagent`/`all`), source badge (`built-in`/`custom`), and a
/// truncated one-line description. Hidden built-ins are excluded because
/// FR-006 resolution excludes them too.
pub fn render_roster(entries: &[RosterEntry]) -> String {
    let mut out = String::from("## Agent roster\n\n");
    for e in entries {
        let source = match e.source {
            PromptSource::Builtin => "built-in",
            PromptSource::Custom => "custom",
        };
        out.push_str(&format!(
            "- `{}` - [{}] [{}] - {}\n",
            e.name, e.mode, source, e.description
        ));
    }
    out
}

/// Build [`RosterEntry`] rows from the built-in roster plus custom agents.
///
/// Hidden built-ins are excluded (FR-007 / FR-006 consistency: names listed
/// here are exactly the names FR-006 resolution accepts).
pub fn roster_entries(
    builtins: &[Arc<AgentInfo>],
    customs: &[ragent_agent::agent::CustomAgentDef],
) -> Vec<RosterEntry> {
    let mut entries: Vec<RosterEntry> = builtins
        .iter()
        .filter(|a| !a.hidden)
        .map(|a| RosterEntry {
            name: a.name.clone(),
            mode: a.mode.clone(),
            source: PromptSource::Builtin,
            description: truncate_one_line(&a.description, 80),
        })
        .collect();
    for c in customs {
        entries.push(RosterEntry {
            name: c.agent_info.name.clone(),
            mode: c.agent_info.mode.clone(),
            source: PromptSource::Custom,
            description: truncate_one_line(&c.agent_info.description, 80),
        });
    }
    entries
}

/// Truncate a description to `max` characters on a single line.
fn truncate_one_line(s: &str, max: usize) -> String {
    let flat: String = s
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect();
    let flat = flat.trim();
    if flat.chars().count() <= max {
        return flat.to_string();
    }
    let cut: String = flat.chars().take(max).collect();
    format!("{cut}...")
}

/// Apply the FR-013 output size cap, appending an explicit truncation marker.
pub fn apply_size_cap(report: &str) -> String {
    if report.chars().count() <= PROMPT_REPORT_MAX_CHARS {
        return report.to_string();
    }
    let shown: String = report.chars().take(PROMPT_REPORT_MAX_CHARS).collect();
    format!(
        "{shown}\n\n... [truncated: showing {} of {} characters - use a lighter agent for the full text]\n",
        PROMPT_REPORT_MAX_CHARS,
        report.chars().count()
    )
}
/// Result of resolving an agent name for `/prompt` (FR-006 / FR-011).
pub enum AgentResolution {
    /// Agent found.
    Found(Arc<AgentInfo>),
    /// No agent matched the requested name; the warning lists the available names.
    Miss {
        /// The unmatched agent-name argument.
        requested: String,
        /// Sorted, deduplicated list of resolvable agent names.
        available: Vec<String>,
    },
}

/// Resolve an agent name against the built-in roster and the loaded custom
/// agents, case-insensitively (FR-006).
///
/// Resolution order: exact-case built-in match first, then case-insensitive
/// non-hidden built-in match, then case-insensitive custom match. Hidden
/// built-ins are excluded from resolution (they are not user-selectable).
pub fn resolve_agent(
    name: &str,
    builtins: &[Arc<AgentInfo>],
    customs: &[ragent_agent::agent::CustomAgentDef],
) -> AgentResolution {
    if let Some(found) = builtins.iter().find(|a| a.name == name) {
        return AgentResolution::Found(found.clone());
    }
    if let Some(found) = builtins
        .iter()
        .filter(|a| !a.hidden)
        .find(|a| a.name.eq_ignore_ascii_case(name))
    {
        return AgentResolution::Found(found.clone());
    }
    if let Some(found) = customs
        .iter()
        .find(|c| c.agent_info.name.eq_ignore_ascii_case(name))
    {
        return AgentResolution::Found(found.agent_info.clone());
    }
    AgentResolution::Miss {
        requested: name.to_string(),
        available: available_agent_names(builtins, customs),
    }
}

/// Sorted, deduplicated list of agent names accepted by FR-006: non-hidden
/// built-ins plus all custom agents.
pub fn available_agent_names(
    builtins: &[Arc<AgentInfo>],
    customs: &[ragent_agent::agent::CustomAgentDef],
) -> Vec<String> {
    let mut names: Vec<String> = builtins
        .iter()
        .filter(|a| !a.hidden)
        .map(|a| a.name.clone())
        .collect();
    names.extend(customs.iter().map(|c| c.agent_info.name.clone()));
    names.sort();
    names.dedup();
    names
}

/// Render the FR-011 miss warning: names the unmatched argument and lists the
/// resolvable agent names. Returns `None` for a successful resolution — no
/// prompt content is ever included in the warning.
pub fn render_miss_warning(res: &AgentResolution) -> Option<String> {
    let AgentResolution::Miss {
        requested,
        available,
    } = res
    else {
        return None;
    };
    let mut out = format!(
        "From: /prompt\n\n**Unknown agent `{requested}`** — no built-in or custom agent matches that name.\n\nAvailable agents:\n"
    );
    for n in available {
        out.push_str(&format!("- `{n}`\n"));
    }
    Some(out)
}

/// Render the FR-010 usage correction: the first argument matched no known
/// subcommand, help alias, or resolvable agent name. Names the unmatched
/// token, lists the valid subcommands, and lists the available agent names.
/// No prompt content is ever included.
pub fn render_usage_correction(token: &str, available: &[String]) -> String {
    let mut out = format!(
        "From: /prompt\n\n**Unknown subcommand or agent `{token}`** - not a known subcommand, help alias, or agent name.\n\nValid subcommands:\n"
    );
    out.push_str("- `/prompt help` - show the help page\n");
    out.push_str("- `/prompt primary [agent]` - primary-mode prompt report\n");
    out.push_str("- `/prompt subagent [agent]` - subagent-mode prompt report\n");
    out.push_str("- `/prompt list` - agent roster\n");
    out.push_str("- `/prompt <agent-name>` - alias for `/prompt primary <agent-name>`\n");
    out.push_str("\nAvailable agents:\n");
    for n in available {
        out.push_str(&format!("- `{n}`\n"));
    }
    out
}

/// Whether the resolved agent is tool-free (FR-009).
///
/// Mirrors the assembler's own tool gate exactly
/// (`ragent-agent/src/agent/mod.rs:2533-2536`): an agent is tool-free when
/// `max_steps` is `Some(1)` or `Some(0)` — the assembler takes its early
/// return with no tool sections, and the session processor sends zero tools
/// on the wire (`processor.rs:1719`, `max_steps <= 1`). `None` means the
/// default unlimited budget (tools), not a tool-free agent.
pub fn is_tool_free_agent(agent: &AgentInfo) -> bool {
    agent.max_steps.is_some_and(|steps| steps <= 1)
}

/// Compose the effective tool surface for `/prompt` (FR-008), mirroring the
/// `loop_steps.rs` filter order:
///
/// 1. registry definitions (already exclude `tool_visibility`-hidden tools —
///    the TUI applies `config.effective_hidden_tools()` through
///    `ToolRegistry::set_hidden` before rendering, the same registry the
///    session loop reads),
/// 2. agent `allowed_tools` allowlist (plus the always-allowed control
///    surface `build_allowed_tool_set` folds in) when non-empty,
/// 3. subagent mode only: interactive tools removed.
///
/// Returns the filtered `ToolDefinition` list; an empty list means the
/// tool-free path (FR-009 `(no tools)` header marker). A tool-free agent
/// (`is_tool_free_agent`) is always reported as empty, regardless of what the
/// registry holds — matching the wire surface the session processor builds.
pub fn effective_tool_defs(
    registry: &ragent_agent::tool::ToolRegistry,
    agent: &AgentInfo,
    subagent_mode: bool,
) -> Vec<ToolDefinition> {
    if is_tool_free_agent(agent) {
        return Vec::new();
    }
    let allowed_set = ragent_agent::tool::build_allowed_tool_set(agent.allowed_tools.as_deref());
    let mut defs: Vec<ToolDefinition> = registry
        .definitions()
        .iter()
        .filter(|d| {
            allowed_set.is_empty() || ragent_agent::tool::is_allowed_tool(&d.name, &allowed_set)
        })
        .cloned()
        .collect();
    if subagent_mode {
        defs.retain(|d| !ragent_agent::session::permissions::is_interactive_tool(&d.name));
    }
    defs
}
