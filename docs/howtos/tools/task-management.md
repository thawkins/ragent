# Tools — Task Management

Session-scoped tasks for tracking multi-step work. `blocked` is a derived
state computed from `blocked_by` dependencies — it cannot be set directly.

| Tool | Description |
|------|-------------|
| `task_create` | Create a session-scoped task. |
| `task_update` | Update task status, subject, or dependencies. |
| `task_get` | Retrieve a single task by ID. |
| `task_list` | List all session tasks. |

---

## task_create

Create a task.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `subject` | string | yes | Imperative title | `"Fix config loader"` |
| `description` | string | yes | Acceptance criteria | `"Reproduce panic and fix root cause"` |
| `active_form` | string | no | Present-continuous phrase for progress indicators | `"Fixing config loader"` |
| `owner` | string | no | Agent/worker label | `"coder"` |
| `metadata` | object | no | Arbitrary key/value pairs | `{"phase":"2"}` |
| `blocked_by` | array | no | Task IDs that must complete first | `["task-001"]` |

---

## task_update

Update an existing task. `blocked` status is rejected — pass blockedness via
`add_blocked_by`. Completing a task auto-evaluates dependents.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `task_id` | string | yes | Task identifier | `"task-001"` |
| `status` | enum | no | `pending`, `in_progress`, or `completed` (never `blocked`) | `"in_progress"` |
| `subject` / `description` / `active_form` | string | no | New field values | — |
| `owner` | string | no | New owner label (empty string clears) | — |
| `metadata` | object | no | Full replacement metadata | — |
| `add_blocked_by` | array | no | Task IDs this task is now blocked by | `["task-002"]` |
| `add_blocks` | array | no | Task IDs that should be blocked by this task | `["task-003"]` |

---

## task_get

Retrieve the full record of one task: status, owner, metadata, dependencies,
timestamps.

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `task_id` | string | yes | Task identifier |

---

## task_list

List all session tasks ordered by creation time.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `status` | enum | no | `pending`, `in_progress`, `completed`, or `all` (default) | `"pending"` |

**Example:**
```text
task_create subject="Fix config loader" description="Reproduce panic and fix root cause"
task_update task_id="task-001" status="in_progress"
```
