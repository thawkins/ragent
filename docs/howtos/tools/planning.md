# Tools — Planning

Delegate read-only analytical work to the plan agent.

| Tool | Description |
|------|-------------|
| `EnterPlanMode` | Delegate to the plan agent for read-only analysis. |
| `ExitPlanMode` | Exit plan mode. |

**Visibility switch:** `plan`.

---

## EnterPlanMode

Enter plan mode / delegate analysis to the plan agent. The plan agent is
read-only: it explores the codebase and produces a structured implementation
plan without making any changes.

**Arguments:** none required — pass the planning request via the session
prompt.

```text
EnterPlanMode
```

---

## ExitPlanMode

Exit plan mode and return to normal editing.

**Arguments:** none.

```text
ExitPlanMode
```
