# Tools — Initiatives

Durable, long-lived project goals with milestones that persist across sessions
and compaction.

| Tool | Description |
|------|-------------|
| `initiative` | Manage durable initiatives with milestones. |

---

## initiative

Create, read, update, checkpoint, and close initiatives.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `action` | enum | yes | `create`, `read`, `update`, `checkpoint`, `list`, `close` | `"create"` |
| `id` | string | all actions except `create`/`list` | Initiative id (`create`: optional short slug, auto-generated if omitted) | `"api-v2"` |
| `title` | string | yes for `create` | Short goal title | `"Ship API v2"` |
| `description` | string | no | Detailed description / success criteria | — |
| `milestones` | array | no | Milestone titles for `create` | `["design","implement","ship"]` |
| `milestone` | string | no | Milestone id to mark complete (`checkpoint`) | `"design"` |
| `progress` | integer | no | Overall progress 0–100 (`update`/`checkpoint`) | `60` |
| `note` | string | no | Free-text note recorded with a checkpoint | `"Design approved"` |
| `status` | enum | no | `active` (default), `paused`, `completed`, `abandoned`, `all`; filter for `list`, transition for `update`/`close` | `"active"` |
| `limit` | integer | no | Max initiatives returned by `list` | `50` |

**Example:**
```text
initiative action="create" id="api-v2" title="Ship API v2" milestones=["design","implement","ship"]
initiative action="checkpoint" id="api-v2" milestone="design" progress=40 note="Design approved"
initiative action="list" status="active"
```
