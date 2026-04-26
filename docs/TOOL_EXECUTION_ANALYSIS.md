# Tool Execution Flow Analysis

**Date:** 2025-01-16  
**Purpose:** Detailed technical analysis of tool execution flow for permission enforcement implementation

---

## Current Tool Execution Flow

**File:** `crates/ragent-core/src/session/processor.rs`  
**Method:** `process_user_message` → agent loop → tool execution tasks

### Key Components

1. **SessionProcessor Structure (line 178-209)**
   - Contains `permission_checker: Arc<tokio::sync::RwLock<PermissionChecker>>`
   - Already instantiated but **never used**

2. **Tool Execution Location (lines 1180-1260)**
   ```rust
   // Line 1191-1197: Tool lookup and execution
   let result = registry
       .get(&tc_clone.name)
       .ok_or_else(|| anyhow::anyhow!("Unknown tool: {}", tc_clone.name));
   let result = match result {
       Ok(tool) => tool.execute(tool_input, &tool_ctx).await,  // ⚠️ NO PERMISSION CHECK
       Err(e) => Err(e),
   };
   ```

3. **Tool Trait (line 332-347 in tool/mod.rs)**
   ```rust
   #[async_trait::async_trait]
   pub trait Tool: Send + Sync {
       fn name(&self) -> &str;
       fn description(&self) -> &str;
       fn parameters_schema(&self) -> Value;
       fn permission_category(&self) -> &str;  // ✅ Already exists
       async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput>;
   }
   ```

---

## Permission Event Flow (Working Reference: question tool)

**File:** `crates/ragent-core/src/tool/question.rs:60-92`

```rust
// 1. Generate request ID
let request_id = uuid::Uuid::new_v4().to_string();

// 2. Subscribe to event bus BEFORE publishing
let mut rx = ctx.event_bus.subscribe();

// 3. Publish permission request
ctx.event_bus.publish(Event::PermissionRequested {
    session_id: ctx.session_id.clone(),
    request_id: request_id.clone(),
    permission: "question".to_string(),
    description: question.to_string(),
});

// 4. Wait for reply event
loop {
    match rx.recv().await {
        Ok(Event::PermissionReplied {
            request_id: rid,
            allowed,
            ..
        }) if rid == request_id => {
            if !allowed {
                anyhow::bail!("User denied permission");
            }
            break;
        }
        Ok(Event::UserInput { request_id: rid, text }) if rid == request_id => {
            return Ok(ToolOutput { content: text, metadata: None });
        }
        Err(RecvError::Lagged(_)) => continue,
        Err(_) => anyhow::bail!("Event bus closed"),
        _ => continue,
    }
}
```

---

## Implementation Strategy

### Challenges

1. **Async Context:** Tool execution happens in spawned tasks (line 1156-1290)
2. **No Deadlock:** Must subscribe to event bus BEFORE publishing permission request
3. **Timeout:** Need 30s timeout for permission prompts
4. **Resource Extraction:** Need to extract path/command/url from tool input JSON

### Solution Architecture

Add permission check **between** lines 1193 and 1195:

```rust
let result = registry
    .get(&tc_clone.name)
    .ok_or_else(|| anyhow::anyhow!("Unknown tool: {}", tc_clone.name));

// NEW: Permission check before execution
let result = match result {
    Ok(tool) => {
        // Check if permission is required
        let perm_category = tool.permission_category();
        if !perm_category.is_empty() && perm_category != "none" {
            // Extract resource from tool input
            let resource = extract_resource_from_input(&tool_input, &tc_clone.name);
            
            // Check permission via PermissionChecker
            let action = check_permission_with_prompt(
                &permission_checker,
                &event_bus,
                &session_id,
                perm_category,
                &resource,
                &tc_clone.name,
            ).await?;
            
            match action {
                PermissionAction::Deny => {
                    Err(anyhow::anyhow!("Permission denied by user or policy"))
                }
                PermissionAction::Allow => {
                    tool.execute(tool_input, &tool_ctx).await
                }
                _ => unreachable!("check_permission_with_prompt returns Allow or Deny"),
            }
        } else {
            // No permission required, execute directly
            tool.execute(tool_input, &tool_ctx).await
        }
    }
    Err(e) => Err(e),
};
```

---

## Helper Function Design

### 1. Extract Resource from Input

