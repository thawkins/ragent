# BUGSPLAN.md - Bug Remediation Plan

## Overview

This document tracks bugs identified from the GitHub Copilot CLI releases page that may be relevant to ragent. The bugs are categorized by severity and implementation effort required.

**Source:** https://github.com/github/copilot-cli/releases  
**Analysis Date:** 2025-04-16  
**Releases Analyzed:** v1.0.3 through v1.0.28

---

## Severity Legend

- **Critical (P0):** Security vulnerabilities, data loss, crashes, broken core functionality
- **High (P1):** Major UX issues, significant performance problems, incorrect behavior
- **Medium (P2):** Minor UX issues, edge cases, polish items
- **Low (P3):** Nice-to-have improvements, documentation

---

## Part A: Critical Bugs (P0)

### A1. Session Corruption on Resume
**Copilot Reference:** v1.0.7 - "Session resume no longer fails with 'Session file is corrupted' for sessions created before 1.0.6"

**Description:** Sessions may fail to resume with a corruption error, especially older sessions or those with certain state configurations.

**Relevant to ragent:** Yes - ragent uses SQLite for session persistence

**Affected Areas:**
- `crates/ragent-core/src/storage/session.rs`
- `crates/ragent-core/src/session/`

**Implementation Tasks:**
1. [ ] **TASK-A1-1:** Audit session serialization/deserialization logic for backward compatibility
2. [ ] **TASK-A1-2:** Add version field to session storage format
3. [ ] **TASK-A1-3:** Implement migration path for older session formats
4. [ ] **TASK-A1-4:** Add defensive checks for corrupted session data with graceful fallback
5. [ ] **TASK-A1-5:** Add tests for session resume with various corruption scenarios

**Estimated Effort:** 8-12 hours

---

### A2. Session Data Loss on Exit
**Copilot Reference:** v1.0.10 - "Session history is no longer lost when exiting via /quit, Ctrl+C, or restart"

**Description:** Session state may not be properly persisted when exiting through various methods (signals, commands).

**Relevant to ragent:** Yes - session persistence on shutdown

**Affected Areas:**
- `crates/ragent-core/src/session/processor.rs`
- `crates/ragent-tui/src/app.rs`

**Implementation Tasks:**
1. [ ] **TASK-A2-1:** Audit signal handling in TUI (SIGINT, SIGTERM)
2. [ ] **TASK-A2-2:** Implement graceful shutdown hook that ensures session persistence
3. [ ] **TASK-A2-3:** Add flush/cleanup on Drop for session manager
4. [ ] **TASK-A2-4:** Test session persistence across various exit scenarios

**Estimated Effort:** 6-8 hours

---

### A3. HTTP/2 Race Conditions with Sub-agents
**Copilot Reference:** v1.0.6 - "Resolve session crashes caused by HTTP/2 connection pool race conditions when sub-agents are active"

**Description:** Concurrent sub-agent execution can cause HTTP/2 connection pool race conditions leading to crashes.

**Relevant to ragent:** Yes - affects background task execution

**Affected Areas:**
- `crates/ragent-core/src/agent/orchestrator.rs`
- `crates/ragent-core/src/provider/`

**Implementation Tasks:**
1. [ ] **TASK-A3-1:** Review HTTP client configuration for connection pooling
2. [ ] **TASK-A3-2:** Ensure proper request/response isolation per sub-agent
3. [ ] **TASK-A3-3:** Add connection pool size limits and timeout handling
4. [ ] **TASK-A3-4:** Implement retry logic with exponential backoff for transient failures
5. [ ] **TASK-A3-5:** Add stress tests for concurrent sub-agent execution

**Estimated Effort:** 12-16 hours

---

## Part B: High Priority Bugs (P1)

### B1. Terminal State Not Restored After Crashes
**Copilot Reference:** v1.0.24 - "Terminal state (alt screen, cursor, raw mode) is restored correctly after CLI crashes like OOM or segfault"

**Description:** If the CLI crashes, terminal may be left in an unusable state (raw mode, alternate screen buffer).

