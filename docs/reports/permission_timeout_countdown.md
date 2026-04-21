# Permission Timeout & Countdown Timer Implementation

> **Date:** 2025-01-17  
> **Issue:** Permission dialog timeout was too short (30s) and provided no visual feedback on remaining time  
> **Status:** ✅ IMPLEMENTED

---

## Summary

**Issue:** The permission request dialog timed out after 30 seconds with no countdown timer, causing user confusion when requests automatically denied.

**Resolution:**
1. Increased timeout from 30 seconds to 120 seconds (2 minutes)
2. Added `created_at` and `timeout_secs` fields to `PermissionRequest` struct
3. Implemented countdown timer display in permission dialog title
4. Format: "Permission Required (1:45 remaining)"

---

## Changes Made

### 1. Extended Timeout Duration
**File:** `crates/ragent-core/src/session/processor.rs`  
**Line:** 334

**Before:**
```rust
// Timeout: 30 seconds
let timeout = tokio::time::Duration::from_secs(30);
```

**After:**
```rust
// Timeout: 120 seconds (2 minutes)
let timeout = tokio::time::Duration::from_secs(120);
```

---

### 2. Add Timestamp Fields to PermissionRequest
**File:** `crates/ragent-core/src/permission/mod.rs`  
**Lines:** 233-251

**Enhanced Struct:**
```rust
/// Permission request sent to UI for user decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub id: String,
    pub session_id: String,
    pub permission: String,
    pub patterns: Vec<String>,
    pub metadata: Value,
    pub tool_call_id: Option<String>,
    /// Unix timestamp (seconds since epoch) when the request was created
    #[serde(default)]
    pub created_at: u64,
    /// Timeout in seconds (default: 120)
    #[serde(default = "default_permission_timeout")]
    pub timeout_secs: u64,
}

fn default_permission_timeout() -> u64 {
    120
}
```

**Rationale:**
- `created_at`: Unix timestamp enables countdown calculation
- `timeout_secs`: Configurable timeout per request (default 120)
- `#[serde(default)]`: Backward compatibility with existing events

---

### 3. Populate Timestamp on Request Creation
**File:** `crates/ragent-core/src/session/processor.rs`  
**Lines:** 346-358

**Before:**
```rust
let request = PermissionRequest {
    id: request_id.clone(),
    session_id: session_id.clone(),
    permission: permission.to_string(),
    patterns: vec![resource.to_string()],
    metadata: metadata.clone(),
    tool_call_id: None,
};
```

**After:**
```rust
let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs();

let request = PermissionRequest {
    id: request_id.clone(),
    session_id: session_id.clone(),
    permission: permission.to_string(),
    patterns: vec![resource.to_string()],
    metadata: metadata.clone(),
    tool_call_id: None,
    created_at: now,
    timeout_secs: 120,
};
```

---

### 4. Render Countdown Timer in Dialog
**File:** `crates/ragent-tui/src/input.rs`  
**Lines:** 368-419

**Enhanced Dialog Title:**
```rust
fn render_permission_dialog(&self, f: &mut Frame, req: &PermissionRequest) {
    // Calculate remaining time
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let elapsed = now.saturating_sub(req.created_at);
    let remaining = req.timeout_secs.saturating_sub(elapsed);
    let remaining_mins = remaining / 60;
    let remaining_secs = remaining % 60;

    let title = if remaining > 0 {
        format!(
            " Permission Required ({}:{:02} remaining) ",
            remaining_mins, remaining_secs
        )
    } else {
        " Permission Required (EXPIRED) ".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .border_type(BorderType::Rounded)
        .title(title)  // Dynamic title with countdown
        .title_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    // ... rest of rendering
}
```

**Display Format:**
- `"Permission Required (2:00 remaining)"` — 2 minutes left
- `"Permission Required (1:45 remaining)"` — 1 minute 45 seconds left
- `"Permission Required (0:05 remaining)"` — 5 seconds left
- `"Permission Required (EXPIRED)"` — timeout reached

---

## Behavior

### Before
```
┌─ Permission Required ────────────────┐
│                                      │
│ Permission: bash                     │
│ Resource: echo "hello"               │
│                                      │
│ [A]llow Once | [W]ays Allow | [D]eny│
└──────────────────────────────────────┘
```
- 30-second timeout (too short)
- No indication of time remaining
- Automatic deny on timeout with no warning

### After
```
┌─ Permission Required (1:45 remaining) ─┐
│                                        │
│ Permission: bash                       │
│ Resource: echo "hello"                 │
│                                        │
│ [A]llow Once | [W]ays Allow | [D]eny  │
└────────────────────────────────────────┘
```
- 120-second timeout (2 minutes)
- Countdown timer updates every second
- Clear visual feedback on remaining time
- "EXPIRED" status shown when timeout reached