```rust
fn extract_resource_from_input(input: &Value, tool_name: &str) -> String {
    // Try common parameter names
    input.get("path")
        .or_else(|| input.get("command"))
        .or_else(|| input.get("url"))
        .or_else(|| input.get("pattern"))
        .or_else(|| input.get("query"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| {
            // Fallback: use tool name as resource identifier
            format!("tool:{}", tool_name)
        })
}
```

### 2. Check Permission with Prompt

```rust
async fn check_permission_with_prompt(
    checker: &Arc<tokio::sync::RwLock<PermissionChecker>>,
    event_bus: &Arc<EventBus>,
    session_id: &str,
    permission: &str,
    resource: &str,
    tool_name: &str,
) -> Result<PermissionAction> {
    // 1. Check PermissionChecker first
    let action = {
        let c = checker.read().await;
        c.check(permission, resource)
    };
    
    match action {
        PermissionAction::Allow | PermissionAction::Deny => {
            // Policy decision — no prompt needed
            Ok(action)
        }
        PermissionAction::Ask => {
            // Need user interaction
            let request_id = uuid::Uuid::new_v4().to_string();
            let mut rx = event_bus.subscribe();
            
            // Publish request
            event_bus.publish(Event::PermissionRequested {
                session_id: session_id.to_string(),
                request_id: request_id.clone(),
                permission: permission.to_string(),
                description: format!("{}: {}", tool_name, resource),
            });
            
            // Wait for reply with timeout
            let timeout = tokio::time::Duration::from_secs(30);
            let deadline = tokio::time::Instant::now() + timeout;
            
            loop {
                let recv_timeout = deadline.saturating_duration_since(tokio::time::Instant::now());
                if recv_timeout.is_zero() {
                    return Ok(PermissionAction::Deny);
                }
                
                match tokio::time::timeout(recv_timeout, rx.recv()).await {
                    Ok(Ok(Event::PermissionReplied {
                        request_id: rid,
                        allowed,
                        ..
                    })) if rid == request_id => {
                        return Ok(if allowed {
                            PermissionAction::Allow
                        } else {
                            PermissionAction::Deny
                        });
                    }
                    Ok(Err(RecvError::Lagged(_))) => continue,
                    Ok(Err(_)) => {
                        return Err(anyhow::anyhow!("Event bus closed during permission check"));
                    }
                    Err(_) => {
                        // Timeout
                        return Ok(PermissionAction::Deny);
                    }
                    _ => continue,
                }
            }
        }
    }
}
```

---

## Verification Strategy

### 1. Tools Without Permission Requirements

These tools should return empty string or "none" from `permission_category()`:
- `think`
- `task_complete`
- `memory_read`
- `list_tasks`
- `codeindex_status`

### 2. Tools Requiring Permissions

| Permission Category | Tools |
|---------------------|-------|
| `read` | read, open_file, view_file, get_file_contents, list, list_directory, glob, grep, search, pdf_read, office_read, libre_read |
| `edit` | write, create, edit, multiedit, patch, rm, append_to_file, move_file, copy_file, mkdir, pdf_write, office_write, libre_write |
| `bash` | bash, execute_bash, run_shell_command, run_terminal_cmd, bash_reset |
| `web` | webfetch, websearch, http_request |
| `question` | question, ask_user |
| `todo` | todo_write |
| `plan_enter` | plan_enter |
| `plan_exit` | plan_exit |

### 3. Test Cases

1. **Allow via policy:**
   - Config: `{ permission: "read", pattern: "**", action: "allow" }`
   - Tool: `read` with path `src/main.rs`
   - Expected: No prompt, tool executes

2. **Deny via policy:**
   - Config: `{ permission: "bash", pattern: "**", action: "deny" }`
   - Tool: `bash` with command `ls`
   - Expected: No prompt, tool returns error "Permission denied by user or policy"

3. **Ask (prompt user):**
   - Config: Empty (default to Ask)
   - Tool: `write` with path `test.txt`
   - Expected: Prompt shown, waits for user response

4. **Timeout:**
   - Config: Empty (default to Ask)
   - Tool: `bash` with command `rm -rf /`
   - Expected: Prompt shown, after 30s returns "Permission denied"

---

## Migration Checklist

- [ ] Add helper function `extract_resource_from_input`
- [ ] Add helper function `check_permission_with_prompt`
- [ ] Inject permission check into tool execution flow (processor.rs:1193-1195)
- [ ] Verify all tools have correct `permission_category()` implementations
- [ ] Add unit tests for permission checking logic
- [ ] Add integration test for full flow (policy allow/deny/ask + timeout)
- [ ] Update SECREVIEW.md with implementation status
