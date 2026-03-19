# Session ID and Step Count Display Synchronization

## Problem

The Session ID and step count shown in the message/log windows does not match what's shown in the Agents window:

- **Messages/Log Window**: Shows `[s1:1]`, `[s1:2]`, etc. (using local `tool_step_counter`)
- **Agents Window**: Shows actual step count from `event_bus.current_step(session_id)`

## Root Cause

The TUI maintains two independent step tracking systems:

1. **Local `tool_step_counter`**: Increments per tool call, resets between messages or session switches
2. **EventBus `steps` map**: Maintains authoritative step count from the backend

These two counters diverge over time, causing the display mismatch.

## Solution

**Use `event_bus.current_step()` as the single source of truth for both displays.**

### Changes Required

#### 1. `crates/ragent-tui/src/app.rs`
Replace local counter with event bus queries:

```rust
// OLD: self.tool_step_counter += 1;
// NEW:
Event::ToolCallStart { session_id, call_id, tool } => {
    if self.is_current_session(session_id) {
        // Increment the step on the event bus
        self.event_bus.set_step(session_id, self.event_bus.current_step(session_id) + 1);
        let step = self.event_bus.current_step(session_id);
        let short_sid = short_session_id(session_id);
        self.tool_step_map.insert(call_id.clone(), (short_sid.clone(), step));
        // ... rest of handler
    }
}
```

#### 2. `crates/ragent-core/src/event/mod.rs`
Add a public setter for step values:

```rust
/// Set the current step number for a session (called by TUI or backend).
pub fn set_step(&self, session_id: &str, step: u64) {
    self.steps
        .write()
        .expect("step map poisoned")
        .insert(session_id.to_string(), step);
}
```

#### 3. Remove `tool_step_counter` from App state
The local counter is no longer needed since we use the event bus.

## Benefits

1. **Single source of truth**: All displays use the same step counter
2. **Consistency**: Agents window and Message window always match
3. **Accuracy**: Backend step count is authoritative
4. **Simplicity**: No complex sync logic needed

## Implementation Status

- [ ] Add `set_step()` to EventBus
- [ ] Update ToolCallStart handler to use `event_bus.set_step()`
- [ ] Remove `tool_step_counter` field from App
- [ ] Test that Agents window and Message window step counts match

## Testing

After implementation, verify:
1. Run a session with multiple tool calls
2. Check Agents window shows correct step count
3. Check message display shows `[sid:step]` matching Agents window
4. Spawn sub-agents and verify their step counts match display
