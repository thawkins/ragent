# ragent

An AI coding agent for the terminal, built in Rust.

ragent is a Rust reimplementation of [OpenCode](https://github.com/anomalyco/opencode) —
the open-source AI coding agent. It provides multi-provider LLM orchestration, a
built-in tool system, a terminal UI, and a client/server architecture, all compiled
into a single statically-linked binary with no runtime dependencies.

## Features

- **Multi-provider LLM support** — Anthropic and OpenAI out of the box, with an
  extensible provider trait for adding more
- **8 built-in tools** — file read/write/edit, bash execution, grep, glob, directory
  listing, and interactive questions
- **Terminal UI** — full-screen ratatui interface with message history, streaming
  output, and a permission approval dialog
- **HTTP server** — axum-based REST + SSE API so any frontend can drive the agent
- **Session management** — persistent conversation history stored in SQLite
- **Permission system** — configurable rules that gate file writes, shell commands,
  and external access before they execute
- **Agent presets** — coder, task, architect, ask, debug, code-review, and custom
  agents with tailored system prompts
- **MCP client** — Model Context Protocol support for extending tool capabilities
  via external servers (stub, in progress)
- **Snapshot & undo** — file snapshots before edits so changes can be rolled back
- **Event bus** — internal pub/sub for real-time UI updates

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

**v0.1.0-alpha.0** — Early development. Core architecture is in place and the binary
compiles, but many features are stubs or incomplete. See [TODO.md](TODO.md) for
unimplemented functions and [CODE_CLEANUP.md](CODE_CLEANUP.md) for the quality
roadmap.

## License

MIT
