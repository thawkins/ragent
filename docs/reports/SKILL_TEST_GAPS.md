# Skill Test Coverage Gaps & Recommendations

## Executive Summary

The skill system has **~70 tests** covering **~75% happy path** and **~30% edge cases**.

**Critical gaps:** Forked execution, input validation, error recovery, and stress testing.

---

## CRITICAL GAPS (Do First)

### 1. Forked Skill Execution (NO TESTS)
**Severity:** HIGH  
**Impact:** Core feature untested end-to-end

Missing tests:
- `invoke_forked_skill()` actual execution
- Session creation in forked context
- Agent resolution and model override application
- Allowed tools enforcement during fork
- Parent-child session communication
- Forked session cleanup/cancellation

**Recommendation:**
```rust
#[tokio::test]
async fn test_forked_skill_execution() {
    // Create mock processor with session manager
    // Invoke forked skill
    // Verify: new session created, correct agent used, model override applied
    // Verify: allowed_tools enforced in fork
    // Verify: response returned to parent
}
```

---

### 2. Input Validation - Unicode & Special Characters
**Severity:** HIGH  
**Impact:** Potential crashes or security issues

Missing tests:
- Unicode in skill names, descriptions, bodies
- Emoji in arguments and skill content
- Control characters in inputs
- Non-UTF8 data handling
- Very long strings (10MB+)
- Null bytes in skill bodies

**Recommendation:**
```rust
#[test]
fn test_parse_unicode_in_description() {
    let content = "---\ndescription: Deploy to 中国 🚀\n---\nBody";
    let skill = parse_skill_md(content, ...);
    assert_eq!(skill.description.as_deref(), Some("Deploy to 中国 🚀"));
}

#[test]
fn test_substitute_emoji_in_args() {
    let result = substitute_args("Message: $0", "Hello 👋", "s1", Path::new("/"));
    assert_eq!(result, "Message: Hello 👋");
}
```

---

### 3. Escaped Characters & Quote Handling
**Severity:** HIGH  
**Impact:** Argument parsing may fail on real-world input

Missing tests:
- Escaped quotes within quoted strings: `"Say \"hello\""`
- Escaped newlines: `"line1\nline2"`
- Backslash handling: `"C:\path\to\file"`
- Single quotes in double quotes: `"can't"`
- Double quotes in single quotes: `'say "hi"'`
- Consecutive variables: `$0$1$ARGUMENTS`

**Recommendation:**
```rust
#[test]
fn test_parse_escaped_quotes() {
    let args = r#""Say \"hello\" to me" world"#;
    let parsed = parse_args(args);
    assert_eq!(parsed[0], r#"Say "hello" to me"#);
    assert_eq!(parsed[1], "world");
}

#[test]
fn test_substitute_consecutive_variables() {
    let result = substitute_args("$0$1-$ARGUMENTS", "a b", "s1", Path::new("/"));
    assert_eq!(result, "ab-a b");  // Or verify exact behavior
}
```

---

### 4. Error Handling & Recovery (LIMITED)
**Severity:** HIGH  
**Impact:** Unknown error paths

Missing tests:
- What happens when SKILL.md is deleted during discovery?
- Partial skill registration failure
- Duplicate skill names in registry (different scopes)
- File permission errors during skill discovery
- Corrupted YAML (invalid structure, not just syntax)
- Model format validation failures

**Recommendation:**
```rust
#[test]
fn test_discover_skills_permission_denied() {
    // Create skill dir with no read permissions
    // verify discovery handles gracefully
}

#[test]
fn test_parse_invalid_yaml_structure() {
    let content = "---\nallowed-tools:\n  key: value\n---\nBody";
    // allowed-tools should be list, not object
    let result = parse_skill_md(content, ...);
    assert!(result.is_err());
}
```

---

## IMPORTANT GAPS (Do Second)

### 5. Performance & Stress Testing
**Severity:** MEDIUM  
**Impact:** Unknown behavior at scale

Missing tests:
- Discover 1000+ skills
- Registry with 10000+ entries
- Large skill bodies (100MB+)
- Commands with massive output (1GB+)
- 100 concurrent skill invocations
- Skill substitution with 10000 variables

**Recommendation:**
```rust
#[test]
fn test_discover_many_skills_performance() {
    let tmp = tempdir();
    for i in 0..1000 {
        create_skill(&tmp, &format!("skill-{}", i));
    }
    let start = Instant::now();
    let skills = discover_skills(&tmp, &[]);
    let duration = start.elapsed();
    assert!(duration.as_secs() < 10, "1000 skills should discover in <10s");
}
```

---

### 6. Context Injection - Edge Cases
**Severity:** MEDIUM  
**Impact:** Unpredictable behavior with complex commands

Missing tests:
- Command output >1GB
- Timeout exactly at 30s boundary
- Commands with stderr + stdout
- Binary output from commands
- Commands that spawn child processes
- Nested !` patterns or code fence interactions
- Multiple patterns in rapid sequence

**Recommendation:**
```rust
#[tokio::test]
async fn test_inject_large_output() {
    let result = inject_dynamic_context(
        "Output: !`python -c 'print(\"x\" * 100_000_000)'`",
        Path::new("/tmp")
    ).await;
    // Verify doesn't OOM or timeout unexpectedly
}

