//! Static catalog of the built-in TUI slash commands.
//!
//! The authoritative slash-command registry is the `SLASH_COMMANDS` table in
//! `crates/ragent-tui/src/app/state.rs`; the subcommand/flag suggestions come
//! from `get_command_suggestions` in `crates/ragent-tui/src/app/slash.rs`.
//! `ragent-agent` cannot depend on `ragent-tui` (the TUI depends on the agent
//! crate, not the other way around), so this catalog is a hand-maintained
//! mirror consumed by the `commands_info` tool. A drift-check test lives in
//! `crates/ragent-tui/tests/test_command_catalog.rs` and fails when the two
//! tables diverge.

/// One entry in the static slash-command catalog.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CommandCatalogEntry {
    /// The trigger word (without the leading `/`).
    pub trigger: &'static str,
    /// Short description, mirroring the TUI autocomplete menu entry.
    pub description: &'static str,
    /// Subcommands suggested by the autocomplete layer (may be empty).
    pub subcommands: &'static [&'static str],
    /// Flags suggested by the autocomplete layer (may be empty).
    pub flags: &'static [&'static str],
}

/// Subcommand suggestions mirrored from `get_command_suggestions`
/// (`crates/ragent-tui/src/app/slash.rs`), which does not split plain
/// subcommands from `--flags`; the split here is purely lexical (a leading
/// `--` or `-h` marks a flag).
const TEAM_SUBS: &[&str] = &[
    "create", "open", "close", "delete", "list", "spawn", "message", "tasks", "cleanup", "help",
];
const MEMORY_SUBS: &[&str] = &["search", "add", "forget", "config", "help"];
const AGENT_SUBS: &[&str] = &["list", "switch", "help"];
const THINKING_LEVELS: &[&str] = &["auto", "off", "low", "medium", "high"];
const CODEINDEX_SUBS: &[&str] = &["on", "off", "sync", "help"];
const GCF_SUBS: &[&str] = &["on", "off", "show", "help"];
const TOOLS_SUBS: &[&str] = &[
    "show",
    "office",
    "github",
    "gitlab",
    "teams",
    "agents",
    "plan",
    "codeindex",
    "masterfetch",
    "browser",
    "help",
];
const ON_OFF_HELP: &[&str] = &["on", "off", "help"];
const WEBSEARCH_SUBS: &[&str] = &["show", "test", "search", "help"];
const HELP_ONLY: &[&str] = &["help"];
const TOOLCHAIN_SUBS: &[&str] = &["list", "help"];
const PROMPT_SUBS: &[&str] = &["help", "primary", "subagent", "list"];
const STATUS_SUBS: &[&str] = &["clear"];
const QUEUE_SUBS: &[&str] = &["list", "clear", "next", "help"];
const HELP_TOPICS: &[&str] = &["team", "memory", "agent", "ui", "accessibility"];
const SPEC_SUBS: &[&str] = &[
    "help",
    "create",
    "govcreate",
    "list",
    "search",
    "validate",
    "status",
    "task",
];
const BLUEPRINTS_SUBS: &[&str] = &["help", "list"];
const ROUTER_SUBS: &[&str] = &[
    "help",
    "on",
    "off",
    "status",
    "tiers",
    "weights",
    "boundaries",
    "test",
    "stats",
    "reload",
];
const CONFIG_SUBS: &[&str] = &["show", "save", "list", "help"];
const TRIGGERS_SUBS: &[&str] = &["list", "enable", "disable", "remove", "status", "help"];
const RESEARCH_SUBS: &[&str] = &[
    "help", "create", "list", "open", "search", "show", "delete", "archive", "update", "cluster",
];
const RESEARCH_FLAGS: &[&str] = &[
    "--mode",
    "--summarization-model",
    "--evaluate",
    "--clarify",
    "--no-clarify",
    "--format",
    "--tier",
    "--depth",
    "--iterations",
    "--fetch-concurrently",
    "--use-local",
    "--use-specs",
    "--use-low-relevance",
    "--use-pdf",
    "--no-papers",
    "--oa-enable",
    "--no-oa",
    "--web-time",
    "--max-concepts",
    "--max-findings",
    "--url-cloak",
];
const INIT_SUBS: &[&str] = &["config", "help"];
const PLUGINS_SUBS: &[&str] = &[
    "list", "add", "remove", "enable", "disable", "test", "stores", "help",
];
const PLUGINS_FLAGS: &[&str] = &["--verbose", "--force"];
const ALOG_SUBS: &[&str] = &[
    "help", "on", "off", "config", "list", "status", "delete", "export",
];
const NEW_FLAGS: &[&str] = &["--language", "--type", "--stack", "--github", "--gitlab"];
// The historical `/spec reverse` flag surface is intentionally absent from the
// catalog: `/reverse` is no longer a top-level command, and a dead constant
// would silently drift from the real flag parsing with nothing failing.
const LOOP_FLAGS: &[&str] = &["--help", "-h"];
const EMPTY: &[&str] = &[];

