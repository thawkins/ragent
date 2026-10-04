# Tools — Utility

Introspection of the running ragent binary, the active LLM connection, and the
host environment.

| Tool | Description |
|------|-------------|
| `model_info` | Report the active provider/model, capabilities, context window, and cost tier. |
| `ragent_info` | Report the running ragent version, build time, git commit, and compiler. |
| `os_info` | Report read-only host OS and hardware introspection (also `/osinfo show`). |
| `tool_info` | Dump the whole tool registry as JSON (name, description, schema, permission category, source). |
| `commands_info` | Dump the slash-command catalog as JSON (built-in TUI commands plus plugin-contributed ones). |

---

## model_info

Report the active provider/model pair, provider display name, capabilities,
context window, max output tokens, cost tier, and thinking support. When the
Model Router is active, also reports whether routing is enabled and that the
effective downstream model is chosen per request.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `format` | enum | no | `text` (human-readable markdown, default) or `json` (structured metadata only) | `"text"` |

**Example:**
```text
model_info
model_info format="json"
```

---

## ragent_info

Report build and version information about the running ragent binary: the ragent
version, the build timestamp, the git commit it was built from (best-effort), and
the compiler version. Read-only and offline — it never shells out or hits the
network — so the LLM can answer "which version am I running?" directly.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `format` | enum | no | `text` (human-readable markdown, default) or `json` (structured metadata only) | `"text"` |

**Example:**
```text
ragent_info
ragent_info format="json"
```

---

## os_info

Report read-only information about the host operating system and hardware: OS
identity and Linux distribution, CPU, graphics adapters and graphics-API
versions, physical hardware (system/chassis/motherboard/BIOS identity, storage
devices, network interfaces), memory, uptime, and the ragent process
environment. It always succeeds, reporting `unknown`/`0` placeholders for any
value the host cannot supply, and never reads serial numbers, UUIDs, or asset
tags. Also available as the `/osinfo show` slash command.

By default (`probe` = `true`) it runs a fixed, timeout-bounded allowlist of
vendor graphics diagnostics (`vulkaninfo`, `glxinfo`, `nvidia-smi`, `rocminfo`,
`system_profiler`) so every graphics API is reported with its version - OpenGL,
OpenGL ES, and Mesa expose no read-only version file. Those diagnostics only
read host state; set `probe: false` for a fully process-free pass.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `format` | enum | no | `text` (human-readable markdown, default) or `json` (structured metadata only) | `"text"` |
| `probe` | bool | no | Run the allowlisted graphics diagnostics (default `true`); `false` for a process-free report | `false` |

**Example:**
```text
os_info
os_info format="json" probe=false
```

---

## tool_info

Return a JSON-encoded dump of the tool registry: every registered tool with its
name, description, parameters JSON schema, permission category, source
(`internal` / `mcp:<server>` / `plugin:<id>` / `visibility:<family>`), and
hidden state, plus MCP server/tool provenance for bridged tools. Read-only,
permission category `none`, hardwired auto-approved.

**Arguments**

None.

**Example:**
```text
tool_info
```

---

## commands_info

Return a JSON-encoded catalog of every slash command — the built-in TUI
commands (trigger, description, subcommands, flags) plus the commands
contributed by enabled plugins, resolved live at call time. Read-only,
permission category `none`, hardwired auto-approved.

**Arguments**

None.

**Example:**
```text
commands_info
```
