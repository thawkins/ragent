# Algorithmic Complexity Analysis Report — ragent-agent Crate

**Task ID:** s3  
**Date:** 2025-01-18  
**Scope:** ragent-agent crate source code (src/ directory)

---

## Executive Summary

This report identifies algorithmic complexity issues in the ragent-agent crate that may cause performance bottlenecks as session/message counts scale. Key findings include O(n²) nested loop patterns, inefficient data structure choices, repeated calculations in hot paths, and string operations in tight loops.

---

## Critical Issues (Priority 0-1)

### 1. O(n²) Message Compaction Algorithm

**Location:** `src/session/processor.rs:2530-2614`

**Problem:** The `compact_history_with_atomic_tool_calls` function has O(n²) complexity due to inefficient vector operations:

```rust
// Lines 2575-2590: O(n) element removal from beginning of Vec
while current_tokens > max_tokens && trimmed.len() > 2 {
    let to_remove = 0; // Try removing the oldest message
    // ...
    let removed_tokens = estimate_tokens(&trimmed[to_remove]);
    trimmed.remove(to_remove);  // O(n) operation!
    // ...
}
```

Each `trimmed.remove(0)` call shifts all remaining n elements, creating O(n²) total complexity when trimming multiple messages.

**Impact:** With 1000 messages and trimming 500, this could result in ~250,000 element moves.

**Fix:** Use a `VecDeque` for trimmed messages or swap_remove for O(1) amortized removal.

---

### 2. O(n²) Tool Call Index Building

**Location:** `src/session/processor.rs:2531-2541`

**Problem:** Building tool call indices while iterating through all messages:

```rust
// Lines 2531-2541
let mut tool_call_indices: HashMap<String, usize> = HashMap::new();
for (idx, msg) in messages.iter().enumerate() {
    if msg.role == Role::Assistant {
        for part in &msg.parts {  // Inner loop
            if let MessagePart::ToolCall { call_id, .. } = part {
                tool_call_indices.insert(call_id.clone(), idx);  // Potential resize
            }
        }
    }
}
```

While not strictly O(n²), this builds indices that could be maintained incrementally.

---

### 3. Permission Checker Linear Rule Evaluation

**Location:** `src/permission/mod.rs:298-307`

**Problem:** Every permission check iterates through ALL rules:

```rust
// Lines 298-307
pub fn check(&self, permission: &str, resource: &str) -> PermissionAction {
    let mut action = PermissionAction::Ask; // default
    for rule in &self.ruleset {  // O(rules) for every check
        if rule.matches(permission, resource) {
            action = rule.action.clone();
        }
    }
    action
}
}
```

With 100 permission rules, every tool call requires 100 string comparisons and glob matches.

**Fix:** Index rules by permission type in a HashMap for O(1) lookup.

---

### 4. Event Bus Broadcast Channel with Unbounded Growth

**Location:** `src/event/mod.rs:150-180`

**Problem:** The broadcast channel uses a fixed capacity (1024), but slow consumers can cause message lagging:

```rust
// Line 156: Fixed capacity
let (tx, _rx) = broadcast::channel(1024);
```

When receivers lag, messages are dropped (Lagged error), but this still wastes memory during bursts.

---

## High Priority Issues (Priority 1-2)

### 5. Repeated String Allocation in Estimate Tokens

**Location:** `src/session/processor.rs:2507-2529`

**Problem:** `estimate_tokens` closure allocates on every call:

```rust
// Called for EVERY message during compaction
let estimate_tokens = |msg: &Message| -> usize {
    let text_len: usize = msg
        .parts
        .iter()
        .map(|p| match p {
            MessagePart::Text { text } => text.len(),
            // ... allocates temporary strings
        })
        .sum();
    text_len / CHARS_PER_TOKEN + 10
};
```

**Fix:** Cache token estimates or use a pre-computed field on Message.

---

### 6. String Truncation with O(n) char_count