**Relevant to ragent:** Yes - TUI uses raw mode and alternate screen

**Affected Areas:**
- `crates/ragent-tui/src/app.rs`
- `crates/ragent-tui/src/main.rs`

**Implementation Tasks:**
1. [ ] **TASK-B1-1:** Implement panic handler that restores terminal state
2. [ ] **TASK-B1-2:** Add signal handlers for SIGSEGV, SIGABRT that cleanup terminal
3. [ ] **TASK-B1-3:** Ensure Drop implementations restore terminal state
4. [ ] **TASK-B1-4:** Test terminal restoration after simulated crashes

**Estimated Effort:** 4-6 hours

---

### B2. Context Compaction Splitting Tool Calls
**Copilot Reference:** v1.0.26 - "Agent sessions no longer fail with unrecoverable errors when context compaction splits a tool call across a checkpoint boundary"

**Description:** Context compaction (trimming conversation history) may split a tool call across boundaries, causing unrecoverable errors.

**Relevant to ragent:** Yes - context window management

**Affected Areas:**
- `crates/ragent-core/src/session/processor.rs`
- `crates/ragent-core/src/agent/context.rs`

**Implementation Tasks:**
1. [ ] **TASK-B2-1:** Review context compaction logic to identify boundary issues
2. [ ] **TASK-B2-2:** Ensure tool calls are atomic - never split across compaction boundaries
3. [ ] **TASK-B2-3:** Add validation that tool calls have matching responses
4. [ ] **TASK-B2-4:** Add tests for context compaction with tool calls

**Estimated Effort:** 8-10 hours

---

### B3. Session Scope Selector Issues
**Copilot Reference:** v1.0.26 - "Session scope selector in sync prompt is now more prominent and keyboard-navigable with left/right arrow keys"

**Description:** Session scope selector (for syncing sessions across devices) has UX issues with keyboard navigation.

**Relevant to ragent:** Yes - if session sync is implemented

**Affected Areas:**
- `crates/ragent-tui/src/components/` (session dialogs)

**Implementation Tasks:**
1. [ ] **TASK-B3-1:** Identify if ragent has session scope/sync functionality
2. [ ] **TASK-B3-2:** If yes, improve keyboard navigation for scope selector
3. [ ] **TASK-B3-3:** Add visual prominence to scope selector

**Estimated Effort:** 4-6 hours

---

### B4. Configuration Directory Ignored on Resume
**Copilot Reference:** v1.0.13 - "Fixed --config-dir being ignored when resuming a session, causing paths to silently fall back to ~/.copilot"

**Description:** Custom config directory not respected when resuming sessions.

**Relevant to ragent:** Yes - ragent supports custom config paths

**Affected Areas:**
- `crates/ragent-core/src/config.rs`
- `src/main.rs`

**Implementation Tasks:**
1. [ ] **TASK-B4-1:** Audit config loading order during session resume
2. [ ] **TASK-B4-2:** Ensure --config-dir is parsed before session resume logic
3. [ ] **TASK-B4-3:** Store config path in session state and validate on resume
4. [ ] **TASK-B4-4:** Add warning when config path changes between sessions

**Estimated Effort:** 4-6 hours

---

### B5. Permission Hooks Not Suppressing Approval Prompts
**Copilot Reference:** v1.0.18 - "preToolUse hook permissionDecision 'allow' now suppresses the tool approval prompt"

**Description:** Permission hooks that programmatically approve tools should suppress the UI prompt, but may not.

**Relevant to ragent:** Yes - permission system exists

**Affected Areas:**
- `crates/ragent-core/src/permission/mod.rs`
- `crates/ragent-core/src/tool/`

**Implementation Tasks:**
1. [ ] **TASK-B5-1:** Review permission hook execution flow
2. [ ] **TASK-B5-2:** Ensure hook decisions are respected and skip UI prompts
3. [ ] **TASK-B5-3:** Add test for hook-based permission approval
4. [ ] **TASK-B5-4:** Document hook behavior in permission system

