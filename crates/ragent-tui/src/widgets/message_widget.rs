//! Widget for rendering a single chat message.
//!
//! Formats user and assistant messages with role-colored prefixes, renders
//! tool call status indicators and reasoning blocks with distinct styles.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget, Wrap},
};

use ragent_core::message::{Message, MessagePart, Role, ToolCallStatus};

/// Helper to build a ternary for pluralization (e.g., "1 item" vs "2 items").
pub fn pluralize(count: usize, singular: &str, plural: &str) -> String {
    if count == 1 {
        format!("{} {}", count, singular)
    } else {
        format!("{} {}", count, plural)
    }
}

/// Truncate a string to a maximum length, appending ellipsis if truncated.
pub fn truncate_str(s: &str, max_len: usize) -> String {
    if s.chars().count() > max_len {
        let truncated: String = s.chars().take(max_len).collect();
        format!("{truncated}...")
    } else {
        s.to_string()
    }
}

fn truncate_json_strings(value: &serde_json::Value, max_len: usize) -> serde_json::Value {
    match value {
        serde_json::Value::String(s) => serde_json::Value::String(truncate_str(s, max_len)),
        serde_json::Value::Array(arr) => serde_json::Value::Array(
            arr.iter()
                .map(|v| truncate_json_strings(v, max_len))
                .collect(),
        ),
        serde_json::Value::Object(map) => {
            let mut out = serde_json::Map::new();
            for (k, v) in map {
                out.insert(k.clone(), truncate_json_strings(v, max_len));
            }
            serde_json::Value::Object(out)
        }
        _ => value.clone(),
    }
}

fn summarize_tool_args(input: &serde_json::Value, max_str_len: usize) -> String {
    let Some(obj) = input.as_object() else {
        return String::new();
    };
    let truncated = truncate_json_strings(input, max_str_len);
    let Some(tobj) = truncated.as_object() else {
        return String::new();
    };
    let mut keys: Vec<&String> = obj.keys().collect();
    keys.sort();
    let mut parts = Vec::new();
    for k in keys {
        if let Some(v) = tobj.get(k) {
            let value_text = match v {
                serde_json::Value::String(s) => format!("\"{s}\""),
                _ => serde_json::to_string(v).unwrap_or_else(|_| "?".to_string()),
            };
            parts.push(format!("{k}={value_text}"));
        }
    }
    parts.join(", ")
}

/// Capitalize the first letter of a tool name for display (e.g., "read" → "Read").
pub fn capitalize_tool_name(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}

/// Map an alias tool name to its canonical equivalent for display purposes.
///
/// When a model calls an alias tool (e.g. `read_file` instead of `read`),
/// both the input summary and result summary functions should produce the same
/// rich display as the canonical tool.
pub fn canonical_tool_name(tool: &str) -> &str {
    match tool {
        "view_file" | "read_file" | "get_file_contents" | "open_file" => "read",
        "list_files" | "list_directory" => "list",
        "find_files" => "glob",
        "search_in_repo" | "file_search" => "search",
        "replace_in_file" => "edit",
        "update_file" => "write",
        "run_shell_command" | "run_terminal_cmd" | "execute_bash" | "execute_code" | "run_code" => {
            "bash"
        }
        other => other,
    }
}

/// Strip the working directory prefix from a path to produce a project-relative path.
pub fn make_relative_path(path: &str, cwd: &str) -> String {
    // Expand ~ in cwd to the home directory for comparison
    let expanded_cwd = if cwd.starts_with('~') {
        if let Some(home) = std::env::var_os("HOME") {
            format!("{}{}", home.to_string_lossy(), &cwd[1..])
        } else {
            cwd.to_string()
        }
    } else {
        cwd.to_string()
    };

    let cwd_prefix = if expanded_cwd.ends_with('/') {
        expanded_cwd
    } else {
        format!("{}/", expanded_cwd)
    };
    if path.starts_with(&cwd_prefix) {
        path[cwd_prefix.len()..].to_string()
    } else {
        path.to_string()
    }
}