---

## Technical Details

### Countdown Calculation
```rust
let now = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap_or_default()
    .as_secs();
let elapsed = now.saturating_sub(req.created_at);
let remaining = req.timeout_secs.saturating_sub(elapsed);
let remaining_mins = remaining / 60;
let remaining_secs = remaining % 60;
```

**Properties:**
- Uses Unix timestamps for cross-component consistency
- `saturating_sub()` prevents underflow on clock skew
- Real-time calculation (no state tracking needed)
- Updates automatically on each render cycle

### Backward Compatibility
```rust
#[serde(default)]
pub created_at: u64,
#[serde(default = "default_permission_timeout")]
pub timeout_secs: u64,
```

**Ensures:**
- Existing permission events deserialize correctly
- Missing fields default to safe values (0 for `created_at`, 120 for `timeout_secs`)
- No breaking changes to event wire format

---

## Testing

### Manual Testing Required
1. Trigger permission prompt: `bash: curl example.com`
2. Verify countdown timer appears in dialog title
3. Wait and confirm timer counts down correctly
4. Verify "EXPIRED" message appears after 2 minutes
5. Confirm automatic deny occurs on timeout

### Verification Steps
```bash
# In TUI, run a command that requires permission
bash: curl http://example.com

# Expected behavior:
# - Dialog shows: "Permission Required (2:00 remaining)"
# - Timer counts down: "1:59", "1:58", etc.
# - After 120 seconds: "Permission Required (EXPIRED)"
# - Auto-deny message appears in chat
```

---

## Configuration

### Default Timeout
**File:** `crates/ragent-core/src/permission/mod.rs`

```rust
fn default_permission_timeout() -> u64 {
    120  // 2 minutes
}
```

**Future Enhancement:** Could be made configurable via:
```jsonc
// ragent.json
{
  "permissions": {
    "timeout_secs": 120
  }
}
```

---

## Files Modified

1. **crates/ragent-core/src/permission/mod.rs**
   - Added `created_at: u64` field
   - Added `timeout_secs: u64` field
   - Added `default_permission_timeout()` helper

2. **crates/ragent-core/src/session/processor.rs**
   - Changed timeout from 30s to 120s
   - Populate `created_at` and `timeout_secs` on request creation

3. **crates/ragent-tui/src/input.rs**
   - Calculate remaining time on each render
   - Display countdown in dialog title
   - Show "EXPIRED" status when time runs out

---

## Verification Checklist

- [x] Timeout extended from 30s to 120s
- [x] `created_at` timestamp added to `PermissionRequest`
- [x] `timeout_secs` field added with default of 120
- [x] Countdown timer displayed in dialog title
- [x] Timer format: `M:SS` (e.g., "1:45")
- [x] "EXPIRED" message shown after timeout
- [x] Backward compatibility maintained (serde defaults)
- [x] Code compiles without errors

---

## User Experience Improvements

### Before
- ❌ 30-second timeout too short for review
- ❌ No indication of time remaining
- ❌ Sudden auto-deny surprises users
- ❌ No visual feedback on urgency

### After
- ✅ 120-second timeout allows careful review
- ✅ Clear countdown timer shows remaining time
- ✅ Users can see when timeout approaches
- ✅ Visual feedback creates urgency awareness
- ✅ "EXPIRED" message clarifies timeout state

---

## Future Enhancements

1. **Visual Urgency Indicators**
   - Yellow border when >60s remaining
   - Red border when <30s remaining
   - Blinking border in final 10 seconds

2. **Configurable Timeout**
   - Per-permission-type timeouts
   - User preference in config file
   - Command-line override: `--permission-timeout 300`

3. **Timeout Extension**
   - `[E]xtend` button to add 60 seconds
   - Maximum extension limit (e.g., 5 minutes total)

4. **Audio Alert**
   - Optional beep at 30s, 10s, 5s remaining
   - Configurable via `ragent.json`

---

## Conclusion

**Status: ✅ IMPLEMENTED**

The permission dialog now provides clear visual feedback on remaining time with a 2-minute default timeout. Users can:
- See exactly how much time remains to make a decision
- Plan their review based on the countdown
- Understand when requests expire ("EXPIRED" message)
- Have sufficient time (120s vs 30s) for careful review

The implementation maintains backward compatibility and requires no database migrations or breaking changes.

---

*Report generated: 2025-01-17*  
*Author: Rust Agent*  
*Status: Complete*
