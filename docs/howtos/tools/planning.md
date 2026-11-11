# Tools — Planning

Delegate read-only analytical work to the plan agent.

| Tool | Description |
|------|-------------|
| `EnterPlanMode` | Delegate to the plan agent for read-only analysis. |
| `ExitPlanMode` | Return the plan result to the previous agent. |

**Visibility switch:** `plan`.

---

## EnterPlanMode

Enter plan mode / delegate analysis to the plan agent. The plan agent is
read-only: it explores the codebase and produces a structured implementation
plan without making any changes.

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `task` | string | yes | What to plan or analyse |
| `context` | string | no | Additional background for the plan agent |

```text
EnterPlanMode task="Plan the migration of the config loader to TOML" context="Keep JSON support"
```

---

## ExitPlanMode

Return control from the plan agent to the previous agent. Use it inside a plan
session once the requested analysis is finished; it does not modify files.

**Arguments**

| Argument | Type | Required | Description |
|----------|------|----------|-------------|
| `summary` | string | yes | The plan or analysis result to return to the calling agent |

```text
ExitPlanMode summary="Three-step migration plan: ..."
```
