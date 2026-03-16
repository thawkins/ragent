# Ragent Quick Start Guide

Get up and running with ragent in minutes. This guide covers installation,
configuration, and common workflows.

---

## Prerequisites

- **Rust 1.85+** (edition 2024) — install via [rustup](https://rustup.rs)
- An LLM provider: **Anthropic**, **OpenAI**, or a local **Ollama** server

## Installation

```bash
# Clone and build
git clone https://github.com/thawkins/ragent.git
cd ragent
cargo build --release

# The binary is at target/release/ragent
# Optionally copy it to your PATH:
cp target/release/ragent ~/.local/bin/
```

---

## 1. Configure a Provider

Ragent needs at least one LLM provider. The easiest way is via the **interactive TUI**:

```bash
ragent        # launch ragent
               # press 'p' on the home screen to open the provider setup dialog
```

The dialog walks you through:
1. **Selecting a provider** (Anthropic, OpenAI, GitHub Copilot, or Ollama)
2. **Entering your API key** (if required — Copilot auto-discovers, Ollama needs none)
3. **Choosing a model** from the provider's available models

The key is stored persistently in `~/.local/share/ragent/ragent.db` so you only need to
configure once.

A **health indicator** appears next to the provider on both the home and chat screens:
- **●** (green) — provider is reachable
- **✗** (red) — provider is unreachable (e.g. Ollama server not running)
- **●** (yellow) — health check in progress

You can also configure providers via **environment variables** or the **CLI**:

### Option A: Anthropic (Claude)

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
# or store persistently:
ragent auth anthropic sk-ant-your-key-here
```

### Option B: OpenAI (GPT-4o)

```bash
export OPENAI_API_KEY="sk-..."
# or store persistently:
ragent auth openai sk-your-key-here
```

### Option C: GitHub Copilot (No Extra API Key)

If you have an active [GitHub Copilot](https://github.com/features/copilot)
subscription and the extension installed in VS Code or JetBrains, ragent will
auto-discover your Copilot token. No configuration needed.

```bash
# Just works if Copilot is configured in your IDE
ragent run --model copilot/gpt-4o "Explain this code"

# Or set the token explicitly
export GITHUB_COPILOT_TOKEN="ghu_your_token_here"
```

### Option D: Ollama (Local — No API Key Required)

```bash
# Install Ollama: https://ollama.com/download
ollama serve          # Start the server
ollama pull llama3.2  # Pull a model

# Ragent auto-detects Ollama at localhost:11434.
# For a remote server, set:
export OLLAMA_HOST="http://your-server:11434"
```

**API key resolution order:** environment variable → provider auto-discovery (Copilot) → database.

---

## 2. First Run

```bash
# Start an interactive session with the default agent ("general")
ragent run "Explain the structure of this project"

# Use a specific model
ragent run --model anthropic/claude-sonnet-4-20250514 "Refactor this function"

# Use a specific agent
ragent run --agent explore "How does the config system work?"

# Use Ollama
ragent run --model ollama/llama3.2 "Write a hello world in Rust"

# Skip the TUI and stream to stdout
ragent run --no-tui "List all TODO items"

# Auto-approve all tool calls (no permission prompts)
ragent run --yes "Fix the failing test in src/lib.rs"
```

---

## 3. List Available Models

```bash
# Show all registered models
ragent models

# Show only Ollama models (queries the running server)
ragent models --provider ollama

# Discover models on a remote Ollama server
ragent models --ollama-url http://remote-server:11434

# Filter by provider
ragent models --provider anthropic
ragent models --provider openai
```

**Default models:**

| Provider   | Model ID                        | Context  | Cost (per 1M tokens)     |
|------------|---------------------------------|----------|--------------------------|
| Anthropic  | `claude-sonnet-4-20250514`      | 200K     | $3 / $15                 |
| Anthropic  | `claude-3-5-haiku-latest`       | 200K     | $0.80 / $4               |
| Copilot    | `gpt-4o`                        | 128K     | Included with subscription |
| Copilot    | `claude-sonnet-4`               | 200K     | Included with subscription |
| Copilot    | `o3-mini`                       | 200K     | Included with subscription |
| OpenAI     | `gpt-4o`                        | 128K     | $2.50 / $10              |
| OpenAI     | `gpt-4o-mini`                   | 128K     | $0.15 / $0.60            |
| Ollama     | *(discovered from server)*      | varies   | Free (local)             |

---

## 4. Configuration File

Ragent loads configuration from multiple sources (last wins):

1. Built-in defaults
2. `~/.config/ragent/ragent.json` (global)
3. `./ragent.json` (project-local)
4. `$RAGENT_CONFIG` (path to a config file)
5. `$RAGENT_CONFIG_CONTENT` (inline JSON)

### Example `ragent.json`

```jsonc
{
  // Default agent to use when --agent is not specified
  "default_agent": "general",

  // Provider configuration
  "provider": {
    "anthropic": {
      "env": ["ANTHROPIC_API_KEY"]
    },
    "ollama": {
      "api": {
        "base_url": "http://localhost:11434"
      },
      "models": {
        "llama3.2": {
          "name": "Llama 3.2"
        },
        "qwen2.5-coder:32b": {
          "name": "Qwen 2.5 Coder 32B"
        }
      }
    }
  },

  // Custom agent definitions
  "agent": {
    "build": {
      "model": "anthropic/claude-sonnet-4-20250514",
      "prompt": "You are a senior software engineer.",
      "temperature": 0.7
    },
    "local": {
      "model": "ollama/llama3.2",
      "prompt": "You are a helpful coding assistant.",
      "temperature": 0.8
    }
  },

  // Permission rules (last match wins)
  "permission": [
    { "permission": "file:read", "action": "allow" },
    { "permission": "file:write", "action": "ask" },
    { "permission": "bash:execute", "action": "ask" }
  ],

  // Extra system instructions appended to the agent prompt
  "instructions": [
    "Always write tests for new code",
    "Use descriptive variable names"
  ]
}
```

View the resolved config at any time:

```bash
ragent config
```

---

## 5. Built-in Agents

| Agent        | Purpose                                    | Mode      |
|--------------|--------------------------------------------|-----------|
| `ask`        | Quick Q&A — answers without tools           | primary   |
| `general`    | General-purpose coding assistant (default) | primary   |
| `build`      | Build, test, and debug                     | subagent  |
| `plan`       | Read-only planning and architecture        | subagent  |
| `explore`    | Fast codebase exploration and search       | subagent  |
| `title`      | Auto-generate session titles               | internal  |
| `summary`    | Summarize conversations                    | internal  |
| `compaction` | Compact long conversation history          | internal  |

Switch agents interactively with the `/agent` slash command (opens a picker dialog),
use `/agent <name>` for direct switching, or cycle with `Tab`/`Shift+Tab`.

```bash
# Use a specific agent from the CLI
ragent run --agent ask "What is the capital of France?"
ragent run --agent plan "Design a REST API for user management"
ragent run --agent explore "How does authentication work in this codebase?"
```

### Project Guidelines (AGENTS.md)

Place an `AGENTS.md` file in your project root to define project-specific guidelines.
On session start, ragent automatically loads this file into the system prompt for all
multi-step agents (general, build, plan, explore). The `ask` agent and internal utility
agents skip it.

The model will acknowledge the guidelines with a brief greeting in the message window
before processing your first message. Example `AGENTS.md` content:

```markdown
# Agent Guidelines
- Language: Rust edition 2021
- Use `cargo build` for builds (timeout 600s)
- Tests go in `tests/` directories, not inline
- Use `tracing` for logging, never `println!`
```

---

## 6. Available Tools

The AI agent can use these tools during a session:

| Tool       | Description                                    | Permission      |
|------------|------------------------------------------------|-----------------|
| `read`     | Read file contents                             | `file:read`     |
| `write`    | Create or overwrite files                      | `file:write`    |
| `edit`     | Apply surgical text replacements               | `file:write`    |
| `bash`     | Execute shell commands (120s timeout)           | `bash:execute`  |
| `grep`     | Search file contents with regex                | `file:read`     |
| `glob`     | Find files by name pattern                     | `file:read`     |
| `list`     | List directory contents (2 levels deep)         | `file:read`     |
| `question` | Ask the user a clarifying question             | `question`      |

MCP servers can provide additional tools that are automatically discovered and
made available to the agent (see [SPEC.md §3.11](SPEC.md#311-mcp-client)). MCP
tools use the official `rmcp` SDK and support both stdio and HTTP transports.

---

## 7. Skills

Skills are reusable task packages that enhance the agent's capabilities. They support both user-initiated invocation (slash commands) and agent auto-invocation (via LLM reasoning).

### Bundled Skills

Ragent includes 4 built-in skills:

| Skill | Description | Usage |
|-------|-------------|-------|
| `/simplify` | Reviews recently changed files for code quality and efficiency | Both user & agent |
| `/batch <instruction>` | Orchestrates large-scale parallel changes across a codebase | Both user & agent |
| `/debug [description]` | Troubleshoots by reading debug logs and error messages | Both user & agent |
| `/loop [interval] <prompt>` | Runs a prompt repeatedly on an interval | User only |

### Creating Custom Skills

Create a skill in your project:

```bash
mkdir -p .ragent/skills/deploy
cat > .ragent/skills/deploy/SKILL.md << 'EOF'
---
name: deploy
description: Deploy the application to production
argument-hint: "[environment]"
agent: build
context: fork
---

Deploy to $0:

1. Run tests: cargo test --release
2. Build: cargo build --release
3. Deploy to $ARGUMENTS
4. Verify the deployment succeeded

Deployment endpoint: !`aws ssm get-parameter --name /deploy/$ARGUMENTS`
EOF
```

### Using Skills

**User invocation:**
```bash
# In TUI, type:
/deploy production

# Or via CLI:
ragent run --agent general "/deploy staging"
```

**Agent auto-invocation:**

The agent automatically sees available skills in its system prompt and can invoke them:

```
User: "Deploy to production"
  ↓
Agent: "I'll use the /deploy skill for this."
  ↓
Agent runs: /deploy production
  ↓
Skill executes (possibly in isolated subagent context)
  ↓
Result returned to agent
```

### Skill Features

- **Arguments**: Use `$0`, `$ARGUMENTS[1]`, `${RAGENT_SESSION_ID}` for dynamic substitution
- **Dynamic context**: Use `` !`command` `` to execute shell commands and inject output
- **Forked execution**: Set `context: fork` to run in an isolated subagent (separate conversation history)
- **Model override**: Use `model: "provider/model"` to use a different model for this skill
- **Tool restrictions**: Use `allowed-tools: [bash, read]` to limit tools available when running the skill

### Personal Skills

Create skills in `~/.ragent/skills/` to make them available across all projects:

```bash
mkdir -p ~/.ragent/skills/code-review
cat > ~/.ragent/skills/code-review/SKILL.md << 'EOF'
---
name: code-review
description: My code review checklist
---

Review this code:
- Variable naming clarity
- Error handling
- Test coverage
- Performance concerns
EOF
```

Then use `/code-review` in any project.

---

## 8. Subagents & Agent Switching

Subagents are specialized agents that handle specific tasks. You can invoke them inline or switch to them interactively.

### Built-in Subagents

| Agent | Purpose | Use Case |
|-------|---------|----------|
| `general` | General-purpose coding (default) | Most tasks |
| `build` | Build, test, and fix compilation errors | Running tests, debugging failures |
| `plan` | Read-only analysis and architecture | Code review, design discussion |
| `explore` | Fast codebase search and analysis | Finding code, understanding patterns |
| `ask` | Quick Q&A without tools | Simple questions |

### Switching Agents

**Interactive switch (TUI):**
```
Tab / Shift+Tab         # Cycle through agents
/agent                  # Open agent picker dialog
/agent <name>           # Switch directly to an agent
```

**CLI invocation:**
```bash
ragent run --agent plan "Design a REST API"
ragent run --agent explore "How does the auth system work?"
ragent run --agent build "Fix the failing tests"
```

---

## 9. Sessions

Ragent persists conversations in sessions stored in SQLite.

```bash
# List all sessions
ragent session list

# Resume a previous session (loads message history into the TUI)
ragent session resume <session-id>

# Export a session to JSON
ragent session export <session-id> > session.json

# Import a session from JSON (creates a new session with re-parented messages)
ragent session import session.json
```

---

## 10. HTTP Server Mode

Run ragent as a headless API server for IDE integrations or remote use:

```bash
# Start the server (prints a bearer token for auth)
ragent serve

# With a custom port
ragent serve --port 8080
```

### API Endpoints

| Method | Path                                     | Description                    |
|--------|------------------------------------------|--------------------------------|
| GET    | `/health`                                | Health check (no auth)         |
| GET    | `/config`                                | Get resolved configuration     |
| GET    | `/providers`                             | List providers and models      |
| POST   | `/sessions`                              | Create a new session           |
| GET    | `/sessions`                              | List all sessions              |
| GET    | `/sessions/{id}`                         | Get session details            |
| POST   | `/sessions/{id}/messages`                | Send a message                 |
| GET    | `/sessions/{id}/messages`                | Get session messages           |
| POST   | `/sessions/{id}/abort`                   | Abort session (archives and publishes `SessionAborted` event) |
| POST   | `/sessions/{id}/permission/{req_id}`     | Reply to a permission request  |
| GET    | `/events`                                | SSE event stream               |

All endpoints except `/health` require the bearer token:

```bash
curl -H "Authorization: Bearer <token>" http://localhost:3000/providers
```

Rate limit: 60 requests per minute per session on the messages endpoint.

---

## 11. Environment Variables

| Variable               | Purpose                                        | Default                    |
|------------------------|------------------------------------------------|----------------------------|
| `ANTHROPIC_API_KEY`    | Anthropic API key                              | —                          |
| `OPENAI_API_KEY`       | OpenAI API key                                 | —                          |
| `GITHUB_COPILOT_TOKEN` | Copilot OAuth token (auto-discovered from IDE) | —                          |
| `OLLAMA_HOST`          | Ollama server URL                              | `http://localhost:11434`   |
| `OLLAMA_API_KEY`       | Optional auth for remote Ollama                | —                          |
| `RAGENT_CONFIG`        | Path to a config file                          | —                          |
| `RAGENT_CONFIG_CONTENT`| Inline JSON config                             | —                          |
| `RUST_LOG`             | Log level filter (e.g. `info`, `debug`)        | `info`                     |

---

## 12. Permissions

Ragent asks for approval before performing potentially dangerous actions.
Control this behavior with permission rules in your config or with `--yes`:

```bash
# Auto-approve everything (use with caution)
ragent run --yes "Delete all .tmp files"
```

**Permission types:** `file:read`, `file:write`, `bash:execute`, `web`,
`question`, `external_directory`

**Actions:** `allow` (always permit), `deny` (always block), `ask` (prompt the user)

Rules are evaluated last-match-wins. Example config:

```json
{
  "permission": [
    { "permission": "file:read", "action": "allow" },
    { "permission": "file:write", "action": "ask" },
    { "permission": "bash:execute", "action": "ask" }
  ]
}
```

---

## 13. Common Workflows

### Code Review

```bash
ragent run --agent explore "Review the changes in the last 3 commits"
```

### Refactoring

```bash
ragent run --agent build "Refactor the user module to use the repository pattern"
```

### Local Ollama for Privacy

```bash
ollama pull qwen2.5-coder:32b
ragent run --model ollama/qwen2.5-coder:32b "Implement a binary search function"
```

### Project Setup

```bash
# Create a project-specific config
cat > ragent.json << 'EOF'
{
  "default_agent": "general",
  "agent": {
    "build": {
      "model": "anthropic/claude-sonnet-4-20250514",
      "prompt": "You are an expert in this project. Follow the conventions in CONTRIBUTING.md."
    }
  },
  "instructions": [
    "Use the existing test patterns in tests/",
    "Follow the error handling conventions in src/error.rs"
  ]
}
EOF

ragent run "Add input validation to the create_user endpoint"
```

---

## 14. TUI Interaction

### Mouse Support

The TUI supports mouse interaction:
- **Scroll wheel** — scrolls the message pane or log panel
- **Scrollbar drag** — click-and-drag the scrollbar track on either pane
- **Text selection** — click-and-drag to select text in any pane
- **Right-click** — copies the current text selection to the clipboard

### Slash Commands

Type `/` in the input to open an autocomplete menu:

| Command | Description |
|---------|-------------|
| `/agent [name]` | Switch agent (dialog or direct) |
| `/clear` | Clear message history |
| `/compact` | Summarise and compact history |
| `/help` | Show available commands |
| `/log` | Toggle log panel |
| `/model` | Switch model |
| `/provider` | Change provider |
| `/provider_reset` | Reset provider credentials |
| `/quit` | Exit ragent |
| `/system <prompt>` | Override system prompt |
| `/tools` | List built-in and MCP tools |

### Key Bindings

| Key | Action |
|-----|--------|
| `Enter` | Send message |
| `Shift+Enter` | Newline in input |
| `Tab` / `Shift+Tab` | Cycle agents |
| `PageUp/PageDown` | Scroll messages |
| `Ctrl+PageUp/Down` | Scroll log panel |
| `Ctrl+C` | Abort / exit |

---

## 15. Data Storage

| Item                | Location                                |
|---------------------|-----------------------------------------|
| Config (global)     | `~/.config/ragent/ragent.json`          |
| Config (project)    | `./ragent.json`                         |
| Database            | `~/.local/share/ragent/ragent.db`       |
| Sessions & messages | Stored in the SQLite database           |
| API keys            | Stored obfuscated in the database       |

---

## Troubleshooting

**"No Copilot token found"**
→ Ensure GitHub Copilot is active in VS Code or JetBrains, or set `GITHUB_COPILOT_TOKEN`. The token is read from `~/.config/github-copilot/apps.json`.

**"No API key found for provider"**
→ Set the environment variable (`ANTHROPIC_API_KEY`, `OPENAI_API_KEY`) or run `ragent auth <provider> <key>`.

**"Failed to connect to Ollama server"**
→ Ensure Ollama is running (`ollama serve`) and accessible at the configured host.

**"No models found on Ollama server"**
→ Pull a model first: `ollama pull llama3.2`

**Permission prompts are annoying**
→ Add `"permission": [{"permission": "file:read", "action": "allow"}]` to your config, or use `--yes`.

**See debug output**
→ Run with `RUST_LOG=debug ragent run "..."` for verbose logging.

---

For full details, see [README.md](README.md) and [SPEC.md](SPEC.md).