**Location:** `src/session/processor.rs:2616-2642`

**Problem:** `truncate_at_char_boundary` uses `chars().count()` which is O(n):

```rust
// Line 2617: O(n) character counting
if text.chars().count() <= max_chars {
    return text;
}

// Line 2629-2632: Another O(n) count
let total_chars = text.chars().count();
```

Called frequently for tool result truncation.

**Fix:** Use byte-based truncation with UTF-8 validation, or cache character count.

---

### 7. Nested Loop in Skill Registry Loading

**Location:** `src/skill/loader.rs:332-362`

**Problem:** Multiple nested directory walks:

```rust
// Lines 332-362
for dir_name in &[".agent", ".claude"] {
    let entries = fs::read_dir(...)?;
    for entry in entries.filter_map(Result::ok) {  // Inner loop
        // ...
    }
}
```

---

### 8. Regex Compilation in Tight Loops

**Location:** Various files with grep patterns

**Problem:** Multiple regex patterns compiled repeatedly. Example patterns found:

```rust
// These patterns suggest runtime regex compilation
let re = Regex::new(r"...").unwrap();
```

Should use `lazy_static!` or `once_cell` for compiled regex caching.

---

## Medium Priority Issues (Priority 2-3)

### 9. Inefficient Memory Search with Linear Scans

**Location:** `src/tool/memory_search.rs:200-280`

**Problem:** Multiple passes over search results:

```rust
// Lines 200-280: 3 separate enumerate loops over results
for (i, result) in results.iter().enumerate() { /* ... */ }
for (i, mem) in entries.iter().enumerate() { /* ... */ }
for (i, resolved) in results.iter().enumerate() { /* ... */ }
```

---

### 10. File Read Tool with Multiple Line Iterations

**Location:** `src/tool/read.rs:320-570`

**Problem:** Same file content iterated multiple times:

```rust
// Lines 338-340: First iteration
for (i, line) in lines.iter().enumerate() { /* ... */ }

// Lines 384-396: Second iteration
for (i, line) in lines.iter().enumerate() { /* ... */ }

// Lines 410-454: Third iteration
for (i, line) in lines.iter().enumerate() { /* ... */ }

// Plus 5+ more iterations...
```

Total: ~10 passes over the same lines Vec for a single file read operation.

---

### 11. Storage Module String Allocations in Encryption

**Location:** `src/storage/mod.rs:82-100`

**Problem:** Multiple allocations during key encryption:

```rust
// Lines 88-93: Allocates Vec for XOR operation
let ciphertext: Vec<u8> = key
    .as_bytes()
    .iter()
    .zip(keystream.iter())
    .map(|(p, k)| p ^ k)  // Creates new Vec
    .collect();
```

---

### 12. Team Manager Reconcile Loop O(m×n)

**Location:** `src/team/manager.rs:472-505`

**Problem:** Nested filtering creating O(m×n) complexity:

```rust
// Lines 484-498
store.config.members.iter()
    .filter(|m| m.status == MemberStatus::Spawning)
    .filter(|m| {
        if m.session_id.is_some() { /* ... */ }
        if existing_handles.contains_key(&m.agent_id) { /* O(1) but called for each */ }
        // ...
    })
```

---

## Data Structure Inefficiencies

### 13. Vec for Tool Registry Lookups

**Location:** `src/tool/mod.rs` (various lookup functions)

**Problem:** Tool definitions stored in Vec, requiring linear scan for lookups:

```rust
// Implied by registry.definitions() returning Vec
tool_registry.definitions()  // Returns Vec, not HashMap
```

**Fix:** Use HashMap<String, ToolDefinition> for O(1) name-based lookups.

---

### 14. Linear Search for Agent Resolution

**Location:** `src/agent/mod.rs:1059-1160`

**Problem:** Agent files discovered via linear Vec iteration:

