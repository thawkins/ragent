# Tools — Utility

Introspection of the active LLM connection.

| Tool | Description |
|------|-------------|
| `model_info` | Report the active provider/model, capabilities, context window, and cost tier. |

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
