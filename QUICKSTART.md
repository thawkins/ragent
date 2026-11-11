# Ragent Quick Start Guide

Get up and running with ragent in minutes. This guide covers installation,
configuration, and common workflows.

---



## Prerequisites

- **Rust 1.91+** (edition 2024) — install via [rustup](https://rustup.rs)
- **cargo-nextest** to run the tests — `cargo install cargo-nextest --locked`
- An LLM provider: **Anthropic**, **OpenAI**, **Hugging Face**, **Amazon Bedrock**, or a local **Ollama** server

## Installation

```bash
# Clone and build
git clone https://github.com/thawkins/ragent.git
cd ragent
cargo build --release

# The binary is at target/release/ragent
# Optionally copy it to your PATH:
cp target/release/ragent ~/.local/bin/

# Check the installed version:
ragent --version    # or: ragent -V
```

Build requirements, the incremental-build setting and expected build cost are in
[README.md - From source](README.md#from-source); [`docs/BUILDSTATS.md`](docs/BUILDSTATS.md) records the
measured numbers.

---

## 1. Configure a Provider

Ragent needs at least one LLM provider. The easiest way is via the **interactive TUI**:

```bash
ragent        # launch ragent
# press 'p' to open the provider setup dialog
```

The dialog walks you through:
1. **Selecting a provider** (Anthropic, OpenAI, Google Gemini, Hugging Face, GitHub Copilot, Amazon Bedrock, OpenRouter, or Ollama)
2. **Entering your API key** (if required — Copilot auto-discovers, Ollama needs none, OpenRouter needs `OPENROUTER_API_KEY`)
3. **Choosing a model** from the provider's available models

Every step is a list wearing the same calm-slate chrome the `/plugins` and
`/connectors` catalogues use: an accent frame naming the step and its subject, a
heading strip with a live count, a pinned column band, a cursor-filled zebra row
list, and a muted key-hint footer. Each list windows around its cursor, so a list
longer than the dialog keeps the highlighted row painted and the legend reads
`Showing a-b of N`; a mouse wheel notch over the dialog steps the cursor exactly
like `Up`/`Down`.

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

### Option D: OpenRouter (200+ models with one key)

```bash
export OPENROUTER_API_KEY="sk-or-..."
# or store persistently:
ragent auth openrouter sk-or-your-key-here
```

OpenRouter exposes vendor models behind an OpenAI-compatible endpoint, so
model ids keep their vendor slug:

```bash
ragent run --model openrouter/anthropic/claude-sonnet-4 "Explain this code"
ragent run --model openrouter/openai/gpt-4o "Write a Rust test"
ragent run --model openrouter/deepseek/deepseek-chat "Summarise this file"
```

### Option E: GitHub Copilot (No Extra API Key)

If you have an active [GitHub Copilot](https://github.com/features/copilot)
subscription and the extension installed in VS Code or JetBrains, ragent will
auto-discover your Copilot token. No configuration needed.

```bash
# Just works if Copilot is configured in your IDE
ragent run --model copilot/gpt-4o "Explain this code"

# Or set the token explicitly
export GITHUB_COPILOT_TOKEN="ghu_your_token_here"
```

### Option F: Hugging Face

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

### Option G: Ollama (Local — No API Key Required)

```bash
# Install Ollama: https://ollama.com/download
ollama serve          # Start the server
ollama pull llama3.2  # Pull a model

# Ragent auto-detects Ollama at localhost:11434.
# For a remote server, set:
export OLLAMA_HOST="http://your-server:11434"
```

### Option H: Amazon Bedrock (AWS Credentials)

```bash
# Set AWS credentials (or use an AWS profile)
export AWS_ACCESS_KEY_ID="AKIAIOSFODNN7EXAMPLE"
export AWS_SECRET_ACCESS_KEY="wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"
export AWS_REGION="us-east-1"

# Use a Bedrock model
ragent run --model bedrock/anthropic.claude-sonnet-4-20250514-v1:0 "Hello world"

# Or use a short alias
ragent run --model bedrock/claude-sonnet-4-20250514 "Explain this code"

# Or configure in ragent.json
# {
#   "provider": {
#     "bedrock": {
#       "options": { "region": "us-east-1", "profile": "my-dev-profile" }
#     }
#   }
# }
```

Bedrock supports both Anthropic Claude models (via Messages API) and
non-Anthropic models like Amazon Nova and Meta Llama (via Converse API).
No API key is needed — authentication uses your AWS credentials.

**API key resolution order:** environment variable → provider auto-discovery (Copilot, OpenRouter discovery without chat) → database.

---

## 1b. Validate Before You Run (Dry Run)

Before invoking the model or executing any tool, run a readiness check to catch
missing credentials, misconfigured MCP servers, or broken skill paths:

```bash
# Human-readable report
ragent --dry-run

# Same check, machine-readable JSON for CI/CD
ragent --dry-run --json

# Equivalent subcommand
ragent config check --json
```

Example human output:

```text
config    ready   ragent.json parsed
provider  ready   anthropic/claude-sonnet-4-20250514 (ANTHROPIC_API_KEY set)
skills    ready   7 skills loaded (4 bundled + 3 discovered)
tools     ready   111 tools visible across 15 families
mcp       ready   1 server reachable
Verdict: READY
```

Example CI script consuming the JSON report:

```bash
#!/usr/bin/env bash
set -euo pipefail

report=$(ragent --dry-run --json)
verdict=$(echo "$report" | jq -r '.verdict')

if [[ "$verdict" == "BLOCKED" ]]; then
  echo "Deployment check failed:" >&2
  echo "$report" | jq '.sections[] | select(.status == "blocked")' >&2
  exit 1
fi

if [[ "$verdict" == "WARNING" ]]; then
  echo "Deployment usable but has warnings:" >&2
  echo "$report" | jq '.sections[] | select(.status == "warning")' >&2
fi

echo "Deployment readiness: $verdict"
```

Exit codes follow the verdict:

| Verdict  | Exit code | Meaning |
|----------|-----------|---------|
| `READY`  | `0`       | Ready to run |
| `WARNING`| `0`       | Usable, but with non-fatal issues |
| `BLOCKED`| `1`       | Fatal problem; do not deploy |

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
ragent run --no-tui "List all tasks"

# Auto-approve all tool calls (no permission prompts)
ragent run --yes "Fix the failing test in src/lib.rs"
```

---

## 3. List Available Models

```bash
# Show all registered models
ragent models

# Show only OpenRouter models
ragent models --provider openrouter

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
| OpenRouter | *(discovered from API)*         | varies   | Per-model from catalog   |

---

## 4. Configuration File

Ragent loads configuration from multiple sources (last wins):

1. Built-in defaults
2. `~/.config/ragent/ragent.json` (global)
3. `./ragent.json` (project-local)
4. `$RAGENT_CONFIG` (path to a config file)
5. `$RAGENT_CONFIG_CONTENT` (inline JSON)

A project-local file is untrusted, so the privileged keys it may not set are
stripped before the merge: `yolo`, allow-rules, bash/dirs lists, `telemetry`,
`hooks`, `openai`, `mcp`, `acp`, and every provider's `api` block. Put those in
the user-global `~/.config/ragent/ragent.json`. Rejected keys are logged at
`warn!`.

### Example `ragent.json`

```jsonc
{
  // Default agent to use when --agent is not specified
  "default_agent": "general",

  // Provider configuration
  "provider": {      "anthropic": {
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
      },
      "openrouter": {
        "api": {
          "base_url": "https://openrouter.ai"
        },
        "models": {
          "anthropic/claude-sonnet-4": {
            "name": "Claude Sonnet 4 (via OpenRouter)",
            "thinking": {
              "enabled": true,
              "level": "medium"
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
  ],

  // Context-window compaction (OpenCode-derived summarisation)
  "compaction": {
    "auto": true,          // auto-summarise before send (FR-008)
    "threshold": 0.7,    // trigger at 70% of the model's context window
    "buffer": 0.10,       // fallback response/safety buffer as a fraction of the window when threshold is null (FR-011)
    "keep": { "tokens": 0.20 },  // fraction of the window kept as recent turns verbatim (FR-011)
    // Optional fast/cheap model for the summarisation call (/compact only):
    // "model": { "provider_id": "ollama", "model_id": "qwen2.5:1.5b" },
    // Optional summary budget (default 1500) and tool-output truncation (default 2000):
    // "summary_tokens": 1500,
    // "tool_output_max_chars": 2000
  },

  // Token-efficient GCF encoding of JSON tool results (default off;
  // omitted from the file while disabled). Toggle with /gcf on|off.
  "gcf": {
    "enabled": false
  },

  // OpenTelemetry metrics export (optional)
  "telemetry": {
    "otel": {
      "enabled": false,
      "endpoint": "http://localhost:4318",
      "protocol": "http",
      "export_interval_seconds": 30,
      "export_timeout_seconds": 10,
      "service_name": "ragent",
      "resource_attributes": { "deployment": "local" },
      "metrics": { "ragent.snapshot.restores": false },
      "internal_port": null
    }
  },

  // Plugin subsystem (optional; defaults shown). Loads Codex- and
  // Claude Code/Desktop-dialect plugins on a sandboxed JS engine.
  // `enabled: false` makes the whole subsystem inert (no discovery/loading).
  "plugins": {
    "enabled": true,
    "max_execution_ms": 5000,   // per-tool wall-clock budget
    "max_entry_ms": 10000,      // entry-point wall-clock budget
    "max_memory_mb": 64         // per-JS-context memory ceiling
    // "store_dir": "/path/to/plugins"          // override the plugin store path
    // "permissions": { "my-plugin": ["network.outbound"] }
  },

  // Connector subsystem (optional; defaults shown). A connector is a named
  // integration (Google Drive, Slack, GitHub, ...) reached through one or more
  // MCP servers. `enabled: false` makes the whole subsystem inert (no
  // discovery, catalogue fetch, or connection).
  "connectors": {
    "enabled": true,
    "store_dir": null,          // override the connector store path
    "stores": {                 // catalogue endpoints + shared fetch budgets
      "timeout_ms": 10000,
      "max_index_bytes": 2097152,
      "cache_ttl_secs": 3600
    },
    "credentials": {}           // non-secret credential-name mapping by connector id
  },

  // Execution backend (optional). Omitted = `local` (host execution). A
  // `docker`/`podman` label runs shell/file/search tools inside a container whose
  // workspace is a named volume, never a host bind mount; `remote` drives a
  // second ragent server. `credentials` names encrypted-store secrets injected as
  // container env vars at spawn time (the value is never in this file).
  "execution_backend": "podman",
  "backends": [
    { "id": "sandbox", "name": "Podman sandbox", "kind": "podman",
      "image": "ghcr.io/yourorg/ragent-sandbox:latest", "workspace": "/projects",
      "credentials": ["EXAMPLE_API_KEY"] }
  ],

  // LLM security analyzer (optional; off by default). A tightening-only layer
  // over the static permission rules: each proposed tool action gets an
  // allow/ask/deny verdict with a rationale, published before it runs.
  "security_analyzer": {
    "enabled": true,
    "model": { "provider_id": "ollama", "model_id": "qwen2.5:1.5b" }
  },

  // OpenAI-compatible inbound surface (optional). When `enabled` is true a
  // `token` (or the ambient `RAGENT_TOKEN`) is required or `ragent serve`
  // refuses to start the surface.
  "openai": {
    "enabled": true,
    "token": "[REDACTED]"
  },

  // UI internationalisation (optional). A non-English ambient `LANG` with an
  // installed catalog enables it automatically.
  "i18n": {
    "enabled": true,
    "locale": "fr"
  }
}
```

See SPEC.md §19D (execution backends), §19E (ACP), and §19F (automation) for
the full schemas, plus the manuals in `docs/howtos/slashcommands/`.

View the resolved config at any time:

```bash
ragent config
```

### Backing Up and Restoring Configuration

Before making risky edits, snapshot the current global config:

```bash
# Inside the TUI
/config save
```

This writes a timestamped copy to `~/.config/ragent/saves/ragent.json.YYYY-MM-DD.HH-MM-SS`.

To view or restore a previous snapshot:

```bash
# Inside the TUI
/config list
```

Use ↑/↓ (or `k`/`j`) to choose a backup, **Enter** to restore it over the active
`ragent.json`, or **Esc**/**q** to cancel. Restore is atomic and the running
session's cached configuration is invalidated so the next agent turn picks up
the restored values.

### OpenTelemetry Metrics and Log Export

Enable background metric export to any OTLP-compatible collector by adding a
`telemetry.otel` block:

```jsonc
{
  "telemetry": {
    "otel": {
      "enabled": true,
      "endpoint": "http://localhost:4318",
      "protocol": "http",
      "export_interval_seconds": 30,
      "export_timeout_seconds": 10,
      "service_name": "ragent",
      "resource_attributes": { "deployment": "local" },
      "metrics": {
        "ragent.llm.requests": true,
        "ragent.tool.invocations": true,
        "ragent.snapshot.restores": false
      },
      "internal_port": 9090,
      "logs": { "enabled": true, "level": "warn" }
    }
  }
}
```

- `enabled` — master switch. Default `false`; no metrics are recorded or
  exported when disabled.
- `endpoint` — OTLP collector **base** URL. Use `http://host:4318` for HTTP and
  `http://host:4317` for gRPC. ragent appends the OTLP/HTTP signal path itself
  (`/v1/metrics`, `/v1/logs`), so `http://localhost:4318` exports metrics to
  `http://localhost:4318/v1/metrics`; a base path
  (`https://gw.example.com/otel`) is kept, and a value that already names a
  signal path (`.../v1/metrics`) is used exactly as written.
- `protocol` — `http` (default) or `grpc`.
- `export_interval_seconds` — how often buffered metrics and logs are exported.
- `export_timeout_seconds` — per-export request timeout; a slow endpoint will
  not block the agent loop.
- `service_name`/`resource_attributes` — attached to every exported metric.
- `metrics` — optional per-metric enable/disable toggles. Metrics not listed
  are enabled by default.
- `internal_port` — optional in-process Prometheus text endpoint on
  `127.0.0.1:<port>/metrics` for local scraping.
- `logs` — opt-in OTLP **logs** signal (`enabled: false` by default). It adds a
  `tracing` sink beside the local one rather than replacing it, so the TUI log
  panel (or stderr in a headless run) still shows everything `--log-level`
  selects; `level` (`trace`/`debug`/`info`/`warn`/`error`, default `info`) is a
  floor for the *remote* export only. Bodies pass through the shared credential
  redactor (over 16 KiB truncated) and attributes through the metric
  sensitive-data guard. Log export rides the same switch as the metrics:
  `/telemetry on` enables both signals and `/telemetry off` stops both - there
  is no separate logs on/off, only `/telemetry logs <level>` to set how much is
  exported. A hand-edited config still needs `telemetry.otel.enabled: true` for
  the `logs` block to ship anything.

Export failures are surfaced rather than silent: each non-2xx response,
transport error or timeout produces an `Agent Notice` naming the consecutive
failure count and the masked endpoint, and five consecutive failures switch
telemetry off (persisting `telemetry.otel.enabled: false`) instead of retrying a
dead collector forever.

Each failure streak belongs to one export pipeline. A reconfigure - any
`/telemetry on|off|logs|setup` - starts a new generation, and the retired
pipeline is shut down on a background thread after the runtime lock is
released, so `/telemetry off` against a dead collector never freezes the TUI
for the length of the export timeout. The retired pipeline's own final export
is forwarded unchanged but no longer resets or extends the live streak, and
`flush()`/`shutdown()` attempt both signals so an unhealthy log endpoint cannot
drop the last metric batch.

`/telemetry on|off` report `[err]` with the cause when the stored block is
invalid, the save fails, or the live subsystem refuses the change - rather than
printing `[ok]` unconditionally - and saving creates the user-global config
directory when it does not exist, so a fresh install can configure telemetry at
all. The switch survives a restart: saving removes the deprecated
`experimental.open_telemetry` alias that would otherwise re-enable export on the
next load. `/telemetry setup` merges the dialog onto the block already stored on
disk (so `service_name`, `resource_attributes`, per-metric toggles and
`cardinality_limit`, which have no dialog row, are preserved) and its protocol
toggle follows the transport with the endpoint's OTLP default port - 4318 for
HTTP, 4317 for gRPC - leaving a custom port alone. A changed
`cardinality_limit` applies on reconfigure instead of at the next restart.

Inside the TUI you can also run:

```
/telemetry on                                # export metrics AND logs
/telemetry off                               # stop both (persists enabled: false)
/telemetry logs [all|trace|debug|info|warn|error]  # log-export level only
/telemetry counters                          # counters plus the log-export state
/telemetry setup                             # dialog, including the Logs level row
/telemetry_panel                             # toggle the live telemetry side panel
```

OTEL support is built in by default: `ragent-telemetry` enables its
`telemetry` Cargo feature by default, so a plain `cargo build --release`
includes the exporters. Only a build that disables that crate's default
features compiles telemetry to zero-overhead no-ops with no OpenTelemetry
dependencies; in such a build `/telemetry on` reports that the feature is
missing.

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

#### Modular instruction files with `@<path>`

To keep large `AGENTS.md` files manageable, split them into smaller files and
pull them in with an `@<path>` directive on its own line. The `@` must appear
in the **first column** (no leading whitespace):

```markdown
# Agent Guidelines

@docs/coding-style.md
@docs/testing.md
@"docs/commit conventions.md"
```

The directive is replaced in-place by the contents of the referenced file
before the guidelines are loaded into the system prompt. Paths resolve
relative to the directory of the file containing the directive, includes are
transitive (included files can themselves include other files), and cycles are
detected and skipped; nesting is capped at 16 levels. Paths must stay under the project root or the global
ragent data dir — `../` escapes and absolute paths are rejected.

A leading `@@` is an escape sequence: the line is emitted verbatim with the
`@@` collapsed to a single literal `@` character (use this to write a line
that starts with `@` without triggering an include).

---

## 5b. Custom Agents

You can define your own agents as `.json` files without writing any Rust code.
They are loaded automatically at startup.

### Where to put them

| Directory | Scope |
|-----------|-------|
| `.ragent/agents/` | Project-local — this project only (takes priority) |
| `~/.config/ragent/agents/` | User-global — every project (XDG config; takes priority over `~/.ragent/agents/`) |
| `~/.ragent/agents/` | User-global — every project (legacy location) |

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
              "max_steps": 1024    }
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

Write this profile to `~/.ragent/agents/security-reviewer.json` (or to
`.ragent/agents/` for one project only):

```json
{
  "name": "security-reviewer",
  "description": "OWASP-focused security code reviewer",
  "version": "1.0.0",
  "schema_version": "0.7.0",
  "modules": [{
    "type": "ragent/agent/v1",
    "payload": {
      "system_prompt": "You are a security-focused code reviewer.\nWorking directory: {{WORKING_DIR}}\n\n{{AGENTS_MD}}",
      "mode": "primary",
      "max_steps": 30,
      "temperature": 0.2,
      "permissions": [
        { "permission": "read", "pattern": "**", "action": "allow" },
        { "permission": "edit", "pattern": "**", "action": "deny"  },
        { "permission": "bash", "pattern": "**", "action": "deny"  }
      ]
    }
  }]
}
```

This gives you a security reviewer that can read all files but cannot edit or
run shell commands.

For the full schema reference and more examples see
[docs/howtos/custom-agents.md](docs/howtos/custom-agents.md).

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
- Teammates finish tasks: `team_task_complete` — accepted only from the task's
  assigned agent or the lead. A refusal does **not** hand the task over; it
  reports `rejection_reason` (`not_owner`, `not_claimed`, `already_completed`)
  plus the current owner, and suggests either `team_task_claim` or
  `team_assign_task` from the lead.
- Lead withdraws work: `team_task_cancel` (lead-only; only `pending` and
  `in_progress` are cancellable, and the owner is notified through its mailbox).
  The task stays in the list with status `cancelled` — it is not deleted.
- Lead monitors: `team_task_list`, or `team_status` for the roster. `team_status`
  reports each member's last five status transitions (from the team's
  `status-history.jsonl`), any message stranded in a terminated member's mailbox
  under `Undelivered messages`, any plan parked in `plan_pending` under
  `Plans awaiting a verdict`, and any quality-gate hook that could not run under
  `Hook failures`.
- Dependency hygiene: a `tasks.json` write validates the whole list first, so a
  `depends_on` id naming no task, or an edge that would close a cycle, is refused
  (`refusing to write invalid <path>: ...`) and the file is left byte-identical.

### Communicate and close out

- Lead message teammate: `/team message <name> <text>` or `team_message`
- Teammate inbox: `team_read_messages`
- Reset task list: `/team clear`
- Close current session context: `/team close`
- Delete persisted team: `/team delete <name>`
- Cleanup: `/team cleanup` or `team_cleanup`

For full details and advanced workflows (plan approval, hooks, graceful shutdown), see:

- [`docs/howtos/teams.md`](docs/howtos/teams.md)
- [`docs/howtos/slashcommands/blueprints.md`](docs/howtos/slashcommands/blueprints.md)
  (`/blueprints list` shows the team blueprints installed locally)

---

## 6. Available Tools

The AI agent can use these tools during a session:

| Tool       | Description                                    | Permission      |
|------------|------------------------------------------------|-----------------|
| `read`     | Read file contents                             | `file:read`     |
| `Write`    | Create or overwrite files                      | `file:write`    |
| `Edit`     | Apply surgical text replacements               | `file:write`    |
| `Bash`     | Execute shell commands (120s timeout)           | `bash:execute`  |
| `Grep`     | Search file contents with regex                | `file:read`     |
| `Glob`     | Find files by name pattern                     | `file:read`     |
| `list`     | List directory contents (2 levels deep)         | `file:read`     |
| `AskUserQuestion` | Ask the user a clarifying question; accepts an optional `default` option used if the dialog times out | `AskUserQuestion`      |
| `ragent_info`| Report ragent version, build time, and execution details | `none`          |
| `os_info`  | Report host OS/hardware (also `/osinfo show`)   | `none`          |
| `tool_info`| JSON dump of the tool registry                | `none`          |
| `commands_info`| JSON dump of the slash-command catalog      | `none`          |
| `DateTime` | Current date/time: RFC 3339 UTC instant, an optional fixed UTC-offset rendering, date/time fields, Unix timestamp | `none`          |

Run `/tools` in the TUI to list every registered tool, or `ragent --dry-run --json
run x` for the total and per-category counts (see
[docs/howtos/tools/INDEX.md](docs/howtos/tools/INDEX.md)). `mf_screenshot` is
registered only when a drivable headless browser engine is present — spec
`openhands` FR-029.

**Provider spellings (optional).** A model, an ACP client or an MCP server may
name a tool with a spelling ragent does not register — Anthropic's `WebFetch`,
OpenAI's `web_search`/`file_search`. Enable `tools.aliases` in `ragent.json` to
resolve such a spelling onto the ragent tool **before** the unknown-tool check:

```json
{ "tools": { "aliases": true } }
```

`true` enables the built-in table (`WebFetch` → `webfetch`, `WebSearch` →
`websearch`, `file_search` → `codeindex_search`, `web_search` → `websearch`);
an object adds your own entries on top. Resolution is exact and case-sensitive
and a registered name always wins, so an alias can never shadow or re-point a
real tool, and an unmapped name still reaches the unknown-tool error with its
did-you-mean hint. The thirteen renamed tools (`Bash`, `Edit`, `Write`, ...) are
registry names themselves and need no alias. `tools.expose_alias_names: true`
additionally advertises the remaining spellings in the outbound tool list. With
`tools` absent, behaviour is unchanged.

The **PDF family** (`pdf_read`, `pdf_write`) reads and writes PDF files
entirely in-process (see [`docs/howtos/office.md`](docs/howtos/office.md)).

Goal-driven loop programming is available via the **`/loop` slash command**:
it runs a `LoopSpec` (agent, goal, verify command, scope, tool set, budget) to
a stop condition, with checkpoint capture and an optional post-loop rollback.
The one-shot form `/loop <agent> [flags] <goal>` accepts `--max-steps N`,
`--cost_limit N`, and `--timeout N` overrides in any position; the default
step budget is 512 (`loop.max_steps` in `ragent.json`).
See [`docs/howtos/loopprogramming.md`](docs/howtos/loopprogramming.md).

GCF (Graph Compact Format) token-efficient tool-result encoding is off by
default. Toggle it with the `/gcf on|off` slash command (persists
`gcf.enabled` to your config file), inspect the effective state and source
with `/gcf show`, and read usage with `/gcf help`. While on, eligible JSON
tool results (>= 200 chars) are re-encoded losslessly inside a
`[BEGIN GCF generic]` ... `[END GCF]` block in the model-facing copy only,
and the system prompt gains a short reading primer. TUI rendering, the
activity log, memory extraction, hooks, and compaction always keep seeing the
raw JSON. See SPEC.md section 5.6 for details.

MCP servers can provide additional tools that are automatically discovered and
made available to the agent (see [SPEC.md §19](SPEC.md#19-mcp-integration-model-context-protocol)). MCP
tools use the official `rmcp` SDK and support both stdio and HTTP transports.

Use **`/mcp config`** to see where ragent looks for MCP server definitions — the
ragent-native `~/.config/ragent/servers/` directory, npm global `node_modules`,
`PATH`, and the Claude/Cline/legacy/XDG registry directories — with the number of
definitions found at each location (and a `(missing)` marker for an absent
directory) followed by the individual servers discovered. Use `/mcp discover` to
scan and install interactively, and `/mcp connect <id>` / `/mcp disconnect <id>`
to toggle a server live. `/mcp remove <id>` deletes a configured server's
`ragent.json` entry (preserving every other key) and clears its enable-ledger
row, so a re-added server starts enabled; a server bridged from a plugin or a
connector is refused by name and is removed with `/plugins remove <id>` or
`/connectors remove <id>` instead. `/mcp config` and the discover dialog both
name the manifest that declares each server, so a bridged row is
distinguishable from your own `ragent.json` entry before any removal.

UI internationalisation is opt-in and English by default. Install a locale
message catalog at `<project>/.ragent/locales/<locale>.json` (or
`~/.config/ragent/locales/<locale>.json`), then enable it with the **`/i18n`**
slash command: `/i18n on`, `/i18n fr` (or any locale tag such as
`/i18n de_DE.UTF-8`), `/i18n off`, `/i18n status`, `/i18n list`, and
`/i18n help`. Translated keys render from the catalog; any key the catalog does
not translate falls back to the compiled English text (never a raw key or a
blank). With no `i18n` config section, a non-English `LANG`/`LC_ALL` with an
installed catalog enables translation automatically. A catalog is JSON:
`{"locale":"fr","messages":{"status.project":"Projet : "}}`.

---

## 7. Skills

Skills are reusable task packages that enhance the agent's capabilities. They support both user-initiated invocation (slash commands) and agent auto-invocation (via LLM reasoning).

### Bundled Skills

Ragent bundles these skills:

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

1. Run tests: cargo nextest run --release
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
- **Tool restrictions**: Use `allowed-tools: [Bash, read]` (or the YAML block form) to limit tools available when running the skill. Entries resolve by name, **ignoring case**: the adopted Claude spellings (`Bash`, `Edit`, `Grep`, ...) and the tools that kept their ragent names (`read`, `list`, ...) work as written, and `bash`/`grep` resolve to the same tools. `any` (any case) lifts the restriction entirely. A retired spelling that names a *different* tool, or a typo, matches nothing and is reported, naming the entry and the skill

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

### AgentSkills Packs

ragent also loads the AgentSkills convention: a directory of packs under
`.agents/skills/` (project) or `~/.agents/skills/` (user), each pack a directory
holding a `SKILL.md` whose YAML frontmatter declares at least `name` and
`description`:

```bash
mkdir -p .agents/skills/tiny
cat > .agents/skills/tiny/SKILL.md << 'EOF'
---
name: tiny
description: A trivial pack used by the manual test plan.
---

# Tiny skill

When asked, reply with exactly: TINY-OK
EOF
```

List the registered packs with `/skills` (or `/skill`), and load one explicitly
with `/skill <name> [args]` (or the bare `/<name>` trigger):

```
/skill
/skill tiny
```

On a name clash the existing higher-scope pack wins, so an AgentSkills pack never
shadows a same-named `.ragent/skills/` pack. Reload after edits with
`/reload skills`.

A skill's `allowed-tools` frontmatter narrows the run's tool surface: entries
resolve against the registry ignoring case (`bash` names `Bash`), a single `any`
entry lifts the restriction entirely, and a retired or misspelt name is reported
by name and skill rather than silently removing the capability. See
[`docs/howtos/skills.md`](docs/howtos/skills.md) for the full authoring guide.

---

## 7b. Plugins

ragent can load third-party plugins written for OpenAI Codex and Claude
Code/Desktop, running their JavaScript on an embedded, budget-sandboxed engine
(no Node.js, Deno, or Bun required). Plugins contribute tools (registered as
`plugin_<id>_<tool>`) and slash commands through a versioned `ragent` host API.
The `plugin_<id>_` namespace keeps a plugin's own tool - even one it calls
`Bash` or `Edit` - from occupying a name ragent owns or a spelling the tool
rename retired; the bridge reports such a contribution once, names the
replacement, and keeps the plugin's other tools working.

Manage plugins from the TUI or the CLI:

| Command | Description |
|---|---|
| `/plugins list [--verbose]` | Open the interactive plugin panel: the table is pinned at the top (a fixed column heading and separator) above a scrollable list, and the capability blocks (contributions, unsupported capabilities, errors, summary) are listed at the end of the same body. `Up`/`Down`/`Home`/`End` navigate, `Space` toggles the highlighted plugin's enabled state (enable/disable), `Esc` closes; `--verbose` appends per-plugin telemetry counters |
| `/plugins add <source> [--force]` | Install from a local directory, `.zip`/`.tar.gz`, an `https://` archive URL, or a `git+<https-url>#<ref>[:<subpath>]` git source (enabled and loads at the next session start) |
| `/plugins remove <pluginid>` | Uninstall a plugin (refused while enabled) |
| `/plugins enable <pluginid>` | Enable, load, and register its tools/commands |
| `/plugins disable <pluginid>` | Unload and deregister without deleting files |
| `/plugins test <pluginid>` | Load in an isolated harness and invoke each tool once |
| `/plugins stores [--check]` | Report each store's effective endpoint and its source; `--check` also contacts each store and reports availability and plugin count |
| `/plugins marketplace show` | List the marketplaces configured for both stores, one section per store (Codex, Claude) with a table of each marketplace's name, url, and description |
| `/plugins marketplace <store> list` | List one store's configured marketplaces (`<store>` is `codex` or `claude`) |
| `/plugins marketplace <store> add <source> [--force]` | Add a marketplace to a store from an `owner/repo` slug, an SSH remote, an `https://` URL, or a local directory; written to the user-global `plugins.stores.marketplaces` and used by the next `/plugins <store>` browse (`--force` replaces an existing marketplace of the same name) |
| `/plugins marketplace <store> remove <name>` | Remove an added marketplace from a store |
| `/plugins codex [query] [--category <name>] [--refresh]` | Open the interactive Codex plugin-marketplace browser; `query` pre-fills the search field, `--category` pre-selects a left-hand category row, and `--refresh` bypasses the store cache. Each row shows the plugin id, version, the marketplace it came from, and the description (rows are sorted alphabetically by plugin id); the panel widens up to 140 columns, and a search box on its bottom edge carries the live field plus an `[x] clear` button that wipes the query on click |
| `/plugins claude [query] [--category <name>] [--refresh]` | Open the interactive Claude plugin-marketplace browser, with the same `query`, `--category`, and `--refresh` arguments (and the same per-row marketplace column) |
| `/plugins help` | Show the usage block |

The same operations are available from a shell as `ragent plugins <sub> …`.
Plugins are discovered under `.ragent/plugins/` (project) or
`~/.config/ragent/plugins/` (user-global). Set `plugins.enabled: false` to make
the whole subsystem inert.

An enabled plugin's slash commands appear in the `/` menu and in `/help`. Besides
the inline commands a plugin registers through the host API, ragent reads the
Claude-dialect `commands/*.md` prompt files (for example the official
`commit-commands` plugin's `/commit`, `/clean_gone`, and `/commit-push-pr`): each
command's markdown body is injected into the session as a user turn, with its
arguments substituted for `$ARGUMENTS`/`$N`. A command whose name collides with a
built-in, a skill, or another plugin registers under the namespaced
`plugin:<plugin-id>:<name>` trigger instead of shadowing it.

Besides tools and commands, an enabled plugin's other contribution surfaces are
bridged into the session:

- **skills** (FR-029) - a declared `skills` directory joins skill discovery;
- **MCP servers** (FR-030) - a declared `mcpServers` entry connects as
  `<plugin-id>.<server>`;
- **agents** (FR-032) - a plugin's `agents/*.md` profiles join agent discovery, so
  they appear in the agent picker and are spawnable via `new_agent`. Both ragent's
  JSON frontmatter and the Claude-dialect YAML frontmatter are accepted, and a
  project agent of the same name wins a clash;
- **hooks** (FR-033) - a plugin's declared `hooks` fire on the matching session
  lifecycle events (session start/end, turn start/end, compaction, pre/post tool
  use). The declaration is read both inline and from a `hooks.json` file (the
  plugin root or `hooks/`), including the Claude group shape
  `{matcher, hooks: [...]}`: a `matcher` selects the tools a hook applies to
  (`Edit|Write`, or a `Bash(git commit:*)` prefix guard). A plugin hook receives
  `CLAUDE_PLUGIN_ROOT` and the Claude event JSON on stdin, a `PostToolUse`
  `additionalContext` is folded into the tool result, and a blocking Stop hook
  can feed findings back to the model before the turn ends. The trigger name
  accepts both ragent's `pre_tool_use` and the Claude `PreToolUse` spellings, and
  configured `ragent.json` hooks run first at a shared trigger.

---

## 7c. Connectors

Connectors are named integrations (Google Drive, Slack, GitHub, Git, Postgres,
Puppeteer, and the wider community catalogue) that reach an external system
through one or more **MCP servers** — the transport ragent already speaks. A
connector is a catalogue entry carrying a display name, a category, an auth
shape, and one or more servers; adding one installs the entry, and enabling it
connects its servers and exposes their tools to the model. The system is
specified in [`specs/connectors/SPEC.md`](specs/connectors/SPEC.md) (FR-001..FR-041).

Manage connectors from the TUI or the CLI:

| Command | Description |
|---|---|
| `/connectors list [--verbose] [--category <name>]` | List installed connectors with state, auth state, category, and bridged server/tool counts; `--category` filters the rows (`ALL` clears the filter) |
| `/connectors claude [query] [--category <name>] [--refresh]` | Open the interactive Claude connector-catalogue browser, wearing the same design language as `/plugins claude` (`query` pre-fills the search field, `--category` pre-selects the filter, `--refresh` bypasses the cache; a calm slate accent frame and structured title; a left column with a `Categories` navigator above a `Marketplace` navigator, each row labelled `Name (N)`, the category navigator listing `ALL`, `Installed`, then the catalogue's categories; a pinned dark-on-light column-heading row above zebra-banded rows whose description runs to the panel edge and whose installed rows carry a bold green `*`; a centred footer and a bottom ` search: ` box with `[x] clear`; three focusable panes cycled by `Tab`/`BackTab`, `Up`/`Down` moving the focused pane and applying a navigator row, `Enter` applying and returning, `c` clearing; click a navigator row or wheel over it, click `[x] clear` to wipe the query; blank descriptions are filled from each entry's manifest off-loop; the base catalogue and every added marketplace are merged) |
| `/connectors add <id\|source> [--force]` | Install a catalogue id, a local directory, a local `.zip`/`.tar.gz`, or an `https://` package URL (recorded **enabled**) |
| `/connectors remove <id>` | Uninstall a connector (refused while enabled) |
| `/connectors enable <id>` | Enable a connector and connect its servers now |
| `/connectors disable <id>` | Disable a connector and disconnect its servers |
| `/connectors connect <id>` | Connect an enabled connector's servers without a restart |
| `/connectors disconnect <id>` | Disconnect a connector's servers, leaving it enabled |
| `/connectors auth <id>` | Manage a connector's credential and report its auth state |
| `/connectors test <id>` | Connect in isolation, invoke one tool, and report per-step results |
| `/connectors stores [--check]` | Report each catalogue endpoint and its source; `--check` probes it |
| `/connectors marketplace show` | List the marketplaces configured for the Claude catalogue (base endpoint plus each added marketplace, tagged `default`/`config`/`added`) |
| `/connectors marketplace claude list\|add <source> [--force]\|remove <name>` | List, add, or remove an additional catalogue marketplace (an `owner/repo` slug, a GitHub SSH remote, an `https://` URL, or a local directory), written to the user-global `connectors.stores.marketplaces` |
| `/connectors help` | Show the usage block |

The same operations are available from a shell as `ragent connectors <sub> …`.
Connectors are discovered under `.ragent/connectors/` (project) or
`~/.config/ragent/connectors/` (user-global). Their secrets live in the encrypted
credential store — the manifest stores only the credential *name*. Set
`connectors.enabled: false` to make the whole subsystem inert. See
[`docs/howtos/slashcommands/connectors.md`](docs/howtos/slashcommands/connectors.md)
and [`docs/howtos/config.md`](docs/howtos/config.md) §7.39.

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
Use the `new_agent` tool to spawn a background agent while the parent continues processing.

### Spawning Background Tasks

**Via Agent Tool (in chat):**

The agent can spawn background tasks automatically:
```
User: Analyze the codebase in parallel — run explore to find patterns,
      and build to check for compilation errors.

Agent uses:
  /new_agent agent="explore" task="Find common patterns in src/ and list them"
  /new_agent agent="build" task="Run cargo check and list any errors" background=true
```

**Via TUI Commands:**

```
/agents     # Open the Agents panel to see running background tasks
/cancel abc # Request cancellation of a sub-agent by task-id prefix
/task list  # List session tasks (the task-service equivalent of old todos)
/spawn explore "Find every call site of resolve_agent"   # detached, fire-and-forget
```

`/cancel <prefix>` asks the task registry to cancel the matching task; it does not
just hide the row. The task keeps running until its agent loop reaches the next step
boundary, and the Agents window updates when it settles. A prefix that matches more
than one live task is refused and every match is listed, so re-run it with a longer
prefix; a prefix that matches nothing lists the live task ids.

`/spawn <agent> <prompt...>` launches a **detached** sub-agent straight from the
input line. It runs concurrently and appears in the Agents panel, but nothing ever
waits on it: it is absent from `list_agents`, cannot be awaited with `wait_agents`,
and its result is never injected back into the chat. Use it for side-effect-only
work (writing a file, refreshing the index). Because nothing reads the reply body,
give the prompt a file to write; every completed sub-agent (detached or not) also
persists its full output to `log/subagents/<task-id>.md`. `/spawn` shares one
bounded launch set with every other TUI launch path (`agents.max_launching`,
default 4), so a second launch queues behind the busy ones and names them rather
than being refused outright.

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
    "max_background_agents": 8,       // Max concurrent background tasks (default: 8)
    "background_agent_timeout": 3600  // Timeout in seconds (default: 1 hour)
  }
}
```

### Result Injection

When a background task completes, the result is automatically injected into the parent
session as a system message, allowing the agent to act on it in the next iteration.

A **detached** task (`/spawn`, or `new_agent` with `detached: true`) is the exception:
its completion is reaped but nothing is injected into the chat, and the full output is
written to `log/subagents/<task-id>.md` for retrieval with the `read` tool.

---

## 9. Sessions

Ragent persists conversations in sessions stored in SQLite.

```bash
# List all sessions
ragent session list

# Resume a previous session (loads message history into the TUI)
ragent session resume <session-id>

# Export a session to a portable archive (default: session_archive.tar.gz)
ragent session export-archive <session-id> -o session_archive.tar.gz

# Import a session from an archive written by export-archive
ragent session import session_archive.tar.gz
```

### Pruning an old transcript

A session's messages can be aged out without deleting the session. The
operation is the same in the TUI and on the CLI; the session row survives, so an
expired conversation stays listed and resumable with an empty transcript.

```bash
# Report how many messages are older than 30 days - deletes nothing
ragent db prune --days 30 --dry-run

# Delete messages older than the configured window
# (storage.retention.message_days, default 14)
ragent db prune

# Delete messages older than 30 days
ragent db prune --days 30
```

```
/db prune --dry-run --days 30     # TUI: report the count, delete nothing
/db prune --days 30               # TUI: asks "Delete messages older than 30 day(s)?" first
/db help                          # TUI: subcommands, defaults, and caveats
```

`--days` accepts both `--days N` and `--days=N`; the value must be a positive day
count (a zero-day window would empty the store, so it is refused). The
confirmation dialog opens on `No`, so a stray `Enter` deletes nothing.

To run the same prune automatically at every start, enable retention in
`ragent.json`:

```json
{ "storage": { "retention": { "enabled": true, "message_days": 14 } } }
```

Retention is **off by default**; with no `storage` section no message is ever
expired. Deleting rows returns their pages to SQLite's freelist, so `ragent.db`
keeps its size until a `VACUUM` - which needs exclusive access, so stop every
other ragent process first.

---

## 10. HTTP Server Mode

Run ragent as a headless API server for IDE integrations or remote use:

```bash
# Start the server (prints a bearer token for auth)
ragent serve

# With a custom bind address (default 127.0.0.1:3000)
ragent serve --addr 127.0.0.1:8080
```

### API Endpoints

| Method | Path                                     | Description                    |
|--------|------------------------------------------|--------------------------------|
| GET    | `/health`                                | Health check (no auth)         |
| GET    | `/openapi.json`                          | OpenAPI 3.1 document (no auth) |
| GET    | `/config`                                | Get resolved configuration     |
| GET    | `/providers`                             | List providers and models      |
| POST   | `/sessions`                              | Create a new session           |
| GET    | `/sessions`                              | List all sessions              |
| GET    | `/sessions/{id}`                         | Get session details            |
| POST   | `/sessions/{id}/messages`                | Send a message                 |
| GET    | `/sessions/{id}/messages`                | Get session messages           |
| POST   | `/sessions/{id}/abort`                   | Abort session (archives and publishes `SessionAborted` event) |
| POST   | `/sessions/{id}/permission/{req_id}`     | Reply to a permission request  |
| GET    | `/events`                                | SSE event stream (at most 64 concurrent connections; further connects get `503`) |
| GET    | `/v1/models`                             | OpenAI-compatible model list   |
| POST   | `/v1/chat/completions`                   | OpenAI-compatible chat completion (stream:false JSON; stream:true SSE) |

The full REST surface is described by an OpenAPI 3.1 document served at
`/openapi.json` (public — it carries no runtime data or secrets). It is rendered
from a single route table so it cannot drift from the served routes, and the same
table generates a dependency-free typed TypeScript client checked in at
[`docs/openapi/ragent-client.d.ts`](docs/openapi/ragent-client.d.ts):

```bash
ragent openapi              # print the OpenAPI 3.1 document
ragent openapi --client     # print the generated typed TypeScript client
```

All endpoints except `/health` and `/openapi.json` require the bearer token:

```bash
curl -H "Authorization: Bearer <token>" http://localhost:3000/providers
```

Rate limit: 60 requests per minute per session on the messages endpoint.

MCP servers come in three transports — `stdio` (a child process), `sse`, and
`http` (Streamable-HTTP request/response). An `http` server connects over plain
HTTP alongside stdio:

```json
{
  "mcp": {
    "my-http-server": { "type": "http", "url": "http://127.0.0.1:3000/mcp" }
  }
}
```

`type` is optional and defaults to `stdio`, matching every other MCP client, so
a command-launched server needs no transport key:

```json
{
  "mcp": {
    "my-stdio-server": { "command": "npx", "args": ["-y", "some-mcp-server"] }
  }
}
```

A config that is written back still carries an explicit `"type": "stdio"`.

### ACP server for IDE clients (`ragent acp-server`)

ragent can serve the Agent Client Protocol (ACP) over stdio so an ACP-capable
editor (Zed, VS Code, JetBrains) can attach to it. This is **feature-gated and
disabled by default** — build with `--features acp-server` and set
`acp.server_enabled: true`:

```json
{
  "acp": {
    "server_enabled": true,
    "server_agent": "general",
    "agents": { "claude": { "id": "claude", "command": "claude-code-acp" } }
  }
}
```

```bash
cargo build --features acp-server
ragent acp-server            # editor attaches over stdin/stdout
```

When either gate is closed the command exits with a usage error (code 2) rather
than serving. The endpoint handles `initialize`, `session/new`, `session/prompt`,
and `session/cancel`, streaming the driven session's text, reasoning, and tool
calls back as ACP `session/update` notifications.

### Execution backends (`/backend`)

A pluggable execution backend selects **where** a session's tools run. The
default is `local` (the host, exactly as before); `docker`/`podman` run them
inside a per-session container, and `remote` drives a second ragent server over
REST+SSE and mirrors its events locally.

```
/backend            # open the switcher: active backend + each backend's health
/backend podman     # switch directly to a backend by kind or id
/backend help
```

The container workspace is a **named volume, never a host bind mount**, so a
sandboxed tool cannot read or write the host working directory; `Bash` is still
gated by the full 7-layer host validator, and a provisioning failure fails the
turn instead of falling back to the host. Configure backends under the
`execution_backend` / `backends` keys (see the example `ragent.json` above).
`ragent config` prints the resolved registry.

### LLM security analyzer (`/security`)

An opt-in mode that asks an LLM to judge each proposed tool action and returns an
`allow` / `ask` / `deny` verdict with a rationale, published *before* the action
runs. It only ever **tightens** the static rules: a `deny` is hard, an `allow`
never overrides an explicit policy `deny`, and any analyzer failure (missing
model, provider error, timeout) degrades to the normal interactive prompt.

```
/security on        # enable and persist security_analyzer.enabled: true
/security off       # disable
/security status    # current state and the model used
/security revert    # clear the setting
/security help
```

### OpenAI-compatible surface

When enabled, `ragent serve` exposes `POST /v1/chat/completions` and
`GET /v1/models` behind the native bearer token, bridging to the same agent loop
(permission checks preserved):

```bash
curl -s http://127.0.0.1:3000/v1/chat/completions \
  -H "Authorization: Bearer <TOKEN>" -H "Content-Type: application/json" \
  -d '{"model":"<MODEL>","stream":false,"messages":[{"role":"user","content":"say hello"}]}'
```

`stream:false` returns an OpenAI `chat.completion` object; `stream:true` returns a
`chat.completion.chunk` `text/event-stream` ending in `data: [DONE]`. Enabling the
surface without a token makes `ragent serve` refuse to start it rather than serve
it unauthenticated.

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

### Scaffold a New Project

The `/new` command (TUI) and `ragent new` (CLI) scaffold a brand-new,
agent-friendly project in an **empty** directory:

```bash
mkdir my-project && cd my-project

# TUI
/new --language rust --type cmdline

# or CLI
ragent new --language rust --type cmdline
```

The scaffold generates the ragent workspace (`.ragent/`, `specs/`, `log/`,
`.gitignore`, `AGENTS.md`), a runnable hello-world artifact set for 26
application languages (`rust`, `python`, `go`, `typescript`, `shell`, ...) in
`library`/`cmdline`/`tui`/`gui`/`webapp` layouts plus sample-document stubs for data,
markup, and build formats (`json`, `yaml`, `sql`, `cmake`, `maven`, ...) —
run `/new help` for the full list —
starter documentation (`README.md`, `QUICKSTART.md`, `STATS.md`, `docs/`),
and a git repository with an initial commit. Add `--github` or `--gitlab`
(mutually exclusive) to also create a private hosting repository and push;
add `--stack axum`/`--stack gtk4`/`--stack ratatui` (Rust) to layer a framework starter on top. In the TUI the
scaffold steps stream into the message window as they complete.

```bash
# Rust web service starter, pushed to GitHub
ragent new --language rust --type cmdline --stack axum --github
```

See [`docs/howtos/newproj.md`](docs/howtos/newproj.md) for the full manual.

### Research → Spec → Implement

The `ragent research` workflow lets you gather information on a topic
before turning it into a spec. The new iterative engine decomposes the topic
into sub-questions, gathers sources for each one in parallel, verifies claims,
detects missing links, and stops adaptively.

Run any of these from the TUI prompt:

```text
/research create rust-async async/await idioms in stable Rust
/research create rust-async async/await idioms --depth deep --iterations 5
/research create rust-async --from-url https://example.com/article
/research continue rust-async focus on io_uring integration
/research create fireworks "Compare Fireworks AI, Together.ai and Groq" --mode competitive   # --format comparison-table is implied
/research create rust-async "async/await idioms" --mode supervisor --summarization-model ollama:phi4 --max-concurrent-research-units 3
/research create rust-async "async/await idioms" --evaluate
/research create brief "Vector databases" --max-concepts 3 --max-findings 10
/research create local-only "Project error handling" --use-local --papers
/research create vendors "Managed vector db vendors" --url-cloak
/research create llm-pricing "Current frontier model pricing" --age 30d
/spec create async-await Add async/await ergonomics --from-research rust-async
/spec specify async-await Add async/await ergonomics --from-research rust-async
/spec plan async-await "Rust 2024 edition, tokio runtime"
/spec tasks async-await
/spec feedback async-await "Users report panics on drop"
/spec jtbd async-await
/spec update async-await
```

`/spec specify <specname> <feature> [--from-research <name>]` generates
`SPEC.md` only — an EARS spec with requirements and `[NEEDS CLARIFICATION]`
markers. It does NOT generate `PLAN.md`; use `/spec plan` after clarification.
When `sdd.branch_per_spec` is enabled, it also creates a `spec/<specname>` git
branch. `--from-research` links the spec to a research artifact via YAML
frontmatter and adds a `## Related Research` section to the SPEC.md body.

`/spec plan <spec-id> <tech-context>` generates (or regenerates)
`specs/<spec-id>/PLAN.md` from the existing `SPEC.md` using the provided
technology context as guidance. Production feedback notes from `FEEDBACK.md`
are automatically surfaced in the plan prompt when the `sdd.feedback_loop`
config flag is enabled.

`/spec tasks <spec-id>` generates `specs/<spec-id>/TASKS.md` (an ordered task
list derived from `PLAN.md`) and `specs/<spec-id>/quickstart.md` (key
validation scenarios derived from `SPEC.md` acceptance criteria). Both are
deterministic extractions — no LLM call required.

`/spec feedback <spec-id> <note>` appends a production feedback note to
`specs/<spec-id>/FEEDBACK.md`. Notes are advisory and surfaced during
`/spec plan` regeneration so real-world issues inform plan updates.

`/spec update <specname>` re-reads the existing `specs/<specname>/SPEC.md`
and regenerates `PLAN.md` and `TESTPLAN.md` to match the current requirements.
The `SPEC.md` file is not modified; existing task IDs in `PLAN.md` are preserved
where unchanged. Archived specs cannot be updated.

`/spec create <name> <title>` also generates a `TESTPLAN.md` manual test
plan (with `TC-NNN` test cases) alongside `SPEC.md` and `PLAN.md`.

`/spec jtbd <specname>` performs a Jobs-To-Be-Done analysis of an existing
spec's `SPEC.md` and writes `JTBD.md` in the same spec folder. Use `--force`
to overwrite an existing `JTBD.md`, and `--agent <name>` to dispatch to a
specific agent instead of the default explore agent.

Or from the CLI:

```bash
# Flags go BEFORE the name: everything after the name is taken as topic text.
ragent research create rust-async "async/await idioms in stable Rust"
ragent research create --depth deep --format executive-summary rust-async "async/await idioms"
ragent research create --from-urls https://example.com/article rust-async
ragent research continue rust-async "focus on io_uring integration"
ragent research create --mode competitive fireworks "Compare Fireworks AI, Together.ai and Groq"   # --format comparison-table is implied
ragent research create --mode supervisor --max-concurrent-research-units 3 rust-async "async/await idioms"
ragent research create --max-concepts 3 --max-findings 10 brief "Vector databases"
ragent research create --use-local --papers local-only "Project error handling"
ragent research create --url-cloak vendors "Managed vector db vendors"
ragent research create --max-age 30d llm-pricing "Current frontier model pricing"
ragent research list          # aligned table; add --json for machine-readable output
ragent research open rust-async
ragent research search "async"
ragent research show rust-async
ragent research archive rust-async
ragent research delete rust-async --yes
```

The HTTP API exposes the same surface at `GET /research`, `POST /research`,
`GET /research/<name>`, and `DELETE /research/<name>` (auth-protected).
`POST /research` returns `202 Accepted` with a `Location` header pointing to
`GET /research/<name>/events` (SSE stream of live research events).
`GET /research/<name>` supports `?full=true` to include `topic`, `queries`,
`output_format`, `model`, `mode`, `summarization_model`, `evaluate`, and `brief` metadata.

A run caps its `## Concepts` and `## Findings` sections at 5 and 20 entries by
default, ordering each most-relevant-first (highest cited source rank, then
cited count) before truncation. Override per run with `--max-concepts N` /
`--max-findings N` (also accepted as `max_concepts` / `max_findings` on
`POST /research`) or persistently via the `research.max_concepts` /
`research.max_findings` config keys; `0` means unbounded. Scholarly engines are
excluded from research runs by default; pass `--papers` to include
academically-classified engines (OpenAlex) in the search sweep
(`research.exclude_academic_engines` persists the exclusion), and
`--oa-enable` / `--no-oa` toggle open-access recovery for the run.

Select the artifact with `--format report|executive-summary|comparison-table|
source-bibliography|imrad|newsletter`. `newsletter` is the reader-facing
layout: it carries only the topic, executive summary, top 10 implications, open
questions, concepts, findings and the findings relationship diagram, and omits
the corpus quality scoreboard, research brief, search-query/engine/provider
blocks, data-quality sections, cross-references and references index. A report
whose synthesis came from the model is named by the LLM from its executive
summary (seeded with the topic, capped at 80 characters) rather than from the
first 80 characters of that summary.

Pass `--url-cloak` to write web source URLs as defanged plain text
(`hxxps://host[.]tld/…` in a code span) in the `Sources` bullets and the
`References Index` table instead of clickable links, so automated URL scanners
do not flag or reject the document. Non-URL rows are unchanged.

Pass `--age <SPAN>` to reject web sources published longer ago than the span,
where `SPAN` is `[n]d` days, `[n]m` months (30 days each) or `[n]y` years (365
days each) — `--age 45d` for the last six weeks, `--age 6m` for the last half
year. A source that carries no publication date is rejected as well, because
you asked for recent material and an undated page cannot be shown to satisfy
that. The window's start date (`today - span`, as ISO `YYYY-MM-DD`) is appended
to every decomposed search query as `after:YYYY-MM-DD`, so the engines steer
towards material inside the window rather than the run discarding an all-old
result set only after fetching it; the progress table shows what it dropped in
its own `age` column. A malformed span is a usage error (exit 2), and
`POST /research` rejects one with 422 rather than silently running unfiltered.
The flag is recorded in frontmatter, so `/research update` replays it.

The `--tier` flag selects the analysis depth: `light` (minimal), `full`
(default), or `dissertation` (extended with depth investigations and
source tensions). The `--mode` flag selects the execution strategy:
`tiered` (default single pipeline), `supervisor` (parallel sub-topic
researchers coordinated by a lead supervisor), or `competitive`
(supervisor graph specialised for comparing entities). In `supervisor`
and `competitive` modes, `--max-concurrent-research-units N` caps the
number of parallel researchers (default 5). `--summarization-model` (TUI
`/research create` only)
selects a lightweight model for per-page webpage summaries, and
`--evaluate` (TUI `/research create` only) appends a deterministic quality scorecard to the final report.

The report is written to a **dated** file, `research/<name>/RESEARCH-<YYYY-MM-DD>.md`,
alongside the QA companion `CORPA-<YYYY-MM-DD>.md`. The primary report is named
for the item's creation date (so an item answers the same path before and after
its first run); a later run writes its own dated file. That is what makes a
re-run non-destructive: an item accumulates one report per run date instead of
overwriting the last one. An item created by an older ragent keeps its legacy
undated `RESEARCH.md` / `CORPA.md`, and every read surface resolves whichever
file exists (newest dated first, legacy last).

Every report frontmatter also records an `invocation` line — the
verbatim `ragent research create ...` command, `/research ...` TUI slash
command, or `POST /research` request summary that started the run. The
`/research update <name>` command (or `ragent research update <name>` /
`PUT /research/<name>`) reads that line and replays the exact run,
writing a fresh dated report and companion while the earlier run's report
stays on disk. The original invocation is preserved on
`research continue` re-runs.

See [`docs/howtos/research.md`](docs/howtos/research.md) for the full workflow guide.

### Spec from an Architecture Document (govcreate)

Author a draft spec (plus `PLAN.md`/`TESTPLAN.md`) directly from an
architecture document — a local folder/file or a public URL:

```bash
ragent spec govcreate payments-arch https://example.com/arch.html ./payments-svc
ragent spec govcreate records-arch ./docs/sad.md ./records --language python --type library
```

The run acquires the document, extracts its structure, and authors the spec
via the configured model (`--model provider/model`, else the stored
`selected_model`). A non-empty target folder is refused unless `--force` is
passed. In the TUI use `/spec govcreate ...` with live staged progress and
Escape to cancel. See `docs/howtos/spec.md` §5.19.

---

## 14. TUI Interaction

### Mouse Support

The TUI supports mouse interaction:

- **Scroll wheel** — scrolls the message pane or log panel.
- **Scrollbar drag** — click-and-drag the scrollbar track on either pane.
- **Text selection** — click-and-drag to select text in any pane.
- **Right-click** — opens a context menu for the current selection. Use it to
  **Copy**, **Cut**, or **Paste** in the input pane. In provider-setup dialogs
  right-click pastes into the active field.

Pasting via right-click, **`Ctrl+V`**, or a terminal bracketed-paste event all
behave the same way: carriage returns are stripped and any active selection is
replaced by the pasted text. Press **`Alt+V`** to paste an image from the
clipboard as an attachment.

### Bang Commands

Prefix a prompt with `!` to run a shell command directly. The command output
is sent to the model for review and error resolution.

```
! ls -la
! cargo nextest run --lib
! git status
```

This also works in headless `ragent run` mode:

```bash
ragent run "! cargo check"
```

### Slash Commands

Type `/` in the input to open an autocomplete menu:

| Command | Description |
|---------|-------------|
| `/agent [name]` | Switch agent (dialog or direct) |
| `/clear` | Clear message history |
| `/compact [help]` | Summarise and compact history |
| `/help` | Show available commands |
| `/log` | Toggle log panel |
| `/model` | Switch model |
| `/thinking auto|off|low|medium|high` | Switch reasoning level for the active model |
| `/provider` | Change provider |
| `/provider_reset` | Reset provider credentials |
| `/queue [list\|clear\|next\|help]` | Inspect and control the message input queue |
| `/quit` | Exit ragent |
| `/system <prompt>` | Override system prompt |
| `/prompt help\|primary [agent]\|subagent [agent]\|list\|<agent>` | Inspect the assembled agent system prompt (read-only, no LLM call) |
| `/bench list` | List available benchmark suites and profiles |
| `/bench init <suite-or-all-or-full>` | Create benchmark data roots in sample mode or full-download mode |
| `/bench run <suite-or-profile-or-all>` | Start a background benchmark run and write workbook output |
| `/bench status` | Show active or last benchmark run status |
| `/bench open last` | Show the latest benchmark workbook path(s) and summary |
| `/bench cancel` | Cancel the active benchmark run |
| `/codeindex on` | Enable codebase indexing |
| `/codeindex off` | Disable codebase indexing |
| `/codeindex show` | Show index status and statistics (incl. graph; non-blocking, reports busy immediately) |
| `/codeindex reindex` | Trigger a full re-index (runs in the background; status bar animates `idx`) |
| `/codeindex lang <language>` | Filter code index by language |
| `/codeindex graph build` | Build the semantic edge graph (background thread; status bar animates `graph`) |
| `/codeindex graph lang <language>` | Build graph for a single language |
| `/codeindex graph export` | Export the graph in DOT/JSON format |
| `/codeindex explain <symbol>` | Show symbol node metadata and connections |
| `/codeindex path <A> <B>` | Find shortest path between two symbols |
| `/codeindex communities` | Run community detection and list communities |
| `/codeindex godnodes [n]` | Show top-N most-connected symbols |
| `/codeindex help` | Show code index help |
| `/context` | Toggle the `Alt+C` context panel (live context-window breakdown) |
| `/cron add \|remove\|enable\|disable\|list\|detail\|log\|help` | Schedule and manage recurring agent runs |
| `/spec create <name> <title> [--from-research <name>]` | Create a new spec (SPEC.md + PLAN.md + TESTPLAN.md) |
| `/spec specify <name> <feature> [--from-research <name>]` | Generate SPEC.md only (EARS spec with clarification markers) |
| `/spec plan <name> <tech-context>` | Generate PLAN.md from existing SPEC.md with tech context |
| `/spec tasks <name>` | Generate TASKS.md + quickstart.md from PLAN.md and SPEC.md |
| `/spec update <name>` | Regenerate PLAN.md and TESTPLAN.md from an edited SPEC.md |
| `/spec add <name> <requirement>` | Add an incremental requirement to an existing spec |
| `/spec feedback <name> <note>` | Append a production feedback note to FEEDBACK.md |
| `/spec jtbd <name> [--force] [--agent <name>]` | Perform JTBD analysis on an existing spec |
| `/spec list \|search \|show \|validate \|status \|task` | Spec lifecycle commands |
| `/new --language <lang> --type <type> [--stack <name>] [--github\|--gitlab]` | Scaffold a new project in the current directory (46 canonical languages spanning the codeindex scanner set, from rust/python to json/sql/cmake; library/cmdline/tui/gui/webapp; optional GitHub/GitLab hosting + push) |
| `/new help` | Show `/new` usage, flag table, and supported values |
| `/spawn <agent> <prompt...>` | Launch a **detached** fire-and-forget background sub-agent from the chat input (not listed by `list_agents`, not awaitable, result never injected back); `/spawn help` for usage |

### Benchmark Workflow

## Cron Scheduling

Ragent supports scheduling agent runs with a cron-like system. Use the
`/cron` slash command to add, remove, list, and view scheduled events.

### Schedule Forms

| Form | Behaviour | Example |
|------|-----------|--------|
| `at <timestamp>` | One-shot — fires once at the specified time | `/cron add nightly general at 2025-01-15T09:00 "Run tests"` |
| `from <timestamp> every <duration>` | Repeating — first fire at the timestamp, then every duration | `/cron add nightly general from 2025-01-15T09:00 every 30m "Run tests"` |
| `every <duration>` | Repeating — first fire is duration from now | `/cron add nightly general every 2h "Run tests"` |

Durations use a positive integer + unit: `m` (minutes), `h` (hours),
`d` (days), `w` (weeks), `mo` (months = 30 days). Plural aliases
(`mins`, `hrs`, `days`, `wks`, `months`) are also accepted.

Timestamps are ISO-8601 (e.g. `2025-01-15T09:00:00Z` or
`2025-01-15T09:00:00+02:00`).

### Commands

| Command | Description |
|---------|--------------|
| `/cron add <cronname> <agent> <schedule> "<prompt>"` | Schedule a new event (`cronname` becomes the event ID) |
| `/cron remove <event_id>` | Remove a scheduled event |
| `/cron enable <event_id>` | Enable a previously disabled event |
| `/cron disable <event_id>` | Disable an event |
| `/cron detail <event_id>` | Show full details of a single event |
| `/cron list` | List all events with human-readable schedules |
| `/cron log [event_id]` | Show execution log (optionally filtered by event id) |
| `/cron help` | Show usage |

### How It Works

A background scheduler runs while the TUI session is active, ticking every
30 seconds. When an event's `next_due` time has passed, the scheduler
spawns a background agent run of the configured agent type with the
configured prompt. For repeating events, `next_due` is advanced by one
duration interval. For one-shot events, the event is marked as fired.

Every execution is logged as a JSONL line to
`<working_dir>/log/cron-<timestamp>.jsonl`, recording the event id,
agent type, prompt, outcome (`"success"`, `"error"`, or `"skipped"`),
and timestamp.

Disabled events are skipped with a `"skipped"` outcome. Past-start
timestamps for `from <ts> every <d>` are advanced by whole duration
intervals until `next_due` is in the future.

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
| `Alt+C` | Toggle the live context-window panel |
| `Alt+Q` | Open the queue-control menu (`Next` / `Stop`/`Resume` / `Clear` / `Show`) |
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

## Automation Service

The automation service runs a named agent on a **schedule** or from a
**webhook**, confined to a chosen execution backend, with durable run history
(spec `openhands` FR-013, FR-014, FR-018, FR-033).

Declare automations under the `automation` block in `.ragent/ragent.json`:

```jsonc
{
  "automation": {
    "enabled": true,
    "automations": [
      {
        "id": "on-issue",
        "agent": "general",
        "prompt": "Triage issue {{payload}}",
        "trigger": { "kind": "webhook" },
        "backend": "local"
      },
      {
        "id": "nightly",
        "trigger": { "kind": "schedule", "schedule": "every 2m" },
        "backend": "local"
      }
    ]
  }
}
```

The service starts only when `automation.enabled` is `true`; an absent block
leaves it inert.

### TUI and CLI

```
/automation                  # list automations, trigger, backend, next-due
/automation runs on-issue    # durable run history
/automation run nightly      # enqueue a manual run now
/automation help

ragent automation list
ragent automation runs on-issue
ragent automation run nightly
```

### HTTP

```sh
# Webhook ingress (public; no bearer token)
curl -s -X POST http://127.0.0.1:3000/auto/on-issue \
  -H "Content-Type: application/json" \
  -d '{"issue":"42","title":"fix login"}'
# → 202 {"run_id":"...","automation_id":"on-issue","trigger":"webhook","backend":"local"}

# Inspect run history (bearer-authenticated)
curl -s http://127.0.0.1:3000/automation/runs/on-issue \
  -H "Authorization: Bearer <token>"
```

A completed run can notify Slack, GitHub, Linear, Notion, or a generic webhook;
each dispatch target names the environment variable holding its credential
(`token_env`), so no secret is stored in `ragent.json`.

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
→ Run with `ragent --log-level debug run "..."` for verbose logging.

---

For full details, see [README.md](README.md) and [SPEC.md](SPEC.md).

## Research prompt configuration

The `/research create` synthesis prompt accepts optional `ragent.json`
keys under a top-level `research` object (see SPEC.md → "Research
Configuration" for the full schema):

- `research.models.research_model` / `compression_model` /
  `final_report_model`, `research.supervisor.max_concurrent_research_units`
  and `research.evaluate.enabled` set the default for every run. A per-run
  flag wins (`--research-model`, `--compression-model`,
  `--final-report-model`, `--max-concurrent-research-units`, and `--evaluate`
  on the TUI `/research create`). The per-page summarization model has no
  config key; pass `--summarization-model` to the TUI `/research create`
  command.

```jsonc
{
  "research": {
    "models": {
      "compression_model": "ollama:phi4",
      "research_model": "openrouter/anthropic/claude-sonnet-4-20250514",
      "final_report_model": "openrouter/anthropic/claude-sonnet-4-20250514"
    },
    "supervisor": {
      "max_concurrent_research_units": 3
    },
    "evaluate": {
      "enabled": true
    }
  }
}
```

All keys are opt-in. The prompt builder also
enforces a fifth **Sources Cited / Date Spread** paragraph and a
recency-weighting rule when the corresponding knobs are enabled, and falls
back to a deterministic mechanical extraction when the LLM response cannot be
parsed into the required structure (FR-005/FR-006).