#[tokio::test]
async fn test_inject_stderr_handling() {
    let result = inject_dynamic_context(
        "Result: !`bash -c 'echo stdout; echo stderr >&2'`",
        Path::new("/tmp")
    ).await;
    // Verify stderr is captured or handled
}
```

---

### 7. Registry - Concurrent Operations
**Severity:** MEDIUM  
**Impact:** Race conditions in multi-threaded usage

Missing tests:
- Concurrent register() calls
- register() + get() simultaneously
- Multiple scope priority updates in parallel
- Load registry while adding skills
- Stress test with many threads

**Recommendation:**
```rust
#[test]
fn test_concurrent_registration() {
    let registry = Arc::new(Mutex::new(SkillRegistry::new()));
    let handles: Vec<_> = (0..100)
        .map(|i| {
            let reg = registry.clone();
            thread::spawn(move || {
                let mut r = reg.lock().unwrap();
                r.register(SkillInfo::new(&format!("skill-{}", i), "..."));
            })
        })
        .collect();
    
    for handle in handles { handle.join().unwrap(); }
    
    let r = registry.lock().unwrap();
    assert_eq!(r.len(), 100);
}
```

---

### 8. Discovery - Edge Cases
**Severity:** MEDIUM  
**Impact:** Missing skills or crashes on unusual filesystems

Missing tests:
- Symlink cycles in skill directories
- Symlinks pointing to skills
- Skills on read-only filesystems
- Case sensitivity (SKILL.md vs Skill.md vs skill.md)
- Very deep nesting (100+ levels)
- Skills with `.` or `..` in names
- Directories with spaces/special chars in names

**Recommendation:**
```rust
#[test]
fn test_discover_symlink_skill() {
    let tmp = tempdir();
    let real_skill = create_skill(&tmp, "real-skill");
    let link = tmp.join("link-skill");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&real_skill, &link).unwrap();
    
    let skills = discover_skills(&tmp, &[]);
    // Verify both or just real one are discovered (define behavior)
}

#[test]
fn test_discover_skill_name_with_special_chars() {
    let tmp = tempdir();
    create_skill(&tmp, "skill with spaces");
    let skills = discover_skills(&tmp, &[]);
    // Verify handling of special characters in skill dir names
}
```

---

## MINOR GAPS (Do Third)

### 9. Bundled Skills - Validation
**Severity:** LOW  
**Impact:** Bad instructions shipped

Missing tests:
- Actual execution of bundled skill instructions
- Tool allowlists match real tool names
- Body instructions are coherent/valid
- Model/agent fields used correctly
- Verify descriptions are actually accurate

**Recommendation:**
```rust
#[test]
fn test_bundled_skills_instructions_valid() {
    // For each bundled skill:
    // 1. Verify body is well-formed markdown
    // 2. Verify no syntax errors in referenced variables
    // 3. Verify allowed_tools are all real tool names
    for skill in bundled_skills() {
        assert!(!skill.body.is_empty());
        // Add more validations
    }
}
```

---

### 10. Integration - Multi-Skill Workflows
**Severity:** LOW  
**Impact:** Complex interactions untested

Missing tests:
- One skill calling another (if supported)
- Skills with dependencies
- Large system prompt with 50+ skills
- Skills in different scopes interacting
- Registry filtering for different agents
- Token impact of large skill lists

**Recommendation:**
```rust
#[test]
fn test_system_prompt_with_many_skills() {
    let mut registry = SkillRegistry::new();
    for i in 0..50 {
        registry.register(SkillInfo::new(&format!("skill-{}", i), "..."));
    }
    let prompt = build_system_prompt(&agent, Some(&registry));
    // Verify prompt is valid, not too large, includes all skills
    assert!(prompt.len() < 100_000);
    assert!(prompt.contains("skill-0"));
    assert!(prompt.contains("skill-49"));
}
```

---

## Test Strategy Recommendations

### Quick Wins (2-3 hours)
1. Add unicode/emoji tests to args.rs (5 tests)
2. Add escaped character tests to args.rs (3 tests)
3. Add large output test to context.rs (2 tests)
4. Add permission error handling to loader.rs (2 tests)

### Medium Effort (1-2 days)
5. Implement concurrent registry tests (5 tests)
6. Add symlink/special char discovery tests (4 tests)
7. Implement forked execution tests (6 tests)
8. Add command timeout edge cases (3 tests)

### Longer Term (1 week)
9. Performance/stress tests (10 tests)
10. Full integration workflows (8 tests)
11. Bundled skill validation (5 tests)

---

## Implementation Priority Matrix

| Gap | Severity | Effort | Priority | Estimate |
|-----|----------|--------|----------|----------|
| Forked execution | HIGH | HIGH | 1st | 4h |
| Unicode/special chars | HIGH | MEDIUM | 1st | 2h |
| Escaped quotes | HIGH | MEDIUM | 1st | 2h |
| Error handling | HIGH | MEDIUM | 1st | 3h |
| Stress testing | MEDIUM | HIGH | 2nd | 8h |
| Context injection edge cases | MEDIUM | MEDIUM | 2nd | 3h |
| Concurrent registry | MEDIUM | MEDIUM | 2nd | 3h |
| Discovery edge cases | MEDIUM | MEDIUM | 2nd | 3h |
| Bundled skill validation | LOW | LOW | 3rd | 2h |
| Integration workflows | LOW | MEDIUM | 3rd | 4h |

**Total estimated work:** ~34 hours to close all gaps

---

## Files to Update

```
crates/ragent-core/src/skill/
├── args.rs        ← Add 8+ tests (unicode, escaped chars)
├── invoke.rs      ← Add 6+ tests (forked execution)
├── context.rs     ← Add 3+ tests (large output, timeout edge cases)
├── loader.rs      ← Add 4+ tests (error handling, edge cases)
├── mod.rs         ← Add 5+ tests (concurrent, performance)
└── bundled.rs     ← Add 3+ tests (validation)

crates/ragent-core/tests/
└── test_agent_system.rs  ← Add 2+ tests (multi-skill workflows)
```

