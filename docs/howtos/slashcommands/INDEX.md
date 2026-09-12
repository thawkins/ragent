# Slash Command Howtos

Per-command reference documents for the ragent TUI slash commands. Each document describes the command, every subcommand and option the dispatcher implements, and worked examples. Typing `/` in the TUI opens the autocomplete menu, and `/<command> help` prints per-command help where the handler supports it.

## Commands

| Command | Document | Summary |
|---|---|---|
| `/about` | [about](about.md) | Show application info, version, and authors |
| `/agent` | [agent](agent.md) | Switch the active agent: /agent [<name>] \| /agent help |
| `/agents` | [agents](agents.md) | List all agents  -  built-in and custom |
| `/browse_refresh` | [browse_refresh](browse_refresh.md) | Refresh the @ file-picker project index |
| `/bench` | [bench](bench.md) | Benchmark runner: /bench list\|init <suite-or-all-or-full>\|show\|run <target>\|status\|open last\|cancel |
| `/clear` | [clear](clear.md) | Clear message history for the current session |
| `/clip` | [clip](clip.md) | Copy the rendered message-window contents to the system clipboard |
| `/cancel` | [cancel](cancel.md) | Cancel a background task (/cancel <task_id_prefix> \| /cancel help) |
| `/config` | [config](config.md) | Configuration: /config show\|save\|list\|help |
| `/context` | [context](context.md) | Manage context cache: /context refresh \| /context help |
| `/cron` | [cron](cron.md) | Schedule agent runs: /cron add <cronname> <agent> <schedule> "<prompt>"\|remove\|enable\|disable\|list\|log\|help |
| `/compact` | [compact](compact.md) | Summarise and compact the conversation history |
| `/inbox` | [inbox](inbox.md) | Triage inbox: /inbox list\|claim <id>\|dismiss <id>\|clear\|help |
| `/cost` | [cost](cost.md) | Show session token usage and estimated cost |
| `/help` | [help](help.md) | Show available slash commands |
| `/history` | [history](history.md) | Browse and re-use previous inputs; /history [filter] restricts to matching entries (arrow keys to select, Enter to insert); /history help |
| `/inputdiag` | [inputdiag](inputdiag.md) | Dump input/cursor/selection diagnostics for troubleshooting |
| `/log` | [log](log.md) | Log panel: /log [clear subagents\|panics\|research\|editlog\|help] |
| `/loop` | [loop](loop.md) | Goal-driven agent loop: /loop opens setup, /loop <agent> <goal> starts, /loop help shows usage |
| `/profile` | [profile](profile.md) | Toggle the agent-loop profiler panel (/profile on\|off\|help) |
| `/perf` | [perf](perf.md) | Alias for /profile  -  toggle the agent-loop perf panel (/perf on\|off\|help) |
| `/llmstats` | [llmstats](llmstats.md) | Show average LLM response time and token throughput |
| `/model` | [model](model.md) | Switch the active model, or show metadata with /model show (/model help for usage) |
| `/thinking` | [thinking](thinking.md) | Switch the current thinking level: /thinking auto\|off\|low\|medium\|high |
| `/provider` | [provider](provider.md) | Change provider, show config, or configure model router: /provider [show\|router\|help] |
| `/provider_reset` | [provider_reset](provider_reset.md) | Reset the current provider and remove stored credentials |
| `/quit` | [quit](quit.md) | Exit ragent |
| `/reload` | [reload](reload.md) | Reload customizations (/reload [all\|config\|mcp\|skills\|agents]) |
| `/resume` | [resume](resume.md) | Resume the agent from where it was halted (/resume help) |
| `/system` | [system](system.md) | Override the agent system prompt (/system <prompt> \| /system help) |
| `/template` | [template](template.md) | List and apply reusable prompt templates: /template [name] [args] |
| `/goal` | [goal](goal.md) | Goal-based autonomous stop: /goal set\|clear\|show\|test |
| `/tools` | [tools](tools.md) | List all available tools (built-in and MCP) |
| `/skills` | [skills](skills.md) | List all registered skills and their descriptions (/skills help) |
| `/mcp` | [mcp](mcp.md) | MCP servers: /mcp [status] \| /mcp discover \| /mcp connect <id> \| /mcp disconnect <id> \| /mcp help |
| `/task` | [task](task.md) | Toggle the TASKS side panel, or list/help tasks: /task [list\|help] |
| `/team` | [team](team.md) | Team management (/team help\|status\|show [name]\|create/open/delete <name>\|close\|message <id> <text>\|tasks\|clear\|cleanup) |
| `/swarm` | [swarm](swarm.md) | Auto-decompose a goal into parallel subtasks (/swarm <prompt> \| /swarm status \| /swarm help) |
| `/bash` | [bash](bash.md) | Manage bash command lists: /bash add\|remove allow\|deny <entry> [--global] \| show \| help |
| `/dirs` | [dirs](dirs.md) | Manage directory/file permission lists: /dirs add\|remove allow\|deny <pattern> [--global] \| show \| help |
| `/yolo` | [yolo](yolo.md) | Toggle YOLO mode  -  bypass all command validation and tool restrictions (/yolo help) |
| `/spec` | [spec](spec.md) | Specification management: /spec create\|add\|delete\|list\|search\|validate\|status\|task\|help |
| `/research` | [research](research.md) | Research system: /research create [--mode tiered\|supervisor\|competitive] [--summarization-model <model>] [--evaluate] [other flags] <name> <topic...> \| list \| open \| search \| show \| delete \| archive \| cluster |
| `/reverse` | [reverse](reverse.md) | Reverse-engineer a GitHub repo: /reverse <owner/repo \| URL> [--tech <stack>] [--create <name>] |
| `/new` | [new](new.md) | Scaffold a new project: /new --language <lang> --type <type> [--stack <name>] [--github \| --gitlab] \| /new help |
| `/autopilot` | [autopilot](autopilot.md) | Autonomous operation: /autopilot on [--max-tokens N] [--max-time N] \| off \| status \| help |
| `/plan` | [plan](plan.md) | Delegate planning to the plan agent: /plan <task description> \| /plan help |
| `/mode` | [mode](mode.md) | Set agent role mode: /mode architect\|coder\|reviewer\|debugger\|tester\|off\|help |
| `/memory` | [memory](memory.md) | Memory panel (Alt+M): /memory \| /memory show \| /memory init \| /memory read <label> \| /memory search <query> |
| `/github` | [github](github.md) | GitHub integration: /github login \| logout \| status \| help |
| `/gitlab` | [gitlab](gitlab.md) | GitLab integration: /gitlab setup \| logout \| status \| help |
| `/update` | [update](update.md) | Check for or install updates: /update \| /update install \| /update help |
| `/doctor` | [doctor](doctor.md) | Run system diagnostics (providers, git, ripgrep, MCP, memory); /doctor help for details |
| `/webapi` | [webapi](webapi.md) | Manage the HTTP REST API: /webapi enable \| disable \| help |
| `/websearch` | [websearch](websearch.md) | Web search engine diagnostics: /websearch show \| test \| help |
| `/init` | [init](init.md) | Analyse the project and write a summary, or create a default config: /init [config\|help] |
| `/codeindex` | [codeindex](codeindex.md) | Manage codebase index: /codeindex on\|off\|show\|lang\|reindex\|rebuild\|graph <build\|export\|lang>\|explain <symbol>\|path <A> <B>\|communities\|godnodes\|help |
| `/status` | [status](status.md) | Show status message history: /status [clear] |
| `/mouse` | [mouse](mouse.md) | Toggle mouse support: /mouse on \| off \| help |
| `/telemetry` | [telemetry](telemetry.md) | Telemetry management: /telemetry help\|on\|off\|setup\|counters |
| `/telemetry_panel` | [telemetry_panel](telemetry_panel.md) | Toggle the telemetry side panel (Alt+O alias); use `/telemetry counters` to list values |
| `/router` | [router](router.md) | Model router management: /router on\|off\|status\|tiers\|weights\|boundaries\|test\|stats\|reload\|help |
| `/startup` | [startup](startup.md) | Show startup timing breakdown for the current session |
| `/editlog` | [editlog](editlog.md) | Edit-operation logging: /editlog on\|off\|status\|show\|analyse\|clear |
| `/actionloop` | [actionloop](actionloop.md) | Agent action-loop timing: /actionloop [help\|clip] |
| `/bug-report` | [bug-report](bug-report.md) | Generate diagnostic bug report with redacted session data (output to log/) |
| `/triggers` | [triggers](triggers.md) | Manage trigger rules: /triggers [list\|enable\|disable\|remove\|status\|help] |
| `/undo` | [undo](undo.md) | Remove the last user/assistant turn pair from the conversation (/undo help) |
| `/name` | [name](name.md) | Set a human-readable display name for the session: /name <display-name> \| /name help |
| `/alog` | [alog](alog.md) | Activity log: /alog help\|on\|off\|config\|list\|status\|delete <run-id> --yes\|export <run-id> --yes |
| `/toolchain` | [toolchain](toolchain.md) | Language toolchain report: /toolchain list [lang] [--json] \| /toolchain help |
| `/prompt` | [prompt](prompt.md) | Agent system-prompt inspector: /prompt help\|primary [agent]\|subagent [agent]\|list\|<agent> |
| `/blueprints` | [blueprints](blueprints.md) | List installed team blueprints: /blueprints help\|list |

## Aliases

| Alias | Canonical |
|---|---|
| `/exit` | `/quit` (single dispatch arm `"quit" | "exit"`) |
| `/teams` | `/team` (single dispatch arm `"team" | "teams"`) |
| `/compress` | `/compact` (deprecated alias; logs a deprecation notice) |

