# Permission System Implementation Plan

> **Document:** PERMPLAN.md  
> **Version:** 1.0  
> **Date:** 2025-01-17  
> **Reference:** SPEC.md Section 24 — Security & Permissions

---

## Executive Summary

This plan provides a structured roadmap for ensuring complete implementation of all security and permission provisions defined in SPEC.md Section 24. The plan is organized into 8 milestones covering the full permission system, bash security layers, file path security, resource limits, credential storage, hooks, and security validation.

---

## Table of Contents

1. [Milestone 1: Core Permission System](#milestone-1-core-permission-system)
2. [Milestone 2: Bash Security (7 Layers)](#milestone-2-bash-security-7-layers)
3. [Milestone 3: File Path Security](#milestone-3-file-path-security)
4. [Milestone 4: Resource Limits & Concurrency Control](#milestone-4-resource-limits--concurrency-control)
5. [Milestone 5: Encrypted Credential Storage](#milestone-5-encrypted-credential-storage)
6. [Milestone 6: Hooks System](#milestone-6-hooks-system)
7. [Milestone 7: Network & HTTP Security](#milestone-7-network--http-security)
8. [Milestone 8: YOLO Mode & Auto-Approve](#milestone-8-yolo-mode--auto-approve)
9. [Milestone 9: Testing & Validation](#milestone-9-testing--validation)

---

## Milestone 1: Core Permission System

**Goal:** Implement and validate the foundational permission checking, rule evaluation, and interactive approval flow.

### Task 1.1: Permission Type Enum

**Status:** Implementation Required  
**Priority:** 0 (Critical)

**Requirements (SPEC.md §24.2.1):**
- `Read`, `Edit`, `Bash`, `Web`, `Question`, `PlanEnter`, `PlanExit`, `Todo`, `ExternalDirectory`, `DoomLoop`, `Custom(String)` variants
- String serialization matching spec table

**Verification:**
- [ ] All 11 permission types defined in `Permission` enum
- [ ] String conversion matches spec (`"read"`, `"edit"`, `"bash"`, etc.)
- [ ] `Custom(name)` supports arbitrary permission names
- [ ] Serialization/deserialization tests pass

**Files:**
- `crates/ragent-core/src/permission/mod.rs`

---

### Task 1.2: Permission Actions

**Status:** Implementation Required  
**Priority:** 0 (Critical)

**Requirements (SPEC.md §24.2.2):**
- `Allow`, `Deny`, `Ask` action variants
- Correct precedence and behavior

**Verification:**
- [ ] `PermissionAction` enum with `Allow`, `Deny`, `Ask`
- [ ] Action semantics implemented correctly:
  - `Allow` → execute without prompt
  - `Deny` → reject without prompt
  - `Ask` → show interactive dialog
- [ ] Unit tests for each action type

**Files:**
- `crates/ragent-core/src/permission/mod.rs`

---

### Task 1.3: Permission Rules & Evaluation

**Status:** Implementation Required  
**Priority:** 0 (Critical)

**Requirements (SPEC.md §24.2.3):**
- Rule structure: `(permission, glob_pattern, action)`
- Last-match-wins evaluation order
- Wildcard permission `"*"` matches all types
- Default ruleset when no custom rules configured

**Verification:**
- [ ] `PermissionRule` struct with `permission`, `pattern`, `action` fields
- [ ] Rules evaluated in sequential order
- [ ] Last matching rule wins
- [ ] Wildcard `"*"` permission matches any permission type
- [ ] Default ruleset applied when no config present:
  - `read` / `**` → Allow
  - `edit` / `**` → Ask
  - `bash` / `*` → Ask
  - `web` / `*` → Ask
  - `plan_enter` / `*` → Ask
  - `todo` / `*` → Allow
- [ ] Tests for rule precedence and matching

**Files:**
- `crates/ragent-core/src/permission/mod.rs`
- `crates/ragent-core/src/config.rs` (default ruleset)

---

### Task 1.4: Permission Checker

**Status:** Implementation Required  
**Priority:** 0 (Critical)

**Requirements (SPEC.md §24.2.5):**
- Check always-grants first (session-lifetime permanent grants)
- Evaluate static ruleset in order
- Fallback to `Ask` if no rule matches

**Verification:**
- [ ] `PermissionChecker` struct implemented
- [ ] `check(permission, resource)` method returns `PermissionAction`
- [ ] Always-grants checked before rules
- [ ] Static ruleset evaluated sequentially
- [ ] Fallback to `Ask` when no match
- [ ] Unit tests for all evaluation paths

**Files:**
- `crates/ragent-core/src/permission/mod.rs`

---

### Task 1.5: Permission Request Flow

**Status:** Implementation Required  
**Priority:** 0 (Critical)

**Requirements (SPEC.md §24.3):**
- Tool invocation triggers permission check
- Pre-tool-use hooks evaluated first
- Permission request sent to TUI on `Ask`
- User decision: Once / Always / Deny
- "Always" grants recorded in checker

**Verification:**
- [ ] `PermissionRequest` struct with all required fields:
  - `id`, `session_id`, `permission`, `patterns`, `metadata`, `tool_call_id`
- [ ] Request published as event to TUI
- [ ] User decision types: Once, Always, Deny
- [ ] "Always" grants stored in `PermissionChecker.always_grants`
- [ ] Grants persist for session lifetime only
- [ ] Integration test for full request flow

**Files:**
- `crates/ragent-core/src/permission/mod.rs`
- `crates/ragent-core/src/event/mod.rs` (PermissionRequest event)
- `crates/ragent-core/src/session/processor.rs` (tool execution flow)

---

### Task 1.6: Permission Queue (TUI)

**Status:** Implementation Required  
**Priority:** 1 (High)

**Requirements (SPEC.md §24.3.3):**
- FIFO queue for multiple simultaneous requests
- Deduplication by `(session_id, permission, first_pattern)`
- Queue depth indicator in UI
- Front request rendered as active dialog

**Verification:**
- [ ] `VecDeque<PermissionRequest>` in TUI state
- [ ] Deduplication logic on enqueue
- [ ] Queue depth displayed in UI (e.g. "Permission: bash (3 queued)")
- [ ] Pop front on user decision
- [ ] Next request becomes active
- [ ] Manual TUI test for queue behavior

**Files:**
- `crates/ragent-tui/src/app.rs`
- `crates/ragent-tui/src/input.rs` (permission dialog rendering)

---

### Task 1.7: Agent Profile Permissions

**Status:** Implementation Required  
**Priority:** 1 (High)

**Requirements (SPEC.md §24.8):**
- Built-in agent default permissions
- Custom agent permission rules (OASF JSON)
- Permission merge order: built-in → global config → agent-specific

**Verification:**
- [ ] Built-in agents have default permission rules:
  - `coder`: edit/bash/web → Ask
  - `reviewer`: edit/bash/web → Ask
  - `researcher`: edit/bash → Deny, web → Ask
  - `librarian`: edit/bash → Deny, web → Ask
- [ ] OASF agent profiles support `permissions` field
- [ ] Permission merging in correct precedence order
- [ ] Tests for permission inheritance and override

**Files:**
- `crates/ragent-core/src/agent/mod.rs`
- `crates/ragent-core/src/agent/oasf.rs` (OASF loader)

---

## Milestone 2: Bash Security (7 Layers)

**Goal:** Implement all 7 independent bash security layers as specified.

### Task 2.1: Layer 1 — Safe Command Whitelist

**Status:** Implementation Required  
**Priority:** 0 (Critical)

**Requirements (SPEC.md §24.4 Layer 1):**
- Auto-approve commands matching safe list (no permission prompt)
- Prefix matching (e.g. `git status` matches `git`)
- Safe commands: ls, cd, pwd, mkdir, touch, cp, mv, cat, head, tail, grep, egrep, fgrep, find, rg, wc, git, gh, cargo, rustc, rustfmt, clippy-driver, npm, yarn, pnpm, node, npx, python3, python, pip, pip3, make, docker-compose, echo, printf, chmod, jq, yq, sed, awk, sort, uniq, cut, tr, xargs, date, which, tree, diff, patch
- **Note:** `rm` intentionally excluded

**Verification:**
- [ ] Safe command list defined as constant or config
- [ ] Prefix matching function implemented
- [ ] Matching commands skip permission prompt
- [ ] `rm` NOT in safe list
- [ ] Unit tests for safe command matching
- [ ] Integration test: `ls -la` executes without prompt

**Files:**
- `crates/ragent-core/src/tool/bash.rs`

---

### Task 2.2: Layer 2 — Banned Commands

**Status:** Implementation Required  
**Priority:** 0 (Critical)

**Requirements (SPEC.md §24.4 Layer 2):**
- Word-boundary matching to avoid false positives
- Banned commands: curl, wget, nc, netcat, telnet, axel, aria2c, lynx, w3m, nmap, masscan, nikto, sqlmap, hydra, john, hashcat, aircrack, metasploit, msfconsole, msfvenom, burpsuite, ettercap, arpspoof, tcpdump, wireshark
- Reject immediately (unless YOLO mode enabled)

**Verification:**
- [ ] Banned command list defined
- [ ] Word-boundary matching implemented (e.g. `/usr/bin/curl_helper` does NOT match `curl`)
- [ ] Rejection with clear error message
- [ ] YOLO mode bypass functional
- [ ] Unit tests for banned command detection
- [ ] Tests for word-boundary edge cases

**Files:**
- `crates/ragent-core/src/tool/bash.rs`

---

### Task 2.3: Layer 3 — Denied Patterns

**Status:** Implementation Required  
**Priority:** 0 (Critical)

**Requirements (SPEC.md §24.4 Layer 3):**
- Substring pattern matching for destructive patterns
- Heredoc body content stripped before matching
- Denied patterns:
  - `rm -rf /`, `rm -r -f /`, `rm -fr /`, `rm -Rf /`, `rmdir /`, `rm -rf ~`, `rm -rf $HOME`, `rm -rf .`
  - `mkfs`, `dd if=`, `wipefs`, `shred /dev`
  - `> /dev/sd`, `> /dev/nvme`, `> /dev/vd`
  - `:(){ :|:&};:`
  - `sudo`, `su -`, `su root`, `doas`, `chmod -R 777 /`, `chmod 000 /`, `chmod -R 000`, `chown -R`
  - `.bash_history`, `.ssh/id_`
  - `insmod`, `modprobe -r`, `sysctl -w`
  - `useradd`, `usermod`, `groupadd`, `passwd`
  - `visudo`, `crontab -`, `systemctl disable`, `systemctl mask`, `chattr +i`
  - `git push --force`, `git push -f`, `git push origin --delete`
  - `grub-install`, `efibootmgr`
  - `> /dev/tcp`, `bash -i >&`, `/dev/tcp/`, `/dev/udp/`
  - `curl.*etc/shadow`, `wget.*etc/shadow`

**Verification:**
- [ ] All denied patterns defined
- [ ] Heredoc stripping implemented (avoid false positives in string literals)
- [ ] Substring matching functional
- [ ] YOLO mode bypass functional
- [ ] Unit tests for each pattern category
- [ ] Tests for heredoc handling

**Files:**
- `crates/ragent-core/src/tool/bash.rs`

---

### Task 2.4: Layer 4 — Directory Escape Prevention

**Status:** Implementation Required  
**Priority:** 0 (Critical)

**Requirements (SPEC.md §24.4 Layer 4):**
- Detect `cd` and `pushd` commands
- Reject parent directory traversal (`cd ..`, `cd ../..`)
- Reject absolute paths outside working directory
- Reject home directory escape (`cd ~`, `cd $HOME`)
- Allow paths within working directory
- Path validation via `canonicalize()` to resolve symlinks
- Allow single-segment slash-prefixed tokens (treated as commands like `/help`)

**Verification:**
- [ ] `cd` and `pushd` command detection
- [ ] Parent traversal rejection (`cd ..`)
- [ ] Absolute path validation
- [ ] Home directory escape rejection
- [ ] Symlink resolution via `canonicalize()`
- [ ] Single-segment tokens allowed (e.g. `/help`)
- [ ] Unit tests for all escape patterns
- [ ] Test: `cd /project/subdir` allowed if within root

**Files:**
- `crates/ragent-core/src/tool/bash.rs`

---

### Task 2.5: Layer 5 — Syntax Validation

**Status:** Implementation Required  
**Priority:** 0 (Critical)

**Requirements (SPEC.md §24.4 Layer 5):**
- Pre-execution syntax check via `sh -n -c <command>`
- 1-second timeout
- Invalid syntax rejected before execution

**Verification:**
- [ ] `sh -n -c` invocation implemented
- [ ] 1-second timeout enforced
- [ ] Invalid syntax rejection
- [ ] Clear error message returned
- [ ] Unit tests for syntax errors
- [ ] Test: `echo "hello` (unclosed quote) rejected

**Files:**
- `crates/ragent-core/src/tool/bash.rs`

---

### Task 2.6: Layer 6 — Obfuscation Detection

**Status:** Implementation Required  
**Priority:** 1 (High)

**Requirements (SPEC.md §24.4 Layer 6):**
- Detect obfuscation patterns:
  - `base64 ... | bash`
  - `python -c "exec(...)"`
  - `$'\x72\x6d'` (hex escape sequences)
  - `eval $(...)`
- Reject obfuscated commands (unless YOLO mode)

**Verification:**
- [ ] All obfuscation patterns defined
- [ ] Pattern matching implemented
- [ ] Rejection with reason
- [ ] YOLO mode bypass functional
- [ ] Unit tests for each pattern

**Files:**
- `crates/ragent-core/src/tool/bash.rs`

---

### Task 2.7: Layer 7 — User Allowlist / Denylist

**Status:** Implementation Required  
**Priority:** 1 (High)

**Requirements (SPEC.md §24.4 Layer 7):**
- `/bash add allow <cmd>` — exempt from banned commands check
- `/bash add deny <cmd>` — add custom denial pattern
- `/bash remove allow <cmd>` — re-enable ban
- `/bash remove deny <cmd>` — remove custom denial
- Configuration via `ragent.json`:
  ```jsonc
  {
    "bash": {
      "allowlist": ["curl", "wget"],
      "denylist": ["git push -f", "rm -rf"]
    }
  }
  ```
- Merge global and project-level configs

**Verification:**
- [ ] Slash commands implemented in TUI
- [ ] Config file parsing for `bash.allowlist` and `bash.denylist`
- [ ] Allowlist exempts from banned commands only
- [ ] Denylist adds custom denial patterns
- [ ] Config merging (global + project)
- [ ] Unit tests for allowlist/denylist logic
- [ ] Integration tests for slash commands

**Files:**
- `crates/ragent-core/src/tool/bash.rs`
- `crates/ragent-core/src/config.rs`
- `crates/ragent-tui/src/input.rs` (slash commands)

---

### Task 2.8: Bash Security Integration Test

**Status:** Implementation Required  
**Priority:** 1 (High)

**Requirements:**
- End-to-end test verifying all 7 layers execute in order
- Test that any single layer can reject a command
- Test layered bypass (e.g. safe whitelist skips other checks)

**Verification:**
- [ ] Integration test simulating real bash tool invocation
- [ ] Tests for each layer independently
- [ ] Test for layer ordering
- [ ] Test for YOLO mode bypass

**Files:**
- `crates/ragent-core/tests/test_bash_security.rs`

---

## Milestone 3: File Path Security

**Goal:** Implement file path guards, snapshots, caching, and large file handling.

### Task 3.1: Directory Escape Guard

**Status:** Implementation Required  
**Priority:** 0 (Critical)

**Requirements (SPEC.md §24.5):**
- `check_path_within_root()` function
- Ensure all file operations stay within project root
- Use `canonicalize()` to resolve symlinks

**Verification:**
- [ ] Function implemented in file tools module
- [ ] Symlink resolution via `canonicalize()`
- [ ] Rejection of paths outside root
- [ ] Unit tests for escape attempts:
  - `../../../etc/passwd`
  - Symlinks pointing outside root
  - Absolute paths
- [ ] Integration tests in file tool tests

**Files:**
- `crates/ragent-core/src/tool/read.rs`
- `crates/ragent-core/src/tool/write.rs`
- `crates/ragent-core/src/tool/edit.rs`
- All file operation tools

---

### Task 3.2: Wildcard Restriction in `rm`

**Status:** Implementation Required  
**Priority:** 0 (Critical)

**Requirements (SPEC.md §24.5):**
- `rm` tool must reject wildcard patterns
- Single file path only

**Verification:**
- [ ] `rm` tool rejects `*`, `?`, `[...]` patterns
- [ ] Clear error message returned
- [ ] Unit tests for wildcard rejection

**Files:**
- `crates/ragent-core/src/tool/rm.rs`

---

### Task 3.3: Edit Snapshots

**Status:** Implementation Required  
**Priority:** 1 (High)

**Requirements (SPEC.md §24.5):**
- Snapshot file contents before edits
- Support rollback via `/undo` command

**Verification:**
- [ ] Snapshot created before edit execution
- [ ] Snapshots stored in session or global state
- [ ] `/undo` command restores snapshot
- [ ] Tests for snapshot creation and restore

**Files:**
- `crates/ragent-core/src/tool/edit.rs`
- `crates/ragent-core/src/snapshot.rs` (if separate module)
- `crates/ragent-tui/src/input.rs` (`/undo` command)

---

### Task 3.4: LRU Read Cache

**Status:** Implementation Required  
**Priority:** 2 (Medium)

**Requirements (SPEC.md §24.5):**
- 256-entry LRU cache for file reads
- Key: `(path, mtime)`
- Invalidate on file modification

**Verification:**
- [ ] LRU cache implemented (e.g. using `lru` crate)
- [ ] 256-entry limit enforced
- [ ] Cache keyed on `(path, mtime)`
- [ ] Cache invalidation on write/edit
- [ ] Benchmark or unit test showing cache hits

**Files:**
- `crates/ragent-core/src/tool/read.rs`

---

### Task 3.5: Large File Handling

**Status:** Implementation Required  
**Priority:** 1 (High)

**Requirements (SPEC.md §24.5):**
- Files >100 lines return:
  - First 100 lines
  - Section map
- Prevent accidental consumption of enormous files

**Verification:**
- [ ] File line count checked before read
- [ ] Return structure includes first 100 lines + section map
- [ ] Section map generation from file structure (headings, function definitions, etc.)
- [ ] Tests for large file handling

**Files:**
- `crates/ragent-core/src/tool/read.rs`

---

## Milestone 4: Resource Limits & Concurrency Control

**Goal:** Implement semaphore-based resource limits for processes and tool calls.

### Task 4.1: Child Process Semaphore

**Status:** Implementation Required  
**Priority:** 0 (Critical)

**Requirements (SPEC.md §24.6):**
- 16-permit semaphore for concurrent child processes
- Applied to: bash commands, dynamic context commands, MCP servers
- Async wait for permit, release on completion

**Verification:**
- [ ] Global or app-level `Semaphore` with 16 permits
- [ ] Bash tool acquires permit before execution
- [ ] Dynamic context commands acquire permit
- [ ] MCP servers acquire permit
- [ ] Permit released on process completion
- [ ] Load test: spawn 20 bash commands, verify only 16 concurrent
- [ ] Unit tests for semaphore logic

**Files:**
- `crates/ragent-core/src/tool/bash.rs`
- `crates/ragent-core/src/orchestrator/mod.rs` (if used for dynamic context)
- `crates/ragent-core/src/mcp/mod.rs`

---

### Task 4.2: Tool Call Semaphore

**Status:** Implementation Required  
**Priority:** 0 (Critical)

**Requirements (SPEC.md §24.6):**
- 5-permit semaphore for concurrent tool calls within a single agent turn
- Applied to all tools
- Async wait for permit, release on completion

**Verification:**
- [ ] Global or session-level `Semaphore` with 5 permits
- [ ] Tool execution acquires permit before invocation
- [ ] Permit released on tool completion (success or error)
- [ ] Test: parallel tool calls respect limit
- [ ] Unit tests for tool concurrency

**Files:**
- `crates/ragent-core/src/session/processor.rs` (tool execution loop)

---

### Task 4.3: Resource Limit Tests

**Status:** Implementation Required  
**Priority:** 1 (High)

**Requirements:**
- Integration tests for semaphore enforcement
- Verify no deadlocks or permit leaks

**Verification:**
- [ ] Test: 20 concurrent bash calls, verify max 16 active
- [ ] Test: 10 concurrent tools, verify max 5 active
- [ ] Test: permit released on error
- [ ] Test: permit released on timeout

**Files:**
- `crates/ragent-core/tests/test_resource_limits.rs`

---

## Milestone 5: Encrypted Credential Storage

**Goal:** Validate encrypted storage implementation for API keys and tokens.

### Task 5.1: Encryption Architecture Validation

**Status:** Validation Required  
**Priority:** 0 (Critical)

**Requirements (SPEC.md §24.13):**
- blake3 key derivation in XOF mode
- Key derivation input: `"ragent credential encryption v2"`, `"{username}:{home_dir}"`
- 16-byte random nonce per encryption
- XOR cipher with blake3 keystream
- Format: `v2:<base64(nonce || ciphertext)>`
- Legacy v1 → v2 auto-migration

**Verification:**
- [ ] `encrypt_key()` function implemented with spec algorithm
- [ ] `decrypt_key()` function implemented
- [ ] `generate_keystream()` using blake3 XOF
- [ ] Nonce generation (16-byte random)
- [ ] v2 format encoding/decoding
- [ ] v1 → v2 migration on read
- [ ] Unit tests for encrypt/decrypt round-trip
- [ ] Test: encrypted value unreadable without correct username/home_dir

**Files:**
- `crates/ragent-core/src/storage/mod.rs`

---

### Task 5.2: Database Schema Validation

**Status:** Validation Required  
**Priority:** 0 (Critical)

**Requirements (SPEC.md §24.13):**
- `provider_auth` table: `provider_id`, `api_key`, `updated_at`
- `settings` table: `key`, `value`, `updated_at`
- `api_key` field contains encrypted values with `v2:` prefix

**Verification:**
- [ ] SQLite schema matches spec
- [ ] `provider_auth` table exists
- [ ] `settings` table exists
- [ ] `api_key` column stores encrypted values
- [ ] `updated_at` timestamp updates on write

**Files:**
- `crates/ragent-core/src/storage/mod.rs` (schema creation)

---

### Task 5.3: Storage API Validation

**Status:** Validation Required  
**Priority:** 0 (Critical)

**Requirements (SPEC.md §24.13):**
- `set_provider_auth(provider_id, api_key)` — encrypt and store
- `get_provider_auth(provider_id) -> Option<String>` — decrypt and return, auto-migrate v1 → v2
- `delete_provider_auth(provider_id)` — remove credential
- `seed_secret_registry()` — load all into redaction registry
- `set_setting(key, value)`, `get_setting(key)`, `delete_setting(key)` — plaintext settings

**Verification:**
- [ ] All API methods implemented
- [ ] `set_provider_auth` encrypts before storing
- [ ] `get_provider_auth` decrypts after loading
- [ ] v1 → v2 migration in `get_provider_auth`
- [ ] `seed_secret_registry` loads all credentials for redaction
- [ ] Settings API methods functional
- [ ] Unit tests for all methods

**Files:**
- `crates/ragent-core/src/storage/mod.rs`

---

### Task 5.4: Caller Integration Validation

**Status:** Validation Required  
**Priority:** 1 (High)

**Requirements (SPEC.md §24.13):**
- Provider auth callers:
  - Provider setup dialog (TUI)
  - Provider removal (TUI)
  - Copilot OAuth flow (TUI)
  - Session processor (LLM client creation)
  - GitLab auth
- Settings callers:
  - Model selection
  - Copilot/Generic OpenAI API base
  - Provider disable flags
  - GitLab config
  - Memory system

**Verification:**
- [ ] Provider setup dialog uses `set_provider_auth`
- [ ] Provider removal uses `delete_provider_auth`
- [ ] Copilot OAuth stores token via `set_provider_auth`
- [ ] Session processor reads credentials via `get_provider_auth`
- [ ] GitLab auth uses storage API
- [ ] Model selection persists via `set_setting`
- [ ] All settings callers use `set_setting`/`get_setting`
- [ ] Integration test for credential lifecycle

**Files:**
- `crates/ragent-tui/src/input.rs`
- `crates/ragent-tui/src/app.rs`
- `crates/ragent-core/src/session/processor.rs`
- `crates/ragent-core/src/gitlab/auth.rs`
- `crates/ragent-core/src/memory/extract.rs`

---

### Task 5.5: Layered Credential Resolution Validation

**Status:** Validation Required  
**Priority:** 1 (High)

**Requirements (SPEC.md §24.13):**
- Resolution order:
  1. Environment variables
  2. ragent.json configuration
  3. Encrypted database
- Applied to all LLM providers and GitLab

**Verification:**
- [ ] Environment variable checked first
- [ ] Config file checked second
- [ ] Database checked last
- [ ] Tests for each resolution layer
- [ ] Test: env var takes precedence over DB
- [ ] Test: config file takes precedence over DB

**Files:**
- `crates/ragent-core/src/provider/anthropic.rs`
- `crates/ragent-core/src/provider/openai.rs`
- `crates/ragent-core/src/provider/copilot.rs`
- `crates/ragent-core/src/provider/ollama.rs`
- `crates/ragent-core/src/provider/ollama_cloud.rs`
- `crates/ragent-core/src/provider/generic_openai.rs`
- `crates/ragent-core/src/provider/gemini.rs`
- `crates/ragent-core/src/provider/huggingface.rs`
- `crates/ragent-core/src/gitlab/auth.rs`

---

## Milestone 6: Hooks System

**Goal:** Validate hooks implementation for lifecycle events and tool execution.

### Task 6.1: Hook Triggers

**Status:** Validation Required  
**Priority:** 1 (High)

**Requirements (SPEC.md §24.9):**
- Triggers: `on_session_start`, `on_session_end`, `on_error`, `on_permission_denied`, `pre_tool_use`, `post_tool_use`
- Execution models:
  - `pre_tool_use`: synchronous (can block)
  - `post_tool_use`: async spawned
  - All others: fire-and-forget

**Verification:**
- [ ] All 6 triggers defined in enum or type
- [ ] `on_session_start` fires on first user message
- [ ] `on_session_end` fires after session completes
- [ ] `on_error` fires on LLM or tool error
- [ ] `on_permission_denied` fires on permission rejection
- [ ] `pre_tool_use` fires before tool execution (synchronous)
- [ ] `post_tool_use` fires after tool execution (async)
- [ ] Integration tests for each trigger

**Files:**
- `crates/ragent-core/src/hooks/mod.rs`
- `crates/ragent-core/src/session/processor.rs`

---

### Task 6.2: Hook Configuration

**Status:** Validation Required  
**Priority:** 1 (High)

**Requirements (SPEC.md §24.9):**
- Config structure:
  ```jsonc
  {
    "hooks": [
      {
        "trigger": "on_session_start",
        "command": "echo ...",
        "timeout_secs": 30
      }
    ]
  }
  ```
- Default timeout: 30 seconds

**Verification:**
- [ ] Hook config struct defined
- [ ] Config parsing implemented
- [ ] Default timeout = 30s
- [ ] Custom timeout from config
- [ ] Tests for config parsing

**Files:**
- `crates/ragent-core/src/config.rs`
- `crates/ragent-core/src/hooks/mod.rs`

---

### Task 6.3: Hook Environment Variables

**Status:** Validation Required  
**Priority:** 1 (High)

**Requirements (SPEC.md §24.9):**
- Base variables (all hooks):
  - `RAGENT_TRIGGER`, `RAGENT_WORKING_DIR`
- Error hooks:
  - `RAGENT_ERROR`
- Tool hooks:
  - `RAGENT_TOOL_NAME`, `RAGENT_TOOL_INPUT`
- Post-tool hooks:
  - `RAGENT_TOOL_OUTPUT`, `RAGENT_TOOL_SUCCESS`

**Verification:**
- [ ] Environment variables set correctly per trigger type
- [ ] Base variables present in all hooks
- [ ] Trigger-specific variables present
- [ ] JSON serialization for `RAGENT_TOOL_INPUT` and `RAGENT_TOOL_OUTPUT`
- [ ] Unit tests for environment variable injection

**Files:**
- `crates/ragent-core/src/hooks/mod.rs`

---

### Task 6.4: Pre-tool-use Hook Results

**Status:** Validation Required  
**Priority:** 0 (Critical)

**Requirements (SPEC.md §24.9):**
- Hook can return JSON to stdout:
  - `{"decision": "allow"}` → execute without prompt
  - `{"decision": "deny", "reason": "..."}` → block with reason
  - `{"modified_input": {...}}` → replace tool arguments
  - Empty/invalid → normal permission flow

**Verification:**
- [ ] `PreToolUseResult` enum with Allow, Deny, ModifiedInput, NoDecision variants
- [ ] JSON parsing from hook stdout
- [ ] Allow decision skips permission prompt
- [ ] Deny decision rejects tool
- [ ] ModifiedInput replaces tool arguments
- [ ] NoDecision continues normal flow
- [ ] Unit tests for each result type
- [ ] Integration test with real hook script

**Files:**
- `crates/ragent-core/src/hooks/mod.rs`
- `crates/ragent-core/src/session/processor.rs`

---

### Task 6.5: Post-tool-use Hook Results

**Status:** Validation Required  
**Priority:** 2 (Medium)

**Requirements (SPEC.md §24.9):**
- Hook can return JSON to stdout:
  - `{"modified_output": {...}}` → replace tool output
  - Empty/invalid → pass through unchanged

**Verification:**
- [ ] JSON parsing from hook stdout
- [ ] ModifiedOutput replaces tool result
- [ ] Empty output passes through unchanged
- [ ] Unit tests for output modification
- [ ] Integration test with real hook script

**Files:**
- `crates/ragent-core/src/hooks/mod.rs`
- `crates/ragent-core/src/session/processor.rs`

---

### Task 6.6: Hook Execution Model

**Status:** Validation Required  
**Priority:** 1 (High)

**Requirements (SPEC.md §24.9):**
- Synchronous: `run_pre_tool_use_hooks()` (blocks tool execution)
- Async spawned: `run_post_tool_use_hooks()` (can inspect/modify output)
- Fire-and-forget: `fire_hooks()` (never block)

**Verification:**
- [ ] `run_pre_tool_use_hooks()` is blocking
- [ ] `run_post_tool_use_hooks()` spawns async tasks
- [ ] `fire_hooks()` spawns async tasks without waiting
- [ ] Hook errors logged but never fatal
- [ ] Timeout enforcement (default 30s)
- [ ] Integration tests for execution model

**Files:**
- `crates/ragent-core/src/hooks/mod.rs`

---

## Milestone 7: Network & HTTP Security

**Goal:** Validate HTTP client configuration, retry policy, and credential handling.

### Task 7.1: HTTP Client Configuration

**Status:** Validation Required  
**Priority:** 1 (High)

**Requirements (SPEC.md §24.10):**
- Connection pool per host: 8
- Pool idle timeout: 90s
- Connect timeout: 30s
- Request timeout: 120s
- Streaming timeout: None (per-chunk managed by provider)
- TCP keep-alive: 60s

**Verification:**
- [ ] HTTP client configuration matches spec values
- [ ] Connection pooling enforced
- [ ] Timeouts applied correctly
- [ ] Streaming requests have per-chunk timeout
- [ ] TCP keep-alive functional
- [ ] Tests for timeout enforcement

**Files:**
- `crates/ragent-core/src/llm/client.rs` (if separate module)
- `crates/ragent-core/src/provider/mod.rs`

---

### Task 7.2: Retry Policy

**Status:** Validation Required  
**Priority:** 1 (High)

**Requirements (SPEC.md §24.10):**
- Max retries: 4
- Backoff: Exponential (1s, 2s, 4s, 8s)
- Retry on: 5xx, connection failures, timeouts, HTTP/2 errors, body decoding errors, broken pipes, unexpected EOF
- No retry on: 4xx errors

**Verification:**
- [ ] Retry middleware or interceptor implemented
- [ ] Exponential backoff with correct delays
- [ ] 5xx errors retried
- [ ] 4xx errors NOT retried
- [ ] Connection failures retried
- [ ] Max 4 retries enforced
- [ ] Unit tests for retry logic

**Files:**
- `crates/ragent-core/src/llm/client.rs` (if separate module)
- `crates/ragent-core/src/provider/mod.rs`

---

### Task 7.3: Credential Handling Validation

**Status:** Validation Required  
**Priority:** 0 (Critical)

**Requirements (SPEC.md §24.10):**
- API keys stored in encrypted database (not plain-text files)
- Copilot uses device flow OAuth with short-lived tokens
- Secrets redacted in logs and SSE payloads

**Verification:**
- [ ] No plain-text credential files created
- [ ] All credentials stored via `set_provider_auth()`
- [ ] Copilot OAuth flow uses device flow
- [ ] Copilot tokens cached in memory
- [ ] Secret redaction functional (see Task 7.4)
- [ ] Integration test for credential lifecycle

**Files:**
- `crates/ragent-core/src/storage/mod.rs`
- `crates/ragent-core/src/provider/copilot.rs`

---

### Task 7.4: Secret Redaction

**Status:** Validation Required  
**Priority:** 0 (Critical)

**Requirements (SPEC.md §24.11):**
- Patterns detected: API keys, bearer tokens, JWTs, AWS credentials, GitHub tokens, SSH keys
- Redaction: Replace with `[REDACTED]`
- Coverage: `tracing` output, SSE event payloads, error messages, TUI tool output

**Verification:**
- [ ] `redact_secrets()` function implemented
- [ ] Regex patterns for all secret types
- [ ] Applied to all log statements
- [ ] Applied to SSE event serialization
- [ ] Applied to error messages
- [ ] Applied to TUI rendering
- [ ] Unit tests for pattern matching
- [ ] Integration test: API key in log → `[REDACTED]`

**Files:**
- `crates/ragent-core/src/redact.rs` (if separate module)
- `crates/ragent-core/src/event/mod.rs` (SSE serialization)
- `crates/ragent-tui/src/app.rs` (TUI rendering)

---

## Milestone 8: YOLO Mode & Auto-Approve

**Goal:** Implement and validate YOLO mode and auto-approve mechanisms.

### Task 8.1: YOLO Mode Implementation

**Status:** Implementation Required  
**Priority:** 2 (Medium)

**Requirements (SPEC.md §24.7):**
- `/yolo` slash command (requires confirmation)
- Bypasses:
  - Banned commands (with warning)
  - Denied patterns (with warning)
  - Obfuscation detection
  - User denylist
  - Dynamic context allowlist
  - MCP config validation
- Does NOT bypass:
  - Safe command whitelist
  - Directory escape prevention
  - Syntax validation
  - Resource semaphores
  - Deny permission rules
- Session-scoped (not persisted)

**Verification:**
- [ ] `/yolo` command implemented in TUI
- [ ] Confirmation dialog shown
- [ ] YOLO mode flag in session state
- [ ] Bypasses listed layers with warning
- [ ] Does NOT bypass listed protections
- [ ] Session-scoped (cleared on restart)
- [ ] Unit tests for YOLO bypass logic
- [ ] Integration test: banned command executes in YOLO

**Files:**
- `crates/ragent-tui/src/input.rs` (`/yolo` command)
- `crates/ragent-core/src/session/mod.rs` (YOLO flag)
- `crates/ragent-core/src/tool/bash.rs` (YOLO checks)

---

### Task 8.2: Auto-Approve Mode

**Status:** Validation Required  
**Priority:** 1 (High)

**Requirements (SPEC.md §24.7):**
- `ragent --yes` flag
- `RAGENT_YES=1` environment variable
- Automatically responds "Once" to all `Ask` permission prompts
- Does NOT bypass security layers (banned commands, denied patterns, etc.)

**Verification:**
- [ ] `--yes` CLI flag implemented
- [ ] `RAGENT_YES` environment variable read
- [ ] Permission prompts automatically approved with "Once"
- [ ] Security layers still enforced
- [ ] Unit tests for auto-approve logic
- [ ] Integration test: `--yes` flag auto-approves file write

**Files:**
- `crates/ragent-core/src/config.rs` (CLI flag)
- `crates/ragent-core/src/permission/mod.rs` (auto-approve logic)
- `src/main.rs` (CLI argument parsing)

---

## Milestone 9: Testing & Validation

**Goal:** Comprehensive testing to validate all security provisions are implemented and functional.

### Task 9.1: Permission System Tests

**Status:** Implementation Required  
**Priority:** 0 (Critical)

**Test Coverage:**
- [ ] Permission type serialization
- [ ] Permission action behavior (Allow, Deny, Ask)
- [ ] Rule evaluation (last-match-wins)
- [ ] Wildcard permission matching
- [ ] Default ruleset application
- [ ] Always-grant storage and retrieval
- [ ] Permission queue (FIFO, deduplication)
- [ ] Agent profile permission inheritance
- [ ] Permission merge order (built-in → config → agent)

**Files:**
- `crates/ragent-core/tests/test_permission_system.rs`

---

### Task 9.2: Bash Security Tests

**Status:** Implementation Required  
**Priority:** 0 (Critical)

**Test Coverage:**
- [ ] Layer 1: Safe command whitelist
- [ ] Layer 2: Banned commands (word-boundary)
- [ ] Layer 3: Denied patterns (heredoc handling)
- [ ] Layer 4: Directory escape prevention (symlinks, absolute paths)
- [ ] Layer 5: Syntax validation
- [ ] Layer 6: Obfuscation detection
- [ ] Layer 7: User allowlist/denylist
- [ ] Full integration: all 7 layers in sequence
- [ ] YOLO mode bypass

**Files:**
- `crates/ragent-core/tests/test_bash_security.rs`

---

### Task 9.3: File Path Security Tests

**Status:** Implementation Required  
**Priority:** 0 (Critical)

**Test Coverage:**
- [ ] Directory escape guard (canonicalize, symlinks)
- [ ] Wildcard restriction in `rm`
- [ ] Edit snapshots and rollback
- [ ] LRU cache hits and invalidation
- [ ] Large file handling (first 100 lines + section map)

**Files:**
- `crates/ragent-core/tests/test_file_security.rs`

---

### Task 9.4: Resource Limits Tests

**Status:** Implementation Required  
**Priority:** 0 (Critical)

**Test Coverage:**
- [ ] Child process semaphore (16 permits)
- [ ] Tool call semaphore (5 permits)
- [ ] Permit release on completion
- [ ] Permit release on error
- [ ] Permit release on timeout
- [ ] No deadlocks

**Files:**
- `crates/ragent-core/tests/test_resource_limits.rs`

---

### Task 9.5: Credential Storage Tests

**Status:** Implementation Required  
**Priority:** 0 (Critical)

**Test Coverage:**
- [ ] Encrypt/decrypt round-trip
- [ ] v1 → v2 migration
- [ ] Machine-local key binding (different username/home_dir cannot decrypt)
- [ ] Provider auth API (set, get, delete)
- [ ] Settings API (set, get, delete)
- [ ] Secret registry seeding
- [ ] Layered credential resolution (env → config → DB)

**Files:**
- `crates/ragent-core/tests/test_credential_storage.rs`

---

### Task 9.6: Hooks Tests

**Status:** Implementation Required  
**Priority:** 1 (High)

**Test Coverage:**
- [ ] All 6 triggers fire correctly
- [ ] Environment variables set per trigger
- [ ] Pre-tool-use hook results (Allow, Deny, ModifiedInput, NoDecision)
- [ ] Post-tool-use hook results (ModifiedOutput)
- [ ] Execution models (synchronous, async, fire-and-forget)
- [ ] Timeout enforcement
- [ ] Error handling (hooks never fatal)

**Files:**
- `crates/ragent-core/tests/test_hooks.rs`

---

### Task 9.7: Network Security Tests

**Status:** Implementation Required  
**Priority:** 1 (High)

**Test Coverage:**
- [ ] HTTP client configuration (timeouts, pool size)
- [ ] Retry policy (exponential backoff, max retries)
- [ ] Retry on 5xx, no retry on 4xx
- [ ] Secret redaction in logs and SSE events
- [ ] Credential storage (no plain-text files)

**Files:**
- `crates/ragent-core/tests/test_network_security.rs`

---

### Task 9.8: YOLO & Auto-Approve Tests

**Status:** Implementation Required  
**Priority:** 2 (Medium)

**Test Coverage:**
- [ ] YOLO mode bypasses correct layers
- [ ] YOLO mode does NOT bypass listed protections
- [ ] YOLO mode confirmation dialog
- [ ] Auto-approve flag (`--yes`)
- [ ] Auto-approve environment variable (`RAGENT_YES`)
- [ ] Auto-approve does NOT bypass security layers

**Files:**
- `crates/ragent-core/tests/test_yolo_autoapprove.rs`

---

### Task 9.9: End-to-End Security Validation

**Status:** Implementation Required  
**Priority:** 0 (Critical)

**Test Coverage:**
- [ ] Full session with multiple tool calls
- [ ] Permission requests queued correctly
- [ ] Bash security layers enforce correctly
- [ ] File path guards prevent escapes
- [ ] Resource limits enforced
- [ ] Hooks fire at correct times
- [ ] Credentials never leaked in logs

**Files:**
- `crates/ragent-core/tests/test_e2e_security.rs`

---

## Summary

| Milestone | Tasks | Priority | Status |
|-----------|-------|----------|--------|
| **M1: Core Permission System** | 7 | Critical | Implementation Required |
| **M2: Bash Security (7 Layers)** | 8 | Critical | Implementation Required |
| **M3: File Path Security** | 5 | Critical | Implementation Required |
| **M4: Resource Limits** | 3 | Critical | Implementation Required |
| **M5: Encrypted Credential Storage** | 5 | Critical | Validation Required |
| **M6: Hooks System** | 6 | High | Validation Required |
| **M7: Network & HTTP Security** | 4 | High | Validation Required |
| **M8: YOLO Mode & Auto-Approve** | 2 | Medium | Implementation Required |
| **M9: Testing & Validation** | 9 | Critical | Implementation Required |

**Total Tasks:** 49

---

## Next Steps

1. **Audit Current Implementation** — For each milestone, run the verification checklist against the existing codebase to determine what is already implemented vs. missing.
2. **Prioritize Critical Milestones** — Focus on M1-M4 first (permission system, bash security, file security, resource limits) as they are foundational.
3. **Create Task Issues** — Convert each task into a GitHub issue with the verification checklist as acceptance criteria.
4. **Implement & Test** — Work through tasks sequentially, ensuring tests pass before moving to the next.
5. **Update Documentation** — Keep SPEC.md and this plan in sync as implementation progresses.

---

*End of Plan*