**Estimated Effort:** 6-8 hours

---

## Part C: Medium Priority Bugs (P2)

### C1. Background Agent Redundant Notifications
**Copilot Reference:** v1.0.28 - "Background agent completion notifications are not sent redundantly when read_agent is already waiting for the result"

**Description:** Duplicate notifications sent when polling for background agent results.

**Relevant to ragent:** Yes - background task system

**Affected Areas:**
- `crates/ragent-core/src/agent/orchestrator.rs`
- `crates/ragent-core/src/tool/new_task.rs`

**Implementation Tasks:**
1. [ ] **TASK-C1-1:** Review notification logic for background tasks
2. [ ] **TASK-C1-2:** Implement deduplication for completion notifications
3. [ ] **TASK-C1-3:** Track waiting clients to avoid redundant notifications

**Estimated Effort:** 4-6 hours

---

### C2. Sub-agent Tool Name Duplication
**Copilot Reference:** v1.0.22 - "Sub-agent activity no longer shows duplicated tool names (e.g. 'view view the file...')"

**Description:** Tool names displayed in sub-agent activity may be duplicated.

**Relevant to ragent:** Yes - affects task display

**Affected Areas:**
- `crates/ragent-core/src/agent/orchestrator.rs`
- `crates/ragent-tui/src/panels/log.rs`

**Implementation Tasks:**
1. [ ] **TASK-C2-1:** Identify source of duplication in tool name display
2. [ ] **TASK-C2-2:** Fix string formatting for sub-agent tool descriptions
3. [ ] **TASK-C2-3:** Add test for tool name display formatting

**Estimated Effort:** 2-4 hours

---

### C3. Timeline Blank When Content Shrinks
**Copilot Reference:** v1.0.21 - "Timeline no longer goes blank when content shrinks (e.g., after cancelling or tool completion)"

**Description:** TUI timeline panel may go blank when content reduces in size.

**Relevant to ragent:** Yes - TUI rendering

**Affected Areas:**
- `crates/ragent-tui/src/panels/timeline.rs`
- `crates/ragent-tui/src/widgets/`

**Implementation Tasks:**
1. [ ] **TASK-C3-1:** Review scroll position handling in timeline widget
2. [ ] **TASK-C3-2:** Ensure scroll bounds are recalculated when content changes
3. [ ] **TASK-C3-3:** Clamp scroll position to valid range after updates

**Estimated Effort:** 4-6 hours

---

### C4. Spinner Delays Visible Output
**Copilot Reference:** v1.0.8 - "Spinner animation no longer delays visible output from appearing in the timeline"

**Description:** Spinner rendering can block or delay output display.

**Relevant to ragent:** Yes - TUI spinner/loading indicators

**Affected Areas:**
- `crates/ragent-tui/src/components/spinner.rs`
- `crates/ragent-tui/src/app.rs`

**Implementation Tasks:**
1. [ ] **TASK-C4-1:** Review spinner rendering loop
2. [ ] **TASK-C4-2:** Ensure spinner doesn't block message rendering
3. [ ] **TASK-C4-3:** Consider async spinner updates

**Estimated Effort:** 4-6 hours

---

### C5. Scroll Position on Terminal Resize
**Copilot Reference:** v1.0.12 - "Scroll position stays in place when the terminal is resized"

**Description:** Scroll position may jump or reset when terminal is resized.

**Relevant to ragent:** Yes - TUI resize handling

**Affected Areas:**
- `crates/ragent-tui/src/app.rs`
- `crates/ragent-tui/src/panels/`

**Implementation Tasks:**
1. [ ] **TASK-C5-1:** Review resize event handling
2. [ ] **TASK-C5-2:** Preserve scroll position proportional to content
3. [ ] **TASK-C5-3:** Add resize handling tests

**Estimated Effort:** 4-6 hours

---

### C6. Sessions with Active Work Cleaned Up
**Copilot Reference:** v1.0.12 - "Sessions with active work are no longer cleaned up by the stale session reaper"

