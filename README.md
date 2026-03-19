# ragent

An AI coding agent for the terminal, built in Rust.

ragent is a Rust coding agent inspired by [OpenCode](https://github.com/anomalyco/opencode) —
the open-source AI coding agent. It provides multi-provider LLM orchestration, a
built-in tool system, a terminal UI, and a client/server architecture, all compiled
into a single statically-linked binary with no runtime dependencies.

It is reimplemented in Rust as a learninh exercise for the author.

## Features

- **Multi-provider LLM support** — Anthropic, OpenAI, GitHub Copilot, and Ollama
  out of the box, with an extensible provider trait for adding more
- **8 built-in tools** — file read/write/create/edit, bash execution, grep, glob, directory
  listing, and interactive questions
- **15 extended tools** — multiedit, patch, webfetch, websearch, plan delegation,
  todo management, office document read/write/info, PDF read/write, and file deletion (rm)
- **3 sub-agent tools** — new_task, cancel_task, list_tasks for spawning and managing
  background agents
- **Terminal UI** — full-screen ratatui interface with home screen, provider setup
  dialog, slash-command autocomplete, agent cycling, streaming chat, step-numbered
  tool calls with pretty-printed JSON in the log panel
- **HTTP server** — axum-based REST + SSE API so any frontend can drive the agent
- **Session management** — persistent conversation history stored in SQLite
- **Permission system** — configurable rules that gate file writes, shell commands,
  and external access before they execute
- **Agent presets** — coder, task, architect, ask, debug, code-review, and custom
  agents with tailored system prompts
- **Project guidelines** — auto-loads `AGENTS.md` from the project root into the
  system prompt so agents follow project-specific conventions
- **MCP client** — Model Context Protocol support for extending tool capabilities
  via external servers (stub, in progress)
- **Snapshot & undo** — file snapshots before edits so changes can be rolled back
- **Event bus** — internal pub/sub for real-time UI updates
- **Background agents** — spawn and run multiple sub-agents concurrently for parallel
  task execution, with REST API and TUI monitoring

## Installation

### From source

```bash
git clone https://github.com/thawkins/ragent.git
cd ragent
cargo build --release
# Binary is at target/release/ragent
```

Requires Rust 1.85+ (edition 2024).

## Quick Start

```bash
# Configure an API key
export ANTHROPIC_API_KEY="sk-..."
# or
export OPENAI_API_KEY="sk-..."

# Launch the interactive TUI
ragent

# Run a one-shot prompt
ragent run "Explain this codebase"

# Start the HTTP server only
ragent serve --port 9100
```

## Usage

```
ragent [OPTIONS] [COMMAND]

Commands:
  run      Execute agent with a prompt
  serve    Start HTTP server only
  session  Manage sessions (list, resume, import, export)
  auth     Configure provider authentication
  models   List available models
  config   Show resolved configuration

Options:
      --model <MODEL>          Override model (provider/model format)
      --agent <AGENT>          Override agent [default: build]
      --log-level <LOG_LEVEL>  Log level [default: warn]
      --no-tui                 Disable TUI, use plain stdout
      --yes                    Auto-approve all permissions
      --config <CONFIG>        Path to config file
```

## Configuration

ragent reads configuration from `ragent.json` (or `ragent.jsonc`) in the current
directory, with fallback to `~/.config/ragent/config.json`. The format is compatible
with OpenCode's `opencode.json`.

```jsonc
{
  "provider": {
    "anthropic": {
      "apiKey": "sk-...",
      "models": {
        "claude-sonnet-4-20250514": { "max_tokens": 8192 }
      }
    }
  },
  "defaultAgent": "coder",
  "permissions": [
    { "permission": "file:write", "pattern": "src/**", "action": "allow" }
  ]
}
```

## Custom Agents

You can define your own agents as JSON files using the
[Open Agentic Schema Framework (OASF)](https://oasf.agntcy.org/) standard.
Place them in:

- `~/.ragent/agents/` — user-global (all projects)
- `.ragent/agents/` — project-local (this project, higher priority)

ragent loads them automatically at startup. Use `/agents` to list loaded agents
and view diagnostics, or `/agent` to open the interactive picker (custom agents
are marked with a yellow `[custom]` badge).

See [docs/custom-agents.md](docs/custom-agents.md) for the full schema
reference, template variables (`{{WORKING_DIR}}`, `{{FILE_TREE}}`, `{{AGENTS_MD}}`,
`{{DATE}}`), permission rules, and worked examples. Ready-to-use example files
are in [`examples/agents/`](examples/agents/).

## Teams

Teams let one lead session coordinate multiple teammates with shared tasks and mailbox messaging.

Quick flow:

- Create a team: `/team create <name>` (or `team_create`)
- Re-open an existing team: `/team open <name>`
- Spawn teammates: `team_spawn`
- Add/list/claim/complete tasks: `team_task_create`, `team_task_list`, `team_task_claim`, `team_task_complete`
- Communicate: `/team message ...` or `team_message`, plus `team_read_messages`
- Reset/close/delete team state: `/team clear`, `/team close`, `/team delete <name>`
- Cleanup when finished: `/team cleanup` or `team_cleanup`

Docs and examples:

- Guide: [`docs/teams.md`](docs/teams.md)
- How-to manual: [`docs/howto_teams.md`](docs/howto_teams.md)
- Example bundles: [`examples/teams/`](examples/teams/)

## Architecture

The project is a Cargo workspace with three crates:

| Crate | Purpose |
|-------|---------|
| `ragent-core` | Types, storage, config, providers, tools, agents, sessions, event bus |
| `ragent-server` | Axum HTTP routes, SSE streaming |
| `ragent-tui` | Ratatui terminal interface |

The binary entry point (`src/main.rs`) wires these together behind a clap CLI.

```
User Input
    │
    ▼
┌──────────┐    ┌──────────────┐    ┌──────────────┐
│   TUI    │◄──►│  Event Bus   │◄──►│ HTTP Server  │
└────┬─────┘    └──────┬───────┘    └──────┬───────┘
     │                 │                   │
     ▼                 ▼                   ▼
┌─────────────────────────────────────────────┐
│              Session Processor              │
│  (agent loop → LLM call → tool execution)  │
└──────────────────┬──────────────────────────┘
                   │
         ┌─────────┼─────────┐
         ▼         ▼         ▼
    ┌─────────┐ ┌──────┐ ┌────────┐
    │Provider │ │Tools │ │Storage │
    │(LLM API)│ │      │ │(SQLite)│
    └─────────┘ └──────┘ └────────┘
```

## Project Status

**v0.1.0-alpha.19** — Early development with active Teams and Copilot UX improvements.
Recent updates include expanded Teams lifecycle commands (`/team open|close|delete|clear`),
tabular `/team tasks`, automatic pre-send context compaction, Copilot reasoning-level
selection, and improved model metadata/compatibility handling.

## License

MIT