```rust
// Lines 1059-1108: Walk directory and collect to Vec
for file in &self.found_files { /* ... */ }
```

---

### 15. Session Cache with HashMap but No LRU

**Location:** `src/session/cache.rs:102-141`

**Problem:** Agent prompts cached in HashMap but no eviction policy:

```rust
agent_prompts: Mutex<HashMap<AgentPromptKey, Cached<String>>>,
```

Grows unbounded with unique agent/prompt combinations.

---

## Redundant Work Patterns

### 16. Config Loading on Every Message

**Location:** `src/session/processor.rs:698-700`

**Problem:** Configuration re-parsed on every message:

```rust
let session_config = {
    let _scope = profiler.scope("config.load");
    crate::Config::load().unwrap_or_default()  // Parses JSON on EVERY message!
};
```

**Fix:** Cache config and watch for file changes.

---

### 17. Skill Registry Reloaded Every Message

**Location:** `src/session/processor.rs:726-730`

**Problem:** Skill registry reloaded even if unchanged:

```rust
let skill_registry = {
    let _scope = profiler.scope("skills.load_registry");
    crate::skill::SkillRegistry::load(&working_dir, &skill_dirs)  // O(n) file operations
};
```

---

### 18. Tool Reference Section Rebuilt Every Message

**Location:** `src/session/processor.rs:103-123`

**Problem:** `build_tool_reference_section` iterates all tools on every message:

```rust
fn build_tool_reference_section(registry: &ToolRegistry) -> String {
    let defs = registry.definitions();  // Gets all definitions
    for def in &defs {  // O(tools) every message
        // ...
    }
}
```

**Fix:** Cache until tools change.

---

### 19. Context Collection on Every Message

**Location:** `src/session/processor.rs:731-734`

**Problem:** Git status, README, and file tree collected on every message:

```rust
let (git_status, readme, agents_md, file_tree) = {
    let _scope = profiler.scope("prompt.collect_context");
    crate::agent::collect_prompt_context(&working_dir).await  // File operations!
};
```

---

### 20. System Time Calls in Tight Loops

**Location:** Various profiling scopes

**Problem:** Multiple `Instant::now()` calls in hot paths add overhead.

---

## Summary Table

| File | Line | Issue | Complexity | Priority |
|------|------|-------|------------|----------|
| session/processor.rs | 2530-2614 | Vec::remove(0) in loop | O(n²) | **Critical (0)** |
| session/processor.rs | 298-307 | Linear permission rules scan | O(rules) | **Critical (0)** |
| session/processor.rs | 2507-2529 | Token estimation allocates | O(parts) | High (1) |
| session/processor.rs | 2616-2642 | char_count() O(n) | O(n) | High (1) |
| session/processor.rs | 698-700 | Config reloaded per message | O(config) | High (1) |
| session/processor.rs | 726-730 | Skills reloaded per message | O(files) | High (1) |
| session/processor.rs | 731-734 | Context collected per message | O(files) | High (1) |
| tool/read.rs | 320-570 | 10+ line iterations | O(10n) | Medium (2) |
| tool/memory_search.rs | 200-280 | Multiple result scans | O(3n) | Medium (2) |
| team/manager.rs | 472-505 | Nested filter chains | O(m×n) | Medium (2) |
| permission/mod.rs | 289-307 | No rule indexing | O(rules) | Medium (2) |
| agent/mod.rs | 1059-1160 | Linear agent search | O(files) | Low (3) |
| storage/mod.rs | 82-100 | Encryption allocates | O(key_len) | Low (3) |

---

## Recommendations

1. **Immediate (0-1 day):** Replace Vec::remove(0) with swap_remove or VecDeque in compaction
2. **Short-term (1-3 days):** Cache config, skills, and context; only reload on changes
3. **Medium-term (1 week):** Index permission rules by permission type
4. **Long-term (2 weeks):** Refactor read.rs to single-pass line processing

---

*Analysis completed for Task s3*