/// Static catalog of every built-in slash command, descriptions mirrored from
/// `SLASH_COMMANDS` in `crates/ragent-tui/src/app/state.rs`.
pub const COMMAND_CATALOG: &[CommandCatalogEntry] = &[
    CommandCatalogEntry {
        trigger: "about",
        description: "Show application info, version, and authors",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "actionloop",
        description: "Agent action-loop timing: /actionloop [help|clip]",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "agent",
        description: "Switch the active agent: /agent [<name>] | /agent help",
        subcommands: AGENT_SUBS,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "agents",
        description: "List all agents — built-in and custom",
        subcommands: AGENT_SUBS,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "alog",
        description: "Activity log: /alog help|on|off|config|list|status|delete <run-id> --yes|export <run-id> --yes",
        subcommands: ALOG_SUBS,
        flags: HELP_ONLY,
    },
    CommandCatalogEntry {
        trigger: "autopilot",
        description: "Autonomous operation: /autopilot on [--max-tokens N] [--max-time N] | off | status | help",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "bash",
        description: "Manage bash command lists: /bash add|remove allow|deny <entry> [--global] | show | help",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "bench",
        description: "Benchmark runner: /bench list|init <suite-or-all-or-full>|show|run <target>|status|open last|cancel",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "blueprints",
        description: "List installed team blueprints: /blueprints help|list",
        subcommands: BLUEPRINTS_SUBS,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "browse_refresh",
        description: "Refresh the @ file-picker project index",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "bug-report",
        description: "Generate diagnostic bug report with redacted session data (output to log/)",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "cancel",
        description: "Cancel a background task (/cancel <task_id_prefix> | /cancel help)",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "clear",
        description: "Clear message history for the current session",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "clip",
        description: "Copy the rendered message-window contents to the system clipboard",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "codeindex",
        description: "Manage codebase index: /codeindex on|off|show|lang|reindex|rebuild|graph <build|export|lang>|explain <symbol>|path <A> <B>|communities|godnodes|help",
        subcommands: CODEINDEX_SUBS,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "compact",
        description: "Summarise and compact the conversation history",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "config",
        description: "Configuration: /config show|save|list|help",
        subcommands: CONFIG_SUBS,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "context",
        description: "Manage context cache: /context refresh | /context help",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "cost",
        description: "Show session token usage and estimated cost",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "cron",
        description: "Schedule agent runs: /cron add <cronname> <agent> <schedule> \"<prompt>\"|remove|enable|disable|list|log|help",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "dirs",
        description: "Manage directory/file permission lists: /dirs add|remove allow|deny <pattern> [--global] | show | help",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "doctor",
        description: "Run system diagnostics (providers, git, ripgrep, MCP, memory); /doctor help for details",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "editlog",
        description: "Edit-operation logging: /editlog on|off|status|show|analyse|clear",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "exit",
        description: "Exit ragent (alias of /quit)",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "gcf",
        description: "Toggle GCF encoding of tool results: /gcf on|off|show|help",
        subcommands: GCF_SUBS,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "github",
        description: "GitHub integration: /github login | logout | status | help",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "gitlab",
        description: "GitLab integration: /gitlab setup | logout | status | help",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "goal",
        description: "Goal-based autonomous stop: /goal set|clear|show|test",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "help",
        description: "Show available slash commands",
        subcommands: HELP_TOPICS,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "history",
        description: "Browse and re-use previous inputs; /history [filter] restricts to matching entries (↑/↓ to select, Enter to insert, c to copy to clipboard); /history help",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "inbox",
        description: "Triage inbox: /inbox list|claim <id>|dismiss <id>|clear|help",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "init",
        description: "Analyse the project and write a summary, or create a default config: /init [config|help]",
        subcommands: INIT_SUBS,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "inputdiag",
        description: "Dump input/cursor/selection diagnostics for troubleshooting",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "llmstats",
        description: "Show average LLM response time and token throughput",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "log",
        description: "Log panel: /log [clear subagents|panics|research|editlog|help]",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "loop",
        description: "Goal-driven agent loop: /loop opens setup, /loop <agent> <goal> starts, /loop help shows usage",
        subcommands: HELP_ONLY,
        flags: LOOP_FLAGS,
    },
    CommandCatalogEntry {
        trigger: "mcp",
        description: "MCP servers: /mcp [status] | /mcp discover | /mcp connect <id> | /mcp disconnect <id> | /mcp help",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "memory",
        description: "Memory panel (Alt+M): /memory | /memory show | /memory init | /memory read <label> | /memory search <query>",
        subcommands: MEMORY_SUBS,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "mode",
        description: "Set agent role mode: /mode architect|coder|reviewer|debugger|tester|off|help",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "model",
        description: "Switch the active model, or show metadata with /model show (/model help for usage)",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "mouse",
        description: "Toggle mouse support: /mouse on | off | help",
        subcommands: ON_OFF_HELP,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "name",
        description: "Set a human-readable display name for the session: /name <display-name> | /name help",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "new",
        description: "Scaffold a new project: /new --language <lang> --type <type> [--stack <name>] [--github | --gitlab] | /new help",
        subcommands: HELP_ONLY,
        flags: NEW_FLAGS,
    },
    CommandCatalogEntry {
        trigger: "perf",
        description: "Alias for /profile — toggle the agent-loop perf panel (/perf on|off|help)",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "plan",
        description: "Delegate planning to the plan agent: /plan <task description> | /plan help",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "plugins",
        description: "Plugin management: /plugins list [--verbose] | add <source> [--force] | remove <pluginid> | enable <pluginid> | disable <pluginid> | test <pluginid> | stores | help",
        subcommands: PLUGINS_SUBS,
        flags: PLUGINS_FLAGS,
    },
    CommandCatalogEntry {
        trigger: "profile",
        description: "Toggle the agent-loop profiler panel (/profile on|off|help)",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "prompt",
        description: "Agent system-prompt inspector: /prompt help|primary [agent]|subagent [agent]|list|<agent>",
        subcommands: PROMPT_SUBS,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "provider",
        description: "Change provider, show config, or configure model router: /provider [show|router|help]",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "provider_reset",
        description: "Reset the current provider and remove stored credentials",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "queue",
        description: "Inspect the message input queue: /queue [list|clear|next|help]",
        subcommands: QUEUE_SUBS,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "quit",
        description: "Exit ragent",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "reload",
        description: "Reload customizations (/reload [all|config|mcp|skills|agents])",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "research",
        description: "Research system: /research create [--mode tiered|supervisor|competitive] [--summarization-model <model>] [--evaluate] [other flags] <name> <topic...> | list | open | search | show | delete | archive | cluster",
        subcommands: RESEARCH_SUBS,
        flags: RESEARCH_FLAGS,
    },
    CommandCatalogEntry {
        trigger: "resume",
        description: "Resume the agent from where it was halted (/resume help)",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "router",
        description: "Model router management: /router on|off|status|tiers|weights|boundaries|test|stats|reload|help",
        subcommands: ROUTER_SUBS,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "skills",
        description: "List all registered skills and their descriptions (/skills help)",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "spawn",
        description: "Spawn a detached background sub-agent that nothing waits on: /spawn <agent> <prompt...> | /spawn help",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "spec",
        description: "Specification management: /spec create|add|delete|list|search|validate|status|task|govcreate <specid> <content-ref> <target-folder> [flags]|help",
        subcommands: SPEC_SUBS,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "startup",
        description: "Show startup timing breakdown for the current session",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "status",
        description: "Show status message history: /status [clear]",
        subcommands: STATUS_SUBS,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "swarm",
        description: "Auto-decompose a goal into parallel subtasks (/swarm <prompt> | /swarm status | /swarm help)",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "system",
        description: "Override the agent system prompt (/system <prompt> | /system help)",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "task",
        description: "Toggle the TASKS side panel, or list/help tasks: /task [list|help]",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "team",
        description: "Team management (/team help|status|show [name]|create/open/delete <name>|close|message <id> <text>|tasks|clear|cleanup)",
        subcommands: TEAM_SUBS,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "teams",
        description: "Alias of /team (supports /teams show <name>)",
        subcommands: TEAM_SUBS,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "telemetry",
        description: "Telemetry management: /telemetry help|on|off|setup|counters",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "telemetry_panel",
        description: "Toggle the telemetry side panel (Alt+O alias); use `/telemetry counters` to list values",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "template",
        description: "List and apply reusable prompt templates: /template [name] [args]",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "thinking",
        description: "Switch the current thinking level: /thinking auto|off|low|medium|high",
        subcommands: THINKING_LEVELS,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "toolchain",
        description: "Language toolchain report: /toolchain list [lang] [--json] | /toolchain help",
        subcommands: TOOLCHAIN_SUBS,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "tools",
        description: "Toggle tool visibility: /tools [office|github|gitlab|teams|agents|plan|codeindex|masterfetch|browser] [on|off] | /tools help",
        subcommands: TOOLS_SUBS,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "triggers",
        description: "Manage trigger rules: /triggers [list|enable|disable|remove|status|help]",
        subcommands: TRIGGERS_SUBS,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "undo",
        description: "Remove the last user/assistant turn pair from the conversation (/undo help)",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "update",
        description: "Check for or install updates: /update | /update install | /update help",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "webapi",
        description: "Manage the HTTP REST API: /webapi enable | disable | help",
        subcommands: EMPTY,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "websearch",
        description: "Web search engine diagnostics: /websearch show | test | help",
        subcommands: WEBSEARCH_SUBS,
        flags: EMPTY,
    },
    CommandCatalogEntry {
        trigger: "yolo",
        description: "Toggle YOLO mode — bypass all command validation and tool restrictions (/yolo help)",
        subcommands: EMPTY,
        flags: EMPTY,
    },
];
