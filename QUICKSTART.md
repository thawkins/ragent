# Ragent Quick Start Guide

Get up and running with ragent in minutes. This guide covers installation,
configuration, and common workflows.

---

## Highlights (0.1.0-alpha.16)

- Image attachment support (Alt+V): paste images from clipboard or file URIs; pending attachments are displayed before sending.
- Keybindings help panel (`?` on empty input) and a right-click context menu for input and message panels.
- New `multiedit` and `patch` tools for atomic multi-file edits and unified diff patching.
- Session-prefixed step numbers for clearer tool call tracing (`[sid:step]`).

## Prerequisites

- **Rust 1.85+** (edition 2024) — install via [rustup](https://rustup.rs)
- An LLM provider: **Anthropic**, **OpenAI**, **Hugging Face**, or a local **Ollama** server

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
                               # press 'p' to open the provider setup dialog```

The dialog walks you through:
1. **Selecting a provider** (Anthropic, OpenAI, Google Gemini, Hugging Face, GitHub Copilot, or Ollama)
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

### Option C: Google Gemini

```bash
export GEMINI_API_KEY="AIza..."
# or store persistently:
ragent auth gemini AIza-your-key-here
```

### Option D: GitHub Copilot (No Extra API Key)

If you have an active [GitHub Copilot](https://github.com/features/copilot)
subscription and the extension installed in VS Code or JetBrains, ragent will
auto-discover your Copilot token. No configuration needed.

```bash
# Just works if Copilot is configured in your IDE
ragent run --model copilot/gpt-4o "Explain this code"

# Or set the token explicitly
export GITHUB_COPILOT_TOKEN="ghu_your_token_here"
```

### Option E: Hugging Face

```bash
export HF_TOKEN="hf_..."
# or store persistently:
ragent auth huggingface hf_your-token-here
```

HuggingFace provides free access to many open-source models via the Inference API.
Get your token at https://huggingface.co/settings/tokens.

```bash
# Use a specific HF model
ragent run --model huggingface/meta-llama/Llama-3.1-70B-Instruct "Hello world"

# For dedicated Inference Endpoints, set base_url in ragent.json
```

### Option F: Ollama (Local — No API Key Required)

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
ragent models --provider gemini
```

**Default models:**

| Provider   | Model ID                        | Context  | Cost (per 1M tokens)     |
|------------|---------------------------------|----------|--------------------------|
| Anthropic  | `claude-sonnet-4-20250514`      | 200K     | $3 / $15                 |
| Anthropic  | `claude-3-5-haiku-latest`       | 200K     | $0.80 / $4               |
| Copilot    | `gpt-4o`                        | 128K     | Included with subscription |
| Copilot    | `claude-sonnet-4`               | 200K     | Included with subscription |
| Copilot    | `o3-mini`                       | 200K     | Included with subscription |
| Gemini     | `gemini-2.5-pro-preview-05-06`  | 1,048K   | $1.25 / $10              |
| Gemini     | `gemini-2.5-flash-preview-05-20`| 1,048K   | $0.15 / $0.60            |
| Gemini     | `gemini-1.5-pro`                | 2,097K   | $1.25 / $5               |
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
      "env": ["ANTHROPIC_API_KEY"],
      "thinking": {
        "enabled": true,
        "level": "low"
      }
    },
    "generic_openai": {
      "env": ["GENERIC_OPENAI_API_KEY"],
      "api": {
        "base_url": "http://127.0.0.1:8080"
      }
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
          "name": "Qwen 2.5 Coder 32B",
          "thinking": {
            "enabled": true,
            "level": "high"
          }
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

## 5b. Custom Agents

You can define your own agents as `.json` files without writing any Rust code.
They are loaded automatically at startup.

### Where to put them

| Directory | Scope |
|-----------|-------|
| `~/.ragent/agents/` | User-global — available in every project |
| `.ragent/agents/` | Project-local — this project only (takes priority) |

### Create a minimal custom agent

```bash
mkdir -p ~/.ragent/agents
cat > ~/.ragent/agents/my-agent.json << 'EOF'
{
  "name": "my-agent",
  "description": "My custom agent",
  "version": "1.0.0",
  "schema_version": "0.7.0",
  "modules": [{
    "type": "ragent/agent/v1",
    "payload": {
      "system_prompt": "You are a helpful AI agent.\nProject: {{WORKING_DIR}}\n\n{{AGENTS_MD}}",
      "mode": "primary",
              "max_steps": 500    }
  }]
}
EOF
```

Start ragent and run `/agents` to confirm it loaded. Use `/agent` to switch
to it — it will appear with a yellow `[custom]` badge in the picker.

### Template variables

Inside `system_prompt` you can use:

| Variable | Value |
|----------|-------|
| `{{WORKING_DIR}}` | Absolute path of the working directory |
| `{{FILE_TREE}}` | Two-level directory listing |
| `{{AGENTS_MD}}` | Contents of `AGENTS.md` (if present) |
| `{{DATE}}` | Current date (`YYYY-MM-DD`) |

### Read-only reviewer example

Copy from the bundled examples and install:

```bash
cp examples/agents/security-reviewer.json ~/.ragent/agents/
```

This gives you a security reviewer that can read all files but cannot edit or
run shell commands.

For the full schema reference and more examples see
[docs/custom-agents.md](docs/custom-agents.md).

---

## 5c. Teams

Use Teams when you want one lead session to orchestrate multiple teammate agents in parallel.

### Start a team in the TUI

```text
/team create feature-squad
```

Check status any time:

```text
/team
/team status
/team tasks
```

Open an existing team:

```text
/team open feature-squad
```

### Spawn teammates (tool-driven)

Use `team_spawn` with a role-specific prompt:

```json
{
  "team_name": "feature-squad",
  "teammate_name": "api-builder",
  "agent_type": "general",
  "prompt": "Implement backend/API changes for the feature."
}
```

### Manage shared tasks

- Lead adds tasks: `team_task_create`
- Teammates claim tasks: `team_task_claim`
- Teammates finish tasks: `team_task_complete`
- Lead monitors: `team_task_list` or `/team tasks`

### Communicate and close out

- Lead message teammate: `/team message <name> <text>` or `team_message`
- Teammate inbox: `team_read_messages`
- Reset task list: `/team clear`
- Close current session context: `/team close`
- Delete persisted team: `/team delete <name>`
- Cleanup: `/team cleanup` or `team_cleanup`

For full details and advanced workflows (plan approval, hooks, graceful shutdown), see:

- [`docs/teams.md`](docs/teams.md)
- [`docs/howto_teams.md`](docs/howto_teams.md)
- [`examples/teams/`](examples/teams/)

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

## 8b. Background Agents (F13 & F14)

Ragent supports spawning multiple sub-agents concurrently for parallel task execution.
Use the `new_task` tool to spawn a background agent while the parent continues processing.

### Spawning Background Tasks

**Via Agent Tool (in chat):**

The agent can spawn background tasks automatically:
```
User: Analyze the codebase in parallel — run explore to find patterns,
      and build to check for compilation errors.

Agent uses:
  /new_task agent="explore" task="Find common patterns in src/ and list them"
  /new_task agent="build" task="Run cargo check and list any errors" background=true
```

**Via TUI Commands:**

```
/tasks              # Show all running and completed tasks
/tasks cancel abc1  # Cancel a task by ID prefix
```

**Via REST API:**

```bash
# Spawn a background task
curl -X POST http://localhost:9100/sessions/{sid}/tasks \
  -H "Authorization: Bearer token" \
  -H "Content-Type: application/json" \
  -d '{
    "agent": "explore",
    "task": "Analyze the authentication module",
    "background": true
  }'

# List tasks for a session
curl http://localhost:9100/sessions/{sid}/tasks \
  -H "Authorization: Bearer token"

# Get task details
curl http://localhost:9100/sessions/{sid}/tasks/{task_id} \
  -H "Authorization: Bearer token"

# Cancel a task
curl -X DELETE http://localhost:9100/sessions/{sid}/tasks/{task_id} \
  -H "Authorization: Bearer token"
```

### Configuration

Control background agent limits in `ragent.json`:

```jsonc
{
  "experimental": {
    "maxBackgroundAgents": 4,        // Max concurrent background tasks (default: 4)
    "backgroundAgentTimeout": 3600   // Timeout in seconds (default: 1 hour)
  }
}
```

### Result Injection

When a background task completes, the result is automatically injected into the parent
session as a system message, allowing the agent to act on it in the next iteration.

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
| `GEMINI_API_KEY`       | Google Gemini API key                          | —                          |
| `GENERIC_OPENAI_API_KEY` | Generic OpenAI API key                       | —                          |
| `GENERIC_OPENAI_API_BASE` | Generic OpenAI endpoint URL (host + port)   | `https://api.openai.com`   |
| `GITHUB_COPILOT_TOKEN` | Copilot OAuth token (auto-discovered from IDE) | —                          |
| `OLLAMA_HOST`          | Ollama server URL                              | `http://localhost:11434`   |
| `OLLAMA_API_KEY`       | Optional auth for remote Ollama                | —                          |
| `HF_TOKEN`             | HuggingFace API token                          | —                          |
| `HUGGING_FACE_HUB_TOKEN` | HuggingFace API token (legacy name)          | —                          |
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
| `/thinking auto|off|low|medium|high` | Switch reasoning level for the active model |
| `/provider` | Change provider |
| `/provider_reset` | Reset provider credentials |
| `/quit` | Exit ragent |
| `/system <prompt>` | Override system prompt |
| `/opt help` | Show prompt optimization method table |
| `/opt <method> <prompt>` | Optimize prompt with named method |
| `/bench list` | List available benchmark suites and profiles |
| `/bench init <suite-or-all-or-full>` | Create benchmark data roots in sample mode or full-download mode |
| `/bench run <suite-or-profile-or-all>` | Start a background benchmark run and write workbook output |
| `/bench status` | Show active or last benchmark run status |
| `/bench open last` | Show the latest benchmark workbook path(s) and summary |
| `/bench cancel` | Cancel the active benchmark run |
| `/codeindex on` | Enable codebase indexing |
| `/codeindex off` | Disable codebase indexing |
| `/codeindex show` | Show index status and statistics |
| `/codeindex reindex` | Trigger a full re-index |
| `/codeindex help` | Show code index help |

### Benchmark Workflow

The benchmark runner uses the **currently selected model/provider** in the TUI and writes one
normalized workbook per benchmark under:

```text
benches/<suite>/<language>/<YYYY-MM-DD UTC>/<provider>/<model>.xlsx
```

Typical workflow:

```text
/bench list
/bench init humaneval
/bench init humaneval --full
/bench init humaneval --full --language rust
/bench init mbpp --full
/bench init mbpp --full --language rust
/bench init multipl-e --language rust
/bench init all
/bench init all --full
/bench init full
/bench init humaneval --verify-only
/bench run quick
/bench run multipl-e --language rust --yes
/bench run all --yes
/bench status
/bench open last
/bench cancel
```

Notes:

- `/bench init <suite-or-all-or-full>` creates or refreshes `benches/data/<suite>/<language>/`.
- `/bench init all` initializes sample fixtures for every registered suite.
- `/bench init all --full` uses full upstream ingestion where available and falls back to sample fixtures for suites that do not yet support full-data initialization.
- `/bench init <suite> --full` pulls full upstream benchmark data for suites that support it.
- `/bench init humaneval --full` now pulls every HumanEvalPack language partition by default; use `--language <lang>` to target just one partition such as `rust`.
- `/bench init mbpp --full` now pulls every BC-MBPP language partition by default; use `--language <lang>` to target just one partition such as `rust`.
- `/bench init full` is a gated virtual target that is reserved for complete full-data ingestion across every suite.
- `/bench run <suite-or-profile-or-all>` runs in the background and records a workbook plus resume/debug sidecars.
- Use `--limit N` or `--cap N` to cap a run to the first `N` cases.
- `/bench status` shows active-run context while running and the last completion summary afterward.
- `/bench open last` prints the latest workbook path(s) so you can open or inspect results directly.
- `/bench cancel` requests shutdown of the active benchmark task.
- Use `--resume` to reuse a same-day workbook only when the benchmark/model/config hash matches.

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

## Multi-Agent Orchestration (F6)

`ragent-core` includes a built-in orchestration layer that lets multiple agents collaborate on a single job.

### Register agents and start a job (Rust)

```rust
use ragent_core::orchestrator::{AgentRegistry, Coordinator, JobDescriptor, Responder};
use ragent_core::orchestrator::policy::{ConflictPolicy, ConflictResolver};
use futures::future::FutureExt;
use std::sync::Arc;

let registry = AgentRegistry::new();

// Register two agents with overlapping capabilities.
let r_a: Responder = Arc::new(|p| async move { format!("A: {}", p) }.boxed());
let r_b: Responder = Arc::new(|p| async move { format!("B: {}", p) }.boxed());
registry.register("agent-a", vec!["search".to_string()], Some(r_a)).await;
registry.register("agent-b", vec!["search".to_string()], Some(r_b)).await;

// Apply a conflict resolution policy (optional; default is Concat).
let coord = Coordinator::new(registry)
    .with_policy(ConflictResolver::new(ConflictPolicy::FirstSuccess));

// Sync: fan-out to all matching agents, aggregate per policy.
let result = coord.start_job_sync(JobDescriptor {
    id: "job-1".to_string(),
    required_capabilities: vec!["search".to_string()],
    payload: "find TODOs".to_string(),
}).await?;

// Async: returns immediately; poll for status.
let job_id = coord.start_job_async(JobDescriptor { ... }).await?;
let (status, result) = coord.get_job_result(&job_id).await.unwrap();
```

Run the complete example with:
```sh
cargo run -p ragent-core --example orchestration
```

### Conflict resolution policies

| Policy | Behaviour |
|---|---|
| `Concat` | Concatenate all responses (default) |
| `FirstSuccess` | Return the first response that doesn't start with `"error:"` |
| `LastResponse` | Return only the final agent's response |
| `Consensus{threshold}` | Return a response that appears ≥ N times; otherwise return all with `[no consensus]` |
| `HumanReview` | Delegate to a `HumanFallback` impl (`LoggingFallback` by default) |

### Remote agents via pluggable transport

```rust
use ragent_core::orchestrator::transport::{HttpRouter, RemoteAgentDescriptor, RouterComposite};

let mut http_router = HttpRouter::new();
http_router.register(RemoteAgentDescriptor {
    agent_id: "remote-search".to_string(),
    base_url: "http://search-service:8080".to_string(),
    capabilities: vec!["search".to_string()],
});

// Chain: try local router first, fall back to remote.
let local = InProcessRouter::new(registry.clone());
let composite = RouterComposite::new(vec![
    Arc::new(local),
    Arc::new(http_router),
]);
let coord = Coordinator::with_router(registry, Arc::new(composite));
```

### Leader election for distributed coordinators

```rust
use ragent_core::orchestrator::leader::{CoordinatorCluster, LeaderElector};

let elector = LeaderElector::new(3); // quorum size
let cluster = CoordinatorCluster::new(elector);

cluster.add("node-1", Coordinator::new(registry_a)).await;
cluster.add("node-2", Coordinator::new(registry_b)).await;

cluster.elect("node-1").await;  // elect node-1 as leader
cluster.start_job(desc).await?; // routes to elected leader
```

### HTTP API

When running `ragent serve`, the orchestration endpoints are available under `/orchestrator`:

```sh
# Start a job (async)
curl -X POST http://localhost:3000/orchestrator/start \
  -H 'Authorization: Bearer <token>' \
  -H 'Content-Type: application/json' \
  -d '{"required_capabilities":["search"],"payload":"find TODOs","mode":"async"}'
# → {"job_id":"<uuid>"}

# Poll for result
curl http://localhost:3000/orchestrator/jobs/<uuid> \
  -H 'Authorization: Bearer <token>'
# → {"id":"...","status":"completed","result":"..."}

# Live metrics
curl http://localhost:3000/orchestrator/metrics \
  -H 'Authorization: Bearer <token>'
# → {"active_jobs":0,"completed_jobs":3,"timeouts":0,"errors":0}
```

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