**Description:** Stale session cleanup may incorrectly remove active sessions.

**Relevant to ragent:** Yes - if session cleanup is implemented

**Affected Areas:**
- `crates/ragent-core/src/storage/session.rs`

**Implementation Tasks:**
1. [ ] **TASK-C6-1:** Check if ragent has stale session reaping
2. [ ] **TASK-C6-2:** If yes, add "active work" detection to prevent cleanup
3. [ ] **TASK-C6-3:** Define criteria for "active" vs "stale" sessions

**Estimated Effort:** 4-6 hours

---

### C7. MCP Server Connection Blocking
**Copilot Reference:** v1.0.15 - "MCP servers that are slow to connect no longer block the agent from starting"

**Description:** MCP server initialization can block agent startup.

**Relevant to ragent:** Yes - MCP client exists (stub)

**Affected Areas:**
- `crates/ragent-core/src/mcp/`

**Implementation Tasks:**
1. [ ] **TASK-C7-1:** Review MCP client initialization
2. [ ] **TASK-C7-2:** Make MCP connection async/non-blocking
3. [ ] **TASK-C7-3:** Add timeout for MCP server connections
4. [ ] **TASK-C7-4:** Allow agent to start with MCP in "connecting" state

**Estimated Effort:** 8-12 hours

---

### C8. Relative Path Resolution in File Edits
**Copilot Reference:** v1.0.26 - "Relative paths in file edit operations resolve against the session working directory"

**Description:** Relative paths in file edit operations may not resolve correctly.

**Relevant to ragent:** Yes - file tool operations

**Affected Areas:**
- `crates/ragent-core/src/tool/edit.rs`
- `crates/ragent-core/src/tool/write.rs`

**Implementation Tasks:**
1. [ ] **TASK-C8-1:** Audit file tool path handling
2. [ ] **TASK-C8-2:** Ensure relative paths resolve against session working directory
3. [ ] **TASK-C8-3:** Add canonicalization for paths before operations
4. [ ] **TASK-C8-4:** Add tests for relative path resolution

**Estimated Effort:** 4-6 hours

---

## Part D: Low Priority Bugs (P3)

### D1. Slash-prefixed Tokens Treated as File Paths
**Copilot Reference:** v1.0.26 - "Single-segment slash-prefixed tokens (e.g. /help, /start) no longer treated as file paths in bash commands"

**Description:** Commands like `/help` in bash may be misinterpreted as file paths.

**Relevant to ragent:** Yes - path detection in bash tool

**Affected Areas:**
- `crates/ragent-core/src/tool/bash.rs`
- `crates/ragent-core/src/permission/mod.rs`

**Implementation Tasks:**
1. [ ] **TASK-D1-1:** Review path detection regex/patterns
2. [ ] **TASK-D1-2:** Exclude slash-prefixed tokens that match known commands
3. [ ] **TASK-D1-3:** Add tests for path detection edge cases

**Estimated Effort:** 2-4 hours

---

### D2. Model Metadata Handling
**Copilot Reference:** v1.0.22 - "Custom agent model field now accepts display names and vendor suffixes"

**Description:** Model display names with vendor suffixes may not be handled correctly.

**Relevant to ragent:** Yes - provider configuration

**Affected Areas:**
- `crates/ragent-core/src/provider/mod.rs`
- `crates/ragent-core/src/config.rs`

**Implementation Tasks:**
1. [ ] **TASK-D2-1:** Review model identifier parsing
2. [ ] **TASK-D2-2:** Support vendor suffixes and display names
3. [ ] **TASK-D2-3:** Add model alias resolution

**Estimated Effort:** 4-6 hours

---

### D3. postToolUse Hooks Modified Args
**Copilot Reference:** v1.0.24 - "preToolUse hooks now respect modifiedArgs/updatedInput, and additionalContext fields"

**Description:** Hooks may not properly receive modified arguments from other hooks.

**Relevant to ragent:** Yes - hook system

