# Tools — Planning

Delegate read-only analytical work to the plan agent.

| Tool | Description |
|------|-------------|
| `plan_enter` | Delegate to the plan agent for read-only analysis. |
| `plan_exit` | Exit plan mode. |

**Visibility switch:** `plan`.

---

## plan_enter

Enter plan mode / delegate analysis to the plan agent. The plan agent is
read-only: it explores the codebase and produces a structured implementation
plan without making any changes.

**Arguments:** none required — pass the planning request via the session
prompt.

```text
plan_enter
```

---

## plan_exit

Exit plan mode and return to normal editing.

**Arguments:** none.

```text
plan_exit
```