/// Extract a brief summary from tool input for display next to the tool name.
/// Visual Style Guide for Tool Input/Output Summaries
///
/// Tool categories with their associated emoji icons:
/// - 📄 File Operations: read, write, create, edit, patch, rm, multiedit
/// - 📁 Directory Operations: list, make_directory/mkdir
/// - ℹ️  File Info: file_info
/// - 🔍 Search Operations: search, grep, glob
/// - ⚡ Execution: bash, execute_python, str_replace_editor, calculator
/// - 🌐 Network: webfetch, websearch, http_request
/// - 🔧 Environment: get_env
/// - ❓ User Interaction: question, ask_user
/// - 💭 Reasoning: think
/// - 📝 Planning: plan_enter, plan_exit
/// - 📋 Task Management: todo_read, todo_write
/// - 🤖 Sub-agent: new_task, cancel_task, list_tasks, wait_tasks
/// - 👥 Team Coordination: team_*
/// - 🔎 LSP/Code Intelligence: lsp_*
/// - 📄 Document: office_*, pdf_*
/// - 📋 GitHub: github_issues, github_prs
/// - ✨ Utility: format, metadata, truncate, read_line_range
pub fn tool_input_summary(tool: &str, input: &serde_json::Value, cwd: &str) -> String {
    // Resolve alias tool names to their canonical equivalents so aliases get
    // the same rich display as canonical tools.
    let tool = canonical_tool_name(tool);

    // Helper: try multiple field names, return first that has a str value.
    let get_str = |keys: &[&str]| -> Option<String> {
        keys.iter()
            .find_map(|k| input.get(*k)?.as_str().map(|s| s.to_string()))
    };

    // Helper: get path and make it relative to cwd
    let get_relative_path = |keys: &[&str]| -> String {
        get_str(keys)
            .as_deref()
            .map(|p| make_relative_path(p, cwd))
            .unwrap_or_default()
    };

    // Helper: truncate to standard 50 chars
    let trunc50 = |s: &str| truncate_str(s, 50);

    match tool {
        // ═══════════════════════════════════════════════════════════════════
        // 📄 FILE OPERATIONS
        // ═══════════════════════════════════════════════════════════════════
        "read" => {
            let path = get_relative_path(&["path"]);
            if path.is_empty() {
                String::new()
            } else {
                format!("📄 {}", path)
            }
        }
        "write" | "create" => {
            let path = get_relative_path(&["path"]);
            if path.is_empty() {
                String::new()
            } else {
                format!("📄 {}", path)
            }
        }
        "edit" | "patch" => {
            let path = get_relative_path(&["path"]);
            if path.is_empty() {
                String::new()
            } else {
                format!("📄 {}", path)
            }
        }
        "rm" => {
            let path = get_relative_path(&["path"]);
            if path.is_empty() {
                String::new()
            } else {
                format!("📄 {}", path)
            }
        }
        "multiedit" => {
            let count = input
                .get("edits")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            format!("📄 {}", pluralize(count, "edit", "edits"))
        }
        "append_to_file" | "append_file" => {
            let path = get_relative_path(&["path"]);
            if path.is_empty() {
                String::new()
            } else {
                format!("📄 {}", path)
            }
        }
        "diff_files" => {
            let a = get_str(&["path_a", "file_a", "original"]).unwrap_or_default();
            let b = get_str(&["path_b", "file_b", "modified"]).unwrap_or_default();
            format!(
                "📄 {} ↔ {}",
                make_relative_path(&a, cwd),
                make_relative_path(&b, cwd)
            )
        }

        // ═══════════════════════════════════════════════════════════════════
        // 📁 DIRECTORY OPERATIONS
        // ═══════════════════════════════════════════════════════════════════
        "list" => {
            // list_directory uses "directory"; canonical list uses "path".
            let path = get_relative_path(&["path", "directory"]);
            if path.is_empty() {
                String::new()
            } else {
                format!("📁 {}", path)
            }
        }
        "make_directory" | "mkdir" => {
            let path = get_relative_path(&["path", "directory"]);
            if path.is_empty() {
                String::new()
            } else {
                format!("📁 {}", path)
            }
        }

        // ═══════════════════════════════════════════════════════════════════
        // ℹ️ FILE INFO
        // ═══════════════════════════════════════════════════════════════════
        "file_info" => {
            let path = get_relative_path(&["path"]);
            if path.is_empty() {
                String::new()
            } else {
                format!("ℹ️  {}", path)
            }
        }

        // ═══════════════════════════════════════════════════════════════════
        // 🔄 FILE MOVE/COPY
        // ═══════════════════════════════════════════════════════════════════
        "move_file" | "rename_file" => {
            let src = get_str(&["source", "src", "from", "path"]).unwrap_or_default();
            let dst = get_str(&["destination", "dst", "to"]).unwrap_or_default();
            if dst.is_empty() {
                format!("📄 {}", make_relative_path(&src, cwd))
            } else {
                format!(
                    "📄 {} → {}",
                    make_relative_path(&src, cwd),
                    make_relative_path(&dst, cwd)
                )
            }
        }
        "copy_file" => {
            let src = get_str(&["source", "src", "from", "path"]).unwrap_or_default();
            let dst = get_str(&["destination", "dst", "to"]).unwrap_or_default();
            format!(
                "📄 {} → {}",
                make_relative_path(&src, cwd),
                make_relative_path(&dst, cwd)
            )
        }

        // ═══════════════════════════════════════════════════════════════════
        // 🔍 SEARCH OPERATIONS
        // ══════════════════════════════════════���════════════════════════════
        "search" => {
            let query = get_str(&["query", "pattern"]).unwrap_or_default();
            let path = get_str(&["path"])
                .as_deref()
                .map(|p| make_relative_path(p, cwd));
            match path {
                Some(p) if !p.is_empty() => format!("🔍 \"{}\" in {}", trunc50(&query), p),
                _ => format!("🔍 \"{}\"", trunc50(&query)),
            }
        }
        "grep" => {
            let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            let path = input
                .get("path")
                .and_then(|v| v.as_str())
                .map(|p| make_relative_path(p, cwd));
            match path {
                Some(p) if !p.is_empty() => {
                    format!("🔍 \"{}\" in {}", trunc50(pattern), p)
                }
                _ => format!("🔍 \"{}\"", trunc50(pattern)),
            }
        }
        "glob" => {
            let pattern = input
                .get("pattern")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            format!("🔍 {}", pattern)
        }

        // ══��════════════════════════════════════════════════════════════════
        // ⚡ EXECUTION
        // ═══════════════════════════════════════════════════════════════════
        "bash" => {
            // Aliases may send code/cmd instead of command.
            get_str(&["command", "code", "cmd"])
                .as_deref()
                .and_then(|s| s.lines().next())
                .map(|s| format!("⚡ $ {}", trunc50(s)))
                .unwrap_or_else(|| "⚡ bash".to_string())
        }
        "execute_python" => {
            // Show first non-empty line of the code snippet.
            get_str(&["code", "script"])
                .as_deref()
                .and_then(|s| s.lines().find(|l| !l.trim().is_empty()))
                .map(|l| format!("⚡ py: {}", trunc50(l)))
                .unwrap_or_else(|| "⚡ python".to_string())
        }
        "str_replace_editor" => {
            let cmd = get_str(&["command", "cmd"]).unwrap_or_else(|| "view".to_string());
            let path = get_relative_path(&["path"]);
            format!("⚡ {}: {}", cmd, path)
        }
        "calculator" => {
            let expr = get_str(&["expression", "expr", "query"]).unwrap_or_default();
            format!("⚡ {}", trunc50(&expr))
        }

        // ═══════════════════════════════════════════════════════════════════
        // 🌐 NETWORK
        // ═══════════════════════════════════════════════════════════════════
        "http_request" | "web_request" => {
            let method = get_str(&["method"]).unwrap_or_else(|| "GET".to_string());
            let url = get_str(&["url"]).unwrap_or_default();
            format!("🌐 {} {}", method, trunc50(&url))
        }
        "webfetch" => input
            .get("url")
            .and_then(|v| v.as_str())
            .map(|u| format!("🌐 {}", trunc50(u)))
            .unwrap_or_else(|| "🌐 fetch".to_string()),
        "websearch" => input
            .get("query")
            .and_then(|v| v.as_str())
            .map(|q| format!("🌐 \"{}\"", trunc50(q)))
            .unwrap_or_else(|| "🌐 search".to_string()),

        // ═══════════════════════════════════════════════════════════════════
        // 🔧 ENVIRONMENT
        // ═══════════════════════════════════════════════════════════════════
        "get_env" => {
            let key = get_str(&["key", "name", "variable"]).unwrap_or_default();
            if key.is_empty() {
                "🔧 all vars".to_string()
            } else {
                format!("🔧 {}", key)
            }
        }
        "bash_reset" => "🔧 reset shell".to_string(),

        // ═══════════════════════════════════════════════════════════════════
        // ❓ USER INTERACTION
        // ═══════════════════════════════════════════════════════════════════
        "question" | "ask_user" => {
            let q = get_str(&["question", "query"]).unwrap_or_default();
            format!("❓ {}", trunc50(&q))
        }

        // ═══════════════════════════════════════════════════════════════════
        // 💭 REASONING
        // ═══════════════════════════════════════════════════════════════════
        "think" => {
            let thought = get_str(&["thought", "thinking", "text"]).unwrap_or_default();
            format!("💭 {}", trunc50(&thought))
        }

        // ═══════════════════════════════════════════════════════════════════
        // 📝 PLANNING
        // ═══════════════════════════════════════════════════════════════════
        "plan_enter" => {
            let task = input.get("task").and_then(|v| v.as_str()).unwrap_or("");
            format!("📝 → {}", trunc50(task))
        }
        "plan_exit" => {
            let summary = input.get("summary").and_then(|v| v.as_str()).unwrap_or("");
            format!("📝 ← {}", trunc50(summary))
        }

        // ═══════════════════════════════════════════════════════════════════
        // 📋 TASK MANAGEMENT
        // ═══════════════════════════════════════════════════════════════════
        "todo_read" => {
            let status = input
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("all");
            format!("📋 filter: {}", status)
        }
        "todo_write" => {
            let action = input.get("action").and_then(|v| v.as_str()).unwrap_or("?");
            let id = input.get("id").and_then(|v| v.as_str()).unwrap_or("");
            let title = input.get("title").and_then(|v| v.as_str()).unwrap_or("");
            match action {
                "add" => format!("📋 +{}", trunc50(&title)),
                "update" => format!("📋 ~{}", id),
                "complete" => format!("📋 ✓{}", id),
                "remove" => format!("📋 -{}", id),
                "clear" => "📋 clear all".to_string(),
                _ => format!("📋 {}", action),
            }
        }

        // ═══════════════════════════════════════════════════════════════════
        // 🤖 SUB-AGENT
        // ═══════════════════════════════════════════════════════════════════
        "new_task" => {
            let agent = input.get("agent").and_then(|v| v.as_str()).unwrap_or("?");
            let task = input.get("task").and_then(|v| v.as_str()).unwrap_or("");
            format!("🤖 {} → {}", agent, trunc50(&task))
        }
        "cancel_task" => {
            let task_id = input.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
            format!("🤖 cancel {}", &task_id[..8.min(task_id.len())])
        }
        "list_tasks" => {
            let status = input
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("all");
            format!("🤖 filter: {}", status)
        }
        "wait_tasks" => {
            let task_ids = input
                .get("task_ids")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            if task_ids > 0 {
                format!("🤖 wait on {} task(s)", task_ids)
            } else {
                "🤖 wait on all tasks".to_string()
            }
        }

        // ═══════════════════════════════════════════════════════════════════
        // 👥 TEAM COORDINATION
        // ═══════════════════════════════════════════════════════════════════
        "team_create" => {
            let name = input.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            format!("👥 create {}", name)
        }
        "team_spawn" => {
            let agent = input
                .get("agent_type")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("👥 spawn {}", agent)
        }
        "team_status" => "👥 status".to_string(),
        "team_idle" => "👥 idle".to_string(),
        "team_cleanup" => "👥 cleanup".to_string(),
        "team_broadcast" => {
            let content = input.get("content").and_then(|v| v.as_str()).unwrap_or("");
            if content.is_empty() {
                "👥 broadcast".to_string()
            } else {
                format!("👥 broadcast: {}", trunc50(content))
            }
        }
        "team_message" => {
            let to = input.get("to").and_then(|v| v.as_str()).unwrap_or("?");
            let content = input.get("content").and_then(|v| v.as_str()).unwrap_or("");
            format!("👥 msg to {}: {}", to, trunc50(content))
        }
        "team_read_messages" => "👥 read messages".to_string(),
        "team_task_create" => {
            let title = input.get("title").and_then(|v| v.as_str()).unwrap_or("");
            format!("👥 task: {}", trunc50(&title))
        }
        "team_task_list" => "👥 list tasks".to_string(),
        "team_task_claim" => {
            let task_id = input.get("task_id").and_then(|v| v.as_str());
            match task_id {
                Some(id) => format!("👥 claim {}", id),
                None => "👥 claim next".to_string(),
            }
        }
        "team_task_complete" => {
            let task_id = input.get("task_id").and_then(|v| v.as_str()).unwrap_or("?");
            format!("👥 complete {}", task_id)
        }
        "team_assign_task" => {
            let task_id = input.get("task_id").and_then(|v| v.as_str()).unwrap_or("?");
            let to = input.get("to").and_then(|v| v.as_str()).unwrap_or("?");
            format!("👥 assign {} to {}", task_id, to)
        }
        "team_submit_plan" => "👥 submit plan".to_string(),
        "team_approve_plan" => {
            let approved = input
                .get("approved")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if approved {
                "👥 approve plan".to_string()
            } else {
                "👥 reject plan".to_string()
            }
        }
        "team_memory_read" => "👥 read memory".to_string(),
        "team_memory_write" => "👥 write memory".to_string(),
        "team_wait" => {
            let agent_ids = input
                .get("agent_ids")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            if agent_ids > 0 {
                format!("👥 wait on {} agent(s)", agent_ids)
            } else {
                "👥 wait on all agents".to_string()
            }
        }
        "team_shutdown_teammate" => {
            let teammate = input
                .get("teammate")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            format!("👥 shutdown {}", teammate)
        }
        "team_shutdown_ack" => "👥 ack shutdown".to_string(),

        // ═══════════════════════════════════════════════════════════════════
        // 🔎 LSP / CODE INTELLIGENCE
        // ═══════════════════════════════════════════════════════════════════
        "lsp_definition" | "lsp_references" => {
            let path = get_relative_path(&["path"]);
            let line = input.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
            let column = input.get("column").and_then(|v| v.as_u64()).unwrap_or(0);
            if path.is_empty() {
                format!("🔎 L{}:{}", line, column)
            } else {
                format!("🔎 {} L{}:{}", path, line, column)
            }
        }
        "lsp_hover" => {
            let path = get_relative_path(&["path"]);
            let line = input.get("line").and_then(|v| v.as_u64()).unwrap_or(0);
            let column = input.get("column").and_then(|v| v.as_u64()).unwrap_or(0);
            if path.is_empty() {
                format!("🔎 L{}:{}", line, column)
            } else {
                format!("🔎 {} L{}:{}", path, line, column)
            }
        }
        "lsp_symbols" | "lsp_diagnostics" => {
            let path = get_relative_path(&["path"]);
            if path.is_empty() {
                "🔎 analyze".to_string()
            } else {
                format!("🔎 {}", path)
            }
        }

        // ═══════════════════════════════════════════════════════════════════
        // 📇 CODE INDEX
        // ═══════════════════════════════════════════════════════════════════
        "codeindex_search" => {
            let query = get_str(&["query"]).unwrap_or_default();
            let kind = get_str(&["kind"]);
            let lang = get_str(&["language"]);
            let mut label = format!("📇 \"{}\"", trunc50(&query));
            if let Some(k) = kind {
                label.push_str(&format!(" kind:{}", k));
            }
            if let Some(l) = lang {
                label.push_str(&format!(" lang:{}", l));
            }
            label
        }
        "codeindex_symbols" => {
            let name = get_str(&["name"]);
            let kind = get_str(&["kind"]);
            let file = get_str(&["file_path"]);
            let mut parts = Vec::new();
            if let Some(n) = name {
                parts.push(format!("name:{}", trunc50(&n)));
            }
            if let Some(k) = kind {
                parts.push(format!("kind:{}", k));
            }
            if let Some(f) = file {
                parts.push(format!("file:{}", make_relative_path(&f, cwd)));
            }
            if parts.is_empty() {
                "📇 all symbols".to_string()
            } else {
                format!("📇 {}", parts.join(" "))
            }
        }

        // ═══════════════════════════════════════════════════════════════════
        // 📄 DOCUMENT (Office/PDF)
        // ═══════���═══════════════════════════════════════════════════════════
        "office_read" | "pdf_read" | "libreoffice_read" => {
            let path = get_relative_path(&["path"]);
            format!("📄 {}", path)
        }
        "office_write" | "pdf_write" | "libreoffice_write" => {
            let path = get_relative_path(&["path"]);
            format!("📄 {}", path)
        }
        "office_info" | "libreoffice_info" => {
            let path = get_relative_path(&["path"]);
            format!("📄 {}", path)
        }

        // ═══════════════════════════════════════════════════════════════════
        // 📋 GITHUB
        // ═══════════════════════════════════════════════════════════════════
        "github_issues" => {
            let action = input.get("action").and_then(|v| v.as_str()).unwrap_or("?");
            let number = input.get("number").and_then(|v| v.as_u64());
            match (action, number) {
                ("list", _) => "📋 list issues".to_string(),
                ("create", _) => "📋 create issue".to_string(),
                ("get", Some(n)) => format!("📋 issue #{}", n),
                ("comment", Some(n)) => format!("📋 comment on #{}", n),
                ("close", Some(n)) => format!("📋 close #{}", n),
                _ => format!("📋 {} issue", action),
            }
        }
        "github_prs" => {
            let action = input.get("action").and_then(|v| v.as_str()).unwrap_or("?");
            let number = input.get("number").and_then(|v| v.as_u64());
            match (action, number) {
                ("list", _) => "📋 list PRs".to_string(),
                ("create", _) => "📋 create PR".to_string(),
                ("get", Some(n)) => format!("📋 PR #{}", n),
                ("review", Some(n)) => format!("📋 review #{}", n),
                ("merge", Some(n)) => format!("📋 merge #{}", n),
                _ => format!("📋 {} PR", action),
            }
        }

        // ══════════════════════════════════════════════════���════════════════
        // ✨ UTILITY
        // ═══════════════════════════════════════════════════════════════════
        "format" => "✨ format".to_string(),
        "metadata" => "✨ metadata".to_string(),
        "truncate" => "✨ truncate".to_string(),
        "read_line_range" => "✨ line range".to_string(),
        "memory_read" => "✨ read memory".to_string(),
        "memory_write" => "✨ write memory".to_string(),

        // ═══════════════════════════════════════════════════════════════════
        // DEFAULT: Unknown tools
        // ═══════════════════════════════════════════════════════════════════
        _ => {
            if tool.starts_with("team_") {
                format!("👥 {}", summarize_tool_args(input, 40))
            } else {
                summarize_tool_args(input, 40)
            }
        }
    }
}

