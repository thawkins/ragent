# TUI Performance & Consistency Improvement Plan

## Overview

This document outlines a comprehensive plan to improve the performance and UI consistency of the `ragent-tui` crate. The plan is based on analysis of the existing codebase, performance review findings, security audit results, and test coverage gaps.

## Current State Analysis

### Architecture Summary

The TUI is built on:
- **ratatui**: Terminal UI framework with immediate-mode rendering
- **crossterm**: Cross-platform terminal event handling
- **tokio**: Async runtime for background tasks
- **EventBus**: Pub/sub messaging between agent and UI

### Key Files

| File | Purpose | Lines |
|------|---------|-------|
| `src/app.rs` | Main application logic & event handling | ~9,600 |
| `src/app/state.rs` | App state struct definitions | ~1,270 |
| `src/layout.rs` | UI layout & rendering | ~1,100 |
| `src/layout_active_agents.rs` | Active agents subpanel | ~200 |
| `src/layout_teams.rs` | Teams subpanel | ~200 |
| `src/widgets/message_widget.rs` | Message rendering widget | ~500 |
| `src/input.rs` | Keyboard input handling | ~900 |
| `src/lib.rs` | Main TUI event loop | ~250 |

### Existing Performance Optimizations

The codebase already has some performance optimizations in place:

1. **Dirty flag rendering** (`needs_redraw`): Already implemented in `App` struct
2. **LRU cache for markdown rendering**: Uses `lru::LruCache` with 256-entry capacity
3. **Debounced history flushing**: `history_dirty` flag with time-based flushing

### Identified Issues

Based on the performance findings document and code analysis:

#### High Priority

1. **Unnecessary/redraw-per-frame of entire TUI** (Partially Fixed)
   - **Status**: `needs_redraw` flag exists but may not be set consistently
   - **Location**: `src/lib.rs` main loop
   - **Issue**: The 50ms timer branch still forces ~20 FPS even with no changes

2. **Quadratic traversal in active-agents panel**
   - **Location**: `src/layout_active_agents.rs`
   - **Issue**: `build_task_rows` scans full tasks list for each node → O(n²)
   - **Impact**: Frame stalls with many active tasks

3. **Per-frame HashSet allocations in active panel**
   - **Location**: `src/layout_active_agents.rs` lines ~140-151
   - **Issue**: `custom_names` and `teammate_ids` rebuilt on every render

#### Medium Priority

4. **UI-blocking synchronous model discovery**
   - **Location**: `src/app.rs` `models_for_provider`
   - **Issue**: Uses `block_in_place` + `block_on` for Ollama/Copilot discovery
   - **Impact**: UI freeze during model discovery

5. **Markdown cache eviction strategy**
   - **Location**: `src/app.rs` `render_markdown_to_ascii`
   - **Issue**: Full cache clear when size >= 256 (bursty behavior)
   - **Current**: Already using LRU cache but with aggressive clearing

6. **Hot-path formatting allocations**
   - **Location**: Layout and widget rendering code
   - **Issue**: Numerous `format!` calls per-line on every draw

#### Low Priority

7. **Blocking git detection at startup**
   - **Location**: `src/app.rs` `detect_git_branch`
   - **Impact**: Slight startup delay

8. **Large clipboard image handling**
   - **Location**: `src/app/state.rs` `save_clipboard_image_to_temp`
   - **Issue**: Double-copy of pixel buffer

### Security & Consistency Issues

1. **Secret exposure in logs** (High)
   - API keys/tokens may appear in logs and UI
   - Need masking helper

2. **Executable path validation** (High)
   - Discovered server paths not validated before persistence
   - Risk of command injection

3. **Percent-decoding fragility** (Medium)
   - Uses `unwrap_or` in hex parsing

## Improvement Plan

### Phase 1: Critical Performance Fixes (High Impact)

#### 1.1 Fix Dirty Flag Consistency

**Goal**: Ensure `needs_redraw` is set consistently when state changes

**Tasks**:
- [ ] Audit all App state mutation sites for missing `needs_redraw = true`
- [ ] Create a helper method `App::mark_dirty()` for consistency
- [ ] Add dirty flag setting to:
  - Event handlers (`handle_event`)
  - Input handlers (`insert_char`, `delete_char`, etc.)
  - Log entry additions (`push_log`)
  - Message appends
  - Scroll position changes
  - Menu state changes

