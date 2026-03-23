# EventBus as Single Source of Truth for Step Counts

## Overview

The system now uses the EventBus step count as the authoritative source for all step numbering in the UI. This eliminates synchronization issues between the Agents window and Message/Log windows.

## Changes Made

### 1. Removed Local Step Counter
**File**: `crates/ragent-tui/src/app/state.rs`

Removed the `tool_step_counter: u32` field from the `App` struct. This local counter was causing divergence from the authoritative EventBus counter.

### 2. Updated ToolCallStart Handler
**File**: `crates/ragent-tui/src/app.rs`

Changed from:
```rust
self.tool_step_counter += 1;
let step = self.tool_step_counter;
```

To:
```rust
// Get the current step count from the event bus (single source of truth)
let step = self.event_bus.current_step(session_id);
```

The step count now comes from `EventBus::current_step()` which is maintained by the backend session processor.

### 3. Session Restoration
**File**: `crates/ragent-tui/src/app.rs`

When loading a previous session, the step map is rebuilt from the message history, but step numbers are re-derived from sequential counting of tool calls, not persisted separately.

### 4. Message Clear Command
**File**: `crates/ragent-tui/src/app.rs`

Removed the reset of `tool_step_counter` from the `/clear` command since the counter no longer exists.

## Architecture

### EventBus Step Tracking
```
Backend (Session Processor)
  ↓
  Event::ToolCallStart published
  ↓
  EventBus.steps[session_id] incremented
  ↓
Frontend (TUI)
  ↓
  Receives event
  ↓
  Queries EventBus.current_step(session_id)
  ↓
  Uses step in tool_step_map for display
```

### Display Layers Using EventBus

1. **Message/Log Window**: Uses `tool_step_map` which stores EventBus step value
   - Shows `[short_session_id:step]` prefix

2. **Agents Window**: Queries EventBus directly
   - Shows `steps:<count>` from `event_bus.current_step(session_id)`

Both layers are now synchronized since they use the same authoritative source.

## Benefits

1. **Correctness**: All displays show the same step count
2. **Simplicity**: No complex synchronization logic needed
3. **Reliability**: EventBus counter is maintained by backend where session events originate
4. **Consistency**: Agents window and Messages window always match

## Implementation Details

### tool_step_map Structure
```rust
// Unchanged structure:
pub tool_step_map: HashMap<String, (String, u32)>,
//                                    (session_id, step)
```

The `u32` step number is populated from EventBus when the tool call starts, not from a local counter.

### Conversion from u64 to u32
EventBus stores steps as `u64`, but the map uses `u32` for backward compatibility:
```rust
self.tool_step_map.insert(call_id.clone(), (short_sid, step as u32));
```

This is safe since step counts will never exceed `u32::MAX` in practice.

## Testing

To verify the fix works:

1. **Open the TUI and start a session**
2. **Execute several tool calls**
3. **Compare**:
   - Message window shows `[s1:1]`, `[s1:2]`, etc.
   - Log window shows same step numbers
   - Agents window shows `steps:1`, `steps:2`, etc.
4. **Spawn sub-agents with `new_task`**
5. **All windows should display consistent step counts**

## Related Code

- `crates/ragent-core/src/event/mod.rs`: EventBus implementation
- `crates/ragent-core/src/session/processor.rs`: Where steps are incremented
- `crates/ragent-tui/src/layout_active_agents.rs`: Agents window renderer
- `crates/ragent-tui/src/widgets/message_widget.rs`: Message window renderer
