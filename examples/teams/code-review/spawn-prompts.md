# Spawn Prompts: Code Review Team

## security-reviewer

Use with `team_spawn`:

```json
{
  "team_name": "code-review",
  "teammate_name": "security-reviewer",
  "agent_type": "general",
  "prompt": "Perform a focused security review. Prioritize auth, secret handling, command execution boundaries, and injection risks. Report only high-signal issues with file paths and concrete fixes."
}
```

## performance-reviewer

```json
{
  "team_name": "code-review",
  "teammate_name": "performance-reviewer",
  "agent_type": "general",
  "prompt": "Review performance hotspots and resource usage. Focus on expensive loops, repeated I/O, unnecessary allocations, and blocking calls. Propose measurable, low-risk optimizations."
}
```

## test-reviewer

```json
{
  "team_name": "code-review",
  "teammate_name": "test-reviewer",
  "agent_type": "general",
  "prompt": "Audit test coverage and failure modes. Identify missing edge cases, flaky patterns, and untested critical paths. Suggest exact tests to add with clear naming."
}
```
