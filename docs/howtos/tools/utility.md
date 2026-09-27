# Tools — Utility

Introspection of the running ragent binary and the active LLM connection.

| Tool | Description |
|------|-------------|
| `model_info` | Report the active provider/model, capabilities, context window, and cost tier. |
| `ragent_info` | Report the running ragent version, build time, git commit, and compiler. |
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