/// Return a line-range label for the read tool (e.g. "lines 5-10").
///
/// Reads from the tool's output metadata which contains the actual lines read.
/// Returns `None` when metadata is not available or tool did not complete.
pub(crate) fn read_line_range(metadata: &Option<serde_json::Value>) -> Option<String> {
    let meta = metadata.as_ref()?;
    let start = meta.get("start_line").and_then(|v| v.as_u64());
    let end = meta.get("end_line").and_then(|v| v.as_u64());
    match (start, end) {
        (Some(s), Some(e)) => Some(format!("lines {}-{}", s, e)),
        (Some(s), None) => Some(format!("from line {}", s)),
        _ => None,
    }
}

/// Return inline diff stats `(+added, -removed)` for tools that support it.
///
/// Currently only the `edit`, `multiedit`, `patch`, and `str_replace_editor` tools
/// provide the necessary `old_lines` / `new_lines` metadata.
pub(crate) fn tool_inline_diff(
    tool: &str,
    output: &Option<serde_json::Value>,
) -> Option<(usize, usize)> {
    let out = output.as_ref()?;
    let tool = canonical_tool_name(tool);
    match tool {
        "edit" | "str_replace_editor" => {
            let old_lines = out.get("old_lines").and_then(|v| v.as_u64());
            let new_lines = out.get("new_lines").and_then(|v| v.as_u64());
            match (old_lines, new_lines) {
                (Some(old), Some(new)) => Some((new as usize, old as usize)),
                // create/write command: lines_added = lines written, none removed
                _ => {
                    let added = out.get("lines").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    if added > 0 { Some((added, 0)) } else { None }
                }
            }
        }
        "multiedit" | "patch" => {
            let added = out.get("lines_added").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let removed = out
                .get("lines_removed")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            if added > 0 || removed > 0 {
                Some((added, removed))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Generate a result summary line for a completed tool call.
/// Generate a result summary line for a completed tool call.
///
/// Uses emoji icons consistent with tool_input_summary for visual alignment.
/// Returns `None` for tools that don't produce meaningful summaries.
pub fn tool_result_summary(
    tool: &str,
    output: &Option<serde_json::Value>,
    input: &serde_json::Value,
    cwd: &str,
) -> Option<String> {
    let out = output.as_ref()?;

    // Resolve alias tool names to their canonical equivalents for display.
    let tool = canonical_tool_name(tool);

    // Convenience: count output lines from content when metadata has no explicit count.
    // Tools that return structured metadata override this below.
    let line_count = out
        .get("lines")
        .or_else(|| out.get("line_count"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    match tool {
        // ═════════════════════════════════════════════════════════���═════════
        // 📄 FILE OPERATIONS
        // ═══════════════════════════════════════════════════════════════════
        "read" => {
            let summarised = out
                .get("summarised")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let total = out.get("total_lines").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            if summarised && total > 0 {
                Some(format!(
                    "{} read (summarised, {} total)",
                    pluralize(line_count, "line", "lines"),
                    total
                ))
            } else {
                Some(format!("{} read", pluralize(line_count, "line", "lines")))
            }
        }
        "write" => {
            let path = input["path"]
                .as_str()
                .map(|p| make_relative_path(p, cwd))
                .unwrap_or_default();
            Some(format!(
                "📄 {} written to {}",
                pluralize(line_count, "line", "lines"),
                path
            ))
        }
        "create" => {
            let path = input["path"]
                .as_str()
                .map(|p| make_relative_path(p, cwd))
                .unwrap_or_default();
            Some(format!(
                "📄 {} created in {}",
                pluralize(line_count, "line", "lines"),
                path
            ))
        }
        "edit" => {
            let path = out
                .get("path")
                .and_then(|v| v.as_str())
                .or_else(|| input.get("path").and_then(|v| v.as_str()))
                .map(|p| make_relative_path(p, cwd))
                .unwrap_or_default();
            let old_lines = out.get("old_lines").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let new_lines = out.get("new_lines").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            Some(format!(
                "📄 Edited {}: {} → {} lines",
                path, old_lines, new_lines
            ))
        }
        "multiedit" => {
            // Return a brief top-level summary; per-file detail rows are rendered separately.
            let edits = out.get("edits").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let files = out.get("file_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            Some(format!(
                "{} across {}",
                pluralize(edits, "edit", "edits"),
                pluralize(files, "file", "files")
            ))
        }
        "patch" => {
            let hunks = out.get("hunks").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let files = out.get("files").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            Some(format!(
                "{} applied across {}",
                pluralize(hunks, "hunk", "hunks"),
                pluralize(files, "file", "files")
            ))
        }
        "rm" => {
            let deleted = out
                .get("deleted")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if deleted {
                Some("deleted".to_string())
            } else {
                Some("failed".to_string())
            }
        }
        "append_to_file" | "append_file" => {
            let bytes = out.get("bytes").and_then(|v| v.as_u64()).unwrap_or(0);
            if bytes > 0 {
                Some(format!("{} bytes appended", bytes))
            } else {
                Some("appended".to_string())
            }
        }
        "diff_files" => {
            let changes = out.get("changes").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            Some(format!("{} found", pluralize(changes, "change", "changes")))
        }

        // ═══════════════════════════════════════════════════════════════════
        // 📁 DIRECTORY OPERATIONS
        // ═══════════════════════════════════════════════════════════════════
        "list" => {
            let count = out.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let path = out
                .get("path")
                .and_then(|v| v.as_str())
                .or_else(|| input.get("path").and_then(|v| v.as_str()))
                .map(|p| make_relative_path(p, cwd))
                .unwrap_or_default();
            Some(format!(
                "{} in {}",
                pluralize(count, "entry", "entries"),
                path
            ))
        }
        "make_directory" | "mkdir" => Some("directory created".to_string()),
        // ═══════════════════════════════════════════════════════════════════
        // ℹ️ FILE INFO
        // ═══════════════════════════════════════════════════════════════════
        "file_info" => {
            let kind = out.get("kind").and_then(|v| v.as_str()).unwrap_or("file");
            let size = out.get("size").and_then(|v| v.as_u64());
            match size {
                Some(s) if s >= 1024 * 1024 => Some(format!(
                    "{} ({:.1} MiB)",
                    kind,
                    s as f64 / (1024.0 * 1024.0)
                )),
                Some(s) if s >= 1024 => Some(format!("{} ({:.1} KiB)", kind, s as f64 / 1024.0)),
                Some(s) => Some(format!("{} ({} bytes)", kind, s)),
                None => Some(format!("{}", kind)),
            }
        }

        // ════════════════════════════════════════════════════════════════���══
        // 🔄 FILE MOVE/COPY
        // ═══════════════════════════════════════════════════════════════════
        "move_file" | "rename_file" => Some("moved".to_string()),
        "copy_file" => {
            let bytes = out.get("bytes").and_then(|v| v.as_u64()).unwrap_or(0);
            if bytes > 0 {
                Some(format!("{} bytes copied", bytes))
            } else {
                Some("copied".to_string())
            }
        }
        // ═══════════════════════════════════════════════════════════════════
        // 🔍 SEARCH OPERATIONS
        // ═══════════════════════════════════════════════════════════════════
        "search" => {
            let count = out.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let truncated = out
                .get("truncated")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let trunc = if truncated { "+" } else { "" };
            Some(format!("{}{} found", count, trunc))
        }
        "grep" => {
            let count = out.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let files = out.get("file_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let truncated = out
                .get("truncated")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let trunc = if truncated { "+" } else { "" };
            Some(format!(
                "{}{} matched in {} searched",
                count,
                trunc,
                pluralize(files, "file", "files")
            ))
        }
        "glob" => {
            let count = out.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let truncated = out
                .get("truncated")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let trunc = if truncated { "+" } else { "" };
            Some(format!(
                "{}{} found",
                pluralize(count, "file", "files"),
                trunc
            ))
        }

        // ════════════════════════���══════════════════════════════════════════
        // ⚡ EXECUTION
        // ═══════════════════════════════════════════════════════════════════
        "bash" => {
            let exit_code = out.get("exit_code").and_then(|v| v.as_i64());
            let timed_out = out
                .get("timed_out")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let duration_ms = out.get("duration_ms").and_then(|v| v.as_u64()).unwrap_or(0);
            if timed_out {
                Some(format!("timed out after {}ms", duration_ms))
            } else if let Some(code) = exit_code {
                Some(format!(
                    "{}… (exit {})",
                    pluralize(line_count, "line", "lines"),
                    code
                ))
            } else {
                Some(format!("{}…", pluralize(line_count, "line", "lines")))
            }
        }
        "execute_python" => {
            let exit_code = out.get("exit_code").and_then(|v| v.as_i64());
            let timed_out = out
                .get("timed_out")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let duration_ms = out.get("duration_ms").and_then(|v| v.as_u64()).unwrap_or(0);
            if timed_out {
                Some(format!("timed out after {}ms", duration_ms))
            } else if let Some(code) = exit_code {
                Some(format!(
                    "{}… (exit {})",
                    pluralize(line_count, "line", "lines"),
                    code
                ))
            } else {
                Some(format!("{}…", pluralize(line_count, "line", "lines")))
            }
        }
        "str_replace_editor" => {
            // The tool call header already shows +N/-M diff for successful edits,
            // so we don't need to duplicate that information here.
            // Return None to suppress the summary line.
            None
        }
        "calculator" => {
            let result = out.get("result").and_then(|v| v.as_str());
            result.map(|r| format!("= {}", truncate_str(r, 50)))
        }
        // ═══════════════════════════════════════════════════════════════════
        // 🌐 NETWORK
        // ═══════════════════════════════════════════════════════════════════
        "webfetch" => {
            let status = out.get("http_status").and_then(|v| v.as_u64()).unwrap_or(0);
            Some(format!(
                "{} (HTTP {})",
                pluralize(line_count, "line", "lines"),
                status
            ))
        }
        "websearch" => {
            let count = out.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            Some(format!("{} found", pluralize(count, "result", "results")))
        }
        "http_request" | "web_request" => {
            let status = out.get("http_status").and_then(|v| v.as_u64()).unwrap_or(0);
            Some(format!(
                "HTTP {} ({})",
                status,
                pluralize(line_count, "line", "lines")
            ))
        }
        // ═══════════════════════════════════════════════════════════════════
        // 🔧 ENVIRONMENT
        // ═══════════════════════════════════════════════════════════════════
        "get_env" => {
            let count = out.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            if count > 0 {
                Some(format!("{}", pluralize(count, "var", "vars")))
            } else {
                Some("not found".to_string())
            }
        }
        "bash_reset" => Some("shell reset".to_string()),
        // ═══════════════════════════════════════════════════════════════════
        // ❓ USER INTERACTION
        // ═══════════════════════════════════════════════════════════════════
        "question" | "ask_user" => {
            let response = out
                .get("response")
                .and_then(|v| v.as_str())
                .or_else(|| out.get("content").and_then(|v| v.as_str()))
                .unwrap_or("");
            Some(format!("{}", truncate_str(response, 60)))
        }
        // ═══════════════════════════════════════════════════════════════════
        // 💭 REASONING
        // ═══════════════════════════════════════════════════════════════════
        "think" => {
            // The think tool records reasoning; show a brief note.
            Some("Thinking ...".to_string())
        }
        // ═══════════════════════════════════════════════════════════════════
        // 📝 PLANNING
        // ═══════════════════════════════════════════════════════════════════
        "plan_enter" => {
            let task = out.get("task").and_then(|v| v.as_str()).unwrap_or("plan");
            Some(format!("delegated → {}", task))
        }
        "plan_exit" => {
            let len = out
                .get("summary_length")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            Some(format!("returned ({} chars)", len))
        }
        // ═══════════════════════════════════════════════════════════════════
        // 📋 TASK MANAGEMENT
        // ═══════════════════════════════════════════════════��═══════════════
        "todo_read" => {
            let count = out.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            Some(format!("{}", pluralize(count, "item", "items")))
        }
        "todo_write" => {
            let action = out.get("action").and_then(|v| v.as_str()).unwrap_or("?");
            let count = out.get("count").and_then(|v| v.as_u64()).unwrap_or(0);
            Some(format!("{} → {} remaining", action, count))
        }
        // ═══════════════════════════════════════════════════════════════════
        // 🤖 SUB-AGENT
        // ═══════════════════════════════════════════════════════════════════
        "new_task" => {
            let agent = out.get("agent").and_then(|v| v.as_str()).unwrap_or("?");
            let background = out
                .get("background")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let status = out
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let task_id = out
                .get("task_id")
                .and_then(|v| v.as_str())
                .map(|id| format!(" ({})", &id[..8.min(id.len())]))
                .unwrap_or_default();
            if background {
                Some(format!("spawned {} agent{}", agent, task_id))
            } else {
                Some(format!(
                    "{} agent {} → {}{}",
                    status, agent, status, task_id
                ))
            }
        }
        "cancel_task" => {
            let cancelled = out
                .get("cancelled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if cancelled {
                Some("task cancelled".to_string())
            } else {
                Some("already completed".to_string())
            }
        }
        "list_tasks" => {
            let count = out.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            Some(format!("{}", pluralize(count, "task", "tasks")))
        }
        "wait_tasks" => {
            let count = out.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let success = out
                .get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if success {
                Some(format!("waited on {} task(s)", count))
            } else {
                Some(format!("timeout waiting for {} task(s)", count))
            }
        }
        // ═══════════════════════════════════════════════════════════════════
        // 👥 TEAM COORDINATION
        // ═══════════════════════════════════════════════════════════════════
        "team_create" => {
            let name = out.get("name").and_then(|v| v.as_str()).unwrap_or("?");
            Some(format!("created {}", name))
        }
        "team_spawn" => {
            let teammate = out
                .get("teammate_name")
                .and_then(|v| v.as_str())
                .unwrap_or("?");
            Some(format!("spawned {}", teammate))
        }
        "team_status" => {
            let members = out.get("members").and_then(|v| v.as_u64()).unwrap_or(0);
            Some(format!("{} member(s)", members))
        }
        "team_idle" => {
            let idle_blocked = out
                .get("idle_blocked")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if idle_blocked {
                let task_id = out
                    .get("blocked_by_task")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                Some(format!("blocked by task '{}'", task_id))
            } else {
                Some("marked idle".to_string())
            }
        }
        "team_cleanup" => Some("cleanup complete".to_string()),
        "team_broadcast" => Some("broadcast sent".to_string()),
        "team_message" => Some("message sent".to_string()),
        "team_read_messages" => {
            let count = out.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            Some(format!("{} new", pluralize(count, "message", "messages")))
        }
        "team_task_create" => {
            let task_id = out.get("task_id").and_then(|v| v.as_str()).unwrap_or("?");
            Some(format!("created task '{}'", task_id))
        }
        "team_task_list" => {
            let count = out.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            Some(format!("{} tasks", count))
        }
        "team_task_claim" => {
            let claimed = out
                .get("claimed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let already_in_progress = out
                .get("already_in_progress")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let task_id = out
                .get("task_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            if already_in_progress {
                Some(format!("already has task '{}'", task_id))
            } else if claimed {
                Some(format!("claimed task '{}'", task_id))
            } else {
                Some("no tasks available".to_string())
            }
        }
        "team_task_complete" => {
            let completed = out
                .get("completed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let task_id = out
                .get("task_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            if completed {
                Some(format!("completed task '{}'", task_id))
            } else {
                Some("task not found or already completed".to_string())
            }
        }
        "team_assign_task" => {
            let task_id = out
                .get("task_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let to = out.get("to").and_then(|v| v.as_str()).unwrap_or("?");
            Some(format!("assigned '{}' to {}", task_id, to))
        }
        "team_submit_plan" => Some("plan submitted".to_string()),
        "team_approve_plan" => {
            let approved = out
                .get("approved")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if approved {
                Some("plan approved".to_string())
            } else {
                Some("plan rejected".to_string())
            }
        }
        "team_memory_read" => {
            let found = out.get("found").and_then(|v| v.as_bool()).unwrap_or(false);
            if found {
                Some("memory read".to_string())
            } else {
                Some("memory not found".to_string())
            }
        }
        "team_memory_write" => Some("memory written".to_string()),
        "team_wait" => {
            let count = out.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let success = out
                .get("success")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            if success {
                Some(format!("waited on {} agent(s)", count))
            } else {
                Some(format!("timeout waiting for {} agent(s)", count))
            }
        }
        "team_shutdown_teammate" => Some("shutdown requested".to_string()),
        "team_shutdown_ack" => Some("shutdown acknowledged".to_string()),
        // ═══════════════════════════════════════════════════════════════════
        // 🔎 LSP / CODE INTELLIGENCE
        // ═══════════════════════════════════════════════════════════════════
        "lsp_definition" | "lsp_references" => {
            let count = out.get("count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            Some(format!(
                "{} found",
                pluralize(count, "location", "locations")
            ))
        }
        "lsp_symbols" => {
            let count = out
                .get("symbol_count")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let path = out
                .get("path")
                .and_then(|v| v.as_str())
                .map(|p| make_relative_path(p, cwd))
                .unwrap_or_default();
            Some(format!(
                "{} in {}",
                pluralize(count, "symbol", "symbols"),
                path
            ))
        }
        "lsp_hover" => Some(format!(
            "{} of info",
            pluralize(line_count, "line", "lines")
        )),
        "lsp_diagnostics" => {
            let count = out.get("total").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            Some(format!(
                "{} found",
                pluralize(count, "diagnostic", "diagnostics")
            ))
        }
        // ═══════════════════════════════════════════════════════════════════
        // 📇 CODE INDEX
        // ═══════════════════════════════════════════════════════════════════
        "codeindex_search" => {
            let total = out
                .get("total_results")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            Some(format!(
                "{} found",
                pluralize(total, "result", "results")
            ))
        }
        "codeindex_symbols" => {
            let total = out
                .get("total_results")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            Some(format!(
                "{} found",
                pluralize(total, "symbol", "symbols")
            ))
        }
        // ═══════════════════════════════════════════════════════════════════
        // 📄 DOCUMENT (Office/PDF)
        // ═════════════════════��═════════════════════════════════════════════
        "office_read" | "pdf_read" | "libreoffice_read" => {
            Some(format!("{} read", pluralize(line_count, "line", "lines")))
        }
        "office_write" | "pdf_write" | "libreoffice_write" => {
            let path = input["path"]
                .as_str()
                .map(|p| make_relative_path(p, cwd))
                .unwrap_or_default();
            Some(format!(
                "{} written to {}",
                pluralize(line_count, "line", "lines"),
                path
            ))
        }
        "office_info" | "libreoffice_info" => Some(format!(
            "{} of metadata",
            pluralize(line_count, "line", "lines")
        )),
        // ═══════════════════════════════════════════════════════════════════
        // 📋 GITHUB
        // ═════════════════════════════════════════════════════════��═════════
        "github_issues" => {
            let action = out.get("action").and_then(|v| v.as_str()).unwrap_or("?");
            let number = out.get("number").and_then(|v| v.as_u64());
            match (action, number) {
                ("list", _) => Some("issues listed".to_string()),
                ("create", _) => Some("issue created".to_string()),
                ("get", Some(n)) => Some(format!("issue #{} retrieved", n)),
                ("comment", Some(n)) => Some(format!("commented on #{}", n)),
                ("close", Some(n)) => Some(format!("issue #{} closed", n)),
                _ => Some(format!("{} issue", action)),
            }
        }
        "github_prs" => {
            let action = out.get("action").and_then(|v| v.as_str()).unwrap_or("?");
            let number = out.get("number").and_then(|v| v.as_u64());
            match (action, number) {
                ("list", _) => Some("PRs listed".to_string()),
                ("create", _) => Some("PR created".to_string()),
                ("get", Some(n)) => Some(format!("PR #{} retrieved", n)),
                ("review", Some(n)) => Some(format!("reviewed #{}", n)),
                ("merge", Some(n)) => Some(format!("PR #{} merged", n)),
                _ => Some(format!("{} PR", action)),
            }
        }
        // ═══════════════════════════════════════════════════════════════════
        // ✨ UTILITY
        // ═══════════════════════════════════════════════════════════════════
        "format" => Some("formatted".to_string()),
        "metadata" => Some("metadata extracted".to_string()),
        "truncate" => {
            let original = out.get("original").and_then(|v| v.as_u64()).unwrap_or(0);
            let truncated = out.get("truncated").and_then(|v| v.as_u64()).unwrap_or(0);
            Some(format!("{} → {} chars", original, truncated))
        }
        "read_line_range" => {
            let start = out.get("start_line").and_then(|v| v.as_u64());
            let end = out.get("end_line").and_then(|v| v.as_u64());
            match (start, end) {
                (Some(s), Some(e)) => Some(format!("lines {}-{}", s, e)),
                (Some(s), None) => Some(format!("from line {}", s)),
                _ => Some("line range".to_string()),
            }
        }
        "memory_read" => {
            let found = out.get("found").and_then(|v| v.as_bool()).unwrap_or(false);
            if found {
                Some("memory read".to_string())
            } else {
                Some("memory not found".to_string())
            }
        }
        "memory_write" => Some("memory written".to_string()),
        // ═══════════════════════════════════════════════════════════════════
        // DEFAULT: Unknown tools
        // ═══════════════════════════════════════════════════════════════════
        _ => {
            if tool.starts_with("team_") {
                Some(format!(
                    "{}",
                    out.get("status").and_then(|v| v.as_str()).unwrap_or("done")
                ))
            } else {
                None
            }
        }
    }
}

/// A widget that renders a single `Message` as a list of styled lines.
pub struct MessageWidget<'a> {
    message: &'a Message,
    cwd: &'a str,
    tool_step_map: &'a std::collections::HashMap<String, (String, u32, u32)>,
}

impl<'a> MessageWidget<'a> {
    /// Create a new [`MessageWidget`] for the given message reference.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to render.
    /// * `cwd` - Current working directory, used to make file paths relative.
    /// * `tool_step_map` - Mapping from tool call IDs to `(short_session_id, step_number, sub_step)`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::collections::HashMap;
    /// use ragent_core::message::{Message, MessagePart, Role};
    /// use ragent_tui::widgets::message_widget::MessageWidget;
    ///
    /// let msg = Message::new(
    ///     "session-1",
    ///     Role::User,
    ///     vec![MessagePart::Text { text: "Hello!".into() }],
    /// );
    /// let map = HashMap::new();
    /// let widget = MessageWidget::new(&msg, "/home/user/project", &map);
    /// ```
    pub fn new(
        message: &'a Message,
        cwd: &'a str,
        tool_step_map: &'a std::collections::HashMap<String, (String, u32, u32)>,
    ) -> Self {
        Self {
            message,
            cwd,
            tool_step_map,
        }
    }

    fn to_lines(&self) -> Vec<Line<'a>> {
        let mut lines = Vec::new();

        for part in &self.message.parts {
            match part {
                MessagePart::Text { text } => {
                    // Detect the model "thinking out loud" with minimal text like "..." or "......"
                    // and render it as a distinct greyed-out thinking indicator.
                    let trimmed = text.trim();
                    let is_thinking_placeholder = trimmed.chars().all(|c| c == '.' || c == ' ')
                        && !trimmed.is_empty()
                        && trimmed.len() <= 6;
                    if is_thinking_placeholder && self.message.role == Role::Assistant {
                        lines.push(Line::from(vec![
                            Span::styled("💭 ", Style::default().fg(Color::DarkGray)),
                            Span::styled(
                                "Thinking ...",
                                Style::default()
                                    .fg(Color::DarkGray)
                                    .add_modifier(Modifier::ITALIC),
                            ),
                        ]));
                        continue;
                    }
                    let (dot, dot_style, indent) = match self.message.role {
                        Role::User => (
                            "You: ",
                            Style::default()
                                .fg(Color::Cyan)
                                .add_modifier(Modifier::BOLD),
                            5,
                        ),
                        Role::Assistant => (
                            "● ",
                            Style::default()
                                .fg(Color::Magenta)
                                .add_modifier(Modifier::BOLD),
                            2,
                        ),
                    };
                    for (i, line) in text.lines().enumerate() {
                        if i == 0 {
                            lines.push(Line::from(vec![
                                Span::styled(dot, dot_style),
                                Span::raw(line.to_string()),
                            ]));
                        } else {
                            lines.push(Line::from(Span::raw(format!(
                                "{}{}",
                                " ".repeat(indent),
                                line
                            ))));
                        }
                    }
                }
                MessagePart::ToolCall {
                    tool,
                    call_id,
                    state,
                } => {
                    let step_tag = self
                        .tool_step_map
                        .get(call_id)
                        .map(|(sid, step, substep)| format!("[{sid}:{step}.{substep}] "))
                        .unwrap_or_default();
                    let (indicator, ind_style, name_style) = match state.status {
                        ToolCallStatus::Completed => (
                            "● ",
                            Style::default().fg(Color::Green),
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ),
                        ToolCallStatus::Error => (
                            "✗ ",
                            Style::default().fg(Color::Red),
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        ),
                        ToolCallStatus::Running | ToolCallStatus::Pending => (
                            "● ",
                            Style::default().fg(Color::DarkGray),
                            Style::default().fg(Color::DarkGray),
                        ),
                    };

                    let display_name = capitalize_tool_name(tool);
                    let summary = tool_input_summary(tool, &state.input, self.cwd);

                    // Build the inline diff stats for edit tool (e.g. "(+25 -5)")
                    let inline_diff = if state.status == ToolCallStatus::Completed {
                        tool_inline_diff(tool, &state.output)
                    } else {
                        None
                    };

                    let mut spans = vec![
                        Span::styled(indicator, ind_style),
                        Span::styled(
                            step_tag,
                            Style::default()
                                .fg(Color::Yellow)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ];
                    if summary.is_empty() {
                        spans.push(Span::styled(display_name, name_style));
                    } else {
                        spans.push(Span::styled(format!("{} ", display_name), name_style));
                        spans.push(Span::styled(summary, Style::default().fg(Color::DarkGray)));
                    }
                    // Show line range for read tool (and aliases) in bold
                    if canonical_tool_name(tool) == "read" {
                        if let Some(range) = read_line_range(&state.output) {
                            spans.push(Span::styled(
                                format!(" {}", range),
                                Style::default()
                                    .fg(Color::White)
                                    .add_modifier(Modifier::BOLD),
                            ));
                        }
                    }
                    if let Some((added, removed)) = inline_diff {
                        spans.push(Span::styled(" (", Style::default().fg(Color::DarkGray)));
                        spans.push(Span::styled(
                            format!("+{}", added),
                            Style::default().fg(Color::Green),
                        ));
                        spans.push(Span::styled(" ", Style::default().fg(Color::DarkGray)));
                        spans.push(Span::styled(
                            format!("-{}", removed),
                            Style::default().fg(Color::Red),
                        ));
                        spans.push(Span::styled(")", Style::default().fg(Color::DarkGray)));
                    }
                    lines.push(Line::from(spans));

                    if state.status == ToolCallStatus::Completed {
                        if tool == "wait_tasks" {
                            // Special handling for wait_tasks: show indented list of tasks
                            if let Some(tasks_array) = state
                                .output
                                .as_ref()
                                .and_then(|out| out.get("tasks"))
                                .and_then(|t| t.as_array())
                            {
                                for task in tasks_array {
                                    let agent = task
                                        .get("agent")
                                        .and_then(|a| a.as_str())
                                        .unwrap_or("unknown");
                                    let task_id = task
                                        .get("id")
                                        .and_then(|id| id.as_str())
                                        .map(|id| &id[..8.min(id.len())])
                                        .unwrap_or("unknown");
                                    let elapsed_ms = task
                                        .get("elapsed_ms")
                                        .and_then(|e| e.as_u64())
                                        .unwrap_or(0);
                                    let output_lines = task
                                        .get("output_lines")
                                        .and_then(|l| l.as_u64())
                                        .unwrap_or(0)
                                        as usize;
                                    let task_status = task
                                        .get("status")
                                        .and_then(|s| s.as_str())
                                        .unwrap_or("unknown");

                                    let status_icon = if task_status == "completed" {
                                        "✓"
                                    } else {
                                        "✗"
                                    };

                                    let elapsed_sec = elapsed_ms as f64 / 1000.0;
                                    let task_line = format!(
                                        "  {} {} ({}): {}s, {} line(s)",
                                        status_icon, agent, task_id, elapsed_sec, output_lines
                                    );

                                    lines.push(Line::from(Span::styled(
                                        task_line,
                                        if task_status == "completed" {
                                            Style::default().fg(Color::Green)
                                        } else {
                                            Style::default().fg(Color::Red)
                                        },
                                    )));
                                }
                            }
                        } else if tool == "multiedit" {
                            // Render per-file edit stats as a tabular list
                            if let Some(file_stats) = state
                                .output
                                .as_ref()
                                .and_then(|out| out.get("file_stats"))
                                .and_then(|v| v.as_array())
                            {
                                // Find the longest relative path for alignment
                                let rel_paths: Vec<String> = file_stats
                                    .iter()
                                    .map(|fs| {
                                        fs.get("path")
                                            .and_then(|p| p.as_str())
                                            .map(|p| make_relative_path(p, self.cwd))
                                            .unwrap_or_default()
                                    })
                                    .collect();
                                let max_len = rel_paths.iter().map(|p| p.len()).max().unwrap_or(0);
                                for (fs, rel_path) in file_stats.iter().zip(rel_paths.iter()) {
                                    let added =
                                        fs.get("added").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let removed =
                                        fs.get("removed").and_then(|v| v.as_u64()).unwrap_or(0);
                                    let padding =
                                        " ".repeat(max_len.saturating_sub(rel_path.len()));
                                    lines.push(Line::from(vec![
                                        Span::styled(
                                            format!("  └ {}{} ", rel_path, padding),
                                            Style::default().fg(Color::DarkGray),
                                        ),
                                        Span::styled(
                                            format!("+{}", added),
                                            Style::default().fg(Color::Green),
                                        ),
                                        Span::styled(" ", Style::default()),
                                        Span::styled(
                                            format!("-{}", removed),
                                            Style::default().fg(Color::Red),
                                        ),
                                    ]));
                                }
                            } else if let Some(result) =
                                tool_result_summary(tool, &state.output, &state.input, self.cwd)
                            {
                                lines.push(Line::from(Span::styled(
                                    format!("  └ {}", result),
                                    Style::default().fg(Color::DarkGray),
                                )));
                            }
                        } else if tool != "edit" {
                            // Skip result summary for edit tool on success (already shows inline diff)
                            if let Some(result) =
                                tool_result_summary(tool, &state.output, &state.input, self.cwd)
                            {
                                lines.push(Line::from(Span::styled(
                                    format!("  └ {}", result),
                                    Style::default().fg(Color::DarkGray),
                                )));
                            }
                        }
                    }
                    if state.status == ToolCallStatus::Error {
                        let err_msg = state
                            .error
                            .as_deref()
                            .unwrap_or("Tool execution failed (no error details available)");
                        lines.push(Line::from(Span::styled(
                            format!("  └ Error: {}", err_msg),
                            Style::default().fg(Color::Red),
                        )));
                    }
                }
                MessagePart::Reasoning { text } => {
                    for line in text.lines() {
                        lines.push(Line::from(Span::styled(
                            format!("  💭 {}", line),
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::ITALIC),
                        )));
                    }
                }
                MessagePart::Image { path, .. } => {
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("image");
                    lines.push(Line::from(Span::styled(
                        format!("  📎 [image: {}]", name),
                        Style::default().fg(Color::Yellow),
                    )));
                }
            }
        }

        lines
    }
}

impl Widget for MessageWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines = self.to_lines();
        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::tool_input_summary;
    use serde_json::json;

    #[test]
    fn test_team_tool_summary_includes_args() {
        let input = json!({
            "team_name": "alpha",
            "teammate_name": "reviewer-1",
            "agent_type": "general"
        });
        let summary = tool_input_summary("team_spawn", &input, "/tmp");
        // New format: "👥 spawn {agent_type}"
        assert!(summary.contains("👥 spawn"));
        assert!(summary.contains("general"));
    }

    #[test]
    fn test_team_tool_summary_truncates_long_strings_with_three_dots() {
        let long = "x".repeat(60);
        let input = json!({
            "team_name": "alpha",
            "content": long
        });
        let summary = tool_input_summary("team_broadcast", &input, "/tmp");
        // New format: "👥 broadcast: {content}" with truncation
        assert!(summary.contains("👥 broadcast:"));
        // The string should be truncated (50 chars + "...")
        assert!(
            summary.len() <= 70, // "👥 broadcast: " is ~14 chars + 50 + "..." = ~67
            "summary should be truncated, got: {summary} (len: {})",
            summary.len()
        );
    }

    #[test]
    fn test_unknown_tool_summary_includes_args() {
        let input = json!({
            "path": "/tmp/file.txt",
            "limit": 10
        });
        let summary = tool_input_summary("some_new_tool", &input, "/tmp");
        assert!(summary.contains("path=\"/tmp/file.txt\""));
        assert!(summary.contains("limit=10"));
    }

    #[test]
    fn test_unknown_tool_summary_truncates_long_strings() {
        let input = json!({
            "note": "abcdefghijklmnopqrstuvwxyz0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"
        });
        let summary = tool_input_summary("some_new_tool", &input, "/tmp");
        assert!(summary.contains("note=\""));
        assert!(
            summary.contains("...\""),
            "summary should truncate with three dots: {summary}"
        );
    }
}
