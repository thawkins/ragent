# Tools — Spec Management

Query specification lifecycle state and work through spec task plans.

| Tool | Description |
|------|-------------|
| `spec_read` | Read a specification by ID. |
| `spec_list` | List all specifications. |
| `spec_search` | Search specifications by keyword. |
| `spec_task_update` | Update a task's status within a spec. |
| `spec_coverage` | Generate a requirement coverage report. |

See `docs/howtos/spec.md` for the full spec workflow.

---

## spec_read

Read a specification: full `SPEC.md` content, requirements, tasks, and
current status.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `spec_id` | string | yes | Alphanumeric/hyphen/underscore identifier | `"auth-refactor"` |

---

## spec_list

List all specifications.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `status` | enum | no | Filter: `draft`, `in_review`, `approved`, `in_progress`, `implemented`, `verified`, `archived` | `"draft"` |

---

## spec_search

Case-insensitive substring search across spec titles and content (not
semantic).

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `query` | string | yes | Keyword to find |

---

## spec_task_update

Update a task's status within a spec; mirrors the session task tracker.

Marking a task `completed` writes an RFC 3339 UTC completion stamp into the
`PLAN.md` Status cell (`completed (2026-11-08T10:12:00Z)`) without adding a
column, so both 6- and 7-column task tables keep parsing. Moving the task to
any other status removes the stamp. If the write fails, the tool reports the
error rather than the new status.

**Arguments**

| Argument | Type | Required | Description | Typical value |
|----------|------|----------|-------------|---------------|
| `spec_id` | string | yes | Spec identifier | `"govdoc"` |
| `task_id` | string | yes | Task within the plan | `"T-001"` |
| `status` | enum | yes | `pending`, `in_progress`, `completed`, `blocked` | `"completed"` |

---

## spec_coverage

Show which spec requirements are linked to completed tasks; verify coverage
before marking a spec complete.

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `spec_id` | string | yes | Spec identifier |

**Example:**
```text
spec_read spec_id="govdoc"
spec_task_update spec_id="govdoc" task_id="T-014" status="completed"
spec_coverage spec_id="govdoc"
```