**Code Pattern**:
```rust
impl App {
    #[inline]
    pub fn mark_dirty(&mut self) {
        self.needs_redraw = true;
    }
}
```

**Estimated Effort**: 2-3 hours

#### 1.2 Optimize Active Agents Panel Rendering

**Goal**: Eliminate O(n²) traversal and per-frame allocations

**Tasks**:
- [ ] Precompute parent->children map once per render pass
- [ ] Replace recursive filtering with map lookup
- [ ] Cache `custom_names` and `teammate_ids` until data changes
- [ ] Avoid cloning `active_tasks` vector

**Current Code** (problematic):
```rust
fn build_task_rows<'a>(
    tasks: &[TaskEntry],  // Full list scanned repeatedly
    parent_sid: &str,
    ...
) {
    let children: Vec<_> = tasks.iter().filter(|t| t.parent_session_id == parent_sid).collect();
    // Recurses for each child, scanning full list again
}
```

**Optimized Code**:
```rust
fn build_task_rows<'a>(
    tasks_map: &HashMap<&str, Vec<&TaskEntry>>,  // Pre-built map
    parent_sid: &str,
    ...
) {
    let children = tasks_map.get(parent_sid).unwrap_or(&[]);
    // Direct lookup, no scanning
}
```

**Estimated Effort**: 3-4 hours

#### 1.3 Reduce Frame Rate When Idle

**Goal**: Lower CPU usage when UI is static

**Tasks**:
- [ ] Increase idle poll interval from 50ms to 250ms
- [ ] Use event-driven wakeups when possible
- [ ] Only use short poll interval when animations active

**Code Changes**:
```rust
// In lib.rs main loop
const IDLE_POLL_MS: u64 = 250;
const ACTIVE_POLL_MS: u64 = 50;

let poll_duration = if app.has_animations() {
    ACTIVE_POLL_MS
} else {
    IDLE_POLL_MS
};
```

**Estimated Effort**: 1-2 hours

### Phase 2: Async Operations & Caching (Medium Impact)

#### 2.1 Async Model Discovery

**Goal**: Prevent UI blocking during model discovery

**Tasks**:
- [ ] Create async model discovery task
- [ ] Add `ModelDiscoveryInProgress` state
- [ ] Send results via event bus
- [ ] Show loading indicator in UI

**Code Pattern**:
```rust
// Spawn discovery in background
tokio::spawn(async move {
    let models = discover_models(provider).await;
    event_bus.publish(Event::ModelsDiscovered { provider, models });
});

// In event handler
Event::ModelsDiscovered { provider, models } => {
    self.provider_models.insert(provider, models);
    self.mark_dirty();
}
```

**Estimated Effort**: 4-6 hours

#### 2.2 Optimize Markdown Cache

**Goal**: Improve cache hit rate and reduce allocations

**Tasks**:
- [ ] Remove manual cache clearing (LRU handles eviction)
- [ ] Return `Arc<str>` instead of cloning Strings
- [ ] Consider using `string_cache` for repeated strings

**Current Code**:
```rust
if self.md_render_cache.len() >= 256 {
    self.md_render_cache.clear();  // Too aggressive
}
```

**Optimized Code**:
```rust
// Remove the clear - LRU handles eviction
// Change cache type to LruCache<u64, Arc<str>>
```

**Estimated Effort**: 2-3 hours

#### 2.3 String Pool for Common Formats

**Goal**: Reduce per-frame format! allocations

**Tasks**:
- [ ] Identify hot format patterns (timestamps, IDs, etc.)
- [ ] Use `format_args!` where possible
- [ ] Cache formatted strings that don't change often

**Example**:
```rust
// Instead of:
Span::styled(format!("{:<8}", elapsed), style)

// Use:
Span::styled(format_args!("{:<8}", elapsed), style)
```

**Estimated Effort**: 3-4 hours

### Phase 3: Security & Consistency (High Priority)

#### 3.1 Secret Masking

**Goal**: Prevent API keys from appearing in logs

**Tasks**:
- [ ] Add `mask_secret(s: &str) -> String` helper
- [ ] Apply masking to all env var reads
- [ ] Audit all `push_log` and `tracing!` calls
- [ ] Add CI lint for secret patterns

**Code Pattern**:
```rust
pub fn mask_secret(s: &str) -> String {
    if s.len() <= 8 {
        "***".to_string()
    } else {
        format!("{}***{}", &s[..4], &s[s.len()-4..])
    }
}
```

