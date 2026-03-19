# Tool Call Information Display Improvements

## Overview

Added enhanced display information for tool calls in the TUI, particularly for sub-agent and task management tools that were previously showing no output summary.

## Changes Made

### 1. Tool Result Summary (`new_task.rs`)
Already provides complete metadata in `ToolOutput`:
```rust
metadata: Some(json!({
    "task_id": entry.id,
    "agent": entry.agent_name,
    "background": true,
    "status": "running"
}))
```

### 2. Tool Input Summary Display (`message_widget.rs`)
Added input summaries for the following tools:

#### Sub-Agent Tools
- **`new_task`**: Displays agent name and truncated task description
  - Input: `agent → <task summary>`
  - Example: `explore → Find all usages of EventBus in src/`

- **`cancel_task`**: Shows the task ID being cancelled
  - Input: `cancel task: <task_id (8 chars)>`
  - Example: `cancel task: 8b085577`

- **`list_tasks`**: Displays filter status
  - Input: `filter: <status>`
  - Example: `filter: running`

- **`wait_tasks`**: Shows count of tasks to wait on
  - Input: `wait on N task(s)` or `wait on all tasks`
  - Example: `wait on 3 task(s)`

#### LSP Tools
- **`lsp_definition`**, **`lsp_hover`**: Line and column numbers
  - Input: `line X, col Y`
  - Example: `line 42, col 15`

- **`lsp_references`**, **`lsp_symbols`**, **`lsp_diagnostics`**: File path
  - Input: `<relative path>`
  - Example: `src/main.rs`

### 3. Tool Output Summary Display (`message_widget.rs`)
Added result summaries for tool completion:

#### Sub-Agent Tools
- **`new_task`** (background): `spawned <agent> agent (<task_id>)`
  - Example: `spawned explore agent (8b085577)`

- **`new_task`** (sync): `<status> <agent> agent → <status> (<task_id>)`
  - Example: `completed explore agent → completed (8b085577)`

- **`cancel_task`**: `task cancelled` or `already completed`

- **`list_tasks`**: `N task(s)`
  - Example: `3 tasks`

- **`wait_tasks`**: `waited on N task(s)` or `timeout waiting for N task(s)`
  - Example: `waited on 3 task(s)`

#### LSP Tools
- **All LSP tools**: `N result(s)`
  - Example: `5 results`

## User Experience Impact

### Before
- Tool calls for `new_task`, `cancel_task`, `list_tasks`, etc. showed no inline information
- Users had to expand the message to see full details
- No quick visual feedback on task creation parameters

### After
- **Input line** shows what the tool is being called with
  - `new_task` displays agent name and task summary on one line
  - LSP tools show line/column or file path
  
- **Output summary** on completion line shows results
  - Background tasks show "spawned explore agent"
  - Task lists show count of tasks
  - LSP queries show result count
  
- **Better visual scanning**: Users can see task operations at a glance without expanding

## Example Output

```
Assistant: Let me check the codebase...

[sid:1] tool call: new_task (explore → Find all usages of EventBus in src/)
[sid:1] → new_task (spawned explore agent (8b085577))

[sid:1] tool call: wait_tasks
[sid:1] → wait_tasks (waited on 1 task(s))

[sid:1] tool call: lsp_references (src/main.rs)
[sid:1] → lsp_references (5 results)
```

## Files Modified

- `crates/ragent-tui/src/widgets/message_widget.rs`
  - `tool_input_summary()`: Added handlers for sub-agent and LSP tools
  - `tool_result_summary()`: Added handlers for sub-agent and LSP tools

## Notes

- All existing tool summaries continue to work unchanged
- The display respects the existing truncation and formatting rules
- Metadata from tool outputs drives the result summaries
- Background vs synchronous tasks show different status strings
