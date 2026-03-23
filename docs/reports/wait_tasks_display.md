# Wait_tasks Tool Display Enhancement

## Overview

Enhanced the `wait_tasks` tool to show a detailed indented list of sub-agent tasks and their completion status in the message window, with elapsed time and output line count for each task.

## Implementation

### Backend Changes: `wait_tasks.rs`

The `wait_tasks` tool now builds detailed metadata for each completed task:

```rust
task_details.push(json!({
    "id": &task.id,
    "agent": &task.agent_name,
    "status": if *success { "completed" } else { "failed" },
    "elapsed_ms": elapsed_ms,
    "output_lines": output_lines,
}));
```

**Metadata fields:**
- `id`: Task identifier (first 8 characters displayed)
- `agent`: Agent name that ran the task
- `status`: "completed" or "failed"
- `elapsed_ms`: Time in milliseconds from task creation to completion
- `output_lines`: Number of lines in the task's result output

### Frontend Changes: `message_widget.rs`

The message widget now renders `wait_tasks` completion as an indented list:

```
● [s1:2] Wait_tasks
  ✓ explore (8b085577): 2.5s, 42 line(s)
  ✓ build (a3d12f09): 5.1s, 128 line(s)
  ✗ plan (c2e8f401): 1.2s, 0 line(s)
```

**Display format:**
- Each task on its own indented line
- ✓ (checkmark) for completed tasks, ✗ (X) for failed tasks
- Color-coded: Green for completed, Red for failed
- Agent name and task ID (8 chars)
- Elapsed time in seconds (converted from milliseconds)
- Output line count

## Example Output

### Input
```json
{
  "task_ids": ["task-1", "task-2", "task-3"]
}
```

### Output Metadata
```json
{
  "completed": 3,
  "timed_out": false,
  "still_running": 0,
  "tasks": [
    {
      "id": "8b085577-e456-484a-bd99-916af402d46e",
      "agent": "explore",
      "status": "completed",
      "elapsed_ms": 2500,
      "output_lines": 42
    },
    {
      "id": "a3d12f09-f567-494b-ce0a-0272e5b351f0",
      "agent": "build",
      "status": "completed",
      "elapsed_ms": 5100,
      "output_lines": 128
    },
    {
      "id": "c2e8f401-96ab-5a5b-de1a-1383f6c262c1",
      "agent": "plan",
      "status": "failed",
      "elapsed_ms": 1200,
      "output_lines": 0
    }
  ]
}
```

### Display in TUI
```
● [s1:2] Wait_tasks
  ✓ explore (8b08557): 2.5s, 42 line(s)
  ✓ build (a3d12f0): 5.1s, 128 line(s)
  ✗ plan (c2e8f40): 1.2s, 0 line(s)
```

## User Benefits

1. **At-a-glance task status**: See all waiting tasks and their completion state without expanding
2. **Performance metrics**: Elapsed time shows task efficiency
3. **Output indication**: Line count helps assess result size
4. **Error visibility**: Failed tasks are clearly marked in red
5. **Quick identification**: Task IDs aid in debugging and logs

## Files Modified

- `crates/ragent-core/src/tool/wait_tasks.rs`: Added task detail collection to metadata
- `crates/ragent-tui/src/widgets/message_widget.rs`: Added special rendering for `wait_tasks` output

## Related Tools

This enhancement works seamlessly with:
- `new_task`: Spawns background tasks that `wait_tasks` monitors
- `list_tasks`: Lists all tasks; `wait_tasks` waits for completion
- `cancel_task`: Can cancel tasks before `wait_tasks` catches them

## Future Enhancements

Potential improvements:
1. **Real-time updates**: Update task status in the UI as tasks complete (not just at the end)
2. **Progress bars**: Show execution progress for long-running tasks
3. **Expandable output**: Click to expand individual task output
4. **Performance graphs**: Show timeline of task execution
5. **Timeout tracking**: Show remaining time for deadline-driven waits