**Estimated Effort**: 3-5 hours

#### 3.2 Executable Path Validation

**Goal**: Prevent command injection via discovered servers

**Tasks**:
- [ ] Add `validate_executable_path` function
- [ ] Check for shell metacharacters
- [ ] Verify file permissions on Unix
- [ ] Apply validation before persisting

**Code Pattern**:
```rust
fn validate_executable_path(p: &Path) -> bool {
    if let Some(s) = p.to_str() {
        if s.chars().any(|c| matches!(c, '\n' | '\r' | ';' | '|' | '&')) {
            return false;
        }
    }
    #[cfg(unix)] {
        if let Ok(meta) = std::fs::metadata(p) {
            if !meta.is_file() { return false; }
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o111 == 0 { return false; }
        }
    }
    true
}
```

**Estimated Effort**: 2-3 hours

#### 3.3 UI Consistency Improvements

**Goal**: Ensure consistent behavior across all UI components

**Tasks**:
- [ ] Standardize focus indicators across dialogs
- [ ] Ensure consistent keyboard navigation (Tab/Shift+Tab)
- [ ] Add missing Esc handlers to all dialogs
- [ ] Standardize error message display
- [ ] Ensure consistent color scheme usage

**Estimated Effort**: 4-6 hours

### Phase 4: Testing & Validation

#### 4.1 Performance Benchmarks

**Tasks**:
- [ ] Add benchmark for active agents panel with many tasks
- [ ] Benchmark markdown rendering cache
- [ ] Add memory allocation profiling tests
- [ ] Create benchmark for message rendering

**Estimated Effort**: 3-4 hours

#### 4.2 Test Coverage Improvements

**Tasks**:
- [ ] Add test for `mask_secret` function
- [ ] Add test for `validate_executable_path`
- [ ] Test dirty flag propagation
- [ ] Add test for active agents panel with mock data
- [ ] Test async model discovery

**Estimated Effort**: 4-6 hours

## Implementation Order

### Immediate (Week 1)

1. **Fix dirty flag consistency** - Highest impact for least effort
2. **Optimize active agents panel** - Fixes O(n²) issue
3. **Add secret masking** - Security critical

### Short-term (Week 2)

4. **Async model discovery** - Improves UI responsiveness
5. **Executable path validation** - Security hardening
6. **Reduce frame rate when idle** - Lower CPU usage

### Medium-term (Week 3)

7. **Markdown cache optimization**
8. **String allocation reduction**
9. **UI consistency improvements**

### Long-term (Week 4)

10. **Comprehensive testing**
11. **Performance validation**
12. **Documentation updates**

## Success Metrics

### Performance Targets

| Metric | Current | Target | Measurement |
|--------|---------|--------|-------------|
| CPU usage (idle) | ~20% | <5% | `top` or Activity Monitor |
| Frame time (active) | ~50ms | <16ms (60 FPS) | Internal timing |
| Memory churn/frame | High | Reduced 50% | Allocation profiler |
| Startup time | ~500ms | <300ms | `time ragent --help` proxy |

### Consistency Targets

- All dialogs respond to Esc key
- Consistent focus indicators
- No UI freezes >100ms
- All text input supports unicode correctly

## Testing Strategy

### Unit Tests

- Dirty flag propagation
- Secret masking edge cases
- Path validation
- Task tree building logic

### Integration Tests

- Full TUI lifecycle
- Event handling pipeline
- Async operation cancellation

### Performance Tests

- Benchmark with 1000+ messages
- Benchmark with 100+ active tasks
- Memory profiling over extended use

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Dirty flag misses | UI not updating | Comprehensive audit + tests |
| Async complexity | Race conditions | Proper cancellation tokens |
| Breaking changes | User disruption | Feature flags for changes |
| Performance regression | Slower than before | Benchmark comparison in CI |

## Dependencies

- `lru` - Already in use for caching
- `tokio` - Already in use for async
- `tracing` - Already in use for logging

No new dependencies required for Phase 1-3.

## Notes

- All changes should maintain backward compatibility
- Prefer small, reviewable PRs over large changes
- Each phase should be independently testable
- Document any API changes in CHANGELOG.md

## References

- `performance_findings.md` - Detailed performance analysis
- `security_findings.md` - Security audit results
- `test_findings.md` - Test coverage gaps
- `COMPLIANCE.md` - Compliance requirements

---

**Last Updated**: 2025-01-15
**Next Review**: After Phase 1 completion