**Affected Areas:**
- `crates/ragent-core/src/hook/` (if exists)

**Implementation Tasks:**
1. [ ] **TASK-D3-1:** Check if ragent has hook system implemented
2. [ ] **TASK-D3-2:** If yes, ensure hook argument propagation works correctly

**Estimated Effort:** 4-6 hours

---

### D4. Sub-agent ID Human-Readability
**Copilot Reference:** v1.0.6 - "Sub-agents launched by the task tool are assigned human-readable IDs based on their name instead of generic identifiers"

**Description:** Sub-agent IDs should be human-readable (e.g., `math-helper-0` vs `agent-0`).

**Relevant to ragent:** Yes - sub-agent naming

**Affected Areas:**
- `crates/ragent-core/src/agent/orchestrator.rs`
- `crates/ragent-core/src/tool/new_task.rs`

**Implementation Tasks:**
1. [ ] **TASK-D4-1:** Review sub-agent ID generation
2. [ ] **TASK-D4-2:** Generate IDs based on agent name/purpose
3. [ ] **TASK-D4-3:** Ensure uniqueness while maintaining readability

**Estimated Effort:** 2-4 hours

---

## Summary Table

| Bug ID | Description | Severity | Effort (hrs) | Status |
|--------|-------------|----------|--------------|--------|
| A1 | Session Corruption on Resume | P0 | 8-12 | ⏳ Not Started |
| A2 | Session Data Loss on Exit | P0 | 6-8 | ⏳ Not Started |
| A3 | HTTP/2 Race Conditions | P0 | 12-16 | ⏳ Not Started |
| B1 | Terminal State Restoration | P1 | 4-6 | ⏳ Not Started |
| B2 | Context Compaction Tool Splits | P1 | 8-10 | ⏳ Not Started |
| B3 | Session Scope Selector | P1 | 4-6 | ⏳ Not Started |
| B4 | Config Dir Ignored on Resume | P1 | 4-6 | ⏳ Not Started |
| B5 | Permission Hook Suppression | P1 | 6-8 | ⏳ Not Started |
| C1 | Background Agent Notifications | P2 | 4-6 | ⏳ Not Started |
| C2 | Sub-agent Tool Name Dupes | P2 | 2-4 | ⏳ Not Started |
| C3 | Timeline Blank on Shrink | P2 | 4-6 | ⏳ Not Started |
| C4 | Spinner Output Delays | P2 | 4-6 | ⏳ Not Started |
| C5 | Scroll Position on Resize | P2 | 4-6 | ⏳ Not Started |
| C6 | Active Session Cleanup | P2 | 4-6 | ⏳ Not Started |
| C7 | MCP Connection Blocking | P2 | 8-12 | ⏳ Not Started |
| C8 | Relative Path Resolution | P2 | 4-6 | ⏳ Not Started |
| D1 | Slash Tokens as Paths | P3 | 2-4 | ⏳ Not Started |
| D2 | Model Metadata | P3 | 4-6 | ⏳ Not Started |
| D3 | Hook Modified Args | P3 | 4-6 | ⏳ Not Started |
| D4 | Sub-agent Readable IDs | P3 | 2-4 | ⏳ Not Started |

**Total Estimated Effort:** 98-140 hours

---

## Implementation Priority

### Phase 1: Critical Stability (P0)
- A1, A2, A3

### Phase 2: Core Functionality (P1)
- B1, B2, B4, B5, B3

### Phase 3: UX Polish (P2)
- C1, C2, C3, C4, C5, C6, C8, C7

### Phase 4: Nice-to-Have (P3)
- D1, D2, D3, D4

---

## Notes

1. Some bugs may not apply if the corresponding feature doesn't exist in ragent
2. Effort estimates are approximate and may vary based on actual codebase complexity
3. Each task should be implemented with corresponding tests
4. Consider creating GitHub issues for tracking each task

---

## Related Documentation

- Copilot CLI releases: https://github.com/github/copilot-cli/releases
- ragent SPEC.md
- ragent AGENTS.md
